//! Tests 27-28 (items 19-20): the video cache does not “stall”
//! under a stream of Playback requests, EOF keys are not re-decoded,
//! coverage with a span gives honest strips.

use kt_cache::{BurstCache, BurstKind, CacheConfig};
use kt_engine::{DecodeRequest, MediaWorker, Priority, RequestKind, source_id};
use kt_media::Quality;
use kt_media::synthetic::{AudioPattern, SyntheticSource, SyntheticStats};
use kt_model::MediaId;
use kt_time::{TICK_HZ, Tick};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

const STEP: i64 = TICK_HZ / 30;
const Q: Quality = Quality { width: 64, height: 36 };

/// Test 27 (item 19): “slow 4K” + a constant stream of Playback requests for the
/// current frame (imitating an incomplete preview). Previously Playback singles
/// degraded spans into “one frame at a time” and the coverage stopped growing.
#[test]
fn t27_span_absorbs_prefetch_tail_under_playback_stream() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let stats = Arc::new(SyntheticStats::default());
    let src = SyntheticSource::with_stats(Tick(90_000 * 60), AudioPattern::Silence, stats.clone())
        .with_video_delay(std::time::Duration::from_millis(8)); // a decode longer than the emulated “frame”
    let worker = MediaWorker::spawn(MediaId(1), Box::new(src), cache.clone());

    let total: i64 = 90; // 3 s of a 30 fps grid
    let t0 = std::time::Instant::now();
    let covered = |cache: &BurstCache| -> i64 {
        cache
            .coverage(source_id(MediaId(1)), BurstKind::video(Some(Q)), 0..total * STEP)
            .iter()
            .map(|r| (r.end - r.start) / STEP)
            .sum()
    };

    let mut mid_covered = None;
    // emulator: the playhead creeps along “real time” (25 ms/frame), Playback
    // for the current frame (if not in the cache), a NearPrefetch pass once per ~100 ms
    let mut last_prefetch = std::time::Instant::now() - std::time::Duration::from_secs(1);
    loop {
        let el = t0.elapsed();
        let cur = ((el.as_millis() as i64) / 25).min(total - 1);
        if !cache.contains_video(source_id(MediaId(1)), Some(Q), cur * STEP) {
            worker.submit(DecodeRequest {
                kind: RequestKind::video_on_grid(Tick(cur * STEP), Some(Q), STEP),
                prio: Priority::Playback,
                generation: 0,
            });
        }
        if last_prefetch.elapsed().as_millis() >= 100 {
            last_prefetch = std::time::Instant::now();
            for i in cur..(cur + 60).min(total) {
                if !cache.contains_video(source_id(MediaId(1)), Some(Q), i * STEP) {
                    worker.submit(DecodeRequest {
                        kind: RequestKind::video_on_grid(Tick(i * STEP), Some(Q), STEP),
                        prio: Priority::NearPrefetch,
                        generation: 0,
                    });
                }
            }
        }
        let c = covered(&cache);
        if c >= total / 2 && mid_covered.is_none() {
            mid_covered = Some(el); // the moment of half coverage
        }
        if c >= total {
            break;
        }
        assert!(
            el < std::time::Duration::from_secs(15),
            "coverage stalled: {}/{total} frames (stagnation item 19)",
            covered(&cache)
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    // progress is continuous: half is covered long before the end
    let mid_at = mid_covered.expect("half of the coverage had to happen");
    assert!(mid_at < t0.elapsed(), "half of the coverage must arrive before the full one ({mid_at:?})");
    // each frame is decoded exactly once
    assert_eq!(stats.video_reads.load(Relaxed), total as u64, "without repeated decodes");
    worker.shutdown();
}

/// Test 27 (item 19g): keys beyond EOF are not re-decoded forever.
#[test]
fn t27_failed_keys_are_not_redecoded() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let stats = Arc::new(SyntheticStats::default());
    let src = SyntheticSource::with_stats(Tick(90_000), AudioPattern::Silence, stats.clone())
        .with_video_eof();
    let worker = MediaWorker::spawn(MediaId(1), Box::new(src), cache.clone());

    let beyond = Tick(90_000 * 10); // far beyond EOF
    for _ in 0..5 {
        worker.submit(DecodeRequest {
            kind: RequestKind::video_on_grid(beyond, Some(Q), STEP),
            prio: Priority::NearPrefetch,
            generation: 0,
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !worker.is_idle() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    assert!(
        stats.video_reads.load(Relaxed) <= 1,
        "the EOF key is decoded at most once, was {}",
        stats.video_reads.load(Relaxed)
    );
    worker.shutdown();
}

/// Test 28 (item 20): the worker inserts coverage as a strip at the grid step —
/// the whole run range = ONE continuous run.
#[test]
fn t28_worker_inserts_grid_span_coverage() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let src = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    let worker = MediaWorker::spawn(MediaId(1), Box::new(src), cache.clone());
    let n: i64 = 30;
    for i in 0..n {
        worker.submit(DecodeRequest {
            kind: RequestKind::video_on_grid(Tick(i * STEP), Some(Q), STEP),
            prio: Priority::NearPrefetch,
            generation: 0,
        });
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !worker.is_idle() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let cov = cache.coverage(source_id(MediaId(1)), BurstKind::video(Some(Q)), 0..n * STEP);
    assert_eq!(cov, vec![0..n * STEP], "the bar must be continuous: {cov:?}");
    worker.shutdown();
}

/// Test 758 (R.63 item 13): after a JUMP the sound does not wait for the video.
///
/// The owner's complaint: “I just jumped to a section with one long
/// event… but that is no reason to load the video for so long with such
/// underruns”. From the flight log every jump into a 870 s file gave seconds of
/// full silence, even though the offline cost of rendering the same place is 2 %
/// of real time: the sound stood in the queue BEHIND the frames of the same worker.
///
/// What is asserted is a CONSEQUENCE: on a jump the first audio bursts are served before the
/// video, that is, the sound makes the deadline and the video catches up on its own.
#[test]
fn t758_audio_is_served_before_video_after_a_seek() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let stats = Arc::new(SyntheticStats::default());
    // “slow 1080p”: a frame costs more than an audio burst — exactly like the owner's H.264
    // against the cheap Opus decode
    let src = SyntheticSource::with_stats(Tick(90_000 * 900), AudioPattern::Ramp, stats.clone())
        .with_video_delay(std::time::Duration::from_millis(12));
    let worker = MediaWorker::spawn(MediaId(1), Box::new(src), cache.clone());

    // A JUMP to the tail: the video prefetch has already requested the window around the new point, and
    // the worker has ALREADY STARTED decoding it — and it is into this gap that the playback sound falls
    // (the span did not look into the queue until the very end)
    let jump_tick = 800 * TICK_HZ;
    let first_burst = jump_tick / kt_time::AUDIO_BURST_TICKS;
    for i in 0..64i64 {
        worker.submit(DecodeRequest {
            kind: RequestKind::video_on_grid(Tick(jump_tick + i * STEP), Some(Q), STEP),
            prio: Priority::NearPrefetch,
            generation: 0,
        });
    }
    // let the worker enter the span (one frame = 12 ms)
    std::thread::sleep(std::time::Duration::from_millis(30));
    for i in 0..8i64 {
        worker.submit(DecodeRequest {
            kind: RequestKind::Audio { idx: first_burst + i },
            prio: Priority::Playback,
            generation: 0,
        });
    }

    // The sound must appear QUICKLY: eight bursts are 170 ms of material, and waiting for them
    // longer than they sound guarantees silence.
    // The yardstick is the output ring (about 85 ms): anything longer is heard as silence.
    // The span here is 64 frames of 12 ms = 768 ms in one piece on the old code.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(250);
    let mut got = 0;
    while std::time::Instant::now() < deadline {
        got = (0..8)
            .filter(|i| cache.get_audio(source_id(MediaId(1)), first_burst + i).is_some())
            .count();
        if got == 8 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let vid = stats.video_reads.load(Relaxed);
    assert_eq!(
        got, 8,
        "audio after the jump did not keep up: read {got} of 8 bursts, \
         while the video frames in the same time were {vid} (audio waits for video)"
    );
    // and the video is not abandoned at the same time — it catches up right behind
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(900);
    while std::time::Instant::now() < deadline && stats.video_reads.load(Relaxed) < 8 {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        stats.video_reads.load(Relaxed) >= 8,
        "video stopped catching up after audio was let through ahead: frames {}, audio {}",
        stats.video_reads.load(Relaxed),
        stats.audio_reads.load(Relaxed)
    );
    worker.shutdown();
}
