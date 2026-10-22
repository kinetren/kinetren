//! Test 716 (R.62, change frame-conform-and-event-geometry): the leading “Geometry” node —
//! a field of the event with the position in the chain; the additional node is an ordinary
//! slot; `Pan` is removed from the registry; a node with defaults is identical.

use kt_model::*;
use kt_time::Tick;

fn project_with_video() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1920,
        height: 1080,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
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
        fx: vec![FxInstance::blur(8), FxInstance::Mirror { horizontal: true, vertical: false }],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

/// Registry: `pan` removed, `geometry` present; a node with defaults is the identity,
/// the leading flag lives only on the node materialized from the event.
#[test]
fn t716_registry_and_identity() {
    let tags: Vec<&str> = FxInstance::all().iter().map(|f| f.tag()).collect();
    assert!(!tags.contains(&"pan"), "Pan abolished (R.62)");
    assert!(tags.contains(&"geometry"));
    let node = FxInstance::geometry_node();
    assert!(!node.is_lead_geometry());
    assert!(node.geometry_is_identity());
    let lead = EventGeometry::default().node();
    assert!(lead.is_lead_geometry());
    assert!(lead.geometry_is_identity());
    assert!(!FxInstance::geometry_zoom(1.5).geometry_is_identity());
    assert!(!FxInstance::geometry_at(0.1, 0.0, 1.0).geometry_is_identity());
    // passport: ten numeric rows, all automatable
    assert_eq!(node.param_specs().len(), 10);
    assert!(node.param_specs().iter().all(|s| s.kind == ParamKind::Smooth));
    // the round trip event ↔ node
    let g = EventGeometry { rule: Conform::Fill, zoom: 2.0, rot: 30.0, ..Default::default() };
    let back = EventGeometry { rule: Conform::Fill, ..Default::default() }.with_node(&g.node());
    assert_eq!(back, g);
    assert_eq!(Conform::Project.resolve(ConformRule::AsIs), ConformRule::AsIs);
    assert_eq!(Conform::Fill.resolve(ConformRule::AsIs), ConformRule::Fill);
}

/// The `SetGeometry` command: the rule, the position and the curves reach the model in one
/// history step; a position past the end of the chain is a failure; there is nothing to
/// delete on the node — the event field always exists.
#[test]
fn t716_set_geometry_command_and_validation() {
    let mut m = TimelineModel::new(project_with_video());
    let g = EventGeometry { rule: Conform::Fill, pos_x: 0.1, ..Default::default() };
    let auto = vec![ParamCurve {
        param: "zoom".into(),
        keys: vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(45_000), 1.5)],
        source: Default::default(),
    }];
    m.apply(Command::SetGeometry { id: ClipId(1), geometry: g, pos: 1, auto: auto.clone() })
        .expect("a legal command");
    let c = m.clip(ClipId(1)).unwrap();
    assert_eq!(c.geometry, g);
    assert_eq!(c.geometry_pos, 1);
    assert_eq!(c.geometry_auto, auto);
    assert_eq!(c.fx.len(), 2, "the chain of the model does not contain the node");
    // a position past the end of the chain — failure, the model untouched
    let err = m.apply(Command::SetGeometry { id: ClipId(1), geometry: g, pos: 5, auto: vec![] });
    assert!(err.is_err(), "a position past the end of the chain is rejected");
    assert_eq!(m.clip(ClipId(1)).unwrap().geometry_pos, 1);
    // undo restores the default
    m.undo().unwrap();
    let c = m.clip(ClipId(1)).unwrap();
    assert!(c.geometry.is_default());
    assert_eq!(c.geometry_pos, 0);
    // shortening the chain pins the node’s position
    m.apply(Command::SetGeometry { id: ClipId(1), geometry: g, pos: 2, auto: vec![] }).unwrap();
    m.apply(Command::SetFx { id: ClipId(1), fx: vec![FxInstance::blur(4)] }).unwrap();
    assert!(m.project().validate().is_ok(), "validation after shortening");
}

/// The project rule is ONLY a rule, not “follow the project”: a file where the setting
/// reads “project” is rejected by validation.
#[test]
fn t716_project_rule_is_a_rule() {
    let mut p = project_with_video();
    p.settings.conform = ConformRule::Fill;
    assert!(p.validate().is_ok());
    // serialization: the default is not written, a non-default is written
    let js = serde_json::to_string(&p.settings).unwrap();
    assert!(js.contains("\"conform\":\"fill\""));
    p.settings.conform = ConformRule::Fit;
    let js = serde_json::to_string(&p.settings).unwrap();
    assert!(!js.contains("conform"), "the “fit” default is not serialized");
    // a leading node with defaults is not serialized on the event
    let js = serde_json::to_string(&p.clips[0]).unwrap();
    assert!(!js.contains("geometry"));
}

/// CamMotion (legacy) is read into the “Geometry” node with position and zoom curves;
/// the pan of the former frame is converted into position.
#[test]
fn t716_cammotion_migrates_to_geometry() {
    let mut p = project_with_video();
    p.clips[0].fx = vec![FxInstance::CamMotion {
        mode: PanMode::Fill,
        ease: true,
        time_scale: 1.0,
        keyframes: vec![
            CamKeyframe { t: 0.0, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 },
            CamKeyframe { t: 1.0, pan_x: 1.0, pan_y: 0.0, zoom: 2.0 },
        ],
    }];
    assert!(migrate_legacy_cammotion(&mut p.clips[0]));
    assert!(matches!(p.clips[0].fx[0], FxInstance::Geometry { lead: false, .. }));
    let names: Vec<&str> = p.clips[0].fx_auto[0].iter().map(|c| c.param.as_str()).collect();
    assert!(names.contains(&"pos_x") && names.contains(&"zoom"));
    assert!(!names.contains(&"pan_x"));
    // pan 1 at zoom 2 → a shift of a quarter of the canvas to the left
    assert!((pan_to_pos(1.0, 2.0) + 0.25).abs() < 1e-9);
    assert_eq!(pan_to_pos(1.0, 1.0), 0.0, "without zoom, pan has nowhere to move the previous frame");
}
