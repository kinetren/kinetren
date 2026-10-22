//! Test 136 (item 136 of porting-notes, R.24): audio crossfade by TRANSITIONS.
//!
//! The equal-power weights come from the TransitionEvent (the fade windows), and the “handles” read
//! the source beyond the edges of the used piece. The cornerstone assert: converting an old overlap
//! (trim to C + a transition with the same window) gives a BYTE-for-BYTE the same render —
//! the immutability of GOLDEN_AUDIO at stage 6 rests on this.

use kt_engine::AudioRenderer;
use kt_engine::DirectFetcher;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;

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
    let mut ids: Vec<MediaId> = clips.iter().map(|c| c.media).collect();
    ids.sort();
    ids.dedup();
    for id in ids {
        p.media.push(MediaRef::new(id, format!("synth:{}", id.0)));
    }
    p.clips = clips;
    p.validate().unwrap();
    p
}

fn clip(id: u64, start_f: i64, dur_f: i64, src_in_f: i64, media: u64) -> ClipEvent {
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
        start: Tick::from_audio_frames(start_f),
        duration: Tick::from_audio_frames(dur_f),
        source_in: Tick::from_audio_frames(src_in_f),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn direct(medias: &[u64]) -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    for &id in medias {
        f.add_source(
            MediaId(id),
            Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)),
        );
    }
    f
}

fn render(p: &Project, medias: &[u64], total: usize) -> (Vec<f32>, u64) {
    let r = AudioRenderer::new(p);
    let mut f = direct(medias);
    let mut out = vec![0f32; total * CH];
    let stats = r.render_frames(&mut f, 0, &mut out);
    (out, stats.underruns)
}

