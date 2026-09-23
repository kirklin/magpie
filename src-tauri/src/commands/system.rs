use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{Manager, State};

use crate::error::AppError;
use crate::platform::{IconsPort, PasterCapabilities, PasterPort};

/// In-memory cache for app icons (app id -> PNG data URL)
pub struct AppIconCache(pub Mutex<HashMap<String, String>>);

impl Default for AppIconCache {
    fn default() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}

/// Get the icon of an application as a PNG data URL. `app_id` is the
/// entry's `source_app`: a bundle id (macOS), an executable path (Windows) or a
/// desktop entry id (Linux).
#[tauri::command]
#[specta::specta]
pub async fn get_app_icon(
    app_handle: tauri::AppHandle,
    app_id: String,
    cache: State<'_, AppIconCache>,
) -> Result<String, AppError> {
    // Check cache first
    {
        let c = cache.0.lock().map_err(|e| AppError::Other { message: e.to_string() })?;
        if let Some(cached) = c.get(&app_id) {
            return Ok(cached.clone());
        }
    }

    let icons = app_handle.state::<IconsPort>().inner().clone();
    let id = app_id.clone();
    let icon = tokio::task::spawn_blocking(move || icons.app_icon(&id))
        .await
        .map_err(|e| AppError::Other { message: e.to_string() })??;

    // Store in cache (bounded to avoid unbounded growth over a long session).
    {
        const MAX_CACHED_ICONS: usize = 256;
        let mut c = cache.0.lock().map_err(|e| AppError::Other { message: e.to_string() })?;
        if c.len() >= MAX_CACHED_ICONS && !c.contains_key(&app_id) {
            c.clear();
        }
        c.insert(app_id, icon.clone());
    }

    Ok(icon)
}

/// Get the icon the system file browser shows for `file_path`, as a PNG data
/// URL. Not cached here: the frontend keeps its own per-path cache.
#[tauri::command]
#[specta::specta]
pub async fn get_file_icon(
    app_handle: tauri::AppHandle,
    file_path: String,
) -> Result<String, AppError> {
    let icons = app_handle.state::<IconsPort>().inner().clone();
    tokio::task::spawn_blocking(move || icons.file_icon(&file_path))
        .await
        .map_err(|e| AppError::Other { message: e.to_string() })?
        .map_err(AppError::from)
}

/// What paste-back can do on this OS and desktop, so the UI only offers
/// actions that work.
#[tauri::command]
#[specta::specta]
pub fn get_paster_capabilities(app_handle: tauri::AppHandle) -> PasterCapabilities {
    app_handle.state::<PasterPort>().capabilities()
}

/// Hide the main window
#[tauri::command]
#[specta::specta]
pub fn hide_window(app_handle: tauri::AppHandle) {
    if let Some(window) = tauri::Manager::get_webview_window(&app_handle, "main") {
        let _ = window.hide();
    }
}

/// Let the desktop move the main window along with the pressed pointer.
#[tauri::command]
#[specta::specta]
pub fn start_window_drag(app_handle: tauri::AppHandle) -> Result<(), AppError> {
    let window = app_handle
        .get_webview_window("main")
        .ok_or_else(|| AppError::Other { message: "main window missing".to_string() })?;
    app_handle.state::<crate::DragStarted>().mark();
    window.start_dragging().map_err(|e| AppError::Other { message: e.to_string() })
}
