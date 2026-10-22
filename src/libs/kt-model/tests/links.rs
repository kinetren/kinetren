//! Tests 702-704 (R.61): a link — a value drives another property.
//!
//! Owner’s decision: ANY event of the project can be addressed. A constraint derived from the
//! invariants: a link does not cross the reel boundary — outward, because the reel would stop being
//! self-sufficient and the law E would break; inward, because public parameters of the composition
//! already exist for that.

use kt_model::*;
use kt_time::Tick;

const SEC: i64 = 90_000;
const FRAME: i64 = SEC / 30;

fn clip(id: u64, start: i64, dur: i64, src_in: i64) -> ClipEvent {
    ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(src_in),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::Blur { radius: 8, radius_y: 0 }],
        fx_bypass: vec![false],
        fx_auto: vec![Vec::new()],
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

fn base() -> Project {
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
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.ensure_tracks(LaneKind::Video, 1);
    p
}

/// a leading curve: linearly from `a` to `b` through the material
fn driver(param: &str, a: f64, b: f64, from: i64, to: i64) -> ParamCurve {
    ParamCurve {
        param: param.into(),
        keys: vec![
            AutoKey::shaped(Tick(from), a, SegShape::Linear),
            AutoKey::shaped(Tick(to), b, SegShape::Linear),
        ],
        source: CurveSource::Keys,
    }
}

#[test]
fn t702_a_link_reads_inside_the_event_and_across_events() {
    let mut p = base();
    // the leading event: the radius goes 0 → 30 along its own material
    let mut lead = clip(1, 0, 3 * SEC, 0);
    lead.fx_auto[0].push(driver("radius", 0.0, 30.0, 0, 3 * SEC));
    // the led event stands LATER and with a different entry point: they have no common
    // material time, the denominator is the TIMELINE
    let follow = clip(2, 5 * SEC, 2 * SEC, 7 * SEC);
    p.clips.push(lead.clone());
    p.clips.push(follow.clone());

    let me = LinkCtx::of(&follow, FRAME);
    let path = LinkPath::parse("1/fx0/radius").expect("address parses");

    // the middle of the led event = 6 s of the timeline; for the leader that is 6 s of its material,
    // i.e. just past its end (3 s) → the edge value
    let at_mid = kt_model::clip_material_at(
        follow.source_in,
        follow.start,
        follow.duration,
        follow.reversed,
        FRAME,
        Tick(6 * SEC),
    );
    let v = read_link(&p, &me, &path, at_mid).expect("link reads");
    assert!((v - 30.0).abs() < 1e-9, "past the source body the edge value is taken: {v}");

    // and if the led event stands INSIDE the leader, the values match on the timeline
    let inside = clip(3, SEC, SEC, 0);
    let me2 = LinkCtx::of(&inside, FRAME);
    let mut q = base();
    q.clips.push(lead.clone());
    q.clips.push(inside.clone());
    let at = kt_model::clip_material_at(
        inside.source_in,
        inside.start,
        inside.duration,
        inside.reversed,
        FRAME,
        Tick(3 * SEC / 2),
    );
    let got = read_link(&q, &me2, &path, at).expect("link reads");
    let want = kt_model::curve_value(&lead.fx_auto[0][0].keys, Tick(3 * SEC / 2));
    assert!((got - want).abs() < 1e-9, "between events the denominator is not timeline: {got} vs {want}");

    // --- INSIDE one event no conversion is needed: the domain is common ----------
    let mut solo = clip(4, 2 * SEC, 2 * SEC, SEC);
    solo.fx = vec![FxInstance::Blur { radius: 8, radius_y: 0 }, FxInstance::gain(0.0)];
    solo.fx_bypass = vec![false, false];
    solo.fx_auto = vec![vec![driver("radius", 4.0, 12.0, SEC, 3 * SEC)], Vec::new()];
    let mut r = base();
    r.clips.push(solo.clone());
    let me3 = LinkCtx::of(&solo, FRAME);
    let own = LinkPath::parse("self/fx0/radius").expect("own address");
    assert_eq!(own.clip, None, "“self” does not require knowing its own number");
    let at = Tick(2 * SEC);
    let got = read_link(&r, &me3, &own, at).expect("own link reads");
    let want = kt_model::curve_value(&solo.fx_auto[0][0].keys, at);
    assert!((got - want).abs() < 1e-9, "inside an event the conversion is superfluous: {got} vs {want}");

    // --- REVERSE and TRIM of the leader are accounted for by the same pair of functions ---
    let mut rev = lead.clone();
    rev.reversed = true;
    let mut t = base();
    t.clips.push(rev);
    t.clips.push(inside);
    let mirrored = read_link(&t, &me2, &path, at).expect("reads");
    assert!(mirrored.is_finite(), "a mirrored source does not read at all");

    // a round address: text → parse → text
    for txt in ["1/fx0/radius", "self/gen/size", "12/clip/opacity", "3/fx7/amount"] {
        let pp = LinkPath::parse(txt).unwrap_or_else(|| panic!("{txt} did not parse"));
        assert_eq!(pp.to_text(), txt, "address did not survive a round trip");
    }
    for bad in ["", "1/fx0", "1/fx0/a/b", "1/wat/x", "x/fx0/a"] {
        assert!(LinkPath::parse(bad).is_none(), "{bad:?} parsed as an address");
    }
}

