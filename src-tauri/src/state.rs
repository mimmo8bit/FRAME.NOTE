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
    pub _log_guard: Option<WorkerGuard>,
}

impl AppState {
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
