//! decode-request-storm: a request lives until it is served or no longer wanted, the decoder reads
//! forward, and a stream stage's history is ordered no faster than it is read. t2040 (a run behind
//! the cursor is read forward), t2041 (a key queued or in flight is not a new request), t2042 (the
//! paced order is the earliest of the history), t2043 (a composite does not compute a history; the
//! advance does, to the same bytes), t2044 (a short loop over a cut mosh whose history does not fit
//! the cache plays, every decoded frame read).
//!
//! The owner's journal (kkt-26.11.08, release 360b50e6): after he selected Datamosh, 1570 decode
//! requests a second while paused, 15–100 frames decoded, every one dropped before it was read, and
//! four loops in a row "V-fps 0.0, new frames 0".

use kt_cache::{BurstCache, CacheConfig};
use kt_engine::preview_bench::{BenchOptions, play_range};
use kt_engine::{DecodeRequest, FramePlan, MediaWorker, Player, Priority, RequestKind, StreamPacing, VideoRenderer};
use kt_media::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_model::*;
use kt_plugin_api::stream::StreamMemo;
use kt_time::Tick;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

const SEC: i64 = 90_000;
const STEP: i64 = SEC / 30;
/// Pictures between keyframes of the model decoder.
const GOP: i64 = 30;

/// The tests here read the process-wide work counters: one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Default)]
struct Counts {
    /// Pictures out of the decoder (a seek decodes from the keyframe up to the key).
    pictures: u64,
    /// Reads behind the decoder's cursor: a seek and a decoder built anew.
    reopens: u64,
    /// Keys served, each read.
    served: Vec<i64>,
}

/// Holds the decoder in its first read until opened (t2041 sets the queue up meanwhile).
#[derive(Default)]
struct Gate {
    open: Mutex<bool>,
    cv: Condvar,
    entered: Mutex<bool>,
}