#[test]
fn t702b_a_link_never_crosses_the_reel_boundary() {
    let mut p = base();
    let mut seq = Sequence::new(MediaId(100), "Reel");
    let mut inner = clip(10, 0, 2 * SEC, 0);
    // an event INSIDE a reel tries to behave as an event of the ROOT
    inner.fx_auto[0].push(ParamCurve {
        param: "radius".into(),
        keys: Vec::new(),
        source: CurveSource::Link { path: "1/fx0/radius".into(), expr: None, stops: Vec::new() },
    });
    seq.clips.push(inner);
    seq.ensure_tracks(LaneKind::Video, 1);
    p.sequences.push(seq);
    p.media.push(MediaRef::new(MediaId(2), "synth:2"));
    let mut root = clip(1, 0, 3 * SEC, 0);
    root.fx_auto[0].push(driver("radius", 0.0, 30.0, 0, 3 * SEC));
    p.clips.push(root);

    let err = p.validate().expect_err("a link across a reel boundary must be a rejection");
    let text = err.to_string();
    assert!(
        text.contains("outside its reel") || text.contains("self-contained"),
        "the rejection does not explain why it is not allowed: {text}"
    );
    assert!(
        text.contains("composition parameter") || text.contains("publish"),
        "the rejection does not name the legal way (public parameters): {text}"
    );

    // and a link INSIDE a reel is legitimate
    let mut q = base();
    let mut seq = Sequence::new(MediaId(100), "Reel");
    let mut lead = clip(10, 0, 3 * SEC, 0);
    lead.fx_auto[0].push(driver("radius", 0.0, 30.0, 0, 3 * SEC));
    let mut follow = clip(11, 0, 3 * SEC, 0);
    follow.lane = 0;
    follow.fx_auto[0].push(ParamCurve {
        param: "radius_y".into(),
        keys: Vec::new(),
        source: CurveSource::Link {
            path: "10/fx0/radius".into(),
            expr: Some("src / 2".into()),
            stops: Vec::new(),
        },
    });
    seq.clips.push(lead);
    seq.clips.push(follow);
    seq.ensure_tracks(LaneKind::Video, 1);
    q.sequences.push(seq);
    q.media.push(MediaRef::new(MediaId(2), "synth:2"));
    q.validate().expect("a link within one reel is legal");
}

