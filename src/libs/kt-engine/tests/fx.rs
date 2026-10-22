//! Tests 4–6 (docs/porting-notes.md) — regressions for slowed&reverb artifacts.
//!
//! 4 — spectral purity of the warp: 440 Hz at rate 0.5 → peak at 220 Hz,
//!     spurious components < −60 dB (catches both the “46.9 Hz crackle” (defect c)
//!     and the skipped bursts (defect a) of the old engine);
//! 5 — parameter smoothing: changing mix in the middle of playback gives no
//!     sample-to-sample jump (defect e — zeroing of the buffers in setParams);
//! 6 — the reverb tail lives past the clip boundary and survives a graph rebuild
//!     (defect f and “rebuildGraph kills the FX state”).

use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick};

/// item 8.1 (R.28): a render with a FULLY available source must run without a
/// single underrun — otherwise the render asks for material beyond its own window,
/// and on a live cache that is silence plus a growing counter.
#[track_caller]
fn no_underruns(s: kt_engine::RenderStats) {
    assert_eq!(s.underruns, 0, "underrun with a fully available source");
}

const CH: usize = AUDIO_CHANNELS;

fn fx_project(fx: Vec<FxInstance>, pattern_path: &str, dur_frames: i64) -> Project {
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
    p.media.push(MediaRef::new(MediaId(1), pattern_path));
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
        duration: Tick::from_audio_frames(dur_frames),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

/// Speed curve in the NEW domain (R.33): the points (fraction of the material, rate)
/// are converted to the material ticks of the clip `0..dur_frames` with `source_in = 0`.
fn rate_curve(points: &[(f64, f64)], dur_frames: i64) -> Vec<Vec<ParamCurve>> {
    vec![vec![ParamCurve {
        param: "rate".into(),

        keys: AutoKey::linear(
            points
                .iter()
                .map(|(s, r)| {
                    AutoKey::at(Tick::from_audio_frames((s * dur_frames as f64).round() as i64), *r)
                })
                .collect(),
        ),
        source: Default::default(),
    }]]
}

/// A project with automation on a single slot.
fn fx_project_auto(
    fx: Vec<FxInstance>,
    auto: Vec<Vec<ParamCurve>>,
    pattern_path: &str,
    dur_frames: i64,
) -> Project {
    let mut p = fx_project(fx, pattern_path, dur_frames);
    p.clips[0].fx_auto = auto;
    p.validate().unwrap();
    p
}

fn direct(pattern: AudioPattern) -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), pattern)));
    f
}

/// Amplitude spectrum of the mono signal (Hann window), dB relative to the peak.
fn spectrum_db(samples: &[f32]) -> Vec<f32> {
    use realfft::RealFftPlanner;
    let n = samples.len();
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n);
    let mut input: Vec<f32> = samples
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos();
            s * w
        })
        .collect();
    let mut out = fft.make_output_vec();
    fft.process(&mut input, &mut out).unwrap();
    let mags: Vec<f32> = out.iter().map(|c| c.norm()).collect();
    let peak = mags.iter().cloned().fold(f32::MIN, f32::max).max(1e-12);
    mags.iter().map(|m| 20.0 * (m / peak).max(1e-12).log10()).collect()
}

// ---- Test 4 -----------------------------------------------------------------

#[test]
fn t4_slowed_halves_pitch_without_artifacts() {
    let dur = 5 * AUDIO_SAMPLE_RATE as i64; // 5 c
    let p = fx_project(vec![FxInstance::slowed_reverb(0.5, 0.0, 140.0)], "synth:sine:440", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Sine { hz: 440.0 });
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);

    let n = 1 << 16; // 65536 analysis samples
    let skip = 8192; // skip the resampler's transient
    let mut out = vec![0f32; (skip + n) * CH];
    let stats = r.render_frames_with(&mut f, &mut store, 0, &mut out);
    assert_eq!(stats.underruns, 0);

    let mono: Vec<f32> = out[skip * CH..].chunks_exact(CH).map(|fr| fr[0]).collect();
    let db = spectrum_db(&mono);
    let bin_hz = AUDIO_SAMPLE_RATE as f32 / n as f32;
    let peak_bin = db.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0;
    let peak_hz = peak_bin as f32 * bin_hz;
    assert!(
        (peak_hz - 220.0).abs() < 2.0 * bin_hz,
        "rate 0.5 must give 220 Hz, got {peak_hz:.2} Hz"
    );
    // Everything outside ±16 bins (±12 Hz — the Hann window's skirt) is quieter than −60 dB.
    // The artifacts of the old engine lay far beyond this window: the 46.9 Hz crackle
    // would give components at peak±47 Hz (±64 bins), the skipping of bursts — a comb
    // across the whole band.
    for (bin, &v) in db.iter().enumerate() {
        if (bin as i64 - peak_bin as i64).abs() > 16 && v > -60.0 {
            let hz = bin as f32 * bin_hz;
            panic!("spurious component {v:.1} dB at {hz:.1} Hz (bin {bin})");
        }
    }
}

