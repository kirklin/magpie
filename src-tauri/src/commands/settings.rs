use crate::database::models::AppSettings;
use crate::error::AppError;

#[tauri::command]
#[specta::specta]
pub fn get_default_settings() -> AppSettings {
    AppSettings::default()
}

/// Show or hide the menu bar tray icon at runtime.
#[tauri::command]
#[specta::specta]
pub fn set_tray_visible(app_handle: tauri::AppHandle, visible: bool) -> Result<(), AppError> {
    if let Some(tray) = app_handle.tray_by_id("main-tray") {
        tray.set_visible(visible)
            .map_err(|e| AppError::Other { message: format!("Failed to set tray visible: {}", e) })?;
        log::info!("Menu bar icon visibility set to: {}", visible);
        Ok(())
    } else {
        Err(AppError::Other { message: "Tray icon not found".to_string() })
    }
}

/// Rebuild the native tray + app menu in the currently-persisted locale.
/// Called by the frontend right after the language setting changes, so the OS
/// menus switch language without requiring a restart.
#[tauri::command]
#[specta::specta]
pub fn relocalize_menus(app_handle: tauri::AppHandle) {
    crate::menu::apply_locale(&app_handle);
    crate::tray::apply_locale(&app_handle);
}

/// Default global shortcut, used as a fallback so the app is never left
/// without a working hotkey.
const DEFAULT_SHORTCUT: &str = "CmdOrCtrl+Shift+V";

/// Re-register the global shortcut at runtime.
///
/// The new shortcut is validated (parsed) BEFORE the old one is unregistered,
/// so an invalid value can never leave the app with no shortcut. If a
/// valid-but-unregisterable combination (e.g. already held by another app)
/// fails to bind, we fall back to the default shortcut and return an error.
#[tauri::command]
#[specta::specta]
pub fn update_global_shortcut(app_handle: tauri::AppHandle, shortcut: String) -> Result<(), AppError> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    let global_shortcut = app_handle.global_shortcut();

    // Validate the format first — no side effects if this fails.
    let parsed: Shortcut = shortcut
        .parse()
        .map_err(|_| AppError::Validation { message: format!("快捷键格式无效: {}", shortcut) })?;

    // Now it's safe to drop the old binding and install the new one.
    global_shortcut
        .unregister_all()
        .map_err(|e| AppError::Other { message: format!("Failed to unregister shortcuts: {}", e) })?;

    let handle = app_handle.clone();
    let register = global_shortcut.on_shortcut(parsed, move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            crate::toggle_window(&handle);
        }
    });

    if let Err(e) = register {
        // Valid format but could not be registered. Restore a working hotkey.
        log::error!("Failed to register '{}': {}; falling back to default", shortcut, e);
        let handle = app_handle.clone();
        let _ = global_shortcut.on_shortcut(DEFAULT_SHORTCUT, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                crate::toggle_window(&handle);
            }
        });
        return Err(AppError::Validation { message: format!("无法注册快捷键 '{}': {}", shortcut, e) });
    }

    log::info!("Global shortcut updated to: {}", shortcut);
    Ok(())
}
