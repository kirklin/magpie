use tauri_plugin_sql::{Migration, MigrationKind};

/// Database migrations for clipboard history and snippets
pub fn get_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "create clipboard_entries table",
            sql: "CREATE TABLE IF NOT EXISTS clipboard_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL DEFAULT 'text',
                text_content TEXT,
                html_content TEXT,
                image_path TEXT,
                file_paths TEXT,
                source_app TEXT,
                source_app_name TEXT,
                custom_name TEXT,
                is_pinned INTEGER NOT NULL DEFAULT 0,
                is_favorite INTEGER NOT NULL DEFAULT 0,
                content_hash TEXT NOT NULL,
                content_preview TEXT,
                byte_size INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                accessed_at TEXT NOT NULL DEFAULT (datetime('now')),
                access_count INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_clipboard_hash ON clipboard_entries(content_hash);
            CREATE INDEX IF NOT EXISTS idx_clipboard_type ON clipboard_entries(content_type);
            CREATE INDEX IF NOT EXISTS idx_clipboard_pinned ON clipboard_entries(is_pinned);
            CREATE INDEX IF NOT EXISTS idx_clipboard_created ON clipboard_entries(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_clipboard_text ON clipboard_entries(text_content);",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "create snippets and snippet_folders tables",
            sql: "CREATE TABLE IF NOT EXISTS snippet_folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                parent_id INTEGER,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (parent_id) REFERENCES snippet_folders(id) ON DELETE SET NULL
            );

            CREATE TABLE IF NOT EXISTS snippets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                content TEXT NOT NULL,
                keyword TEXT,
                folder_id INTEGER,
                tags TEXT,
                is_pinned INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (folder_id) REFERENCES snippet_folders(id) ON DELETE SET NULL
            );
            CREATE INDEX IF NOT EXISTS idx_snippet_keyword ON snippets(keyword);
            CREATE INDEX IF NOT EXISTS idx_snippet_folder ON snippets(folder_id);",
            kind: MigrationKind::Up,
        },
        // De-duplicate content_hash and enforce UNIQUE(content_hash). The whole
        // string runs in ONE transaction (the plugin uses no_tx=false), so it is
        // all-or-nothing: if the unique index can't be created the dedup rolls
        // back too. Steps 1-2 MUST precede step 3 — CREATE UNIQUE INDEX aborts if
        // any duplicate survives. The survivor is chosen to preserve the most user
        // intent (pinned > favorited > has-custom-name > most-recently-accessed),
        // access_count is SUMMED across the group, and flags/name are folded in, so
        // no pin/favorite/rename/usage is lost. NO filesystem work happens here:
        // every row sharing a content_hash references the byte-identical {hash}.png,
        // so collapsing rows never orphans the survivor's image.
        Migration {
            version: 3,
            description: "dedup content_hash and enforce UNIQUE(content_hash)",
            sql: "
            UPDATE clipboard_entries
            SET access_count = (SELECT SUM(d.access_count) FROM clipboard_entries d
                                WHERE d.content_hash = clipboard_entries.content_hash),
                is_pinned    = (SELECT MAX(d.is_pinned) FROM clipboard_entries d
                                WHERE d.content_hash = clipboard_entries.content_hash),
                is_favorite  = (SELECT MAX(d.is_favorite) FROM clipboard_entries d
                                WHERE d.content_hash = clipboard_entries.content_hash),
                custom_name  = COALESCE(custom_name,
                                (SELECT d.custom_name FROM clipboard_entries d
                                 WHERE d.content_hash = clipboard_entries.content_hash
                                   AND d.custom_name IS NOT NULL ORDER BY d.id LIMIT 1))
            WHERE id IN (
                SELECT id FROM (
                    SELECT id, ROW_NUMBER() OVER (
                        PARTITION BY content_hash
                        ORDER BY is_pinned DESC, is_favorite DESC,
                                 (custom_name IS NOT NULL) DESC, accessed_at DESC, id DESC
                    ) AS rn FROM clipboard_entries
                ) WHERE rn = 1
            )
            AND content_hash IN (
                SELECT content_hash FROM clipboard_entries
                GROUP BY content_hash HAVING COUNT(*) > 1
            );

            DELETE FROM clipboard_entries
            WHERE id NOT IN (
                SELECT id FROM (
                    SELECT id, ROW_NUMBER() OVER (
                        PARTITION BY content_hash
                        ORDER BY is_pinned DESC, is_favorite DESC,
                                 (custom_name IS NOT NULL) DESC, accessed_at DESC, id DESC
                    ) AS rn FROM clipboard_entries
                ) WHERE rn = 1
            );

            DROP INDEX IF EXISTS idx_clipboard_hash;
            CREATE UNIQUE INDEX IF NOT EXISTS idx_clipboard_hash_unique
                ON clipboard_entries(content_hash);",
            kind: MigrationKind::Up,
        },
        // Drop the snippet tables created in v2 but never wired to any feature.
        // Safe: no code ever inserts into them, so they are empty. Children
        // first (snippets FKs snippet_folders), then the parent. DROP TABLE also
        // removes the tables' indexes.
        Migration {
            version: 4,
            description: "drop unused snippet tables",
            sql: "DROP TABLE IF EXISTS snippets;
                  DROP TABLE IF EXISTS snippet_folders;",
            kind: MigrationKind::Up,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::SqlitePool;

    fn migration_sql(version: i64) -> &'static str {
        get_migrations()
            .into_iter()
            .find(|m| m.version == version)
            .expect("migration exists")
            .sql
    }

    /// Single-connection in-memory pool so the schema and the test queries share
    /// one database (a multi-connection :memory: pool would give each connection
    /// its own empty DB).
    async fn mem_pool_with_v1() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(migration_sql(1)).execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn v3_dedups_preserving_pin_customname_and_summed_count() {
        let pool = mem_pool_with_v1().await;

        // Duplicate group on hash 'h': a pinned+named row and a plain one.
        sqlx::query(
            "INSERT INTO clipboard_entries
             (content_type, content_hash, is_pinned, is_favorite, custom_name, access_count, accessed_at, created_at)
             VALUES ('text','h',1,0,'keep',3,'2026-01-01 00:00:00','2026-01-01 00:00:00')",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO clipboard_entries
             (content_type, content_hash, is_pinned, is_favorite, custom_name, access_count, accessed_at, created_at)
             VALUES ('text','h',0,0,NULL,2,'2026-01-02 00:00:00','2026-01-02 00:00:00')",
        ).execute(&pool).await.unwrap();
        // A non-duplicate row that must be left untouched.
        sqlx::query(
            "INSERT INTO clipboard_entries
             (content_type, content_hash, access_count, accessed_at, created_at)
             VALUES ('text','other',1,'2026-01-01 00:00:00','2026-01-01 00:00:00')",
        ).execute(&pool).await.unwrap();

        sqlx::raw_sql(migration_sql(3)).execute(&pool).await.expect("v3 migration runs");

        // Exactly one row survives for 'h', keeping the pin, name, and summed count.
        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM clipboard_entries WHERE content_hash='h'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(cnt, 1);
        let (pinned, name, count): (bool, Option<String>, i64) = sqlx::query_as(
            "SELECT is_pinned, custom_name, access_count FROM clipboard_entries WHERE content_hash='h'",
        ).fetch_one(&pool).await.unwrap();
        assert!(pinned, "survivor must keep the pin");
        assert_eq!(name.as_deref(), Some("keep"), "survivor must keep the custom name");
        assert_eq!(count, 5, "access_count must be summed (3 + 2)");

        // The non-duplicate row is untouched.
        let (other,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM clipboard_entries WHERE content_hash='other'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(other, 1);

        // The UNIQUE index now rejects a new duplicate hash.
        let dup = sqlx::query(
            "INSERT INTO clipboard_entries (content_type, content_hash, accessed_at, created_at)
             VALUES ('text','other','2026-01-03 00:00:00','2026-01-03 00:00:00')",
        ).execute(&pool).await;
        assert!(dup.is_err(), "UNIQUE(content_hash) must reject a duplicate insert");
    }

    #[tokio::test]
    async fn v4_drops_unused_snippet_tables() {
        let pool = mem_pool_with_v1().await;
        sqlx::raw_sql(migration_sql(2)).execute(&pool).await.unwrap();

        let (before,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' \
             AND name IN ('snippets','snippet_folders')",
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(before, 2, "snippet tables exist after v2");

        sqlx::raw_sql(migration_sql(4)).execute(&pool).await.expect("v4 runs");

        let (after,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' \
             AND name IN ('snippets','snippet_folders')",
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(after, 0, "v4 drops both snippet tables");
    }

    #[tokio::test]
    async fn v3_is_a_noop_on_already_unique_data() {
        let pool = mem_pool_with_v1().await;
        sqlx::query(
            "INSERT INTO clipboard_entries (content_type, content_hash, access_count, accessed_at, created_at)
             VALUES ('text','a',1,'2026-01-01 00:00:00','2026-01-01 00:00:00')",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO clipboard_entries (content_type, content_hash, access_count, accessed_at, created_at)
             VALUES ('text','b',1,'2026-01-01 00:00:00','2026-01-01 00:00:00')",
        ).execute(&pool).await.unwrap();

        sqlx::raw_sql(migration_sql(3)).execute(&pool).await.expect("v3 runs on clean data");

        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM clipboard_entries")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(cnt, 2, "no rows removed when there are no duplicates");
    }
}
