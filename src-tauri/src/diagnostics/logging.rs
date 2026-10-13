//! Log su file nella cartella dati: rotazione giornaliera, al massimo 7 file.
//! Nei log non finiscono mai contenuti dell'utente (testi, trascrizioni, titoli).

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub const LOG_PREFIX: &str = "framenote";
pub const LOG_SUFFIX: &str = "log";
pub const MAX_LOG_FILES: usize = 7;

/// Avvia il logging. Il `WorkerGuard` va tenuto vivo per tutta la durata
/// dell'app: quando viene distrutto scrive su disco le ultime righe.
pub fn init(logs_dir: &Path) -> Result<WorkerGuard, String> {
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(LOG_PREFIX)
        .filename_suffix(LOG_SUFFIX)
        .max_log_files(MAX_LOG_FILES)
        .build(logs_dir)
        .map_err(|e| format!("impossibile aprire i log: {e}"))?;
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let filter = EnvFilter::try_from_env("FRAMENOTE_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(writer).with_ansi(false))
        .with(cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr)))
        .try_init()
        .map_err(|e| format!("logging già inizializzato: {e}"))?;

    install_panic_hook();
    Ok(guard)
}

/// I panic finiscono nel log (con posizione), poi prosegue l'hook predefinito.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "posizione sconosciuta".into());
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic senza messaggio".into());
        tracing::error!(target: "panic", %location, "panic: {message}");
        default_hook(info);
    }));
}

/// Elenca i file di log (più recenti per ultimi, per nome).
pub fn log_files(logs_dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(logs_dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(LOG_PREFIX) && n.ends_with(LOG_SUFFIX))
        })
        .collect();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elenca_solo_i_log_di_framenote() {
        let tmp = tempfile::tempdir().unwrap();
        for name in [
            "framenote.2026-10-12.log",
            "framenote.2026-10-13.log",
            "altro.txt",
            "framenote.tmp",
        ] {
            std::fs::write(tmp.path().join(name), "x").unwrap();
        }
        let files = log_files(tmp.path());
        let names: Vec<_> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            ["framenote.2026-10-12.log", "framenote.2026-10-13.log"]
        );
    }

    #[test]
    fn cartella_inesistente_nessun_log() {
        assert!(log_files(Path::new("/percorso/che/non/esiste")).is_empty());
    }
}