#[test]
fn t4_warp_consumes_source_at_rate() {
    // rate 0.5 on a clip of length D requires D/2 source frames: the ramp at the end of the
    // source window must reach exactly source_in + D*rate (± the resampler chunk).
    let dur = 4 * AUDIO_SAMPLE_RATE as i64;
    let p = fx_project(vec![FxInstance::slowed_reverb(0.5, 0.0, 140.0)], "synth:ramp", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Ramp);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
    // the last output sample ≈ the ramp value at source frame dur*0.5
    let got = out[(dur as usize - 1) * CH];
    let expect = SyntheticSource::sample_at(AudioPattern::Ramp, dur / 2, 0);
    // tolerance = the group delay of the sinc resampler + a couple of chunks; the main point
    // is that there is NO ACCUMULATED drift (the old engine lost a burst every ~230 ms,
    // which over 4 s would give ~17,000 frames).
    let tolerance = 4096.0 * 1e-6;
    assert!(
        (got - expect).abs() < tolerance,
        "the source was consumed not at rate 0.5: end at {got} vs {expect}"
    );
}

// ---- Test 5 -----------------------------------------------------------------

#[test]
fn t5_param_step_does_not_click() {
    let dur = 4 * AUDIO_SAMPLE_RATE as i64;
    let mk = |mix: f32| {
        fx_project(vec![FxInstance::slowed_reverb(1.0, mix, 140.0)], "synth:sine:220", dur)
    };
    let mut f = direct(AudioPattern::Sine { hz: 220.0 });
    let mut store = FxStateStore::new();

    let half = dur as usize / 2;
    let mut out = vec![0f32; dur as usize * CH];

    // first half with mix 0.1
    let p1 = mk(0.1);
    let r1 = AudioRenderer::new(&p1);
    r1.sync_fx(&mut store);
    no_underruns(r1.render_frames_with(&mut f, &mut store, 0, &mut out[..half * CH]));

    // step: mix 0.9 (the same chain structure → the same state, smoothing)
    let p2 = mk(0.9);
    let r2 = AudioRenderer::new(&p2);
    r2.sync_fx(&mut store);
    no_underruns(r2.render_frames_with(&mut f, &mut store, half as i64, &mut out[half * CH..]));

    // the maximum sample-to-sample jump around the step is no larger than
    // the natural slope of the sine with a margin (smoothing ~20 ms)
    let natural = 2.0 * std::f32::consts::PI * 220.0 / AUDIO_SAMPLE_RATE as f32; // ~0.0288 at amplitude 1
    let window = &out[(half - 2000) * CH..(half + 4000) * CH];
    let mut max_jump = 0f32;
    for w in window.chunks_exact(CH).collect::<Vec<_>>().windows(2) {
        max_jump = max_jump.max((w[1][0] - w[0][0]).abs());
    }
    assert!(
        max_jump < natural * 1.5,
        "a step in mix gave a click: max sample-to-sample jump {max_jump}"
    );
}

// ---- Test 6 -----------------------------------------------------------------

#[test]
fn t6_reverb_tail_rings_past_clip_end_and_decays() {
    let dur = AUDIO_SAMPLE_RATE as i64; // 1 c
    let p = fx_project(vec![FxInstance::slowed_reverb(1.0, 0.6, 140.0)], "synth:impulse", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Impulse);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);

    let tail = store.get_mut(ClipId(1)).unwrap().tail_frames();
    assert!(tail > 0);
    let total = (dur + tail) as usize;
    let mut out = vec![0f32; total * CH];
    no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));

    // energy right past the clip boundary is present (the tail rings on)
    let after: f32 = out[dur as usize * CH..(dur as usize + 4800) * CH]
        .iter()
        .map(|s| s.abs())
        .fold(0.0, f32::max);
    assert!(after > 1e-4, "the reverb tail was cut at the clip boundary: {after}");

    // by the end of the tail it decays below −60 dB from the tail's peak
    let tail_last: f32 = out[(total - 4800) * CH..].iter().map(|s| s.abs()).fold(0.0, f32::max);
    assert!(tail_last < after * 0.001, "the tail did not decay within tail(): peak {after}, end {tail_last}");
}

#[test]
fn t6_graph_rebuild_mid_tail_preserves_state() {
    // Editing ANOTHER clip (rebuilding the AudioRenderer) in the middle of playback
    // must not interrupt or change the reverb (the old rebuildGraph killed the state).
    let dur = AUDIO_SAMPLE_RATE as i64;
    let fx = vec![FxInstance::slowed_reverb(1.0, 0.6, 140.0)];
    let p1 = fx_project(fx.clone(), "synth:sine:330", dur);

    // reference: a continuous render in a single graph
    let r1 = AudioRenderer::new(&p1);
    let mut store_a = FxStateStore::new();
    r1.sync_fx(&mut store_a);
    let mut fa = direct(AudioPattern::Sine { hz: 330.0 });
    let mut full = vec![0f32; dur as usize * CH];
    no_underruns(r1.render_frames_with(&mut fa, &mut store_a, 0, &mut full));

    // the same render, but with an “edit” in the middle: an extraneous clip is added,
    // the graph is rebuilt, the store is the same
    let mut store_b = FxStateStore::new();
    r1.sync_fx(&mut store_b);
    let mut fb = direct(AudioPattern::Sine { hz: 330.0 });
    let half = dur as usize / 2;
    let mut split = vec![0f32; dur as usize * CH];
    no_underruns(r1.render_frames_with(&mut fb, &mut store_b, 0, &mut split[..half * CH]));

    let mut p2 = p1.clone();
    p2.media.push(MediaRef::new(MediaId(2), "synth:silence"));
    p2.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(99),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 3,
        start: Tick::from_audio_frames(10 * dur),
        duration: Tick::from_audio_frames(dur),
        source_in: Tick(0),
        media: MediaId(2),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    let r2 = AudioRenderer::new(&p2);
    r2.sync_fx(&mut store_b); // the state of clip 1 must survive the rebuild
    no_underruns(r2.render_frames_with(
        &mut fb,
        &mut store_b,
        half as i64,
        &mut split[half * CH..],
    ));

    assert_eq!(full, split, "rebuilding the graph must not change the sound (state preserved)");
}

