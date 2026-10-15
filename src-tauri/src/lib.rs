// In M1 la registrazione esiste solo su macOS: sugli altri sistemi parte di
// questi moduli è usata soltanto dai test (il controllo completo gira in CI su macOS).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod capture;
mod commands;
mod diagnostics;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod encoder;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod input_events;
mod library;
mod paths;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod project;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod render;
mod state;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod zoom_commands;

use std::sync::Mutex;

use tauri::{AppHandle, Manager};

use crate::diagnostics::info::AppMeta;
use crate::paths::{anonymize_path, AppPaths};
use crate::state::AppState;

fn init_state(app: &AppHandle) -> AppState {
    let mut problems = Vec::new();
    let resolver = app.path();
    let home = resolver.home_dir().ok();
    let data_dir = resolver
        .app_local_data_dir()
        .expect("cartella dati dell'app non determinabile");
    let documents = resolver
        .document_dir()
        .ok()
        .or_else(|| home.as_ref().map(|h| h.join("Documents")))
        .unwrap_or_else(|| {
            problems.push(
                "Cartella Documenti non trovata: i progetti vanno nella cartella dati dell'app."
                    .into(),
            );
            data_dir.clone()
        });
    let paths = AppPaths::new(home, &documents, &data_dir);

    if let Err(e) = paths.ensure_dirs() {
        problems.push(format!(
            "Non riesco a creare le cartelle di FrameNote ({e}). Controlla i permessi della cartella Documenti in Impostazioni di Sistema → Privacy e sicurezza."
        ));
    }

    let log_guard = match diagnostics::logging::init(&paths.logs_dir) {
        Ok(g) => Some(g),
        Err(e) => {
            problems.push(format!("Log non disponibili: {e}"));
            None
        }
    };

    let package = app.package_info();
    let meta = AppMeta {
        name: package.name.clone(),
        version: package.version.to_string(),
        identifier: app.config().identifier.clone(),
    };
    tracing::info!(
        version = %meta.version,
        commit = env!("FRAMENOTE_COMMIT"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        "FrameNote avviato"
    );
    tracing::info!(
        projects = %anonymize_path(&paths.projects_dir, paths.home_dir.as_deref()),
        data = %anonymize_path(&paths.data_dir, paths.home_dir.as_deref()),
        "cartelle"
    );

    let db = library::db::open(&paths.db_path, &meta.version).map_err(|e| {
        tracing::error!("database non aperto: {e}");
        problems.push("Il database non si è aperto: i dettagli sono nella diagnostica.".into());
        e.to_string()
    });

    AppState {
        meta,
        paths,
        db: Mutex::new(db),
        last_info: Mutex::new(None),
        startup_problems: problems,
        encoder: Mutex::new(None),
        #[cfg(target_os = "macos")]
        recording: Mutex::new(None),
        export_cancel: std::sync::atomic::AtomicBool::new(false),
        _log_guard: log_guard,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = init_state(app.handle());
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::diagnostic_info,
            commands::startup_problems,
            commands::default_diagnostics_file_name,
            commands::export_diagnostics,
            commands::log_frontend,
            zoom_commands::capture_status,
            zoom_commands::request_screen_permission,
            zoom_commands::open_screen_permission_settings,
            zoom_commands::start_recording,
            zoom_commands::recording_status,
            zoom_commands::stop_recording,
            zoom_commands::export_zoom,
            zoom_commands::cancel_export,
            zoom_commands::open_project_file,
            zoom_commands::replan_zooms,
            zoom_commands::last_recording,
        ])
        .run(tauri::generate_context!())
        .expect("errore all'avvio di FrameNote");
}
