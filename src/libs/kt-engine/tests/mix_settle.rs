//! kept-mix-settle: a kept range sounds as a continuous play does (found-defects 390, kkt-26.11.10
//! J1). Before, the live chunk after a replayed run reset the effects and was kept: in a loop the
//! lap it opened rang the timeline before the region instead of the previous lap, and was replayed
//! on every lap. t2370–t2373.

use kt_engine::mix::{MixCtx, MixPlay};
use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::AudioBurst;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};
use std::sync::Arc;

const CH: usize = AUDIO_CHANNELS;
const BURST: i64 = AUDIO_BURST_FRAMES as i64;
const SEC: i64 = 48_000;

/// Synthetic sources; the source bursts in `missing` read as absent (a starved cache).
struct Src {
    inner: DirectFetcher,
    missing: std::ops::Range<i64>,
}

impl Src {
    fn new() -> Src {
        let mut inner = DirectFetcher::new(None);
        inner.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Ramp)));
        inner.add_source(
            MediaId(2),
            Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 220.0 })),
        );
        Src { inner, missing: 0..0 }
    }
}

impl AudioFetcher for Src {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<AudioBurst>> {
        if self.missing.contains(&idx) {
            return None;
        }
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
    p.ensure_tracks(LaneKind::Audio, 2);
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
        fx,
        opacity: 1.0,
        fx_off: false,
    }
}

fn reverb(mix: f64) -> FxInstance {
    kt_plugins_builtin::install();
    FxInstance::plugin_with("slowed_reverb", &[1.0, mix, 60.0, 0.4, 0.7, 1.0, 0.0, 0.0, 1.0]).unwrap()
}

/// A chunk as the player renders it with no store: folded into an armed loop from its start on.
fn direct(r: &AudioRenderer, f: &mut dyn AudioFetcher, fx: &mut FxStateStore, region: Option<(i64, i64)>, head: i64, out: &mut [f32]) {
    match region {
        Some((rs, re)) if head >= rs => r.render_looped_with(f, fx, rs..re, head, out),
        _ => r.render_frames_with(f, fx, head, out),
    };
}

/// The continuous play of `n` chunks from a seek to `from`, with no store.
fn continuous(r: &AudioRenderer, region: Option<(i64, i64)>, from: i64, n: usize) -> Vec<f32> {
    let mut fx = FxStateStore::new();
    r.sync_fx(&mut fx);
    let mut f = Src::new();
    let mut out = vec![0f32; n * AUDIO_BURST_FRAMES * CH];
    for (i, o) in out.chunks_exact_mut(AUDIO_BURST_FRAMES * CH).enumerate() {
        direct(r, &mut f, &mut fx, region, from + i as i64 * BURST, o);
    }
    out
}

/// `n` chunks from `head` through the store; the samples and each chunk's outcome.
fn play(
    mix: &mut MixPlay,
    r: &AudioRenderer,
    f: &mut dyn AudioFetcher,
    fx: &mut FxStateStore,
    region: Option<(i64, i64)>,
    head: i64,
    n: usize,
) -> (Vec<f32>, Vec<kt_engine::mix::ChunkOutcome>) {
    let mut out = Vec::with_capacity(n * AUDIO_BURST_FRAMES * CH);
    let mut chunk = vec![0f32; AUDIO_BURST_FRAMES * CH];
    let mut outcomes = Vec::new();
    for i in 0..n {
        outcomes.push(mix.render(r, f, fx, region, head + i as i64 * BURST, &mut chunk));
        out.extend_from_slice(&chunk);
    }
    (out, outcomes)
}

fn seek(mix: &mut MixPlay, r: &AudioRenderer, fx: &mut FxStateStore, head: i64) {
    fx.reset_all();
    mix.fx_reset(r, head);
}

