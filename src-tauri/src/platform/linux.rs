//! Linux selection capture. The PRIMARY selection always holds the currently
//! selected text, so no copy is simulated and the clipboard is never touched.
//!
//! Order: `arboard` (X11, and Wayland compositors with the data-control
//! protocol) -> on Wayland, `wl-paste --primary` (compositors without
//! data-control, e.g. GNOME, still serve the primary selection to it).

use std::io::ErrorKind;
use std::process::{Command, Stdio};
use std::time::Duration;

use arboard::{Clipboard, GetExtLinux, LinuxClipboardKind};

use super::run_with_timeout;
use crate::core::capture::{
    CaptureError, CaptureMethod, Captured, SelectionProvider, selection_text,
};

pub struct LinuxSelectionProvider;

impl SelectionProvider for LinuxSelectionProvider {
    fn capture(&self, timeout: Duration) -> Result<Captured, CaptureError> {
        run_with_timeout(timeout, read_primary)
    }
}

fn read_primary() -> Result<Captured, CaptureError> {
    let first = arboard_primary();
    let result = match first {
        Ok(text) => Ok(text),
        Err(error) if is_wayland() => wl_paste_primary().map_err(|fallback| {
            // Report the more useful of the two failures.
            if fallback == CaptureError::NothingSelected {
                fallback
            } else {
                error
            }
        }),
        Err(error) => Err(error),
    };
    let text = selection_text([result?]).ok_or(CaptureError::NothingSelected)?;
    Ok(Captured {
        text,
        method: CaptureMethod::PrimarySelection,
    })
}

fn arboard_primary() -> Result<String, CaptureError> {
    let mut clipboard = Clipboard::new().map_err(from_arboard)?;
    clipboard
        .get()
        .clipboard(LinuxClipboardKind::Primary)
        .text()
        .map_err(from_arboard)
}

fn from_arboard(error: arboard::Error) -> CaptureError {
    match error {
        arboard::Error::ContentNotAvailable => CaptureError::NothingSelected,
        other => CaptureError::Platform(other.to_string()),
    }
}

fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

fn wl_paste_primary() -> Result<String, CaptureError> {
    let output = Command::new("wl-paste")
        .args(["--primary", "--no-newline", "--type", "text"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => CaptureError::Platform(
                "wl-paste is missing; install the wl-clipboard package".into(),
            ),
            _ => CaptureError::Platform(format!("cannot run wl-paste: {error}")),
        })?;
    wl_paste_result(output.status.success(), output.stdout)
}

/// `wl-paste` exits non-zero when there is no selection (or no text in it).
fn wl_paste_result(success: bool, stdout: Vec<u8>) -> Result<String, CaptureError> {
    if !success {
        return Err(CaptureError::NothingSelected);
    }
    String::from_utf8(stdout)
        .map_err(|_| CaptureError::Platform("the selection is not valid UTF-8".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_content_means_nothing_selected() {
        assert_eq!(
            from_arboard(arboard::Error::ContentNotAvailable),
            CaptureError::NothingSelected
        );
    }

    #[test]
    fn other_arboard_errors_keep_their_detail() {
        let error = from_arboard(arboard::Error::ClipboardNotSupported);
        assert!(matches!(error, CaptureError::Platform(detail) if !detail.is_empty()));
    }

    #[test]
    fn failed_wl_paste_means_nothing_selected() {
        assert_eq!(
            wl_paste_result(false, Vec::new()),
            Err(CaptureError::NothingSelected)
        );
    }

    #[test]
    fn wl_paste_output_is_kept_verbatim() {
        let text = "می\u{200C}خواهم\nline 2";
        assert_eq!(
            wl_paste_result(true, text.as_bytes().to_vec()),
            Ok(text.to_owned())
        );
    }

    #[test]
    fn invalid_utf8_is_a_platform_error() {
        assert!(matches!(
            wl_paste_result(true, vec![0xFF, 0xFE]),
            Err(CaptureError::Platform(_))
        ));
    }
}
