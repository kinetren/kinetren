//! Test 190: automation of effect parameters.
//! curve_value (edge clamp, linearity, ease), set/get_param over ALL
//! specs of every variant, commands SetFxAuto/SetFx (trimming), validation of broken
//! curves, byte stability of files without automation.
//! Under R.33 keyframes sit in MATERIAL TICKS, not in fractions of the event length.

use kt_model::*;
use kt_time::Tick;

fn key(t: i64, v: f64) -> AutoKey {
    AutoKey::at(Tick(t), v)
}

fn lin(t: i64, v: f64) -> AutoKey {
    AutoKey::shaped(Tick(t), v, SegShape::Linear)
}

#[test]
fn t190_curve_value_semantics() {
    let keys = vec![key(20_000, 10.0), key(80_000, 40.0)];
    // edge clamp: before the first point and after the last — the edge values
    assert_eq!(curve_value(&keys, Tick(0)), 10.0);
    assert_eq!(curve_value(&keys, Tick(20_000)), 10.0);
    assert_eq!(curve_value(&keys, Tick(100_000)), 40.0);
    assert_eq!(
        curve_value(&keys, Tick(5_000_000)),
        40.0,
        "far past the last keyframe — its value"
    );
    // the domain is NOT clamped from above: a position past the clip body is legitimate
    // (a transition handle, pre-roll) and continues the curve with its edge value
    assert_eq!(curve_value(&keys, Tick(-1)), 10.0);
    // the midpoint of a segment matches for linear and third-order (smoothstep(0.5) = 0.5),
    // the quarter point does not
    let straight = vec![lin(20_000, 10.0), lin(80_000, 40.0)];
    assert!((curve_value(&straight, Tick(50_000)) - 25.0).abs() < 1e-9);
    assert!((curve_value(&keys, Tick(50_000)) - 25.0).abs() < 1e-9);
    let quarter_lin = curve_value(&straight, Tick(35_000));
    let quarter_smooth = curve_value(&keys, Tick(35_000));
    assert!(quarter_smooth < quarter_lin, "the third order pinches the start of the segment");
    // one point — a constant
    assert_eq!(curve_value(&[key(50_000, 7.0)], Tick(0)), 7.0);
    assert_eq!(curve_value(&[key(50_000, 7.0)], Tick(90_000)), 7.0);
    // empty curve — zero (does not fire via apply_fx_auto)
    assert_eq!(curve_value(&[], Tick(50_000)), 0.0);
}

/// Keyframes are pinned to the MATERIAL (R.33, user decision): trimming does not move them,
/// a split gives each half its own piece, reverse mirrors the animation together with the
/// material. Previously the domain was “fraction of the event length”: trimming stretched
/// the animation, and Split copied the curve whole into both halves — both played it
/// from start to end.
#[test]
fn t212_keys_stick_to_material_through_edits() {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(ClipEvent {
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
        fx: vec![FxInstance::blur(8)],
        fx_bypass: vec![],
        fx_auto: vec![vec![ParamCurve {
            param: "radius".into(),

            keys: AutoKey::linear(vec![key(0, 1.0), key(45_000, 40.0), key(90_000, 10.0)]),
            source: Default::default(),
        }]],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);
    let keys_of = |m: &TimelineModel, id: ClipId| -> Vec<(i64, f64)> {
        m.clip(id).unwrap().fx_auto[0][0].keys.iter().map(|k| (k.t.0, k.v)).collect()
    };
    let before = keys_of(&m, ClipId(1));

    // trimming the end: keyframes do NOT move, the last one just goes past the edge
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(60_000) }).unwrap();
    assert_eq!(keys_of(&m, ClipId(1)), before, "trim must not move keyframes");
    m.undo().unwrap();

    // trimming the start: also does not move — the material under the keyframes is the same
    m.apply(Command::TrimStart { id: ClipId(1), new_start: Tick(30_000) }).unwrap();
    assert_eq!(keys_of(&m, ClipId(1)), before, "trimming the start must not move keyframes");
    m.undo().unwrap();

    // split: both halves hold ONE curve in material coordinates,
    // so each plays its own piece (the left one shows the key at 0..45k, the
    // right one at 45k..90k) — the animation is no longer duplicated
    m.apply(Command::Split { id: ClipId(1), at: Tick(45_000), new_id: ClipId(2), new_group: None })
        .unwrap();
    let (l, r) = (m.clip(ClipId(1)).unwrap().clone(), m.clip(ClipId(2)).unwrap().clone());
    assert_eq!(keys_of(&m, ClipId(1)), before);
    assert_eq!(keys_of(&m, ClipId(2)), before);
    // the curve value at the join is identical on both sides — the animation
    // is continuous across the split
    let left_end =
        kt_model::clip_material_at(l.source_in, l.start, l.duration, false, 3000, Tick(45_000));
    let right_start =
        kt_model::clip_material_at(r.source_in, r.start, r.duration, false, 3000, Tick(45_000));
    assert_eq!(left_end, right_start, "the cut seam is one material point");
    assert_eq!(
        curve_value(&l.fx_auto[0][0].keys, left_end),
        curve_value(&r.fx_auto[0][0].keys, right_start),
        "the curve is continuous across the cut"
    );
}

