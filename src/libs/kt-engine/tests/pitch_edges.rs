//! Tests t1105-t1114 (pitch-event-edges, kkt-26.11.01 item 15): pitch across the edges of
//! short consecutive events.
//!
//! The owner: fifteen short events in a row sound right without pitch and torn with it. What is
//! proven: a run of events cut from one continuous material, with pitch on each, joins without
//! a dip and without a step at the cuts — as one long event with the same pitch does.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore};
use std::sync::Arc;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const TONE: f64 = 440.0;

fn settings(engine: PitchEngine) -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: engine,
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

fn event(id: u64, start: i64, len: i64, source_in: i64, fx: Vec<FxInstance>) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick::from_audio_frames(start),
        duration: Tick::from_audio_frames(len),
        source_in: Tick::from_audio_frames(source_in),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn pitch(st: f64) -> Vec<FxInstance> {
    kt_plugins_builtin::install();
    vec![FxInstance::plugin_with("pitch_shift", &[st, 0.0]).unwrap()]
}

fn source() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: TONE })),
    );
    f
}

fn render(p: &Project, frames: i64) -> Vec<f32> {
    render_sliced(p, 0, frames, frames)
}

/// The range [from, from + frames) rendered from a cold store in pieces of `piece` frames.
fn render_sliced(p: &Project, from: i64, frames: i64, piece: i64) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut f = source();
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = Vec::with_capacity(frames as usize * CH);
    let mut pos = from;
    while pos < from + frames {
        let n = piece.min(from + frames - pos);
        let mut buf = vec![0f32; n as usize * CH];
        let stats = r.render_frames_with(&mut f, &mut store, pos, &mut buf);
        assert_eq!(stats.underruns, 0, "source fully available");
        out.extend_from_slice(&buf);
        pos += n;
    }
    out
}

/// Events at the given (start, len, source_in) on one track, all with the same chain.
fn events_project(engine: PitchEngine, evs: &[(i64, i64, i64)], fx: Vec<FxInstance>) -> Project {
    let mut p = Project::new(settings(engine));
    p.media.push(MediaRef::new(MediaId(1), "synth:tone"));
    for (i, &(start, len, src)) in evs.iter().enumerate() {
        p.clips.push(event(i as u64 + 1, start, len, src, fx.clone()));
    }
    p.validate().unwrap();
    p
}

const ENGINES: [PitchEngine; 2] = [PitchEngine::Wsola, PitchEngine::Vocoder];
const SLICES: [i64; 2] = [1024, 333];

/// RMS of the left channel over [from, from + w).
fn rms(buf: &[f32], from: i64, w: i64) -> f64 {
    let s: f64 = (from..from + w).map(|i| (buf[i as usize * CH] as f64).powi(2)).sum();
    (s / w as f64).sqrt()
}

/// The deepest 64-frame window within 1 024 frames of each join, as a share of the steady level
/// of the event the window lies in (the RMS of its middle), and the largest sample step (second
/// difference) there.
fn worst_join(buf: &[f32], evs: &[(i64, i64, i64)]) -> (f64, f64) {
    const W: i64 = 64;
    let steady: Vec<f64> = evs.iter().map(|&(s, l, _)| rms(buf, s + 1024, l - 2048)).collect();
    let mut dip = f64::INFINITY;
    let mut step = 0f64;
    for k in 1..evs.len() {
        let at = evs[k].0;
        let mut f = at - 1024;
        while f + W <= at + 1024 {
            let mid = f + W / 2;
            let e = evs.iter().position(|&(s, l, _)| mid >= s && mid < s + l).unwrap_or(k);
            dip = dip.min(rms(buf, f, W) / steady[e]);
            f += W / 2;
        }
        for i in at - 1024..at + 1024 {
            let (a, m, c) = (buf[(i - 1) as usize * CH], buf[i as usize * CH], buf[(i + 1) as usize * CH]);
            step = step.max((a - 2.0 * m + c).abs() as f64);
        }
    }
    (dip, step)
}

const N: usize = 15;
const LEN: i64 = 4800; // 100 ms
const SRC0: i64 = 48_000;

