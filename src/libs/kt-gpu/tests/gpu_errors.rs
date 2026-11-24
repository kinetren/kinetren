//! gpu-errors-survive (kkt-26.11.15 N1): a wgpu error never ends the program. The owner's editor died
//! in wgpu's default error handler — a `Surface::configure` refused while the preview's thread
//! composed on the same device — because no device of ours had an uncaptured-error handler and no
//! frame ran inside an error scope. Skipped without an adapter (`KT_REQUIRE_GPU=1` fails instead).

use kt_engine::{FramePlan, GpuExecError, VideoGpuExec};
use kt_time::Tick;

/// t2720: an error outside any scope, on a device we opened, is counted and written, and the
/// process goes on; the device still works after it.
#[test]
fn t2720_an_uncaptured_error_is_written_not_a_panic() {
    let Ok(ctx) = kt_gpu::GpuContext::new_headless() else {
        assert_ne!(std::env::var("KT_REQUIRE_GPU").as_deref(), Ok("1"), "no adapter");
        eprintln!("skipped: no adapter");
        return;
    };
    let role = ctx_role(&ctx);
    let before = kt_gpu::errors::errors_seen(role);
    // MAP_READ with MAP_WRITE is refused by validation (no MAPPABLE_PRIMARY_BUFFERS)
    let _bad = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("t2720 refused"),
        size: 256,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::MAP_WRITE,
        mapped_at_creation: false,
    });
    let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
    assert_eq!(kt_gpu::errors::errors_seen(role), before + 1, "the error was not seen by our handler");
    // the device still serves
    let ok = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("t2720 fine"),
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    ctx.queue.write_buffer(&ok, 0, &[1u8; 256]);
    ctx.queue.submit([]);
    let _ = ctx.device.poll(wgpu::PollType::wait_indefinitely());
    assert_eq!(kt_gpu::errors::errors_seen(role), before + 1);
}

fn ctx_role(_ctx: &kt_gpu::GpuContext) -> &'static str {
    kt_gpu::errors::role::HEADLESS
}

/// t2721: a frame whose own work raises a wgpu error returns `GpuExecError::Validation` (the
/// poisoning contract) and is not counted as uncaptured: a layer whose frame says 64×64 and carries
/// the bytes of 4×4 overruns its upload. Before, the default handler panicked inside the frame.
#[test]
fn t2721_a_frames_own_error_is_its_error() {
    kt_plugins_builtin::install();
    let Some(engine) = kt_gpu::test_engine() else { return };
    let before = kt_gpu::errors::errors_seen(kt_gpu::errors::role::HEADLESS);
    let mut out = Vec::new();
    match engine.execute_into(&plan(64, 64, 4 * 4 * 4, 1), &mut out) {
        Err(GpuExecError::Validation(text)) => assert!(!text.is_empty()),
        other => panic!("an overrunning upload is a validation error of the frame: {other:?}"),
    }
    assert_eq!(kt_gpu::errors::errors_seen(kt_gpu::errors::role::HEADLESS), before, "the frame's scope caught it");
    // the engine composes the next frame
    assert!(engine.execute_into(&plan(16, 8, 16 * 8 * 4, 2), &mut out).is_ok());
    assert_eq!(out.len(), 16 * 8 * 4);
}

fn plan(w: u32, h: u32, bytes: usize, id: u64) -> FramePlan<'static> {
    let fr = std::sync::Arc::new(kt_media::VideoFrame {
        pts: Tick(0),
        width: w,
        height: h,
        rgba: vec![200u8; bytes].into_boxed_slice(),
        has_alpha: false,
    });
    let src = kt_engine::LayerSrc {
        source: kt_cache::SourceId(id),
        cache_idx: 0,
        content: id,
        src_scale: 1,
        input: kt_engine::video_plan::LayerInput::Frame(Some(fr)),
        fx: kt_engine::LayerChain::Owned(Vec::new()),
        fx_hash: id,
        u: 0.5,
        cam3d: None,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: None,
        masks: Vec::new(),
        fx_in: &[],
        reads: Vec::new(),
    };
    FramePlan { t: Tick(90_000), out_w: w, out_h: h, layers: vec![kt_engine::PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }] }
}
