//! t2128 (pipeline-prewarm D2b; capability `video-plan`): on the GL backend (the live check's
//! `WGPU_BACKEND=gl`) a prewarmed software engine keeps composing on the caller's thread, and another
//! engine opens after it. The first version opened a shadow device on GL beside the watched one; on a
//! tree where GL devices still shared one EGL context (before segv-crashes' `own_instance`) the next
//! frame panicked with `BadAccess`, and the live render scenarios went red. Its own test binary: the
//! backend is chosen once per process. No GL adapter: skip.

use std::sync::Arc;

use kt_cache::SourceId;
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec};
use kt_gpu::{GpuContext, GpuFxEngine};
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

#[test]
fn t2128_a_prewarmed_gl_engine_keeps_composing() {
    // SAFETY: the only test of this binary, before any thread or wgpu instance exists
    unsafe { std::env::set_var("WGPU_BACKEND", "gl") };
    kt_plugins_builtin::install();
    let Ok(ctx) = GpuContext::new_headless() else {
        eprintln!("skipped: no GL adapter");
        return;
    };
    if ctx.adapter_info.backend != wgpu::Backend::Gl {
        eprintln!("skipped: the adapter is {:?}, not GL", ctx.adapter_info.backend);
        return;
    }
    let engine = GpuFxEngine::new(ctx).expect("engine");
    let reg = kt_plugin_api::installed().unwrap();
    let ids: Vec<String> = reg.plugins().map(|p| p.manifest().id.to_string()).collect();
    engine.prewarm(&ids);
    engine.settle_prewarm();
    let (w, h) = (64u32, 36u32);
    let rgba: Vec<u8> = (0..w * h).flat_map(|i| [i as u8, (i / w) as u8, 99, 255]).collect();
    let frame = Arc::new(VideoFrame { pts: Tick(0), width: w, height: h, rgba: rgba.into_boxed_slice(), has_alpha: false });
    let fx = vec![FxInstance::plugin("blur").unwrap()];
    let src = LayerSrc {
        source: SourceId(3),
        cache_idx: 0,
        content: 3,
        src_scale: 1,
        input: kt_engine::video_plan::LayerInput::Frame(Some(frame)),
        fx: std::borrow::Cow::Borrowed(&fx),
        fx_hash: 3,
        u: 0.5,
        cam3d: None,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: None,
        masks: Vec::new(),
    };
    let plan = FramePlan { t: Tick(0), out_w: w, out_h: h, layers: vec![PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }] };
    let mut out = Vec::new();
    assert!(engine.execute_once(&plan, &mut out).expect("a frame after the prewarm"));
    assert_eq!(kt_gpu::cold_draws(), 0, "a frame drew a pipeline the prewarm left cold");
    let other = GpuFxEngine::new(GpuContext::new_headless().expect("a second device")).expect("a second engine");
    assert!(other.execute_once(&plan, &mut out).expect("a frame on the second engine"));
}
