//! Cartella di progetto (§4) e `project.json`.
//!
//! Il montaggio è non distruttivo: le tracce registrate non si riscrivono mai,
//! `project.json` descrive come comporle (zoom, cursore) e cosa è stato esportato.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::capture::RecordingReport;
use crate::input_events::DisplayGeometry;
use crate::render::cursor::CursorSettings;
use crate::render::zoom::{ZoomBlock, ZoomSettings};

pub const PROJECT_FILE: &str = "project.json";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub schema_version: u32,
    pub title: String,
    pub created_at: String,
    pub app_version: String,
    pub screen: Option<ScreenTrack>,
    pub zoom: ZoomConfig,
    pub cursor: CursorSettings,
    pub recording_report: Option<RecordingReport>,
    #[serde(default)]
    pub exports: Vec<ExportRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenTrack {
    /// Percorso relativo alla cartella del progetto.
    pub file: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration_ms: u64,
    pub display: DisplayGeometry,
    pub events_file: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomConfig {
    pub settings: ZoomSettings,
    /// Blocchi zoom generati (in M7 modificabili a mano).
    pub blocks: Vec<ZoomBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRecord {
    pub file: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub frames: u64,
    pub duration_ms: u64,
    pub render_ms: u64,
    pub encoder: String,
    pub created_at: String,
}

impl Project {
    pub fn new(title: &str, created_at: String, app_version: &str) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            title: title.to_string(),
            created_at,
            app_version: app_version.to_string(),
            screen: None,
            zoom: ZoomConfig::default(),
            cursor: CursorSettings::default(),
            recording_report: None,
            exports: Vec::new(),
        }
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join(PROJECT_FILE);
        let text = fs::read_to_string(&path)
            .map_err(|e| format!("non riesco a leggere project.json ({e})"))?;
        let p: Project =
            serde_json::from_str(&text).map_err(|e| format!("project.json non valido ({e})"))?;
        if p.schema_version > SCHEMA_VERSION {
            return Err(format!(
                "il progetto è stato creato da una versione più recente di FrameNote (schema v{})",
                p.schema_version
            ));
        }
        Ok(p)
    }

    /// Salvataggio atomico: file temporaneo, fsync, rinomina.
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let tmp = dir.join(".project.json.tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(&serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, dir.join(PROJECT_FILE))
    }
}

/// "Prova zoom!" → "prova-zoom".
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    for ch in title.chars() {
        let mapped = match ch {
            'à' | 'á' | 'â' | 'ä' | 'À' | 'Á' => 'a',
            'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' | 'Ò' | 'Ó' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' => 'u',
            c if c.is_ascii_alphanumeric() => c.to_ascii_lowercase(),
            _ => '-',
        };
        if mapped == '-' && (out.is_empty() || out.ends_with('-')) {
            continue;
        }
        out.push(mapped);
    }
    let out = out
        .trim_end_matches('-')
        .chars()
        .take(40)
        .collect::<String>();
    if out.is_empty() {
        "progetto".into()
    } else {
        out
    }
}

/// Crea `Progetti/AAAA-MM-GG_HHMM_titolo/` con le sottocartelle; se esiste già
/// aggiunge `-2`, `-3`… Non sovrascrive mai un progetto.
pub fn create_project_dir(
    projects_dir: &Path,
    now: time::OffsetDateTime,
    title: &str,
) -> std::io::Result<PathBuf> {
    let fmt = time::macros::format_description!("[year]-[month]-[day]_[hour][minute]");
    let stamp = now.format(&fmt).map_err(std::io::Error::other)?;
    let base = format!("{stamp}_{}", slugify(title));
    let mut dir = projects_dir.join(&base);
    let mut n = 2;
    while dir.exists() {
        dir = projects_dir.join(format!("{base}-{n}"));
        n += 1;
    }
    fs::create_dir_all(&dir)?;
    for sub in ["screen", "exports", "logs"] {
        fs::create_dir_all(dir.join(sub))?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug() {
        assert_eq!(slugify("Prova zoom"), "prova-zoom");
        assert_eq!(slugify("  Call con Città!! "), "call-con-citta");
        assert_eq!(slugify("???"), "progetto");
    }

    #[test]
    fn cartella_progetto_mai_sovrascritta() {
        let tmp = tempfile::tempdir().unwrap();
        let now = time::macros::datetime!(2026-10-13 19:15 UTC);
        let a = create_project_dir(tmp.path(), now, "Prova zoom").unwrap();
        let b = create_project_dir(tmp.path(), now, "Prova zoom").unwrap();
        assert_eq!(a.file_name().unwrap(), "2026-10-13_1915_prova-zoom");
        assert_eq!(b.file_name().unwrap(), "2026-10-13_1915_prova-zoom-2");
        assert!(a.join("screen").is_dir() && a.join("exports").is_dir());
    }

    #[test]
    fn salvataggio_e_lettura() {
        let tmp = tempfile::tempdir().unwrap();
        let mut p = Project::new("Prova zoom", "2026-10-13T19:15:00+02:00".into(), "0.1.0");
        p.zoom.blocks.push(ZoomBlock {
            start_ms: 1.0,
            end_ms: 2.0,
            scale: 2.0,
            center_x: 3.0,
            center_y: 4.0,
            clicks: 1,
        });
        p.save(tmp.path()).unwrap();
        assert!(!tmp.path().join(".project.json.tmp").exists());
        let q = Project::load(tmp.path()).unwrap();
        assert_eq!(p, q);
        let text = std::fs::read_to_string(tmp.path().join(PROJECT_FILE)).unwrap();
        assert!(text.contains("\"schemaVersion\": 1"));
    }

    #[test]
    fn schema_piu_recente_rifiutato() {
        let tmp = tempfile::tempdir().unwrap();
        let mut p = Project::new("x", String::new(), "0.1.0");
        p.schema_version = 99;
        p.save(tmp.path()).unwrap();
        assert!(Project::load(tmp.path())
            .unwrap_err()
            .contains("più recente"));
    }
}