// ---- Test 4c: automation of rate really changes the speed over time -------
// (regression: evaluating the automation once per buffer gave a constant
// slowdown; caught by the user by ear — fix: per-chunk evaluation)

/// Instantaneous frequency of the signal in the window by zero crossings (channel 0).
fn window_freq_hz(out: &[f32], from_frame: usize, frames: usize) -> f64 {
    let mut crossings = 0u32;
    let mut prev = out[from_frame * CH];
    for f in (from_frame + 1)..(from_frame + frames) {
        let v = out[f * CH];
        if (prev < 0.0 && v >= 0.0) || (prev > 0.0 && v <= 0.0) {
            crossings += 1;
        }
        prev = v;
    }
    crossings as f64 / 2.0 * AUDIO_SAMPLE_RATE as f64 / frames as f64
}

#[test]
fn t4_automation_changes_rate_over_time() {
    // EVENT DOMAIN (R.37): kf.t is the position in the material, but the AXIS is linear, so
    // at output time τ exactly rate(τ) sounds. The checkpoints are taken
    // straight from the profile — the test no longer needs the map it guards.
    let dur = 40 * AUDIO_SAMPLE_RATE as i64;
    let fx = vec![FxInstance::SlowedReverb {
        rate: 1.0,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    }];
    let p = fx_project_auto(fx, rate_curve(&automation_rate_profile(), dur), "synth:sine:240", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Sine { hz: 240.0 });
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    // render in ONE piece — like the offline export (where the regression was)
    let mut out = vec![0f32; dur as usize * CH];
    assert_eq!(r.render_frames_with(&mut f, &mut store, 0, &mut out).underruns, 0);

    let win = AUDIO_SAMPLE_RATE as usize; // window 1 s
    // (the profile here and in the clip's curve is the same one)
    let checks = [
        (0.10, 1.0),  // before the first point of the profile
        (0.30, 0.75), // middle of the decline
        (0.45, 0.5),  // centre of the plateau
        (0.95, 1.0),  // far after the last kf
    ];
    for (t, expected_rate) in checks {
        let center = (dur as f64 * t) as usize;
        let freq = window_freq_hz(&out, center - win / 2, win);
        let expected = 240.0 * expected_rate;
        assert!(
            (freq - expected).abs() < expected * 0.05,
            "t={t}: frequency {freq:.1} Hz, expected ~{expected:.1} Hz — \
             automation must change the speed along the profile"
        );
    }
}

fn automation_rate_profile() -> Vec<(f64, f64)> {
    vec![(0.20, 1.0), (0.40, 0.5), (0.50, 0.5), (0.65, 1.0)]
}

fn automation_probe() -> &'static [(f64, f64)] {
    &[(0.2, 1.0), (0.3, 0.35), (0.7, 0.6), (0.8, 1.0)]
}

#[test]
fn t4_automation_consumes_source_by_integral() {
    // “Dangling end”: at duration = src/avg_rate the source must be read
    // to the end (± the resampler delay), not to the avg fraction of a constant.
    let avg = kt_fx::avg_rate(1.0, automation_probe());
    let dur = 20 * AUDIO_SAMPLE_RATE as i64;
    let fx = vec![FxInstance::SlowedReverb {
        rate: 1.0,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    }];
    let p = fx_project_auto(fx, rate_curve(automation_probe(), dur), "synth:ramp", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Ramp);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));

    // we try not the very edge (the last ~64 samples legitimately decay:
    // the sinc resampler sees zeros beyond the source window), but a point slightly earlier
    // plus several intermediate ones — the integral mapping across the whole clip
    for frac in [0.35f64, 0.55, 0.75, 0.95, 0.995] {
        let x = (dur as f64 * frac) as usize;
        let got = out[x * CH];
        let expect_src = dur as f64 * kt_fx::source_progress(1.0, automation_probe(), frac);
        let expect = SyntheticSource::sample_at(AudioPattern::Ramp, expect_src as i64, 0);
        let tolerance = 4096.0 * 1e-6;
        assert!(
            (got - expect).abs() < tolerance,
            "t={frac}: source at {:.0}, expected {:.0} (integral of the profile)",
            got as f64 / 1e-6,
            expect_src
        );
    }
    let _ = avg;
}

