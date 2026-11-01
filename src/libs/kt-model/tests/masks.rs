//! shape-masks: the mask model — passport, curves, the path keyed as a whole, links,
//! validation, commands, the file version by content.

use kt_model::*;
use kt_time::Tick;

const SEC: i64 = 90_000;
const FRAME: i64 = 3000;

fn clip(id: u64, kind: ClipKind) -> ClipEvent {
    kt_plugins_builtin::install();
    ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind,
        lane: 0,
        start: Tick(0),
        duration: Tick(2 * SEC),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::plugin_with("blur", &[8.0, 0.0]).unwrap()],
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
        masks: Default::default(),
    }
}

fn project() -> Project {
    kt_plugins_builtin::install();
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
    p.ensure_tracks(LaneKind::Audio, 1);
    p.clips.push(clip(1, ClipKind::Video));
    p.validate().unwrap();
    p
}

fn rect(id: MaskId) -> Mask {
    Mask::new(id, format!("Mask {id}"), MaskShape::Rect { cx: 0.5, cy: 0.5, w: 0.4, h: 0.3, rot: 0.0, round: 0.0 })
}

fn tri(id: MaskId) -> Mask {
    Mask::new(
        id,
        format!("Mask {id}"),
        MaskShape::Path {
            verts: vec![
                Vertex::corner(0, 0.2, 0.2),
                Vertex::corner(1, 0.8, 0.2),
                Vertex { id: 2, x: 0.5, y: 0.8, ix: 0.1, iy: 0.0, ox: -0.1, oy: 0.0 },
            ],
        },
    )
}

fn keys(param: &str, k: &[(i64, f64)]) -> ParamCurve {
    ParamCurve {
        param: param.into(),
        keys: k.iter().map(|&(t, v)| AutoKey::shaped(Tick(t), v, SegShape::Linear)).collect(),
        source: CurveSource::Keys,
    }
}

/// Test 1260: masks of the picture, a slot and a track round-trip through the project file; a
/// maskless project's text has no trace of them; every malformed list is refused by name.
#[test]
fn t1260_masks_round_trip_validate_and_leave_maskless_files_alone() {
    let plain = project();
    let text0 = project_file::to_text(&plain, None);
    assert!(!text0.contains("masks"), "a maskless project grew a field");

    let mut p = plain.clone();
    let mut m = tri(1);
    m.feather = 12.0;
    m.invert = true;
    m.mode = MaskMode::Subtract;
    m.auto.push(keys("feather", &[(0, 4.0), (SEC, 20.0)]));
    p.clips[0].masks.picture.push(m);
    p.clips[0].masks.slots = vec![vec![rect(1)]];
    let vt = p.tracks.iter().position(|t| t.kind == LaneKind::Video).unwrap();
    p.tracks[vt].masks.push(rect(7));
    p.validate().unwrap();
    let text = project_file::to_text(&p, None);
    let back = project_file::from_text(&text).unwrap().project;
    assert_eq!(back.clips[0].masks, p.clips[0].masks);
    assert_eq!(back.tracks[vt].masks, p.tracks[vt].masks);
    assert_eq!(project_file::to_text(&back, None), text, "the write is not idempotent");

    // malformed lists — each refused with its own name
    let bad = |f: &dyn Fn(&mut Project)| {
        let mut q = plain.clone();
        f(&mut q);
        q.validate().unwrap_err()
    };
    assert!(matches!(
        bad(&|q| q.clips[0].masks.picture = vec![rect(1), rect(1)]),
        ProjectError::BadMasks(_, MaskError::DuplicateId(1))
    ));
    assert!(matches!(
        bad(&|q| {
            let mut a = rect(2);
            a.name = "Mask 1".into();
            q.clips[0].masks.picture = vec![rect(1), a];
        }),
        ProjectError::BadMasks(_, MaskError::DuplicateName(_))
    ));
    assert!(matches!(
        bad(&|q| q.clips[0].masks.picture = vec![Mask::new(
            1,
            "x",
            MaskShape::Path { verts: vec![Vertex::corner(0, 0.0, 0.0), Vertex::corner(1, 1.0, 1.0)] }
        )]),
        ProjectError::BadMasks(_, MaskError::TooFewVertices(1))
    ));
    assert!(matches!(
        bad(&|q| {
            let mut a = rect(1);
            a.auto.push(keys("v9.x", &[(0, 0.0)]));
            q.clips[0].masks.picture = vec![a];
        }),
        ProjectError::BadMasks(_, MaskError::OrphanCurve(1, _))
    ));
    assert!(matches!(
        bad(&|q| q.clips[0].masks.slots = vec![vec![], vec![rect(1)]]),
        ProjectError::MasksPastChain(_)
    ));
    assert!(matches!(
        bad(&|q| {
            let mut a = clip(2, ClipKind::Audio);
            a.fx.clear();
            a.fx_bypass.clear();
            a.fx_auto.clear();
            a.masks.picture.push(rect(1));
            q.clips.push(a);
        }),
        ProjectError::MasksOnAudio(_)
    ));
    assert!(matches!(
        bad(&|q| {
            let mut a = rect(1);
            a.auto.push(keys("feather", &[(0, 1.0), (SEC, 2.0)]));
            q.tracks[vt].masks = vec![a];
        }),
        ProjectError::TrackCurvesUnsupported(_)
    ));
}

