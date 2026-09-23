//! Linux platform adapters.
//!
//! Each port is served by whatever the session offers:
//!
//! - Clipboard changes: the Wayland data-control protocol where the compositor
//!   offers it (KDE, wlroots-based compositors, Hyprland, niri, COSMIC);
//!   otherwise XFixes on the X display — X11 sessions, and GNOME's Wayland
//!   session, whose compositor has no data-control but bridges the clipboard to
//!   XWayland clients.
//! - Clipboard content: arboard, which picks data-control or X11 the same way.
//! - The focused window: EWMH on X11; the wlr foreign-toplevel protocol on the
//!   Wayland compositors that offer it; unknown elsewhere (GNOME, KDE).
//! - Paste: XTest on X11; the virtual-keyboard protocol on wlroots-family
//!   compositors; the RemoteDesktop portal elsewhere (GNOME, KDE).
//! - Icons: the GTK icon theme.

mod desktop;
mod portal;
mod wayland;
mod x11;

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use super::{
    AppInfo, Captured, Clipboard, FocusedWindow, Paster, PasterCapabilities, Platform, WritePayload,
};

pub use desktop::ensure_identity_entry;
pub use portal::GlobalShortcutPortal;

/// Magpie's identity towards desktop portals, and the name of the desktop
/// entry that backs it (see `desktop::ensure_identity_entry`).
pub const APP_ID: &str = "com.magpie.clipboard";

/// Whether this is a Wayland session (the X display, if any, is XWayland).
pub fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// Whether keyboard focus just moved to one of Magpie's own GTK popups (a
/// context menu), which Wayland reports as the window losing focus. Focus
/// normally comes back when the popup closes; when it went elsewhere instead
/// (the user clicked into another app), the window is hidden then, as the
/// blur would have done.
pub fn focus_moved_to_own_popup(window: &tauri::WebviewWindow) -> bool {
    use gtk::prelude::*;

    let Some(popup) = gtk::Window::list_toplevels().into_iter().find_map(|widget| {
        widget
            .downcast::<gtk::Window>()
            .ok()
            .filter(|w| w.window_type() == gtk::WindowType::Popup && w.is_visible())
    }) else {
        return false;
    };
    let window = window.clone();
    popup.connect_unmap(move |_| {
        let window = window.clone();
        glib::timeout_add_local_once(Duration::from_millis(100), move || {
            if window.gtk_window().is_ok_and(|main| !main.is_active()) {
                let _ = window.hide();
            }
        });
    });
    true
}

pub fn build(app: &AppHandle) -> Result<Platform, String> {
    let selection = Arc::new(SelectionState::default());
    let wayland = is_wayland();

    // Clipboard changes: data-control when the compositor has it, else X11.
    let data_control = if wayland { wayland::watch_data_control(Arc::clone(&selection))? } else { None };
    let targets = if let Some(protocol) = data_control {
        log::info!("[Linux] watching the clipboard through Wayland {protocol}");
        None
    } else {
        x11::watch_clipboard(Arc::clone(&selection)).map_err(|e| {
            format!("Neither Wayland data-control nor an X display is available to watch the clipboard ({e})")
        })?;
        log::info!("[Linux] watching the clipboard through XFixes");
        Some(x11::TargetsReader::new()?)
    };
    let inner = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    let clipboard = LinuxClipboard {
        inner: Mutex::new(inner),
        selection,
        targets,
        unreadable_since: Mutex::new(None),
    };

    let (focus, injector) = if wayland {
        let tracker = wayland::ToplevelTracker::connect()?.map(Arc::new);
        let injector = if wayland::virtual_keyboard_available()? {
            Injector::VirtualKeyboard
        } else if let Some(portal) = portal::RemoteDesktopPaster::start(app)? {
            Injector::Portal(portal)
        } else {
            Injector::None
        };
        (FocusSource::Wayland(tracker), injector)
    } else {
        let desktop = Arc::new(x11::X11Desktop::new()?);
        (FocusSource::X11(Arc::clone(&desktop)), Injector::XTest(desktop))
    };
    log::info!(
        "[Linux] session: {}, focus: {}, paste: {}",
        if wayland { "Wayland" } else { "X11" },
        focus.describe(),
        injector.describe()
    );

    Ok(Platform {
        clipboard: Arc::new(clipboard),
        paster: Arc::new(LinuxPaster { app: app.clone(), focus, injector, names: Mutex::default() }),
        icons: Arc::new(desktop::LinuxIcons::new(app.clone())),
    })
}

