use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Get the frontmost application info on macOS
pub fn get_frontmost_app(app_handle: &AppHandle) -> (Option<String>, Option<String>) {
    #[cfg(target_os = "macos")]
    {
        use std::sync::mpsc;
        use objc2_app_kit::NSWorkspace;
        use objc2::rc::autoreleasepool;

        let (tx, rx) = mpsc::channel();
        
        let _ = app_handle.run_on_main_thread(move || {
            let result = autoreleasepool(|_| {
                let workspace = NSWorkspace::sharedWorkspace();
                if let Some(app) = workspace.frontmostApplication() {
                    let bundle_id = app
                        .bundleIdentifier()
                        .map(|s| s.to_string());
                    let name = app
                        .localizedName()
                        .map(|s| s.to_string());
                    (bundle_id, name)
                } else {
                    (None, None)
                }
            });
            let _ = tx.send(result);
        });

        rx.recv().unwrap_or((None, None))
    }

    #[cfg(not(target_os = "macos"))]
    {
        (None, None)
    }
}

/// Paste content to the active application by writing to clipboard
/// and simulating Cmd+V keystroke
pub fn paste_to_active_app(_app_handle: &AppHandle, _text: &str, _plain_text_only: bool) -> Result<(), String> {
    // Content is already written to clipboard by the caller

    // Simulate Cmd+V using CGEvent on macOS
    #[cfg(target_os = "macos")]
    {
        simulate_paste_keystroke();
    }

    Ok(())
}

/// Copy content to clipboard without pasting
pub fn copy_to_clipboard(app_handle: &AppHandle, text: &str) -> Result<(), String> {
    app_handle
        .clipboard()
        .write_text(text)
        .map_err(|e| format!("Failed to write to clipboard: {}", e))
}

/// Simulate Cmd+V keystroke on macOS using CGEvent API.
/// This is more reliable than osascript and doesn't require
/// separate System Events permission — only Accessibility access.
#[cfg(target_os = "macos")]
fn simulate_paste_keystroke() {
    use core_graphics::event::{CGEvent, CGEventFlags, CGKeyCode};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    // Key code 9 = 'V' on macOS
    const V_KEY: CGKeyCode = 9;

    let source = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        Ok(s) => s,
        Err(_) => {
            log::error!("[Paste] Failed to create CGEventSource");
            return;
        }
    };

    // Key down
    let key_down = match CGEvent::new_keyboard_event(source.clone(), V_KEY, true) {
        Ok(e) => e,
        Err(_) => {
            log::error!("[Paste] Failed to create key down event");
            return;
        }
    };
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(core_graphics::event::CGEventTapLocation::HID);

    // Key up
    let key_up = match CGEvent::new_keyboard_event(source, V_KEY, false) {
        Ok(e) => e,
        Err(_) => {
            log::error!("[Paste] Failed to create key up event");
            return;
        }
    };
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.post(core_graphics::event::CGEventTapLocation::HID);

    log::debug!("[Paste] Simulated Cmd+V via CGEvent");
}
