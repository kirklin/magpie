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

        loop {
            if !state_clone.is_running.load(Ordering::SeqCst) {
                log::info!("Clipboard monitor stopped");
                break;
            }

            // Wrap each poll cycle with a timeout to guarantee the monitor
            // loop never gets stuck — if any cycle takes >5s, skip and retry.
            let monitor_result = tokio::time::timeout(
                Duration::from_secs(5),
                read_clipboard_and_store(&app_handle, &state_clone, &classifier),
            ).await;

            match monitor_result {
                Ok(Ok(Some(payload))) => {
                    log::info!("[Clipboard] New entry: type={}, preview={:?}, id={}", 
                        payload.content_type,
                        payload.content_preview.as_deref().unwrap_or("?"),
                        payload.id
                    );
                    if let Err(e) = app_handle.emit("clipboard://changed", payload) {
                        log::error!("[Clipboard] Failed to emit event: {}", e);
                    }
                }
                Ok(Ok(None)) => {} // no change
                Ok(Err(e)) => {
                    log::error!("[Clipboard] Monitor error: {}", e);
                }
                Err(_) => {
                    log::error!("[Clipboard] Monitor cycle timed out (5s) — skipping this cycle");
                }
            }

            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    });
}

/// Read the current pasteboard change count (macOS).
/// IMPORTANT: NSPasteboard API must be called on the main thread.
/// We dispatch via run_on_main_thread with a timeout to avoid deadlocking.
#[cfg(target_os = "macos")]
fn get_pasteboard_change_count(app_handle: &AppHandle) -> i64 {
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
        log::warn!("[Clipboard] Failed to dispatch changeCount to main thread");
        return -1;
    }

    // 100ms timeout — changeCount is very fast, if it takes longer something is wrong
    rx.recv_timeout(std::time::Duration::from_millis(100))
        .unwrap_or_else(|_| {
            log::warn!("[Clipboard] Timed out reading changeCount");
            -1
        })
}

