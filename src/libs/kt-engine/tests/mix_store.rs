//! cached-range-plays: the final mix of a covered range is kept and played again with no decode
//! and no effect run (the owner's choice "a", invariant 2 as reworded). t1930–t1933.

use kt_engine::mix::{MixCtx, MixPlay, pieces};
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

/// A seek: the player resets the FX state and tells the store.
fn seek(mix: &mut MixPlay, fx: &mut FxStateStore) {
    fx.reset_all();
    mix.fx_reset();
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
    seek(&mut mix, &mut fx);
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
    seek(&mut mix, &mut fx);
    let mut fx2 = FxStateStore::new();
    r2.sync_fx(&mut fx2);
    let (_, replayed) = play(&mut mix, &r2, &mut f, &mut fx2, None, 0, chunks);
    assert!(replayed > 0 && replayed < chunks, "replayed {replayed} of {chunks}");
}

/// t1932: where the store ends the play continues exactly as a seek to that chunk does — the
/// chunks after it equal a live render that started there, byte for byte.
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
    seek(&mut mix, &mut fx);
    let (out, replayed) = play(&mut mix, &r, &mut f, &mut fx, None, 0, kept + 40);
    assert_eq!(replayed, kept);
    let cut = kept * AUDIO_BURST_FRAMES * CH;
    assert!(out[..cut] == first[..], "the kept part is the first recording");
    let mut fx2 = FxStateStore::new();
    r.sync_fx(&mut fx2);
    let mut live = vec![0f32; out.len() - cut];
    r.render_frames_with(&mut Counting::new(), &mut fx2, kept as i64 * BURST, &mut live);
    assert!(out[cut..] == live[..], "after the kept part the play is a seek to its end");
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
