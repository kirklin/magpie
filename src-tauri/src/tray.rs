use tauri::{
    AppHandle, Emitter, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::i18n::{read_locale, tr, Locale};
use crate::show_window;

/// The tray icon image. macOS draws a white glyph in the menu bar; the Windows
/// notification area and Linux panels can be light or dark, where only the
/// full-colour app icon stays visible.
#[cfg(target_os = "macos")]
const TRAY_ICON: &[u8] = include_bytes!("../icons/tray-iconTemplate.png");
#[cfg(target_os = "windows")]
const TRAY_ICON: &[u8] = include_bytes!("../icons/32x32.png");
#[cfg(target_os = "linux")]
const TRAY_ICON: &[u8] = include_bytes!("../icons/64x64.png");

/// Build the tray context menu in the given locale. Item ids are stable across
/// locales so the tray's `on_menu_event` handler keeps matching after a rebuild.
/// `shortcut` is the global shortcut Magpie registered, shown next to
/// "Show / Hide"; none when the desktop owns it.
fn build_tray_menu(
    app: &AppHandle,
    locale: Locale,
    shortcut: Option<&str>,
) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", tr(locale, "tray.show"), true, shortcut)?;
    let separator1 = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", tr(locale, "menu.settings"), true, Some("CmdOrCtrl+,"))?;
    let about = MenuItem::with_id(app, "about", tr(locale, "menu.about"), true, None::<&str>)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", tr(locale, "tray.quit"), true, Some("CmdOrCtrl+Q"))?;

    Ok(Menu::with_items(
        app,
        &[&show, &separator1, &settings, &about, &separator2, &quit],
    )?)
}

/// Rebuild the tray menu from the persisted locale and the registered shortcut.
/// Called when either changes so the tray updates without a restart.
pub fn rebuild_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let shortcut = app.state::<crate::shortcut::ActiveShortcut>().get();
        if let Ok(menu) = build_tray_menu(app, read_locale(app), shortcut.as_deref()) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

/// Show the main window and switch it to `view`.
fn show_view(app: &AppHandle, view: &str) {
    show_window(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("navigate", view);
    }
}

/// Create and configure the system tray
pub fn create_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let shortcut = app.state::<crate::shortcut::ActiveShortcut>().get();
    let menu = build_tray_menu(app, read_locale(app), shortcut.as_deref())?;
    let tray_icon = tauri::image::Image::from_bytes(TRAY_ICON)?;

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(tray_icon)
        .tooltip("Magpie")
        .menu(&menu)
        // Windows convention: left click opens the window, right click the
        // menu. macOS keeps the menu on left click.
        .show_menu_on_left_click(!cfg!(target_os = "windows"))
        .on_menu_event(move |app, event| {
            log::debug!("Tray menu event: {}", event.id.as_ref());
            match event.id.as_ref() {
                "show" => crate::toggle_window(app),
                "settings" => show_view(app, "settings"),
                "about" => show_view(app, "about"),
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            // Not emitted on Linux, where a click always opens the menu.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                log::debug!("Tray icon left click");
                crate::toggle_window_from_tray(tray.app_handle());
            }
        })
        .build(app)?;

    log::info!("System tray created");
    Ok(())
}
