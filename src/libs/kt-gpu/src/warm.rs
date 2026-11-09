//! Warm draws (pipeline-prewarm): one draw of a pipeline into a target of a few texels, off the
//! frame path. A software rasterizer (llvmpipe, lavapipe) compiles a pipeline's code at its FIRST
//! DRAW, not when it is created: on an empty Mesa cache the first composite took 10.5 s and Camera
//! ISP's first frame 4.6 s, while creating every pipeline took 1.9 s (`kt-cli first-frame`). A card's
//! driver compiles at creation, and the warm draw costs it a few microseconds.
//!
//! The kit's resources are made apart from `crate::created()`: they are not a frame's (t2023 counts
//! what a frame makes, and a warm thread runs beside frames).

use std::collections::HashMap;

use crate::pool::{DISPLAY_FORMAT, FORMAT, WORD_FORMAT};

/// The side of the kit's textures: not a power of two, as a frame's are not, so a driver whose
/// code variant depends on that picks the frame's variant.
const SIDE: u32 = 6;

/// Which bind group layout a pipeline takes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layout {
    /// The shared one: a 128 B uniform and three textures.
    Shared,
    /// The fused geometry pass's: a 1024 B uniform and one texture.
    Geom,
}

/// What a warm draw needs, made once per device.
pub(crate) struct Kit {
    shared: wgpu::BindGroup,
    geom: wgpu::BindGroup,
    targets: HashMap<wgpu::TextureFormat, wgpu::TextureView>,
}

fn texture(device: &wgpu::Device, format: wgpu::TextureFormat, usage: wgpu::TextureUsages) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("kt-gpu warm"),
            size: wgpu::Extent3d { width: SIDE, height: SIDE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

impl Kit {
    pub(crate) fn new(device: &wgpu::Device, shared: &wgpu::BindGroupLayout, geom: &wgpu::BindGroupLayout) -> Kit {
        // zeros: every loop bound a uniform gives is 0, every size is 0 — the cheapest run of the code
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kt-gpu warm uniform"),
            size: 1024,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        let input = texture(device, FORMAT, wgpu::TextureUsages::TEXTURE_BINDING);
        let buf = |size: u64| {
            wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &uniform, offset: 0, size: wgpu::BufferSize::new(size) })
        };
        let tex = |binding: u32| wgpu::BindGroupEntry { binding, resource: wgpu::BindingResource::TextureView(&input) };
        let shared = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kt-gpu warm"),
            layout: shared,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: buf(128) }, tex(1), tex(2), tex(3)],
        });
        let geom = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kt-gpu warm geom"),
            layout: geom,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: buf(1024) }, tex(1)],
        });
        let targets = [FORMAT, WORD_FORMAT, DISPLAY_FORMAT]
            .into_iter()
            .map(|f| (f, texture(device, f, wgpu::TextureUsages::RENDER_ATTACHMENT)))
            .collect();
        Kit { shared, geom, targets }
    }

    /// Draw each pipeline once and wait for the device: when this returns, their code is compiled.
    pub(crate) fn draw(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipes: &[(wgpu::RenderPipeline, Layout, wgpu::TextureFormat)],
    ) {
        if pipes.is_empty() {
            return;
        }
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("kt-gpu warm") });
        for (pipe, layout, format) in pipes {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kt-gpu warm"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.targets[format],
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(pipe);
            match layout {
                Layout::Shared => rp.set_bind_group(0, &self.shared, &[0]),
                Layout::Geom => rp.set_bind_group(0, &self.geom, &[0]),
            }
            rp.draw(0..3, 0..1);
        }
        queue.submit([enc.finish()]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
    }
}
