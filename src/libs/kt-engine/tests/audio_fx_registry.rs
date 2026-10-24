//! Test 322 (R.47, item 326): a PARAMETERIZED frame over the audio FX registry.
//! Each audio kind from `FxInstance::all()` automatically passes the same set of checks:
//! the default sounds and is deterministic, each Smooth parameter is audible, the curve
//! works, reverse with a curve is deterministic, a repeated seek is byte-for-byte, and the tail
//! is consistent between `chain_tail` (without state)
//! and `ClipProcessor::tail()` (with state).
//! Completeness lock: a new effect added to `all()` gets in here by itself;
//! one that is not added will not build, because of the exhaustive `tag()`.

use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const DUR: i64 = 48_000 * 2;

fn project(fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>, reversed: bool) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:clicks"));
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
        reversed,
        fx,
        fx_bypass: vec![],
        fx_auto: auto,
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

/// Render of the clip; the stimulus — broadband STEREO-DIFFERENT clicks: the filters
/// have something to cut, the stereo parameters have the s-component, and the
/// curves have events along the whole length of the clip.
fn render(fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>, reversed: bool, from: i64) -> Vec<f32> {
    render_on(fx, auto, reversed, from, AudioPattern::Clicks)
}

/// Same render on the chosen stimulus: dynamics (gate/compressor) are measured
/// on clicks WITH BACKGROUND — on a zero background hold/release are fundamentally
/// inaudible (there is nothing to duck between events).
fn render_on(
    fx: Vec<FxInstance>,
    auto: Vec<Vec<ParamCurve>>,
    reversed: bool,
    from: i64,
    pattern: AudioPattern,
) -> Vec<f32> {
    let p = project(fx, auto, reversed);
    let r = AudioRenderer::new(&p);
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), pattern)));
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let frames = (DUR - from) as usize;
    let mut out = vec![0f32; frames * CH];
    let stats = r.render_frames_with(&mut f, &mut store, from, &mut out);
    assert_eq!(stats.underruns, 0, "underrun on a fully available source");
    out
}

fn max_delta(a: &[f32], b: &[f32]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs() as f64).fold(0.0, f64::max)
}

fn rms(s: &[f32]) -> f64 {
    (s.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / s.len().max(1) as f64).sqrt()
}

/// Curve of one parameter of slot 0: default → trial value at the end.
fn curve_for(spec: &ParamSpec, probe: f64) -> Vec<Vec<ParamCurve>> {
    vec![vec![ParamCurve {
        param: spec.name.to_string(),

        keys: AutoKey::linear(vec![
            AutoKey::at(Tick(0), spec.default),
            AutoKey::at(Tick::from_audio_frames(DUR), probe),
        ]),
        source: Default::default(),
    }]]
}

/// Probe candidates: both ends of the range (the far one first). Some parameters
/// have a “transparent” edge (rate_hz=48000 on the crusher is a grid in every
/// frame), so audibility is required from at least one end, and the curve
/// is built on the audible one.
fn probe_values(spec: &ParamSpec) -> [f64; 2] {
    if (spec.default - spec.max).abs() > (spec.default - spec.min).abs() {
        [spec.max, spec.min]
    } else {
        [spec.min, spec.max]
    }
}

