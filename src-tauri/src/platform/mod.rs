//! OS-specific implementations of [`SelectionProvider`]. Everything that
//! touches native APIs lives here so `core` stays portable (docs/PLAN.md §3).

use crate::core::capture::SelectionProvider;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// Returns the capture backend for the current OS.
pub fn selection_provider() -> Box<dyn SelectionProvider + Send + Sync> {
    #[cfg(windows)]
    {
        Box::new(self::windows::WindowsSelectionProvider)
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(self::linux::LinuxSelectionProvider)
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(self::macos::MacSelectionProvider)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        Box::new(Unsupported)
    }
}

/// Whether capture has the OS permission it needs; `None` where capture
/// needs no permission (everywhere but macOS: Accessibility).
pub fn capture_permission() -> Option<bool> {
    #[cfg(target_os = "macos")]
    {
        Some(self::macos::is_trusted(false))
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Asks for the capture permission: the system prompt (adds the app to the
/// list) plus the System Settings pane where it is turned on.
pub fn request_capture_permission() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if self::macos::is_trusted(true) {
            return Ok(());
        }
        self::macos::open_accessibility_settings()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

/// Registers the Services menu entry ("Show in RightPeek"); `handler` gets
/// the selected text. macOS only, on the main thread; a no-op elsewhere.
pub fn register_services(handler: Box<dyn Fn(String) + Send + Sync>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        self::macos::register_services(handler)
    }
    #[cfg(not(target_os = "macos"))]
    {
        drop(handler);
        Ok(())
    }
}

/// Runs only the clipboard fallback (diagnostics only: capture probe).
pub fn capture_via_clipboard(
    timeout: std::time::Duration,
) -> Result<crate::core::capture::Captured, crate::core::capture::CaptureError> {
    #[cfg(windows)]
    {
        self::windows::capture_via_clipboard(timeout)
    }
    #[cfg(target_os = "macos")]
    {
        run_with_timeout(timeout, self::macos::capture_via_copy)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = timeout;
        Err(crate::core::capture::CaptureError::Unsupported)
    }
}

/// Runs only the accessibility API: UI Automation on Windows, `AXSelectedText`
/// on macOS (diagnostics only: capture probe).
pub fn capture_via_uia(
    timeout: std::time::Duration,
) -> Result<crate::core::capture::Captured, crate::core::capture::CaptureError> {
    #[cfg(windows)]
    {
        self::windows::capture_via_uia(timeout)
    }
    #[cfg(target_os = "macos")]
    {
        run_with_timeout(timeout, self::macos::capture_via_accessibility)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = timeout;
        Err(crate::core::capture::CaptureError::Unsupported)
    }
}

/// Current plain-text clipboard content, if any (diagnostics only).
pub fn clipboard_text() -> Option<String> {
    #[cfg(windows)]
    {
        self::windows::clipboard_text()
    }
    #[cfg(target_os = "macos")]
    {
        self::macos::clipboard_text()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

/// Whether the global hotkey misses keys pressed in some apps: on Wayland it
/// only fires while an X11 (XWayland) window has focus, so users need a
/// desktop shortcut running `rightpeek --show` instead.
pub fn global_hotkey_is_limited() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// Runs `job` on a fresh thread and gives up after `timeout`. A job that
/// overruns keeps running detached; its OS objects never leave its thread.
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn run_with_timeout<T, F>(
    timeout: std::time::Duration,
    job: F,
) -> Result<T, crate::core::capture::CaptureError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, crate::core::capture::CaptureError> + Send + 'static,
{
    use crate::core::capture::CaptureError;

    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("rightpeek-capture".into())
        .spawn(move || {
            // The receiver may be gone after a timeout; nothing to do then.
            let _ = sender.send(job());
        })
        .map_err(|e| CaptureError::Platform(format!("cannot start capture thread: {e}")))?;
    receiver
        .recv_timeout(timeout)
        .map_err(|_| CaptureError::Timeout)?
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
struct Unsupported;

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
impl SelectionProvider for Unsupported {
    fn capture(
        &self,
        _timeout: std::time::Duration,
    ) -> Result<crate::core::capture::Captured, crate::core::capture::CaptureError> {
        Err(crate::core::capture::CaptureError::Unsupported)
    }
}

#[cfg(all(test, any(windows, target_os = "linux", target_os = "macos")))]
mod tests {
    use std::thread;
    use std::time::Duration;

    use super::run_with_timeout;
    use crate::core::capture::CaptureError;

    #[test]
    fn timeout_returns_timeout_error() {
        let result: Result<(), _> = run_with_timeout(Duration::from_millis(10), || {
            thread::sleep(Duration::from_millis(200));
            Ok(())
        });
        assert_eq!(result, Err(CaptureError::Timeout));
    }

    #[test]
    fn finished_job_returns_its_result() {
        let result = run_with_timeout(Duration::from_secs(1), || Ok(7));
        assert_eq!(result, Ok(7));
    }
}
