//! Tests 337-340 (R.48, items 339-340): pitch change without a change of duration.
//!
//! What is proven: a neutral pitch does not touch the bytes at all; the result
//! does not depend on buffer slicing (grain and hop grids are absolute);
//! the pitch really moves by the given number of semitones with BOTH
//! engines; the engine is a project setting, it changes the sound, survives
//! saving, and is rolled back by undo.

use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const DUR: i64 = 48_000 * 2;
const TONE: f64 = 440.0;

fn project(fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>, engine: PitchEngine) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: engine,

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:tone"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick::from_audio_frames(DUR),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: vec![],
        fx_auto: auto,
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn source() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: TONE })),
    );
    f
}

fn render(fx: Vec<FxInstance>, engine: PitchEngine) -> Vec<f32> {
    render_auto(fx, vec![], engine)
}

fn render_auto(fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>, engine: PitchEngine) -> Vec<f32> {
    let p = project(fx, auto, engine);
    let r = AudioRenderer::new(&p);
    let mut f = source();
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; DUR as usize * CH];
    let stats = r.render_frames_with(&mut f, &mut store, 0, &mut out);
    assert_eq!(stats.underruns, 0, "source fully available");
    out
}

/// The same render in several consecutive pieces (slicing must not affect it).
fn render_chunked(fx: Vec<FxInstance>, engine: PitchEngine, chunks: &[i64]) -> Vec<f32> {
    let p = project(fx, vec![], engine);
    let r = AudioRenderer::new(&p);
    let mut f = source();
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = Vec::new();
    let mut pos = 0i64;
    for &n in chunks {
        let mut buf = vec![0f32; n as usize * CH];
        r.render_frames_with(&mut f, &mut store, pos, &mut buf);
        out.extend_from_slice(&buf);
        pos += n;
    }
    out
}

/// Dominant frequency of the left channel in the window [from, from+N) — the peak
/// of the Hann spectrum.
fn dominant_hz(buf: &[f32], from: usize) -> f64 {
    use realfft::RealFftPlanner;
    const N: usize = 16384;
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(N);
    let mut time: Vec<f32> = (0..N)
        .map(|i| {
            let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / N as f32).cos();
            buf[(from + i) * CH] * w
        })
        .collect();
    let mut spec = fft.make_output_vec();
    fft.process(&mut time, &mut spec).unwrap();
    let (mut best, mut best_mag) = (0usize, 0f32);
    for (k, c) in spec.iter().enumerate().skip(1) {
        let m = c.re * c.re + c.im * c.im;
        if m > best_mag {
            best_mag = m;
            best = k;
        }
    }
    best as f64 * 48_000.0 / N as f64
}

/// Energy of the ±20 Hz band around the frequency (to compare dominance).
fn band_energy(buf: &[f32], from: usize, hz: f64) -> f64 {
    use realfft::RealFftPlanner;
    const N: usize = 16384;
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(N);
    let mut time: Vec<f32> = (0..N)
        .map(|i| {
            let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / N as f32).cos();
            buf[(from + i) * CH] * w
        })
        .collect();
    let mut spec = fft.make_output_vec();
    fft.process(&mut time, &mut spec).unwrap();
    let bin = |f: f64| (f * N as f64 / 48_000.0).round() as usize;
    let (a, b) = (bin(hz - 20.0).max(1), bin(hz + 20.0).min(spec.len() - 1));
    spec[a..=b].iter().map(|c| (c.re * c.re + c.im * c.im) as f64).sum()
}

/// Test 337: NEUTRAL — BYTE FOR BYTE.
///
/// Zero semitones and cents without curves: the node does not touch the buffer at all, and
/// the chain has no lookahead. An effect added “just to try” must not
/// silently change the export. (Red-proof: remove the early exit on
/// `la == 0` in `PitchNode::process`.)
#[test]
fn t337_neutral_pitch_is_byte_passthrough() {
    kt_plugins_builtin::install();
    let dry = render(vec![], PitchEngine::Wsola);
    for engine in [PitchEngine::Wsola, PitchEngine::Vocoder] {
        let neutral = render(vec![FxInstance::builtin("pitch_shift")], engine);
        assert_eq!(dry, neutral, "neutral pitch must be an identity ({engine:?})");
    }
    // and the neutral setting needs no lookahead
    let auto = kt_fx::AutoProfiles::from_curves_rev(&[vec![]], 0, DUR, false);
    assert_eq!(kt_fx::chain_lookahead(&[FxInstance::builtin("pitch_shift")], &auto), 0);
    // but a live curve is NOT neutral, even with a zero base
    let curve = vec![vec![ParamCurve {
        param: "semitones".into(),

        keys: AutoKey::linear(vec![
            AutoKey::at(Tick(0), 0.0),
            AutoKey::at(Tick::from_audio_frames(DUR), 7.0),
        ]),
        source: Default::default(),
    }]];
    let auto_c = kt_fx::AutoProfiles::from_curves_rev(&curve, 0, DUR, false);
    assert!(
        kt_fx::chain_lookahead(&[FxInstance::builtin("pitch_shift")], &auto_c) > 0,
        "a curve over a zero base is not neutral"
    );
}

