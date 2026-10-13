//! Database SQLite (incluso nel binario, con FTS5).
//!
//! Le migrazioni usano `PRAGMA user_version`: ogni voce di `MIGRATIONS`
//! porta lo schema dalla versione `i` alla `i + 1`. Le tabelle della
//! libreria arrivano in M4; in M0 ci sono solo `meta` e `settings`.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

const MIGRATIONS: &[&str] = &[
    // v1
    "CREATE TABLE meta (
        key   TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    ) STRICT;
    CREATE TABLE settings (
        key   TEXT PRIMARY KEY NOT NULL,
        value TEXT NOT NULL
    ) STRICT;",
];

pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("errore SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error(
        "il database ha lo schema v{found}, più recente di questa versione di FrameNote (v{supported})"
    )]
    TooNew { found: i64, supported: i64 },
}

/// Apre (o crea) il database e applica le migrazioni mancanti.
pub fn open(path: &Path, app_version: &str) -> Result<Connection, DbError> {
    let mut conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&mut conn, app_version)?;
    Ok(conn)
}

pub fn migrate(conn: &mut Connection, app_version: &str) -> Result<(), DbError> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > SCHEMA_VERSION {
        return Err(DbError::TooNew {
            found: current,
            supported: SCHEMA_VERSION,
        });
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let target = i as i64 + 1;
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", target)?;
        tx.commit()?;
        tracing::info!(schema = target, "migrazione del database applicata");
    }
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('created_with', ?1)
         ON CONFLICT(key) DO NOTHING",
        params![app_version],
    )?;
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('last_opened_with', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![app_version],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DbStatus {
    pub ok: bool,
    pub schema_version: Option<i64>,
    pub size_bytes: Option<u64>,
    pub fts5: bool,
    pub sqlite_version: String,
    pub created_with: Option<String>,
    pub error: Option<String>,
}

impl DbStatus {
    pub fn failed(error: String) -> Self {
        Self {
            ok: false,
            schema_version: None,
            size_bytes: None,
            fts5: false,
            sqlite_version: rusqlite::version().to_string(),
            created_with: None,
            error: Some(error),
        }
    }
}

/// Verifica che FTS5 sia compilato nel SQLite incluso creando una tabella
/// virtuale temporanea (nel database `temp`, quindi senza toccare i dati).
pub fn fts5_available(conn: &Connection) -> bool {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE temp.fts5_probe USING fts5(testo);
         DROP TABLE temp.fts5_probe;",
    )
    .is_ok()
}

pub fn status(conn: &Connection, path: &Path) -> DbStatus {
    let result = (|| -> rusqlite::Result<(i64, String, Option<String>)> {
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let check: String = conn.pragma_query_value(None, "quick_check", |r| r.get(0))?;
        let created_with = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'created_with'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok((version, check, created_with))
    })();
    let size_bytes = std::fs::metadata(path).ok().map(|m| m.len());
    match result {
        Ok((version, check, created_with)) => DbStatus {
            ok: check == "ok" && version == SCHEMA_VERSION,
            schema_version: Some(version),
            size_bytes,
            fts5: fts5_available(conn),
            sqlite_version: rusqlite::version().to_string(),
            created_with,
            error: (check != "ok").then(|| format!("controllo di integrità: {check}")),
        },
        Err(e) => DbStatus {
            size_bytes,
            ..DbStatus::failed(e.to_string())
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrazione_idempotente() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("t.sqlite");
        {
            let conn = open(&path, "0.0.1").unwrap();
            let v: i64 = conn
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            assert_eq!(v, SCHEMA_VERSION);
        }
        // Riapertura con una versione successiva: nessun errore, meta aggiornata.
        let conn = open(&path, "0.0.2").unwrap();
        let st = status(&conn, &path);
        assert!(st.ok, "{st:?}");
        assert_eq!(st.schema_version, Some(SCHEMA_VERSION));
        assert_eq!(st.created_with.as_deref(), Some("0.0.1"));
        let last: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'last_opened_with'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(last, "0.0.2");
        let tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name IN ('meta', 'settings')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 2);
    }

    #[test]
    fn fts5_disponibile() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(fts5_available(&conn));
        conn.execute_batch(
            "CREATE VIRTUAL TABLE prova USING fts5(testo);
             INSERT INTO prova (testo) VALUES ('ciao, registro la call per gli appunti');",
        )
        .unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM prova WHERE prova MATCH 'appunti'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn rifiuta_uno_schema_piu_recente() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        assert!(matches!(
            migrate(&mut conn, "0.0.1"),
            Err(DbError::TooNew { .. })
        ));
    }
}
