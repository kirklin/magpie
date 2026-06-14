use tauri::AppHandle;
use tauri::Manager;
use sqlx::Row;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::database::models::{ClipboardEntry, ClipboardQuery};
use crate::database::pool::get_pool;
use crate::clipboard::native;
use crate::clipboard::paste;
use crate::error::AppError;
use crate::platform::{ClipboardPort, PasterPort, WritePayload};

#[tauri::command]
#[specta::specta]
pub async fn get_clipboard_entries(
    app_handle: AppHandle,
    query: ClipboardQuery,
) -> Result<Vec<ClipboardEntry>, AppError> {
    let pool = get_pool(&app_handle).await?;

    let mut sql = String::from(
        "SELECT id, content_type, text_content, html_content, image_path, file_paths, \
         source_app, source_app_name, custom_name, is_pinned, is_favorite, content_hash, \
         content_preview, byte_size, created_at, accessed_at, access_count \
         FROM clipboard_entries WHERE 1=1",
    );
    let mut bind_values: Vec<String> = vec![];

    if let Some(ref search) = query.search {
        sql.push_str(" AND (text_content LIKE ? OR custom_name LIKE ? OR content_preview LIKE ?)");
        let search_pattern = format!("%{}%", search);
        bind_values.push(search_pattern.clone());
        bind_values.push(search_pattern.clone());
        bind_values.push(search_pattern);
    }

    if let Some(ref ct) = query.content_type {
        sql.push_str(" AND content_type = ?");
        bind_values.push(ct.clone());
    }

    if query.pinned_only {
        sql.push_str(" AND is_pinned = 1");
    }

    // Pinned items first, then by most recently accessed
    sql.push_str(" ORDER BY is_pinned DESC, accessed_at DESC");
    sql.push_str(&format!(" LIMIT {} OFFSET {}", query.limit, query.offset));

    let mut query_builder = sqlx::query(&sql);

    for val in &bind_values {
        query_builder = query_builder.bind(val);
    }

    let rows = query_builder.fetch_all(&pool).await?;

    let entries: Vec<ClipboardEntry> = rows
        .iter()
        .map(|row| ClipboardEntry {
            id: row.get("id"),
            content_type: row.get("content_type"),
            text_content: row.get("text_content"),
            html_content: row.get("html_content"),
            image_path: row.get("image_path"),
            file_paths: row.get("file_paths"),
            source_app: row.get("source_app"),
            source_app_name: row.get("source_app_name"),
            custom_name: row.get("custom_name"),
            is_pinned: row.get("is_pinned"),
            is_favorite: row.get("is_favorite"),
            content_hash: row.get("content_hash"),
            content_preview: row.get("content_preview"),
            byte_size: row.get("byte_size"),
            created_at: row.get("created_at"),
            accessed_at: row.get("accessed_at"),
            access_count: row.get("access_count"),
        })
        .collect();

    Ok(entries)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_clipboard_entry(app_handle: AppHandle, id: i32) -> Result<(), AppError> {
    let pool = get_pool(&app_handle).await?;

    // Capture the image path first so we can remove the file after the row.
    let image_path: Option<String> = sqlx::query_scalar(
        "SELECT image_path FROM clipboard_entries WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?
    .flatten();

    sqlx::query("DELETE FROM clipboard_entries WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await?;

    if let Some(path) = image_path {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn clear_clipboard_history(app_handle: AppHandle) -> Result<(), AppError> {
    let pool = get_pool(&app_handle).await?;

    // Collect image files of the rows we're about to delete so they don't
    // become orphaned on disk.
    let image_paths: Vec<String> = sqlx::query_scalar(
        "SELECT image_path FROM clipboard_entries \
         WHERE is_pinned = 0 AND image_path IS NOT NULL",
    )
    .fetch_all(&pool)
    .await?;

    sqlx::query("DELETE FROM clipboard_entries WHERE is_pinned = 0")
        .execute(&pool)
        .await?;

    for path in image_paths {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_pin_entry(app_handle: AppHandle, id: i32) -> Result<bool, AppError> {
    let pool = get_pool(&app_handle).await?;

    sqlx::query("UPDATE clipboard_entries SET is_pinned = NOT is_pinned WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await?;

    let row = sqlx::query("SELECT is_pinned FROM clipboard_entries WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await?;

    Ok(row.get::<bool, _>("is_pinned"))
}

#[tauri::command]
#[specta::specta]
pub async fn rename_clipboard_entry(
    app_handle: AppHandle,
    id: i32,
    name: String,
) -> Result<(), AppError> {
    let pool = get_pool(&app_handle).await?;

    sqlx::query("UPDATE clipboard_entries SET custom_name = ? WHERE id = ?")
        .bind(&name)
        .bind(id)
        .execute(&pool)
        .await?;
    Ok(())
}

/// Paste an image entry by writing the saved PNG to the clipboard, then pasting.
#[tauri::command]
#[specta::specta]
pub async fn paste_image_entry(app_handle: AppHandle, image_path: String) -> Result<(), AppError> {
    let clipboard = app_handle.state::<ClipboardPort>().inner().clone();
    let paster = app_handle.state::<PasterPort>().inner().clone();

    clipboard.write(&WritePayload::ImageFile(image_path))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);

    // Hide and paste
    app_handle.hide().map_err(|e| AppError::Other { message: e.to_string() })?;
    paste::wait_for_frontmost_app_switch(&paster, paste::MAGPIE_BUNDLE_ID).await;
    paster.paste().map_err(AppError::from)
}

/// Copy an image entry to the clipboard without pasting
#[tauri::command]
#[specta::specta]
pub fn copy_image_entry(app_handle: AppHandle, image_path: String) -> Result<(), AppError> {
    let clipboard = app_handle.state::<ClipboardPort>().inner().clone();
    clipboard.write(&WritePayload::ImageFile(image_path))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn paste_clipboard_entry(app_handle: AppHandle, text: String) -> Result<(), AppError> {
    let clipboard = app_handle.state::<ClipboardPort>().inner().clone();
    let paster = app_handle.state::<PasterPort>().inner().clone();

    // 1. Write to clipboard, then stop the monitor re-capturing our own write.
    clipboard.write(&WritePayload::Text(text))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);

    // 2. Hide the app (returns focus to the previous app)
    app_handle.hide().map_err(|e| AppError::Other { message: e.to_string() })?;

    // 3. Wait for the focus switch to complete by reading the active app
    paste::wait_for_frontmost_app_switch(&paster, paste::MAGPIE_BUNDLE_ID).await;

    // 4. Synthesize the paste keystroke
    paster.paste().map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub fn copy_clipboard_entry(app_handle: AppHandle, text: String) -> Result<(), AppError> {
    let clipboard = app_handle.state::<ClipboardPort>().inner().clone();
    clipboard.write(&WritePayload::Text(text))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn paste_as_plain_text(app_handle: AppHandle, text: String) -> Result<(), AppError> {
    let clipboard = app_handle.state::<ClipboardPort>().inner().clone();
    let paster = app_handle.state::<PasterPort>().inner().clone();

    clipboard.write(&WritePayload::Text(text))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);

    app_handle.hide().map_err(|e| AppError::Other { message: e.to_string() })?;

    // Wait for frontmost app switch
    paste::wait_for_frontmost_app_switch(&paster, paste::MAGPIE_BUNDLE_ID).await;

    paster.paste().map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn paste_file_entry(app_handle: AppHandle, file_paths_json: String) -> Result<(), AppError> {
    let file_paths: Vec<String> = serde_json::from_str(&file_paths_json)
        .map_err(|e| AppError::Other { message: format!("Failed to parse file paths: {}", e) })?;

    let clipboard = app_handle.state::<ClipboardPort>().inner().clone();
    let paster = app_handle.state::<PasterPort>().inner().clone();

    clipboard.write(&WritePayload::Files(file_paths))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);

    // Hide the app and paste
    app_handle.hide().map_err(|e| AppError::Other { message: e.to_string() })?;
    paste::wait_for_frontmost_app_switch(&paster, paste::MAGPIE_BUNDLE_ID).await;
    paster.paste().map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub fn copy_file_entry(app_handle: AppHandle, file_paths_json: String) -> Result<(), AppError> {
    let file_paths: Vec<String> = serde_json::from_str(&file_paths_json)
        .map_err(|e| AppError::Other { message: format!("Failed to parse file paths: {}", e) })?;

    let clipboard = app_handle.state::<ClipboardPort>().inner().clone();
    clipboard.write(&WritePayload::Files(file_paths))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);
    Ok(())
}

/// Update the text content of a clipboard entry (Edit Content action)
#[tauri::command]
#[specta::specta]
pub async fn update_entry_content(
    app_handle: AppHandle,
    id: i32,
    content: String,
) -> Result<(), AppError> {
    let pool = get_pool(&app_handle).await?;

    // Generate a preview (first 200 chars, single line)
    let preview = content
        .chars()
        .take(200)
        .collect::<String>()
        .replace('\n', " ");

    // Compute new hash and byte size
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let hash = hex::encode(hasher.finalize());
    let byte_size = content.len() as i64;

    sqlx::query(
        "UPDATE clipboard_entries SET text_content = ?, content_preview = ?, \
         content_hash = ?, byte_size = ? WHERE id = ?",
    )
        .bind(&content)
        .bind(&preview)
        .bind(&hash)
        .bind(byte_size)
        .bind(id)
        .execute(&pool)
        .await?;
    Ok(())
}

/// Append text to the current clipboard content
#[tauri::command]
#[specta::specta]
pub fn append_to_clipboard(app_handle: AppHandle, text: String) -> Result<(), AppError> {
    // Read current clipboard content (cross-platform via the clipboard plugin)
    let current = app_handle
        .clipboard()
        .read_text()
        .unwrap_or_default();

    // Append with newline separator
    let combined = if current.is_empty() {
        text
    } else {
        format!("{}\n{}", current, text)
    };

    let clipboard = app_handle.state::<ClipboardPort>().inner().clone();
    clipboard.write(&WritePayload::Text(combined))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);
    Ok(())
}

/// Save clipboard entry content to a file using a native save dialog
#[tauri::command]
#[specta::specta]
pub async fn save_entry_as_file(
    app_handle: AppHandle,
    content: String,
    default_name: String,
) -> Result<bool, AppError> {

    #[cfg(target_os = "macos")]
    {
        match native::run_save_panel(&app_handle, &default_name) {
            Some(path) => Ok(std::fs::write(&path, &content).is_ok()),
            None => Ok(false),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (&app_handle, &content, &default_name);
        Err(AppError::Other { message: "Save dialog not supported on this platform".to_string() })
    }
}

/// Paste content to the target app while keeping the Magpie window visible.
/// Activates the target app (window stays on screen due to always_on_top),
/// simulates Cmd+V, then re-focuses Magpie.
#[tauri::command]
#[specta::specta]
pub async fn paste_and_keep_window(app_handle: AppHandle, text: String) -> Result<(), AppError> {
    let clipboard = app_handle.state::<ClipboardPort>().inner().clone();
    clipboard.write(&WritePayload::Text(text))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);

    paste_to_previous_app_keeping_window(&app_handle).await.map_err(AppError::from)
}

/// Paste an image entry while keeping the Magpie window visible.
#[tauri::command]
#[specta::specta]
pub async fn paste_image_and_keep_window(app_handle: AppHandle, image_path: String) -> Result<(), AppError> {
    let clipboard = app_handle.state::<ClipboardPort>().inner().clone();
    clipboard.write(&WritePayload::ImageFile(image_path))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);

    paste_to_previous_app_keeping_window(&app_handle).await.map_err(AppError::from)
}

/// Paste file entries while keeping the Magpie window visible.
#[tauri::command]
#[specta::specta]
pub async fn paste_file_and_keep_window(app_handle: AppHandle, file_paths_json: String) -> Result<(), AppError> {
    let file_paths: Vec<String> = serde_json::from_str(&file_paths_json)
        .map_err(|e| AppError::Other { message: format!("Failed to parse file paths: {}", e) })?;

    let clipboard = app_handle.state::<ClipboardPort>().inner().clone();
    clipboard.write(&WritePayload::Files(file_paths))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);

    paste_to_previous_app_keeping_window(&app_handle).await.map_err(AppError::from)
}

/// Shared tail of the paste-and-keep-window commands. The content must already
/// be on the clipboard. Activates the previously-focused app, waits until it is
/// actually frontmost (instead of a fixed sleep), synthesizes Cmd+V, then
/// re-focuses Magpie. The skip-blur flag is always cleared, even on error.
async fn paste_to_previous_app_keeping_window(app_handle: &AppHandle) -> Result<(), String> {
    use std::sync::atomic::Ordering;

    let paster = app_handle.state::<PasterPort>().inner().clone();

    let target_bundle_id = {
        let prev_state = app_handle.state::<crate::PreviousAppBundleId>();
        let guard = prev_state.0.lock().map_err(|_| "previous-app lock poisoned".to_string())?;
        guard.clone()
    };
    let Some(target_bundle_id) = target_bundle_id else {
        return Err("No previous app to paste to".to_string());
    };

    // Keep the window visible while focus moves to the target app.
    let skip = app_handle.state::<crate::SkipBlurHide>();
    skip.0.store(true, Ordering::Relaxed);

    let result = async {
        if !paster.activate_app(&target_bundle_id) {
            return Err(format!("Could not activate target app: {}", target_bundle_id));
        }
        // Wait until the target app is genuinely frontmost before pasting.
        paste::wait_until_frontmost(&paster, &target_bundle_id).await;
        paster.paste()?;

        // Let the paste land, then re-focus Magpie.
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_focus();
        }
        Ok(())
    }
    .await;

    skip.0.store(false, Ordering::Relaxed);
    result
}
