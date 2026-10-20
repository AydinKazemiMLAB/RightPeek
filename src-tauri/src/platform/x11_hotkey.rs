//! Global hotkey on Linux: a passive X11 key grab on the root window (like
//! the global-shortcut plugin, never an input hook), with a thread that
//! blocks on X events. The plugin's X11 backend polls every 50 ms, which
//! cost about 1% CPU while idle (docs/budgets.md).
//!
//! On Wayland this goes through Xwayland, so it fires only while an X11 app
//! has focus (as before); users bind a desktop shortcut to `rightpeek --show`.

use std::sync::{Arc, Mutex, PoisonError};

use x11rb::connection::Connection as _;
use x11rb::protocol::Event;
use x11rb::protocol::xkb::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{ConnectionExt as _, GrabMode, Keycode, ModMask, Window};
use x11rb::rust_connection::RustConnection;

use crate::core::hotkey::{LOCK_VARIANTS, X11Hotkey};

/// The grabbed key, shared with the event thread.
type Grabbed = Arc<Mutex<Option<(Keycode, X11Hotkey)>>>;

pub struct X11HotkeyListener {
    conn: Arc<RustConnection>,
    root: Window,
    grabbed: Grabbed,
}

impl X11HotkeyListener {
    /// Connects to the X server and starts the event thread, which calls
    /// `on_press` once per press of the grabbed hotkey.
    pub fn start(on_press: impl Fn() + Send + 'static) -> Result<Self, String> {
        let (conn, screen) = RustConnection::connect(None)
            .map_err(|error| format!("cannot connect to the X server: {error}"))?;
        // With detectable auto-repeat a held key sends presses only, so a
        // held hotkey fires once.
        conn.xkb_use_extension(1, 0)
            .map_err(|error| error.to_string())?
            .reply()
            .map_err(|error| format!("no XKB extension: {error}"))?;
        conn.xkb_per_client_flags(
            xkb::ID::USE_CORE_KBD.into(),
            xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT,
            xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT,
            Default::default(),
            Default::default(),
            Default::default(),
        )
        .map_err(|error| error.to_string())?
        .reply()
        .map_err(|error| format!("cannot set detectable auto-repeat: {error}"))?;
        let root = conn.setup().roots[screen].root;
        let listener = Self {
            conn: Arc::new(conn),
            root,
            grabbed: Arc::default(),
        };
        let conn = Arc::clone(&listener.conn);
        let grabbed = Arc::clone(&listener.grabbed);
        std::thread::Builder::new()
            .name("rightpeek-x11-hotkey".into())
            .spawn(move || listen(&conn, &grabbed, on_press))
            .map_err(|error| format!("cannot start the hotkey thread: {error}"))?;
        Ok(listener)
    }

    /// Grabs `hotkey` in place of the current one, also with Caps Lock and
    /// Num Lock on. Fails if another app holds the combination.
    pub fn grab(&self, hotkey: X11Hotkey) -> Result<(), String> {
        let keycode = self.keycode(hotkey.keysym)?;
        self.ungrab();
        for lock in LOCK_VARIANTS {
            let result = self
                .conn
                .grab_key(
                    true,
                    self.root,
                    ModMask::from(hotkey.modifiers | lock),
                    keycode,
                    GrabMode::ASYNC,
                    GrabMode::ASYNC,
                )
                .map_err(|error| error.to_string())
                .and_then(|cookie| cookie.check().map_err(|error| error.to_string()));
            if let Err(error) = result {
                self.ungrab_key(keycode, hotkey);
                return Err(format!("the key is taken by another app ({error})"));
            }
        }
        *self.grabbed.lock().unwrap_or_else(PoisonError::into_inner) = Some((keycode, hotkey));
        Ok(())
    }

    /// Releases the current hotkey, if any.
    pub fn ungrab(&self) {
        let taken = self
            .grabbed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some((keycode, hotkey)) = taken {
            self.ungrab_key(keycode, hotkey);
        }
    }

    fn ungrab_key(&self, keycode: Keycode, hotkey: X11Hotkey) {
        for lock in LOCK_VARIANTS {
            let modifiers = ModMask::from(hotkey.modifiers | lock);
            let _ = self.conn.ungrab_key(keycode, self.root, modifiers);
        }
        let _ = self.conn.flush();
    }

    /// The first keycode whose keyboard-map row lists `keysym`.
    fn keycode(&self, keysym: u32) -> Result<Keycode, String> {
        let setup = self.conn.setup();
        let (min, max) = (setup.min_keycode, setup.max_keycode);
        let mapping = self
            .conn
            .get_keyboard_mapping(min, max - min + 1)
            .map_err(|error| error.to_string())?
            .reply()
            .map_err(|error| format!("cannot read the keyboard map: {error}"))?;
        let per_keycode = usize::from(mapping.keysyms_per_keycode).max(1);
        mapping
            .keysyms
            .chunks(per_keycode)
            .position(|row| row.contains(&keysym))
            .and_then(|index| u8::try_from(index).ok())
            .map(|index| min + index)
            .ok_or_else(|| "the key is not on this keyboard layout".to_owned())
    }
}

/// Event thread: blocks until the X server sends something; never polls.
fn listen(conn: &RustConnection, grabbed: &Grabbed, on_press: impl Fn()) {
    let mut pressed = false;
    loop {
        let event = match conn.wait_for_event() {
            Ok(event) => event,
            // The connection is gone (X server exited): nothing left to wait for.
            Err(_) => return,
        };
        let current = *grabbed.lock().unwrap_or_else(PoisonError::into_inner);
        let Some((keycode, hotkey)) = current else {
            continue;
        };
        match event {
            Event::KeyPress(event)
                if event.detail == keycode && hotkey.matches_state(u16::from(event.state)) =>
            {
                if !pressed {
                    pressed = true;
                    on_press();
                }
            }
            Event::KeyRelease(event) if event.detail == keycode => pressed = false,
            _ => {}
        }
    }
}