// ---------------------------------------------------------------------------
// Clipboard
// ---------------------------------------------------------------------------

/// Clipboard changes seen by the watcher thread.
#[derive(Default)]
pub struct SelectionState {
    changes: Mutex<i64>,
    changed: Condvar,
    /// MIME types of the current selection, when the watcher learns them from
    /// its events (data-control). X11 asks the owner for TARGETS instead.
    mime_types: Mutex<Vec<String>>,
}

impl SelectionState {
    fn bump(&self) {
        *self.changes.lock().expect("selection lock poisoned") += 1;
        self.changed.notify_all();
    }

    fn changes(&self) -> i64 {
        *self.changes.lock().expect("selection lock poisoned")
    }

    /// Block until more than `seen` changes were counted, or `timeout` passes.
    fn wait_past(&self, seen: i64, timeout: Duration) -> bool {
        let guard = self.changes.lock().expect("selection lock poisoned");
        let (guard, _) = self
            .changed
            .wait_timeout_while(guard, timeout, |changes| *changes <= seen)
            .expect("selection lock poisoned");
        *guard > seen
    }

    fn set_mime_types(&self, types: Vec<String>) {
        *self.mime_types.lock().expect("selection lock poisoned") = types;
    }

    fn mime_types(&self) -> Vec<String> {
        self.mime_types.lock().expect("selection lock poisoned").clone()
    }
}

/// Clipboard managers that honour it (KDE's Klipper, and Magpie) skip content
/// carrying this type; password managers such as KeePassXC set it.
const SECRET_HINT: &str = "x-kde-passwordManagerHint";
const TEXT_TYPES: [&str; 6] = [
    "text/plain;charset=utf-8",
    "text/plain;charset=UTF-8",
    "UTF8_STRING",
    "text/plain",
    "STRING",
    "TEXT",
];
/// How long a change whose targets can't be read is retried before being
/// skipped. XWayland's bridge can take a moment after the change event.
const UNREADABLE_GRACE: Duration = Duration::from_secs(2);

pub struct LinuxClipboard {
    inner: Mutex<arboard::Clipboard>,
    selection: Arc<SelectionState>,
    /// Present when watching through X11, where the offered types come from
    /// asking the owner.
    targets: Option<x11::TargetsReader>,
    unreadable_since: Mutex<Option<(i64, Instant)>>,
}

impl LinuxClipboard {
    fn offered_types(&self) -> Result<Vec<String>, String> {
        let Some(reader) = &self.targets else {
            return Ok(self.selection.mime_types());
        };
        let change = self.selection.changes();
        match reader.targets() {
            Ok(types) => {
                *self.unreadable_since.lock().expect("clipboard lock poisoned") = None;
                Ok(types)
            }
            Err(e) => {
                let mut since = self.unreadable_since.lock().expect("clipboard lock poisoned");
                let first = match *since {
                    Some((seen, at)) if seen == change => at,
                    _ => {
                        *since = Some((change, Instant::now()));
                        Instant::now()
                    }
                };
                if first.elapsed() > UNREADABLE_GRACE {
                    log::warn!("[Linux] skipping a clipboard change whose owner never answered: {e}");
                    Ok(Vec::new())
                } else {
                    Err(e)
                }
            }
        }
    }
}

impl Clipboard for LinuxClipboard {
    fn change_token(&self) -> Option<i64> {
        Some(self.selection.changes())
    }

