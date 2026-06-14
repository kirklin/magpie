use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use sha2::{Sha256, Digest};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_sql::DbInstances;

use super::classifier::ContentClassifier;
use crate::database::pool::get_pool;
use crate::error::AppError;
use crate::platform::{AppInfo, Captured, ClipboardPort, PasterPort};

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
    let clipboard: ClipboardPort = app_handle.state::<ClipboardPort>().inner().clone();
    let paster: PasterPort = app_handle.state::<PasterPort>().inner().clone();

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

        // Switch the database to WAL so the capture writer and the UI/retention
        // readers don't block each other. WAL is a persistent file-header setting,
        // so running it once on any pooled connection sticks for all current and
        // future connections. It must be a runtime PRAGMA, NOT a migration: sqlx
        // wraps each migration's SQL in a transaction and SQLite refuses to switch
        // journal_mode inside one. (foreign_keys=ON and busy_timeout=5000 are
        // already applied per-connection by sqlx's defaults, so only WAL is needed.)
        match get_pool(&app_handle).await {
            Ok(pool) => match sqlx::query("PRAGMA journal_mode=WAL;").fetch_optional(&pool).await {
                Ok(_) => log::info!("[Monitor] journal_mode=WAL enabled"),
                Err(e) => log::warn!("[Monitor] failed to enable WAL: {e}"),
            },
            Err(e) => log::warn!("[Monitor] could not acquire pool to enable WAL: {e}"),
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

            // Read the current clipboard change token (an integer) via the
            // platform port. macOS implements this as NSPasteboard.changeCount.
            if let Some(current) = clipboard.change_token() {
                let last = state_clone.last_change_count.load(Ordering::SeqCst);
                if current != last && current >= 0 {
                    // Claim the change immediately. This stops the next poll from
                    // re-spawning a task for the same change (no task storm), while
                    // still letting rapid successive copies each spawn their own
                    // reader — serializing captures here (the old in-flight guard)
                    // dropped fast successive copies, which is what regressed.
                    state_clone.last_change_count.store(current, Ordering::SeqCst);
                    log::debug!("Clipboard change token: {} -> {}", last, current);

                    // Spawn processing in a separate task so we NEVER block the poller.
                    let app = app_handle.clone();
                    let cls = classifier.clone();
                    let st = Arc::clone(&state_clone);
                    let clip = clipboard.clone();
                    let pst = paster.clone();
                    tokio::spawn(async move {
                        let result = tokio::time::timeout(
                            Duration::from_secs(5),
                            async {
                                // Read the whole clipboard as one snapshot via the
                                // platform port. None = sensitive/unrecognized.
                                let Some(captured) = clip.read() else {
                                    return Ok(None);
                                };
                                // Attribute the source app once, right after the
                                // read, so it can't drift from the content.
                                let source = pst.frontmost_app();
                                store_captured(&app, &cls, captured, source).await
                            },
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

/// Mark the current clipboard state as "already seen" by the monitor.
///
/// Call this immediately after Magpie itself writes to the clipboard (paste or
/// copy actions). Otherwise the monitor detects that write as a brand-new
/// clipboard change and re-captures it — creating spurious history entries and
/// bumping the just-used item to the top in a feedback loop.
pub fn mark_self_write(app_handle: &AppHandle) {
    let clipboard = app_handle.state::<ClipboardPort>();
    if let Some(token) = clipboard.change_token() {
        let state = app_handle.state::<Arc<ClipboardMonitorState>>();
        state.last_change_count.store(token, Ordering::SeqCst);
    }
}

/// Dispatch a captured clipboard snapshot to the right store routine. The
/// source app has already been attributed by the caller.
async fn store_captured(
    app_handle: &AppHandle,
    classifier: &ContentClassifier,
    captured: Captured,
    source: AppInfo,
) -> Result<Option<ClipboardChangedPayload>, String> {
    match captured {
        Captured::Files { paths } => store_file_entry(app_handle, paths, source).await,
        Captured::Text { text, html } => {
            store_text_entry(app_handle, classifier, text, html, source).await
        }
        Captured::Image { rgba, width, height } => {
            store_image_entry(app_handle, rgba, width, height, source).await
        }
    }
}

/// Store a file clipboard entry
/// The DB-authoritative fields of a stored entry, read back via RETURNING.
/// On a dedup hit these reflect the ORIGINAL row (created_at, is_pinned,
/// source_app, source_app_name are not overwritten), which is exactly the
/// behavior the old SELECT-then-UPDATE produced.
struct UpsertResult {
    id: i64,
    created_at: String,
    accessed_at: String,
    access_count: i64,
    is_pinned: bool,
    source_app: Option<String>,
    source_app_name: Option<String>,
    byte_size: i64,
}

/// Insert a new clipboard entry, or on a content_hash conflict bump its
/// accessed_at / access_count / byte_size — atomically, in one statement.
/// This replaces the racy SELECT-then-INSERT (two concurrent identical captures
/// could both miss the SELECT and double-insert; UNIQUE(content_hash) + ON
/// CONFLICT closes that window). DO UPDATE touches ONLY accessed_at/access_count/
/// byte_size, so created_at/is_pinned/source_app/source_app_name/content keep
/// their original values, and every payload field comes from RETURNING.
#[allow(clippy::too_many_arguments)]
async fn upsert_entry(
    pool: &SqlitePool,
    content_type: &str,
    text_content: Option<&str>,
    html_content: Option<&str>,
    image_path: Option<&str>,
    file_paths: Option<&str>,
    content_hash: &str,
    content_preview: &str,
    byte_size: i64,
    source_app: Option<&str>,
    source_app_name: Option<&str>,
    now: &str,
) -> Result<UpsertResult, AppError> {
    let (id, created_at, accessed_at, access_count, is_pinned, ret_source_app, ret_source_app_name, ret_byte_size):
        (i64, String, String, i64, bool, Option<String>, Option<String>, i64) = sqlx::query_as(
        "INSERT INTO clipboard_entries
            (content_type, text_content, html_content, image_path, file_paths,
             content_hash, content_preview, byte_size, source_app, source_app_name,
             created_at, accessed_at, access_count)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)
         ON CONFLICT(content_hash) DO UPDATE SET
            accessed_at = excluded.accessed_at,
            access_count = clipboard_entries.access_count + 1,
            byte_size = excluded.byte_size
         RETURNING id, created_at, accessed_at, access_count, is_pinned,
                   source_app, source_app_name, byte_size",
    )
    .bind(content_type)
    .bind(text_content)
    .bind(html_content)
    .bind(image_path)
    .bind(file_paths)
    .bind(content_hash)
    .bind(content_preview)
    .bind(byte_size)
    .bind(source_app)
    .bind(source_app_name)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;

    Ok(UpsertResult {
        id,
        created_at,
        accessed_at,
        access_count,
        is_pinned,
        source_app: ret_source_app,
        source_app_name: ret_source_app_name,
        byte_size: ret_byte_size,
    })
}

async fn store_file_entry(
    app_handle: &AppHandle,
    file_paths: Vec<String>,
    source: AppInfo,
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

    // Upsert into database (atomic dedup on content_hash).
    let pool = get_pool(app_handle).await.map_err(String::from)?;
    let stored = upsert_entry(
        &pool,
        "file",
        Some(&file_paths_text), // human-readable paths for search/display
        None,
        None,
        Some(&file_paths_json),
        &hash,
        &preview,
        byte_size,
        source.bundle_id.as_deref(),
        source.name.as_deref(),
        &now,
    )
    .await
    .map_err(String::from)?;

    Ok(Some(ClipboardChangedPayload {
        id: stored.id,
        content_type: "file".to_string(),
        text_content: Some(file_paths_text),
        content_preview: Some(preview),
        image_path: None,
        file_paths: Some(file_paths_json),
        source_app: stored.source_app,
        source_app_name: stored.source_app_name,
        is_pinned: stored.is_pinned,
        created_at: stored.created_at,
        accessed_at: stored.accessed_at,
        access_count: stored.access_count,
        byte_size: stored.byte_size,
    }))
}

/// Store a text clipboard entry. `html` carries the original HTML markup when
/// the content was captured from a rich-text/HTML-only source.
async fn store_text_entry(
    app_handle: &AppHandle,
    classifier: &ContentClassifier,
    text: String,
    html: Option<String>,
    source: AppInfo,
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

    // Upsert into database (atomic dedup on content_hash).
    let pool = get_pool(app_handle).await.map_err(String::from)?;
    let stored = upsert_entry(
        &pool,
        content_type,
        Some(&text),
        html.as_deref(),
        None,
        None,
        &hash,
        &preview,
        byte_size,
        source.bundle_id.as_deref(),
        source.name.as_deref(),
        &now,
    )
    .await
    .map_err(String::from)?;

    Ok(Some(ClipboardChangedPayload {
        id: stored.id,
        content_type: content_type.to_string(),
        text_content: Some(text),
        content_preview: Some(preview),
        image_path: None,
        file_paths: None,
        source_app: stored.source_app,
        source_app_name: stored.source_app_name,
        is_pinned: stored.is_pinned,
        created_at: stored.created_at,
        accessed_at: stored.accessed_at,
        access_count: stored.access_count,
        byte_size: stored.byte_size,
    }))
}

/// Store an image clipboard entry from a raw RGBA bitmap.
async fn store_image_entry(
    app_handle: &AppHandle,
    rgba_bytes: Vec<u8>,
    width: u32,
    height: u32,
    source: AppInfo,
) -> Result<Option<ClipboardChangedPayload>, String> {
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

    // Upsert into database (atomic dedup on content_hash). The PNG is already
    // written above; its filename is the content hash, so a dedup hit references
    // the byte-identical file that's already on disk.
    let pool = get_pool(app_handle).await.map_err(String::from)?;
    let stored = upsert_entry(
        &pool,
        "image",
        None,
        None,
        Some(&file_path_str),
        None,
        &hash,
        &preview,
        byte_size,
        source.bundle_id.as_deref(),
        source.name.as_deref(),
        &now,
    )
    .await
    .map_err(String::from)?;

    Ok(Some(ClipboardChangedPayload {
        id: stored.id,
        content_type: "image".to_string(),
        text_content: None,
        content_preview: Some(preview),
        image_path: Some(file_path_str),
        file_paths: None,
        source_app: stored.source_app,
        source_app_name: stored.source_app_name,
        is_pinned: stored.is_pinned,
        created_at: stored.created_at,
        accessed_at: stored.accessed_at,
        access_count: stored.access_count,
        byte_size: stored.byte_size,
    }))
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use crate::database::repository::get_migrations;

    /// Single-connection in-memory pool with the v1 schema + v3 UNIQUE index
    /// applied (so ON CONFLICT(content_hash) has an index to fire against).
    async fn pool_with_schema() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for v in [1_i64, 3] {
            let sql = get_migrations().into_iter().find(|m| m.version == v).unwrap().sql;
            sqlx::raw_sql(sql).execute(&pool).await.unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn upsert_inserts_then_bumps_on_conflict_keeping_original_fields() {
        let pool = pool_with_schema().await;

        let first = upsert_entry(
            &pool, "text", Some("hi"), None, None, None, "hash1", "hi", 2,
            Some("com.a"), Some("AppA"), "2026-01-01 00:00:00",
        ).await.unwrap();
        assert_eq!(first.access_count, 1);
        assert_eq!(first.source_app.as_deref(), Some("com.a"));

        // Re-copy the SAME content from a DIFFERENT app at a later time: a dedup
        // hit must keep the original id/created_at/source_app and bump the count,
        // while refreshing accessed_at and byte_size.
        let second = upsert_entry(
            &pool, "text", Some("hi"), None, None, None, "hash1", "hi", 9,
            Some("com.b"), Some("AppB"), "2026-02-02 00:00:00",
        ).await.unwrap();
        assert_eq!(second.id, first.id, "same row");
        assert_eq!(second.access_count, 2, "access_count bumped, not reset to 1");
        assert_eq!(second.created_at, first.created_at, "created_at preserved on hit");
        assert_eq!(second.source_app.as_deref(), Some("com.a"), "original source_app preserved on hit");
        assert_eq!(second.accessed_at, "2026-02-02 00:00:00", "accessed_at refreshed");
        assert_eq!(second.byte_size, 9, "byte_size refreshed");

        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM clipboard_entries")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(cnt, 1, "still a single row");
    }
}
