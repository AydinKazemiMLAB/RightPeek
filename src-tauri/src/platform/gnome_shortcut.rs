//! The hotkey on GNOME (Wayland) without the GlobalShortcuts portal: a
//! custom keyboard shortcut in GNOME's settings that runs `rightpeek --show`.
//! GNOME handles the key itself, so nothing runs in RightPeek until it is
//! pressed. Written with `gsettings`, which every GNOME install has.

use std::process::Command;

use crate::core::hotkey::{GNOME_SHORTCUT_PATH, gvariant_string, is_gnome, with_path_added};

const LIST_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const LIST_KEY: &str = "custom-keybindings";
const ENTRY_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";

/// Whether this is a GNOME session whose custom shortcuts can be written.
pub fn available() -> bool {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    is_gnome(&desktop) && gsettings(&["get", LIST_SCHEMA, LIST_KEY]).is_ok()
}

/// Creates or updates RightPeek's entry: `binding` (GNOME form, e.g.
/// `<Control><Shift>space`) runs `command`. Other shortcuts are untouched.
pub fn set(binding: &str, command: &str) -> Result<(), String> {
    let entry = format!("{ENTRY_SCHEMA}:{GNOME_SHORTCUT_PATH}");
    gsettings(&["set", &entry, "name", &gvariant_string("RightPeek")])?;
    gsettings(&["set", &entry, "command", &gvariant_string(command)])?;
    gsettings(&["set", &entry, "binding", &gvariant_string(binding)])?;
    // Listed last, so GNOME never sees a half-written entry.
    let list = gsettings(&["get", LIST_SCHEMA, LIST_KEY])?;
    if let Some(updated) = with_path_added(&list, GNOME_SHORTCUT_PATH) {
        gsettings(&["set", LIST_SCHEMA, LIST_KEY, &updated])?;
    }
    Ok(())
}

/// Runs `gsettings` and returns its standard output.
fn gsettings(args: &[&str]) -> Result<String, String> {
    let output = Command::new("gsettings")
        .args(args)
        .output()
        .map_err(|error| format!("cannot run gsettings: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(format!(
            "gsettings failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
