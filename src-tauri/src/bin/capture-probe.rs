//! Dev-only diagnostic for the Phase 1 capture matrix (docs/capture-matrix.md).
//!
//! Usage: `cargo run --manifest-path src-tauri/Cargo.toml --bin capture-probe -- [--uia | --clipboard] [delay_secs]`
//! Select text in the target app, focus it within the delay, then read the report.
//! `--clipboard` skips UI Automation and tests only the clipboard fallback;
//! `--uia` runs only UI Automation and prints why it failed.

use std::time::{Duration, Instant};

use rightpeek_lib::platform;

const DEFAULT_DELAY_SECS: u64 = 3;
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(3);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let clipboard_only = args.iter().any(|arg| arg == "--clipboard");
    let uia_only = args.iter().any(|arg| arg == "--uia");
    let delay = args
        .iter()
        .find_map(|arg| arg.parse().ok())
        .unwrap_or(DEFAULT_DELAY_SECS);

    let mode = match (uia_only, clipboard_only) {
        (true, _) => "UIA only",
        (false, true) => "clipboard only",
        (false, false) => "UIA, then clipboard",
    };
    println!("Mode: {mode}. Focus the target app with text selected. Capturing in {delay} s...");
    std::thread::sleep(Duration::from_secs(delay));

    let clipboard_before = platform::clipboard_text();
    let started = Instant::now();
    let result = if uia_only {
        platform::capture_via_uia(CAPTURE_TIMEOUT)
    } else if clipboard_only {
        platform::capture_via_clipboard(CAPTURE_TIMEOUT)
    } else {
        platform::selection_provider().capture(CAPTURE_TIMEOUT)
    };
    let elapsed = started.elapsed();
    let clipboard_after = platform::clipboard_text();

    println!("----------------------------------------");
    match &result {
        Ok(captured) => {
            println!("method   : {:?}", captured.method);
            println!("chars    : {}", captured.text.chars().count());
            println!("text     :");
            println!("{}", captured.text);
        }
        Err(error) => println!("error    : {error}"),
    }
    println!("----------------------------------------");
    println!("elapsed  : {} ms", elapsed.as_millis());
    println!(
        "clipboard: {}",
        if clipboard_before == clipboard_after {
            "restored (text unchanged)"
        } else {
            "CHANGED - restore failed"
        }
    );
}