/// Every parameter from param_specs must be settable and readable;
/// values are clamped to the range; an unknown name is a failure (an orphan curve).
#[test]
fn t190_set_get_param_all_specs() {
    // R.47 (item 326): a registry instead of a hand-written list — a kind not
    // in FxInstance::all() will not pass the completeness lock below.
    let variants: Vec<FxInstance> = FxInstance::all().to_vec();
    let tags: std::collections::BTreeSet<&str> = variants.iter().map(|f| f.tag()).collect();
    assert_eq!(tags.len(), variants.len(), "registry tags are unique");
    let mut specs_total = 0;
    for mut f in variants {
        for spec in f.param_specs() {
            specs_total += 1;
            let (name, lo, hi) = (spec.name.clone(), spec.min, spec.max);
            // the spec default must lie within its own range
            assert!(
                (lo..=hi).contains(&spec.default),
                "{name}: default {} outside [{lo}, {hi}]",
                spec.default
            );
            assert!(spec.step > 0.0, "{name}: step must be positive");
            // the middle of the range can be set and read back (up to rounding
            // of integer fields)
            let mid = (lo + hi) / 2.0;
            assert!(f.set_param(&name, mid), "{name} did not set");
            let got = f.get_param(&name).unwrap();
            assert!((got - mid).abs() <= 0.5, "{name}: {got} vs {mid}");
            // clamping for values outside the range
            // f32 fields return the nearest representable value to the bound
            // (0.99f32 = 0.99000001) — a tolerance of honest f32 epsilon
            let eps = 1e-6 * hi.abs().max(1.0);
            assert!(f.set_param(&name, hi + 1e6));
            assert!(f.get_param(&name).unwrap() <= hi + eps, "{name} does not clamp");
            assert!(f.set_param(&name, lo - 1e6));
            assert!(f.get_param(&name).unwrap() >= lo - eps);
        }
        // unknown name — rejected, value untouched
        assert!(!f.set_param("no_such_param", 1.0));
        assert_eq!(f.get_param("no_such_param"), None);
    }
    assert!(specs_total >= 20, "specs cover all video variants: {specs_total}");
    // SPEED WITH SIGN (R.57, owner’s decision): minus is motion backward through the
    // material, zero is a freeze frame. The generic check above already probes the clamps;
    // here the BOUNDARIES THEMSELVES are named, because they were the complaint
    // (“not turning the range -0.25...0...0.25 into 0”).
    let mut slowed = FxInstance::slowed_reverb(1.0, 0.0, 85.0);
    let rate = slowed.param_specs().iter().find(|s| s.name == "rate").expect("speed spec");
    assert_eq!((rate.min, rate.max), (-32.0, 32.0), "speed range");
    for v in [-32.0, -1.0, -0.03, 0.0, 0.05, 1.0, 32.0] {
        assert!(slowed.set_param("rate", v), "speed {v} did not set");
        assert_eq!(slowed.get_param("rate").unwrap(), v, "speed {v} drifted");
    }
    // the mirror has no numeric parameters at all
    assert!(FxInstance::Mirror { horizontal: true, vertical: false }.param_specs().is_empty());
    // R.33: audio received specs — it is automated by the same mechanism
    let gain = FxInstance::gain(0.0);
    assert_eq!(
        gain.auto_params().count(),
        2,
        "volume and balance must be automatable (R.33 C5)"
    );
    let slowed = FxInstance::slowed_reverb(1.0, 0.3, 60.0);
    assert!(
        slowed.auto_params().any(|s| s.name == "rate"),
        "slowed&reverb speed must be automatable"
    );
    assert_eq!(
        slowed.param_spec("reverb_delay_ms").map(|s| s.kind),
        Some(ParamKind::Structural),
        "reverb line length is a structural parameter, not subject to curves"
    );
    assert!(
        !slowed.auto_params().any(|s| s.name == "reverb_delay_ms"),
        "a structural parameter is not offered for automation"
    );
}

#[test]
fn t190_apply_fx_auto() {
    let mut f = FxInstance::blur(8);
    let curves = vec![
        ParamCurve {
            param: "radius".into(),

            keys: AutoKey::linear(vec![key(0, 1.0), key(90_000, 47.0)]),
            source: Default::default(),
        },
        ParamCurve {
            param: "orphan".into(),

            keys: AutoKey::linear(vec![key(0, 5.0)]),
            source: Default::default(),
        },
        ParamCurve {
            param: "empty".into(),
            keys: AutoKey::linear(vec![]),
            source: Default::default(),
        },
    ];
    assert_eq!(apply_fx_auto(&mut f, &curves, Tick(45_000)), 1, "orphans and empty ones are not counted");
    assert_eq!(f.get_param("radius"), Some(24.0), "lerp 1..47 in the middle");
}

fn clip_with_fx() -> ClipEvent {
    ClipEvent {
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
        fx: vec![FxInstance::blur(8), FxInstance::glow(0.5)],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn model_with_clip() -> TimelineModel {
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
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips.push(clip_with_fx());
    p.validate().unwrap();
    TimelineModel::new(p)
}

#[test]
fn t190_commands_undo_and_truncation() {
    let mut m = model_with_clip();
    let auto = vec![
        vec![ParamCurve {
            param: "radius".into(),

            keys: AutoKey::linear(vec![key(0, 1.0), key(90_000, 48.0)]),
            source: Default::default(),
        }],
        vec![ParamCurve {
            param: "strength".into(),
            keys: vec![key(0, 0.0), key(90_000, 2.0)],
            source: Default::default(),
        }],
    ];
    assert!(m.apply(Command::SetFxAuto { id: ClipId(1), auto: auto.clone() }).is_ok());
    assert_eq!(m.project().clips[0].fx_auto, auto);
    // broken curves do not pass: not sorted / before the start of the material /
    // not finite
    for bad_keys in
        [vec![key(90_000, 1.0), key(10_000, 2.0)], vec![key(-1, 1.0)], vec![key(45_000, f64::NAN)]]
    {
        let bad = vec![vec![ParamCurve {
            param: "radius".into(),

            keys: AutoKey::linear(bad_keys),
            source: Default::default(),
        }]];
        assert!(
            m.apply(Command::SetFxAuto { id: ClipId(1), auto: bad }).is_err(),
            "a broken curve must be rejected"
        );
    }
    // SetFx with a shortened chain trims the curves of the surplus slots
    assert!(m.apply(Command::SetFx { id: ClipId(1), fx: vec![FxInstance::blur(8)] }).is_ok());
    assert_eq!(m.project().clips[0].fx_auto.len(), 1, "curves of the second slot dropped off");
    assert_eq!(m.project().clips[0].fx_auto[0][0].param, "radius");
    // undo restores both the chain and the curves
    m.undo().unwrap();
    assert_eq!(m.project().clips[0].fx.len(), 2);
    assert_eq!(m.project().clips[0].fx_auto, auto);
}

#[test]
fn t190_files_without_auto_are_byte_stable() {
    let m = model_with_clip();
    let json = serde_json::to_string(m.project()).unwrap();
    assert!(!json.contains("fx_auto"), "empty automation is not serialized: {json}");
    // active_fx_auto is aligned with active_fx under bypass
    let mut c = clip_with_fx();
    c.fx_auto = vec![
        vec![ParamCurve {
            param: "radius".into(),

            keys: AutoKey::linear(vec![key(0, 1.0)]),
            source: Default::default(),
        }],
        vec![ParamCurve {
            param: "strength".into(),

            keys: AutoKey::linear(vec![key(0, 1.0)]),
            source: Default::default(),
        }],
    ];
    c.fx_bypass = vec![true]; // the first slot is disabled
    let fx = c.active_fx();
    let auto = c.active_fx_auto();
    assert_eq!(fx.len(), 1);
    assert_eq!(auto.len(), 1);
    assert_eq!(auto[0][0].param, "strength", "the curve moved TOGETHER with its slot");
}

/// Test 214 (R.33 C1, user request: “there is no master checkbox to disable the current
/// fx chain; in morphers and fx batches likewise, everything should be unified”).
/// The switch turns the chain off ENTIRELY, without touching the state of the slots,
/// and is not written to the file while it is on.
#[test]
fn t214_master_switch_disables_chain_without_losing_it() {
    let mut m = model_with_clip();
    let auto = vec![vec![ParamCurve {
        param: "radius".into(),

        keys: AutoKey::linear(vec![key(0, 1.0), key(90_000, 48.0)]),
        source: Default::default(),
    }]];
    m.apply(Command::SetFxAuto { id: ClipId(1), auto: auto.clone() }).unwrap();
    m.apply(Command::SetFxBypass { id: ClipId(1), bypass: vec![false, true] }).unwrap();
    let before = m.clip(ClipId(1)).unwrap().clone();
    assert_eq!(before.active_fx().len(), 1, "the second slot is disabled by bypass");

    // master switch: the engine sees NOTHING
    m.apply(Command::SetFxOff { id: ClipId(1), off: true }).unwrap();
    let c = m.clip(ClipId(1)).unwrap();
    assert!(c.active_fx().is_empty(), "a disabled chain does not reach the engine");
    assert!(c.active_fx_auto().is_empty(), "curves of a disabled chain too");
    // but the chain itself, bypass and the curves are intact
    assert_eq!(c.fx, before.fx, "the chain must survive");
    assert_eq!(c.fx_bypass, before.fx_bypass, "slot state is not touched");
    assert_eq!(c.fx_auto, before.fx_auto, "curves are not touched");

    // back to how it was — undo works
    m.apply(Command::SetFxOff { id: ClipId(1), off: false }).unwrap();
    assert_eq!(m.clip(ClipId(1)).unwrap().active_fx().len(), 1);
    m.undo().unwrap();
    assert!(m.clip(ClipId(1)).unwrap().fx_off, "undo restored the disabled state");

    // a disabled morpher is not applied to its targets, but stays on the track
    let mut p = m.project().clone();
    p.ensure_tracks(LaneKind::Morph, 1);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        targets: MorphTargets::Clips { ids: vec![ClipId(1)] },
        patch: MorphPatch {
            video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
            ..Default::default()
        },
        timed: false,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    });
    p.validate().unwrap();
    // The master switch turns off the chain of the EVENT, not the layer above it
    // (R.35): its own chain goes away, the morpher’s effect stays in force.
    let layered = p.effective_clips().remove(0);
    assert_eq!(layered.active_fx().len(), 1, "a layer over the disabled chain of an event");
    assert!(!layered.fx_off, "and the switch itself does not belong to the layer");
    // a disabled morpher is not applied to its targets, but stays on the track
    p.morphers[0].enabled = false;
    let off = p.effective_clips().remove(0);
    assert!(off.active_fx().is_empty(), "a disabled morpher adds no effects");
    assert_eq!(off.fx, before.fx, "and does not touch the chain of the event");
    assert!(off.fx_off, "the event switch stayed a switch");
    assert_eq!(p.morphers.len(), 1, "and it stays on the track");
}