    fn read(&self) -> Result<Option<Captured>, String> {
        let types = self.offered_types()?;
        let offers = |t: &str| types.iter().any(|x| x == t);
        if offers(SECRET_HINT) {
            log::debug!("Skipping clipboard content marked as sensitive");
            return Ok(None);
        }

        let mut clipboard = self.inner.lock().expect("clipboard lock poisoned");
        // What the owner offered but then failed to deliver is retried: the
        // owner may still be preparing it.
        let unreadable = |e: arboard::Error| format!("Failed to read the clipboard: {e}");

        if offers("text/uri-list") {
            let paths: Vec<String> = clipboard
                .get()
                .file_list()
                .map_err(unreadable)?
                .into_iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            // A link copied in a browser is a uri-list too, without file URLs.
            if !paths.is_empty() {
                return Ok(Some(Captured::Files { paths }));
            }
        }

        if TEXT_TYPES.iter().any(|t| offers(t)) {
            let text = clipboard.get_text().map_err(unreadable)?;
            if !text.is_empty() {
                return Ok(Some(Captured::Text { text, html: None }));
            }
        }

        // Rich content that exposes only an HTML flavor with no plain-text
        // representation: keep the HTML and a stripped plain-text version for
        // display/search.
        if offers("text/html") {
            let html = clipboard.get().html().map_err(unreadable)?;
            let plain = super::html_to_plain_text(&html);
            if !plain.is_empty() {
                return Ok(Some(Captured::Text { text: plain, html: Some(html) }));
            }
        }

        if offers("image/png") {
            let image = clipboard.get_image().map_err(unreadable)?;
            if !image.bytes.is_empty() {
                return Ok(Some(Captured::Image {
                    rgba: image.bytes.into_owned(),
                    width: image.width as u32,
                    height: image.height as u32,
                }));
            }
        }

        log::debug!("Clipboard changed but no recognizable content found: {types:?}");
        Ok(None)
    }

    fn write(&self, payload: &WritePayload) -> Result<(), String> {
        let seen = self.selection.changes();
        {
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
            result.map_err(|e| format!("Failed to write to clipboard: {e}"))?;
        }
        // Our own write reaches the watcher as a change like any other. Wait
        // for it, so the caller's `mark_self_write` absorbs it instead of the
        // monitor capturing it back.
        if !self.selection.wait_past(seen, Duration::from_secs(1)) {
            log::warn!("[Linux] Magpie's own clipboard write was not observed within 1s");
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Paster
// ---------------------------------------------------------------------------

enum FocusSource {
    X11(Arc<x11::X11Desktop>),
    /// `None` where the compositor doesn't reveal toplevels (GNOME, KDE).
    Wayland(Option<Arc<wayland::ToplevelTracker>>),
}

impl FocusSource {
    fn describe(&self) -> &'static str {
        match self {
            FocusSource::X11(_) => "EWMH",
            FocusSource::Wayland(Some(_)) => "wlr foreign-toplevel",
            FocusSource::Wayland(None) => "unavailable",
        }
    }
}

enum Injector {
    XTest(Arc<x11::X11Desktop>),
    VirtualKeyboard,
    Portal(portal::RemoteDesktopPaster),
    None,
}

impl Injector {
    fn describe(&self) -> &'static str {
        match self {
            Injector::XTest(_) => "XTest",
            Injector::VirtualKeyboard => "virtual keyboard",
            Injector::Portal(_) => "RemoteDesktop portal",
            Injector::None => "unavailable",
        }
    }
}

/// (WM_CLASS or Wayland app id, executable) of a window.
type WindowApp = (Option<String>, Option<String>);
/// (app id, display name) of an application.
type AppName = (Option<String>, Option<String>);

pub struct LinuxPaster {
    app: AppHandle,
    focus: FocusSource,
    injector: Injector,
    /// Cached so each capture doesn't re-scan the desktop entries.
    names: Mutex<HashMap<WindowApp, AppName>>,
}

impl LinuxPaster {
    fn describe(&self, class: Option<String>, exe: Option<String>) -> AppName {
        let mut names = self.names.lock().expect("app name cache poisoned");
        names
            .entry((class.clone(), exe.clone()))
            .or_insert_with(|| desktop::describe_app(class.as_deref(), exe.as_deref()))
            .clone()
    }
}

impl Paster for LinuxPaster {
    fn focused_window(&self) -> FocusedWindow {
        match &self.focus {
            FocusSource::X11(x11) => x11.focused_window().unwrap_or_else(|e| {
                log::warn!("{e}");
                FocusedWindow::default()
            }),
            FocusSource::Wayland(Some(tracker)) => tracker.focused().map(|t| t.focus).unwrap_or_default(),
            FocusSource::Wayland(None) => FocusedWindow::default(),
        }
    }

