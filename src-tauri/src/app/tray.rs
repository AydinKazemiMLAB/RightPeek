//! Tray icon and menu: Show last, Settings, Quit.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use super::{AppState, lock, settings, viewer};

const SHOW_LAST: &str = "show_last";
const SETTINGS: &str = "settings";
const QUIT: &str = "quit";
const TRAY_ID: &str = "main";

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, SHOW_LAST, "Show last", true, None::<&str>)?,
            &MenuItem::with_id(app, SETTINGS, "Settings…", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, QUIT, "Quit", true, None::<&str>)?,
        ],
    )?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(tooltip(app))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW_LAST => viewer::show_last(app),
            SETTINGS => settings::open_window(app),
            QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                viewer::show_last(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Updates the tooltip after the hotkey changes or fails to register.
pub fn refresh_tooltip(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(tooltip(app)));
    }
}

fn tooltip(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let hotkey = lock(&state.settings).hotkey.clone();
    if lock(&state.hotkey_error).is_some() {
        format!("RightPeek: {hotkey} is unavailable. Open Settings to pick another shortcut.")
    } else {
        format!("RightPeek ({hotkey})")
    }
}
