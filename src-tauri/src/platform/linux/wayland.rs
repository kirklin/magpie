//! Wayland protocols Magpie speaks directly (each on its own connection):
//!
//! - data-control (`ext-data-control-v1`, or the older
//!   `wlr-data-control-unstable-v1`): clipboard change events and the offered
//!   MIME types. The content itself is read and written through arboard.
//! - `wlr-foreign-toplevel-management-unstable-v1`: which window is focused,
//!   and activating another one.
//! - `virtual-keyboard-unstable-v1`: the synthetic Ctrl+V.

use std::collections::HashMap;
use std::io::Write;
use std::os::fd::AsFd;
use std::sync::{Arc, Mutex};

use wayland_client::backend::ObjectId;
use wayland_client::globals::{registry_queue_init, GlobalList, GlobalListContents};
use wayland_client::protocol::{wl_registry, wl_seat::WlSeat};
use wayland_client::{event_created_child, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::{self, ExtDataControlOfferV1},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1::{self, ZwlrDataControlDeviceV1},
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_offer_v1::{self, ZwlrDataControlOfferV1},
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

use super::SelectionState;
use crate::platform::FocusedWindow;

fn err(e: impl std::fmt::Display) -> String {
    format!("Wayland: {e}")
}

fn has_global(globals: &GlobalList, interface: &str) -> bool {
    globals.contents().with_list(|list| list.iter().any(|g| g.interface == interface))
}

/// State shared by every connection's registry and seat; they need no handling.
macro_rules! ignore_events {
    ($state:ty: $($proxy:ty),+) => {
        $(impl Dispatch<$proxy, ()> for $state {
            fn event(_: &mut Self, _: &$proxy, _: <$proxy as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
        })+
        impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for $state {
            fn event(_: &mut Self, _: &wl_registry::WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
        }
    };
}

// ---------------------------------------------------------------------------
// data-control
// ---------------------------------------------------------------------------

enum Manager {
    Ext(ExtDataControlManagerV1),
    Wlr(ZwlrDataControlManagerV1),
}

enum Offer {
    Ext(ExtDataControlOfferV1),
    Wlr(ZwlrDataControlOfferV1),
}

impl Offer {
    fn id(&self) -> ObjectId {
        match self {
            Offer::Ext(o) => o.id(),
            Offer::Wlr(o) => o.id(),
        }
    }

    fn destroy(&self) {
        match self {
            Offer::Ext(o) => o.destroy(),
            Offer::Wlr(o) => o.destroy(),
        }
    }
}

struct Watch {
    state: Arc<SelectionState>,
    manager: Manager,
    seat: WlSeat,
    /// MIME types announced for offers that haven't become the selection yet.
    announced: HashMap<ObjectId, Vec<String>>,
    current: Option<Offer>,
}

impl Watch {
    fn get_device(&self, qh: &QueueHandle<Self>) {
        match &self.manager {
            Manager::Ext(m) => {
                m.get_data_device(&self.seat, qh, ());
            }
            Manager::Wlr(m) => {
                m.get_data_device(&self.seat, qh, ());
            }
        }
    }

    fn selection(&mut self, offer: Option<Offer>) {
        if let Some(previous) = self.current.take() {
            previous.destroy();
        }
        match offer {
            Some(offer) => {
                let types = self.announced.remove(&offer.id()).unwrap_or_default();
                self.current = Some(offer);
                self.state.set_mime_types(types);
                self.state.bump();
            }
            // Cleared (its owner quit): there is nothing new to capture.
            None => self.state.set_mime_types(Vec::new()),
        }
    }
}

ignore_events!(Watch: WlSeat, ExtDataControlManagerV1, ZwlrDataControlManagerV1);

impl Dispatch<ExtDataControlDeviceV1, ()> for Watch {
    fn event(
        watch: &mut Self,
        _: &ExtDataControlDeviceV1,
        event: ext_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_device_v1::Event::DataOffer { id } => {
                watch.announced.insert(id.id(), Vec::new());
            }
            ext_data_control_device_v1::Event::Selection { id } => watch.selection(id.map(Offer::Ext)),
            ext_data_control_device_v1::Event::PrimarySelection { id: Some(offer) } => {
                watch.announced.remove(&offer.id());
                offer.destroy();
            }
            // The device became invalid (e.g. its seat went away): get a new one.
            ext_data_control_device_v1::Event::Finished => watch.get_device(qh),
            _ => {}
        }
    }

    event_created_child!(Watch, ExtDataControlDeviceV1, [
        ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ZwlrDataControlDeviceV1, ()> for Watch {
    fn event(
        watch: &mut Self,
        _: &ZwlrDataControlDeviceV1,
        event: zwlr_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_data_control_device_v1::Event::DataOffer { id } => {
                watch.announced.insert(id.id(), Vec::new());
            }
            zwlr_data_control_device_v1::Event::Selection { id } => watch.selection(id.map(Offer::Wlr)),
            zwlr_data_control_device_v1::Event::PrimarySelection { id: Some(offer) } => {
                watch.announced.remove(&offer.id());
                offer.destroy();
            }
            zwlr_data_control_device_v1::Event::Finished => watch.get_device(qh),
            _ => {}
        }
    }

    event_created_child!(Watch, ZwlrDataControlDeviceV1, [
        zwlr_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ZwlrDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ExtDataControlOfferV1, ()> for Watch {
    fn event(
        watch: &mut Self,
        offer: &ExtDataControlOfferV1,
        event: ext_data_control_offer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_data_control_offer_v1::Event::Offer { mime_type } = event {
            watch.announced.entry(offer.id()).or_default().push(mime_type);
        }
    }
}

impl Dispatch<ZwlrDataControlOfferV1, ()> for Watch {
    fn event(
        watch: &mut Self,
        offer: &ZwlrDataControlOfferV1,
        event: zwlr_data_control_offer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_data_control_offer_v1::Event::Offer { mime_type } = event {
            watch.announced.entry(offer.id()).or_default().push(mime_type);
        }
    }
}

/// Start counting clipboard changes through data-control. Returns the
/// protocol in use, or `None` when the compositor offers neither (GNOME).
pub fn watch_data_control(state: Arc<SelectionState>) -> Result<Option<&'static str>, String> {
    let conn = Connection::connect_to_env().map_err(err)?;
    let (globals, mut queue) = registry_queue_init::<Watch>(&conn).map_err(err)?;
    let qh = queue.handle();

    let (manager, protocol) = if let Ok(m) = globals.bind::<ExtDataControlManagerV1, _, _>(&qh, 1..=1, ()) {
        (Manager::Ext(m), "ext-data-control")
    } else if let Ok(m) = globals.bind::<ZwlrDataControlManagerV1, _, _>(&qh, 1..=2, ()) {
        (Manager::Wlr(m), "wlr-data-control")
    } else {
        return Ok(None);
    };
    let seat: WlSeat = globals.bind(&qh, 1..=1, ()).map_err(err)?;

    let mut watch = Watch { state, manager, seat, announced: HashMap::new(), current: None };
    watch.get_device(&qh);
    // The compositor announces the current selection right away; take it in
    // before returning so content copied before Magpie started counts too.
    queue.roundtrip(&mut watch).map_err(err)?;

    std::thread::Builder::new()
        .name("wayland-clipboard-watch".into())
        .spawn(move || loop {
            if let Err(e) = queue.blocking_dispatch(&mut watch) {
                log::error!("[Wayland] clipboard watch connection lost: {e}");
                return;
            }
        })
        .map_err(err)?;
    Ok(Some(protocol))
}

// ---------------------------------------------------------------------------
// Foreign toplevels (the focused window)
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Toplevel {
    app_id: Option<String>,
    activated: bool,
    /// Double-buffered: applied on `done`.
    pending_app_id: Option<String>,
    pending_activated: Option<bool>,
}

type Windows = Arc<Mutex<HashMap<u32, (ZwlrForeignToplevelHandleV1, Toplevel)>>>;

/// Owned by the dispatching thread; readers only take the `windows` lock.
#[derive(Default)]
struct Toplevels {
    windows: Windows,
}

/// Toplevel state value of `zwlr_foreign_toplevel_handle_v1.state`.
const STATE_ACTIVATED: u32 = 2;

ignore_events!(Toplevels: WlSeat);

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for Toplevels {
    fn event(
        toplevels: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } = event {
            let mut windows = toplevels.windows.lock().expect("toplevel lock poisoned");
            windows.insert(toplevel.id().protocol_id(), (toplevel, Toplevel::default()));
        }
    }

    event_created_child!(Toplevels, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for Toplevels {
    fn event(
        toplevels: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = handle.id().protocol_id();
        let mut windows = toplevels.windows.lock().expect("toplevel lock poisoned");
        if let zwlr_foreign_toplevel_handle_v1::Event::Closed = event {
            if let Some((handle, _)) = windows.remove(&id) {
                handle.destroy();
            }
            return;
        }
        let Some((_, toplevel)) = windows.get_mut(&id) else {
            return;
        };
        match event {
            zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => toplevel.pending_app_id = Some(app_id),
            zwlr_foreign_toplevel_handle_v1::Event::State { state } => {
                let activated = state.as_chunks::<4>().0.iter().any(|v| u32::from_ne_bytes(*v) == STATE_ACTIVATED);
                toplevel.pending_activated = Some(activated);
            }
            zwlr_foreign_toplevel_handle_v1::Event::Done => {
                if let Some(app_id) = toplevel.pending_app_id.take() {
                    toplevel.app_id = Some(app_id);
                }
                if let Some(activated) = toplevel.pending_activated.take() {
                    toplevel.activated = activated;
                }
            }
            _ => {}
        }
    }
}

/// The focused toplevel as reported by the compositor.
pub struct FocusedToplevel {
    pub focus: FocusedWindow,
    pub app_id: Option<String>,
}

/// Tracks which toplevel is focused, on compositors offering
/// `wlr-foreign-toplevel-management` (Sway, Hyprland, niri, labwc, …).
pub struct ToplevelTracker {
    conn: Connection,
    seat: WlSeat,
    windows: Windows,
    /// The app id GTK gives Magpie's own window (the program name), which
    /// tells Magpie's window apart from the one it pastes into.
    own_app_id: String,
}

impl ToplevelTracker {
    /// `None` when the compositor doesn't offer the protocol (GNOME, KDE).
    pub fn connect() -> Result<Option<Self>, String> {
        let conn = Connection::connect_to_env().map_err(err)?;
        let (globals, mut queue) = registry_queue_init::<Toplevels>(&conn).map_err(err)?;
        let qh = queue.handle();
        let Ok(_manager) = globals.bind::<ZwlrForeignToplevelManagerV1, _, _>(&qh, 1..=3, ()) else {
            return Ok(None);
        };
        let seat: WlSeat = globals.bind(&qh, 1..=1, ()).map_err(err)?;

        // The first roundtrip announces the toplevels, the second their state.
        let mut state = Toplevels::default();
        queue.roundtrip(&mut state).map_err(err)?;
        queue.roundtrip(&mut state).map_err(err)?;
        let windows = Arc::clone(&state.windows);

        std::thread::Builder::new()
            .name("wayland-toplevels".into())
            .spawn(move || loop {
                if let Err(e) = queue.blocking_dispatch(&mut state) {
                    log::error!("[Wayland] toplevel tracking connection lost: {e}");
                    return;
                }
            })
            .map_err(err)?;
        let own_app_id = glib::prgname().ok_or("GTK has no program name")?.to_string();
        Ok(Some(Self { conn, seat, windows, own_app_id }))
    }

    pub fn focused(&self) -> Option<FocusedToplevel> {
        let windows = self.windows.lock().expect("toplevel lock poisoned");
        windows.iter().find(|(_, (_, t))| t.activated).map(|(id, (_, t))| {
            let is_magpie = t.app_id.as_deref() == Some(self.own_app_id.as_str());
            FocusedToplevel {
                focus: FocusedWindow {
                    pid: is_magpie.then(std::process::id),
                    window: Some(u64::from(*id)),
                },
                app_id: t.app_id.clone(),
            }
        })
    }

    pub fn activate(&self, window: u64) -> bool {
        let windows = self.windows.lock().expect("toplevel lock poisoned");
        let Some((handle, _)) = u32::try_from(window).ok().and_then(|id| windows.get(&id)) else {
            return false;
        };
        handle.activate(&self.seat);
        self.conn.flush().is_ok()
    }

    /// Activate Magpie's own window.
    pub fn activate_own(&self) -> bool {
        let windows = self.windows.lock().expect("toplevel lock poisoned");
        let own = windows.values().find(|(_, t)| t.app_id.as_deref() == Some(self.own_app_id.as_str()));
        let Some((handle, _)) = own else {
            return false;
        };
        handle.activate(&self.seat);
        self.conn.flush().is_ok()
    }
}

// ---------------------------------------------------------------------------
// Virtual keyboard (the synthetic Ctrl+V)
// ---------------------------------------------------------------------------

struct Keyboard;

ignore_events!(Keyboard: WlSeat, ZwpVirtualKeyboardManagerV1, ZwpVirtualKeyboardV1);

/// Whether the compositor lets clients create virtual keyboards (Sway,
/// Hyprland, niri, COSMIC — not GNOME or KDE).
pub fn virtual_keyboard_available() -> Result<bool, String> {
    let conn = Connection::connect_to_env().map_err(err)?;
    let (globals, _queue) = registry_queue_init::<Keyboard>(&conn).map_err(err)?;
    Ok(has_global(&globals, "zwp_virtual_keyboard_manager_v1"))
}

/// A regular US layout on evdev keycodes, the same keymap a physical keyboard
/// would carry, so every client interprets the keys as a real Ctrl+V.
const KEYMAP: &str = "xkb_keymap {\n\
    xkb_keycodes \"magpie\" { include \"evdev\" };\n\
    xkb_types \"magpie\" { include \"complete\" };\n\
    xkb_compatibility \"magpie\" { include \"complete\" };\n\
    xkb_symbols \"magpie\" { include \"pc+us\" };\n\
};\n";
/// `wl_keyboard.keymap_format.xkb_v1`.
const KEYMAP_FORMAT_XKB_V1: u32 = 1;
/// Linux evdev code of the V key (input-event-codes.h).
const KEY_V: u32 = 47;
/// The Control modifier's mask in the keymap above.
const MOD_CONTROL: u32 = 1 << 2;

pub fn paste_with_virtual_keyboard() -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(err)?;
    let (globals, mut queue) = registry_queue_init::<Keyboard>(&conn).map_err(err)?;
    let qh = queue.handle();
    let seat: WlSeat = globals.bind(&qh, 1..=1, ()).map_err(err)?;
    let manager: ZwpVirtualKeyboardManagerV1 = globals.bind(&qh, 1..=1, ()).map_err(err)?;
    let keyboard = manager.create_virtual_keyboard(&seat, &qh, ());

    let fd = rustix::fs::memfd_create("magpie-keymap", rustix::fs::MemfdFlags::CLOEXEC).map_err(err)?;
    let mut file = std::fs::File::from(fd);
    let keymap = format!("{KEYMAP}\0");
    file.write_all(keymap.as_bytes()).map_err(err)?;
    keyboard.keymap(KEYMAP_FORMAT_XKB_V1, file.as_fd(), keymap.len() as u32);
    // The focused client drops a key that arrives together with the keymap
    // switch to this new device; let the switch land first.
    queue.roundtrip(&mut Keyboard).map_err(err)?;
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Only Control is down for the V press, whatever the user still holds on
    // the real keyboard: this device carries its own modifier state.
    let time = 0;
    keyboard.modifiers(MOD_CONTROL, 0, 0, 0);
    keyboard.key(time, KEY_V, 1);
    keyboard.key(time, KEY_V, 0);
    keyboard.modifiers(0, 0, 0, 0);
    queue.roundtrip(&mut Keyboard).map_err(err)?;
    keyboard.destroy();
    conn.flush().map_err(err)?;
    log::debug!("[Paste] Simulated Ctrl+V via virtual keyboard");
    Ok(())
}
