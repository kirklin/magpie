//! Windows platform adapters.
//!
//! Clipboard content goes through arboard (CF_UNICODETEXT, "HTML Format",
//! PNG/CF_DIBV5 and CF_HDROP); change detection uses the clipboard sequence
//! number, the focused window comes from `GetForegroundWindow`, paste-back is a
//! `SendInput` Ctrl+V, and icons come from the shell.

use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;

use ::windows::core::{BOOL, PCWSTR, PWSTR};
use ::windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, SIZE};
use ::windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use ::windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use ::windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use ::windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
use ::windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use ::windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, VIRTUAL_KEY, VK_CONTROL, VK_LMENU,
    VK_LSHIFT, VK_LWIN, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_V,
};
use ::windows::Win32::UI::Shell::{
    IShellItemImageFactory, ILFree, SHCreateItemFromIDList, SHCreateItemFromParsingName,
    SHSimpleIDListFromPath, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GetAncestor, GetForegroundWindow, GetWindowThreadProcessId, IsIconic,
    IsWindow, SetForegroundWindow, ShowWindow, GA_ROOTOWNER, SW_RESTORE,
};
use tauri::{AppHandle, Manager};

use super::{
    AppInfo, Captured, Clipboard, FocusedWindow, Icons, Paster, PasterCapabilities, WritePayload,
};

// ---------------------------------------------------------------------------
// Clipboard
// ---------------------------------------------------------------------------

pub struct WinClipboard {
    inner: Mutex<arboard::Clipboard>,
}

impl WinClipboard {
    pub fn new() -> Result<Self, String> {
        let inner = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        Ok(Self { inner: Mutex::new(inner) })
    }
}

impl Clipboard for WinClipboard {
    fn change_token(&self) -> Option<i64> {
        // 0 means this process may not access the clipboard right now (e.g. the
        // session is locked): no reliable reading this tick.
        let seq = unsafe { GetClipboardSequenceNumber() };
        (seq != 0).then_some(i64::from(seq))
    }

    fn read(&self) -> Result<Option<Captured>, String> {
        if is_concealed()? {
            log::debug!("Skipping clipboard content marked as sensitive");
            return Ok(None);
        }

        let mut clipboard = self.inner.lock().expect("clipboard lock poisoned");

        // Files first: Explorer's file copy has no text flavor, but other apps
        // put CF_HDROP next to one.
        match clipboard.get().file_list() {
            Ok(paths) if !paths.is_empty() => {
                let paths = paths.into_iter().map(|p| p.to_string_lossy().into_owned()).collect();
                return Ok(Some(Captured::Files { paths }));
            }
            Ok(_) | Err(arboard::Error::ContentNotAvailable) => {}
            Err(e) => return Err(read_error(e)),
        }

        match clipboard.get_text() {
            Ok(text) if !text.is_empty() => return Ok(Some(Captured::Text { text, html: None })),
            Ok(_) | Err(arboard::Error::ContentNotAvailable) => {}
            Err(e) => return Err(read_error(e)),
        }

        // Rich content that exposes only an HTML flavor with no plain-text
        // representation: keep the HTML and a stripped plain-text version for
        // display/search.
        if html_available() {
            let html = clipboard.get().html().map_err(read_error)?;
            let plain = super::html_to_plain_text(&html);
            if !plain.is_empty() {
                return Ok(Some(Captured::Text { text: plain, html: Some(html) }));
            }
        }

        match clipboard.get_image() {
            Ok(image) if !image.bytes.is_empty() => {
                return Ok(Some(Captured::Image {
                    rgba: image.bytes.into_owned(),
                    width: image.width as u32,
                    height: image.height as u32,
                }));
            }
            Ok(_) | Err(arboard::Error::ContentNotAvailable) => {}
            Err(e) => return Err(read_error(e)),
        }

        log::debug!("Clipboard changed but no recognizable content found");
        Ok(None)
    }

    fn write(&self, payload: &WritePayload) -> Result<(), String> {
        let mut clipboard = self.inner.lock().expect("clipboard lock poisoned");
        let result = match payload {
            WritePayload::Text(text) => clipboard.set_text(text.as_str()),
            WritePayload::ImageFile(path) => {
                let (rgba, width, height) = super::decode_png_rgba(path)?;
                clipboard.set_image(arboard::ImageData {
                    width: width as usize,
                    height: height as usize,
                    bytes: rgba.into(),
                })
            }
            WritePayload::Files(paths) => clipboard.set().file_list(paths),
        };
        result.map_err(|e| format!("Failed to write to clipboard: {e}"))
    }
}

