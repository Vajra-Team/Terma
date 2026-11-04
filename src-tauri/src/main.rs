mod commands;

use std::sync::Arc;

use tauri::{Emitter, Manager};
use terma_core::dirs::UserDirs;
use terma_core::ops::Terma;

/// WebKitGTK bricht mit NVIDIA unter Wayland beim DMA-BUF-Renderer mit „Error 71 (Protokollfehler)“ ab
/// (auf einem PC mit Nvidia-Karte so passiert). Ohne ihn läuft es überall; wer die Variable selbst setzt, behält seinen Wert.
fn webkit_workaround() {
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: läuft als Erstes in main, bevor Threads entstehen.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
}

fn main() {
    webkit_workaround();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let terma = Terma::new(UserDirs::from_env());
    let _ = terma.first_start();
    let state = commands::AppState { terma: Arc::new(terma), startup_file: commands::file_arg(&args) };

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(path) = commands::file_arg(argv.get(1..).unwrap_or(&[])) {
                let _ = app.emit("open-file", path);
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::overview,
            commands::search,
            commands::inspect_file,
            commands::plan_install,
            commands::install,
            commands::setup_appimage,
            commands::remove,
            commands::remove_preview,
            commands::remove_flatpak,
            commands::update_all,
            commands::force_update_aur,
            commands::get_settings,
            commands::set_passwordless,
            commands::startup_file,
            commands::open_url,
        ])
        .run(tauri::generate_context!())
        .expect("Terma konnte nicht starten");
}

#[cfg(test)]
mod tests {
    use super::webkit_workaround;

    #[test]
    fn dmabuf_renderer_off_unless_user_decided() {
        // SAFETY: einziger Test, der diese Variable anfasst.
        unsafe { std::env::remove_var("WEBKIT_DISABLE_DMABUF_RENDERER") };
        webkit_workaround();
        assert_eq!(std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").as_deref(), Ok("1"));

        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "0") };
        webkit_workaround();
        assert_eq!(std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").as_deref(), Ok("0"), "eigene Einstellung bleibt");
    }
}
