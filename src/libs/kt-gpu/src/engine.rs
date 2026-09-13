//! GPU-исполнитель FramePlan: фрагментные пассы (НЕ compute — дефолтный
//! device egui-wgpu на GL-бэкенде живёт в downlevel-лимитах, а live-check
//! гоняет llvmpipe), ping-pong композит, readback финального кадра.
//! Семантика зеркалит CPU-исполнитель kt-engine (execute_plan_cpu):
//! целочисленные ядра байт-в-байт, билинейные — в допуске (t181/t182).

use std::sync::{Arc, Mutex};

use kt_engine::{
    Cam3d, FramePlan, GpuBackendInfo, GpuExecError, LayerSrc, PlanLayer, TransMix, VideoGpuExec,
    video_plan::SoloFade,
};
use kt_model::FxInstance;
use kt_time::Tick;
use xxhash_rust::xxh3::Xxh3;

use crate::context::{GpuContext, GpuInitError};
use crate::pool::{FORMAT, ReadbackCache, Tex, TexturePool, UploadCache, create_tex};

const SLOT: u64 = 256; // выравнивание dynamic offset (максимум по спеке)
/// Юниформа слитого гео-пасса (Р.44): программа примитивов не влезает в
/// общие 128 Б — свой bind group layout и 4 слота арены на пасс.
const GEOM_UNIFORM_BYTES: usize = 1024;
const GEOM_SLOT_UNITS: u32 = (GEOM_UNIFORM_BYTES as u64 / SLOT) as u32;
const UPLOAD_BUDGET: u64 = 256 * 1024 * 1024;
const TRANSIENT_BUDGET: u64 = 128 * 1024 * 1024;
const READBACK_BUDGET: u64 = 64 * 1024 * 1024;

/// Униформа всех пассов: 2×vec4<u32> + 2×vec4<f32> + mat4 (см. common.wgsl).
#[derive(Default, Clone, Copy)]
struct UniformData {
    u: [u32; 8],
    f: [f32; 8],
    /// MVP 3D-пассов (column-major, как ждёт WGSL mat4x4).
    m: [f32; 16],
}

