mod clipboard;
mod commands;
mod database;
mod dialogs;
mod error;
mod i18n;
#[cfg(target_os = "macos")]
mod menu;
mod platform;
mod shortcut;
mod tray;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use clipboard::monitor::ClipboardMonitorState;
use database::repository::get_migrations;
use platform::{AppInfo, PasterPort};
use tauri::{Manager, Emitter};

/// The application that was frontmost when Magpie was last shown: where pastes
/// go back to.
pub struct PreviousApp(Mutex<Option<AppInfo>>);

impl PreviousApp {
    pub fn get(&self) -> Option<AppInfo> {
        self.0.lock().expect("previous-app lock poisoned").clone()
    }

    fn set(&self, app: AppInfo) {
        *self.0.lock().expect("previous-app lock poisoned") = Some(app);
    }
}

/// When true, the blur handler will NOT auto-hide the window.
/// Used by paste_and_keep_window to prevent hide during focus switch.
pub struct SkipBlurHide(pub AtomicBool);

/// When the blur handler last hid the window. Clicking the tray icon first
/// takes focus from the window (hiding it) and only then delivers the click, so
/// a click right after a blur-hide means "hide", not "show again".
struct LastBlurHide(Mutex<Option<Instant>>);

/// Whether the main window has been shown before. It is created centered;
/// re-centering it before it was ever mapped uses a size GTK doesn't know yet
/// and puts it off-center on Linux.
struct ShownBefore(AtomicBool);

/// When Magpie last asked the desktop to move its window (a drag on one of
/// the title-bar regions). Some compositors take keyboard focus away for the
/// whole interactive move, which must not count as the user leaving the window.
pub struct DragStarted(Mutex<Option<Instant>>);

impl DragStarted {
    pub fn mark(&self) {
        *self.0.lock().expect("drag lock poisoned") = Some(Instant::now());
    }

    fn just_started(&self) -> bool {
        self.0.lock().expect("drag lock poisoned").is_some_and(|at| at.elapsed() < Duration::from_millis(500))
    }
}

/// Single source of truth for the IPC command surface. Used both to build the
/// runtime invoke handler and to export the TypeScript bindings (see the
/// `export_typescript_bindings` test, run via `cargo test`).
fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    use tauri_specta::collect_commands;
    tauri_specta::Builder::<tauri::Wry>::new().commands(collect_commands![
        // Clipboard commands
        commands::clipboard::get_clipboard_entries,
        commands::clipboard::delete_clipboard_entry,
        commands::clipboard::clear_clipboard_history,
        commands::clipboard::toggle_pin_entry,
        commands::clipboard::rename_clipboard_entry,
        commands::clipboard::paste_clipboard_entry,
        commands::clipboard::paste_image_entry,
        commands::clipboard::copy_image_entry,
        commands::clipboard::get_thumbnail,
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
        commands::history_io::export_clipboard_history,
        commands::history_io::import_clipboard_history,
        // Settings commands
        commands::settings::get_default_settings,
        commands::settings::update_global_shortcut,
        commands::settings::get_shortcut_binding,
        commands::settings::configure_system_shortcut,
        commands::settings::set_tray_visible,
        commands::settings::relocalize_menus,
        // System commands
        commands::system::get_app_icon,
        commands::system::get_file_icon,
        commands::system::get_paster_capabilities,
        commands::system::hide_window,
        commands::system::start_window_drag,
    ])
}

