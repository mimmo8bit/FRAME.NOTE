//! Registrazione dello schermo principale con ScreenCaptureKit.
//!
//! Thread coinvolti:
//! - coda di ScreenCaptureKit: copia i piani NV12 del frame e li passa al writer
//!   (se il writer è indietro il frame viene scartato e contato);
//! - writer: ritmo costante con il `Pacer`, scrittura verso FFmpeg;
//! - poller: posizione e pulsanti del mouse a 120 Hz → `events.jsonl`;
//! - monitor: CPU di FrameNote + FFmpeg una volta al secondo.
//!
//! Frame ed eventi usano lo stesso orologio (host time di macOS).

use std::ffi::c_void;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use screencapturekit::cm::CMClock;
use screencapturekit::cv::CVPixelBufferLockFlags;
use screencapturekit::prelude::*;

use super::pacer::Pacer;
use super::{CaptureSupport, RecordingReport, CAPTURE_FPS};
use crate::encoder::pipe::capture_encoder_args;
use crate::input_events::{
    macos as mouse, ButtonTracker, DisplayGeometry, EventKind, EventWriter, InputEvent, EVENTS_FILE,
};

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
    fn CGMainDisplayID() -> u32;
    fn CGDisplayCopyDisplayMode(display: u32) -> *mut c_void;
    fn CGDisplayModeGetPixelWidth(mode: *mut c_void) -> usize;
    fn CGDisplayModeGetPixelHeight(mode: *mut c_void) -> usize;
    fn CGDisplayModeRelease(mode: *mut c_void);
}

pub fn has_permission() -> bool {
    // SAFETY: funzione senza argomenti né effetti collaterali.
    unsafe { CGPreflightScreenCaptureAccess() }
}

/// Mostra la richiesta di sistema (solo la prima volta; poi va abilitato a mano).
pub fn request_permission() -> bool {
    // SAFETY: funzione senza argomenti.
    unsafe { CGRequestScreenCaptureAccess() }
}

pub fn support() -> CaptureSupport {
    let granted = has_permission();
    CaptureSupport {
        supported: true,
        permission_granted: granted,
        message: (!granted).then(|| {
            "Per registrare, FrameNote ha bisogno del permesso «Registrazione schermo». Concedilo e poi riapri l'app.".into()
        }),
    }
}

fn host_time_ns(clock: &CMClock) -> u64 {
    clock
        .time()
        .as_seconds()
        .map(|s| (s * 1e9) as u64)
        .unwrap_or(0)
}

fn pixel_size(display_id: u32) -> Option<(u32, u32)> {
    // SAFETY: il modo restituito da CGDisplayCopyDisplayMode viene rilasciato
    // con CGDisplayModeRelease dopo la lettura.
    unsafe {
        let mode = CGDisplayCopyDisplayMode(display_id);
        if mode.is_null() {
            return None;
        }
        let w = CGDisplayModeGetPixelWidth(mode) as u32;
        let h = CGDisplayModeGetPixelHeight(mode) as u32;
        CGDisplayModeRelease(mode);
        Some((w, h))
    }
}

enum Msg {
    Frame(u64, Vec<u8>),
    End,
}

#[derive(Default)]
struct Shared {
    t0_ns: AtomicU64,
    stop_ns: AtomicU64,
    /// Da qui in poi i frame in arrivo si ignorano (non sono "persi").
    stopping: AtomicBool,
    received: AtomicU64,
    idle: AtomicU64,
    dropped: AtomicU64,
    written: AtomicU64,
    clicks: AtomicU64,
    mouse_events: AtomicU64,
    size_mismatch: AtomicU32,
    failed: Mutex<Option<String>>,
}

impl Shared {
    fn fail(&self, msg: String) {
        tracing::error!("registrazione: {msg}");
        let mut f = self.failed.lock().expect("lock errore");
        f.get_or_insert(msg);
    }
}

type Pool = Arc<Mutex<Vec<Vec<u8>>>>;

pub struct ActiveRecording {
    stream: SCStream,
    tx: SyncSender<Msg>,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    writer: Option<JoinHandle<Result<WriterStats, String>>>,
    poller: Option<JoinHandle<Result<(), String>>>,
    monitor: Option<JoinHandle<CpuStats>>,
    started: Instant,
    pub geometry: DisplayGeometry,
    pub width: u32,
    pub height: u32,
    pub encoder: String,
}