/// Test 31 (R.33; FLIPPED in R.37 by the user's decision). Before, it
/// asserted the opposite: “an early speed bump does not move the MATERIAL that the
/// late keys fire on, but it moves the TIME”. With R.37 the automation axis is the
/// event's own span, so the mirror image is true: **an early bump does not move the
/// MOMENT at which the late keys fire, but moves the material playing at that moment.**
///
/// The test is not weakened but flipped: the number of checks is the same,
/// and a probe is added in the TAIL. A probe only on the plateau (τ=0.55)
/// would give a false green — there the old and new semantics coincide;
/// they diverge at τ=0.90.
#[test]
fn t31_keyframes_anchor_to_event_time() {
    let dur = 20 * AUDIO_SAMPLE_RATE as i64; // L, frames
    let profile_a: Vec<(f64, f64)> = vec![(0.40, 1.0), (0.50, 0.5), (0.70, 0.5), (0.75, 1.0)];
    let mut profile_b: Vec<(f64, f64)> = vec![(0.10, 1.0), (0.18, 2.0), (0.26, 2.0), (0.34, 1.0)];
    profile_b.extend(profile_a.iter().copied());

    let render = |profile: Vec<(f64, f64)>| -> Vec<f32> {
        let fx = vec![FxInstance::SlowedReverb {
            rate: 1.0,
            mix: 0.0, // the ramp must not be contaminated by the reverb
            reverb_delay_ms: 85.0,
            damp: 0.32,
            decay: 0.58,
            wet_gain: 0.95,
            predelay_ms: 0.0,
            width: 1.0,
            frames: Default::default(),
            automation: vec![],
        }];
        let p = fx_project_auto(fx, rate_curve(&profile, dur), "synth:ramp", dur);
        let r = AudioRenderer::new(&p);
        let mut f = direct(AudioPattern::Ramp);
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; dur as usize * CH];
        assert_eq!(r.render_frames_with(&mut f, &mut store, 0, &mut out).underruns, 0);
        out
    };
    let out_a = render(profile_a.clone());
    let out_b = render(profile_b.clone());

    // the material playing at the OUTPUT progress τ (the ramp encodes the frame)
    let material_at_out =
        |out: &[f32], tau: f64| -> f64 { out[((tau * dur as f64) as usize) * CH] as f64 / 1e-6 };
    // the actual speed at the OUTPUT progress τ (window 0.5 s)
    let rate_at_out = |out: &[f32], tau: f64| -> f64 {
        let x = (tau * dur as f64) as usize;
        let w = AUDIO_SAMPLE_RATE as usize / 2;
        (out[(x + w) * CH] as f64 - out[x * CH] as f64) / (w as f64 * 1e-6)
    };

    // 1) ANCHOR: at the same output time the same speed plays —
    //    the early bump did NOT shift the moment
    for (tau, want) in [(0.55, 0.5), (0.60, 0.5), (0.64, 0.5), (0.85, 1.0), (0.90, 1.0)] {
        let ra = rate_at_out(&out_a, tau);
        let rb = rate_at_out(&out_b, tau);
        assert!(
            (ra - want).abs() < 0.06,
            "A: at τ={tau} the speed must be {want}, got {ra:.3}"
        );
        assert!(
            (rb - want).abs() < 0.06,
            "B: an early keyframe must NOT shift the MOMENT of firing, got {rb:.3}"
        );
    }

    // 2) the bump is active at ITS output time, and only for B
    assert!((rate_at_out(&out_b, 0.22) - 2.0).abs() < 0.2, "B: τ=0.22 sped up ×2");
    assert!((rate_at_out(&out_a, 0.22) - 1.0).abs() < 0.1, "A: τ=0.22 without speed-up");

    // 3) acceleration shifts the MATERIAL playing at that moment, not the moment;
    //    the size of the shift is predicted by the map: S_B(τ) − S_A(τ)
    let ma = material_at_out(&out_a, 0.5);
    let mb = material_at_out(&out_b, 0.5);
    assert!(mb > ma, "with an early speed-up toward τ=0.5 MORE material was consumed: {mb} vs {ma}");
    let pred =
        kt_fx::source_progress(1.0, &profile_b, 0.5) - kt_fx::source_progress(1.0, &profile_a, 0.5);
    let got = (mb - ma) / dur as f64;
    assert!(
        (got - pred).abs() < 8192.0 / dur as f64,
        "material shift {got:.5} must match the prediction of the map {pred:.5}"
    );

    // 4) by the end of the event exactly avg_rate of material is consumed — the source
    //    window and the prefetch rest on this
    for (out, prof) in [(&out_a, &profile_a), (&out_b, &profile_b)] {
        let end = material_at_out(out, 0.995) / dur as f64;
        let want = kt_fx::avg_rate(1.0, prof);
        assert!(
            (end - want).abs() < 0.03,
            "consumed {end:.4} of material, the map promises {want:.4}"
        );
    }
}

