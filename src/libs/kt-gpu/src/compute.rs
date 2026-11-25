//! The compute host (compute-kernels): `kt_plugin_api::compute` on a device of its own. The
//! interface's device may be GL, which has no compute, so this one asks for Vulkan, DX12 or Metal
//! whatever `WGPU_BACKEND` says. WARP is refused, as the engine refuses it. Jobs are admitted by
//! their caller's class on the device's ring (gpu-arbiter): the frame on screen before the
//! look-ahead, and a job cut between dispatches while a higher class waits.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use kt_plugin_api::compute::{Buf, ComputeHost, Held, Init, Job, Shader};

/// One shader's pipeline; the layout follows from how many buffers a step binds.
struct Pipe {
    pipe: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    /// Dispatched once already (by `prepare` or a job): a software rasterizer has compiled its code.
    warm: std::sync::atomic::AtomicBool,
}

/// Timestamps of one job: a pair per dispatch; dispatches past the room run untimed. A batched
/// history walk (stream-history-on-device) carries hundreds of dispatches: an untimed one is never
/// predicted, and the arbiter cannot cut there.
const STAMPS: u32 = 2048;

/// Timestamp sets made up front (gpu-throughput D6, found-defects 486): a job in flight holds one, so two
/// concurrent jobs never resolve each other's; a job finding none free runs untimed.
const STAMP_SETS: usize = 4;

/// The resolve buffer's room: the stamps plus the alignment a piece's own resolve may add (D2).
const RESOLVE_BYTES: u64 = STAMPS as u64 * 8 + 256;

/// A timestamp set and its resolve buffer.
type Stamps = (wgpu::QuerySet, wgpu::Buffer);

fn make_stamps(device: &wgpu::Device) -> Vec<Stamps> {
    if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
        return Vec::new();
    }
    (0..STAMP_SETS)
        .map(|_| {
            let set = device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("kt-gpu compute time"),
                ty: wgpu::QueryType::Timestamp,
                count: STAMPS,
            });
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("kt-gpu compute time resolve"),
                size: RESOLVE_BYTES,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            (set, buf)
        })
        .collect()
}

/// A dispatch's work units: its workgroups (what a seed is scaled by).
fn units(groups: [u32; 3]) -> u64 {
    groups.iter().map(|&g| g.max(1) as u64).product()
}

/// The timestamps of a piece already submitted, coming back while the job goes on (D2): its
/// dispatches `from..to`, mapped when `ready`.
struct Learn {
    buf: wgpu::Buffer,
    from: usize,
    to: usize,
    ready: Arc<std::sync::atomic::AtomicU8>,
}

/// What the pool keeps at most: a job of a 4K frame fits many times over.
const POOL_BYTES: u64 = 1 << 30;

/// The buffers of finished jobs by size and usage: a kernel asks for the same set every frame.
#[derive(Default)]
struct Pool {
    free: HashMap<(u64, u32), Vec<wgpu::Buffer>>,
    bytes: u64,
}

/// How the device's timestamps map onto the trace's clock (found-defects 398): a timestamp written by
/// an otherwise empty submission lies between the CPU clock before the submission and after its
/// completion; the narrowest of the recent brackets is kept, its half-width is the error.
#[derive(Default)]
struct Calibration {
    /// Its own query pair and buffers, made at the first calibration.
    set: Option<(wgpu::QuerySet, wgpu::Buffer, wgpu::Buffer)>,
}

/// What a calibration's empty submission claims on the ring: one timestamp write.
const CALIBRATION_NS: u64 = 20_000;

/// What the warm-up's one workgroup claims on the ring.
const WARM_NS: u64 = 100_000;

/// A calibration older than this is made again (the device's clock drifts against the CPU's).
const CALIBRATION_AGE: std::time::Duration = std::time::Duration::from_secs(10);

pub struct ComputeDevice {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    /// The ring of its adapter (gpu-arbiter), registered as a compute host while it lives.
    arb: Arc<crate::arbiter::Ring>,
    _on_ring: crate::arbiter::Registered,
    /// The device time each dispatch took when last measured, by its name (where a job is cut).
    last: Mutex<HashMap<String, u64>>,
    adapter: String,
    info: wgpu::AdapterInfo,
    /// Timestamp sets free for a job (gpu-pass-cost; one per job in flight, gpu-throughput D6); none —
    /// the adapter has no timestamps.
    stamps: Mutex<Vec<Stamps>>,
    timed: bool,
    /// The device's key in the pass-time seeds (gpu-throughput D2).
    seed_key: String,
    /// The device's timestamps against the trace's clock (stream-history-on-device D7), made while the
    /// trace records.
    calib: Mutex<Calibration>,
    /// The narrowest bracket: (device ticks, trace ns at its middle, half-width ns, when it was made).
    /// Apart from `calib`, so a record reads it without waiting for a calibration in progress
    /// (found-defects 562).
    best: Mutex<Option<(u64, u64, u64, std::time::Instant)>>,
    /// By text: a slot of its own each, so a compile (the driver reads its disk cache in it) holds only
    /// the jobs that want that same text (gpu-waits-named).
    pipes: Mutex<HashMap<(String, usize, usize), Arc<Mutex<Option<Arc<Pipe>>>>>>,
    pool: Mutex<Pool>,
    /// The pool's free buffers, to the memory ledger (memory-ledger).
    account: kt_time::mem::Account,
    /// Where the buffers jobs kept go back when their last holder drops them (stream-round-trips D1).
    spares: Arc<crate::held::Spares>,
    /// The disk tier of its pipelines (pipeline-prewarm D7), where the device has one.
    cache: Option<crate::pipeline_cache::DiskCache>,
}

