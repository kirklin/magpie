use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use sha2::{Sha256, Digest};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_sql::{DbInstances, DbPool};

use super::classifier::ContentClassifier;

/// Clipboard monitor state shared across threads
pub struct ClipboardMonitorState {
    pub last_change_count: AtomicI64,
    pub is_running: std::sync::atomic::AtomicBool,
}

impl Default for ClipboardMonitorState {
    fn default() -> Self {
        Self {
            last_change_count: AtomicI64::new(0),
            is_running: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

/// Payload emitted when clipboard changes
#[derive(Clone, serde::Serialize)]
pub struct ClipboardChangedPayload {
    pub id: i64,
    pub content_type: String,
    pub text_content: Option<String>,
    pub content_preview: Option<String>,
    pub image_path: Option<String>,
    pub file_paths: Option<String>,
    pub source_app: Option<String>,
    pub source_app_name: Option<String>,
    pub is_pinned: bool,
    pub created_at: String,
    pub accessed_at: String,
    pub access_count: i64,
    pub byte_size: i64,
}

/// Start the clipboard monitoring loop
pub fn start_monitor(app_handle: AppHandle) {
    let state = app_handle
        .state::<Arc<ClipboardMonitorState>>();

    if state.is_running.load(Ordering::SeqCst) {
        log::info!("Clipboard monitor already running");
        return;
    }

    state.is_running.store(true, Ordering::SeqCst);
    let state_clone = Arc::clone(&state);
    let classifier = ContentClassifier::new();
    // classifier is wrapped in Arc inside the loop below

    tauri::async_runtime::spawn(async move {
        log::info!("Clipboard monitor started, waiting for DB...");

        // First, ensure the database is loaded by opening it
        {
            let db_instances = app_handle.state::<DbInstances>();
            let instances = db_instances.0.read().await;
            if instances.get("sqlite:magpie.db").is_none() {
                log::warn!("Database not yet loaded, will try opening via plugin...");
                drop(instances);
                let mut retries = 0;
                loop {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    let instances = db_instances.0.read().await;
                    if instances.get("sqlite:magpie.db").is_some() {
                        log::info!("Database is now available!");
                        break;
                    }
                    retries += 1;
                    if retries > 30 {
                        log::error!("Database still not available after 15s, giving up");
                        return;
                    }
                }
            } else {
                log::info!("Database already available");
            }
        }

        // Ensure clipboard_images directory exists
        if let Some(app_data_dir) = app_handle.path().app_data_dir().ok() {
            let images_dir = app_data_dir.join("clipboard_images");
            let _ = std::fs::create_dir_all(&images_dir);
        }

        log::info!("Clipboard monitor polling started");

        let classifier = Arc::new(classifier);

        // Enforce history size/retention limits periodically, OFF the capture hot
        // path. Pruning on every copy added DB latency that, combined with the old
        // in-flight guard, dropped fast successive copies.
        {
            let app = app_handle.clone();
            let running = Arc::clone(&state_clone);
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    if !running.is_running.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Err(e) = super::retention::prune_history(&app).await {
                        log::warn!("[Retention] prune failed: {}", e);
                    }
                }
            });
        }

        loop {
            if !state_clone.is_running.load(Ordering::SeqCst) {
                log::info!("Clipboard monitor stopped");
                break;
            }

            // Read the current pasteboard change count (an integer).
            if let Some(current) = current_change_count(&app_handle) {
                let last = state_clone.last_change_count.load(Ordering::SeqCst);
                if current != last && current >= 0 {
                    // Claim the change immediately. This stops the next poll from
                    // re-spawning a task for the same change (no task storm), while
                    // still letting rapid successive copies each spawn their own
                    // reader — serializing captures here (the old in-flight guard)
                    // dropped fast successive copies, which is what regressed.
                    state_clone.last_change_count.store(current, Ordering::SeqCst);
                    log::debug!("Pasteboard changeCount: {} -> {}", last, current);

                    // Spawn processing in a separate task so we NEVER block the poller.
                    let app = app_handle.clone();
                    let cls = classifier.clone();
                    let st = Arc::clone(&state_clone);
                    tokio::spawn(async move {
                        let result = tokio::time::timeout(
                            Duration::from_secs(5),
                            read_clipboard_and_store(&app, &cls),
                        ).await;

                        match result {
                            Ok(Ok(Some(payload))) => {
                                log::info!("[Clipboard] New entry: type={}, preview={:?}, id={}",
                                    payload.content_type,
                                    payload.content_preview.as_deref().unwrap_or("?"),
                                    payload.id
                                );
                                if let Err(e) = app.emit("clipboard://changed", payload) {
                                    log::error!("[Clipboard] Failed to emit event: {}", e);
                                }
                            }
                            // Nothing storable (unrecognized / concealed content):
                            // leave the count claimed so we don't retry it forever.
                            Ok(Ok(None)) => {}
                            // Genuine failure/timeout: roll the count back so the next
                            // poll retries this change — but only if no newer change
                            // has been claimed since, so we never clobber a newer copy.
                            Ok(Err(e)) => {
                                let _ = st.last_change_count.compare_exchange(
                                    current, last, Ordering::SeqCst, Ordering::SeqCst,
                                );
                                log::error!("[Clipboard] Process error (will retry): {}", e);
                            }
                            Err(_) => {
                                let _ = st.last_change_count.compare_exchange(
                                    current, last, Ordering::SeqCst, Ordering::SeqCst,
                                );
                                log::error!("[Clipboard] Process timed out (5s, will retry)");
                            }
                        }
                    });
                }
            }

            // 50ms polling — the per-tick check is just an integer comparison
            // dispatched to the main thread. All heavy work is spawned above.
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
}

/// Read the current pasteboard change count (an integer) on the main thread.
/// Returns `None` if the dispatch or read fails/times out — the caller treats
/// `None` as "no reliable reading this tick" and simply retries next poll,
/// rather than mistaking it for "no change".
#[cfg(target_os = "macos")]
fn current_change_count(app_handle: &AppHandle) -> Option<i64> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    let dispatched = app_handle.run_on_main_thread(move || {
        use objc2_app_kit::NSPasteboard;
        use objc2::rc::autoreleasepool;

        let count = autoreleasepool(|_| {
            let pasteboard = NSPasteboard::generalPasteboard();
            pasteboard.changeCount() as i64
        });
        let _ = tx.send(count);
    });

    if dispatched.is_err() {
        return None;
    }

    rx.recv_timeout(std::time::Duration::from_millis(100)).ok()
}

#[cfg(not(target_os = "macos"))]
fn current_change_count(_app_handle: &AppHandle) -> Option<i64> {
    None
}

/// Mark the current pasteboard state as "already seen" by the monitor.
///
/// Call this immediately after Magpie itself writes to the clipboard (paste or
/// copy actions). Otherwise the monitor detects that write as a brand-new
/// clipboard change and re-captures it — creating spurious history entries and
/// bumping the just-used item to the top in a feedback loop.
pub fn mark_self_write(app_handle: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        if let Some(count) = current_change_count(app_handle) {
            let state = app_handle.state::<Arc<ClipboardMonitorState>>();
            state.last_change_count.store(count, Ordering::SeqCst);
        }
    }
}

/// Read clipboard content and store. Called AFTER change is detected.
async fn read_clipboard_and_store(
    app_handle: &AppHandle,
    classifier: &ContentClassifier,
) -> Result<Option<ClipboardChangedPayload>, String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    // Step 2: The pasteboard changed — determine what's on it.