/// t1105: fifteen 100 ms pieces of one pitched event (a split in place) sound as the event — byte
/// for byte, whatever the slicing. Before this change every join had a dip (WSOLA +7: ≈ 320 frames
/// of total silence before it, −7: ≈ 250 after it) and a step ≈ 110× the uncut one.
#[test]
fn t1105_a_split_of_a_pitched_event_is_the_event() {
    let total = N as i64 * LEN;
    let cut: Vec<(i64, i64, i64)> = (0..N as i64).map(|i| (i * LEN, LEN, SRC0 + i * LEN)).collect();
    for engine in ENGINES {
        for st in [7.0, -7.0] {
            let whole = render(&events_project(engine, &[(0, total, SRC0)], pitch(st)), total);
            let p = events_project(engine, &cut, pitch(st));
            let one = render(&p, total);
            let (dip, step) = worst_join(&one, &cut);
            let (_, whole_step) = worst_join(&whole, &cut);
            assert!(
                one == whole,
                "{engine:?} {st:+}: the split differs from the uncut event (worst dip {dip:.3} of \
                 the steady level, step {step:.4} against {whole_step:.4} uncut)"
            );
            for piece in SLICES {
                assert!(render_sliced(&p, 0, total, piece) == whole, "{engine:?} {st:+}: pieces of {piece}");
            }
            assert!(render(&p, total) == one, "{engine:?} {st:+}: two runs differ");
        }
    }
}

/// t1106: fifteen pitched events of OTHER material (no event continues another): the joins are
/// cuts, as without pitch, but no event has a dip at its edges.
#[test]
fn t1106_pitched_events_of_other_material_have_no_dip() {
    let total = N as i64 * LEN;
    let evs: Vec<(i64, i64, i64)> =
        (0..N as i64).map(|i| (i * LEN, LEN, SRC0 + i * (LEN + 1777))).collect();
    for engine in ENGINES {
        for st in [7.0, -7.0] {
            let p = events_project(engine, &evs, pitch(st));
            let one = render(&p, total);
            let (dip, _) = worst_join(&one, &evs);
            assert!(dip >= 0.5, "{engine:?} {st:+}: a window at a join fell to {dip:.3} of the steady level");
            for piece in SLICES {
                assert!(render_sliced(&p, 0, total, piece) == one, "{engine:?} {st:+}: pieces of {piece}");
            }
        }
    }
}

fn fx(id: &str, vals: &[f64]) -> FxInstance {
    kt_plugins_builtin::install();
    FxInstance::plugin_with(id, vals).unwrap()
}

/// A plain echo of 100 ms without feedback: its output past `input end + 100 ms` is exactly zero.
fn echo() -> FxInstance {
    fx("delay", &[100.0, 0.0, 0.5, 0.0])
}

/// t1107: nothing outside the event is heard; an echo after the pitch hears only the event.
#[test]
fn t1107_nothing_outside_the_event_is_heard() {
    let (start, len, src) = (24_000i64, 9_600i64, 96_000i64);
    let total = 48_000i64;
    for engine in ENGINES {
        let out = render(&events_project(engine, &[(start, len, src)], pitch(7.0)), total);
        let outside = (0..start).chain(start + len..total);
        assert!(
            outside.clone().all(|i| out[i as usize * CH] == 0.0 && out[i as usize * CH + 1] == 0.0),
            "{engine:?}: the material around the event leaked into the output"
        );
        assert!(rms(&out, start, 512) > 0.1 && rms(&out, start + len - 512, 512) > 0.1, "{engine:?}: edges sound");
        // the echo after the pitch: past `end + 100 ms` only the echo's own silence remains — the
        // context the pitch read past the out-point never reached the echo
        let chain = vec![pitch(7.0).remove(0), echo()];
        let out = render(&events_project(engine, &[(start, len, src)], chain), total);
        let echo_end = start + len + 4_800;
        assert!(rms(&out, start + len, 4_800) > 0.05, "{engine:?}: the echo rings after the event");
        assert!(
            (echo_end + 64..total).all(|i| out[i as usize * CH] == 0.0),
            "{engine:?}: the echo carried material from outside the event"
        );
        assert!((0..start).all(|i| out[i as usize * CH] == 0.0), "{engine:?}: pre-roll leaked into the echo");
    }
}

