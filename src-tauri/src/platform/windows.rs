//! Windows capture backend: UI Automation first, clipboard fallback second.
//! Implemented in Phase 1 per PLAN.md §4 and the `windows-text-capture` skill.

use std::time::Duration;

use crate::core::capture::{CaptureError, Captured, SelectionProvider};

pub struct WindowsSelectionProvider;

impl SelectionProvider for WindowsSelectionProvider {
    fn capture(&self, _timeout: Duration) -> Result<Captured, CaptureError> {
        Err(CaptureError::Unsupported)
    }
}
