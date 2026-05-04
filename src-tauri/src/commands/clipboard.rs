use tauri::AppHandle;
use tauri_plugin_sql::{DbInstances, DbPool};
use tauri::Manager;
use sqlx::Row;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::database::models::{ClipboardEntry, ClipboardQuery};
use crate::clipboard::paste;

#[tauri::command]
pub async fn get_clipboard_entries(
    app_handle: AppHandle,
    query: ClipboardQuery,
) -> Result<Vec<ClipboardEntry>, String> {
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(DbPool::Sqlite(pool)) = instances.get("sqlite:magpie.db") {
        let mut sql = String::from(
            "SELECT id, content_type, text_content, html_content, image_path, file_paths, \
             source_app, source_app_name, custom_name, is_pinned, is_favorite, content_hash, \
             content_preview, byte_size, created_at, accessed_at, access_count \
             FROM clipboard_entries WHERE 1=1"
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

        let rows = query_builder
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

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
    } else {
        Err("Database not available".to_string())
    }
}

#[tauri::command]
pub async fn delete_clipboard_entry(app_handle: AppHandle, id: i64) -> Result<(), String> {
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(DbPool::Sqlite(pool)) = instances.get("sqlite:magpie.db") {
        sqlx::query("DELETE FROM clipboard_entries WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Database not available".to_string())
    }
}

#[tauri::command]
pub async fn clear_clipboard_history(app_handle: AppHandle) -> Result<(), String> {
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(DbPool::Sqlite(pool)) = instances.get("sqlite:magpie.db") {
        sqlx::query("DELETE FROM clipboard_entries WHERE is_pinned = 0")
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Database not available".to_string())
    }
}

#[tauri::command]
pub async fn toggle_pin_entry(app_handle: AppHandle, id: i64) -> Result<bool, String> {
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(DbPool::Sqlite(pool)) = instances.get("sqlite:magpie.db") {
        sqlx::query("UPDATE clipboard_entries SET is_pinned = NOT is_pinned WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

        let row = sqlx::query("SELECT is_pinned FROM clipboard_entries WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok(row.get::<bool, _>("is_pinned"))
    } else {
        Err("Database not available".to_string())
    }
}

#[tauri::command]
pub async fn rename_clipboard_entry(
    app_handle: AppHandle,
    id: i64,
    name: String,
) -> Result<(), String> {
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(DbPool::Sqlite(pool)) = instances.get("sqlite:magpie.db") {
        sqlx::query("UPDATE clipboard_entries SET custom_name = ? WHERE id = ?")
            .bind(&name)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Database not available".to_string())
    }
}

#[tauri::command]
pub async fn paste_clipboard_entry(app_handle: AppHandle, text: String) -> Result<(), String> {
    // 1. Write to clipboard
    app_handle
        .clipboard()
        .write_text(&text)
        .map_err(|e| format!("Failed to write to clipboard: {}", e))?;

    // 2. Hide the entire application (returns focus to previous app)
    app_handle.hide().map_err(|e| e.to_string())?;

    // 3. Wait for macOS to complete the focus switch by actively reading the active app
    wait_for_frontmost_app_switch("com.magpie.clipboard", &app_handle).await;

    // 4. Simulate Cmd+V
    paste::paste_to_active_app(&app_handle, &text, false)
}

#[tauri::command]
pub fn copy_clipboard_entry(app_handle: AppHandle, text: String) -> Result<(), String> {
    paste::copy_to_clipboard(&app_handle, &text)
}

#[tauri::command]
pub async fn paste_as_plain_text(app_handle: AppHandle, text: String) -> Result<(), String> {
    app_handle
        .clipboard()
        .write_text(&text)
        .map_err(|e| format!("Failed to write to clipboard: {}", e))?;

    app_handle.hide().map_err(|e| e.to_string())?;
    
    // Wait for frontmost app switch
    wait_for_frontmost_app_switch("com.magpie.clipboard", &app_handle).await;

    paste::paste_to_active_app(&app_handle, &text, true)
}

#[tauri::command]
pub async fn paste_file_entry(app_handle: AppHandle, file_paths_json: String) -> Result<(), String> {
    let file_paths: Vec<String> = serde_json::from_str(&file_paths_json)
        .map_err(|e| format!("Failed to parse file paths: {}", e))?;

    // Write file URLs to the pasteboard using native API
    #[cfg(target_os = "macos")]
    {
        write_files_to_pasteboard(&file_paths)?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Fallback: write file paths as text
        app_handle
            .clipboard()
            .write_text(&file_paths.join("\n"))
            .map_err(|e| format!("Failed to write to clipboard: {}", e))?;
    }

    // Hide the app and paste
    app_handle.hide().map_err(|e| e.to_string())?;
    wait_for_frontmost_app_switch("com.magpie.clipboard", &app_handle).await;
    paste::paste_to_active_app(&app_handle, "", false)
}

#[tauri::command]
pub fn copy_file_entry(file_paths_json: String) -> Result<(), String> {
    let file_paths: Vec<String> = serde_json::from_str(&file_paths_json)
        .map_err(|e| format!("Failed to parse file paths: {}", e))?;

    #[cfg(target_os = "macos")]
    {
        write_files_to_pasteboard(&file_paths)?;
    }

    Ok(())
}

/// Write file paths to macOS NSPasteboard as file URLs
#[cfg(target_os = "macos")]
fn write_files_to_pasteboard(file_paths: &[String]) -> Result<(), String> {
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::{NSString, NSArray};
    use objc2::rc::autoreleasepool;

    autoreleasepool(|_| {
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();

        // Declare NSFilenamesPboardType and public.file-url
        let filenames_type = NSString::from_str("NSFilenamesPboardType");
        let file_url_type = NSString::from_str("public.file-url");
        let types = NSArray::from_retained_slice(&[
            NSString::from_str("NSFilenamesPboardType"),
            NSString::from_str("public.file-url"),
        ]);
        // SAFETY: declaring pasteboard types with no owner is safe
        unsafe { pasteboard.declareTypes_owner(&types, None) };

        // Build an NSArray of NSString paths for the property list
        let ns_paths: Vec<_> = file_paths.iter()
            .map(|p| NSString::from_str(p))
            .collect();
        let ns_array = NSArray::from_retained_slice(&ns_paths);

        // Set the property list (array of file paths) for the filenames type
        // SAFETY: we're passing a valid NSArray<NSString> which matches NSFilenamesPboardType's expected format
        let success = unsafe { pasteboard.setPropertyList_forType(&ns_array, &filenames_type) };

        // Also set the first file as a file URL for apps that prefer public.file-url
        if let Some(first_path) = file_paths.first() {
            let encoded = format!("file://{}", first_path.replace(' ', "%20"));
            let url_str = NSString::from_str(&encoded);
            pasteboard.setString_forType(&url_str, &file_url_type);
        }

        if success {
            Ok(())
        } else {
            Err("Failed to write file paths to pasteboard".to_string())
        }
    })
}

/// Polls until the frontmost application is NOT the specified bundle ID
async fn wait_for_frontmost_app_switch(ignore_bundle_id: &str, app_handle: &tauri::AppHandle) {
    let mut retries = 0;
    while retries < 50 { // max 500ms
        let (bundle_id, _) = paste::get_frontmost_app(app_handle);
        if let Some(id) = bundle_id {
            if id != ignore_bundle_id {
                log::debug!("Active app switched to: {}", id);
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        retries += 1;
    }
}