struct Recording {
    inner: DirectFetcher,
    log: Vec<(MediaId, i64)>,
}

impl AudioFetcher for Recording {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<kt_media::AudioBurst>> {
        self.log.push((media, idx));
        self.inner.audio_burst(media, idx)
    }
}

/// t1108: the prefetch orders the context — the pre-roll before the in-point and the look-ahead
/// past the out-point — forward and reversed.
#[test]
fn t1108_prefetch_orders_the_context() {
    let (start, len, src) = (48_000i64, 9_600i64, 96_000i64);
    for reversed in [false, true] {
        let mut p = events_project(PitchEngine::Wsola, &[(start, len, src)], pitch(7.0));
        p.clips[0].reversed = reversed;
        let r = AudioRenderer::new(&p);
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut reads = 0usize;
        for &(pos, frames) in &[(start - 4_096, 8_192i64), (start, 4_096), (start + 3_000, 9_000), (start + len - 1_000, 2_048)] {
            store.reset_all();
            let mut f = Recording { inner: source(), log: Vec::new() };
            let mut out = vec![0f32; frames as usize * CH];
            r.render_frames_with(&mut f, &mut store, pos, &mut out);
            let wanted = r.wanted_audio_bursts(pos..pos + frames);
            for &(media, idx) in &f.log {
                assert!(
                    wanted.iter().any(|(m, rg)| *m == media && rg.contains(&idx)),
                    "reversed {reversed}: burst {idx} read at {pos}..{} is not in the prefetch request",
                    pos + frames
                );
            }
            reads += f.log.len();
        }
        assert!(reads > 0);
        // the context is really read: a cold start at the in-point reads before it
        store.reset_all();
        let mut f = Recording { inner: source(), log: Vec::new() };
        let mut out = vec![0f32; 1_024 * CH];
        r.render_frames_with(&mut f, &mut store, start, &mut out);
        let burst = kt_time::AUDIO_BURST_FRAMES as i64;
        let (lo, hi) = if reversed { (src + len, src + len + 4_000) } else { (src - 4_000, src) };
        assert!(
            f.log.iter().any(|(_, i)| *i * burst < hi && (*i + 1) * burst > lo),
            "reversed {reversed}: the pre-roll did not read the material beyond the event's start"
        );
    }
}

/// A fetcher that is silent outside the source frames `[lo, hi)`.
struct Windowed {
    inner: DirectFetcher,
    lo: i64,
    hi: i64,
}

impl AudioFetcher for Windowed {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<kt_media::AudioBurst>> {
        let b = self.inner.audio_burst(media, idx)?;
        let n = kt_time::AUDIO_BURST_FRAMES as i64;
        let mut out = (*b).clone();
        for i in 0..n {
            let f = idx * n + i;
            if f < self.lo || f >= self.hi {
                for ch in 0..CH {
                    out.data[i as usize * CH + ch] = 0.0;
                }
            }
        }
        Some(Arc::new(out))
    }
}

