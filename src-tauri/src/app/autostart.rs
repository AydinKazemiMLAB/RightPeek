//! "Start RightPeek when you sign in".
//!
//! On Windows the entry is always per user (HKCU Run). The autostart plugin
//! tries HKLM first and uses it whenever that key is writable, which some
//! machines allow; RightPeek then started for every user, and twice when an
//! HKCU entry existed too. Elsewhere the autostart plugin is used.

use tauri::AppHandle;

#[cfg(windows)]
mod imp {
    use auto_launch::{AutoLaunch, AutoLaunchBuilder, WindowsEnableMode};
    use tauri::AppHandle;

    use crate::core::settings::AUTOSTART_ARG;

    fn launcher(app: &AppHandle) -> Result<AutoLaunch, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        AutoLaunchBuilder::new()
            .set_app_name(&app.package_info().name)
            // Quoted: the per-user install path may contain spaces.
            .set_app_path(&format!("\"{}\"", exe.display()))
            .set_args(&[AUTOSTART_ARG])
            .set_windows_enable_mode(WindowsEnableMode::CurrentUser)
            .build()
            .map_err(|e| e.to_string())
    }

    pub fn is_enabled(app: &AppHandle) -> Result<bool, String> {
        launcher(app)?.is_enabled().map_err(|e| e.to_string())
    }

    pub fn set(app: &AppHandle, enabled: bool) -> Result<(), String> {
        let launcher = launcher(app)?;
        // Removes the HKLM entry too where that key is writable, so an
        // entry left by an earlier version does not start a second copy.
        launcher.disable().map_err(|e| e.to_string())?;
        if enabled {
            launcher.enable().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use tauri::AppHandle;
    use tauri_plugin_autostart::ManagerExt as _;

    pub fn is_enabled(app: &AppHandle) -> Result<bool, String> {
        app.autolaunch().is_enabled().map_err(|e| e.to_string())
    }

    pub fn set(app: &AppHandle, enabled: bool) -> Result<(), String> {
        let launcher = app.autolaunch();
        let result = if enabled {
            launcher.enable()
        } else {
            launcher.disable()
        };
        result.map_err(|e| e.to_string())
    }
}

/// Whether the sign-in entry exists and is not disabled in Task Manager.
pub fn is_enabled(app: &AppHandle) -> Result<bool, String> {
    imp::is_enabled(app)
}

pub fn set(app: &AppHandle, enabled: bool) -> Result<(), String> {
    imp::set(app, enabled)
}
