//! Tests 641-643 (R.59, item 466-467): the property model and compiled
//! automation.
//!
//! The main one here is **t643**: a frozen oracle. The index path must
//! give the same numbers as the named one, otherwise the picture drifts silently:
//! cache keys do not change with the path switch, and no guard would catch it.

use kt_model::*;
use kt_time::Tick;

/// All the property carriers that exist in the model.
fn owners() -> Vec<Box<dyn HasParams>> {
    let mut v: Vec<Box<dyn HasParams>> = Vec::new();
    for f in FxInstance::all() {
        v.push(Box::new(f.clone()));
    }
    for k in TransitionKind::ALL {
        v.push(Box::new(k.clone()));
    }
    for g in GeneratorKind::all() {
        v.push(Box::new(g.clone()));
    }
    v.push(Box::new(CameraParams::default()));
    v
}

#[test]
fn t641_every_passport_is_typed_and_unique() {
    for o in owners() {
        let specs = o.param_specs();
        let mut seen = std::collections::BTreeSet::new();
        for s in specs {
            assert!(
                seen.insert(s.name.to_string()),
                "parameter name {} occurs twice in one passport",
                s.name
            );
            // The built-in rows are declared by a macro, and all of them are numeric:
            // the typing was set up for DYNAMIC owners.
            assert_eq!(s.ty, ParamType::Num, "built-in row {} is not a number", s.name);
            assert!(s.label.is_empty(), "built-in row {} carries its own label", s.name);
            assert!(s.min <= s.default && s.default <= s.max, "{}: default out of range", s.name);
            assert!(s.step > 0.0, "{}: step must be positive", s.name);
        }
    }
}

#[test]
fn t642_index_addressing_equals_names() {
    for mut o in owners() {
        let specs: Vec<ParamSpec> = o.param_specs().to_vec();
        for (i, s) in specs.iter().enumerate() {
            let mid = (s.min + s.max) * 0.5;
            assert!(o.set_param_idx(i, mid), "{}: set by index refused", s.name);
            let by_idx = o.get_param_idx(i).expect("read by index");
            let by_name = o.get_param(&s.name).expect("read by name");
            assert_eq!(by_idx.to_bits(), by_name.to_bits(), "{}: two paths disagree", s.name);

            // The clamp is ONE AND THE SAME for both paths — that is the main point here.
            // The comparison with the limit itself uses a tolerance for f32: the variant field
            // stores 0.99 as 0.990000009, and that is not a clamp miss
            // but the width of the type.
            o.set_param_idx(i, s.max + 1e6);
            let hi = o.get_param_idx(i).unwrap();
            o.set_param(&s.name, s.max + 1e6);
            assert_eq!(hi.to_bits(), o.get_param_idx(i).unwrap().to_bits(), "{}: clamp", s.name);
            let tol = s.max.abs().max(1.0) * 1e-6;
            assert!(hi <= s.max + tol, "{}: index path does not clamp ({hi} > {})", s.name, s.max);
            o.set_param_idx(i, s.min - 1e6);
            let lo = o.get_param_idx(i).unwrap();
            assert!(
                lo >= s.min - tol,
                "{}: index path does not clamp below ({lo} < {})",
                s.name,
                s.min
            );
        }
        assert!(!o.set_param_idx(specs.len() + 7, 1.0), "index past the passport was accepted");
        assert_eq!(o.get_param_idx(specs.len() + 7), None);
    }
}

/// FROZEN ORACLE: a verbatim copy of the named path as it was before R.59.
/// It lives HERE and is not to be edited — the plan is checked against it.
mod frozen {
    use kt_model::*;
    use kt_time::Tick;

    pub fn apply_by_name(fx: &mut [FxInstance], auto: &[Vec<ParamCurve>], at: Tick) {
        for (i, f) in fx.iter_mut().enumerate() {
            if let Some(curves) = auto.get(i) {
                apply_fx_auto(f, curves, at);
            }
        }
    }
}

fn curve(param: &str, a: f64, b: f64, ease: bool) -> ParamCurve {
    // the former boolean smoothing is one form for all segments (R.61)
    let shape = if ease { SegShape::DEFAULT } else { SegShape::Linear };
    ParamCurve {
        param: param.into(),
        keys: vec![AutoKey::shaped(Tick(0), a, shape), AutoKey::shaped(Tick(90_000), b, shape)],
        source: CurveSource::Keys,
    }
}

