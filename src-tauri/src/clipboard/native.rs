//! Native macOS file dialogs (save / open panels), used by the export/import
//! and "save entry as file" commands.
//!
//! Clipboard read/write and paste-back have moved behind the platform ports
//! (see `crate::platform`). These panels are a separate concern and remain here
//! until a later phase abstracts file dialogs too.

#[cfg(target_os = "macos")]
use tauri::AppHandle;

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
