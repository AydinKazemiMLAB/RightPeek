//! App wiring only. Logic lives in `core`, OS calls in `platform` (docs/PLAN.md §3).

pub mod core;
pub mod platform;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
