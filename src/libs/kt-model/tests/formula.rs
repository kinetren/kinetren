//! Test 701 (R.61): the mini-formula evaluator is closed, deterministic
//! and explains its failures.
//!
//! Owner’s decision (R.59): “Expressions are their own mini-formula, evaluated live on every frame;
//! there are no external interpreters.” From the determinism invariant
//! the rest follows: the operation set is closed, the language has no environment,
//! and a failure is NAMED — a silent zero would show a person a picture
//! that they would believe.

use kt_model::{Formula, FormulaErrorKind};
use std::collections::BTreeMap;

fn vars(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
}

#[test]
fn t701_formula_is_closed_deterministic_and_names_its_refusals() {
    let v = vars(&[("t", 2.0), ("x", 3.0), ("blur.radius", 8.0)]);

    // --- the SET of operations is computed, and computed the SAME way ---------------
    let cases: &[(&str, f64)] = &[
        ("1 + 2 * 3", 7.0),
        ("(1 + 2) * 3", 9.0),
        ("-x", -3.0),
        ("x - -x", 6.0),
        ("7 % 4", 3.0),
        ("t < x", 1.0),
        ("t > x", 0.0),
        ("t <= 2", 1.0),
        ("x != 3", 0.0),
        ("if(t < x, 10, 20)", 10.0),
        ("if(t > x, 10, 20)", 20.0),
        ("min(t, x)", 2.0),
        ("max(t, x)", 3.0),
        ("clamp(99, 0, 10)", 10.0),
        ("abs(0 - x)", 3.0),
        ("floor(2.7)", 2.0),
        ("ceil(2.1)", 3.0),
        ("round(2.5)", 3.0),
        ("sqrt(9)", 3.0),
        ("lerp(0, 10, 0.25)", 2.5),
        // a property address is an ordinary dotted name
        ("blur.radius / 2", 4.0),
    ];
    for (src, want) in cases {
        let f = Formula::parse(src).unwrap_or_else(|e| panic!("{src:?} did not parse: {e}"));
        let got = f.eval(&v).unwrap_or_else(|e| panic!("{src:?} did not evaluate: {e}"));
        assert!((got - want).abs() < 1e-12, "{src:?} gave {got}, expected {want}");
        // TWO runs, byte-for-byte
        assert_eq!(f.eval(&v).unwrap().to_bits(), got.to_bits(), "{src:?} is not deterministic");
    }

    // noise is deterministic and bounded, and depends on the WHOLE seed
    let n = Formula::parse("noise(t)").expect("noise parses");
    let a = n.eval(&v).expect("noise evaluates");
    assert_eq!(a.to_bits(), n.eval(&v).unwrap().to_bits(), "noise is not deterministic");
    assert!((-1.0..=1.0).contains(&a), "noise out of range: {a}");
    let b = n.eval(&vars(&[("t", 9.0)])).unwrap();
    assert_ne!(a.to_bits(), b.to_bits(), "noise does not depend on the seed");

    // --- FOUR failures, each with its OWN cause -------------------------------
    let unknown = Formula::parse("no_such + 1")
        .err()
        .or_else(|| Formula::parse("qqq").ok().and_then(|f| f.eval(&v).err()))
        .expect("an unknown name must be a rejection");
    assert!(
        matches!(unknown.kind, FormulaErrorKind::UnknownName(_) | FormulaErrorKind::BadChar(_)),
        "unknown name rejected for the wrong reason: {unknown:?}"
    );

    let div = Formula::parse("x / (t - 2)").expect("parses");
    let e = div.eval(&v).expect_err("division by zero must be a REJECTION, not infinity");
    assert_eq!(e.kind, FormulaErrorKind::DivZero, "{e:?}");

    let open = Formula::parse("(1 + 2").expect_err("an unclosed bracket must be a rejection");
    assert_eq!(open.kind, FormulaErrorKind::Unclosed, "{open:?}");

    // the language has NO environment on purpose: no clock, no randomness
    for src in ["now()", "now", "rand()", "time()"] {
        let bad = Formula::parse(src)
            .err()
            .or_else(|| Formula::parse(src).ok().and_then(|f| f.eval(&v).err()))
            .unwrap_or_else(|| panic!("{src:?} passed — the environment leaked into the language"));
        assert!(
            matches!(bad.kind, FormulaErrorKind::UnknownName(_)),
            "{src:?} rejected not as an unknown name: {bad:?}"
        );
    }

    // --- no failure gives a SILENT ZERO ---------------------------------------
    for src in ["1/0", "(2", "wat(1)", "min(1)"] {
        let r = Formula::parse(src).and_then(|f| f.eval(&v));
        assert!(r.is_err(), "{src:?} evaluated instead of being rejected: {r:?}");
        let text = r.unwrap_err().to_string();
        assert!(text.len() > 12 && text.contains("formula"), "rejection without a reason: {text}");
    }

    // the position in the string is named: the input field will highlight it
    let e = Formula::parse("1 + $").expect_err("garbage must be a rejection");
    assert!(e.at >= 4, "error position not given: {e:?} ({})", e.at);

    // --- which NAMES an expression reads is known BEFORE evaluation -----------
    let f = Formula::parse("blur.radius * 2 + t").expect("parses");
    assert_eq!(f.reads, vec!["blur.radius".to_string(), "t".to_string()], "{:?}", f.reads);
    let none = Formula::parse("1 + 2").expect("parses");
    assert!(none.reads.is_empty(), "a constant declared dependencies for no reason: {:?}", none.reads);
}