/// Test 213 (R.33 C1): AUDIO parameters are automated by the common mechanism.
/// Until this round, curves existed only for video FX, and for audio there was
/// only slowed&reverb's own machinery; there was nothing to automate the volume with
/// (the user's requirement:
/// “automation of all parameters of all effects is needed”).
#[test]
fn t213_audio_gain_follows_its_curve() {
    let dur = 4 * AUDIO_SAMPLE_RATE as i64;
    let fx = vec![FxInstance::gain(0.0)];
    // volume curve: silence → 0 dB → silence, keys in material ticks
    let auto = vec![vec![ParamCurve {
        param: "db".into(),

        keys: AutoKey::linear(vec![
            AutoKey::at(Tick::from_audio_frames(0), -60.0),
            AutoKey::at(Tick::from_audio_frames(dur / 2), 0.0),
            AutoKey::at(Tick::from_audio_frames(dur), -60.0),
        ]),
        source: Default::default(),
    }]];
    let p = fx_project_auto(fx, auto, "synth:sine:440", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Sine { hz: 440.0 });
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));

    let rms = |from: usize, n: usize| -> f64 {
        let s: f64 =
            out[from * CH..(from + n) * CH].iter().map(|v| (*v as f64) * (*v as f64)).sum();
        (s / (n * CH) as f64).sqrt()
    };
    let win = AUDIO_SAMPLE_RATE as usize / 4;
    let head = rms(win / 2, win);
    let mid = rms(dur as usize / 2 - win / 2, win);
    let tail = rms(dur as usize - win * 2, win);
    assert!(mid > head * 8.0, "loudness must rise toward the middle: {head:.4} → {mid:.4}");
    assert!(mid > tail * 8.0, "and fall toward the end: {mid:.4} → {tail:.4}");
    // at the very peak — roughly the original sine amplitude (0 dB): 0.5/√2.
    // The window is narrow: a wide one averages the slope around the peak and underestimates the RMS, and
    // the parameter smoothing (one-pole ~20 ms) lags slightly behind the curve —
    // this is not a defect, but the very thing it exists for (without it there would be clicks here)
    let peak = rms(dur as usize / 2 - 375, 750);
    assert!((peak - 0.3536).abs() < 0.03, "at 0 dB the original level is expected, got {peak:.4}");

    // without a curve the sound is byte for byte as before (the constant path is untouched)
    let plain = fx_project(vec![FxInstance::gain(0.0)], "synth:sine:440", dur);
    let r2 = AudioRenderer::new(&plain);
    let mut f2 = direct(AudioPattern::Sine { hz: 440.0 });
    let mut store2 = FxStateStore::new();
    r2.sync_fx(&mut store2);
    let mut out2 = vec![0f32; dur as usize * CH];
    no_underruns(r2.render_frames_with(&mut f2, &mut store2, 0, &mut out2));
    let src: Vec<f32> = (0..dur)
        .flat_map(|i| {
            let v = SyntheticSource::sample_at(AudioPattern::Sine { hz: 440.0 }, i, 0);
            [v, v]
        })
        .collect();
    assert_eq!(out2, src, "without a curve the loudness must not touch the signal at all");
}

/// Keyframes of the old-style slowed&reverb move into the common mechanism
/// (R.33): they no longer have their own domain, but the user's project
/// must sound the same.
#[test]
fn t213_legacy_slowed_automation_migrates_to_curves() {
    let dur = 20 * AUDIO_SAMPLE_RATE as i64;
    let mut p = fx_project(
        vec![FxInstance::SlowedReverb {
            rate: 1.0,
            mix: 0.0,
            reverb_delay_ms: 85.0,
            damp: 0.32,
            decay: 0.58,
            wet_gain: 0.95,
            predelay_ms: 0.0,
            width: 1.0,
            frames: Default::default(),
            automation: vec![
                SlowedKeyframe { t: 0.2, rate: 1.0, mix: 0.0 },
                SlowedKeyframe { t: 0.8, rate: 0.5, mix: 0.0 },
            ],
        }],
        "synth:ramp",
        dur,
    );
    assert!(kt_model::migrate_legacy_slowed(&mut p.clips[0]), "the migration must fire");
    let c = &p.clips[0];
    // the legacy field is now empty, the curves are set in material ticks
    assert!(
        matches!(&c.fx[0], FxInstance::SlowedReverb { automation, .. } if automation.is_empty()),
        "the old field must be emptied — otherwise it moves back into the file"
    );
    let rate = c.fx_auto[0].iter().find(|k| k.param == "rate").expect("rate curve");
    assert_eq!(
        rate.keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![
            (0.2 * c.duration.0 as f64).round() as i64,
            (0.8 * c.duration.0 as f64).round() as i64
        ],
        "fractions of material must become ticks of material"
    );
    assert!(c.fx_auto[0].iter().any(|k| k.param == "mix"), "mix moves too");
    // and the project plays: the speed profile continues to act
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Ramp);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
    let end_src = out[(dur as usize - 1) * CH] as f64 / 1e-6;
    assert!(
        end_src < dur as f64 * 0.95,
        "the slowdown must remain: consumed {end_src:.0} of {dur}"
    );
}

