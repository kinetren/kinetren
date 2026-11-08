//! t2020, t2022 (gpu-pass-cost; capabilities `telemetry`, `video-plan`): every pass of a frame is
//! timed on the device and summed per effect and pass; the record's passes add up to the frame's
//! device time; nothing is recorded while nothing composes. Its own test binary: the sum is
//! process-wide. Without a wgpu adapter: skip with a printout (KT_REQUIRE_GPU=1 requires one).

use std::sync::Arc;

use kt_cache::SourceId;
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec};
use kt_gpu::{GpuContext, pass_time, test_engine};
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

const W: u32 = 192;
const H: u32 = 108;

fn frame() -> Arc<VideoFrame> {
    let rgba: Vec<u8> = (0..W * H).flat_map(|i| [(i % W) as u8, (i / W) as u8, 128, 255]).collect();
    Arc::new(VideoFrame { pts: Tick(0), width: W, height: H, rgba: rgba.into_boxed_slice(), has_alpha: false })
}

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

fn plan<'a>(f: &Arc<VideoFrame>, fx: &'a [FxInstance], t: i64) -> FramePlan<'a> {
    FramePlan {
        t: Tick(t),
        out_w: W,
        out_h: H,
        layers: vec![
            PlanLayer::One { src: layer(f, &[], 1), alpha_k: 4096, scale_alpha: false, fade: None },
            PlanLayer::One { src: layer(f, fx, 2), alpha_k: 2048, scale_alpha: false, fade: None },
        ],
    }
}

fn defaults_of(id: &str) -> Vec<f64> {
    let reg = kt_plugin_api::installed().unwrap();
    reg.get(kt_plugin_api::Family::Fx, id).unwrap().manifest().passport.iter().map(|s| s.default).collect()
}

/// `name mean ×n` items of a record's line: Σ mean·n.
fn line_sum(line: &str) -> (f64, u32) {
    let mut sum = 0.0;
    let mut n = 0;
    for part in line.split(" | ").nth(3).unwrap_or("").split(", ") {
        let w: Vec<&str> = part.split_whitespace().collect();
        if let [.., mean, count] = w[..]
            && let (Ok(m), Some(Ok(c))) = (mean.parse::<f64>(), count.strip_prefix('×').map(str::parse::<u32>))
        {
            sum += m * c as f64;
            n += c;
        }
    }
    (sum, n)
}

#[test]
fn t2020_t2022_every_pass_is_timed_and_the_passes_sum_to_the_frame() {
    kt_plugins_builtin::install();
    let Some(engine) = test_engine() else { return };
    let stamps = GpuContext::new_headless().unwrap().device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
    let _ = pass_time::take();
    // t2022: an engine that composes nothing records nothing
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(pass_time::take(), None, "nothing composed, nothing recorded");

    let f = frame();
    let fx = vec![FxInstance::plugin_with("vhs_tape", &defaults_of("vhs_tape")).expect("vhs_tape")];
    let mut out = Vec::new();
    for t in 0..3 {
        assert!(engine.execute_once(&plan(&f, &fx, t * 3750), &mut out).expect("execute"));
    }
    // the export's ring: its frames come back with `collect`
    let ticket = engine.submit(&plan(&f, &fx, 4 * 3750)).expect("submit");
    assert!(engine.collect(ticket, &mut out).expect("collect"));

    let s = pass_time::take().expect("four frames composed");
    let line = s.line();
    eprintln!("t2020 GPU   {line}");
    assert_eq!((s.frames, s.offline, s.export), (4, 3, 1), "{line}");
    assert_eq!((s.out, s.decode, s.layers), ((W, H), (W, H), 2), "{line}");
    if !stamps {
        assert!(!s.timestamps && line.contains("no timestamps"), "{line}");
        eprintln!("t2020: this adapter has no timestamps; only the submit-to-done time is checked");
        return;
    }
    assert!(s.timestamps, "{line}");
    // the passes add up to the frames' device time, exactly in nanoseconds
    let total: u64 = s.passes.iter().map(|p| p.1).sum();
    assert_eq!(total, s.frame_ns, "{line}");
    assert!(s.span_ns >= s.frame_ns / 2 && s.frame_ns > 0, "{line}");
    let count = |name: &str| s.passes.iter().find(|p| p.0 == name).map_or(0, |p| p.2);
    // every frame ran the tape's head read and two composite passes; the tape's resample to its
    // grid is named after the tape
    assert_eq!(count("vhs_tape:s5-head-read"), 4, "{line}");
    assert_eq!(count("composite"), 8, "{line}");
    assert!(count("vhs_tape:resample") >= 4, "{line}");
    assert!(s.passes.iter().all(|p| !p.0.contains(":p")), "every tape pass has its name: {line}");
    // the line says the same: Σ mean × count = frames × the frame's mean, within the rounding
    let (sum, n) = line_sum(&line);
    let passes: u32 = s.passes.iter().map(|p| p.2).sum();
    assert_eq!(n, passes, "{line}");
    let frame_ms = s.frame_ns as f64 / 1e6;
    assert!((sum - frame_ms).abs() <= 0.0005 * n as f64 + 1e-9, "Σ {sum} ms against {frame_ms} ms: {line}");
    // t2022: once taken, and nothing composes, the next second has nothing
    assert_eq!(pass_time::take(), None);

    // a barrier of host nodes alone (blur: a mip down and up) names them after itself, not after
    // the barrier whose pipes follow it
    let blur = vec![FxInstance::plugin_with("blur", &defaults_of("blur")).expect("blur")];
    assert!(engine.execute_once(&plan(&f, &blur, 8 * 3750), &mut out).expect("execute"));
    let s = pass_time::take().expect("a blurred frame");
    let line = s.line();
    assert!(s.passes.iter().any(|p| p.0 == "blur:resample" && p.2 == 2), "{line}");
    assert!(s.passes.iter().all(|p| !p.0.starts_with("glow")), "{line}");

    // a device without timestamps (the interface's GL device, say): the frame's submit-to-done time
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).expect("adapter");
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits { max_texture_dimension_2d: 8192, ..wgpu::Limits::default() },
        ..Default::default()
    }))
    .expect("device");
    let bare = kt_gpu::GpuFxEngine::new(GpuContext::from_parts(Arc::new(device), Arc::new(queue), info)).expect("engine");
    assert!(bare.execute_once(&plan(&f, &fx, 9 * 3750), &mut out).expect("execute"));
    let s = pass_time::take().expect("one frame");
    let line = s.line();
    eprintln!("t2020 no timestamps: GPU   {line}");
    assert!(!s.timestamps && s.frames == 1 && s.frame_ns > 0 && s.passes.is_empty(), "{line}");
    assert!(line.contains("from submit to done (no timestamps on this device)"), "{line}");
}