/// What the compute host's disk-tier key takes from the shaders: every text the installed plugins
/// declare (pipeline-prewarm D7).
fn declared_hash() -> u64 {
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    if let Some(reg) = kt_plugin_api::installed() {
        for p in reg.plugins() {
            for s in p.compute_shaders() {
                h.update(s.text.as_bytes());
                h.update(&[s.reads as u8, s.writes as u8]);
            }
        }
    }
    h.digest()
}

/// Open the device: what `kt_plugin_api::compute::install` takes.
pub fn open() -> Result<Arc<dyn ComputeHost>, String> {
    let d = open_device()?;
    let _ = OPENED.set(d.clone());
    Ok(d)
}

static OPENED: std::sync::OnceLock<Arc<ComputeDevice>> = std::sync::OnceLock::new();

/// The process's compute host when it is this crate's device (opened through
/// `kt_plugin_api::compute::host`, which `open` serves): the device the slots in front of a stream
/// stage run on, so their output reaches the codec where it lies (stream-front-on-device). `None` — no
/// host, another host installed, or it failed.
pub fn host_device() -> Option<Arc<ComputeDevice>> {
    kt_plugin_api::compute::host()?;
    OPENED.get().cloned()
}

/// The window's device, when the compute host runs on it (stream-picture-stays D2): set by
/// [`share_display`] before the host opens.
static SHARED: std::sync::OnceLock<crate::GpuContext> = std::sync::OnceLock::new();

/// How the compute host orders the adapters it may open: past WARP (listed beside a real device, as
/// lavapipe on a Windows runner), a discrete card, then an integrated one, then the rest.
fn compute_rank(name: &str, kind: wgpu::DeviceType) -> Option<u8> {
    if name.to_lowercase().contains("microsoft basic render") {
        return None;
    }
    Some(match kind {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        _ => 2,
    })
}

/// The adapter the compute host opens of `adapters` on its own: the first of the best rank on a
/// primary backend (Vulkan, Metal, DX12).
pub fn compute_pick(adapters: &[crate::AdapterFact]) -> Option<&crate::AdapterFact> {
    let primary = |b: wgpu::Backend| matches!(b, wgpu::Backend::Vulkan | wgpu::Backend::Metal | wgpu::Backend::Dx12);
    adapters.iter().filter(|a| primary(a.backend)).filter_map(|a| Some((compute_rank(&a.name, a.kind)?, a))).min_by_key(|(r, _)| *r).map(|(_, a)| a)
}

/// Why the compute host does not run on the window's device `display`, or `None` — it does
/// (stream-picture-stays D2; the owner, kkt-26.11.20 Q51: "Сделать сейчас"). On GL the window's device
/// computes nothing; on another adapter than the one the host would open (a hybrid laptop's window
/// on the integrated chip, the host on the card) the host keeps its own device.
pub fn share_refusal(display: &wgpu::AdapterInfo, pick: Option<&crate::AdapterFact>) -> Option<&'static str> {
    if crate::display_is_ui_only(display) {
        return Some("the window's backend is GL: no compute on it");
    }
    let Some(p) = pick else { return Some("no adapter the compute host would open") };
    let same = p.backend == display.backend && p.vendor == display.vendor && p.device == display.device && p.name == display.name;
    (!same).then_some("the compute host would open another adapter")
}

/// The window's device as the process's one context (stream-picture-stays D2): the preview's engine
/// and, where [`share_refusal`] allows, the compute host run on this very `Arc`, so a buffer the host
/// keeps is the engine's to read (`held::on` compares the device pointer). Called before anything
/// opens the host; a later call keeps the first context.
pub fn share_display(device: &wgpu::Device, queue: &wgpu::Queue, info: wgpu::AdapterInfo) -> crate::GpuContext {
    static DISPLAY: std::sync::OnceLock<crate::GpuContext> = std::sync::OnceLock::new();
    let ctx = DISPLAY
        .get_or_init(|| {
            let ctx = crate::GpuContext::from_parts(Arc::new(device.clone()), Arc::new(queue.clone()), info.clone());
            let facts = crate::adapter_facts();
            let why = share_refusal(&info, compute_pick(&facts));
            kt_time::trace::machine("gpu.compute.shares_display", why.unwrap_or("yes"));
            if why.is_none() {
                let _ = SHARED.set(ctx.clone());
            }
            ctx
        })
        .clone();
    // another window's device (a second harness in one process): its own context, nothing shared
    if *ctx.device != *device {
        return crate::GpuContext::from_parts(Arc::new(device.clone()), Arc::new(queue.clone()), info);
    }
    ctx
}

