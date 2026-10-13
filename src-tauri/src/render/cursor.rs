//! Cursore ridisegnato: percorso smussato, dissolvenza da fermo, anello sul clic.
//!
//! Tutto è funzione del tempo (ms dall'inizio del video): lo stesso istante
//! dà sempre lo stesso risultato, in anteprima come in export.

use serde::{Deserialize, Serialize};

use crate::input_events::{EventKind, InputEvent};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorSettings {
    /// Ingrandimento rispetto alla dimensione di sistema.
    pub size: f64,
    /// Smussatura: frequenza minima del filtro One Euro (Hz). Più bassa = più liscio.
    pub min_cutoff_hz: f64,
    /// Smussatura: quanto il filtro diventa reattivo con la velocità.
    pub beta: f64,
    /// Dopo quanti ms di immobilità il cursore sparisce.
    pub hide_after_ms: f64,
    pub fade_out_ms: f64,
    pub fade_in_ms: f64,
    pub click_ring: bool,
    pub ring_ms: f64,
}

impl Default for CursorSettings {
    fn default() -> Self {
        Self {
            size: 1.5,
            min_cutoff_hz: 1.5,
            beta: 0.004,
            hide_after_ms: 2000.0,
            fade_out_ms: 350.0,
            fade_in_ms: 120.0,
            click_ring: true,
            ring_ms: 450.0,
        }
    }
}

/// Frequenza a cui viene ricampionato il percorso prima del filtro.
const RESAMPLE_HZ: f64 = 240.0;

/// Filtro One Euro (Casiez et al., 2012): poco ritardo quando il cursore
/// corre, molta stabilità quando è quasi fermo.
#[derive(Debug, Clone)]
struct OneEuro {
    min_cutoff: f64,
    beta: f64,
    d_cutoff: f64,
    prev: Option<(f64, f64)>, // (valore filtrato, derivata filtrata)
}

impl OneEuro {
    fn new(min_cutoff: f64, beta: f64) -> Self {
        Self {
            min_cutoff,
            beta,
            d_cutoff: 1.0,
            prev: None,
        }
    }

    fn alpha(cutoff: f64, dt: f64) -> f64 {
        let tau = 1.0 / (2.0 * std::f64::consts::PI * cutoff);
        1.0 / (1.0 + tau / dt)
    }