/// Test 215 (R.33 C1, user request: “copy-paste is missing — both for individual effects
/// and for the fx chain, and for automation”).
/// Blocks live in the model: the system clipboard uses their own shape (JSON).
#[test]
fn t215_fx_clipboard_slot_chain_and_envelope() {
    use kt_model::ops;
    let mut m = model_with_clip();
    let auto = vec![
        vec![ParamCurve {
            param: "radius".into(),

            keys: AutoKey::linear(vec![key(0, 1.0), key(90_000, 48.0)]),
            source: Default::default(),
        }],
        vec![ParamCurve {
            param: "strength".into(),
            keys: vec![key(0, 0.0), key(90_000, 2.0)],
            source: Default::default(),
        }],
    ];
    m.apply(Command::SetFxAuto { id: ClipId(1), auto }).unwrap();
    m.apply(Command::SetFxBypass { id: ClipId(1), bypass: vec![false, true] }).unwrap();
    let src = m.clip(ClipId(1)).unwrap().clone();

    // target: a DIFFERENT entry point — the keys must be re-hung on its material
    let mut target = src.clone();
    target.id = ClipId(2);
    target.start = Tick(180_000);
    target.source_in = Tick(45_000);
    target.fx = vec![];
    target.fx_auto = vec![];
    target.fx_bypass = vec![];
    let mut p = m.project().clone();
    p.clips.push(target.clone());
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);

    // 1) the whole chain: slots, bypass and curves move “entry to entry” —
    //    by one function, which both the UI and presets (R.48) call
    let block = ops::FxChainBlock {
        slots: src
            .fx
            .iter()
            .enumerate()
            .map(|(i, f)| ops::FxSlotBlock {
                fx: f.clone(),
                bypass: src.fx_bypass[i],
                curves: src.fx_auto[i].clone(),
            })
            .collect(),
        src_in: src.source_in,
        fx_off: src.fx_off,
    };
    let target = m.clip(ClipId(2)).unwrap().clone();
    let (mut fx, mut bypass, mut auto) =
        (target.fx.clone(), target.fx_bypass.clone(), target.fx_auto.clone());
    ops::append_chain_block(&mut fx, &mut bypass, &mut auto, &block, target.source_in);
    m.apply_batch(vec![
        Command::SetFx { id: ClipId(2), fx },
        Command::SetFxBypass { id: ClipId(2), bypass },
        Command::SetFxAuto { id: ClipId(2), auto },
    ])
    .unwrap();
    let got = m.clip(ClipId(2)).unwrap().clone();
    assert_eq!(got.fx, src.fx, "the chain did not move");
    assert_eq!(got.fx_bypass, src.fx_bypass, "slot state did not move");
    assert_eq!(
        got.fx_auto[0][0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![45_000, 135_000],
        "keyframes must land on the same piece of the target material"
    );
    // undo — one step for the whole gesture
    let depth = m.undo_depth();
    m.undo().unwrap();
    assert!(m.clip(ClipId(2)).unwrap().fx.is_empty(), "undo removed the inserted chain");
    assert_eq!(m.undo_depth(), depth - 1, "chain insertion is one undo step");
    m.redo().unwrap();

    // 2) APPENDING: a second call puts the slots at the end, without overwriting
    let target = m.clip(ClipId(2)).unwrap().clone();
    let before = target.fx.len();
    let (mut fx, mut bypass, mut auto) =
        (target.fx.clone(), target.fx_bypass.clone(), target.fx_auto.clone());
    let one = ops::FxChainBlock {
        slots: vec![block.slots[0].clone()],
        src_in: block.src_in,
        fx_off: false,
    };
    ops::append_chain_block(&mut fx, &mut bypass, &mut auto, &one, target.source_in);
    assert_eq!(fx.len(), before + 1, "slot was not added");
    assert_eq!(auto.len(), fx.len(), "curves must run parallel to the slots");

    // 3) the rebase is symmetric: event → morpher (entry = 0) and back
    let to_morph = ops::rebase_curves(&src.fx_auto[0], src.source_in, kt_time::Tick(0));
    assert_eq!(
        to_morph[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![0, 90_000],
        "in the morpher domain keyframes are counted from the entry point"
    );
    let back_to_clip = ops::rebase_curves(&to_morph, kt_time::Tick(0), Tick(45_000));
    assert_eq!(
        back_to_clip[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![45_000, 135_000],
        "the reverse transfer puts keyframes back on the material of the receiver"
    );
    // a key to the left of the entry point is clamped to zero, not sent negative
    let below = ops::rebase_curves(&src.fx_auto[0], Tick(90_000), Tick(0));
    assert!(below[0].keys.iter().all(|k| k.t.0 >= 0), "keyframes do not go into negative time");

    // 4) the block survives the system clipboard (JSON) without loss
    let json = serde_json::to_string(&block).unwrap();
    let back: ops::FxChainBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(back, block, "a block must survive a round trip through the system buffer");
}

/// Test 248 (R.37 C3, user request: “an effect for audio/video can be combined at once,
/// not added one by one”). The rule lives in the MODEL, not in the FX window:
/// `SetFx` also arrives from chain insertion, from the morpher bake, from the format
/// reader and from kt-cli.
#[test]
fn t248_warp_rate_is_shared_by_the_link_group() {
    use kt_time::Tick;
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
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    let mk = |id: u64, kind: ClipKind| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: Some(GroupId(1)),
        kind,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    p.clips.push(mk(1, ClipKind::Video));
    p.clips.push(mk(2, ClipKind::Audio));
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);
    let slot = |m: &TimelineModel, id: u64| -> Option<FxInstance> {
        m.clip(ClipId(id)).unwrap().fx.iter().find(|f| f.warps_time()).cloned()
    };
    let rate_of = |f: &FxInstance| match f {
        FxInstance::SlowedReverb { rate, .. } => *rate,
        _ => unreachable!(),
    };

    // one addition puts the instance on BOTH events of the pair
    let depth = m.undo_depth();
    m.apply(Command::SetFx {
        id: ClipId(2),
        fx: vec![FxInstance::gain(0.0), FxInstance::slowed_reverb(0.5, 0.2, 85.0)],
    })
    .unwrap();
    assert_eq!(m.undo_depth(), depth + 1, "the pair is edited in ONE history step");
    let v = slot(&m, 1).expect("the video instance was created by itself");
    assert_eq!(rate_of(&v), 0.5, "and with the same speed");
    assert_eq!(m.clip(ClipId(1)).unwrap().fx.len(), 1, "video has only it");

    // editing the rate of the VIDEO propagates to the audio partner
    let mut vfx = m.clip(ClipId(1)).unwrap().fx.clone();
    if let FxInstance::SlowedReverb { rate, .. } = &mut vfx[0] {
        *rate = 0.25;
    }
    m.apply(Command::SetFx { id: ClipId(1), fx: vfx }).unwrap();
    assert_eq!(rate_of(&slot(&m, 2).unwrap()), 0.25, "speed is shared both ways");

    // the slot is looked up BY KIND, not by index: for audio it is the second one
    assert!(matches!(m.clip(ClipId(2)).unwrap().fx[0], FxInstance::Gain { .. }));

    // the rate curve is also synchronized, but the curves of OTHER parameters are not
    let curve = ParamCurve {
        param: "rate".into(),

        keys: AutoKey::linear(vec![key(0, 1.0), key(90_000, 2.0)]),
        source: Default::default(),
    };
    let mix = ParamCurve {
        param: "mix".into(),

        keys: AutoKey::linear(vec![key(0, 0.9)]),
        source: Default::default(),
    };
    m.apply(Command::SetFxAuto { id: ClipId(2), auto: vec![vec![], vec![curve, mix]] }).unwrap();
    let va = &m.clip(ClipId(1)).unwrap().fx_auto[0];
    assert_eq!(va.len(), 1, "only the speed curve travels to the partner: {va:?}");
    assert_eq!(va[0].param, "rate");
    assert_eq!(va[0].keys.len(), 2);

    // the reverb of the video instance is its own and is not imposed on the partner
    let mut vfx = m.clip(ClipId(1)).unwrap().fx.clone();
    if let FxInstance::SlowedReverb { mix, .. } = &mut vfx[0] {
        *mix = 0.0;
    }
    m.apply(Command::SetFx { id: ClipId(1), fx: vfx }).unwrap();
    assert!(
        matches!(slot(&m, 2).unwrap(), FxInstance::SlowedReverb { mix, .. } if mix == 0.2),
        "the reverb of the partner is untouched"
    );

    // removal on one removes it on both
    m.apply(Command::SetFx { id: ClipId(2), fx: vec![FxInstance::gain(0.0)] }).unwrap();
    assert!(slot(&m, 1).is_none(), "removed on one — gone on both");
    assert!(m.clip(ClipId(1)).unwrap().fx_auto.iter().all(|c| c.is_empty()));

    // BREAKING THE GROUP stops the synchronization
    m.apply(Command::UnlinkClip { id: ClipId(1) }).unwrap();
    m.apply(Command::SetFx { id: ClipId(2), fx: vec![FxInstance::slowed_reverb(0.5, 0.2, 85.0)] })
        .unwrap();
    assert!(slot(&m, 1).is_none(), "an unlinked clip no longer picks it up");
    m.project().validate().unwrap();
}

/// Test 248b: a single clip WITHOUT a pair is not touched by the rule — the golden project of
/// test 11 depends on this (there slowed sits on an audio clip without a link group).
#[test]
fn t248_lone_clip_is_untouched_by_the_rule() {
    let mut m = model_with_clip();
    let id = m.project().clips[0].id;
    assert!(m.project().clips[0].link_group.is_none());
    m.apply(Command::SetFx { id, fx: vec![FxInstance::slowed_reverb(0.5, 0.2, 85.0)] }).unwrap();
    assert_eq!(m.project().clips.iter().filter(|c| !c.fx.is_empty()).count(), 1);
}

/// Test 342 (R.48, item 342): chain insertion — ONE source of truth for both carriers.
///
/// Before R.48 the UI duplicated the rebase inline in two places, and for a morpher the
/// curves could go nowhere through a silent `Option` branch. Now insertion is done by
/// `append_chain_block`, and its behaviour is pinned here: the transfer “entry to entry”
/// in BOTH directions, appending to the end, the receiver is not turned off by a foreign
/// `fx_off`.
#[test]
fn t342_append_chain_block_is_one_truth() {
    use kt_model::ops;
    let curves = vec![ParamCurve {
        param: "radius".into(),

        keys: AutoKey::linear(vec![key(45_000, 1.0), key(135_000, 48.0)]),
        source: Default::default(),
    }];
    let block = ops::FxChainBlock {
        slots: vec![ops::FxSlotBlock {
            fx: FxInstance::blur(8),
            bypass: true,
            curves: curves.clone(),
        }],
        // the donor — an event with an entry point of 45_000 ticks of material
        src_in: Tick(45_000),
        // the donor’s master switch is off: the receiver does NOT inherit this
        fx_off: true,
    };

    // event → MORPHER: the morpher’s domain is the block time, entry = 0
    let (mut fx, mut bypass, mut auto) = (vec![], vec![], vec![]);
    ops::append_chain_block(&mut fx, &mut bypass, &mut auto, &block, Tick(0));
    assert_eq!(fx.len(), 1);
    assert_eq!(bypass, vec![true], "slot state must move");
    assert_eq!(
        auto[0][0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![0, 90_000],
        "in the morpher keyframes are counted from the entry point of the donor"
    );

    // MORPHER → event: the same offset from the entry, but now in the target’s material
    let from_morph = ops::FxChainBlock {
        slots: vec![ops::FxSlotBlock {
            fx: FxInstance::blur(8),
            bypass: false,
            curves: auto[0].clone(),
        }],
        src_in: Tick(0),
        fx_off: false,
    };
    let (mut fx2, mut bypass2, mut auto2) = (vec![], vec![], vec![]);
    ops::append_chain_block(&mut fx2, &mut bypass2, &mut auto2, &from_morph, Tick(30_000));
    assert_eq!(
        auto2[0][0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![30_000, 120_000],
        "the reverse transfer puts keyframes on the material of the receiver"
    );

    // APPENDING: the second block goes to the end, the first is intact
    ops::append_chain_block(&mut fx2, &mut bypass2, &mut auto2, &block, Tick(30_000));
    assert_eq!(fx2.len(), 2, "insertion appends rather than replaces");
    assert_eq!(auto2.len(), 2, "curves run parallel to the slots");
    assert_eq!(bypass2, vec![false, true]);

    // a receiver with a non-empty chain and WITHOUT curves receives them correctly
    let (mut fx3, mut bypass3, mut auto3) = (vec![FxInstance::gain(-3.0)], vec![], vec![]);
    ops::append_chain_block(&mut fx3, &mut bypass3, &mut auto3, &block, Tick(0));
    assert_eq!(auto3.len(), 2, "empty curves grow to the chain length");
    assert!(auto3[0].is_empty(), "the previous slot got no curves");
    assert!(!auto3[1].is_empty(), "the inserted one got some");
}

// ---- 695, 697 (R.61). the segment shape ---------------------------------------

/// A frozen ORACLE of the old smoothing: a verbatim copy of `curve_value`
/// in the revision before R.61, when the shape was one per curve (`ease: bool`).
///
/// It lives in the test on purpose: the default shape must compute EXACTLY the same
/// thing, otherwise GOLDEN_AUDIO and GOLDEN_VIDEO would move, and that is a determinism
/// regression, not a new capability.
fn frozen_curve_value(keys: &[AutoKey], ease: bool, at: Tick) -> f64 {
    let Some(first) = keys.first() else { return 0.0 };
    if at <= first.t {
        return first.v;
    }
    for w in keys.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if at <= b.t {
            let span = (b.t.0 - a.t.0).max(1) as f64;
            let mut s = (((at.0 - a.t.0) as f64) / span).clamp(0.0, 1.0);
            if ease {
                s = s * s * (3.0 - 2.0 * s);
            }
            return a.v + (b.v - a.v) * s;
        }
    }
    keys.last().unwrap().v
}

/// Test 695 (R.61): segment shapes compute in a closed and deterministic way.
///
/// A shape belongs to the LEFT key of the segment and has ITS OWN parameters — not one
/// shared “stiffness” number (owner’s decision: a zigzag is a segment shape, revision of
/// verdict F-AUT-009 item 3). From this follow two obligations: the ends of the segment
/// match the key values for all shapes except the step, and the amplitude of the zigzag
/// is set IN PARAMETER UNITS — otherwise jitter around a CONSTANT value (the most common
/// case) would collapse to zero.
#[test]
fn t695_segment_shapes_are_closed_form_and_join_at_keys() {
    let shapes = [
        SegShape::Linear,
        SegShape::DEFAULT,
        SegShape::EaseIn { tension: 1.0 },
        SegShape::EaseOut { tension: 1.0 },
        SegShape::EaseIn { tension: 0.0 },
        SegShape::EaseOut { tension: 4.0 },
        SegShape::Smooth { tension: 0.0 },
        SegShape::Smooth { tension: 4.0 },
        SegShape::Zigzag { periods: 3.5, amplitude: 2.0, profile: ZigProfile::Triangle },
        SegShape::Zigzag { periods: 1.0, amplitude: 2.0, profile: ZigProfile::Square },
        SegShape::Zigzag { periods: 7.0, amplitude: 2.0, profile: ZigProfile::Sine },
    ];

    // --- the ENDS match the keys for all shapes except the step ---------
    for sh in shapes {
        assert!((sh.at(0.0, 10.0, 40.0) - 10.0).abs() < 1e-12, "left end diverged: {sh:?}");
        assert!((sh.at(1.0, 10.0, 40.0) - 40.0).abs() < 1e-12, "right end diverged: {sh:?}");
    }
    // the step holds the left value and arrives at the right one exactly on the key
    assert_eq!(SegShape::Hold.at(0.0, 10.0, 40.0), 10.0);
    assert_eq!(SegShape::Hold.at(0.999, 10.0, 40.0), 10.0, "a step must hold its value");
    assert_eq!(SegShape::Hold.at(1.0, 10.0, 40.0), 40.0);

    // --- DETERMINISM: two runs, byte-for-byte -------------------------------
    let probe = |sh: SegShape| -> Vec<u64> {
        (0..=64).map(|i| sh.at(i as f64 / 64.0, -3.0, 11.0).to_bits()).collect()
    };
    for sh in shapes {
        assert_eq!(probe(sh), probe(sh), "shape {sh:?} is not deterministic");
    }

    // --- a ZIGZAG around a CONSTANT value ---------------------------
    // exactly the case the amplitude in parameter units exists for:
    // the difference between the keys is zero, and a fraction would collapse the jitter
    let zz = SegShape::Zigzag { periods: 4.0, amplitude: 5.0, profile: ZigProfile::Triangle };
    let vals: Vec<f64> = (0..=200).map(|i| zz.at(i as f64 / 200.0, 7.0, 7.0)).collect();
    let (lo, hi) = vals.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    assert!(hi - lo > 4.0, "zigzag on a constant segment does not jitter: {lo}..{hi}");
    assert!(hi <= 7.0 + 5.0 + 1e-9 && lo >= 7.0 - 5.0 - 1e-9, "went beyond the amplitude: {lo}..{hi}");
    assert_eq!(vals[0], 7.0, "the left end of the zigzag must meet the keyframe");
    assert_eq!(*vals.last().unwrap(), 7.0, "the right end of the zigzag must meet the keyframe");
    // a fractional number of periods is legitimate, and the ends still meet
    let odd = SegShape::Zigzag { periods: 2.5, amplitude: 1.0, profile: ZigProfile::Sine };
    assert!((odd.at(0.0, 1.0, 2.0) - 1.0).abs() < 1e-12);
    assert!((odd.at(1.0, 1.0, 2.0) - 2.0).abs() < 1e-12);

    // --- the LEFT key carries the SHAPE of the segment ------------------
    let keys = vec![
        AutoKey::shaped(Tick(0), 0.0, SegShape::Hold),
        AutoKey::shaped(Tick(90_000), 10.0, SegShape::Linear),
        AutoKey::at(Tick(180_000), 20.0),
    ];
    assert_eq!(curve_value(&keys, Tick(45_000)), 0.0, "the first segment is a step");
    assert!(
        (curve_value(&keys, Tick(135_000)) - 15.0).abs() < 1e-9,
        "the second segment is straight, its shape is set by ITS OWN left keyframe"
    );
}

/// Test 695b (R.61): the default shape computes exactly the previous smoothing.
///
/// A guard of the cause, not of the consequence: the values are compared with the FROZEN
/// oracle of the previous revision. If they diverged even in the last bit, the committed
/// audio and video references would shift.
#[test]
fn t695b_default_shape_equals_the_frozen_smoothstep() {
    let cases: Vec<Vec<AutoKey>> = vec![
        vec![key(0, 0.0), key(90_000, 1.0)],
        vec![key(20_000, 10.0), key(80_000, 40.0), key(150_000, -5.0)],
        vec![key(50_000, 7.0)],
        vec![],
        vec![key(-90_000, 3.0), key(0, 9.0)],
    ];
    let probes = [
        Tick(-1_000_000),
        Tick(-1),
        Tick(0),
        Tick(1),
        Tick(19_999),
        Tick(35_000),
        Tick(45_000),
        Tick(50_000),
        Tick(80_000),
        Tick(90_000),
        Tick(5_000_000),
    ];
    let mut checked = 0usize;
    for keys in &cases {
        for t in probes {
            let now = curve_value(keys, t);
            let was = frozen_curve_value(keys, true, t);
            assert_eq!(
                now.to_bits(),
                was.to_bits(),
                "default shape diverged from the previous smoothing at {t:?}: {now} vs {was}"
            );
            // and the straight line must match the previous `ease = false`
            let straight: Vec<AutoKey> = AutoKey::linear(keys.clone());
            assert_eq!(
                curve_value(&straight, t).to_bits(),
                frozen_curve_value(keys, false, t).to_bits(),
                "the straight line diverged from the previous “no smoothing” at {t:?}"
            );
            checked += 1;
        }
    }
    assert!(checked >= 50, "too few probes to assert anything: {checked}");
}

/// Test 697 (R.61): the default shape lives in the PROJECT, not in the machine settings,
/// and is applied to new keys; “to all segments” is a command.
///
/// A machine setting would give exactly the class of `Lock Envelopes to Events`
/// in Vegas: one file, two behaviours.
#[test]
fn t697_default_shape_lives_in_the_project_and_applies_to_new_keys() {
    let zig = SegShape::Zigzag { periods: 2.0, amplitude: 0.5, profile: ZigProfile::Square };

    // “apply to all segments” — one model function, not UI
    let keys = vec![key(0, 0.0), key(90_000, 1.0), key(180_000, 2.0)];
    let all = AutoKey::all_shaped(keys.clone(), zig);
    assert!(all.iter().all(|k| k.shape == zig), "the shape did not apply to all segments");
    assert_eq!(all.len(), keys.len(), "the command has no right to change the number of keys");
    assert_eq!(
        all.iter().map(|k| (k.t, k.v.to_bits())).collect::<Vec<_>>(),
        keys.iter().map(|k| (k.t, k.v.to_bits())).collect::<Vec<_>>(),
        "a shape command has no right to move keyframes"
    );

    // old keys are not touched by changing the default: the default acts on
    // NEW ones, and this is verified by the constructor taking DEFAULT
    assert_eq!(AutoKey::at(Tick(0), 0.0).shape, SegShape::DEFAULT);
    assert!(SegShape::DEFAULT.is_default());
    assert!(!SegShape::Linear.is_default());

    // the project default travels IN THE FILE (a document field), not in the settings
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
    p.settings.default_shape = zig;
    let json = serde_json::to_string(&p.settings).expect("settings are serialized");
    assert!(json.contains("zigzag"), "the default shape did not leak into the document: {json}");
}

// ---- 700 (R.61). matching fields when pasting an envelope -----------------

/// Test 700 (R.61): pasting into a DIFFERENT kind of effect spreads the curves, and none
/// of them is lost silently.
///
/// Owner: “at least let remapping suggest fields”. Before R.61 the paste button on a
/// foreign kind simply went grey — remapping was not absent,
/// it was FORBIDDEN.
#[test]
fn t700_envelope_fits_into_another_effect_kind() {
    let curve = |p: &str| ParamCurve {
        param: p.into(),
        keys: vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(90_000), 8.0)],
        source: CurveSource::Keys,
    };
    let blur = FxInstance::Blur { radius: 8, radius_y: 0 };
    let names: Vec<String> = blur.param_specs().iter().map(|s| s.name.to_string()).collect();
    assert!(names.len() >= 2, "Blur has at least two parameters: {names:?}");

    // --- everything matched: no remainder, no window needs to be shown ------------
    let all: Vec<ParamCurve> = names.iter().map(|n| curve(n)).collect();
    let fit = kt_model::ops::fit_envelope(&all, &blur);
    assert_eq!(fit.matched.len(), all.len(), "matched ones did not land silently");
    assert!(fit.leftover.is_empty(), "remainder from nowhere: {:?}", fit.leftover);
    assert!(fit.free.is_empty(), "there must be no free ones — all are taken");

    // --- part did not match: it is NAMED, not lost -----------------
    let mixed = vec![curve(&names[0]), curve("no_such_field"), curve("and_this_too")];
    let fit = kt_model::ops::fit_envelope(&mixed, &blur);
    assert_eq!(fit.matched.len(), 1, "the matched one did not land");
    assert_eq!(fit.leftover.len(), 2, "unmatched ones must be named: {fit:?}");
    assert_eq!(
        fit.matched.len() + fit.leftover.len(),
        mixed.len(),
        "a curve got lost between the branches — exactly what must not happen"
    );
    assert!(
        fit.free.iter().any(|f| f == &names[1]),
        "free target parameter was not offered: {:?}",
        fit.free
    );
    assert!(!fit.free.iter().any(|f| f == &names[0]), "an occupied parameter was offered as free");

    // --- remapping changes the ADDRESS, not the content ----------------------
    let src = ParamCurve {
        param: "foreign".into(),
        keys: vec![
            AutoKey::shaped(Tick(0), 1.0, SegShape::Hold),
            AutoKey::shaped(Tick(45_000), 4.0, SegShape::Linear),
        ],
        source: CurveSource::Keys,
    };
    let moved = kt_model::ops::remap_curve(src.clone(), &names[1]);
    assert_eq!(moved.param, names[1], "the address did not change");
    assert_eq!(moved.keys, src.keys, "the matching touched keyframes or segment shapes");

    // --- insertion into the SAME kind still passes in full ------------------
    let same = kt_model::ops::fit_envelope(&all, &FxInstance::Blur { radius: 2, radius_y: 2 });
    assert!(same.leftover.is_empty(), "own kind left a remainder");
}

