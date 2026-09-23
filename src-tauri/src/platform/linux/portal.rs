//! xdg-desktop-portal on Wayland desktops that give clients no direct way to
//! synthesize keys or grab global shortcuts (GNOME, KDE):
//!
//! - RemoteDesktop: the synthetic Ctrl+V. The desktop asks the user once; the
//!   restore token it hands back lets later sessions start without asking.
//! - GlobalShortcuts: the toggle shortcut, owned and configurable by the
//!   desktop rather than grabbed by Magpie.
//!
//! Each portal runs on its own thread with its own async runtime, so the
//! synchronous platform ports can call into it from anywhere.

use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use ashpd::desktop::global_shortcuts::{
    BindShortcutsOptions, ConfigureShortcutsOptions, GlobalShortcuts, NewShortcut,
};
use ashpd::desktop::remote_desktop::{
    DeviceType, KeyState, NotifyKeyboardKeysymOptions, RemoteDesktop, SelectDevicesOptions, StartOptions,
};
use ashpd::desktop::{CreateSessionOptions, PersistMode, Session};
use ashpd::zbus;
use futures_util::StreamExt;
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc as async_mpsc;

use super::APP_ID;

fn err(e: impl std::fmt::Display) -> String {
    format!("Portal: {e}")
}

/// A portal connection that identifies itself as Magpie. Host apps (not
/// sandboxed) must register before their first portal call; desktops whose
/// portal predates the registry fall back to identifying the app by its
/// launch scope, so a failed registration isn't fatal.
async fn connect() -> Result<zbus::Connection, String> {
    let conn = zbus::Connection::session().await.map_err(err)?;
    let app_id = ashpd::AppID::try_from(APP_ID).map_err(err)?;
    if let Err(e) = ashpd::register_host_app_with_connection(conn.clone(), app_id).await {
        log::info!("[Portal] host app registry unavailable ({e}); identity comes from the launch scope");
    }
    Ok(conn)
}

/// Whether `e` means the desktop has no such portal: the interface isn't
/// implemented, or no portal service is installed at all.
fn portal_missing(e: &ashpd::Error) -> bool {
    fn missing(e: &zbus::Error) -> bool {
        const MISSING: [&str; 3] = [
            "org.freedesktop.DBus.Error.ServiceUnknown",
            "org.freedesktop.DBus.Error.UnknownInterface",
            "org.freedesktop.DBus.Error.UnknownMethod",
        ];
        match e {
            zbus::Error::FDO(fdo) => matches!(
                **fdo,
                zbus::fdo::Error::ServiceUnknown(_)
                    | zbus::fdo::Error::UnknownInterface(_)
                    | zbus::fdo::Error::UnknownMethod(_)
            ),
            zbus::Error::MethodError(name, _, _) => MISSING.contains(&name.as_str()),
            _ => false,
        }
    }
    match e {
        ashpd::Error::PortalNotFound(_) => true,
        ashpd::Error::Zbus(e) | ashpd::Error::Portal(ashpd::PortalError::ZBus(e)) => missing(e),
        _ => false,
    }
}

/// Run `task` on a dedicated thread with a single-threaded runtime.
fn spawn_portal_thread<F>(name: &str, task: F) -> Result<(), String>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("portal runtime");
            runtime.block_on(task);
        })
        .map(|_| ())
        .map_err(err)
}

// ---------------------------------------------------------------------------
// RemoteDesktop: the synthetic Ctrl+V
// ---------------------------------------------------------------------------

/// Keysyms (X11/keysymdef.h).
const XK_CONTROL_L: i32 = 0xffe3;
const XK_V: i32 = 0x0076;
/// Close a restorable session after this long without a paste: desktops show a
/// "remote control" indicator while a session is open.
const IDLE_CLOSE: Duration = Duration::from_secs(30);

type PasteReply = mpsc::Sender<Result<(), String>>;

pub struct RemoteDesktopPaster {
    requests: async_mpsc::UnboundedSender<PasteReply>,
}