pub struct RecordingOutcome {
    pub geometry: DisplayGeometry,
    pub width: u32,
    pub height: u32,
    pub report: RecordingReport,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveStatus {
    pub elapsed_ms: u64,
    pub frames: u64,
    pub clicks: u64,
    pub dropped: u64,
}

struct WriterStats {
    written: u64,
    repeated: u64,
}

#[derive(Default)]
struct CpuStats {
    avg: f32,
    max: f32,
}

/// Avvia la registrazione dello schermo principale nella cartella del progetto.
pub fn start(project_dir: &Path, ffmpeg: &Path, encoder: &str) -> Result<ActiveRecording, String> {
    if !has_permission() {
        return Err("Manca il permesso «Registrazione schermo».".into());
    }
    let content = SCShareableContent::get()
        .map_err(|e| format!("non riesco a leggere gli schermi disponibili ({e})"))?;
    // SAFETY: funzione senza argomenti.
    let main_id = unsafe { CGMainDisplayID() };
    let displays = content.displays();
    let display = displays
        .iter()
        .find(|d| d.display_id() == main_id)
        .or_else(|| displays.first())
        .ok_or("nessuno schermo da registrare")?;
    let frame = display.frame();
    let (px_w, px_h) =
        pixel_size(display.display_id()).unwrap_or((display.width(), display.height()));
    // H.264 4:2:0 vuole dimensioni pari.
    let (width, height) = (px_w & !1, px_h & !1);
    let geometry = DisplayGeometry {
        display_id: display.display_id(),
        origin_x_pt: frame.origin.x,
        origin_y_pt: frame.origin.y,
        width_pt: frame.size.width,
        height_pt: frame.size.height,
        scale: px_w as f64 / frame.size.width,
    };
    tracing::info!(
        display = geometry.display_id,
        pixels = %format!("{width}x{height}"),
        scale = geometry.scale,
        "registrazione: schermo scelto"
    );

    // Le finestre di FrameNote non finiscono nel video.
    let me = std::process::id() as i32;
    let apps = content.applications();
    let own: Vec<&SCRunningApplication> = apps.iter().filter(|a| a.process_id() == me).collect();
    // I clic sulle finestre di FrameNote (che nel video non si vedono, per
    // esempio su «Ferma») non devono generare zoom.
    let own_windows: Vec<Rect> = content
        .windows()
        .iter()
        .filter(|w| {
            w.is_on_screen() && w.owning_application().is_some_and(|a| a.process_id() == me)
        })
        .map(|w| {
            let f = w.frame();
            Rect {
                x: f.origin.x,
                y: f.origin.y,
                w: f.size.width,
                h: f.size.height,
            }
        })
        .collect();
    tracing::info!(
        finestre = own_windows.len(),
        "registrazione: finestre di FrameNote escluse"
    );
    let filter = SCContentFilter::create()
        .with_display(display)
        .with_excluding_applications(&own, &[])
        .build()
        .map_err(|e| format!("filtro di cattura non valido ({e})"))?;
    let config = SCStreamConfiguration::new()
        .with_width(width)
        .with_height(height)
        .with_pixel_format(PixelFormat::YCbCr_420v)
        .with_shows_cursor(false)
        .with_minimum_frame_interval(&CMTime::new(1, CAPTURE_FPS as i32))
        .with_queue_depth(6);

    let shared = Arc::new(Shared::default());
    let frame_len = (width * height * 3 / 2) as usize;
    let pool: Pool = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = sync_channel::<Msg>(4);

    // Writer + FFmpeg.
    let logs = project_dir.join("logs");
    let output = project_dir.join(super::SCREEN_FILE);
    let child = Command::new(ffmpeg)
        .args(capture_encoder_args(
            width,
            height,
            CAPTURE_FPS,
            encoder,
            &output,
        ))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(
            File::create(logs.join("ffmpeg-capture.log"))
                .map_err(|e| format!("non riesco a creare il log ({e})"))?,
        )
        .spawn()
        .map_err(|e| format!("FFmpeg non si avvia ({e})"))?;
    let writer = {
        let shared = shared.clone();
        let pool = pool.clone();
        let log = logs.join("ffmpeg-capture.log");
        std::thread::Builder::new()
            .name("framenote-writer".into())
            .spawn(move || writer_loop(child, rx, shared, pool, log))
            .map_err(|e| e.to_string())?
    };

    // Callback di ScreenCaptureKit.
    let handler = {
        let shared = shared.clone();
        let pool = pool.clone();
        let tx = tx.clone();
        move |sample: CMSampleBuffer, of_type: SCStreamOutputType| {
            if !matches!(of_type, SCStreamOutputType::Screen)
                || shared.stopping.load(Ordering::Relaxed)
            {
                return;
            }
            let has_content = sample.frame_status().is_none_or(|s| s.is_complete());
            if !has_content {
                shared.idle.fetch_add(1, Ordering::Relaxed);
                return;
            }
            let Some(pts) = sample.presentation_timestamp().as_seconds() else {
                return;
            };
            let Some(pb) = sample.pixel_buffer() else {
                shared.idle.fetch_add(1, Ordering::Relaxed);
                return;
            };
            let Ok(guard) = pb.lock(CVPixelBufferLockFlags::READ_ONLY) else {
                return;
            };
            if guard.width() as u32 != width
                || guard.height() as u32 != height
                || guard.plane_count() != 2
            {
                if shared.size_mismatch.fetch_add(1, Ordering::Relaxed) == 0 {
                    tracing::warn!(
                        got = %format!("{}x{} piani={}", guard.width(), guard.height(), guard.plane_count()),
                        "registrazione: frame con formato inatteso, scartato"
                    );
                }
                return;
            }
            let mut buf = pool
                .lock()
                .ok()
                .and_then(|mut p| p.pop())
                .unwrap_or_else(|| vec![0u8; frame_len]);
            let w = width as usize;
            let mut off = 0;
            // Piano Y (altezza righe) e piano UV (altezza/2 righe), senza padding.
            for (plane, rows) in [(0usize, height as usize), (1, height as usize / 2)] {
                for r in 0..rows {
                    // SAFETY: il buffer è bloccato in lettura per tutta la copia.
                    let Some(row) = (unsafe { guard.plane_row(plane, r) }) else {
                        return;
                    };
                    buf[off..off + w].copy_from_slice(&row[..w]);
                    off += w;
                }
            }
            drop(guard);
            shared.received.fetch_add(1, Ordering::Relaxed);
            match tx.try_send(Msg::Frame((pts * 1e9) as u64, buf)) {
                Ok(()) => {}
                Err(TrySendError::Full(Msg::Frame(_, b)))
                | Err(TrySendError::Disconnected(Msg::Frame(_, b))) => {
                    shared.dropped.fetch_add(1, Ordering::Relaxed);
                    if let Ok(mut p) = pool.lock() {
                        p.push(b);
                    }
                }
                Err(_) => {}
            }
        }
    };

    let mut stream = SCStream::new(&filter, &config)
        .map_err(|e| format!("non riesco a preparare la cattura ({e})"))?;
    stream
        .add_output_handler(handler, SCStreamOutputType::Screen)
        .map_err(|e| format!("non riesco a ricevere i frame ({e})"))?;

    stream
        .start_capture()
        .map_err(|e| format!("la cattura non parte ({e})"))?;
    tracing::info!(encoder, "registrazione avviata");

    let stop = Arc::new(AtomicBool::new(false));
    let poller = {
        let (stop, shared) = (stop.clone(), shared.clone());
        let path = project_dir.join(EVENTS_FILE);
        std::thread::Builder::new()
            .name("framenote-mouse".into())
            .spawn(move || poller_loop(&path, geometry, &own_windows, stop, shared))
            .map_err(|e| e.to_string())?
    };
    let monitor = {
        let stop = stop.clone();
        std::thread::Builder::new()
            .name("framenote-cpu".into())
            .spawn(move || cpu_monitor(stop))
            .map_err(|e| e.to_string())?
    };

    Ok(ActiveRecording {
        stream,
        tx,
        shared,
        stop,
        writer: Some(writer),
        poller: Some(poller),
        monitor: Some(monitor),
        started: Instant::now(),
        geometry,
        width,
        height,
        encoder: encoder.to_string(),
    })
}

fn writer_loop(
    mut child: Child,
    rx: Receiver<Msg>,
    shared: Arc<Shared>,
    pool: Pool,
    log: PathBuf,
) -> Result<WriterStats, String> {
    let mut stdin = child.stdin.take().expect("stdin di FFmpeg");
    let mut pacer = Pacer::new(CAPTURE_FPS);
    let mut last: Option<Vec<u8>> = None;
    let mut repeated = 0u64;
    let mut error: Option<String> = None;
    let mut write = |data: &[u8], error: &mut Option<String>| {
        if error.is_none() {
            if let Err(e) = stdin.write_all(data) {
                *error = Some(format!("FFmpeg ha interrotto la codifica ({e})"));
            } else {
                shared.written.fetch_add(1, Ordering::Relaxed);
            }
        }
    };
    for msg in rx {
        match msg {
            Msg::Frame(t_ns, buf) => {
                if pacer.t0_ns().is_none() {
                    shared.t0_ns.store(t_ns, Ordering::SeqCst);
                }
                let action = pacer.on_frame(t_ns);
                if let Some(prev) = &last {
                    for _ in 0..action.repeat_previous {
                        write(prev, &mut error);
                    }
                    repeated += action.repeat_previous;
                }
                if action.write {
                    write(&buf, &mut error);
                    if let Some(old) = last.replace(buf) {
                        if let Ok(mut p) = pool.lock() {
                            p.push(old);
                        }
                    }
                } else if let Ok(mut p) = pool.lock() {
                    p.push(buf);
                }
                if let Some(e) = &error {
                    shared.fail(e.clone());
                }
            }
            Msg::End => break,
        }
    }
    // Coda fino all'istante di stop.
    let tail = pacer.on_stop(shared.stop_ns.load(Ordering::SeqCst));
    if let Some(prev) = &last {
        for _ in 0..tail {
            write(prev, &mut error);
        }
        repeated += tail;
    }
    drop(write);
    drop(stdin);
    let status = child.wait().map_err(|e| e.to_string())?;
    if let Some(e) = error {
        return Err(e);
    }
    if !status.success() {
        let msg = std::fs::read_to_string(&log).unwrap_or_default();
        let last_line = msg
            .lines()
            .last()
            .unwrap_or("errore sconosciuto")
            .to_string();
        return Err(format!("la codifica non è riuscita: {last_line}"));
    }
    Ok(WriterStats {
        written: pacer.written(),
        repeated,
    })
}

fn poller_loop(
    path: &Path,
    geometry: DisplayGeometry,
    own_windows: &[Rect],
    stop: Arc<AtomicBool>,
    shared: Arc<Shared>,
) -> Result<(), String> {
    let clock = CMClock::host_time_clock();
    let mut out = EventWriter::create(path).map_err(|e| e.to_string())?;
    let mut buttons = ButtonTracker::default();
    let mut last_pos: Option<(f64, f64)> = None;
    let mut last_flush = Instant::now();
    let period = Duration::from_micros(1_000_000 / 120);
    while !stop.load(Ordering::Relaxed) {
        let tick = Instant::now();
        let t0 = shared.t0_ns.load(Ordering::SeqCst);
        let now = host_time_ns(&clock);
        if t0 != 0 && now >= t0 {
            let t_ms = (now - t0) / 1_000_000;
            if let Some((xp, yp)) = mouse::cursor_location() {
                if geometry.contains_point(xp, yp) {
                    let (x, y) = geometry.to_pixels(xp, yp);
                    let ev = |kind, button| InputEvent {
                        t_ms,
                        kind,
                        x,
                        y,
                        button,
                        display: geometry.display_id,
                    };
                    let moved = last_pos
                        .is_none_or(|(lx, ly)| (lx - x).abs() > 0.25 || (ly - y).abs() > 0.25);
                    if moved {
                        out.write(&ev(EventKind::Move, None))
                            .map_err(|e| e.to_string())?;
                        last_pos = Some((x, y));
                    }
                    let on_framenote = point_in_rects(xp, yp, own_windows);
                    for (button, kind) in buttons.update(mouse::buttons()) {
                        if on_framenote {
                            continue;
                        }
                        out.write(&ev(kind, Some(button)))
                            .map_err(|e| e.to_string())?;
                        if kind == EventKind::Click {
                            shared.clicks.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    shared.mouse_events.store(out.written, Ordering::Relaxed);
                } else {
                    // Fuori dallo schermo registrato: aggiorna solo lo stato dei pulsanti.
                    buttons.update(mouse::buttons());
                }
            }
            if last_flush.elapsed() >= Duration::from_secs(1) {
                out.flush().map_err(|e| e.to_string())?;
                last_flush = Instant::now();
            }
        }
        if let Some(rest) = period.checked_sub(tick.elapsed()) {
            std::thread::sleep(rest);
        }
    }
    out.finish().map_err(|e| e.to_string())
}

fn cpu_monitor(stop: Arc<AtomicBool>) -> CpuStats {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let mut sys = System::new();
    let me = Pid::from_u32(std::process::id());
    let mut samples = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_secs(1));
        sys.refresh_processes(ProcessesToUpdate::All, true);
        // FrameNote + i suoi processi figli (FFmpeg).
        let total: f32 = sys
            .processes()
            .values()
            .filter(|p| p.pid() == me || p.parent() == Some(me))
            .map(|p| p.cpu_usage())
            .sum();
        samples.push(total);
    }
    // Il primo campione non ha un intervallo di riferimento: si scarta.
    let s = if samples.len() > 1 {
        &samples[1..]
    } else {
        &samples[..]
    };
    CpuStats {
        avg: if s.is_empty() {
            0.0
        } else {
            s.iter().sum::<f32>() / s.len() as f32
        },
        max: s.iter().copied().fold(0.0, f32::max),
    }
}

impl ActiveRecording {
    pub fn status(&self) -> LiveStatus {
        LiveStatus {
            elapsed_ms: self.started.elapsed().as_millis() as u64,
            frames: self.shared.written.load(Ordering::Relaxed),
            clicks: self.shared.clicks.load(Ordering::Relaxed),
            dropped: self.shared.dropped.load(Ordering::Relaxed),
        }
    }

