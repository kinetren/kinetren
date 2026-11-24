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
            fx_in: Vec::new(),
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
/// The frames decoded in the lap.
fn lap(vr: &VideoRenderer, cache: &Arc<BurstCache>, q: Quality, region: (i64, i64), sizing: &mut FramesSizing) -> usize {
    let step = vr.grid.frame_step();
    let mut decoded = 0;
    let mut t = Tick(region.0);
    while t.0 < region.1 {
        let (missing, window) = video_prefetch_pass_sized(vr, cache, t, q, Some(region), &|_| false);
        size_frames_cache(vr, cache, q, Some(region), 1, window, (&|_| false, 0), sizing);
        for (m, pts, dq) in missing {
            decoded += 1;
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
    decoded
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

/// t2606 (loop-keeps-its-lap, found-defects 551): the same 830 MB loop over the 512 MiB base with
/// no memory to grow, driven as the player drives it — the cache told of the loop, a lap marked at
/// each wrap. The plain LRU decodes every frame of every lap; told of the loop, the cache keeps a
/// stable part of it and later laps decode only the rest, with no frame of the look-ahead dropped
/// unread.
#[test]
fn t2606_a_loop_past_the_bound_keeps_a_part_when_the_memory_is_not_there() {
    let p = stack(3);
    let vr = VideoRenderer::new(&p);
    let q = Quality { width: 640, height: 360 };
    let region = (2 * SEC, 14 * SEC);
    let frames = 3 * 300;
    let blind: &'static kt_time::mem::Ledger = Box::leak(Box::new(kt_time::mem::Ledger::new()));
    let run = |tell: bool| {
        let cache = Arc::new(BurstCache::with_ledger(CacheConfig::default(), blind));
        let mut sizing = FramesSizing::default();
        cache.keep_loop(tell);
        let laps: Vec<usize> = (0..4)
            .map(|i| {
                if i > 0 {
                    cache.lap();
                }
                lap(&vr, &cache, q, region, &mut sizing)
            })
            .collect();
        (laps, cache.stats().unused_evictions)
    };
    let (plain, _) = run(false);
    assert!(plain[1..].iter().all(|&d| d >= frames), "the plain LRU decodes every frame of every lap: {plain:?}");
    let (kept, thrash) = run(true);
    // the first lap decodes every frame, and the look-ahead folded past the wrap the next lap's head
    assert!(kept[0] >= frames, "{kept:?}");
    for (i, &d) in kept.iter().enumerate().skip(1) {
        // half of the 512 MiB base is 291 frames of 640×360
        assert!(d <= frames - 250, "lap {i} decoded {d} of {frames}: {kept:?}");
    }
    assert!(kept[2..].windows(2).all(|w| w[0] == w[1]), "the kept part is stable: {kept:?}");
    assert_eq!(thrash, 0, "no frame of the look-ahead is dropped unread");
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

/// t3605 (cache-audit cut 1): a moment whose final frame is held — in the kept composites or on the
/// disk prerender — needs no source frame. A loop with half its moments held decodes nothing for them,
/// and the cache wants only the other half (plus the eighth and the window), counted AFTER the skip:
/// when the held set grows the loop is counted again and the cap comes down to it.
#[test]
fn t3605_held_moments_neither_decode_nor_count() {
    let p = stack(3);
    let vr = VideoRenderer::new(&p);
    let q = Quality { width: 640, height: 360 };
    let region = (2 * SEC, 14 * SEC);
    let frame = 640 * 360 * 4;
    let half = 8 * SEC;
    let held = |t: Tick| t.0 < half;
    assert_eq!(kt_engine::loop_frame_bytes_except(&vr, q, region, usize::MAX, &held), 3 * 150 * frame, "the held half counted out");
    let l = ledger(16 << 30);
    let cache = Arc::new(BurstCache::with_ledger(CacheConfig::default(), l));
    let mut sizing = FramesSizing::default();
    // nothing held first: the cap grows for the whole loop
    lap(&vr, &cache, q, region, &mut sizing);
    let whole = cache.max_video_bytes();
    assert!(whole >= 3 * 300 * frame, "the whole loop wanted: {} MB", whole >> 20);
    // half the loop held: a lap decodes none of its frames, and the want comes down to the other half
    let step = vr.grid.frame_step();
    let mut decoded_held = 0;
    let mut t = Tick(region.0);
    while t.0 < region.1 {
        let (missing, window) = video_prefetch_pass_sized(&vr, &cache, t, q, Some(region), &held);
        size_frames_cache(&vr, &cache, q, Some(region), 1, window, (&held, 150), &mut sizing);
        decoded_held += missing.iter().filter(|(_, pts, _)| pts.0 < half).count();
        t = Tick(t.0 + step);
    }
    assert_eq!(decoded_held, 0, "no source frame of a held moment asked");
    let cut = cache.max_video_bytes();
    assert!(cut < whole, "the want sized after the skip: {} MB of {} MB", cut >> 20, whole >> 20);
    assert!(cut >= 3 * 150 * frame + 3 * 150 * frame / 8, "still the unheld half and its eighth: {} MB", cut >> 20);
}
