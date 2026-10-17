//! Tray icon and menu: Show last, Settings, Quit.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use super::{AppState, lock, settings, viewer};
use crate::core::settings::hotkey_label;

const SHOW_LAST: &str = "show_last";
const SETTINGS: &str = "settings";
const QUIT: &str = "quit";
const TRAY_ID: &str = "main";
/// Menu-bar template image (black + alpha, 36x36 px), drawn by
/// scripts/macos-tray-icon.swift.
#[cfg(target_os = "macos")]
const MAC_ICON: &[u8] = include_bytes!("../../icons/tray-macos.rgba");

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
        // macOS menu-bar items open their menu on click; elsewhere a left
        // click shows the last capture.
        .show_menu_on_left_click(cfg!(target_os = "macos"))
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
    #[cfg(target_os = "macos")]
    {
        builder = builder
            .icon(tauri::image::Image::new(MAC_ICON, 36, 36))
            .icon_as_template(true);
    }
    #[cfg(not(target_os = "macos"))]
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
    let hotkey = hotkey_label(&lock(&state.settings).hotkey, cfg!(target_os = "macos"));
    if lock(&state.hotkey_error).is_some() {
        format!("RightPeek: {hotkey} is unavailable. Open Settings to pick another shortcut.")
    } else {
        format!("RightPeek ({hotkey})")
    }
}
