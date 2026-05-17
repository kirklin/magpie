mod clipboard;
mod commands;
mod database;
mod menu;
mod tray;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use clipboard::monitor::ClipboardMonitorState;
use database::repository::get_migrations;
use tauri::{Manager, Emitter};

/// Stores the bundle ID of the app that was active before Magpie was shown.
pub struct PreviousAppBundleId(pub Mutex<Option<String>>);

/// When true, the blur handler will NOT auto-hide the window.
/// Used by paste_and_keep_window to prevent hide during focus switch.
pub struct SkipBlurHide(pub AtomicBool);

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

    let mut app = tauri::Builder::default()
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
        .manage(PreviousAppBundleId(Mutex::new(None)))
        .manage(SkipBlurHide(AtomicBool::new(false)))
        // --- Commands ---
        .invoke_handler(tauri::generate_handler![
            // Clipboard commands
            commands::clipboard::get_clipboard_entries,
            commands::clipboard::delete_clipboard_entry,
            commands::clipboard::clear_clipboard_history,
            commands::clipboard::toggle_pin_entry,
            commands::clipboard::rename_clipboard_entry,
            commands::clipboard::paste_clipboard_entry,
            commands::clipboard::paste_image_entry,
            commands::clipboard::copy_image_entry,
            commands::clipboard::copy_clipboard_entry,
            commands::clipboard::paste_as_plain_text,
            commands::clipboard::paste_file_entry,
            commands::clipboard::copy_file_entry,
            commands::clipboard::update_entry_content,
            commands::clipboard::append_to_clipboard,
            commands::clipboard::save_entry_as_file,
            commands::clipboard::paste_and_keep_window,
            commands::clipboard::paste_image_and_keep_window,
            commands::clipboard::paste_file_and_keep_window,
            commands::clipboard::export_clipboard_history,
            commands::clipboard::import_clipboard_history,
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
            commands::settings::update_global_shortcut,
            commands::settings::set_tray_visible,
            // System commands
            commands::system::get_app_icon,
            commands::system::get_file_icon,
            commands::system::hide_window,
        ])
        // --- Setup ---
        .setup(|app| {
            let handle = app.handle().clone();

            // Disable App Nap — macOS suspends Accessory apps when the window
            // is hidden, which kills our clipboard monitor timer.
            #[cfg(target_os = "macos")]
            {
                disable_app_nap();
            }

            // Create system tray
            tray::create_tray(&handle)
                .expect("Failed to create system tray");

            // Apply persisted tray icon visibility setting
            {
                let app_dir = app.path().app_data_dir().ok();
                if let Some(dir) = app_dir {
                    let store_path = dir.join("settings.json");
                    if store_path.exists() {
                        if let Ok(contents) = std::fs::read_to_string(&store_path) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
                                if let Some(visible) = json.get("show_menu_bar_icon").and_then(|v| v.as_bool()) {
                                    if !visible {
                                        if let Some(tray) = handle.tray_by_id("main-tray") {
                                            let _ = tray.set_visible(false);
                                            log::info!("Menu bar icon hidden per saved setting");
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Create standard macOS application menu bar
            // Provides ⌘, ⌘Q, ⌘H, ⌘W and standard Edit menu shortcuts
            menu::create_app_menu(&handle)
                .expect("Failed to create application menu");

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
                let _ = window.set_decorations(false);
                let _ = window.set_always_on_top(true);

                // Auto-hide on blur (lose focus), unless SkipBlurHide is set
                let window_clone = window.clone();
                let handle_for_blur = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let skip = handle_for_blur.state::<SkipBlurHide>();
                        if skip.0.load(Ordering::Relaxed) {
                            return; // Don't hide during paste-and-keep-window
                        }
                        if window_clone.is_visible().unwrap_or(false) {
                            let _ = window_clone.hide();
                        }
                    }
                });
            }

            // Register global shortcut — read from persisted settings or use default
            use tauri_plugin_global_shortcut::GlobalShortcutExt;

            let shortcut_key = {
                // Try to read from the settings store file
                let app_dir = app.path().app_data_dir().ok();
                let mut saved_shortcut: Option<String> = None;
                if let Some(dir) = app_dir {
                    let store_path = dir.join("settings.json");
                    if store_path.exists() {
                        if let Ok(contents) = std::fs::read_to_string(&store_path) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
                                if let Some(s) = json.get("global_shortcut").and_then(|v| v.as_str()) {
                                    saved_shortcut = Some(s.to_string());
                                }
                            }
                        }
                    }
                }
                saved_shortcut.unwrap_or_else(|| "CmdOrCtrl+Shift+V".to_string())
            };

            let handle_for_shortcut = app.handle().clone();
            app.global_shortcut().on_shortcut(
                shortcut_key.as_str(),
                move |_app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        toggle_window(&handle_for_shortcut);
                    }
                },
            )?;

            log::info!("Global shortcut registered: {}", shortcut_key);

            // Delay clipboard monitor start to let DB initialize
            let monitor_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                log::info!("Starting clipboard monitor...");
                clipboard::monitor::start_monitor(monitor_handle);
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Set Accessory activation policy AFTER build but BEFORE run.
    // This sets the policy on TAO's EventLoop aux state so that when
    // applicationDidFinishLaunching fires, TAO applies Accessory
    // (not Regular), and the Dock icon never appears at all.
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    app.run(|_, _| {});
}

