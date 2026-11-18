//! decode-request-storm: a request lives until it is served or no longer wanted, the decoder reads
//! forward, and a stream stage's history is ordered no faster than it is read. t2040 (a run behind
//! the cursor is read forward), t2041 (a key queued or in flight is not a new request), t2042 (the
//! paced order is the earliest of the history), t2043 (a composite does not compute a history; the
//! advance does, to the same bytes), t2044 (a short loop over a cut mosh whose history does not fit
//! the cache plays, every decoded frame read).
//! catch-up-shows-loading: t2410–t2414 (a frame behind its history is incomplete and says how far,
//! never a held picture with later stages over it; an export composes from its own positions).
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
    // the classic soft mosh these tests were written for: its stream is ordered at the codec's size (a
    // new Datamosh is sharp since kkt-26.11.18b U7; the order and the walk are the same code either way)
    for (name, v) in [("lines", 0.0), ("before", 12.0), ("mosh", 1.0), ("gop", 12.0), ("qp", 14.0), ("sharp", 0.0)] {
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
        let paced = renderer(&p, Some(StreamPacing { order_bytes: n * frame, compose_steps: 2 })).layers_at(t, Q);
        assert_eq!(paced, whole[..n], "{n} frames of budget: the first {n} of the history");
    }
    let tiny = renderer(&p, Some(StreamPacing { order_bytes: 1, compose_steps: 2 })).layers_at(t, Q);
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

/// t2459 (class-walk-revision): the preview bench names a frame behind its stream history — a cold
/// start over the mosh comes out incomplete while the history catches up, and the report says by how
/// many positions. Since catch-up-shows-loading such a frame misses no decode, so the bench's only cause
/// (`starved`, the missing layers) named nothing, and `kt-cli preview`/`pause-scan` printed no cause.
#[test]
fn t2459_the_bench_names_a_frame_behind_its_history() {
    let _g = serial();
    if no_device() {
        return;
    }
    let (player, ring) = mosh_player(CacheConfig { max_audio_bytes: 16 << 20, max_video_bytes: 256 << 20 });
    let r = play_range(
        &player,
        ring,
        BenchOptions { from: Tick(5 * SEC), to: Tick(5 * SEC + 8 * STEP), settle: Duration::ZERO, ..Default::default() },
    );
    assert!(r.incomplete > 0, "scene guard: the cold start's composites were incomplete");
    assert!(!r.catching_up.is_empty(), "{} incomplete composites, none named as catching up", r.incomplete);
    assert!(r.catching_up.iter().all(|&(_, n)| n > 0));
}


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

const SHOW: kt_engine::PresentOptions = kt_engine::PresentOptions { glow: false };

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

