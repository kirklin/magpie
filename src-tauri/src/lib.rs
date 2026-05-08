mod clipboard;
mod commands;
mod database;
mod tray;

use std::sync::Arc;
use clipboard::monitor::ClipboardMonitorState;
use database::repository::get_migrations;
use tauri::{Manager, Emitter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging: write to both stderr and a log file
    // Log file location: ~/Library/Application Support/com.magpie.clipboard/magpie.log
    let log_file_path = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("com.magpie.clipboard");
    let _ = std::fs::create_dir_all(&log_file_path);
    let log_file = log_file_path.join("magpie.log");

    // Truncate log file if it's too large (> 5MB)
    if let Ok(meta) = std::fs::metadata(&log_file) {
        if meta.len() > 5 * 1024 * 1024 {
            let _ = std::fs::write(&log_file, b"");
        }
    }

    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file);

    let mut builder = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,magpie=debug")
    );

    if let Ok(file) = file {
        let file = std::sync::Mutex::new(file);
        builder.format(move |buf, record| {
            use std::io::Write;
            let msg = format!(
                "[{}] {} - {}\n",
                record.level(),
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.args()
            );
            // Write to stderr (default behavior)
            let _ = buf.write_all(msg.as_bytes());
            // Also write to log file
            if let Ok(mut f) = file.lock() {
                let _ = f.write_all(msg.as_bytes());
            }
            Ok(())
        });
    }

    builder.init();

    tauri::Builder::default()
        // --- Plugins ---
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::AppleScript,
            None,
        ))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            toggle_window(app);
        }))
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:magpie.db", get_migrations())
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_positioner::init())
        // --- State ---
        .manage(Arc::new(ClipboardMonitorState::default()))
        .manage(commands::system::AppIconCache::default())
        // --- Commands ---
        .invoke_handler(tauri::generate_handler![
            // Clipboard commands
            commands::clipboard::get_clipboard_entries,
            commands::clipboard::delete_clipboard_entry,
            commands::clipboard::clear_clipboard_history,
            commands::clipboard::toggle_pin_entry,
            commands::clipboard::rename_clipboard_entry,
            commands::clipboard::paste_clipboard_entry,
            commands::clipboard::copy_clipboard_entry,
            commands::clipboard::paste_as_plain_text,
            commands::clipboard::paste_file_entry,
            commands::clipboard::copy_file_entry,
            // Snippet commands
            commands::snippet::get_snippets,
            commands::snippet::create_snippet,
            commands::snippet::update_snippet,
            commands::snippet::delete_snippet,
            commands::snippet::get_snippet_folders,
            commands::snippet::create_snippet_folder,
            commands::snippet::delete_snippet_folder,
            commands::snippet::save_as_snippet,
            // Settings commands
            commands::settings::get_default_settings,
            // System commands
            commands::system::get_app_icon,
            commands::system::get_file_icon,
            commands::system::hide_window,
        ])
        // --- Setup ---
        .setup(|app| {
            let handle = app.handle().clone();

            // Create system tray
            tray::create_tray(&handle)
                .expect("Failed to create system tray");

            // Check Accessibility permission (required for paste simulation)
            #[cfg(target_os = "macos")]
            {
                if !check_accessibility_permission() {
                    log::warn!("Accessibility permission not granted — paste will not work!");
                    // Show system prompt asking user to grant permission
                    request_accessibility_permission();

                    // Also show a notification so the user knows
                    use tauri_plugin_notification::NotificationExt;
                    let _ = app.notification()
                        .builder()
                        .title("Magpie 需要辅助功能权限")
                        .body("请在「系统设置 → 隐私与安全性 → 辅助功能」中开启 Magpie，否则无法粘贴内容到其他应用。")
                        .show();
                }
            }

            // Configure main window
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "macos")]
                {
                    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                }

                let _ = window.set_decorations(false);
                let _ = window.set_always_on_top(true);

                // Auto-hide on blur (lose focus)
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        // Only hide if the window is currently visible
                        if window_clone.is_visible().unwrap_or(false) {
                            let _ = window_clone.hide();
                        }
                    }
                });
            }

            // Register global shortcut: Cmd+Shift+V
            use tauri_plugin_global_shortcut::GlobalShortcutExt;

            let handle_for_shortcut = app.handle().clone();
            app.global_shortcut().on_shortcut(
                "CmdOrCtrl+Shift+V",
                move |_app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        toggle_window(&handle_for_shortcut);
                    }
                },
            )?;

            log::info!("Global shortcut registered: Cmd+Shift+V");

            // Delay clipboard monitor start to let DB initialize
            let monitor_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                log::info!("Starting clipboard monitor...");
                clipboard::monitor::start_monitor(monitor_handle);
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Toggle the main window visibility
pub fn toggle_window(handle: &tauri::AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            // Get previous active app before showing Magpie
            let (_, name) = clipboard::paste::get_frontmost_app(handle);
            if let Some(app_name) = name {
                let _ = window.emit("active-app-changed", app_name);
            } else {
                let _ = window.emit("active-app-changed", "Active App");
            }

            // Bring app to front first
            let _ = handle.show();
            // Then show window
            let _ = window.center();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

/// Show and focus the main window
pub fn show_window(handle: &tauri::AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        let (_, name) = clipboard::paste::get_frontmost_app(handle);
        if let Some(app_name) = name {
            let _ = window.emit("active-app-changed", app_name);
        } else {
            let _ = window.emit("active-app-changed", "Active App");
        }

        let _ = handle.show();
        let _ = window.center();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Check if the app has Accessibility permission (macOS)
#[cfg(target_os = "macos")]
fn check_accessibility_permission() -> bool {
    // AXIsProcessTrusted is in ApplicationServices framework
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    unsafe { AXIsProcessTrusted() }
}

/// Request Accessibility permission by showing the system prompt (macOS)
#[cfg(target_os = "macos")]
fn request_accessibility_permission() {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    unsafe extern "C" {
        fn AXIsProcessTrustedWithOptions(options: core_foundation::base::CFTypeRef) -> bool;
    }

    // kAXTrustedCheckOptionPrompt = true → shows the system dialog
    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let value = CFBoolean::true_value();
    let options = CFDictionary::from_CFType_pairs(&[(key, value)]);

    unsafe {
        AXIsProcessTrustedWithOptions(options.as_CFTypeRef());
    }
}
