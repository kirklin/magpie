//! X11: clipboard change events (XFixes), the offered targets, the focused
//! window (EWMH) and paste-back (XTest).
//!
//! Also used on GNOME's Wayland session, where Mutter offers no data-control
//! protocol but bridges the Wayland clipboard to X clients through XWayland:
//! every copy re-asserts Mutter's ownership of CLIPBOARD, which XFixes reports.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xfixes::{ConnectionExt as _, SelectionEventMask};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConnectionExt as _, CreateWindowAux, EventMask, Window,
    WindowClass, KEY_PRESS_EVENT, KEY_RELEASE_EVENT,
};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, CURRENT_TIME, NONE};

use super::SelectionState;
use crate::platform::FocusedWindow;

fn err(e: impl std::fmt::Display) -> String {
    format!("X11: {e}")
}

struct Display {
    conn: RustConnection,
    root: Window,
    /// A 1x1 unmapped window: the requestor of selection conversions.
    window: Window,
    atoms: Mutex<HashMap<String, Atom>>,
    names: Mutex<HashMap<Atom, String>>,
}

impl Display {
    fn open() -> Result<Self, String> {
        let (conn, screen) = x11rb::connect(None).map_err(err)?;
        let root = conn.setup().roots[screen].root;
        let window = conn.generate_id().map_err(err)?;
        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            window,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            COPY_FROM_PARENT,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .map_err(err)?;
        conn.flush().map_err(err)?;
        Ok(Self { conn, root, window, atoms: Mutex::default(), names: Mutex::default() })
    }

    fn atom(&self, name: &str) -> Result<Atom, String> {
        if let Some(atom) = self.atoms.lock().expect("atom cache poisoned").get(name) {
            return Ok(*atom);
        }
        let atom = self.conn.intern_atom(false, name.as_bytes()).map_err(err)?.reply().map_err(err)?.atom;
        self.atoms.lock().expect("atom cache poisoned").insert(name.to_string(), atom);
        Ok(atom)
    }

    fn atom_name(&self, atom: Atom) -> Result<String, String> {
        if let Some(name) = self.names.lock().expect("atom name cache poisoned").get(&atom) {
            return Ok(name.clone());
        }
        let reply = self.conn.get_atom_name(atom).map_err(err)?.reply().map_err(err)?;
        let name = String::from_utf8_lossy(&reply.name).into_owned();
        self.names.lock().expect("atom name cache poisoned").insert(atom, name.clone());
        Ok(name)
    }

    /// First 32-bit value of a window property of the given type.
    fn property_u32(&self, window: Window, name: &str, kind: impl Into<Atom>) -> Result<Option<u32>, String> {
        let reply = self
            .conn
            .get_property(false, window, self.atom(name)?, kind, 0, 1)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        Ok(reply.value32().and_then(|mut values| values.next()))
    }
}

// ---------------------------------------------------------------------------
// Clipboard change events and targets
// ---------------------------------------------------------------------------

/// Counts CLIPBOARD ownership changes reported by XFixes. Every copy — ours
/// included — re-asserts ownership, so every copy counts once.
pub fn watch_clipboard(state: Arc<SelectionState>) -> Result<(), String> {
    let display = Display::open()?;
    let conn = &display.conn;
    conn.xfixes_query_version(5, 0).map_err(err)?.reply().map_err(err)?;
    let clipboard = display.atom("CLIPBOARD")?;
    conn.xfixes_select_selection_input(display.window, clipboard, SelectionEventMask::SET_SELECTION_OWNER)
        .map_err(err)?;
    conn.flush().map_err(err)?;

    // XFixes only reports later changes; content copied before Magpie started
    // counts as one change so the monitor captures it.
    if conn.get_selection_owner(clipboard).map_err(err)?.reply().map_err(err)?.owner != NONE {
        state.bump();
    }

    std::thread::Builder::new()
        .name("x11-clipboard-watch".into())
        .spawn(move || loop {
            match display.conn.wait_for_event() {
                Ok(Event::XfixesSelectionNotify(event)) if event.owner != NONE => state.bump(),
                Ok(_) => {}
                Err(e) => {
                    log::error!("[X11] clipboard watch connection lost: {e}");
                    return;
                }
            }
        })
        .map_err(err)?;
    Ok(())
}

/// Reads the targets (MIME types and legacy X names) the CLIPBOARD owner offers.
pub struct TargetsReader {
    /// Locked for the whole request: concurrent readers (rapid successive
    /// copies) would take each other's SelectionNotify.
    display: Mutex<Display>,
}

impl TargetsReader {
    pub fn new() -> Result<Self, String> {
        Ok(Self { display: Mutex::new(Display::open()?) })
    }

    /// The offered targets; empty when nothing owns the clipboard. `Err` when
    /// the owner didn't answer — XWayland's bridge can briefly be unready right
    /// after a change, so the caller retries.
    pub fn targets(&self) -> Result<Vec<String>, String> {
        let d = self.display.lock().expect("targets display lock poisoned");
        let d = &*d;
        let clipboard = d.atom("CLIPBOARD")?;
        if d.conn.get_selection_owner(clipboard).map_err(err)?.reply().map_err(err)?.owner == NONE {
            return Ok(Vec::new());
        }
        let targets = d.atom("TARGETS")?;
        let property = d.atom("MAGPIE_TARGETS")?;
        d.conn.delete_property(d.window, property).map_err(err)?;
        d.conn.convert_selection(d.window, clipboard, targets, property, CURRENT_TIME).map_err(err)?;
        d.conn.flush().map_err(err)?;

        let deadline = Instant::now() + Duration::from_millis(1000);
        let converted = loop {
            match d.conn.poll_for_event().map_err(err)? {
                Some(Event::SelectionNotify(event)) if event.requestor == d.window => {
                    break event.property != NONE;
                }
                Some(_) => {}
                None if Instant::now() >= deadline => return Err("TARGETS request timed out".into()),
                None => std::thread::sleep(Duration::from_millis(2)),
            }
        };
        if !converted {
            return Err("the clipboard owner refused TARGETS".into());
        }

        let reply = d
            .conn
            .get_property(true, d.window, property, AtomEnum::ANY, 0, 4096)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        let atoms: Vec<Atom> = reply.value32().map(|values| values.collect()).unwrap_or_default();
        atoms.into_iter().filter(|a| *a != NONE).map(|a| d.atom_name(a)).collect()
    }
}

