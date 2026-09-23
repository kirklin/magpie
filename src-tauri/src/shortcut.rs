//! The global shortcut that toggles the Magpie window.
//!
//! macOS, Windows and X11 let Magpie grab the key itself
//! (tauri-plugin-global-shortcut). Wayland doesn't: there the desktop owns the
//! shortcut through the GlobalShortcuts portal, and where no portal is
//! available the user binds one in the desktop's own keyboard settings to the
//! `--toggle` command (which reaches this instance through single-instance).

use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::platform::DEFAULT_SHORTCUT;

/// The shortcut Magpie itself registered, in Tauri accelerator syntax; `None`
/// when the desktop owns the shortcut.
pub struct ActiveShortcut(Mutex<Option<String>>);

impl ActiveShortcut {
    pub fn new() -> Self {
        Self(Mutex::new(None))
    }

    pub fn get(&self) -> Option<String> {
        self.0.lock().expect("active shortcut lock poisoned").clone()
    }

    fn set(&self, shortcut: &str) {
        *self.0.lock().expect("active shortcut lock poisoned") = Some(shortcut.to_string());
    }
}

/// How the toggle shortcut is bound on this desktop, for the settings screen.
/// The desktop-owned variants only occur on Linux but belong to the shared
/// IPC type.
#[derive(Clone, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub enum ShortcutBinding {
    /// Magpie registers the shortcut itself; the user records it in Settings.
    Native,
    /// The desktop owns the shortcut (Wayland GlobalShortcuts portal).
    Portal {
        /// The trigger as the desktop words it, once assigned.
        trigger: Option<String>,
        /// Whether the desktop can open its own dialog to change it.
        can_configure: bool,
        /// What a shortcut bound in the desktop's keyboard settings runs, for
        /// desktops whose portal leaves the key unassigned (Hyprland, niri) or
        /// when the user declined the desktop's prompt.
        command: String,
    },
    /// Nothing can register a shortcut for Magpie: bind one to `command` in
    /// the desktop's keyboard settings.
    Manual { command: String },
}

enum Mode {
    Native,
    #[cfg(target_os = "linux")]
    Portal(crate::platform::GlobalShortcutPortal),
    #[cfg(target_os = "linux")]
    Manual,
}

struct ShortcutMode(Mode);

/// Register `shortcut` at startup. A persisted shortcut that can no longer be
/// registered (another app took it, or it was edited by hand) must not keep
/// Magpie from launching, so it gives way to the default.
pub fn register_at_startup(app: &AppHandle, shortcut: &str) {
    #[cfg(target_os = "linux")]
    if crate::platform::is_wayland() {
        let mode = match crate::platform::GlobalShortcutPortal::start(app, shortcut) {
            Ok(Some(portal)) => {
                log::info!("Global shortcut handled by the desktop's GlobalShortcuts portal");
                Mode::Portal(portal)
            }
            Ok(None) => {
                log::info!("No GlobalShortcuts portal; the shortcut is bound in the desktop's settings");
                Mode::Manual
            }
            Err(e) => {
                log::error!("GlobalShortcuts portal failed: {e}; the shortcut is bound in the desktop's settings");
                Mode::Manual
            }
        };
        app.manage(ShortcutMode(mode));
        crate::tray::rebuild_menu(app);
        return;
    }

    match bind(app, shortcut) {
        Ok(()) => log::info!("Global shortcut registered: {}", shortcut),
        Err(e) => {
            log::error!("Failed to register saved shortcut '{}': {}; using the default", shortcut, e);
            if let Err(e) = bind(app, DEFAULT_SHORTCUT) {
                log::error!("Failed to register the default shortcut '{}': {}", DEFAULT_SHORTCUT, e);
            }
        }
    }
    app.manage(ShortcutMode(Mode::Native));
    crate::tray::rebuild_menu(app);
}

/// Replace the registered shortcut with `shortcut`.
///
/// The new shortcut is validated (parsed) BEFORE the old one is released, so an
/// invalid value can never leave the app with no shortcut. If a valid but
/// unregisterable combination (e.g. already held by another app) fails to
/// bind, the default shortcut is restored and the error returned.
pub fn replace(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    let loc = crate::i18n::read_locale(app);
    if !matches!(app.state::<ShortcutMode>().0, Mode::Native) {
        return Err(crate::i18n::tr(loc, "err.shortcut_owned_by_desktop").to_string());
    }
    shortcut
        .parse::<Shortcut>()
        .map_err(|_| format!("{}{}", crate::i18n::tr(loc, "err.shortcut_invalid"), shortcut))?;

    let result = match bind(app, shortcut) {
        Ok(()) => {
            log::info!("Global shortcut updated to: {}", shortcut);
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to register '{}': {}; restoring the default", shortcut, e);
            let _ = bind(app, DEFAULT_SHORTCUT);
            Err(format!(
                "{}'{}': {}",
                crate::i18n::tr(loc, "err.shortcut_register_failed"),
                shortcut,
                e
            ))
        }
    };
    crate::tray::rebuild_menu(app);
    result
}

/// How the shortcut is bound, for the settings screen.
pub fn binding(app: &AppHandle) -> ShortcutBinding {
    match &app.state::<ShortcutMode>().0 {
        Mode::Native => ShortcutBinding::Native,
        #[cfg(target_os = "linux")]
        Mode::Portal(portal) => {
            let state = portal.state();
            ShortcutBinding::Portal {
                trigger: state.trigger,
                can_configure: state.can_configure,
                command: toggle_command(),
            }
        }
        #[cfg(target_os = "linux")]
        Mode::Manual => ShortcutBinding::Manual { command: toggle_command() },
    }
}

/// Open the desktop's dialog for changing the shortcut (portal mode only).
pub fn configure_in_desktop(app: &AppHandle) -> Result<(), String> {
    match &app.state::<ShortcutMode>().0 {
        #[cfg(target_os = "linux")]
        Mode::Portal(portal) => portal.configure(),
        _ => Err(crate::i18n::tr(crate::i18n::read_locale(app), "err.shortcut_not_configurable").to_string()),
    }
}

/// The command a desktop shortcut runs to toggle Magpie.
#[cfg(target_os = "linux")]
fn toggle_command() -> String {
    let exe = std::env::var_os("APPIMAGE")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "magpie".to_string());
    if exe.contains(' ') {
        format!("\"{exe}\" --toggle")
    } else {
        format!("{exe} --toggle")
    }
}

/// Release every shortcut, then bind `shortcut` to toggle the window.
fn bind(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    let global_shortcut = app.global_shortcut();
    global_shortcut.unregister_all().map_err(|e| e.to_string())?;

    let handle = app.clone();
    global_shortcut
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                crate::toggle_window(&handle);
            }
        })
        .map_err(|e| e.to_string())?;

    app.state::<ActiveShortcut>().set(shortcut);
    Ok(())
}
