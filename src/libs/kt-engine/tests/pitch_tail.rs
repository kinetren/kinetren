//! Tests t1330-t1335, t1339 (pitch-tail-choice, kkt-26.11.01 item 15), t1440-t1441 (loop-pitch-tail-dip): the echo of the effects
//! standing before the pitch — cut at the event's end (the default) or ringing on after it.
//!
//! The owner, asked why his reverb's echo now ends with the event: "what does reverb have to do with
//! pitch?!? well then make an option so it can be done this way and that way." What is proven:
//! "rings on" lets the echo of the effects before the pitch ring past the event's end — with nothing
//! of the material after the out-point in it and without a hole — while the joins keep no dips, and
//! "cut" is the former sound.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};
use std::sync::Arc;

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
        masks: Default::default(),
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

fn fx(id: &str, vals: &[f64]) -> FxInstance {
    kt_plugins_builtin::install();
    FxInstance::plugin_with(id, vals).unwrap()
}

fn pitch(st: f64) -> FxInstance {
    fx("pitch_shift", &[st, 0.0])
}

/// The pitch with its echo choice at "rings on".
fn ring(mut f: FxInstance) -> FxInstance {
    if let FxInstance::Plugin(p) = &mut f {
        assert!(p.set("echo_before", 1.0), "the pitch has the echo row");
    }
    f
}

/// A plain echo of 100 ms without feedback.
fn echo() -> FxInstance {
    fx("delay", &[100.0, 0.0, 0.5, 0.0])
}

fn slowed(rate: f64) -> FxInstance {
    fx("slowed_reverb", &[rate, 0.1, 85.0, 0.2, 0.84, 1.0, 0.0, 0.0, 1.0])
}

fn sine() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: TONE })));
    f
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

fn render_with(p: &Project, f: &mut dyn AudioFetcher, from: i64, frames: i64, piece: i64) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = Vec::with_capacity(frames as usize * CH);
    let mut pos = from;
    while pos < from + frames {
        let n = piece.min(from + frames - pos);
        let mut buf = vec![0f32; n as usize * CH];
        let stats = r.render_frames_with(f, &mut store, pos, &mut buf);
        assert_eq!(stats.underruns, 0, "source fully available");
        out.extend_from_slice(&buf);
        pos += n;
    }
    out
}

fn render(p: &Project, frames: i64) -> Vec<f32> {
    render_with(p, &mut sine(), 0, frames, frames)
}

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

/// The deepest 64-frame window within 1 024 frames of each join, as a share of the steady level of
/// the event the window lies in (the RMS of its middle).
fn worst_join(buf: &[f32], evs: &[(i64, i64, i64)]) -> f64 {
    const W: i64 = 64;
    let steady: Vec<f64> = evs.iter().map(|&(s, l, _)| rms(buf, s + 1024, l - 2048)).collect();
    let mut dip = f64::INFINITY;
    for k in 1..evs.len() {
        let at = evs[k].0;
        let mut f = at - 1024;
        while f + W <= at + 1024 {
            let mid = f + W / 2;
            let e = evs.iter().position(|&(s, l, _)| mid >= s && mid < s + l).unwrap_or(k);
            dip = dip.min(rms(buf, f, W) / steady[e]);
            f += W / 2;
        }
    }
    dip
}

