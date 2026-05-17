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

/// Paste an image entry by reading the saved PNG file and writing it to the pasteboard
#[tauri::command]
pub async fn paste_image_entry(app_handle: AppHandle, image_path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // Read the PNG file
        let png_data = std::fs::read(&image_path)
            .map_err(|e| format!("Failed to read image file: {}", e))?;

        // Write PNG data to pasteboard on main thread
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = app_handle.run_on_main_thread(move || {
            use objc2_app_kit::NSPasteboard;
            use objc2_foundation::{NSData, NSString};
            use objc2::rc::autoreleasepool;

            let result = autoreleasepool(|_| {
                let pasteboard = NSPasteboard::generalPasteboard();
                pasteboard.clearContents();

                let png_type = NSString::from_str("public.png");
                let ns_data = NSData::with_bytes(&png_data);
                let success = pasteboard.setData_forType(Some(&ns_data), &png_type);
                success
            });
            let _ = tx.send(result);
        });

        let success = rx.recv().map_err(|e| e.to_string())?;
        if !success {
            return Err("Failed to write image to pasteboard".to_string());
        }

        // Hide and paste
        app_handle.hide().map_err(|e| e.to_string())?;
        wait_for_frontmost_app_switch("com.magpie.clipboard", &app_handle).await;
        paste::paste_to_active_app(&app_handle, "", false)?;
    }

    Ok(())
}

/// Copy an image entry to the clipboard without pasting
#[tauri::command]
pub fn copy_image_entry(app_handle: AppHandle, image_path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let png_data = std::fs::read(&image_path)
            .map_err(|e| format!("Failed to read image file: {}", e))?;

        let (tx, rx) = std::sync::mpsc::channel();
        let _ = app_handle.run_on_main_thread(move || {
            use objc2_app_kit::NSPasteboard;
            use objc2_foundation::{NSData, NSString};
            use objc2::rc::autoreleasepool;

            let result = autoreleasepool(|_| {
                let pasteboard = NSPasteboard::generalPasteboard();
                pasteboard.clearContents();
                let png_type = NSString::from_str("public.png");
                let ns_data = NSData::with_bytes(&png_data);
                pasteboard.setData_forType(Some(&ns_data), &png_type)
            });
            let _ = tx.send(result);
        });

        let success = rx.recv().map_err(|e| e.to_string())?;
        if !success {
            return Err("Failed to write image to pasteboard".to_string());
        }
    }

    Ok(())
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

/// Update the text content of a clipboard entry (Edit Content action)
#[tauri::command]
pub async fn update_entry_content(
    app_handle: AppHandle,
    id: i64,
    content: String,
) -> Result<(), String> {
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(DbPool::Sqlite(pool)) = instances.get("sqlite:magpie.db") {
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
             content_hash = ?, byte_size = ? WHERE id = ?"
        )
            .bind(&content)
            .bind(&preview)
            .bind(&hash)
            .bind(byte_size)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Database not available".to_string())
    }
}

/// Append text to the current clipboard content
#[tauri::command]
pub fn append_to_clipboard(app_handle: AppHandle, text: String) -> Result<(), String> {
    // Read current clipboard content
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

    app_handle
        .clipboard()
        .write_text(&combined)
        .map_err(|e| format!("Failed to write to clipboard: {}", e))
}

