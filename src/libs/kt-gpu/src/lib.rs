//! The GPU backend of video FX and compositing (R.29): wgpu f32/integer
//! fragment passes on top of the eframe device (preview) or headless
//! (export/kt-cli). Implements kt_engine::VideoGpuExec; kt-engine does NOT depend on
//! this crate: the executor is injected from above (kt-app,
//! kt-cli). The CPU composite is deleted (compute-kernels-render); the stored frames of its last
//! output are the reference, and the conformance is tests/parity.rs (tolerances, t181/t182/t183).

pub mod compute;
pub mod memory;
pub mod pass_time;
pub mod reference;
pub mod testkit;
mod compiled;
mod context;
mod exit;
mod engine;
mod pipeline_cache;
mod pool;
mod warm;

pub use context::{
    AdapterFact, DrivingCard, GpuContext, GpuInitError, adapter_facts, display_power_preference, driving_cards,
    instances_created, power_preference_for,
};
pub use compiled::{cold_draws, pipelines_in_frame};
pub use exit::background_gpu_work;
pub use pipeline_cache::set_cache_dir;
pub use engine::{DisplayTarget, GpuFxEngine, assembled_composite, assembled_shaders, plugin_taps, plugin_transitions, plugin_warps};

/// Textures, buffers and pipelines the engine and the compute host created since the process
/// started (gpu-pass-cost): after warm-up a frame creates none (t2023). Bind groups and command
/// encoders, which every pass makes by design, are not counted.
pub fn resources_created() -> u64 {
    CREATED.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) static CREATED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn created() {
    CREATED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// A single method of comparing GPU frames with the stored ones (R.29): ad-hoc epsilons in
/// tests are forbidden; only this helper is used.
#[derive(Clone, Copy)]
pub struct FrameTol {
    /// |Δ| ≤ max_lsb for “ordinary” pixels.
    pub max_lsb: u32,
    /// The share of pixels allowed to exceed max_lsb.
    pub outlier_frac: f64,
    /// A ceiling of |Δ| even for outliers (255 = no limit: threshold
    /// effects flip 0↔255 at the threshold edge).
    pub outlier_max: u32,
    /// Mean |Δ| over all channels.
    pub mean_lsb: f64,
}

impl FrameTol {
    pub const EXACT: FrameTol =
        FrameTol { max_lsb: 0, outlier_frac: 0.0, outlier_max: 0, mean_lsb: 0.0 };
    pub const BILINEAR: FrameTol =
        FrameTol { max_lsb: 2, outlier_frac: 0.001, outlier_max: 4, mean_lsb: 0.5 };
    pub const THRESHOLD: FrameTol =
        FrameTol { max_lsb: 2, outlier_frac: 0.005, outlier_max: 255, mean_lsb: 1.0 };
    /// Merged geometry series with Pan3d (R.44): inside the quad, bilinear precision;
    /// the SILHOUETTE (the quad boundaries, test w≤0) is decided by f32 predicates against f64.
    /// Measurement (lavapipe, t181 geo3d + t187 + t301): worst |Δ| = 1–2 LSB,
    /// share of |Δ|=1 up to 6.2 %, mean ≤ 0.07, NO outliers AT ALL: the predicates
    /// agreed on all pixels. The outlier budget is left for drivers with FMA
    /// contraction of predicates (cross-backend bit-exactness is unreachable,
    /// item 308): contour pixels there flip by the full amount.
    pub const GEOM: FrameTol =
        FrameTol { max_lsb: 2, outlier_frac: 0.002, outlier_max: 255, mean_lsb: 0.5 };
    /// A signal simulation (vhs-tape-simulation, t1186): f32 filters of hundreds of taps against
    /// the same f32 text, and an FM discriminator whose impulsive "clicks" at a carrier drowned in
    /// noise flip on one ULP. Measured (lavapipe, 2026-11-01, 192×108, every preset): a clean
    /// deck |Δ| ≤ 1, mean ≤ 0.002; the clogged head (field A is snow) 1.61 % of the samples above
    /// 2, up to 153, mean 0.324. Set at ×2 of the clogged head.
    pub const SIGNAL: FrameTol =
        FrameTol { max_lsb: 2, outlier_frac: 0.033, outlier_max: 255, mean_lsb: 0.65 };
}

/// A post-mortem of the divergence: where, how many, a histogram. It panics on
/// a tolerance violation.
pub fn assert_frames_close(cpu: &[u8], gpu: &[u8], w: u32, h: u32, tol: FrameTol, label: &str) {
    assert_eq!(cpu.len(), gpu.len(), "{label}: buffer sizes differ");
    assert_eq!(cpu.len(), (w * h * 4) as usize, "{label}: size {w}x{h}");
    let mut worst = 0u32;
    let mut worst_at = (0u32, 0u32, 0usize);
    let mut over = 0usize;
    let mut sum = 0u64;
    let mut hist = [0usize; 9]; // 0,1,2,3,4,5-8,9-16,17-64,65+
    for (i, (c, g)) in cpu.iter().zip(gpu.iter()).enumerate() {
        let d = (*c as i32 - *g as i32).unsigned_abs();
        sum += d as u64;
        let bucket = match d {
            0..=4 => d as usize,
            5..=8 => 5,
            9..=16 => 6,
            17..=64 => 7,
            _ => 8,
        };
        hist[bucket] += 1;
        if d > tol.max_lsb {
            over += 1;
        }
        if d > worst {
            worst = d;
            let px = i / 4;
            worst_at = (px as u32 % w, px as u32 / w, i % 4);
        }
    }
    let total = cpu.len();
    let mean = sum as f64 / total as f64;
    let frac = over as f64 / total as f64;
    let ok = worst <= tol.outlier_max.max(tol.max_lsb)
        && frac <= tol.outlier_frac
        && mean <= tol.mean_lsb;
    assert!(
        ok,
        "{label}: GPU diverged from CPU: worst |d|={worst} @({},{}) ch{}, \
         >|{}| at {:.4}% (allowed {:.4}%), mean {:.4} (allowed {}), hist {:?}",
        worst_at.0,
        worst_at.1,
        worst_at.2,
        tol.max_lsb,
        frac * 100.0,
        tol.outlier_frac * 100.0,
        mean,
        tol.mean_lsb,
        hist
    );
}

/// The mean of |a − b| over all bytes: how far two frames lie, whatever the outliers.
pub fn mean_lsb(a: &[u8], b: &[u8]) -> f64 {
    let sum: u64 = a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as u64).sum();
    sum as f64 / a.len().max(1) as f64
}