fn read_error(e: arboard::Error) -> String {
    format!("Failed to read the clipboard: {e}")
}

/// Registered formats that password managers place next to a secret to keep
/// it out of clipboard histories.
///
/// `ExcludeClipboardContentFromMonitorProcessing` and a zero
/// `CanIncludeInClipboardHistory` are Windows' own markers (honoured by the
/// built-in clipboard history); `Clipboard Viewer Ignore` is the older
/// convention third-party clipboard managers agreed on.
fn is_concealed() -> Result<bool, String> {
    let present = |name: &str| {
        clipboard_win::register_format(name).is_some_and(|f| clipboard_win::is_format_avail(f.get()))
    };
    if present("ExcludeClipboardContentFromMonitorProcessing") || present("Clipboard Viewer Ignore") {
        return Ok(true);
    }

    let Some(history) = clipboard_win::register_format("CanIncludeInClipboardHistory") else {
        return Ok(false);
    };
    if !clipboard_win::is_format_avail(history.get()) {
        return Ok(false);
    }
    let _open = clipboard_win::Clipboard::new_attempts(10)
        .map_err(|e| format!("Clipboard is busy: {e}"))?;
    let mut value = Vec::new();
    clipboard_win::raw::get_vec(history.get(), &mut value)
        .map_err(|e| format!("Failed to read CanIncludeInClipboardHistory: {e}"))?;
    Ok(value.len() >= 4 && u32::from_ne_bytes([value[0], value[1], value[2], value[3]]) == 0)
}

fn html_available() -> bool {
    clipboard_win::register_format("HTML Format").is_some_and(|f| clipboard_win::is_format_avail(f.get()))
}

// ---------------------------------------------------------------------------
// Paster
// ---------------------------------------------------------------------------

pub struct WinPaster {
    app: AppHandle,
    /// Executable path -> display name, so each capture doesn't re-read the
    /// executable's version resource.
    names: Mutex<HashMap<String, Option<String>>>,
}

impl WinPaster {
    pub fn new(app: AppHandle) -> Self {
        Self { app, names: Mutex::new(HashMap::new()) }
    }

    fn display_name(&self, exe: &str) -> Option<String> {
        let mut names = self.names.lock().expect("app name cache poisoned");
        names
            .entry(exe.to_string())
            .or_insert_with(|| {
                file_description(exe).or_else(|| {
                    std::path::Path::new(exe).file_stem().map(|s| s.to_string_lossy().into_owned())
                })
            })
            .clone()
    }
}

impl Paster for WinPaster {
    fn focused_window(&self) -> FocusedWindow {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_invalid() {
            return FocusedWindow::default();
        }
        FocusedWindow { pid: app_process(hwnd), window: Some(hwnd.0 as usize as u64) }
    }

    fn frontmost_app(&self) -> AppInfo {
        let focus = self.focused_window();
        let app_id = focus.pid.and_then(process_image_path);
        let name = app_id.as_deref().and_then(|exe| self.display_name(exe));
        AppInfo { app_id, name, focus }
    }

    fn activate(&self, target: &AppInfo) -> bool {
        let Some(window) = target.focus.window else {
            return false;
        };
        let hwnd = HWND(window as usize as *mut c_void);
        unsafe {
            if !IsWindow(Some(hwnd)).as_bool() {
                return false;
            }
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            // Allowed without tricks: Magpie is the foreground process (or at
            // least received the last input, the key or click that asked for
            // the paste), so it may hand the foreground to another window.
            SetForegroundWindow(hwnd).as_bool()
        }
    }

