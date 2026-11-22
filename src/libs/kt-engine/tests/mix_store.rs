//! cached-range-plays: the final mix of a covered range is kept and played again with no decode
//! and no effect run (the owner's choice "a", invariant 2 as reworded). t1930–t1933.

use kt_engine::mix::{MixBudget, MixCtx, MixPlay, MixStore, mix_want, pieces};
use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::AudioBurst;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};
use std::sync::Arc;

const CH: usize = AUDIO_CHANNELS;
const BURST: i64 = AUDIO_BURST_FRAMES as i64;
const SEC: i64 = 48_000;

/// Counts the source reads: a burst handed out from the store reads nothing.
struct Counting {
    inner: DirectFetcher,
    reads: u64,
}

impl Counting {
    fn new() -> Counting {
        let mut inner = DirectFetcher::new(None);
        inner.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Ramp)));
        inner.add_source(
            MediaId(2),
            Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 220.0 })),
        );
        Counting { inner, reads: 0 }
    }
}

impl AudioFetcher for Counting {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<AudioBurst>> {
        self.reads += 1;
        self.inner.audio_burst(media, idx)
    }
}

fn project(clips: Vec<ClipEvent>) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.media.push(MediaRef::new(MediaId(2), "synth:sine"));
    p.clips = clips;
    p
}

fn clip(id: u64, media: u64, lane: u32, start: i64, dur: i64, fx: Vec<FxInstance>) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane,
        start: Tick::from_audio_frames(start),
        duration: Tick::from_audio_frames(dur),
        source_in: Tick::ZERO,
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: fx.clone(),
        opacity: 1.0,
        fx_off: false,
        fx_in: Vec::new(),
    }
}

fn reverb(mix: f64) -> FxInstance {
    kt_plugins_builtin::install();
    FxInstance::plugin_with("slowed_reverb", &[1.0, mix, 60.0, 0.4, 0.7, 1.0, 0.0, 0.0, 1.0]).unwrap()
}

/// `n` chunks of the forward stream from `head` through the store; (samples, replayed chunks).
fn play(
    mix: &mut MixPlay,
    r: &AudioRenderer,
    f: &mut dyn AudioFetcher,
    fx: &mut FxStateStore,
    region: Option<(i64, i64)>,
    head: i64,
    n: usize,
) -> (Vec<f32>, usize) {
    let mut out = Vec::with_capacity(n * AUDIO_BURST_FRAMES * CH);
    let mut chunk = vec![0f32; AUDIO_BURST_FRAMES * CH];
    let mut replayed = 0;
    for i in 0..n {
        let o = mix.render(r, f, fx, region, head + i as i64 * BURST, &mut chunk);
        assert_eq!(o.stats.underruns, 0);
        replayed += usize::from(o.replayed);
        out.extend_from_slice(&chunk);
    }
    (out, replayed)
}

/// A seek to `head`: the player resets the FX state and tells the store.
fn seek(mix: &mut MixPlay, r: &AudioRenderer, fx: &mut FxStateStore, head: i64) {
    fx.reset_all();
    mix.fx_reset(r, head);
}

/// t1930: a covered range plays again with no source read and no effect run, and it sounds as
/// it did the first time, byte for byte.
#[test]
fn t1930_covered_range_replays_without_work() {
    let p = project(vec![clip(1, 1, 0, 0, 2 * SEC, vec![reverb(0.5)])]);
    let r = AudioRenderer::new(&p);
    let mut fx = FxStateStore::new();
    r.sync_fx(&mut fx);
    let mut f = Counting::new();
    let mut mix = MixPlay::default();
    let (first, replayed) = play(&mut mix, &r, &mut f, &mut fx, None, 0, 60);
    assert_eq!(replayed, 0, "nothing was held the first time");
    assert!(f.reads > 0);
    seek(&mut mix, &r, &mut fx, 0);
    let reads = f.reads;
    let (again, replayed) = play(&mut mix, &r, &mut f, &mut fx, None, 0, 60);
    assert_eq!(replayed, 60, "every chunk of the covered range came from the store");
    assert_eq!(f.reads, reads, "no source was read for it");
    assert!(first == again, "the range keeps the sound it was first heard with");
    // and that sound is the live render's
    let mut fx2 = FxStateStore::new();
    r.sync_fx(&mut fx2);
    let mut live = vec![0f32; first.len()];
    r.render_frames_with(&mut Counting::new(), &mut fx2, 0, &mut live);
    assert!(first == live);
}

