//! The hotkey through the XDG GlobalShortcuts portal (Wayland: KDE Plasma
//! 5.27+, GNOME 48+, ...). The desktop owns the key and may ask the user to
//! confirm it; RightPeek only receives `Activated` signals, on a thread that
//! blocks on D-Bus (no polling). zbus is already in the build through the
//! single-instance plugin.

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
/// The one shortcut RightPeek binds.
const SHORTCUT_ID: &str = "show";
/// How long startup waits to learn whether the portal exists.
const PROBE_TIMEOUT: Duration = Duration::from_millis(1500);

pub struct PortalShortcut {
    conn: Connection,
    /// The current portal session, closed before binding again.
    session: Arc<Mutex<Option<OwnedObjectPath>>>,
}

impl PortalShortcut {
    /// Connects and checks that the portal offers GlobalShortcuts, then
    /// starts the thread that calls `on_activate` for every press. Gives up
    /// after a short timeout so a slow session bus cannot stall startup.
    pub fn start(on_activate: impl Fn() + Send + 'static) -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("rightpeek-portal-probe".into())
            .spawn(move || {
                let _ = sender.send(probe());
            })
            .map_err(|error| error.to_string())?;
        let conn = receiver
            .recv_timeout(PROBE_TIMEOUT)
            .map_err(|_| "the desktop portal did not answer".to_owned())??;
        let listener = conn.clone();
        std::thread::Builder::new()
            .name("rightpeek-portal".into())
            .spawn(move || listen(&listener, on_activate))
            .map_err(|error| format!("cannot start the portal thread: {error}"))?;
        Ok(Self {
            conn,
            session: Arc::default(),
        })
    }

    /// Asks the desktop to bind `trigger` (XDG form, e.g. `CTRL+SHIFT+space`)
    /// on a worker thread: the desktop may show a dialog and wait for the
    /// user. `on_result` gets the trigger the desktop reports, or an error.
    pub fn bind(
        &self,
        trigger: String,
        on_result: impl FnOnce(Result<String, String>) + Send + 'static,
    ) -> Result<(), String> {
        let conn = self.conn.clone();
        let session = Arc::clone(&self.session);
        std::thread::Builder::new()
            .name("rightpeek-portal-bind".into())
            .spawn(move || on_result(bind(&conn, &session, &trigger)))
            .map(drop)
            .map_err(|error| format!("cannot start the portal thread: {error}"))
    }
}

/// Session bus connection, if the portal has the GlobalShortcuts interface.
fn probe() -> Result<Connection, String> {
    let conn = Connection::session().map_err(|error| format!("no session bus: {error}"))?;
    let proxy = portal(&conn)?;
    proxy
        .get_property::<u32>("version")
        .map_err(|_| "the desktop has no GlobalShortcuts portal".to_owned())?;
    Ok(conn)
}

fn portal(conn: &Connection) -> Result<Proxy<'static>, String> {
    Proxy::new(conn, DESTINATION, PATH, INTERFACE).map_err(|error| error.to_string())
}

/// Calls `Activated` handlers for our shortcut until the bus goes away.
fn listen(conn: &Connection, on_activate: impl Fn()) {
    let Ok(proxy) = portal(conn) else { return };
    let Ok(signals) = proxy.receive_signal("Activated") else {
        return;
    };
    for message in signals {
        let body = message.body();
        let Ok((_session, id, _timestamp, _options)) =
            body.deserialize::<(OwnedObjectPath, String, u64, HashMap<String, OwnedValue>)>()
        else {
            continue;
        };
        if id == SHORTCUT_ID {
            on_activate();
        }
    }
}

/// Closes the previous session (a session binds once), opens a new one and
/// binds the shortcut. Returns the trigger the desktop chose.
fn bind(
    conn: &Connection,
    session: &Mutex<Option<OwnedObjectPath>>,
    trigger: &str,
) -> Result<String, String> {
    let previous = session
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    if let Some(path) = previous
        && let Ok(old) = Proxy::new(conn, DESTINATION, path, "org.freedesktop.portal.Session")
    {
        let _ = old.call::<_, _, ()>("Close", &());
    }

    let proxy = portal(conn)?;
    let results = request(conn, "rightpeek_session", |token| {
        let options = HashMap::from([
            ("handle_token", Value::from(token)),
            ("session_handle_token", Value::from("rightpeek")),
        ]);
        proxy.call::<_, _, OwnedObjectPath>("CreateSession", &(options,))
    })?;
    let handle = results
        .get("session_handle")
        .and_then(session_path)
        .ok_or("the portal returned no session")?;
    *session.lock().unwrap_or_else(PoisonError::into_inner) = Some(handle.clone());

    let results = request(conn, "rightpeek_bind", |token| {
        let shortcut = HashMap::from([
            (
                "description",
                Value::from("Show the selected text in RightPeek"),
            ),
            ("preferred_trigger", Value::from(trigger)),
        ]);
        let options = HashMap::from([("handle_token", Value::from(token))]);
        proxy.call::<_, _, OwnedObjectPath>(
            "BindShortcuts",
            &(&handle, vec![(SHORTCUT_ID, shortcut)], "", options),
        )
    })?;
    Ok(bound_trigger(&results).unwrap_or_else(|| trigger.to_owned()))
}

/// A session handle; the spec says string, some portals send an object path.
fn session_path(value: &OwnedValue) -> Option<OwnedObjectPath> {
    match &**value {
        Value::Str(path) => OwnedObjectPath::try_from(path.as_str()).ok(),
        Value::ObjectPath(path) => Some(path.to_owned().into()),
        _ => None,
    }
}

/// The `trigger_description` of our shortcut in a BindShortcuts response.
fn bound_trigger(results: &HashMap<String, OwnedValue>) -> Option<String> {
    type Shortcuts = Vec<(String, HashMap<String, OwnedValue>)>;
    let shortcuts = Shortcuts::try_from(results.get("shortcuts")?.try_clone().ok()?).ok()?;
    let (_, properties) = shortcuts.into_iter().find(|(id, _)| id == SHORTCUT_ID)?;
    String::try_from(properties.get("trigger_description")?.try_clone().ok()?).ok()
}

/// Makes a portal request and waits for its `Response`. Subscribes before
/// calling, on the path the portal will use, so the reply cannot be missed.
fn request(
    conn: &Connection,
    token: &str,
    call: impl FnOnce(&str) -> zbus::Result<OwnedObjectPath>,
) -> Result<HashMap<String, OwnedValue>, String> {
    let sender = conn
        .unique_name()
        .ok_or("no bus name")?
        .trim_start_matches(':')
        .replace('.', "_");
    let path = format!("{PATH}/request/{sender}/{token}");
    let request = Proxy::new(conn, DESTINATION, path, "org.freedesktop.portal.Request")
        .map_err(|error| error.to_string())?;
    let mut responses = request
        .receive_signal("Response")
        .map_err(|error| error.to_string())?;
    call(token).map_err(|error| error.to_string())?;
    let message = responses.next().ok_or("the portal closed the request")?;
    let (code, results) = message
        .body()
        .deserialize::<(u32, HashMap<String, OwnedValue>)>()
        .map_err(|error| error.to_string())?;
    match code {
        0 => Ok(results),
        1 => Err("the shortcut was not confirmed".to_owned()),
        _ => Err("the desktop refused the shortcut".to_owned()),
    }
}