/// Frames the slow device composed.
static SLOW_RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl kt_engine::VideoGpuExec for Slow {
    fn execute_into(&self, plan: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, kt_engine::GpuExecError> {
        SLOW_RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
///
/// t2705 (stream-round-trips D7, found-defects 622): the slow device's picture, once kept on the
/// display's executor, is shown from there at once — on the display, not uploaded again as bytes.
#[test]
fn t2151_t2705_the_slow_route_keeps_its_composites() {
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
    let runs = |_: ()| SLOW_RUNS.load(std::sync::atomic::Ordering::Relaxed);
    assert!(runs(()) > 0, "a slow chain composes on the slow device");
    assert!(first.on_display, "the slow device's picture is shown from the display's memo, not uploaded again (t2705)");
    let (r0, s0) = (kt_time::work::count(kt_time::work::Work::CompositeReplay), runs(()));
    let again = ask(t);
    assert!(again.complete && again.on_display, "shown on the display from the kept composite");
    assert_eq!(kt_time::work::count(kt_time::work::Work::CompositeReplay) - r0, 1);
    assert_eq!(runs(()), s0, "the replay composed nothing on the slow device");
    let other = loop {
        let c = ask(Tick(SEC + STEP));
        if c.complete {
            break c;
        }
    };
    assert!(runs(()) > s0, "a moment not kept still goes to the slow device: the replay timed no chain");
    assert!(other.on_display, "and is shown from the display's memo (t2705)");
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

/// t2451 (class-walk-revision): the memo's view is the player's renderer's, a tick of ITS timeline: a
/// renderer sharing the memo that never set it (a reel's renderer, a strip job, a poster) reads every
/// position of its walk back. Before, the player's view at 8 s kept a walk of 5 s from reading any
/// picture back, and every frame of it stayed short.
#[test]
fn t2451_a_renderer_that_set_no_view_reads_its_walk_back() {
    let _g = serial();
    if no_device() {
        return;
    }
    let p = mosh_project();
    let t = Tick(5 * SEC);
    let to = Tick(t.0 + 8 * STEP);
    let memo = Arc::new(StreamMemo::new(256 << 20));
    let mut player = VideoRenderer::new(&p);
    player.set_stream_memo(memo.clone());
    let mut strip = VideoRenderer::new(&p);
    strip.set_stream_memo(memo);
    strip.set_stream_pacing(Some(StreamPacing::preview(1 << 30)));
    let cache = big_cache();
    let plain = renderer(&p, None);
    let mut m = t;
    while m.0 <= to.0 {
        warm(&plain.layers_at(m, Q), &cache);
        m = Tick(m.0 + STEP);
    }
    player.set_stream_view(Some(Tick(8 * SEC)));
    assert!(!strip.stream_advance_run(&cache, t, to, Q), "every frame was cached");
    let mut m = t;
    while m.0 <= to.0 {
        assert!(strip.layers_at(m, Q).is_empty(), "the frame at {} lacks its history: the player's view applied", m.0);
        m = Tick(m.0 + STEP);
    }
}

// ---- catch-up-shows-loading -------------------------------------------------------------------

/// The mosh project with VHS Tape after Datamosh on the event and as its track's stage: the stages the
/// owner saw laid over a held picture (kkt-26.11.10b K5).
fn mosh_vhs_project() -> Project {
    let mut p = mosh_project();
    p.clips[1].fx.push(FxInstance::plugin("vhs_tape").expect("vhs_tape is built in"));
    // the first video track (a new project may carry default tracks)
    match p.tracks.iter_mut().find(|t| t.kind == LaneKind::Video) {
        Some(t) => t.fx.push(FxInstance::plugin("vhs_tape").unwrap()),
        None => {
            let mut track = Track::new(TrackId(1), LaneKind::Video);
            track.fx.push(FxInstance::plugin("vhs_tape").unwrap());
            p.tracks.push(track);
        }
    }
    p.validate().unwrap();
    p
}

/// Every source a plan's layers read a picture of, through reels and track stages.
fn sources_with_picture(plan: &FramePlan<'_>) -> Vec<kt_cache::SourceId> {
    use kt_engine::video_plan::{LayerInput, LayerSrc, PlanLayer};
    fn walk(src: &LayerSrc<'_>, out: &mut Vec<kt_cache::SourceId>) {
        match &src.input {
            LayerInput::Frame(Some(_)) => out.push(src.source),
            LayerInput::Frame(None) => {}
            LayerInput::Nest(n) => {
                for l in &n.plan.layers {
                    layer(l, out);
                }
            }
        }
    }
    fn layer(l: &PlanLayer<'_>, out: &mut Vec<kt_cache::SourceId>) {
        match l {
            PlanLayer::One { src, .. } => walk(src, out),
            PlanLayer::Dissolve { a, b, .. } => {
                walk(a, out);
                walk(b, out);
            }
        }
    }
    let mut out = Vec::new();
    for l in &plan.layers {
        layer(l, &mut out);
    }
    out
}

/// A renderer whose history of the frame at `t` stands ten positions behind it, every source frame
/// cached; and the unpaced renderer's bytes of `t`.
fn behind(p: &Project, t: Tick) -> (VideoRenderer, BurstCache, Vec<u8>) {
    let (vr, cache) = (renderer(p, Some(StreamPacing::preview(1 << 30))), big_cache());
    let plain = renderer(p, None);
    warm(&plain.layers_at(t, Q), &cache);
    let (want, complete) = kt_gpu::testkit::compose(&plain, &cache, t, Q);
    assert!(complete, "scene guard: the unpaced frame composes");
    assert!(!vr.stream_advance(&cache, Tick(t.0 - 10 * STEP), Q), "the history up to ten frames behind");
    (vr, cache, want)
}

/// t2410 (catch-up-shows-loading, found-defects 403): a frame whose stream stage lacks more positions
/// than a composite computes is incomplete, its layer has no picture and the renderer says how far
/// the history is behind; there is no held picture of an earlier position. After the catch-up the
/// frame is the unpaced renderer's.
#[test]
fn t2410_a_frame_behind_its_history_is_loading_not_a_held_picture() {
    let _g = serial();
    if no_device() {
        return;
    }
    let p = mosh_project();
    let t = Tick(5 * SEC);
    let (vr, cache, want) = behind(&p, t);
    let mosh = kt_engine::source_id(MediaId(2));
    {
        let plan = vr.frame_plan(&cache, t, Q);
        assert!(!sources_with_picture(&plan).contains(&mosh), "the mosh layer got a picture of another position");
    }
    assert!(vr.stream_short() >= 8, "the frame lacks the ten positions ({})", vr.stream_short());
    let (_, complete) = kt_gpu::testkit::compose(&vr, &cache, t, Q);
    assert!(!complete, "a frame short of its history is incomplete");
    assert!(!vr.stream_advance(&cache, t, Q));
    let (got, complete) = kt_gpu::testkit::compose(&vr, &cache, t, Q);
    assert!(complete && vr.stream_short() == 0, "the caught-up frame is complete and nothing is short");
    assert_eq!(got, want, "the moment's own picture");
}

/// t2411 (catch-up-shows-loading, kkt-26.11.10b K5): VHS Tape after Datamosh and VHS Tape as the
/// track's stage never run over another position's mosh: the short frame has no mosh picture anywhere
/// in its plan and is incomplete; the caught-up frame is the unpaced renderer's bytes.
#[test]
fn t2411_vhs_is_never_laid_over_another_positions_mosh() {
    let _g = serial();
    if no_device() {
        return;
    }
    let p = mosh_vhs_project();
    let t = Tick(5 * SEC);
    let (vr, cache, want) = behind(&p, t);
    let mosh = kt_engine::source_id(MediaId(2));
    {
        let plan = vr.frame_plan(&cache, t, Q);
        assert!(
            plan.layers.iter().any(|l| matches!(l, kt_engine::video_plan::PlanLayer::One { src, .. } if src.input.nest().is_some())),
            "scene guard: the track stage is a sub-composite"
        );
        assert!(!sources_with_picture(&plan).contains(&mosh), "a stage got another position's mosh");
    }
    assert!(vr.stream_short() >= 8, "the track stage's sub-plan reports the shortage ({})", vr.stream_short());
    assert!(!kt_gpu::testkit::compose(&vr, &cache, t, Q).1, "the short frame is incomplete");
    assert!(!vr.stream_advance(&cache, t, Q));
    let (got, complete) = kt_gpu::testkit::compose(&vr, &cache, t, Q);
    assert!(complete);
    assert_eq!(got, want, "VHS over the moment's own mosh");
}

/// t2412 (catch-up-shows-loading): an export composes every frame from its own positions — the
/// serial export of a range of the mosh with VHS Tape after it equals the unpaced renderer's frames
/// at each moment, none short.
#[test]
fn t2412_an_export_composes_every_frame_from_its_own_positions() {
    let _g = serial();
    if no_device() {
        return;
    }
    let p = mosh_vhs_project();
    let (from, to) = (Tick(5 * SEC), Tick(5 * SEC + 4 * STEP));
    let counts = Arc::new(Mutex::new(Counts::default()));
    let sources: Vec<(MediaId, Box<dyn MediaSource>)> = vec![
        (MediaId(1), Box::new(GopSource::new(Duration::ZERO, counts.clone()))),
        (MediaId(2), Box::new(GopSource::new(Duration::ZERO, counts))),
    ];
    let mut frames: Vec<Vec<u8>> = Vec::new();
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    kt_gpu::testkit::render_range(&p, sources, Q, Some((from, to)), &cancel, |_| {}, |f| {
        frames.push(f.to_vec());
        Ok(())
    })
    .expect("export");
    assert_eq!(frames.len(), 4, "four frames in the range");
    let (plain, cache) = (renderer(&p, None), big_cache());
    for (i, got) in frames.iter().enumerate() {
        let t = Tick(from.0 + i as i64 * STEP);
        warm(&plain.layers_at(t, Q), &cache);
        let (want, complete) = kt_gpu::testkit::compose(&plain, &cache, t, Q);
        assert!(complete && plain.stream_short() == 0, "frame {i}: the unpaced renderer is never short");
        assert!(*got == want, "frame {i}: the export's frame is not the moment's own");
    }
}

/// t2413 (catch-up-shows-loading): a reel holding the mosh reports its catch-up to the root: the root
/// composite is incomplete with the inner positions short, then complete with nothing short.
#[test]
fn t2413_a_reel_reports_its_catch_up_to_the_root() {
    let _g = serial();
    if no_device() {
        return;
    }
    let inner = mosh_project();
    let mut p = inner.clone();
    p.clips.clear();
    let mut seq = Sequence::new(MediaId(100), "mosh reel");
    seq.clips = inner.clips.clone();
    p.sequences.push(seq);
    p.clips.push(clip(10, 100, 0, 8 * SEC, vec![]));
    p.validate().unwrap();
    let t = Tick(5 * SEC);
    let (vr, cache, want) = behind(&p, t);
    assert!(!kt_gpu::testkit::compose(&vr, &cache, t, Q).1, "the root frame is incomplete");
    assert!(vr.stream_short() >= 8, "the reel's shortage reaches the root ({})", vr.stream_short());
    assert!(!vr.stream_advance(&cache, t, Q));
    let (got, complete) = kt_gpu::testkit::compose(&vr, &cache, t, Q);
    assert!(complete && vr.stream_short() == 0);
    assert_eq!(got, want);
}

/// t2414 (catch-up-shows-loading): a playing player answers a frame whose history cannot advance with
/// an incomplete composite that says how far it is behind; a kept moment is answered complete at once,
/// with nothing to catch up, without its history.
#[test]
fn t2414_the_player_answers_the_catch_up_and_a_kept_moment_at_once() {
    let _g = serial();
    if no_device() {
        return;
    }
    let gate = Arc::new(Gate::default());
    let counts = Arc::new(Mutex::new(Counts::default()));
    let gated = |c: &Arc<Mutex<Counts>>| {
        let mut s = GopSource::new(Duration::from_millis(1), c.clone());
        s.gate = Some(gate.clone());
        Box::new(s) as Box<dyn MediaSource>
    };
    let sources = vec![(MediaId(1), gated(&counts)), (MediaId(2), gated(&counts))];
    let (player, _ring) =
        Player::with_cache(mosh_project(), sources, CacheConfig { max_audio_bytes: 16 << 20, max_video_bytes: 256 << 20 });
    player.set_preview_quality(Q);
    player.set_video_gpu(Some(kt_gpu::testkit::engine()));
    let t = Tick(5 * SEC);
    player.seek(t);
    player.play();
    // the decoders stand: the history cannot move, the composite is short whatever the timing
    player.ask_preview(t, Some(SHOW));
    let done = player.take_composed(Duration::from_secs(600)).expect("a composite");
    assert!(!done.complete, "a frame short of its history is incomplete while playing");
    assert!(done.catching_up > 2, "it says how far the history is behind ({})", done.catching_up);
    gate.release();
    player.pause();
    present_until_complete(&player, t);
    kt_engine::video::stream_memo().clear();
    for m in [1, 2] {
        player.cache.invalidate_source(kt_engine::source_id(MediaId(m)));
    }
    player.play();
    player.ask_preview(t, Some(SHOW));
    let done = player.take_composed(Duration::from_secs(600)).expect("a composite");
    assert!(done.complete && done.catching_up == 0, "the kept moment shows at once ({})", done.catching_up);
    player.pause();
}

/// t2432 (gpu-throughput D3, found-defects 482, the owner's "compute ahead" (kkt-26.11.11 L4)): with a loop region over a mosh
/// and every source frame cached, ONE pass of the prefetch's far look-ahead — paused and playing —
/// walks the history to the region's last frame, at class background; a pass with the history complete
/// runs nothing.
#[test]
fn t2432_a_loop_is_computed_to_its_end() {
    let _g = serial();
    if no_device() {
        return;
    }
    let p = mosh_project();
    let (s, e) = (3 * SEC, 4 * SEC);
    for playing in [false, true] {
        let (vr, cache) = (renderer(&p, Some(StreamPacing::preview(1 << 30))), big_cache());
        let plain = renderer(&p, None);
        let mut m = Tick(2 * SEC);
        while m.0 < e {
            warm(&plain.layers_at(m, Q), &cache);
            m = Tick(m.0 + STEP);
        }
        let t = Tick(s);
        vr.set_stream_view(Some(t));
        let far = kt_engine::player::stream_far_frames(&vr, t, Q, Some((s, e)), playing);
        assert!(far as i64 >= (e - s) / STEP, "the far part does not reach the loop's end: {far} frames");
        assert_eq!(kt_engine::player::stream_far_frames(&vr, t, Q, None, false), 0, "paused outside a loop walks nothing ahead");
        let items = trace::testing::capture(|| {
            let _class = kt_time::sched::with_class(kt_time::sched::Class::Background);
            assert!(!kt_engine::player::stream_advance_ahead(&vr, &cache, t, Q, Some((s, e)), far, &|_| false), "every frame was cached");
        });
        let jobs = of(&items, Ev::ComputeJob);
        assert!(!jobs.is_empty(), "the walk ran no job");
        assert!(jobs.iter().all(|r| r.id == kt_time::sched::Class::Background as u64), "a far walk ran above the lowest class");
        let mut m = t;
        while m.0 < e {
            assert!(vr.layers_at(m, Q).is_empty(), "playing={playing}: the frame at {} still needs its history", m.0);
            m = Tick(m.0 + STEP);
        }
        let again = trace::testing::capture(|| {
            assert!(!kt_engine::player::stream_advance_ahead(&vr, &cache, t, Q, Some((s, e)), far, &|_| false));
        });
        assert!(of(&again, Ev::ComputeJob).is_empty(), "a complete history ran jobs again");
    }
}

/// t2924 (render-ahead D6, replacing gpu-throughput D3's eight-pass `FAR_STALLS`): a far walk left
/// pending runs again while it walks or while the decoded-frame cache gains frames; one that walked
/// nothing over a pass that decoded nothing stops, and the cache's count moves with every video frame
/// inserted (never with audio).
#[test]
fn t2924_a_stalled_far_walk_waits_for_a_decoded_frame() {
    use kt_engine::player::far_walk_again;
    assert!(far_walk_again(true, true, 5, 5), "a walk that walked goes on");
    assert!(far_walk_again(true, false, 5, 6), "a frame came: try again");
    assert!(!far_walk_again(true, false, 5, 5), "nothing walked, nothing decoded: stop");
    assert!(!far_walk_again(false, true, 5, 6), "a walk with nothing pending is done");
    let p = mosh_project();
    let (vr, cache) = (renderer(&p, None), big_cache());
    let before = cache.video_inserted();
    let layers = vr.layers_at(Tick(2 * SEC), Q);
    assert!(!layers.is_empty(), "the scene: a frame with layers");
    warm(&layers, &cache);
    assert_eq!(cache.video_inserted() - before, layers.len() as u64, "every decoded frame counts once");
    cache.insert_audio(kt_engine::source_id(MediaId(1)), Arc::new(AudioBurst::silence(0)));
    assert_eq!(cache.video_inserted() - before, layers.len() as u64, "audio moved the video count");
}

/// t2710 (stream-round-trips D8, found-defects 660, kkt-26.11.15 N2): playing a piece with no stream
/// stage on screen in a project that has one elsewhere, the far look-ahead walks nothing — it walked
/// 33.5 million frames a pass (26 s of the prefetch thread on the owner's machine, the look-ahead
/// starved) — and inside a stage it walks at most `STREAM_WALK_SECS` of frames whatever the memo holds.
#[test]
fn t2710_no_stage_on_screen_walks_nothing_far() {
    let _g = serial();
    let p = mosh_project();
    let vr = renderer(&p, None);
    let plain = Tick(SEC / 2);
    assert_eq!(vr.stream_frame_bytes(plain, Q), 0, "the scene: no stage at the frame on screen");
    assert_eq!(kt_engine::player::stream_far_frames(&vr, plain, Q, None, true), 0, "a far walk with no stage on screen");
    let inside = Tick(3 * SEC);
    let far = kt_engine::player::stream_far_frames(&vr, inside, Q, None, true);
    assert!(far > 0, "a stage on screen walks ahead");
    assert!(far <= 60 * 30, "{far} frames: more than a minute of the project");
}

/// t2711 (stream-round-trips D10, found-defects 662): the look-ahead's walk of the frame ON SCREEN runs
/// at the display's class — the preview shows Loading until it is walked — and the frames after it at
/// the look-ahead's own class.
#[test]
fn t2711_the_frame_on_screen_walks_at_the_displays_class() {
    let _g = serial();
    if no_device() {
        return;
    }
    let p = mosh_project();
    let (vr, cache) = (renderer(&p, Some(StreamPacing::preview(1 << 30))), big_cache());
    let plain = renderer(&p, None);
    let t = Tick(3 * SEC);
    let mut m = Tick(2 * SEC);
    while m.0 <= t.0 + 8 * STEP {
        warm(&plain.layers_at(m, Q), &cache);
        m = Tick(m.0 + STEP);
    }
    let items = trace::testing::capture(|| {
        let _class = kt_time::sched::with_class(kt_time::sched::Class::Near);
        assert!(!kt_engine::player::stream_advance_ahead(&vr, &cache, t, Q, None, 8, &|_| false), "every frame was cached");
    });
    let jobs = of(&items, Ev::ComputeJob);
    assert!(jobs.len() >= 2, "{} jobs: the frame on screen and the frames after it", jobs.len());
    let (now, near) = (kt_time::sched::Class::Now as u64, kt_time::sched::Class::Near as u64);
    let ids: Vec<u64> = jobs.iter().map(|r| r.id).collect();
    // the history up to the frame on screen (its batches), then the frames after it
    let split = ids.iter().position(|&c| c != now).unwrap_or(ids.len());
    assert!(split > 0, "the frame on screen's walk ran below the display's class: {ids:?}");
    assert!(split < ids.len() && ids[split..].iter().all(|&c| c == near), "the frames ahead ran at another class: {ids:?}");
    assert!(vr.layers_at(Tick(t.0 + 8 * STEP), Q).is_empty(), "the run reached its end");
}

// ---- close-while-busy ----------------------------------------------------------------------------

/// t2952 (close-while-busy, kkt-26.11.18 T1): a decoder in a long span stops at the next chunk once
/// its worker closes, and the close's wait for it is bounded and recorded. Before, the span of 32
/// pictures ran to its end (6.4 s here) and the join waited for all of it — the player's drop, the
/// window's close.
#[test]
fn t2952_a_closing_decoder_stops_between_chunks() {
    let _g = serial();
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let counts = Arc::new(Mutex::new(Counts::default()));
    let worker = MediaWorker::spawn(MediaId(1), Box::new(GopSource::new(Duration::from_millis(200), counts.clone())), cache);
    let items = kt_time::trace::testing::capture(|| {
        for k in 0..32 {
            worker.submit(video(k * STEP));
        }
        let deadline = kt_time::test_deadline();
        while counts.lock().unwrap().served.is_empty() {
            assert!(std::time::Instant::now() < deadline, "the decoder never started");
            std::thread::sleep(Duration::from_millis(1));
        }
        let t0 = std::time::Instant::now();
        worker.shutdown();
        // the bound IS the check: a close that waits for a whole span is the defect
        assert!(t0.elapsed() < kt_time::shutdown::WAIT + Duration::from_secs(1), "the close waited {:?}", t0.elapsed());
    });
    let recs = kt_time::trace::testing::recs(&items);
    let wait = recs
        .iter()
        .find(|r| r.ev == kt_time::trace::Ev::Shutdown as u16 && r.aux == kt_time::trace::name("media-worker"))
        .expect("the close's wait for the decoder is in the trace");
    assert_eq!(wait.b, 0, "the decoder stopped between chunks (it was left running: no stop check in its span)");
    assert!(counts.lock().unwrap().pictures < 32, "the span ran to its end");
}
