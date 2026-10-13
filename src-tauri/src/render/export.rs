//! Export con zoom: FFmpeg decodifica la sorgente → il compositore GPU applica
//! telecamera, cursore e anello → FFmpeg codifica il video finale.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use super::cursor::{CursorSettings, CursorTrack};
use super::gpu::{Compositor, FrameParams};
use super::sprite::{cursor_sprite, ARROW_HEIGHT_PT};
use super::zoom::{CameraTrack, ZoomBlock, ZoomSettings};
use crate::encoder::pipe::{decoder_args, export_encoder_args, output_size};
use crate::input_events::InputEvent;

pub const MAX_OUTPUT_LONG_SIDE: u32 = 1920;

pub struct ExportJob<'a> {
    pub ffmpeg: &'a Path,
    pub input: &'a Path,
    pub src_width: u32,
    pub src_height: u32,
    pub fps: u32,
    pub duration_ms: u64,
    /// Pixel del video per punto dello schermo (2 sui Retina): serve a dare al
    /// cursore la stessa grandezza che ha sullo schermo.
    pub display_scale: f64,
    pub events: &'a [InputEvent],
    pub zoom_blocks: &'a [ZoomBlock],
    pub zoom_settings: &'a ZoomSettings,
    pub cursor_settings: &'a CursorSettings,
    pub output: &'a Path,
    pub encoder: &'a str,
    /// Dove finiscono i messaggi di FFmpeg (per la diagnostica).
    pub log_dir: &'a Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportStats {
    pub frames: u64,
    pub width: u32,
    pub height: u32,
    pub render_ms: u64,
    pub gpu: String,
}