/// Test 229 (R.33 C5): new parameters of AUDIO effects. The defaults
/// byte for byte are held by GOLDEN_AUDIO; here — that a shifted parameter
/// is really audible and is audible correctly.
#[test]
fn t229_new_audio_params() {
    let dur = 2 * AUDIO_SAMPLE_RATE as i64;
    let render = |fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>| -> Vec<f32> {
        let p = fx_project_auto(fx, auto, "synth:sine:440", dur);
        let r = AudioRenderer::new(&p);
        let mut f = direct(AudioPattern::Sine { hz: 440.0 });
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; dur as usize * CH];
        no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
        out
    };
    let rms = |out: &[f32], ch: usize| -> f64 {
        let mut s = 0.0;
        for f in out.chunks_exact(CH) {
            s += (f[ch] as f64).powi(2);
        }
        (s / (out.len() / CH) as f64).sqrt()
    };

    // ---- Gain.pan: balance ------------------------------------------------
    let flat = render(vec![FxInstance::gain(0.0)], vec![]);
    assert_eq!(
        flat,
        render(vec![FxInstance::Gain { db: 0.0, pan: 0.0 }], vec![]),
        "balance 0 — the old path is byte-for-byte"
    );
    let left = render(vec![FxInstance::Gain { db: 0.0, pan: -1.0 }], vec![]);
    assert!(rms(&left, 1) < 1e-9, "full shift to the left mutes the right channel");
    assert!((rms(&left, 0) - rms(&flat, 0)).abs() < 1e-6, "the left channel is untouched");
    let half = render(vec![FxInstance::Gain { db: 0.0, pan: 0.5 }], vec![]);
    assert!(
        (rms(&half, 0) / rms(&flat, 0) - 0.5).abs() < 0.01,
        "balance +0.5 must halve the left: {}",
        rms(&half, 0) / rms(&flat, 0)
    );

    // ---- Gain.pan is automated by the same mechanism ---------------------
    let curve = vec![vec![ParamCurve {
        param: "pan".into(),

        keys: AutoKey::linear(vec![
            AutoKey::at(Tick(0), -1.0),
            AutoKey::at(Tick::from_audio_frames(dur), 1.0),
        ]),
        source: Default::default(),
    }]];
    let swept = render(vec![FxInstance::Gain { db: 0.0, pan: 0.0 }], curve);
    let half_len = swept.len() / 2;
    let (l1, r1) = (rms(&swept[..half_len], 0), rms(&swept[..half_len], 1));
    let (l2, r2) = (rms(&swept[half_len..], 0), rms(&swept[half_len..], 1));
    assert!(l1 > r1 && l2 < r2, "the balance curve must move from left to right");

    // ---- SlowedReverb: pre-delay and width ----------------------------
    let rev = |predelay_ms: f32, width: f32| {
        vec![FxInstance::SlowedReverb {
            rate: 1.0,
            mix: 1.0,
            reverb_delay_ms: 85.0,
            damp: 0.2,
            decay: 0.84,
            wet_gain: 1.0,
            predelay_ms,
            width,
            frames: Default::default(),
            automation: vec![],
        }]
    };
    let plain = render(rev(0.0, 1.0), vec![]);
    // the tail is audible AFTER the end of the material — we compare the onset of the response
    let onset = |out: &[f32]| -> usize {
        out.chunks_exact(CH).position(|f| f[0].abs() > 0.02).unwrap_or(usize::MAX)
    };
    let delayed = render(rev(100.0, 1.0), vec![]);
    assert!(
        onset(&delayed) > onset(&plain),
        "pre-delay must shift the response: {} vs {}",
        onset(&delayed),
        onset(&plain)
    );
    // width 0 — the tail's channels coincide (mono), 1 — they diverge
    let mono = render(rev(0.0, 0.0), vec![]);
    let diff = |out: &[f32]| -> f64 {
        out.chunks_exact(CH).map(|f| (f[0] - f[1]).abs() as f64).sum::<f64>()
    };
    assert!(
        diff(&mono) < diff(&plain) * 0.5,
        "width 0 must collapse the tail to mono: {} vs {}",
        diff(&mono),
        diff(&plain)
    );
}

/// Test 246 (R.37 C1): consumption of the source is the INTEGRAL of the speed curve over
/// OUTPUT time. A direct check of the new semantics, independent of
/// GOLDEN_AUDIO: before, the reference was the only thing guarding it.
#[test]
fn t246_consumption_is_the_integral_over_event_time() {
    let dur = 10 * AUDIO_SAMPLE_RATE as i64;
    // ramp 1.0 → 2.0 exactly across the event: integral = average speed = 1.5
    let profile: Vec<(f64, f64)> = vec![(0.0, 1.0), (1.0, 2.0)];
    assert!(
        (kt_fx::avg_rate(1.0, &profile) - 1.5).abs() < 1e-15,
        "map: a linear ramp 1→2 consumes exactly 1.5 of the event length"
    );

    let fx = vec![FxInstance::SlowedReverb {
        rate: 1.0,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    }];
    let p = fx_project_auto(fx, rate_curve(&profile, dur), "synth:ramp", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Ramp);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    assert_eq!(r.render_frames_with(&mut f, &mut store, 0, &mut out).underruns, 0);

    // the ramp encodes the source frame number — we read it directly from the output
    let src_at = |tau: f64| out[((tau * dur as f64) as usize) * CH] as f64 / 1e-6;
    for tau in [0.25f64, 0.5, 0.75, 0.99] {
        let want = dur as f64 * kt_fx::source_progress(1.0, &profile, tau);
        let got = src_at(tau);
        assert!(
            (got - want).abs() < 8192.0,
            "τ={tau}: consumed {got:.0} of material, the integral promises {want:.0}"
        );
    }
    // the quadraticness is visible to the naked eye: by the middle of the event 0.625 of the
    // length is consumed (a parabola), not 0.75 (which linear interpolation would give)
    assert!((src_at(0.5) / dur as f64 - 0.625).abs() < 0.01);
}

