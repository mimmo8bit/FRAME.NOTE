//! Compositore su GPU con wgpu: lo stesso codice servirà all'anteprima in
//! tempo reale (M3/M7). Qui lavora "offline": frame in ingresso, RGBA in uscita.

use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

/// Parametri di un singolo frame, già in pixel di uscita dove serve.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FrameParams {
    /// Inquadratura nella sorgente: x0, y0, larghezza, altezza.
    pub viewport: [f32; 4],
    /// Punta del cursore in pixel di uscita.
    pub cursor_xy: [f32; 2],
    /// Altezza dello sprite del cursore in pixel di uscita.
    pub cursor_height: f32,
    pub cursor_opacity: f32,
    /// Anello del clic: centro, raggio (pixel di uscita) e opacità.
    pub ring: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    viewport: [f32; 4],
    sizes: [f32; 4],
    cursor: [f32; 4],
    cursor_img: [f32; 4],
    ring: [f32; 4],
}

pub struct Compositor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniforms: wgpu::Buffer,
    tex_y: wgpu::Texture,
    tex_uv: wgpu::Texture,
    target: wgpu::Texture,
    readback: wgpu::Buffer,
    src: (u32, u32),
    out: (u32, u32),
    padded_row: u32,
    cursor_img: [f32; 4],
    pub adapter_name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    #[error("nessuna GPU disponibile per il montaggio ({0})")]
    NoAdapter(String),
    #[error("la GPU ha rifiutato la richiesta ({0})")]
    Device(String),
    #[error("errore durante la lettura del frame dalla GPU ({0})")]
    Readback(String),
}

fn texture(
    device: &wgpu::Device,
    label: &str,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

fn write_tex(queue: &wgpu::Queue, tex: &wgpu::Texture, data: &[u8], bytes_per_pixel: u32) {
    let size = tex.size();
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size.width * bytes_per_pixel),
            rows_per_image: Some(size.height),
        },
        size,
    );
}

