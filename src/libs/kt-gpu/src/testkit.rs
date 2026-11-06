//! What the tests of other crates compose through (compute-kernels-render, D5): one engine per test
//! process, opened at the first guard. A test starts with `if no_gpu(install) { return; }`: without a
//! device it skips with a printout (`KT_REQUIRE_GPU=1` makes that a failure).

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock};

use kt_cache::BurstCache;
use kt_cache::SourceId;
use kt_engine::export::{ExportError, ExportEvent, ExportOptions, ExportProgress, ExportStats, render_raw_video_range_with};
use kt_engine::video_plan::LayerInput;
use kt_fx::video::{Frame, VideoFxCtx};
use kt_model::FxInstance;
use kt_engine::{ExportBackends, FramePlan, LayerSrc, PlanLayer, TransMix, VideoGpuExec, VideoRenderer};
use kt_media::{MediaSource, Quality, VideoFrame};
use kt_model::{MediaId, Project};
use kt_time::Tick;

use crate::GpuFxEngine;

static ENGINE: OnceLock<Option<Arc<GpuFxEngine>>> = OnceLock::new();

/// Install the plugins (the engine assembles their shaders when it is built) and open the engine.
pub fn no_gpu<R>(install: impl FnOnce() -> R) -> bool {
    install();
    ENGINE.get_or_init(crate::test_engine).is_none()
}

pub fn engine() -> Arc<GpuFxEngine> {
    ENGINE.get().and_then(Clone::clone).expect("a test that composes starts with `no_gpu`")
}

/// A committed hash of video bytes holds on the software adapter it was taken on; a card is held to
/// the stored frames within tolerances instead.
pub fn hash_holds() -> bool {
    engine().info().software
}

/// An export's backends: an engine of its own, as in the product. Parallel tests would fill one
/// engine's readback ring.
pub fn backends() -> ExportBackends {
    let own = crate::test_engine().expect("a test that exports starts with `no_gpu`");
    ExportBackends { video_gpu: Some(own as Arc<dyn VideoGpuExec>), ..Default::default() }
}

/// `backends` with a device per frame worker, the path of a software adapter.
pub fn backends_many(n: usize) -> ExportBackends {
    let more = (1..n).map(|_| crate::test_engine().expect("a test that exports starts with `no_gpu`") as Arc<dyn VideoGpuExec>);
    ExportBackends { video_gpu_more: more.collect(), ..backends() }
}

/// What the last frame of the shared engine ran.
pub fn last_cost() -> kt_fx::video::FrameCost {
    engine().last_cost()
}

pub fn compose(video: &VideoRenderer, cache: &BurstCache, t: Tick, q: Quality) -> (Vec<u8>, bool) {
    let mut out = Vec::new();
    let complete = compose_into(video, cache, t, q, &mut out);
    (out, complete)
}

pub fn compose_into(video: &VideoRenderer, cache: &BurstCache, t: Tick, q: Quality, out: &mut Vec<u8>) -> bool {
    video.compose_on(&*engine(), cache, t, q, out).expect("GPU execute")
}

/// The export's video stream of the whole project.
pub fn render(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    quality: Quality,
    emit: impl FnMut(&[u8]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    render_range(project, sources, quality, None, &Arc::new(AtomicBool::new(false)), |_| {}, emit)
}

pub fn render_range(
    project: &Project,
    sources: Vec<(MediaId, Box<dyn MediaSource>)>,
    quality: Quality,
    range: Option<(Tick, Tick)>,
    cancel: &Arc<AtomicBool>,
    progress: impl FnMut(u64),
    emit: impl FnMut(&[u8]) -> std::io::Result<()>,
) -> std::io::Result<ExportStats> {
    render_raw_video_range_with(project, sources, quality, range, &backends(), cancel, progress, emit)
}

/// The MP4 export on the test engine, reporting progress alone.
#[allow(clippy::too_many_arguments)]
pub fn export_with(
    project: &Project,
    audio: Vec<(MediaId, Box<dyn MediaSource>)>,
    video: Vec<(MediaId, Box<dyn MediaSource>)>,
    out: &str,
    range: Option<(Tick, Tick)>,
    opts: &ExportOptions,
    cancel: &Arc<AtomicBool>,
    pause: &Arc<AtomicBool>,
    progress: impl Fn(ExportProgress) + Send + Sync + 'static,
) -> Result<ExportStats, ExportError> {
    kt_engine::export_mp4_with_backends(project, audio, video, out, range, opts, &backends(), cancel, pause, move |e| {
        if let ExportEvent::Progress(p) = e {
            progress(p);
        }
    })
}

/// A transition of two frames of one size at progress `k` (0..4096), through the composite.
pub fn mix(dst: &mut Vec<u8>, a: &VideoFrame, b: &VideoFrame, k: u32, m: TransMix) {
    let side = |f: &VideoFrame, id: u64| LayerSrc {
        source: SourceId(id),
        cache_idx: 0,
        content: xxhash_rust::xxh3::xxh3_64(&f.rgba),
        src_scale: 1,
        input: LayerInput::Frame(Some(Arc::new(f.clone()))),
        fx: std::borrow::Cow::Borrowed(&[]),
        fx_hash: 0,
        u: 0.5,
        cam3d: None,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: None,
        masks: Vec::new(),
    };
    let layers = vec![PlanLayer::Dissolve { a: side(a, 1), b: side(b, 2), k, alpha_k: 4096, mix: m }];
    let plan = FramePlan { t: Tick(0), out_w: a.width, out_h: a.height, layers };
    engine().execute_into(&plan, dst).expect("GPU execute");
}

/// A chain over one frame, through the composite.
pub fn chain(fx: &[FxInstance], frame: Frame, ctx: &VideoFxCtx) -> Frame {
    let hash = xxhash_rust::xxh3::xxh3_64(format!("{fx:?}").as_bytes());
    let content = xxhash_rust::xxh3::xxh3_64(&frame.rgba);
    let src = LayerSrc {
        source: SourceId(3),
        cache_idx: 0,
        content,
        src_scale: ctx.in_scale,
        input: LayerInput::Frame(Some(Arc::new(VideoFrame {
            pts: Tick(0),
            width: frame.w,
            height: frame.h,
            rgba: frame.rgba.into_boxed_slice(),
            has_alpha: frame.has_alpha,
        }))),
        fx: std::borrow::Cow::Borrowed(fx),
        fx_hash: hash,
        u: ctx.u,
        cam3d: ctx.cam3d,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: ctx.lead,
        masks: Vec::new(),
    };
    let layers = vec![PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }];
    let plan = FramePlan { t: Tick(ctx.burst_index), out_w: ctx.out_w, out_h: ctx.out_h, layers };
    let mut rgba = Vec::new();
    engine().execute_into(&plan, &mut rgba).expect("GPU execute");
    let has_alpha = rgba.chunks_exact(4).any(|p| p[3] != 255);
    Frame { w: ctx.out_w, h: ctx.out_h, rgba, has_alpha }
}