/// t1109: who reads across edges — the static `chain_context` and the processor agree; the limiter
/// renders an event the same whatever lies around it, pitch does not.
#[test]
fn t1109_who_reads_across_edges() {
    let limiter = || fx("limiter", &[-6.0, -6.0, 5.0, 80.0]);
    let p7 = || pitch(7.0).remove(0);
    let chains: Vec<(&str, Vec<FxInstance>, bool)> = vec![
        ("pitch", vec![p7()], true),
        ("limiter", vec![limiter()], false),
        ("neutral pitch", pitch(0.0), false),
        ("limiter + pitch", vec![limiter(), p7()], true),
        ("pitch + echo", vec![p7(), echo()], true),
        ("echo + pitch", vec![echo(), p7()], true),
        ("slowed + pitch", vec![FxInstance::builtin("slowed_reverb"), p7()], true),
        ("slowed backwards + pitch", vec![slowed(-1.0), p7()], false),
    ];
    let (start, len, src) = (24_000i64, 9_600i64, 96_000i64);
    for (name, chain, expect) in chains {
        let auto = kt_fx::AutoProfiles::default();
        assert_eq!(kt_fx::chain_context(&chain, &auto), expect, "{name}: static");
        let p = events_project(PitchEngine::Wsola, &[(start, len, src)], chain.clone());
        let r = AudioRenderer::new(&p);
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        assert_eq!(store.get_mut(ClipId(1)).unwrap().reads_across_edges(), expect, "{name}: processor");
    }
    // the material around the event: heard by pitch's edges, never by the limiter's
    let around = |chain: Vec<FxInstance>, windowed: bool| -> Vec<f32> {
        let p = events_project(PitchEngine::Wsola, &[(start, len, src)], chain);
        let r = AudioRenderer::new(&p);
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; 48_000 * CH];
        let (lo, hi) = if windowed { (src, src + len) } else { (i64::MIN, i64::MAX) };
        r.render_frames_with(&mut Windowed { inner: source(), lo, hi }, &mut store, 0, &mut out);
        out
    };
    assert!(around(vec![limiter()], false) == around(vec![limiter()], true), "the limiter read past the event");
    assert!(around(vec![p7()], false) != around(vec![p7()], true), "pitch did not read past the event");
}

/// The fifteen pieces of t1105 (a split in place).
fn split_run() -> Vec<(i64, i64, i64)> {
    (0..N as i64).map(|i| (i * LEN, LEN, SRC0 + i * LEN)).collect()
}

/// t1110: a seek into the run equals a cold start there; continuous play through the same join is
/// the uncut event (t1105), which a seek does not have to be.
#[test]
fn t1110_a_seek_into_the_run_is_a_cold_start() {
    let evs = split_run();
    let total = N as i64 * LEN;
    for engine in ENGINES {
        let p = events_project(engine, &evs, pitch(7.0));
        let r = AudioRenderer::new(&p);
        for pos in [7 * LEN + 1_234, 7 * LEN] {
            let frames = 2 * LEN;
            // played from the start up to somewhere else, then sought
            let mut store = FxStateStore::new();
            r.sync_fx(&mut store);
            let mut f = source();
            let mut pre = vec![0f32; 3 * LEN as usize * CH];
            r.render_frames_with(&mut f, &mut store, 0, &mut pre);
            store.reset_all();
            let mut sought = vec![0f32; frames as usize * CH];
            r.render_frames_with(&mut f, &mut store, pos, &mut sought);
            let cold = render_sliced(&p, pos, frames, frames);
            assert!(sought == cold, "{engine:?}: the seek to {pos} differs from a cold start there");
            // the cold start at the join reads its context: no dip at the event's first frames
            let steady = rms(&cold, LEN + 1_024, LEN - 2_048);
            let first = rms(&cold, 0, 256);
            assert!(first >= 0.5 * steady, "{engine:?}: a cold start at {pos} dips ({first:.3} of {steady:.3})");
        }
        // continuous play through the same join is the uncut event
        let whole = render(&events_project(engine, &[(0, total, SRC0)], pitch(7.0)), total);
        assert!(render(&p, total) == whole);
    }
}

/// t1111: the run breaks exactly where it is not one material and one chain: an 8-frame gap on the
/// timeline, a source that skips 8 frames, a different chain, an echo with a tail.
#[test]
fn t1111_no_hand_off_where_the_run_breaks() {
    let total = N as i64 * LEN;
    for engine in ENGINES {
        let mut evs = split_run();
        // join 3: the 4th event starts 8 frames later (its material still continues); 8 frames are
        // 15 ticks — a single frame would round away in the tick grid
        evs[3] = (evs[3].0 + 8, LEN - 8, evs[3].2 + 8);
        // join 6: the 7th event's source skips 8 frames
        evs[6].2 += 8;
        let mut p = events_project(engine, &evs, pitch(7.0));
        // joins 9 and 10: the 10th event has another chain structure
        p.clips[9].fx.push(fx("gain", &[0.0]));
        // joins 12 and 13: the 13th event has an echo with a tail
        p.clips[12].fx.push(echo());
        p.validate().unwrap();
        // the skipped source breaks joins 6 AND 7: the 8th event no longer continues the 7th
        let broken = [3usize, 6, 7, 9, 10, 12, 13];
        let r = AudioRenderer::new(&p);
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; total as usize * CH];
        r.render_frames_with(&mut source(), &mut store, 0, &mut out);
        for j in 1..N {
            // a handed-on predecessor holds the reset processor it got in exchange
            let handed = store.get_mut(ClipId(j as u64)).unwrap().next_local().is_none();
            assert_eq!(handed, !broken.contains(&j), "{engine:?}: join {j}");
        }
        for piece in SLICES {
            assert!(render_sliced(&p, 0, total, piece) == out, "{engine:?}: pieces of {piece}");
        }
        let plain: Vec<(i64, i64, i64)> = evs.clone();
        let (dip, _) = worst_join(&out, &plain);
        assert!(dip >= 0.5, "{engine:?}: a join dips to {dip:.3}");
    }
}