    // 2.0 Respect the de-facto-standard "concealed/transient" pasteboard markers
    // that password managers and similar apps set, so we never persist passwords
    // or other sensitive, short-lived content.
    #[cfg(target_os = "macos")]
    {
        if pasteboard_is_concealed(app_handle) {
            log::debug!("Skipping concealed/transient pasteboard content");
            return Ok(None);
        }
    }

    // 2a. Check for file URLs on the pasteboard first (macOS native)
    #[cfg(target_os = "macos")]
    {
        let file_paths = read_file_urls_from_pasteboard(app_handle);
        if !file_paths.is_empty() {
            return store_file_entry(app_handle, file_paths).await;
        }
    }

    // 2b. Try to read plain text content
    let text_result = app_handle.clipboard().read_text();

    match text_result {
        Ok(text) if !text.is_empty() => {
            store_text_entry(app_handle, classifier, text, None).await
        }
        _ => {
            // 2c. Fallback: rich content that exposes only an HTML flavor with no
            // plain-text representation (some editors / web apps). Capture the
            // HTML and store a stripped plain-text version for display/search.
            #[cfg(target_os = "macos")]
            {
                if let Some(html) = read_html_from_pasteboard(app_handle) {
                    let plain = html_to_plain_text(&html);
                    if !plain.is_empty() {
                        return store_text_entry(app_handle, classifier, plain, Some(html)).await;
                    }
                }
            }

            // 2d. Try to read image
            match app_handle.clipboard().read_image() {
                Ok(image_data) => {
                    store_image_entry(app_handle, image_data).await
                }
                Err(_) => {
                    log::debug!("Pasteboard changed but no recognizable content found");
                    Ok(None)
                },
            }
        }
    }
}