/// Tests: the host holds a stream's pictures on its device although no window shares it (an engine on
/// the host's device, `crate::front_on_compute`, composes them).
static HOLD_FOR_TESTS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[doc(hidden)]
pub fn hold_pictures_for_tests() {
    HOLD_FOR_TESTS.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// A headless export on a card (kt-cli, stream-picture-stays D4): the host holds a stream's pictures,
/// and the export composes on the host's device (`crate::export_engines`), so they reach it as they
/// lie. On a software adapter, or with no host, nothing changes: the export keeps its several devices.
pub fn hold_pictures_for_export() {
    if let Some(dev) = host_device()
        && dev.info.device_type != wgpu::DeviceType::Cpu
    {
        HOLD_FOR_TESTS.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Whether an export composes on the compute host's device (stream-picture-stays D4): a card's export
/// (one engine) while the host holds pictures; never a software adapter's several devices, never with
/// `KT_VIDEO_ENGINES` set.
pub fn export_on_host(host: &wgpu::AdapterInfo, holds: bool, engines_forced: bool) -> bool {
    holds && !engines_forced && host.device_type != wgpu::DeviceType::Cpu
}

/// Whether the host holds pictures, asked without opening it.
pub fn pictures_held() -> bool {
    HOLD_FOR_TESTS.load(std::sync::atomic::Ordering::Relaxed) || SHARED.get().is_some()
}

/// Whether the compute host runs on the window's device (tests, the trace's facts).
pub fn shares_display() -> bool {
    SHARED.get().is_some()
}

pub fn open_device() -> Result<Arc<ComputeDevice>, String> {
    // the prewarm opens it on its own thread at a project's opening: the exit waits (pipeline-prewarm D10)
    let _busy = crate::exit::busy().ok_or("the process is exiting")?;
    // a frame whose generator or stage waits for the opening names it (found-defects 845: 24 s on a
    // standing disk, and the trace said nothing)
    let _h = crate::holds::hold(crate::errors::role::COMPUTE, "open the device");
    if let Some(ctx) = SHARED.get() {
        crate::context::trace_device("compute", &ctx.adapter_info, &ctx.device);
        return Ok(ComputeDevice::on_context(ctx));
    }
    // the process's one instance and opening lock (segv-crashes): see `context::instance`
    let g = crate::context::opening();
    let instance = crate::context::instance(&g, wgpu::Backends::PRIMARY);
    let mut adapters: Vec<(u8, wgpu::Adapter)> = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::PRIMARY))
        .into_iter()
        .filter_map(|a| Some((compute_rank(&a.get_info().name, a.get_info().device_type)?, a)))
        .collect();
    adapters.sort_by_key(|(r, _)| *r);
    let adapter = adapters.into_iter().next().map(|(_, a)| a).ok_or("no compute device")?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("kt-gpu compute"),
        required_features: crate::pass_time::wanted_features(&adapter),
        ..Default::default()
    }))
    .map_err(|e| e.to_string())?;
    crate::errors::watch(&device, crate::errors::role::COMPUTE);
    let stamps = make_stamps(&device);
    let info = adapter.get_info();
    crate::context::trace_device("compute", &info, &device);
    Ok(ComputeDevice::with(Arc::new(device), Arc::new(queue), &info, stamps))
}

impl ComputeDevice {
    fn with(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        info: &wgpu::AdapterInfo,
        stamps: Vec<Stamps>,
    ) -> Arc<ComputeDevice> {
        let cache = crate::pipeline_cache::DiskCache::open(&device, info, "compute", declared_hash());
        let arb = crate::arbiter::ring_for(crate::arbiter::RingKey::of(info, &device));
        Arc::new(ComputeDevice {
            _on_ring: arb.register_compute(),
            arb,
            last: Mutex::new(HashMap::new()),
            device,
            queue,
            adapter: info.name.clone(),
            info: info.clone(),
            timed: !stamps.is_empty(),
            stamps: Mutex::new(stamps),
            seed_key: crate::seeds::device_key(info),
            pipes: Mutex::new(HashMap::new()),
            calib: Mutex::default(),
            best: Mutex::default(),
            pool: Mutex::default(),
            account: kt_time::mem::open(kt_time::mem::Tenant::Compute, POOL_BYTES),
            spares: Arc::default(),
            cache,
        })
    }

    /// A compute host on an engine's own device (the bench `kt-cli gpu-share`, gpu-arbiter): the
    /// display, the offline work and the compute jobs on ONE queue, as a card's ring runs them.
    pub fn on_context(ctx: &crate::GpuContext) -> Arc<ComputeDevice> {
        let stamps = make_stamps(&ctx.device);
        ComputeDevice::with(ctx.device.clone(), ctx.queue.clone(), &ctx.adapter_info, stamps)
    }

    /// Its device as an engine's context (the engine of the slots in front of a stream stage).
    pub fn context(&self) -> crate::GpuContext {
        crate::GpuContext { device: self.device.clone(), queue: self.queue.clone(), adapter_info: self.info.clone() }
    }

    /// Tests: hold the calibration as a job calibrating would, until the guard drops (found-defects 562).
    #[doc(hidden)]
    pub fn hold_calibration_for_tests(&self) -> impl Sized + '_ {
        self.calib.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Tests: make the calibration `by` older.
    #[doc(hidden)]
    pub fn age_calibration_for_tests(&self, by: std::time::Duration) {
        if let Some(b) = self.best.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            b.3 = b.3.checked_sub(by).unwrap_or(b.3);
        }
    }

    /// The ring it runs on.
    pub fn ring(&self) -> &Arc<crate::arbiter::Ring> {
        &self.arb
    }
}

impl ComputeDevice {
    /// A buffer of at least `size` bytes; true when it comes from the pool and holds a past job.
    fn take(&self, size: u64, usage: wgpu::BufferUsages) -> (wgpu::Buffer, bool) {
        let size = size.max(16).next_multiple_of(16);
        if usage == JOB_USAGE
            && let Some(b) = self.spares.try_take(size)
        {
            return (b, true);
        }
        let mut pool = self.pool.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(b) = pool.free.get_mut(&(size, usage.bits())).and_then(Vec::pop) {
            pool.bytes -= size;
            self.account.set(pool.bytes);
            return (b, true);
        }
        drop(pool);
        crate::created();
        (self.device.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage, mapped_at_creation: false }), false)
    }

    fn give(&self, buffers: impl IntoIterator<Item = wgpu::Buffer>) {
        let mut pool = self.pool.lock().unwrap_or_else(|e| e.into_inner());
        for b in buffers {
            if pool.bytes + b.size() <= POOL_BYTES {
                pool.bytes += b.size();
                pool.free.entry((b.size(), b.usage().bits())).or_default().push(b);
            }
        }
        self.account.set(pool.bytes);
    }

    /// What the pool holds now.
    pub fn pooled_bytes(&self) -> u64 {
        self.pool.lock().unwrap_or_else(|e| e.into_inner()).bytes
    }

    /// The pipeline of a text, and whether this call created it.
    fn pipe(&self, text: &str, reads: usize, writes: usize) -> Result<(Arc<Pipe>, bool), String> {
        let slot = self.pipes.lock().unwrap_or_else(|e| e.into_inner()).entry((text.to_string(), reads, writes)).or_default().clone();
        let mut slot = crate::holds::lock(&slot, "compute pipeline", Some(crate::errors::role::COMPUTE));
        if let Some(p) = slot.as_ref() {
            return Ok((p.clone(), false));
        }
        let _h = crate::holds::hold(crate::errors::role::COMPUTE, "create compute pipeline");
        let d = &self.device;
        // the uniform at a dynamic offset into the job's one arena (gpu-leftovers, found-defects 520)
        let entry = |binding: usize, ty| wgpu::BindGroupLayoutEntry {
            binding: binding as u32,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: binding == 0, min_binding_size: None },
            count: None,
        };
        let mut entries = vec![entry(0, wgpu::BufferBindingType::Uniform)];
        for i in 0..reads + writes {
            entries.push(entry(i + 1, wgpu::BufferBindingType::Storage { read_only: i < reads }));
        }
        // two scopes: a compiler's refusal is an internal error, a bad text a validation one
        let validation = d.push_error_scope(wgpu::ErrorFilter::Validation);
        let internal = d.push_error_scope(wgpu::ErrorFilter::Internal);
        let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(text.into()),
        });
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &entries });
        let pipe_layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        crate::created();
        let pipe = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipe_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: self.cache.as_ref().map(|c| c.get()),
        });
        if let Some(e) = pollster::block_on(internal.pop()).or(pollster::block_on(validation.pop())) {
            return Err(format!("compute shader refused: {e}"));
        }
        let p = Arc::new(Pipe { pipe, layout, warm: Default::default() });
        *slot = Some(p.clone());
        Ok((p, true))
    }
}

