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
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,magpie=debug")
    ).init();

    tauri::Builder::default()
        // --- Plugins ---
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
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
