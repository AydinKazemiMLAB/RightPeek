//! Viewer window: created on demand and destroyed on close (docs/design.md).
//! The page pulls its content with `take_pending_text` (no push race).

use std::io::Write;
use std::time::Instant;

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};
use tauri_plugin_window_state::{AppHandleExt as _, StateFlags};

use super::{AppState, lock, logging, settings};
use crate::core::capture::CaptureError;
use crate::core::settings::{Theme, clamp_font_size};
use crate::core::text::{Paragraph, normalize, paragraphs};

pub const LABEL: &str = "viewer";
/// Event that tells an open viewer to pull new content.
pub const PENDING_EVENT: &str = "rightpeek://pending";
/// Dev/benchmark only: append hotkey->visible timings to this file.
const TIMING_LOG_ENV: &str = "RIGHTPEEK_TIMING_LOG";
/// What the window-state plugin remembers for the viewer.
pub const WINDOW_STATE: StateFlags = StateFlags::SIZE
    .union(StateFlags::POSITION)
    .union(StateFlags::MAXIMIZED);

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ViewerContent {
    /// Captured text, prepared for rendering; `plain` feeds the Copy button.
    Text {
        paragraphs: Vec<Paragraph>,
        plain: String,
    },
    /// A message instead of text (capture error, nothing captured yet).
    Notice { title: String, hint: String },
}

impl ViewerContent {
    pub fn from_text(raw: &str) -> Self {
        let plain = normalize(raw);
        Self::Text {
            paragraphs: paragraphs(&plain),
            plain,
        }
    }

    pub fn notice(title: impl Into<String>, hint: impl Into<String>) -> Self {
        Self::Notice {
            title: title.into(),
            hint: hint.into(),
        }
    }

    pub fn from_error(error: &CaptureError) -> Self {
        Self::notice(error.title(), error.hint())
    }

    /// Text handed over by the macOS Services menu.
    #[cfg(target_os = "macos")]
    pub fn from_service(raw: &str) -> Self {
        match crate::core::capture::selection_text([raw.to_owned()]) {
            Some(text) => Self::from_text(&text),
            None => Self::from_error(&CaptureError::NothingSelected),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerPayload {
    content: ViewerContent,
    font_size: u8,
    justify: bool,
    theme: Theme,
}

/// Shows `content` in the viewer, creating the window if needed.
pub fn show(app: &AppHandle, content: ViewerContent, pressed_at: Option<Instant>) {
    let state = app.state::<AppState>();
    if matches!(content, ViewerContent::Text { .. }) {
        *lock(&state.last) = Some(content.clone());
    }
    *lock(&state.pending) = Some(content);
    *lock(&state.hotkey_pressed_at) = pressed_at;

    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = app.emit_to(LABEL, PENDING_EVENT, ());
        reveal(&window);
        return;
    }
    if let Some(window) = build(app) {
        reveal(&window);
    }
}

fn reveal(window: &WebviewWindow) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

/// Builds the viewer hidden; the window-state plugin restores its size and
/// position before it is first shown.
fn build(app: &AppHandle) -> Option<WebviewWindow> {
    let theme = lock(&app.state::<AppState>().settings).theme;
    let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("RightPeek")
        .theme(settings::native_theme(theme))
        .inner_size(560.0, 420.0)
        .min_inner_size(320.0, 200.0)
        .center()
        .visible(false)
        .build();
    let window = match built {
        Ok(window) => window,
        Err(error) => {
            logging::error(&format!("cannot open viewer: {error}"));
            return None;
        }
    };
    // Look the window up instead of capturing a clone: a clone inside its own
    // event handler is a reference cycle that keeps every closed viewer alive.
    let handle = app.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed
        ) {
            // Persist now: the plugin itself only writes on app exit.
            let _ = handle.save_window_state(WINDOW_STATE);
        }
    });
    Some(window)
}

/// Tray "Show last": the last capture, or a hint when there is none.
pub fn show_last(app: &AppHandle) {
    let state = app.state::<AppState>();
    let last = lock(&state.last).clone();
    let content = last.unwrap_or_else(|| {
        let hotkey = lock(&state.settings).hotkey.clone();
        ViewerContent::notice(
            "Nothing captured yet",
            format!("Select text in any app and press {hotkey}."),
        )
    });
    show(app, content, None);
}

#[tauri::command]
pub fn take_pending_text(state: State<'_, AppState>) -> ViewerPayload {
    let pending = lock(&state.pending).take();
    let content = pending
        .or_else(|| lock(&state.last).clone())
        .unwrap_or_else(|| ViewerContent::notice("Nothing captured yet", ""));
    let settings = lock(&state.settings);
    ViewerPayload {
        content,
        font_size: settings.font_size,
        justify: settings.justify,
        theme: settings.theme,
    }
}

/// Text edited in the viewer, prepared like a capture (normalized, split,
/// directions detected). The edit stays in the window; "Show last" keeps
/// the capture.
#[tauri::command]
pub fn prepare_text(text: String) -> ViewerContent {
    ViewerContent::from_text(&text)
}

/// Called by the page once the text is rendered; records the latency.
#[tauri::command]
pub fn viewer_ready(state: State<'_, AppState>) {
    let Some(pressed_at) = lock(&state.hotkey_pressed_at).take() else {
        return;
    };
    let elapsed_ms = pressed_at.elapsed().as_millis();
    if let Some(path) = std::env::var_os(TIMING_LOG_ENV)
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        let _ = writeln!(file, "hotkey_to_visible_ms={elapsed_ms}");
    }
}

#[tauri::command]
pub fn set_font_size(app: AppHandle, state: State<'_, AppState>, size: u8) -> u8 {
    let size = clamp_font_size(size);
    lock(&state.settings).font_size = size;
    settings::save(&app);
    size
}

#[tauri::command]
pub fn set_justify(app: AppHandle, state: State<'_, AppState>, enabled: bool) -> bool {
    lock(&state.settings).justify = enabled;
    settings::save(&app);
    enabled
}