    fn frontmost_app(&self) -> AppInfo {
        match &self.focus {
            FocusSource::X11(x11) => {
                let facts = match x11.focused_window_facts() {
                    Ok(facts) => facts,
                    Err(e) => {
                        log::warn!("{e}");
                        return AppInfo::default();
                    }
                };
                let exe = facts
                    .focus
                    .pid
                    .and_then(|pid| std::fs::read_link(format!("/proc/{pid}/exe")).ok())
                    .map(|p| p.to_string_lossy().into_owned());
                let class = facts.wm_class.map(|(_, class)| class);
                let (app_id, name) = self.describe(class, exe);
                AppInfo { app_id, name, focus: facts.focus }
            }
            FocusSource::Wayland(Some(tracker)) => match tracker.focused() {
                Some(toplevel) => {
                    let (app_id, name) = self.describe(toplevel.app_id, None);
                    AppInfo { app_id, name, focus: toplevel.focus }
                }
                None => AppInfo::default(),
            },
            FocusSource::Wayland(None) => AppInfo::default(),
        }
    }

    fn activate(&self, target: &AppInfo) -> bool {
        let Some(window) = target.focus.window else {
            return false;
        };
        match &self.focus {
            FocusSource::X11(x11) => x11.activate(window).map_err(|e| log::warn!("{e}")).is_ok(),
            FocusSource::Wayland(Some(tracker)) => tracker.activate(window),
            FocusSource::Wayland(None) => false,
        }
    }

    fn hide_and_restore_focus(&self, previous: Option<&AppInfo>) -> Result<(), String> {
        if let Some(window) = self.app.get_webview_window("main") {
            window.hide().map_err(|e| e.to_string())?;
        }
        // Hiding lets the window manager pick the next window, which isn't
        // necessarily the one Magpie was summoned from.
        if let Some(previous) = previous
            && self.capabilities().can_activate_app
            && !self.activate(previous)
        {
            log::warn!("Could not re-activate {:?} before paste", previous.name);
        }
        Ok(())
    }

    fn refocus_magpie(&self) {
        // A Wayland client can't take focus for itself, but the toplevel
        // protocol can hand it to any window, Magpie's included.
        if let FocusSource::Wayland(Some(tracker)) = &self.focus {
            if !tracker.activate_own() {
                log::warn!("[Wayland] Magpie's window isn't among the compositor's toplevels");
            }
        } else if let Some(window) = self.app.get_webview_window("main") {
            let _ = window.set_focus();
        }
    }

    fn paste(&self) -> Result<(), String> {
        let loc = crate::i18n::read_locale(&self.app);
        let result = match &self.injector {
            Injector::XTest(x11) => x11.paste(),
            Injector::VirtualKeyboard => wayland::paste_with_virtual_keyboard(),
            Injector::Portal(portal) => portal.paste(),
            Injector::None => Err(crate::i18n::tr(loc, "err.paste_unsupported").to_string()),
        };
        // Magpie's window is already hidden, so tell the user through the
        // desktop that the content is waiting on the clipboard.
        if let Err(e) = &result {
            use tauri_plugin_notification::NotificationExt;
            let _ = self
                .app
                .notification()
                .builder()
                .title(crate::i18n::tr(loc, "notify.paste_failed_title"))
                .body(e)
                .show();
        }
        result
    }

    fn capabilities(&self) -> PasterCapabilities {
        let reads_focus = !matches!(self.focus, FocusSource::Wayland(None));
        PasterCapabilities {
            can_paste: !matches!(self.injector, Injector::None),
            can_activate_app: reads_focus,
            can_read_focus: reads_focus,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_waits_for_the_next_change() {
        let state = Arc::new(SelectionState::default());
        let seen = state.changes();
        assert!(!state.wait_past(seen, Duration::from_millis(20)), "nothing changed yet");

        let bumper = Arc::clone(&state);
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            bumper.bump();
        });
        assert!(state.wait_past(seen, Duration::from_secs(2)), "the bump wakes the waiter");
        handle.join().unwrap();
        assert_eq!(state.changes(), seen + 1);
    }
}
