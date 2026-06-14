//! Minimal locale support for the native tray + app menu (the only Rust-side
//! user-facing strings). The locale is owned by the frontend and persisted to
//! settings.json under `"locale"`; we read it here so the native menus match
//! the chosen language at startup, and rebuild them on change (see the
//! `relocalize_menus` command).

use tauri::{AppHandle, Manager};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    Zh,
    En,
}

/// Read the persisted UI locale from settings.json, defaulting to Chinese.
pub fn read_locale(app: &AppHandle) -> Locale {
    let Ok(dir) = app.path().app_data_dir() else {
        return Locale::Zh;
    };
    let Ok(contents) = std::fs::read_to_string(dir.join("settings.json")) else {
        return Locale::Zh;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return Locale::Zh;
    };
    match json.get("locale").and_then(|v| v.as_str()) {
        Some("en") => Locale::En,
        _ => Locale::Zh,
    }
}

/// Translate a tray/menu label key for the given locale.
pub fn tr(locale: Locale, key: &str) -> &'static str {
    macro_rules! pick {
        ($zh:literal, $en:literal) => {
            match locale {
                Locale::Zh => $zh,
                Locale::En => $en,
            }
        };
    }
    match key {
        // tray
        "tray.show" => pick!("打开/隐藏 Magpie", "Show / Hide Magpie"),
        "tray.quit" => pick!("退出", "Quit"),
        // app menu — application submenu
        "menu.about" => pick!("关于 Magpie", "About Magpie"),
        "menu.settings" => pick!("设置…", "Settings…"),
        "menu.hide" => pick!("隐藏 Magpie", "Hide Magpie"),
        "menu.hide_others" => pick!("隐藏其他", "Hide Others"),
        "menu.show_all" => pick!("全部显示", "Show All"),
        "menu.quit" => pick!("退出 Magpie", "Quit Magpie"),
        // app menu — edit submenu
        "menu.edit" => pick!("编辑", "Edit"),
        "menu.undo" => pick!("撤销", "Undo"),
        "menu.redo" => pick!("重做", "Redo"),
        "menu.cut" => pick!("剪切", "Cut"),
        "menu.copy" => pick!("复制", "Copy"),
        "menu.paste" => pick!("粘贴", "Paste"),
        "menu.select_all" => pick!("全选", "Select All"),
        // app menu — window submenu
        "menu.window" => pick!("窗口", "Window"),
        "menu.close_window" => pick!("关闭窗口", "Close Window"),
        "menu.minimize" => pick!("最小化", "Minimize"),
        _ => "?",
    }
}
