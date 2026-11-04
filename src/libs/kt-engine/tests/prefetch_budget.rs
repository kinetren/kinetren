//! Tests 1416, 1416b and 1418 (video-prefetch-thrash; t1417 is in kt-cache): the video look-ahead never asks for more than
//! the cache can hold until it is read.
//!
//! The README showcase played with the cache 99 % full and a stream of "entries dropped
//! before the first read": the window was half the budget divided by ONE frame of the
//! largest decode size, while a stack of layers asks for several frames per tick. The
//! thrash is reproduced without FFmpeg: `simulate_video_playback` is an infinitely fast
//! decoder (a blank frame of the key's decode size) plus the plan's own reads.

use kt_cache::{BurstCache, CacheConfig};
use kt_engine::{
    DecodeRequest, MediaWorker, Priority, RequestKind, VideoRenderer, simulate_video_playback,
    video_prefetch_wants,
};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_model::*;
use kt_time::Tick;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SEC: i64 = 90_000;

/// `layers` video events stacked on their own tracks, each from its own file, 6 s long, at
/// 25 fps (the showcase's grid; the stack stands for a cut over its blurred backdrop and a
/// floating frame).
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
            duration: Tick(6 * SEC),
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

const Q: Quality = Quality { width: 1920, height: 1080 };
const BUDGET: usize = 128 << 20;

fn bytes(wants: &[(MediaId, Tick, Quality)]) -> usize {
    wants.iter().map(|(_, _, q)| q.width as usize * q.height as usize * 4).sum()
}

/// Test 1416: three layers of 1080p frames against a 128 MB cache. The window of one
/// frame per tick was 7 ticks = 21 frames = 174 MB, the cache dropped the nearest
/// frames of the window before they were read, and playback missed them. Now the window
/// is cut at whole frames within half the budget, and playing 4 s drops nothing unread.
#[test]
fn t1416_window_of_layers_fits_the_cache() {
    let p = stack(3);
    let vr = VideoRenderer::new(&p);
    let step = vr.grid.frame_step();
    // scene guard: the stack really asks for three frames per tick
    assert_eq!(vr.layers_at(Tick(SEC), Q).len(), 3, "three layers at a tick");

    let wants = video_prefetch_wants(&vr, Tick(SEC), Q, None, BUDGET);
    assert!(bytes(&wants) <= BUDGET / 2, "the window is {} MB", bytes(&wants) >> 20);
    let ahead = wants.iter().map(|(_, t, _)| t.0).max().unwrap();
    assert!(ahead > SEC, "the window still looks past the playhead's own frame");
    assert_eq!(wants.len() % 3, 0, "the window is cut at a WHOLE frame: {}", wants.len());
    assert_eq!(wants.first().unwrap().1, Tick(SEC), "nearest first");

    let cache = BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: BUDGET });
    simulate_video_playback(&vr, &cache, Tick(0), Tick(4 * SEC), Q);
    let s = cache.stats();
    assert!(s.evictions > 0, "4 s of three layers do not fit 128 MB: the cache must evict");
    assert_eq!(s.unused_evictions, 0, "no entry is dropped before its first read");
    assert_eq!(s.video_misses, 0, "every frame playback reads is in the cache");
    assert_eq!(s.video_hits as i64, 3 * 4 * SEC / step, "every layer of every frame was read");
}

/// Test 1416b: a frame too large for half the budget is still asked for at the playhead —
/// a window cut to nothing would show nothing.
#[test]
fn t1416b_the_playhead_frame_is_always_asked() {
    let p = stack(3);
    let vr = VideoRenderer::new(&p);
    let wants = video_prefetch_wants(&vr, Tick(SEC), Q, None, 8 << 20);
    assert_eq!(wants.len(), 3, "exactly the playhead's frame: {wants:?}");
    assert!(wants.iter().all(|(_, t, _)| *t == Tick(SEC)));
}

/// A video source whose every frame takes a few milliseconds and is logged; audio bursts
/// are slow too, to keep the worker busy while the test edits its queue.
struct LoggedSource {
    inner: SyntheticSource,
    served: Arc<Mutex<Vec<i64>>>,
}

impl MediaSource for LoggedSource {
    fn info(&self) -> &MediaInfo {
        self.inner.info()
    }
    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        std::thread::sleep(Duration::from_millis(80));
        self.inner.read_audio_burst(idx)
    }
    fn read_video_frame(&mut self, p: Tick, q: Option<Quality>) -> Result<VideoFrame, MediaError> {
        self.served.lock().unwrap().push(p.0);
        self.inner.read_video_frame(p, q)
    }
    fn keyframe_before(&self, p: Tick) -> Option<Tick> {
        self.inner.keyframe_before(p)
    }
}

