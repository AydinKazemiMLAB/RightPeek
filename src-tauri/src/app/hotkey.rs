//! Global hotkey (never a low-level hook): press -> capture on a worker
//! thread -> viewer. Windows and macOS use the global-shortcut plugin;
//! Linux uses `platform::x11_hotkey`, which waits for X events instead of
//! polling.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
#[cfg(not(target_os = "linux"))]
use tauri_plugin_global_shortcut::{Shortcut, ShortcutEvent, ShortcutState};

use super::viewer::{self, ViewerContent};
use super::{AppState, lock, logging};
use crate::core::settings::hotkey_label;
use crate::platform;

/// Overall capture budget (UIA + clipboard fallback).
const CAPTURE_TIMEOUT: Duration = Duration::from_millis(1500);

/// Plugin handler for every registered shortcut (we register exactly one).
#[cfg(not(target_os = "linux"))]
pub fn handle(app: &AppHandle, _shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state == ShortcutState::Pressed {
        capture_and_show(app);
    }
}

/// Captures the selection on a worker thread and shows it in the viewer.
/// Shared by the hotkey and `--show` launches.
pub fn capture_and_show(app: &AppHandle) {
    let pressed_at = Instant::now();
    let state = app.state::<AppState>();
    if state.capturing.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("rightpeek-hotkey".into())
        .spawn(move || {
            let result = platform::selection_provider().capture(CAPTURE_TIMEOUT);
            let elapsed_ms = pressed_at.elapsed().as_millis();
            // Log the outcome only, never the captured text.
            let content = match result {
                Ok(captured) => {
                    logging::info(&format!(
                        "captured {} chars via {:?} in {elapsed_ms} ms",
                        captured.text.chars().count(),
                        captured.method
                    ));
                    ViewerContent::from_text(&captured.text)
                }
                Err(error) => {
                    logging::warn(&format!("capture failed after {elapsed_ms} ms: {error}"));
                    ViewerContent::from_error(&error)
                }
            };
            app.state::<AppState>()
                .capturing
                .store(false, Ordering::Release);
            viewer::show(&app, content, Some(pressed_at));
        });
    if let Err(error) = spawned {
        logging::error(&format!("cannot start capture thread: {error}"));
        state.capturing.store(false, Ordering::Release);
    }
}

/// Registers the hotkey from settings at startup; a failure (e.g. taken by
/// another app) is kept for the settings window instead of aborting.
pub fn register_from_settings(app: &AppHandle) {
    let state = app.state::<AppState>();
    let hotkey = lock(&state.settings).hotkey.clone();
    let error = backend::register(app, &hotkey)
        .err()
        .map(|error| format!("Cannot register {}: {error}", label(&hotkey)));
    match &error {
        Some(message) => logging::warn(message),
        None => logging::info(&format!("hotkey {hotkey} registered")),
    }
    *lock(&state.hotkey_error) = error;
}

/// Swaps the registered hotkey; restores the previous one if the new one
/// cannot be registered.
pub fn replace(app: &AppHandle, new_hotkey: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let old_hotkey = lock(&state.settings).hotkey.clone();
    backend::unregister(app, &old_hotkey);
    match backend::register(app, new_hotkey) {
        Ok(()) => {
            *lock(&state.hotkey_error) = None;
            logging::info(&format!("hotkey changed to {new_hotkey}"));
            Ok(())
        }
        Err(error) => {
            let _ = backend::register(app, &old_hotkey);
            logging::warn(&format!("cannot register {new_hotkey}: {error}"));
            Err(format!(
                "Cannot register {} (maybe another app uses it): {error}",
                label(new_hotkey)
            ))
        }
    }
}

fn label(hotkey: &str) -> String {
    hotkey_label(hotkey, cfg!(target_os = "macos"))
}

#[cfg(not(target_os = "linux"))]
mod backend {
    use tauri::AppHandle;
    use tauri_plugin_global_shortcut::GlobalShortcutExt as _;

    pub fn register(app: &AppHandle, hotkey: &str) -> Result<(), String> {
        app.global_shortcut()
            .register(hotkey)
            .map_err(|error| error.to_string())
    }

    pub fn unregister(app: &AppHandle, hotkey: &str) {
        let shortcuts = app.global_shortcut();
        if shortcuts.is_registered(hotkey) {
            let _ = shortcuts.unregister(hotkey);
        }
    }
}

#[cfg(target_os = "linux")]
mod backend {
    use std::sync::OnceLock;

    use tauri::AppHandle;

    use crate::core::hotkey::X11Hotkey;
    use crate::platform::x11_hotkey::X11HotkeyListener;

    /// Started on the first registration; one X connection for the app's life.
    static LISTENER: OnceLock<Result<X11HotkeyListener, String>> = OnceLock::new();

    pub fn register(app: &AppHandle, hotkey: &str) -> Result<(), String> {
        let hotkey = X11Hotkey::parse(hotkey).map_err(|error| error.to_string())?;
        let listener = LISTENER.get_or_init(|| {
            let app = app.clone();
            X11HotkeyListener::start(move || super::capture_and_show(&app))
        });
        listener.as_ref().map_err(Clone::clone)?.grab(hotkey)
    }

    /// Only one hotkey is ever grabbed, so the argument is not needed.
    pub fn unregister(_app: &AppHandle, _hotkey: &str) {
        if let Some(Ok(listener)) = LISTENER.get() {
            listener.ungrab();
        }
    }
}
