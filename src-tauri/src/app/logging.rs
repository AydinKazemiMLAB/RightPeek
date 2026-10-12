//! Rotating log file (docs/PLAN.md Phase 3): off by default in release,
//! toggled in Settings. Logs events and errors, never the captured text.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};

use super::lock;
use crate::core::log::{Level, format_line, needs_rotation};

const FILE_NAME: &str = "rightpeek.log";
const BACKUP_NAME: &str = "rightpeek.old.log";

struct Logger {
    dir: PathBuf,
    enabled: AtomicBool,
    file: Mutex<Option<File>>,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

/// Sets the log folder and initial on/off state, and logs panics.
pub fn init(app: &AppHandle, enabled: bool) {
    let Ok(dir) = app.path().app_log_dir() else {
        return;
    };
    let logger = LOGGER.get_or_init(|| Logger {
        dir,
        enabled: AtomicBool::new(false),
        file: Mutex::new(None),
    });
    logger.enabled.store(enabled, Ordering::Release);
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        error(&format!("panic: {info}"));
        default_hook(info);
    }));
}

pub fn set_enabled(enabled: bool) {
    if let Some(logger) = LOGGER.get() {
        logger.enabled.store(enabled, Ordering::Release);
        if !enabled {
            // Close the file so it can be deleted or opened elsewhere.
            *lock(&logger.file) = None;
        }
    }
}

/// Folder that holds the log files, for the Settings window.
pub fn dir() -> Option<PathBuf> {
    LOGGER.get().map(|logger| logger.dir.clone())
}

pub fn info(message: &str) {
    write(Level::Info, message);
}

pub fn warn(message: &str) {
    write(Level::Warn, message);
}

pub fn error(message: &str) {
    write(Level::Error, message);
}

fn write(level: Level, message: &str) {
    if cfg!(debug_assertions) {
        eprintln!("RightPeek: {level} {message}");
    }
    let Some(logger) = LOGGER.get() else {
        return;
    };
    if !logger.enabled.load(Ordering::Acquire) {
        return;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let path = logger.dir.join(FILE_NAME);
    let mut file = lock(&logger.file);
    let len = match file.as_ref() {
        Some(f) => f.metadata().map_or(0, |meta| meta.len()),
        None => fs::metadata(&path).map_or(0, |meta| meta.len()),
    };
    if needs_rotation(len) {
        *file = None;
        // Replaces the previous backup, so at most two files exist.
        let _ = fs::rename(&path, logger.dir.join(BACKUP_NAME));
    }
    if file.is_none() {
        *file = open(&logger.dir);
    }
    if let Some(f) = file.as_mut() {
        let _ = writeln!(f, "{}", format_line(now, level, message));
    }
}

fn open(dir: &Path) -> Option<File> {
    fs::create_dir_all(dir).ok()?;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(FILE_NAME))
        .ok()
}
