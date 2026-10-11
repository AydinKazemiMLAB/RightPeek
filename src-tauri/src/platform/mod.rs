//! OS-specific implementations of [`SelectionProvider`]. Everything that
//! touches native APIs lives here so `core` stays portable (PLAN.md §3).

use crate::core::capture::SelectionProvider;

#[cfg(windows)]
mod windows;

/// Returns the capture backend for the current OS.
pub fn selection_provider() -> Box<dyn SelectionProvider + Send + Sync> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsSelectionProvider)
    }
    #[cfg(not(windows))]
    {
        Box::new(Unsupported)
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