/// t1330: every built-in audio kind forks, and the fork continues byte for byte as the original —
/// at its defaults and with every number row at 70 %; different input then leaves the other copy
/// alone.
#[test]
fn t1330_every_audio_kind_forks_and_continues_as_itself() {
    kt_plugins_builtin::install();
    let reg = kt_plugin_api::installed().expect("installed");
    let input = |from: usize, n: usize| -> Vec<f32> {
        (from..from + n)
            .flat_map(|i| {
                let v = (std::f64::consts::TAU * TONE * i as f64 / 48_000.0).sin() as f32 * 0.5;
                [v, -v * 0.7]
            })
            .collect()
    };
    let mut kinds = 0;
    for p in reg.plugins() {
        let m = p.manifest();
        let (Some(kernel), false) = (p.audio(), reg.is_test_only(&m.id)) else { continue };
        for engine in [0.0, 1.0] {
            for at70 in [false, true] {
                let vals: Vec<f64> = m
                    .passport
                    .iter()
                    .enumerate()
                    .map(|(i, s)| m.row(i).store.cast(if at70 { s.min + 0.7 * (s.max - s.min) } else { s.default }))
                    .collect();
                let mut a = kernel.build(&kt_plugin_api::HostOpts { opts: vec![("pitch_engine", engine)] });
                a.apply(&vals, true);
                a.bind(&kt_plugin_api::NoCurves, 48_000);
                let mut pos = 0i64;
                for _ in 0..47 {
                    let mut buf = input(pos as usize, 1024);
                    a.process(&mut buf, pos, None);
                    pos += 1024;
                }
                let mut b = a.fork().unwrap_or_else(|| panic!("{} does not fork", m.id));
                let (mut x, mut y) = (input(pos as usize, 48_000), input(pos as usize, 48_000));
                for (cx, cy) in x.chunks_mut(1024 * CH).zip(y.chunks_mut(1024 * CH)) {
                    a.process(cx, pos, None);
                    b.process(cy, pos, None);
                    pos += (cx.len() / CH) as i64;
                }
                assert!(x == y, "{} (engine {engine}, at 70 % {at70}): the fork went another way", m.id);
                // different input: each copy keeps to its own stream
                let mut c = b.fork().unwrap();
                let mut z = vec![0f32; 4096 * CH];
                let mut w = input(pos as usize, 4096);
                b.process(&mut z, pos, None);
                let mut w2 = w.clone();
                c.process(&mut w, pos, None);
                a.process(&mut w2, pos, None);
                assert!(w == w2, "{} (engine {engine}, at 70 % {at70}): feeding one copy moved the other", m.id);
            }
        }
        kinds += 1;
    }
    assert!(kinds >= 18, "every built-in audio kind is covered ({kinds})");
}

const START: i64 = 24_000;
const LEN: i64 = 9_600;
const SRC: i64 = 96_000;

/// t1331: "rings on" — the echo of the effects before the pitch rings past the out-point, carries
/// nothing of the material after it, and starts without a hole; the body is the "cut" body.
#[test]
fn t1331_the_echo_rings_on_without_the_next_material() {
    let total = START + LEN + 24_000;
    let end = START + LEN;
    for engine in ENGINES {
        for st in [7.0, -7.0] {
            let cut = events_project(engine, &[(START, LEN, SRC)], vec![echo(), pitch(st)]);
            let rings = events_project(engine, &[(START, LEN, SRC)], vec![echo(), ring(pitch(st))]);
            // the source is silent before the in-point (so "cut" and "rings on" begin alike) and loud
            // or silent past the out-point
            let loud = |p: &Project| render_with(p, &mut Windowed { inner: sine(), lo: SRC, hi: i64::MAX }, 0, total, total);
            let quiet = |p: &Project| render_with(p, &mut Windowed { inner: sine(), lo: SRC, hi: SRC + LEN }, 0, total, total);
            let c = loud(&cut);
            let r = loud(&rings);
            let q = quiet(&rings);
            assert!((end..total).all(|i| c[i as usize * CH] == 0.0), "{engine:?} {st:+}: \"cut\" rang past the out-point");
            assert!(c[..end as usize * CH] == r[..end as usize * CH], "{engine:?} {st:+}: the body moved");
            assert!(
                r[end as usize * CH..] == q[end as usize * CH..],
                "{engine:?} {st:+}: the echo carried the material after the out-point"
            );
            let echo_level = rms(&r, end, 4_800);
            let body = rms(&r, START + 2_048, LEN - 4_096);
            assert!(echo_level > 0.1 * body, "{engine:?} {st:+}: no echo past the out-point ({echo_level:.4})");
            let mut hole = f64::INFINITY;
            let mut f = end;
            while f + 64 <= end + 1_024 {
                hole = hole.min(rms(&r, f, 64));
                f += 32;
            }
            eprintln!(
                "t1331 {engine:?} {st:+}: body {body:.4}, the echo's first 100 ms {echo_level:.4}, \
                 the deepest 64-frame window of the first 1 024 frames past the out-point {hole:.4}"
            );
            assert!(hole >= 0.1 * echo_level, "{engine:?} {st:+}: a hole past the out-point ({hole:.4})");
            for piece in SLICES {
                let s = render_with(&rings, &mut Windowed { inner: sine(), lo: SRC, hi: i64::MAX }, 0, total, piece);
                assert!(s == r, "{engine:?} {st:+}: pieces of {piece}");
            }
        }
    }
}