/// Test 338: SLICING DOES NOT AFFECT.
///
/// The grain grids (WSOLA) and hops (vocoder) are absolute in clip position,
/// so a render in one piece must match a render in pieces of arbitrary length —
/// otherwise export and playback would sound different.
#[test]
fn t338_pitch_is_independent_of_chunking() {
    kt_plugins_builtin::install();
    for engine in [PitchEngine::Wsola, PitchEngine::Vocoder] {
        let fx = vec![kt_model::FxInstance::plugin_with("pitch_shift", &[5.0, 0.0]).unwrap()];
        let one = render(fx.clone(), engine);
        let many = render_chunked(fx, engine, &[1000, 17, 48_000, DUR - 49_017]);
        assert_eq!(one.len(), many.len());
        assert_eq!(one, many, "{engine:?}: buffer slicing changed the result");
    }
}

/// Test 339: PITCH MOVES BY THE GIVEN INTERVAL.
///
/// A 440 Hz sine, raised by 7 semitones: the peak must land at
/// 440·2^(7/12) ≈ 659.3 Hz with both engines, and it must dominate
/// over the original frequency (not merely be present).
#[test]
fn t339_pitch_shifts_the_frequency() {
    kt_plugins_builtin::install();
    let from = 48_000usize / 2;
    for engine in [PitchEngine::Wsola, PitchEngine::Vocoder] {
        for st in [7.0f32, -5.0] {
            let out = render(vec![kt_model::FxInstance::plugin_with("pitch_shift", &[(st) as f64, 0.0]).unwrap()], engine);
            let want = TONE * 2f64.powf(st as f64 / 12.0);
            let got = dominant_hz(&out, from);
            println!("t339 {engine:?} {st:+} st: peak {got:.1} Hz (expected {want:.1})");
            assert!(
                (got - want).abs() / want < 0.02,
                "{engine:?} {st:+} st: peak {got:.1} Hz instead of {want:.1}"
            );
            let moved = band_energy(&out, from, want);
            let stayed = band_energy(&out, from, TONE);
            assert!(
                moved > stayed * 100.0,
                "{engine:?} {st:+} st: original frequency did not leave ({moved:.3e} against {stayed:.3e})"
            );
        }
        // determinism between runs
        let a = render(vec![kt_model::FxInstance::plugin_with("pitch_shift", &[3.0, 20.0]).unwrap()], engine);
        let b = render(vec![kt_model::FxInstance::plugin_with("pitch_shift", &[3.0, 20.0]).unwrap()], engine);
        assert_eq!(a, b, "{engine:?}: render must be deterministic");
    }
}

/// Test 340: THE ENGINE IS A PROJECT SETTING.
///
/// The two engines sound different (otherwise the choice is meaningless), the choice
/// survives saving and reading the file, and a change is rolled back by undo —
/// this is an ordinary model edit, not a hidden application setting.
#[test]
fn t340_pitch_engine_is_a_project_setting() {
    kt_plugins_builtin::install();
    let fx = vec![kt_model::FxInstance::plugin_with("pitch_shift", &[7.0, 0.0]).unwrap()];
    let w = render(fx.clone(), PitchEngine::Wsola);
    let v = render(fx.clone(), PitchEngine::Vocoder);
    assert_ne!(w, v, "engines must sound different");

    // the change is an ordinary model command: undo restores it as it was
    // (the file round-trip is guarded by kt-otio t144/t147: the field travels in the
    // settings section and is not written to the file by default)
    let mut m = TimelineModel::new(project(vec![], vec![], PitchEngine::Wsola));
    let mut settings = m.project().settings.clone();
    settings.pitch_engine = PitchEngine::Vocoder;
    m.apply(Command::SetSettings { settings }).expect("set settings");
    assert_eq!(m.project().settings.pitch_engine, PitchEngine::Vocoder);
    m.undo().expect("undo");
    assert_eq!(
        m.project().settings.pitch_engine,
        PitchEngine::Wsola,
        "undo must restore the previous engine"
    );
}

