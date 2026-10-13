//! Sidecar FFmpeg incluso nel pacchetto: ricerca, versione, encoder.
//!
//! Un encoder è dichiarato disponibile solo se compare in `ffmpeg -encoders`
//! **e** una codifica di prova reale di 0,2 s va a buon fine: l'elenco da
//! solo non basta (es. NVENC è compilato ma senza GPU NVIDIA fallisce).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

/// Encoder H.264/HEVC da provare, in ordine di preferenza.
/// `true` = hardware.
pub const CANDIDATES: &[(&str, bool)] = &[
    ("h264_videotoolbox", true),
    ("hevc_videotoolbox", true),
    ("h264_nvenc", true),
    ("hevc_nvenc", true),
    ("h264_qsv", true),
    ("hevc_qsv", true),
    ("h264_amf", true),
    ("hevc_amf", true),
    ("h264_mf", true),
    ("hevc_mf", true),
    ("libopenh264", false),
    ("libx264", false),
];

const PROBE_TIMEOUT: Duration = Duration::from_secs(20);
const INFO_TIMEOUT: Duration = Duration::from_secs(10);

/// Percorso atteso del sidecar: Tauri lo installa accanto all'eseguibile
/// (`Contents/MacOS/ffmpeg` su macOS, accanto a `FrameNote.exe` su Windows).
pub fn sidecar_path() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| std::io::Error::other("eseguibile senza cartella"))?;
    Ok(dir.join(format!("ffmpeg{}", std::env::consts::EXE_SUFFIX)))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListedEncoder {
    /// `V`, `A` o `S`.
    pub kind: char,
    pub name: String,
    pub description: String,
}

/// Interpreta l'output di `ffmpeg -encoders`.
pub fn parse_encoders(output: &str) -> Vec<ListedEncoder> {
    let mut out = Vec::new();
    let mut in_list = false;
    for line in output.lines() {
        let trimmed = line.trim_start();
        if !in_list {
            if trimmed.starts_with("------") {
                in_list = true;
            }
            continue;
        }
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let (Some(flags), Some(rest)) = (parts.next(), parts.next()) else {
            continue;
        };
        if flags.len() != 6 || !flags.is_ascii() {
            continue;
        }
        let kind = flags.chars().next().unwrap_or('.');
        if !matches!(kind, 'V' | 'A' | 'S') {
            continue;
        }
        let rest = rest.trim_start();
        let mut np = rest.splitn(2, char::is_whitespace);
        let Some(name) = np.next().filter(|n| !n.is_empty()) else {
            continue;
        };
        out.push(ListedEncoder {
            kind,
            name: name.to_string(),
            description: np.next().unwrap_or("").trim().to_string(),
        });
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub version: Option<String>,
    pub configuration: Option<String>,
    pub license_lgpl: Option<bool>,
}

/// Interpreta l'output di `ffmpeg -version`.
pub fn parse_version(output: &str) -> VersionInfo {
    let mut info = VersionInfo::default();
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("ffmpeg version ") {
            info.version = rest.split_whitespace().next().map(str::to_string);
        } else if let Some(rest) = line.strip_prefix("configuration:") {
            let conf = rest.trim().to_string();
            // Senza --enable-gpl / --enable-nonfree la build è LGPL.
            info.license_lgpl =
                Some(!conf.contains("--enable-gpl") && !conf.contains("--enable-nonfree"));
            info.configuration = Some(conf);
        }
    }
    info
}

#[derive(Debug)]
pub struct RunOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub elapsed: Duration,
}