const N: usize = 15;
const PIECE: i64 = 4_800;
const SRC0: i64 = 48_000;

/// t1332: a split under "rings on" — the pieces that the next piece continues do not ring (its
/// context carries their echo, as the uncut event would), so up to the last piece's end the split
/// sounds as under "cut", byte for byte; the last piece rings on.
#[test]
fn t1332_a_split_rings_only_at_its_end() {
    let evs: Vec<(i64, i64, i64)> = (0..N as i64).map(|i| (i * PIECE, PIECE, SRC0 + i * PIECE)).collect();
    let end = N as i64 * PIECE;
    let total = end + 24_000;
    for engine in ENGINES {
        for st in [7.0, -7.0] {
            let cut = render(&events_project(engine, &evs, vec![echo(), pitch(st)]), total);
            let rings = render(&events_project(engine, &evs, vec![echo(), ring(pitch(st))]), total);
            assert!(
                cut[..end as usize * CH] == rings[..end as usize * CH],
                "{engine:?} {st:+}: a piece rang into the next piece of its split"
            );
            assert!((end..total).all(|i| cut[i as usize * CH] == 0.0), "{engine:?} {st:+}: \"cut\" rang");
            assert!(rms(&rings, end, 4_800) > 0.05, "{engine:?} {st:+}: the last piece did not ring on");
        }
    }
}


