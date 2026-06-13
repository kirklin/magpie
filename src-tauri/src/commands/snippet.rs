use tauri::AppHandle;
use tauri_plugin_sql::{DbInstances, DbPool};
use tauri::Manager;
use sqlx::Row;

use crate::database::models::{Snippet, SnippetFolder};

#[tauri::command]
#[specta::specta]
pub async fn get_snippets(
    app_handle: AppHandle,
    folder_id: Option<i32>,
    search: Option<String>,
) -> Result<Vec<Snippet>, String> {
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(DbPool::Sqlite(pool)) = instances.get("sqlite:magpie.db") {
        let mut sql = String::from(
            "SELECT id, name, content, keyword, folder_id, tags, is_pinned, created_at, updated_at \
             FROM snippets WHERE 1=1"
        );
        let mut bind_values: Vec<String> = vec![];

        if let Some(fid) = folder_id {
            sql.push_str(" AND folder_id = ?");
            bind_values.push(fid.to_string());
        }

        if let Some(ref s) = search {
            sql.push_str(" AND (name LIKE ? OR content LIKE ? OR keyword LIKE ?)");
            let pattern = format!("%{}%", s);
            bind_values.push(pattern.clone());
            bind_values.push(pattern.clone());
            bind_values.push(pattern);
        }

        sql.push_str(" ORDER BY is_pinned DESC, updated_at DESC");

        let mut query = sqlx::query(&sql);

        for val in &bind_values {
            query = query.bind(val);
        }

        let rows = query.fetch_all(pool).await.map_err(|e| e.to_string())?;

        Ok(rows
            .iter()
            .map(|r| Snippet {
                id: r.get("id"),
                name: r.get("name"),
                content: r.get("content"),
                keyword: r.get("keyword"),
                folder_id: r.get("folder_id"),
                tags: r.get("tags"),
                is_pinned: r.get("is_pinned"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect())
    } else {
        Err("Database not available".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub async fn create_snippet(
    app_handle: AppHandle,
    name: String,
    content: String,
    keyword: Option<String>,
    folder_id: Option<i32>,
    tags: Option<String>,
) -> Result<i32, String> {
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(DbPool::Sqlite(pool)) = instances.get("sqlite:magpie.db") {
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let result = sqlx::query(
            "INSERT INTO snippets (name, content, keyword, folder_id, tags, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&name)
        .bind(&content)
        .bind(&keyword)
        .bind(folder_id)
        .bind(&tags)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        Ok(result.last_insert_rowid() as i32)
    } else {
        Err("Database not available".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub async fn update_snippet(
    app_handle: AppHandle,
    id: i32,
    name: String,
    content: String,
    keyword: Option<String>,
    folder_id: Option<i32>,
    tags: Option<String>,
) -> Result<(), String> {
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(DbPool::Sqlite(pool)) = instances.get("sqlite:magpie.db") {
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        sqlx::query(
            "UPDATE snippets SET name = ?, content = ?, keyword = ?, folder_id = ?, tags = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&name)
        .bind(&content)
        .bind(&keyword)
        .bind(folder_id)
        .bind(&tags)
        .bind(&now)
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
#[specta::specta]
pub async fn delete_snippet(app_handle: AppHandle, id: i32) -> Result<(), String> {
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(DbPool::Sqlite(pool)) = instances.get("sqlite:magpie.db") {
        sqlx::query("DELETE FROM snippets WHERE id = ?")
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
#[specta::specta]
pub async fn get_snippet_folders(app_handle: AppHandle) -> Result<Vec<SnippetFolder>, String> {
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(DbPool::Sqlite(pool)) = instances.get("sqlite:magpie.db") {
        let rows = sqlx::query(
            "SELECT id, name, parent_id, sort_order, created_at FROM snippet_folders ORDER BY sort_order ASC",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        Ok(rows
            .iter()
            .map(|r| SnippetFolder {
                id: r.get("id"),
                name: r.get("name"),
                parent_id: r.get("parent_id"),
                sort_order: r.get("sort_order"),
                created_at: r.get("created_at"),
            })
            .collect())
    } else {
        Err("Database not available".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub async fn create_snippet_folder(
    app_handle: AppHandle,
    name: String,
    parent_id: Option<i32>,
) -> Result<i32, String> {
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(DbPool::Sqlite(pool)) = instances.get("sqlite:magpie.db") {
        let result = sqlx::query(
            "INSERT INTO snippet_folders (name, parent_id) VALUES (?, ?)",
        )
        .bind(&name)
        .bind(parent_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        Ok(result.last_insert_rowid() as i32)
    } else {
        Err("Database not available".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub async fn delete_snippet_folder(app_handle: AppHandle, id: i32) -> Result<(), String> {
    let db_instances = app_handle.state::<DbInstances>();
    let instances = db_instances.0.read().await;

    if let Some(DbPool::Sqlite(pool)) = instances.get("sqlite:magpie.db") {
        // Move snippets in this folder to no folder
        sqlx::query("UPDATE snippets SET folder_id = NULL WHERE folder_id = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

        sqlx::query("DELETE FROM snippet_folders WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    } else {
        Err("Database not available".to_string())
    }
}

/// Convert a clipboard entry to a snippet
#[tauri::command]
#[specta::specta]
pub async fn save_as_snippet(
    app_handle: AppHandle,
    name: String,
    content: String,
    folder_id: Option<i32>,
) -> Result<i32, String> {
    create_snippet(app_handle, name, content, None, folder_id, None).await
}
