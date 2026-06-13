use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tauri_plugin_sql::{DbInstances, DbPool};

use crate::error::AppError;

/// The single connection-key for the app database. Centralized here so the
/// literal isn't repeated across every DB-touching command.
pub const DB_KEY: &str = "sqlite:magpie.db";

/// Central accessor for the app's SQLite pool.
///
/// Clones the pool handle (a cheap `Arc` bump) and drops the `DbInstances`
/// read-guard before returning, so callers don't hold the lock across their
/// queries. Returns [`AppError::DbUnavailable`] if the plugin hasn't opened the
/// database yet (e.g. during early startup).
pub async fn get_pool(app: &AppHandle) -> Result<SqlitePool, AppError> {
    let instances = app.state::<DbInstances>();
    let instances = instances.0.read().await;
    match instances.get(DB_KEY) {
        Some(DbPool::Sqlite(pool)) => Ok(pool.clone()),
        _ => Err(AppError::DbUnavailable),
    }
}
