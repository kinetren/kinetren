//! render-ahead (review R2; the loop fill's tests before it, scheduler-followups D5, found-defects 251):
//! playing, one producer composes the moments ahead and keeps them on the display's executor, and the
//! display only presents. t2334, t2335, t2336, t2920, t2925. Without a wgpu adapter: skip.

use kt_engine::{FramePlan, GpuBackendInfo, GpuExecError, Player, PresentOptions, VideoGpuExec};
use kt_media::{MediaSource, Quality};
use kt_model::demo::{DemoSources, demo_project};
use kt_time::{Tick, TickRange};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const MOMENTS: i64 = 24;

fn player(opacity: f32) -> Option<(Player, Vec<Tick>)> {
    kt_plugins_builtin::install();
    let gpu = kt_gpu::test_engine()?;
    let mut p = demo_project(DemoSources::Synth);
    for c in &mut p.clips {
        c.opacity = opacity; // a picture no other test presents on the shared engine
    }
    let sources: Vec<(kt_model::MediaId, Box<dyn MediaSource>)> = p
        .media
        .iter()
        .map(|m| (m.id, kt_media::factory::open_pseudo(&m.path).unwrap().unwrap()))
        .collect();
    let (player, _ring) = Player::new(p, sources);
    player.set_preview_quality(Quality { width: 320, height: 180 });
    player.set_video_gpu(Some(gpu));
    let grid = player.video_grid();
    let step = grid.frame_step();
    let start = grid.align_floor(Tick(90_000));
    let moments: Vec<Tick> = (0..MOMENTS).map(|i| Tick(start.0 + i * step)).collect();
    player.set_loop_region(Some(TickRange::new(start, Tick(start.0 + MOMENTS * step))));
    player.seek(start);
    Some((player, moments))
}