/// t1931: an edit drops exactly the bursts whose sound it determines — the event's body and tail —
/// and keeps the rest.
#[test]
fn t1931_parameter_change_drops_only_its_range() {
    let a = clip(1, 1, 0, 0, SEC, vec![reverb(0.3)]);
    let b = clip(2, 2, 0, 3 * SEC, SEC, vec![reverb(0.3)]);
    let p = project(vec![a.clone(), b.clone()]);
    let r = AudioRenderer::new(&p);
    let mut fx = FxStateStore::new();
    r.sync_fx(&mut fx);
    let mut f = Counting::new();
    let mut mix = MixPlay::default();
    let chunks = (6 * SEC / BURST) as usize;
    play(&mut mix, &r, &mut f, &mut fx, None, 0, chunks);
    let before = mix.store.runs().linear;
    assert_eq!(before, vec![0..chunks as i64 * BURST], "the whole range is held");

    let mut b2 = b.clone();
    b2.fx = vec![reverb(0.8)];
    let p2 = project(vec![a, b2]);
    let r2 = AudioRenderer::new(&p2);
    mix.revalidate(&r2);
    let after = mix.store.runs().linear;
    let b_start = 3 * SEC;
    // the event's span with its tail: what the renderer itself counts as active
    let b_end = 4 * SEC + 2 * SEC;
    for run in &after {
        assert!(run.end <= b_start || run.start >= 4 * SEC, "a burst of B's body survived: {run:?}");
    }
    assert!(after.iter().any(|r| r.start == 0 && r.end >= b_start - BURST), "A's range was dropped: {after:?}");
    let held: i64 = after.iter().map(|r| r.end - r.start).sum();
    assert!(held >= b_start - BURST, "too much was dropped: {after:?}");
    assert!(held < chunks as i64 * BURST, "nothing was dropped");
    // what lies past B's tail, with nothing sounding, is held still
    assert!(after.iter().any(|r| r.end == chunks as i64 * BURST) || b_end >= chunks as i64 * BURST);
    // a second play renders B's range live and replays the rest
    seek(&mut mix, &r, &mut fx, 0);
    let mut fx2 = FxStateStore::new();
    r2.sync_fx(&mut fx2);
    let (_, replayed) = play(&mut mix, &r2, &mut f, &mut fx2, None, 0, chunks);
    assert!(replayed > 0 && replayed < chunks, "replayed {replayed} of {chunks}");
}

/// t1932: where the store ends the play goes on as a continuous play (kept-mix-settle): the effects
/// are brought back by a discarded play of their settle length before the chunk, so the chunks
/// after it equal a live render that started that far before, byte for byte.
#[test]
fn t1932_kept_to_live_boundary_is_a_seek_byte_for_byte() {
    let p = project(vec![
        clip(1, 1, 0, 0, 3 * SEC, vec![reverb(0.5)]),
        clip(2, 2, 1, SEC / 2, 2 * SEC, vec![reverb(0.2)]),
    ]);
    let r = AudioRenderer::new(&p);
    let mut fx = FxStateStore::new();
    r.sync_fx(&mut fx);
    let mut f = Counting::new();
    let mut mix = MixPlay::default();
    let kept = 40usize;
    let (first, _) = play(&mut mix, &r, &mut f, &mut fx, None, 0, kept);
    seek(&mut mix, &r, &mut fx, 0);
    let (out, replayed) = play(&mut mix, &r, &mut f, &mut fx, None, 0, kept + 40);
    assert_eq!(replayed, kept);
    let cut = kept * AUDIO_BURST_FRAMES * CH;
    assert!(out[..cut] == first[..], "the kept part is the first recording");
    let h = kept as i64 * BURST;
    let settle = r.settle_frames(h..h + BURST);
    assert!(settle > 0 && settle < h, "the chains keep a state shorter than the kept run: {settle}");
    let from = h - (settle + BURST - 1) / BURST * BURST;
    let mut fx2 = FxStateStore::new();
    r.sync_fx(&mut fx2);
    let mut live = vec![0f32; (out.len() - cut) + (h - from) as usize * CH];
    let mut chunk = vec![0f32; AUDIO_BURST_FRAMES * CH];
    let mut f2 = Counting::new();
    for (i, o) in live.chunks_exact_mut(AUDIO_BURST_FRAMES * CH).enumerate() {
        r.render_frames_with(&mut f2, &mut fx2, from + i as i64 * BURST, &mut chunk);
        o.copy_from_slice(&chunk);
    }
    let warm = (h - from) as usize * CH;
    assert!(out[cut..] == live[warm..], "after the kept part the play is a play from its settle length before");
    // the junction is no click: its step is within the material's own
    let step = |s: &[f32], i: usize| (s[i] - s[i - CH]).abs();
    let own = (CH..out.len()).filter(|i| *i != cut).map(|i| step(&out, i)).fold(0f32, f32::max);
    assert!(step(&out, cut) <= own, "junction step {} against the material's {}", step(&out, cut), own);
}