/// Test 1418: the look-ahead requests the playhead has passed are withdrawn from the
/// worker's queue, the others and the Playback ones stay. Before, a request stayed until a
/// seek, and a decoder slower than the picture decoded frames behind the playhead into
/// entries nobody read — the same "dropped before the first read" in the panel.
#[test]
fn t1418_passed_look_ahead_is_withdrawn() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let served = Arc::new(Mutex::new(Vec::new()));
    let source = LoggedSource {
        inner: SyntheticSource::new(Tick(600 * SEC), AudioPattern::Ramp),
        served: served.clone(),
    };
    let worker = MediaWorker::spawn(MediaId(1), Box::new(source), cache.clone());
    let q = Quality { width: 64, height: 36 };
    let step = SEC / 25;
    // the worker is busy with a slow burst while the queue is filled and edited
    worker.submit(DecodeRequest {
        kind: RequestKind::Audio { idx: 0 },
        prio: Priority::Playback,
        generation: 0,
    });
    std::thread::sleep(Duration::from_millis(20));
    for i in 0..20 {
        worker.submit(DecodeRequest {
            kind: RequestKind::video_on_grid(Tick(i * step), Some(q), step),
            prio: Priority::NearPrefetch,
            generation: 0,
        });
    }
    worker.submit(DecodeRequest {
        kind: RequestKind::video_on_grid(
            Tick(3 * step),
            Some(Quality { width: 32, height: 18 }),
            step,
        ),
        prio: Priority::Playback,
        generation: 0,
    });
    // the playhead is at frame 10: the look-ahead before it is no longer wanted
    worker.withdraw_video(Priority::NearPrefetch, |pts, _| pts >= 10 * step);
    for _ in 0..2000 {
        if worker.is_idle() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut got = served.lock().unwrap().clone();
    got.sort();
    let want: Vec<i64> = std::iter::once(3 * step).chain((10..20).map(|i| i * step)).collect();
    assert_eq!(got, want, "only the Playback frame and the look-ahead still ahead are decoded");
    worker.shutdown();
}

/// Test 1418b: the preview's Playback request is THE FRAME ON SCREEN. Paging the paused
/// preview over ten frames of a slow file decodes the frame being decoded and the one now
/// shown, not the eight in between. Before, every shown frame queued one more Playback
/// request; Playback outranks the look-ahead, so a decoder slower than the picture spent
/// itself on frames nobody would read while the window ahead starved.
#[test]
fn t1418b_preview_withdraws_the_frames_it_left() {
    let mut p = stack(1);
    p.clips[0].duration = Tick(20 * SEC);
    let src = SyntheticSource::new(Tick(60 * SEC), AudioPattern::Silence)
        .with_video_delay(Duration::from_millis(60));
    let (player, _consumer) = kt_engine::Player::new(p, vec![(MediaId(1), Box::new(src))]);
    let q = player.preview_quality();
    let step = SEC / 25;
    // far from the paused playhead: the look-ahead window (at 0) does not cover these
    let at = |i: i64| Tick(10 * SEC + i * step);
    let sid = kt_engine::source_id(MediaId(1));
    let mut buf = Vec::new();
    for i in 0..10 {
        let _ = player.compose_preview_into(at(i), &mut buf);
        std::thread::sleep(Duration::from_millis(5));
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while !player.cache.contains_video(sid, Some(q), at(9).0) {
        assert!(std::time::Instant::now() < deadline, "the frame on screen never arrived");
        std::thread::sleep(Duration::from_millis(10));
    }
    // what was still queued would be decoded within a few frame times
    std::thread::sleep(Duration::from_millis(400));
    let between: Vec<i64> =
        (1..9).filter(|&i| player.cache.contains_video(sid, Some(q), at(i).0)).collect();
    assert!(between.len() <= 1, "frames the preview had left were decoded: {between:?}");
}

/// Test 1528 (preview-throughput D3): a cut past the window's bytes still gets its first frame
/// asked, and only that: a decoder needs the same time to reach it however near it is asked,
/// and the bytes of the frames after it stay within the budget.
#[test]
fn t1528_the_first_frame_of_a_cut_is_asked_past_the_window() {
    let mut p = stack(1);
    p.clips[0].duration = Tick(SEC);
    p.media.push(MediaRef::new(MediaId(2), "checker"));
    let mut b = p.clips[0].clone();
    (b.id, b.media, b.start, b.source_in) = (ClipId(2), MediaId(2), Tick(SEC), Tick(40 * SEC));
    p.clips.push(b);
    p.validate().unwrap();
    let vr = VideoRenderer::new(&p);
    let step = vr.grid.frame_step();
    let frame = Q.width as usize * Q.height as usize * 4;
    // a window of 4 frames: 0.16 s, the cut stands 1 s ahead
    let wants = video_prefetch_wants(&vr, Tick(0), Q, None, 8 * frame + 1);
    let of = |m: u64| wants.iter().filter(|(id, _, _)| id.0 == m).collect::<Vec<_>>();
    assert_eq!(of(1).len(), 4, "the window is its bytes: {wants:?}");
    assert_eq!(of(2).len(), 1, "exactly the cut's first frame: {wants:?}");
    let first = vr.layers_at(Tick(SEC), Q)[0];
    assert_eq!(*of(2)[0], first, "it is the key the plan will read at the cut");
    assert_eq!(wants.last(), Some(&first), "after the window: the nearest frames go first");

    // nearer than the bytes reach, the cut is an ordinary part of the window
    let wants = video_prefetch_wants(&vr, Tick(SEC - 2 * step), Q, None, 8 * frame + 1);
    assert_eq!(wants.len(), 4, "no lead on top of a window that holds the cut: {wants:?}");

    // a stack without a cut asks exactly the window it asked before
    let vr = VideoRenderer::new(&stack(3));
    let wants = video_prefetch_wants(&vr, Tick(SEC), Q, None, BUDGET);
    assert!(bytes(&wants) <= BUDGET / 2);
}
