use tauri::{
    AppHandle, Emitter, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::show_window;

/// Create and configure the system tray
pub fn create_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "打开/隐藏 Magpie", true, Some("CmdOrCtrl+Shift+V"))?;
    let separator1 = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(app, "settings", "设置…", true, Some("CmdOrCtrl+,"))?;
    let about = MenuItem::with_id(app, "about", "关于 Magpie", true, None::<&str>)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, Some("CmdOrCtrl+Q"))?;

    let menu = Menu::with_items(
        app,
        &[&show, &separator1, &settings, &about, &separator2, &quit],
    )?;

    let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-iconTemplate.png"))
        .unwrap_or_else(|_| app.default_window_icon().cloned().unwrap());

    let _tray = TrayIconBuilder::new()
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
