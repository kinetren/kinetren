//! Tests t2393 and t2394 (frames-cache-growth, found-defects 396): the player's decoded-frame cache
//! asks the memory ledger for what its look-ahead and loop need at the preview quality, through the
//! prefetch pass the player runs — the owner's trace showed the frames cap at 576 MB for a whole run
//! while ~21 GB of growth budget stood free, and a loop that did not fit evicted on every lap.
//!
//! No file is decoded: a missing key is filled with a blank frame of its decode size (an infinitely
//! fast decoder, as `simulate_video_playback`), and the frame is read by the plan itself.

use kt_cache::{BurstCache, CacheConfig};
use kt_engine::{FramesSizing, VideoRenderer, size_frames_cache, video_prefetch_pass_sized};
use kt_media::{Quality, VideoFrame};
use kt_model::*;
use kt_time::Tick;
use std::sync::Arc;

const SEC: i64 = 90_000;

/// `layers` video events stacked on their own tracks, 20 s long, at 25 fps.
fn stack(layers: u64) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 25,
        width: 1920,
        height: 1080,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: true,
    });
    for i in 0..layers {
        p.media.push(MediaRef::new(MediaId(i + 1), "checker"));
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            masks: Default::default(),
            id: ClipId(i + 1),
            link_group: None,
            kind: ClipKind::Video,
            lane: i as u32,
            start: Tick(0),
            duration: Tick(20 * SEC),
            source_in: Tick(0),
            media: MediaId(i + 1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 0.5,
            fx_off: false,
        });
    }
    p.ensure_tracks(LaneKind::Video, layers as u32);
    p.validate().unwrap();
    p
}

fn ledger(available: u64) -> &'static kt_time::mem::Ledger {
    let l: &'static kt_time::mem::Ledger = Box::leak(Box::new(kt_time::mem::Ledger::new()));
    l.set_probe(move || Some(available));
    l
}

/// One lap of the loop region as the player's prefetch thread drives it: a pass at every frame (the
/// window folded into the region), the sizing after it, the misses filled, the frame read by the plan.
fn lap(vr: &VideoRenderer, cache: &Arc<BurstCache>, q: Quality, region: (i64, i64), sizing: &mut FramesSizing) {
    let step = vr.grid.frame_step();
    let mut t = Tick(region.0);
    while t.0 < region.1 {
        let (missing, window) = video_prefetch_pass_sized(vr, cache, t, q, Some(region), &|_| false);
        size_frames_cache(vr, cache, q, Some(region), 1, window, sizing);
        for (m, pts, dq) in missing {
            let frame = VideoFrame {
                pts,
                width: dq.width,
                height: dq.height,
                // zeroed pages are not touched: the test holds GBs of frames without the memory
                rgba: vec![0u8; dq.width as usize * dq.height as usize * 4].into_boxed_slice(),
                has_alpha: false,
            };
            cache.insert_video_span(kt_engine::source_id(m), Some(dq), kt_engine::video_cache_idx(pts), step, Arc::new(frame));
        }
        let _ = vr.frame_plan(cache, t, q);
        t = Tick(t.0 + step);
    }
}

/// t2393: three 640×360 layers over a 12 s loop are 830 MB of frames, past the 512 MiB base. With
/// memory to spare the cap rises past the base on the first lap, and the laps after it evict
/// nothing; an export's claim takes the cap back to the base at once and its end gives it back; a
/// lower preview quality shrinks it. The base alone evicts on every lap (the premise).
#[test]
fn t2393_a_loop_past_the_base_is_held_whole_when_the_memory_is_there() {
    let p = stack(3);
    let vr = VideoRenderer::new(&p);
    let q = Quality { width: 640, height: 360 };
    let region = (2 * SEC, 14 * SEC);
    let frame = 640 * 360 * 4;
    let loop_bytes = kt_engine::loop_frame_bytes(&vr, q, region, usize::MAX);
    assert_eq!(loop_bytes, 3 * 300 * frame, "every frame of the loop counted once");
    let base = CacheConfig::default().max_video_bytes;
    assert!(loop_bytes > base, "the scene's premise: the loop does not fit the base");

    // the base alone (memory unknown: nothing granted) evicts on every lap
    let blind: &'static kt_time::mem::Ledger = Box::leak(Box::new(kt_time::mem::Ledger::new()));
    let cache = Arc::new(BurstCache::with_ledger(CacheConfig::default(), blind));
    let mut sizing = FramesSizing::default();
    lap(&vr, &cache, q, region, &mut sizing);
    let ev = cache.stats().evictions;
    lap(&vr, &cache, q, region, &mut sizing);
    assert_eq!(cache.max_video_bytes(), base);
    assert!(cache.stats().evictions - ev > 600, "the base thrashes a lap: {}", cache.stats().evictions - ev);
    drop(cache);

    let l = ledger(16 << 30);
    let cache = Arc::new(BurstCache::with_ledger(CacheConfig::default(), l));
    let mut sizing = FramesSizing::default();
    lap(&vr, &cache, q, region, &mut sizing);
    let cap = cache.max_video_bytes();
    assert!(cap > base && cap >= loop_bytes + loop_bytes / 8, "the cap grew for the loop: {} MB", cap >> 20);
    let ev = cache.stats().evictions;
    lap(&vr, &cache, q, region, &mut sizing);
    lap(&vr, &cache, q, region, &mut sizing);
    assert_eq!(cache.stats().evictions, ev, "a lap of a loop that fits evicts nothing");
    assert_eq!(cache.stats().unused_evictions, 0);

    let claim = l.claim();
    assert_eq!(cache.max_video_bytes(), base, "an export's claim takes the growth back");
    assert!(cache.video_bytes() <= base, "at once: {} MB held", cache.video_bytes() >> 20);
    drop(claim);
    assert_eq!(cache.max_video_bytes(), cap, "the claim gone, the grant is back");

    let small = Quality { width: 320, height: 180 };
    lap(&vr, &cache, small, region, &mut sizing);
    assert!(cache.max_video_bytes() < cap, "a lower quality wants less: {} MB", cache.max_video_bytes() >> 20);
    assert!(cache.video_bytes() <= cache.max_video_bytes());
}

/// t2394: the stream pacing of a shared renderer build orders a quarter of the cache's cap as it is
/// when it orders, not as it was when the build was made.
#[test]
fn t2394_the_stream_pacing_reads_the_cap_it_has_now() {
    let p = stack(1);
    let l = ledger(16 << 30);
    let cache = Arc::new(BurstCache::with_ledger(CacheConfig::default(), l));
    let mut vr = VideoRenderer::new(&p);
    vr.set_stream_pacing(Some(kt_engine::StreamPacing::preview(cache.max_video_bytes())));
    vr.set_stream_cap(cache.video_cap_reader());
    assert_eq!(vr.stream_order_bytes(), Some(CacheConfig::default().max_video_bytes / 4));
    cache.want_video(4 << 30);
    assert_eq!(vr.stream_order_bytes(), Some(1 << 30), "a quarter of the grown cap");
}