    pub fn error(&self) -> Option<String> {
        self.shared.failed.lock().ok().and_then(|f| f.clone())
    }

    pub fn stop(mut self) -> Result<RecordingOutcome, String> {
        self.shared.stopping.store(true, Ordering::SeqCst);
        self.shared
            .stop_ns
            .store(host_time_ns(&CMClock::host_time_clock()), Ordering::SeqCst);
        if let Err(e) = self.stream.stop_capture() {
            tracing::warn!("stop della cattura: {e}");
        }
        let _ = self.tx.send(Msg::End);
        let writer = self
            .writer
            .take()
            .expect("writer")
            .join()
            .map_err(|_| "il writer si è interrotto".to_string())?;
        self.stop.store(true, Ordering::Relaxed);
        let poller = self.poller.take().expect("poller").join();
        let cpu = self
            .monitor
            .take()
            .expect("monitor")
            .join()
            .unwrap_or_default();
        let w = writer?;
        if let Ok(Err(e)) = poller {
            tracing::warn!("eventi del mouse incompleti: {e}");
        }
        if w.written == 0 {
            return Err("Non è arrivato nessun frame dallo schermo. Controlla il permesso «Registrazione schermo» e riprova.".into());
        }
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get()) as f32;
        let s = &self.shared;
        let report = RecordingReport {
            duration_ms: w.written * 1000 / CAPTURE_FPS as u64,
            frames_received: s.received.load(Ordering::Relaxed),
            frames_idle: s.idle.load(Ordering::Relaxed),
            frames_dropped: s.dropped.load(Ordering::Relaxed),
            frames_written: w.written,
            frames_repeated: w.repeated,
            mouse_events: s.mouse_events.load(Ordering::Relaxed),
            clicks: s.clicks.load(Ordering::Relaxed),
            cpu_avg_percent: cpu.avg,
            cpu_max_percent: cpu.max,
            cpu_avg_total_percent: cpu.avg / cores,
            encoder: self.encoder.clone(),
        };
        tracing::info!(?report, "registrazione conclusa");
        Ok(RecordingOutcome {
            geometry: self.geometry,
            width: self.width,
            height: self.height,
            report,
        })
    }
}
