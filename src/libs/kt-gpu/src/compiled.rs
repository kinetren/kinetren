//! Compiled pipelines belong to the device (pipeline-prewarm D3): every engine on one device shares
//! one `Compiled` — the layouts, every pipeline, which of them are warm, the geometry shapes and the
//! device's pipeline cache. Its warm worker (one thread, started when work arrives, gone when the
//! queue is empty) draws what a project reaches (D5) and compiles the geometry shapes frames met
//! (D4), so a frame never compiles.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use crate::context::GpuInitError;
use crate::engine::{
    COMMON, DISPLAY, GEOM_UNIFORM_BYTES, PIPE_COUNT, SHADERS, assembled_composite, assembled_shaders, geom_specialized,
    plugin_barriers, plugin_taps, texture_entry,
};
use crate::pool::{DISPLAY_FORMAT, FORMAT, WORD_FORMAT};
use crate::warm::{Kit, Layout};

/// A frame drew or dispatched a pipeline that was never warmed: its code compiled inside the
/// frame on a driver that compiles at first use (t2120).
static COLD_DRAWS: AtomicU64 = AtomicU64::new(0);
/// A pipeline created on a frame's path (a compute job compiling a text no prewarm prepared).
static IN_FRAME: AtomicU64 = AtomicU64::new(0);

/// Draws and dispatches of a never-warmed pipeline inside frames, since the process started.
pub fn cold_draws() -> u64 {
    COLD_DRAWS.load(Ordering::Relaxed)
}

/// Pipelines created on a frame's path since the process started.
pub fn pipelines_in_frame() -> u64 {
    IN_FRAME.load(Ordering::Relaxed)
}

pub(crate) fn cold_draw() {
    COLD_DRAWS.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn in_frame() {
    IN_FRAME.fetch_add(1, Ordering::Relaxed);
}

/// A geometry shape's own pipeline (D4).
enum Shape {
    /// On the worker's queue: frames run the interpreter.
    Pending,
    /// Compiled and warm; `None` — the shape has no text of its own, or its compile failed.
    Ready(Option<wgpu::RenderPipeline>),
}

enum Job {
    Slots(Vec<usize>),
    Shape([u32; 4]),
}

#[derive(Default)]
struct Work {
    queue: VecDeque<Job>,
    running: bool,
}

#[derive(Default)]
struct Asked {
    core: bool,
    ids: HashSet<String>,
}

pub(crate) struct Compiled {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub layout: wgpu::BindGroupLayout,
    pub pipes: [wgpu::RenderPipeline; PIPE_COUNT],
    /// The own passes of the point-wise plugins, then the barrier plugins' passes (`plugin_barriers`).
    pub plugin_pipes: Vec<wgpu::RenderPipeline>,
    /// The plugin pipes that target `WORD_FORMAT`, by the same index.
    pub plugin_word: Vec<bool>,
    /// The id of the plugin each plugin pipe belongs to.
    plugin_owner: Vec<&'static str>,
    pub geom_layout: wgpu::BindGroupLayout,
    geom_pipe_layout: wgpu::PipelineLayout,
    pub geom_pipe: wgpu::RenderPipeline,
    pub display_pipe: wgpu::RenderPipeline,
    cache: Option<crate::pipeline_cache::DiskCache>,
    kit: Kit,
    /// Warm flags by slot: the fixed pipes, the plugin pipes, `display`, `geom_fused`.
    warm: Vec<AtomicBool>,
    shapes: Mutex<HashMap<[u32; 4], Shape>>,
    work: Mutex<Work>,
    idle: Condvar,
    asked: Mutex<Asked>,
    /// Frames composing on this device now: the worker submits nothing while one is (D2b).
    frames: Mutex<usize>,
    frame_done: Condvar,
    /// The adapter, and whether the worker warms a shadow device first (D2b).
    info: wgpu::AdapterInfo,
    shadow: bool,
}

/// A frame composing on the device (`Compiled::frame`): the warm worker waits for it to end.
pub(crate) struct FrameGuard<'a>(&'a Compiled);

impl Drop for FrameGuard<'_> {
    fn drop(&mut self) {
        let mut n = self.0.frames.lock().unwrap_or_else(|e| e.into_inner());
        *n -= 1;
        if *n == 0 {
            self.0.frame_done.notify_all();
        }
    }
}

/// A pipeline's slot in the warm flags.
#[derive(Clone, Copy)]
pub(crate) enum Slot {
    Fixed(usize),
    Plugin(usize),
    Display,
    Geom,
}