/// Test 747b (R.63 item 1): a speed curve from the PREVIOUS file sounds the same
/// as it did before R.61.
///
/// What is asserted is not “the shape is straight” (the reader would say that too) but
/// the CONSEQUENCE: the curve values and the stretch map match the piecewise-linear profile —
/// the very one by which the warp always computed, because it never read the `ease` field
/// at all. A scene guard: for a smoothed curve the value in the middle of the segment is
/// DIFFERENT, otherwise the test would catch nothing.
#[test]
fn t747b_rate_curve_from_old_file_sounds_piecewise_linear() {
    let wire = r#"{"param":"rate","ease":true,"keys":[{"t":0,"v":1.0},{"t":90000,"v":0.5}]}"#;
    let c: ParamCurve = serde_json::from_str(wire).expect("the previous curve reads");
    assert!(
        c.keys.iter().all(|k| k.shape == SegShape::Linear),
        "speed curve was read as not straight: {:?}",
        c.keys
    );

    // the reference — the same curve, written out explicitly as straight
    let linear = ParamCurve {
        param: "rate".into(),
        keys: vec![
            AutoKey::shaped(Tick(0), 1.0, SegShape::Linear),
            AutoKey::shaped(Tick(90_000), 0.5, SegShape::Linear),
        ],
        source: Default::default(),
    };
    // and the “smooth” variant — what the R.61 reader turned it into
    let smooth = ParamCurve {
        param: "x".into(),
        keys: vec![
            AutoKey::shaped(Tick(0), 1.0, SegShape::default()),
            AutoKey::shaped(Tick(90_000), 0.5, SegShape::default()),
        ],
        source: Default::default(),
    };
    for t in [0, 15_000, 22_500, 45_000, 67_500, 90_000] {
        assert_eq!(
            curve_value(&c.keys, Tick(t)),
            curve_value(&linear.keys, Tick(t)),
            "value at {t} diverged from the straight line"
        );
    }
    let mid = 22_500;
    assert!(
        (curve_value(&smooth.keys, Tick(mid)) - curve_value(&linear.keys, Tick(mid))).abs() > 1e-6,
        "scene degenerate: smoothed and straight curves coincide in the middle of the segment"
    );

    // the warp map: the profile and the position along it are computed from the STRAIGHT one
    let mut clip = ClipEvent {
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
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::slowed_reverb(1.0, 0.5, 120.0)],
        fx_bypass: vec![false],
        fx_auto: vec![vec![c.clone()]],
        opacity: 1.0,
        fx_off: false,
    };
    let from_old = clip_warp(&clip).expect("warp is present");
    clip.fx_auto = vec![vec![linear.clone()]];
    let from_linear = clip_warp(&clip).expect("warp is present");
    for u in [0.0, 0.25, 0.5, 0.75, 1.0] {
        assert_eq!(
            eff_rate(from_old.base(), from_old.prof(), u),
            eff_rate(from_linear.base(), from_linear.prof(), u),
            "speed at {u} diverged from the straight profile"
        );
    }
    assert_eq!(
        from_old.used(),
        from_linear.used(),
        "consumed material diverged: the warp map is not the one from before R.61"
    );
}