    fn hide_and_restore_focus(&self, previous: Option<&AppInfo>) -> Result<(), String> {
        // Hand the foreground over while Magpie still holds it, which Windows
        // always allows; hiding first would let it pick the next window in
        // z-order, which is not necessarily the one Magpie was summoned from.
        if let Some(previous) = previous
            && !self.activate(previous)
        {
            log::warn!("Could not re-activate {:?} before paste", previous.name);
        }
        if let Some(window) = self.app.get_webview_window("main") {
            window.hide().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn refocus_magpie(&self) {
        if let Some(window) = self.app.get_webview_window("main") {
            let _ = window.set_focus();
        }
    }

    fn paste(&self) -> Result<(), String> {
        let mut inputs = vec![key_input(VK_CONTROL, false)];
        // Modifiers still held from the key that triggered the paste (Shift for
        // "paste as plain text", Alt for "paste and keep window") would turn
        // Ctrl+V into a different shortcut in the target app. Release them
        // after Ctrl is down, so a lone Alt/Win release can't open a menu.
        for vk in [VK_LSHIFT, VK_RSHIFT, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN] {
            if unsafe { GetAsyncKeyState(i32::from(vk.0)) } < 0 {
                inputs.push(key_input(vk, true));
            }
        }
        inputs.push(key_input(VK_V, false));
        inputs.push(key_input(VK_V, true));
        inputs.push(key_input(VK_CONTROL, true));

        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize != inputs.len() {
            return Err(format!(
                "SendInput delivered {sent} of {} key events: {}",
                inputs.len(),
                std::io::Error::last_os_error()
            ));
        }
        log::debug!("[Paste] Simulated Ctrl+V via SendInput");
        Ok(())
    }

    fn capabilities(&self) -> PasterCapabilities {
        PasterCapabilities { can_paste: true, can_activate_app: true, can_read_focus: true }
    }
}

/// Whether the foreground window is Magpie's own window `hwnd` or one it owns
/// (a file dialog). Losing keyboard focus while that holds is internal: WebView2
/// drops and retakes focus within a single message when a window drag starts
/// and ends (tauri-apps/tauri#10767).
pub fn foreground_is_own(hwnd: isize) -> bool {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.0.is_null() {
            return false;
        }
        foreground.0 as isize == hwnd || GetAncestor(foreground, GA_ROOTOWNER).0 as isize == hwnd
    }
}

fn key_input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    let scan = unsafe { MapVirtualKeyW(u32::from(vk.0), MAPVK_VK_TO_VSC) } as u16;
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// The process behind a top-level window. UWP apps (Calculator, Settings…)
/// are framed by ApplicationFrameHost.exe; the app itself owns a child window.
fn app_process(hwnd: HWND) -> Option<u32> {
    let pid = window_pid(hwnd)?;
    let is_frame_host = process_image_path(pid).is_some_and(|exe| {
        std::path::Path::new(&exe)
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("ApplicationFrameHost.exe"))
    });
    if !is_frame_host {
        return Some(pid);
    }

    struct Search {
        host: u32,
        found: Option<u32>,
    }
    unsafe extern "system" fn visit(child: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam.0 as *mut Search) };
        match window_pid(child) {
            Some(pid) if pid != search.host => {
                search.found = Some(pid);
                BOOL(0)
            }
            _ => BOOL(1),
        }
    }
    let mut search = Search { host: pid, found: None };
    unsafe {
        let _ = EnumChildWindows(Some(hwnd), Some(visit), LPARAM(&mut search as *mut Search as isize));
    }
    Some(search.found.unwrap_or(pid))
}

fn window_pid(hwnd: HWND) -> Option<u32> {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    (pid != 0).then_some(pid)
}

