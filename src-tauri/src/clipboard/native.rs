//! macOS pasteboard / AppKit FFI, isolated from the IPC command layer.
//!
//! These helpers wrap the unsafe-ish objc2 calls and the main-thread dispatch
//! that NSPasteboard requires, so the command handlers stay thin orchestration.

use tauri::AppHandle;

use crate::clipboard::paste;

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

/// Poll until `target_bundle_id` is the frontmost app (up to ~500ms), then add
/// a short settle delay. Returns whether it became frontmost.
pub async fn wait_until_frontmost(target_bundle_id: &str, app_handle: &AppHandle) -> bool {
    for _ in 0..50 {
        if let (Some(id), _) = paste::get_frontmost_app(app_handle) {
            if id == target_bundle_id {
                tokio::time::sleep(std::time::Duration::from_millis(40)).await;
                return true;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    log::warn!("Target app {} never became frontmost before paste", target_bundle_id);
    false
}

/// Polls until the frontmost application is NOT the specified bundle ID, then
/// adds a short settle delay so the newly-focused app is ready to receive the
/// synthesized Cmd+V. Returns whether the focus actually switched.
pub async fn wait_for_frontmost_app_switch(ignore_bundle_id: &str, app_handle: &AppHandle) -> bool {
    let mut switched = false;
    for _ in 0..50 { // max ~500ms
        let (bundle_id, _) = paste::get_frontmost_app(app_handle);
        if let Some(id) = bundle_id {
            if id != ignore_bundle_id {
                log::debug!("Active app switched to: {}", id);
                switched = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    if switched {
        // Give the now-frontmost app a moment to become first responder before
        // we synthesize the paste keystroke — without this the Cmd+V can race
        // the focus change and be dropped or land in Magpie.
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    } else {
        log::warn!("Frontmost app never switched away from {} before paste", ignore_bundle_id);
    }

    switched
}

/// Activate a macOS application by its bundle identifier.
/// Uses NSRunningApplication to bring the app to the foreground.
/// Returns whether a running app with that bundle id was found and activated.
#[cfg(target_os = "macos")]
pub fn activate_app_by_bundle_id(app_handle: &AppHandle, bundle_id: &str) -> bool {
    use objc2_app_kit::NSRunningApplication;
    use objc2_foundation::NSString;
    use std::sync::mpsc;

    let bid = bundle_id.to_string();
    let (tx, rx) = mpsc::channel();
    let dispatched = app_handle.run_on_main_thread(move || {
        let ns_bid = NSString::from_str(&bid);
        let apps = unsafe {
            NSRunningApplication::runningApplicationsWithBundleIdentifier(&ns_bid)
        };
        let activated = if apps.count() > 0 {
            let app = unsafe { apps.objectAtIndex(0) };
            #[allow(deprecated)]
            let _ = unsafe {
                app.activateWithOptions(
                    objc2_app_kit::NSApplicationActivationOptions::ActivateIgnoringOtherApps,
                )
            };
            true
        } else {
            false
        };
        let _ = tx.send(activated);
    });

    if dispatched.is_err() {
        return false;
    }
    rx.recv_timeout(std::time::Duration::from_millis(300)).unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
pub fn activate_app_by_bundle_id(_app_handle: &AppHandle, _bundle_id: &str) -> bool {
    false
}

/// Write file paths to macOS NSPasteboard as file URLs.
#[cfg(target_os = "macos")]
pub fn write_files_to_pasteboard(file_paths: &[String]) -> Result<(), String> {
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::{NSString, NSArray};
    use objc2::rc::autoreleasepool;

    autoreleasepool(|_| {
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();

        // Declare NSFilenamesPboardType and public.file-url
        let filenames_type = NSString::from_str("NSFilenamesPboardType");
        let file_url_type = NSString::from_str("public.file-url");
        let types = NSArray::from_retained_slice(&[
            NSString::from_str("NSFilenamesPboardType"),
            NSString::from_str("public.file-url"),
        ]);
        // SAFETY: declaring pasteboard types with no owner is safe
        unsafe { pasteboard.declareTypes_owner(&types, None) };

        // Build an NSArray of NSString paths for the property list
        let ns_paths: Vec<_> = file_paths.iter()
            .map(|p| NSString::from_str(p))
            .collect();
        let ns_array = NSArray::from_retained_slice(&ns_paths);

        // Set the property list (array of file paths) for the filenames type
        // SAFETY: we're passing a valid NSArray<NSString> which matches NSFilenamesPboardType's expected format
        let success = unsafe { pasteboard.setPropertyList_forType(&ns_array, &filenames_type) };

        // Also set the first file as a file URL for apps that prefer public.file-url
        if let Some(first_path) = file_paths.first() {
            let encoded = format!("file://{}", first_path.replace(' ', "%20"));
            let url_str = NSString::from_str(&encoded);
            pasteboard.setString_forType(&url_str, &file_url_type);
        }

        if success {
            Ok(())
        } else {
            Err("Failed to write file paths to pasteboard".to_string())
        }
    })
}

/// Show an NSSavePanel pre-filled with `default_name`; returns the path the user
/// chose, or None if they cancelled. The caller writes the file.
#[cfg(target_os = "macos")]
pub fn run_save_panel(app_handle: &AppHandle, default_name: &str) -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let name = default_name.to_string();
    let _ = app_handle.run_on_main_thread(move || {
        use objc2_app_kit::NSSavePanel;
        use objc2_foundation::{NSString, MainThreadMarker};
        use objc2::rc::autoreleasepool;

        let result = autoreleasepool(|_| {
            let mtm = MainThreadMarker::new().expect("Must be called on main thread");
            let panel = NSSavePanel::savePanel(mtm);
            let ns_name = NSString::from_str(&name);
            panel.setNameFieldStringValue(&ns_name);
            panel.setCanCreateDirectories(true);
            // NSModalResponseOK = 1
            if panel.runModal() == 1 {
                panel.URL().and_then(|url| url.path()).map(|p| p.to_string())
            } else {
                None
            }
        });
        let _ = tx.send(result);
    });
    rx.recv().ok().flatten()
}

/// Show an NSOpenPanel for picking one existing file; returns its path, or None
/// if cancelled. `message` is shown as the panel prompt.
#[cfg(target_os = "macos")]
pub fn run_open_panel(app_handle: &AppHandle, message: &str) -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let msg = message.to_string();
    let _ = app_handle.run_on_main_thread(move || {
        use objc2_app_kit::NSOpenPanel;
        use objc2_foundation::{NSString, MainThreadMarker};
        use objc2::rc::autoreleasepool;

        let result: Option<String> = autoreleasepool(|_| {
            let mtm = MainThreadMarker::new().expect("Must be called on main thread");
            let panel = NSOpenPanel::openPanel(mtm);
            panel.setCanChooseFiles(true);
            panel.setCanChooseDirectories(false);
            panel.setAllowsMultipleSelection(false);
            let ns_title = NSString::from_str(&msg);
            panel.setMessage(Some(&ns_title));
            if panel.runModal() == 1 {
                panel.URL().and_then(|url| url.path()).map(|p| p.to_string())
            } else {
                None
            }
        });
        let _ = tx.send(result);
    });
    rx.recv().ok().flatten()
}
