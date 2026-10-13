//! Comandi invocabili dal frontend. Gli errori sono frasi in italiano
//! pensate per Domenico; i dettagli tecnici vanno nei log.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::diagnostics::{export, info};
use crate::state::AppState;

fn collect_info(app: &AppHandle) -> info::DiagnosticInfo {
    let state = app.state::<AppState>();
    let database = state.db_status();
    let collected = info::collect(&state.meta, &state.paths, database);
    if let Ok(mut last) = state.last_info.lock() {
        *last = Some(collected.clone());
    }
    collected
}

/// Raccoglie i dati di diagnostica (include le codifiche di prova: qualche secondo).
#[tauri::command]
pub async fn diagnostic_info(app: AppHandle) -> Result<info::DiagnosticInfo, String> {
    tracing::info!("raccolta diagnostica avviata");
    tauri::async_runtime::spawn_blocking(move || collect_info(&app))
        .await
        .map_err(|e| {
            tracing::error!("raccolta diagnostica interrotta: {e}");
            "La raccolta della diagnostica si è interrotta. Riprova; se succede ancora, esporta la diagnostica.".to_string()
        })
}

#[tauri::command]
pub fn startup_problems(state: State<'_, AppState>) -> Vec<String> {
    state.startup_problems.clone()
}

#[tauri::command]
pub fn default_diagnostics_file_name() -> String {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    export::default_file_name(now)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub entries: Vec<String>,
}

/// Crea lo zip di diagnostica nel percorso scelto con la finestra di salvataggio.
#[tauri::command]
pub async fn export_diagnostics(app: AppHandle, dest: String) -> Result<ExportResult, String> {
    let dest = export::with_zip_extension(&PathBuf::from(dest));
    tauri::async_runtime::spawn_blocking(move || {
        let cached = app
            .state::<AppState>()
            .last_info
            .lock()
            .ok()
            .and_then(|l| l.clone());
        let info = match cached {
            Some(i) => i,
            None => collect_info(&app),
        };
        let state = app.state::<AppState>();
        let info_json = serde_json::to_value(&info).map_err(|e| e.to_string())?;
        let entries = export::write_zip(
            &dest,
            &export::ExportInput {
                info_json,
                logs_dir: &state.paths.logs_dir,
                projects_dir: &state.paths.projects_dir,
                home_dir: state.paths.home_dir.as_deref(),
            },
        )
        .map_err(|e| e.to_string())?;
        tracing::info!(files = entries.len(), "diagnostica esportata");
        Ok(ExportResult {
            path: dest.display().to_string(),
            entries,
        })
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r: Result<ExportResult, String>| r)
    .map_err(|e| {
        tracing::error!("esportazione diagnostica non riuscita: {e}");
        format!("Non sono riuscito a salvare il file di diagnostica ({e}). Prova a sceglierlo in un'altra cartella, ad esempio la Scrivania.")
    })
}

/// Errori e avvisi del frontend, scritti nello stesso log del backend.
#[tauri::command]
pub fn log_frontend(level: String, message: String) {
    let message: String = message.chars().take(2000).collect();
    match level.as_str() {
        "error" => tracing::error!(target: "frontend", "{message}"),
        "warn" => tracing::warn!(target: "frontend", "{message}"),
        _ => tracing::info!(target: "frontend", "{message}"),
    }
}
