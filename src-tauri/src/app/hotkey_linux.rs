//! The Linux hotkey backend (`app::hotkey::backend`). Exactly one mechanism
//! holds the key, chosen on the first registration:
//!
//! - Wayland with the GlobalShortcuts portal: the desktop owns the key.
//! - Wayland on GNOME without it: a GNOME custom shortcut running
//!   `rightpeek --show` (nothing runs in RightPeek until it is pressed).
//! - Otherwise an X11 key grab (on Wayland through Xwayland, so only while
//!   an X11 window has focus).
//!
//! On Wayland the X server is not touched unless both others fail: an open
//! X connection would keep Xwayland running.

use std::sync::Mutex;

use tauri::{AppHandle, Manager};

use super::hotkey::capture_and_show;
use super::settings::show_command;
use super::{AppState, lock, logging};
use crate::core::hotkey::{X11Hotkey, gnome_binding, portal_trigger};
use crate::platform::gnome_shortcut;
use crate::platform::portal_shortcut::PortalShortcut;
use crate::platform::x11_hotkey::X11HotkeyListener;

enum Backend {
    /// With the trigger the desktop reported, once bound.
    Portal(PortalShortcut, Option<String>),
    Gnome,
    X11(X11HotkeyListener),
}

static BACKEND: Mutex<Option<Backend>> = Mutex::new(None);

pub fn register(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let mut slot = lock(&BACKEND);
    let backend = match slot.take() {
        Some(backend) => backend,
        None => choose(app)?,
    };
    apply(app, slot.insert(backend), hotkey)
}

/// The previous key is replaced by the next `register` (portal: a new
/// session; GNOME: the same entry), so only the X11 grab is released here.
pub fn unregister(_app: &AppHandle, _hotkey: &str) {
    if let Some(Backend::X11(listener)) = lock(&BACKEND).as_ref() {
        listener.ungrab();
    }
}

/// A line for Settings about who holds the shortcut, where that is not
/// RightPeek itself.
pub fn note() -> Option<String> {
    match lock(&BACKEND).as_ref()? {
        Backend::Portal(_, Some(trigger)) => Some(format!(
            "Your desktop manages this shortcut (now: {trigger}); change it in the desktop's settings if it differs."
        )),
        Backend::Portal(_, None) => {
            Some("Your desktop manages this shortcut and may ask you to confirm it.".to_owned())
        }
        Backend::Gnome => Some(
            "RightPeek keeps this shortcut in GNOME Settings › Keyboard › Custom Shortcuts."
                .to_owned(),
        ),
        Backend::X11(_) => None,
    }
}

/// Whether the shortcut only works while an X11 window has focus, so the
/// user needs a desktop shortcut running `rightpeek --show`.
pub fn needs_desktop_shortcut() -> bool {
    on_wayland() && matches!(lock(&BACKEND).as_ref(), Some(Backend::X11(_)) | None)
}

fn on_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

fn choose(app: &AppHandle) -> Result<Backend, String> {
    if on_wayland() {
        let capture = app.clone();
        match PortalShortcut::start(move || capture_and_show(&capture)) {
            Ok(portal) => {
                logging::info("hotkey through the GlobalShortcuts portal");
                return Ok(Backend::Portal(portal, None));
            }
            Err(error) => logging::info(&format!("no GlobalShortcuts portal: {error}")),
        }
        if gnome_shortcut::available() {
            logging::info("hotkey as a GNOME custom shortcut");
            return Ok(Backend::Gnome);
        }
    }
    start_x11(app)
}

fn start_x11(app: &AppHandle) -> Result<Backend, String> {
    let capture = app.clone();
    X11HotkeyListener::start(move || capture_and_show(&capture)).map(Backend::X11)
}

fn apply(app: &AppHandle, backend: &mut Backend, hotkey: &str) -> Result<(), String> {
    match backend {
        Backend::Portal(portal, bound) => {
            *bound = None;
            let trigger = portal_trigger(hotkey).map_err(|error| error.to_string())?;
            let (app, hotkey) = (app.clone(), hotkey.to_owned());
            portal.bind(trigger, move |result| portal_bound(&app, &hotkey, result))
        }
        Backend::Gnome => {
            let binding = gnome_binding(hotkey).map_err(|error| error.to_string())?;
            let command = show_command().ok_or("cannot find RightPeek's own path")?;
            gnome_shortcut::set(&binding, &command)
        }
        Backend::X11(listener) => {
            listener.grab(X11Hotkey::parse(hotkey).map_err(|error| error.to_string())?)
        }
    }
}

/// Result of a portal bind (on the bind thread). On failure the hotkey
/// moves to the GNOME shortcut or the X11 grab, so it still works somewhere.
fn portal_bound(app: &AppHandle, hotkey: &str, result: Result<String, String>) {
    let mut backend = lock(&BACKEND);
    match result {
        Ok(trigger) => {
            logging::info(&format!("hotkey bound by the desktop as {trigger}"));
            if let Some(Backend::Portal(_, bound)) = backend.as_mut() {
                *bound = Some(trigger);
            }
        }
        Err(error) => {
            logging::warn(&format!("the desktop did not bind the hotkey: {error}"));
            let fallback = if gnome_shortcut::available() {
                Ok(Backend::Gnome)
            } else {
                start_x11(app)
            };
            let applied = fallback.and_then(|mut next| {
                apply(app, &mut next, hotkey)?;
                *backend = Some(next);
                Ok(())
            });
            if let Err(error) = applied {
                logging::warn(&format!("hotkey fallback failed: {error}"));
                *lock(&app.state::<AppState>().hotkey_error) = Some(error);
            }
        }
    }
}
