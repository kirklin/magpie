use crate::database::models::AppSettings;

#[tauri::command]
pub fn get_default_settings() -> AppSettings {
    AppSettings::default()
}

/// Show or hide the menu bar tray icon at runtime.
#[tauri::command]
pub fn set_tray_visible(app_handle: tauri::AppHandle, visible: bool) -> Result<(), String> {
    use tauri::Manager;
    if let Some(tray) = app_handle.tray_by_id("main-tray") {
        tray.set_visible(visible)
            .map_err(|e| format!("Failed to set tray visible: {}", e))?;
        log::info!("Menu bar icon visibility set to: {}", visible);
        Ok(())
    } else {
        Err("Tray icon not found".to_string())
    }
}

/// Re-register the global shortcut at runtime.
/// Unregisters all existing shortcuts first, then registers the new one.
#[tauri::command]
pub fn update_global_shortcut(app_handle: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let global_shortcut = app_handle.global_shortcut();

    // Unregister all existing shortcuts
    global_shortcut.unregister_all()
        .map_err(|e| format!("Failed to unregister shortcuts: {}", e))?;

    // Register the new shortcut
    let handle = app_handle.clone();
    global_shortcut.on_shortcut(
        shortcut.as_str(),
        move |_app, _shortcut, event| {
            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                crate::toggle_window(&handle);
            }
        },
    ).map_err(|e| format!("快捷键格式无效: {}", e))?;

    log::info!("Global shortcut updated to: {}", shortcut);
    Ok(())
}
