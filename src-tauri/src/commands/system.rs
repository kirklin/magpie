use std::collections::HashMap;
use std::sync::Mutex;
use tauri::State;

/// In-memory cache for app icons (bundle_id -> base64 PNG)
pub struct AppIconCache(pub Mutex<HashMap<String, String>>);

impl Default for AppIconCache {
    fn default() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}

/// Get the app icon as a base64-encoded PNG string for a given bundle ID
#[tauri::command]
pub async fn get_app_icon(
    bundle_id: String,
    cache: State<'_, AppIconCache>,
) -> Result<String, String> {
    // Check cache first
    {
        let c = cache.0.lock().map_err(|e| e.to_string())?;
        if let Some(cached) = c.get(&bundle_id) {
            return Ok(cached.clone());
        }
    }

    // Fetch from system
    let icon_base64 = fetch_app_icon_macos(&bundle_id)?;

    // Store in cache (bounded to avoid unbounded growth over a long session).
    {
        const MAX_CACHED_ICONS: usize = 256;
        let mut c = cache.0.lock().map_err(|e| e.to_string())?;
        if c.len() >= MAX_CACHED_ICONS && !c.contains_key(&bundle_id) {
            c.clear();
        }
        c.insert(bundle_id, icon_base64.clone());
    }

    Ok(icon_base64)
}

#[tauri::command]
pub async fn get_file_icon(
    file_path: String,
) -> Result<String, String> {
    // We don't cache file icons for now as they might change or be too numerous
    fetch_file_icon_macos(&file_path)
}

#[cfg(target_os = "macos")]
fn fetch_app_icon_macos(bundle_id: &str) -> Result<String, String> {
    use objc2_app_kit::{NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSData, NSString};
    use objc2::rc::Retained;

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

#[cfg(target_os = "macos")]
fn fetch_icon_for_path(path: &objc2_foundation::NSString, size: f64) -> Result<String, String> {
    use objc2_app_kit::{NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::NSData;
    use objc2::rc::Retained;
    
    let workspace = NSWorkspace::sharedWorkspace();

    // Get icon for the app path
    let icon = workspace.iconForFile(path);

    // Set a reasonable size for the icon
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

    // Encode to base64
    let bytes = png_data.to_vec();
    let base64_str = base64_encode(&bytes);

    Ok(format!("data:image/png;base64,{}", base64_str))
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
pub fn hide_window(app_handle: tauri::AppHandle) {
    if let Some(window) = tauri::Manager::get_webview_window(&app_handle, "main") {
        let _ = window.hide();
    }
}