impl ComputeDevice {
    /// Bracket a device timestamp by the trace's clock (D7): while the trace records, at the first job
    /// and when the calibration is older than `CALIBRATION_AGE`; three brackets the first time, the
    /// narrowest kept. One empty submission and a wait each — never at rest (no job, no calibration).
    /// `true` — another job was calibrating and this one went on without (found-defects 562).
    fn calibrate(&self) -> bool {
        if !self.timed || !kt_time::trace::on() {
            return false;
        }
        // a job finding another calibrating goes on without: the calibration waits for its admission,
        // and a frame's job must never wait for a lower class's admission behind this lock
        let mut c = match self.calib.try_lock() {
            Ok(c) => c,
            Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return true,
        };
        let prior = *self.best.lock().unwrap_or_else(|e| e.into_inner());
        if prior.is_some_and(|b| b.3.elapsed() < CALIBRATION_AGE) {
            return false;
        }
        let d = &self.device;
        if c.set.is_none() {
            crate::created();
            let set = d.create_query_set(&wgpu::QuerySetDescriptor { label: Some("kt-gpu compute calibration"), ty: wgpu::QueryType::Timestamp, count: 1 });
            let resolve = d.create_buffer(&wgpu::BufferDescriptor {
                label: Some("kt-gpu compute calibration resolve"),
                size: 8,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let read = d.create_buffer(&wgpu::BufferDescriptor {
                label: Some("kt-gpu compute calibration read"),
                size: 8,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            c.set = Some((set, resolve, read));
        }
        let tries = if prior.is_none() { 3 } else { 1 };
        let mut best: Option<(u64, u64, u64, std::time::Instant)> = None;
        for _ in 0..tries {
            let Some((set, resolve, read)) = c.set.as_ref() else { return false };
            let mut enc = d.create_command_encoder(&Default::default());
            drop(enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("kt-gpu compute calibration"),
                timestamp_writes: Some(wgpu::ComputePassTimestampWrites { query_set: set, beginning_of_pass_write_index: Some(0), end_of_pass_write_index: None }),
            }));
            enc.resolve_query_set(set, 0..1, resolve, 0);
            enc.copy_buffer_to_buffer(resolve, 0, read, 0, 8);
            // admitted on the ring as every other submission (gpu-leftovers, found-defects 511): the
            // bracket is taken after the admission, so a wait for room never widens it
            let ticket = self.arb.admit(kt_time::sched::current_class(), CALIBRATION_NS);
            let before = kt_time::trace::now_ns();
            let sub = { let _h = crate::holds::hold(crate::errors::role::COMPUTE, "compute submit"); self.queue.submit([enc.finish()]) };
            let flight = ticket.submitted(&self.device, &self.queue, sub.clone(), CALIBRATION_NS, false);
            let (tx, rx) = std::sync::mpsc::channel();
            read.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
            // its own submission (datamosh-history-wait D2): the bracket's half-width is the clock
            // error, and a wait for everything widened it by every job submitted meanwhile (his
            // kt26.11.16 trace: error up to 39 ms, wider than the waits it was to measure)
            let waited = {
                let _h = crate::holds::hold(crate::errors::role::COMPUTE, "calibration wait");
                crate::wait_for(d, sub)
            };
            if waited.is_err() {
                return false;
            }
            let after = kt_time::trace::now_ns();
            flight.wait();
            if !matches!(rx.recv(), Ok(Ok(()))) {
                return false;
            }
            let g = read.slice(..).get_mapped_range().map(|m| u64::from_le_bytes(m[..8].try_into().unwrap())).unwrap_or(0);
            read.unmap();
            let half = (after - before) / 2;
            if g != 0 && best.is_none_or(|b| half < b.2) {
                best = Some((g, before + half, half, std::time::Instant::now()));
            }
        }
        if best.is_some() {
            *self.best.lock().unwrap_or_else(|e| e.into_inner()) = best;
        }
        false
    }

    /// A device timestamp on the trace's clock, and the error; `None` — no calibration.
    fn to_trace_ns(&self, ticks: u64, skipped: bool) -> Option<(u64, u64)> {
        let (g0, mid, half, made) = (*self.best.lock().unwrap_or_else(|e| e.into_inner()))?;
        // a calibration older than its age is not used: the job that would renew it found another
        // calibrating and went on without (gpu-leftovers D1); its record says it has no position
        // rather than one on a drifted clock (found-defects 562)
        if skipped && made.elapsed() > CALIBRATION_AGE {
            STALE_RECORDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return None;
        }
        let period = self.queue.get_timestamp_period() as f64;
        let ns = mid as f64 + (ticks as f64 - g0 as f64) * period;
        Some((ns.max(0.0) as u64, half))
    }
}

impl ComputeHost for ComputeDevice {
    fn prepare(&self, shader: &Shader) -> Result<(), String> {
        // on the compute-warm thread: the exit waits for it (pipeline-prewarm D10)
        let _busy = crate::exit::busy().ok_or("the process is exiting")?;
        let (p, _) = self.pipe(&shader.text, shader.reads, shader.writes)?;
        if p.warm.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }
        // one workgroup over zero buffers: every size the shader reads from its uniform is 0. The
        // uniform takes the largest binding a device must allow (a uniform may carry a table: the
        // Datamosh residual's DCT basis)
        let d = &self.device;
        let zeros = |size: u64, usage| d.create_buffer(&wgpu::BufferDescriptor { label: Some("kt-gpu compute warm"), size, usage, mapped_at_creation: false });
        let uniform = zeros(d.limits().max_uniform_buffer_binding_size.min(1 << 16), wgpu::BufferUsages::UNIFORM);
        let bufs: Vec<wgpu::Buffer> = (0..shader.reads + shader.writes).map(|_| zeros(256, wgpu::BufferUsages::STORAGE)).collect();
        let size = wgpu::BufferSize::new(uniform.size());
        let mut entries = vec![wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &uniform, offset: 0, size }) }];
        entries.extend(bufs.iter().enumerate().map(|(i, b)| wgpu::BindGroupEntry { binding: i as u32 + 1, resource: b.as_entire_binding() }));
        let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &p.layout, entries: &entries });
        let mut enc = d.create_command_encoder(&Default::default());
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("kt-gpu compute warm"), timestamp_writes: None });
            cp.set_pipeline(&p.pipe);
            cp.set_bind_group(0, &bind, &[0]);
            cp.dispatch_workgroups(1, 1, 1);
        }
        // the warm thread's class (background) on the ring: it never stands in front of a frame
        let ticket = self.arb.admit(kt_time::sched::current_class(), WARM_NS);
        let sub = { let _h = crate::holds::hold(crate::errors::role::COMPUTE, "compute submit"); self.queue.submit([enc.finish()]) };
        let flight = ticket.submitted(&self.device, &self.queue, sub.clone(), WARM_NS, false);
        let refused = pollster::block_on(scope.pop());
        crate::wait_for(d, sub).map_err(|e| e.to_string())?;
        flight.wait();
        if let Some(e) = refused {
            return Err(format!("compute warm-up refused: {e}"));
        }
        p.warm.store(true, std::sync::atomic::Ordering::Release);
        if let Some(c) = &self.cache {
            c.save();
        }
        Ok(())
    }

    fn run(&self, job: &Job<'_>) -> Result<Vec<Vec<u8>>, String> {
        self.run_job(job, &[]).map(|r| r.0)
    }

    fn keeps(&self) -> bool {
        true
    }

    fn holds_pictures(&self) -> bool {
        HOLD_FOR_TESTS.load(std::sync::atomic::Ordering::Relaxed) || SHARED.get().is_some_and(|c| Arc::ptr_eq(&c.device, &self.device))
    }

    fn run_keeping(&self, job: &Job<'_>, keep: &[Buf]) -> Result<(Vec<Vec<u8>>, Vec<Held>), String> {
        self.run_job(job, keep)
    }
}

