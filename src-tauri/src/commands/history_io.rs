//! JSON export/import of the clipboard history, split out of the command god
//! file. The native save/open dialogs live in clipboard::native.

use tauri::AppHandle;
use sqlx::Row;

use crate::database::pool::get_pool;
#[cfg(target_os = "macos")]
use crate::clipboard::native;

/// Serializable export format for clipboard entries
#[derive(serde::Serialize, serde::Deserialize)]
struct ExportedEntry {
    content_type: String,
    text_content: Option<String>,
    html_content: Option<String>,
    image_path: Option<String>,
    file_paths: Option<String>,
    source_app: Option<String>,
    source_app_name: Option<String>,
    custom_name: Option<String>,
    is_pinned: bool,
    is_favorite: bool,
    content_hash: String,
    content_preview: Option<String>,
    byte_size: i64,
    created_at: String,
    accessed_at: String,
    access_count: i32,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ExportData {
    version: u32,
    app: String,
    exported_at: String,
    entries: Vec<ExportedEntry>,
}

/// Export clipboard history to a JSON file via native save dialog.
/// Returns the number of entries exported, or 0 if the user cancelled.
#[tauri::command]
#[specta::specta]
pub async fn export_clipboard_history(app_handle: AppHandle) -> Result<u32, String> {
    // 1. Read all entries from the database
    let pool = get_pool(&app_handle).await.map_err(String::from)?;

    let rows = sqlx::query(
        "SELECT content_type, text_content, html_content, image_path, file_paths, \
         source_app, source_app_name, custom_name, is_pinned, is_favorite, content_hash, \
         content_preview, byte_size, created_at, accessed_at, access_count \
         FROM clipboard_entries ORDER BY accessed_at DESC"
    )
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let entries: Vec<ExportedEntry> = rows.iter().map(|row| ExportedEntry {
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
    }).collect();

    let count = entries.len() as u32;

    let export_data = ExportData {
        version: 1,
        app: "Magpie".to_string(),
        exported_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        entries,
    };

    let json = serde_json::to_string_pretty(&export_data)
        .map_err(|e| format!("Failed to serialize: {}", e))?;

    // 2. Show native save dialog and write the file to the chosen path.
    #[cfg(target_os = "macos")]
    {
        let default_name = format!(
            "magpie-export-{}.json",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        );
        match native::run_save_panel(&app_handle, &default_name) {
            Some(path) if std::fs::write(&path, &json).is_ok() => Ok(count),
            _ => Ok(0), // cancelled or write failed
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Export not supported on this platform".to_string())
    }
}

/// Import clipboard history from a JSON file via native open dialog.
/// Returns the number of entries imported (skipping duplicates).
#[tauri::command]
#[specta::specta]
pub async fn import_clipboard_history(app_handle: AppHandle) -> Result<u32, String> {
    // 1. Show native open dialog to pick a JSON file and read it.
    let json_content: String;

    #[cfg(target_os = "macos")]
    {
        match native::run_open_panel(&app_handle, "选择 Magpie 导出文件") {
            Some(path) => match std::fs::read_to_string(&path) {
                Ok(content) => json_content = content,
                Err(_) => return Ok(0),
            },
            None => return Ok(0), // User cancelled
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        return Err("Import not supported on this platform".to_string());
    }

    // 2. Parse the JSON
    let export_data: ExportData = serde_json::from_str(&json_content)
        .map_err(|e| format!("无法解析导入文件: {}", e))?;

    if export_data.app != "Magpie" {
        return Err("不是有效的 Magpie 导出文件".to_string());
    }

    // 3. Insert entries into the database, skipping duplicates
    let pool = get_pool(&app_handle).await.map_err(String::from)?;

    let mut imported_count = 0u32;

    for entry in &export_data.entries {
        // Insert, skipping rows whose content_hash already exists. ON CONFLICT
        // DO NOTHING tolerates both pre-existing rows and duplicates within the
        // import file; a conflict affects 0 rows, so it isn't counted.
        let result = sqlx::query(
            "INSERT INTO clipboard_entries \
             (content_type, text_content, html_content, image_path, file_paths, \
              source_app, source_app_name, custom_name, is_pinned, is_favorite, \
              content_hash, content_preview, byte_size, created_at, accessed_at, access_count) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(content_hash) DO NOTHING"
        )
            .bind(&entry.content_type)
            .bind(&entry.text_content)
            .bind(&entry.html_content)
            .bind(&entry.image_path)
            .bind(&entry.file_paths)
            .bind(&entry.source_app)
            .bind(&entry.source_app_name)
            .bind(&entry.custom_name)
            .bind(entry.is_pinned)
            .bind(entry.is_favorite)
            .bind(&entry.content_hash)
            .bind(&entry.content_preview)
            .bind(entry.byte_size)
            .bind(&entry.created_at)
            .bind(&entry.accessed_at)
            .bind(entry.access_count)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;

        if result.rows_affected() > 0 {
            imported_count += 1;
        }
    }

    Ok(imported_count)
}