/// Full path of a process's executable.
fn process_image_path(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let result = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(process);
        result.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// The "File description" from an executable's version resource — the name
/// Task Manager shows (e.g. "Notepad", "Google Chrome").
fn file_description(exe: &str) -> Option<String> {
    let path = wide(exe);
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(path.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut block = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(path.as_ptr()), None, size, block.as_mut_ptr().cast()).ok()?;

        let query = |sub_block: &str| -> Option<(*const u16, usize)> {
            let sub_block = wide(sub_block);
            let mut value: *mut c_void = std::ptr::null_mut();
            let mut len = 0u32;
            VerQueryValueW(block.as_ptr().cast(), PCWSTR(sub_block.as_ptr()), &mut value, &mut len)
                .as_bool()
                .then_some((value as *const u16, len as usize))
                .filter(|(ptr, len)| !ptr.is_null() && *len > 0)
        };

        // Pairs of (language, code page) the strings are stored under.
        let (table, table_bytes) = query("\\VarFileInfo\\Translation")?;
        let table = std::slice::from_raw_parts(table, table_bytes / 2);
        table.as_chunks::<2>().0.iter().find_map(|[language, code_page]| {
            let (text, chars) =
                query(&format!("\\StringFileInfo\\{language:04x}{code_page:04x}\\FileDescription"))?;
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(text, chars));
            let text = text.trim_end_matches('\0').trim().to_string();
            (!text.is_empty()).then_some(text)
        })
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ---------------------------------------------------------------------------
// Icons
// ---------------------------------------------------------------------------

pub struct WinIcons;

impl Icons for WinIcons {
    fn app_icon(&self, app_id: &str) -> Result<String, String> {
        shell_icon(app_id, 32)
    }

    fn file_icon(&self, path: &str) -> Result<String, String> {
        shell_icon(path, 128)
    }
}

/// The icon Explorer shows for `path`, rendered at `size` pixels.
fn shell_icon(path: &str, size: i32) -> Result<String, String> {
    ensure_com();
    let path_w = wide(path);
    let factory: IShellItemImageFactory = unsafe {
        if std::path::Path::new(path).exists() {
            SHCreateItemFromParsingName(PCWSTR(path_w.as_ptr()), None).map_err(|e| e.to_string())?
        } else {
            // A file copied earlier and deleted since still gets the icon of its
            // type: a simple ID list is built from the name alone.
            let pidl = SHSimpleIDListFromPath(PCWSTR(path_w.as_ptr()));
            if pidl.is_null() {
                return Err(format!("No shell item for {path}"));
            }
            let item = SHCreateItemFromIDList(pidl);
            ILFree(Some(pidl));
            item.map_err(|e| e.to_string())?
        }
    };
    let bitmap = unsafe {
        factory
            .GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK)
            .map_err(|e| e.to_string())?
    };
    let pixels = bitmap_rgba(bitmap);
    unsafe {
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
    }
    let (rgba, width, height) = pixels?;
    Ok(super::png_data_url(&super::encode_png(&rgba, width, height)?))
}

/// Copy a 32-bit shell bitmap out as straight-alpha RGBA. The shell hands out
/// premultiplied BGRA (it is meant for `AlphaBlend`), which PNG can't store.
fn bitmap_rgba(bitmap: HBITMAP) -> Result<(Vec<u8>, u32, u32), String> {
    unsafe {
        let mut info = BITMAP::default();
        if GetObjectW(HGDIOBJ(bitmap.0), std::mem::size_of::<BITMAP>() as i32, Some((&mut info as *mut BITMAP).cast())) == 0 {
            return Err("GetObject failed on the icon bitmap".to_string());
        }
        let (width, height) = (info.bmWidth, info.bmHeight.abs());

        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bgra = vec![0u8; (width * height * 4) as usize];
        let dc = CreateCompatibleDC(None);
        let lines = GetDIBits(dc, bitmap, 0, height as u32, Some(bgra.as_mut_ptr().cast()), &mut header, DIB_RGB_COLORS);
        let _ = DeleteDC(dc);
        if lines != height {
            return Err("GetDIBits failed on the icon bitmap".to_string());
        }

        for px in bgra.as_chunks_mut::<4>().0 {
            let [b, g, r, a] = *px;
            let unmultiply = |c: u8| if a == 0 { 0 } else { ((u32::from(c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8 };
            *px = [unmultiply(r), unmultiply(g), unmultiply(b), a];
        }
        Ok((bgra, width as u32, height as u32))
    }
}

/// Shell item APIs need COM on the calling thread. Icons are fetched on the
/// blocking pool, whose threads are reused, so initialize each thread once.
fn ensure_com() {
    thread_local! {
        static INITIALIZED: Cell<bool> = const { Cell::new(false) };
    }
    INITIALIZED.with(|done| {
        if !done.get() {
            // S_FALSE (already initialized) and RPC_E_CHANGED_MODE (already in
            // the multithreaded apartment) both leave COM usable.
            let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            done.set(true);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_a_system_executable() {
        let notepad = std::env::var("WINDIR").map(|w| format!("{w}\\System32\\notepad.exe")).unwrap();
        assert!(file_description(&notepad).is_some_and(|d| !d.is_empty()));
    }

    #[test]
    fn renders_file_icons_including_for_missing_files() {
        let windir = std::env::var("WINDIR").unwrap();
        for path in [format!("{windir}\\System32\\notepad.exe"), format!("{windir}\\no-such-file.txt")] {
            let url = shell_icon(&path, 32).unwrap();
            assert!(url.starts_with("data:image/png;base64,"), "{path}");
        }
    }
}
