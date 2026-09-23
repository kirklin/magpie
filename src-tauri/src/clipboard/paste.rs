//! Paste-back orchestration. Platform-agnostic: the OS-specific bits (reading
//! the focused window, handing focus back, synthesizing ⌘/Ctrl+V) live behind
//! the [`Paster`](crate::platform::Paster) port. This module just sequences them.

use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::platform::{FocusedWindow, PasterPort};

/// How long a freshly focused app gets to become first responder before the
/// synthesized ⌘/Ctrl+V arrives. Without it the keystroke can race the focus
/// change and be dropped or land in Magpie.
const SETTLE: Duration = Duration::from_millis(40);

/// Where focus cannot be observed (Wayland), how long to give the compositor to
/// hand keyboard focus back after Magpie's window is hidden.
const BLIND_FOCUS_HANDOFF: Duration = Duration::from_millis(150);

/// Hide Magpie, give focus back to the app it was summoned from, and paste
/// there. The content must already be on the clipboard.
pub async fn paste_into_previous_app(app_handle: &AppHandle) -> Result<(), String> {
    let paster = app_handle.state::<PasterPort>().inner().clone();
    let previous = app_handle.state::<crate::PreviousApp>().get();

    paster.hide_and_restore_focus(previous.as_ref())?;
    wait_for_focus_to_leave_magpie(&paster).await;
    // The window is hidden by now, so the frontend's error toast goes unseen.
    paster.paste().inspect_err(|e| log::error!("[Paste] failed: {e}"))
}

/// Poll until the focused window is no longer Magpie's (up to ~500ms), then add
/// a short settle delay. Returns whether focus was seen to move.
pub async fn wait_for_focus_to_leave_magpie(paster: &PasterPort) -> bool {
    if !paster.capabilities().can_read_focus {
        tokio::time::sleep(BLIND_FOCUS_HANDOFF).await;
        return true;
    }
    for _ in 0..50 {
        let focused = paster.focused_window();
        if focused.is_known() && !focused.is_magpie() {
            log::debug!("Focus moved to {:?}", focused);
            tokio::time::sleep(SETTLE).await;
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    log::warn!("Focus never left Magpie before paste");
    false
}

/// Poll until `target` is the focused window (up to ~500ms), then add a short
/// settle delay. Returns whether it became focused.
pub async fn wait_until_focused(paster: &PasterPort, target: &FocusedWindow) -> bool {
    for _ in 0..50 {
        if paster.focused_window().same_target(target) {
            tokio::time::sleep(SETTLE).await;
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    log::warn!("Target {:?} never became focused before paste", target);
    false
}
