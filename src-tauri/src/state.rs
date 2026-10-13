use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use rusqlite::Connection;
use tracing_appender::non_blocking::WorkerGuard;

use crate::diagnostics::info::{AppMeta, DiagnosticInfo};
use crate::library::db;
use crate::paths::AppPaths;

pub struct AppState {
    pub meta: AppMeta,
    pub paths: AppPaths,
    /// Il database può non aprirsi (es. disco pieno): l'app parte lo stesso
    /// e la diagnostica mostra l'errore.
    pub db: Mutex<Result<Connection, String>>,
    /// Ultima diagnostica raccolta, riusata dall'esportazione.
    pub last_info: Mutex<Option<DiagnosticInfo>>,
    /// Problemi dell'avvio da mostrare nella home (frasi in italiano).
    pub startup_problems: Vec<String>,
    /// Encoder H.264 scelto con la prova reale (calcolato alla prima registrazione).
    pub encoder: Mutex<Option<String>>,
    /// Registrazione in corso (solo macOS in M1).
    #[cfg(target_os = "macos")]
    pub recording: Mutex<Option<Recording>>,
    pub export_cancel: AtomicBool,
    pub _log_guard: Option<WorkerGuard>,
}

#[cfg(target_os = "macos")]
pub struct Recording {
    pub project_dir: std::path::PathBuf,
    pub active: crate::capture::macos::ActiveRecording,
}

impl AppState {
    pub fn is_recording(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.recording.lock().map(|r| r.is_some()).unwrap_or(false)
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    /// Stato del database per la diagnostica.
    pub fn db_status(&self) -> db::DbStatus {
        match self.db.lock() {
            Ok(guard) => match guard.as_ref() {
                Ok(conn) => db::status(conn, &self.paths.db_path),
                Err(e) => db::DbStatus::failed(e.clone()),
            },
            Err(_) => db::DbStatus::failed("database non disponibile (lock)".into()),
        }
    }
}
