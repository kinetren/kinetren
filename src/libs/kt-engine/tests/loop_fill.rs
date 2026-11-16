//! scheduler-followups D5 (found-defects 251): a playing loop's unshown moments are composed below
//! the display and kept on its executor. t2334, t2335, t2336. Without a wgpu adapter: skip.

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

/// Wait for `n` more prefetch passes while a loop plays (each kicks the fill; playing, they keep
/// coming every 100 ms).
fn passes(player: &Player, n: u64) {
    passes_since(player, pass_count(player), n);
}

/// Test t2334: a loop of 24 moments plays, the display presents every fourth; without a fill
/// executor the rest stay unkept lap after lap, with one every moment is kept, and the display's
/// later presents are replays. The fill's jobs ask no decode.
#[test]
fn t2334_a_loop_shown_in_part_becomes_all_kept() {
    let Some((player, moments)) = player(0.37) else { return };
    let Some(side) = kt_gpu::side_engine() else { return };
    for t in moments.iter().step_by(4) {
        present_complete(&player, *t);
    }
    player.play();
    // the "before": no fill executor, the laps keep only what the display presented
    passes(&player, 5);
    let before = kept(&player, &moments);
    assert_eq!(before, moments.len() / 4, "scene guard: only the presented moments are kept");
    let items = kt_time::trace::testing::capture(|| {
        player.set_fill_gpu(Some(side as Arc<dyn VideoGpuExec>));
        let deadline = kt_time::test_deadline();
        while kept(&player, &moments) < moments.len() {
            assert!(std::time::Instant::now() < deadline, "the loop never became all-kept: {} of {}", kept(&player, &moments), moments.len());
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    });
    player.pause();
    assert!(player.fill_kept() >= (moments.len() - moments.len() / 4) as u64, "the fill kept fewer than it was missing");
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
        .filter(|r| r.ev == kt_time::trace::Ev::Job as u16 && r.aux == kt_time::trace::job::LOOP_FILL)
        .map(|r| r.id)
        .collect();
    assert!(!jobs.is_empty(), "no fill job was traced");
    let asked = recs.iter().filter(|r| r.ev == kt_time::trace::Ev::DecReq as u16 && jobs.contains(&r.cause)).count();
    assert_eq!(asked, 0, "a fill job asked decodes");
    eprintln!("t2334: before {before} of {} kept, after {} (fill kept {})", moments.len(), moments.len(), player.fill_kept());
}

/// Test t2335: with the play paused the fill runs no job, whatever the prefetch thread does.
#[test]
fn t2335_the_fill_rests_with_the_player() {
    let Some((player, moments)) = player(0.41) else { return };
    let Some(side) = kt_gpu::side_engine() else { return };
    present_complete(&player, moments[0]);
    player.set_fill_gpu(Some(side as Arc<dyn VideoGpuExec>));
    // paused: a seek moves the prefetch, which passes; the fill must not run
    let near = |s: kt_time::sched::Stats| s.classes[kt_time::sched::Class::Near as usize].started;
    let started0 = near(kt_time::sched::stats());
    for t in moments.iter().step_by(6) {
        let p0 = pass_count(&player);
        player.seek(*t);
        passes_since(&player, p0, 1);
    }
    assert_eq!(near(kt_time::sched::stats()), started0, "the fill ran on a pause");
    assert_eq!(player.fill_kept(), 0, "the fill kept moments on a pause");
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

/// Test t2336: a display memo that stops keeping after eight of the fill's composites stops the
/// fill for the lap: eight kept, the rest skipped, no more jobs until the lap moves.
#[test]
fn t2336_the_fill_stops_where_the_memo_stops_keeping() {
    let Some((player, moments)) = player(0.29) else { return };
    let Some(side) = kt_gpu::side_engine() else { return };
    let Some(inner) = kt_gpu::test_engine() else { return };
    let display = Arc::new(Refusing { inner, limit: 8, kept: AtomicUsize::new(0) });
    player.set_video_gpu(Some(display.clone()));
    present_complete(&player, moments[0]);
    player.set_fill_gpu(Some(side as Arc<dyn VideoGpuExec>));
    player.play();
    // every moment tried once: eight kept, the others skipped (the playhead stands: one lap)
    let deadline = kt_time::test_deadline();
    while display.kept.load(Ordering::Acquire) < moments.len() - 1 {
        assert!(std::time::Instant::now() < deadline, "the fill did not try every moment: {}", display.kept.load(Ordering::Acquire));
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    passes(&player, 5);
    let tried = display.kept.load(Ordering::Acquire);
    passes(&player, 5);
    player.pause();
    assert_eq!(player.fill_kept(), 8, "the fill kept other than what the memo admitted");
    assert_eq!(tried, moments.len() - 1, "a skipped moment was composed again in the lap");
    assert_eq!(display.kept.load(Ordering::Acquire), tried, "the fill went on composing after the lap was done");
}

/// What a [`Card`] saw: every composite's wall span, and how many ran at once.
#[derive(Default)]
struct CardLog {
    now: AtomicUsize,
    peak: AtomicUsize,
    calls: AtomicUsize,
    /// (start, end) of each composite.
    spans: std::sync::Mutex<Vec<(std::time::Instant, std::time::Instant)>>,
    /// The call that answers incomplete (0 — none) and when it ended.
    incomplete_at: usize,
    incomplete_end: std::sync::Mutex<Option<std::time::Instant>>,
}

/// A fill executor that reports a hardware device and takes `delay` of wall per composite (a card's
/// round trip: admissions and the readback), over the side engine's siblings.
struct Card {
    inner: Arc<kt_gpu::GpuFxEngine>,
    info: GpuBackendInfo,
    delay: std::time::Duration,
    siblings: bool,
    log: Arc<CardLog>,
}

impl Card {
    fn new(delay_ms: u64, siblings: bool, incomplete_at: usize) -> Option<Arc<Card>> {
        let inner = kt_gpu::side_engine()?.sibling();
        let mut info = inner.info().clone();
        info.software = false;
        let log = Arc::new(CardLog { incomplete_at, ..Default::default() });
        Some(Arc::new(Card { inner, info, delay: std::time::Duration::from_millis(delay_ms), siblings, log }))
    }
}

impl VideoGpuExec for Card {
    fn execute_into(&self, p: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, GpuExecError> {
        let l = &self.log;
        let call = l.calls.fetch_add(1, Ordering::AcqRel) + 1;
        let n = l.now.fetch_add(1, Ordering::AcqRel) + 1;
        l.peak.fetch_max(n, Ordering::AcqRel);
        let t0 = std::time::Instant::now();
        let r = self.inner.execute_into(p, out);
        std::thread::sleep(self.delay.saturating_sub(t0.elapsed()));
        l.now.fetch_sub(1, Ordering::AcqRel);
        let t1 = std::time::Instant::now();
        l.spans.lock().unwrap().push((t0, t1));
        if call == l.incomplete_at {
            *l.incomplete_end.lock().unwrap() = Some(t1);
            out.clear();
            return Ok(false);
        }
        r
    }
    fn info(&self) -> &GpuBackendInfo {
        &self.info
    }
    fn last_cost(&self) -> kt_fx::video::FrameCost {
        self.inner.last_cost()
    }
    fn last_stages(&self) -> kt_engine::GpuStages {
        self.inner.last_stages()
    }
    fn present_key(&self, p: &FramePlan<'_>) -> Option<u64> {
        self.inner.present_key(p)
    }
    fn sibling_exec(&self) -> Option<Arc<dyn VideoGpuExec>> {
        self.siblings.then(|| {
            Arc::new(Card { inner: self.inner.sibling(), info: self.info.clone(), delay: self.delay, siblings: true, log: self.log.clone() })
                as Arc<dyn VideoGpuExec>
        })
    }
}

/// Fill a loop shown in part (every fourth moment presented) on `card`, the display composing a
/// frame of its own all the while; the wall time from the executor's arrival to all-kept.
fn fill_on_card(opacity: f32, card: Arc<Card>) -> Option<(std::time::Duration, Player, Vec<Tick>)> {
    let (player, moments) = player(opacity)?;
    for t in moments.iter().step_by(4) {
        present_complete(&player, *t);
    }
    player.play();
    passes(&player, 5);
    let busy = player.composing_for_tests();
    let t0 = std::time::Instant::now();
    player.set_fill_gpu(Some(card as Arc<dyn VideoGpuExec>));
    let deadline = kt_time::test_deadline();
    while kept(&player, &moments) < moments.len() {
        assert!(std::time::Instant::now() < deadline, "the loop never became all-kept: {} of {}", kept(&player, &moments), moments.len());
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let wall = t0.elapsed();
    drop(busy);
    player.pause();
    Some((wall, player, moments))
}

/// Test t2860: on a card (a composite is a 150 ms round trip) the fill keeps several moments in
/// flight, each on its own engine, takes no moment twice, is not stopped by the display's own
/// composite, and fills the loop in at most two thirds of the time one moment at a time takes.
#[test]
fn t2860_on_a_card_the_fill_keeps_several_moments_in_flight() {
    // the side engine's pipelines compile at their first draw: warmed by an untimed fill first
    let Some(warm) = Card::new(0, false, 0) else { return };
    if fill_on_card(0.47, warm).is_none() {
        return;
    }
    let Some(wide) = Card::new(150, true, 0) else { return };
    let Some(one) = Card::new(150, false, 0) else { return };
    let (wide_log, one_log) = (wide.log.clone(), one.log.clone());
    let items = kt_time::trace::testing::capture(|| {
        let Some((w, player, _)) = fill_on_card(0.53, wide) else { return };
        let width = player.fill_width();
        assert!(width >= 2, "the machine's width on a card is {width}");
        assert!(wide_log.peak.load(Ordering::Acquire) >= 2, "no two fill composites ran at once");
        let Some((w1, player1, _)) = fill_on_card(0.61, one) else { return };
        assert_eq!(player1.fill_width(), 1, "an executor without siblings fills one moment at a time");
        assert_eq!(one_log.peak.load(Ordering::Acquire), 1);
        eprintln!("t2860: width {width}: {w:?}, width 1: {w1:?}");
        assert!(w.as_secs_f64() <= w1.as_secs_f64() * 2.0 / 3.0, "width {width} took {w:?} against {w1:?} one at a time");
    });
    // no moment composed twice at once: a moment's fill jobs never overlap
    let recs = kt_time::trace::testing::recs(&items);
    let mut jobs: Vec<(u64, u64, u64)> = recs
        .iter()
        .filter(|r| r.ev == kt_time::trace::Ev::Job as u16 && r.aux == kt_time::trace::job::LOOP_FILL)
        .map(|r| (r.a, r.t, r.t + r.dur))
        .collect();
    jobs.sort();
    for w in jobs.windows(2) {
        assert!(w[0].0 != w[1].0 || w[0].2 <= w[1].1, "the moment {} was composed by two lanes at once", w[0].0);
    }
}

/// Test t2862: on a card at the machine's width, a moment that comes out incomplete narrows the fill
/// to one moment at a time until the lap moves (here: the loop region changes), then it widens again.
#[test]
fn t2862_an_incomplete_moment_narrows_the_fill_for_the_lap() {
    let Some(card) = Card::new(100, true, 3) else { return };
    let log = card.log.clone();
    let Some((player, moments)) = player(0.67) else { return };
    for t in moments.iter().step_by(4) {
        present_complete(&player, *t);
    }
    player.play();
    passes(&player, 5);
    player.set_fill_gpu(Some(card as Arc<dyn VideoGpuExec>));
    if player.fill_width() < 2 {
        return;
    }
    // the incomplete composite, then the fill goes on alone until it has nothing left this lap
    let deadline = kt_time::test_deadline();
    let inc = loop {
        if let Some(t) = *log.incomplete_end.lock().unwrap() {
            break t;
        }
        assert!(std::time::Instant::now() < deadline, "no composite came out incomplete");
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    // three composites started after it, with moments still unkept (eighteen to fill); a lane that
    // claimed its moment before the incomplete one ended starts within its own few milliseconds
    let inc = inc + std::time::Duration::from_millis(50);
    let started_after = || log.spans.lock().unwrap().iter().filter(|(s, _)| *s > inc).count();
    while started_after() < 3 {
        assert!(std::time::Instant::now() < deadline, "the narrowed fill stopped");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let after: Vec<_> = log.spans.lock().unwrap().iter().filter(|(s, _)| *s > inc).copied().collect();
    assert!(kept(&player, &moments) + 2 <= moments.len(), "the scene left too few moments to widen on");
    for (i, a) in after.iter().enumerate() {
        for b in &after[i + 1..] {
            assert!(a.1 <= b.0 || b.1 <= a.0, "two fill composites ran at once after an incomplete one in the lap");
        }
    }
    // a new lap (the region moves): the incomplete moment and the wide fill come back
    let peak0 = log.peak.swap(0, Ordering::AcqRel);
    assert!(peak0 >= 2, "the fill never ran wide before the incomplete moment");
    let end = *moments.last().unwrap();
    player.set_loop_region(Some(TickRange::new(moments[0], end)));
    while log.peak.load(Ordering::Acquire) < 2 {
        assert!(std::time::Instant::now() < deadline, "the fill did not widen on the new lap");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    player.pause();
}
