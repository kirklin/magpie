use std::collections::HashMap;
use std::sync::Mutex;
use tauri::State;

use crate::error::AppError;

/// In-memory cache for app icons (bundle_id -> base64 PNG)
pub struct AppIconCache(pub Mutex<HashMap<String, String>>);

impl Default for AppIconCache {
    fn default() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}

/// Get the app icon as a base64-encoded PNG string for a given bundle ID
#[tauri::command]
#[specta::specta]
pub async fn get_app_icon(
    bundle_id: String,
    cache: State<'_, AppIconCache>,
) -> Result<String, AppError> {
    // Check cache first
    {
        let c = cache.0.lock().map_err(|e| AppError::Other { message: e.to_string() })?;
        if let Some(cached) = c.get(&bundle_id) {
            return Ok(cached.clone());
        }
    }

    // Fetch from system
    let icon_base64 = fetch_app_icon_macos(&bundle_id)?;

    // Store in cache (bounded to avoid unbounded growth over a long session).
    {
        const MAX_CACHED_ICONS: usize = 256;
        let mut c = cache.0.lock().map_err(|e| AppError::Other { message: e.to_string() })?;
        if c.len() >= MAX_CACHED_ICONS && !c.contains_key(&bundle_id) {
            c.clear();
        }
        c.insert(bundle_id, icon_base64.clone());
    }

    Ok(icon_base64)
}

#[tauri::command]
#[specta::specta]
pub async fn get_file_icon(
    file_path: String,
) -> Result<String, AppError> {
    // We don't cache file icons for now as they might change or be too numerous
    fetch_file_icon_macos(&file_path).map_err(AppError::from)
}

#[cfg(target_os = "macos")]
fn fetch_app_icon_macos(bundle_id: &str) -> Result<String, String> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;

    let workspace = NSWorkspace::sharedWorkspace();

    // Get path for the bundle ID
    let ns_bundle_id = NSString::from_str(bundle_id);
    let url = workspace
        .URLForApplicationWithBundleIdentifier(&ns_bundle_id)
        .ok_or_else(|| format!("App not found: {}", bundle_id))?;

    let path = url.path().ok_or("No path for app URL")?;

    fetch_icon_for_path(&path, 32.0)
}

#[cfg(target_os = "macos")]
fn fetch_file_icon_macos(file_path: &str) -> Result<String, String> {
    use objc2_foundation::NSString;
    let path = NSString::from_str(file_path);
    // Request a larger size for file icons in the preview panel
    fetch_icon_for_path(&path, 128.0)
}

/// Render the icon for `path` as a base64 PNG data URL.
///
/// The whole body runs inside an `autoreleasepool`, and that is load-bearing
/// rather than tidiness. Every step here hands back an autoreleased object:
/// `iconForFile:` returns an NSImage carrying EVERY representation of the icon
/// (16pt through 1024pt), and `TIFFRepresentation` serializes all of them into
/// one UNCOMPRESSED NSData — megabytes per call. Tauri commands run on runtime
/// worker threads that have no pool of their own, so without this those objects
/// were never released: scrolling the history once leaked hundreds of MB into
/// the Foundation zone, and it never came back. `get_file_icon` deliberately
/// has no Rust-side cache, so it is called for every distinct file path, which
/// is what turned the leak into gigabytes.
#[cfg(target_os = "macos")]
fn fetch_icon_for_path(path: &objc2_foundation::NSString, size: f64) -> Result<String, String> {
    use objc2::rc::autoreleasepool;

    autoreleasepool(|_| {
        use objc2_app_kit::{NSBitmapImageRep, NSWorkspace};
        use objc2_foundation::NSData;
        use objc2::rc::Retained;

        let workspace = NSWorkspace::sharedWorkspace();

        // Get icon for the app path
        let icon = workspace.iconForFile(path);

        // Set a reasonable size for the icon. This shrinks what gets drawn, but
        // NOT what TIFFRepresentation serializes below — hence the pool.
        let ns_size = objc2_foundation::NSSize::new(size, size);
        icon.setSize(ns_size);

        // Convert to TIFF data
        let tiff_data: Retained<NSData> = icon.TIFFRepresentation()
            .ok_or("Failed to get TIFF representation")?;

        // Create bitmap rep from TIFF
        let bitmap_rep = NSBitmapImageRep::imageRepWithData(&tiff_data)
            .ok_or("Failed to create bitmap rep")?;

        // Convert to PNG
        use objc2_app_kit::NSBitmapImageFileType;
        let png_data = unsafe { bitmap_rep
            .representationUsingType_properties(NSBitmapImageFileType::PNG, &objc2_foundation::NSDictionary::new()) }
            .ok_or("Failed to convert to PNG")?;

        // Copy the bytes out BEFORE the pool drains — png_data dies with it.
        let bytes = png_data.to_vec();
        let base64_str = base64_encode(&bytes);

        Ok(format!("data:image/png;base64,{}", base64_str))
    })
}