/// Test 246b (R.37 C1): the curve of ANY parameter also lives in output
/// time. Before, `mix`/`gain`/`pan` were indexed by the consumed
/// material, and an early speed key shifted the moment they fired —
/// only GOLDEN_AUDIO guarded this half.
#[test]
fn t246_gain_curve_peaks_at_event_middle_under_warp() {
    let dur = 8 * AUDIO_SAMPLE_RATE as i64;
    let t = |frac: f64| Tick::from_audio_frames((dur as f64 * frac) as i64);
    // the clip is slowed by half; the volume is a triangle peaking in the MIDDLE
    let fx = vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0), FxInstance::gain(0.0)];
    let auto = vec![
        vec![],
        vec![ParamCurve {
            param: "db".into(),

            keys: AutoKey::linear(vec![
                AutoKey::at(t(0.0), -60.0),
                AutoKey::at(t(0.5), 0.0),
                AutoKey::at(t(1.0), -60.0),
            ]),
            source: Default::default(),
        }],
    ];
    let p = fx_project_auto(fx, auto, "synth:sine:440", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Sine { hz: 440.0 });
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    assert_eq!(r.render_frames_with(&mut f, &mut store, 0, &mut out).underruns, 0);

    let win = AUDIO_SAMPLE_RATE as usize / 2;
    let rms = |center: usize| -> f64 {
        let (a, b) = (center.saturating_sub(win / 2), (center + win / 2).min(dur as usize));
        let s: f64 = out[a * CH..b * CH].iter().map(|v| (*v as f64).powi(2)).sum();
        (s / ((b - a) * CH) as f64).sqrt()
    };
    let mid = rms(dur as usize / 2);
    let quarter = rms(dur as usize / 4);
    let three_q = rms(dur as usize * 3 / 4);
    // the peak — in the MIDDLE OF THE EVENT. Under the old semantics the key “middle of the
    // material” fell at the END of the event (0.5 of the material at 0.5× is
    // consumed over the whole event), and here there would be a monotonically rising ramp
    assert!(mid > quarter * 3.0, "the loudness peak must be in the middle: {mid:.4} vs {quarter:.4}");
    assert!(mid > three_q * 3.0, "and fall toward the end: {mid:.4} vs {three_q:.4}");
    assert!(
        (quarter / three_q).ln().abs() < 0.7,
        "the triangle is symmetric: {quarter:.5} vs {three_q:.5}"
    );
}

// ---- Test 320/321 (R.47, item 325): honest order of the audio slots ---------------

/// Test 320: processing runs IN CHAIN ORDER. An automated Gain
/// BEFORE the reverb is an input FADE: the reverb tail must still ring on after
/// the input has gone to zero. Until R.47 the pipeline was fixed (the volume was always
/// after the reverb), and the same fade choked the TAIL — red-proof: on a commit before the node chain
/// the RMS of the last quarter is ~0.
#[test]
fn t320_slot_order_is_honest_gain_before_reverb_fades_the_input() {
    let dur = 48_000 * 4;
    let p = fx_project_auto(
        vec![
            FxInstance::gain(0.0),
            FxInstance::SlowedReverb {
                rate: 1.0,
                mix: 1.0,
                reverb_delay_ms: 100.0,
                damp: 0.1,
                decay: 0.95,
                wet_gain: 1.0,
                predelay_ms: 0.0,
                width: 1.0,
                frames: Default::default(),
                automation: vec![],
            },
        ],
        vec![
            vec![ParamCurve {
                param: "db".into(),

                keys: AutoKey::linear(vec![
                    AutoKey::at(Tick(0), 0.0),
                    AutoKey::at(Tick::from_audio_frames(dur / 4), -60.0),
                ]),
                source: Default::default(),
            }],
            vec![],
        ],
        "synth:sine:330",
        dur,
    );
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Sine { hz: 330.0 });
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
    let rms = |s: &[f32]| -> f64 {
        (s.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / s.len() as f64).sqrt()
    };
    let frames = |a: f64, b: f64| ((a * 48_000.0) as usize * CH)..((b * 48_000.0) as usize * CH);
    let head = rms(&out[frames(0.0, 0.5)]);
    // a window right after the input fade (2.4..3.6 s): the reverb tail from the loud
    // first half is still heavy (decay 0.95 ≈ t60 5 s)
    let tail = rms(&out[frames(1.2, 2.4)]);
    assert!(head > 0.03, "start must sound: {head}");
    assert!(
        tail > head * 0.05,
        "reverb tail must ring after the INPUT fade (slot order): tail {tail}, head {head}"
    );
}

/// Test 321: byte anchors of single-slot chains after the node refactoring.
/// [Gain −6] — exactly the source × 10^(−6/20) element-wise (the constant path —
/// the same multiplier; slicing into chunks does not change the float multiplication).
#[test]
fn t321_single_gain_is_exact_scalar_multiply() {
    let dur = 48_000;
    let dry = {
        let p = fx_project(vec![], "synth:ramp", dur);
        let r = AudioRenderer::new(&p);
        let mut f = direct(AudioPattern::Ramp);
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; dur as usize * CH];
        no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
        out
    };
    let gained = {
        let p = fx_project(vec![FxInstance::gain(-6.0)], "synth:ramp", dur);
        let r = AudioRenderer::new(&p);
        let mut f = direct(AudioPattern::Ramp);
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; dur as usize * CH];
        no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
        out
    };
    let k = 10f32.powf(-6.0 / 20.0);
    for (i, (d, g)) in dry.iter().zip(gained.iter()).enumerate() {
        assert_eq!(*g, d * k, "sample {i}: gain must be one exact multiply");
    }
}

// ---- Tests 323/324 (R.47, item 327): the Ka2 quintet -------------------------------

