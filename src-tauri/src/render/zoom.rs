//! Motore degli zoom automatici.
//!
//! 1. `plan_zooms` trasforma i clic in **blocchi zoom** (modificabili, salvati
//!    in `project.json`): clic vicini nel tempo e nello spazio diventano un solo
//!    blocco, blocchi troppo vicini si fondono (isteresi contro gli zoom nervosi).
//! 2. `CameraTrack` simula una telecamera con molle smorzate criticamente a
//!    passo fisso: transizioni morbide, senza rimbalzi, deterministiche.

use serde::{Deserialize, Serialize};

use super::cursor::CursorTrack;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomSettings {
    /// Clic entro questo intervallo (ms) possono stare nello stesso zoom.
    pub cluster_time_ms: f64,
    /// … se distano meno di questa frazione della larghezza del video.
    pub cluster_distance: f64,
    /// Lo zoom inizia così prima del primo clic.
    pub lead_ms: f64,
    /// … e finisce così dopo l'ultimo.
    pub hold_ms: f64,
    pub min_duration_ms: f64,
    /// Blocchi separati da meno di questo intervallo si fondono.
    pub merge_gap_ms: f64,
    pub min_scale: f64,
    pub max_scale: f64,
    /// Margine attorno all'area dei clic, in frazione dell'inquadratura.
    pub margin: f64,
    /// Frequenza propria delle molle (rad/s): più alta = transizioni più rapide.
    pub spring_omega: f64,
    /// Zona morta centrale (frazione dell'inquadratura) entro cui la telecamera non segue il cursore.
    pub follow_deadzone: f64,
}

