//! Platform port layer.
//!
//! Everything that must talk to the operating system lives behind three narrow
//! traits:
//!
//! - [`Clipboard`] — read / write the OS clipboard.
//! - [`Paster`] — the focused application, handing focus back to it, and the
//!   synthetic paste keystroke (⌘V / Ctrl+V).
//! - [`Icons`] — application and file icons shown in the UI.
//!
//! The rest of Magpie (the monitor loop, the classifier, the database, the IPC
//! commands, the UI) depends ONLY on these traits. [`build`] is the single place
//! that picks an implementation per OS.

use std::sync::Arc;

use tauri::AppHandle;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{ensure_identity_entry, focus_moved_to_own_popup, is_wayland, GlobalShortcutPortal};
#[cfg(target_os = "windows")]
pub use windows::foreground_is_own;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
compile_error!("Magpie supports macOS, Windows and Linux");

/// Global shortcut that toggles the window until the user records their own.
///
/// Ctrl+Shift+V is the paste shortcut of every Linux terminal, so grabbing it
/// system-wide would break pasting there; Linux uses Ctrl+Alt+V instead.
#[cfg(not(target_os = "linux"))]
pub const DEFAULT_SHORTCUT: &str = "CmdOrCtrl+Shift+V";
#[cfg(target_os = "linux")]
pub const DEFAULT_SHORTCUT: &str = "Ctrl+Alt+V";

/// One complete snapshot of what is on the clipboard right now.
///
/// Returned whole by [`Clipboard::read`] so the rest of the app never pokes the
/// OS clipboard piece-by-piece. (Reading content, then separately asking "is
/// this sensitive?" and "who is frontmost?" is how source attribution drifts
/// out of sync with the content it describes.)
#[derive(Debug, Clone)]
pub enum Captured {
    /// Plain text. `html` carries the original markup when the content was
    /// captured from an HTML-only source and `text` is its stripped form.
    Text { text: String, html: Option<String> },
    /// A raw RGBA bitmap (the core encodes + writes the PNG; that is not
    /// OS-specific so it stays out of the adapter).
    Image { rgba: Vec<u8>, width: u32, height: u32 },
    /// One or more file paths copied as file URLs.
    Files { paths: Vec<String> },
}

/// What to put back onto the OS clipboard for a copy / paste action.
#[derive(Debug, Clone)]
pub enum WritePayload {
    Text(String),
    /// Path to a PNG already on disk (a clipboard-history image).
    ImageFile(String),
    Files(Vec<String>),
}

/// Identity of the focused window. Cheap to read, so it can be polled while
/// waiting for focus to move.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FocusedWindow {
    /// The process that owns the focused window.
    pub pid: Option<u32>,
    /// Native window handle (Windows `HWND`, X11 window id). macOS activates
    /// whole applications rather than windows, so it leaves this empty.
    pub window: Option<u64>,
}

impl FocusedWindow {
    /// Whether anything is known about the focused window.
    pub fn is_known(&self) -> bool {
        self.pid.is_some() || self.window.is_some()
    }

    /// Whether the focused window belongs to Magpie itself.
    pub fn is_magpie(&self) -> bool {
        self.pid == Some(std::process::id())
    }

    /// Whether `self` is the same focus target as `other`: the same window
    /// when both know it, otherwise the same process.
    pub fn same_target(&self, other: &FocusedWindow) -> bool {
        match (self.window, other.window) {
            (Some(a), Some(b)) => a == b,
            _ => self.pid.is_some() && self.pid == other.pid,
        }
    }
}

/// The frontmost application — used both for source attribution at capture time
/// and for remembering who to paste back into.
#[derive(Debug, Clone, Default)]
pub struct AppInfo {
    /// Stable identifier stored with each entry and used to look its icon up:
    /// the bundle id on macOS, the executable path on Windows, the desktop entry
    /// id on Linux (the executable path when no desktop entry matches).
    pub app_id: Option<String>,
    /// Name shown to the user ("Paste to <name>", the entry's source).
    pub name: Option<String>,
    pub focus: FocusedWindow,
}