/// t1333: the joins do not dip with "rings on" — every one of fifteen events of other material with
/// an echo before the pitch keeps the "cut" body and has no dip at its own edges, and the owner's
/// five copies with Slowed+Reverb and a morpher's pitch do not dip at the joins.
#[test]
fn t1333_rings_on_joins_do_not_dip() {
    let total = N as i64 * PIECE + 24_000;
    let evs: Vec<(i64, i64, i64)> = (0..N as i64).map(|i| (i * PIECE, PIECE, SRC0 + i * (PIECE + 1777))).collect();
    for engine in ENGINES {
        for st in [7.0, -7.0] {
            let p = events_project(engine, &evs, vec![echo(), ring(pitch(st))]);
            let one = render(&p, total);
            for piece in SLICES {
                assert!(render_with(&p, &mut sine(), 0, total, piece) == one, "{engine:?} {st:+}: pieces of {piece}");
            }
            // each event alone: the body is the "cut" body and its edges do not dip; what rings past
            // its end lies over the next event as any ringing effect does
            for &(s0, l, src) in &evs {
                let alone = render(&events_project(engine, &[(s0, l, src)], vec![echo(), ring(pitch(st))]), total);
                let cut = render(&events_project(engine, &[(s0, l, src)], vec![echo(), pitch(st)]), total);
                assert!(alone[..(s0 + l) as usize * CH] == cut[..(s0 + l) as usize * CH], "{engine:?} {st:+}: the body moved");
                let steady = rms(&alone, s0 + 1024, l - 2048);
                let mut f = s0;
                while f + 64 <= s0 + l {
                    if f < s0 + 1024 || f + 64 > s0 + l - 1024 {
                        let v = rms(&alone, f, 64) / steady;
                        assert!(v >= 0.5, "{engine:?} {st:+}: the event at {s0} dips to {v:.3} at frame {}", f - s0);
                    }
                    f += 32;
                }
            }
        }
    }
    // the owner's shape: copies of one piece, Slowed+Reverb on each, the morpher's pitch after it
    const COPY: i64 = 34_762;
    const COPY_SRC: i64 = 432_000;
    const COPY_AT: i64 = 48_000;
    let evs: Vec<(i64, i64, i64)> = (0..5).map(|i| (COPY_AT + i * COPY, COPY, COPY_SRC)).collect();
    let total = 2 * COPY_AT + 5 * COPY;
    let copies = |engine: PitchEngine, evs: &[(i64, i64, i64)], rings: bool| -> Project {
        let mut p = events_project(engine, evs, vec![slowed(1.0)]);
        let pitch5 = if rings { ring(pitch(5.0)) } else { pitch(5.0) };
        p.morphers.push(MorpherEvent {
            id: MorphId(1),
            lane: 0,
            start: Tick::from_audio_frames(COPY_AT),
            duration: Tick::from_audio_frames(5 * COPY),
            targets: MorphTargets::Clips { ids: (1..=evs.len() as u64).map(ClipId).collect() },
            patch: MorphPatch { audio_fx: vec![pitch5], ..Default::default() },
            timed: false,
            strength: 1.0,
            fade_in: Tick::ZERO,
            fade_out: Tick::ZERO,
            enabled: true,
        });
        p.ensure_tracks(LaneKind::Morph, 1);
        p.validate().unwrap();
        p
    };
    for engine in ENGINES {
        let p = copies(engine, &evs, true);
        let out = render(&p, total);
        let cut = render(&copies(engine, &evs, false), total);
        // nothing changes before the first out-point (the tick grid puts the second copy's first
        // frame one frame early); the joins of "cut" do not dip (t1113)
        let first_end = (COPY_AT + COPY - 1) as usize * CH;
        assert!(out[..first_end] == cut[..first_end], "{engine:?} copies: \"rings on\" moved the first copy");
        assert!(worst_join(&cut, &evs) >= 0.5, "{engine:?} copies: \"cut\" dips");
        // each copy alone: its body is the "cut" body and its edges do not dip; past its end the
        // reverb rings on — over the next copy it lies as any ringing effect does
        for (i, &(s0, l, src)) in evs.iter().enumerate() {
            let alone = render(&copies(engine, &[(s0, l, src)], true), total);
            let alone_cut = render(&copies(engine, &[(s0, l, src)], false), total);
            let e = (s0 + l - 1) as usize * CH;
            assert!(alone[..e] == alone_cut[..e], "{engine:?} copy {i}: the body moved");
            assert!(rms(&alone, s0 + l, 4_800) > 0.005, "{engine:?} copy {i}: the reverb did not ring on");
            let steady = rms(&alone, s0 + 1024, l - 2048);
            for f in (s0..s0 + 1024 - 64).chain(s0 + l - 1024..s0 + l - 64).step_by(32) {
                let v = rms(&alone, f, 64) / steady;
                assert!(v >= 0.5, "{engine:?} copy {i}: its edge dips to {v:.3}");
            }
        }
        eprintln!(
            "t1333 {engine:?} copies: the deepest 64-frame window at a join {:.3} with \"rings on\" (the \
             previous copy's reverb and its last pitch window over the next copy, a pure tone), {:.3} with \"cut\"",
            worst_join(&out, &evs),
            worst_join(&cut, &evs)
        );
        for piece in SLICES {
            assert!(render_with(&p, &mut sine(), 0, total, piece) == out, "{engine:?} copies: pieces of {piece}");
        }
    }
}

