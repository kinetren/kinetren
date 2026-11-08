//! t2023 (gpu-pass-cost; capabilities `video-plan`, `video-fx`): after warm-up a frame creates no
//! texture, buffer or pipeline — every video effect of the registry at its defaults and at each of
//! its presets, and the heavy stack of the owner's project (VHS Tape and Camera ISP over two more
//! layers), through the readback path and the preview's present path, at 1080p. Its own test
//! binary: the counter is process-wide. Without a wgpu adapter: skip with a printout.

use std::sync::Arc;

use kt_cache::SourceId;
use kt_engine::{FramePlan, LayerSrc, PlanLayer, PresentOptions, VideoGpuExec};
use kt_gpu::{resources_created, test_engine};
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

const W: u32 = 1920;
const H: u32 = 1080;

fn layer<'a>(f: &Arc<VideoFrame>, fx: &'a [FxInstance], id: u64) -> LayerSrc<'a> {
    LayerSrc {
        source: SourceId(id),
        cache_idx: 0,
        content: id,
        src_scale: 1,
        input: kt_engine::video_plan::LayerInput::Frame(Some(f.clone())),
        fx: std::borrow::Cow::Borrowed(fx),
        fx_hash: id,
        u: 0.5,
        cam3d: None,
        blend: None,
        blend_mode: kt_model::BlendMode::Normal,
        lead: None,
        masks: Vec::new(),
    }
}

fn plan<'a>(frames: &[Arc<VideoFrame>], fx: &'a [FxInstance], t: i64) -> FramePlan<'a> {
    let layers = frames
        .iter()
        .enumerate()
        .map(|(i, f)| PlanLayer::One {
            src: layer(f, if i + 1 == frames.len() { fx } else { &[] }, 10 + i as u64),
            alpha_k: if i == 0 { 4096 } else { 3000 },
            scale_alpha: false,
            fade: None,
        })
        .collect();
    FramePlan { t: Tick(t), out_w: W, out_h: H, layers }
}

fn instance(id: &str, sets: &[(&str, f64)]) -> FxInstance {
    let reg = kt_plugin_api::installed().unwrap();
    let m = reg.get(kt_plugin_api::Family::Fx, id).unwrap().manifest();
    let mut v: Vec<f64> = m.passport.iter().map(|s| s.default).collect();
    for (n, x) in sets {
        v[m.passport.iter().position(|s| s.name == *n).unwrap()] = *x;
    }
    FxInstance::plugin_with(id, &v).unwrap()
}

#[test]
fn t2023_no_resource_is_created_per_frame_after_warm_up() {
    kt_plugins_builtin::install();
    let Some(engine) = test_engine() else { return };
    let frame = |alpha: bool, seed: u32| {
        let rgba: Vec<u8> = (0..W * H)
            .flat_map(|i| [(i % W) as u8 ^ seed as u8, (i / W) as u8, (i * 7 % 251) as u8, if alpha { (i % 200) as u8 + 55 } else { 255 }])
            .collect();
        Arc::new(VideoFrame { pts: Tick(0), width: W, height: H, rgba: rgba.into_boxed_slice(), has_alpha: alpha })
    };
    let (a, b, c) = (frame(false, 1), frame(false, 2), frame(true, 3));
    let reg = kt_plugin_api::installed().unwrap();
    let mut cases: Vec<(String, Vec<FxInstance>)> = Vec::new();
    for p in reg.plugins().filter(|p| p.manifest().media == kt_plugin_api::Media::VideoFx) {
        let m = p.manifest();
        cases.push((m.id.to_string(), vec![instance(&m.id, &[])]));
        for v in m.ui.variants {
            cases.push((format!("{} {}", m.id, v.name_key), vec![instance(&m.id, v.sets)]));
        }
    }
    cases.push((
        "the heavy stack".into(),
        vec![instance("vhs_tape", &[]), instance("camera_isp", &[]), instance("glow", &[]), instance("radial_blur", &[])],
    ));
    let mut t = 0;
    let mut out = Vec::new();
    for (name, fx) in &cases {
        let frames = [a.clone(), b.clone(), c.clone()];
        let mut execute = |t: i64| engine.execute_once(&plan(&frames, fx, t), &mut out).expect("execute");
        let present = |t: i64| engine.present(&plan(&frames, fx, t), PresentOptions { show_incomplete: true, glow: true }).expect("present");
        // warm-up: the pipelines of new shapes, the pool's textures, the arena, the timer's buffers
        for _ in 0..3 {
            t += 3600;
            execute(t);
        }
        let before = resources_created();
        for _ in 0..3 {
            t += 3600;
            execute(t);
        }
        let made = resources_created() - before;
        assert_eq!(made, 0, "{name}: {made} textures, buffers or pipelines created in three warm frames read back");
        // the preview keeps each new complete composite (cached-range-plays) until its budget is
        // full, then recycles the evicted ones: one texture a new frame at most, nothing else; its
        // composites come out of the pool the readback path shares
        for _ in 0..2 {
            t += 3600;
            present(t);
        }
        let before = resources_created();
        for _ in 0..3 {
            t += 3600;
            present(t);
        }
        let made = resources_created() - before;
        assert!(made <= 3, "{name}: {made} created in three warm presented frames, more than the memo keeps");
    }
    eprintln!("t2023: {} cases, nothing created after warm-up", cases.len());
}