/// What a platform's paster can actually do, so the UI can degrade honestly.
#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
pub struct PasterCapabilities {
    /// A paste keystroke can be synthesized into the focused app.
    pub can_paste: bool,
    /// Another application can be brought to the foreground, which "paste and
    /// keep window open" needs.
    pub can_activate_app: bool,
    /// The focused window can be read, so the UI can name the paste target.
    pub can_read_focus: bool,
}

/// Read and write the OS clipboard.
pub trait Clipboard: Send + Sync {
    /// A token that changes whenever ANOTHER application changes the clipboard
    /// (macOS: `NSPasteboard.changeCount`, Windows: the clipboard sequence
    /// number). `None` means "couldn't read this tick" — the caller retries next
    /// poll; it does NOT mean "unchanged".
    fn change_token(&self) -> Option<i64>;

    /// Read the clipboard as one complete snapshot, or `None` when there is
    /// nothing worth storing — sensitive/concealed content (e.g. a password
    /// copied from a password manager) or unrecognized formats. `Err` means the
    /// clipboard could not be read right now (Windows: another app still holds
    /// it open), so the caller retries on its next poll.
    fn read(&self) -> Result<Option<Captured>, String>;

    /// Put content back onto the OS clipboard. Does NOT mark the write as
    /// self-originated; the caller does that via
    /// [`crate::clipboard::monitor::mark_self_write`] so every write path is
    /// consistent.
    fn write(&self, payload: &WritePayload) -> Result<(), String>;
}

/// The focused application, focus hand-back and synthetic paste.
pub trait Paster: Send + Sync {
    /// Identity of the focused window.
    fn focused_window(&self) -> FocusedWindow;

    /// The frontmost application, with the name shown to the user.
    fn frontmost_app(&self) -> AppInfo;

    /// Bring `target` to the foreground. Returns whether it was activated.
    fn activate(&self, target: &AppInfo) -> bool;

    /// Hide Magpie so that keyboard focus returns to `previous`, the app that
    /// was frontmost when Magpie was summoned.
    fn hide_and_restore_focus(&self, previous: Option<&AppInfo>) -> Result<(), String>;

    /// Give keyboard focus back to Magpie's (visible) window after pasting
    /// into another app.
    fn refocus_magpie(&self);

    /// Synthesize a paste (⌘/Ctrl+V) into the focused app. The content must
    /// already be on the clipboard. Errors (e.g. missing macOS Accessibility
    /// permission) bubble up so the caller can tell the user to paste manually.
    fn paste(&self) -> Result<(), String>;

    /// What this platform's paster can do.
    fn capabilities(&self) -> PasterCapabilities;
}

/// Application and file icons for the UI, as `data:image/png;base64,…` URLs.
pub trait Icons: Send + Sync {
    /// The icon of the application identified by `app_id` (see
    /// [`AppInfo::app_id`]).
    fn app_icon(&self, app_id: &str) -> Result<String, String>;

    /// The icon the system file browser shows for `path`.
    fn file_icon(&self, path: &str) -> Result<String, String>;
}

/// Shared, cheap-to-clone handle to the clipboard adapter (stored in Tauri
/// managed state).
pub type ClipboardPort = Arc<dyn Clipboard>;
/// Shared, cheap-to-clone handle to the paster adapter.
pub type PasterPort = Arc<dyn Paster>;
/// Shared, cheap-to-clone handle to the icon adapter.
pub type IconsPort = Arc<dyn Icons>;

/// The adapters for the current OS.
pub struct Platform {
    pub clipboard: ClipboardPort,
    pub paster: PasterPort,
    pub icons: IconsPort,
}