/// Present until the moment comes out complete (its frames decode in the background).
fn present_complete(player: &Player, t: Tick) {
    let deadline = kt_time::test_deadline();
    while !matches!(player.present_preview(t, PresentOptions::default()), Some((_, true))) {
        assert!(std::time::Instant::now() < deadline, "the moment {t:?} never came out complete");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// The moments the bars draw as playing without work, settled.
fn kept(player: &Player, moments: &[Tick]) -> usize {
    let runs = loop {
        let (runs, pending) = player.final_video_runs();
        if !pending {
            break runs;
        }
    };
    moments.iter().filter(|t| runs.iter().any(|r| r.start <= t.0 && t.0 < r.end)).count()
}

/// The prefetch passes made so far.
fn pass_count(player: &Player) -> u64 {
    player.loop_stats.prefetch_passes.load(Ordering::Relaxed)
}

/// Wait until `n` passes more than `p0` were made. `p0` is read BEFORE what causes the passes: on a
/// pause a seek owes exactly one, and under load it lands before a count read after the seek, which
/// then waits for a pass nothing will ever ask (t2335 stood 600 s that way).
fn passes_since(player: &Player, p0: u64, n: u64) {
    let deadline = kt_time::test_deadline();
    while pass_count(player) < p0 + n {
        assert!(std::time::Instant::now() < deadline, "the prefetch thread stopped passing");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Wait for `n` more prefetch passes while a loop plays (each kicks the producer; playing, they keep
/// coming every 100 ms).
fn passes(player: &Player, n: u64) {
    passes_since(player, pass_count(player), n);
}

/// Test t2334: a loop of 24 moments plays, the display presents every fourth; without a producer
/// executor the rest stay unkept lap after lap, with one every moment is kept, and the display's
/// later presents are replays. The producer's jobs ask no decode.
#[test]
fn t2334_a_loop_shown_in_part_becomes_all_kept() {
    let Some((player, moments)) = player(0.37) else { return };
    let Some(side) = kt_gpu::side_engine() else { return };
    for t in moments.iter().step_by(4) {
        present_complete(&player, *t);
    }
    // the "before": no producer, the laps keep only what the display presented
    player.force_ahead_gpu(Some(None));
    player.play();
    passes(&player, 5);
    let before = kept(&player, &moments);
    assert_eq!(before, moments.len() / 4, "scene guard: only the presented moments are kept");
    let items = kt_time::trace::testing::capture(|| {
        // the rule's own executor: the display's sibling, on its device
        let _ = side;
        player.force_ahead_gpu(None);
        let deadline = kt_time::test_deadline();
        while kept(&player, &moments) < moments.len() {
            assert!(std::time::Instant::now() < deadline, "the loop never became all-kept: {} of {}", kept(&player, &moments), moments.len());
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    });
    player.pause();
    assert!(player.ahead_kept() >= (moments.len() - moments.len() / 4) as u64, "the producer kept fewer than it was missing");
    // the display's own composites are replays now
    let replays0 = kt_time::work::count(kt_time::work::Work::CompositeReplay);
    for t in &moments {
        present_complete(&player, *t);
    }
    let replays = kt_time::work::count(kt_time::work::Work::CompositeReplay) - replays0;
    assert_eq!(replays, moments.len() as u64, "a kept moment was composed again by the display");
    // no decode request came from a fill job
    let recs = kt_time::trace::testing::recs(&items);
    let jobs: std::collections::HashSet<u64> = recs
        .iter()
        .filter(|r| r.ev == kt_time::trace::Ev::Job as u16 && r.aux == kt_time::trace::job::RENDER_AHEAD)
        .map(|r| r.id)
        .collect();
    assert!(!jobs.is_empty(), "no render-ahead job was traced");
    let asked = recs.iter().filter(|r| r.ev == kt_time::trace::Ev::DecReq as u16 && jobs.contains(&r.cause)).count();
    assert_eq!(asked, 0, "a render-ahead job asked decodes");
    eprintln!("t2334: before {before} of {} kept, after {} (the producer kept {})", moments.len(), moments.len(), player.ahead_kept());
}

/// Test t2335: with the play paused the producer runs no job, whatever the prefetch thread does.
#[test]
fn t2335_the_producer_rests_with_the_player() {
    let Some((player, moments)) = player(0.41) else { return };
    let Some(side) = kt_gpu::side_engine() else { return };
    present_complete(&player, moments[0]);
    player.set_ahead_gpu(Some(side as Arc<dyn VideoGpuExec>));
    // paused: a seek moves the prefetch, which passes; the producer must not run
    let near = |s: kt_time::sched::Stats| s.classes[kt_time::sched::Class::Near as usize].started;
    let started0 = near(kt_time::sched::stats());
    for t in moments.iter().step_by(6) {
        let p0 = pass_count(&player);
        player.seek(*t);
        passes_since(&player, p0, 1);
    }
    assert_eq!(near(kt_time::sched::stats()), started0, "the producer ran on a pause");
    assert_eq!(player.ahead_kept(), 0, "the producer kept moments on a pause");
}

/// The display's executor whose memo keeps only the first `limit` moments another device made.
struct Refusing {
    inner: Arc<kt_gpu::GpuFxEngine>,
    limit: usize,
    kept: AtomicUsize,
}

impl VideoGpuExec for Refusing {
    fn execute_into(&self, p: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, GpuExecError> {
        self.inner.execute_into(p, out)
    }
    fn info(&self) -> &GpuBackendInfo {
        self.inner.info()
    }
    fn last_cost(&self) -> kt_fx::video::FrameCost {
        self.inner.last_cost()
    }
    fn can_present(&self) -> bool {
        true
    }
    fn present(&self, p: &FramePlan<'_>, o: PresentOptions) -> Result<bool, GpuExecError> {
        self.inner.present(p, o)
    }
    fn wait_presented(&self) {
        self.inner.wait_presented()
    }
    fn finish_present(&self) {
        self.inner.finish_present()
    }
    fn last_stages(&self) -> kt_engine::GpuStages {
        self.inner.last_stages()
    }
    fn present_key(&self, p: &FramePlan<'_>) -> Option<u64> {
        self.inner.present_key(p)
    }
    fn holds(&self, key: u64) -> bool {
        self.inner.holds(key)
    }
    fn present_kept(&self, key: u64, o: PresentOptions) -> bool {
        self.inner.present_kept(key, o)
    }
    fn keep(&self, key: u64, w: u32, h: u32, rgba: &[u8]) {
        if self.kept.fetch_add(1, Ordering::AcqRel) < self.limit {
            self.inner.keep(key, w, h, rgba);
        }
    }
}

/// Test t2336: a display memo that stops keeping after eight of the producer's composites: eight kept,
/// the rest skipped for the lap, no more jobs until the lap moves.
#[test]
fn t2336_the_producer_skips_what_the_memo_does_not_keep() {
    let Some((player, moments)) = player(0.29) else { return };
    let Some(side) = kt_gpu::side_engine() else { return };
    let Some(inner) = kt_gpu::test_engine() else { return };
    let display = Arc::new(Refusing { inner, limit: 8, kept: AtomicUsize::new(0) });
    player.set_video_gpu(Some(display.clone()));
    present_complete(&player, moments[0]);
    player.set_ahead_gpu(Some(side as Arc<dyn VideoGpuExec>));
    player.play();
    // every moment tried once: eight kept, the others skipped (the playhead stands: one lap)
    let deadline = kt_time::test_deadline();
    while display.kept.load(Ordering::Acquire) < moments.len() - 1 {
        assert!(std::time::Instant::now() < deadline, "the producer did not try every moment: {}", display.kept.load(Ordering::Acquire));
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    passes(&player, 5);
    let tried = display.kept.load(Ordering::Acquire);
    passes(&player, 5);
    player.pause();
    assert_eq!(player.ahead_kept(), 8, "the producer kept other than what the memo admitted");
    assert_eq!(tried, moments.len() - 1, "a skipped moment was composed again in the lap");
    assert_eq!(display.kept.load(Ordering::Acquire), tried, "the producer went on composing after the lap was done");
}

/// A producer executor that reports a hardware device: it composes on the side engine and keeps on the
/// display at once, and its "device" ends the moment `delay` later — by a done flag (`flag`), or by
/// holding the call that long (a moment waited for).
struct Card {
    inner: Arc<kt_gpu::GpuFxEngine>,
    info: GpuBackendInfo,
    delay: std::time::Duration,
    flag: bool,
    /// (moment key, done flag) of the moments in flight now, for the peak.
    live: std::sync::Mutex<Vec<Arc<std::sync::atomic::AtomicBool>>>,
    peak: AtomicUsize,
    keys: std::sync::Mutex<Vec<u64>>,
    /// The wall of each moment's own work (compose and hand over), the device latency aside.
    work: std::sync::Mutex<Vec<std::time::Duration>>,
}

impl Card {
    fn new(flag: bool) -> Option<Arc<Card>> {
        let inner = kt_gpu::side_engine()?;
        let mut info = inner.info().clone();
        info.software = false;
        Some(Arc::new(Card {
            inner,
            info,
            delay: std::time::Duration::from_millis(40),
            flag,
            live: Default::default(),
            peak: AtomicUsize::new(0),
            keys: Default::default(),
            work: Default::default(),
        }))
    }
}

impl VideoGpuExec for Card {
    fn execute_into(&self, p: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, GpuExecError> {
        self.inner.execute_into(p, out)
    }
    fn info(&self) -> &GpuBackendInfo {
        &self.info
    }
    fn ahead(&self, plan: &FramePlan<'_>, keeper: &Arc<dyn VideoGpuExec>, buf: &mut Vec<u8>) -> Result<kt_engine::Ahead, GpuExecError> {
        let t0 = std::time::Instant::now();
        let mut a = kt_engine::ahead_by_bytes(self, plan, keeper, buf)?;
        self.work.lock().unwrap().push(t0.elapsed());
        if let Some(k) = a.kept {
            self.keys.lock().unwrap().push(k);
        }
        if !self.flag {
            std::thread::sleep(self.delay);
            return Ok(a);
        }
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let mut live = self.live.lock().unwrap();
            live.retain(|d| !d.load(Ordering::Acquire));
            live.push(done.clone());
            self.peak.fetch_max(live.len(), Ordering::AcqRel);
        }
        let (d, delay) = (done.clone(), self.delay);
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            d.store(true, Ordering::Release);
        });
        a.done = Some(done);
        Ok(a)
    }
}

/// Fill the loop (every fourth moment presented) with `card` as the producer's executor; the wall time
/// from its arrival to all-kept.
fn fill_with(card: Arc<Card>, opacity: f32) -> Option<std::time::Duration> {
    let (player, moments) = player(opacity)?;
    // no producer until the card arrives
    player.force_ahead_gpu(Some(None));
    for t in moments.iter().step_by(4) {
        present_complete(&player, *t);
    }
    // every moment decoded before the clock starts: the producer asks no decode
    for t in &moments {
        let deadline = kt_time::test_deadline();
        while !player.missing_layers(*t).is_empty() {
            assert!(std::time::Instant::now() < deadline, "the moment {t:?} never decoded");
            player.seek(*t);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    player.seek(moments[0]);
    player.play();
    let t0 = std::time::Instant::now();
    player.force_ahead_gpu(Some(Some(card as Arc<dyn VideoGpuExec>)));
    let deadline = kt_time::test_deadline();
    while kept(&player, &moments) < moments.len() {
        assert!(std::time::Instant::now() < deadline, "the loop never became all-kept");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let took = t0.elapsed();
    player.pause();
    Some(took)
}

/// Test t2920 (render-ahead D3, D4; replaces t2860–t2862): a producer executor on a card returns from
/// each moment at once and the device ends it 40 ms later. Several moments are in flight on the one
/// engine, none is composed twice in the lap, and the wall beyond the moments' own work is under half
/// of the latency the same executor pays when every moment is waited for.
#[test]
fn t2920_on_a_card_several_moments_are_in_flight() {
    let (Some(flagged), Some(waited)) = (Card::new(true), Card::new(false)) else { return };
    let Some(fast) = fill_with(flagged.clone(), 0.33) else { return };
    let slow = fill_with(waited.clone(), 0.31).unwrap();
    let waits = waited.keys.lock().unwrap().len() as u32;
    let own = |c: &Card| {
        let mut w = c.work.lock().unwrap().clone();
        w.sort();
        (w.first().copied().unwrap_or_default(), w.get(w.len() / 2).copied().unwrap_or_default(), w.iter().sum::<std::time::Duration>())
    };
    eprintln!("t2920: own work (min, median, sum) in flight {:?}, waited {:?}", own(&flagged), own(&waited));
    let peak = flagged.peak.load(Ordering::Acquire);
    let keys = flagged.keys.lock().unwrap().clone();
    let distinct: std::collections::HashSet<u64> = keys.iter().copied().collect();
    eprintln!("t2920: in flight up to {peak}, all-kept in {fast:?} against {slow:?} waited for ({waits} moments waited)");
    assert!(peak >= 2, "one moment in flight at a time");
    assert_eq!(distinct.len(), keys.len(), "a moment was composed twice in the lap");
    // The waited run pays the device's latency once a moment; in flight it is hidden behind the next
    // moments' own work. Judged by the wall NOT explained by each run's own work (compose and hand
    // over), measured per moment: the own work itself moves with the machine's load and between the two
    // runs (found-defects 730: under a full parallel run the in-flight run's own work was 1.2 s against
    // the waited run's 0.74 s, and a comparison of the two walls read it as latency not hidden)
    let latency = waited.delay * waits;
    let unexplained = |wall: std::time::Duration, c: &Card| wall.saturating_sub(own(c).2);
    let (paid, left) = (unexplained(slow, &waited), unexplained(fast, &flagged));
    eprintln!("t2920: latency {latency:?}: the waited run paid {paid:?}, in flight {left:?} was left");
    assert!(paid * 10 >= latency * 9, "the scene: the waited run pays the latency ({paid:?} of {latency:?})");
    assert!(left * 2 <= latency, "in flight {left:?} of the wall is not the moments' own work: the latency ({latency:?}) is not hidden");
}

/// The display's executor that counts the plans it presents.
struct Counting {
    inner: Arc<kt_gpu::GpuFxEngine>,
    presents: AtomicUsize,
}

impl VideoGpuExec for Counting {
    fn execute_into(&self, p: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, GpuExecError> {
        self.inner.execute_into(p, out)
    }
    fn info(&self) -> &GpuBackendInfo {
        self.inner.info()
    }
    fn can_present(&self) -> bool {
        true
    }
    fn present(&self, p: &FramePlan<'_>, o: PresentOptions) -> Result<bool, GpuExecError> {
        self.presents.fetch_add(1, Ordering::AcqRel);
        self.inner.present(p, o)
    }
    fn wait_presented(&self) {
        self.inner.wait_presented()
    }
    fn last_stages(&self) -> kt_engine::GpuStages {
        self.inner.last_stages()
    }
    fn present_key(&self, p: &FramePlan<'_>) -> Option<u64> {
        self.inner.present_key(p)
    }
    fn holds(&self, key: u64) -> bool {
        self.inner.holds(key)
    }
    fn present_kept(&self, key: u64, o: PresentOptions) -> bool {
        self.inner.present_kept(key, o)
    }
    fn keep(&self, key: u64, w: u32, h: u32, rgba: &[u8]) {
        self.inner.keep(key, w, h, rgba)
    }
}

/// Ask the compose thread for `t` with a display target and take its answer.
fn ask(player: &Player, t: Tick) -> kt_engine::compose_queue::Composed {
    let seq = player.ask_preview(t, Some(PresentOptions::default()));
    let deadline = kt_time::test_deadline();
    loop {
        if let Some(c) = player.take_composed(std::time::Duration::from_millis(50))
            && c.seq == seq
        {
            return c;
        }
        assert!(std::time::Instant::now() < deadline, "no answer to the ask for {t:?}");
    }
}

/// Test t2925 (render-ahead D1): playing with a producer, the display presents only kept composites —
/// it runs no plan, and a moment not yet kept has no picture; the loop's answers become kept
/// composites. Paused, a moment is composed on demand.
#[test]
fn t2925_playing_the_display_shows_only_kept_moments() {
    let Some((player, moments)) = player(0.23) else { return };
    let Some(side) = kt_gpu::side_engine() else { return };
    let Some(inner) = kt_gpu::test_engine() else { return };
    let display = Arc::new(Counting { inner, presents: AtomicUsize::new(0) });
    player.set_video_gpu(Some(display.clone()));
    for t in &moments {
        let deadline = kt_time::test_deadline();
        while !player.missing_layers(*t).is_empty() {
            assert!(std::time::Instant::now() < deadline, "the moment {t:?} never decoded");
            player.seek(*t);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    player.seek(moments[0]);
    player.set_ahead_gpu(Some(side as Arc<dyn VideoGpuExec>));
    player.play();
    let deadline = kt_time::test_deadline();
    let mut missing = 0;
    loop {
        let answers: Vec<bool> = moments.iter().map(|t| ask(&player, *t).complete).collect();
        missing += answers.iter().filter(|c| !**c).count();
        if answers.iter().all(|c| *c) {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "the loop's answers never all became kept composites");
    }
    assert_eq!(display.presents.load(Ordering::Acquire), 0, "playing, the display ran a plan");
    player.pause();
    // a moment outside the loop, never kept: composed on demand
    let away = Tick(moments[0].0 - 6 * player.video_grid().frame_step());
    let deadline = kt_time::test_deadline();
    while !ask(&player, away).complete {
        assert!(std::time::Instant::now() < deadline, "paused, the moment never came out complete");
        player.seek(away);
    }
    assert!(display.presents.load(Ordering::Acquire) > 0, "paused, the display did not compose on demand");
    eprintln!("t2925: {missing} answers had no picture before the producer reached them");
}