/// Read an HTML representation from the pasteboard, if present.
#[cfg(target_os = "macos")]
fn read_html_from_pasteboard(app_handle: &AppHandle) -> Option<String> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    let dispatched = app_handle.run_on_main_thread(move || {
        use objc2_app_kit::NSPasteboard;
        use objc2_foundation::NSString;
        use objc2::rc::autoreleasepool;

        let html = autoreleasepool(|_| {
            let pasteboard = NSPasteboard::generalPasteboard();
            let html_type = NSString::from_str("public.html");
            pasteboard
                .stringForType(&html_type)
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
        });
        let _ = tx.send(html);
    });

    if dispatched.is_err() {
        return None;
    }

    rx.recv_timeout(std::time::Duration::from_millis(150)).ok().flatten()
}

/// Strip HTML markup down to a readable plain-text approximation.
#[cfg(target_os = "macos")]
fn html_to_plain_text(html: &str) -> String {
    use regex::Regex;

    // Drop <script>/<style> blocks entirely, then all remaining tags.
    let re = Regex::new(r"(?is)<(script|style)\b[^>]*>.*?</(script|style)>|<[^>]+>").unwrap();
    let stripped = re.replace_all(html, " ");
    let decoded = stripped
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    // Collapse runs of whitespace.
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Returns true if the general pasteboard carries one of the standard markers
/// used to indicate sensitive/transient content that should not be recorded
/// (e.g. passwords copied from a password manager).
#[cfg(target_os = "macos")]
fn pasteboard_is_concealed(app_handle: &AppHandle) -> bool {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    let dispatched = app_handle.run_on_main_thread(move || {
        use objc2_app_kit::NSPasteboard;
        use objc2::rc::autoreleasepool;

        let concealed = autoreleasepool(|_| {
            let pasteboard = NSPasteboard::generalPasteboard();
            NSPasteboard::types(&pasteboard).is_some_and(|types| {
                types.iter().any(|t| {
                    matches!(
                        t.to_string().as_str(),
                        "org.nspasteboard.ConcealedType"
                            | "org.nspasteboard.TransientType"
                            | "org.nspasteboard.AutoGeneratedType"
                    )
                })
            })
        });
        let _ = tx.send(concealed);
    });

    if dispatched.is_err() {
        return false;
    }

    rx.recv_timeout(std::time::Duration::from_millis(100))
        .unwrap_or(false)
}

/// Read file URLs from macOS NSPasteboard.
/// IMPORTANT: Must dispatch to main thread since NSPasteboard is not thread-safe.
#[cfg(target_os = "macos")]
fn read_file_urls_from_pasteboard(app_handle: &AppHandle) -> Vec<String> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    let dispatched = app_handle.run_on_main_thread(move || {
        use objc2_app_kit::NSPasteboard;
        use objc2_foundation::{NSString, NSArray, NSURL};
        use objc2::rc::autoreleasepool;

        let result = autoreleasepool(|_| {
            let pasteboard = NSPasteboard::generalPasteboard();

            // Check if the pasteboard contains file URLs
            let types = NSPasteboard::types(&pasteboard);
            let has_file_url = types.map_or(false, |ts| {
                ts.iter().any(|t| {
                    let s = t.to_string();
                    s == "public.file-url" || s == "NSFilenamesPboardType"
                })
            });

            if !has_file_url {
                return vec![];
            }

            // Method 1: Use NSFilenamesPboardType which returns actual file paths.
            // propertyListForType returns an untyped object; downcast it (a runtime
            // class check) instead of transmuting + unwrapping, so a malformed
            // pasteboard falls through to Method 2 rather than risking UB on iter.
            let filenames_type = NSString::from_str("NSFilenamesPboardType");
            if let Some(array) = pasteboard
                .propertyListForType(&filenames_type)
                .and_then(|obj| obj.downcast::<NSArray>().ok())
            {
                let mut paths: Vec<String> = Vec::new();
                for item in array.iter() {
                    if let Some(path) = item.downcast_ref::<NSString>() {
                        let p = path.to_string();
                        if !p.is_empty() {
                            paths.push(p);
                        }
                    }
                }

                if !paths.is_empty() {
                    return paths;
                }
            }

            // Method 2: Read NSURL from pasteboard items and resolve via NSURL
            let file_url_type = NSString::from_str("public.file-url");
            if let Some(items) = pasteboard.pasteboardItems() {
                let mut paths: Vec<String> = Vec::new();
                for item in items.iter() {
                    if let Some(url_string) = item.stringForType(&file_url_type) {
                        let ns_url_str = url_string.to_string();
                        let ns_str = NSString::from_str(&ns_url_str);
                        if let Some(url) = NSURL::URLWithString(&ns_str) {
                            if let Some(file_path_url) = url.filePathURL() {
                                if let Some(path) = file_path_url.path() {
                                    let p = path.to_string();
                                    if !p.is_empty() {
                                        paths.push(p);
                                        continue;
                                    }
                                }
                            }
                            if let Some(path) = url.path() {
                                let p = path.to_string();
                                if !p.is_empty() {
                                    paths.push(p);
                                }
                            }
                        }
                    }
                }
                if !paths.is_empty() {
                    return paths;
                }
            }

            vec![]
        });
        let _ = tx.send(result);
    });

    if dispatched.is_err() {
        return vec![];
    }

    rx.recv_timeout(std::time::Duration::from_millis(200))
        .unwrap_or_default()
}

