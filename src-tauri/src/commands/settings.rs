use crate::database::models::AppSettings;

#[tauri::command]
pub fn get_default_settings() -> AppSettings {
    AppSettings::default()
}