/// t1112: a loop over the run — the wrap is a cut between two events, without silence on either
/// side; the laps after the first are equal; one piece == pieces.
#[test]
fn t1112_a_loop_over_the_run() {
    let evs = split_run();
    for engine in ENGINES {
        let p = events_project(engine, &evs, pitch(7.0));
        let whole_steady = rms(&render(&p, N as i64 * LEN), LEN, 10 * LEN);
        for region in [3 * LEN..10 * LEN, 3 * LEN + 1_700..9 * LEN + 2_100] {
            let span = region.end - region.start;
            let laps = 4;
            let looped = |step: i64| -> Vec<f32> {
                let r = AudioRenderer::new(&p);
                let mut store = FxStateStore::new();
                r.sync_fx(&mut store);
                let mut f = source();
                let frames = span * laps;
                let mut out = vec![0f32; frames as usize * CH];
                let (mut done, mut pos) = (0i64, region.start);
                while done < frames {
                    let n = step.min(frames - done);
                    r.render_looped_with(&mut f, &mut store, region.clone(), pos, &mut out[done as usize * CH..(done + n) as usize * CH]);
                    done += n;
                    pos += n;
                }
                out
            };
            let out = looped(1_024);
            assert!(looped(333) == out, "{engine:?} {region:?}: pieces of 333");
            let frames = span * laps;
            let mut f = 0;
            while f + 64 <= frames {
                let v = rms(&out, f, 64);
                assert!(v >= 0.5 * whole_steady, "{engine:?} {region:?}: frame {f} (lap {}) dips to {v:.3}", f / span);
                f += 32;
            }
            let lap = |k: i64| &out[(k * span) as usize * CH..((k + 1) * span) as usize * CH];
            assert!(lap(2) == lap(1) && lap(3) == lap(1), "{engine:?} {region:?}: the laps differ");
        }
    }
}

/// Slowed+Reverb as the owner has it on his copies: rate 1, a little reverb.
fn slowed(rate: f64) -> FxInstance {
    fx("slowed_reverb", &[rate, 0.1, 85.0, 0.2, 0.84, 1.0, 0.0, 0.0, 1.0])
}

/// A rate curve that stays 1 over the events and turns negative ten seconds of material past them
/// (the owner's curve does): a signed profile whose read range is forward.
fn signed_far_curve(src: i64) -> Vec<Vec<ParamCurve>> {
    let at = |f: i64, v: f64| AutoKey::at(Tick::from_audio_frames(f), v);
    let far = src + 480_000;
    vec![vec![ParamCurve {
        param: "rate".into(),
        keys: AutoKey::linear(vec![at(0, 1.0), at(far, 1.0), at(far + 24_000, -2.0), at(far + 48_000, 1.0)]),
        source: Default::default(),
    }]]
}

const COPY: i64 = 34_762; // the owner's 65 179 ticks
const COPY_SRC: i64 = 432_000;
const COPY_AT: i64 = 48_000;