/// The kept compiled pipelines (the editor's display device, D3). Only `new_kept` asks it: a
/// `wgpu::Device` compares by its id inside its instance, so a device of another instance (a test
/// opening its own) can equal one of ours, and an ordinary engine — an export's, the offline one, a
/// test's — compiles its own and lets them go with it.
static KEPT: Mutex<Vec<Arc<Compiled>>> = Mutex::new(Vec::new());

/// Shaders assembled for this registry: (name, WGSL, word target) of the fixed pipes then of the
/// plugin pipes, the fused pass's text, and the owners of the plugin pipes.
struct Sources {
    fixed: Vec<(String, String)>,
    plugins: Vec<(String, String, bool, &'static str)>,
    fused: String,
}

fn sources() -> Sources {
    let (fused, plugin_srcs) = assembled_shaders();
    let composite = assembled_composite();
    let fixed = SHADERS
        .iter()
        .map(|(n, b)| (n.to_string(), if *n == "composite" { composite.clone() } else { b.to_string() }))
        .collect();
    let mut plugins: Vec<(String, String, bool, &'static str)> = plugin_taps()
        .iter()
        .zip(&plugin_srcs)
        .map(|((id, _), body)| (format!("plugin {id}"), body.clone(), false, *id))
        .collect();
    // the barrier plugins' passes follow, in `plugin_barriers` order (`barrier_pipe_base`)
    for (id, shaders, words) in plugin_barriers() {
        for (i, body) in shaders.iter().enumerate() {
            plugins.push((format!("barrier {id} {i}"), body.to_string(), words >> i & 1 == 1, id));
        }
    }
    Sources { fixed, plugins, fused }
}

impl Sources {
    /// What the disk tier's key takes from the shaders (D7): every text the engine compiles.
    fn hash(&self) -> u64 {
        let mut h = xxhash_rust::xxh3::Xxh3::new();
        h.update(COMMON.as_bytes());
        h.update(DISPLAY.as_bytes());
        h.update(self.fused.as_bytes());
        for (n, b) in &self.fixed {
            h.update(n.as_bytes());
            h.update(b.as_bytes());
        }
        for (n, b, w, _) in &self.plugins {
            h.update(n.as_bytes());
            h.update(b.as_bytes());
            h.update(&[*w as u8]);
        }
        h.digest()
    }
}

fn render_pipe(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    cache: Option<&wgpu::PipelineCache>,
    name: &str,
    text: String,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(name),
        source: wgpu::ShaderSource::Wgsl(text.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(name),
        layout: Some(layout),
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
            targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache,
    })
}

/// The compiled pipelines of `device`: with `keep`, the kept ones of that device if an engine made
/// them before (a project opened again) and kept from now on; else new ones. `shadow` — a software
/// device whose frames a person watches: its worker warms a shadow device first (D2b).
pub(crate) fn for_device(
    device: &Arc<wgpu::Device>,
    queue: &Arc<wgpu::Queue>,
    info: &wgpu::AdapterInfo,
    keep: bool,
    shadow: bool,
) -> Result<Arc<Compiled>, GpuInitError> {
    if !keep {
        return Ok(Arc::new(Compiled::compile(device, queue, info, shadow)?));
    }
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = kept.iter().find(|c| *c.device == **device) {
        return Ok(c.clone());
    }
    let c = Arc::new(Compiled::compile(device, queue, info, shadow)?);
    kept.push(c.clone());
    Ok(c)
}

impl Compiled {
    fn compile(device: &Arc<wgpu::Device>, queue: &Arc<wgpu::Queue>, info: &wgpu::AdapterInfo, shadow: bool) -> Result<Compiled, GpuInitError> {
        // an engine built on a thread (the editor's, the offline one, a shadow) compiles on the device:
        // the exit waits for it (D10)
        let _busy = crate::exit::busy().ok_or_else(|| GpuInitError::Unsupported("the process is exiting".into()))?;
        let src = sources();
        let cache = crate::pipeline_cache::DiskCache::open(device, info, "engine", src.hash());
        let pc = cache.as_ref().map(|c| c.get());
        // Two scopes: a shader the backend's compiler rejects (FXC on D3D12: "not natively
        // addressable", windows-build) is an INTERNAL error, not a validation one. With only
        // the validation scope it escaped to the default handler, which panics — a crash on
        // Windows instead of a clean error. Popped in reverse order.
        let internal_scope = device.push_error_scope(wgpu::ErrorFilter::Internal);
        let err_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kt-gpu layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    // The VERTEX stage also reads the uniform (vs3d Pan3d:
                    // MVP/anchor): FRAGMENT-only made create_render_pipeline fail
                    // on real hardware and refused the WHOLE engine (R.31 item 1)
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
        let pipes: Vec<wgpu::RenderPipeline> = src
            .fixed
            .iter()
            .map(|(name, body)| render_pipe(device, &pipe_layout, pc, name, format!("{COMMON}\n{body}"), FORMAT))
            .collect();
        let plugin_pipes: Vec<wgpu::RenderPipeline> = src
            .plugins
            .iter()
            .map(|(name, body, word, _)| {
                render_pipe(device, &pipe_layout, pc, name, format!("{COMMON}\n{body}"), if *word { WORD_FORMAT } else { FORMAT })
            })
            .collect();
        let display_pipe = render_pipe(device, &pipe_layout, pc, "display", format!("{COMMON}\n{DISPLAY}"), DISPLAY_FORMAT);
        // The merged geometry pass (R.44): the GEOM_UNIFORM_BYTES uniform does not fit into
        // the shared layout (128 B) — its own layout and pipe from the same arena buffer.
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
        let geom_pipe = render_pipe(device, &geom_pipe_layout, pc, "geom_fused", src.fused.clone(), FORMAT);
        let validation = pollster::block_on(err_scope.pop());
        let internal = pollster::block_on(internal_scope.pop());
        if let Some(e) = validation.or(internal) {
            return Err(GpuInitError::Pipeline(e.to_string()));
        }
        let pipes: [wgpu::RenderPipeline; PIPE_COUNT] =
            pipes.try_into().map_err(|_| GpuInitError::Pipeline("pipe count".into()))?;
        let kit = Kit::new(device, &layout, &geom_layout);
        let slots = PIPE_COUNT + plugin_pipes.len() + 2;
        Ok(Compiled {
            device: device.clone(),
            queue: queue.clone(),
            layout,
            pipes,
            plugin_word: src.plugins.iter().map(|p| p.2).collect(),
            plugin_owner: src.plugins.iter().map(|p| p.3).collect(),
            plugin_pipes,
            geom_layout,
            geom_pipe_layout,
            geom_pipe,
            display_pipe,
            cache,
            kit,
            warm: (0..slots).map(|_| AtomicBool::new(false)).collect(),
            shapes: Mutex::new(HashMap::new()),
            work: Mutex::default(),
            idle: Condvar::new(),
            asked: Mutex::default(),
            frames: Mutex::new(0),
            frame_done: Condvar::new(),
            info: info.clone(),
            shadow,
        })
    }