#[test]
fn t703_cycles_are_refused_with_the_path() {
    let link = |to: &str| ParamCurve {
        param: "radius".into(),
        keys: Vec::new(),
        source: CurveSource::Link { path: to.into(), expr: None, stops: Vec::new() },
    };

    // --- a pair leading each other and read by NOBODY ----------------------
    let mut p = base();
    let mut a = clip(1, 0, SEC, 0);
    let mut b = clip(2, 0, SEC, 0);
    a.fx_auto[0].push(link("2/fx0/radius"));
    b.fx_auto[0].push(link("1/fx0/radius"));
    p.clips.push(a);
    p.clips.push(b);
    let err = p.validate().expect_err("a pair in a cycle must be a rejection");
    let text = err.to_string();
    assert!(text.contains("->"), "cycle path was not printed: {text}");
    assert!(text.contains("circle") || text.contains("drive"), "{text}");

    // --- a triple through different events ----------------------------------
    let mut q = base();
    let mut a = clip(1, 0, SEC, 0);
    let mut b = clip(2, 0, SEC, 0);
    let mut c = clip(3, 0, SEC, 0);
    a.fx_auto[0].push(link("2/fx0/radius"));
    b.fx_auto[0].push(link("3/fx0/radius"));
    c.fx_auto[0].push(link("1/fx0/radius"));
    q.clips.push(a);
    q.clips.push(b);
    q.clips.push(c);
    assert!(q.validate().is_err(), "a cycle of length three passed");

    // --- a property leading ITSELF ------------------------------------------
    let mut r = base();
    let mut a = clip(1, 0, SEC, 0);
    a.fx_auto[0].push(link("1/fx0/radius"));
    r.clips.push(a);
    assert!(r.validate().is_err(), "self-reference passed");
    let mut r2 = base();
    let mut a2 = clip(1, 0, SEC, 0);
    a2.fx_auto[0].push(link("self/fx0/radius"));
    r2.clips.push(a2);
    assert!(r2.validate().is_err(), "self-reference via “self” passed");

    // --- a CHAIN without a cycle is legitimate ------------------------------
    let mut s = base();
    let mut a = clip(1, 0, SEC, 0);
    let mut b = clip(2, 0, SEC, 0);
    let mut c = clip(3, 0, SEC, 0);
    a.fx_auto[0].push(driver("radius", 0.0, 10.0, 0, SEC));
    b.fx_auto[0].push(link("1/fx0/radius"));
    c.fx_auto[0].push(link("2/fx0/radius"));
    s.clips.push(a);
    s.clips.push(b);
    s.clips.push(c);
    s.validate().expect("chain A → B → C without a cycle is legal");

    // --- a project WITHOUT links is valid as before -------------------------
    let mut plain = base();
    plain.clips.push(clip(1, 0, SEC, 0));
    plain.validate().expect("a project without links is valid");
}

#[test]
fn t704_non_numeric_targets_follow_thresholds_and_snapshots() {
    let snap = |at: f64, v: ParamValue| LinkStop { at, value: v };
    let stops = vec![
        snap(0.0, ParamValue::Text("bottom".into())),
        snap(10.0, ParamValue::Text("middle".into())),
        snap(20.0, ParamValue::Text("top".into())),
    ];

    // between the marks the PREVIOUS snapshot holds:
    // the halves between the variants do not exist
    let text = |v: f64| match stop_value(&stops, v) {
        Some(ParamValue::Text(s)) => s.clone(),
        other => panic!("not text: {other:?}"),
    };
    assert_eq!(text(0.0), "bottom", "at the mark itself its own snapshot applies");
    assert_eq!(text(9.99), "bottom", "between marks the previous one holds");
    assert_eq!(text(10.0), "middle");
    assert_eq!(text(19.0), "middle");
    assert_eq!(text(20.0), "top");
    assert_eq!(text(1e9), "top", "above the last — the last");
    assert_eq!(text(-5.0), "bottom", "below the first — the FIRST snapshot, not empty");

    // a flag — the same mechanism with two marks, without jitter on equality
    let flag = vec![snap(0.0, ParamValue::Flag(false)), snap(0.5, ParamValue::Flag(true))];
    let on = |v: f64| matches!(stop_value(&flag, v), Some(ParamValue::Flag(true)));
    assert!(!on(0.49));
    assert!(on(0.5), "at the threshold itself the switch has already happened");
    assert!(on(0.51));
    // equality is treated to ONE side and does not jitter
    for _ in 0..8 {
        assert!(on(0.5), "the threshold jitters at equality");
    }

    // an empty set of marks is “nothing to substitute”, not a panic
    assert!(stop_value(&[], 1.0).is_none());

    // the marks travel to the file together with the link
    let src = CurveSource::Link { path: "1/gen/text".into(), expr: None, stops: stops.clone() };
    let j = serde_json::to_string(&src).expect("is serialized");
    let back: CurveSource = serde_json::from_str(&j).expect("reads");
    assert_eq!(back, src, "snapshots did not survive a round trip");
    // and a link WITHOUT snapshots does not write them to the file
    let bare = CurveSource::Link { path: "1/fx0/radius".into(), expr: None, stops: Vec::new() };
    let j = serde_json::to_string(&bare).expect("is serialized");
    assert!(!j.contains("stops"), "empty snapshots leaked into the file: {j}");
}

