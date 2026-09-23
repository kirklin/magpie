//! Desktop entries (application names, icons, Magpie's own identity) and
//! icons from the GTK icon theme.

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use gio::prelude::*;
use tauri::AppHandle;

use super::APP_ID;
use crate::platform::Icons;

/// Name an application from what the window reveals: `class` is the X11
/// WM_CLASS class or the Wayland app id, `exe` the process's executable.
/// Returns (app id, display name); the app id is the desktop entry id when one
/// matches, so its icon can be found later.
pub fn describe_app(class: Option<&str>, exe: Option<&str>) -> (Option<String>, Option<String>) {
    if let Some(entry) = find_entry(class, exe) {
        return (entry.id().map(|id| id.to_string()), Some(entry.display_name().to_string()));
    }
    let exe_name = exe.and_then(|e| Path::new(e).file_name()).map(|n| n.to_string_lossy().into_owned());
    (exe.or(class).map(str::to_string), class.map(str::to_string).or(exe_name))
}

fn find_entry(class: Option<&str>, exe: Option<&str>) -> Option<gio::DesktopAppInfo> {
    // A Wayland app id is the desktop entry's name by convention.
    if let Some(entry) = class.and_then(|c| gio::DesktopAppInfo::new(&format!("{c}.desktop"))) {
        return Some(entry);
    }
    let entries: Vec<gio::DesktopAppInfo> = gio::AppInfo::all()
        .into_iter()
        .filter_map(|info| info.downcast::<gio::DesktopAppInfo>().ok())
        .collect();

    if let Some(class) = class {
        let by_wm_class = entries.iter().find(|e| {
            e.startup_wm_class().is_some_and(|wm| wm.eq_ignore_ascii_case(class))
        });
        let by_id = || {
            entries.iter().find(|e| {
                e.id().is_some_and(|id| {
                    let stem = id.trim_end_matches(".desktop");
                    stem.eq_ignore_ascii_case(class)
                        || stem.rsplit('.').next().is_some_and(|last| last.eq_ignore_ascii_case(class))
                })
            })
        };
        if let Some(entry) = by_wm_class.or_else(by_id) {
            return Some(entry.clone());
        }
    }

    let exe_name = Path::new(exe?).file_name()?;
    entries
        .iter()
        .find(|e| e.executable().file_name() == Some(exe_name))
        .cloned()
}

/// Install `com.magpie.clipboard.desktop` for the current user. Desktop portals
/// identify an unsandboxed app by a desktop entry named after its app id; the
/// packages only ship `Magpie.desktop`. Hidden from menus, which list that one.
pub fn ensure_identity_entry() {
    let Some(dir) = dirs::data_dir().map(|d| d.join("applications")) else {
        return;
    };
    let exe = std::env::var_os("APPIMAGE")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_exe().ok());
    let Some(exe) = exe else {
        return;
    };
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Magpie\nExec=\"{}\" --toggle\nIcon=magpie\nNoDisplay=true\n",
        exe.display()
    );
    let path = dir.join(format!("{APP_ID}.desktop"));
    if std::fs::read_to_string(&path).is_ok_and(|current| current == entry) {
        return;
    }
    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, entry)) {
        log::warn!("[Linux] couldn't write {}: {e}", path.display());
    }
}

// ---------------------------------------------------------------------------
// Icons
// ---------------------------------------------------------------------------

pub struct LinuxIcons {
    app: AppHandle,
}

impl LinuxIcons {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    /// Render `icon` at `size` pixels through the GTK icon theme, which lives
    /// on the main thread. GIcons aren't `Send`, so the icon travels there in
    /// its serialized form.
    fn render(&self, icon: gio::Icon, size: i32) -> Result<Vec<u8>, String> {
        let serialized = IconExt::to_string(&icon).ok_or("icon can't be serialized")?.to_string();
        let (tx, rx) = mpsc::channel();
        self.app
            .run_on_main_thread(move || {
                let result = (|| {
                    use gtk::prelude::*;
                    let icon = gio::Icon::for_string(&serialized).map_err(|e| e.to_string())?;
                    let theme = gtk::IconTheme::default().ok_or("no icon theme")?;
                    let info = theme
                        .lookup_by_gicon(&icon, size, gtk::IconLookupFlags::FORCE_SIZE)
                        .ok_or("icon not in the theme")?;
                    let pixbuf = info.load_icon().map_err(|e| e.to_string())?;
                    pixbuf.save_to_bufferv("png", &[]).map_err(|e| e.to_string())
                })();
                let _ = tx.send(result);
            })
            .map_err(|e| e.to_string())?;
        rx.recv_timeout(Duration::from_secs(2)).map_err(|e| e.to_string())?
    }
}

impl Icons for LinuxIcons {
    fn app_icon(&self, app_id: &str) -> Result<String, String> {
        let entry = gio::DesktopAppInfo::new(app_id).ok_or_else(|| format!("No desktop entry {app_id}"))?;
        let icon = entry.icon().ok_or_else(|| format!("{app_id} has no icon"))?;
        self.render(icon, 32).map(|png| crate::platform::png_data_url(&png))
    }

    fn file_icon(&self, path: &str) -> Result<String, String> {
        let icon = if Path::new(path).exists() {
            gio::File::for_path(path)
                .query_info("standard::icon", gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE)
                .map_err(|e| e.to_string())?
                .icon()
                .ok_or("no icon for this file")?
        } else {
            // A file copied earlier and deleted since still gets the icon of
            // its type, guessed from the name.
            let (content_type, _) = gio::content_type_guess(Some(path), &[]);
            gio::content_type_get_icon(&content_type)
        };
        self.render(icon, 128).map(|png| crate::platform::png_data_url(&png))
    }
}