    /// A frame starts composing on the device; it ends when the guard drops.
    pub(crate) fn frame(&self) -> FrameGuard<'_> {
        *self.frames.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        FrameGuard(self)
    }

    /// Wait until no frame composes on the device: a software rasterizer runs every submission on
    /// one queue thread, so a warm draw submitted first is a draw the frame waits behind.
    /// Returns early when the process is exiting: the exit waits for this thread (D10), and a frame
    /// may never end then.
    fn wait_frames(&self) {
        let mut n = self.frames.lock().unwrap_or_else(|e| e.into_inner());
        while *n > 0 && !crate::exit::exiting() {
            n = self.frame_done.wait_timeout(n, std::time::Duration::from_millis(50)).unwrap_or_else(|e| e.into_inner()).0;
        }
    }

    /// The shadow device (D2b): the same adapter opened once more, its own queue thread. What it
    /// compiles lands in the driver's disk cache, so this device's warm draw of the same code is a
    /// load. `None` — no such device (another adapter answers, the device does not open).
    fn open_shadow(&self) -> Option<Compiled> {
        let ctx = crate::context::GpuContext::new_headless().ok()?;
        let i = &ctx.adapter_info;
        if (i.name.as_str(), i.backend, i.driver_info.as_str()) != (self.info.name.as_str(), self.info.backend, self.info.driver_info.as_str()) {
            return None;
        }
        Compiled::compile(&ctx.device, &ctx.queue, i, false).ok()
    }

    fn index(&self, s: Slot) -> usize {
        match s {
            Slot::Fixed(i) => i,
            Slot::Plugin(i) => PIPE_COUNT + i,
            Slot::Display => PIPE_COUNT + self.plugin_pipes.len(),
            Slot::Geom => PIPE_COUNT + self.plugin_pipes.len() + 1,
        }
    }