/// Store a file clipboard entry
async fn store_file_entry(
    app_handle: &AppHandle,
    file_paths: Vec<String>,
) -> Result<Option<ClipboardChangedPayload>, String> {
    let file_paths_json = serde_json::to_string(&file_paths).map_err(|e| e.to_string())?;
    // Human-readable text for search/display (the raw JSON lives in file_paths).
    let file_paths_text = file_paths.join("\n");

    // Hash the file paths for database dedup only
    let mut hasher = Sha256::new();
    hasher.update(file_paths_json.as_bytes());
    let result = hasher.finalize();
    let hash: String = result.iter().map(|b| format!("{:02x}", b)).collect();

    // Generate preview from file names
    let preview = if file_paths.len() == 1 {
        // Show the filename for single file
        std::path::Path::new(&file_paths[0])
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| file_paths[0].clone())
    } else {
        format!("{} files", file_paths.len())
    };

    // Calculate actual file sizes from filesystem
    let byte_size: i64 = file_paths.iter()
        .map(|p| std::fs::metadata(p).map(|m| m.len() as i64).unwrap_or(0))
        .sum();
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let (source_app, source_app_name) = get_frontmost_app_async(app_handle).await;

    // Insert into database
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(db) = instances.get("sqlite:magpie.db") {
        let (id, created_at, access_count, is_pinned, final_source_app, final_source_app_name) = match db {
            DbPool::Sqlite(pool) => {
                // Check for duplicate hash first
                let existing: Option<(i64, String, i64, bool, Option<String>, Option<String>)> = sqlx::query_as(
                    "SELECT id, created_at, access_count, is_pinned, source_app, source_app_name FROM clipboard_entries WHERE content_hash = ? LIMIT 1",
                )
                .bind(&hash)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;

                if let Some((existing_id, created_at, access_count, is_pinned, ext_source_app, ext_source_app_name)) = existing {
                    sqlx::query(
                        "UPDATE clipboard_entries SET accessed_at = ?, access_count = access_count + 1, byte_size = ? WHERE id = ?",
                    )
                    .bind(&now)
                    .bind(byte_size)
                    .bind(existing_id)
                    .execute(pool)
                    .await
                    .map_err(|e| e.to_string())?;

                    (existing_id, created_at, access_count + 1, is_pinned, ext_source_app, ext_source_app_name)
                } else {
                    let result = sqlx::query(
                        "INSERT INTO clipboard_entries (content_type, text_content, file_paths, content_hash, content_preview, byte_size, source_app, source_app_name, created_at, accessed_at, access_count) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
                    )
                    .bind("file")
                    .bind(&file_paths_text) // human-readable paths for search/display
                    .bind(&file_paths_json)
                    .bind(&hash)
                    .bind(&preview)
                    .bind(byte_size)
                    .bind(&source_app)
                    .bind(&source_app_name)
                    .bind(&now)
                    .bind(&now)
                    .execute(pool)
                    .await
                    .map_err(|e| e.to_string())?;

                    (result.last_insert_rowid(), now.clone(), 1, false, source_app, source_app_name)
                }
            }
            #[allow(unreachable_patterns)]
            _ => return Err("Unsupported database type".to_string()),
        };

        Ok(Some(ClipboardChangedPayload {
            id,
            content_type: "file".to_string(),
            text_content: Some(file_paths_text),
            content_preview: Some(preview),
            image_path: None,
            file_paths: Some(file_paths_json),
            source_app: final_source_app,
            source_app_name: final_source_app_name,
            is_pinned,
            created_at,
            accessed_at: now,
            access_count,
            byte_size,
        }))
    } else {
        Err("Database not initialized".to_string())
    }
}