/// Build the platform adapters for the current OS. The ONLY place `#[cfg]`
/// selects an implementation.
pub fn build(app: &AppHandle) -> Result<Platform, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(Platform {
            clipboard: Arc::new(macos::MacClipboard::new(app.clone())),
            paster: Arc::new(macos::MacPaster::new(app.clone())),
            icons: Arc::new(macos::MacIcons),
        })
    }
    #[cfg(target_os = "windows")]
    {
        Ok(Platform {
            clipboard: Arc::new(windows::WinClipboard::new()?),
            paster: Arc::new(windows::WinPaster::new(app.clone())),
            icons: Arc::new(windows::WinIcons),
        })
    }
    #[cfg(target_os = "linux")]
    {
        linux::build(app)
    }
}

/// Strip HTML markup down to a readable plain-text approximation, for content
/// copied from sources that offer only an HTML flavor.
pub fn html_to_plain_text(html: &str) -> String {
    use regex::Regex;

    // Drop <script>/<style> blocks entirely, then all remaining tags.
    let re = Regex::new(r"(?is)<(script|style)\b[^>]*>.*?</(script|style)>|<[^>]+>").unwrap();
    let stripped = re.replace_all(html, " ");
    let decoded = stripped
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Encode PNG bytes as a `data:` URL the webview can show directly.
pub fn png_data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64_encode(png))
}

/// Standard base64 with padding (no external dependency needed).
fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(input.len().div_ceil(3) * 4);

    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let combined = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((combined >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((combined >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(CHARS[((combined >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARS[(combined & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

/// Decode a PNG file into straight-alpha RGBA8, for the platforms whose
/// clipboard takes raw pixels rather than PNG bytes.
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn decode_png_rgba(path: &str) -> Result<(Vec<u8>, u32, u32), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Failed to read image file: {e}"))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());

    let pixels = (info.width as usize) * (info.height as usize);
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.as_chunks::<3>().0.iter().flat_map(|&[r, g, b]| [r, g, b, 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.as_chunks::<2>().0.iter().flat_map(|&[g, a]| [g, g, g, a]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("unexpanded indexed PNG".to_string()),
    };
    if rgba.len() != pixels * 4 {
        return Err("PNG pixel data has an unexpected size".to_string());
    }
    Ok((rgba, info.width, info.height))
}

/// Encode straight-alpha RGBA8 as PNG bytes.
#[cfg(any(target_os = "windows", all(test, target_os = "linux")))]
fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (input, expected) in cases {
            assert_eq!(base64_encode(input.as_bytes()), expected);
        }
    }

    #[test]
    fn focus_targets_compare_by_window_then_process() {
        let a = FocusedWindow { pid: Some(1), window: Some(10) };
        let same_window = FocusedWindow { pid: Some(1), window: Some(10) };
        let other_window_same_app = FocusedWindow { pid: Some(1), window: Some(11) };
        assert!(a.same_target(&same_window));
        assert!(!a.same_target(&other_window_same_app), "a different window of the same app is a different target");

        let app_level = FocusedWindow { pid: Some(1), window: None };
        assert!(app_level.same_target(&FocusedWindow { pid: Some(1), window: None }));
        assert!(!app_level.same_target(&FocusedWindow { pid: Some(2), window: None }));
        assert!(!FocusedWindow::default().same_target(&FocusedWindow::default()), "unknown focus never matches");
    }

    #[test]
    fn magpie_is_recognized_by_process_id() {
        assert!(FocusedWindow { pid: Some(std::process::id()), window: None }.is_magpie());
        assert!(!FocusedWindow { pid: Some(std::process::id() + 1), window: None }.is_magpie());
        assert!(!FocusedWindow::default().is_magpie());
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[test]
    fn png_round_trips_through_rgba() {
        let rgba: Vec<u8> = (0..(3 * 2 * 4)).map(|i| (i * 7) as u8).collect();
        let png = encode_png(&rgba, 3, 2).unwrap();
        let path = std::env::temp_dir().join(format!("magpie_png_rt_{}.png", std::process::id()));
        std::fs::write(&path, &png).unwrap();
        let (decoded, w, h) = decode_png_rgba(path.to_str().unwrap()).unwrap();
        assert_eq!((w, h), (3, 2));
        assert_eq!(decoded, rgba);
        let _ = std::fs::remove_file(&path);
    }
}