    fn filter(&mut self, x: f64, dt: f64) -> f64 {
        let Some((prev_x, prev_dx)) = self.prev else {
            self.prev = Some((x, 0.0));
            return x;
        };
        let dx = (x - prev_x) / dt;
        let a_d = Self::alpha(self.d_cutoff, dt);
        let dx_hat = a_d * dx + (1.0 - a_d) * prev_dx;
        let cutoff = self.min_cutoff + self.beta * dx_hat.abs();
        let a = Self::alpha(cutoff, dt);
        let x_hat = a * x + (1.0 - a) * prev_x;
        self.prev = Some((x_hat, dx_hat));
        x_hat
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ring {
    pub x: f64,
    pub y: f64,
    /// 0 → appena cliccato, 1 → animazione finita.
    pub progress: f64,
}

#[derive(Debug, Clone)]
pub struct CursorTrack {
    settings: CursorSettings,
    /// Campioni smussati a passo fisso: (t_ms, x, y).
    smoothed: Vec<(f64, f64, f64)>,
    clicks: Vec<(f64, f64, f64)>,
    /// Istanti di attività (movimenti e clic), ordinati.
    activity: Vec<f64>,
}

impl CursorTrack {
    pub fn build(events: &[InputEvent], settings: CursorSettings) -> Self {
        let mut raw: Vec<(f64, f64, f64)> = events
            .iter()
            .map(|e| (e.t_ms as f64, e.x, e.y))
            .collect();
        raw.sort_by(|a, b| a.0.total_cmp(&b.0));
        let clicks = events
            .iter()
            .filter(|e| e.kind == EventKind::Click)
            .map(|e| (e.t_ms as f64, e.x, e.y))
            .collect();
        let activity = raw.iter().map(|s| s.0).collect();

        let mut smoothed = Vec::new();
        if let (Some(first), Some(last)) = (raw.first(), raw.last()) {
            // Ricampionamento a passo fisso tenendo l'ultimo valore, fino a 1 s
            // dopo l'ultimo evento: così il filtro converge sempre sulla posizione
            // reale anche quando il mouse si ferma.
            let dt = 1.0 / RESAMPLE_HZ;
            let mut fx = OneEuro::new(settings.min_cutoff_hz, settings.beta);
            let mut fy = OneEuro::new(settings.min_cutoff_hz, settings.beta);
            let end = last.0 + 1000.0;
            let mut i = 0;
            let mut t = first.0;
            let mut cur = (first.1, first.2);
            while t <= end {
                while i < raw.len() && raw[i].0 <= t {
                    cur = (raw[i].1, raw[i].2);
                    i += 1;
                }
                smoothed.push((t, fx.filter(cur.0, dt), fy.filter(cur.1, dt)));
                t += dt * 1000.0;
            }
        }
        Self {
            settings,
            smoothed,
            clicks,
            activity,
        }
    }

    pub fn settings(&self) -> &CursorSettings {
        &self.settings
    }

    pub fn is_empty(&self) -> bool {
        self.smoothed.is_empty()
    }

    /// Posizione smussata in pixel del video.
    pub fn position(&self, t_ms: f64) -> Option<(f64, f64)> {
        let s = &self.smoothed;
        let first = s.first()?;
        if t_ms <= first.0 {
            return Some((first.1, first.2));
        }
        let last = s.last()?;
        if t_ms >= last.0 {
            return Some((last.1, last.2));
        }
        let i = s.partition_point(|p| p.0 <= t_ms);
        let (a, b) = (s[i - 1], s[i]);
        let k = (t_ms - a.0) / (b.0 - a.0);
        Some((a.1 + (b.1 - a.1) * k, a.2 + (b.2 - a.2) * k))
    }

    /// Opacità 0..1: sparisce dopo `hide_after_ms` di immobilità, riappare al primo movimento.
    pub fn opacity(&self, t_ms: f64) -> f64 {
        if self.smoothed.is_empty() {
            return 0.0;
        }
        let s = &self.settings;
        let idx = self.activity.partition_point(|&a| a <= t_ms);
        // Prima del primo evento il cursore è visibile (è dove l'utente l'ha lasciato).
        let last = if idx == 0 { 0.0 } else { self.activity[idx - 1] };
        let idle = t_ms - last;
        let fade_out = 1.0 - ((idle - s.hide_after_ms) / s.fade_out_ms).clamp(0.0, 1.0);
        let prev = if idx >= 2 {
            Some(self.activity[idx - 2])
        } else {
            None
        };
        let was_hidden = prev.is_some_and(|p| last - p > s.hide_after_ms + s.fade_out_ms);
        let fade_in = if was_hidden {
            ((t_ms - last) / s.fade_in_ms).clamp(0.0, 1.0)
        } else {
            1.0
        };
        fade_out * fade_in
    }

    /// Anello del clic più recente ancora in animazione.
    pub fn ring(&self, t_ms: f64) -> Option<Ring> {
        if !self.settings.click_ring {
            return None;
        }
        let idx = self.clicks.partition_point(|c| c.0 <= t_ms);
        let c = self.clicks.get(idx.checked_sub(1)?)?;
        let progress = (t_ms - c.0) / self.settings.ring_ms;
        (progress < 1.0).then_some(Ring {
            x: c.1,
            y: c.2,
            progress,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_events::Button;

    fn mv(t: u64, x: f64, y: f64) -> InputEvent {
        InputEvent {
            t_ms: t,
            kind: EventKind::Move,
            x,
            y,
            button: None,
            display: 1,
        }
    }

    fn click(t: u64, x: f64, y: f64) -> InputEvent {
        InputEvent {
            kind: EventKind::Click,
            button: Some(Button::Left),
            ..mv(t, x, y)
        }
    }

    #[test]
    fn senza_eventi_niente_cursore() {
        let t = CursorTrack::build(&[], CursorSettings::default());
        assert!(t.position(100.0).is_none());
        assert_eq!(t.opacity(100.0), 0.0);
    }

    #[test]
    fn il_tremolio_viene_smorzato() {
        // Mano quasi ferma con tremolio di ±2 px a 120 Hz.
        let evs: Vec<_> = (0..240)
            .map(|i| {
                let j = if i % 2 == 0 { 2.0 } else { -2.0 };
                mv(i * 8, 500.0 + j, 300.0 - j)
            })
            .collect();
        let t = CursorTrack::build(&evs, CursorSettings::default());
        let mut max_dev: f64 = 0.0;
        for ms in (500..1900).step_by(5) {
            let (x, y) = t.position(ms as f64).unwrap();
            max_dev = max_dev.max((x - 500.0).abs()).max((y - 300.0).abs());
        }
        assert!(max_dev < 0.8, "tremolio residuo {max_dev:.2} px");
    }

    #[test]
    fn segue_un_movimento_veloce_e_si_ferma_sul_punto_giusto() {
        // Da 0 a 2000 px in 250 ms, poi fermo.
        let mut evs: Vec<_> = (0..=30).map(|i| mv(i * 8, i as f64 * 66.67, 100.0)).collect();
        evs.push(mv(400, 2000.0, 100.0));
        let t = CursorTrack::build(&evs, CursorSettings::default());
        // Durante il movimento il ritardo resta contenuto.
        let (x, _) = t.position(200.0).unwrap();
        assert!((x - 1666.0).abs() < 400.0, "ritardo eccessivo: x={x:.0}");
        // Poco dopo l'arresto è sul punto finale.
        let (x, y) = t.position(900.0).unwrap();
        assert!((x - 2000.0).abs() < 2.0, "x={x:.2}");
        assert!((y - 100.0).abs() < 0.01);
        // Nessun superamento del punto finale (niente effetto elastico).
        for ms in (0..1500).step_by(4) {
            assert!(t.position(ms as f64).unwrap().0 <= 2000.5);
        }
    }

    #[test]
    fn dissolvenza_da_fermo_e_ricomparsa() {
        let s = CursorSettings::default();
        let evs = [mv(0, 10.0, 10.0), mv(100, 20.0, 20.0), mv(6000, 30.0, 30.0)];
        let t = CursorTrack::build(&evs, s);
        assert_eq!(t.opacity(50.0), 1.0);
        assert_eq!(t.opacity(2000.0), 1.0);
        let mid = t.opacity(100.0 + s.hide_after_ms + s.fade_out_ms / 2.0);
        assert!(mid > 0.4 && mid < 0.6, "{mid}");
        assert_eq!(t.opacity(4000.0), 0.0);
        // Al nuovo movimento ricompare con una breve dissolvenza in entrata.
        assert_eq!(t.opacity(6000.0), 0.0);
        assert!(t.opacity(6000.0 + s.fade_in_ms / 2.0) > 0.4);
        assert_eq!(t.opacity(6000.0 + s.fade_in_ms), 1.0);
    }

    #[test]
    fn anello_del_clic() {
        let s = CursorSettings::default();
        let t = CursorTrack::build(&[mv(0, 1.0, 1.0), click(1000, 50.0, 60.0)], s);
        assert!(t.ring(999.0).is_none());
        let r = t.ring(1000.0 + s.ring_ms / 2.0).unwrap();
        assert_eq!((r.x, r.y), (50.0, 60.0));
        assert!((r.progress - 0.5).abs() < 1e-9);
        assert!(t.ring(1000.0 + s.ring_ms).is_none());
        let off = CursorSettings {
            click_ring: false,
            ..s
        };
        assert!(CursorTrack::build(&[click(0, 1.0, 1.0)], off).ring(10.0).is_none());
    }
}