/// Test 784 (R.67): the `t` of a formula is seconds from the START OF THE MATERIAL, and in
/// the preview it is the same: rendering on 12 s of material with an entry point of 10 s
/// gives 2.0, and the preview for two different windows matches the render at the
/// same ticks. In the old code the preview counted `t` from the left edge of the
/// window — the line lied and drifted on scrolling.
#[test]
fn t784_formula_time_counts_from_material_start_everywhere() {
    use kt_model::*;
    use kt_time::Tick;
    let curve = ParamCurve {
        param: "amount".into(),
        keys: vec![],
        source: CurveSource::Formula { expr: "t".into() },
    };
    let clip = ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(10 * 90_000),
        source_in: Tick(10 * 90_000),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
        fx_bypass: vec![],
        fx_auto: vec![vec![curve.clone()]],
        opacity: 1.0,
        fx_off: false,
    };
    let step = 3000;
    let out = resolve_derived(std::slice::from_ref(&clip), step);
    let rendered = &out[0].fx_auto[0][0].keys;
    let at = Tick(12 * 90_000);
    let v = curve_value(rendered, at);
    assert!((v - 2.0).abs() < 1e-9, "render: {v}, expected 2.0 (not 12 and not 0)");
    // preview: two windows — one line, equal to the render
    let material = (clip.source_in, Tick(clip.source_in.0 + clip.duration.0));
    let win_a = formula_preview(&curve, material, Tick(11 * 90_000), Tick(13 * 90_000), step);
    let win_b = formula_preview(&curve, material, Tick(5 * 90_000), Tick(20 * 90_000), step);
    for k in win_a.iter().chain(win_b.iter()) {
        let r = curve_value(rendered, k.t);
        assert!((k.v - r).abs() < 1e-9, "preview {} ≠ render {} at tick {}", k.v, r, k.t.0);
    }
    let a12 = win_a.iter().find(|k| k.t == at).map(|k| k.v).unwrap();
    let b12 = win_b.iter().find(|k| k.t == at).map(|k| k.v).unwrap();
    assert!((a12 - 2.0).abs() < 1e-9 && (b12 - 2.0).abs() < 1e-9, "windows diverged: {a12} / {b12}");
    // `k` is present in the preview as well
    let with_keys = ParamCurve {
        keys: vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(20 * 90_000), 1.0)],
        source: CurveSource::Formula { expr: "k + t".into() },
        ..curve
    };
    let p = formula_preview(&with_keys, material, at, at, step);
    assert!((p[0].v - 3.0).abs() < 1e-9, "k + t in preview: {}", p[0].v);
}
