//! Settings persistence (store plugin), the settings window and its commands.

use serde::Serialize;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_store::StoreExt as _;

use super::{AppState, hotkey, lock, logging, tray};
use crate::core::settings::{Settings, normalize_hotkey};

pub const LABEL: &str = "settings";
const STORE_FILE: &str = "settings.json";
const STORE_KEY: &str = "settings";

/// Loads stored settings into state; invalid or missing values fall back to
/// defaults.
pub fn load(app: &AppHandle) {
    let mut settings: Settings = app
        .store(STORE_FILE)
        .ok()
        .and_then(|store| store.get(STORE_KEY))
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    settings.hotkey =
        normalize_hotkey(&settings.hotkey).unwrap_or_else(|_| Settings::default().hotkey);
    // The registry is the source of truth for autostart.
    if let Ok(enabled) = app.autolaunch().is_enabled() {
        settings.autostart = enabled;
    }
    *lock(&app.state::<AppState>().settings) = settings;
}

pub fn save(app: &AppHandle) {
    let settings = lock(&app.state::<AppState>().settings).clone();
    let saved = app
        .store(STORE_FILE)
        .map_err(|e| e.to_string())
        .and_then(|store| {
            let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
            store.set(STORE_KEY, value);
            store.save().map_err(|e| e.to_string())
        });
    if let Err(error) = saved {
        logging::error(&format!("cannot save settings: {error}"));
    }
}

pub fn open_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("settings.html".into()))
        .title("RightPeek Settings")
        .inner_size(460.0, 420.0)
        .resizable(false)
        .center()
        .focused(true)
        .build();
    if let Err(error) = built {
        logging::error(&format!("cannot open settings: {error}"));
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    settings: Settings,
    hotkey_error: Option<String>,
    log_dir: Option<String>,
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> SettingsView {
    SettingsView {
        settings: lock(&state.settings).clone(),
        hotkey_error: lock(&state.hotkey_error).clone(),
        log_dir: logging::dir().map(|dir| dir.display().to_string()),
    }
}

/// Validates and registers a new hotkey; keeps the old one on failure.
#[tauri::command]
pub fn set_hotkey(app: AppHandle, hotkey: String) -> Result<String, String> {
    let hotkey = normalize_hotkey(&hotkey).map_err(|e| e.to_string())?;
    let replaced = hotkey::replace(&app, &hotkey);
    if replaced.is_ok() {
        lock(&app.state::<AppState>().settings).hotkey = hotkey.clone();
        save(&app);
    }
    tray::refresh_tooltip(&app);
    replaced?;
    Ok(hotkey)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let launcher = app.autolaunch();
    let result = if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    };
    result.map_err(|e| e.to_string())?;
    lock(&app.state::<AppState>().settings).autostart = enabled;
    save(&app);
    Ok(enabled)
}

#[tauri::command]
pub fn set_logging(app: AppHandle, enabled: bool) -> bool {
    if !enabled {
        logging::info("logging off");
    }
    logging::set_enabled(enabled);
    if enabled {
        logging::info("logging on");
    }
    lock(&app.state::<AppState>().settings).logging = enabled;
    save(&app);
    enabled
}
