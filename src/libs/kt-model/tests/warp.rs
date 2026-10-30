//! Test 696 (R.61) — INVERTED by the owner's decision (2026-10-28, slowed-reverb-is-an-event:
//! "yes, invert t696, t696b and t238"). It held "the segment shape of a warp speed is ONLY
//! STRAIGHT": the closed form of the inverse stretch map sits on a piecewise-linear profile. That
//! reason constrains the PROFILE, not the curve: `rate_profile_keys` samples a shaped speed
//! curve into straight points, so every shape is accepted and the map follows it (t913 checks
//! the numbers against an independent integral). The owner: "childish excuses".

use kt_model::*;
use kt_time::Tick;

const LEN: Tick = Tick(90_000);

fn settings30() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 1920,
        height: 1080,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

fn clip_with(rate_keys: Vec<AutoKey>) -> ClipEvent {
    ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick::ZERO,
        duration: LEN,
        source_in: Tick::ZERO,
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::slowed_reverb(0.8, 0.0, 85.0)],
        fx_bypass: vec![false],
        fx_auto: vec![vec![ParamCurve {
            param: "rate".into(),
            keys: rate_keys,
            source: CurveSource::Keys,
        }]],
        opacity: 1.0,
        fx_off: false,
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
    }
}

#[test]
fn t696_speed_curve_takes_the_linear_shape_only() {
    kt_plugins_builtin::install();
    let straight = vec![
        AutoKey::shaped(Tick(0), 0.5, SegShape::Linear),
        AutoKey::shaped(LEN, 2.0, SegShape::Linear),
    ];

    // a straight line is legal
    assert!(
        check_fx_auto(ClipId(1), &clip_with(straight.clone()).fx_auto).is_ok(),
        "piecewise-linear speed must pass"
    );

    // every other shape is legal too — the former refusal is gone
    for shape in [
        SegShape::DEFAULT,
        SegShape::Hold,
        SegShape::EaseIn { tension: 1.0 },
        SegShape::EaseOut { tension: 1.0 },
        SegShape::Zigzag { periods: 3.0, amplitude: 0.2, profile: ZigProfile::Triangle },
    ] {
        let keys = vec![AutoKey::shaped(Tick(0), 0.5, shape), AutoKey::shaped(LEN, 2.0, shape)];
        assert!(
            check_fx_auto(ClipId(1), &clip_with(keys).fx_auto).is_ok(),
            "shape {shape:?} on speed was refused"
        );
    }

    // a shape of a DIFFERENT parameter of the same slot is legal: the rule is about speed,
    // not about the whole slot
    let mut c = clip_with(straight);
    c.fx_auto[0].push(ParamCurve {
        param: "mix".into(),
        keys: vec![
            AutoKey::shaped(
                Tick(0),
                0.0,
                SegShape::Zigzag { periods: 4.0, amplitude: 0.1, profile: ZigProfile::Sine },
            ),
            AutoKey::at(LEN, 1.0),
        ],
        source: CurveSource::Keys,
    });
    assert!(
        check_fx_auto(ClipId(1), &c.fx_auto).is_ok(),
        "zigzag on mix rejected — the rule drifted from speed to the whole slot"
    );

    // and the warp MAP computes the same numbers along a STRAIGHT speed curve as before
    let len = LEN.to_audio_frames();
    let prof = AutoProfiles::from_curves(&c.fx_auto, 0, len);
    let at = |s: f64| prof.value(0, "rate", 0.8, s);
    assert!((at(0.0) - 0.5).abs() < 1e-9, "the start of the profile drifted: {}", at(0.0));
    assert!((at(1.0) - 2.0).abs() < 1e-9, "the end of the profile drifted: {}", at(1.0));
    assert!(
        (at(0.5) - 1.25).abs() < 1e-9,
        "the middle of the speed profile must be LINEAR: {}",
        at(0.5)
    );
}

