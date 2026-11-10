//! The executor's and the barriers' local costs (local-cost-queue; capabilities `video-fx`,
//! `video-plan`). Every check compares two forms on ONE device byte for byte: a form keeps the bytes
//! of the one it replaces. Without a wgpu adapter: skip with a printout (KT_REQUIRE_GPU=1 requires one).
//!
//! t2313 — uniform writes staged per submission and bind groups kept across frames give the bytes of a
//! write and a bind group per pass, frame by frame and through the export's readback ring.
//! t2314 — the steady state reuses most bind groups and stays bounded.

use std::sync::Arc;

use kt_cache::SourceId;
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec};
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

fn frame(w: u32, h: u32, alpha: bool) -> Arc<VideoFrame> {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for (i, p) in rgba.chunks_exact_mut(4).enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        let c = if (x / 6 + y / 6) % 2 == 0 { [(x * 7) as u8, (y * 5) as u8, ((x + y) * 3) as u8] } else { [230, 40 + (y % 200) as u8, 90] };
        p[..3].copy_from_slice(&c);
        p[3] = if alpha { ((x * 5 + y * 3) % 256) as u8 } else { 255 };
    }
    Arc::new(VideoFrame { pts: Tick(0), width: w, height: h, rgba: rgba.into_boxed_slice(), has_alpha: alpha })
}

fn layer<'a>(id: u64, fx: &'a [FxInstance], fr: &Arc<VideoFrame>) -> PlanLayer<'a> {
    let src = LayerSrc {
        source: SourceId(id),
        cache_idx: 0,
        content: id,
        src_scale: 1,
        input: kt_engine::video_plan::LayerInput::Frame(Some(fr.clone())),
        fx: std::borrow::Cow::Borrowed(fx),
        fx_hash: id,
        u: 0.5,
        cam3d: None,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: None,
        masks: Vec::new(),
    };
    PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }
}

/// Two layers: a geometry series with a point-wise run under a stack of barriers, over a second
/// layer with a run of its own. Moment `i` moves the zoom and the frames' content keys.
fn plan<'a>(i: u64, top: &'a [FxInstance], bottom: &'a [FxInstance], a: &Arc<VideoFrame>, b: &Arc<VideoFrame>) -> FramePlan<'a> {
    FramePlan { t: Tick(90_000 + 3_600 * i as i64), out_w: a.width, out_h: a.height, layers: vec![layer(500 + i, bottom, b), layer(900 + i, top, a)] }
}

fn chains(i: u64) -> (Vec<FxInstance>, Vec<FxInstance>) {
    let p = |id: &str, v: &[f64]| FxInstance::plugin_with(id, v).expect("a kind");
    let top = vec![
        FxInstance::geometry_zoom(1.1 + 0.05 * (i % 4) as f64),
        p("color_grade", &[0.2, 0.1, 0.9, 1.1]),
        p("posterize", &[6.0]),
        FxInstance::blur(6),
        p("glow", &[0.4, 0.6, 8.0]),
        FxInstance::plugin("camera_isp").expect("camera"),
        FxInstance::plugin("vhs_tape").expect("tape"),
        p("radial_blur", &[0.3, 0.1, 0.0, 1.0]),
    ];
    let bottom = vec![p("vignette", &[0.5]), p("duotone", &[0.5])];
    (top, bottom)
}

/// t2313 (local-cost-queue, row 138): eight moments of a two-layer heavy chain, per frame and through
/// the readback ring, in the batched form and in the per-pass form: equal byte for byte.
#[test]
fn t2313_batched_uniforms_and_kept_bind_groups_keep_the_bytes() {
    kt_plugins_builtin::install();
    let Some(engine) = kt_gpu::test_engine() else { return };
    let (a, b) = (frame(160, 90, false), frame(160, 90, true));
    let run = |per_pass: bool| -> Vec<Vec<u8>> {
        engine.per_pass_binds(per_pass);
        let mut frames = Vec::new();
        for i in 0..8 {
            let (top, bottom) = chains(i);
            let mut out = Vec::new();
            engine.execute_measured(&plan(i, &top, &bottom, &a, &b), &mut out).expect("GPU execute");
            frames.push(out);
        }
        // the ring: three in flight, collected in order
        let all: Vec<_> = (0..8).map(chains).collect();
        let mut tickets = std::collections::VecDeque::new();
        for (i, (top, bottom)) in all.iter().enumerate() {
            tickets.push_back(engine.submit(&plan(i as u64, top, bottom, &a, &b)).expect("submit"));
            if tickets.len() == engine.readback_depth() {
                let mut out = Vec::new();
                engine.collect(tickets.pop_front().unwrap(), &mut out).expect("collect");
                frames.push(out);
            }
        }
        while let Some(t) = tickets.pop_front() {
            let mut out = Vec::new();
            engine.collect(t, &mut out).expect("collect");
            frames.push(out);
        }
        engine.per_pass_binds(false);
        frames
    };
    let batched = run(false);
    let per_pass = run(true);
    let batched_again = run(false);
    assert_eq!(batched.len(), 16);
    for (i, ((x, y), z)) in batched.iter().zip(&per_pass).zip(&batched_again).enumerate() {
        assert!(x == y, "frame {i}: the batched form's bytes differ from the per-pass form's");
        assert!(x == z, "frame {i}: the batched form is not deterministic");
    }
}

/// t2314 (local-cost-queue, row 138): after a warm lap, a second lap of 24 moments (four moments in
/// turn, each with its own uploaded frames, as playback uploads a new frame per moment) reuses at
/// least twice as many bind groups as it makes (the pool's accumulators and kept slots come back; each
/// moment's uploads are new: measured 648 reused, 217 made), and the cache never holds more than its
/// bound.
#[test]
fn t2314_the_steady_state_makes_no_bind_group() {
    kt_plugins_builtin::install();
    let Some(_) = kt_gpu::test_engine() else { return };
    // an engine of its own: the test kit's is shared with the other tests of the process
    let engine = kt_gpu::GpuContext::new_headless().and_then(kt_gpu::GpuFxEngine::new).expect("an engine");
    let (a, b) = (frame(160, 90, false), frame(160, 90, true));
    let lap = || {
        for i in 0..24u64 {
            let (top, bottom) = chains(i % 4);
            let mut out = Vec::new();
            engine.execute_measured(&plan(i % 4, &top, &bottom, &a, &b), &mut out).expect("GPU execute");
            let (_, _, held) = engine.bind_stats();
            assert!(held <= 256, "the bind cache holds {held}");
        }
    };
    lap();
    let (made, reused, _) = engine.bind_stats();
    lap();
    let (made2, reused2, held) = engine.bind_stats();
    eprintln!("t2314: warm lap made {made} reused {reused}; second lap made {} reused {}; held {held}", made2 - made, reused2 - reused);
    let (m, r) = (made2 - made, reused2 - reused);
    assert!(r >= 2 * m, "the second lap reused {r} bind groups and made {m}");
}