/// Read one key from the persisted settings store (`settings.json`, written by
/// the frontend's store plugin).
fn read_persisted_setting(app: &tauri::AppHandle, key: &str) -> Option<serde_json::Value> {
    let path = app.path().app_data_dir().ok()?.join("settings.json");
    let contents = std::fs::read_to_string(path).ok()?;
    let json = serde_json::from_str::<serde_json::Value>(&contents).ok()?;
    json.get(key).cloned()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging: write to both stderr and a log file named magpie.log
    // in the per-user data directory (~/Library/Application Support on macOS,
    // %APPDATA% on Windows, ~/.local/share on Linux) under the app identifier.
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

    // Silence tauri's asset-protocol "File does not exist" errors: clipboard
    // history legitimately references files the user may have since deleted, so
    // these are expected and handled in the UI with a fallback, not real errors.
    // zbus (the Linux D-Bus client) logs every message at info level, and warns
    // about each short-lived portal request object whose properties it can't
    // cache.
    let mut builder = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,magpie=debug,tauri::protocol::asset=off,zbus=error")
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

    // Desktop portals identify Magpie by this entry. Written first, so the
    // portal has noticed it by the time Magpie makes its first portal call.
    #[cfg(target_os = "linux")]
    platform::ensure_identity_entry();

    let specta = specta_builder();

    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_positioner::init())
        // --- State ---
        .manage(Arc::new(ClipboardMonitorState::default()))
        .manage(commands::system::AppIconCache::default())
        .manage(PreviousApp(Mutex::new(None)))
        .manage(SkipBlurHide(AtomicBool::new(false)))
        .manage(LastBlurHide(Mutex::new(None)))
        .manage(ShownBefore(AtomicBool::new(false)))
        .manage(DragStarted(Mutex::new(None)))
        .manage(shortcut::ActiveShortcut::new())
        // --- Commands ---
        .invoke_handler(specta.invoke_handler())
        // --- Setup ---
        .setup(|app| {
            let handle = app.handle().clone();

            // Build the platform adapters (clipboard, paste-back, icons) for this
            // OS and expose them to the monitor and IPC commands via managed
            // state. This is the single place an OS implementation is selected.
            let platform = platform::build(&handle)?;
            app.manage(platform.clipboard);
            app.manage(platform.paster);
            app.manage(platform.icons);

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
            if read_persisted_setting(&handle, "show_menu_bar_icon").and_then(|v| v.as_bool()) == Some(false)
                && let Some(tray) = handle.tray_by_id("main-tray")
            {
                let _ = tray.set_visible(false);
                log::info!("Tray icon hidden per saved setting");
            }

            // Create standard macOS application menu bar
            // Provides ⌘, ⌘Q, ⌘H, ⌘W and standard Edit menu shortcuts
            #[cfg(target_os = "macos")]
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
                    let loc = i18n::read_locale(app.handle());
                    let _ = app.notification()
                        .builder()
                        .title(i18n::tr(loc, "notify.accessibility_title"))
                        .body(i18n::tr(loc, "notify.accessibility_body"))
                        .show();
                }
            }

            // Configure main window
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_decorations(false);
                let _ = window.set_always_on_top(true);

                // Round the NATIVE window corners. The window is frameless and
                // transparent and only the CSS content is rounded, so the square
                // native content layer pokes past the rounded corners, leaving an
                // opaque notch at each corner (visible in both light and dark
                // mode). Clipping the content view's layer to a rounded rect and
                // recomputing the shadow makes the whole window corner clean.
                #[cfg(target_os = "macos")]
                {
                    use objc2::runtime::AnyObject;
                    if let Ok(ns_window) = window.ns_window() {
                        let ns_window = ns_window as *mut AnyObject;
                        unsafe {
                            let content_view: *mut AnyObject = objc2::msg_send![ns_window, contentView];
                            if !content_view.is_null() {
                                let _: () = objc2::msg_send![content_view, setWantsLayer: true];
                                let layer: *mut AnyObject = objc2::msg_send![content_view, layer];
                                if !layer.is_null() {
                                    // Matches the CSS `rounded-2xl` (16px) on the root element.
                                    let _: () = objc2::msg_send![layer, setCornerRadius: 16.0f64];
                                    let _: () = objc2::msg_send![layer, setMasksToBounds: true];
                                }
                            }
                            let _: () = objc2::msg_send![ns_window, invalidateShadow];
                        }
                    }
                }

                let window_clone = window.clone();
                let handle_for_events = app.handle().clone();
                #[cfg(target_os = "windows")]
                let hwnd = window.hwnd().expect("main window handle").0 as isize;
                window.on_window_event(move |event| match event {
                    // Auto-hide on blur (lose focus), unless SkipBlurHide is set
                    tauri::WindowEvent::Focused(false) => {
                        let skip = handle_for_events.state::<SkipBlurHide>();
                        if skip.0.load(Ordering::Relaxed) {
                            return; // Don't hide during paste-and-keep-window
                        }
                        if handle_for_events.state::<DragStarted>().just_started() {
                            return;
                        }
                        #[cfg(target_os = "linux")]
                        if platform::focus_moved_to_own_popup(&window_clone) {
                            return;
                        }
                        // WebView2 loses and regains focus within one message
                        // when a window drag starts or ends; only a focus loss
                        // that leaves another window in the foreground counts.
                        #[cfg(target_os = "windows")]
                        {
                            let window = window_clone.clone();
                            let handle = handle_for_events.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(Duration::from_millis(150));
                                if platform::foreground_is_own(hwnd)
                                    || handle.state::<SkipBlurHide>().0.load(Ordering::Relaxed)
                                {
                                    return;
                                }
                                hide_on_blur(&window, &handle);
                            });
                        }
                        #[cfg(not(target_os = "windows"))]
                        hide_on_blur(&window_clone, &handle_for_events);
                    }
                    // Alt+F4 and the window manager's close action hide the
                    // window like Escape does. Closing it would destroy the only
                    // window of a tray app that has no way to create it again.
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = window_clone.hide();
                    }
                    _ => {}
                });
            }

            // Register the global shortcut — the persisted one, or the default.
            let saved_shortcut = read_persisted_setting(&handle, "global_shortcut")
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_else(|| platform::DEFAULT_SHORTCUT.to_string());
            shortcut::register_at_startup(&handle, &saved_shortcut);

            // Launched by a desktop shortcut bound to `magpie --toggle` (see
            // `shortcut`) while no instance was running: show right away.
            if std::env::args().any(|arg| arg == "--toggle") {
                show_window(&handle);
            }

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
/// Hide the window because the user moved on to something else, remembering
/// when, so that the tray click or shortcut which caused the blur doesn't
/// bring it straight back.
fn hide_on_blur(window: &tauri::WebviewWindow, handle: &tauri::AppHandle) {
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        *handle.state::<LastBlurHide>().0.lock().expect("blur lock poisoned") = Some(Instant::now());
    }
}

