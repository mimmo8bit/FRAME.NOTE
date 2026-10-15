//! "Esporta diagnostica": uno .zip da allegare in chat con `info.json`,
//! `privacy.json`, i log e il `project.json` dell'ultimo progetto, se esiste.
//! Mai video, audio, eventi del mouse o altri file dei progetti.
//!
//! Anonimizzazione: la home diventa `~`, i nomi delle cartelle dei progetti
//! (che possono contenere nomi di clienti) diventano `progetto-1`, `progetto-2`…
//! e dal `project.json` si toglie il titolo. Prima di salvare, l'interfaccia
//! mostra l'elenco esatto dei file (`preview`).

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::diagnostics::logging::log_files;
use crate::paths::anonymize_text;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("impossibile scrivere il file: {0}")]
    Io(#[from] std::io::Error),
    #[error("errore nella creazione dello zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("errore nei dati di diagnostica: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct ExportInput<'a> {
    pub info_json: serde_json::Value,
    pub privacy_json: Option<serde_json::Value>,
    pub logs_dir: &'a Path,
    pub projects_dir: &'a Path,
    pub home_dir: Option<&'a Path>,
}

/// Aggiunge `.zip` se manca.
pub fn with_zip_extension(path: &Path) -> PathBuf {
    let is_zip = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    if is_zip {
        path.to_path_buf()
    } else {
        let mut s = path.as_os_str().to_owned();
        s.push(".zip");
        PathBuf::from(s)
    }
}

/// `project.json` del progetto più recente (le cartelle iniziano con
/// `AAAA-MM-GG_HHMM`, quindi l'ordine per nome è cronologico).
pub fn latest_project_json(projects_dir: &Path) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(projects_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("project.json").is_file())
        .collect();
    dirs.sort();
    dirs.pop().map(|d| d.join("project.json"))
}

/// Nomi delle cartelle dei progetti e il loro sostituto nella diagnostica
/// (`progetto-1` è il più vecchio). I più lunghi prima, per le sostituzioni.
fn project_aliases(projects_dir: &Path) -> Vec<(String, String)> {
    let mut names: Vec<String> = std::fs::read_dir(projects_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().to_str().map(String::from))
                .filter(|n| n.len() >= 4)
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    let mut aliases: Vec<(String, String)> = names
        .into_iter()
        .enumerate()
        .map(|(i, n)| (n, format!("progetto-{}", i + 1)))
        .collect();
    aliases.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(a.0.cmp(&b.0)));
    aliases
}

/// Home → `~` e nomi dei progetti → `progetto-N`.
fn redact(text: &str, input: &ExportInput, aliases: &[(String, String)]) -> String {
    let mut out = anonymize_text(text, input.home_dir);
    for (name, alias) in aliases {
        out = out.replace(name.as_str(), alias);
    }
    out
}