/// Test 913 (slowed-reverb-is-an-event, ktprompts 26.10.18 item 2): speed takes EVERY shape.
///
/// The owner: "slowed does not let me put generated curves on — childish excuses". The excuse
/// was the closed form of the warp map, which stands on a piecewise-linear PROFILE; the curve
/// itself may have any shape — the profile is sampled from it by one function. Here every
/// shape of the closed set, a segment formula and a formula layer on the speed of a 10 s event:
/// the command accepts them, the consumed material agrees with an independent numeric integral
/// of the shaped curve, the raw event (the interface's path) and the resolved event (the
/// render's path) read the same profile bit for bit, and a straight curve keeps its old profile.
#[test]
fn t913_speed_takes_every_shape_and_the_map_follows_it() {
    kt_plugins_builtin::install();
    const LEN10: Tick = Tick(900_000);
    let ten = |rate_keys: Vec<AutoKey>, source: CurveSource| -> ClipEvent {
        let mut c = clip_with(rate_keys);
        c.duration = LEN10;
        c.fx_auto[0][0].source = source;
        c
    };
    // the independent oracle: the speed at output time τ is the curve's value at the material
    // tick the interface writes the key at (`source_in + τ`), integrated by the midpoint rule on
    // a 1-tick step — no profile, no sampling of ours
    let oracle_used = |c: &ClipEvent, rate: &dyn Fn(Tick) -> f64| -> f64 {
        let mut sum = 0.0;
        for t in 0..c.duration.0 {
            sum += rate(Tick(t)).clamp(RATE_CLAMP.0, RATE_CLAMP.1);
        }
        sum
    };
    let audio_frame = 90_000.0 / 48_000.0;
    let mut problems: Vec<String> = Vec::new();

    let formula = ExprId::intern("a + (b - a) * s * s");
    let shapes = [
        ("smooth", SegShape::DEFAULT),
        ("hold", SegShape::Hold),
        ("ease in", SegShape::EaseIn { tension: 1.0 }),
        ("ease out", SegShape::EaseOut { tension: 1.0 }),
        ("zigzag sine", SegShape::Zigzag { periods: 3.0, amplitude: 0.2, profile: ZigProfile::Sine }),
        ("zigzag triangle", SegShape::Zigzag { periods: 3.0, amplitude: 0.2, profile: ZigProfile::Triangle }),
        ("segment formula", SegShape::Formula { expr: formula }),
    ];
    for (name, shape) in shapes {
        let keys = vec![AutoKey::shaped(Tick(0), 0.5, shape), AutoKey::shaped(LEN10, 1.5, SegShape::Linear)];
        let c = ten(keys.clone(), CurveSource::Keys);
        // the command takes it
        let mut p = Project::new(settings30());
        p.media.push(MediaRef::new(MediaId(1), "synth:1"));
        p.clips.push(c.clone());
        let mut m = TimelineModel::new(p);
        if let Err(e) = m.apply(Command::SetFxAuto { id: ClipId(1), auto: c.fx_auto.clone() }) {
            problems.push(format!("{name}: the speed curve was refused: {e}"));
            continue;
        }
        let warp = clip_warp(&c).expect("a slowed event warps");
        let want = oracle_used(&c, &|t| curve_value(&keys, t));
        if (warp.used() as f64 - want).abs() > audio_frame {
            problems.push(format!(
                "{name}: consumed {} ticks, the integral of the shaped curve is {want:.1}",
                warp.used()
            ));
        }
        // the inverse map returns the output time of every consumed point
        for q in [0.25, 0.5, 0.75] {
            let a = consumed_abs(warp.base(), warp.prof(), q);
            let back = output_time_for_consumption(warp.base(), warp.prof(), a);
            if (back - q).abs() * LEN10.0 as f64 > audio_frame {
                problems.push(format!("{name}: the inverse map of {q} gave {back}"));
            }
        }
        // the raw event and the resolved event read the same profile
        let resolved = resolve_derived(std::slice::from_ref(&c), 3_000);
        let r = clip_warp(&resolved[0]).expect("the resolved event warps");
        if r.prof() != warp.prof() || r.used() != warp.used() {
            problems.push(format!("{name}: the raw and the resolved event read different profiles"));
        }
        // determinism
        if clip_warp(&c) != Some(warp.clone()) {
            problems.push(format!("{name}: two readings of the map differ"));
        }
    }

    // a formula LAYER on speed: 1 + 0.5·sin(6·t), t in seconds from the material entry
    let layer = ten(Vec::new(), CurveSource::Formula { expr: "1 + 0.5 * sin(t * 6)".into() });
    let mut p = Project::new(settings30());
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.clips.push(layer.clone());
    let mut m = TimelineModel::new(p);
    match m.apply(Command::SetFxAuto { id: ClipId(1), auto: layer.fx_auto.clone() }) {
        Err(e) => problems.push(format!("formula layer: refused: {e}")),
        Ok(_) => match clip_warp(&layer) {
            None => problems.push("formula layer: the raw event has no warp map".into()),
            Some(w) => {
                let want =
                    oracle_used(&layer, &|t| 1.0 + 0.5 * (t.0 as f64 / 90_000.0 * 6.0).sin());
                if (w.used() as f64 - want).abs() > audio_frame {
                    problems.push(format!(
                        "formula layer: the raw event consumes {} ticks, the formula integrates to {want:.1} \
                         (the interface's map ignores the layer)",
                        w.used()
                    ));
                }
                let resolved = resolve_derived(std::slice::from_ref(&layer), 3_000);
                if clip_warp(&resolved[0]).map(|r| r.prof().to_vec()) != Some(w.prof().to_vec()) {
                    problems.push("formula layer: the raw and the resolved event read different profiles".into());
                }
            }
        },
    }

    // a straight curve keeps its former profile bit for bit: the key points, nothing else
    let straight = ten(
        vec![
            AutoKey::shaped(Tick(0), 0.5, SegShape::Linear),
            AutoKey::shaped(Tick(300_000), 2.0, SegShape::Linear),
            AutoKey::shaped(LEN10, 1.0, SegShape::Linear),
        ],
        CurveSource::Keys,
    );
    let w = clip_warp(&straight).unwrap();
    let len = LEN10.to_audio_frames() as f64;
    let expect: Vec<(f64, f64)> = straight.fx_auto[0][0]
        .keys
        .iter()
        .map(|k| (k.t.to_audio_frames() as f64 / len, k.v))
        .collect();
    if w.prof() != expect.as_slice() {
        problems.push(format!("a straight speed curve changed its profile: {:?}", w.prof()));
    }
    assert!(problems.is_empty(), "speed does not take every shape: {}", problems.join("; "));
}
