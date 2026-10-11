//! Selection-capture contract shared by every platform backend.

use std::time::Duration;

/// How the selected text was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMethod {
    /// Read directly through the OS accessibility API (UI Automation on Windows).
    Uia,
    /// Obtained by simulating a copy and reading the clipboard.
    Clipboard,
}

/// Text captured from the foreground application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captured {
    pub text: String,
    pub method: CaptureMethod,
}

/// Why a capture produced no text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureError {
    /// The focused application has no selected text.
    NothingSelected,
    /// The target did not answer within the allotted time.
    Timeout,
    /// The target runs elevated; UIPI blocks a normal process from reading it.
    AccessDenied,
    /// No capture backend exists for this platform yet.
    Unsupported,
    /// Unexpected OS failure, with a diagnostic message.
    Platform(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NothingSelected => f.write_str("No text is selected in the active window."),
            Self::Timeout => f.write_str("The active window did not respond in time."),
            Self::AccessDenied => {
                f.write_str("Cannot read text from a window running as administrator.")
            }
            Self::Unsupported => f.write_str("Text capture is not supported on this platform."),
            Self::Platform(detail) => write!(f, "Text capture failed: {detail}"),
        }
    }
}

impl std::error::Error for CaptureError {}

/// A platform backend that reads the current selection of the focused app.
pub trait SelectionProvider {
    /// Captures the current selection, giving up after `timeout`.
    fn capture(&self, timeout: Duration) -> Result<Captured, CaptureError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_selected_message_tells_user_to_select_text() {
        assert_eq!(
            CaptureError::NothingSelected.to_string(),
            "No text is selected in the active window."
        );
    }

    #[test]
    fn timeout_message_names_the_unresponsive_target() {
        assert_eq!(
            CaptureError::Timeout.to_string(),
            "The active window did not respond in time."
        );
    }

    #[test]
    fn access_denied_message_mentions_administrator_windows() {
        assert_eq!(
            CaptureError::AccessDenied.to_string(),
            "Cannot read text from a window running as administrator."
        );
    }

    #[test]
    fn platform_message_includes_os_detail() {
        assert_eq!(
            CaptureError::Platform("COM init failed".into()).to_string(),
            "Text capture failed: COM init failed"
        );
    }

    #[test]
    fn capture_error_is_a_std_error() {
        fn assert_error<E: std::error::Error>(_: &E) {}
        assert_error(&CaptureError::Unsupported);
    }

    #[test]
    fn provider_is_usable_as_trait_object() {
        struct Fixed;
        impl SelectionProvider for Fixed {
            fn capture(&self, _: Duration) -> Result<Captured, CaptureError> {
                Ok(Captured {
                    text: "سلام".into(),
                    method: CaptureMethod::Uia,
                })
            }
        }

        let provider: Box<dyn SelectionProvider> = Box::new(Fixed);
        let captured = provider.capture(Duration::from_millis(10)).unwrap();
        assert_eq!(captured.text, "سلام");
        assert_eq!(captured.method, CaptureMethod::Uia);
    }
}
