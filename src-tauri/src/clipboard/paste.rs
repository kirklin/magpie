//! Paste-back orchestration. Platform-agnostic: the OS-specific bits (reading
//! the frontmost app, synthesizing ⌘/Ctrl+V, activating an app) live behind the
//! [`Paster`](crate::platform::Paster) port. This module just sequences them.

use crate::platform::PasterPort;

/// Magpie's own bundle identifier — used to tell "focus is still on Magpie"
/// from "focus has moved to the target app".
pub const MAGPIE_BUNDLE_ID: &str = "com.magpie.clipboard";

/// Poll until `target_id` is the frontmost app (up to ~500ms), then add a short
/// settle delay. Returns whether it became frontmost.
pub async fn wait_until_frontmost(paster: &PasterPort, target_id: &str) -> bool {
    for _ in 0..50 {
        if let Some(id) = paster.frontmost_app().bundle_id {
            if id == target_id {
                tokio::time::sleep(std::time::Duration::from_millis(40)).await;
                return true;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    log::warn!("Target app {} never became frontmost before paste", target_id);
    false
}

/// Poll until the frontmost application is NOT `ignore_id`, then add a short
/// settle delay so the newly-focused app is ready to receive the synthesized
/// ⌘/Ctrl+V. Returns whether the focus actually switched.
pub async fn wait_for_frontmost_app_switch(paster: &PasterPort, ignore_id: &str) -> bool {
    let mut switched = false;
    for _ in 0..50 {
        // max ~500ms
        if let Some(id) = paster.frontmost_app().bundle_id {
            if id != ignore_id {
                log::debug!("Active app switched to: {}", id);
                switched = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    if switched {
        // Give the now-frontmost app a moment to become first responder before
        // we synthesize the paste keystroke — without this the ⌘/Ctrl+V can race
        // the focus change and be dropped or land in Magpie.
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    } else {
        log::warn!("Frontmost app never switched away from {} before paste", ignore_id);
    }

    switched
}
