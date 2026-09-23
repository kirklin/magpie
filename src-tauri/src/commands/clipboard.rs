use tauri::AppHandle;
use tauri::Manager;
use sqlx::Row;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::database::models::{ClipboardEntry, ClipboardQuery};
use crate::database::pool::get_pool;
use crate::clipboard::paste;
use crate::clipboard::thumbnail;
use crate::error::AppError;
use crate::platform::{ClipboardPort, WritePayload};

/// Resolve the image the history list should actually load for `image_path`.
///
/// Returns a small pre-scaled thumbnail when one applies, else the original
/// path. Rendering the original in a 24pt row forced the WebView to decode the
/// full bitmap (tens of MB for a screenshot) purely to throw the pixels away;
/// see `clipboard::thumbnail` for the full rationale.
///
/// Generation is lazy so the ~1000 images captured before thumbnails existed
/// get one on first display, and it runs on the blocking pool because decoding
/// and re-encoding a PNG is CPU-bound work that must not stall the async
/// runtime that also drives clipboard capture.
#[tauri::command]
#[specta::specta]
pub async fn get_thumbnail(app_handle: AppHandle, image_path: String) -> Result<String, AppError> {
    let path = tokio::task::spawn_blocking(move || {
        let src = std::path::Path::new(&image_path);
        thumbnail::ensure_thumbnail(&app_handle, src)
    })
    .await
    .map_err(|e| AppError::Other { message: e.to_string() })?
    .map_err(|message| AppError::Other { message })?;

    Ok(path.to_string_lossy().to_string())
}

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
        thumbnail::remove_for_image(&app_handle, std::path::Path::new(&path));
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
        thumbnail::remove_for_image(&app_handle, std::path::Path::new(&path));
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
    clipboard.write(&WritePayload::ImageFile(image_path))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);

    paste::paste_into_previous_app(&app_handle).await.map_err(AppError::from)
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

    // Write to clipboard, then stop the monitor re-capturing our own write.
    clipboard.write(&WritePayload::Text(text))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);

    paste::paste_into_previous_app(&app_handle).await.map_err(AppError::from)
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
    clipboard.write(&WritePayload::Text(text))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);

    paste::paste_into_previous_app(&app_handle).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn paste_file_entry(app_handle: AppHandle, file_paths_json: String) -> Result<(), AppError> {
    let file_paths: Vec<String> = serde_json::from_str(&file_paths_json)
        .map_err(|e| AppError::Other { message: format!("Failed to parse file paths: {}", e) })?;

    let clipboard = app_handle.state::<ClipboardPort>().inner().clone();
    clipboard.write(&WritePayload::Files(file_paths))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);

    paste::paste_into_previous_app(&app_handle).await.map_err(AppError::from)
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

/// Save clipboard entry content to a file using a native save dialog.
/// Returns false when the user cancelled.
#[tauri::command]
#[specta::specta]
pub async fn save_entry_as_file(
    app_handle: AppHandle,
    content: String,
    default_name: String,
) -> Result<bool, AppError> {
    match crate::dialogs::ask_save_path(&app_handle, &default_name).await? {
        Some(path) => {
            std::fs::write(&path, &content)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Paste content to the target app while keeping the Magpie window visible.
/// Activates the target app (window stays on screen due to always_on_top),
/// simulates ⌘/Ctrl+V, then re-focuses Magpie.
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
/// actually focused (instead of a fixed sleep), synthesizes ⌘/Ctrl+V, then
/// re-focuses Magpie. The skip-blur flag is always cleared, even on error.
async fn paste_to_previous_app_keeping_window(app_handle: &AppHandle) -> Result<(), String> {
    use std::sync::atomic::Ordering;

    let paster = app_handle.state::<crate::platform::PasterPort>().inner().clone();
    let loc = crate::i18n::read_locale(app_handle);
    if !paster.capabilities().can_activate_app {
        return Err(crate::i18n::tr(loc, "err.keep_window_unsupported").to_string());
    }

    let Some(target) = app_handle.state::<crate::PreviousApp>().get() else {
        return Err(crate::i18n::tr(loc, "err.no_previous_app").to_string());
    };

    // Keep the window visible while focus moves to the target app.
    let skip = app_handle.state::<crate::SkipBlurHide>();
    skip.0.store(true, Ordering::Relaxed);

    let result = async {
        if !paster.activate(&target) {
            let name = target.name.as_deref().or(target.app_id.as_deref()).unwrap_or("?");
            return Err(format!("{}{}", crate::i18n::tr(loc, "err.activate_failed"), name));
        }
        // Wait until the target is genuinely focused before pasting.
        paste::wait_until_focused(&paster, &target.focus).await;
        paster.paste()?;

        // Let the paste land, then re-focus Magpie.
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        paster.refocus_magpie();
        Ok(())
    }
    .await;

    skip.0.store(false, Ordering::Relaxed);
    result
}
