use tauri::{
    AppHandle, Emitter, Manager,
    menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu},
};

use crate::i18n::{read_locale, tr, Locale};
use crate::show_window;

/// Build the full macOS menu bar in the given locale. Custom item ids
/// (`app_settings`, `close_window`) are stable across locales so the
/// `on_menu_event` handler registered once in [`create_app_menu`] keeps working
/// after a locale rebuild.
fn build_app_menu(app: &AppHandle, locale: Locale) -> Result<Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    // --- App submenu (Magpie) ---
    let about = PredefinedMenuItem::about(app, Some(tr(locale, "menu.about")), Some(AboutMetadata {
        name: Some("Magpie".to_string()),
        version: Some(app.config().version.clone().unwrap_or("0.1.1".to_string())),
        ..Default::default()
    }))?;
    let separator1 = PredefinedMenuItem::separator(app)?;
    let settings = MenuItem::with_id(
        app, "app_settings", tr(locale, "menu.settings"),
        true, Some("CmdOrCtrl+,"),
    )?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let hide = PredefinedMenuItem::hide(app, Some(tr(locale, "menu.hide")))?;
    let hide_others = PredefinedMenuItem::hide_others(app, Some(tr(locale, "menu.hide_others")))?;
    let show_all = PredefinedMenuItem::show_all(app, Some(tr(locale, "menu.show_all")))?;
    let separator3 = PredefinedMenuItem::separator(app)?;
    let quit = PredefinedMenuItem::quit(app, Some(tr(locale, "menu.quit")))?;

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
    let undo = PredefinedMenuItem::undo(app, Some(tr(locale, "menu.undo")))?;
    let redo = PredefinedMenuItem::redo(app, Some(tr(locale, "menu.redo")))?;
    let sep_edit1 = PredefinedMenuItem::separator(app)?;
    let cut = PredefinedMenuItem::cut(app, Some(tr(locale, "menu.cut")))?;
    let copy = PredefinedMenuItem::copy(app, Some(tr(locale, "menu.copy")))?;
    let paste = PredefinedMenuItem::paste(app, Some(tr(locale, "menu.paste")))?;
    let select_all = PredefinedMenuItem::select_all(app, Some(tr(locale, "menu.select_all")))?;

    let edit_submenu = Submenu::with_items(
        app, tr(locale, "menu.edit"), true,
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
        app, "close_window", tr(locale, "menu.close_window"),
        true, Some("CmdOrCtrl+W"),
    )?;
    let minimize = PredefinedMenuItem::minimize(app, Some(tr(locale, "menu.minimize")))?;

    let window_submenu = Submenu::with_items(
        app, tr(locale, "menu.window"), true,
        &[&close_window, &minimize],
    )?;

    Ok(Menu::with_items(app, &[&app_submenu, &edit_submenu, &window_submenu])?)
}

/// Rebuild the menu bar in the currently-persisted locale. Called on language
/// change so the menu updates without a restart. Does NOT re-register the menu
/// event handler (that is attached once in [`create_app_menu`]).
pub fn apply_locale(app: &AppHandle) {
    if let Ok(menu) = build_app_menu(app, read_locale(app)) {
        let _ = app.set_menu(menu);
    }
}

/// Create and set the standard macOS application menu bar.
///
/// This provides the OS-convention keyboard shortcuts that macOS users expect:
/// - ⌘, → Settings
/// - ⌘Q → Quit
/// - ⌘H → Hide
/// - ⌘W → Close Window (hide)
/// - Standard Edit menu: ⌘Z, ⌘X, ⌘C, ⌘V, ⌘A
pub fn create_app_menu(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_app_menu(app, read_locale(app))?;

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
