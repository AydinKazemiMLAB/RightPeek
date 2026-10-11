//! OS-specific implementations of [`SelectionProvider`]. Everything that
//! touches native APIs lives here so `core` stays portable (docs/PLAN.md §3).

use crate::core::capture::SelectionProvider;

#[cfg(windows)]
mod windows;

/// Returns the capture backend for the current OS.
pub fn selection_provider() -> Box<dyn SelectionProvider + Send + Sync> {
    #[cfg(windows)]
    {
        Box::new(self::windows::WindowsSelectionProvider)
    }
    #[cfg(not(windows))]
    {
        Box::new(Unsupported)
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
    #[cfg(not(windows))]
    {
        let _ = timeout;
        Err(crate::core::capture::CaptureError::Unsupported)
    }
}

/// Runs only UI Automation (diagnostics only: capture probe).
pub fn capture_via_uia(
    timeout: std::time::Duration,
) -> Result<crate::core::capture::Captured, crate::core::capture::CaptureError> {
    #[cfg(windows)]
    {
        self::windows::capture_via_uia(timeout)
    }
    #[cfg(not(windows))]
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
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(not(windows))]
struct Unsupported;

#[cfg(not(windows))]
impl SelectionProvider for Unsupported {
    fn capture(
        &self,
        _timeout: std::time::Duration,
    ) -> Result<crate::core::capture::Captured, crate::core::capture::CaptureError> {
        Err(crate::core::capture::CaptureError::Unsupported)
    }
}
