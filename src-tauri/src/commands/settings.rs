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
    #[cfg(target_os = "macos")]
    crate::menu::apply_locale(&app_handle);
    crate::tray::rebuild_menu(&app_handle);
}

/// Re-register the global shortcut at runtime. An invalid or unregisterable
/// shortcut is rejected, and the app keeps a working one.
#[tauri::command]
#[specta::specta]
pub fn update_global_shortcut(app_handle: tauri::AppHandle, shortcut: String) -> Result<(), AppError> {
    crate::shortcut::replace(&app_handle, &shortcut).map_err(|message| AppError::Validation { message })
}

/// How the global shortcut is bound on this desktop: recorded in Magpie, owned
/// by the desktop, or to be set up by the user in the desktop's settings.
#[tauri::command]
#[specta::specta]
pub fn get_shortcut_binding(app_handle: tauri::AppHandle) -> crate::shortcut::ShortcutBinding {
    crate::shortcut::binding(&app_handle)
}

/// Open the desktop's own dialog for changing the global shortcut.
#[tauri::command]
#[specta::specta]
pub async fn configure_system_shortcut(app_handle: tauri::AppHandle) -> Result<(), AppError> {
    tokio::task::spawn_blocking(move || crate::shortcut::configure_in_desktop(&app_handle))
        .await
        .map_err(|e| AppError::Other { message: e.to_string() })?
        .map_err(AppError::from)
}