/// Conversion of the overlap by the upgrade_legacy formula: overlap [b0, a1) →
/// the joint C = the midpoint, trims, the transition with the same window. The reference is the analytic
/// formula of the old weight_at (the old render path itself is removed, decision 8 R.24;
/// render-against-render is checked by the immutability of GOLDEN_AUDIO in t11).
#[test]
fn t136_converted_overlap_matches_old_crossfade_math() {
    // old world: A[0,12288) src 0, B[8192,20480) src 0 — overlap [8192,12288);
    // new world: C = 10240, A is trimmed to C, B: source_in += C − b0
    let mut p = project(vec![clip(1, 0, 10_240, 0, 1), clip(2, 10_240, 10_240, 2_048, 2)]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick::from_audio_frames(2_048),
        out_offset: Tick::from_audio_frames(2_048),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();

    let total = 20_480usize;
    let (got, underruns) = render(&p, &[1, 2], total);
    assert_eq!(underruns, 0, "handles must not underrun");
    let (w0, w1) = (8_192i64, 12_288i64);
    for f in 0..total as i64 {
        // old semantics: A reads src=f, B reads src=f−8192; equal-power weights by the
        // progress of the overlap — literally the weight_at formulas
        let a_val = SyntheticSource::sample_at(AudioPattern::Ramp, f, 0);
        let b_val = SyntheticSource::sample_at(AudioPattern::Ramp, f - w0, 0);
        let expect = if f < w0 {
            a_val
        } else if f < w1 {
            let t = (f - w0) as f64 / (w1 - w0) as f64;
            a_val * (((1.0 - t).max(0.0)).sqrt() as f32) + b_val * ((t.max(0.0)).sqrt() as f32)
        } else {
            b_val
        };
        assert_eq!(got[(f as usize) * CH], expect, "frame {f}: conversion must be exact");
    }
}

/// The weights and the handle material — exact per-frame math.
#[test]
fn t136_fade_math_and_handle_material() {
    // the joint at 10240, window [8192, 12288); the sources are distinguishable by their source_in's
    let mut p = project(vec![clip(1, 0, 10_240, 0, 1), clip(2, 10_240, 10_240, 50_000, 2)]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick::from_audio_frames(2_048),
        out_offset: Tick::from_audio_frames(2_048),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let (out, underruns) = render(&p, &[1, 2], 14_336);
    assert_eq!(underruns, 0);
    let (w0, w1) = (8_192i64, 12_288i64);
    for f in [0i64, 4_000, 8_191, 8_192, 9_000, 10_239, 10_240, 11_000, 12_287, 12_288, 14_000] {
        let a_src = f; // clip 1: source_in 0, body [0,10240), handle [10240,12288)
        let b_src = 50_000 + (f - 10_240); // clip 2: the handle to the left of source_in
        let a_val = SyntheticSource::sample_at(AudioPattern::Ramp, a_src, 0);
        let b_val = SyntheticSource::sample_at(AudioPattern::Ramp, b_src, 0);
        let t = ((f - w0) as f64 / (w1 - w0) as f64).clamp(0.0, 1.0);
        let (wa, wb) = if f < w0 {
            (1.0f32, 0.0f32)
        } else if f < w1 {
            (((1.0 - t).max(0.0)).sqrt() as f32, (t.max(0.0)).sqrt() as f32)
        } else {
            (0.0, 1.0)
        };
        // the render summation order: clip 1 (early) then clip 2
        let expect = if f < w0 {
            a_val * wa
        } else if f < w1 {
            a_val * wa + b_val * wb
        } else {
            b_val * wb
        };
        assert_eq!(out[(f as usize) * CH], expect, "frame {f}");
    }
}

/// A handle longer than the material: source_in = 0 for the incoming — silence without an underrun.
#[test]
fn t136_missing_left_handle_is_silent_without_underruns() {
    let mut p = project(vec![clip(1, 0, 10_240, 0, 1), clip(2, 10_240, 10_240, 0, 2)]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick::from_audio_frames(2_048),
        out_offset: Tick::from_audio_frames(2_048),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let (out, underruns) = render(&p, &[1, 2], 12_288);
    assert_eq!(underruns, 0, "missing handle material is silence, not underrun");
    // in [w0, joint): only the decaying A sounds (B has no material)
    let (w0, w1) = (8_192i64, 12_288i64);
    for f in [8_192i64, 9_000, 10_000] {
        let t = (f - w0) as f64 / (w1 - w0) as f64;
        let expect = SyntheticSource::sample_at(AudioPattern::Ramp, f, 0)
            * (((1.0 - t).max(0.0)).sqrt() as f32);
        assert_eq!(out[(f as usize) * CH], expect, "frame {f}: A only");
    }
}

/// the FX path (a chain without warp) must read the handles the same way as the plain path.
#[test]
fn t136_fx_path_handles_match_plain() {
    let mk = |fx: Vec<FxInstance>| {
        let mut c1 = clip(1, 0, 10_240, 0, 1);
        c1.fx = fx;
        let mut p = project(vec![c1, clip(2, 10_240, 10_240, 2_048, 2)]);
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(ClipId(1)),
            b: Some(ClipId(2)),
            in_offset: Tick::from_audio_frames(2_048),
            out_offset: Tick::from_audio_frames(2_048),
            kind: TransitionKind::Crossfade,
            audio: Default::default(),
            auto: vec![],
            invert: false,
        });
        p.validate().unwrap();
        p
    };
    let (plain, _) = render(&mk(vec![]), &[1, 2], 14_336);
    let (fxed, u) = render(&mk(vec![FxInstance::gain(0.0)]), &[1, 2], 14_336);
    assert_eq!(u, 0);
    if let Some(i) = (0..plain.len()).find(|&i| plain[i] != fxed[i]) {
        panic!("fx-path handle differs from plain at sample {i}");
    }
}

/// A reversed neighbor (plain): the handles continue the source's mirrored formula.
#[test]
fn t136_reversed_incoming_handle_mirrors_source() {
    let mut c2 = clip(2, 10_240, 10_240, 4_096, 2);
    c2.reversed = true;
    let mut p = project(vec![clip(1, 0, 10_240, 0, 1), c2]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick::from_audio_frames(2_048),
        out_offset: Tick::from_audio_frames(2_048),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let (out, underruns) = render(&p, &[1, 2], 14_336);
    assert_eq!(underruns, 0);
    let (w0, w1) = (8_192i64, 12_288i64);
    // reversed: src = source_in + (len − 1 − local); in the left handle local < 0
    // → material to the RIGHT of the used piece
    for f in [8_500i64, 10_239, 10_240, 11_000] {
        let local = f - 10_240;
        let b_src = 4_096 + (10_240 - 1 - local);
        let b_val = SyntheticSource::sample_at(AudioPattern::Ramp, b_src, 0);
        let a_val = SyntheticSource::sample_at(AudioPattern::Ramp, f, 0);
        let t = (f - w0) as f64 / (w1 - w0) as f64;
        let (wa, wb) = (((1.0 - t).max(0.0)).sqrt() as f32, (t.max(0.0)).sqrt() as f32);
        let expect = a_val * wa + b_val * wb;
        assert_eq!(out[(f as usize) * CH], expect, "frame {f}");
    }
}

/// Prefetch warms the handles: wanted covers the material beyond the body edges.
#[test]
fn t136_wanted_covers_handles() {
    let mut p = project(vec![clip(1, 0, 10_240, 0, 1), clip(2, 10_240, 10_240, 2_048, 2)]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick::from_audio_frames(2_048),
        out_offset: Tick::from_audio_frames(2_048),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);
    let wanted = r.wanted_audio_bursts(8_192..12_288);
    // A: body [8192,10240) + right handle [10240,12288) → src up to 12288
    let a = wanted.iter().find(|(m, _)| *m == MediaId(1)).unwrap();
    assert!(a.1.end * 1024 >= 12_288, "outgoing handle must be prefetched: {:?}", a.1);
    // B: left handle src from 2048−2048 = 0
    let b = wanted.iter().find(|(m, _)| *m == MediaId(2)).unwrap();
    assert_eq!(b.1.start, 0, "incoming handle must be prefetched: {:?}", b.1);
}

// ---- test 156: audio fade at the edge (R.25) --------------------------------------

/// Audio fade-in: the same equal-power weight as the crossfade, only facing the
/// empty space outside the window — the sound rises from silence (and symmetrically fades into it).
#[test]
fn t156_audio_fade_ramps_from_silence() {
    let mut p = project(vec![clip(1, 0, 4800, 0, 1)]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: None,
        b: Some(ClipId(1)),
        in_offset: Tick::ZERO,
        out_offset: Tick::from_audio_frames(2400), // fade-in over 2400 frames
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let (out, underruns) = render(&p, &[1], 4800);
    assert_eq!(underruns, 0);

    // dry reference of the same clip without a transition
    let dry = render(&project(vec![clip(1, 0, 4800, 0, 1)]), &[1], 4800).0;
    assert_eq!(out[0], 0.0, "at the start of the window — silence");
    for f in [600usize, 1200, 2399] {
        let w = (f as f64 / 2400.0).sqrt() as f32; // equal-power, as in the crossfade
        let expect = dry[f * CH] * w;
        assert!(
            (out[f * CH] - expect).abs() < 1e-6,
            "frame {f}: expected weight {w}, got {} against {expect}",
            out[f * CH]
        );
    }
    for f in [2400usize, 4000] {
        assert_eq!(out[f * CH], dry[f * CH], "outside the fade window the clip sounds as is");
    }
}

/// Fade-out: toward the end of the clip the sound goes to silence.
#[test]
fn t156_audio_fade_out_ends_in_silence() {
    let mut p = project(vec![clip(1, 0, 4800, 0, 1)]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: None,
        in_offset: Tick::from_audio_frames(2400),
        out_offset: Tick::ZERO,
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let (out, _) = render(&p, &[1], 4800);
    let dry = render(&project(vec![clip(1, 0, 4800, 0, 1)]), &[1], 4800).0;
    assert_eq!(out[100 * CH], dry[100 * CH], "before the window — no changes");
    let f = 3600usize; // the middle of the window [2400, 4800)
    let w = (1.0 - (f - 2400) as f64 / 2400.0).sqrt() as f32;
    assert!((out[f * CH] - dry[f * CH] * w).abs() < 1e-6);
    assert!(out[4799 * CH].abs() < 1e-3, "near the end of the window — almost silence");
}

/// Test 226 (R.33 C4): the curve of the audio crossfade is chosen by the user and
/// reaches the samples. Equal-power must stay the OLD formula
/// (GOLDEN_AUDIO), the other three are computed by their own.
#[test]
fn t226_audio_crossfade_curve_is_applied() {
    let (w0, w1) = (8_192i64, 12_288i64);
    let render_with = |curve: AudioCurve| {
        let mut p = project(vec![clip(1, 0, 10_240, 0, 1), clip(2, 10_240, 10_240, 50_000, 2)]);
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(ClipId(1)),
            b: Some(ClipId(2)),
            in_offset: Tick::from_audio_frames(2_048),
            out_offset: Tick::from_audio_frames(2_048),
            kind: TransitionKind::Crossfade,
            audio: curve,
            auto: vec![],
            invert: false,
        });
        p.validate().unwrap();
        let (out, underruns) = render(&p, &[1, 2], 14_336);
        assert_eq!(underruns, 0, "{curve:?}: the handles must not starve");
        out
    };
    let expect = |curve: AudioCurve, f: i64| {
        let a_val = SyntheticSource::sample_at(AudioPattern::Ramp, f, 0);
        let b_val = SyntheticSource::sample_at(AudioPattern::Ramp, 50_000 + (f - 10_240), 0);
        let t = ((f - w0) as f64 / (w1 - w0) as f64).clamp(0.0, 1.0);
        a_val * (curve.rise(1.0 - t) as f32) + b_val * (curve.rise(t) as f32)
    };
    let probes = [8_192i64, 9_000, 10_240, 11_000, 12_287];
    for curve in
        [AudioCurve::EqualPower, AudioCurve::Linear, AudioCurve::SCurve, AudioCurve::Exponential]
    {
        let out = render_with(curve);
        for f in probes {
            let got = out[f as usize * CH];
            let want = expect(curve, f);
            assert!((got - want).abs() < 1e-6, "{curve:?} @{f}: {got} vs {want}");
        }
    }
    // equal-power — the old formula BYTE-for-BYTE: otherwise GOLDEN would shift
    let eq = render_with(AudioCurve::EqualPower);
    let mut old = eq.clone();
    for (f, s) in old.chunks_exact_mut(CH).enumerate() {
        let f = f as i64;
        if f < w0 || f >= w1 {
            continue;
        }
        let t = (f - w0) as f64 / (w1 - w0) as f64;
        let a_val = SyntheticSource::sample_at(AudioPattern::Ramp, f, 0);
        let b_val = SyntheticSource::sample_at(AudioPattern::Ramp, 50_000 + (f - 10_240), 0);
        s[0] = a_val * (((1.0 - t).max(0.0)).sqrt() as f32) + b_val * ((t.max(0.0)).sqrt() as f32);
    }
    for (i, (g, w)) in eq.chunks_exact(CH).zip(old.chunks_exact(CH)).enumerate() {
        assert_eq!(g[0].to_bits(), w[0].to_bits(), "equal-power shifted at frame {i}");
    }
    // the curves must DIFFER: an identical output = the field did not arrive
    let lin = render_with(AudioCurve::Linear);
    let mid = 10_240usize * CH;
    assert!(
        (eq[mid] - lin[mid]).abs() > 1e-4,
        "a linear curve must sound different from equal-power: {} vs {}",
        eq[mid],
        lin[mid]
    );
}

/// Test 463 (item 399, R.51): an explicit transition on an OVERLAP applies its own
/// curve instead of the built-in equal-power overlap; there is no double attenuation;
/// without a transition the path is byte-for-byte the old one (GOLDEN intact — see golden_export).
#[test]
fn t463_overlap_transition_applies_its_curve() {
    // two clips overlapping in a “staircase”: [0..2s) and [1s..3s), overlap 1s
    let sec = 48_000i64;
    let clips = || vec![clip(1, 0, 2 * sec, 0, 1), clip(2, sec, 2 * sec, 0, 2)];
    let total = (3 * sec) as usize;

    // without a transition — the old √-law of the overlap
    let plain = render(&project(clips()), &[1, 2], total).0;

    // with a Linear transition — in the middle of the window the weight of each side is 0.5
    let mut p = project(clips());
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick::ZERO,
        out_offset: Tick::ZERO,
        kind: TransitionKind::Crossfade,
        audio: AudioCurve::Linear,
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let linear = render(&p, &[1, 2], total).0;
    assert_ne!(plain, linear, "a linear curve must sound different from the √ overlap");

    // the middle of the window (1.5 s): the ramp of both sources is deterministic,
    // direct weight computation: linear = 0.5·a + 0.5·b, √-law = 0.707·(a+b)
    let mid = (3 * sec / 2) as usize;
    let sample = |buf: &[f32]| buf[mid * CH];
    let (a_alone, _) = render(&project(vec![clips()[0].clone()]), &[1], total);
    let (b_alone, _) = render(&project(vec![clips()[1].clone()]), &[2], total);
    let (sa, sb) = (sample(&a_alone), sample(&b_alone));
    assert!(
        (sample(&linear) - (0.5 * sa + 0.5 * sb)).abs() < 1e-6,
        "the middle of a Linear window is exactly the half-sum of the sides: {} vs {}",
        sample(&linear),
        0.5 * sa + 0.5 * sb
    );
    let ep = 0.5f32.sqrt();
    assert!(
        (sample(&plain) - (ep * sa + ep * sb)).abs() < 1e-6,
        "an overlap without a transition — the prior √ law"
    );

    // an equal-power transition on an overlap coincides with the same WITHOUT a transition:
    // the formulas are the same (√t / √(1−t) on the same window) — adding a transition
    // with the default curve does not change the sound
    let mut p = project(clips());
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick::ZERO,
        out_offset: Tick::ZERO,
        kind: TransitionKind::Crossfade,
        audio: AudioCurve::EqualPower,
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let ep_render = render(&p, &[1, 2], total).0;
    assert_eq!(plain, ep_render, "an equal-power transition on an overlay sounds like an overlap");
}