/// Store a text clipboard entry. `html` carries the original HTML markup when
/// the content was captured from a rich-text/HTML-only source.
async fn store_text_entry(
    app_handle: &AppHandle,
    classifier: &ContentClassifier,
    text: String,
    html: Option<String>,
) -> Result<Option<ClipboardChangedPayload>, String> {
    // Hash the content for database dedup only
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let result = hasher.finalize();
    let hash: String = result.iter().map(|b| format!("{:02x}", b)).collect();

    // Classify content type
    let content_type = classifier.classify_text(&text);
    let preview = ContentClassifier::generate_preview(&text, 100);
    let byte_size = text.len() as i64;
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // Get the source app (frontmost app)
    let (source_app, source_app_name) = get_frontmost_app_async(app_handle).await;

    // Insert into database
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(db) = instances.get("sqlite:magpie.db") {
        let (id, created_at, access_count, is_pinned, final_source_app, final_source_app_name) = match db {
            DbPool::Sqlite(pool) => {
                // Check for duplicate hash first
                let existing: Option<(i64, String, i64, bool, Option<String>, Option<String>)> = sqlx::query_as(
                    "SELECT id, created_at, access_count, is_pinned, source_app, source_app_name FROM clipboard_entries WHERE content_hash = ? LIMIT 1",
                )
                .bind(&hash)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;

                if let Some((existing_id, created_at, access_count, is_pinned, ext_source_app, ext_source_app_name)) = existing {
                    // Update accessed_at and access_count
                    sqlx::query(
                        "UPDATE clipboard_entries SET accessed_at = ?, access_count = access_count + 1, byte_size = ? WHERE id = ?",
                    )
                    .bind(&now)
                    .bind(byte_size)
                    .bind(existing_id)
                    .execute(pool)
                    .await
                    .map_err(|e| e.to_string())?;
                    
                    (existing_id, created_at, access_count + 1, is_pinned, ext_source_app, ext_source_app_name)
                } else {
                    let result = sqlx::query(
                        "INSERT INTO clipboard_entries (content_type, text_content, html_content, content_hash, content_preview, byte_size, source_app, source_app_name, created_at, accessed_at, access_count) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
                    )
                    .bind(content_type)
                    .bind(&text)
                    .bind(&html)
                    .bind(&hash)
                    .bind(&preview)
                    .bind(byte_size)
                    .bind(&source_app)
                    .bind(&source_app_name)
                    .bind(&now)
                    .bind(&now)
                    .execute(pool)
                    .await
                    .map_err(|e| e.to_string())?;

                    (result.last_insert_rowid(), now.clone(), 1, false, source_app, source_app_name)
                }
            }
            #[allow(unreachable_patterns)]
            _ => return Err("Unsupported database type".to_string()),
        };

        Ok(Some(ClipboardChangedPayload {
            id,
            content_type: content_type.to_string(),
            text_content: Some(text),
            content_preview: Some(preview),
            image_path: None,
            file_paths: None,
            source_app: final_source_app,
            source_app_name: final_source_app_name,
            is_pinned,
            created_at,
            accessed_at: now,
            access_count,
            byte_size,
        }))
    } else {
        Err("Database not initialized".to_string())
    }
}