/// t1933: a loop keeps its first lap as linear play and its second as a lap; from the third lap on
/// every chunk comes from the store and each lap sounds as the second did.
#[test]
fn t1933_loop_laps_are_kept_apart_and_replayed() {
    let p = project(vec![clip(1, 1, 0, 0, 4 * SEC, vec![reverb(0.5)])]);
    let r = AudioRenderer::new(&p);
    let mut fx = FxStateStore::new();
    r.sync_fx(&mut fx);
    let mut f = Counting::new();
    let mut mix = MixPlay::default();
    // a region off the burst grid, 30.5 bursts long
    let region = (SEC + 696, SEC + 696 + 30 * BURST + 512);
    let len = region.1 - region.0;
    let laps = 6;
    let n = ((laps * len) / BURST) as usize;
    let (out, replayed) = play(&mut mix, &r, &mut f, &mut fx, Some(region), region.0, n);
    let runs = mix.store.runs();
    assert_eq!(runs.linear, vec![region.0..region.1], "the first lap is linear play");
    let whole: Vec<std::ops::Range<i64>> = std::iter::once(region.0..region.1).collect();
    assert_eq!(runs.laps, vec![(region, whole)], "the second lap is the lap");
    // laps 2.. come from the store: at most the chunks that straddle into lap 2 rendered live
    assert!(replayed as i64 >= n as i64 - (2 * len) / BURST - 2, "replayed {replayed} of {n}");
    let lap = |k: i64| {
        let a = (k * len) as usize * CH;
        &out[a..a + len as usize * CH]
    };
    for k in 2..laps - 1 {
        assert!(lap(k) == lap(1), "lap {k} differs from the kept lap");
    }
    // the pieces of a chunk that straddles the wrap
    let ps = pieces(region.1 - 100, BURST, Some(region));
    assert_eq!(ps, vec![(MixCtx::Linear, region.1 - 100, 100), (MixCtx::Lap(region.0, region.1), region.0, BURST - 100)]);
}

/// t1940: a rebuild costs the audio render thread what the edit changed. The swap synced every
/// event (250–425 ms on 2400 events under load, against a 341 ms ring: run 1 of the orchestrator's
/// workspace run heard the sound stop), and asked a reel's pre-roll per placement. Now one edited
/// event is one sync, the rest keep their processors untouched, and the sound after the swap is
/// what a full sync gives.
#[test]
fn t1940_a_rebuild_syncs_what_the_edit_changed() {
    let clips: Vec<ClipEvent> =
        (0..50).map(|i| clip(i + 1, 1 + (i % 2), (i % 3) as u32, i as i64 * SEC / 4, SEC, vec![reverb(0.3)])).collect();
    let p = project(clips);
    let r1 = AudioRenderer::new(&p);
    let mut fx = FxStateStore::new();
    r1.sync_fx(&mut fx);
    assert_eq!(fx.clip_syncs(), 50);
    let mut f = Counting::new();
    let mut warm = vec![0f32; 20 * AUDIO_BURST_FRAMES * CH];
    r1.render_frames_with(&mut f, &mut fx, 0, &mut warm);

    let mut p2 = p.clone();
    p2.clips[7].gain_db = -3.0;
    let r2 = AudioRenderer::new(&p2);
    let before = fx.clip_syncs();
    r2.sync_fx_since(Some(&r1), &mut fx);
    assert_eq!(fx.clip_syncs() - before, 1, "only the edited event is synced");

    // the same continuation as after syncing everything
    let mut full = FxStateStore::new();
    r1.sync_fx(&mut full);
    r1.render_frames_with(&mut Counting::new(), &mut full, 0, &mut warm.clone());
    r2.sync_fx(&mut full);
    let (mut a, mut b) = (vec![0f32; 20 * AUDIO_BURST_FRAMES * CH], vec![0f32; 20 * AUDIO_BURST_FRAMES * CH]);
    r2.render_frames_with(&mut f, &mut fx, 20 * BURST, &mut a);
    r2.render_frames_with(&mut Counting::new(), &mut full, 20 * BURST, &mut b);
    assert!(a == b, "a partial sync continues as a full one");
}

