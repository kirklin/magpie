use tauri::{
    AppHandle, Emitter, Manager,
    menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu},
};

use crate::show_window;

/// Create and set the standard macOS application menu bar.
///
/// This provides the OS-convention keyboard shortcuts that macOS users expect:
/// - ⌘, → Settings
/// - ⌘Q → Quit
/// - ⌘H → Hide
/// - ⌘W → Close Window (hide)
/// - Standard Edit menu: ⌘Z, ⌘X, ⌘C, ⌘V, ⌘A
pub fn create_app_menu(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // --- App submenu (Magpie) ---
    let about = PredefinedMenuItem::about(app, Some("关于 Magpie"), Some(AboutMetadata {
        name: Some("Magpie".to_string()),
        version: Some(app.config().version.clone().unwrap_or("0.1.1".to_string())),
        ..Default::default()
    }))?;
    let separator1 = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(
        app, "app_settings", "设置…",
        true, Some("CmdOrCtrl+,"),
    )?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let hide = PredefinedMenuItem::hide(app, Some("隐藏 Magpie"))?;
    let hide_others = PredefinedMenuItem::hide_others(app, Some("隐藏其他"))?;
    let show_all = PredefinedMenuItem::show_all(app, Some("全部显示"))?;
    let separator3 = PredefinedMenuItem::separator(app)?;
    let quit = PredefinedMenuItem::quit(app, Some("退出 Magpie"))?;

    let app_submenu = Submenu::with_items(
        app, "Magpie", true,
        &[
            &about,
            &separator1,
            &settings,
            &separator2,
            &hide,
            &hide_others,
            &show_all,
            &separator3,
            &quit,
        ],
    )?;

    // --- Edit submenu ---
    let undo = PredefinedMenuItem::undo(app, Some("撤销"))?;
    let redo = PredefinedMenuItem::redo(app, Some("重做"))?;
    let sep_edit1 = PredefinedMenuItem::separator(app)?;
    let cut = PredefinedMenuItem::cut(app, Some("剪切"))?;
    let copy = PredefinedMenuItem::copy(app, Some("复制"))?;
    let paste = PredefinedMenuItem::paste(app, Some("粘贴"))?;
    let select_all = PredefinedMenuItem::select_all(app, Some("全选"))?;

    let edit_submenu = Submenu::with_items(
        app, "编辑", true,
        &[
            &undo,
            &redo,
            &sep_edit1,
            &cut,
            &copy,
            &paste,
            &select_all,
        ],
    )?;

    // --- Window submenu ---
    let close_window = MenuItem::with_id(
        app, "close_window", "关闭窗口",
        true, Some("CmdOrCtrl+W"),
    )?;
    let minimize = PredefinedMenuItem::minimize(app, Some("最小化"))?;

    let window_submenu = Submenu::with_items(
        app, "窗口", true,
        &[&close_window, &minimize],
    )?;

    // --- Build full menu bar ---
    let menu = Menu::with_items(app, &[&app_submenu, &edit_submenu, &window_submenu])?;

    // Set as the app menu and handle custom menu events
    app.set_menu(menu)?;
    let handle = app.clone();
    app.on_menu_event(move |_app, event| {
        match event.id().as_ref() {
            "app_settings" => {
                show_window(&handle);
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.emit("navigate", "settings");
                }
            }
            "close_window" => {
                // ⌘W hides the window instead of quitting (same as Escape)
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            _ => {}
        }
    });

    log::info!("Application menu bar created with standard macOS shortcuts");
    Ok(())
}