#[cfg(not(target_os = "macos"))]
fn fetch_app_icon_macos(_bundle_id: &str) -> Result<String, String> {
    Err("App icons are only supported on macOS".to_string())
}

/// Simple base64 encoder (no external dependency needed)
fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((input.len() + 2) / 3 * 4);

    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let combined = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((combined >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((combined >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(CHARS[((combined >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARS[(combined & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

/// Hide the main window
#[tauri::command]
#[specta::specta]
pub fn hide_window(app_handle: tauri::AppHandle) {
    if let Some(window) = tauri::Manager::get_webview_window(&app_handle, "main") {
        let _ = window.hide();
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    /// Resident set size of this process, in MB.
    fn rss_mb() -> u64 {
        let out = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .expect("ps");
        String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().unwrap_or(0) / 1024
    }

    /// Icon fetching must not grow memory without bound.
    ///
    /// `fetch_icon_for_path` builds an NSImage holding every representation of
    /// the icon and serializes all of them into one uncompressed TIFF. Without
    /// an `autoreleasepool` around it those objects are never released on a
    /// Tauri worker thread, and repeated calls (one per distinct file path in
    /// the history list, uncached by design) grow the Foundation zone into the
    /// gigabytes. Ignored by default: it measures process RSS, so it is timing
    /// and machine dependent rather than a clean unit assertion.
    ///
    /// Run with: cargo test --lib icon_fetch_does_not_leak -- --ignored --nocapture
    /// Byte-for-byte what `fetch_icon_for_path` does, minus the autoreleasepool.
    /// Exists purely so the test can A/B the pool against its absence in ONE
    /// process — comparing across runs is too noisy to prove anything.
    fn fetch_icon_unpooled(path: &objc2_foundation::NSString, size: f64) -> Result<String, String> {
        use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};

        let workspace = NSWorkspace::sharedWorkspace();
        let icon = workspace.iconForFile(path);
        icon.setSize(objc2_foundation::NSSize::new(size, size));
        let tiff_data = icon.TIFFRepresentation().ok_or("no tiff")?;
        let bitmap_rep = NSBitmapImageRep::imageRepWithData(&tiff_data).ok_or("no rep")?;
        let png_data = unsafe {
            bitmap_rep.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &objc2_foundation::NSDictionary::new(),
            )
        }
        .ok_or("no png")?;
        Ok(base64_encode(&png_data.to_vec()))
    }

    #[test]
    #[ignore = "measures process RSS; run explicitly"]
    fn icon_fetch_does_not_leak() {
        const N: usize = 400;
        let path = objc2_foundation::NSString::from_str("/Applications");

        // Warm up so first-call initialization counts against neither variant.
        for _ in 0..50 {
            let _ = fetch_icon_for_path(&path, 128.0);
        }

        let base = rss_mb();
        for _ in 0..N {
            let _ = fetch_icon_unpooled(&path, 128.0);
        }
        let unpooled = rss_mb().saturating_sub(base);

        // Reclaim what the unpooled run stranded, so the pooled measurement
        // starts from a settled baseline rather than inheriting that growth.
        objc2::rc::autoreleasepool(|_| {});
        let base = rss_mb();
        for _ in 0..N {
            let _ = fetch_icon_for_path(&path, 128.0);
        }
        let pooled = rss_mb().saturating_sub(base);

        println!("over {N} icon fetches — without pool: +{unpooled} MB, with pool: +{pooled} MB");
        assert!(
            pooled < 50,
            "pooled icon fetching still grew {pooled} MB over {N} calls"
        );
    }
}