/// Test 1261: the passport follows the shape; curves are evaluated at the material tick; a vertex
/// id is a stable address.
#[test]
fn t1261_passport_rows_evaluation_and_stable_vertex_ids() {
    let r = rect(1);
    let names: Vec<String> = r.specs().iter().map(|s| s.name.to_string()).collect();
    assert_eq!(names, ["opacity", "feather", "expansion", "cx", "cy", "w", "h", "rot", "round"]);
    let mut p = tri(1);
    let names: Vec<String> = p.specs().iter().map(|s| s.name.to_string()).collect();
    assert!(names.contains(&"v2.ox".to_string()) && names.len() == 3 + 3 * 6, "{names:?}");
    // the passport clamps
    assert!(p.set("feather", 5000.0));
    assert_eq!(p.feather, FEATHER_MAX);
    assert!(!p.set("v9.x", 0.1), "a missing vertex row was accepted");

    p.feather = 0.0;
    p.auto.push(keys("feather", &[(0, 0.0), (SEC, 10.0)]));
    p.auto.push(keys("v2.x", &[(0, 0.5), (SEC, 0.7)]));
    let mid = p.at(Tick(SEC / 2));
    assert!((mid.feather - 5.0).abs() < 1e-9);
    assert!((mid.get("v2.x").unwrap() - 0.6).abs() < 1e-9);
    assert!(mid.auto.is_empty(), "an evaluated mask carries curves");

    // deleting vertex 1 keeps v2's name and curve
    let mut p4 = p.clone();
    if let MaskShape::Path { verts } = &mut p4.shape {
        verts.push(Vertex::corner(3, 0.1, 0.6));
    }
    assert!(p4.remove_vertex(1));
    assert!(p4.auto.iter().any(|c| c.param == "v2.x"));
    assert!((p4.at(Tick(SEC)).get("v2.x").unwrap() - 0.7).abs() < 1e-9);
    // a path keeps three
    assert!(!p.remove_vertex(0), "a triangle lost a vertex");
}