/// The bytes of one kept burst.
const UNIT_BYTES: usize = AUDIO_BURST_FRAMES * CH * 4;

/// Keep burst `k` of `ctx` (scene 0); whether it was admitted.
fn keep(s: &mut MixStore, ctx: MixCtx, k: i64) -> bool {
    s.write(ctx, k * BURST, &vec![k as f32; AUDIO_BURST_FRAMES * CH], &|_| 0)
}

/// t2601 (loop-keeps-its-lap, found-defects 550): a loop longer than the store keeps a stable
/// PREFIX of its lap. The bursts of other contexts go first; a lap burst later than every kept one
/// is not admitted, an earlier one takes the latest's place; a linear burst finds no room in a
/// store full of the lap. Without a loop the store is the plain LRU it was.
#[test]
fn t2601_a_loop_longer_than_the_store_keeps_its_prefix() {
    let region = (0, 10 * BURST);
    let lap = MixCtx::Lap(region.0, region.1);
    let held = |s: &MixStore| s.runs();
    let mut s = MixStore::new(4 * UNIT_BYTES);
    for k in 100..102 {
        assert!(keep(&mut s, MixCtx::Linear, k));
    }
    s.retain_laps(Some(region));
    let admitted: Vec<bool> = (0..10).map(|k| keep(&mut s, lap, k)).collect();
    assert_eq!(admitted, [true, true, true, true, false, false, false, false, false, false]);
    assert!(held(&s).linear.is_empty(), "the linear bursts went first");
    assert_eq!(held(&s).laps, vec![(region, vec![0..4 * BURST])], "the lap's prefix");
    // the next laps: the same prefix stays, whatever is written past it
    for _ in 0..3 {
        for k in 0..10 {
            keep(&mut s, lap, k);
        }
    }
    assert_eq!(held(&s).laps, vec![(region, vec![0..4 * BURST])]);
    assert!(!keep(&mut s, MixCtx::Linear, 200), "no room for linear play over the lap's prefix");
    // an edit dropped burst 1: it comes back; a burst dropped from the middle frees room for the latest
    s.retain_valid(&|k| if k == 1 { 9 } else { 0 });
    assert_eq!(s.len(), 3);
    assert!(keep(&mut s, lap, 6), "room again");
    assert!(s.write(lap, BURST, &vec![1.0; AUDIO_BURST_FRAMES * CH], &|k| if k == 1 { 9 } else { 0 }));
    assert_eq!(held(&s).laps, vec![(region, vec![0..4 * BURST])], "an earlier burst takes the latest's place");
    // no loop: least recently heard first
    let mut s = MixStore::new(4 * UNIT_BYTES);
    for k in 0..10 {
        assert!(keep(&mut s, MixCtx::Linear, k));
    }
    assert_eq!(held(&s).linear, vec![6 * BURST..10 * BURST]);
}

