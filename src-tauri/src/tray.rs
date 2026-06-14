use tauri::{
    AppHandle, Emitter, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::i18n::{read_locale, tr, Locale};
use crate::show_window;

/// Build the tray context menu in the given locale. Item ids are stable across
/// locales so the tray's `on_menu_event` handler keeps matching after a rebuild.
fn build_tray_menu(app: &AppHandle, locale: Locale) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", tr(locale, "tray.show"), true, Some("CmdOrCtrl+Shift+V"))?;
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

/// Rebuild the tray menu in the currently-persisted locale. Called when the
/// user changes language so the tray updates without a restart.
pub fn apply_locale(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        if let Ok(menu) = build_tray_menu(app, read_locale(app)) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

/// Create and configure the system tray
pub fn create_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_tray_menu(app, read_locale(app))?;

    let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-iconTemplate.png"))
        .unwrap_or_else(|_| app.default_window_icon().cloned().unwrap());

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(tray_icon)
        .menu(&menu)
        // Allow default left/right click to show menu, or use click event to toggle
        .on_menu_event(move |app, event| {
            log::debug!("Tray menu event: {}", event.id.as_ref());
            match event.id.as_ref() {
                "show" => {
                    crate::toggle_window(app);
                }
                "settings" => {
                    show_window(app);
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.emit("navigate", "settings");
                    }
                }
                "about" => {
                    show_window(app);
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                log::debug!("Tray icon left click");
                crate::toggle_window(tray.app_handle());
            }
        })
        .build(app)?;

    log::info!("System tray created");
    Ok(())
}
