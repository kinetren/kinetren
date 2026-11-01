//! shape-masks: the masks pass through the OTIO door in the `kt` envelope (v14 by content).

use kt_model::*;
use kt_time::Tick;

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
    p.clips.push(ClipEvent {
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
        fx: vec![FxInstance::plugin_with("blur", &[8.0, 0.0]).unwrap()],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
    });
    p.validate().unwrap();
    p
}

/// Test 1265: masks of the picture, a slot and a track come back from an `.otio` export; the
/// envelope is v14 only when there are masks.
#[test]
fn t1265_masks_through_the_otio_door() {
    let plain = project();
    let text0 = kt_otio::save_project(&plain, &Default::default(), None);
    let v0 = kt_otio::kt_version(&kt_otio::parse_document(&text0).unwrap());
    assert_ne!(v0, Some(14), "a maskless file was raised to v14");
    assert!(!text0.contains("\"masks\""));

    let mut p = plain.clone();
    let mut m = Mask::new(1, "Face", MaskShape::Ellipse { cx: 0.4, cy: 0.5, w: 0.3, h: 0.4, rot: 10.0 });
    m.feather = 8.0;
    m.auto.push(ParamCurve {
        param: "cx".into(),
        keys: vec![AutoKey::shaped(Tick(0), 0.4, SegShape::Linear), AutoKey::shaped(Tick(45_000), 0.6, SegShape::Linear)],
        source: CurveSource::Keys,
    });
    p.clips[0].masks.picture.push(m.clone());
    p.clips[0].masks.slots = vec![vec![Mask { name: "Slot".into(), ..m.clone() }]];
    let vt = p.tracks.iter().position(|t| t.kind == LaneKind::Video).unwrap();
    p.tracks[vt].masks.push(Mask { auto: vec![], ..m });
    let text = kt_otio::save_project(&p, &Default::default(), None);
    assert_eq!(kt_otio::kt_version(&kt_otio::parse_document(&text).unwrap()), Some(14));
    let back = kt_otio::load_project(&text, None).unwrap().project;
    assert_eq!(back.clips[0].masks, p.clips[0].masks);
    assert_eq!(back.tracks[vt].masks, p.tracks[vt].masks);
}