pub fn toggle_window(handle: &tauri::AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            show_window(handle);
        }
    }
}

/// Toggle the main window from a tray icon click, which arrives only after the
/// click already took focus from (and so hid) a visible window.
pub fn toggle_window_from_tray(handle: &tauri::AppHandle) {
    let hidden_by_this_click = handle
        .state::<LastBlurHide>()
        .0
        .lock()
        .expect("blur lock poisoned")
        .is_some_and(|at| at.elapsed() < Duration::from_millis(300));
    if !hidden_by_this_click {
        toggle_window(handle);
    }
}

/// Show and focus the main window, remembering which app to paste back into.
pub fn show_window(handle: &tauri::AppHandle) {
    if let Some(window) = handle.get_webview_window("main") {
        let frontmost = handle.state::<PasterPort>().frontmost_app();

        // Remember where to paste back. Magpie itself (e.g. summoned again from
        // its own tray menu) is never a paste target.
        if !frontmost.focus.is_magpie() {
            let name = frontmost.name.clone();
            handle.state::<PreviousApp>().set(frontmost);
            let _ = window.emit("active-app-changed", name);
        }

        // Show and focus window — do NOT call handle.show() as it
        // resets activation policy to Regular, causing a Dock icon flash.
        if handle.state::<ShownBefore>().0.swap(true, Ordering::Relaxed) {
            let _ = window.center();
        }
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Check if the app has Accessibility permission (macOS)
#[cfg(target_os = "macos")]
pub(crate) fn check_accessibility_permission() -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Regenerates src/bindings.ts from the Rust command surface.
    /// Run with `cargo test export_typescript_bindings`. Keep the output
    /// committed; CI can run this with --check semantics once wired up.
    #[test]
    fn export_typescript_bindings() {
        // i64 ids/byte_size are exported as TS `number` via #[specta(type = i32)]
        // on the model fields (Tauri's JSON IPC sends them as numbers anyway;
        // values stay well within Number.MAX_SAFE_INTEGER).
        specta_builder()
            .export(
                specta_typescript::Typescript::default(),
                "../src/bindings.ts",
            )
            .expect("failed to export typescript bindings");
    }
}
