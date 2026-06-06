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
        // Capture the image path first so we can remove the file after the row.
        let image_path: Option<String> = sqlx::query_scalar(
            "SELECT image_path FROM clipboard_entries WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .flatten();

        sqlx::query("DELETE FROM clipboard_entries WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(path) = image_path {
            let _ = std::fs::remove_file(path);
        }
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
        // Collect image files of the rows we're about to delete so they don't
        // become orphaned on disk.
        let image_paths: Vec<String> = sqlx::query_scalar(
            "SELECT image_path FROM clipboard_entries \
             WHERE is_pinned = 0 AND image_path IS NOT NULL",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        sqlx::query("DELETE FROM clipboard_entries WHERE is_pinned = 0")
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

        for path in image_paths {
            let _ = std::fs::remove_file(path);
        }
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
        crate::clipboard::monitor::mark_self_write(&app_handle);

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
        crate::clipboard::monitor::mark_self_write(&app_handle);
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
    // Don't let the monitor re-capture our own write.
    crate::clipboard::monitor::mark_self_write(&app_handle);

    // 2. Hide the entire application (returns focus to previous app)
    app_handle.hide().map_err(|e| e.to_string())?;

    // 3. Wait for macOS to complete the focus switch by actively reading the active app
    wait_for_frontmost_app_switch("com.magpie.clipboard", &app_handle).await;

    // 4. Simulate Cmd+V
    paste::paste_to_active_app(&app_handle, &text, false)
}

#[tauri::command]
pub fn copy_clipboard_entry(app_handle: AppHandle, text: String) -> Result<(), String> {
    paste::copy_to_clipboard(&app_handle, &text)?;
    crate::clipboard::monitor::mark_self_write(&app_handle);
    Ok(())
}

#[tauri::command]
pub async fn paste_as_plain_text(app_handle: AppHandle, text: String) -> Result<(), String> {
    app_handle
        .clipboard()
        .write_text(&text)
        .map_err(|e| format!("Failed to write to clipboard: {}", e))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);

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
    crate::clipboard::monitor::mark_self_write(&app_handle);

    // Hide the app and paste
    app_handle.hide().map_err(|e| e.to_string())?;
    wait_for_frontmost_app_switch("com.magpie.clipboard", &app_handle).await;
    paste::paste_to_active_app(&app_handle, "", false)
}

#[tauri::command]
pub fn copy_file_entry(app_handle: AppHandle, file_paths_json: String) -> Result<(), String> {
    let file_paths: Vec<String> = serde_json::from_str(&file_paths_json)
        .map_err(|e| format!("Failed to parse file paths: {}", e))?;

    #[cfg(target_os = "macos")]
    {
        write_files_to_pasteboard(&file_paths)?;
    }

    crate::clipboard::monitor::mark_self_write(&app_handle);
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
        .map_err(|e| format!("Failed to write to clipboard: {}", e))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);
    Ok(())
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
    app_handle
        .clipboard()
        .write_text(&text)
        .map_err(|e| format!("Failed to write to clipboard: {}", e))?;
    crate::clipboard::monitor::mark_self_write(&app_handle);

    paste_to_previous_app_keeping_window(&app_handle).await
}

/// Paste an image entry while keeping the Magpie window visible.
#[tauri::command]
pub async fn paste_image_and_keep_window(app_handle: AppHandle, image_path: String) -> Result<(), String> {
    // Write image to pasteboard
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
        crate::clipboard::monitor::mark_self_write(&app_handle);
    }

    paste_to_previous_app_keeping_window(&app_handle).await
}

/// Paste file entries while keeping the Magpie window visible.
#[tauri::command]
pub async fn paste_file_and_keep_window(app_handle: AppHandle, file_paths_json: String) -> Result<(), String> {
    let file_paths: Vec<String> = serde_json::from_str(&file_paths_json)
        .map_err(|e| format!("Failed to parse file paths: {}", e))?;

    // Write file URLs to pasteboard
    #[cfg(target_os = "macos")]
    {
        write_files_to_pasteboard(&file_paths)?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        app_handle
            .clipboard()
            .write_text(&file_paths.join("\n"))
            .map_err(|e| format!("Failed to write to clipboard: {}", e))?;
    }
    crate::clipboard::monitor::mark_self_write(&app_handle);

    paste_to_previous_app_keeping_window(&app_handle).await
}