/// Test 910 (pitch-loop-silence): a real seek and a moved clip with pitch.
///
/// A seek is still a cold start (the stream restarts at the clip position, so seek == cold
/// render); a clip moved on the timeline during playback is a SEAM, and the pitch node continues
/// its stream instead of restarting from an empty ring (it used to go silent for 96 ms there too);
/// a neutral pitch stays byte-for-byte transparent in a loop and asks for no look-ahead.
#[test]
fn t910_seek_and_moved_clip_with_pitch() {
    kt_plugins_builtin::install();
    let seven = || vec![kt_model::FxInstance::plugin_with("pitch_shift", &[7.0, 0.0]).unwrap()];
    let burst = 1024i64;
    let silent = |buf: &[f32]| -> Option<(usize, usize)> {
        let mut run = 0usize;
        for i in 0..buf.len() / CH {
            if buf[i * CH].abs() < 1e-4 && buf[i * CH + 1].abs() < 1e-4 {
                run += 1;
                if run >= 64 {
                    return Some((i + 1 - run, run));
                }
            } else {
                run = 0;
            }
        }
        None
    };
    for engine in [PitchEngine::Wsola, PitchEngine::Vocoder] {
        // --- a seek == the cold render of that position --------------------------------
        let p = project(seven(), vec![], engine);
        let r = AudioRenderer::new(&p);
        let at = 30_000i64;
        let mut cold = vec![0f32; 8 * burst as usize * CH];
        {
            let mut f = source();
            let mut store = FxStateStore::new();
            r.sync_fx(&mut store);
            r.render_frames_with(&mut f, &mut store, at, &mut cold);
        }
        let mut sought = vec![0f32; 8 * burst as usize * CH];
        {
            let mut f = source();
            let mut store = FxStateStore::new();
            r.sync_fx(&mut store);
            let mut buf = vec![0f32; burst as usize * CH];
            for k in 0..10 {
                r.render_frames_with(&mut f, &mut store, k * burst, &mut buf);
            }
            store.reset_all(); // the player's seek
            r.render_frames_with(&mut f, &mut store, at, &mut sought);
        }
        assert!(cold == sought, "{engine:?}: a seek is not the cold render of its position");

        // --- the clip moved by 3 000 frames during playback: no silent run ----------------
        let mut f = source();
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = Vec::new();
        let mut buf = vec![0f32; burst as usize * CH];
        let mut pos = 0i64;
        for _ in 0..20 {
            r.render_frames_with(&mut f, &mut store, pos, &mut buf);
            out.extend_from_slice(&buf);
            pos += burst;
        }
        let mut moved = p.clone();
        moved.clips[0].start = Tick::from_audio_frames(3_000);
        moved.validate().unwrap();
        let r2 = AudioRenderer::new(&moved);
        r2.sync_fx(&mut store);
        let after_from = out.len() / CH;
        for _ in 0..20 {
            r2.render_frames_with(&mut f, &mut store, pos, &mut buf);
            out.extend_from_slice(&buf);
            pos += burst;
        }
        if let Some((s, n)) = silent(&out[after_from * CH..]) {
            panic!("{engine:?}: the moved clip went silent for {n} frames at +{s} after the move");
        }

        // --- a neutral pitch in a loop is the dry loop, byte for byte --------------------
        let neutral = project(vec![kt_model::FxInstance::plugin_with("pitch_shift", &[0.0, 0.0]).unwrap()], vec![], engine);
        let dry = project(vec![], vec![], engine);
        let loop_of = |p: &Project| -> Vec<f32> {
            let r = AudioRenderer::new(p);
            let mut f = source();
            let mut store = FxStateStore::new();
            r.sync_fx(&mut store);
            let region = 20_137i64..31_000;
            let mut out = vec![0f32; 4 * 11_000 * CH];
            let total = out.len() / CH;
            let mut done = 0usize;
            while done < total {
                let n = (burst as usize).min(total - done);
                r.render_looped_with(&mut f, &mut store, region.clone(), region.start + done as i64, &mut out[done * CH..(done + n) * CH]);
                done += n;
            }
            out
        };
        assert!(loop_of(&neutral) == loop_of(&dry), "{engine:?}: a neutral pitch changed the looped sound");
        assert_eq!(
            kt_fx::chain_lookahead(&neutral.clips[0].fx, &kt_fx::AutoProfiles::default()),
            0,
            "a neutral pitch asks for a look-ahead"
        );
    }
}