fn tail(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Parametri del frame all'istante `t_ms`, in pixel di uscita.
pub fn frame_params(
    t_ms: f64,
    camera: &CameraTrack,
    cursor: &CursorTrack,
    src: (f64, f64),
    out_w: f64,
    arrow_height_src: f64,
    sprite_height_ratio: f64,
) -> FrameParams {
    let cam = camera.at(t_ms);
    let (x0, y0, vw, vh) = cam.viewport(src.0, src.1);
    let k = out_w / vw; // pixel di uscita per pixel sorgente
    let mut p = FrameParams {
        viewport: [x0 as f32, y0 as f32, vw as f32, vh as f32],
        ..Default::default()
    };
    if let Some((cx, cy)) = cursor.position(t_ms) {
        p.cursor_xy = [((cx - x0) * k) as f32, ((cy - y0) * k) as f32];
        p.cursor_height = (arrow_height_src * k * sprite_height_ratio) as f32;
        p.cursor_opacity = cursor.opacity(t_ms) as f32;
    }
    if let Some(r) = cursor.ring(t_ms) {
        // L'anello si allarga e svanisce (uscita "ease-out").
        let e = 1.0 - (1.0 - r.progress).powi(3);
        let radius = arrow_height_src * k * (0.45 + 0.75 * e);
        p.ring = [
            ((r.x - x0) * k) as f32,
            ((r.y - y0) * k) as f32,
            radius as f32,
            (1.0 - r.progress) as f32,
        ];
    }
    p
}

pub fn export(
    job: &ExportJob,
    progress: &mut dyn FnMut(u64, u64),
    cancel: &AtomicBool,
) -> Result<ExportStats, String> {
    let started = Instant::now();
    let (sw, sh) = (job.src_width, job.src_height);
    let (ow, oh) = output_size(sw, sh, MAX_OUTPUT_LONG_SIDE);
    let mut gpu = Compositor::new((sw, sh), (ow, oh)).map_err(|e| e.to_string())?;
    tracing::info!(gpu = %gpu.adapter_name, src = %format!("{sw}x{sh}"), out = %format!("{ow}x{oh}"), "export avviato");

    let cursor = CursorTrack::build(job.events, *job.cursor_settings);
    let duration = job.duration_ms as f64;
    let camera = CameraTrack::simulate(
        job.zoom_blocks,
        Some(&cursor),
        sw as f64,
        sh as f64,
        duration,
        job.zoom_settings,
    );
    let sprite = cursor_sprite();
    let sprite_ratio = sprite.height as f64 / sprite.arrow_height as f64;
    let arrow_src = ARROW_HEIGHT_PT as f64 * job.display_scale * job.cursor_settings.size;

    let dec_log = job.log_dir.join("ffmpeg-export-decode.log");
    let enc_log = job.log_dir.join("ffmpeg-export-encode.log");
    let mut decoder = Command::new(job.ffmpeg)
        .args(decoder_args(job.input))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(File::create(&dec_log).map_err(|e| e.to_string())?)
        .spawn()
        .map_err(|e| format!("FFmpeg non si avvia ({e})"))?;
    let part: PathBuf = job.output.with_extension("mp4.part");
    let mut encoder = Command::new(job.ffmpeg)
        .args(export_encoder_args(ow, oh, job.fps, job.encoder, &part))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(File::create(&enc_log).map_err(|e| e.to_string())?)
        .spawn()
        .map_err(|e| format!("FFmpeg non si avvia ({e})"))?;

    let frame_len = (sw * sh * 3 / 2) as usize;
    let expected = (job.duration_ms * job.fps as u64).div_ceil(1000).max(1);
    let mut nv12 = vec![0u8; frame_len];
    let mut rgba = Vec::new();
    let mut frames = 0u64;
    let result: Result<(), String> = (|| {
        let mut src = decoder.stdout.take().expect("stdout del decoder");
        let mut sink = encoder.stdin.take().expect("stdin dell'encoder");
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("esportazione annullata".into());
            }
            match src.read_exact(&mut nv12) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(format!("lettura della sorgente non riuscita ({e})")),
            }
            let t_ms = frames as f64 * 1000.0 / job.fps as f64;
            let p = frame_params(
                t_ms,
                &camera,
                &cursor,
                (sw as f64, sh as f64),
                ow as f64,
                arrow_src,
                sprite_ratio,
            );
            gpu.render(&nv12, &p, &mut rgba)
                .map_err(|e| e.to_string())?;
            sink.write_all(&rgba)
                .map_err(|e| format!("FFmpeg ha chiuso la codifica ({e}): {}", tail(&enc_log)))?;
            frames += 1;
            if frames.is_multiple_of(15) {
                progress(frames.min(expected), expected);
            }
        }
        Ok(())
    })();

    let dec_status = decoder.wait();
    let enc_status = encoder.wait();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        tracing::error!("export interrotto: {e}");
        return Err(e);
    }
    if !dec_status.map(|s| s.success()).unwrap_or(false) {
        let _ = std::fs::remove_file(&part);
        return Err(format!("la sorgente non si decodifica: {}", tail(&dec_log)));
    }
    if !enc_status.map(|s| s.success()).unwrap_or(false) || frames == 0 {
        let _ = std::fs::remove_file(&part);
        return Err(format!("la codifica non è riuscita: {}", tail(&enc_log)));
    }
    std::fs::rename(&part, job.output).map_err(|e| e.to_string())?;
    progress(expected, expected);
    let render_ms = started.elapsed().as_millis() as u64;
    tracing::info!(frames, render_ms, "export completato");
    Ok(ExportStats {
        frames,
        width: ow,
        height: oh,
        render_ms,
        gpu: gpu.adapter_name.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_events::{Button, EventKind};
    use crate::render::zoom::{plan_zooms, Click};

    /// Pipeline completa con un FFmpeg vero e una GPU (anche software).
    /// In CI: `FRAMENOTE_TEST_FFMPEG` e `FRAMENOTE_REQUIRE_GPU` la rendono obbligatoria.
    #[test]
    fn export_completo_con_cinque_clic() {
        let Some(ffmpeg) = std::env::var_os("FRAMENOTE_TEST_FFMPEG").map(PathBuf::from) else {
            eprintln!("FRAMENOTE_TEST_FFMPEG non impostata: test saltato");
            return;
        };
        if Compositor::new((64, 64), (64, 64)).is_err() {
            assert!(
                std::env::var_os("FRAMENOTE_REQUIRE_GPU").is_none(),
                "GPU richiesta ma non disponibile"
            );
            eprintln!("GPU non disponibile: test saltato");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let (w, h, fps, secs) = (640u32, 416u32, 30u32, 3u64);
        // Sorgente sintetica nello stesso formato della registrazione.
        let input = tmp.path().join("capture.mp4");
        let st = Command::new(&ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
            .arg(format!("testsrc2=size={w}x{h}:rate={fps}:duration={secs}"))
            .args(["-pix_fmt", "yuv420p", "-c:v", "mpeg4", "-q:v", "2", "-y"])
            .arg(&input)
            .status()
            .unwrap();
        assert!(st.success());

        let mut events = Vec::new();
        for i in 0..90u64 {
            events.push(InputEvent {
                t_ms: i * 33,
                kind: EventKind::Move,
                x: 50.0 + i as f64 * 5.0,
                y: 100.0 + i as f64 * 2.0,
                button: None,
                display: 1,
            });
        }
        let clicks: Vec<Click> = [300u64, 900, 1500, 2100, 2700]
            .iter()
            .map(|&t| {
                let x = 50.0 + (t / 33) as f64 * 5.0;
                let y = 100.0 + (t / 33) as f64 * 2.0;
                events.push(InputEvent {
                    t_ms: t,
                    kind: EventKind::Click,
                    x,
                    y,
                    button: Some(Button::Left),
                    display: 1,
                });
                Click {
                    t_ms: t as f64,
                    x,
                    y,
                }
            })
            .collect();
        events.sort_by_key(|e| e.t_ms);
        let zs = ZoomSettings::default();
        let blocks = plan_zooms(&clicks, w as f64, h as f64, (secs * 1000) as f64, &zs);
        assert!(!blocks.is_empty());

        let output = tmp.path().join("zoom.mp4");
        let mut calls = 0;
        let stats = export(
            &ExportJob {
                ffmpeg: &ffmpeg,
                input: &input,
                src_width: w,
                src_height: h,
                fps,
                duration_ms: secs * 1000,
                display_scale: 1.0,
                events: &events,
                zoom_blocks: &blocks,
                zoom_settings: &zs,
                cursor_settings: &CursorSettings::default(),
                output: &output,
                encoder: "mpeg4",
                log_dir: tmp.path(),
            },
            &mut |_, _| calls += 1,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(stats.frames, (secs * fps as u64));
        assert_eq!((stats.width, stats.height), (w, h));
        assert!(calls > 0);
        assert!(!output.with_extension("mp4.part").exists());

        // Il file finale ha la durata e il numero di frame attesi.
        let probe = Command::new(&ffmpeg)
            .args(["-hide_banner", "-i"])
            .arg(&output)
            .args(["-f", "null", "-"])
            .output()
            .unwrap();
        let log = String::from_utf8_lossy(&probe.stderr);
        assert!(probe.status.success(), "{log}");
        assert!(log.contains(&format!("{w}x{h}")), "{log}");
        assert!(
            log.contains("frame=   90") || log.contains("frame=90"),
            "{log}"
        );
    }

    #[test]
    fn annullamento() {
        let Some(ffmpeg) = std::env::var_os("FRAMENOTE_TEST_FFMPEG").map(PathBuf::from) else {
            return;
        };
        if Compositor::new((64, 64), (64, 64)).is_err() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("c.mp4");
        assert!(Command::new(&ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x200:rate=30:duration=2",
                "-c:v",
                "mpeg4",
                "-y"
            ])
            .arg(&input)
            .status()
            .unwrap()
            .success());
        let output = tmp.path().join("zoom.mp4");
        let err = export(
            &ExportJob {
                ffmpeg: &ffmpeg,
                input: &input,
                src_width: 320,
                src_height: 200,
                fps: 30,
                duration_ms: 2000,
                display_scale: 1.0,
                events: &[],
                zoom_blocks: &[],
                zoom_settings: &ZoomSettings::default(),
                cursor_settings: &CursorSettings::default(),
                output: &output,
                encoder: "mpeg4",
                log_dir: tmp.path(),
            },
            &mut |_, _| {},
            &AtomicBool::new(true),
        )
        .unwrap_err();
        assert!(err.contains("annullata"));
        assert!(!output.exists() && !output.with_extension("mp4.part").exists());
    }

    /// Prova visiva manuale (non gira in CI): esporta una sorgente reale per
    /// guardarne i fotogrammi. `FRAMENOTE_VISUAL_SRC` = MP4 3024×1964 di 8 s.
    #[test]
    #[ignore]
    fn prova_visiva() {
        let ffmpeg = PathBuf::from(std::env::var_os("FRAMENOTE_TEST_FFMPEG").unwrap());
        let input = PathBuf::from(std::env::var_os("FRAMENOTE_VISUAL_SRC").unwrap());
        let output = input.with_file_name("zoom.mp4");
        let mut events = Vec::new();
        // Il cursore va verso il pulsante, clicca, poi va sul testo e clicca.
        let path = [
            (0u64, 600.0, 900.0),
            (2500, 1900.0, 1340.0),
            (5200, 1900.0, 1340.0),
            (6000, 400.0, 255.0),
        ];
        for t in (0..8000u64).step_by(8) {
            let seg = path.windows(2).find(|w| t >= w[0].0 && t <= w[1].0);
            let (x, y) = match seg {
                Some(w) => {
                    let k = (t - w[0].0) as f64 / (w[1].0 - w[0].0) as f64;
                    let k = k * k * (3.0 - 2.0 * k);
                    (
                        w[0].1 + (w[1].1 - w[0].1) * k,
                        w[0].2 + (w[1].2 - w[0].2) * k,
                    )
                }
                None => (400.0, 255.0),
            };
            events.push(InputEvent {
                t_ms: t,
                kind: EventKind::Move,
                x,
                y,
                button: None,
                display: 1,
            });
        }
        let clicks = [
            Click {
                t_ms: 3000.0,
                x: 1900.0,
                y: 1340.0,
            },
            Click {
                t_ms: 6300.0,
                x: 400.0,
                y: 255.0,
            },
        ];
        for c in &clicks {
            events.push(InputEvent {
                t_ms: c.t_ms as u64,
                kind: EventKind::Click,
                x: c.x,
                y: c.y,
                button: Some(Button::Left),
                display: 1,
            });
        }
        events.sort_by_key(|e| e.t_ms);
        let zs = ZoomSettings::default();
        let blocks = plan_zooms(&clicks, 3024.0, 1964.0, 8000.0, &zs);
        eprintln!("{blocks:#?}");
        let stats = export(
            &ExportJob {
                ffmpeg: &ffmpeg,
                input: &input,
                src_width: 3024,
                src_height: 1964,
                fps: 30,
                duration_ms: 8000,
                display_scale: 2.0,
                events: &events,
                zoom_blocks: &blocks,
                zoom_settings: &zs,
                cursor_settings: &CursorSettings::default(),
                output: &output,
                encoder: "libx264",
                log_dir: input.parent().unwrap(),
            },
            &mut |_, _| {},
            &AtomicBool::new(false),
        )
        .unwrap();
        eprintln!("{stats:?}");
    }
}
