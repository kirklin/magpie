use serde::{Deserialize, Serialize};

/// Content type classification for clipboard entries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    Text,
    Image,
    File,
    Url,
    Email,
    Color,
    Code,
    RichText,
}

impl ContentType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "text" => ContentType::Text,
            "image" => ContentType::Image,
            "file" => ContentType::File,
            "url" => ContentType::Url,
            "email" => ContentType::Email,
            "color" => ContentType::Color,
            "code" => ContentType::Code,
            "richtext" => ContentType::RichText,
            _ => ContentType::Text,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            ContentType::Text => "text",
            ContentType::Image => "image",
            ContentType::File => "file",
            ContentType::Url => "url",
            ContentType::Email => "email",
            ContentType::Color => "color",
            ContentType::Code => "code",
            ContentType::RichText => "richtext",
        }
    }
}

/// A clipboard history entry
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ClipboardEntry {
    #[specta(type = i32)]
    pub id: i64,
    pub content_type: String,
    pub text_content: Option<String>,
    pub html_content: Option<String>,
    pub image_path: Option<String>,
    pub file_paths: Option<String>, // JSON array
    pub source_app: Option<String>,
    pub source_app_name: Option<String>,
    pub custom_name: Option<String>,
    pub is_pinned: bool,
    pub is_favorite: bool,
    pub content_hash: String,
    pub content_preview: Option<String>, // truncated preview for list display
    #[specta(type = i32)]
    pub byte_size: i64,
    pub created_at: String,
    pub accessed_at: String,
    pub access_count: i32,
}

/// Query parameters for fetching clipboard entries
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ClipboardQuery {
    pub search: Option<String>,
    pub content_type: Option<String>,
    pub pinned_only: bool,
    pub limit: i32,
    pub offset: i32,
}

impl Default for ClipboardQuery {
    fn default() -> Self {
        Self {
            search: None,
            content_type: None,
            pinned_only: false,
            limit: 50,
            offset: 0,
        }
    }
}

/// Settings
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AppSettings {
    pub history_retention_days: i32,    // -1 = unlimited
    pub max_history_count: i32,         // -1 = unlimited
    pub default_action: String,         // "paste" or "copy"
    pub global_shortcut: String,        // e.g. "CmdOrCtrl+Shift+V"
    pub excluded_apps: Vec<String>,     // bundle identifiers
    pub theme: String,                  // "system", "dark", "light"
    pub launch_at_login: bool,
    pub move_to_top_on_use: bool,
    pub show_menu_bar_icon: bool,       // show/hide menu bar tray icon
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // Unlimited by default (-1): never silently prune history while there
            // is no UI to configure these limits.
            history_retention_days: -1,
            max_history_count: -1,
            default_action: "paste".to_string(),
            global_shortcut: "CmdOrCtrl+Shift+V".to_string(),
            excluded_apps: vec![],
            theme: "system".to_string(),
            launch_at_login: false,
            move_to_top_on_use: true,
            show_menu_bar_icon: true,
        }
    }
}
