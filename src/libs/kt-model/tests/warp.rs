//! Test 696 (R.61): the segment shape of a warp speed is ONLY STRAIGHT.
//!
//! The closed form of the inverse stretch map sits on a piecewise-linear
//! profile: the integral of the third order would be of the fourth degree, and a
//! zigzag is not expressible at all. The rule existed even before R.61 (the former
//! boolean smoothing on `rate` was dropped silently) — the new shape must not
//! be dropped silently but REJECTED: a silent drop would show one thing in the
//! editor and another in the sound.

use kt_model::*;
use kt_time::Tick;

const LEN: Tick = Tick(90_000);

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
    let straight = vec![
        AutoKey::shaped(Tick(0), 0.5, SegShape::Linear),
        AutoKey::shaped(LEN, 2.0, SegShape::Linear),
    ];

    // a straight line is legal
    assert!(
        check_fx_auto(ClipId(1), &clip_with(straight.clone()).fx_auto).is_ok(),
        "piecewise-linear speed must pass"
    );

    // any other shape — REJECTION, and it is named as such
    for shape in [
        SegShape::DEFAULT,
        SegShape::Hold,
        SegShape::EaseIn { tension: 1.0 },
        SegShape::EaseOut { tension: 1.0 },
        SegShape::Zigzag { periods: 3.0, amplitude: 0.2, profile: ZigProfile::Triangle },
    ] {
        let keys = vec![AutoKey::shaped(Tick(0), 0.5, shape), AutoKey::shaped(LEN, 2.0, shape)];
        let err = check_fx_auto(ClipId(1), &clip_with(keys).fx_auto).unwrap_err();
        assert!(
            matches!(err, ProjectError::RateShapeMustBeLinear(_)),
            "shape {shape:?} on speed passed silently or was rejected for the wrong reason: {err:?}"
        );
        // the rejection text names the REASON, not “bad curve”
        let text = err.to_string();
        assert!(
            text.contains("linear") && text.contains("warp"),
            "the rejection does not explain why exactly straight: {text}"
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

    // and the warp MAP computes the same numbers along a straight line as before:
    // the speed profile does not see the shape
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
