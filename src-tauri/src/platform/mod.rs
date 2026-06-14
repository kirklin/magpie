//! Platform port layer.
//!
//! Everything that must talk to the operating system to capture or paste the
//! clipboard lives behind two narrow traits:
//!
//! - [`Clipboard`] — read / write the OS clipboard.
//! - [`Paster`] — the active application and synthetic paste-back (⌘/Ctrl+V).
//!
//! The rest of Magpie (the monitor loop, the classifier, the database, the IPC
//! commands, the UI) depends ONLY on these traits. Adding Windows or Linux is
//! therefore a matter of writing one adapter that implements them — not editing
//! the capture/store/paste logic. [`build`] is the single place that picks an
//! implementation per OS.

use std::sync::Arc;

use tauri::AppHandle;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod noop;

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

/// The frontmost application — used both for source attribution at capture time
/// and for remembering who to paste back into.
#[derive(Debug, Clone, Default)]
pub struct AppInfo {
    pub bundle_id: Option<String>,
    pub name: Option<String>,
}

/// What a platform's paster can actually do, so the UI can degrade honestly
/// (e.g. on Linux/Wayland synthetic paste may be unavailable).
///
/// Designed now, consumed once the non-macOS adapters land (P4/P5): the UI will
/// query this to show "copied — paste manually" instead of silently failing.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct PasterCapabilities {
    pub can_paste: bool,
    pub can_activate_app: bool,
}

/// Read and write the OS clipboard.
pub trait Clipboard: Send + Sync {
    /// A token that changes whenever the OS clipboard changes (macOS:
    /// `NSPasteboard.changeCount`). `None` means "couldn't read this tick" —
    /// the caller retries next poll; it does NOT mean "unchanged".
    fn change_token(&self) -> Option<i64>;

    /// Read the clipboard as one complete snapshot, or `None` when there is
    /// nothing worth storing — sensitive/concealed content (e.g. a password
    /// copied from a password manager) or unrecognized formats.
    fn read(&self) -> Option<Captured>;

    /// Put content back onto the OS clipboard. Does NOT mark the write as
    /// self-originated; the caller does that via
    /// [`crate::clipboard::monitor::mark_self_write`] so every write path is
    /// consistent.
    fn write(&self, payload: &WritePayload) -> Result<(), String>;
}

/// The active application and synthetic paste-back.
pub trait Paster: Send + Sync {
    /// The currently frontmost application.
    fn frontmost_app(&self) -> AppInfo;

    /// Bring the application identified by `app_id` (macOS: bundle id) to the
    /// foreground. Returns whether a matching running app was activated.
    fn activate_app(&self, app_id: &str) -> bool;

    /// Synthesize a paste (⌘/Ctrl+V) into the frontmost app. The content must
    /// already be on the clipboard. Errors (e.g. missing macOS Accessibility
    /// permission) bubble up so the caller can fall back to "copied — paste
    /// manually".
    fn paste(&self) -> Result<(), String>;

    /// What this platform's paster can do. Consumed by later phases to let the
    /// UI degrade honestly on platforms where synthetic paste is unavailable.
    #[allow(dead_code)]
    fn capabilities(&self) -> PasterCapabilities;
}

/// Shared, cheap-to-clone handle to the clipboard adapter (stored in Tauri
/// managed state).
pub type ClipboardPort = Arc<dyn Clipboard>;
/// Shared, cheap-to-clone handle to the paster adapter.
pub type PasterPort = Arc<dyn Paster>;

/// Build the platform adapters for the current OS. The ONLY place `#[cfg]`
/// selects an implementation.
pub fn build(app: &AppHandle) -> (ClipboardPort, PasterPort) {
    #[cfg(target_os = "macos")]
    {
        (
            Arc::new(macos::MacClipboard::new(app.clone())),
            Arc::new(macos::MacPaster::new(app.clone())),
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        (
            Arc::new(noop::NoopClipboard::new(app.clone())),
            Arc::new(noop::NoopPaster),
        )
    }
}