#[test]
fn t322_audio_registry_contract() {
    let audio: Vec<&FxInstance> = FxInstance::all().iter().filter(|f| f.is_audio()).collect();
    assert!(audio.len() >= 2, "registry must know the audio effects");
    let mut covered: Vec<&'static str> = Vec::new();

    let dry = render(vec![], vec![], false, 0);
    for f in &audio {
        covered.push(f.tag());
        let fx = vec![(*f).clone()];

        // (a) default: not silence, deterministic between runs
        let a = render(fx.clone(), vec![], false, 0);
        assert!(rms(&a) > 1e-4, "{}: default renders silence", f.tag());
        let b = render(fx.clone(), vec![], false, 0);
        assert_eq!(a, b, "{}: default render is not deterministic", f.tag());

        // (b) every Smooth parameter is audible from its trial value.
        // Enablers: a parameter that is inaudible at its neighbours' defaults (EQ band
        // frequency at zero gain) is probed on a base where the neighbour is
        // enabled — a table with a comment on each row.
        let stim = match f.tag() {
            "gate" | "compressor" => AudioPattern::ClicksBed,
            _ => AudioPattern::Clicks,
        };
        let base_for = |param: &str| -> FxInstance {
            match (f.tag(), param) {
                // the detector with the default 10 ms attack does not keep up with a
                // 4-frame click — the compressor probes run with a fast
                ("compressor", _) => {
                    let mut e = (*f).clone();
                    e.set_param("attack_ms", 0.1);
                    e
                }
                // default threshold -40 is below the stimulus background (-34): the gate is always
                // open and ramps/hold do not take part — the probes run at a threshold
                // between the background and the clicks
                ("gate", "hold_ms") | ("gate", "release_ms") | ("gate", "attack_ms") => {
                    let mut e = (*f).clone();
                    e.set_param("threshold_db", -25.0);
                    e
                }
                // frequency/Q of a band are audible only at a non-zero
                // gain of that band
                ("eq", "low_freq_hz") => {
                    let mut e = (*f).clone();
                    e.set_param("low_gain_db", -12.0);
                    e
                }
                ("eq", "mid_freq_hz") | ("eq", "mid_q") => {
                    let mut e = (*f).clone();
                    e.set_param("mid_gain_db", -12.0);
                    e
                }
                ("eq", "high_freq_hz") => {
                    let mut e = (*f).clone();
                    e.set_param("high_gain_db", -12.0);
                    e
                }
                _ => (*f).clone(),
            }
        };
        let mut audible_probe: Vec<(String, f64)> = Vec::new();
        for spec in f.auto_params() {
            let base = base_for(&spec.name);
            let base_out = render_on(vec![base.clone()], vec![], false, 0, stim);
            let heard = probe_values(spec).into_iter().find(|probe| {
                let mut probed = base.clone();
                assert!(probed.set_param(&spec.name, *probe));
                let p = render_on(vec![probed], vec![], false, 0, stim);
                max_delta(&base_out, &p) > 1e-4
            });
            let heard = heard.unwrap_or_else(|| {
                panic!("{}.{}: no end of the range is audible", f.tag(), spec.name)
            });
            audible_probe.push((spec.name.to_string(), heard));
        }

        // (c) the curve of the first Smooth parameter works: the output is equal to
        // neither the constant default nor the constant probe
        if let Some(spec) = f.auto_params().next() {
            let probe =
                audible_probe.iter().find(|(n, _)| *n == spec.name).map(|(_, v)| *v).unwrap();
            let curved = render_on(fx.clone(), curve_for(spec, probe), false, 0, stim);
            let mut probed = (*f).clone();
            probed.set_param(&spec.name, probe);
            let const_probe = render_on(vec![probed], vec![], false, 0, stim);
            let default_on_stim = render_on(fx.clone(), vec![], false, 0, stim);
            assert!(
                max_delta(&curved, &default_on_stim) > 1e-4,
                "{}.{}: curve equals the default render",
                f.tag(),
                spec.name
            );
            assert!(
                max_delta(&curved, &const_probe) > 1e-4,
                "{}.{}: curve equals the constant probe",
                f.tag(),
                spec.name
            );

            // (d) reverse + curve: deterministic and differs from reverse without a curve
            // (the engine t271 case: before R.39 nobody combined reversed with a non-empty
            // fx_auto)
            let rev_plain = render_on(fx.clone(), vec![], true, 0, stim);
            let rev_curved = render_on(fx.clone(), curve_for(spec, probe), true, 0, stim);
            let rev_curved2 = render_on(fx.clone(), curve_for(spec, probe), true, 0, stim);
            assert_eq!(rev_curved, rev_curved2, "{}: reversed+curve not deterministic", f.tag());
            assert!(
                max_delta(&rev_curved, &rev_plain) > 1e-4,
                "{}.{}: curve is lost on a reversed clip",
                f.tag(),
                spec.name
            );
        }

        // (e) repeated seek to the same position with a fresh store — byte for byte
        let s1 = render(fx.clone(), vec![], false, DUR / 3);
        let s2 = render(fx.clone(), vec![], false, DUR / 3);
        assert_eq!(s1, s2, "{}: seek render is not reproducible", f.tag());

        // (f) tail: a formula without state == the processor's formula (relative
        // tolerance 2% — integer truncations of line lengths)
        let auto = AutoProfiles::from_curves_rev(&[vec![]], 0, DUR, false);
        let ct = kt_fx::chain_tail(&fx, &auto).0 as f64;
        let proc = kt_fx::ClipProcessor::new(&fx, &auto, 0, DUR);
        let pt = proc.tail().0 as f64;
        let diff = (ct - pt).abs();
        assert!(
            diff <= (ct.max(pt) * 0.02).max(200.0),
            "{}: chain_tail {ct} vs processor tail {pt}",
            f.tag()
        );

        // (g) lookahead: the static formula == the processor's implementation (R.48, item 337).
        // Equality here is EXACT — these are whole frames by which the source window
        // and the seek warm-up are widened: if they diverge, the chain would request
        // material that the prefetch did not warm.
        assert_eq!(
            kt_fx::chain_lookahead(&fx, &auto),
            proc.lookahead_frames(),
            "{}: chain_lookahead disagrees with the processor",
            f.tag()
        );
        // the same with the live curve of the first parameter: for lookahead kinds the
        // neutral depends on curves (base “zero” with a curve is not neutral)
        if let Some(sp) = f.auto_params().next() {
            let curve = vec![vec![kt_model::ParamCurve {
                param: sp.name.to_string(),
                keys: vec![
                    kt_model::AutoKey::at(Tick(0), sp.min),
                    kt_model::AutoKey::at(Tick(DUR), sp.max),
                ],
                source: Default::default(),
            }]];
            let auto_c = AutoProfiles::from_curves_rev(&curve, 0, DUR, false);
            let proc_c = kt_fx::ClipProcessor::new(&fx, &auto_c, 0, DUR);
            assert_eq!(
                kt_fx::chain_lookahead(&fx, &auto_c),
                proc_c.lookahead_frames(),
                "{}: chain_lookahead disagrees with the processor (curved)",
                f.tag()
            );
        }
    }

    // LayerMix — the strength slot; in a single chain it mixes the input with itself:
    // the audibility contract does not apply to it, but determinism and
    // neutrality must hold
    let lm = render(vec![FxInstance::LayerMix { amount: 0.5, from: 0 }], vec![], false, 0);
    assert_eq!(lm, dry, "solo LayerMix must be a no-op (dry == wet)");

    // completeness lock: every audio kind of the registry is covered
    let all_tags: std::collections::BTreeSet<&str> = audio.iter().map(|f| f.tag()).collect();
    let covered: std::collections::BTreeSet<&str> = covered.into_iter().collect();
    assert_eq!(covered, all_tags, "every audio kind of all() must pass this contract");
}

