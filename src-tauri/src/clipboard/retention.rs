use tauri::{AppHandle, Manager};
use tauri_plugin_sql::{DbInstances, DbPool};

/// Fallbacks used when the corresponding setting is absent from settings.json.
const DEFAULT_MAX_COUNT: i64 = 5000;
const DEFAULT_RETENTION_DAYS: i64 = 30;

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

    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;
    let Some(DbPool::Sqlite(pool)) = instances.get("sqlite:magpie.db") else {
        return Err("Database not available".to_string());
    };

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
        .fetch_all(pool)
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
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
        victims.extend(rows);
    }

    if victims.is_empty() {
        return Ok(());
    }

    let mut deleted = 0u32;
    for (id, image_path) in &victims {
        let result = sqlx::query("DELETE FROM clipboard_entries WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await;
        if let Ok(r) = result {
            if r.rows_affected() > 0 {
                deleted += 1;
                if let Some(path) = image_path {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }

    if deleted > 0 {
        log::info!("[Retention] Pruned {} entries", deleted);
    }
    Ok(())
}
