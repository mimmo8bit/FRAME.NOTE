//! Eventi di input registrati durante la cattura (`events.jsonl`, §4).
//!
//! Ogni riga è un oggetto JSON con l'istante in ms dall'inizio del video e la
//! posizione in **pixel del video** (non in punti dello schermo).

#[cfg(target_os = "macos")]
pub mod macos;

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const EVENTS_FILE: &str = "events.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Move,
    /// Pressione di un pulsante.
    Click,
    /// Rilascio di un pulsante.
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InputEvent {
    pub t_ms: u64,
    #[serde(rename = "type")]
    pub kind: EventKind,
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button: Option<Button>,
    pub display: u32,
}

/// Geometria dello schermo catturato: origine e dimensioni in punti
/// (coordinate globali di macOS, origine in alto a sinistra dello schermo
/// principale) e fattore di scala verso i pixel del video.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayGeometry {
    pub display_id: u32,
    pub origin_x_pt: f64,
    pub origin_y_pt: f64,
    pub width_pt: f64,
    pub height_pt: f64,
    pub scale: f64,
}

impl DisplayGeometry {
    #[cfg(test)]
    pub fn width_px(self) -> u32 {
        (self.width_pt * self.scale).round() as u32
    }

    #[cfg(test)]
    pub fn height_px(self) -> u32 {
        (self.height_pt * self.scale).round() as u32
    }

    /// Da punti globali a pixel del video.
    pub fn to_pixels(self, x_pt: f64, y_pt: f64) -> (f64, f64) {
        (
            (x_pt - self.origin_x_pt) * self.scale,
            (y_pt - self.origin_y_pt) * self.scale,
        )
    }

    pub fn contains_point(self, x_pt: f64, y_pt: f64) -> bool {
        x_pt >= self.origin_x_pt
            && y_pt >= self.origin_y_pt
            && x_pt < self.origin_x_pt + self.width_pt
            && y_pt < self.origin_y_pt + self.height_pt
    }
}

/// Scrittura incrementale di `events.jsonl`.
pub struct EventWriter {
    out: BufWriter<File>,
    pub written: u64,
    pub clicks: u64,
}

impl EventWriter {
    pub fn create(path: &Path) -> std::io::Result<Self> {
        Ok(Self {
            out: BufWriter::new(File::create(path)?),
            written: 0,
            clicks: 0,
        })
    }

    pub fn write(&mut self, ev: &InputEvent) -> std::io::Result<()> {
        serde_json::to_writer(&mut self.out, ev)?;
        self.out.write_all(b"\n")?;
        self.written += 1;
        if ev.kind == EventKind::Click {
            self.clicks += 1;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.out.flush()
    }

    pub fn finish(mut self) -> std::io::Result<()> {
        self.out.flush()?;
        self.out.get_ref().sync_all()
    }
}

#[derive(Debug, Default)]
pub struct ReadEvents {
    pub events: Vec<InputEvent>,
    /// Righe illeggibili (per esempio l'ultima, troncata da una chiusura improvvisa).
    pub skipped_lines: usize,
}

pub fn read_events(path: &Path) -> std::io::Result<ReadEvents> {
    let mut out = ReadEvents::default();
    for line in BufReader::new(File::open(path)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<InputEvent>(&line) {
            Ok(ev) => out.events.push(ev),
            Err(_) => out.skipped_lines += 1,
        }
    }
    out.events.sort_by_key(|e| e.t_ms);
    Ok(out)
}

/// Rileva pressioni e rilasci confrontando lo stato dei pulsanti tra due letture.
#[derive(Debug, Default, Clone, Copy)]
pub struct ButtonTracker {
    pressed: [bool; 3],
}

impl ButtonTracker {
    /// `now` = stato attuale di sinistro, destro, centrale.
    pub fn update(&mut self, now: [bool; 3]) -> Vec<(Button, EventKind)> {
        const BUTTONS: [Button; 3] = [Button::Left, Button::Right, Button::Middle];
        let mut out = Vec::new();
        for i in 0..3 {
            if now[i] != self.pressed[i] {
                out.push((
                    BUTTONS[i],
                    if now[i] {
                        EventKind::Click
                    } else {
                        EventKind::Release
                    },
                ));
            }
        }
        self.pressed = now;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retina() -> DisplayGeometry {
        DisplayGeometry {
            display_id: 1,
            origin_x_pt: 0.0,
            origin_y_pt: 0.0,
            width_pt: 1512.0,
            height_pt: 982.0,
            scale: 2.0,
        }
    }

    #[test]
    fn punti_in_pixel_retina() {
        let d = retina();
        assert_eq!(d.width_px(), 3024);
        assert_eq!(d.height_px(), 1964);
        assert_eq!(d.to_pixels(100.0, 50.5), (200.0, 101.0));
        assert!(d.contains_point(0.0, 0.0));
        assert!(!d.contains_point(1512.0, 10.0));
    }

    #[test]
    fn schermo_secondario_con_origine_negativa() {
        // Monitor esterno a sinistra di quello principale, non Retina.
        let d = DisplayGeometry {
            display_id: 2,
            origin_x_pt: -1920.0,
            origin_y_pt: -200.0,
            width_pt: 1920.0,
            height_pt: 1080.0,
            scale: 1.0,
        };
        assert_eq!(d.to_pixels(-1920.0, -200.0), (0.0, 0.0));
        assert_eq!(d.to_pixels(-1.0, 879.0), (1919.0, 1079.0));
        assert!(!d.contains_point(5.0, 5.0));
    }

    #[test]
    fn scrittura_e_lettura_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(EVENTS_FILE);
        let mut w = EventWriter::create(&path).unwrap();
        let evs = [
            InputEvent {
                t_ms: 10,
                kind: EventKind::Move,
                x: 1.5,
                y: 2.0,
                button: None,
                display: 1,
            },
            InputEvent {
                t_ms: 20,
                kind: EventKind::Click,
                x: 3.0,
                y: 4.0,
                button: Some(Button::Left),
                display: 1,
            },
        ];
        for e in &evs {
            w.write(e).unwrap();
        }
        assert_eq!(w.clicks, 1);
        w.finish().unwrap();

        // Formato della specifica: campo "type", pulsante solo sui clic.
        let text = std::fs::read_to_string(&path).unwrap();
        let first = text.lines().next().unwrap();
        assert_eq!(
            first,
            r#"{"t_ms":10,"type":"move","x":1.5,"y":2.0,"display":1}"#
        );

        // Ultima riga troncata (chiusura improvvisa): viene saltata.
        std::fs::write(&path, format!("{text}{{\"t_ms\":30,\"ty")).unwrap();
        let read = read_events(&path).unwrap();
        assert_eq!(read.events, evs);
        assert_eq!(read.skipped_lines, 1);
    }

    #[test]
    fn pressioni_e_rilasci() {
        let mut t = ButtonTracker::default();
        assert!(t.update([false, false, false]).is_empty());
        assert_eq!(
            t.update([true, false, false]),
            [(Button::Left, EventKind::Click)]
        );
        assert!(t.update([true, false, false]).is_empty());
        assert_eq!(
            t.update([false, true, false]),
            [
                (Button::Left, EventKind::Release),
                (Button::Right, EventKind::Click)
            ]
        );
    }
}