/// t2370: his rendereg loop (kkt-26.11.10 J1). The first 7 chunks of a 163 ms loop are kept from an
/// earlier play; the loop then replays them and goes live 634 frames before the wrap. Every lap
/// heard equals the continuous play from the seek byte for byte — the first two as they are, the
/// later ones as the kept second lap — and the effects went on through the replayed frames (no
/// reset), so the kept lap rings the previous lap.
#[test]
fn t2370_a_partly_kept_loop_keeps_the_continuous_laps() {
    let p = project(vec![clip(1, 1, 0, 0, 4 * SEC, vec![reverb(0.5)])]);
    let r = AudioRenderer::new(&p);
    let mut fx = FxStateStore::new();
    r.sync_fx(&mut fx);
    let mut f = Src::new();
    let mut mix = MixPlay::default();
    let rs = SEC + 696;
    let len = 7802; // his 4690.339..4690.502
    let region = (rs, rs + len);
    // the earlier play: from a seek to the region start, seven chunks, no loop
    seek(&mut mix, &r, &mut fx, rs);
    play(&mut mix, &r, &mut f, &mut fx, None, rs, 7);
    assert_eq!(mix.store.runs().linear, vec![rs..rs + 7 * BURST]);
    // the loop
    seek(&mut mix, &r, &mut fx, rs);
    let laps = 6;
    let n = ((laps * len) / BURST) as usize;
    let (out, outs) = play(&mut mix, &r, &mut f, &mut fx, Some(region), rs, n);
    assert!(outs[..7].iter().all(|o| o.replayed), "the kept part is replayed");
    assert!(!outs[7].replayed);
    let settle = r.settle_frames(rs + 7 * BURST..rs + 8 * BURST);
    assert!(settle > 7 * BURST, "scene guard: the replayed run is within the settle length ({settle})");
    assert_eq!(outs[7].warmed, 7 * BURST, "the effects went on through the replayed frames");
    // what was heard is the continuous play
    let cont = continuous(&r, Some(region), rs, (2 * len / BURST + 2) as usize);
    let live = outs.iter().rposition(|o| !o.replayed).unwrap() + 1;
    let cut = live * AUDIO_BURST_FRAMES * CH;
    assert!(out[..cut] == cont[..cut], "the first two laps are the continuous play: the second rings the first");
    // from there each frame is the kept second lap's
    let lap1 = |f: i64| &cont[(f * CH as i64) as usize..(f * CH as i64) as usize + CH];
    for fr in (live as i64 * BURST)..(n as i64 * BURST) {
        let pos = (fr - len).rem_euclid(len) + len;
        let o = (fr * CH as i64) as usize;
        assert!(out[o..o + CH] == *lap1(pos), "frame {fr} is not the kept second lap's");
    }
    let runs = mix.store.runs();
    let whole: Vec<std::ops::Range<i64>> = std::iter::once(region.0..region.1).collect();
    assert_eq!(runs.laps, vec![(region, whole)], "the lap is kept");
}

/// t2371: a chunk the source starved is not kept, and neither is any chunk within the settle
/// length after it — the hole rings on in the effects. From there on the play is kept again.
#[test]
fn t2371_no_chunk_is_kept_within_the_settle_length_after_a_hole() {
    let p = project(vec![clip(1, 1, 0, 0, 4 * SEC, vec![reverb(0.5)])]);
    let r = AudioRenderer::new(&p);
    let mut fx = FxStateStore::new();
    r.sync_fx(&mut fx);
    let mut f = Src::new();
    // source burst 30 is missing on this play: the chunk at 30 bursts reads it
    f.missing = 30..31;
    let mut mix = MixPlay::default();
    seek(&mut mix, &r, &mut fx, 0);
    let n = 120usize;
    let (_, outs) = play(&mut mix, &r, &mut f, &mut fx, None, 0, n);
    let hole = outs.iter().position(|o| o.stats.underruns > 0).expect("the starved chunk") as i64;
    let settle = r.settle_frames(hole * BURST..(hole + 1) * BURST);
    assert!(settle > BURST, "scene guard: the reverb keeps a state ({settle})");
    let resume = ((hole + 1) * BURST + settle + BURST - 1) / BURST * BURST;
    assert_eq!(
        mix.store.runs().linear,
        vec![0..hole * BURST, resume..n as i64 * BURST],
        "kept up to the hole and again only past its settle length"
    );
}

