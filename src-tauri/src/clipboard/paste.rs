use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Get the frontmost application info on macOS
pub fn get_frontmost_app() -> (Option<String>, Option<String>) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSWorkspace;
        use objc2::rc::autoreleasepool;

        autoreleasepool(|_| {
            let workspace = NSWorkspace::sharedWorkspace();
            if let Some(app) = workspace.frontmostApplication() {
                let bundle_id = app
                    .bundleIdentifier()
                    .map(|s| s.to_string());
                let name = app
                    .localizedName()
                    .map(|s| s.to_string());
                return (bundle_id, name);
            }
            (None, None)
        })
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

/// Simulate Cmd+V keystroke on macOS using CGEvent
#[cfg(target_os = "macos")]
fn simulate_paste_keystroke() {
    use std::process::Command;

    // Use osascript for reliable key simulation
    // This requires Accessibility permissions
    let output = Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to keystroke "v" using command down"#)
        .output();
        
    if let Ok(out) = output {
        if !out.status.success() {
            log::error!("Paste failed: {}", String::from_utf8_lossy(&out.stderr));
        } else {
            log::debug!("Paste successful");
        }
    } else {
        log::error!("Failed to execute osascript");
    }
}
