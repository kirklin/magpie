//! Placeholder adapters for non-macOS targets, so the crate compiles
//! everywhere. Real Windows/Linux adapters land in later phases.
//!
//! Text read/write still goes through the cross-platform clipboard plugin;
//! everything that needs OS-specific FFI (change detection, file/image, paste,
//! app activation) is a no-op for now.

use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

use super::{AppInfo, Captured, Clipboard, Paster, PasterCapabilities, WritePayload};

pub struct NoopClipboard {
    app: AppHandle,
}

impl NoopClipboard {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl Clipboard for NoopClipboard {
    fn change_token(&self) -> Option<i64> {
        // No change detection yet: the monitor loop simply never fires.
        None
    }

    fn read(&self) -> Option<Captured> {
        None
    }

    fn write(&self, payload: &WritePayload) -> Result<(), String> {
        // Best-effort text-only fallback (files/images degrade to their paths).
        let text = match payload {
            WritePayload::Text(t) => t.clone(),
            WritePayload::Files(paths) => paths.join("\n"),
            WritePayload::ImageFile(path) => path.clone(),
        };
        self.app
            .clipboard()
            .write_text(text.as_str())
            .map_err(|e| format!("Failed to write to clipboard: {e}"))
    }
}

pub struct NoopPaster;

impl Paster for NoopPaster {
    fn frontmost_app(&self) -> AppInfo {
        AppInfo::default()
    }

    fn activate_app(&self, _app_id: &str) -> bool {
        false
    }

    fn paste(&self) -> Result<(), String> {
        Ok(())
    }

    fn capabilities(&self) -> PasterCapabilities {
        PasterCapabilities {
            can_paste: false,
            can_activate_app: false,
        }
    }
}