/// t1113: the owner's `broken_silents_steps` on a sine — five abutting COPIES of one piece (the same
/// in-point), Slowed+Reverb on each and a pitch +5 after it, in the chain and by a morpher, over the
/// file and over a reel holding it. Before: 0.034 of the level at every join.
#[test]
fn t1113_copies_with_slowed_and_a_morpher_pitch_do_not_tear() {
    let evs: Vec<(i64, i64, i64)> = (0..5).map(|i| (COPY_AT + i * COPY, COPY, COPY_SRC)).collect();
    let total = 2 * COPY_AT + 5 * COPY;
    for engine in ENGINES {
        for (how, morph, reel) in [("chain", false, false), ("morpher", true, false), ("reel", true, true)] {
            let chain = if morph { vec![slowed(1.0)] } else { vec![slowed(1.0), pitch(5.0).remove(0)] };
            let mut p = events_project(engine, &evs, chain);
            for c in &mut p.clips {
                c.fx_auto = signed_far_curve(COPY_SRC);
            }
            if morph {
                p.morphers.push(MorpherEvent {
                    id: MorphId(1),
                    lane: 0,
                    start: Tick::from_audio_frames(COPY_AT),
                    duration: Tick::from_audio_frames(5 * COPY),
                    targets: MorphTargets::Clips { ids: (1..=5).map(ClipId).collect() },
                    patch: MorphPatch { audio_fx: pitch(5.0), ..Default::default() },
                    timed: false,
                    strength: 1.0,
                    fade_in: Tick::ZERO,
                    fade_out: Tick::ZERO,
                    enabled: true,
                });
                p.ensure_tracks(LaneKind::Morph, 1);
            }
            if reel {
                let mut s = Sequence::new(MediaId(100), "Piece");
                s.clips.push(event(50, 0, 48_000 * 60, 0, vec![]));
                p.sequences.push(s);
                for c in &mut p.clips {
                    c.media = MediaId(100);
                }
            }
            p.validate().unwrap();
            let out = render(&p, total);
            let (dip, _) = worst_join(&out, &evs);
            assert!(dip >= 0.5, "{engine:?} {how}: a join dips to {dip:.3} of the steady level");
            for piece in SLICES {
                assert!(render_sliced(&p, 0, total, piece) == out, "{engine:?} {how}: pieces of {piece}");
            }
        }
    }
}

/// t1114: a node BEFORE the pitch processes the material too; its tail ends with the event, nothing
/// outside the event is heard, and the prefetch orders what the render reads — with a warp too,
/// forward and reversed.
#[test]
fn t1114_a_node_before_the_pitch_reads_the_material() {
    let (start, len, src) = (24_000i64, 9_600i64, 96_000i64);
    let total = 48_000i64;
    let p7 = || pitch(7.0).remove(0);
    for (name, chain) in [("echo + pitch", vec![echo(), p7()]), ("slowed 0.8 + pitch", vec![slowed(0.8), p7()])] {
        for reversed in [false, true] {
            let mut p = events_project(PitchEngine::Wsola, &[(start, len, src)], chain.clone());
            p.clips[0].reversed = reversed;
            let out = render(&p, total);
            assert!(
                (0..start).chain(start + len..total).all(|i| out[i as usize * CH] == 0.0 && out[i as usize * CH + 1] == 0.0),
                "{name} reversed {reversed}: sound outside the event"
            );
            assert!(
                rms(&out, start, 256) > 0.1 && rms(&out, start + len - 256, 256) > 0.1,
                "{name} reversed {reversed}: an edge dips ({:.3} / {:.3})",
                rms(&out, start, 256),
                rms(&out, start + len - 256, 256)
            );
            let r = AudioRenderer::new(&p);
            let mut store = FxStateStore::new();
            r.sync_fx(&mut store);
            for &(pos, frames) in &[(start - 4_096, 8_192i64), (start, 4_096), (start + len - 1_000, 2_048)] {
                store.reset_all();
                let mut f = Recording { inner: source(), log: Vec::new() };
                let mut buf = vec![0f32; frames as usize * CH];
                r.render_frames_with(&mut f, &mut store, pos, &mut buf);
                let wanted = r.wanted_audio_bursts(pos..pos + frames);
                for &(media, idx) in &f.log {
                    assert!(
                        wanted.iter().any(|(m, rg)| *m == media && rg.contains(&idx)),
                        "{name} reversed {reversed}: burst {idx} read at {pos} is not in the prefetch request"
                    );
                }
            }
        }
    }
}