impl Gate {
    fn pass(&self) {
        *self.entered.lock().unwrap() = true;
        let mut open = self.open.lock().unwrap();
        while !*open {
            open = self.cv.wait(open).unwrap();
        }
    }
    fn wait_busy(&self) {
        let deadline = kt_time::test_deadline();
        while !*self.entered.lock().unwrap() {
            assert!(std::time::Instant::now() < deadline, "the worker took no request");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn release(&self) {
        *self.open.lock().unwrap() = true;
        self.cv.notify_all();
    }
}

/// A file as `FfmpegSource` reads it: one decoder going forward; a read behind its cursor seeks to
/// the keyframe before it and decodes up to it again.
struct GopSource {
    info: MediaInfo,
    cursor: Option<i64>,
    picture: Duration,
    counts: Arc<Mutex<Counts>>,
    gate: Option<Arc<Gate>>,
}

impl GopSource {
    fn new(picture: Duration, counts: Arc<Mutex<Counts>>) -> GopSource {
        GopSource {
            info: MediaInfo {
                duration: Tick(60 * SEC),
                has_video: true,
                has_audio: false,
                fps: Some((30, 1)),
                width: 640,
                height: 360,
                sar: (1, 1),
                still: false,
            },
            cursor: None,
            picture,
            counts,
            gate: None,
        }
    }

    fn reach(&mut self, pts: i64) {
        if let Some(g) = self.gate.take() {
            g.pass();
        }
        let n = pts / STEP;
        let from = match self.cursor {
            Some(c) if c <= n => c + 1,
            at => {
                self.counts.lock().unwrap().reopens += u64::from(at.is_some());
                n - n.rem_euclid(GOP)
            }
        };
        let decoded = (n - from + 1).max(0);
        {
            let mut c = self.counts.lock().unwrap();
            c.pictures += decoded as u64;
            c.served.push(pts);
        }
        std::thread::sleep(self.picture * decoded as u32);
        self.cursor = Some(n);
    }

    /// A moving texture, so the stream's motion search has something to find.
    fn frame(pts: i64, q: Quality) -> VideoFrame {
        let k = (pts / STEP) as u32;
        let mut rgba = vec![255u8; (q.width * q.height * 4) as usize];
        for y in 0..q.height {
            for x in 0..q.width {
                let v = ((x + 2 * k) / 8 * 37 + y / 8 * 91) % 200 + 28;
                let i = ((y * q.width + x) * 4) as usize;
                rgba[i] = v as u8;
                rgba[i + 1] = (v * 3 % 251) as u8;
                rgba[i + 2] = (255 - v) as u8;
            }
        }
        VideoFrame { pts: Tick(pts), width: q.width, height: q.height, rgba: rgba.into_boxed_slice(), has_alpha: false }
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
        Ok(GopSource::frame(p.0, q.unwrap_or(Quality { width: 64, height: 36 })))
    }
    fn keyframe_before(&self, p: Tick) -> Option<Tick> {
        Some(Tick(p.0 - p.0.rem_euclid(GOP * STEP)))
    }
}

fn wait_idle(w: &MediaWorker) {
    let deadline = kt_time::test_deadline();
    while !w.is_idle() {
        assert!(std::time::Instant::now() < deadline, "the worker did not finish the queue");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn video(pts: i64) -> DecodeRequest {
    DecodeRequest {
        kind: RequestKind::video_on_grid(Tick(pts), Some(Quality { width: 64, height: 36 }), STEP),
        prio: Priority::Playback,
        generation: 0,
    }
}

/// t2040: a run of keys queued behind the decoder's cursor is read forward from its start, one
/// seek for the run, not one seek and a decode from the keyframe per key read from its end.
#[test]
fn t2040_a_run_behind_the_cursor_is_read_forward() {
    let _g = serial();
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let counts = Arc::new(Mutex::new(Counts::default()));
    let worker = MediaWorker::spawn(MediaId(1), Box::new(GopSource::new(Duration::ZERO, counts.clone())), cache);
    // the cursor goes to frame 400
    worker.submit(video(400 * STEP));
    wait_idle(&worker);
    let before = std::mem::take(&mut *counts.lock().unwrap());
    // then a run of 40 frames far behind it — a stream stage's history asked after the decoder had
    // passed it
    for k in 100..140 {
        worker.submit(video(k * STEP));
    }
    wait_idle(&worker);
    let c = counts.lock().unwrap();
    assert_eq!(before.reopens, 0);
    let mut served = c.served.clone();
    served.sort();
    served.dedup();
    assert_eq!(served.len(), 40, "every key once");
    assert!(c.reopens <= 2, "one seek back to the run's start, then forward: {} reopens", c.reopens);
    assert!(c.pictures <= 40 + GOP as u64, "{} pictures for 40 keys (a seek per key would be ~600)", c.pictures);
    let first_back = c.served.iter().position(|&p| p < 400 * STEP).unwrap();
    assert_eq!(c.served[first_back], 100 * STEP, "the run is read from its start: {:?}", c.served);
    worker.shutdown();
}

/// t2041: a key already queued, or already taken into the span the decoder holds, is not queued
/// again and is not counted as a request; every key is decoded once.
#[test]
fn t2041_a_key_in_flight_or_queued_is_not_a_new_request() {
    let _g = serial();
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let counts = Arc::new(Mutex::new(Counts::default()));
    let gate = Arc::new(Gate::default());
    let mut src = GopSource::new(Duration::ZERO, counts.clone());
    src.gate = Some(gate.clone());
    let worker = MediaWorker::spawn(MediaId(1), Box::new(src), cache);
    use kt_time::work::{Work, count};
    let r0 = count(Work::DecodeRequest);
    // keys 0..4 go into the decoder's first span, which waits at the gate
    for k in 0..4 {
        worker.submit(video(k * STEP));
    }
    gate.wait_busy();
    assert_eq!(count(Work::DecodeRequest) - r0, 4);
    // the preview re-asks its whole frame on every composite: fifty composites of keys 0..20
    for _ in 0..50 {
        for k in 0..20 {
            worker.submit(video(k * STEP));
        }
    }
    let asked = count(Work::DecodeRequest) - r0;
    gate.release();
    wait_idle(&worker);
    assert_eq!(asked, 20, "four in flight + sixteen queued once, not 1004");
    let c = counts.lock().unwrap();
    let mut served = c.served.clone();
    served.sort();
    assert_eq!(served, (0..20).map(|k| k * STEP).collect::<Vec<_>>(), "every key decoded once");
    worker.shutdown();
}

// --- the stream stage ---------------------------------------------------------------------------

/// The stage's encoder needs the host's compute device: skip without one, as the GPU tests do.
fn no_device() -> bool {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return true;
    }
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    let none = kt_plugin_api::compute::host().is_none();
    if none {
        assert!(std::env::var("KT_REQUIRE_GPU").as_deref() != Ok("1"), "KT_REQUIRE_GPU=1, but no compute device");
        eprintln!("skipped: no compute device");
    }
    none
}

fn clip(id: u64, media: u64, start: i64, dur: i64, fx: Vec<FxInstance>) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
        masks: Default::default(),
    }
}

/// The classic cut mosh: shot A for 2 s, then shot B for 6 s with `datamosh` at 144 lines — the
/// stream starts 12 frames into A and every I-frame after it is dropped, so a frame at 5 s depends
/// on the ~100 frames since 1.6 s.
fn mosh_project() -> Project {
    kt_plugins_builtin::install();
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.media.push(MediaRef::new(MediaId(2), "synth:2"));
    let mut f = FxInstance::plugin("datamosh").expect("datamosh is built in");
    for (name, v) in [("lines", 0.0), ("before", 12.0), ("mosh", 1.0), ("gop", 12.0), ("qp", 14.0)] {
        assert!(f.set_param(name, v), "{name}");
    }
    p.clips.push(clip(1, 1, 0, 2 * SEC, vec![]));
    p.clips.push(clip(2, 2, 2 * SEC, 6 * SEC, vec![f]));
    p.validate().unwrap();
    p
}

const Q: Quality = Quality { width: 160, height: 90 };

fn renderer(p: &Project, pacing: Option<StreamPacing>) -> VideoRenderer {
    let mut vr = VideoRenderer::new(p);
    vr.set_stream_memo(Arc::new(StreamMemo::new(256 << 20)));
    vr.set_stream_pacing(pacing);
    vr
}

/// Warm what `order` names from model sources.
fn warm(order: &[(MediaId, Tick, Quality)], cache: &BurstCache) {
    for &(m, pts, q) in order {
        let f = GopSource::frame(pts.0, q);
        cache.insert_video(kt_engine::source_id(m), Some(q), kt_engine::video_cache_idx(pts), Arc::new(f));
    }
}

fn big_cache() -> BurstCache {
    BurstCache::new(CacheConfig { max_audio_bytes: 1 << 20, max_video_bytes: 1 << 30 })
}

/// t2042: under the preview's pacing the order of a stream layer is the EARLIEST of the history it
/// lacks, as many frames as fit the budget (at least one) — the frames the stage reads next.
#[test]
fn t2042_the_paced_order_is_the_earliest_of_the_history() {
    let _g = serial();
    if no_device() {
        return;
    }
    let p = mosh_project();
    let t = Tick(5 * SEC);
    let whole = renderer(&p, None).layers_at(t, Q);
    assert!(whole.len() > 90, "scene guard: a long history ({})", whole.len());
    let q = whole[0].2;
    let frame = q.width as usize * q.height as usize * 4;
    for n in [1usize, 7, 30] {
        let paced = renderer(&p, Some(StreamPacing { order_bytes: n * frame, compose_steps: 2, hold: false })).layers_at(t, Q);
        assert_eq!(paced, whole[..n], "{n} frames of budget: the first {n} of the history");
    }
    let tiny = renderer(&p, Some(StreamPacing { order_bytes: 1, compose_steps: 2, hold: false })).layers_at(t, Q);
    assert_eq!(tiny, whole[..1], "a budget below one frame still asks one");
}

/// t2043: a paced composite does not compute a history (that is the UI thread): with every frame
/// of it cached the frame comes out incomplete; `stream_advance` (the prefetch thread) computes
/// it, after which the composite is complete and its bytes are the unpaced renderer's.
#[test]
fn t2043_the_advance_computes_the_history_not_the_composite() {
    let _g = serial();
    if no_device() {
        return;
    }
    let p = mosh_project();
    let t = Tick(5 * SEC);
    let (plain, cache) = (renderer(&p, None), big_cache());
    warm(&plain.layers_at(t, Q), &cache);
    let (want, complete) = kt_gpu::testkit::compose(&plain, &cache, t, Q);
    assert!(complete);

    let paced = renderer(&p, Some(StreamPacing::preview(1 << 30)));
    assert!(!kt_gpu::testkit::compose(&paced, &cache, t, Q).1, "the composite left the history alone");
    assert!(!paced.layers_at(t, Q).is_empty(), "nothing was computed");
    assert!(!paced.stream_advance(&cache, t, Q), "the advance had every frame it needed");
    assert!(paced.layers_at(t, Q).is_empty(), "the memo holds the frame");
    let (got, complete) = kt_gpu::testkit::compose(&paced, &cache, t, Q);
    assert!(complete);
    assert_eq!(got, want, "the same bytes as the unpaced renderer");
    // the next frame is one step: the composite takes it itself
    let next = Tick(t.0 + STEP);
    warm(&paced.layers_at(next, Q), &cache);
    assert!(kt_gpu::testkit::compose(&paced, &cache, next, Q).1, "a step of playback is the composite's");
}

/// t2044: a short loop over a cut mosh whose history (~100 frames, 15 MB) does not fit a 4 MB
/// video cache, played cold through the real player on model decoders: the loop shows frames,
/// no decoded frame is dropped before it is read, and the decoders do bounded work per frame of
/// history. Before decode-request-storm the whole history was ordered at once, the decoder ran
/// through it while the stage read only at composites, the cache dropped the front of it, and
/// the loop stayed at V-fps 0 while the decoder sought back frame by frame.
#[test]
fn t2044_a_short_loop_over_a_mosh_plays() {
    let _g = serial();
    if no_device() {
        return;
    }
    let p = mosh_project();
    let history = renderer(&p, None).layers_at(Tick(5 * SEC), Q).len() as u64;
    let counts = Arc::new(Mutex::new(Counts::default()));
    let sources: Vec<(MediaId, Box<dyn MediaSource>)> = vec![
        (MediaId(1), Box::new(GopSource::new(Duration::from_millis(2), counts.clone()))),
        (MediaId(2), Box::new(GopSource::new(Duration::from_millis(2), counts.clone()))),
    ];
    let (player, ring) =
        Player::with_cache(p, sources, CacheConfig { max_audio_bytes: 16 << 20, max_video_bytes: 4 << 20 });
    player.set_preview_quality(Q);
    player.set_video_gpu(Some(kt_gpu::testkit::engine()));
    let r = play_range(
        &player,
        ring,
        BenchOptions {
            from: Tick(5 * SEC),
            to: Tick(5 * SEC + 8 * STEP),
            settle: Duration::ZERO,
            loop_for: Some(Duration::from_secs(10)),
            ..Default::default()
        },
    );
    let c = counts.lock().unwrap();
    eprintln!(
        "history {history}: shown {}, incomplete {}, dropped unread {}, pictures {}, reopens {}",
        r.shown, r.incomplete, r.unused_evictions, c.pictures, c.reopens
    );
    assert!(r.shown > 0, "the loop shows frames: V-fps 0 ({} incomplete composites)", r.incomplete);
    assert_eq!(r.unused_evictions, 0, "every decoded frame is read before the cache drops it");
    assert!(
        c.pictures <= 4 * (history + 8 * GOP as u64),
        "{} pictures for a history of {history}: the decoder seeks in a circle",
        c.pictures
    );
}

// ---- structured-telemetry -----------------------------------------------------------------------

use kt_time::trace::{self, Ev, Rec};

/// A player over the mosh project on model decoders, the test kit's executor, paused at `t`.
fn mosh_player(cache: CacheConfig) -> (Player, rtrb::Consumer<f32>) {
    let counts = Arc::new(Mutex::new(Counts::default()));
    let sources: Vec<(MediaId, Box<dyn MediaSource>)> = vec![
        (MediaId(1), Box::new(GopSource::new(Duration::from_millis(1), counts.clone()))),
        (MediaId(2), Box::new(GopSource::new(Duration::from_millis(1), counts))),
    ];
    let (player, ring) = Player::with_cache(mosh_project(), sources, cache);
    player.set_preview_quality(Q);
    player.set_video_gpu(Some(kt_gpu::testkit::engine()));
    (player, ring)
}

const SHOW: kt_engine::PresentOptions = kt_engine::PresentOptions { show_incomplete: false, glow: false };

/// Present `t` until it comes out complete (the prefetch thread computes the history meanwhile).
fn present_until_complete(player: &Player, t: Tick) {
    let deadline = kt_time::test_deadline();
    // the prefetch thread advances the history of the frame at the playhead
    player.seek(t);
    loop {
        if player.present_preview(t, SHOW).is_some_and(|(_, c)| c) {
            player.finish_present();
            return;
        }
        assert!(std::time::Instant::now() < deadline, "the frame at {t:?} never came");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn of(items: &[trace::Item], e: Ev) -> Vec<Rec> {
    trace::testing::recs(items).into_iter().filter(|r| r.kind() == Some(e)).collect()
}

/// t2144: one preview frame's life is joined across threads by its id: asked on the compose
/// thread, it waits for a decode whose request names the frame as its cause, the media worker
/// decodes that key on its own thread, and the plan, the executor and the answer name the frame.
#[test]
fn t2144_a_frame_is_joined_to_the_decode_it_waited_for() {
    let _g = serial();
    if no_device() {
        return;
    }
    let (player, _ring) = mosh_player(CacheConfig::default());
    // the plain event before the mosh: one layer, one decode
    let t = Tick(SEC);
    // the playhead stands past it, so the prefetch's window (which starts at the frame on screen)
    // never orders that key first: since one-scheduler the quality set by `mosh_player` wakes the
    // prefetch at once, and the test had relied on its first pass coming 100 ms later
    player.seek(Tick(7 * SEC));
    let mut seq = 0;
    let items = trace::testing::capture(|| {
        seq = player.ask_preview(t, Some(SHOW));
        let deadline = kt_time::test_deadline();
        loop {
            if player.take_composed(Duration::from_millis(50)).is_some_and(|c| c.seq == seq && c.complete) {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "the frame never came");
            seq = player.ask_preview(t, Some(SHOW));
        }
    });
    let asks: Vec<u64> = of(&items, Ev::FrameAsk).iter().filter(|r| r.a == t.0 as u64).map(|r| r.id).collect();
    let first = *asks.first().expect("the ask is recorded");
    let wait = of(&items, Ev::FrameWait)
        .into_iter()
        .find(|w| w.id == first && w.aux == trace::wait::DECODE)
        .expect("the first composite waits for a decode");
    let req = of(&items, Ev::DecReq).into_iter().find(|r| r.id == wait.a).expect("its request");
    assert_eq!(req.cause, first, "the request names the frame that made it");
    let dec = of(&items, Ev::Decoded).into_iter().find(|r| r.id == wait.a).expect("the key is decoded");
    assert_ne!(dec.th, req.th, "the decode is the worker's, on another thread");
    assert!(dec.t >= req.t);
    let done = of(&items, Ev::FrameDone).into_iter().find(|d| d.id == seq).expect("the answer");
    assert_eq!(done.aux, 1, "complete");
    assert!(of(&items, Ev::FramePlan).iter().any(|p| p.id == seq) && of(&items, Ev::FrameExec).iter().any(|p| p.id == seq));
    let compose_th = of(&items, Ev::FramePlan).iter().find(|p| p.id == seq).unwrap().th;
    assert_eq!(compose_th, req.th, "the request went out from the compose thread");
}

/// t2145 (kkt-26.11.09 I8): a moment whose composite the executor keeps is shown from it with no
/// stream stage run, no decode asked and no plan of the chain, even after the stage's history memo
/// and the cache dropped everything; before, the plan of a kept frame ran the stage, found its
/// history gone and the frame came out incomplete ("Loading").
#[test]
fn t2145_a_kept_moment_needs_no_history() {
    let _g = serial();
    if no_device() {
        return;
    }
    let (player, _ring) = mosh_player(CacheConfig { max_audio_bytes: 16 << 20, max_video_bytes: 256 << 20 });
    let t = Tick(5 * SEC);
    present_until_complete(&player, t);
    // the history and the source frames are gone
    kt_engine::video::stream_memo().clear();
    for m in [1, 2] {
        player.cache.invalidate_source(kt_engine::source_id(MediaId(m)));
    }
    let mut shown = None;
    let items = trace::testing::capture(|| shown = player.present_preview(t, SHOW));
    assert_eq!(shown.map(|s| s.1), Some(true), "the kept moment is complete at once");
    let execs = of(&items, Ev::FrameExec);
    assert!(execs.iter().any(|e| e.aux & trace::exec::MEMO_HIT != 0), "shown from the kept composite: {execs:?}");
    let frame = execs.iter().find(|e| e.aux & trace::exec::MEMO_HIT != 0).unwrap().id;
    assert!(!of(&items, Ev::StreamRun).iter().any(|r| r.cause == frame), "no stream stage ran for it");
    assert!(!of(&items, Ev::DecReq).iter().any(|r| r.cause == frame), "no decode was asked for it");
    assert!(!of(&items, Ev::FrameWait).iter().any(|r| r.id == frame), "it waited for nothing");
    assert!(player.final_video_runs().0.iter().any(|r| r.contains(&t.0)), "the bars show the moment as kept");
}

/// t2150: the look-ahead asks nothing for the moments whose composite is kept, and asks the rest
/// as before.
#[test]
fn t2150_the_look_ahead_skips_kept_moments() {
    let _g = serial();
    let p = mosh_project();
    let r = renderer(&p, None);
    let t0 = Tick(SEC);
    let all = kt_engine::video_prefetch_wants(&r, t0, Q, None, 1 << 30);
    let kept = |t: Tick| t.0 < SEC + 10 * STEP;
    let some = kt_engine::video_prefetch_wants_except(&r, t0, Q, None, 1 << 30, &kept);
    assert!(!all.is_empty() && some.len() < all.len());
    let kept_keys: Vec<_> = (0..10).flat_map(|k| r.layers_at(Tick(SEC + k * STEP), Q)).collect();
    assert!(some.iter().all(|k| !kept_keys.contains(k)), "a kept moment's keys are not asked");
    assert!(all.iter().filter(|k| !kept_keys.contains(k)).all(|k| some.contains(k)), "the rest is asked as before");
}

/// The slow-chain device of t2151: the test kit's executor, past `SLOW_CHAIN` per frame.
struct Slow(Arc<dyn kt_engine::VideoGpuExec>);

impl kt_engine::VideoGpuExec for Slow {
    fn execute_into(&self, plan: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, kt_engine::GpuExecError> {
        std::thread::sleep(kt_engine::compose_queue::SLOW_CHAIN * 2);
        self.0.execute_into(plan, out)
    }
    fn info(&self) -> &kt_engine::GpuBackendInfo {
        self.0.info()
    }
    fn present_key(&self, plan: &FramePlan<'_>) -> Option<u64> {
        self.0.present_key(plan)
    }
}

/// t2151: a moment composed on the slow-chain device (bytes) is kept on the display's executor: the
/// next ask of it is shown on the display target from the kept composite, with no chain composed,
/// and the route stays on the slow device for what is not kept.
#[test]
fn t2151_the_slow_route_keeps_its_composites() {
    let _g = serial();
    if no_device() {
        return;
    }
    let (player, _ring) = mosh_player(CacheConfig::default());
    (player.slow_gpu_setter())(Some(Arc::new(Slow(kt_gpu::testkit::engine()))));
    let t = Tick(SEC);
    let deadline = kt_time::test_deadline();
    let ask = |t: Tick| {
        let seq = player.ask_preview(t, Some(SHOW));
        loop {
            if let Some(c) = player.take_composed(Duration::from_millis(100)).filter(|c| c.seq == seq) {
                break c;
            }
            assert!(std::time::Instant::now() < deadline, "the frame never came");
        }
    };
    let first = loop {
        let c = ask(t);
        if c.complete {
            break c;
        }
    };
    assert!(!first.on_display, "a slow chain composes on the slow device, as bytes");
    let r0 = kt_time::work::count(kt_time::work::Work::CompositeReplay);
    let again = ask(t);
    assert!(again.complete && again.on_display, "shown on the display from the kept composite");
    assert_eq!(kt_time::work::count(kt_time::work::Work::CompositeReplay) - r0, 1);
    let other = loop {
        let c = ask(Tick(SEC + STEP));
        if c.complete {
            break c;
        }
    };
    assert!(!other.on_display, "a moment not kept still goes to the slow device: the replay timed no chain");
}

// ---- gpu-arbiter ---------------------------------------------------------------------------------

/// A stop check that says stop from its `after`-th question on.
fn stop_after(after: u32) -> impl Fn() -> bool + 'static {
    let asked = std::sync::atomic::AtomicU32::new(0);
    move || asked.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1 >= after
}

/// t2258: a frame's stream history run under the compose thread's stop check stops once the check
/// says nobody wants the frame (a newer ask, an edit): the frame comes out incomplete, the stop is in
/// the trace by its owner, the memo kept the positions computed before it, and the next composite
/// resumes to the bytes of an uninterrupted one.
#[test]
fn t2258_the_preview_stops_a_history_it_left() {
    let _g = serial();
    if no_device() {
        return;
    }
    let p = mosh_project();
    let t = Tick(5 * SEC);
    let (plain, cache) = (renderer(&p, None), big_cache());
    warm(&plain.layers_at(t, Q), &cache);
    let (want, _) = kt_gpu::testkit::compose(&renderer(&p, None), &cache, t, Q);
    let history = plain.layers_at(t, Q).len();
    let items = trace::testing::capture(|| {
        let (got, complete) =
            kt_engine::stop_when(trace::stop::COMPOSE, stop_after(6), || kt_gpu::testkit::compose(&plain, &cache, t, Q));
        assert!(!complete, "a history nobody wanted ran to its end");
        drop(got);
    });
    let stops: Vec<Rec> = of(&items, Ev::StreamStop).into_iter().filter(|r| r.aux == trace::stop::COMPOSE).collect();
    assert_eq!(stops.len(), 1, "the stop is not in the trace");
    assert!(plain.layers_at(t, Q).len() < history, "nothing computed before the stop was kept");
    let (got, complete) = kt_gpu::testkit::compose(&plain, &cache, t, Q);
    assert!(complete);
    assert_eq!(got, want, "the resumed history's frame differs from an uninterrupted one's");
}

/// t2259: the look-ahead's advance stops once the playhead leaves it: `stream_advance` under the
/// prefetch thread's stop check returns "still pending" at the next position and says why in the
/// trace; a later advance with no stop finishes it.
#[test]
fn t2259_the_look_ahead_stops_when_the_playhead_leaves() {
    let _g = serial();
    if no_device() {
        return;
    }
    let p = mosh_project();
    let t = Tick(5 * SEC);
    let (paced, cache) = (renderer(&p, Some(StreamPacing::preview(1 << 30))), big_cache());
    warm(&renderer(&p, None).layers_at(t, Q), &cache);
    let history = paced.layers_at(t, Q).len();
    let items = trace::testing::capture(|| {
        let pending = kt_engine::stop_when(trace::stop::PREFETCH, stop_after(4), || paced.stream_advance(&cache, t, Q));
        assert!(pending, "a stopped advance said it was done");
    });
    assert_eq!(of(&items, Ev::StreamStop).iter().filter(|r| r.aux == trace::stop::PREFETCH).count(), 1);
    let advances = of(&items, Ev::StreamAdvance);
    assert!(advances.iter().any(|r| r.aux == trace::stream::STOPPED), "the advance's span does not say it stopped");
    let left = paced.layers_at(t, Q).len();
    assert!(left > 0 && left < history, "{left} of {history}: the stop came at the wrong place");
    assert!(!paced.stream_advance(&cache, t, Q), "a free advance did not finish");
    assert!(kt_gpu::testkit::compose(&paced, &cache, t, Q).1);
}

// ---- stream-history-on-device ---------------------------------------------------------------------

/// t2360 (found-defects 391): the frames ahead of the one on screen are one walk — the history from the
/// chain's anchor and the nine frames come in at most a submission per batch, and every one of the
/// nine then needs nothing.
#[test]
fn t2360_frames_ahead_are_one_walk() {
    let _g = serial();
    if no_device() {
        return;
    }
    let p = mosh_project();
    let t = Tick(5 * SEC);
    let to = Tick(t.0 + 8 * STEP);
    let (vr, cache) = (renderer(&p, Some(StreamPacing::preview(1 << 30))), big_cache());
    let plain = renderer(&p, None);
    let mut m = t;
    while m.0 <= to.0 {
        warm(&plain.layers_at(m, Q), &cache);
        m = Tick(m.0 + STEP);
    }
    vr.set_stream_view(Some(t));
    let items = trace::testing::capture(|| {
        assert!(!vr.stream_advance_run(&cache, t, to, Q), "every frame was cached");
    });
    let walks = of(&items, Ev::StreamWalk);
    let positions: u64 = walks.iter().map(|r| r.b).sum();
    assert!(positions >= 9, "the walk covered the nine frames ({positions})");
    assert!(
        walks.len() as u64 <= positions.div_ceil(32) + 1, // the codec's batch, `fx_video_codec::walk::MAX_BATCH`
        "{} submissions for {positions} positions",
        walks.len()
    );
    let mut m = t;
    while m.0 <= to.0 {
        assert!(vr.layers_at(m, Q).is_empty(), "the frame at {} still needs its history", m.0);
        m = Tick(m.0 + STEP);
    }
}
