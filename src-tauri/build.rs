fn main() {
    // Declaring the app commands makes each one a permission
    // (`allow-<command>`) that capabilities must grant explicitly.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "take_pending_text",
        "viewer_ready",
        "set_font_size",
        "set_justify",
        "prepare_text",
        "open_settings",
        "get_settings",
        "set_hotkey",
        "set_theme",
        "set_autostart",
        "set_logging",
        "set_show_settings_on_start",
        "request_capture_permission",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("failed to run tauri-build");
}