/// Test 1275: the stopwatch keys every shape row; a gesture on an animated path keys the whole
/// path; an inserted vertex is keyed so the shape at every key is unchanged.
#[test]
fn t1275_the_path_is_keyed_as_a_whole_and_insertion_keeps_the_shape() {
    let mut m = tri(1);
    let at0 = Tick(0);
    let snapshot = m.clone();
    m.key_path(at0, &snapshot, SegShape::Linear);
    let rows = m.shape_rows();
    assert_eq!(rows.len(), 18);
    assert!(rows.iter().all(|r| m.auto.iter().any(|c| &c.param == r && c.keys.len() == 1)));
    assert!(m.path_animated());

    // a gesture at t1 moves vertex 0 only — every shape row gets a key at t1
    let t1 = Tick(SEC);
    let mut edited = m.at(t1);
    edited.set("v0.x", 0.1);
    m.write_gesture(t1, &edited, SegShape::Linear);
    for r in &rows {
        let c = m.auto.iter().find(|c| &c.param == r).unwrap();
        assert_eq!(c.keys.len(), 2, "{r} has no key at the new tick");
    }
    assert!((m.at(t1).get("v0.x").unwrap() - 0.1).abs() < 1e-12);
    assert!((m.at(at0).get("v0.x").unwrap() - 0.2).abs() < 1e-12, "the old key moved");
    assert_eq!(m.at(t1).get("v1.x"), Some(0.8), "an untouched vertex moved");

    // insertion into segment 2 (vertex 2 → vertex 0, a curved one)
    let shape_at = |m: &Mask, t: Tick| m.at(t).cubics(100.0, 100.0);
    let point = |seg: &[(f64, f64); 4], u: f64| {
        let v = 1.0 - u;
        let f = |i: usize| match i {
            0 => seg[0],
            1 => seg[1],
            2 => seg[2],
            _ => seg[3],
        };
        let (a, b, c, d) = (f(0), f(1), f(2), f(3));
        (
            v * v * v * a.0 + 3.0 * v * v * u * b.0 + 3.0 * v * u * u * c.0 + u * u * u * d.0,
            v * v * v * a.1 + 3.0 * v * v * u * b.1 + 3.0 * v * u * u * c.1 + u * u * u * d.1,
        )
    };
    let before: Vec<_> = [at0, t1].iter().map(|&t| shape_at(&m, t)).collect();
    let id = m.insert_vertex(2, 0.5, SegShape::Linear).unwrap();
    assert_eq!(id, 3);
    for (k, &t) in [at0, t1].iter().enumerate() {
        let after = shape_at(&m, t);
        assert_eq!(after.len(), 4);
        // the old segment 2 at u = 0.25 and 0.75 lies on the two new halves at 0.5
        let old = &before[k][2];
        for (u_old, seg, u_new) in [(0.25, &after[2], 0.5), (0.75, &after[3], 0.5)] {
            let a = point(old, u_old);
            let b = point(seg, u_new);
            assert!((a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9, "the shape changed at key {k}");
        }
    }
    assert!(m.auto.iter().any(|c| c.param == "v3.x" && c.keys.len() == 2), "the new vertex is not keyed");
    check_masks(std::slice::from_ref(&m)).unwrap();
}

/// Test 1262: the commands are one undo step each; `SetFx` trims the slot masks; a copied slot
/// carries its masks with keys re-hung on the target's entry.
#[test]
fn t1262_commands_undo_trim_and_the_slot_clipboard() {
    let mut m = TimelineModel::new(project());
    let id = ClipId(1);
    let mut em = EventMasks::default();
    em.picture.push(rect(1));
    em.slots = vec![vec![tri(1)]];
    m.apply(Command::SetMasks { id, masks: em.clone() }).unwrap();
    assert_eq!(m.clip(id).unwrap().masks, em);
    m.undo().unwrap();
    assert!(m.clip(id).unwrap().masks.is_empty());
    m.redo().unwrap();

    // an invalid list is refused before the undo snapshot
    let depth = m.undo_depth();
    let mut bad = em.clone();
    bad.picture.push(rect(1));
    assert!(m.apply(Command::SetMasks { id, masks: bad }).is_err());
    assert_eq!(m.undo_depth(), depth);

    // the chain shortened → the slot masks go with the slot
    m.apply(Command::SetFx { id, fx: vec![] }).unwrap();
    let c = m.clip(id).unwrap();
    assert!(c.masks.slots.is_empty() && c.masks.picture.len() == 1);

    // track masks: one step; a curve is refused loudly
    let vt = m.project().tracks.iter().find(|t| t.kind == LaneKind::Video).unwrap().id;
    m.apply(Command::SetTrackMasks { id: vt, masks: vec![rect(1)] }).unwrap();
    let mut animated = rect(1);
    animated.auto.push(keys("feather", &[(0, 0.0), (SEC, 3.0)]));
    let e = m.apply(Command::SetTrackMasks { id: vt, masks: vec![animated] }).unwrap_err();
    assert!(e.to_string().contains("curves on a track"), "{e}");

    // the clipboard: a slot with masks pasted into an event entering at 1 s
    let mut src = tri(1);
    src.auto.push(keys("feather", &[(0, 0.0), (SEC / 2, 3.0)]));
    let block = ops::FxChainBlock {
        slots: vec![ops::FxSlotBlock {
            fx: FxInstance::plugin_with("blur", &[2.0, 0.0]).unwrap(),
            bypass: false,
            curves: vec![],
            masks: vec![src.clone()],
        }],
        src_in: Tick(0),
        fx_off: false,
    };
    let json = serde_json::to_string(&block).unwrap();
    let back: ops::FxChainBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(back, block);
    let old: ops::FxChainBlock =
        serde_json::from_str(&json.replace(&format!(",\"masks\":{}", serde_json::to_string(&back.slots[0].masks).unwrap()), ""))
            .unwrap();
    assert!(old.slots[0].masks.is_empty(), "older clipboard text does not read");
    let mut masks = EventMasks::default();
    ops::append_chain_masks(&mut masks, 2, &block, Tick(SEC));
    assert_eq!(masks.slots.len(), 3);
    assert_eq!(masks.slot(2)[0].auto[0].keys[1].t, Tick(SEC + SEC / 2), "keys were not re-hung");
}

/// Test 1263: a mask row is a link target and a link source, a formula resolves, a cycle through
/// a mask is refused with its path.
#[test]
fn t1263_mask_rows_link_both_ways_and_cycles_are_refused() {
    let mut lead = clip(1, ClipKind::Video);
    lead.fx_auto[0].push(keys("radius", &[(0, 2.0), (2 * SEC, 6.0)]));
    let mut m = tri(1);
    m.auto.push(ParamCurve {
        param: "v0.x".into(),
        keys: Vec::new(),
        source: CurveSource::Link { path: "self/fx0/radius".into(), expr: Some("src / 10".into()), stops: Vec::new() },
    });
    lead.masks.picture.push(m);
    let mut slot = rect(4);
    slot.auto.push(ParamCurve { param: "feather".into(), keys: Vec::new(), source: CurveSource::Formula { expr: "t * 10".into() } });
    lead.masks.slots = vec![vec![slot]];
    // another event reads the mask point
    let mut other = clip(2, ClipKind::Video);
    other.fx_auto[0].push(ParamCurve {
        param: "radius".into(),
        keys: Vec::new(),
        source: CurveSource::Link { path: "1/mask1/v0.x".into(), expr: Some("src * 100".into()), stops: Vec::new() },
    });
    for txt in ["self/mask1/v0.x", "3/fx2.mask5/feather"] {
        assert_eq!(LinkPath::parse(txt).unwrap().to_text(), txt);
    }
    let clips = vec![lead, other];
    let out = resolve_derived(&clips, FRAME);
    let v = &out[0].masks.picture[0].auto[0];
    assert!(v.source.is_keys(), "the mask link did not resolve");
    let mid = curve_value(&v.keys, Tick(SEC));
    assert!((mid - 0.4).abs() < 1e-6, "the mask point does not follow its leader: {mid}");
    let f = &out[0].masks.slots[0][0].auto[0];
    assert!((curve_value(&f.keys, Tick(SEC)) - 10.0).abs() < 1e-6, "the formula on a mask row");
    let o = &out[1].fx_auto[0][0];
    assert!(o.source.is_keys());
    assert!((curve_value(&o.keys, Tick(SEC)) - 40.0).abs() < 1e-3, "a mask row as a source");

    // a cycle: the mask point leads the radius and the radius leads the point
    let mut p = project();
    let mut c = clip(1, ClipKind::Video);
    c.fx_auto[0].push(ParamCurve {
        param: "radius".into(),
        keys: Vec::new(),
        source: CurveSource::Link { path: "self/mask1/v0.x".into(), expr: None, stops: Vec::new() },
    });
    c.masks.picture.push(clips[0].masks.picture[0].clone());
    p.clips = vec![c];
    let e = p.validate().unwrap_err();
    assert!(e.to_string().contains("mask1/v0.x"), "the cycle's path is not named: {e}");
}

/// Test 1264: the file version is set by content.
#[test]
fn t1264_the_file_version_is_set_by_content() {
    let p = project();
    let t1 = project_file::to_text(&p, None);
    assert!(t1.contains("\"kinetren\": 1"));
    let mut q = p.clone();
    q.clips[0].masks.picture.push(rect(1));
    let t2 = project_file::to_text(&q, None);
    assert!(t2.contains("\"kinetren\": 2"), "a file with masks is not format 2");
    assert_eq!(project_file::from_text(&t2).unwrap().project.clips[0].masks, q.clips[0].masks);
    let t3 = t2.replace("\"kinetren\": 2", "\"kinetren\": 3");
    assert!(matches!(project_file::from_text(&t3), Err(project_file::ProjectFileError::TooNew { version: 3 })));
}