/// Test 787 (R.67): the SEGMENT FORMULA — a shape on the left key with its own variables
/// `s`, `a`, `b`, `t`; it computes in a closed and deterministic way, does not touch
/// neighbouring segments, and an uncomputable text is rejected loudly by validation —
/// it does not give a silent zero.
#[test]
fn t787_segment_formula_is_a_shape_with_its_own_variables() {
    let f = |expr: &str| SegShape::Formula { expr: ExprId::intern(expr) };
    // keys: 1.0 s (0) → 3.0 s (1) with the formula, neighbours — zigzag and straight
    let keys = vec![
        AutoKey::shaped(Tick(0), 5.0, SegShape::Zigzag { periods: 2.0, amplitude: 1.0, profile: ZigProfile::Triangle }),
        AutoKey::shaped(Tick(90_000), 0.0, f("a + (b - a) * s * s")),
        AutoKey::shaped(Tick(270_000), 1.0, SegShape::Linear),
        AutoKey::shaped(Tick(360_000), 3.0, SegShape::Linear),
    ];
    // --- the value on the segment — per the oracle -------------------------------
    for (frac, want) in [(0.0, 0.0), (0.25, 0.0625), (0.5, 0.25), (1.0, 1.0)] {
        let at = Tick(90_000 + (180_000.0 * frac) as i64);
        let got = curve_value(&keys, at);
        assert!((got - want).abs() < 1e-9, "s={frac}: {got} vs {want}");
    }
    // `t` — seconds from the LEFT key: sin(t·2π) goes to zero after one second
    let keys_t = vec![
        AutoKey::shaped(Tick(90_000), 0.0, f("sin(t * 6.283185307179586)")),
        AutoKey::shaped(Tick(270_000), 0.0, SegShape::Linear),
    ];
    assert!(curve_value(&keys_t, Tick(90_000 + 22_500)).abs() > 0.9, "a quarter second — peak");
    assert!(curve_value(&keys_t, Tick(180_000)).abs() < 1e-9, "a second — zero");
    // --- neighbours intact ----------------------------------------------------
    let zig = curve_value(&keys, Tick(22_500));
    let plain = 5.0 + (0.0 - 5.0) * 0.25;
    assert!((zig - plain).abs() > 0.1, "left zigzag is silent: {zig}");
    let lin = curve_value(&keys, Tick(315_000));
    assert!((lin - 2.0).abs() < 1e-9, "right straight line is broken: {lin}");
    // --- determinism ----------------------------------------------------
    let probe = || -> Vec<u64> { (0..=64).map(|i| curve_value(&keys, Tick(i * 5625)).to_bits()).collect() };
    assert_eq!(probe(), probe());
    // --- an uncomputable one is rejected loudly --------------------------------
    let bad = |expr: &str| ParamCurve {
        param: "amount".into(),
        keys: vec![AutoKey::shaped(Tick(0), 0.0, f(expr)), AutoKey::at(Tick(90_000), 1.0)],
        source: CurveSource::Keys,
    };
    let e = check_fx_auto(ClipId(1), &[vec![bad("foo(s)")]]).unwrap_err().to_string();
    assert!(e.contains("foo"), "name not named: {e}");
    let e = check_fx_auto(ClipId(1), &[vec![bad("s + zz")]]).unwrap_err().to_string();
    assert!(e.contains("zz") && e.contains("s, a, b, t"), "variable not named: {e}");
    let e = check_fx_auto(ClipId(1), &[vec![bad("(s")]]).unwrap_err().to_string();
    assert!(e.contains("bracket"), "bracket not named: {e}");
    assert!(check_fx_auto(ClipId(1), &[vec![bad("1 / s")]]).is_ok(), "division by zero at a point — not a parse rejection");
    // division by zero AT THE POINT gives the straight line between the ends, not zero
    let c = bad("1 / s");
    assert_eq!(curve_value(&c.keys, Tick(0)), 0.0);
    // an identical text — one shape
    assert_eq!(f("s * 2"), f("s * 2"));
    assert_ne!(f("s * 2"), f("s * 3"));
}