    fn slot(&self, i: usize) -> Slot {
        let n = self.plugin_pipes.len();
        match i {
            i if i < PIPE_COUNT => Slot::Fixed(i),
            i if i < PIPE_COUNT + n => Slot::Plugin(i - PIPE_COUNT),
            i if i == PIPE_COUNT + n => Slot::Display,
            _ => Slot::Geom,
        }
    }

    /// A frame is about to draw the pipeline of `s`: a never-warmed one is a cold draw.
    pub(crate) fn drawn(&self, s: Slot) {
        let i = self.index(s);
        if !self.warm[i].swap(true, Ordering::AcqRel) {
            cold_draw();
            if kt_time::trace::on() {
                let n = kt_time::trace::name(&self.label(i));
                kt_time::trace::ev(kt_time::trace::Ev::ColdDraw).aux(n).emit();
            }
        }
    }

    /// What the trace calls a pipeline (structured-telemetry).
    fn label(&self, i: usize) -> String {
        const FIXED: [&str; PIPE_COUNT] =
            ["resample", "mirror", "composite", "layer_mix", "unpremul", "premul", "grid_tile", "mask"];
        match self.slot(i) {
            Slot::Fixed(f) => FIXED[f].to_string(),
            Slot::Plugin(p) => format!("{}:pass{p}", self.plugin_owner[p]),
            Slot::Display => "display".into(),
            Slot::Geom => "geometry".into(),
        }
    }


    /// The warm pipeline of a geometry shape, or `None` while the interpreter runs it: a shape seen
    /// for the first time goes to the worker (D4).
    pub(crate) fn shape(self: &Arc<Self>, h: [u32; 4]) -> Option<wgpu::RenderPipeline> {
        let mut shapes = self.shapes.lock().unwrap_or_else(|e| e.into_inner());
        match shapes.get(&h) {
            Some(Shape::Ready(p)) => p.clone(),
            Some(Shape::Pending) => None,
            None => {
                shapes.insert(h, Shape::Pending);
                drop(shapes);
                self.enqueue(Job::Shape(h));
                None
            }
        }
    }

    /// How many shapes have a warm pipeline of their own.
    pub(crate) fn shapes_ready(&self) -> usize {
        self.shapes.lock().unwrap_or_else(|e| e.into_inner()).values().filter(|s| matches!(s, Shape::Ready(Some(_)))).count()
    }

    /// Warm what the effects `ids` reach, and the core the first time (D5). Returns at once.
    pub(crate) fn prewarm(self: &Arc<Self>, ids: &[String]) {
        let mut asked = self.asked.lock().unwrap_or_else(|e| e.into_inner());
        let mut slots = Vec::new();
        if !asked.core {
            asked.core = true;
            slots.extend((0..PIPE_COUNT).map(Slot::Fixed));
            slots.push(Slot::Display);
            slots.push(Slot::Geom);
        }
        for id in ids {
            if asked.ids.insert(id.clone()) {
                slots.extend(self.plugin_owner.iter().enumerate().filter(|(_, o)| **o == id.as_str()).map(|(i, _)| Slot::Plugin(i)));
            }
        }
        drop(asked);
        let slots: Vec<usize> = slots.into_iter().map(|s| self.index(s)).filter(|&i| !self.warm[i].load(Ordering::Acquire)).collect();
        if !slots.is_empty() {
            self.enqueue(Job::Slots(slots));
        }
    }

    /// Wait until the worker has nothing left.
    pub(crate) fn settle(&self) {
        let mut w = self.work.lock().unwrap_or_else(|e| e.into_inner());
        while w.running || !w.queue.is_empty() {
            w = self.idle.wait(w).unwrap_or_else(|e| e.into_inner());
        }
    }

    fn enqueue(self: &Arc<Self>, job: Job) {
        let mut w = self.work.lock().unwrap_or_else(|e| e.into_inner());
        w.queue.push_back(job);
        if w.running {
            return;
        }
        w.running = true;
        drop(w);
        let me = self.clone();
        if let Err(e) = std::thread::Builder::new().name("pipeline-warm".into()).spawn(move || me.run()) {
            // no thread: nothing is warmed and frames compile as before
            eprintln!("kt-gpu: no warm thread: {e}");
            let mut w = self.work.lock().unwrap_or_else(|e| e.into_inner());
            w.queue.clear();
            w.running = false;
            self.idle.notify_all();
        }
    }

