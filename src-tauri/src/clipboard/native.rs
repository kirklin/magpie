//! macOS pasteboard / AppKit FFI, isolated from the IPC command layer.
//!
//! These helpers wrap the unsafe-ish objc2 calls and the main-thread dispatch
//! that NSPasteboard requires, so the command handlers stay thin orchestration.

use tauri::AppHandle;

/// Read a PNG file and write it to the general pasteboard, then mark the write
/// as self-originated so the clipboard monitor doesn't re-capture it.
///
/// Previously this exact block was copy-pasted into paste_image_entry,
/// copy_image_entry, and paste_image_and_keep_window.
#[cfg(target_os = "macos")]
pub fn write_png_to_pasteboard(app_handle: &AppHandle, image_path: &str) -> Result<(), String> {
    let png_data = std::fs::read(image_path)
        .map_err(|e| format!("Failed to read image file: {}", e))?;

    let (tx, rx) = std::sync::mpsc::channel();
    let _ = app_handle.run_on_main_thread(move || {
        use objc2_app_kit::NSPasteboard;
        use objc2_foundation::{NSData, NSString};
        use objc2::rc::autoreleasepool;

        let result = autoreleasepool(|_| {
            let pasteboard = NSPasteboard::generalPasteboard();
            pasteboard.clearContents();
            let png_type = NSString::from_str("public.png");
            let ns_data = NSData::with_bytes(&png_data);
            pasteboard.setData_forType(Some(&ns_data), &png_type)
        });
        let _ = tx.send(result);
    });

    let success = rx.recv().map_err(|e| e.to_string())?;
    if !success {
        return Err("Failed to write image to pasteboard".to_string());
    }
    crate::clipboard::monitor::mark_self_write(app_handle);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn write_png_to_pasteboard(_app_handle: &AppHandle, _image_path: &str) -> Result<(), String> {
    Ok(())
}