/// The export's GPU engine (export-throughput D5).
pub fn export_engine() -> Result<std::sync::Arc<GpuFxEngine>, String> {
    let ctx = GpuContext::new_headless_for_export().map_err(|e| e.to_string())?;
    GpuFxEngine::new_for_export(ctx).map_err(|e| e.to_string())
}

/// The engine of the work that is neither the preview nor an export (effect thumbnails, reel strips,
/// project posters), on its own device: it stays out of the preview's queue. Opened once, at the
/// first use. `None` — no device.
pub fn offline_engine() -> Option<std::sync::Arc<GpuFxEngine>> {
    OFFLINE.get_or_init(|| GpuContext::new_headless().and_then(GpuFxEngine::new).ok()).clone()
}

static OFFLINE: std::sync::OnceLock<Option<std::sync::Arc<GpuFxEngine>>> = std::sync::OnceLock::new();

/// The process is about to exit: wait for the offline engine's frame on the device and let no other
/// start. A frame on the device at exit kills the process: libLLVM's destructors run under
/// llvmpipe's threads (gpu-render-hardening, D6).
pub fn park_offline_engine() {
    if let Some(Some(e)) = OFFLINE.get() {
        e.park();
    }
}

/// The engines an export of `w`×`h` composes on (compute-kernels-render, D4): a card gets one; a
/// software adapter runs one frame per queue, so it gets a device per worker, as many as the cores
/// and the memory allow (`memory::engines_for`). `KT_VIDEO_ENGINES` overrides it.
pub fn export_engines(w: u32, h: u32) -> Result<Vec<std::sync::Arc<GpuFxEngine>>, String> {
    let first = export_engine()?;
    use kt_engine::VideoGpuExec as _;
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let fit = if first.info().software { memory::engines_for(cores, memory::available(), w, h) } else { 1 };
    let n = std::env::var("KT_VIDEO_ENGINES").ok().and_then(|v| v.parse().ok()).unwrap_or(fit);
    let mut engines = vec![first];
    let more: Vec<_> = std::thread::scope(|s| {
        let open: Vec<_> = (1..n).map(|_| s.spawn(export_engine)).collect();
        open.into_iter().map(|h| h.join().expect("engine thread")).collect()
    });
    for e in more {
        engines.push(e?);
    }
    Ok(engines)
}

/// Adapter for tests: skip with a printout if there is NO wgpu adapter; the caller installs the
/// built-in plugin set FIRST (`kt_plugins_builtin::install()`): point-wise plugins are assembled
/// into the passes when the engine is created (plugin-api-stage1, D5);
/// `KT_REQUIRE_GPU=1` turns this skip into a failure too (a copy of
/// KT_REQUIRE_CROSS_RENDER). lavapipe/llvmpipe is a full-fledged adapter.
///
/// An adapter IS there, but the engine did not build (shader/pipeline): this is ALWAYS
/// a failure, not a skip. A lesson of R.31: a broken Pan3d layout dropped the
/// creation of the engine as a whole, and the tests turned vacuously green via “skip without adapter”
/// and the defect reached the user’s machine.
pub fn test_engine() -> Option<std::sync::Arc<GpuFxEngine>> {
    let ctx = match GpuContext::new_headless() {
        Ok(c) => c,
        Err(err) => {
            if std::env::var("KT_REQUIRE_GPU").as_deref() == Ok("1") {
                panic!("KT_REQUIRE_GPU=1, but no wgpu adapter: {err}");
            }
            eprintln!("skipped: wgpu adapter unavailable ({err}); apt install mesa-vulkan-drivers");
            return None;
        }
    };
    let name = ctx.adapter_info.name.clone();
    match GpuFxEngine::new(ctx) {
        Ok(e) => Some(e),
        // the engine refuses this adapter on purpose (WARP): no GPU to test, as without one
        Err(err @ GpuInitError::Unsupported(_)) if std::env::var("KT_REQUIRE_GPU").as_deref() != Ok("1") => {
            eprintln!("skipped: {err}");
            None
        }
        Err(err) => {
            panic!("adapter '{name}' is present, but the GPU engine failed to build: {err}")
        }
    }
}