/// Toggle the main window visibility
pub fn toggle_window(handle: &tauri::AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            // Get previous active app before showing Magpie
            let (bundle_id, name) = clipboard::paste::get_frontmost_app(handle);

            // Save the bundle_id for paste-and-keep-window
            if let Some(ref bid) = bundle_id {
                let state = handle.state::<PreviousAppBundleId>();
                *state.0.lock().unwrap() = Some(bid.clone());
            }

            if let Some(app_name) = name {
                let _ = window.emit("active-app-changed", app_name);
            } else {
                let _ = window.emit("active-app-changed", "Active App");
            }

            // Show and focus window — do NOT call handle.show() as it
            // resets activation policy to Regular, causing a Dock icon flash.
            let _ = window.center();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

/// Show and focus the main window
pub fn show_window(handle: &tauri::AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        let (bundle_id, name) = clipboard::paste::get_frontmost_app(handle);

        // Save the bundle_id for paste-and-keep-window
        if let Some(ref bid) = bundle_id {
            let state = handle.state::<PreviousAppBundleId>();
            *state.0.lock().unwrap() = Some(bid.clone());
        }

        if let Some(app_name) = name {
            let _ = window.emit("active-app-changed", app_name);
        } else {
            let _ = window.emit("active-app-changed", "Active App");
        }

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

/// Disable macOS App Nap to keep the clipboard monitor running in the background.
/// Without this, macOS will suspend timers and background work for Accessory apps
/// when the window is hidden, causing the monitor to stop detecting clipboard changes.
#[cfg(target_os = "macos")]
fn disable_app_nap() {
    use objc2_foundation::{NSProcessInfo, NSString, NSActivityOptions};

    let process_info = NSProcessInfo::processInfo();
    let reason = NSString::from_str("Clipboard monitoring requires continuous background execution");

    // NSActivityUserInitiatedAllowingIdleSystemSleep = 0x00FFFFFFULL
    // This prevents App Nap and timer throttling while allowing the system to sleep
    let activity_options = NSActivityOptions(0x00FFFFFF);

    // beginActivityWithOptions:reason: returns a token that must be retained
    // We intentionally leak it because we want this to last for the app's lifetime
    let _activity = unsafe {
        process_info.beginActivityWithOptions_reason(activity_options, &reason)
    };
    // Leak the activity token so it stays alive forever
    std::mem::forget(_activity);

    log::info!("App Nap disabled for clipboard monitoring");
}
