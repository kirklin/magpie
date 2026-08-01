use tauri::{AppHandle, Manager};

use crate::database::pool::get_pool;

/// Fallbacks used when the corresponding setting is absent from settings.json.
/// Both default to -1 ("unlimited"): with no UI to configure retention yet, the
/// app must never silently delete the user's history. A user who wants a cap can
/// still set a positive value in settings.json; `prune_history` honors it.
const DEFAULT_MAX_COUNT: i64 = -1;
const DEFAULT_RETENTION_DAYS: i64 = -1;

/// Read an integer setting from the persisted settings.json store, falling back
/// to `default` when the file/key is missing or unreadable.
fn read_setting_i64(app_handle: &AppHandle, key: &str, default: i64) -> i64 {
    let Ok(dir) = app_handle.path().app_data_dir() else {
        return default;
    };
    let path = dir.join("settings.json");
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return default;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return default;
    };
    json.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
}

/// Enforce `max_history_count` and `history_retention_days`: delete the oldest
/// non-pinned entries beyond the configured limits and remove their image files
/// from disk so storage does not grow unbounded. A value of `<= 0` disables that
/// particular limit (treated as "unlimited"). Pinned entries are never pruned.
pub async fn prune_history(app_handle: &AppHandle) -> Result<(), String> {
    let max_count = read_setting_i64(app_handle, "max_history_count", DEFAULT_MAX_COUNT);
    let retention_days = read_setting_i64(app_handle, "history_retention_days", DEFAULT_RETENTION_DAYS);

    let pool = get_pool(app_handle).await.map_err(String::from)?;

    // (id, image_path) of rows to delete. A row may match both rules; the
    // second DELETE for it is simply a no-op.
    let mut victims: Vec<(i64, Option<String>)> = Vec::new();

    // Over-count prune: keep only the newest `max_count` non-pinned rows.
    if max_count > 0 {
        let rows = sqlx::query_as::<_, (i64, Option<String>)>(
            "SELECT id, image_path FROM clipboard_entries \
             WHERE is_pinned = 0 \
             ORDER BY accessed_at DESC \
             LIMIT -1 OFFSET ?",
        )
        .bind(max_count)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;
        victims.extend(rows);
    }

    // Retention prune: non-pinned rows older than the cutoff.
    if retention_days > 0 {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(retention_days))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let rows = sqlx::query_as::<_, (i64, Option<String>)>(
            "SELECT id, image_path FROM clipboard_entries \
             WHERE is_pinned = 0 AND accessed_at < ?",
        )
        .bind(&cutoff)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;
        victims.extend(rows);
    }

    // A row can match both rules; dedup by id (keeping its image path) so we
    // delete each exactly once.
    let mut by_id: std::collections::HashMap<i64, Option<String>> = std::collections::HashMap::new();
    for (id, image_path) in victims {
        by_id.entry(id).or_insert(image_path);
    }
    if by_id.is_empty() {
        return Ok(());
    }

    // Delete in chunked batch statements instead of one round-trip per row.
    // Chunked to stay well under SQLite's bound-parameter limit.
    let ids: Vec<i64> = by_id.keys().copied().collect();
    let mut deleted = 0u64;
    for chunk in ids.chunks(500) {
        let placeholders = std::iter::repeat("?").take(chunk.len()).collect::<Vec<_>>().join(",");
        let sql = format!("DELETE FROM clipboard_entries WHERE id IN ({})", placeholders);
        let mut q = sqlx::query(&sql);
        for id in chunk {
            q = q.bind(id);
        }
        let res = q.execute(&pool).await.map_err(|e| e.to_string())?;
        deleted += res.rows_affected();
    }

    // Remove the image files of the pruned rows from disk. Each image filename is
    // its content hash and UNIQUE(content_hash) means one row per file, so a
    // pruned row's image is never referenced by a surviving row.
    for path in by_id.values().flatten() {
        super::thumbnail::remove_for_image(app_handle, std::path::Path::new(path));
        let _ = std::fs::remove_file(path);
    }

    if deleted > 0 {
        log::info!("[Retention] Pruned {} entries", deleted);
    }
    Ok(())
}