/// t1334: "cut" is the former sound; "rings on" with nothing ringing before the pitch changes
/// nothing; with an echo before it, it does.
#[test]
fn t1334_rings_on_changes_only_a_ringing_prefix() {
    let evs = [(START, LEN, SRC), (START + LEN, LEN, SRC + 30_000), (START + 2 * LEN, LEN, SRC + 70_000)];
    let total = START + 3 * LEN + 24_000;
    let limiter = || fx("limiter", &[-6.0, -6.0, 5.0, 80.0]);
    for engine in ENGINES {
        let mut explicit_cut = pitch(7.0);
        if let FxInstance::Plugin(p) = &mut explicit_cut {
            assert!(p.set("echo_before", 0.0));
        }
        assert!(explicit_cut == pitch(7.0), "the default is \"cut\"");
        for (name, cut, rings) in [
            ("pitch alone", vec![pitch(7.0)], vec![ring(pitch(7.0))]),
            ("limiter before", vec![limiter(), pitch(7.0)], vec![limiter(), ring(pitch(7.0))]),
            ("echo after", vec![pitch(7.0), echo()], vec![ring(pitch(7.0)), echo()]),
            ("neutral pitch, echo before", vec![echo(), pitch(0.0)], vec![echo(), ring(pitch(0.0))]),
        ] {
            let a = render(&events_project(engine, &evs, cut), total);
            let b = render(&events_project(engine, &evs, rings), total);
            assert!(a == b, "{engine:?} {name}: \"rings on\" without a ringing prefix moved the sound");
        }
        let a = render(&events_project(engine, &evs, vec![echo(), pitch(7.0)]), total);
        let b = render(&events_project(engine, &evs, vec![echo(), ring(pitch(7.0))]), total);
        assert!(a != b, "{engine:?}: an echo before the pitch rings on");
    }
}

/// t1335: a split under "rings on" hands nothing on (its pieces have a tail), does not dip, is the
/// same in pieces, the prefetch orders what is read, and a loop over it is the same in pieces.
#[test]
fn t1335_a_split_under_rings_on() {
    let total = N as i64 * PIECE;
    let evs: Vec<(i64, i64, i64)> = (0..N as i64).map(|i| (i * PIECE, PIECE, SRC0 + i * PIECE)).collect();
    for engine in ENGINES {
        let p = events_project(engine, &evs, vec![echo(), ring(pitch(7.0))]);
        let r = AudioRenderer::new(&p);
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut rec = Recording { inner: sine(), log: Vec::new() };
        let mut out = vec![0f32; total as usize * CH];
        r.render_frames_with(&mut rec, &mut store, 0, &mut out);
        for j in 1..N {
            // a handed-on predecessor would hold the reset processor it got in exchange
            assert!(store.get_mut(ClipId(j as u64)).unwrap().next_local().is_some(), "{engine:?}: join {j} handed on");
        }
        let wanted = r.wanted_audio_bursts(0..total);
        for &(media, idx) in &rec.log {
            assert!(wanted.iter().any(|(m, rg)| *m == media && rg.contains(&idx)), "{engine:?}: burst {idx} not ordered");
        }
        let dip = worst_join(&out, &evs);
        assert!(dip >= 0.5, "{engine:?}: a join dips to {dip:.3}");
        for piece in SLICES {
            assert!(render_with(&p, &mut sine(), 0, total, piece) == out, "{engine:?}: pieces of {piece}");
        }
        // a loop over pieces 3–9: a pitch stream that rings goes on through the wrap (a seam), so a
        // lap is not a copy of the one before; it is the same however the output is sliced
        let region = 3 * PIECE..10 * PIECE;
        let span = region.end - region.start;
        let looped = |step: i64| -> Vec<f32> {
            let mut store = FxStateStore::new();
            r.sync_fx(&mut store);
            let mut f = sine();
            let mut out = vec![0f32; (4 * span) as usize * CH];
            let (mut done, mut pos) = (0i64, region.start);
            while done < 4 * span {
                let n = step.min(4 * span - done);
                r.render_looped_with(&mut f, &mut store, region.clone(), pos, &mut out[done as usize * CH..(done + n) as usize * CH]);
                done += n;
                pos += n;
            }
            out
        };
        let out = looped(1_024);
        assert!(looped(333) == out, "{engine:?}: the loop in pieces of 333");
        assert!(rms(&out, 3 * span, span) > 0.1, "{engine:?}: the fourth lap sounds");
    }
}