/// Test 787c (R.67): a LAYER formula over the keys. The keys and shapes stay intact,
/// `k` is their value; removing the layer returns exactly `k`; the link and the layer
/// combine, and removing the link does not touch the layer. A source formula from older
/// files (without keys) sounds as before: `k` = 0.
#[test]
fn t787c_layer_formula_over_keys_and_removal() {
    let keys = vec![
        AutoKey::shaped(Tick(0), 0.0, SegShape::Linear),
        AutoKey::shaped(Tick(90_000), 1.0, SegShape::Zigzag { periods: 3.0, amplitude: 0.2, profile: ZigProfile::Triangle }),
        AutoKey::shaped(Tick(180_000), 0.5, SegShape::Linear),
    ];
    let clip = |curve: ParamCurve| ClipEvent {
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
        duration: Tick(180_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::Blur { radius: 8, radius_y: 0 }],
        fx_bypass: vec![],
        fx_auto: vec![vec![curve]],
        opacity: 1.0,
        fx_off: false,
    };
    let step = 3000;
    let layered = ParamCurve {
        param: "radius".into(),
        keys: keys.clone(),
        source: CurveSource::Formula { expr: "k * 0.5 + 0.1".into() },
    };
    assert!(check_fx_auto(ClipId(1), &[vec![layered.clone()]]).is_ok(), "a layer over keyframes is legal");
    let clips = [clip(layered.clone())];
    let out = resolve_derived(&clips, step);
    let res = &out[0].fx_auto[0][0];
    assert!(res.source.is_keys() && !res.keys.is_empty(), "the layer was not expanded into keyframes");
    for i in (0..=60).map(|i| i * step) {
        let k = curve_value(&keys, Tick(i));
        let got = curve_value(&res.keys, Tick(i));
        assert!((got - (k * 0.5 + 0.1)).abs() < 1e-9, "tick {i}: {got} vs {}", k * 0.5 + 0.1);
    }
    // keys and zigzag in the MODEL are intact — the resolver worked on a copy
    assert_eq!(clips[0].fx_auto[0][0].keys, keys);
    // --- removing the layer: exactly k --------------------------------------------
    let plain = ParamCurve { source: CurveSource::Keys, ..layered.clone() };
    let plain_clips = [clip(plain)];
    let out = resolve_derived(&plain_clips, step);
    assert!(matches!(out, std::borrow::Cow::Borrowed(_)), "without a layer the resolver does nothing");
    // --- link + layer: k + src; removing the link leaves the layer -----------------
    let mut leader = clip(ParamCurve::constant("radius", Tick(0), Tick(180_000), 2.0, SegShape::Linear));
    leader.id = ClipId(2);
    let linked = ParamCurve {
        param: "radius".into(),
        keys: keys.clone(),
        source: CurveSource::Link { path: "2/fx0/radius".into(), expr: Some("k + src".into()), stops: vec![] },
    };
    let both = [clip(linked), leader.clone()];
    let out = resolve_derived(&both, step);
    let res = &out[0].fx_auto[0][0];
    let got = curve_value(&res.keys, Tick(90_000));
    assert!((got - (1.0 + 2.0)).abs() < 1e-9, "k + src did not add up: {got}");
    // --- a source formula without keys: k = 0 — the previous sound -----------
    let bare = ParamCurve { param: "radius".into(), keys: vec![], source: CurveSource::Formula { expr: "t * 2 + k".into() } };
    let bare_clips = [clip(bare)];
    let out = resolve_derived(&bare_clips, step);
    let v = curve_value(&out[0].fx_auto[0][0].keys, Tick(90_000));
    assert!((v - 2.0).abs() < 1e-9, "formula without keyframes: {v}");
    // --- a layer with a foreign name — a loud failure -----------------------------
    let bad = ParamCurve { source: CurveSource::Formula { expr: "k + q".into() }, ..layered };
    let e = check_fx_auto(ClipId(1), &[vec![bad]]).unwrap_err().to_string();
    assert!(e.contains("q") && e.contains("t, src, k"), "{e}");
}