/// Esegue un comando con timeout, leggendo stdout/stderr in parallelo
/// (altrimenti un output lungo riempie la pipe e blocca il processo).
pub fn run_with_timeout(mut cmd: Command, timeout: Duration) -> std::io::Result<RunOutput> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let start = Instant::now();
    let mut child = cmd.spawn()?;
    let reader = |mut pipe: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).into_owned()
        })
    };
    let out_t = reader(Box::new(child.stdout.take().expect("stdout piped")));
    let err_t = reader(Box::new(child.stderr.take().expect("stderr piped")));

    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() >= timeout {
            timed_out = true;
            let _ = child.kill();
            break child.wait()?;
        }
        thread::sleep(Duration::from_millis(25));
    };
    Ok(RunOutput {
        success: status.success() && !timed_out,
        stdout: out_t.join().unwrap_or_default(),
        stderr: err_t.join().unwrap_or_default(),
        timed_out,
        elapsed: start.elapsed(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncoderProbe {
    pub name: String,
    pub description: String,
    pub hardware: bool,
    pub working: bool,
    pub duration_ms: u64,
    /// Ultima riga d'errore di FFmpeg, se la prova è fallita.
    pub error: Option<String>,
}

/// Codifica di prova reale: 0,2 s di `testsrc2` 1280×720 scartati con `-f null`.
pub fn probe_encoder(
    ffmpeg: &Path,
    name: &str,
    hardware: bool,
) -> (bool, Duration, Option<String>) {
    let mut cmd = Command::new(ffmpeg);
    cmd.args([
        "-hide_banner",
        "-nostdin",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=1280x720:rate=30",
        "-t",
        "0.2",
        "-pix_fmt",
        if hardware { "nv12" } else { "yuv420p" },
        "-c:v",
        name,
        "-f",
        "null",
        "-",
    ]);
    match run_with_timeout(cmd, PROBE_TIMEOUT) {
        Ok(out) if out.success => (true, out.elapsed, None),
        Ok(out) if out.timed_out => (
            false,
            out.elapsed,
            Some(format!(
                "nessuna risposta entro {} s",
                PROBE_TIMEOUT.as_secs()
            )),
        ),
        Ok(out) => (false, out.elapsed, Some(last_error_line(&out.stderr))),
        Err(e) => (
            false,
            Duration::ZERO,
            Some(format!("avvio non riuscito: {e}")),
        ),
    }
}

fn last_error_line(stderr: &str) -> String {
    let line = stderr
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or("errore sconosciuto");
    line.chars().take(300).collect()
}

/// Sceglie l'encoder da usare: il primo funzionante nell'ordine di `CANDIDATES`.
pub fn preferred(probes: &[EncoderProbe]) -> Option<&EncoderProbe> {
    CANDIDATES
        .iter()
        .find_map(|(name, _)| probes.iter().find(|p| p.working && p.name == *name))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegReport {
    pub found: bool,
    pub path: String,
    pub version: VersionInfo,
    /// Solo i candidati presenti in `-encoders`, ognuno con l'esito della prova.
    pub encoders: Vec<EncoderProbe>,
    pub preferred: Option<String>,
    pub preferred_hardware: Option<bool>,
    pub error: Option<String>,
}

pub fn detect(ffmpeg: &Path, display_path: String) -> FfmpegReport {
    let mut report = FfmpegReport {
        found: ffmpeg.is_file(),
        path: display_path,
        version: VersionInfo::default(),
        encoders: Vec::new(),
        preferred: None,
        preferred_hardware: None,
        error: None,
    };
    if !report.found {
        report.error = Some("FFmpeg non trovato accanto all'app".to_string());
        return report;
    }

    let run = |args: &[&str]| {
        let mut cmd = Command::new(ffmpeg);
        cmd.args(args);
        run_with_timeout(cmd, INFO_TIMEOUT)
    };

    match run(&["-hide_banner", "-version"]) {
        Ok(out) if out.success => report.version = parse_version(&out.stdout),
        Ok(out) => report.error = Some(last_error_line(&out.stderr)),
        Err(e) => report.error = Some(format!("FFmpeg non si avvia: {e}")),
    }
    if report.error.is_some() {
        return report;
    }

    let listed = match run(&["-hide_banner", "-encoders"]) {
        Ok(out) if out.success => parse_encoders(&out.stdout),
        Ok(out) => {
            report.error = Some(last_error_line(&out.stderr));
            return report;
        }
        Err(e) => {
            report.error = Some(format!("FFmpeg non si avvia: {e}"));
            return report;
        }
    };

    for (name, hardware) in CANDIDATES {
        let Some(enc) = listed.iter().find(|e| e.name == *name) else {
            continue;
        };
        let (working, elapsed, error) = probe_encoder(ffmpeg, name, *hardware);
        tracing::info!(
            encoder = name,
            working,
            ms = elapsed.as_millis() as u64,
            "prova encoder"
        );
        report.encoders.push(EncoderProbe {
            name: name.to_string(),
            description: enc.description.clone(),
            hardware: *hardware,
            working,
            duration_ms: elapsed.as_millis() as u64,
            error,
        });
    }
    if let Some(p) = preferred(&report.encoders) {
        report.preferred = Some(p.name.clone());
        report.preferred_hardware = Some(p.hardware);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENCODERS_LINUX: &str = include_str!("../../tests/fixtures/encoders-linux-ubuntu.txt");
    const VERSION_LINUX: &str = include_str!("../../tests/fixtures/version-linux-ubuntu.txt");
    const ENCODERS_LGPL: &str = include_str!("../../tests/fixtures/encoders-8.1.2-lgpl-linux.txt");
    const VERSION_LGPL: &str = include_str!("../../tests/fixtures/version-8.1.2-lgpl-linux.txt");

    #[test]
    fn legge_l_elenco_encoder() {
        let list = parse_encoders(ENCODERS_LINUX);
        assert!(list.len() > 100, "trovati solo {}", list.len());
        let nvenc = list.iter().find(|e| e.name == "h264_nvenc").unwrap();
        assert_eq!(nvenc.kind, 'V');
        assert_eq!(nvenc.description, "NVIDIA NVENC H.264 encoder (codec h264)");
        assert!(list.iter().any(|e| e.name == "aac" && e.kind == 'A'));
        // Le righe della legenda prima di "------" non sono encoder.
        assert!(!list.iter().any(|e| e.name == "="));
    }

    #[test]
    fn la_build_lgpl_non_ha_encoder_gpl() {
        let list = parse_encoders(ENCODERS_LGPL);
        assert!(list.iter().any(|e| e.name == "mpeg4"));
        assert!(!list.iter().any(|e| e.name == "libx264"));
        let v = parse_version(VERSION_LGPL);
        assert_eq!(v.version.as_deref(), Some("8.1.2"));
        assert_eq!(v.license_lgpl, Some(true));
    }

    #[test]
    fn legge_versione_e_licenza() {
        let v = parse_version(VERSION_LINUX);
        assert_eq!(v.version.as_deref(), Some("6.1.1-3ubuntu5"));
        assert_eq!(v.license_lgpl, Some(false));
        assert!(v.configuration.unwrap().contains("--enable-gpl"));
    }

    #[test]
    fn righe_videotoolbox() {
        // Formato delle righe sui Mac (stessa colonna dei flag di FFmpeg 8).
        let sample = " ------\n V....D h264_videotoolbox    VideoToolbox H.264 Encoder (codec h264)\n V....D hevc_videotoolbox    VideoToolbox H.265 Encoder (codec hevc)\n";
        let list = parse_encoders(sample);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "h264_videotoolbox");
    }

    #[test]
    fn preferenza_hardware_prima_del_software() {
        let p = |name: &str, hardware, working| EncoderProbe {
            name: name.into(),
            description: String::new(),
            hardware,
            working,
            duration_ms: 1,
            error: None,
        };
        let probes = vec![
            p("libopenh264", false, true),
            p("h264_nvenc", true, false),
            p("h264_qsv", true, true),
        ];
        assert_eq!(preferred(&probes).unwrap().name, "h264_qsv");
        assert!(preferred(&[p("h264_nvenc", true, false)]).is_none());
    }

    #[test]
    fn sidecar_accanto_all_eseguibile() {
        let p = sidecar_path().unwrap();
        let exe = std::env::current_exe().unwrap();
        assert_eq!(p.parent(), exe.parent());
        assert!(p
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("ffmpeg"));
    }

    /// Prova reale con un FFmpeg vero: in CI `FRAMENOTE_TEST_FFMPEG` punta al
    /// binario da usare. In locale, senza la variabile, il test non fa nulla.
    #[test]
    fn prova_reale_con_ffmpeg() {
        let Some(path) = std::env::var_os("FRAMENOTE_TEST_FFMPEG") else {
            eprintln!("FRAMENOTE_TEST_FFMPEG non impostata: prova reale saltata");
            return;
        };
        let path = PathBuf::from(path);
        let (ok, _, err) = probe_encoder(&path, "mpeg4", false);
        assert!(ok, "mpeg4 dovrebbe funzionare: {err:?}");
        let (ok, _, err) = probe_encoder(&path, "encoder_inesistente", false);
        assert!(!ok);
        assert!(err.is_some());
        let report = detect(&path, "ffmpeg".into());
        assert!(report.found);
        assert!(report.error.is_none(), "{:?}", report.error);
        assert!(report.version.version.is_some());
    }
}
