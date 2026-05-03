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