/// t2372: an edit drops the kept mix around one event of a 3 s loop; the next play replays a long
/// run and goes live in the second lap. The effects are reset and played through their settle
/// length before the chunk THROUGH THE LOOP (not the timeline), and what is rendered then is that
/// play's, byte for byte.
#[test]
fn t2372_a_long_replay_warms_the_lap_through_its_settle_length() {
    let rs = SEC;
    let len = 3 * SEC;
    let region = (rs, rs + len);
    let a = clip(1, 1, 0, 0, 6 * SEC, vec![reverb(0.5)]);
    let b = clip(2, 2, 1, rs + 5 * SEC / 2, SEC / 4, vec![reverb(0.3)]);
    let p = project(vec![a.clone(), b.clone()]);
    let r = AudioRenderer::new(&p);
    let mut fx = FxStateStore::new();
    r.sync_fx(&mut fx);
    let mut f = Src::new();
    let mut mix = MixPlay::default();
    let n = ((3 * len) / BURST) as usize;
    seek(&mut mix, &r, &mut fx, rs);
    play(&mut mix, &r, &mut f, &mut fx, Some(region), rs, n);
    // the edit
    let mut b2 = b.clone();
    b2.fx = vec![reverb(0.8)];
    let p2 = project(vec![a, b2]);
    let r2 = AudioRenderer::new(&p2);
    r2.sync_fx_since(Some(&r), &mut fx);
    mix.revalidate(&r2);
    seek(&mut mix, &r2, &mut fx, rs);
    let (out, outs) = play(&mut mix, &r2, &mut f, &mut fx, Some(region), rs, n);
    // the exit in the second lap
    let lap1 = ((rs + len - rs) / BURST) as usize + 1;
    let i = (lap1..n).find(|&i| !outs[i].replayed && outs[i - 1].replayed).expect("a live chunk in the second lap");
    let h = rs + i as i64 * BURST;
    assert!(h - BURST >= rs + len, "scene guard: the exit's warm-up lies in the lap");
    let tl = rs + (h - rs).rem_euclid(len);
    let settle = r2.settle_frames(tl..tl + BURST);
    assert!(settle > BURST, "scene guard: the chains heard there keep a state ({settle})");
    let from = h - (settle + BURST - 1) / BURST * BURST;
    assert_eq!(outs[i].warmed, h - from, "reset and played through the settle length");
    assert!(from >= rs + len, "scene guard: the warm-up does not reach the first lap");
    // the reference: a seek to `from` on the loop's head, played on
    let live = (i..n).take_while(|&k| !outs[k].replayed).count();
    let reference = continuous(&r2, Some(region), from, ((h - from) / BURST) as usize + live);
    let warm = ((h - from) as usize) * CH;
    let a = i * AUDIO_BURST_FRAMES * CH;
    let m = live * AUDIO_BURST_FRAMES * CH;
    assert!(out[a..a + m] == reference[warm..warm + m], "the live chunks are the warmed play's");
    // and they were kept under the lap
    let runs = mix.store.runs();
    let held = runs.laps.iter().find(|(reg, _)| *reg == region).map(|(_, v)| v.clone()).unwrap_or_default();
    assert!(held.iter().any(|run| run.start <= tl && run.end >= tl + BURST), "the warmed chunk is kept: {held:?}");
    let _ = MixCtx::Linear;
}

/// t2373: a track stage has no seek pre-roll. After a seek where its lane still rings, the chunks
/// within its reach are not kept (they play its cold start); a seek where nothing rings keeps from
/// the head.
#[test]
fn t2373_a_track_stage_after_a_seek_is_kept_once_it_settled() {
    kt_plugins_builtin::install();
    let mut p = project(vec![clip(1, 2, 0, 0, 2 * SEC, vec![])]);
    let id = p.track_at(LaneKind::Audio, 0).unwrap().id;
    p.track_mut(id).unwrap().fx = vec![reverb(0.5)];
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);
    let mut fx = FxStateStore::new();
    r.sync_fx(&mut fx);
    let mut f = Src::new();
    let mut mix = MixPlay::default();
    let h = SEC + 37 * BURST;
    let cold = r.cold_after_seek(h);
    assert!(cold > 0, "the stage rings at {h}");
    seek(&mut mix, &r, &mut fx, h);
    let n = 80usize;
    play(&mut mix, &r, &mut f, &mut fx, None, h, n);
    let first = mix.store.runs().linear.first().cloned().expect("kept once it settled");
    assert!(first.start >= h + cold, "a cold stage was kept: {first:?}, cold until {}", h + cold);
    assert!(first.start < h + cold + BURST);
    // far past the lane's sound: warm at once
    let far = 10 * SEC;
    assert_eq!(r.cold_after_seek(far), 0);
    seek(&mut mix, &r, &mut fx, far);
    play(&mut mix, &r, &mut f, &mut fx, None, far, 4);
    assert!(mix.store.runs().linear.iter().any(|run| run.start == far), "kept from the head");
}
