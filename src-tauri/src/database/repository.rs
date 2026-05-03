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
    ]
}
