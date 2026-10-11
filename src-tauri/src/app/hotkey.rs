//! Global hotkey (global-shortcut plugin, RegisterHotKey-based; never a
//! low-level hook): press -> capture on a worker thread -> viewer.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{
    GlobalShortcutExt as _, Shortcut, ShortcutEvent, ShortcutState,
};

use super::viewer::{self, ViewerContent};
use super::{AppState, lock};
use crate::platform;

/// Overall capture budget (UIA + clipboard fallback).
const CAPTURE_TIMEOUT: Duration = Duration::from_millis(1500);

/// Plugin handler for every registered shortcut (we register exactly one).
pub fn handle(app: &AppHandle, _shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != ShortcutState::Pressed {
        return;
    }
    let pressed_at = Instant::now();
    let state = app.state::<AppState>();
    if state.capturing.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("rightpeek-hotkey".into())
        .spawn(move || {
            let content = match platform::selection_provider().capture(CAPTURE_TIMEOUT) {
                Ok(captured) => ViewerContent::from_text(&captured.text),
                Err(error) => ViewerContent::notice(error.to_string()),
            };
            app.state::<AppState>()
                .capturing
                .store(false, Ordering::Release);
            viewer::show(&app, content, Some(pressed_at));
        });
    if spawned.is_err() {
        state.capturing.store(false, Ordering::Release);
    }
}

/// Registers the hotkey from settings at startup; a failure (e.g. taken by
/// another app) is kept for the settings window instead of aborting.
pub fn register_from_settings(app: &AppHandle) {
    let state = app.state::<AppState>();
    let hotkey = lock(&state.settings).hotkey.clone();
    let error = app
        .global_shortcut()
        .register(hotkey.as_str())
        .err()
        .map(|error| format!("Cannot register {hotkey}: {error}"));
    if let Some(message) = &error {
        eprintln!("RightPeek: {message}");
    }
    *lock(&state.hotkey_error) = error;
}

/// Swaps the registered hotkey; restores the previous one if the new one
/// cannot be registered.
pub fn replace(app: &AppHandle, new_hotkey: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let old_hotkey = lock(&state.settings).hotkey.clone();
    let shortcuts = app.global_shortcut();
    if shortcuts.is_registered(old_hotkey.as_str()) {
        let _ = shortcuts.unregister(old_hotkey.as_str());
    }
    match shortcuts.register(new_hotkey) {
        Ok(()) => {
            *lock(&state.hotkey_error) = None;
            Ok(())
        }
        Err(error) => {
            let _ = shortcuts.register(old_hotkey.as_str());
            Err(format!(
                "Cannot register {new_hotkey} (maybe another app uses it): {error}"
            ))
        }
    }
}