    fn run(self: Arc<Self>) {
        // the exit waits for this guard (D10); the shadow, declared after it, goes first
        let busy = crate::exit::busy();
        // opened at the first job of a run that wants it, dropped with the run
        let mut shadow: Option<Option<Compiled>> = None;
        let shadow_of = |me: &Compiled| -> Option<Compiled> { if me.shadow { me.open_shadow() } else { None } };
        loop {
            let job = if busy.is_none() || crate::exit::exiting() {
                // the process is exiting: drop the queue, warm nothing more
                self.work.lock().unwrap_or_else(|e| e.into_inner()).queue.clear();
                None
            } else {
                self.work.lock().unwrap_or_else(|e| e.into_inner()).queue.pop_front()
            };
            match job {
                Some(Job::Slots(slots)) => {
                    if let Some(sh) = shadow.get_or_insert_with(|| shadow_of(&self)) {
                        // one pipeline a submission there too: the exit waits for one draw, not all (D10)
                        sh.warm_slots(&slots);
                    }
                    self.warm_slots(&slots)
                }
                Some(Job::Shape(h)) => {
                    if let Some(sh) = shadow.get_or_insert_with(|| shadow_of(&self)) {
                        sh.compile_shape(h);
                    }
                    self.compile_shape(h)
                }
                None => {
                    // the queue is empty: keep what was compiled, then end unless work came meanwhile
                    if let (Some(c), false) = (&self.cache, crate::exit::exiting()) {
                        c.save();
                    }
                    let mut w = self.work.lock().unwrap_or_else(|e| e.into_inner());
                    if w.queue.is_empty() {
                        w.running = false;
                        self.idle.notify_all();
                        return;
                    }
                }
            }
        }
    }

    /// Warm `slots` one pipeline per submission, each after the frames composing now: a frame
    /// waits behind one warm draw at most (D2b).
    fn warm_slots(&self, slots: &[usize]) {
        for &i in slots {
            if self.warm[i].load(Ordering::Acquire) {
                continue;
            }
            self.wait_frames();
            if crate::exit::exiting() {
                return;
            }
            let mut span = kt_time::trace::span(kt_time::trace::Ev::Warm);
            if kt_time::trace::on() {
                span.aux(kt_time::trace::name(&self.label(i)));
            }
            self.warm_batch(&[i]);
        }
    }

    /// Warm `slots` in one submission.
    fn warm_batch(&self, slots: &[usize]) {
        let list: Vec<(wgpu::RenderPipeline, Layout, wgpu::TextureFormat)> = slots
            .iter()
            .filter(|&&i| !self.warm[i].load(Ordering::Acquire))
            .map(|&i| match self.slot(i) {
                Slot::Fixed(f) => (self.pipes[f].clone(), Layout::Shared, FORMAT),
                Slot::Plugin(p) => {
                    (self.plugin_pipes[p].clone(), Layout::Shared, if self.plugin_word[p] { WORD_FORMAT } else { FORMAT })
                }
                Slot::Display => (self.display_pipe.clone(), Layout::Shared, DISPLAY_FORMAT),
                Slot::Geom => (self.geom_pipe.clone(), Layout::Geom, FORMAT),
            })
            .collect();
        self.kit.draw(&self.device, &self.queue, &list);
        for &i in slots {
            self.warm[i].store(true, Ordering::Release);
        }
    }

    fn compile_shape(&self, h: [u32; 4]) {
        let pipe = geom_specialized(h).and_then(|text| {
            let internal = self.device.push_error_scope(wgpu::ErrorFilter::Internal);
            let validation = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let p = render_pipe(&self.device, &self.geom_pipe_layout, self.cache.as_ref().map(|c| c.get()), "geom_shape", text, FORMAT);
            let failed = pollster::block_on(validation.pop()).is_some() | pollster::block_on(internal.pop()).is_some();
            (!failed).then_some(p)
        });
        if let Some(p) = pipe.as_ref().filter(|_| !crate::exit::exiting()) {
            let mut span = kt_time::trace::span(kt_time::trace::Ev::Warm);
            span.aux(kt_time::trace::name("geometry:shape"));
            self.wait_frames();
            self.kit.draw(&self.device, &self.queue, &[(p.clone(), Layout::Geom, FORMAT)]);
        }
        self.shapes.lock().unwrap_or_else(|e| e.into_inner()).insert(h, Shape::Ready(pipe));
    }
}