// ---------------------------------------------------------------------------
// Focus and paste-back
// ---------------------------------------------------------------------------

// Keysyms (X11/keysymdef.h).
const XK_CONTROL_L: u32 = 0xffe3;
const XK_V: u32 = 0x0076;
/// Modifiers the user may still hold from the key that asked for the paste.
const HELD_MODIFIERS: [u32; 9] = [
    0xffe1, // Shift_L
    0xffe2, // Shift_R
    0xffe9, // Alt_L
    0xffea, // Alt_R
    0xffe7, // Meta_L
    0xffe8, // Meta_R
    0xffeb, // Super_L
    0xffec, // Super_R
    0xfe03, // ISO_Level3_Shift (AltGr)
];

pub struct X11Desktop {
    display: Display,
}

/// A focused window with the facts needed to name its application.
pub struct WindowFacts {
    pub focus: FocusedWindow,
    /// WM_CLASS (instance, class).
    pub wm_class: Option<(String, String)>,
}

impl X11Desktop {
    pub fn new() -> Result<Self, String> {
        Ok(Self { display: Display::open()? })
    }

    pub fn focused_window(&self) -> Result<FocusedWindow, String> {
        let d = &self.display;
        let window = d.property_u32(d.root, "_NET_ACTIVE_WINDOW", AtomEnum::WINDOW)?.filter(|w| *w != NONE);
        let Some(window) = window else {
            return Ok(FocusedWindow::default());
        };
        let pid = d.property_u32(window, "_NET_WM_PID", AtomEnum::CARDINAL)?.filter(|p| *p != 0);
        Ok(FocusedWindow { pid, window: Some(u64::from(window)) })
    }

    pub fn focused_window_facts(&self) -> Result<WindowFacts, String> {
        let focus = self.focused_window()?;
        let wm_class = match focus.window {
            Some(window) => self.wm_class(window as Window)?,
            None => None,
        };
        Ok(WindowFacts { focus, wm_class })
    }

    fn wm_class(&self, window: Window) -> Result<Option<(String, String)>, String> {
        let d = &self.display;
        let reply = d
            .conn
            .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 1024)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        let mut parts = reply.value.split(|b| *b == 0).map(|p| String::from_utf8_lossy(p).into_owned());
        Ok(match (parts.next(), parts.next()) {
            (Some(instance), Some(class)) if !class.is_empty() => Some((instance, class)),
            _ => None,
        })
    }

    /// Ask the window manager to activate `window`, as a pager would (source
    /// indication 2), which window managers honour without focus-stealing
    /// prevention.
    pub fn activate(&self, window: u64) -> Result<(), String> {
        let d = &self.display;
        let event = ClientMessageEvent::new(32, window as Window, d.atom("_NET_ACTIVE_WINDOW")?, [2, CURRENT_TIME, 0, 0, 0]);
        d.conn
            .send_event(false, d.root, EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY, event)
            .map_err(err)?;
        d.conn.flush().map_err(err)
    }

    /// Synthesize Ctrl+V into the focused window.
    pub fn paste(&self) -> Result<(), String> {
        let d = &self.display;
        let setup = d.conn.setup();
        let (min, max) = (setup.min_keycode, setup.max_keycode);
        let mapping = d
            .conn
            .get_keyboard_mapping(min, max - min + 1)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        let per_keycode = usize::from(mapping.keysyms_per_keycode);
        let keycode_of = |keysym: u32| -> Option<u8> {
            mapping
                .keysyms
                .chunks(per_keycode)
                .position(|syms| syms.contains(&keysym))
                .map(|i| min + i as u8)
        };
        let ctrl = keycode_of(XK_CONTROL_L).ok_or("no keycode produces Control_L")?;
        let v = keycode_of(XK_V).ok_or("no keycode produces v")?;

        let pressed = d.conn.query_keymap().map_err(err)?.reply().map_err(err)?.keys;
        let is_down = |keycode: u8| pressed[usize::from(keycode / 8)] & (1 << (keycode % 8)) != 0;
        let held: Vec<u8> = HELD_MODIFIERS.iter().filter_map(|k| keycode_of(*k)).filter(|k| is_down(*k)).collect();

        let key = |kind: u8, keycode: u8| {
            d.conn.xtest_fake_input(kind, keycode, CURRENT_TIME, d.root, 0, 0, 0).map(|_| ()).map_err(err)
        };
        key(KEY_PRESS_EVENT, ctrl)?;
        // Release modifiers still held from the key that asked for the paste
        // (Shift for plain text, Alt for keep-window): Ctrl+V must arrive alone.
        for keycode in held {
            key(KEY_RELEASE_EVENT, keycode)?;
        }
        key(KEY_PRESS_EVENT, v)?;
        key(KEY_RELEASE_EVENT, v)?;
        key(KEY_RELEASE_EVENT, ctrl)?;
        d.conn.sync().map_err(err)?;
        log::debug!("[Paste] Simulated Ctrl+V via XTest");
        Ok(())
    }
}
