//! preview-throughput: the live preview keeps up. t1520–t1522 hold the worker's span, t1524 its
//! consequence as the performance panel counts it (V-fps, V-skipped).

use kt_cache::{BurstCache, CacheConfig};
use kt_engine::preview_bench::{BenchOptions, play_range};
use kt_engine::{DecodeRequest, MediaWorker, Player, Priority, RequestKind, VideoRenderer};
use kt_media::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_model::*;
use kt_time::Tick;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SEC: i64 = 90_000;
const STEP: i64 = SEC / 25;
/// Pictures between keyframes of the model decoder.
const GOP: i64 = 50;

#[derive(Default)]
struct Counts {
    /// Pictures out of the decoder.
    pictures: u64,
    /// Backward reads: a seek and a decoder built anew.
    reopens: u64,
    /// Every read's keys, as (pts, width, height).
    calls: Vec<Vec<(i64, u32, u32)>>,
}

/// A file as `FfmpegSource` reads it: one decoder going forward; a read behind its cursor seeks
/// to the keyframe before it and decodes up to it again; a span at several sizes is ONE forward
/// pass, the default trait method would be a pass per size.
struct GopSource {
    info: MediaInfo,
    cursor: Option<i64>,
    picture: Duration,
    counts: Arc<Mutex<Counts>>,
}

impl GopSource {
    fn new(native: (u32, u32), picture: Duration, counts: Arc<Mutex<Counts>>) -> GopSource {
        GopSource {
            info: MediaInfo {
                duration: Tick(60 * SEC),
                has_video: true,
                has_audio: false,
                fps: Some((25, 1)),
                width: native.0,
                height: native.1,
                sar: (1, 1),
                still: false,
            },
            cursor: None,
            picture,
            counts,
        }
    }

    /// Bring the decoder to the picture of `pts`.
    fn reach(&mut self, pts: i64) {
        let n = pts / STEP;
        let from = match self.cursor {
            Some(c) if c <= n => c + 1,
            at => {
                self.counts.lock().unwrap().reopens += u64::from(at.is_some());
                n - n.rem_euclid(GOP)
            }
        };
        let decoded = (n - from + 1).max(0);
        self.counts.lock().unwrap().pictures += decoded as u64;
        std::thread::sleep(self.picture * decoded as u32);
        self.cursor = Some(n);
    }

    fn frame(pts: i64, q: Quality) -> VideoFrame {
        let n = q.width as usize * q.height as usize * 4;
        VideoFrame {
            pts: Tick(pts),
            width: q.width,
            height: q.height,
            rgba: vec![255u8; n].into_boxed_slice(),
            has_alpha: false,
        }
    }
}

impl MediaSource for GopSource {
    fn info(&self) -> &MediaInfo {
        &self.info
    }
    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        Ok(AudioBurst::silence(idx))
    }
    fn read_video_frame(&mut self, p: Tick, q: Option<Quality>) -> Result<VideoFrame, MediaError> {
        self.reach(p.0);
        Ok(GopSource::frame(p.0, q.unwrap()))
    }
    fn keyframe_before(&self, p: Tick) -> Option<Tick> {
        Some(Tick(p.0 - p.0.rem_euclid(GOP * STEP)))
    }
    fn read_video_span(
        &mut self,
        keys: &[Tick],
        q: Option<Quality>,
        emit: &mut dyn FnMut(Tick, VideoFrame),
    ) -> Result<(), MediaError> {
        let q = q.unwrap();
        self.counts
            .lock()
            .unwrap()
            .calls
            .push(keys.iter().map(|k| (k.0, q.width, q.height)).collect());
        for k in keys {
            self.reach(k.0);
            emit(*k, GopSource::frame(k.0, q));
        }
        Ok(())
    }
    fn read_video_span_qualities(
        &mut self,
        keys: &[(Tick, Quality)],
        emit: &mut dyn FnMut(Tick, Quality, VideoFrame),
    ) -> Result<(), MediaError> {
        self.counts
            .lock()
            .unwrap()
            .calls
            .push(keys.iter().map(|(k, q)| (k.0, q.width, q.height)).collect());
        for (k, q) in keys {
            self.reach(k.0);
            emit(*k, *q, GopSource::frame(k.0, *q));
        }
        Ok(())
    }
}

