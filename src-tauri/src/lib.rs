//! App wiring only. Logic lives in `core`, OS calls in `platform`, app
//! behaviour in `app` (docs/PLAN.md §3).

mod app;
pub mod core;
pub mod platform;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch is forwarded here and exits.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            app::settings::open_window(app);
        }))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(app::hotkey::handle)
                .build(),
        )
        .manage(app::AppState::default())
        .invoke_handler(tauri::generate_handler![
            app::viewer::take_pending_text,
            app::viewer::viewer_ready,
            app::viewer::set_font_size,
            app::settings::get_settings,
            app::settings::set_hotkey,
            app::settings::set_autostart,
        ])
        .setup(|app| app::setup(app.handle()))
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(app::on_run_event);
}