/// Test 705 (R.61): the resolution of derived sources costs ZERO when there are none, and
/// does not burden the neighbours
/// when there are.
///
/// Owner’s requirement: “if the project is simple without tricks, it must not lag at all,
/// since plugins/curves are not applied”. For sound this rule has been fulfilled from the very
/// start; here it is checked by a test, not by a promise.
#[test]
fn t705_derived_sources_cost_nothing_when_unused() {
    // --- NO derived sources — the reel BORROWS -----------------------------------
    let mut plain = Vec::new();
    for i in 1..=8 {
        let mut c = clip(i, (i as i64 - 1) * SEC, SEC, 0);
        c.fx_auto[0].push(driver("radius", 1.0, 9.0, 0, SEC));
        plain.push(c);
    }
    let got = resolve_derived(&plain, FRAME);
    assert!(
        matches!(got, std::borrow::Cow::Borrowed(_)),
        "a reel without links and formulas was copied — there must be no graph at all"
    );
    assert_eq!(&got[..], &plain[..], "the borrowed reel changed");

    // --- ONE link among many: neighbours byte-for-byte as before -----------------
    let mut mixed = plain.clone();
    mixed[7].fx_auto[0].push(ParamCurve {
        param: "radius_y".into(),
        keys: Vec::new(),
        source: CurveSource::Link {
            path: "1/fx0/radius".into(),
            expr: Some("src * 2".into()),
            stops: Vec::new(),
        },
    });
    let out = resolve_derived(&mixed, FRAME);
    assert!(matches!(out, std::borrow::Cow::Owned(_)), "the link did not resolve at all");
    for i in 0..7 {
        assert_eq!(
            out[i], plain[i],
            "event {i} changed due to a link on a NEIGHBOR — the price drifted along the edit"
        );
    }

    // --- the link resolved into ordinary keys -----------------------------------
    let done = &out[7].fx_auto[0][1];
    assert!(done.source.is_keys(), "source remained raw: {:?}", done.source);
    assert!(!done.keys.is_empty(), "no keyframes after resolution");
    assert!(
        done.keys.iter().all(|k| k.shape == SegShape::Linear),
        "a derived curve must be piecewise linear: it is sampled"
    );
    // and the values are exactly twice the leader
    let lead = &plain[0].fx_auto[0][0];
    for k in &done.keys {
        let tl = kt_model::clip_timeline_at(
            plain[7].source_in,
            plain[7].start,
            plain[7].duration,
            false,
            FRAME,
            k.t,
        );
        let tl = Tick(tl.0.clamp(plain[0].start.0, plain[0].start.0 + plain[0].duration.0));
        let mat = kt_model::clip_material_at(
            plain[0].source_in,
            plain[0].start,
            plain[0].duration,
            false,
            FRAME,
            tl,
        );
        let want = kt_model::curve_value(&lead.keys, mat) * 2.0;
        assert!((k.v - want).abs() < 1e-9, "the recalculation expression was not applied: {} vs {want}", k.v);
    }

    // --- a CHAIN A → B → C is resolved entirely -----------------------------------
    let mut chain = vec![clip(1, 0, SEC, 0), clip(2, 0, SEC, 0), clip(3, 0, SEC, 0)];
    chain[0].fx_auto[0].push(driver("radius", 2.0, 2.0, 0, SEC));
    chain[1].fx_auto[0].push(ParamCurve {
        param: "radius".into(),
        keys: Vec::new(),
        source: CurveSource::Link {
            path: "1/fx0/radius".into(),
            expr: Some("src + 1".into()),
            stops: Vec::new(),
        },
    });
    chain[2].fx_auto[0].push(ParamCurve {
        param: "radius".into(),
        keys: Vec::new(),
        source: CurveSource::Link {
            path: "2/fx0/radius".into(),
            expr: Some("src + 1".into()),
            stops: Vec::new(),
        },
    });
    let out = resolve_derived(&chain, FRAME);
    for (i, want) in [(1usize, 3.0), (2usize, 4.0)] {
        let c = &out[i].fx_auto[0][0];
        assert!(c.source.is_keys(), "link {i} of the chain remained raw");
        let v = c.keys[0].v;
        assert!(
            (v - want).abs() < 1e-9,
            "link {i} resolved before its leader: {v}, expected {want} (order is not topological)"
        );
    }
}

