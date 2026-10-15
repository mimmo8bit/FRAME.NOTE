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
mod privacy;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod project;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod render;
mod security;
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
    let projects_parent = home.clone().unwrap_or_else(|| {
        problems.push(
            "Cartella Inizio non trovata: i progetti vanno nella cartella dati dell'app.".into(),
        );
        data_dir.clone()
    });
    let paths = AppPaths::new(home, &projects_parent, &data_dir);

    // Fino a v0.1.0 i progetti erano in Documenti/FrameNote: li sposto una volta
    // sola, e solo se FrameNote era già installata (il database esiste). Così
    // una nuova installazione non tocca mai Documenti e macOS non chiede nulla.
    // Nell'avvio di prova della CI no.
    let migration_marker = data_dir.join(".progetti-spostati-da-documenti");
    let migration = if std::env::var_os(SMOKE_TEST_ENV).is_some()
        || migration_marker.exists()
        || !paths.db_path.exists()
    {
        Ok(paths::Migration::Nothing)
    } else {
        let result = match resolver.document_dir() {
            Ok(docs) => paths::migrate_projects(
                &docs.join("FrameNote").join("Progetti"),
                &paths.projects_dir,
            ),
            Err(_) => Ok(paths::Migration::Nothing),
        };
        if matches!(
            result,
            Ok(paths::Migration::Nothing | paths::Migration::Moved(_))
        ) {
            let _ = std::fs::write(&migration_marker, b"");
        }
        result
    };

    if let Err(e) = paths.ensure_dirs() {
        problems.push(format!(
            "Non riesco a creare le cartelle di FrameNote ({e}). Controlla che la cartella Inizio non sia piena o protetta."
        ));
    }
    let protect_problems = paths.protect();

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
    match migration {
        Ok(paths::Migration::Nothing) => {}
        Ok(paths::Migration::Moved(n)) => {
            tracing::info!(progetti = n, "progetti spostati da Documenti a ~/FrameNote")
        }
        Ok(paths::Migration::Partial { moved, left }) => {
            tracing::warn!(moved, left, "progetti spostati in parte da Documenti");
            problems.push(format!(
                "{left} progetti sono rimasti in Documenti/FrameNote/Progetti perché in ~/FrameNote/Progetti c'è già una cartella con lo stesso nome."
            ));
        }
        Err(e) => {
            tracing::warn!("spostamento dei progetti da Documenti non riuscito: {e}");
            problems.push(
                "Non sono riuscita a spostare i vecchi progetti da Documenti/FrameNote: sono ancora lì, intatti.".into(),
            );
        }
    }
    for p in protect_problems {
        tracing::warn!("cartella non protetta: {p}");
    }

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

/// Avvio di prova della CI: l'app si apre davvero e si chiude da sola.
pub const SMOKE_TEST_ENV: &str = "FRAMENOTE_SMOKE_TEST";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Tutti i file creati da FrameNote (e da FFmpeg, che eredita l'impostazione)
    // sono leggibili solo dall'utente.
    #[cfg(unix)]
    // SAFETY: umask non ha precondizioni e cambia solo la maschera del processo.
    unsafe {
        libc::umask(0o077);
    }
    tauri::Builder::default()
        .plugin(security::navigation_guard())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = init_state(app.handle());
            app.manage(state);
            if std::env::var_os(SMOKE_TEST_ENV).is_some() {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    tracing::info!("avvio di prova completato");
                    println!("FRAMENOTE_SMOKE_OK");
                    handle.exit(0);
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::diagnostic_info,
            commands::startup_problems,
            commands::default_diagnostics_file_name,
            commands::export_diagnostics,
            commands::log_frontend,
            commands::diagnostics_preview,
            commands::privacy_report,
            commands::open_filevault_settings,
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