impl Compositor {
    /// `src`: dimensioni della sorgente NV12 (pari). `out`: dimensioni dell'uscita.
    pub fn new(src: (u32, u32), out: (u32, u32)) -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| GpuError::NoAdapter(e.to_string()))?;
        let adapter_name = format!(
            "{} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("framenote"),
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|e| GpuError::Device(e.to_string()))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compose"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/compose.wgsl").into()),
        });
        let sampled = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let tex_y = texture(
            &device,
            "y",
            src.0,
            src.1,
            wgpu::TextureFormat::R8Unorm,
            sampled,
        );
        let tex_uv = texture(
            &device,
            "uv",
            src.0 / 2,
            src.1 / 2,
            wgpu::TextureFormat::Rg8Unorm,
            sampled,
        );
        let sprite = super::sprite::cursor_sprite();
        let tex_cursor = texture(
            &device,
            "cursor",
            sprite.width,
            sprite.height,
            wgpu::TextureFormat::Rgba8Unorm,
            sampled,
        );
        write_tex(&queue, &tex_cursor, &sprite.rgba, 4);
        let target = texture(
            &device,
            "target",
            out.0,
            out.1,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let padded_row = (out.0 * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: padded_row as u64 * out.1 as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let uniforms = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniforms"),
            contents: bytemuck::bytes_of(&Uniforms::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compose"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex_entry(1),
                tex_entry(2),
                tex_entry(3),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let view = |t: &wgpu::Texture| t.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compose"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view(&tex_y)),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&view(&tex_uv)),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&view(&tex_cursor)),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compose"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("compose"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group,
            uniforms,
            tex_y,
            tex_uv,
            target,
            readback,
            src,
            out,
            padded_row,
            cursor_img: [
                sprite.width as f32,
                sprite.height as f32,
                sprite.hotspot.0,
                sprite.hotspot.1,
            ],
            adapter_name,
        })
    }

    /// Compone un frame. `nv12`: piano Y seguito dal piano UV interlacciato,
    /// senza padding. `out_rgba` riceve `larghezza × altezza × 4` byte.
    pub fn render(
        &mut self,
        nv12: &[u8],
        p: &FrameParams,
        out_rgba: &mut Vec<u8>,
    ) -> Result<(), GpuError> {
        let (w, h) = self.src;
        let y_len = (w * h) as usize;
        write_tex(&self.queue, &self.tex_y, &nv12[..y_len], 1);
        write_tex(
            &self.queue,
            &self.tex_uv,
            &nv12[y_len..y_len + y_len / 2],
            2,
        );
        let u = Uniforms {
            viewport: p.viewport,
            sizes: [w as f32, h as f32, self.out.0 as f32, self.out.1 as f32],
            cursor: [
                p.cursor_xy[0],
                p.cursor_xy[1],
                p.cursor_height,
                p.cursor_opacity,
            ],
            cursor_img: self.cursor_img,
            ring: p.ring,
        };
        self.queue
            .write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));

        let target_view = self
            .target
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("compose"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_row),
                    rows_per_image: Some(self.out.1),
                },
            },
            self.target.size(),
        );
        self.queue.submit([enc.finish()]);

        let slice = self.readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| GpuError::Readback(e.to_string()))?;
        rx.recv()
            .map_err(|e| GpuError::Readback(e.to_string()))?
            .map_err(|e| GpuError::Readback(e.to_string()))?;
        {
            let data = slice
                .get_mapped_range()
                .map_err(|e| GpuError::Readback(e.to_string()))?;
            let row = (self.out.0 * 4) as usize;
            out_rgba.clear();
            out_rgba.reserve(row * self.out.1 as usize);
            for r in 0..self.out.1 as usize {
                let start = r * self.padded_row as usize;
                out_rgba.extend_from_slice(&data[start..start + row]);
            }
        }
        self.readback.unmap();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Senza GPU (nemmeno software) i test del renderer non possono girare: in
    /// CI `FRAMENOTE_REQUIRE_GPU=1` li rende obbligatori.
    fn compositor(src: (u32, u32), out: (u32, u32)) -> Option<Compositor> {
        match Compositor::new(src, out) {
            Ok(c) => Some(c),
            Err(e) if std::env::var_os("FRAMENOTE_REQUIRE_GPU").is_none() => {
                eprintln!("GPU non disponibile, test saltato: {e}");
                None
            }
            Err(e) => panic!("GPU richiesta ma non disponibile: {e}"),
        }
    }

    /// Sorgente NV12: metà sinistra nera, metà destra bianca, colore neutro.
    fn half_black_half_white(w: u32, h: u32) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 3 / 2) as usize);
        for _ in 0..h {
            for x in 0..w {
                v.push(if x < w / 2 { 16 } else { 235 });
            }
        }
        v.resize((w * h * 3 / 2) as usize, 128);
        v
    }

    fn px(img: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * w + x) * 4) as usize;
        [img[i], img[i + 1], img[i + 2], img[i + 3]]
    }

    fn full(w: u32, h: u32) -> FrameParams {
        FrameParams {
            viewport: [0.0, 0.0, w as f32, h as f32],
            ..Default::default()
        }
    }

    #[test]
    fn inquadratura_intera_ridotta() {
        let (sw, sh, ow, oh) = (640, 400, 320, 200);
        let Some(mut c) = compositor((sw, sh), (ow, oh)) else {
            return;
        };
        let mut out = Vec::new();
        c.render(&half_black_half_white(sw, sh), &full(sw, sh), &mut out)
            .unwrap();
        assert_eq!(out.len(), (ow * oh * 4) as usize);
        assert!(px(&out, ow, 40, 100)[0] < 5, "sinistra nera");
        assert!(px(&out, ow, 280, 100)[0] > 250, "destra bianca");
        assert_eq!(px(&out, ow, 280, 100)[3], 255);
    }

    #[test]
    fn zoom_2x_sulla_meta_destra() {
        let (sw, sh, ow, oh) = (640, 400, 320, 200);
        let Some(mut c) = compositor((sw, sh), (ow, oh)) else {
            return;
        };
        let mut out = Vec::new();
        // Inquadratura 2× centrata nel quarto destro: tutta bianca.
        let p = FrameParams {
            viewport: [400.0, 100.0, 320.0, 200.0],
            ..Default::default()
        };
        c.render(&half_black_half_white(sw, sh), &p, &mut out)
            .unwrap();
        for (x, y) in [(2, 2), (160, 100), (317, 197)] {
            assert!(
                px(&out, ow, x, y)[0] > 250,
                "({x},{y}) {:?}",
                px(&out, ow, x, y)
            );
        }
        // Inquadratura 2× sul confine: il bordo nero/bianco è al centro dell'uscita.
        let p = FrameParams {
            viewport: [240.0, 100.0, 160.0, 100.0],
            ..Default::default()
        };
        c.render(&half_black_half_white(sw, sh), &p, &mut out)
            .unwrap();
        assert!(px(&out, ow, 150, 100)[0] < 10);
        assert!(px(&out, ow, 170, 100)[0] > 245);
    }

    #[test]
    fn cursore_e_anello() {
        let (sw, sh, ow, oh) = (640, 400, 640, 400);
        let Some(mut c) = compositor((sw, sh), (ow, oh)) else {
            return;
        };
        // Sorgente grigia uniforme.
        let mut gray = vec![126u8; (sw * sh) as usize];
        gray.resize((sw * sh * 3 / 2) as usize, 128);
        let mut out = Vec::new();
        let mut p = full(sw, sh);
        p.cursor_xy = [100.0, 100.0];
        p.cursor_height = 60.0;
        p.cursor_opacity = 1.0;
        c.render(&gray, &p, &mut out).unwrap();
        let bg = px(&out, ow, 500, 300)[0];
        // Dentro la freccia (poco sotto e a destra della punta) il cursore è scuro.
        let inside = px(&out, ow, 106, 120)[0];
        assert!(inside < bg / 3, "inside={inside} bg={bg}");
        // A sinistra della punta resta lo sfondo.
        assert!(px(&out, ow, 80, 120)[0].abs_diff(bg) < 3);
        // Cursore invisibile: tutto sfondo.
        p.cursor_opacity = 0.0;
        c.render(&gray, &p, &mut out).unwrap();
        assert!(px(&out, ow, 106, 120)[0].abs_diff(bg) < 3);
        // Anello: chiaro sulla circonferenza, sfondo al centro.
        p.ring = [320.0, 200.0, 40.0, 1.0];
        c.render(&gray, &p, &mut out).unwrap();
        assert!(px(&out, ow, 360, 200)[0] > bg + 60);
        assert!(px(&out, ow, 320, 200)[0].abs_diff(bg) < 3);
    }
}