/// t1339: the ringing echo survives a seam (a loop over the event and 200 ms past it) and a seek: on every lap the echo rings and carries nothing of the material after the out-point, the
/// loop is the same however it is sliced; a seek into the echo equals a cold render there.
#[test]
fn t1339_the_ringing_echo_through_a_seam_and_a_seek() {
    let end = START + LEN;
    // the region holds the whole event and 200 ms past it: the wrap re-enters the event through a
    // seam, and its echo rings on every lap
    let region = START - 4_800..end + 9_600;
    let span = region.end - region.start;
    for engine in ENGINES {
        let p = events_project(engine, &[(START, LEN, SRC)], vec![echo(), ring(pitch(7.0))]);
        let r = AudioRenderer::new(&p);
        let looped = |f: &mut dyn AudioFetcher, step: i64| -> Vec<f32> {
            let mut store = FxStateStore::new();
            r.sync_fx(&mut store);
            let mut out = vec![0f32; (4 * span) as usize * CH];
            let (mut done, mut pos) = (0i64, region.start);
            while done < 4 * span {
                let n = step.min(4 * span - done);
                r.render_looped_with(f, &mut store, region.clone(), pos, &mut out[done as usize * CH..(done + n) as usize * CH]);
                done += n;
                pos += n;
            }
            out
        };
        let loud = looped(&mut Windowed { inner: sine(), lo: 0, hi: i64::MAX }, 1_024);
        let quiet = looped(&mut Windowed { inner: sine(), lo: 0, hi: SRC + LEN }, 1_024);
        assert!(looped(&mut Windowed { inner: sine(), lo: 0, hi: i64::MAX }, 333) == loud, "{engine:?}: pieces of 333");
        let lap = |o: &[f32], k: i64| o[(k * span) as usize * CH..((k + 1) * span) as usize * CH].to_vec();
        let past = (end - region.start) as usize * CH;
        for k in 0..4 {
            let (a, b) = (lap(&loud, k), lap(&quiet, k));
            assert!(a[past..] == b[past..], "{engine:?}: lap {k} carried the material after the out-point");
            assert!(rms(&a, end - region.start, 2_400) > 0.01, "{engine:?}: lap {k} has no echo");
        }
        // a seek into the echo equals a cold render there
        let at = end + 1_500;
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut f = sine();
        let mut pre = vec![0f32; (end + 3_000) as usize * CH];
        r.render_frames_with(&mut f, &mut store, 0, &mut pre);
        store.reset_all();
        let mut sought = vec![0f32; 4_800 * CH];
        r.render_frames_with(&mut f, &mut store, at, &mut sought);
        assert!(sought == render_with(&p, &mut sine(), at, 4_800, 4_800), "{engine:?}: the seek into the echo");
    }
}

/// `laps` laps of `region` played from its start in pieces of `step`, as the transport does.
fn looped(p: &Project, region: std::ops::Range<i64>, laps: i64, step: i64) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut f = sine();
    let span = region.end - region.start;
    let mut out = vec![0f32; (laps * span) as usize * CH];
    let (mut done, mut pos) = (0i64, region.start);
    while done < laps * span {
        let n = step.min(laps * span - done);
        r.render_looped_with(
            &mut f,
            &mut store,
            region.clone(),
            pos,
            &mut out[done as usize * CH..(done + n) as usize * CH],
        );
        done += n;
        pos += n;
    }
    out
}

/// The deepest 64-frame window of the first 1 024 frames of `a` as a share of the same window of `b`.
fn worst_start(a: &[f32], b: &[f32]) -> f64 {
    (0..31).map(|w| rms(a, w * 32, 64) / rms(b, w * 32, 64).max(1e-9)).fold(f64::INFINITY, f64::min)
}

const LAPS: i64 = 4;

