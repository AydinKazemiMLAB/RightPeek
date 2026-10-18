//! App layer: tray, hotkey, single instance, autostart, settings, viewer
//! (docs/design.md). Pure logic stays in `core`, OS calls in `platform`.

mod autostart;
pub mod hotkey;
pub mod logging;
pub mod settings;
pub mod tray;
pub mod viewer;

use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use tauri::{AppHandle, Manager, RunEvent};

use crate::core::settings::{Settings, open_settings_on_start, wants_capture};
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
    let state = app.state::<AppState>();
    let log_enabled = lock(&state.settings).logging;
    logging::init(app, log_enabled);
    logging::info(&format!("RightPeek {} started", env!("CARGO_PKG_VERSION")));
    #[cfg(target_os = "macos")]
    {
        // Menu-bar app: no Dock icon, no app menu.
        app.set_activation_policy(tauri::ActivationPolicy::Accessory)?;
        let handle = app.clone();
        let services = crate::platform::register_services(Box::new(move |text| {
            viewer::show(&handle, viewer::ViewerContent::from_service(&text), None);
        }));
        if let Err(error) = services {
            logging::error(&format!("cannot register the Services menu entry: {error}"));
        }
    }
    hotkey::register_from_settings(app);
    tray::create(app)?;
    let hotkey_failed = lock(&state.hotkey_error).is_some();
    let show = open_settings_on_start(&lock(&state.settings), hotkey_failed, std::env::args());
    if show {
        settings::open_window(app);
    }
    if wants_capture(std::env::args()) {
        hotkey::capture_and_show(app);
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