/// Shared tail of the paste-and-keep-window commands. The content must already
/// be on the clipboard. Activates the previously-focused app, waits until it is
/// actually frontmost (instead of a fixed sleep), synthesizes Cmd+V, then
/// re-focuses Magpie. The skip-blur flag is always cleared, even on error.
async fn paste_to_previous_app_keeping_window(app_handle: &AppHandle) -> Result<(), String> {
    use std::sync::atomic::Ordering;

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
        if !activate_app_by_bundle_id(app_handle, &target_bundle_id) {
            return Err(format!("Could not activate target app: {}", target_bundle_id));
        }
        // Wait until the target app is genuinely frontmost before pasting.
        wait_until_frontmost(&target_bundle_id, app_handle).await;
        paste::paste_to_active_app(app_handle, "", false)?;

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

/// Poll until `target_bundle_id` is the frontmost app (up to ~500ms), then add
/// a short settle delay. Returns whether it became frontmost.
async fn wait_until_frontmost(target_bundle_id: &str, app_handle: &AppHandle) -> bool {
    for _ in 0..50 {
        if let (Some(id), _) = paste::get_frontmost_app(app_handle) {
            if id == target_bundle_id {
                tokio::time::sleep(std::time::Duration::from_millis(40)).await;
                return true;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    log::warn!("Target app {} never became frontmost before paste", target_bundle_id);
    false
}

/// Activate a macOS application by its bundle identifier.
/// Uses NSRunningApplication to bring the app to the foreground.
/// Returns whether a running app with that bundle id was found and activated.
#[cfg(target_os = "macos")]
fn activate_app_by_bundle_id(app_handle: &AppHandle, bundle_id: &str) -> bool {
    use objc2_app_kit::NSRunningApplication;
    use objc2_foundation::NSString;
    use std::sync::mpsc;

    let bid = bundle_id.to_string();
    let (tx, rx) = mpsc::channel();
    let dispatched = app_handle.run_on_main_thread(move || {
        let ns_bid = NSString::from_str(&bid);
        let apps = unsafe {
            NSRunningApplication::runningApplicationsWithBundleIdentifier(&ns_bid)
        };
        let activated = if apps.count() > 0 {
            let app = unsafe { apps.objectAtIndex(0) };
            #[allow(deprecated)]
            let _ = unsafe {
                app.activateWithOptions(
                    objc2_app_kit::NSApplicationActivationOptions::ActivateIgnoringOtherApps,
                )
            };
            true
        } else {
            false
        };
        let _ = tx.send(activated);
    });

    if dispatched.is_err() {
        return false;
    }
    rx.recv_timeout(std::time::Duration::from_millis(300)).unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
fn activate_app_by_bundle_id(_app_handle: &AppHandle, _bundle_id: &str) -> bool {
    false
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

/// Polls until the frontmost application is NOT the specified bundle ID, then
/// adds a short settle delay so the newly-focused app is ready to receive the
/// synthesized Cmd+V. Returns whether the focus actually switched.
async fn wait_for_frontmost_app_switch(ignore_bundle_id: &str, app_handle: &tauri::AppHandle) -> bool {
    let mut switched = false;
    for _ in 0..50 { // max ~500ms
        let (bundle_id, _) = paste::get_frontmost_app(app_handle);
        if let Some(id) = bundle_id {
            if id != ignore_bundle_id {
                log::debug!("Active app switched to: {}", id);
                switched = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    if switched {
        // Give the now-frontmost app a moment to become first responder before
        // we synthesize the paste keystroke — without this the Cmd+V can race
        // the focus change and be dropped or land in Magpie.
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    } else {
        log::warn!("Frontmost app never switched away from {} before paste", ignore_bundle_id);
    }

    switched
}

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
pub async fn export_clipboard_history(app_handle: AppHandle) -> Result<u32, String> {
    // 1. Read all entries from the database
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    let pool = match instances.get("sqlite:magpie.db") {
        Some(DbPool::Sqlite(pool)) => pool.clone(),
        _ => return Err("Database not available".to_string()),
    };

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

    // 2. Show native save dialog
    #[cfg(target_os = "macos")]
    {
        let default_name = format!(
            "magpie-export-{}.json",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        );

        let (tx, rx) = std::sync::mpsc::channel();

        let _ = app_handle.run_on_main_thread(move || {
            use objc2_app_kit::NSSavePanel;
            use objc2_foundation::{NSString, MainThreadMarker};
            use objc2::rc::autoreleasepool;

            let result = autoreleasepool(|_| {
                let mtm = MainThreadMarker::new()
                    .expect("Must be called on main thread");
                let panel = NSSavePanel::savePanel(mtm);
                let ns_name = NSString::from_str(&default_name);
                panel.setNameFieldStringValue(&ns_name);
                panel.setCanCreateDirectories(true);

                let response = panel.runModal();

                if response == 1 {
                    if let Some(url) = panel.URL() {
                        if let Some(path) = url.path() {
                            let path_str = path.to_string();
                            if std::fs::write(&path_str, &json).is_ok() {
                                return count;
                            }
                        }
                    }
                }
                0u32
            });
            let _ = tx.send(result);
        });

        rx.recv().map_err(|e| e.to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Export not supported on this platform".to_string())
    }
}

/// Import clipboard history from a JSON file via native open dialog.
/// Returns the number of entries imported (skipping duplicates).
#[tauri::command]
pub async fn import_clipboard_history(app_handle: AppHandle) -> Result<u32, String> {
    // 1. Show native open dialog to pick a JSON file
    let json_content: String;

    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel();

        let _ = app_handle.run_on_main_thread(move || {
            use objc2_app_kit::NSOpenPanel;
            use objc2_foundation::{NSString, MainThreadMarker};
            use objc2::rc::autoreleasepool;

            let result: Option<String> = autoreleasepool(|_| {
                let mtm = MainThreadMarker::new()
                    .expect("Must be called on main thread");
                let panel = NSOpenPanel::openPanel(mtm);
                panel.setCanChooseFiles(true);
                panel.setCanChooseDirectories(false);
                panel.setAllowsMultipleSelection(false);
                let ns_title = NSString::from_str("选择 Magpie 导出文件");
                panel.setMessage(Some(&ns_title));

                let response = panel.runModal();

                if response == 1 {
                    if let Some(url) = panel.URL() {
                        if let Some(path) = url.path() {
                            let path_str = path.to_string();
                            if let Ok(content) = std::fs::read_to_string(&path_str) {
                                return Some(content);
                            }
                        }
                    }
                }
                None
            });
            let _ = tx.send(result);
        });

        match rx.recv().map_err(|e| e.to_string())? {
            Some(content) => json_content = content,
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
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    let pool = match instances.get("sqlite:magpie.db") {
        Some(DbPool::Sqlite(pool)) => pool.clone(),
        _ => return Err("Database not available".to_string()),
    };

    let mut imported_count = 0u32;

    for entry in &export_data.entries {
        // Check if entry with this content_hash already exists
        let exists = sqlx::query("SELECT 1 FROM clipboard_entries WHERE content_hash = ?")
            .bind(&entry.content_hash)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

        if exists.is_some() {
            continue; // Skip duplicate
        }

        sqlx::query(
            "INSERT INTO clipboard_entries \
             (content_type, text_content, html_content, image_path, file_paths, \
              source_app, source_app_name, custom_name, is_pinned, is_favorite, \
              content_hash, content_preview, byte_size, created_at, accessed_at, access_count) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
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

        imported_count += 1;
    }

    Ok(imported_count)
}