/// Read clipboard content and store if changed
async fn read_clipboard_and_store(
    app_handle: &AppHandle,
    state: &Arc<ClipboardMonitorState>,
    classifier: &ContentClassifier,
) -> Result<Option<ClipboardChangedPayload>, String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    // Step 1: Check if the pasteboard has actually changed using changeCount.
    // This is much more reliable than comparing content hashes.
    #[cfg(target_os = "macos")]
    {
        let current_count = get_pasteboard_change_count(app_handle);
        let last_count = state.last_change_count.load(Ordering::SeqCst);

        // -1 means we failed to read changeCount (timeout) — skip this cycle
        if current_count == -1 {
            return Ok(None);
        }

        if current_count == last_count {
            return Ok(None); // Pasteboard unchanged, skip
        }

        // Update the change count immediately so we don't re-process
        state.last_change_count.store(current_count, Ordering::SeqCst);
        log::debug!("Pasteboard changeCount: {} -> {}", last_count, current_count);
    }

    // Step 2: The pasteboard changed — determine what's on it.

    // 2a. Check for file URLs on the pasteboard first (macOS native)
    #[cfg(target_os = "macos")]
    {
        let file_paths = read_file_urls_from_pasteboard(app_handle);
        if !file_paths.is_empty() {
            return store_file_entry(app_handle, file_paths).await;
        }
    }

    // 2b. Try to read text content
    let text_result = app_handle.clipboard().read_text();

    match text_result {
        Ok(text) if !text.is_empty() => {
            store_text_entry(app_handle, classifier, text).await
        }
        _ => {
            // 2c. Try to read image
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

/// Read file URLs from macOS NSPasteboard.
/// IMPORTANT: Must dispatch to main thread since NSPasteboard is not thread-safe.
#[cfg(target_os = "macos")]
fn read_file_urls_from_pasteboard(app_handle: &AppHandle) -> Vec<String> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    let dispatched = app_handle.run_on_main_thread(move || {
        use objc2_app_kit::NSPasteboard;
        use objc2_foundation::{NSString, NSArray, NSURL};
        use objc2::rc::{autoreleasepool, Retained};

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

            // Method 1: Use NSFilenamesPboardType which returns actual file paths
            let filenames_type = NSString::from_str("NSFilenamesPboardType");
            if let Some(obj) = pasteboard.propertyListForType(&filenames_type) {
                // SAFETY: NSFilenamesPboardType always returns an NSArray of NSString file paths
                let raw_ptr = Retained::into_raw(obj) as *mut NSArray<NSString>;
                let array: Retained<NSArray<NSString>> = unsafe { Retained::from_raw(raw_ptr).unwrap() };

                let mut paths: Vec<String> = Vec::new();
                for item in array.iter() {
                    let path = item.to_string();
                    if !path.is_empty() {
                        paths.push(path);
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
        let (id, created_at, access_count, final_source_app, final_source_app_name) = match db {
            DbPool::Sqlite(pool) => {
                // Check for duplicate hash first
                let existing: Option<(i64, String, i64, Option<String>, Option<String>)> = sqlx::query_as(
                    "SELECT id, created_at, access_count, source_app, source_app_name FROM clipboard_entries WHERE content_hash = ? LIMIT 1",
                )
                .bind(&hash)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;

                if let Some((existing_id, created_at, access_count, ext_source_app, ext_source_app_name)) = existing {
                    sqlx::query(
                        "UPDATE clipboard_entries SET accessed_at = ?, access_count = access_count + 1, byte_size = ? WHERE id = ?",
                    )
                    .bind(&now)
                    .bind(byte_size)
                    .bind(existing_id)
                    .execute(pool)
                    .await
                    .map_err(|e| e.to_string())?;

                    (existing_id, created_at, access_count + 1, ext_source_app, ext_source_app_name)
                } else {
                    let result = sqlx::query(
                        "INSERT INTO clipboard_entries (content_type, text_content, file_paths, content_hash, content_preview, byte_size, source_app, source_app_name, created_at, accessed_at, access_count) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
                    )
                    .bind("file")
                    .bind(&file_paths_json) // also store as text_content for search
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

                    (result.last_insert_rowid(), now.clone(), 1, source_app, source_app_name)
                }
            }
            #[allow(unreachable_patterns)]
            _ => return Err("Unsupported database type".to_string()),
        };

        Ok(Some(ClipboardChangedPayload {
            id,
            content_type: "file".to_string(),
            text_content: Some(file_paths_json.clone()),
            content_preview: Some(preview),
            image_path: None,
            file_paths: Some(file_paths_json),
            source_app: final_source_app,
            source_app_name: final_source_app_name,
            is_pinned: false,
            created_at,
            accessed_at: now,
            access_count,
            byte_size,
        }))
    } else {
        Err("Database not initialized".to_string())
    }
}

/// Store a text clipboard entry
async fn store_text_entry(
    app_handle: &AppHandle,
    classifier: &ContentClassifier,
    text: String,
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
        let (id, created_at, access_count, final_source_app, final_source_app_name) = match db {
            DbPool::Sqlite(pool) => {
                // Check for duplicate hash first
                let existing: Option<(i64, String, i64, Option<String>, Option<String>)> = sqlx::query_as(
                    "SELECT id, created_at, access_count, source_app, source_app_name FROM clipboard_entries WHERE content_hash = ? LIMIT 1",
                )
                .bind(&hash)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;

                if let Some((existing_id, created_at, access_count, ext_source_app, ext_source_app_name)) = existing {
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
                    
                    (existing_id, created_at, access_count + 1, ext_source_app, ext_source_app_name)
                } else {
                    let result = sqlx::query(
                        "INSERT INTO clipboard_entries (content_type, text_content, content_hash, content_preview, byte_size, source_app, source_app_name, created_at, accessed_at, access_count) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
                    )
                    .bind(content_type)
                    .bind(&text)
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
                    
                    (result.last_insert_rowid(), now.clone(), 1, source_app, source_app_name)
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
            is_pinned: false,
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
    let filename = format!("{}.png", &hash[..16]);
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
        let (id, created_at, access_count, final_source_app, final_source_app_name) = match db {
            DbPool::Sqlite(pool) => {
                // Check for duplicate hash
                let existing: Option<(i64, String, i64, Option<String>, Option<String>)> = sqlx::query_as(
                    "SELECT id, created_at, access_count, source_app, source_app_name FROM clipboard_entries WHERE content_hash = ? LIMIT 1",
                )
                .bind(&hash)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;

                if let Some((existing_id, created_at, access_count, ext_source_app, ext_source_app_name)) = existing {
                    sqlx::query(
                        "UPDATE clipboard_entries SET accessed_at = ?, access_count = access_count + 1, byte_size = ? WHERE id = ?",
                    )
                    .bind(&now)
                    .bind(byte_size)
                    .bind(existing_id)
                    .execute(pool)
                    .await
                    .map_err(|e| e.to_string())?;
                    
                    (existing_id, created_at, access_count + 1, ext_source_app, ext_source_app_name)
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
                    
                    (result.last_insert_rowid(), now.clone(), 1, source_app, source_app_name)
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
            is_pinned: false,
            created_at,
            accessed_at: now,
            access_count,
            byte_size,
        }))
    } else {
        Err("Database not initialized".to_string())
    }
}

/// Encode RGBA bytes into a PNG file
fn encode_rgba_to_png(rgba: &[u8], width: u32, height: u32, path: &std::path::Path) -> Result<(), String> {
    use std::io::Write;

    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut writer = std::io::BufWriter::new(file);

    // Use a simple approach: write raw PNG
    // PNG signature
    writer.write_all(&[137, 80, 78, 71, 13, 10, 26, 10]).map_err(|e| e.to_string())?;

    // IHDR chunk
    let mut ihdr_data = Vec::new();
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.push(8); // bit depth
    ihdr_data.push(6); // color type: RGBA
    ihdr_data.push(0); // compression
    ihdr_data.push(0); // filter
    ihdr_data.push(0); // interlace
    write_png_chunk(&mut writer, b"IHDR", &ihdr_data)?;

    // IDAT chunk - we need to zlib compress the filtered image data
    // Each row has a filter byte (0 = None) followed by RGBA pixels
    let row_size = (width as usize) * 4 + 1;
    let mut raw_data = Vec::with_capacity(row_size * height as usize);
    for y in 0..height as usize {
        raw_data.push(0); // filter: None
        let row_start = y * (width as usize) * 4;
        let row_end = row_start + (width as usize) * 4;
        if row_end <= rgba.len() {
            raw_data.extend_from_slice(&rgba[row_start..row_end]);
        }
    }

    // Simple zlib wrapper: header + deflate stored blocks + adler32
    let compressed = zlib_compress_stored(&raw_data);
    write_png_chunk(&mut writer, b"IDAT", &compressed)?;

    // IEND chunk
    write_png_chunk(&mut writer, b"IEND", &[])?;

    writer.flush().map_err(|e| e.to_string())?;
    Ok(())
}

fn write_png_chunk(writer: &mut impl std::io::Write, chunk_type: &[u8; 4], data: &[u8]) -> Result<(), String> {
    let len = data.len() as u32;
    writer.write_all(&len.to_be_bytes()).map_err(|e| e.to_string())?;
    writer.write_all(chunk_type).map_err(|e| e.to_string())?;
    writer.write_all(data).map_err(|e| e.to_string())?;

    // CRC32 over chunk_type + data
    let mut crc_data = Vec::with_capacity(4 + data.len());
    crc_data.extend_from_slice(chunk_type);
    crc_data.extend_from_slice(data);
    let crc = crc32(&crc_data);
    writer.write_all(&crc.to_be_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

fn zlib_compress_stored(data: &[u8]) -> Vec<u8> {
    // zlib header: CM=8, CINFO=7, FCHECK adjusted
    let mut out = Vec::new();
    out.push(0x78); // CMF
    out.push(0x01); // FLG (no dict, FLEVEL=0)

    // Deflate stored blocks
    let chunks: Vec<&[u8]> = data.chunks(65535).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let is_last = i == chunks.len() - 1;
        out.push(if is_last { 0x01 } else { 0x00 }); // BFINAL + BTYPE=00 (stored)
        let len = chunk.len() as u16;
        let nlen = !len;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(chunk);
    }

    // Adler32 checksum
    let adler = adler32(data);
    out.extend_from_slice(&adler.to_be_bytes());

    out
}

fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
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