impl RemoteDesktopPaster {
    /// `None` when the desktop has no RemoteDesktop portal with keyboard
    /// support.
    pub fn start(app: &AppHandle) -> Result<Option<Self>, String> {
        let token_path = app.path().app_data_dir().map_err(err)?.join("remote-desktop-token");
        let (requests, mut receiver) = async_mpsc::unbounded_channel::<PasteReply>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<bool, String>>();
        let loc = crate::i18n::read_locale(app);

        spawn_portal_thread("portal-remote-desktop", async move {
            let portal = match open_remote_desktop().await {
                Ok(Some(portal)) => {
                    let _ = ready_tx.send(Ok(true));
                    portal
                }
                Ok(None) => {
                    let _ = ready_tx.send(Ok(false));
                    return;
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            // Sessions that can be restored without asking are closed when
            // idle; older portals would ask again, so theirs stays open.
            let restorable = portal.version() >= 2;
            let mut session: Option<Session<RemoteDesktop>> = None;
            let mut denied = false;
            loop {
                let request = if restorable && session.is_some() {
                    match tokio::time::timeout(IDLE_CLOSE, receiver.recv()).await {
                        Ok(request) => request,
                        Err(_) => {
                            if let Some(open) = session.take() {
                                let _ = open.close().await;
                            }
                            continue;
                        }
                    }
                } else {
                    receiver.recv().await
                };
                let Some(reply) = request else {
                    return;
                };

                let result = async {
                    if denied {
                        return Err(crate::i18n::tr(loc, "err.paste_denied").to_string());
                    }
                    if session.is_none() {
                        match start_session(&portal, &token_path, restorable).await {
                            Ok(started) => session = Some(started),
                            Err(PortalFailure::Denied) => {
                                denied = true;
                                return Err(crate::i18n::tr(loc, "err.paste_denied").to_string());
                            }
                            Err(PortalFailure::Other(e)) => return Err(e),
                        }
                    }
                    let open = session.as_ref().expect("session was just started");
                    let sent = press_ctrl_v(&portal, open).await;
                    if sent.is_err() {
                        // The desktop may have ended the session (the user
                        // stopped sharing); start a new one next time.
                        session = None;
                    }
                    sent
                }
                .await;
                let _ = reply.send(result);
            }
        })?;

        match ready_rx.recv().map_err(err)? {
            Ok(true) => Ok(Some(Self { requests })),
            Ok(false) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn paste(&self) -> Result<(), String> {
        let (reply, result) = mpsc::channel();
        self.requests.send(reply).map_err(err)?;
        // The first paste waits for the user to answer the permission dialog.
        result.recv_timeout(Duration::from_secs(120)).map_err(err)?
    }
}

async fn open_remote_desktop() -> Result<Option<RemoteDesktop>, String> {
    let conn = connect().await?;
    let portal = match RemoteDesktop::with_connection(conn).await {
        Ok(portal) => portal,
        Err(e) if portal_missing(&e) => return Ok(None),
        Err(e) => return Err(err(e)),
    };
    // The proxy is created lazily; a missing portal only shows on first use.
    let devices = match portal.available_device_types().await {
        Ok(devices) => devices,
        Err(e) if portal_missing(&e) => return Ok(None),
        Err(e) => return Err(err(e)),
    };
    Ok(devices.contains(DeviceType::Keyboard).then_some(portal))
}

enum PortalFailure {
    Denied,
    Other(String),
}

async fn start_session(
    portal: &RemoteDesktop,
    token_path: &PathBuf,
    restorable: bool,
) -> Result<Session<RemoteDesktop>, PortalFailure> {
    let other = |e: ashpd::Error| PortalFailure::Other(err(e));
    let session = portal.create_session(CreateSessionOptions::default()).await.map_err(other)?;

    let saved = std::fs::read_to_string(token_path).ok();
    let mut devices =
        SelectDevicesOptions::default().set_devices(ashpd::enumflags2::BitFlags::from(DeviceType::Keyboard));
    if restorable {
        devices = devices
            .set_persist_mode(PersistMode::ExplicitlyRevoked)
            .set_restore_token(saved.as_deref().map(str::trim));
    }
    portal.select_devices(&session, devices).await.map_err(other)?.response().map_err(other)?;

    let started = match portal.start(&session, None, StartOptions::default()).await.map_err(other)?.response() {
        Ok(started) => started,
        Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => return Err(PortalFailure::Denied),
        Err(e) => return Err(other(e)),
    };
    if !started.devices().contains(DeviceType::Keyboard) {
        return Err(PortalFailure::Denied);
    }
    // Tokens are single-use: every start hands out the next one.
    if let Some(token) = started.restore_token()
        && let Err(e) = std::fs::write(token_path, token)
    {
        log::warn!("[Portal] couldn't save the remote desktop restore token: {e}");
    }
    Ok(session)
}

async fn press_ctrl_v(portal: &RemoteDesktop, session: &Session<RemoteDesktop>) -> Result<(), String> {
    for (keysym, state) in [
        (XK_CONTROL_L, KeyState::Pressed),
        (XK_V, KeyState::Pressed),
        (XK_V, KeyState::Released),
        (XK_CONTROL_L, KeyState::Released),
    ] {
        portal
            .notify_keyboard_keysym(session, keysym, state, NotifyKeyboardKeysymOptions::default())
            .await
            .map_err(err)?;
    }
    log::debug!("[Paste] Simulated Ctrl+V via the RemoteDesktop portal");
    Ok(())
}

// ---------------------------------------------------------------------------
// GlobalShortcuts: the toggle shortcut
// ---------------------------------------------------------------------------

const SHORTCUT_ID: &str = "toggle";

/// What the desktop reports about the bound shortcut.
#[derive(Clone, Default)]
pub struct PortalShortcutState {
    /// Human-readable trigger, as the desktop words it ("Ctrl+Alt+V").
    pub trigger: Option<String>,
    /// Whether the desktop can open its own dialog to change it (portal v2).
    pub can_configure: bool,
}

pub struct GlobalShortcutPortal {
    state: Arc<Mutex<PortalShortcutState>>,
    configure: async_mpsc::UnboundedSender<mpsc::Sender<Result<(), String>>>,
}

impl GlobalShortcutPortal {
    /// Open a GlobalShortcuts session and bind the toggle shortcut, preferring
    /// `accelerator` (Tauri syntax) as its trigger. `None` when the desktop has
    /// no GlobalShortcuts portal. The desktop may first ask the user to approve
    /// the shortcut; the binding completes whenever they answer, without
    /// holding up startup.
    pub fn start(app: &AppHandle, accelerator: &str) -> Result<Option<Self>, String> {
        let preferred = portal_trigger(accelerator);
        let description = crate::i18n::tr(crate::i18n::read_locale(app), "shortcut.toggle_description");
        let state = Arc::new(Mutex::new(PortalShortcutState::default()));
        let (configure, mut configure_rx) = async_mpsc::unbounded_channel::<mpsc::Sender<Result<(), String>>>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<bool, String>>();
        let shared = Arc::clone(&state);
        let app = app.clone();

        spawn_portal_thread("portal-global-shortcuts", async move {
            let (portal, session) = match open_global_shortcuts().await {
                Ok(Some(opened)) => opened,
                Ok(None) => {
                    let _ = ready_tx.send(Ok(false));
                    return;
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            shared.lock().expect("shortcut state poisoned").can_configure = portal.version() >= 2;

            let (mut activated, mut changed) = match (portal.receive_activated().await, portal.receive_shortcuts_changed().await) {
                (Ok(activated), Ok(changed)) => (activated, changed),
                (Err(e), _) | (_, Err(e)) => {
                    let _ = ready_tx.send(Err(err(e)));
                    return;
                }
            };
            let _ = ready_tx.send(Ok(true));

            let bind = bind_toggle(&portal, &session, preferred.as_deref(), description);
            tokio::pin!(bind);
            let mut binding = true;
            loop {
                tokio::select! {
                    bound = &mut bind, if binding => {
                        binding = false;
                        match bound {
                            Ok(trigger) => {
                                log::info!(
                                    "[Portal] toggle shortcut bound to {}",
                                    trigger.as_deref().unwrap_or("no trigger yet")
                                );
                                shared.lock().expect("shortcut state poisoned").trigger = trigger;
                            }
                            Err(e) => log::warn!("[Portal] toggle shortcut not bound: {e}"),
                        }
                    }
                    Some(event) = activated.next() => {
                        if event.shortcut_id() != SHORTCUT_ID {
                            continue;
                        }
                        // GNOME 50+ passes an activation token that lets the
                        // window take focus even under strict focus rules.
                        let token = event
                            .options()
                            .get("activation_token")
                            .and_then(|v| String::try_from(v.clone()).ok());
                        let handle = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            if let (Some(token), Some(window)) = (token, handle.get_webview_window("main"))
                                && let Ok(gtk_window) = window.gtk_window()
                            {
                                use gtk::prelude::GtkWindowExt;
                                gtk_window.set_startup_id(&token);
                            }
                            crate::toggle_window(&handle);
                        });
                    }
                    Some(event) = changed.next() => {
                        let trigger = event
                            .shortcuts()
                            .iter()
                            .find(|s| s.id() == SHORTCUT_ID)
                            .map(|s| s.trigger_description().to_string())
                            .filter(|t| !t.is_empty());
                        shared.lock().expect("shortcut state poisoned").trigger = trigger;
                    }
                    Some(reply) = configure_rx.recv() => {
                        let result = portal
                            .configure_shortcuts(&session, None, ConfigureShortcutsOptions::default())
                            .await
                            .map_err(err);
                        let _ = reply.send(result);
                    }
                    else => return,
                }
            }
        })?;

        match ready_rx.recv().map_err(err)? {
            Ok(true) => Ok(Some(Self { state, configure })),
            Ok(false) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn state(&self) -> PortalShortcutState {
        self.state.lock().expect("shortcut state poisoned").clone()
    }

    /// Open the desktop's own dialog for changing the shortcut.
    pub fn configure(&self) -> Result<(), String> {
        let (reply, result) = mpsc::channel();
        self.configure.send(reply).map_err(err)?;
        result.recv_timeout(Duration::from_secs(10)).map_err(err)?
    }
}

async fn open_global_shortcuts() -> Result<Option<(GlobalShortcuts, Session<GlobalShortcuts>)>, String> {
    let conn = connect().await?;
    let portal = match GlobalShortcuts::with_connection(conn).await {
        Ok(portal) => portal,
        Err(e) if portal_missing(&e) => return Ok(None),
        Err(e) => return Err(err(e)),
    };
    // The proxy is created lazily; a missing portal only shows on first use.
    match portal.create_session(CreateSessionOptions::default()).await {
        Ok(session) => Ok(Some((portal, session))),
        Err(e) if portal_missing(&e) => Ok(None),
        Err(e) => Err(err(e)),
    }
}

/// Bind the toggle shortcut; returns its trigger as the desktop words it.
async fn bind_toggle(
    portal: &GlobalShortcuts,
    session: &Session<GlobalShortcuts>,
    preferred: Option<&str>,
    description: &str,
) -> Result<Option<String>, String> {
    let shortcut = NewShortcut::new(SHORTCUT_ID, description).preferred_trigger(preferred);
    let bound = portal
        .bind_shortcuts(session, &[shortcut], None, BindShortcutsOptions::default())
        .await
        .map_err(err)?
        .response()
        .map_err(err)?;
    Ok(bound
        .shortcuts()
        .iter()
        .find(|s| s.id() == SHORTCUT_ID)
        .map(|s| s.trigger_description().to_string())
        .filter(|t| !t.is_empty()))
}

/// Convert a Tauri accelerator ("CmdOrCtrl+Alt+V") into the portal's
/// `preferred_trigger` syntax ("CTRL+ALT+v": XDG shortcut modifiers plus an
/// xkbcommon keysym name).
pub fn portal_trigger(accelerator: &str) -> Option<String> {
    let mut parts = Vec::new();
    for part in accelerator.split('+') {
        let mapped = match part.to_ascii_lowercase().as_str() {
            "cmdorctrl" | "commandorcontrol" | "ctrl" | "control" => "CTRL".to_string(),
            "alt" | "option" => "ALT".to_string(),
            "shift" => "SHIFT".to_string(),
            "super" | "cmd" | "command" | "meta" => "LOGO".to_string(),
            key => keysym_name(key)?,
        };
        parts.push(mapped);
    }
    Some(parts.join("+"))
}

fn keysym_name(key: &str) -> Option<String> {
    let name = match key {
        k if k.len() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()) => k.to_string(),
        k if k.starts_with('f') && k[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)) => k.to_uppercase(),
        "space" => "space".into(),
        "enter" | "return" => "Return".into(),
        "tab" => "Tab".into(),
        "backspace" => "BackSpace".into(),
        "delete" => "Delete".into(),
        "escape" | "esc" => "Escape".into(),
        "up" | "arrowup" => "Up".into(),
        "down" | "arrowdown" => "Down".into(),
        "left" | "arrowleft" => "Left".into(),
        "right" | "arrowright" => "Right".into(),
        "-" | "minus" => "minus".into(),
        "=" | "equal" => "equal".into(),
        "[" | "bracketleft" => "bracketleft".into(),
        "]" | "bracketright" => "bracketright".into(),
        "\\" | "backslash" => "backslash".into(),
        ";" | "semicolon" => "semicolon".into(),
        "'" | "quote" => "apostrophe".into(),
        "," | "comma" => "comma".into(),
        "." | "period" => "period".into(),
        "/" | "slash" => "slash".into(),
        "`" | "backquote" => "grave".into(),
        _ => return None,
    };
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_tauri_accelerators_to_portal_triggers() {
        assert_eq!(portal_trigger("Ctrl+Alt+V").as_deref(), Some("CTRL+ALT+v"));
        assert_eq!(portal_trigger("CmdOrCtrl+Shift+V").as_deref(), Some("CTRL+SHIFT+v"));
        assert_eq!(portal_trigger("Super+Space").as_deref(), Some("LOGO+space"));
        assert_eq!(portal_trigger("Alt+F12").as_deref(), Some("ALT+F12"));
        assert_eq!(portal_trigger("Ctrl+.").as_deref(), Some("CTRL+period"));
        assert_eq!(portal_trigger("Ctrl+1").as_deref(), Some("CTRL+1"));
        assert_eq!(portal_trigger("Ctrl+Up").as_deref(), Some("CTRL+Up"));
        assert_eq!(portal_trigger("Ctrl+Nope"), None, "unknown keys are rejected, not guessed");
    }
}