/// Store an image clipboard entry
async fn store_image_entry(
    app_handle: &AppHandle,
    image_data: tauri::image::Image<'_>,
) -> Result<Option<ClipboardChangedPayload>, String> {
    let rgba_bytes = image_data.rgba();
    if rgba_bytes.is_empty() {
        return Ok(None);
    }

    // Hash the image bytes for database dedup only
    let mut hasher = Sha256::new();
    hasher.update(&rgba_bytes);
    let result = hasher.finalize();
    let hash: String = result.iter().map(|b| format!("{:02x}", b)).collect();

    // Save image to disk
    let app_data_dir = app_handle.path().app_data_dir()
        .map_err(|e| e.to_string())?;
    let images_dir = app_data_dir.join("clipboard_images");
    let _ = std::fs::create_dir_all(&images_dir);

    let width = image_data.width();
    let height = image_data.height();
    // Use the full content hash for the filename so it matches the DB dedup key
    // and two distinct images can never collide on a 16-char prefix.
    let filename = format!("{}.png", hash);
    let file_path = images_dir.join(&filename);

    // Encode to PNG using a minimal PNG encoder
    encode_rgba_to_png(&rgba_bytes, width, height, &file_path)
        .map_err(|e| format!("Failed to save image: {}", e))?;

    let file_path_str = file_path.to_string_lossy().to_string();
    let preview = format!("Image ({}×{})", width, height);
    let byte_size = std::fs::metadata(&file_path).map(|m| m.len() as i64).unwrap_or(rgba_bytes.len() as i64);
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let (source_app, source_app_name) = get_frontmost_app_async(app_handle).await;

    // Insert into database
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(db) = instances.get("sqlite:magpie.db") {
        let (id, created_at, access_count, is_pinned, final_source_app, final_source_app_name) = match db {
            DbPool::Sqlite(pool) => {
                // Check for duplicate hash
                let existing: Option<(i64, String, i64, bool, Option<String>, Option<String>)> = sqlx::query_as(
                    "SELECT id, created_at, access_count, is_pinned, source_app, source_app_name FROM clipboard_entries WHERE content_hash = ? LIMIT 1",
                )
                .bind(&hash)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;

                if let Some((existing_id, created_at, access_count, is_pinned, ext_source_app, ext_source_app_name)) = existing {
                    sqlx::query(
                        "UPDATE clipboard_entries SET accessed_at = ?, access_count = access_count + 1, byte_size = ? WHERE id = ?",
                    )
                    .bind(&now)
                    .bind(byte_size)
                    .bind(existing_id)
                    .execute(pool)
                    .await
                    .map_err(|e| e.to_string())?;
                    
                    (existing_id, created_at, access_count + 1, is_pinned, ext_source_app, ext_source_app_name)
                } else {
                    let result = sqlx::query(
                        "INSERT INTO clipboard_entries (content_type, image_path, content_hash, content_preview, byte_size, source_app, source_app_name, created_at, accessed_at, access_count) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
                    )
                    .bind("image")
                    .bind(&file_path_str)
                    .bind(&hash)
                    .bind(&preview)
                    .bind(byte_size)
                    .bind(&source_app)
                    .bind(&source_app_name)
                    .bind(&now)
                    .bind(&now)
                    .execute(pool)
                    .await
                    .map_err(|e| e.to_string())?;
                    
                    (result.last_insert_rowid(), now.clone(), 1, false, source_app, source_app_name)
                }
            }
            #[allow(unreachable_patterns)]
            _ => return Err("Unsupported database type".to_string()),
        };

        Ok(Some(ClipboardChangedPayload {
            id,
            content_type: "image".to_string(),
            text_content: None,
            content_preview: Some(preview),
            image_path: Some(file_path_str),
            file_paths: None,
            source_app: final_source_app,
            source_app_name: final_source_app_name,
            is_pinned,
            created_at,
            accessed_at: now,
            access_count,
            byte_size,
        }))
    } else {
        Err("Database not initialized".to_string())
    }
}