/// Test 323: slot order on NONLINEARITY. “Quiet → overload” is clean (the drive
/// barely bites at −40 dB), “overload → quiet” is dirty but quiet: the spectra
/// and RMS must differ several times over. On linear effects the order is audible
/// only with curves (t320) — distortion proves it on constants.
#[test]
fn t323_slot_order_matters_on_nonlinear_chains() {
    let dur = 48_000;
    let render = |fx: Vec<FxInstance>| -> Vec<f32> {
        let p = fx_project(fx, "synth:sine:440", dur);
        let r = AudioRenderer::new(&p);
        let mut f = direct(AudioPattern::Sine { hz: 440.0 });
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; dur as usize * CH];
        no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
        out
    };
    let quiet_then_drive = render(vec![
        FxInstance::gain(-40.0),
        FxInstance::Distortion { drive_db: 24.0, tone: 1.0, mix: 1.0 },
    ]);
    let drive_then_quiet = render(vec![
        FxInstance::Distortion { drive_db: 24.0, tone: 1.0, mix: 1.0 },
        FxInstance::gain(-40.0),
    ]);
    let rms = |s: &[f32]| -> f64 {
        (s.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / s.len() as f64).sqrt()
    };
    let (a, b) = (rms(&quiet_then_drive), rms(&drive_then_quiet));
    assert!(a / b > 2.0 || b / a > 2.0, "nonlinear chain must depend on slot order: {a} vs {b}");
}

/// Test 324: spectral probes of the Ka2 quintet — the formulas do what
/// their knobs say.
#[test]
fn t324_ka2_spectral_probes() {
    let dur = 48_000;
    let render = |fx: Vec<FxInstance>, hz: f64| -> Vec<f32> {
        let p = fx_project(fx, "synth:sine:1000", dur);
        let r = AudioRenderer::new(&p);
        let mut f = direct(AudioPattern::Sine { hz });
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; dur as usize * CH];
        no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
        out
    };
    let rms_mono = |out: &[f32]| -> f64 {
        let mono: Vec<f64> =
            out.chunks_exact(CH).map(|f| (f[0] as f64 + f[1] as f64) * 0.5).collect();
        (mono.iter().map(|v| v * v).sum::<f64>() / mono.len() as f64).sqrt()
    };
    let lp = |cutoff: f32| FxInstance::Filter {
        mode: FilterMode::Lowpass,
        cutoff_hz: cutoff,
        resonance: 0.707,
    };

    // LP 800: 200 Hz passes, 8 kHz is muted by ≥ 40 dB
    let pass = rms_mono(&render(vec![lp(800.0)], 200.0));
    let dry = rms_mono(&render(vec![], 200.0));
    assert!((pass / dry - 1.0).abs() < 0.1, "LP passband: {pass} vs {dry}");
    let cut = rms_mono(&render(vec![lp(800.0)], 8000.0));
    let dry8 = rms_mono(&render(vec![], 8000.0));
    assert!(cut / dry8 < 0.01, "LP must kill 8 kHz by 40 dB: {}", cut / dry8);

    // EQ: low_gain −12 mutes 100 Hz by more than half, does not touch 8 kHz
    let mut eq = FxInstance::eq();
    eq.set_param("low_gain_db", -12.0);
    let low = rms_mono(&render(vec![eq.clone()], 100.0));
    let dry100 = rms_mono(&render(vec![], 100.0));
    let ratio = low / dry100;
    assert!((ratio - 0.25).abs() < 0.1, "EQ low shelf -12 dB: {ratio}");
    let high = rms_mono(&render(vec![eq], 8000.0));
    assert!((high / dry8 - 1.0).abs() < 0.1, "EQ low shelf must not touch 8 kHz");

    // Bitcrusher bits 3 raises the noise floor on a pure sine
    let crushed =
        render(vec![FxInstance::Bitcrusher { bits: 3.0, rate_hz: 48_000.0, mix: 1.0 }], 1000.0);
    let clean = render(vec![], 1000.0);
    let mono: Vec<f32> = crushed.chunks_exact(CH).map(|f| (f[0] + f[1]) * 0.5).collect();
    let mono_clean: Vec<f32> = clean.chunks_exact(CH).map(|f| (f[0] + f[1]) * 0.5).collect();
    let sp = spectrum_db(&mono[8192..16384]);
    let sp_clean = spectrum_db(&mono_clean[8192..16384]);
    // the quantization is PERIODIC — a comb of harmonics, not a continuous floor: we measure the PEAK
    // in a band far from the tone (bin = 48000/8192 ≈ 5.86 Hz; 1000..3000 =
    // 5.9..17.6 kHz — where the odd harmonics of the quantization live)
    let peak = |sp: &[f32]| -> f32 { sp[1000..3000].iter().cloned().fold(f32::MIN, f32::max) };
    assert!(
        peak(&sp) > peak(&sp_clean) + 60.0,
        "3-bit quantization must spawn harmonics: {} vs {}",
        peak(&sp),
        peak(&sp_clean)
    );

    // StereoWidth 0 — mono: the channels are equal; 2 — the side energy grows
    let p = fx_project(vec![FxInstance::StereoWidth { width: 0.0 }], "synth:ramp", dur);
    let r = AudioRenderer::new(&p);
    let mut f = direct(AudioPattern::Ramp);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; dur as usize * CH];
    no_underruns(r.render_frames_with(&mut f, &mut store, 0, &mut out));
    for fr in out.chunks_exact(CH) {
        assert!((fr[0] - fr[1]).abs() < 1e-6, "width 0 must be mono");
    }
}