/// t2602 (loop-keeps-its-lap, found-defects 550): the kept mix asks the memory ledger for its loop,
/// as the frames cache and the memos do. Its bound grows to the loop's bursts and an eighth; a store
/// that never asked keeps its base; a claim takes the growth back and the store evicts to its base
/// when its thread fits it, keeping the lap's prefix; the claim's end gives the growth back.
#[test]
fn t2602_the_kept_mix_grows_by_the_ledger_for_its_loop() {
    let l: &'static kt_time::mem::Ledger = Box::leak(Box::new(kt_time::mem::Ledger::new()));
    l.set_probe(|| Some(1 << 30));
    let base = 128 * UNIT_BYTES; // 1 MB
    let budget = MixBudget::with_ledger(base, l);
    let other = MixBudget::with_ledger(base, l);
    let region = (0, 256 * BURST); // 2 MB of bursts
    let want = mix_want(Some(region));
    assert!(want >= 257 * UNIT_BYTES * 9 / 8 && want % (16 << 20) == 0, "every burst and an eighth, rounded: {want}");
    assert_eq!(mix_want(None), 0);
    assert_eq!(budget.ask(want), want);
    assert_eq!(other.cap(), base, "only the asker grows");
    let mut s = MixStore::with_budget(budget.clone());
    s.retain_laps(Some(region));
    let lap = MixCtx::Lap(region.0, region.1);
    assert!((0..256).all(|k| keep(&mut s, lap, k)), "the whole loop is kept");
    assert_eq!(budget.held(), (256 * UNIT_BYTES) as u64, "reported to the ledger");
    let claim = l.claim();
    assert_eq!(budget.cap(), base);
    s.fit();
    assert_eq!(s.len(), 128);
    assert_eq!(s.runs().laps, vec![(region, vec![0..128 * BURST])], "the prefix stays");
    drop(claim);
    assert_eq!(budget.cap(), want, "the claim gone, the growth is back");
}

/// t2603 (loop-keeps-its-lap, found-defects 550): the forward render over a loop three times longer
/// than the store. Before, an LRU over the cycle replayed nothing; now from the third lap on each
/// lap replays the kept prefix — no source read for it — and it sounds as the kept second lap did
/// (but the chunk that straddles the wrap, live as the lap's end before it is).
#[test]
fn t2603_a_long_loop_replays_its_prefix_every_lap() {
    let p = project(vec![clip(1, 1, 0, 0, 4 * SEC, vec![reverb(0.5)])]);
    let r = AudioRenderer::new(&p);
    let region = (SEC + 696, SEC + 696 + 30 * BURST + 512);
    let len = region.1 - region.0;
    let laps = 6;
    let n = ((laps * len) / BURST) as usize;
    let run = |tell: bool| {
        let mut fx = FxStateStore::new();
        r.sync_fx(&mut fx);
        let mut f = Counting::new();
        let mut mix = MixPlay::with_store(MixStore::new(12 * UNIT_BYTES));
        if tell {
            mix.store.retain_laps(Some(region));
        }
        let mut out = Vec::new();
        let mut per_lap = vec![0usize; laps as usize + 1];
        let mut chunk = vec![0f32; AUDIO_BURST_FRAMES * CH];
        for i in 0..n {
            let head = region.0 + i as i64 * BURST;
            let o = mix.render(&r, &mut f, &mut fx, Some(region), head, &mut chunk);
            per_lap[((head - region.0) / len) as usize] += usize::from(o.replayed);
            out.extend_from_slice(&chunk);
        }
        (out, per_lap, mix.store.runs())
    };
    let (_, plain, _) = run(false);
    assert!(plain.iter().all(|&r| r == 0), "an LRU over the cycle replays nothing: {plain:?}");
    let (out, per_lap, runs) = run(true);
    let kept = &runs.laps[0].1;
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].start, region.0, "a prefix of the lap");
    let prefix = (kept[0].end - region.0) as usize;
    assert!(prefix as i64 >= 9 * BURST, "most of the bound: {prefix}");
    for (k, &rep) in per_lap.iter().enumerate().take(laps as usize).skip(2) {
        assert!(rep >= 8, "lap {k} replayed {rep} chunks: {per_lap:?}");
        // the chunk that straddles the wrap renders live (the lap before's end is past the prefix);
        // from the first chunk inside the lap on, the prefix's chunks are handed out
        let start = k as i64 * len;
        let o = ((start + BURST - 1) / BURST * BURST - start) as usize;
        let (a, b) = ((start as usize + o) * CH, (len as usize + o) * CH);
        // to the last chunk wholly inside the prefix (the one past its end is live)
        let m = (prefix - o) / BURST as usize * BURST as usize * CH;
        let first = (0..m).find(|&i| out[a + i] != out[b + i]);
        assert!(first.is_none(), "lap {k}'s prefix is the kept second lap's: first differs at frame {:?} of {} (o {o}, prefix {prefix}, {per_lap:?})", first.map(|i| i / CH), m / CH);
    }
}