/// t1440: a loop that re-enters an event with an echo before its pitch — at the event's start or
/// inside it, the region running 200 ms past the out-point, so the echo has rung out by the wrap —
/// starts every lap as playing from that point does, in both echo choices. Before, a lap began with
/// the pitch's look-ahead ring of where the previous lap stopped: ≈ 75 ms of silence with "rings on"
/// (the deepest window 0.000 of the straight play), about half the level with "cut".
#[test]
fn t1440_a_loop_lap_starts_as_playing_from_there() {
    let end = START + LEN;
    for engine in ENGINES {
        for st in [7.0, -7.0] {
            for rings in [false, true] {
                let pf = if rings { ring(pitch(st)) } else { pitch(st) };
                let p = events_project(engine, &[(START, LEN, SRC)], vec![echo(), pf]);
                for rs in [START, START + 2_400] {
                    let region = rs..end + 9_600;
                    let span = region.end - region.start;
                    let out = looped(&p, region.clone(), LAPS, 1_024);
                    assert!(
                        looped(&p, region.clone(), LAPS, 333) == out,
                        "{engine:?} {st} {rings} from {rs}: pieces of 333"
                    );
                    let from_there = render_with(&p, &mut sine(), rs, span, 1_024);
                    for k in 0..LAPS {
                        let lap = &out[(k * span) as usize * CH..((k + 1) * span) as usize * CH];
                        let dip = worst_start(lap, &from_there);
                        assert!(
                            lap == &from_there[..],
                            "{engine:?} {st} rings {rings} from {rs}: lap {k} is not the play from there (deepest start window {dip:.3})"
                        );
                    }
                }
            }
        }
    }
}

/// t1441: where the previous lap still rings at the wrap (the region ends 50 ms past the out-point;
/// an echo without and with feedback), a lap is the play from its start plus what the earlier laps
/// ring on, each continuing its own stream — as copies of the event one after another would sound;
/// with "cut" nothing of them is heard past the out-point, so the lap is the play from there byte for
/// byte. At most `MAX_LAPS` earlier laps ring at once.
#[test]
fn t1441_earlier_laps_ring_on_over_a_new_lap() {
    let end = START + LEN;
    let region = START..end + 2_400;
    let span = region.end - region.start;
    let feedback = |fb: f64| fx("delay", &[100.0, fb, 0.5, 0.0]);
    for engine in ENGINES {
        for fb in [0.0, 0.6] {
            for rings in [false, true] {
                let pf = if rings { ring(pitch(7.0)) } else { pitch(7.0) };
                let p = events_project(engine, &[(START, LEN, SRC)], vec![feedback(fb), pf]);
                let out = looped(&p, region.clone(), LAPS + 3, 1_024);
                assert!(
                    looped(&p, region.clone(), LAPS + 3, 333) == out,
                    "{engine:?} fb {fb} rings {rings}: pieces of 333"
                );
                // one cold play from the region start, long enough for every lap's ring
                let long = span * (kt_engine::fxstore::MAX_LAPS as i64 + 2) + 48_000 * 2;
                let straight = render_with(&p, &mut sine(), region.start, long, 1_024);
                let at = |i: i64| straight.get(i as usize).copied().unwrap_or(0.0);
                for k in 0..LAPS + 3 {
                    let lap = &out[(k * span) as usize * CH..((k + 1) * span) as usize * CH];
                    let earlier = (k as usize).min(kt_engine::fxstore::MAX_LAPS) as i64;
                    let want: Vec<f32> = (0..span * CH as i64)
                        .map(|i| {
                            let mut ring = 0f32;
                            for j in (1..=earlier).rev() {
                                ring += at(j * span * CH as i64 + i);
                            }
                            at(i) + ring
                        })
                        .collect();
                    assert!(lap == &want[..], "{engine:?} fb {fb} rings {rings}: lap {k}");
                    if !rings {
                        assert!(
                            lap == &straight[..(span as usize * CH)],
                            "{engine:?} fb {fb} cut: lap {k} is the play from there"
                        );
                    }
                    if k > 0 {
                        let dip = worst_start(lap, &straight);
                        assert!(
                            dip >= 0.5,
                            "{engine:?} fb {fb} rings {rings}: lap {k} starts at {dip:.3} of the play from there"
                        );
                    }
                }
            }
        }
    }
}
