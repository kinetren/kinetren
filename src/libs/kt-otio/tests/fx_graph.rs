//! fx-graph-dag D7 (capability `project-format`): an effect graph through the OTIO door — the chain
//! stays an ordered `Effect.1` list, the wiring rides in each effect's `kt` envelope (`in`), a Merge
//! is `kt:merge`, and the envelope is v18 only when a chain is a graph.

use kt_model::fx_graph::{FxIn, SlotInputs};
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
        fx: vec![FxInstance::plugin("blur").unwrap(), FxInstance::plugin("glow").unwrap()],
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
        fx_in: Vec::new(),
    });
    p.validate().unwrap();
    p
}

fn merge() -> FxInstance {
    FxInstance::Merge { mode: MergeMode::Blend(BlendMode::Screen), mix: 0.75 }
}

fn graph() -> Vec<SlotInputs> {
    vec![
        SlotInputs::default(),
        SlotInputs { main: FxIn::Source, ..Default::default() },
        SlotInputs { main: FxIn::Prev, second: Some(FxIn::Slot(0)), matte: None },
    ]
}

/// Test t3211 (fx-graph-dag D7): the branching project exported to `.otio` and imported: the effects
/// are an ordered `Effect.1` list with `kt:merge`, the envelope is v18, and the chains of the event
/// and of the track come back equal with their inputs; a line project is not raised.
#[test]
fn t3211_the_otio_door_keeps_the_wiring() {
    let line = project();
    let text0 = kt_otio::save_project(&line, &Default::default(), None);
    assert_ne!(kt_otio::kt_version(&kt_otio::parse_document(&text0).unwrap()), Some(18), "a line was raised to v18");
    assert!(!text0.contains("\"in\""), "a line wrote the wiring");

    let mut p = line.clone();
    p.clips[0].fx.push(merge());
    p.clips[0].fx_in = graph();
    let vt = p.tracks.iter().position(|t| t.kind == LaneKind::Video).unwrap();
    p.tracks[vt].fx = p.clips[0].fx.clone();
    p.tracks[vt].fx_in = graph();
    p.validate().unwrap();
    let text = kt_otio::save_project(&p, &Default::default(), None);
    assert_eq!(kt_otio::kt_version(&kt_otio::parse_document(&text).unwrap()), Some(18));
    assert!(text.contains("\"effect_name\": \"kt:merge\""), "no kt:merge effect");
    let back = kt_otio::load_project(&text, None).unwrap().project;
    assert_eq!((&back.clips[0].fx, &back.clips[0].fx_in), (&p.clips[0].fx, &p.clips[0].fx_in), "the event's graph");
    assert_eq!((&back.tracks[vt].fx, &back.tracks[vt].fx_in), (&p.tracks[vt].fx, &p.tracks[vt].fx_in), "the track's graph");
}