/// A job's storage buffers: read and written by its steps, copied to and from.
const JOB_USAGE: wgpu::BufferUsages =
    wgpu::BufferUsages::STORAGE.union(wgpu::BufferUsages::COPY_DST).union(wgpu::BufferUsages::COPY_SRC);

/// Buffers jobs kept on the device (stream-round-trips D1), for tests.
static KEPT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How many job buffers stayed on the device after their job.
pub fn kept_buffers() -> u64 {
    KEPT.load(std::sync::atomic::Ordering::Relaxed)
}

impl ComputeDevice {
    /// A job; the buffers `keep` names stay on the device as held buffers, the rest go back to the pool.
    fn run_job(&self, job: &Job<'_>, keep: &[Buf]) -> Result<(Vec<Vec<u8>>, Vec<Held>), String> {
        let d = &self.device;
        // the job in the trace: its kernel, dispatches, the device time and the wait for its buffers;
        // its cause is the caller's (a frame's stream stage, a prefetch advance)
        let mut span = kt_time::trace::span(kt_time::trace::Ev::ComputeJob);
        let class = kt_time::sched::current_class();
        if kt_time::trace::on() {
            span.aux(job.steps.first().map_or(0, |s| kt_time::trace::name(s.name)))
                .a(job.steps.len() as u64)
                .id(class as u64);
        }
        let skipped = self.calibrate();
        // what each dispatch took when last measured — in this process, else on this device before
        // (gpu-throughput D2): the job's claim on the ring and its cuts
        let mut predicted: Vec<Option<u64>> = {
            let last = self.last.lock().unwrap_or_else(|e| e.into_inner());
            job.steps
                .iter()
                .map(|s| last.get(s.name).copied().or_else(|| crate::seeds::predict(&self.seed_key, s.name, units(s.groups))))
                .collect()
        };
        let pipes = job
            .steps
            .iter()
            .map(|s| {
                let (p, created) = self.pipe(s.shader, s.read.len(), s.write.len())?;
                // a text no prewarm prepared compiles here, inside the frame (pipeline-prewarm D6)
                if created {
                    crate::compiled::in_frame();
                }
                if !p.warm.swap(true, std::sync::atomic::Ordering::AcqRel) {
                    crate::compiled::cold_draw();
                }
                Ok(p)
            })
            .collect::<Result<Vec<_>, String>>()?;
        if job.buffers.iter().any(|b| matches!(b, Init::Held(h) if crate::held::on(h, &self.device).is_none())) {
            return Err("a held buffer of another device".into());
        }
        let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
        let usage = JOB_USAGE;
        let mut enc = d.create_command_encoder(&Default::default());
        let mut bufs: Vec<wgpu::Buffer> = Vec::with_capacity(job.buffers.len());
        for init in job.buffers {
            bufs.push(match init {
                Init::Bytes(b) => {
                    let (buf, _) = self.take(b.len() as u64, usage);
                    self.queue.write_buffer(&buf, 0, b);
                    buf
                }
                Init::Zero(n) => {
                    let (buf, used) = self.take(*n, usage);
                    if used {
                        enc.clear_buffer(&buf, 0, None);
                    }
                    buf
                }
                // a frame an engine on this device left here (stream-front-on-device): copied on the
                // device, ordered after the engine's submission on the one queue
                Init::Held(h) => {
                    let src = crate::held::on(h, &self.device).expect("checked before the scope");
                    let (buf, _) = self.take(h.len, usage);
                    enc.copy_buffer_to_buffer(src, 0, &buf, 0, h.len);
                    HELD_READS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    buf
                }
            });
        }
        // every step's uniform in one arena, one write (gpu-leftovers, found-defects 520): a step binds
        // its slot by a dynamic offset, and a bind group serves every step of the same shader, slot
        // size and buffers
        let align = (d.limits().min_uniform_buffer_offset_alignment as u64).max(16);
        let mut slots = Vec::with_capacity(job.steps.len());
        let mut total = 0u64;
        for st in job.steps {
            let size = (st.uniform.len() as u64).max(16).next_multiple_of(16);
            slots.push((total, size));
            total = (total + size).next_multiple_of(align);
        }
        let (arena, _) = self.take(total.max(16), wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        let mut bytes = vec![0u8; arena.size() as usize];
        for (st, &(at, _)) in job.steps.iter().zip(&slots) {
            bytes[at as usize..at as usize + st.uniform.len()].copy_from_slice(st.uniform);
        }
        self.queue.write_buffer(&arena, 0, &bytes);
        drop(bytes);
        let mut groups: Vec<BindKey<'_>> = Vec::new();
        // this job's own timestamp set (D6): none free — untimed
        let stamps: Option<Stamps> = if self.timed { self.stamps.lock().unwrap_or_else(|e| e.into_inner()).pop() } else { None };
        let timed = stamps.as_ref().map_or(0, |_| job.steps.len().min(STAMPS as usize / 2));
        let claim = if predicted.iter().all(Option::is_some) { predicted.iter().flatten().sum() } else { 0 };
        let mut piece = crate::arbiter::Piece::begin(&self.arb, claim);
        // the pieces submitted: (first dispatch, end, submit on the trace's clock)
        let mut pieces: Vec<(usize, usize, u64)> = Vec::new();
        let mut from = 0usize;
        let mut learns: Vec<Learn> = Vec::new();
        for i in 0..job.steps.len() {
            let (s, p) = (&job.steps[i], &pipes[i]);
            // what the pieces already run measured: every later dispatch of the same name (D2)
            if !learns.is_empty() {
                self.learn(&mut learns, job.steps, &mut predicted, i);
            }
            // a dispatch past the ring's bound, or a higher class waiting, ends the piece: the
            // buffers stay on the device, the next piece goes on with them (gpu-arbiter D4)
            if piece.due(predicted[i]) {
                if predicted[i].is_none() {
                    GUESS_CUTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                let mut full = std::mem::replace(&mut enc, d.create_command_encoder(&Default::default()));
                let to = i.min(timed);
                if let Some((set, resolve)) = stamps.as_ref().filter(|_| to > from) {
                    // the piece's own timestamps come back while the job goes on
                    let at = (16 * from as u64).next_multiple_of(wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT);
                    let bytes = 16 * (to - from) as u64;
                    full.resolve_query_set(set, 2 * from as u32..2 * to as u32, resolve, at);
                    let (lb, _) = self.take(bytes, wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ);
                    full.copy_buffer_to_buffer(resolve, at, &lb, 0, bytes);
                    learns.push(Learn { buf: lb, from, to, ready: Default::default() });
                }
                kt_time::work::bump(kt_time::work::Work::DeviceSubmit);
                let at = kt_time::trace::now_ns();
                let sub = { let _h = crate::holds::hold(crate::errors::role::COMPUTE, "compute submit"); self.queue.submit([full.finish()]) };
                pieces.push((from, i, at));
                from = i;
                if let Some(l) = learns.last().filter(|l| l.to == i.min(timed) && l.ready.load(std::sync::atomic::Ordering::Acquire) == 0) {
                    let ready = l.ready.clone();
                    let bytes = 16 * (l.to - l.from) as u64;
                    l.buf.slice(..bytes).map_async(wgpu::MapMode::Read, move |r| {
                        ready.store(if r.is_ok() { 1 } else { 2 }, std::sync::atomic::Ordering::Release);
                    });
                }
                piece.cut(&self.device, &self.queue, sub, false, predicted[i]);
            }
            piece.add(predicted[i]);
            let (at, size) = slots[i];
            let found = groups.iter().position(|g| Arc::ptr_eq(&g.pipe, p) && g.size == size && g.read == s.read && g.write == s.write);
            let g = match found {
                Some(g) => g,
                None => {
                    let mut entries = vec![wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &arena, offset: 0, size: wgpu::BufferSize::new(size) }),
                    }];
                    for (i, b) in s.read.iter().chain(s.write).enumerate() {
                        entries.push(wgpu::BindGroupEntry { binding: i as u32 + 1, resource: bufs[b.0].as_entire_binding() });
                    }
                    BIND_GROUPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let bind = d.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &p.layout, entries: &entries });
                    groups.push(BindKey { pipe: p.clone(), size, read: s.read, write: s.write, bind });
                    groups.len() - 1
                }
            };
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: stamps.as_ref().filter(|_| i < timed).map(|(set, _)| wgpu::ComputePassTimestampWrites {
                    query_set: set,
                    beginning_of_pass_write_index: Some(2 * i as u32),
                    end_of_pass_write_index: Some(2 * i as u32 + 1),
                }),
            });
            cp.set_pipeline(&p.pipe);
            cp.set_bind_group(0, &groups[g].bind, &[at as u32]);
            cp.dispatch_workgroups(s.groups[0], s.groups[1], s.groups[2]);
            drop(cp);
        }
        drop(groups);
        let staging: Vec<wgpu::Buffer> = job
            .reads
            .iter()
            .map(|b| {
                let size = bufs[b.0].size();
                let (st, _) = self.take(size, wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ);
                enc.copy_buffer_to_buffer(&bufs[b.0], 0, &st, 0, size);
                st
            })
            .collect();
        // the dispatches' timestamps come back with the job's own buffers
        let stamp_read = stamps.as_ref().filter(|_| timed > 0).map(|(set, resolve)| {
            let bytes = timed as u64 * 16;
            enc.resolve_query_set(set, 0..2 * timed as u32, resolve, 0);
            let (st, _) = self.take(bytes, wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ);
            enc.copy_buffer_to_buffer(resolve, 0, &st, 0, bytes);
            st
        });
        kt_time::work::bump(kt_time::work::Work::DeviceSubmit);
        let submitted = std::time::Instant::now();
        let submitted_ns = kt_time::trace::now_ns();
        let sub = { let _h = crate::holds::hold(crate::errors::role::COMPUTE, "compute submit"); self.queue.submit([enc.finish()]) };
        pieces.push((from, job.steps.len(), submitted_ns));
        // this job's own submission: a wait for "the most recent" one waited for every piece another
        // thread submitted to the device after it too (stream-round-trips D9, found-defects 661: its
        // buffers came back 311 ms after its last dispatch ended at the median on the owner's iGPU)
        let mine = sub.clone();
        let _flight = piece.end(&self.device, &self.queue, sub, false);
        if let Some(e) = pollster::block_on(scope.pop()) {
            self.give_back(stamps, learns);
            return Err(format!("compute job refused: {e}"));
        }
        // every read mapped first, then one wait: a wait per buffer was a poll per buffer
        let maps: Vec<std::sync::mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>> = staging
            .iter()
            .map(|st| {
                let (tx, rx) = std::sync::mpsc::channel();
                st.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                    let _ = tx.send(r);
                });
                rx
            })
            .collect();
        if !maps.is_empty() {
            let _h = crate::holds::hold(crate::errors::role::COMPUTE, "compute wait");
            crate::wait_for(d, mine.clone()).map_err(|e| e.to_string())?;
        }
        let mut out = Vec::with_capacity(staging.len());
        for ((st, b), rx) in staging.iter().zip(job.reads).zip(maps) {
            rx.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
            let len = job.buffers[b.0].len();
            out.push(st.slice(..len).get_mapped_range().map_err(|e| e.to_string())?.to_vec());
            st.unmap();
        }
        let waited = submitted.elapsed().as_nanos() as u64;
        span.c(waited);
        match stamp_read {
            Some(st) => {
                let (tx, rx) = std::sync::mpsc::channel();
                let bytes = timed as u64 * 16;
                st.slice(..bytes).map_async(wgpu::MapMode::Read, move |r| {
                    let _ = tx.send(r);
                });
                // the job's own wait has passed: this map is ready at the next poll
                crate::wait_for(d, mine).map_err(|e| e.to_string())?;
                if let Ok(Ok(())) = rx.recv() {
                    let ticks: Vec<u64> = st
                        .slice(..bytes)
                        .get_mapped_range()
                        .map_err(|e| e.to_string())?
                        .chunks_exact(8)
                        .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
                        .collect();
                    st.unmap();
                    let period = self.queue.get_timestamp_period() as f64;
                    let ds: Vec<(&str, u64)> = job.steps[..timed]
                        .iter()
                        .enumerate()
                        .map(|(i, s)| (s.name, (ticks[2 * i + 1].saturating_sub(ticks[2 * i]) as f64 * period) as u64))
                        .collect();
                    crate::pass_time::note_job(&self.adapter, ds.iter().map(|d| d.1).sum(), &ds);
                    // when each piece ran on the device, on the trace's clock (found-defects 398, 486)
                    if kt_time::trace::on() {
                        for &(a, b, at) in pieces.iter().filter(|p| p.0 < timed) {
                            let start = self.to_trace_ns(ticks[2 * a], skipped);
                            let end = self.to_trace_ns(ticks[2 * b.min(timed) - 1], skipped);
                            kt_time::trace::ev(kt_time::trace::Ev::GpuStart)
                                .id(class as u64)
                                .a(at)
                                .b(start.map_or(0, |s| s.0))
                                .c(end.map_or(0, |e| e.0))
                                .aux(start.map_or(u32::MAX - 1, |s| (s.1 / 1000).min(u32::MAX as u64 - 2) as u32))
                                .emit();
                        }
                    }
                    let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
                    for (i, (name, ns)) in ds.iter().enumerate() {
                        let old = last.get(*name).copied();
                        // the seeds hear a dispatch when it is new or moved by a quarter
                        if old.is_none_or(|o| ns.abs_diff(o) > o / 4) {
                            crate::seeds::note(&self.seed_key, name, units(job.steps[i].groups), *ns);
                        }
                        match last.get_mut(*name) {
                            Some(v) => *v = *ns,
                            None => {
                                last.insert(name.to_string(), *ns);
                            }
                        }
                    }
                    span.b(ds.iter().map(|d| d.1).sum());
                }
                self.give([st]);
            }
            None => {
                crate::pass_time::note_job(&self.adapter, waited, &[]);
                // the device gives no timestamps (or no set was free): the record says so
                if kt_time::trace::on() {
                    for &(_, _, at) in &pieces {
                        kt_time::trace::ev(kt_time::trace::Ev::GpuStart).id(class as u64).a(at).aux(u32::MAX).emit();
                    }
                }
            }
        }
        self.give_back(stamps, learns);
        let mut bufs: Vec<Option<wgpu::Buffer>> = bufs.into_iter().map(Some).collect();
        let held = keep
            .iter()
            .map(|b| {
                let len = job.buffers[b.0].len();
                let buf = bufs[b.0].take().expect("a buffer kept once");
                KEPT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                crate::held::held(buf, len, self.device.clone(), self.spares.clone())
            })
            .collect();
        self.give(bufs.into_iter().flatten().chain([arena]).chain(staging));
        Ok((out, held))
    }
}