impl UniformData {
    fn bytes(&self) -> [u8; 128] {
        let mut b = [0u8; 128];
        for (i, v) in self.u.iter().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (i, v) in self.f.iter().enumerate() {
            b[32 + i * 4..32 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (i, v) in self.m.iter().enumerate() {
            b[64 + i * 4..64 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
}

/// Юниформа geom_fused.wgsl (зеркало структуры GeomParams): заголовок +
/// vec4-слоты примитивов. 1024 Б = GEOM_SLOT_UNITS слотов арены.
struct GeomUniform {
    h: [u32; 4],
    d: [u32; 4],
    p: [[f32; 4]; 62],
}

impl GeomUniform {
    fn bytes(&self) -> [u8; GEOM_UNIFORM_BYTES] {
        let mut b = [0u8; GEOM_UNIFORM_BYTES];
        for (i, v) in self.h.iter().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (i, v) in self.d.iter().enumerate() {
            b[16 + i * 4..16 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (s, sl) in self.p.iter().enumerate() {
            for (c, v) in sl.iter().enumerate() {
                let o = 32 + s * 16 + c * 4;
                b[o..o + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        b
    }
}

/// Упаковка программы kt-fx в юниформу: коэффициенты уже перемножены в f64
/// при построении — здесь ОДНОКРАТНОЕ округление f64→f32 каждого числа.
fn encode_geom(prog: &kt_fx::geom::GeomProgram) -> GeomUniform {
    use kt_fx::geom::GeomPrim;
    let mut u = GeomUniform { h: [0; 4], d: [0; 4], p: [[0.0; 4]; 62] };
    let mut kinds: u64 = 0;
    let mut cur = 0usize;
    debug_assert!(prog.prims.len() <= kt_fx::geom::MAX_PRIMS);
    for (i, p) in prog.prims.iter().enumerate() {
        let kind: u64 = match p {
            GeomPrim::Affine(a) => {
                u.p[cur] = [a.ax as f32, a.bx as f32, a.ay as f32, a.by as f32];
                cur += 1;
                0
            }
            GeomPrim::Clamp { x0, x1, y0, y1 } => {
                u.p[cur] = [*x0 as f32, *x1 as f32, *y0 as f32, *y1 as f32];
                cur += 1;
                1
            }
            GeomPrim::Fold { gw, gh, cw, ch, in_w, in_h } => {
                u.p[cur] = [*gw as f32, *gh as f32, *cw as f32, *ch as f32];
                u.p[cur + 1] = [*in_w as f32, *in_h as f32, 0.0, 0.0];
                cur += 2;
                2
            }
            GeomPrim::Proj { m } => {
                for (r, row) in m.iter().enumerate() {
                    u.p[cur + r] = [row[0] as f32, row[1] as f32, row[2] as f32, 0.0];
                }
                cur += 3;
                3
            }
        };
        kinds |= kind << (i * 4);
    }
    debug_assert!(cur <= 62, "geom program exceeds uniform slots");
    // суперсемплинг (п.309): тапы аффинных серий посчитаны на CPU и едут
    // числом; Proj-серии меряют якобиан по-пиксельно ТОЙ ЖЕ формулой
    let ss = match prog.ss {
        kt_fx::geom::SsSpec::Const { nx, ny } => (nx << 8) | (ny << 16),
        kt_fx::geom::SsSpec::PerPixel => 1,
    };
    u.h = [prog.prims.len() as u32, kinds as u32, (kinds >> 32) as u32, ss];
    u.d = [prog.in_w, prog.in_h, prog.out_w, prog.out_h];
    u
}

#[derive(Clone, Copy)]
enum Pipe {
    Resample,
    Mirror,
    Noise,
    Greenscreen,
    Edges,
    Vhs,
    GlowAdd,
    Composite,
    /// Сила наложения слоя (Р.38 п.12): смешение сухой и мокрой сторон.
    LayerMix,
}

const PIPE_COUNT: usize = 9;

/// Один шаг цепочки, предвычисленный на CPU (общая математика — kt-fx:
/// pan_frame/cam_at/noise_seed). Кол-во пассов и размеры известны заранее.
enum FxOp {
    /// Билинейная рамка (Pan/CamMotion/Blur down+up): выход tw×th.
    Resample { tw: u32, th: u32, frame: (f64, f64, f64, f64) },
    Mirror { h: bool, v: bool },
    Noise { seed: u32, k: u32, size: u32, color: bool },
    Greenscreen { key: [f32; 3], tol: f32, hard: f32, spill: f32, kc: u32 },
    Edges { th: u32, d: u32, color: [u32; 3] },
    Vhs { shift: u32, line_off: u32, bleed: f32, jq: i32, step: u32, seed: u32 },
    /// Bloom: даунскейл dw×dh → апскейл → аддитив ×s (3 пасса).
    Glow { dw: u32, dh: u32, s: f32, th: u32, tint: [f32; 3] },
    /// Слитая гео-серия (Р.44): программа обратных отображений строится в
    /// kt-fx (общий код с CPU-исполнителем), один пасс, юниформа 1024 Б.
    /// EventGrid — всегда этим путём (мозаика прежних 2 пассов удалена).
    GeomFused { prog: kt_fx::geom::GeomProgram },
    /// Снимок состояния ДО вклада слоя (Р.38 п.12): сухая сторона
    /// будущего смешения. Только маркер — пассов не порождает.
    Snapshot,
    /// Смешать текущий результат со снимком весом `k` (0..4096).
    /// Размеры могли разойтись (Pan/CamMotion ресемплят) — тогда сухая
    /// сторона вписывается ТОЙ ЖЕ билинейкой, что и на CPU.
    Mix { k: u32 },
}

impl FxOp {
    fn passes(&self) -> u32 {
        match self {
            FxOp::Glow { .. } => 3,
            // снимок пассов не порождает; смешение при разошедшихся
            // размерах добирает ресемпл сухой стороны
            FxOp::Snapshot => 0,
            FxOp::Mix { .. } => 2,
            _ => 1,
        }
    }

    /// Слоты юниформ-арены (SLOT = 256 Б): обычный пасс — один слот,
    /// слитый гео-пасс — GEOM_SLOT_UNITS. Сумма обязана ТОЧНО предсказывать
    /// потребление арены (debug_assert в pass/pass_geom).
    fn slot_units(&self) -> u32 {
        match self {
            FxOp::GeomFused { .. } => GEOM_SLOT_UNITS,
            _ => self.passes(),
        }
    }
}

/// Слагаемые ключа ReadbackCache от разрешённой камеры (Р.29).
fn put_cam3d(h: &mut Xxh3, c: &Option<Cam3d>) {
    match c {
        None => h.update(&[0u8]),
        Some(c) => {
            h.update(&[1u8, c.cam.ortho as u8]);
            for v in [
                c.cam.pos_x,
                c.cam.pos_y,
                c.cam.pos_z,
                c.cam.rot_x,
                c.cam.rot_y,
                c.cam.rot_z,
                c.cam.fov_deg,
                c.world_scale,
            ] {
                h.update(&v.to_bits().to_le_bytes());
            }
            if let Some((x, y, z)) = c.cam.target {
                for v in [x, y, z] {
                    h.update(&v.to_bits().to_le_bytes());
                }
            }
        }
    }
}

/// Цепочка одного слоя: ops + предсказанные выходные размеры/альфа.
/// Зеркало трансформаций apply_video_chain_pooled (kt-fx/src/video.rs).
#[allow(clippy::too_many_arguments)]
fn chain_ops(
    fx: &[FxInstance],
    src_w: u32,
    src_h: u32,
    out_w: u32,
    out_h: u32,
    u: f64,
    t: Tick,
    cam3d: Option<Cam3d>,
) -> (Vec<FxOp>, (u32, u32), bool, bool) {
    let (mut w, mut h) = (src_w, src_h);
    let mut has_alpha_known = false; // выставлял ли эффект альфу явно
    let mut has_alpha = false;
    let mut ops = Vec::new();
    let (mow, moh) = (out_w.max(2), out_h.max(2));
    // границы вклада слоёв (Р.38 п.12): на них делается снимок сухой
    // стороны будущего смешения
    let froms: Vec<usize> = fx
        .iter()
        .enumerate()
        .filter_map(|(i, f)| match f {
            FxInstance::LayerMix { from, .. } => Some((*from as usize).min(i)),
            _ => None,
        })
        .collect();
    let mut idx = 0usize;
    while idx < fx.len() {
        if froms.contains(&idx) {
            ops.push(FxOp::Snapshot);
        }
        // Гео-серия (Р.44): сегментацию и выбор legacy/fused делает kt-fx —
        // ОБЩИЙ код обоих бэкендов (инвариант №7). Снапшот-точки силы слоя
        // рвут серию так же, как apply_chain_with_mixes на CPU.
        if kt_fx::geom::is_geo(&fx[idx]) {
            let end = kt_fx::geom::geo_run_end(fx, idx, &froms);
            let steps: Vec<&FxInstance> =
                fx[idx..end].iter().filter(|f| kt_fx::geom::is_geo(f)).collect();
            match kt_fx::geom::build_geom_exec(&steps, w, h, out_w, out_h, u, cam3d) {
                kt_fx::geom::GeomExec::Legacy => match steps[0] {
                    FxInstance::Pan { mode, pan_x, pan_y, zoom } => {
                        let frame = kt_fx::video::pan_frame(
                            w, h, out_w, out_h, *mode, *pan_x, *pan_y, *zoom,
                        );
                        ops.push(FxOp::Resample { tw: mow, th: moh, frame });
                        w = mow;
                        h = moh;
                    }
                    FxInstance::CamMotion { mode, ease, time_scale, keyframes } => {
                        let (px, py, z) =
                            kt_fx::video::cam_at(keyframes, u, *ease, *time_scale);
                        let frame =
                            kt_fx::video::pan_frame(w, h, out_w, out_h, *mode, px, py, z);
                        ops.push(FxOp::Resample { tw: mow, th: moh, frame });
                        w = mow;
                        h = moh;
                    }
                    FxInstance::Mirror { horizontal, vertical } => {
                        if *horizontal || *vertical {
                            ops.push(FxOp::Mirror { h: *horizontal, v: *vertical });
                        }
                    }
                    _ => debug_assert!(false, "legacy geo must be Pan/CamMotion/Mirror"),
                },
                kt_fx::geom::GeomExec::Fused(prog) => {
                    w = prog.out_w;
                    h = prog.out_h;
                    if prog.has_alpha_known {
                        has_alpha_known = true;
                        has_alpha = prog.has_alpha;
                    }
                    ops.push(FxOp::GeomFused { prog });
                }
            }
            idx = end;
            continue;
        }
        match &fx[idx] {
            FxInstance::Blur { radius, radius_y } => {
                let radius = (*radius).clamp(1, 48);
                let step = (radius / 3).max(1) + 1;
                let step_y =
                    if *radius_y == 0 { step } else { ((*radius_y).clamp(1, 48) / 3).max(1) + 1 };
                let (dw, dh) = ((w / step).max(1), (h / step_y).max(1));
                ops.push(FxOp::Resample {
                    tw: dw,
                    th: dh,
                    frame: (0.0, 0.0, w as f64, h as f64),
                });
                ops.push(FxOp::Resample {
                    tw: w,
                    th: h,
                    frame: (0.0, 0.0, dw as f64, dh as f64),
                });
            }
            FxInstance::Greenscreen { key_r, key_g, key_b, tolerance, softness, spill } => {
                let tol = (*tolerance).clamp(1, 200);
                let soft = (*softness).clamp(0, 100);
                let hard = (tol as f64 - soft as f64).max(0.0);
                let key = [*key_r, *key_g, *key_b];
                ops.push(FxOp::Greenscreen {
                    key: [key[0] as f32, key[1] as f32, key[2] as f32],
                    tol: tol as f32,
                    hard: hard as f32,
                    spill: spill.clamp(0.0, 1.0) as f32,
                    kc: (0..3).max_by_key(|&i| key[i]).unwrap_or(1) as u32,
                });
                has_alpha_known = true;
                has_alpha = true;
            }
            FxInstance::Edges { threshold, thickness, color_r, color_g, color_b } => {
                ops.push(FxOp::Edges {
                    th: (*threshold).clamp(1, 255),
                    d: (*thickness).clamp(1, 8),
                    color: [*color_r as u32, *color_g as u32, *color_b as u32],
                });
                has_alpha_known = true;
                has_alpha = false;
            }
            FxInstance::Noise { amount, size, color } => {
                let k = (amount.clamp(0.0, 1.0) * 32.0) as i64;
                if k > 0 {
                    ops.push(FxOp::Noise {
                        seed: kt_fx::video::noise_seed(t.0),
                        k: k as u32,
                        size: (*size).clamp(1, 64),
                        color: *color,
                    });
                }
            }
            FxInstance::Vhs { rgb_shift, bleed, scan_step, jitter } => {
                ops.push(FxOp::Vhs {
                    shift: (*rgb_shift).clamp(0, 12),
                    line_off: ((t.0 / 3000) % 7) as u32,
                    bleed: bleed.clamp(0.0, 1.0) as f32,
                    step: (*scan_step).clamp(0, 32),
                    // квант дрожания — тот же, что считает CPU-ядро
                    jq: (jitter.clamp(0.0, 1.0) * 1024.0).round() as i32,
                    seed: kt_fx::video::noise_seed(t.0),
                });
            }
            FxInstance::Glow { strength, radius, threshold, tint_r, tint_g, tint_b } => {
                let s = strength.clamp(0.0, 2.0) * 0.35;
                if s > 0.0 {
                    let r = (*radius).clamp(1, 32);
                    ops.push(FxOp::Glow {
                        dw: (w / r).max(1),
                        dh: (h / r).max(1),
                        s: s as f32,
                        th: (*threshold).min(255),
                        tint: [*tint_r as f32, *tint_g as f32, *tint_b as f32],
                    });
                }
            }
            FxInstance::LayerMix { amount, .. } => {
                let k = (amount.clamp(0.0, 1.0) * 4096.0).round() as u32;
                if k < 4096 {
                    ops.push(FxOp::Mix { k });
                }
            }
            _ => {} // аудио-FX
        }
        idx += 1;
    }
    (ops, (w, h), has_alpha, has_alpha_known)
}

struct ChainSpec {
    ops: Vec<FxOp>,
    src_view: wgpu::TextureView,
    src_w: u32,
    src_h: u32,
    has_alpha: bool, // после цепочки
}

impl ChainSpec {
    fn slot_units(&self) -> u32 {
        self.ops.iter().map(|o| o.slot_units()).sum()
    }
}

enum Job {
    Single { chain: ChainSpec, alpha_k: u32, scale: bool, fade: Option<SoloFade> },
    Dissolve { a: ChainSpec, b: ChainSpec, k: u32, alpha_k: u32, mix: TransMix },
}

impl Job {
    /// Слоты юниформ-арены на джоб: композит-пасс + цепочки слоёв.
    fn slot_units(&self) -> u32 {
        1 + match self {
            Job::Single { chain, .. } => chain.slot_units(),
            Job::Dissolve { a, b, .. } => a.slot_units() + b.slot_units(),
        }
    }
}

struct Inner {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    layout: wgpu::BindGroupLayout,
    pipes: [wgpu::RenderPipeline; PIPE_COUNT],
    /// Слитый гео-пасс (Р.44): своя юниформа 1024 Б → свой layout/пайп.
    geom_layout: wgpu::BindGroupLayout,
    geom_pipe: wgpu::RenderPipeline,
    arena: wgpu::Buffer,
    arena_slots: u32,
    slot: u32,
    pool: TexturePool,
    upload: UploadCache,
    readback: ReadbackCache,
    staging: Option<(wgpu::Buffer, u32, u32, u32)>, // (buf, padded_bpr, w, h)
    dummy: Tex,
    max_dim: u32,
}

pub struct GpuFxEngine {
    info: GpuBackendInfo,
    inner: Mutex<Inner>,
}

const SHADERS: [(&str, &str); PIPE_COUNT] = [
    ("resample", include_str!("shaders/resample.wgsl")),
    ("mirror", include_str!("shaders/mirror.wgsl")),
    ("noise", include_str!("shaders/noise.wgsl")),
    ("greenscreen", include_str!("shaders/greenscreen.wgsl")),
    ("edges", include_str!("shaders/edges.wgsl")),
    ("vhs", include_str!("shaders/vhs.wgsl")),
    ("glow_add", include_str!("shaders/glow_add.wgsl")),
    ("composite", include_str!("shaders/composite.wgsl")),
    ("layer_mix", include_str!("shaders/layer_mix.wgsl")),
];

const COMMON: &str = include_str!("shaders/common.wgsl");

impl GpuFxEngine {
    /// Собирает пайплайны; ошибка компиляции шейдера/устройства → Err
    /// (вызывающий уходит на CPU-фолбэк с notice).
    pub fn new(ctx: GpuContext) -> Result<Arc<GpuFxEngine>, GpuInitError> {
        let device = ctx.device;
        let queue = ctx.queue;
        let ai = &ctx.adapter_info;
        let name_l = ai.name.to_lowercase();
        let info = GpuBackendInfo {
            name: ai.name.clone(),
            backend: format!("{:?}", ai.backend),
            software: ai.device_type == wgpu::DeviceType::Cpu
                || name_l.contains("llvmpipe")
                || name_l.contains("softpipe")
                || name_l.contains("swiftshader"),
        };

        let err_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kt-gpu layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    // ВЕРШИННЫЙ стейдж тоже читает униформу (vs3d Pan3d —
                    // MVP/якорь): FRAGMENT-only валил create_render_pipeline
                    // на настоящем железе и ронял ВЕСЬ движок в CPU (Р.31 п.1)
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(128),
                    },
                    count: None,
                },
                texture_entry(1),
                texture_entry(2),
                texture_entry(3),
            ],
        });
        let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kt-gpu pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipes: Vec<wgpu::RenderPipeline> = SHADERS
            .iter()
            .map(|(name, body)| {
                let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(name),
                    source: wgpu::ShaderSource::Wgsl(format!("{COMMON}\n{body}").into()),
                });
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(name),
                    layout: Some(&pipe_layout),
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
                            format: FORMAT,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    multiview_mask: None,
                    cache: None,
                })
            })
            .collect();
        // Слитый гео-пасс (Р.44): юниформа GEOM_UNIFORM_BYTES не влезает в
        // общий layout (128 Б) — свой layout и пайп из того же буфера-арены.
        let geom_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kt-gpu geom layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(GEOM_UNIFORM_BYTES as u64),
                    },
                    count: None,
                },
                texture_entry(1),
            ],
        });
        let geom_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kt-gpu geom pipeline layout"),
            bind_group_layouts: &[Some(&geom_layout)],
            immediate_size: 0,
        });
        let geom_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("geom_fused"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/geom_fused.wgsl").into()),
        });
        let geom_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("geom_fused"),
            layout: Some(&geom_pipe_layout),
            vertex: wgpu::VertexState {
                module: &geom_module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &geom_module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
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
        if let Some(e) = pollster::block_on(err_scope.pop()) {
            return Err(GpuInitError::Pipeline(e.to_string()));
        }
        let pipes: [wgpu::RenderPipeline; PIPE_COUNT] =
            pipes.try_into().map_err(|_| GpuInitError::Pipeline("pipe count".into()))?;

        let arena_slots = 256u32;
        let arena = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kt-gpu uniforms"),
            size: SLOT * arena_slots as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dummy = create_tex(&device, 1, 1, "kt-gpu dummy");
        let max_dim = device.limits().max_texture_dimension_2d;
        Ok(Arc::new(GpuFxEngine {
            info,
            inner: Mutex::new(Inner {
                device,
                queue,
                layout,
                pipes,
                geom_layout,
                geom_pipe,
                arena,
                arena_slots,
                slot: 0,
                pool: TexturePool::new(TRANSIENT_BUDGET),
                upload: UploadCache::new(UPLOAD_BUDGET),
                readback: ReadbackCache::new(READBACK_BUDGET),
                staging: None,
                dummy,
                max_dim,
            }),
        }))
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Uint,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

impl VideoGpuExec for GpuFxEngine {
    fn execute_into(&self, plan: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, GpuExecError> {
        self.inner.lock().unwrap().execute(plan, out)
    }

    fn info(&self) -> &GpuBackendInfo {
        &self.info
    }
}

impl Inner {
    fn execute(&mut self, plan: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, GpuExecError> {
        let (ow, oh) = (plan.out_w, plan.out_h);
        if ow > self.max_dim || oh > self.max_dim {
            return Err(GpuExecError::TooLarge { w: ow, h: oh });
        }
        let key = plan_key(plan);
        if let Some((data, complete)) = self.readback.get(key) {
            out.clear();
            out.extend_from_slice(data);
            return Ok(complete);
        }

        // --- resolve: присутствие/размер по правилам CPU-исполнителя ---
        let mut complete = true;
        let mut jobs: Vec<Job> = Vec::new();
        for layer in &plan.layers {
            match layer {
                PlanLayer::One { src, alpha_k, scale_alpha, fade } => {
                    match self.resolve_chain(src, ow, oh, plan.t)? {
                        None => complete = false,
                        Some((chain, fits)) => {
                            if fits {
                                jobs.push(Job::Single {
                                    chain,
                                    alpha_k: *alpha_k,
                                    scale: *scale_alpha,
                                    fade: *fade,
                                });
                            } // чужой размер — молча пропускается
                        }
                    }
                }
                PlanLayer::Dissolve { a, b, k, alpha_k, mix } => {
                    let ca = self.resolve_chain(a, ow, oh, plan.t)?;
                    let cb = self.resolve_chain(b, ow, oh, plan.t)?;
                    match (ca, cb) {
                        (Some((a, true)), Some((b, true))) => {
                            jobs.push(Job::Dissolve { a, b, k: *k, alpha_k: *alpha_k, mix: *mix });
                        }
                        (Some((c, true)), None) | (None, Some((c, true))) => {
                            jobs.push(Job::Single {
                                chain: c,
                                alpha_k: *alpha_k,
                                scale: *alpha_k < 4096,
                                fade: None,
                            });
                            complete = false;
                        }
                        _ => complete = false,
                    }
                }
            }
        }

        let n = (ow as usize) * (oh as usize) * 4;
        if jobs.is_empty() {
            out.clear();
            out.resize(n, 0);
            return Ok(complete);
        }

        // --- uniform-арена под все пассы (в слот-юнитах: гео-пасс = 4) ---
        let total: u32 = jobs.iter().map(|j| j.slot_units()).sum();
        if total > self.arena_slots {
            let slots = total.next_power_of_two();
            self.arena = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("kt-gpu uniforms"),
                size: SLOT * slots as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.arena_slots = slots;
        }
        self.slot = 0;

        // --- encode ---
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("kt-gpu") });
        let acc_a = self.pool.acquire(&self.device, ow, oh);
        let acc_b = self.pool.acquire(&self.device, ow, oh);
        let acc = [acc_a, acc_b];
        let mut cur = 0usize; // куда писал ПОСЛЕДНИЙ композит-пасс
        for (ji, job) in jobs.iter().enumerate() {
            let first = (ji == 0) as u32;
            let target = 1 - cur;
            match job {
                Job::Single { chain, alpha_k, scale, fade } => {
                    let (view, owned) = self.encode_chain(&mut enc, chain);
                    let mode = if fade.is_some() {
                        3
                    } else if *scale {
                        1
                    } else {
                        0
                    };
                    // сольный фейд с видом: кадр стоит на своей стороне
                    // пары, вторая помечена пустой (clear-бит) — ядро её
                    // не читает вовсе (зеркало Img::Clear)
                    let has = chain.has_alpha as u32;
                    let (u0z, u0w, clear, swap) = match fade {
                        Some(f) if f.incoming => (0, has, 1u32, true),
                        Some(_) => (has, 0, 2u32, false),
                        None => (has, 0, 0, false),
                    };
                    let u = UniformData {
                        u: [
                            mode,
                            *alpha_k,
                            u0z,
                            u0w,
                            first,
                            fade.map(|f| f.k).unwrap_or(0),
                            fade.map(|f| f.mix.mode).unwrap_or(0),
                            clear,
                        ],
                        f: fade.map(|f| f.mix.p).unwrap_or_default(),
                        ..Default::default()
                    };
                    let acc_in = acc[cur].view.clone();
                    let dummy = self.dummy.view.clone();
                    let (t1, t2) = if swap { (&dummy, &view) } else { (&view, &dummy) };
                    self.pass(&mut enc, Pipe::Composite, &acc[target].view, &acc_in, t1, t2, u);
                    for t in owned {
                        self.pool.release(t);
                    }
                }
                Job::Dissolve { a, b, k, alpha_k, mix } => {
                    let (va, oa) = self.encode_chain(&mut enc, a);
                    let (vb, ob) = self.encode_chain(&mut enc, b);
                    let u = UniformData {
                        u: [
                            2,
                            *alpha_k,
                            a.has_alpha as u32,
                            b.has_alpha as u32,
                            first,
                            *k,
                            mix.mode,
                            0,
                        ],
                        f: mix.p,
                        ..Default::default()
                    };
                    let acc_in = acc[cur].view.clone();
                    self.pass(&mut enc, Pipe::Composite, &acc[target].view, &acc_in, &va, &vb, u);
                    for t in oa.into_iter().chain(ob) {
                        self.pool.release(t);
                    }
                }
            }
            cur = target;
        }

        // --- readback ---
        let padded_bpr = (ow * 4).next_multiple_of(256);
        let need = (padded_bpr as u64) * (oh as u64);
        let recreate = match &self.staging {
            Some((_, bpr, w, h)) => *bpr != padded_bpr || *w != ow || *h != oh,
            None => true,
        };
        if recreate {
            self.staging = Some((
                self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("kt-gpu staging"),
                    size: need,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                padded_bpr,
                ow,
                oh,
            ));
        }
        let (staging, _, _, _) = self.staging.as_ref().unwrap();
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &acc[cur].texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bpr),
                    rows_per_image: Some(oh),
                },
            },
            wgpu::Extent3d { width: ow, height: oh, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|_| GpuExecError::DeviceLost)?;
        match rx.recv() {
            Ok(Ok(())) => {}
            _ => return Err(GpuExecError::DeviceLost),
        }
        out.clear();
        out.reserve(n);
        {
            let data = slice.get_mapped_range().map_err(|_| GpuExecError::DeviceLost)?;
            for row in 0..oh as usize {
                let s = row * padded_bpr as usize;
                out.extend_from_slice(&data[s..s + ow as usize * 4]);
            }
        }
        staging.unmap();

        let [a0, a1] = acc;
        self.pool.release(a0);
        self.pool.release(a1);
        self.readback.insert(key, out.clone(), complete);
        Ok(complete)
    }

    /// None — голода нет кадра; Some((chain, fits)) — цепочка готова,
    /// fits = финальный размер совпал с Quality.
    fn resolve_chain(
        &mut self,
        src: &LayerSrc<'_>,
        ow: u32,
        oh: u32,
        t: Tick,
    ) -> Result<Option<(ChainSpec, bool)>, GpuExecError> {
        let Some(frame) = &src.frame else { return Ok(None) };
        if frame.width > self.max_dim || frame.height > self.max_dim {
            return Err(GpuExecError::TooLarge { w: frame.width, h: frame.height });
        }
        let (ops, (fw, fh), ha, ha_known) =
            chain_ops(&src.fx, frame.width, frame.height, ow, oh, src.u, t, src.cam3d);
        let has_alpha = if ha_known { ha } else { frame.has_alpha };
        let view =
            self.upload
                .get_or_upload(&self.device, &self.queue, src.source, src.cache_idx, frame);
        let fits = (fw, fh) == (ow, oh);
        Ok(Some((
            ChainSpec { ops, src_view: view, src_w: frame.width, src_h: frame.height, has_alpha },
            fits,
        )))
    }

    /// Кодирует цепочку слоя; возвращает view результата + транзиенты,
    /// которые нельзя вернуть в пул до композит-пасса.
    fn encode_chain(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        chain: &ChainSpec,
    ) -> (wgpu::TextureView, Vec<Tex>) {
        let mut view = chain.src_view.clone();
        let (mut w, mut h) = (chain.src_w, chain.src_h);
        let mut owned: Option<Tex> = None;
        let mut retired: Vec<Tex> = Vec::new();
        let dummy = self.dummy.view.clone();
        // сухая сторона смешения (Р.38 п.12): снимок состояния ДО вклада
        // слоя. Текстуру снимка нельзя вернуть в пул до самого смешения.
        let mut snap: Option<(wgpu::TextureView, u32, u32)> = None;
        for op in &chain.ops {
            match op {
                FxOp::Snapshot => {
                    // текущий результат ещё живёт в `owned` (или это сам
                    // источник) — придерживаем его до конца цепочки
                    if let Some(t) = owned.take() {
                        snap = Some((t.view.clone(), w, h));
                        retired.push(t);
                    } else {
                        snap = Some((view.clone(), w, h));
                    }
                }
                FxOp::Mix { k } => {
                    let Some((dry_view, dw, dh)) = snap.clone() else { continue };
                    // размеры могли разойтись — вписываем сухую сторону
                    // ТОЙ ЖЕ билинейкой, что и на CPU (fit_into)
                    let dry_fit = if (dw, dh) == (w, h) {
                        dry_view
                    } else {
                        let fit = self.pool.acquire(&self.device, w, h);
                        let u = UniformData {
                            u: [w, h, dw, dh, 0, 0, 0, 0],
                            f: [0.0, 0.0, dw as f32, dh as f32, 0.0, 0.0, 0.0, 0.0],
                            ..Default::default()
                        };
                        self.pass(enc, Pipe::Resample, &fit.view, &dry_view, &dummy, &dummy, u);
                        let v = fit.view.clone();
                        retired.push(fit);
                        v
                    };
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [*k, chain.has_alpha as u32, chain.has_alpha as u32, 0, 0, 0, 0, 0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::LayerMix, &dst.view, &dry_fit, &view, &dummy, u);
                    if let Some(t) = owned.take() {
                        retired.push(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Resample { tw, th, frame } => {
                    let dst = self.pool.acquire(&self.device, *tw, *th);
                    let u = UniformData {
                        u: [*tw, *th, w, h, 0, 0, 0, 0],
                        f: [
                            frame.0 as f32,
                            frame.1 as f32,
                            frame.2 as f32,
                            frame.3 as f32,
                            0.0,
                            0.0,
                            0.0,
                            0.0,
                        ],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Resample, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    w = *tw;
                    h = *th;
                    owned = Some(dst);
                }
                FxOp::Mirror { h: fh, v: fv } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, h, *fh as u32, *fv as u32, 0, 0, 0, 0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Mirror, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Noise { seed, k, size, color } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [*seed, *k, *size, *color as u32, 0, 0, 0, 0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Noise, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Greenscreen { key, tol, hard, spill, kc } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [*kc, 0, 0, 0, 0, 0, 0, 0],
                        f: [key[0], key[1], key[2], *tol, *hard, *spill, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Greenscreen, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Edges { th, d, color } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, h, *th, *d, color[0], color[1], color[2], 0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Edges, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Vhs { shift, line_off, bleed, jq, step, seed } => {
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, *shift, *line_off, *step, *seed, *jq as u32, 0, 0],
                        f: [*bleed, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Vhs, &dst.view, &view, &dummy, &dummy, u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::Glow { dw, dh, s, th, tint } => {
                    let small = self.pool.acquire(&self.device, *dw, *dh);
                    let u = UniformData {
                        u: [*dw, *dh, w, h, 0, 0, 0, 0],
                        f: [0.0, 0.0, w as f32, h as f32, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Resample, &small.view, &view, &dummy, &dummy, u);
                    let up = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [w, h, *dw, *dh, 0, 0, 0, 0],
                        f: [0.0, 0.0, *dw as f32, *dh as f32, 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::Resample, &up.view, &small.view, &dummy, &dummy, u);
                    self.pool.release(small);
                    let dst = self.pool.acquire(&self.device, w, h);
                    let u = UniformData {
                        u: [*th, 0, 0, 0, 0, 0, 0, 0],
                        f: [*s, tint[0], tint[1], tint[2], 0.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    self.pass(enc, Pipe::GlowAdd, &dst.view, &view, &up.view, &dummy, u);
                    self.pool.release(up);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    owned = Some(dst);
                }
                FxOp::GeomFused { prog } => {
                    debug_assert_eq!((prog.in_w, prog.in_h), (w, h), "geom run input dims");
                    let dst = self.pool.acquire(&self.device, prog.out_w, prog.out_h);
                    let u = encode_geom(prog);
                    self.pass_geom(enc, &dst.view, &view, &u);
                    if let Some(t) = owned.take() {
                        self.pool.release(t);
                    }
                    view = dst.view.clone();
                    w = prog.out_w;
                    h = prog.out_h;
                    owned = Some(dst);
                }
            }
        }
        if let Some(t) = owned {
            retired.push(t);
        }
        (view, retired)
    }

    /// Пасс слитой гео-серии: юниформа GEOM_UNIFORM_BYTES (4 слота арены),
    /// свой layout — остальная механика как у `pass`.
    fn pass_geom(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        t0: &wgpu::TextureView,
        u: &GeomUniform,
    ) {
        let offset = self.slot as u64 * SLOT;
        debug_assert!(
            self.slot + GEOM_SLOT_UNITS <= self.arena_slots,
            "uniform arena overflow (geom)"
        );
        self.slot += GEOM_SLOT_UNITS;
        self.queue.write_buffer(&self.arena, offset, &u.bytes());
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.geom_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.arena,
                        offset: 0,
                        size: wgpu::BufferSize::new(GEOM_UNIFORM_BYTES as u64),
                    }),
                },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(t0) },
            ],
        });
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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
        rp.set_pipeline(&self.geom_pipe);
        rp.set_bind_group(0, &bg, &[offset as u32]);
        rp.draw(0..3, 0..1);
    }

    #[allow(clippy::too_many_arguments)]
    fn pass(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        pipe: Pipe,
        target: &wgpu::TextureView,
        t0: &wgpu::TextureView,
        t1: &wgpu::TextureView,
        t2: &wgpu::TextureView,
        u: UniformData,
    ) {
        let offset = self.slot as u64 * SLOT;
        debug_assert!(self.slot < self.arena_slots, "uniform arena overflow");
        self.slot += 1;
        self.queue.write_buffer(&self.arena, offset, &u.bytes());
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.arena,
                        offset: 0,
                        size: wgpu::BufferSize::new(128),
                    }),
                },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(t0) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(t1) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(t2) },
            ],
        });
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
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
        rp.set_pipeline(&self.pipes[pipe as usize]);
        rp.set_bind_group(0, &bg, &[offset as u32]);
        rp.draw(0..3, 0..1);
    }
}

