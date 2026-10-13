// Composizione di un frame: sorgente NV12 con l'inquadratura della telecamera,
// cursore ridisegnato e anello del clic. Un solo passaggio, triangolo a schermo intero.

struct Uniforms {
    // Inquadratura nella sorgente: x0, y0, larghezza, altezza (pixel sorgente).
    viewport: vec4f,
    // Larghezza e altezza della sorgente, larghezza e altezza dell'uscita.
    sizes: vec4f,
    // Cursore in pixel di uscita: x, y della punta, altezza dello sprite, opacità.
    cursor: vec4f,
    // Sprite del cursore: larghezza, altezza, punta x, punta y (pixel della texture).
    cursor_img: vec4f,
    // Anello del clic in pixel di uscita: x, y, raggio, opacità.
    ring: vec4f,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var tex_y: texture_2d<f32>;
@group(0) @binding(2) var tex_uv: texture_2d<f32>;
@group(0) @binding(3) var tex_cursor: texture_2d<f32>;
@group(0) @binding(4) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4f,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    // Triangolo che copre tutto lo schermo.
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.pos = vec4f(p * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

// Catmull-Rom con 9 campioni bilineari (Sigg & Hadwiger / MJP):
// nitido quando si ingrandisce, senza aloni marcati.
fn sample_bicubic(t: texture_2d<f32>, pos_px: vec2f, size: vec2f) -> vec4f {
    let sample_pos = pos_px;
    let tex1 = floor(sample_pos - 0.5) + 0.5;
    let f = sample_pos - tex1;
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let w12 = w1 + w2;
    let off12 = w2 / w12;
    let t0 = (tex1 - 1.0) / size;
    let t3 = (tex1 + 2.0) / size;
    let t12 = (tex1 + off12) / size;
    var r = vec4f(0.0);
    r += textureSampleLevel(t, samp, vec2f(t0.x, t0.y), 0.0) * w0.x * w0.y;
    r += textureSampleLevel(t, samp, vec2f(t12.x, t0.y), 0.0) * w12.x * w0.y;
    r += textureSampleLevel(t, samp, vec2f(t3.x, t0.y), 0.0) * w3.x * w0.y;
    r += textureSampleLevel(t, samp, vec2f(t0.x, t12.y), 0.0) * w0.x * w12.y;
    r += textureSampleLevel(t, samp, vec2f(t12.x, t12.y), 0.0) * w12.x * w12.y;
    r += textureSampleLevel(t, samp, vec2f(t3.x, t12.y), 0.0) * w3.x * w12.y;
    r += textureSampleLevel(t, samp, vec2f(t0.x, t3.y), 0.0) * w0.x * w3.y;
    r += textureSampleLevel(t, samp, vec2f(t12.x, t3.y), 0.0) * w12.x * w3.y;
    r += textureSampleLevel(t, samp, vec2f(t3.x, t3.y), 0.0) * w3.x * w3.y;
    return r;
}

// YCbCr BT.709 "video range" → RGB.
fn yuv_to_rgb(y: f32, cb: f32, cr: f32) -> vec3f {
    let yy = (y * 255.0 - 16.0) / 219.0;
    let u = (cb * 255.0 - 128.0) / 224.0;
    let v = (cr * 255.0 - 128.0) / 224.0;
    return clamp(vec3f(
        yy + 1.5748 * v,
        yy - 0.1873 * u - 0.4681 * v,
        yy + 1.8556 * u,
    ), vec3f(0.0), vec3f(1.0));
}

fn source_rgb(src_px: vec2f, ratio: f32) -> vec3f {
    let src_size = u.sizes.xy;
    let uv_size = src_size * 0.5;
    if (ratio <= 1.0) {
        // Ingrandimento o 1:1: bicubico sulla luminanza, bilineare sul colore.
        let y = sample_bicubic(tex_y, src_px, src_size).r;
        let c = textureSampleLevel(tex_uv, samp, src_px / src_size, 0.0).rg;
        return yuv_to_rgb(clamp(y, 0.0, 1.0), c.r, c.g);
    }
    // Riduzione: media su una griglia di campioni (fino a 4×4) per evitare
    // lo sfarfallio del testo sottile.
    let n = min(i32(ceil(ratio)), 4);
    let step = ratio / f32(n);
    var acc = vec3f(0.0);
    for (var j = 0; j < n; j++) {
        for (var i = 0; i < n; i++) {
            let off = (vec2f(f32(i), f32(j)) + 0.5) * step - ratio * 0.5;
            let p = src_px + off;
            let y = textureSampleLevel(tex_y, samp, p / src_size, 0.0).r;
            let c = textureSampleLevel(tex_uv, samp, p / src_size, 0.0).rg;
            acc += yuv_to_rgb(y, c.r, c.g);
        }
    }
    return acc / f32(n * n);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4f {
    let out_px = in.pos.xy; // centro del pixel di uscita
    let out_size = u.sizes.zw;
    let ratio = u.viewport.z / out_size.x;
    let src_px = u.viewport.xy + out_px * ratio;
    var color = source_rgb(src_px, ratio);

    // Anello del clic: due cerchi (scuro sotto, chiaro sopra) per vedersi su ogni sfondo.
    if (u.ring.w > 0.0) {
        let d = distance(out_px, u.ring.xy);
        let thickness = max(u.ring.z * 0.12, 1.5);
        let edge = abs(d - u.ring.z);
        let a_dark = (1.0 - smoothstep(thickness * 0.5, thickness * 0.5 + 1.5, edge - thickness * 0.4)) * 0.35;
        let a_light = 1.0 - smoothstep(thickness * 0.5 - 0.5, thickness * 0.5 + 0.75, edge);
        color = mix(color, vec3f(0.0), a_dark * u.ring.w);
        color = mix(color, vec3f(1.0), a_light * u.ring.w * 0.9);
    }

    // Cursore: sprite premoltiplicato, scalato all'altezza voluta.
    if (u.cursor.w > 0.0) {
        let k = u.cursor.z / u.cursor_img.y; // pixel di uscita per pixel dello sprite
        let top_left = u.cursor.xy - u.cursor_img.zw * k;
        let local = (out_px - top_left) / k; // pixel dello sprite
        if (all(local >= vec2f(0.0)) && all(local < u.cursor_img.xy)) {
            let c = textureSampleLevel(tex_cursor, samp, local / u.cursor_img.xy, 0.0);
            color = c.rgb * u.cursor.w + color * (1.0 - c.a * u.cursor.w);
        }
    }
    return vec4f(color, 1.0);
}