fn wait_idle(w: &MediaWorker) {
    let until = Instant::now() + Duration::from_secs(20);
    while !w.is_idle() {
        assert!(Instant::now() < until, "the worker did not finish");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn near(w: &MediaWorker, pts: i64, q: Quality) {
    w.submit(DecodeRequest {
        kind: RequestKind::video_on_grid(Tick(pts), Some(q), STEP),
        prio: Priority::NearPrefetch,
        generation: 0,
    });
}

const SHARP: Quality = Quality { width: 640, height: 360 };
const BACKDROP: Quality = Quality { width: 1280, height: 720 };

/// t1520: one file asked at two decode sizes at the same moments is decoded once, forward. Two
/// spans, one per size, sent the second behind the cursor: 9 pictures decoded per frame asked on
/// the showcase.
#[test]
fn t1520_a_moment_is_decoded_once_for_every_size() {
    let counts = Arc::new(Mutex::new(Counts::default()));
    let src = GopSource::new((3840, 1608), Duration::ZERO, counts.clone());
    let cache = Arc::new(BurstCache::new(CacheConfig {
        max_audio_bytes: 1 << 20,
        max_video_bytes: 1 << 30,
    }));
    let w = MediaWorker::spawn(MediaId(1), Box::new(src), cache.clone());
    for i in 0..8 {
        near(&w, (20 + i) * STEP, SHARP);
        near(&w, (20 + i) * STEP, BACKDROP);
    }
    wait_idle(&w);
    let sid = kt_engine::source_id(MediaId(1));
    for i in 0..8 {
        let idx = kt_engine::video_cache_idx(Tick((20 + i) * STEP));
        assert!(cache.contains_video(sid, Some(SHARP), idx), "sharp {i}");
        assert!(cache.contains_video(sid, Some(BACKDROP), idx), "backdrop {i}");
    }
    let c = counts.lock().unwrap();
    assert_eq!(c.reopens, 0, "no size goes back behind the cursor");
    assert_eq!(c.pictures, 28, "the lead-in from the keyframe plus 8 pictures, once each");
    w.shutdown();
}

/// t1521: the worker looks into its queue between chunks of a span, and a chunk is whole moments:
/// cutting one between its sizes would ask the decoder for the same moment twice.
#[test]
fn t1521_a_chunk_never_splits_a_moment() {
    let counts = Arc::new(Mutex::new(Counts::default()));
    let src = GopSource::new((3840, 1608), Duration::ZERO, counts.clone());
    let cache = Arc::new(BurstCache::new(CacheConfig {
        max_audio_bytes: 1 << 20,
        max_video_bytes: 1 << 30,
    }));
    let w = MediaWorker::spawn(MediaId(1), Box::new(src), cache.clone());
    for i in 0..10 {
        near(&w, i * STEP, SHARP);
        near(&w, i * STEP, BACKDROP);
    }
    wait_idle(&w);
    let c = counts.lock().unwrap();
    let mut seen = BTreeSet::new();
    for call in &c.calls {
        let moments: BTreeSet<i64> = call.iter().map(|k| k.0).collect();
        assert!(moments.len() <= 4, "a chunk is at most four moments: {call:?}");
        for m in moments {
            assert!(seen.insert(m), "the moment {m} was asked in two chunks");
        }
    }
    assert_eq!(seen.len(), 10);
    w.shutdown();
}

/// t1522: a span holds at most half the video budget, every key counted at ITS decode size, and
/// what did not fit stays queued. Counting every key at the first key's size let a small first
/// key carry a span larger than the cache.
#[test]
fn t1522_a_span_fits_half_the_budget_at_each_keys_size() {
    let counts = Arc::new(Mutex::new(Counts::default()));
    let src = GopSource::new((3840, 2160), Duration::ZERO, counts.clone());
    let budget = 3 * 3840 * 2160 * 4;
    let cache = Arc::new(BurstCache::new(CacheConfig {
        max_audio_bytes: 1 << 20,
        max_video_bytes: budget,
    }));
    let w = MediaWorker::spawn(MediaId(1), Box::new(src), cache.clone());
    let big = Quality { width: 3840, height: 2160 };
    let tiny = Quality { width: 64, height: 36 };
    // the first key is tiny: the whole span was budgeted at 64×36
    near(&w, 0, tiny);
    for i in 0..12 {
        near(&w, (i + 1) * STEP, big);
    }
    wait_idle(&w);
    let c = counts.lock().unwrap();
    for call in &c.calls {
        let bytes: usize = call.iter().map(|k| k.1 as usize * k.2 as usize * 4).sum();
        assert!(bytes <= budget / 2, "a read of {} MB of a {} MB cache", bytes >> 20, budget >> 20);
    }
    let asked: BTreeSet<i64> = c.calls.iter().flatten().map(|k| k.0).collect();
    assert_eq!(asked.len(), 13, "the keys past the budget stayed queued");
    w.shutdown();
}

fn clip(id: u64, lane: u32, geometry: EventGeometry) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry,
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane,
        start: Tick(0),
        duration: Tick(20 * SEC),
        source_in: Tick(10 * SEC),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 0.5,
        fx_off: false,
    }
}

/// The showcase's cut: a wide file over its own filled backdrop, 25 fps.
fn sharp_over_backdrop() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 25,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: true,
    });
    p.media.push(MediaRef::new(MediaId(1), "gop"));
    p.clips.push(clip(1, 0, Default::default()));
    p.clips.push(clip(2, 1, EventGeometry { rule: Conform::Fill, ..Default::default() }));
    p.ensure_tracks(LaneKind::Video, 2);
    p.validate().unwrap();
    p
}

/// t1524: the consequence, in the performance panel's terms. A file that takes 12 ms a picture
/// keeps up with 25 fps at one pass; the two passes of the sharp layer and its backdrop, the second
/// seeking back a GOP, do not.
#[test]
fn t1524_a_layer_over_its_backdrop_plays_in_time() {
    let p = sharp_over_backdrop();
    let counts = Arc::new(Mutex::new(Counts::default()));
    let src = GopSource::new((3840, 1608), Duration::from_millis(12), counts.clone());
    let mut facts = kt_engine::SourceFacts::default();
    facts.observe(MediaId(1), src.info());
    let vr = VideoRenderer::new_with_facts(&p, &facts);
    let q = Quality { width: 640, height: 360 };
    let sizes: BTreeSet<_> = vr.layers_at(Tick(SEC), q).iter().map(|l| l.2).collect();
    assert_eq!(sizes.len(), 2, "scene guard: one file at two decode sizes: {sizes:?}");

    let (player, ring) = Player::new(p, vec![(MediaId(1), Box::new(src) as Box<dyn MediaSource>)]);
    player.set_preview_quality(q);
    let r = play_range(
        &player,
        ring,
        BenchOptions { from: Tick(0), to: Tick(4 * SEC), ..Default::default() },
    );
    let c = counts.lock().unwrap();
    assert!(
        r.shown * 10 >= r.grid_frames * 7,
        "{} of {} frames, skipped {}, incomplete {}, {} pictures, {} reopens",
        r.shown,
        r.grid_frames,
        r.skipped,
        r.incomplete,
        c.pictures,
        c.reopens,
    );
}
