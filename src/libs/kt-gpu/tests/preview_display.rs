//! preview-throughput D2: the preview's display target. t1525–t1527 hold it to the bytes the
//! readback path shows: the picture must not change when the readback goes.

use kt_cache::{BurstCache, CacheConfig};
use kt_engine::{FramePlan, PresentOptions, VideoGpuExec, VideoRenderer};
use kt_media::{MediaSource, Quality};
use kt_model::demo::{DemoSources, demo_project};
use kt_time::Tick;

const Q: Quality = Quality { width: 320, height: 180 };

fn warmed(p: &kt_model::Project, ts: &[Tick]) -> (VideoRenderer, BurstCache) {
    let cache = BurstCache::new(CacheConfig { max_audio_bytes: 1, max_video_bytes: 256 << 20 });
    let video = VideoRenderer::new(p);
    let mut sources: std::collections::HashMap<_, Box<dyn MediaSource>> = p
        .media
        .iter()
        .map(|m| (m.id, kt_media::factory::open_pseudo(&m.path).unwrap().unwrap()))
        .collect();
    for (a, st) in kt_engine::gen_sources(p) {
        sources.insert(a, kt_media::factory::open_gen(&st));
    }
    for (media, pts, dq) in ts.iter().flat_map(|t| video.layers_at(*t, Q)) {
        if let Some(s) = sources.get_mut(&media) {
            let f = s.read_video_frame(pts, Some(dq)).unwrap();
            cache.insert_video(
                kt_engine::source_id(media),
                Some(dq),
                kt_engine::video_cache_idx(pts),
                f.into(),
            );
        }
    }
    (video, cache)
}

/// What `Color32::from_rgba_unmultiplied` makes of the readback's straight bytes.
fn premultiplied(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|p| {
            let pm = |v: u8| ((v as f32 * p[3] as f32 / 255.0) + 0.5).floor() as u8;
            [pm(p[0]), pm(p[1]), pm(p[2]), p[3]]
        })
        .collect()
}

fn plan_at<'a>(v: &'a VideoRenderer, c: &'a BurstCache, t: Tick) -> FramePlan<'a> {
    v.frame_plan(c, t, Q)
}

/// t1525: the display target holds exactly what the readback path uploads, moment by moment,
/// across a transition and over the demo's effects.
#[test]
fn t1525_the_display_shows_the_readback_picture() {
    kt_plugins_builtin::install();
    let Some(gpu) = kt_gpu::test_engine() else { return };
    let mut p = demo_project(DemoSources::Synth);
    // half-transparent events: an opaque frame cannot see the premultiplication
    for c in &mut p.clips {
        c.opacity = 0.5;
    }
    let ts: Vec<Tick> = (0..8).map(|i| Tick(90_000 * 3 + i * 40_000)).collect();
    let (video, cache) = warmed(&p, &ts);
    let mut translucent = 0usize;
    for t in &ts {
        let mut rgba = Vec::new();
        let complete = gpu.execute_into(&plan_at(&video, &cache, *t), &mut rgba).unwrap();
        assert!(complete, "scene guard: the frame at {t:?} is warmed");
        let shown = gpu.present(&plan_at(&video, &cache, *t), PresentOptions::default()).unwrap();
        assert!(shown);
        assert_eq!(gpu.read_display().unwrap(), premultiplied(&rgba), "the picture at {t:?}");
        translucent += rgba.chunks_exact(4).filter(|p| p[3] != 255 && p[3] != 0).count();
    }
    assert!(translucent > 0, "scene guard: opaque frames cannot see the premultiplication");
}

/// t1526: an incomplete frame leaves the picture as it is: the preview holds the last full frame
/// (the flicker "black frame ↔ Loading", item 14/15), unless nothing has been shown yet.
#[test]
fn t1526_an_incomplete_frame_does_not_touch_the_picture() {
    kt_plugins_builtin::install();
    let Some(gpu) = kt_gpu::test_engine() else { return };
    let p = demo_project(DemoSources::Synth);
    let t = Tick(90_000 * 3);
    let (video, cache) = warmed(&p, &[t]);
    assert!(gpu.present(&plan_at(&video, &cache, t), PresentOptions::default()).unwrap());
    let held = gpu.read_display().unwrap();

    let cold = BurstCache::new(CacheConfig { max_audio_bytes: 1, max_video_bytes: 1 << 20 });
    let hole = plan_at(&video, &cold, t);
    assert!(!gpu.present(&hole, PresentOptions::default()).unwrap(), "scene guard: starved");
    assert_eq!(gpu.read_display().unwrap(), held, "the last full frame stays");
    let first = PresentOptions { show_incomplete: true, glow: false };
    assert!(!gpu.present(&hole, first).unwrap());
    assert_ne!(gpu.read_display().unwrap(), held, "with nothing shown yet the frame is drawn");
}

/// t1527: the ambient glow picks the pixels the CPU loop picked, and a change of preview size is
/// a new texture the UI must register.
#[test]
fn t1527_glow_and_a_new_size() {
    kt_plugins_builtin::install();
    let Some(gpu) = kt_gpu::test_engine() else { return };
    let p = demo_project(DemoSources::Synth);
    let t = Tick(90_000 * 3);
    let (video, cache) = warmed(&p, &[t]);
    let plan = plan_at(&video, &cache, t);
    let mut rgba = Vec::new();
    gpu.execute_into(&plan, &mut rgba).unwrap();
    gpu.present(&plan, PresentOptions { show_incomplete: false, glow: true }).unwrap();
    let (gw, gh, sw, sh) = (32usize, 18usize, Q.width as usize, Q.height as usize);
    let mut want = vec![0u8; gw * gh * 4];
    for gy in 0..gh {
        for gx in 0..gw {
            let si = (gy * sh / gh * sw + gx * sw / gw) * 4;
            want[(gy * gw + gx) * 4..][..4].copy_from_slice(&rgba[si..si + 4]);
        }
    }
    assert_eq!(gpu.read_glow().unwrap(), premultiplied(&want));

    let before = gpu.display_target().unwrap();
    assert_eq!(before.size, (Q.width, Q.height));
    gpu.present(&plan, PresentOptions::default()).unwrap();
    assert_eq!(gpu.display_target().unwrap().generation, before.generation, "the same size");
    let half = Quality { width: 160, height: 90 };
    let first = PresentOptions { show_incomplete: true, glow: false };
    gpu.present(&video.frame_plan(&cache, t, half), first).unwrap();
    let after = gpu.display_target().unwrap();
    assert_eq!(after.size, (160, 90));
    assert_ne!(after.generation, before.generation, "a new size is a new texture");
}