#[test]
fn t643_compiled_plan_equals_the_frozen_name_path() {
    let probes = [
        Tick(-45_000),
        Tick(0),
        Tick(1),
        Tick(22_500),
        Tick(45_000),
        Tick(89_999),
        Tick(90_000),
        Tick(180_000),
    ];
    let mut checked = 0usize;
    for f in FxInstance::all() {
        let specs: Vec<ParamSpec> = f.auto_params().cloned().collect();
        if specs.is_empty() {
            continue;
        }
        for s in &specs {
            for ease in [false, true] {
                // the curve goes OUTSIDE the spec’s range at both ends: the clamp must
                // match in both paths, not only the value
                let curves = vec![vec![
                    curve(&s.name, s.min - 5.0, s.max + 5.0, ease),
                    // an orphan next to it: both paths must skip it
                    curve("no_such_param_at_all", 0.0, 1.0, ease),
                ]];
                let chain = vec![f.clone()];
                let plan = AutoPlan::compile(&chain, &curves).expect("live curve compiles");
                assert_eq!(plan.len(), 1, "{}: orphan curve got compiled", s.name);
                for t in probes {
                    let mut by_plan = chain.clone();
                    plan.apply(&mut by_plan, t);
                    let mut by_name = chain.clone();
                    frozen::apply_by_name(&mut by_name, &curves, t);
                    assert_eq!(
                        by_plan,
                        by_name,
                        "{}.{} at {t:?}: the compiled plan diverged from the frozen name path",
                        f.tag(),
                        s.name
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "the sweep covered too little: {checked}");
}

#[test]
fn t643b_nothing_to_do_compiles_to_nothing() {
    let chain = vec![FxInstance::gain(0.0)];
    assert!(AutoPlan::compile(&chain, &[]).is_none(), "empty automation must compile to None");
    assert!(
        AutoPlan::compile(&chain, &[vec![]]).is_none(),
        "an empty curve list must compile to None"
    );
    let orphan = vec![vec![curve("no_such_param", 0.0, 1.0, false)]];
    assert!(
        AutoPlan::compile(&chain, &orphan).is_none(),
        "a chain of orphan curves must compile to None — otherwise the clip pays for nothing"
    );
    let keyless =
        vec![vec![ParamCurve { param: "db".into(), keys: vec![], source: CurveSource::Keys }]];
    assert!(AutoPlan::compile(&chain, &keyless).is_none(), "a curve without keys is not live");
}

#[test]
fn t649_formula_curves_are_refused_loudly() {
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
    let mut c = ClipEvent {
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
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::gain(0.0)],
        fx_bypass: vec![],
        fx_auto: vec![vec![curve("db", -6.0, 0.0, true)]],
        opacity: 1.0,
        fx_off: false,
    };
    p.clips.push(c.clone());
    p.ensure_tracks(LaneKind::Video, 1);
    p.validate().expect("a project with ordinary key curves is legal");

    // ...and with R.61 the formula is COMPUTED: what gets rejected is not the source
    // but a source that is NOT ALLOWED. A silent render “by keys” would show
    // something other than what is written in the file.
    c.fx_auto[0][0].source = CurveSource::Formula { expr: "t * 2".into() };
    c.fx_auto[0][0].keys.clear();
    p.clips[0] = c.clone();
    p.validate().expect("a formula-driven curve is legal since R.61");

    c.fx_auto[0][0].source = CurveSource::Formula { expr: "1 + ".into() };
    p.clips[0] = c.clone();
    let err = p.validate().expect_err("an unparseable formula must be refused");
    assert!(
        matches!(err, ProjectError::CurveSourceBroken(ClipId(1), _)),
        "wrong error for a broken curve source: {err:?}"
    );

    c.fx_auto[0][0].source =
        CurveSource::Link { path: "not an address".into(), expr: None, stops: Vec::new() };
    p.clips[0] = c;
    let err = p.validate().expect_err("a malformed link address must be refused");
    assert!(matches!(err, ProjectError::CurveSourceBroken(ClipId(1), _)), "{err:?}");
}