/// Save clipboard entry content to a file using a native save dialog
#[tauri::command]
pub async fn save_entry_as_file(
    app_handle: AppHandle,
    content: String,
    default_name: String,
) -> Result<bool, String> {

    #[cfg(target_os = "macos")]
    {
        // Use NSSavePanel on macOS
        let (tx, rx) = std::sync::mpsc::channel();
        let content_clone = content.clone();
        let default_name_clone = default_name.clone();

        let _ = app_handle.run_on_main_thread(move || {
            use objc2_app_kit::NSSavePanel;
            use objc2_foundation::{NSString, MainThreadMarker};
            use objc2::rc::autoreleasepool;

            let result = autoreleasepool(|_| {
                let mtm = MainThreadMarker::new()
                    .expect("Must be called on main thread");
                let panel = NSSavePanel::savePanel(mtm);
                let ns_name = NSString::from_str(&default_name_clone);
                panel.setNameFieldStringValue(&ns_name);
                panel.setCanCreateDirectories(true);

                let response = panel.runModal();

                // NSModalResponseOK = 1
                if response == 1 {
                    if let Some(url) = panel.URL() {
                        if let Some(path) = url.path() {
                            let path_str = path.to_string();
                            if std::fs::write(&path_str, &content_clone).is_ok() {
                                return true;
                            }
                        }
                    }
                }
                false
            });
            let _ = tx.send(result);
        });

        rx.recv().map_err(|e| e.to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Save dialog not supported on this platform".to_string())
    }
}

/// Paste content to the target app while keeping the Magpie window visible.
/// Activates the target app (window stays on screen due to always_on_top),
/// simulates Cmd+V, then re-focuses Magpie.
#[tauri::command]
pub async fn paste_and_keep_window(app_handle: AppHandle, text: String) -> Result<(), String> {
    use std::sync::atomic::Ordering;

    // 1. Write to clipboard
    app_handle
        .clipboard()
        .write_text(&text)
        .map_err(|e| format!("Failed to write to clipboard: {}", e))?;

    // 2. Get the target app's bundle_id
    let prev_state = app_handle.state::<crate::PreviousAppBundleId>();
    let bundle_id = prev_state.0.lock().unwrap().clone();
    let Some(target_bundle_id) = bundle_id else {
        return Err("No previous app to paste to".to_string());
    };

    // 3. Set skip-blur flag so the window doesn't auto-hide when we switch focus
    let skip = app_handle.state::<crate::SkipBlurHide>();
    skip.0.store(true, Ordering::Relaxed);

    // 4. Activate the target app (Magpie window stays visible because always_on_top)
    activate_app_by_bundle_id(&app_handle, &target_bundle_id);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 5. Simulate Cmd+V — goes to the now-frontmost target app
    paste::paste_to_active_app(&app_handle, &text, false)?;

    // 6. Wait for paste to land, then re-focus Magpie
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.set_focus();
    }

    // 7. Clear skip-blur flag
    skip.0.store(false, Ordering::Relaxed);

    Ok(())
}

/// Paste an image entry while keeping the Magpie window visible.
#[tauri::command]
pub async fn paste_image_and_keep_window(app_handle: AppHandle, image_path: String) -> Result<(), String> {
    use std::sync::atomic::Ordering;

    // 1. Write image to pasteboard
    #[cfg(target_os = "macos")]
    {
        let png_data = std::fs::read(&image_path)
            .map_err(|e| format!("Failed to read image file: {}", e))?;

        let (tx, rx) = std::sync::mpsc::channel();
        let _ = app_handle.run_on_main_thread(move || {
            use objc2_app_kit::NSPasteboard;
            use objc2_foundation::{NSData, NSString};
            use objc2::rc::autoreleasepool;

            let result = autoreleasepool(|_| {
                let pasteboard = NSPasteboard::generalPasteboard();
                pasteboard.clearContents();
                let png_type = NSString::from_str("public.png");
                let ns_data = NSData::with_bytes(&png_data);
                pasteboard.setData_forType(Some(&ns_data), &png_type)
            });
            let _ = tx.send(result);
        });

        let success = rx.recv().map_err(|e| e.to_string())?;
        if !success {
            return Err("Failed to write image to pasteboard".to_string());
        }
    }

    // 2. Activate target app, paste, re-focus (same as text version)
    let prev_state = app_handle.state::<crate::PreviousAppBundleId>();
    let bundle_id = prev_state.0.lock().unwrap().clone();
    let Some(target_bundle_id) = bundle_id else {
        return Err("No previous app to paste to".to_string());
    };

    let skip = app_handle.state::<crate::SkipBlurHide>();
    skip.0.store(true, Ordering::Relaxed);

    activate_app_by_bundle_id(&app_handle, &target_bundle_id);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    paste::paste_to_active_app(&app_handle, "", false)?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.set_focus();
    }

    skip.0.store(false, Ordering::Relaxed);

    Ok(())
}

/// Activate a macOS application by its bundle identifier.
/// Uses NSRunningApplication to bring the app to the foreground.
#[cfg(target_os = "macos")]
fn activate_app_by_bundle_id(app_handle: &AppHandle, bundle_id: &str) {
    use objc2_app_kit::NSRunningApplication;
    use objc2_foundation::NSString;

    let bid = bundle_id.to_string();
    let _ = app_handle.run_on_main_thread(move || {
        let ns_bid = NSString::from_str(&bid);
        let apps = unsafe {
            NSRunningApplication::runningApplicationsWithBundleIdentifier(&ns_bid)
        };
        if apps.count() > 0 {
            let app = unsafe { apps.objectAtIndex(0) };
            #[allow(deprecated)]
            let _ = unsafe {
                app.activateWithOptions(
                    objc2_app_kit::NSApplicationActivationOptions::ActivateIgnoringOtherApps,
                )
            };
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn activate_app_by_bundle_id(_app_handle: &AppHandle, _bundle_id: &str) {}

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