/// Test 325 (R.47, item 328): LFO phase determinism. The phase is the integral of the
/// curve of frequency over the clip's POSITION, not a counter of processed samples:
/// a render in one piece equals a render in pieces, a seek equals a cold render,
/// and the frequency curve speeds up the modulation toward the end of the clip.
#[test]
fn t325_lfo_phase_is_a_function_of_position() {
    let trem = vec![FxInstance::Tremolo { rate_hz: 5.0, depth: 1.0 }];

    // (a) in one piece == in pieces: the phase does not depend on buffer slicing
    let whole = render(trem.clone(), vec![], false, 0);
    let p = project(trem.clone(), vec![], false);
    let r = AudioRenderer::new(&p);
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Clicks)),
    );
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut pieces = vec![0f32; DUR as usize * CH];
    let third = (DUR / 3) as usize * CH;
    let n0 = r.render_frames_with(&mut f, &mut store, 0, &mut pieces[..third]);
    assert_eq!(n0.underruns, 0);
    let n1 = r.render_frames_with(&mut f, &mut store, DUR / 3, &mut pieces[third..2 * third]);
    assert_eq!(n1.underruns, 0);
    let n2 = r.render_frames_with(&mut f, &mut store, 2 * (DUR / 3), &mut pieces[2 * third..]);
    assert_eq!(n2.underruns, 0);
    assert_eq!(whole, pieces, "LFO phase must not depend on buffer slicing");

    // (b) sine under tremolo: the rate_hz curve 2 -> 8 Hz speeds up the modulation
    // (the count of envelope minima in the second half is larger than in the first)
    let sine_render = |auto: Vec<Vec<ParamCurve>>| -> Vec<f32> {
        let mut p = project(vec![FxInstance::Tremolo { rate_hz: 2.0, depth: 1.0 }], auto, false);
        p.media[0].path = "synth:sine:440".into();
        let r = AudioRenderer::new(&p);
        let mut f = DirectFetcher::new(None);
        f.add_source(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 440.0 })),
        );
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; DUR as usize * CH];
        assert_eq!(r.render_frames_with(&mut f, &mut store, 0, &mut out).underruns, 0);
        out
    };
    let curve = vec![vec![ParamCurve {
        param: "rate_hz".into(),

        keys: AutoKey::linear(vec![
            AutoKey::at(Tick(0), 2.0),
            AutoKey::at(Tick::from_audio_frames(DUR), 8.0),
        ]),
        source: Default::default(),
    }]];
    let swept = sine_render(curve);
    // envelope over 4 ms windows; minima = tremolo dips
    let env: Vec<f64> = swept
        .chunks(192 * CH)
        .map(|w| w.iter().map(|v| (*v as f64).abs()).fold(0.0, f64::max))
        .collect();
    let dips = |e: &[f64]| -> usize {
        let mut n = 0;
        for i in 1..e.len() - 1 {
            if e[i] < 0.2 && e[i] <= e[i - 1] && e[i] < e[i + 1] {
                n += 1;
            }
        }
        n
    };
    let half = env.len() / 2;
    let (d1, d2) = (dips(&env[..half]), dips(&env[half..]));
    assert!(d2 > d1 + 1, "rate curve 2->8 Hz must speed the modulation up: {d1} dips then {d2}");
}

