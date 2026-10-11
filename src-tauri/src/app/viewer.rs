//! Viewer window: created on demand, destroyed on close (docs/PLAN.md §3).
//! The page pulls its content with `take_pending_text` (no push race).

use std::io::Write;
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

use super::{AppState, lock, settings};
use crate::core::settings::clamp_font_size;
use crate::core::text::{Paragraph, normalize, paragraphs};

pub const LABEL: &str = "viewer";
/// Event that tells an open viewer to pull new content.
pub const PENDING_EVENT: &str = "rightpeek://pending";
/// Dev/benchmark only: append hotkey->visible timings to this file.
const TIMING_LOG_ENV: &str = "RIGHTPEEK_TIMING_LOG";

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ViewerContent {
    /// Captured text, prepared for rendering; `plain` feeds the Copy button.
    Text {
        paragraphs: Vec<Paragraph>,
        plain: String,
    },
    /// A message instead of text (capture error, nothing captured yet).
    Notice { message: String },
}

impl ViewerContent {
    pub fn from_text(raw: &str) -> Self {
        let plain = normalize(raw);
        Self::Text {
            paragraphs: paragraphs(&plain),
            plain,
        }
    }

    pub fn notice(message: impl Into<String>) -> Self {
        Self::Notice {
            message: message.into(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerPayload {
    content: ViewerContent,
    font_size: u8,
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
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("RightPeek")
        .inner_size(560.0, 420.0)
        .min_inner_size(320.0, 200.0)
        .center()
        .focused(true)
        .build();
    if let Err(error) = built {
        eprintln!("RightPeek: cannot open viewer: {error}");
    }
}

/// Tray "Show last": the last capture, or a hint when there is none.
pub fn show_last(app: &AppHandle) {
    let state = app.state::<AppState>();
    let last = lock(&state.last).clone();
    let content = last.unwrap_or_else(|| {
        let hotkey = lock(&state.settings).hotkey.clone();
        ViewerContent::notice(format!(
            "Nothing captured yet. Select text in any app and press {hotkey}."
        ))
    });
    show(app, content, None);
}

#[tauri::command]
pub fn take_pending_text(state: State<'_, AppState>) -> ViewerPayload {
    let pending = lock(&state.pending).take();
    let content = pending
        .or_else(|| lock(&state.last).clone())
        .unwrap_or_else(|| ViewerContent::notice("Nothing captured yet."));
    ViewerPayload {
        content,
        font_size: lock(&state.settings).font_size,
    }
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
