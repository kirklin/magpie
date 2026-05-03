use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Paste content to the active application by writing to clipboard
/// and simulating Cmd+V keystroke
pub fn paste_to_active_app(app_handle: &AppHandle, text: &str, _plain_text_only: bool) -> Result<(), String> {
    // Write content to system clipboard
    app_handle
        .clipboard()
        .write_text(text)
        .map_err(|e| format!("Failed to write to clipboard: {}", e))?;

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
    let _ = Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to keystroke "v" using command down"#)
        .output();
}
