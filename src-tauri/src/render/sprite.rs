//! Sprite del cursore: freccia nera con bordo bianco e ombra morbida,
//! rasterizzata in software con antialiasing (nessun file immagine da includere).

/// Freccia in punti (y verso il basso), punta in (0, 0).
const ARROW: [(f32, f32); 7] = [
    (0.0, 0.0),
    (0.0, 16.2),
    (3.9, 12.5),
    (6.5, 18.4),
    (9.0, 17.3),
    (6.5, 11.6),
    (11.5, 11.6),
];
/// Altezza della freccia in punti (come il cursore di sistema).
pub const ARROW_HEIGHT_PT: f32 = 18.4;
const SCALE: f32 = 6.0; // pixel dello sprite per punto
const PAD: f32 = 4.0; // margine in punti per bordo e ombra
const BORDER: f32 = 1.1;
const SUB: usize = 4; // sottocampioni per lato

pub struct Sprite {
    pub width: u32,
    pub height: u32,
    /// RGBA premoltiplicato.
    pub rgba: Vec<u8>,
    /// Punta della freccia in pixel dello sprite.
    pub hotspot: (f32, f32),
    /// Altezza della sola freccia in pixel dello sprite.
    pub arrow_height: f32,
}

fn inside(p: (f32, f32)) -> bool {
    let mut c = false;
    let n = ARROW.len();
    for i in 0..n {
        let (a, b) = (ARROW[i], ARROW[(i + n - 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            c = !c;
        }
    }
    c
}

fn edge_distance(p: (f32, f32)) -> f32 {
    let n = ARROW.len();
    (0..n)
        .map(|i| {
            let (a, b) = (ARROW[i], ARROW[(i + 1) % n]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
        })
        .fold(f32::MAX, f32::min)
}

pub fn cursor_sprite() -> Sprite {
    let w = ((11.5 + 2.0 * PAD) * SCALE).ceil() as usize;
    let h = ((ARROW_HEIGHT_PT + 2.0 * PAD) * SCALE).ceil() as usize;
    let mut outer = vec![0f32; w * h];
    let mut core = vec![0f32; w * h];
    let inv = 1.0 / (SUB * SUB) as f32;
    for y in 0..h {
        for x in 0..w {
            let (mut o, mut c) = (0.0, 0.0);
            for sy in 0..SUB {
                for sx in 0..SUB {
                    let p = (
                        (x as f32 + (sx as f32 + 0.5) / SUB as f32) / SCALE - PAD,
                        (y as f32 + (sy as f32 + 0.5) / SUB as f32) / SCALE - PAD,
                    );
                    let ins = inside(p);
                    if ins || edge_distance(p) < BORDER {
                        o += inv;
                    }
                    if ins {
                        c += inv;
                    }
                }
            }
            outer[y * w + x] = o;
            core[y * w + x] = c;
        }
    }
    // Ombra: sagoma esterna sfocata (box blur separabile) e spostata in basso.
    let radius = (1.2 * SCALE) as isize;
    let shift = (0.9 * SCALE) as isize;
    let blur = |src: &[f32], horizontal: bool| -> Vec<f32> {
        let mut dst = vec![0f32; w * h];
        for y in 0..h as isize {
            for x in 0..w as isize {
                let mut acc = 0.0;
                for k in -radius..=radius {
                    let (xx, yy) = if horizontal { (x + k, y) } else { (x, y + k) };
                    if xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < h {
                        acc += src[yy as usize * w + xx as usize];
                    }
                }
                dst[y as usize * w + x as usize] = acc / (2 * radius + 1) as f32;
            }
        }
        dst
    };
    let shadow = blur(&blur(&outer, true), false);

    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let sy = y as isize - shift;
            let sh = if sy >= 0 { shadow[sy as usize * w + x] * 0.35 } else { 0.0 };
            let a_outer = outer[i];
            // Bianco dove c'è il bordo, nero nel corpo; premoltiplicato.
            let white = a_outer - core[i];
            let alpha = a_outer + sh * (1.0 - a_outer);
            let v = (white * 255.0).round() as u8;
            rgba[i * 4..i * 4 + 4].copy_from_slice(&[v, v, v, (alpha * 255.0).round() as u8]);
        }
    }
    Sprite {
        width: w as u32,
        height: h as u32,
        rgba,
        hotspot: (PAD * SCALE, PAD * SCALE),
        arrow_height: ARROW_HEIGHT_PT * SCALE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freccia_con_bordo_e_trasparenza() {
        let s = cursor_sprite();
        assert_eq!(s.rgba.len(), (s.width * s.height * 4) as usize);
        let at = |x: f32, y: f32| {
            let (px, py) = ((x + PAD) * SCALE, (y + PAD) * SCALE);
            let i = ((py as u32 * s.width + px as u32) * 4) as usize;
            &s.rgba[i..i + 4]
        };
        // Corpo nero opaco.
        assert_eq!(at(3.0, 9.0), &[0, 0, 0, 255]);
        // Bordo bianco appena fuori dal lato sinistro.
        let border = at(-0.6, 8.0);
        assert!(border[0] > 200 && border[3] > 200, "{border:?}");
        // Lontano dalla freccia: trasparente.
        assert_eq!(at(10.0, 2.0)[3], 0);
        // Premoltiplicato: il colore non supera mai l'alfa.
        assert!(s.rgba.chunks(4).all(|p| p[0] <= p[3]));
    }
}
