//! Cattura dello schermo. In M1 solo macOS (ScreenCaptureKit); la cattura
//! Windows arriva in M2.

#[cfg(target_os = "macos")]
pub mod macos;
pub mod pacer;

use serde::{Deserialize, Serialize};

pub const CAPTURE_FPS: u32 = 30;
pub const SCREEN_FILE: &str = "screen/capture.mp4";

/// Numeri della registrazione salvati in `project.json` (modalità auto-test).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingReport {
    pub duration_ms: u64,
    /// Frame con contenuto consegnati da ScreenCaptureKit.
    pub frames_received: u64,
    /// Notifiche senza contenuto nuovo (schermo fermo).
    pub frames_idle: u64,
    /// Frame scartati perché la codifica non teneva il passo.
    pub frames_dropped: u64,
    /// Frame scritti nel video (frame rate costante).
    pub frames_written: u64,
    /// Di questi, ripetizioni del frame precedente.
    pub frames_repeated: u64,
    pub mouse_events: u64,
    pub clicks: u64,
    /// CPU media e massima di FrameNote + FFmpeg, in % di un core.
    pub cpu_avg_percent: f32,
    pub cpu_max_percent: f32,
    /// La stessa media in % della CPU totale della macchina.
    pub cpu_avg_total_percent: f32,
    pub encoder: String,
}

/// Stato del supporto alla registrazione su questa piattaforma.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSupport {
    pub supported: bool,
    /// Permesso di registrazione schermo (solo macOS).
    pub permission_granted: bool,
    pub message: Option<String>,
}

#[cfg(not(target_os = "macos"))]
pub fn support() -> CaptureSupport {
    CaptureSupport {
        supported: false,
        permission_granted: false,
        message: Some(
            "Su questo sistema la registrazione arriva in una prossima versione: la prova degli zoom per ora funziona solo su Mac."
                .into(),
        ),
    }
}
