//! Comandi della schermata "Prova zoom (M1)": registrazione, calcolo degli
//! zoom ed export. Gli errori sono frasi per Domenico; i dettagli vanno nei log.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::capture::{self, RecordingReport};
use crate::encoder::ffmpeg;
use crate::input_events::{read_events, EventKind};
use crate::project::{ExportRecord, Project};
use crate::render::export::{export, ExportJob};
use crate::render::zoom::{plan_zooms, Click, ZoomBlock};
use crate::state::AppState;
#[cfg(target_os = "macos")]
use crate::{
    input_events::EVENTS_FILE,
    project::{self, ScreenTrack},
};

pub const PROJECT_TITLE: &str = "Prova zoom";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatus {
    pub supported: bool,
    pub permission_granted: bool,
    pub message: Option<String>,
    pub recording: bool,
}

#[tauri::command]
pub fn capture_status(state: State<'_, AppState>) -> CaptureStatus {
    let s = capture_support();
    CaptureStatus {
        supported: s.supported,
        permission_granted: s.permission_granted,
        message: s.message,
        recording: state.is_recording(),
    }
}

fn capture_support() -> capture::CaptureSupport {
    #[cfg(target_os = "macos")]
    {
        capture::macos::support()
    }
    #[cfg(not(target_os = "macos"))]
    {
        capture::support()
    }
}

#[tauri::command]
pub fn request_screen_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        let granted = capture::macos::request_permission();
        tracing::info!(granted, "richiesta permesso registrazione schermo");
        granted
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Apre il pannello di sistema del permesso di registrazione schermo.
#[tauri::command]
pub fn open_screen_permission_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Non riesco ad aprire Impostazioni di Sistema ({e})."))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Disponibile solo su Mac.".into())
    }
}

