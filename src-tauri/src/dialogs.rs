//! Native save / open dialogs.
//!
//! Each dialog is parented to the main window. The window is always-on-top, so
//! an unparented dialog would open underneath it on Windows; parented, the
//! dialog stays above it (a sheet on macOS, an owned modal on Windows, a
//! transient dialog on Linux) and Magpie stays open behind it instead of
//! auto-hiding the moment the dialog takes focus.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};

use crate::error::AppError;

/// Ask where to save a file named `default_name`. `None` when cancelled.
pub async fn ask_save_path(app: &AppHandle, default_name: &str) -> Result<Option<PathBuf>, AppError> {
    let default_name = default_name.to_string();
    run(app, move |dialog| {
        dialog
            .set_file_name(default_name)
            .set_can_create_directories(true)
            .blocking_save_file()
    })
    .await
}

/// Ask for one existing file, with `title` as the dialog's prompt. `None`
/// when cancelled.
pub async fn ask_open_path(app: &AppHandle, title: &str) -> Result<Option<PathBuf>, AppError> {
    let title = title.to_string();
    run(app, move |dialog| dialog.set_title(title).blocking_pick_file()).await
}

async fn run<F>(app: &AppHandle, show: F) -> Result<Option<PathBuf>, AppError>
where
    F: FnOnce(FileDialogBuilder<Wry>) -> Option<FilePath> + Send + 'static,
{
    let window = app.get_webview_window("main");
    let mut dialog = app.dialog().file();
    if let Some(window) = &window {
        dialog = dialog.set_parent(window);
    }

    // The dialog takes focus from the main window; keep the window up behind
    // it rather than letting the blur handler hide it (and the dialog with it).
    let skip = app.state::<crate::SkipBlurHide>();
    skip.0.store(true, Ordering::Relaxed);
    let picked = tokio::task::spawn_blocking(move || show(dialog)).await;
    // When the dialog closes the window manager may hand focus to another app
    // first; take it back before the blur handler is re-armed.
    if let Some(window) = &window {
        let _ = window.set_focus();
        for _ in 0..50 {
            if window.is_focused().unwrap_or(false) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
    skip.0.store(false, Ordering::Relaxed);

    picked
        .map_err(|e| AppError::Other { message: e.to_string() })?
        .map(|path| path.into_path().map_err(|e| AppError::Other { message: e.to_string() }))
        .transpose()
}