/// Test 326 (R.47, item 329): echo and dynamics do what their knobs say — repeat
/// position, a series of decays, ping-pong into the opposite channel,
/// the compression law, muting by the gate.
#[test]
fn t326_delay_and_dynamics_behave() {
    // --- Delay: repeat exactly at time_ms, the series decays, the tail is honest ----
    let time_ms = 250.0f32;
    let one = render_on(
        vec![FxInstance::Delay { time_ms, feedback: 0.6, mix: 1.0, pingpong: false }],
        vec![],
        false,
        0,
        AudioPattern::Impulse,
    );
    let frames = |ms: f32| ((ms / 1000.0) * 48_000.0) as usize;
    let at = |out: &[f32], f: usize, ch: usize| out[f * CH + ch].abs();
    // repeats in the left channel: at 250/500/750 ms, decaying
    let (e1, e2, e3) =
        (at(&one, frames(250.0), 0), at(&one, frames(500.0), 0), at(&one, frames(750.0), 0));
    assert!(e1 > 0.2, "first echo at time_ms: {e1}");
    assert!(e2 > 0.05 && e2 < e1, "second echo decays: {e2} vs {e1}");
    assert!(e3 < e2, "series keeps decaying: {e3} vs {e2}");
    // silent between repeats
    assert!(at(&one, frames(125.0), 0) < 1e-3, "silence between echoes");

    // ping-pong: the Clicks alternate by channel — the first (n=0..3)
    // is only LEFT. The time 230 ms is chosen so that the repeats (11040,
    // 22080) do not coincide with the source's clicks (grid 4800).
    let pp = render_on(
        vec![FxInstance::Delay { time_ms: 230.0, feedback: 0.6, mix: 1.0, pingpong: true }],
        vec![],
        false,
        0,
        AudioPattern::Clicks,
    );
    let win = |out: &[f32], f: usize, ch: usize| -> f64 {
        (f.saturating_sub(4)..f + 8).map(|i| out[i * CH + ch].abs() as f64).sum()
    };
    // the first repeat of the left click is still in the left (its own
    // line is read), the second, after the cross-feedback, has moved to the right
    let (l1, r1) = (win(&pp, frames(230.0), 0), win(&pp, frames(230.0), 1));
    assert!(l1 > 0.5 && r1 < l1 * 0.1, "first pp echo stays in its line: {l1}/{r1}");
    let (l2, r2) = (win(&pp, frames(460.0), 0), win(&pp, frames(460.0), 1));
    assert!(r2 > 0.1 && l2 < r2 * 0.2, "second pp echo crosses channels: {l2}/{r2}");

    // --- Compressor: output ~ thr + (in-thr)/ratio ----------------------
    let comp = |fx: Vec<FxInstance>| -> f64 {
        let out = render_on(fx, vec![], false, 0, AudioPattern::Sine { hz: 440.0 });
        // stable section after the attack: the second half
        let tail = &out[out.len() / 2..];
        let rms = (tail.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / tail.len() as f64)
            .sqrt();
        20.0 * (rms * std::f64::consts::SQRT_2).log10()
    };
    let dry_db = comp(vec![]);
    let comp_db = comp(vec![FxInstance::Compressor {
        threshold_db: -18.0,
        ratio: 4.0,
        attack_ms: 1.0,
        release_ms: 150.0,
        makeup_db: 0.0,
        key: None,
    }]);
    // sine 0.5 = -6 dBFS: expect -18 + ( -6 - -18 )/4 = -15 dBFS
    let expect = -18.0 + (dry_db + 18.0) / 4.0;
    assert!(
        (comp_db - expect).abs() < 1.5,
        "compression law: got {comp_db:.1} dB, expected {expect:.1} dB (dry {dry_db:.1})"
    );

    // --- Gate: a quiet sine is muted, a loud one passes ------------------
    let g = |amp_db: f32| -> f64 {
        let hz = 440.0;
        let out = render_on(
            vec![
                FxInstance::gain(amp_db),
                FxInstance::Gate {
                    threshold_db: -30.0,
                    attack_ms: 1.0,
                    hold_ms: 40.0,
                    release_ms: 50.0,
                },
            ],
            vec![],
            false,
            0,
            AudioPattern::Sine { hz },
        );
        let tail = &out[out.len() / 2..];
        (tail.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / tail.len() as f64).sqrt()
    };
    let loud = g(0.0);
    let quiet = g(-40.0);
    assert!(loud > 0.2, "loud sine passes the gate: {loud}");
    assert!(quiet < 1e-4, "quiet sine is gated: {quiet}");
}

