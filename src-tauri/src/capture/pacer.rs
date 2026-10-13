//! Da frame a ritmo variabile a video a frame rate costante.
//!
//! ScreenCaptureKit consegna un frame solo quando lo schermo cambia; il video
//! invece deve avere un frame ogni 1/fps s, allineato agli eventi del mouse.
//! Il frame `n` del video mostra lo schermo all'istante `t0 + n/fps`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacerAction {
    /// Quante volte riscrivere il frame precedente prima di questo.
    pub repeat_previous: u64,
    /// Se scrivere questo frame (false = arrivato in ritardo per il suo posto).
    pub write: bool,
}

#[derive(Debug, Clone)]
pub struct Pacer {
    fps: u32,
    t0_ns: Option<u64>,
    written: u64,
}

impl Pacer {
    pub fn new(fps: u32) -> Self {
        Self {
            fps,
            t0_ns: None,
            written: 0,
        }
    }

    pub fn t0_ns(&self) -> Option<u64> {
        self.t0_ns
    }

    pub fn written(&self) -> u64 {
        self.written
    }

    fn slot(&self, t0: u64, t_ns: u64) -> u64 {
        let dt = t_ns.saturating_sub(t0) as f64 / 1e9;
        (dt * self.fps as f64).round() as u64
    }

    pub fn on_frame(&mut self, t_ns: u64) -> PacerAction {
        let t0 = *self.t0_ns.get_or_insert(t_ns);
        let slot = self.slot(t0, t_ns);
        if slot < self.written {
            return PacerAction {
                repeat_previous: 0,
                write: false,
            };
        }
        let repeat_previous = slot - self.written;
        self.written = slot + 1;
        PacerAction {
            repeat_previous,
            write: true,
        }
    }

    /// Frame da ripetere in coda per arrivare fino all'istante di stop.
    pub fn on_stop(&mut self, stop_ns: u64) -> u64 {
        let Some(t0) = self.t0_ns else { return 0 };
        let dt = stop_ns.saturating_sub(t0) as f64 / 1e9;
        let total = (dt * self.fps as f64).floor() as u64;
        let extra = total.saturating_sub(self.written);
        self.written += extra;
        extra
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    #[test]
    fn frame_regolari() {
        let mut p = Pacer::new(30);
        for i in 0..10u64 {
            let a = p.on_frame(1_000 * MS + i * 33_333_333);
            assert_eq!(
                a,
                PacerAction {
                    repeat_previous: 0,
                    write: true
                }
            );
        }
        assert_eq!(p.written(), 10);
        assert_eq!(p.t0_ns(), Some(1_000 * MS));
    }

    #[test]
    fn schermo_fermo_ripete_il_frame() {
        let mut p = Pacer::new(30);
        p.on_frame(0);
        // Nessun cambiamento per 1 s: il frame successivo arriva al posto 30.
        let a = p.on_frame(1_000 * MS);
        assert_eq!(a.repeat_previous, 29);
        assert!(a.write);
        assert_eq!(p.written(), 31);
    }

    #[test]
    fn frame_in_ritardo_non_duplica_il_posto() {
        let mut p = Pacer::new(30);
        p.on_frame(0);
        p.on_frame(33 * MS);
        // Due frame quasi simultanei per lo stesso posto: il secondo non va scritto.
        assert!(!p.on_frame(40 * MS).write);
        assert_eq!(p.written(), 2);
    }

    #[test]
    fn coda_fino_allo_stop() {
        let mut p = Pacer::new(30);
        assert_eq!(p.on_stop(5_000 * MS), 0, "nessun frame: niente coda");
        p.on_frame(0);
        // Stop a 2 s: il video deve avere 60 frame.
        assert_eq!(p.on_stop(2_000 * MS), 59);
        assert_eq!(p.written(), 60);
    }
}
