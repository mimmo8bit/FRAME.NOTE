//! Comandi FFmpeg per i flussi via pipe: codifica dei frame grezzi
//! (registrazione ed export) e decodifica della sorgente per il renderer.

use std::path::Path;

/// Qualità in bit per pixel per frame: la sorgente della registrazione è più
/// generosa perché verrà ingrandita dagli zoom.
pub const CAPTURE_BPP: f64 = 0.12;
pub const EXPORT_BPP: f64 = 0.10;

pub fn bitrate(width: u32, height: u32, fps: u32, bpp: f64) -> u64 {
    (width as f64 * height as f64 * fps as f64 * bpp).round() as u64
}

/// Opzioni di qualità specifiche dell'encoder.
fn quality_args(encoder: &str, bitrate: u64) -> Vec<String> {
    let b = bitrate.to_string();
    match encoder {
        "libx264" => vec!["-preset".into(), "veryfast".into(), "-crf".into(), "18".into()],
        "libopenh264" => vec!["-b:v".into(), b],
        "mpeg4" => vec!["-q:v".into(), "3".into()],
        e if e.ends_with("_videotoolbox") => vec![
            "-b:v".into(),
            b.clone(),
            "-maxrate".into(),
            (bitrate * 3 / 2).to_string(),
            "-bufsize".into(),
            (bitrate * 2).to_string(),
        ],
        _ => vec!["-b:v".into(), b],
    }
}

const COLOR_TAGS: [&str; 8] = [
    "-colorspace",
    "bt709",
    "-color_primaries",
    "bt709",
    "-color_trc",
    "bt709",
    "-color_range",
    "tv",
];

/// Codifica dei frame NV12 della registrazione in MP4 **frammentato**: se l'app
/// si chiude di colpo, il video scritto fin lì resta leggibile.
pub fn capture_encoder_args(
    width: u32,
    height: u32,
    fps: u32,
    encoder: &str,
    output: &Path,
) -> Vec<String> {
    let mut a: Vec<String> = [
        "-hide_banner",
        "-nostdin",
        "-loglevel",
        "warning",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "nv12",
        "-color_range",
        "tv",
        "-colorspace",
        "bt709",
        "-video_size",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    a.push(format!("{width}x{height}"));
    a.extend(["-framerate".into(), fps.to_string(), "-i".into(), "-".into()]);
    a.extend(["-c:v".into(), encoder.to_string()]);
    a.extend(quality_args(encoder, bitrate(width, height, fps, CAPTURE_BPP)));
    a.extend(["-g".into(), (fps * 2).to_string()]);
    a.extend(COLOR_TAGS.iter().map(|s| s.to_string()));
    a.extend([
        "-movflags".into(),
        "+frag_keyframe+empty_moov+default_base_moof".into(),
        "-f".into(),
        "mp4".into(),
        "-y".into(),
        output.display().to_string(),
    ]);
    a
}

/// Decodifica della sorgente in NV12 grezzo su stdout.
pub fn decoder_args(input: &Path) -> Vec<String> {
    vec![
        "-hide_banner".into(),
        "-nostdin".into(),
        "-loglevel".into(),
        "error".into(),
        "-i".into(),
        input.display().to_string(),
        "-an".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "nv12".into(),
        "-".into(),
    ]
}

/// Codifica dei frame RGBA renderizzati nel video finale.
pub fn export_encoder_args(
    width: u32,
    height: u32,
    fps: u32,
    encoder: &str,
    output: &Path,
) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "-hide_banner".into(),
        "-nostdin".into(),
        "-loglevel".into(),
        "warning".into(),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgba".into(),
        "-video_size".into(),
        format!("{width}x{height}"),
        "-framerate".into(),
        fps.to_string(),
        "-i".into(),
        "-".into(),
        "-vf".into(),
        "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p".into(),
        "-c:v".into(),
        encoder.to_string(),
    ];
    a.extend(quality_args(encoder, bitrate(width, height, fps, EXPORT_BPP)));
    a.extend(["-g".into(), (fps * 2).to_string()]);
    a.extend(COLOR_TAGS.iter().map(|s| s.to_string()));
    a.extend([
        "-movflags".into(),
        "+faststart".into(),
        "-f".into(),
        "mp4".into(),
        "-y".into(),
        output.display().to_string(),
    ]);
    a
}

/// Dimensioni del video finale: stesse proporzioni della sorgente, lato lungo
/// al massimo `max_long`, entrambe pari (richiesto da H.264 4:2:0).
pub fn output_size(src_w: u32, src_h: u32, max_long: u32) -> (u32, u32) {
    let long = src_w.max(src_h) as f64;
    let k = (max_long as f64 / long).min(1.0);
    let even = |v: f64| ((v / 2.0).round() as u32 * 2).max(2);
    (even(src_w as f64 * k), even(src_h as f64 * k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensioni_di_uscita() {
        assert_eq!(output_size(3024, 1964, 1920), (1920, 1246));
        assert_eq!(output_size(1920, 1080, 1920), (1920, 1080));
        assert_eq!(output_size(1280, 720, 1920), (1280, 720));
        assert_eq!(output_size(1964, 3024, 1920), (1246, 1920));
        assert_eq!(output_size(1001, 501, 1920), (1002, 502));
    }

    #[test]
    fn registrazione_in_mp4_frammentato_hardware() {
        let a = capture_encoder_args(3024, 1964, 30, "h264_videotoolbox", Path::new("/p/c.mp4"));
        let s = a.join(" ");
        assert!(s.contains("-pix_fmt nv12"));
        assert!(s.contains("-video_size 3024x1964"));
        assert!(s.contains(&format!("-c:v h264_videotoolbox -b:v {}", bitrate(3024, 1964, 30, CAPTURE_BPP))));
        assert!(s.contains("+frag_keyframe+empty_moov"));
        assert!(s.ends_with("-y /p/c.mp4"));
    }

    #[test]
    fn export_con_qualita_per_encoder() {
        let s = export_encoder_args(1920, 1246, 30, "libx264", Path::new("o.mp4")).join(" ");
        assert!(s.contains("-crf 18"));
        assert!(s.contains("-pix_fmt rgba"));
        assert!(s.contains("out_color_matrix=bt709"));
        let s = export_encoder_args(1920, 1246, 30, "hevc_videotoolbox", Path::new("o.mp4")).join(" ");
        assert!(s.contains(&format!("-b:v {}", bitrate(1920, 1246, 30, EXPORT_BPP))));
    }
}
