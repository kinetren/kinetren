//! t2125 (pipeline-prewarm D7; capability `video-plan`): the disk tier of compiled pipelines is speed
//! only. Three engines on three devices compose the same frames of every video effect: one with no
//! cache folder, one that writes the cache, one that reads it back — the bytes are the same. Where the
//! device has `PIPELINE_CACHE`, the file is written under the folder, named by its key. Its own test
//! binary: the folder is set once per process. Without a wgpu adapter: skip with a printout.

use std::sync::Arc;

use kt_cache::SourceId;
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec};
use kt_gpu::{GpuContext, GpuFxEngine};
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

const W: u32 = 320;
const H: u32 = 180;

fn compose(engine: &GpuFxEngine, frame: &Arc<VideoFrame>, cases: &[Vec<FxInstance>]) -> Vec<Vec<u8>> {
    cases
        .iter()
        .enumerate()
        .map(|(i, fx)| {
            let src = LayerSrc {
                source: SourceId(5),
                cache_idx: 0,
                content: 5,
                src_scale: 1,
                input: kt_engine::video_plan::LayerInput::Frame(Some(frame.clone())),
                fx: std::borrow::Cow::Borrowed(fx),
                fx_hash: i as u64,
                u: 0.5,
                cam3d: None,
                blend: None,
                blend_mode: kt_model::BlendMode::Normal,
                lead: None,
                masks: Vec::new(),
            };
            let plan = FramePlan {
                t: Tick(i as i64 * 3600),
                out_w: W,
                out_h: H,
                layers: vec![PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }],
            };
            let mut out = Vec::new();
            engine.execute_once(&plan, &mut out).expect("execute");
            out
        })
        .collect()
}

#[test]
fn t2125_the_disk_tier_changes_no_byte() {
    kt_plugins_builtin::install();
    let Some(plain) = GpuContext::new_headless().ok().and_then(|c| GpuFxEngine::new(c).ok()) else {
        eprintln!("skipped: no wgpu adapter");
        return;
    };
    let reg = kt_plugin_api::installed().unwrap();
    let cases: Vec<Vec<FxInstance>> = reg
        .plugins()
        .filter(|p| p.manifest().media == kt_plugin_api::Media::VideoFx && p.manifest().decl.video_class != Some(kt_plugin_api::VideoClass::Stream))
        .filter_map(|p| {
            let m = p.manifest();
            FxInstance::plugin_with(&m.id, &m.passport.iter().map(|s| s.default).collect::<Vec<_>>()).map(|f| vec![f])
        })
        .collect();
    let ids: Vec<String> = cases.iter().map(|c| c[0].tag().to_string()).collect();
    let rgba: Vec<u8> = (0..W * H).flat_map(|i| [(i % W) as u8, (i / W) as u8, (i * 11 % 251) as u8, 255]).collect();
    let frame = Arc::new(VideoFrame { pts: Tick(0), width: W, height: H, rgba: rgba.into_boxed_slice(), has_alpha: false });
    let reference = compose(&plain, &frame, &cases);
    drop(plain);

    let dir = std::env::temp_dir().join(format!("kt-t2125-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    kt_gpu::set_cache_dir(dir.clone());
    let ctx = GpuContext::new_headless().unwrap();
    let has_cache = ctx.device.features().contains(wgpu::Features::PIPELINE_CACHE);
    let writer = GpuFxEngine::new(ctx).expect("engine");
    writer.prewarm(&ids);
    writer.settle_prewarm();
    assert!(compose(&writer, &frame, &cases) == reference, "frames moved with the cache being written");
    drop(writer);
    let files: Vec<_> = std::fs::read_dir(&dir).map(|r| r.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
    if has_cache {
        assert!(files.iter().any(|f| f.to_string_lossy().starts_with("engine-")), "no cache file: {files:?}");
    } else {
        assert!(files.is_empty(), "a cache written for a device without the feature: {files:?}");
        eprintln!("t2125: this device has no PIPELINE_CACHE; only the bytes are checked");
    }
    let reader = GpuFxEngine::new(GpuContext::new_headless().unwrap()).expect("engine");
    assert!(compose(&reader, &frame, &cases) == reference, "frames moved with the cache read back");
    let _ = std::fs::remove_dir_all(&dir);
}