impl ComputeDevice {
    /// The pieces' timestamps that came back (D2): every later dispatch of a name they measured is
    /// predicted by it, from step `next` on.
    fn learn(&self, learns: &mut Vec<Learn>, steps: &[kt_plugin_api::compute::Step<'_>], predicted: &mut [Option<u64>], next: usize) {
        use std::sync::atomic::Ordering;
        if learns.iter().all(|l| l.ready.load(Ordering::Acquire) == 0) {
            let _h = crate::holds::hold(crate::errors::role::COMPUTE, "compute poll");
            let _ = self.device.poll(wgpu::PollType::Poll);
        }
        let period = self.queue.get_timestamp_period() as f64;
        let mut i = 0;
        while i < learns.len() {
            match learns[i].ready.load(Ordering::Acquire) {
                0 => i += 1,
                state => {
                    let l = learns.remove(i);
                    if state == 1 {
                        let bytes = 16 * (l.to - l.from) as u64;
                        let ticks: Vec<u64> = match l.buf.slice(..bytes).get_mapped_range() {
                            Ok(m) => m.chunks_exact(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).collect(),
                            Err(_) => Vec::new(),
                        };
                        l.buf.unmap();
                        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
                        for (k, j) in (l.from..l.to).enumerate() {
                            let Some(pair) = ticks.get(2 * k..2 * k + 2) else { break };
                            let ns = (pair[1].saturating_sub(pair[0]) as f64 * period) as u64;
                            let name = steps[j].name;
                            for (q, s) in steps.iter().enumerate().skip(next) {
                                if s.name == name && predicted[q].is_none() {
                                    predicted[q] = Some(ns);
                                }
                            }
                            last.entry(name.to_string()).or_insert(ns);
                        }
                        LEARNED.fetch_add(1, Ordering::Relaxed);
                    }
                    self.give([l.buf]);
                }
            }
        }
    }

    /// A job's timestamp set back to the free ones, and its pieces' read buffers to the pool (a map
    /// still pending is unmapped first).
    fn give_back(&self, stamps: Option<Stamps>, learns: Vec<Learn>) {
        if let Some(s) = stamps {
            self.stamps.lock().unwrap_or_else(|e| e.into_inner()).push(s);
        }
        for l in learns {
            if l.ready.load(std::sync::atomic::Ordering::Acquire) == 1 {
                l.buf.unmap();
            }
            // a map never answered: the buffer is dropped rather than pooled mapped-pending
            if l.ready.load(std::sync::atomic::Ordering::Acquire) != 0 {
                self.give([l.buf]);
            }
        }
    }
}

/// A job's bind group, by what it binds: the step's shader, its uniform slot's size and its buffers.
struct BindKey<'a> {
    pipe: Arc<Pipe>,
    size: u64,
    read: &'a [kt_plugin_api::compute::Buf],
    write: &'a [kt_plugin_api::compute::Buf],
    bind: wgpu::BindGroup,
}