/// Test 707 (R.61): a formula — a source of a value, a curve without keys.
///
/// Owner: “the possibility of setting an automation curve by typing a formula
/// (without clicking the mouse)”. It is resolved by the same code as a link: the difference is
/// only that it has no address, but has a time.
#[test]
fn t707_a_formula_is_a_curve_without_keys() {
    let formula = |expr: &str| ParamCurve {
        param: "radius".into(),
        keys: Vec::new(),
        source: CurveSource::Formula { expr: expr.into() },
    };

    // --- a formula FROM TIME ------------------------------------------------------
    let mut c = clip(1, 0, 2 * SEC, 0);
    c.fx_auto[0].push(formula("t * 10"));
    let only = vec![c.clone()];
    let out = resolve_derived(&only, FRAME);
    let got = &out[0].fx_auto[0][0];
    assert!(got.source.is_keys(), "formula did not resolve: {:?}", got.source);
    assert!(got.keys.len() > 30, "the frame grid gave too few points: {}", got.keys.len());
    // an independent oracle: t in SECONDS from the start of the event’s material
    for k in &got.keys {
        let t_s = (k.t.0 - c.source_in.0) as f64 / 90_000.0;
        assert!(
            (k.v - t_s * 10.0).abs() < 1e-9,
            "formula evaluated wrongly: at {:?} gave {}, expected {}",
            k.t,
            k.v,
            t_s * 10.0
        );
    }

    // --- a formula READING another property goes by the same address ------
    let mut lead = clip(1, 0, 2 * SEC, 0);
    lead.fx_auto[0].push(driver("radius", 4.0, 4.0, 0, 2 * SEC));
    let mut follow = clip(2, 0, 2 * SEC, 0);
    follow.fx_auto[0].push(ParamCurve {
        param: "radius_y".into(),
        keys: Vec::new(),
        source: CurveSource::Link {
            path: "1/fx0/radius".into(),
            expr: Some("src * src".into()),
            stops: Vec::new(),
        },
    });
    let pair = vec![lead, follow];
    let out = resolve_derived(&pair, FRAME);
    let sq = &out[1].fx_auto[0][0];
    assert!(sq.source.is_keys(), "link with an expression did not resolve");
    assert!(
        (sq.keys[0].v - 16.0).abs() < 1e-9,
        "expression over the leader was not applied: {}",
        sq.keys[0].v
    );

    // --- a formula on SPEED has the same rule as a non-straight shape --
    let mut p = base();
    let mut rate = clip(1, 0, SEC, 0);
    rate.fx = vec![FxInstance::slowed_reverb(1.0, 0.0, 85.0)];
    rate.fx_bypass = vec![false];
    rate.fx_auto = vec![vec![formula("1 + t")]];
    rate.fx_auto[0][0].param = "rate".into();
    p.clips.push(rate);
    // a formula is legitimate in itself; after resolution the keys are STRAIGHT, and therefore
    // the speed rule is not violated —
    // the sampled curve is piecewise-linear by construction
    let out = resolve_derived(&p.clips, FRAME);
    assert!(
        out[0].fx_auto[0][0].keys.iter().all(|k| k.shape == SegShape::Linear),
        "a resolved formula on speed must be piecewise linear"
    );
    assert!(
        kt_model::check_fx_auto(ClipId(1), &out[0].fx_auto).is_ok(),
        "a resolved formula on speed was rejected"
    );
}
