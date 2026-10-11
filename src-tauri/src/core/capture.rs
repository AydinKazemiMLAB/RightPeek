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

impl CaptureError {
    /// Short heading for the viewer notice.
    pub fn title(&self) -> &'static str {
        match self {
            Self::NothingSelected => "No text selected",
            Self::Timeout => "The app did not respond",
            Self::AccessDenied => "Administrator window",
            Self::Unsupported => "Not supported",
            Self::Platform(_) => "Capture failed",
        }
    }

    /// What the user can do next; shown under the title.
    pub fn hint(&self) -> String {
        match self {
            Self::NothingSelected => {
                "Select some text in any app, then press the shortcut again.".into()
            }
            Self::Timeout => "It may be busy. Wait a moment and try again.".into(),
            Self::AccessDenied => "Windows does not let RightPeek read apps running as \
                administrator. Copy the text into a normal app such as Notepad and \
                capture it there."
                .into(),
            Self::Unsupported => "Text capture is not available on this platform yet.".into(),
            Self::Platform(detail) => format!("Try again. Details: {detail}"),
        }
    }
}

/// Joins the text of each selected range with `\n`, skipping empty ranges.
/// Returns `None` when nothing visible is left (whitespace and zero-width
/// characters only), so callers can report [`CaptureError::NothingSelected`].
pub fn selection_text<I>(parts: I) -> Option<String>
where
    I: IntoIterator<Item = String>,
{
    let joined = parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let visible = joined
        .chars()
        .any(|c| !c.is_whitespace() && !is_zero_width(c));
    visible.then_some(joined)
}

fn is_zero_width(c: char) -> bool {
    matches!(
        c,
        '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'
    )
}

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
    fn every_error_has_a_short_title_and_a_next_step() {
        let cases = [
            (CaptureError::NothingSelected, "No text selected"),
            (CaptureError::Timeout, "The app did not respond"),
            (CaptureError::AccessDenied, "Administrator window"),
            (CaptureError::Unsupported, "Not supported"),
            (CaptureError::Platform("x".into()), "Capture failed"),
        ];
        for (error, title) in cases {
            assert_eq!(error.title(), title);
            assert!(!error.hint().is_empty(), "{error:?} has no hint");
        }
    }

    #[test]
    fn nothing_selected_hint_says_how_to_retry() {
        assert_eq!(
            CaptureError::NothingSelected.hint(),
            "Select some text in any app, then press the shortcut again."
        );
    }

    #[test]
    fn access_denied_hint_explains_the_windows_limit() {
        let hint = CaptureError::AccessDenied.hint();
        assert!(hint.contains("administrator"), "{hint}");
        assert!(hint.contains("Notepad"), "{hint}");
    }

    #[test]
    fn platform_hint_keeps_the_os_detail() {
        let hint = CaptureError::Platform("COM init failed".into()).hint();
        assert!(hint.contains("COM init failed"), "{hint}");
    }

    #[test]
    fn capture_error_is_a_std_error() {
        fn assert_error<E: std::error::Error>(_: &E) {}
        assert_error(&CaptureError::Unsupported);
    }

    #[test]
    fn selection_text_joins_ranges_with_newline() {
        let parts = ["سلام".to_string(), "دنیا".to_string()];
        assert_eq!(selection_text(parts).as_deref(), Some("سلام\nدنیا"));
    }

    #[test]
    fn selection_text_skips_empty_ranges() {
        let parts = [String::new(), "متن".to_string(), String::new()];
        assert_eq!(selection_text(parts).as_deref(), Some("متن"));
    }

    #[test]
    fn selection_text_is_none_for_whitespace_only() {
        let parts = [" \r\n\t".to_string(), "\u{200C}".to_string()];
        assert_eq!(selection_text(parts), None);
    }

    #[test]
    fn selection_text_is_none_without_ranges() {
        assert_eq!(selection_text(Vec::<String>::new()), None);
    }

    #[test]
    fn selection_text_keeps_inner_whitespace_and_zwnj() {
        let parts = ["  می\u{200C}خواهم  ".to_string()];
        assert_eq!(
            selection_text(parts).as_deref(),
            Some("  می\u{200C}خواهم  ")
        );
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