impl Default for ZoomSettings {
    fn default() -> Self {
        Self {
            cluster_time_ms: 2500.0,
            cluster_distance: 0.15,
            lead_ms: 400.0,
            hold_ms: 1500.0,
            min_duration_ms: 1200.0,
            merge_gap_ms: 1000.0,
            min_scale: 1.5,
            max_scale: 2.0,
            margin: 0.3,
            spring_omega: 7.5,
            follow_deadzone: 0.6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomBlock {
    pub start_ms: f64,
    pub end_ms: f64,
    pub scale: f64,
    /// Centro dell'inquadratura in pixel del video.
    pub center_x: f64,
    pub center_y: f64,
    /// Numero di clic che hanno generato il blocco.
    pub clicks: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Click {
    pub t_ms: f64,
    pub x: f64,
    pub y: f64,
}

/// Inquadratura: livello di zoom e centro in pixel del video.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub scale: f64,
    pub cx: f64,
    pub cy: f64,
}

impl Camera {
    /// Rettangolo sorgente visibile: (x0, y0, larghezza, altezza).
    pub fn viewport(&self, width: f64, height: f64) -> (f64, f64, f64, f64) {
        let w = width / self.scale;
        let h = height / self.scale;
        (self.cx - w / 2.0, self.cy - h / 2.0, w, h)
    }
}

/// Sposta il centro quanto basta perché l'inquadratura resti nel video.
pub fn clamp_center(cx: f64, cy: f64, scale: f64, width: f64, height: f64) -> (f64, f64) {
    let hw = width / scale / 2.0;
    let hh = height / scale / 2.0;
    (cx.clamp(hw, width - hw), cy.clamp(hh, height - hh))
}

fn block_from_clicks(
    clicks: &[Click],
    s: &ZoomSettings,
    width: f64,
    height: f64,
    duration_ms: f64,
) -> ZoomBlock {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for c in clicks {
        x0 = x0.min(c.x);
        y0 = y0.min(c.y);
        x1 = x1.max(c.x);
        y1 = y1.max(c.y);
    }
    // Livello che fa stare l'area dei clic con il margine.
    let need_w = (x1 - x0) / (1.0 - 2.0 * s.margin).max(0.1);
    let need_h = (y1 - y0) / (1.0 - 2.0 * s.margin).max(0.1);
    let fit = (width / need_w.max(1.0)).min(height / need_h.max(1.0));
    let scale = fit.clamp(s.min_scale, s.max_scale);
    let (center_x, center_y) = clamp_center((x0 + x1) / 2.0, (y0 + y1) / 2.0, scale, width, height);
    let first = clicks.first().map_or(0.0, |c| c.t_ms);
    let last = clicks.last().map_or(0.0, |c| c.t_ms);
    let start_ms = (first - s.lead_ms).max(0.0);
    let end_ms = (last + s.hold_ms)
        .max(start_ms + s.min_duration_ms)
        .min(duration_ms.max(start_ms));
    ZoomBlock {
        start_ms,
        end_ms,
        scale,
        center_x,
        center_y,
        clicks: clicks.len(),
    }
}

/// Dai clic ai blocchi zoom.
pub fn plan_zooms(
    clicks: &[Click],
    width: f64,
    height: f64,
    duration_ms: f64,
    s: &ZoomSettings,
) -> Vec<ZoomBlock> {
    let mut sorted: Vec<Click> = clicks
        .iter()
        .copied()
        .filter(|c| c.t_ms >= 0.0 && c.t_ms <= duration_ms)
        .collect();
    sorted.sort_by(|a, b| a.t_ms.total_cmp(&b.t_ms));

    // 1. Raggruppamento: il clic entra nel gruppo corrente se è vicino nel
    //    tempo all'ultimo e nello spazio al baricentro del gruppo.
    let max_dist = s.cluster_distance * width;
    let mut groups: Vec<Vec<Click>> = Vec::new();
    for c in sorted {
        let joins = groups.last().is_some_and(|g| {
            let last = g.last().expect("gruppo non vuoto");
            let n = g.len() as f64;
            let (mx, my) = g
                .iter()
                .fold((0.0, 0.0), |(ax, ay), k| (ax + k.x / n, ay + k.y / n));
            c.t_ms - last.t_ms <= s.cluster_time_ms
                && (c.x - mx).hypot(c.y - my) <= max_dist
        });
        if joins {
            groups.last_mut().expect("gruppo presente").push(c);
        } else {
            groups.push(vec![c]);
        }
    }

    // 2. Fusione dei gruppi i cui blocchi sono troppo vicini o sovrapposti
    //    (isteresi): si ricalcola il blocco sull'unione dei clic.
    let mut merged: Vec<Vec<Click>> = Vec::new();
    for g in groups {
        if let Some(prev) = merged.last_mut() {
            let a = block_from_clicks(prev, s, width, height, duration_ms);
            let b = block_from_clicks(&g, s, width, height, duration_ms);
            if b.start_ms - a.end_ms < s.merge_gap_ms {
                prev.extend(g);
                continue;
            }
        }
        merged.push(g);
    }
    merged
        .iter()
        .map(|g| block_from_clicks(g, s, width, height, duration_ms))
        .collect()
}

/// Molla smorzata criticamente: raggiunge il bersaglio senza oscillare.
#[derive(Debug, Clone, Copy)]
struct Spring {
    x: f64,
    v: f64,
}

impl Spring {
    fn step(&mut self, target: f64, omega: f64, dt: f64) {
        // Integrazione esatta della molla critica per un passo con bersaglio costante.
        let d = self.x - target;
        let e = (-omega * dt).exp();
        let c = self.v + omega * d;
        self.x = target + (d + c * dt) * e;
        self.v = (c - omega * (d + c * dt)) * e;
    }
}

/// Passo della simulazione della telecamera.
const SIM_HZ: f64 = 240.0;

/// Traccia della telecamera precalcolata a passo fisso.
#[derive(Debug, Clone)]
pub struct CameraTrack {
    width: f64,
    height: f64,
    samples: Vec<Camera>,
}

impl CameraTrack {
    pub fn simulate(
        blocks: &[ZoomBlock],
        cursor: Option<&CursorTrack>,
        width: f64,
        height: f64,
        duration_ms: f64,
        s: &ZoomSettings,
    ) -> Self {
        let dt = 1.0 / SIM_HZ;
        let n = (duration_ms / 1000.0 * SIM_HZ).ceil() as usize + 2;
        // Il livello si anima in scala logaritmica: uno zoom da 1× a 2× e
        // ritorno ha la stessa "velocità percepita" in entrata e in uscita.
        let mut ln_scale = Spring { x: 0.0, v: 0.0 };
        let mut cx = Spring {
            x: width / 2.0,
            v: 0.0,
        };
        let mut cy = Spring {
            x: height / 2.0,
            v: 0.0,
        };
        let mut samples = Vec::with_capacity(n);
        let mut follow_target: Option<(f64, f64)> = None;
        for i in 0..n {
            let t_ms = i as f64 * 1000.0 / SIM_HZ;
            let block = blocks.iter().find(|b| t_ms >= b.start_ms && t_ms < b.end_ms);
            let (target_scale, tx, ty) = match block {
                Some(b) => {
                    // Dentro lo zoom la telecamera segue il cursore solo quando
                    // esce dalla zona morta centrale dell'inquadratura.
                    let (mut fx, mut fy) = follow_target.unwrap_or((b.center_x, b.center_y));
                    if let Some((px, py)) = cursor.and_then(|c| c.position(t_ms)) {
                        let dz_w = width / b.scale / 2.0 * s.follow_deadzone;
                        let dz_h = height / b.scale / 2.0 * s.follow_deadzone;
                        if px > fx + dz_w {
                            fx = px - dz_w;
                        } else if px < fx - dz_w {
                            fx = px + dz_w;
                        }
                        if py > fy + dz_h {
                            fy = py - dz_h;
                        } else if py < fy - dz_h {
                            fy = py + dz_h;
                        }
                    }
                    let (fx, fy) = clamp_center(fx, fy, b.scale, width, height);
                    follow_target = Some((fx, fy));
                    (b.scale, fx, fy)
                }
                None => {
                    follow_target = None;
                    (1.0, width / 2.0, height / 2.0)
                }
            };
            ln_scale.step(target_scale.ln(), s.spring_omega, dt);
            cx.step(tx, s.spring_omega, dt);
            cy.step(ty, s.spring_omega, dt);
            let scale = ln_scale.x.exp().max(1.0);
            let (x, y) = clamp_center(cx.x, cy.x, scale, width, height);
            samples.push(Camera { scale, cx: x, cy: y });
        }
        Self {
            width,
            height,
            samples,
        }
    }

    /// Inquadratura all'istante `t_ms` (interpolazione lineare tra i campioni).
    pub fn at(&self, t_ms: f64) -> Camera {
        let pos = (t_ms.max(0.0) / 1000.0 * SIM_HZ).min((self.samples.len() - 1) as f64);
        let i = pos.floor() as usize;
        let j = (i + 1).min(self.samples.len() - 1);
        let k = pos - i as f64;
        let (a, b) = (self.samples[i], self.samples[j]);
        let scale = a.scale + (b.scale - a.scale) * k;
        let (cx, cy) = clamp_center(
            a.cx + (b.cx - a.cx) * k,
            a.cy + (b.cy - a.cy) * k,
            scale,
            self.width,
            self.height,
        );
        Camera { scale, cx, cy }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_events::{Button, EventKind, InputEvent};
    use crate::render::cursor::CursorSettings;

    const W: f64 = 3024.0;
    const H: f64 = 1964.0;

    fn c(t: f64, x: f64, y: f64) -> Click {
        Click { t_ms: t, x, y }
    }

    /// Lo scenario del criterio di M1: 30 s, 5 clic.
    fn five_clicks() -> Vec<Click> {
        vec![
            c(3000.0, 400.0, 300.0),
            c(4200.0, 520.0, 360.0), // vicino al primo: stesso zoom
            c(11000.0, 2500.0, 1600.0),
            c(19000.0, 1500.0, 1000.0),
            c(26000.0, 2900.0, 100.0), // vicino al bordo
        ]
    }

    #[test]
    fn cinque_clic_quattro_zoom() {
        let s = ZoomSettings::default();
        let blocks = plan_zooms(&five_clicks(), W, H, 30_000.0, &s);
        assert_eq!(blocks.len(), 4, "{blocks:#?}");
        assert_eq!(blocks[0].clicks, 2);
        assert_eq!(blocks[0].start_ms, 3000.0 - s.lead_ms);
        assert_eq!(blocks[0].end_ms, 4200.0 + s.hold_ms);
        for b in &blocks {
            assert!(b.scale >= s.min_scale && b.scale <= s.max_scale);
            assert!(b.end_ms - b.start_ms >= s.min_duration_ms);
            // L'inquadratura del blocco è interamente nel video.
            let (x0, y0, w, h) = Camera {
                scale: b.scale,
                cx: b.center_x,
                cy: b.center_y,
            }
            .viewport(W, H);
            assert!(x0 >= 0.0 && y0 >= 0.0 && x0 + w <= W + 1e-9 && y0 + h <= H + 1e-9);
        }
        // Clic vicino al bordo: il centro viene spostato verso l'interno.
        let last = blocks[3];
        assert!(last.center_x < 2900.0 && last.center_y > 100.0);
    }

    #[test]
    fn nessun_clic_nessuno_zoom() {
        let s = ZoomSettings::default();
        assert!(plan_zooms(&[], W, H, 30_000.0, &s).is_empty());
        let cam = CameraTrack::simulate(&[], None, W, H, 5000.0, &s);
        for t in (0..5000).step_by(33) {
            let k = cam.at(t as f64);
            assert_eq!(k.scale, 1.0);
            assert_eq!((k.cx, k.cy), (W / 2.0, H / 2.0));
        }
    }

    #[test]
    fn clic_ravvicinati_si_fondono_isteresi() {
        let s = ZoomSettings::default();
        // Due clic lontani nello spazio ma a 1,8 s: senza isteresi sarebbero due
        // zoom con un "su e giù" in mezzo. I blocchi distano meno di merge_gap.
        let clicks = [c(5000.0, 300.0, 300.0), c(6800.0, 2700.0, 1700.0)];
        let blocks = plan_zooms(&clicks, W, H, 20_000.0, &s);
        assert_eq!(blocks.len(), 1, "{blocks:#?}");
        assert_eq!(blocks[0].clicks, 2);
        // Area larga: il livello scende al minimo per farla stare.
        assert_eq!(blocks[0].scale, s.min_scale);
    }

    #[test]
    fn durata_minima_e_limiti_del_video() {
        let s = ZoomSettings::default();
        let blocks = plan_zooms(&[c(100.0, 1000.0, 1000.0)], W, H, 900.0, &s);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].start_ms, 0.0);
        assert_eq!(blocks[0].end_ms, 900.0);
        // Clic fuori dalla durata ignorati.
        assert!(plan_zooms(&[c(5000.0, 1.0, 1.0)], W, H, 900.0, &s).is_empty());
    }

    fn simulate_five() -> (Vec<ZoomBlock>, CameraTrack) {
        let s = ZoomSettings::default();
        let blocks = plan_zooms(&five_clicks(), W, H, 30_000.0, &s);
        let cam = CameraTrack::simulate(&blocks, None, W, H, 30_000.0, &s);
        (blocks, cam)
    }

    #[test]
    fn telecamera_sempre_dentro_il_video_e_senza_rimbalzi() {
        let s = ZoomSettings::default();
        let (blocks, cam) = simulate_five();
        let max_target = blocks.iter().map(|b| b.scale).fold(1.0, f64::max);
        for f in 0..900 {
            let t = f as f64 * 1000.0 / 30.0;
            let k = cam.at(t);
            assert!(k.scale >= 1.0 && k.scale <= max_target + 1e-6, "rimbalzo a {t}: {k:?}");
            let (x0, y0, w, h) = k.viewport(W, H);
            assert!(x0 >= -1e-6 && y0 >= -1e-6, "{t}: {k:?}");
            assert!(x0 + w <= W + 1e-6 && y0 + h <= H + 1e-6, "{t}: {k:?}");
        }
        // Al centro di un blocco la telecamera è arrivata al livello voluto.
        let b = blocks[1];
        let mid = cam.at((b.start_ms + b.end_ms) / 2.0);
        assert!((mid.scale - b.scale).abs() < 0.02, "{mid:?} vs {b:?}");
        assert!((mid.cx - b.center_x).abs() < 5.0 && (mid.cy - b.center_y).abs() < 5.0);
        // Tra due blocchi distanti torna a 1×.
        assert!((cam.at(16_000.0).scale - 1.0).abs() < 0.01);
        let _ = s;
    }

    #[test]
    fn transizioni_morbide() {
        let (_, cam) = simulate_five();
        // Tra un fotogramma e l'altro (30 fps) il livello cambia poco: niente scatti.
        let mut prev = cam.at(0.0);
        let mut prev_speed = 0.0;
        for f in 1..900 {
            let k = cam.at(f as f64 * 1000.0 / 30.0);
            // Variazione relativa del livello (in scala logaritmica, come la percepisce l'occhio).
            let speed = (k.scale.ln() - prev.scale.ln()).abs();
            assert!(speed < 0.08, "scatto di livello al frame {f}: {speed}");
            // Anche la velocità varia gradualmente (accelerazione limitata).
            assert!((speed - prev_speed).abs() < 0.06, "accelerazione brusca al frame {f}");
            let pan = (k.cx - prev.cx).hypot(k.cy - prev.cy);
            assert!(pan < 140.0, "spostamento brusco al frame {f}: {pan:.0} px");
            prev = k;
            prev_speed = speed;
        }
    }

    #[test]
    fn deterministico() {
        let (_, a) = simulate_five();
        let (_, b) = simulate_five();
        for t in [0.0, 1234.5, 15_000.0, 29_999.0] {
            assert_eq!(a.at(t), b.at(t));
        }
    }

    #[test]
    fn segue_il_cursore_solo_fuori_dalla_zona_morta() {
        let s = ZoomSettings::default();
        let block = ZoomBlock {
            start_ms: 0.0,
            end_ms: 10_000.0,
            scale: 2.0,
            center_x: 1512.0,
            center_y: 982.0,
            clicks: 1,
        };
        let ev = |t: u64, x: f64| InputEvent {
            t_ms: t,
            kind: EventKind::Move,
            x,
            y: 982.0,
            button: Some(Button::Left).filter(|_| false),
            display: 1,
        };
        // Piccolo movimento dentro la zona morta: l'inquadratura resta ferma.
        let small = CursorTrack::build(&[ev(0, 1512.0), ev(3000, 1700.0)], CursorSettings::default());
        let cam = CameraTrack::simulate(&[block], Some(&small), W, H, 10_000.0, &s);
        assert!((cam.at(6000.0).cx - 1512.0).abs() < 1.0);
        // Il cursore va verso il bordo destro: la telecamera lo segue.
        let big = CursorTrack::build(&[ev(0, 1512.0), ev(3000, 2900.0)], CursorSettings::default());
        let cam = CameraTrack::simulate(&[block], Some(&big), W, H, 10_000.0, &s);
        let k = cam.at(6000.0);
        let (x0, _, w, _) = k.viewport(W, H);
        assert!(k.cx > 1600.0, "{k:?}");
        assert!(2900.0 > x0 && 2900.0 < x0 + w, "cursore fuori inquadratura: {k:?}");
    }
}