/// `project.json` senza il titolo (che può contenere il nome del cliente).
fn redact_project(bytes: &[u8]) -> Result<Vec<u8>, ExportError> {
    let mut v: serde_json::Value = serde_json::from_slice(bytes)?;
    if let Some(obj) = v.as_object_mut() {
        obj.remove("title");
    }
    Ok(serde_json::to_vec_pretty(&v)?)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewEntry {
    pub name: String,
    pub description: String,
}

/// Cosa conterrà lo zip, da mostrare prima di salvarlo.
pub fn preview(input: &ExportInput) -> Vec<PreviewEntry> {
    let entry = |name: &str, description: &str| PreviewEntry {
        name: name.into(),
        description: description.into(),
    };
    let mut out = vec![entry(
        "info.json",
        "Dati tecnici: versione, sistema, processore, memoria, scheda grafica, FFmpeg, spazio libero",
    )];
    if input.privacy_json.is_some() {
        out.push(entry(
            "privacy.json",
            "Stato della protezione: FileVault, sincronizzazione, firma, esclusioni",
        ));
    }
    for log in log_files(input.logs_dir) {
        if let Some(name) = log.file_name().and_then(|n| n.to_str()) {
            out.push(entry(
                &format!("logs/{name}"),
                "Registro tecnico dell'app (cartelle e nomi dei progetti anonimizzati)",
            ));
        }
    }
    if latest_project_json(input.projects_dir).is_some() {
        out.push(entry(
            "ultimo-progetto/project.json",
            "Impostazioni e zoom dell'ultima registrazione, senza titolo",
        ));
    }
    out
}

/// Scrive lo zip in `dest` e restituisce l'elenco dei file inclusi.
pub fn write_zip(dest: &Path, input: &ExportInput) -> Result<Vec<String>, ExportError> {
    // Si scrive su un file temporaneo e poi si rinomina: niente zip a metà.
    let tmp = dest.with_extension("zip.part");
    let result = (|| -> Result<Vec<String>, ExportError> {
        let file = File::create(&tmp)?;
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        let mut entries = Vec::new();

        let aliases = project_aliases(input.projects_dir);
        let mut add_json = |zip: &mut ZipWriter<File>, name: &str, v: &serde_json::Value| {
            let text = redact(&serde_json::to_string_pretty(v)?, input, &aliases);
            zip.start_file(name, opts)?;
            zip.write_all(text.as_bytes())?;
            entries.push(name.to_string());
            Ok::<(), ExportError>(())
        };
        add_json(&mut zip, "info.json", &input.info_json)?;
        if let Some(privacy) = &input.privacy_json {
            add_json(&mut zip, "privacy.json", privacy)?;
        }

        for log in log_files(input.logs_dir) {
            let Some(name) = log.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let bytes = std::fs::read(&log)?;
            let text = redact(&String::from_utf8_lossy(&bytes), input, &aliases);
            let entry = format!("logs/{name}");
            zip.start_file(entry.as_str(), opts)?;
            zip.write_all(text.as_bytes())?;
            entries.push(entry);
        }

        if let Some(project_json) = latest_project_json(input.projects_dir) {
            let bytes = redact_project(&std::fs::read(&project_json)?)?;
            let text = redact(&String::from_utf8_lossy(&bytes), input, &aliases);
            zip.start_file("ultimo-progetto/project.json", opts)?;
            zip.write_all(text.as_bytes())?;
            entries.push("ultimo-progetto/project.json".to_string());
        }

        zip.finish()?.sync_all()?;
        Ok(entries)
    })();
    match result {
        Ok(entries) => {
            std::fs::rename(&tmp, dest)?;
            Ok(entries)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Nome proposto nella finestra di salvataggio.
pub fn default_file_name(now: time::OffsetDateTime) -> String {
    let fmt = time::macros::format_description!("[year][month][day]-[hour][minute]");
    format!(
        "framenote-diagnostica-{}.zip",
        now.format(&fmt).unwrap_or_else(|_| "export".into())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn read_zip(path: &Path) -> Vec<(String, String)> {
        let mut zip = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
        (0..zip.len())
            .map(|i| {
                let mut f = zip.by_index(i).unwrap();
                let mut s = String::new();
                f.read_to_string(&mut s).unwrap();
                (f.name().unwrap().to_string(), s)
            })
            .collect()
    }

    #[test]
    fn contenuto_dello_zip() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let logs = home.join("data/logs");
        let projects = home.join("Documents/FrameNote/Progetti");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(
            logs.join("framenote.2026-10-13.log"),
            format!(
                "aperto {} e {}",
                home.join("Documents").display(),
                projects.join("2026-10-13_1530_riunione-rossi").display()
            ),
        )
        .unwrap();
        std::fs::write(logs.join("estraneo.txt"), "no").unwrap();

        let old = projects.join("2026-10-01_0900_vecchio");
        let new = projects.join("2026-10-13_1530_riunione-rossi");
        for (dir, title) in [(&old, "vecchio"), (&new, "Riunione Rossi")] {
            std::fs::create_dir_all(dir.join("screen")).unwrap();
            std::fs::write(
                dir.join("project.json"),
                format!("{{\"title\":\"{title}\",\"schemaVersion\":1}}"),
            )
            .unwrap();
            std::fs::write(dir.join("screen/seg-0001.m4s"), [0u8; 64]).unwrap();
            std::fs::write(dir.join("notes.md"), "note private").unwrap();
        }

        let dest = with_zip_extension(&tmp.path().join("diagnostica"));
        let entries = write_zip(
            &dest,
            &ExportInput {
                info_json: serde_json::json!({"app": {"version": "0.0.1"}}),
                privacy_json: Some(serde_json::json!({"projectsDir": "~/FrameNote/Progetti"})),
                logs_dir: &logs,
                projects_dir: &projects,
                home_dir: Some(&home),
            },
        )
        .unwrap();

        let expected = [
            "info.json",
            "privacy.json",
            "logs/framenote.2026-10-13.log",
            "ultimo-progetto/project.json",
        ];
        assert_eq!(entries, expected);
        let input = ExportInput {
            info_json: serde_json::json!({}),
            privacy_json: Some(serde_json::json!({})),
            logs_dir: &logs,
            projects_dir: &projects,
            home_dir: Some(&home),
        };
        let names: Vec<String> = preview(&input).into_iter().map(|e| e.name).collect();
        assert_eq!(
            names, expected,
            "l'anteprima elenca esattamente i file dello zip"
        );

        let files = read_zip(&dest);
        assert_eq!(files.len(), 4);
        assert!(files[0].1.contains("\"0.0.1\""));
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(
            files[2].1,
            format!("aperto ~{sep}Documents e ~{sep}Documents{sep}FrameNote{sep}Progetti{sep}progetto-2")
        );
        // Né il titolo né il nome della cartella del progetto.
        let all: String = files.iter().map(|(_, t)| t.as_str()).collect();
        assert!(!all.contains("Rossi") && !all.contains("rossi"), "{all}");
        assert!(files[3].1.contains("\"schemaVersion\": 1"));
        // Mai video o note.
        assert!(!files
            .iter()
            .any(|(n, _)| n.contains("m4s") || n.contains("notes")));
        assert!(!tmp.path().join("diagnostica.zip.part").exists());
    }

    #[test]
    fn senza_progetti_ne_log() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("d.zip");
        let entries = write_zip(
            &dest,
            &ExportInput {
                info_json: serde_json::json!({}),
                privacy_json: None,
                logs_dir: &tmp.path().join("nessun-log"),
                projects_dir: &tmp.path().join("nessun-progetto"),
                home_dir: None,
            },
        )
        .unwrap();
        assert_eq!(entries, ["info.json"]);
    }

    #[test]
    fn estensione_e_nome() {
        assert_eq!(
            with_zip_extension(Path::new("a/b")),
            PathBuf::from("a/b.zip")
        );
        assert_eq!(
            with_zip_extension(Path::new("a/b.ZIP")),
            PathBuf::from("a/b.ZIP")
        );
        let t = time::macros::datetime!(2026-10-13 15:07 UTC);
        assert_eq!(
            default_file_name(t),
            "framenote-diagnostica-20261013-1507.zip"
        );
    }
}
