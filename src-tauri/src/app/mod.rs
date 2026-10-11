//! App layer: tray, hotkey, single instance, autostart, settings, viewer
//! (docs/PLAN.md §3). Pure logic stays in `core`, OS calls in `platform`.

pub mod hotkey;
pub mod settings;
pub mod tray;
pub mod viewer;

use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use tauri::{AppHandle, Manager, RunEvent};

use crate::core::settings::Settings;
use viewer::ViewerContent;

/// State shared by commands, the hotkey handler and the tray.
#[derive(Default)]
pub struct AppState {
    /// Content the viewer has not pulled yet (`take_pending_text`).
    pub pending: Mutex<Option<ViewerContent>>,
    /// Last successful capture, for "Show last".
    pub last: Mutex<Option<ViewerContent>>,
    pub settings: Mutex<Settings>,
    /// Why the configured hotkey could not be registered, if it failed.
    pub hotkey_error: Mutex<Option<String>>,
    /// When the hotkey was pressed, to time "hotkey -> text visible".
    pub hotkey_pressed_at: Mutex<Option<Instant>>,
    /// Set while a capture runs, so key repeat cannot stack captures.
    pub capturing: AtomicBool,
}

/// Locks a mutex, recovering the data if another thread panicked with it.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    settings::load(app);
    hotkey::register_from_settings(app);
    tray::create(app)?;
    // Without a working hotkey the app is unusable: let the user pick another.
    if lock(&app.state::<AppState>().hotkey_error).is_some() {
        settings::open_window(app);
    }
    Ok(())
}

/// Keeps the process alive when the last window closes (tray app); only an
/// explicit `exit` (Quit) ends it.
pub fn on_run_event(_app: &AppHandle, event: RunEvent) {
    if let RunEvent::ExitRequested { api, code, .. } = event
        && code.is_none()
    {
        api.prevent_exit();
    }
}