/// Ключ ReadbackCache: полное описание плана. Кадры по (source, cache_idx,
/// dims) детерминированы — контент в ключ не входит.
fn plan_key(plan: &FramePlan<'_>) -> u64 {
    fn put_src(h: &mut Xxh3, s: &LayerSrc<'_>) {
        h.update(&s.source.0.to_le_bytes());
        h.update(&s.cache_idx.to_le_bytes());
        // смешивание кадров (Р.37 С5) — В КЛЮЧ: те же два кадра с другим
        // весом дают ДРУГУЮ картинку, и без этого ReadbackCache отдал бы
        // чужой смес (та же причина, что у put_mix)
        match &s.blend {
            None => h.update(&[0u8]),
            Some(b) => {
                h.update(&[1u8]);
                h.update(&b.cache_idx.to_le_bytes());
                h.update(&b.k.to_le_bytes());
            }
        }
        h.update(&s.fx_hash.to_le_bytes());
        h.update(&s.u.to_bits().to_le_bytes());
        put_cam3d(h, &s.cam3d);
        match &s.frame {
            None => h.update(&[0u8]),
            Some(f) => {
                h.update(&[1u8, f.has_alpha as u8]);
                h.update(&f.width.to_le_bytes());
                h.update(&f.height.to_le_bytes());
            }
        }
    }
    /// Параметры перехода — В КЛЮЧ: без них ReadbackCache отдал бы кадр
    /// другого вида перехода (Р.33 С4).
    fn put_mix(h: &mut Xxh3, m: &TransMix) {
        h.update(&m.mode.to_le_bytes());
        for v in m.p {
            h.update(&v.to_bits().to_le_bytes());
        }
    }
    let mut h = Xxh3::new();
    h.update(&plan.t.0.to_le_bytes());
    h.update(&plan.out_w.to_le_bytes());
    h.update(&plan.out_h.to_le_bytes());
    for layer in &plan.layers {
        match layer {
            PlanLayer::One { src, alpha_k, scale_alpha, fade } => {
                h.update(&[1u8, *scale_alpha as u8]);
                h.update(&alpha_k.to_le_bytes());
                match fade {
                    None => h.update(&[0u8]),
                    Some(f) => {
                        h.update(&[1u8, f.incoming as u8]);
                        h.update(&f.k.to_le_bytes());
                        put_mix(&mut h, &f.mix);
                    }
                }
                put_src(&mut h, src);
            }
            PlanLayer::Dissolve { a, b, k, alpha_k, mix } => {
                h.update(&[2u8]);
                h.update(&k.to_le_bytes());
                h.update(&alpha_k.to_le_bytes());
                put_mix(&mut h, mix);
                put_src(&mut h, a);
                put_src(&mut h, b);
            }
        }
    }
    h.digest()
}
