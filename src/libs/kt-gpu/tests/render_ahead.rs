//! t2922 (render-ahead D3; capability `video-plan`): a moment the render-ahead producer keeps on the
//! display's own device is the display's own composite — no readback, the same key and bytes as the
//! bytes route and as the display's `present`.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use kt_engine::{PresentOptions, VideoGpuExec, VideoRenderer};
use kt_gpu::test_engine;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;

fn project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 128,
        height: 72,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    for id in [1u64, 2] {
        p.media.push(MediaRef::new(MediaId(id), "synth:ramp"));
    }
    let sec = 90_000i64;
    let clip = |id: u64, lane: u32, start: i64, dur: i64, media: u64, opacity: f32| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![
            kt_model::FxInstance::plugin_with("mirror", &[1.0, 0.0]).unwrap(),
            FxInstance::glow(1.0),
            FxInstance::vhs(2, 0.4),
        ],
        opacity,
        fx_off: false,
    };
    p.clips = vec![
        clip(1, 0, 0, 2 * sec, 1, 1.0),
        clip(2, 1, sec / 2, sec, 2, 0.75), // the top layer with a level
    ];
    p.normalize(); // tracks for the content (R.62)
    p.validate().unwrap();
    p
}

fn sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    [1u64, 2]
        .iter()
        .map(|&id| {
            (
                MediaId(id),
                Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp))
                    as Box<dyn MediaSource>,
            )
        })
        .collect()
}

/// The display engine's kept composite of `plan`, read back from its memo (a replay, no composition).
fn kept_bytes(display: &kt_gpu::GpuFxEngine, plan: &kt_engine::FramePlan<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    assert!(display.execute_into(plan, &mut out).unwrap());
    assert!(display.last_stages().replayed, "the composite was not read from the display's memo");
    out
}

#[test]
fn t2922_a_moment_kept_on_the_device_is_the_displays_own_composite() {
    kt_plugins_builtin::install();
    let (Some(a), Some(b), Some(c), Some(other)) = (test_engine(), test_engine(), test_engine(), test_engine()) else { return };
    let q = Quality { width: 128, height: 72 };
    let p = project();
    let video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let t = Tick(17 * 3000);
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> = sources().into_iter().collect();
    for (media, src_pts, dq) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(kt_engine::source_id(media), Some(dq), kt_engine::video::video_cache_idx(src_pts), frame.into());
    }
    let plan = video.frame_plan(&cache, t, q);
    for e in [&a, &b, &c] {
        e.keep_composites_for_bytes();
    }
    // on the device: a sibling of the display keeps into its memo and returns before the device ends
    let sib = a.sibling();
    let keeper_a: Arc<dyn VideoGpuExec> = a.clone();
    let mut buf = Vec::new();
    let on = sib.ahead(&plan, &keeper_a, &mut buf).unwrap();
    assert!(on.complete);
    let done = on.done.clone().expect("the on-device route hands back a done flag");
    assert!(buf.is_empty(), "the on-device route read the frame back");
    let stages = sib.last_stages();
    assert_eq!((stages.wait_ns, stages.copy_ns), (0, 0), "the on-device route waited or copied");
    let deadline = kt_time::test_deadline();
    while !done.load(Ordering::Acquire) {
        assert!(std::time::Instant::now() < deadline, "the device never ended the moment");
        sib.poll_done();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    // by bytes: an engine of another device composes, the keeper uploads
    let keeper_b: Arc<dyn VideoGpuExec> = b.clone();
    let by = other.ahead(&plan, &keeper_b, &mut buf).unwrap();
    assert!(by.done.is_none(), "the bytes route is synchronous");
    // the display's own present
    assert!(c.present(&plan, PresentOptions::default()).unwrap());
    let key = c.present_key(&plan);
    assert_eq!(on.kept, key, "the on-device route kept under another key");
    assert_eq!(by.kept, key, "the bytes route kept under another key");
    let (ka, kb, kc) = (kept_bytes(&a, &plan), kept_bytes(&b, &plan), kept_bytes(&c, &plan));
    assert_eq!(ka.len(), (q.width * q.height * 4) as usize);
    assert!(ka == kc, "the on-device composite differs from the display's own");
    assert!(kb == kc, "the bytes route's composite differs from the display's own");
}

/// Test t2929 (render-ahead fix, found-defects 728): the producer's engine reports the device time of
/// the moments it composes ahead — the render window predicts a render from it. Its frames are the
/// preview's; read as an offline engine's, the time was 0 and the window said "not measured" after
/// the range had played.
#[test]
fn t2929_the_producers_engine_reports_its_device_time() {
    kt_plugins_builtin::install();
    let Some(a) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let p = project();
    let video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> = sources().into_iter().collect();
    let moments: Vec<Tick> = (10..16).map(|i| Tick(i * 3000)).collect();
    for &t in &moments {
        for (media, src_pts, dq) in video.layers_at(t, q) {
            let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
            cache.insert_video(kt_engine::source_id(media), Some(dq), kt_engine::video::video_cache_idx(src_pts), frame.into());
        }
    }
    let sib = a.sibling();
    let keeper: Arc<dyn VideoGpuExec> = a.clone();
    let mut buf = Vec::new();
    let deadline = kt_time::test_deadline();
    let mut i = 0;
    // a frame's time is harvested at a later frame's start: compose until one came back
    while sib.last_device_ns() == 0 {
        assert!(std::time::Instant::now() < deadline, "the producer's engine never reported a device time");
        let plan = video.frame_plan(&cache, moments[i % moments.len()], q);
        let _ = sib.ahead(&plan, &keeper, &mut buf).unwrap();
        sib.poll_done();
        std::thread::sleep(std::time::Duration::from_millis(5));
        i += 1;
    }
}