/// Test 327 (R.47, item 330): the cost of a dense chain is a PRINT, not an assert
/// (speed is judged by the release run: item 301). Seven events, each with a
/// chain of six nodes.
#[test]
#[ignore]
fn t327_print_dense_chain_cost() {
    let chain = || {
        vec![
            FxInstance::filter(),
            FxInstance::eq(),
            FxInstance::chorus(),
            FxInstance::delay(),
            FxInstance::compressor(),
            // lookahead nodes of R.48: the limiter is cheap, pitch is the most expensive
            // of all (grain resampling for every grain)
            FxInstance::limiter(),
            FxInstance::PitchShift { semitones: 3.0, cents: 0.0 },
            FxInstance::gain(-3.0),
        ]
    };
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:clicks"));
    for i in 0..7 {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(i + 1),
            link_group: None,
            kind: ClipKind::Audio,
            lane: i as u32 % 3,
            start: Tick((i as i64) * 90_000 / 2),
            duration: Tick(90_000 * 8),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: chain(),
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Clicks)),
    );
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let secs = 10i64;
    let mut out = vec![0f32; (48_000 * secs) as usize * CH];
    let t = std::time::Instant::now();
    let stats = r.render_frames_with(&mut f, &mut store, 0, &mut out);
    let ms = t.elapsed().as_secs_f64() * 1e3;
    assert_eq!(stats.underruns, 0);
    println!(
        "t327: 7 events x 8-node chains, {secs} s of audio in {ms:.1} ms ({:.1} ms per second, {:.1}% of realtime)",
        ms / secs as f64,
        ms / (secs as f64 * 1000.0) * 100.0
    );
}
/// Perf print of the lookahead nodes (R.48): how much ONE event costs with the
/// limiter and with each engine's pitch. There are no asserts — the numbers go
/// into the capability’s design.md, and speed can be judged only by the RELEASE build (item 301):
/// `cargo test -p kt-engine --test audio_fx_registry --release -- --ignored bench_pitch`
#[test]
#[ignore]
fn bench_pitch_cost() {
    for (name, fx, engine) in [
        ("dry", vec![], PitchEngine::Wsola),
        ("limiter", vec![FxInstance::limiter()], PitchEngine::Wsola),
        (
            "pitch wsola",
            vec![FxInstance::PitchShift { semitones: 3.0, cents: 0.0 }],
            PitchEngine::Wsola,
        ),
        (
            "pitch vocoder",
            vec![FxInstance::PitchShift { semitones: 3.0, cents: 0.0 }],
            PitchEngine::Vocoder,
        ),
    ] {
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
        p.media.push(MediaRef::new(MediaId(1), "synth:clicks"));
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
            duration: Tick(90_000 * 10),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx,
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        p.validate().unwrap();
        let r = AudioRenderer::new(&p);
        let mut f = DirectFetcher::new(None);
        f.add_source(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Clicks)),
        );
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; (48_000 * 10) as usize * CH];
        let t = std::time::Instant::now();
        r.render_frames_with(&mut f, &mut store, 0, &mut out);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        println!(
            "bench {name}: {:.1} ms per second of audio ({:.1}% of realtime)",
            ms / 10.0,
            ms / 100.0
        );
    }
}