static BIND_GROUPS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Bind groups the compute host's jobs created (tests).
pub fn bind_groups_made() -> u64 {
    BIND_GROUPS.load(std::sync::atomic::Ordering::Relaxed)
}

static LEARNED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static GUESS_CUTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Cuts made before a dispatch never measured (tests): what a guess cost, apart from the cuts the
/// measured times call for — those follow the machine's speed (a software device's time is processor
/// time).
pub fn guess_cuts() -> u64 {
    GUESS_CUTS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Pieces whose timestamps a running job learned from (tests).
pub fn pieces_learned() -> u64 {
    LEARNED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Compile one compute shader on `ctx` and run nothing (t1651, FXC on the Windows runner).
pub fn compile(ctx: &crate::GpuContext, text: &str) -> Result<(), String> {
    let d = &ctx.device;
    let validation = d.push_error_scope(wgpu::ErrorFilter::Validation);
    let internal = d.push_error_scope(wgpu::ErrorFilter::Internal);
    let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(text.into()),
    });
    let _ = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    match pollster::block_on(internal.pop()).or(pollster::block_on(validation.pop())) {
        Some(e) => Err(e.to_string()),
        None => Ok(()),
    }
}

static HELD_READS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How many job buffers started as a copy of a held frame on the device (stream-front-on-device:
/// tests and `kt-cli stream-cost`).
pub fn held_reads() -> u64 {
    HELD_READS.load(std::sync::atomic::Ordering::Relaxed)
}

static STALE_RECORDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How many device timestamps were not placed on the trace's clock because the calibration was
/// older than its age when the job found another calibrating (found-defects 562).
pub fn stale_records() -> u64 {
    STALE_RECORDS.load(std::sync::atomic::Ordering::Relaxed)
}
