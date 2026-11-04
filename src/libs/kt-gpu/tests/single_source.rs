//! t1501 (compute-backends, capabilities `plugins`, `video-fx`): the WGSL form of Color Grade
//! written once compiles on the GPU in the own-pass frame `assembled_shaders` gives a point-wise
//! plugin, and stays within the kind's `FrameTol` of the CPU reference — as the hand-written
//! `color_grade.wgsl` does. The benchmark below (`--ignored`) gives the GPU numbers of F-EXT-004.
//!
//! The harness is raw wgpu on purpose: the prototype must not enter the engine (the plugin keeps
//! its hand-written WGSL), and both texts run in the SAME harness, so their numbers compare.

use fx_video_color::single_source::{color_grade, shipped_frame, tap_args};
use kt_gpu::{FrameTol, GpuContext, assert_frames_close};
use kt_plugin_api::Plugin;

const HAND_WRITTEN: &str = include_str!("../../../plugins/fx-video-color/src/color_grade.wgsl");

/// The own pass of a point-wise plugin, as `assembled_shaders` writes it, over a minimal header.
fn own_pass(tap_wgsl: &str) -> String {
    format!(
        "struct Params {{ f0: vec4<f32>, f1: vec4<f32>, }};\n\
         @group(0) @binding(0) var<uniform> P: Params;\n\
         @group(0) @binding(1) var T0: texture_2d<u32>;\n\
         @vertex fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {{\n    \
         let x = f32(i32(vi / 2u)) * 4.0 - 1.0;\n    let y = f32(i32(vi & 1u)) * 4.0 - 1.0;\n    \
         return vec4<f32>(x, y, 0.0, 1.0);\n}}\n\
         {tap_wgsl}\n@fragment\nfn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<u32> {{\n    \
         let t = textureLoad(T0, vec2<i32>(pos.xy), 0);\n    \
         let v = tap_color_grade(vec4<f32>(f32(t.x), f32(t.y), f32(t.z), f32(t.w)), P.f0, P.f1, floor(pos.xy));\n    \
         return vec4<u32>(clamp(round(v), vec4<f32>(0.0), vec4<f32>(255.0)));\n}}\n"
    )
}

struct Harness {
    ctx: GpuContext,
    w: u32,
    h: u32,
    src: wgpu::Texture,
    dst: wgpu::Texture,
    staging: wgpu::Buffer,
    bpr: u32,
    uniform: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    pipe_layout: wgpu::PipelineLayout,
}

impl Harness {
    fn new(ctx: GpuContext, w: u32, h: u32) -> Harness {
        let d = &ctx.device;
        let tex = |usage| {
            d.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Uint,
                usage,
                view_formats: &[],
            })
        };
        let src = tex(wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST);
        let dst = tex(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let bpr = (w * 4).next_multiple_of(256);
        let staging = d.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bpr as u64 * h as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let uniform = d.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pipe_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        Harness { ctx, w, h, src, dst, staging, bpr, uniform, layout, pipe_layout }
    }

    /// The pipeline of one tap text; `Err` is the compiler's or the validation's refusal.
    fn pipeline(&self, tap_wgsl: &str) -> Result<wgpu::RenderPipeline, String> {
        let d = &self.ctx.device;
        let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(own_pass(tap_wgsl).into()),
        });
        let pipe = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&self.pipe_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Uint,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        match pollster::block_on(scope.pop()) {
            Some(e) => Err(e.to_string()),
            None => Ok(pipe),
        }
    }

    fn upload(&self, rgba: &[u8], params: &[f32; 8]) {
        let q = &self.ctx.queue;
        q.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.src,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.w * 4),
                rows_per_image: Some(self.h),
            },
            wgpu::Extent3d { width: self.w, height: self.h, depth_or_array_layers: 1 },
        );
        let bytes: Vec<u8> = params.iter().flat_map(|f| f.to_le_bytes()).collect();
        q.write_buffer(&self.uniform, 0, &bytes);
    }

    /// `passes` runs of the pipeline in one submission, then (optionally) the readback; waits.
    fn run(&self, pipe: &wgpu::RenderPipeline, passes: usize, readback: Option<&mut Vec<u8>>) {
        let d = &self.ctx.device;
        let view_src = self.src.create_view(&Default::default());
        let view_dst = self.dst.create_view(&Default::default());
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view_src),
                },
            ],
        });
        let mut enc = d.create_command_encoder(&Default::default());
        for _ in 0..passes {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view_dst,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(pipe);
            rp.set_bind_group(0, &bind, &[]);
            rp.draw(0..3, 0..1);
        }
        let want = readback.is_some();
        if want {
            enc.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.dst,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &self.staging,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(self.bpr),
                        rows_per_image: Some(self.h),
                    },
                },
                wgpu::Extent3d { width: self.w, height: self.h, depth_or_array_layers: 1 },
            );
        }
        self.ctx.queue.submit([enc.finish()]);
        let Some(out) = readback else {
            d.poll(wgpu::PollType::wait_indefinitely()).expect("device lost");
            return;
        };
        let slice = self.staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        d.poll(wgpu::PollType::wait_indefinitely()).expect("device lost");
        rx.recv().unwrap().expect("map failed");
        out.clear();
        {
            let data = slice.get_mapped_range().expect("mapped");
            for row in 0..self.h as usize {
                let s = row * self.bpr as usize;
                out.extend_from_slice(&data[s..s + self.w as usize * 4]);
            }
        }
        self.staging.unmap();
    }
}

