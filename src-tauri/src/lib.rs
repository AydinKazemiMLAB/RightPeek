//! App wiring only. Logic lives in `core`, OS calls in `platform`, app
//! behaviour in `app` (docs/PLAN.md §3).

mod app;
pub mod core;
pub mod platform;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch is forwarded here and exits.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            use core::settings::{SecondLaunch, second_launch};
            match second_launch(args) {
                SecondLaunch::Capture => app::hotkey::capture_and_show(app),
                SecondLaunch::OpenSettings => app::settings::open_window(app),
                SecondLaunch::Nothing => {}
            }
        }))
        .plugin(
            tauri_plugin_store::Builder::new()
                .default_deserialize_fn(app::settings::deserialize_store)
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![core::settings::AUTOSTART_ARG]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(app::hotkey::handle)
                .build(),
        )
        .plugin(
            // The settings window is small and fixed; only the viewer is remembered.
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(app::viewer::WINDOW_STATE)
                .with_denylist(&[app::settings::LABEL])
                .build(),
        )
        .manage(app::AppState::default())
        .invoke_handler(tauri::generate_handler![
            app::viewer::take_pending_text,
            app::viewer::viewer_ready,
            app::viewer::set_font_size,
            app::viewer::set_justify,
            app::settings::open_settings,
            app::settings::get_settings,
            app::settings::set_hotkey,
            app::settings::set_autostart,
            app::settings::set_logging,
            app::settings::set_show_settings_on_start,
        ])
        .setup(|app| app::setup(app.handle()))
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(app::on_run_event);
}
