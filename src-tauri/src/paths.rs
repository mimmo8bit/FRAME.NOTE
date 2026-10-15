//! Cartelle di FrameNote (§4 della specifica).
//!
//! - Progetti: `~/FrameNote/Progetti/`, fuori da Documenti e Scrivania così
//!   iCloud "Scrivania e Documenti" e OneDrive non li sincronizzano
//!   (`PIANO_SICUREZZA.md`, D1). Fino a v0.1.0 erano in `Documenti/FrameNote`.
//! - Dati app (modelli, database, log): cartella dati locale dell'app,
//!   separata dai progetti.
//!
//! Le cartelle sono leggibili solo dall'utente (`0700` su macOS) ed escluse
//! dall'indicizzazione di Spotlight.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub const DB_FILE_NAME: &str = "framenote.sqlite";

/// File che chiede a Spotlight di non indicizzare la cartella.
pub const NO_INDEX_FILE: &str = ".metadata_never_index";

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
    /// `projects_parent`: cartella che contiene `FrameNote/` (la home).
    /// `data_dir`: cartella dati locale dell'app (già specifica di FrameNote).
    pub fn new(home_dir: Option<PathBuf>, projects_parent: &Path, data_dir: &Path) -> Self {
        let projects_root = projects_parent.join("FrameNote");
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

    /// Cartelle leggibili solo dall'utente e fuori da Spotlight (idempotente).
    /// Gli errori non bloccano l'avvio: la schermata Privacy li mostra.
    pub fn protect(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for dir in [&self.projects_root, &self.data_dir] {
            if let Err(e) = make_private(dir) {
                problems.push(format!(
                    "{}: {e}",
                    anonymize_path(dir, self.home_dir.as_deref())
                ));
            }
            let marker = dir.join(NO_INDEX_FILE);
            if !marker.exists() {
                let _ = fs::write(&marker, b"");
            }
        }
        problems
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

/// Permessi `0700` sulla cartella e su tutte le sottocartelle già esistenti
/// (create da versioni precedenti con permessi più larghi).
#[cfg(unix)]
pub fn make_private(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if !dir.is_dir() {
        return Ok(());
    }
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
pub fn make_private(_dir: &Path) -> io::Result<()> {
    // Su Windows le cartelle del profilo sono già riservate all'utente.
    Ok(())
}

/// `Some(true)` se gli altri utenti non possono leggere la cartella.
#[cfg(unix)]
pub fn is_private(dir: &Path) -> Option<bool> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(dir).ok()?.permissions().mode();
    Some(mode & 0o077 == 0)
}

#[cfg(not(unix))]
pub fn is_private(_dir: &Path) -> Option<bool> {
    None
}

/// Esito dello spostamento dei progetti dalla vecchia cartella in Documenti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Migration {
    /// Niente da spostare.
    Nothing,
    /// Progetti spostati.
    Moved(usize),
    /// Alcuni progetti hanno lo stesso nome di uno nuovo e sono rimasti al loro posto.
    Partial { moved: usize, left: usize },
}

/// Sposta i progetti da `legacy` (es. `Documenti/FrameNote/Progetti`) a `target`
/// (`~/FrameNote/Progetti`) e rimuove le vecchie cartelle se restano vuote.
/// Stesso disco: è un semplice cambio di nome, nessuna copia.
pub fn migrate_projects(legacy: &Path, target: &Path) -> io::Result<Migration> {
    if !legacy.is_dir() || legacy == target {
        return Ok(Migration::Nothing);
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let target_empty = !target.exists() || fs::read_dir(target)?.next().is_none();
    let entries: Vec<_> = fs::read_dir(legacy)?.collect::<Result<_, _>>()?;
    let outcome = if target_empty {
        if target.exists() {
            fs::remove_dir(target)?;
        }
        fs::rename(legacy, target)?;
        Migration::Moved(entries.len())
    } else {
        let (mut moved, mut left) = (0, 0);
        for e in entries {
            let dest = target.join(e.file_name());
            if dest.exists() {
                left += 1;
            } else {
                fs::rename(e.path(), dest)?;
                moved += 1;
            }
        }
        if left == 0 {
            Migration::Moved(moved)
        } else {
            Migration::Partial { moved, left }
        }
    };
    // Pulizia: Progetti e poi FrameNote, solo se vuote (fallisce in silenzio altrimenti).
    let _ = fs::remove_dir(legacy);
    if let Some(root) = legacy.parent() {
        let _ = fs::remove_file(root.join(".DS_Store"));
        let _ = fs::remove_dir(root);
    }
    Ok(match outcome {
        Migration::Moved(0) => Migration::Nothing,
        o => o,
    })
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
            Path::new("/Users/dom"),
            Path::new("/Users/dom/Library/Application Support/com.mimmo8bit.framenote"),
        );
        assert_eq!(
            p.projects_dir,
            PathBuf::from("/Users/dom/FrameNote/Progetti")
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

    #[test]
    fn sposta_i_progetti_da_documenti() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy = tmp.path().join("Documents/FrameNote/Progetti");
        let target = tmp.path().join("FrameNote/Progetti");
        for name in ["2026-10-13_1915_prova-zoom", "2026-10-15_1817_prova-zoom"] {
            fs::create_dir_all(legacy.join(name).join("screen")).unwrap();
            fs::write(legacy.join(name).join("project.json"), name).unwrap();
        }
        assert_eq!(
            migrate_projects(&legacy, &target).unwrap(),
            Migration::Moved(2)
        );
        assert_eq!(
            fs::read_to_string(target.join("2026-10-15_1817_prova-zoom/project.json")).unwrap(),
            "2026-10-15_1817_prova-zoom"
        );
        // Le vecchie cartelle vuote spariscono, Documenti resta.
        assert!(!tmp.path().join("Documents/FrameNote").exists());
        assert!(tmp.path().join("Documents").is_dir());
        // Seconda volta: niente da fare.
        assert_eq!(
            migrate_projects(&legacy, &target).unwrap(),
            Migration::Nothing
        );
    }

    #[test]
    fn spostamento_senza_sovrascrivere() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy = tmp.path().join("Documents/FrameNote/Progetti");
        let target = tmp.path().join("FrameNote/Progetti");
        fs::create_dir_all(legacy.join("a")).unwrap();
        fs::create_dir_all(legacy.join("b")).unwrap();
        fs::create_dir_all(target.join("b")).unwrap();
        fs::write(target.join("b/nuovo"), "nuovo").unwrap();
        assert_eq!(
            migrate_projects(&legacy, &target).unwrap(),
            Migration::Partial { moved: 1, left: 1 }
        );
        assert!(target.join("a").is_dir());
        assert_eq!(fs::read_to_string(target.join("b/nuovo")).unwrap(), "nuovo");
        // Il progetto in conflitto resta dov'era, non viene perso.
        assert!(legacy.join("b").is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn cartelle_private_e_fuori_da_spotlight() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let p = AppPaths::new(None, tmp.path(), &tmp.path().join("data"));
        p.ensure_dirs().unwrap();
        fs::set_permissions(&p.projects_root, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(is_private(&p.projects_root), Some(false));
        assert!(p.protect().is_empty());
        assert_eq!(is_private(&p.projects_root), Some(true));
        assert_eq!(is_private(&p.data_dir), Some(true));
        assert!(p.projects_root.join(NO_INDEX_FILE).is_file());
        assert!(p.data_dir.join(NO_INDEX_FILE).is_file());
    }
}