/// Encoder H.264 da usare: il preferito della diagnostica (prova reale).
fn preferred_encoder(state: &AppState) -> Result<String, String> {
    if let Some(e) = state.encoder.lock().ok().and_then(|e| e.clone()) {
        return Ok(e);
    }
    let path = ffmpeg::sidecar_path().map_err(|e| e.to_string())?;
    let report = ffmpeg::detect(&path, String::new());
    // Per la registrazione serve H.264 (compatibilità ovunque): prima scelta
    // l'encoder preferito se è H.264, altrimenti il primo H.264 funzionante.
    let enc = report
        .encoders
        .iter()
        .filter(|e| e.working && (e.name.starts_with("h264_") || e.name.contains("264")))
        .map(|e| e.name.clone())
        .next()
        .ok_or_else(|| {
            "Nessun encoder video H.264 funzionante: apri «Info e diagnostica» ed esporta la diagnostica.".to_string()
        })?;
    if let Ok(mut slot) = state.encoder.lock() {
        *slot = Some(enc.clone());
    }
    Ok(enc)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStarted {
    pub project_dir: String,
}

#[tauri::command]
pub async fn start_recording(app: AppHandle) -> Result<RecordingStarted, String> {
    tauri::async_runtime::spawn_blocking(move || start_recording_blocking(&app))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(target_os = "macos")]
fn start_recording_blocking(app: &AppHandle) -> Result<RecordingStarted, String> {
    let state = app.state::<AppState>();
    if state.is_recording() {
        return Err("C'è già una registrazione in corso.".into());
    }
    let encoder = preferred_encoder(&state)?;
    let ffmpeg = ffmpeg::sidecar_path().map_err(|e| e.to_string())?;
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let dir = project::create_project_dir(&state.paths.projects_dir, now, PROJECT_TITLE)
        .map_err(|e| format!("Non riesco a creare la cartella del progetto ({e})."))?;
    let proj = Project::new(
        PROJECT_TITLE,
        crate::diagnostics::info::now_rfc3339(),
        &state.meta.version,
    );
    proj.save(&dir).map_err(|e| e.to_string())?;
    let active = capture::macos::start(&dir, &ffmpeg, &encoder)?;
    let shown = dir.display().to_string();
    *state.recording.lock().map_err(|e| e.to_string())? = Some(crate::state::Recording {
        project_dir: dir,
        active,
    });
    Ok(RecordingStarted { project_dir: shown })
}

#[cfg(not(target_os = "macos"))]
fn start_recording_blocking(_app: &AppHandle) -> Result<RecordingStarted, String> {
    Err(capture::support().message.unwrap_or_default())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStatus {
    pub recording: bool,
    pub elapsed_ms: u64,
    pub frames: u64,
    pub clicks: u64,
    pub dropped: u64,
    pub error: Option<String>,
}

#[tauri::command]
pub fn recording_status(state: State<'_, AppState>) -> RecordingStatus {
    #[cfg(target_os = "macos")]
    if let Ok(guard) = state.recording.lock() {
        if let Some(r) = guard.as_ref() {
            let s = r.active.status();
            return RecordingStatus {
                recording: true,
                elapsed_ms: s.elapsed_ms,
                frames: s.frames,
                clicks: s.clicks,
                dropped: s.dropped,
                error: r.active.error(),
            };
        }
    }
    let _ = &state;
    RecordingStatus {
        recording: false,
        elapsed_ms: 0,
        frames: 0,
        clicks: 0,
        dropped: 0,
        error: None,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingSummary {
    pub project_dir: String,
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub clicks: usize,
    pub zoom_blocks: Vec<ZoomBlock>,
    pub report: RecordingReport,
}

#[tauri::command]
pub async fn stop_recording(app: AppHandle) -> Result<RecordingSummary, String> {
    tauri::async_runtime::spawn_blocking(move || stop_recording_blocking(&app))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(target_os = "macos")]
fn stop_recording_blocking(app: &AppHandle) -> Result<RecordingSummary, String> {
    let state = app.state::<AppState>();
    let rec = state
        .recording
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("Nessuna registrazione in corso.")?;
    let dir = rec.project_dir;
    let outcome = rec.active.stop()?;
    let mut proj = Project::load(&dir)?;
    proj.screen = Some(ScreenTrack {
        file: capture::SCREEN_FILE.into(),
        width: outcome.width,
        height: outcome.height,
        fps: capture::CAPTURE_FPS,
        duration_ms: outcome.report.duration_ms,
        display: outcome.geometry,
        events_file: EVENTS_FILE.into(),
    });
    proj.recording_report = Some(outcome.report);
    plan_project_zooms(&dir, &mut proj)?;
    proj.save(&dir).map_err(|e| e.to_string())?;
    summary(&dir, &proj)
}

#[cfg(not(target_os = "macos"))]
fn stop_recording_blocking(_app: &AppHandle) -> Result<RecordingSummary, String> {
    Err("Nessuna registrazione in corso.".into())
}

/// Calcola i blocchi zoom dai clic registrati e li salva nel progetto.
fn plan_project_zooms(dir: &Path, proj: &mut Project) -> Result<(), String> {
    let screen = proj
        .screen
        .as_ref()
        .ok_or("Il progetto non ha una registrazione.")?;
    let read = read_events(&dir.join(&screen.events_file))
        .map_err(|e| format!("non riesco a leggere gli eventi del mouse ({e})"))?;
    if read.skipped_lines > 0 {
        tracing::warn!(
            righe = read.skipped_lines,
            "events.jsonl: righe illeggibili saltate"
        );
    }
    let clicks: Vec<Click> = read
        .events
        .iter()
        .filter(|e| e.kind == EventKind::Click)
        .map(|e| Click {
            t_ms: e.t_ms as f64,
            x: e.x,
            y: e.y,
        })
        .collect();
    proj.zoom.blocks = plan_zooms(
        &clicks,
        screen.width as f64,
        screen.height as f64,
        screen.duration_ms as f64,
        &proj.zoom.settings,
    );
    tracing::info!(
        clic = clicks.len(),
        zoom = proj.zoom.blocks.len(),
        "zoom calcolati"
    );
    Ok(())
}

fn summary(dir: &Path, proj: &Project) -> Result<RecordingSummary, String> {
    let screen = proj
        .screen
        .as_ref()
        .ok_or("Il progetto non ha una registrazione.")?;
    let report = proj.recording_report.clone().unwrap_or_default();
    Ok(RecordingSummary {
        project_dir: dir.display().to_string(),
        duration_ms: screen.duration_ms,
        width: screen.width,
        height: screen.height,
        clicks: proj.zoom.blocks.iter().map(|b| b.clicks).sum(),
        zoom_blocks: proj.zoom.blocks.clone(),
        report,
    })
}

/// Accetta solo cartelle dentro `Progetti/`.
fn checked_project_dir(state: &AppState, dir: &str) -> Result<PathBuf, String> {
    let root = state
        .paths
        .projects_dir
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let dir = PathBuf::from(dir)
        .canonicalize()
        .map_err(|_| "La cartella del progetto non esiste più.".to_string())?;
    if !dir.starts_with(&root) {
        return Err("Cartella non valida.".into());
    }
    Ok(dir)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgress {
    pub done: u64,
    pub total: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub frames: u64,
    pub render_ms: u64,
}

pub const EXPORT_FILE: &str = "exports/zoom.mp4";

#[tauri::command]
pub async fn export_zoom(app: AppHandle, project_dir: String) -> Result<ExportSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let dir = checked_project_dir(&state, &project_dir)?;
        let mut proj = Project::load(&dir)?;
        let screen = proj
            .screen
            .clone()
            .ok_or("Il progetto non ha una registrazione.")?;
        let events = read_events(&dir.join(&screen.events_file))
            .map(|r| r.events)
            .unwrap_or_default();
        let encoder = preferred_encoder(&state)?;
        let ffmpeg = ffmpeg::sidecar_path().map_err(|e| e.to_string())?;
        let output = dir.join(EXPORT_FILE);
        state.export_cancel.store(false, Ordering::SeqCst);
        let app2 = app.clone();
        let mut progress = move |done: u64, total: u64| {
            let _ = app2.emit("zoom-export-progress", ExportProgress { done, total });
        };
        let stats = export(
            &ExportJob {
                ffmpeg: &ffmpeg,
                input: &dir.join(&screen.file),
                src_width: screen.width,
                src_height: screen.height,
                fps: screen.fps,
                duration_ms: screen.duration_ms,
                display_scale: screen.display.scale,
                events: &events,
                zoom_blocks: &proj.zoom.blocks,
                zoom_settings: &proj.zoom.settings,
                cursor_settings: &proj.cursor,
                output: &output,
                encoder: &encoder,
                log_dir: &dir.join("logs"),
            },
            &mut progress,
            &state.export_cancel,
        )
        .map_err(|e| format!("Esportazione non riuscita: {e}"))?;
        proj.exports.push(ExportRecord {
            file: EXPORT_FILE.into(),
            width: stats.width,
            height: stats.height,
            fps: screen.fps,
            frames: stats.frames,
            duration_ms: stats.frames * 1000 / screen.fps as u64,
            render_ms: stats.render_ms,
            encoder: encoder.clone(),
            created_at: crate::diagnostics::info::now_rfc3339(),
        });
        proj.save(&dir).map_err(|e| e.to_string())?;
        Ok(ExportSummary {
            output: output.display().to_string(),
            width: stats.width,
            height: stats.height,
            frames: stats.frames,
            render_ms: stats.render_ms,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn cancel_export(state: State<'_, AppState>) {
    state.export_cancel.store(true, Ordering::SeqCst);
}

/// Apre un file (per esempio il video esportato) con l'app predefinita.
#[tauri::command]
pub fn open_project_file(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let root = state
        .paths
        .projects_dir
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let p = PathBuf::from(&path)
        .canonicalize()
        .map_err(|_| "Il file non esiste più.".to_string())?;
    if !p.starts_with(&root) {
        return Err("File non valido.".into());
    }
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(&p)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Non riesco ad aprire il file ({e})."))
}
