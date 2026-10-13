//! Cartelle di FrameNote (§4 della specifica).
//!
//! - Progetti: `Documenti/FrameNote/Progetti/`
//! - Dati app (modelli, database, log): cartella dati locale dell'app,
//!   separata dai progetti.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub const DB_FILE_NAME: &str = "framenote.sqlite";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub home_dir: Option<PathBuf>,
    pub projects_root: PathBuf,
    pub projects_dir: PathBuf,
    pub data_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub models_dir: PathBuf,
    pub db_path: PathBuf,
}

impl AppPaths {
    /// Costruisce i percorsi a partire dalle cartelle di sistema.
    /// `documents_dir`: cartella Documenti dell'utente.
    /// `data_dir`: cartella dati locale dell'app (già specifica di FrameNote).
    pub fn new(home_dir: Option<PathBuf>, documents_dir: &Path, data_dir: &Path) -> Self {
        let projects_root = documents_dir.join("FrameNote");
        Self {
            home_dir,
            projects_dir: projects_root.join("Progetti"),
            projects_root,
            data_dir: data_dir.to_path_buf(),
            logs_dir: data_dir.join("logs"),
            models_dir: data_dir.join("models"),
            db_path: data_dir.join(DB_FILE_NAME),
        }
    }

    /// Crea le cartelle se mancano (idempotente).
    pub fn ensure_dirs(&self) -> io::Result<()> {
        for dir in [
            &self.projects_dir,
            &self.data_dir,
            &self.logs_dir,
            &self.models_dir,
        ] {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// Versione dei percorsi da mostrare o esportare, con la home sostituita da `~`.
    pub fn anonymized(&self) -> PathsView {
        let a = |p: &Path| anonymize_path(p, self.home_dir.as_deref());
        PathsView {
            projects_dir: a(&self.projects_dir),
            data_dir: a(&self.data_dir),
            logs_dir: a(&self.logs_dir),
            models_dir: a(&self.models_dir),
            database: a(&self.db_path),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PathsView {
    pub projects_dir: String,
    pub data_dir: String,
    pub logs_dir: String,
    pub models_dir: String,
    pub database: String,
}

/// Sostituisce il prefisso della home con `~` (solo se è davvero un prefisso di componenti).
pub fn anonymize_path(path: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home {
        if !home.as_os_str().is_empty() {
            if let Ok(rest) = path.strip_prefix(home) {
                if rest.as_os_str().is_empty() {
                    return "~".to_string();
                }
                let sep = std::path::MAIN_SEPARATOR;
                return format!("~{sep}{}", rest.display());
            }
        }
    }
    path.display().to_string()
}

/// Sostituisce ogni occorrenza della home in un testo (per i log esportati).
/// Su Windows i percorsi possono comparire anche con le barre normali.
pub fn anonymize_text(text: &str, home: Option<&Path>) -> String {
    let Some(home) = home else {
        return text.to_string();
    };
    let home = home.display().to_string();
    let home = home.trim_end_matches(['/', '\\']);
    if home.len() < 2 {
        return text.to_string();
    }
    let mut out = text.replace(home, "~");
    let alt = home.replace('\\', "/");
    if alt != home {
        out = out.replace(&alt, "~");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_segue_la_specifica() {
        let p = AppPaths::new(
            Some(PathBuf::from("/Users/dom")),
            Path::new("/Users/dom/Documents"),
            Path::new("/Users/dom/Library/Application Support/com.mimmo8bit.framenote"),
        );
        assert_eq!(
            p.projects_dir,
            PathBuf::from("/Users/dom/Documents/FrameNote/Progetti")
        );
        assert_eq!(
            p.db_path,
            PathBuf::from(
                "/Users/dom/Library/Application Support/com.mimmo8bit.framenote/framenote.sqlite"
            )
        );
        assert!(p.logs_dir.ends_with("logs"));
        assert!(p.models_dir.ends_with("models"));
    }

    #[test]
    fn ensure_dirs_crea_tutto_ed_e_idempotente() {
        let tmp = tempfile::tempdir().unwrap();
        let p = AppPaths::new(None, &tmp.path().join("Docs"), &tmp.path().join("data"));
        p.ensure_dirs().unwrap();
        p.ensure_dirs().unwrap();
        assert!(p.projects_dir.is_dir());
        assert!(p.logs_dir.is_dir());
        assert!(p.models_dir.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn anonimizza_la_home() {
        let home = Path::new("/Users/dom");
        assert_eq!(
            anonymize_path(Path::new("/Users/dom/Documents/FrameNote"), Some(home)),
            "~/Documents/FrameNote"
        );
        assert_eq!(anonymize_path(Path::new("/Users/dom"), Some(home)), "~");
        // "/Users/domenico" non è dentro "/Users/dom": niente sostituzione parziale.
        assert_eq!(
            anonymize_path(Path::new("/Users/domenico/x"), Some(home)),
            "/Users/domenico/x"
        );
        assert_eq!(anonymize_path(Path::new("/tmp/x"), None), "/tmp/x");
    }

    #[test]
    fn anonimizza_il_testo_dei_log() {
        let home = Path::new(r"C:\Users\Dom");
        let log = r"aperto C:\Users\Dom\Documents\FrameNote e C:/Users/Dom/AppData";
        assert_eq!(
            anonymize_text(log, Some(home)),
            r"aperto ~\Documents\FrameNote e ~/AppData"
        );
        assert_eq!(anonymize_text("niente", None), "niente");
        // Una home vuota o di un solo carattere non deve cancellare il testo.
        assert_eq!(anonymize_text("/a/b", Some(Path::new("/"))), "/a/b");
    }
}
