//! The GPU backend of video FX and compositing (R.29): wgpu f32/integer
//! fragment passes on top of the eframe device (preview) or headless
//! (export/kt-cli). Implements kt_engine::VideoGpuExec; kt-engine does NOT depend on
//! this crate: the executor is injected from above (kt-app,
//! kt-cli). The CPU path of kt-fx/kt-engine remains the byte reference;
//! GPU-vs-CPU conformance is tests/parity.rs (tolerances, t181/t182/t183).

mod context;
mod engine;
mod pool;

pub use context::{GpuContext, GpuInitError};
pub use engine::GpuFxEngine;

/// A single method of comparing GPU-vs-CPU frames (R.29): ad-hoc epsilons in
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

/// Adapter for tests: skip with a printout if there is NO wgpu adapter;
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
        Err(err) => {
            panic!("adapter '{name}' is present, but the GPU engine failed to build: {err}")
        }
    }
}