fn frame(w: u32, h: u32) -> Vec<u8> {
    let mut v = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let s = x.wrapping_mul(2654435761).wrapping_add(y.wrapping_mul(40503));
            v[i] = (x * 7 + y) as u8;
            v[i + 1] = (s >> 13) as u8;
            v[i + 2] = (x ^ (y * 3)) as u8;
            v[i + 3] = (x + y * 5) as u8;
        }
    }
    v
}

fn cooked(p: &[f64]) -> [f64; 8] {
    fx_video_color::ColorGrade::new().video().unwrap().cook(p)
}

/// The uniform both texts read: `gpu_params` packed as the engine packs a point-wise plugin.
fn uniform(k: &[f64; 8]) -> [f32; 8] {
    let (a, b) = tap_args(k);
    [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]].map(|x| x as f32)
}

fn harness(w: u32, h: u32) -> Option<Harness> {
    match GpuContext::new_headless() {
        Ok(ctx) => Some(Harness::new(ctx, w, h)),
        Err(e) => {
            if std::env::var("KT_REQUIRE_GPU").as_deref() == Ok("1") {
                panic!("KT_REQUIRE_GPU=1, but no wgpu adapter: {e}");
            }
            eprintln!("skipped: wgpu adapter unavailable ({e})");
            None
        }
    }
}

#[test]
fn t1501_single_text_wgsl_runs_within_the_kinds_tolerance() {
    let (w, h) = (192, 108);
    let Some(hx) = harness(w, h) else { return };
    let single = hx
        .pipeline(color_grade::WGSL)
        .unwrap_or_else(|e| panic!("the WGSL form refused: {e}\n{}", color_grade::WGSL));
    let hand = hx.pipeline(HAND_WRITTEN).expect("the hand-written color_grade.wgsl");
    let src = frame(w, h);
    for p in [
        [0.12, 0.35, 1.6, 60.0],
        [0.0, 0.0, 1.0, 0.0],
        [-0.4, -0.8, 0.0, -180.0],
        [1.0, 1.0, 2.0, 180.0],
    ] {
        let k = cooked(&p);
        let mut cpu = src.clone();
        shipped_frame(&k, &mut cpu);
        hx.upload(&src, &uniform(&k));
        let (mut g1, mut g2) = (Vec::new(), Vec::new());
        hx.run(&single, 1, Some(&mut g1));
        hx.run(&hand, 1, Some(&mut g2));
        // color_grade's class in t181 (`parity.rs`): BILINEAR on both backends
        assert_frames_close(&cpu, &g1, w, h, FrameTol::BILINEAR, &format!("single text {p:?}"));
        assert_frames_close(&cpu, &g2, w, h, FrameTol::BILINEAR, &format!("hand-written {p:?}"));
    }
}

fn best_of(reps: usize, mut f: impl FnMut()) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..reps {
        let t = std::time::Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64() * 1e3);
    }
    best
}

/// Differences from the CPU reference: (channels that differ, the largest |Δ|).
fn diff(a: &[u8], b: &[u8]) -> (usize, u8) {
    a.iter()
        .zip(b)
        .fold((0, 0), |(n, m), (x, y)| if x == y { (n, m) } else { (n + 1, m.max(x.abs_diff(*y))) })
}

/// The F-EXT-004 GPU rows: the pass alone (20 passes in one submission, per pass) and the round
/// trip a CPU-side caller pays (upload + pass + readback), for both texts, 1080p and 4K.
/// `cargo test --release -p kt-gpu --test single_source -- --ignored --nocapture`.
#[test]
#[ignore]
fn bench_color_grade_gpu() {
    let k = cooked(&[0.12, 0.35, 1.6, 60.0]);
    for (name, w, h) in [("1080p", 1920u32, 1080u32), ("4K", 3840, 2160)] {
        let Some(hx) = harness(w, h) else { return };
        if name == "1080p" {
            let i = &hx.ctx.adapter_info;
            println!(
                "adapter: {} ({:?}, driver {} {})",
                i.name, i.backend, i.driver, i.driver_info
            );
        }
        let src = frame(w, h);
        let mut cpu = src.clone();
        shipped_frame(&k, &mut cpu);
        hx.upload(&src, &uniform(&k));
        let mut outs = Vec::new();
        for (label, text) in [("hand-written", HAND_WRITTEN), ("single text", color_grade::WGSL)] {
            let t0 = std::time::Instant::now();
            let pipe = hx.pipeline(text).unwrap();
            let compile = t0.elapsed().as_secs_f64() * 1e3;
            let mut out = Vec::new();
            hx.run(&pipe, 1, Some(&mut out));
            let pass = best_of(5, || hx.run(&pipe, 20, None)) / 20.0;
            let trip = best_of(8, || {
                hx.upload(&src, &uniform(&k));
                hx.run(&pipe, 1, Some(&mut out));
            });
            let (n, m) = diff(&cpu, &out);
            println!(
                "{name} {label}: pipeline {compile:.1} ms; pass {pass:.3} ms; upload+pass+readback {trip:.2} ms; \
                 vs CPU reference {n} of {} channels differ, max {m}",
                cpu.len()
            );
            outs.push(out);
        }
        let (n, m) = diff(&outs[0], &outs[1]);
        println!("{name}: hand-written vs single text on the GPU: {n} channels differ, max {m}");
    }
}