/// Encode RGBA bytes into a (properly compressed) PNG file using the `png` crate.
fn encode_rgba_to_png(rgba: &[u8], width: u32, height: u32, path: &std::path::Path) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let writer = std::io::BufWriter::new(file);

    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut png_writer = encoder.write_header().map_err(|e| e.to_string())?;
    png_writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    png_writer.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Get the frontmost application info on macOS.
/// Fully async — runs the blocking main-thread dispatch inside spawn_blocking
/// with an outer tokio timeout, so the monitor loop is never blocked.
async fn get_frontmost_app_async(app_handle: &AppHandle) -> (Option<String>, Option<String>) {
    #[cfg(target_os = "macos")]
    {
        let handle = app_handle.clone();
        let result = tokio::time::timeout(
            Duration::from_millis(300),
            tokio::task::spawn_blocking(move || {
                use std::sync::mpsc;
                use objc2_app_kit::NSWorkspace;
                use objc2::rc::autoreleasepool;

                let (tx, rx) = mpsc::channel();
                let dispatched = handle.run_on_main_thread(move || {
                    let result = autoreleasepool(|_| {
                        let workspace = NSWorkspace::sharedWorkspace();
                        if let Some(app) = workspace.frontmostApplication() {
                            let bundle_id = app
                                .bundleIdentifier()
                                .map(|s| s.to_string());
                            let name = app
                                .localizedName()
                                .map(|s| s.to_string());
                            (bundle_id, name)
                        } else {
                            (None, None)
                        }
                    });
                    let _ = tx.send(result);
                });

                if dispatched.is_err() {
                    return (None, None);
                }

                rx.recv_timeout(std::time::Duration::from_millis(200))
                    .unwrap_or((None, None))
            })
        ).await;

        match result {
            Ok(Ok(v)) => v,
            _ => {
                log::warn!("[Clipboard] Timed out or failed getting frontmost app");
                (None, None)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        (None, None)
    }
}
