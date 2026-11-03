//! pitch-tail-choice: the pitch's echo choice in the project file — written only when it rings on,
//! and then the file is format 3, which an older build refuses instead of cutting the echo silently.

use kt_model::*;
use kt_time::Tick;

fn pitch(ring: bool) -> FxInstance {
    kt_plugins_builtin::install();
    let mut f = FxInstance::plugin_with("pitch_shift", &[5.0, 0.0]).unwrap();
    if let FxInstance::Plugin(p) = &mut f {
        assert!(p.set("echo_before", if ring { 1.0 } else { 0.0 }));
    }
    f
}

fn project(ring: bool) -> Project {
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
    p.media.push(MediaRef::new(MediaId(1), "synth:tone"));
    p.ensure_tracks(LaneKind::Audio, 1);
    p.ensure_tracks(LaneKind::Morph, 1);
    p.clips.push(ClipEvent {
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
        fx: vec![FxInstance::plugin_with("delay", &[100.0, 0.0, 0.5, 0.0]).unwrap(), pitch(ring)],
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
    let at = p.tracks.iter().position(|t| t.kind == LaneKind::Audio).unwrap();
    p.tracks[at].fx = vec![pitch(ring)];
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        targets: MorphTargets::Clips { ids: vec![ClipId(1)] },
        patch: MorphPatch { audio_fx: vec![pitch(ring)], ..Default::default() },
        timed: false,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    });
    p.validate().unwrap();
    p
}

/// t1336: "rings on" is format 3 with `"echo_before": "ring"` on each slot and reads back equal;
/// "cut" is format 1 without the row; a reader of format 2 refuses format 3 by its version.
#[test]
fn t1336_rings_on_is_format_3_cut_is_the_former_file() {
    let cut = project(false);
    let text = project_file::to_text(&cut, None);
    assert!(!text.contains("echo_before"), "\"cut\" grew a row");
    assert_eq!(project_file::version_for(&cut), project_file::PROJECT_FILE_VERSION);
    assert!(text.contains("\"kinetren\": 1"), "the former file is format 1");

    let rings = project(true);
    assert!(rings.has_ringing_echo());
    let text = project_file::to_text(&rings, None);
    assert_eq!(text.matches("\"echo_before\": \"ring\"").count(), 3, "each slot carries the choice:\n{text}");
    assert!(text.contains("\"kinetren\": 3"));
    let back = project_file::from_text(&text).unwrap().project;
    assert_eq!(back, rings);
    assert_eq!(project_file::to_text(&back, None), text, "the write is not idempotent");
    // only the event's own slot rings: still format 3 (the predicate walks every owner)
    let mut one = project(false);
    one.clips[0].fx[1] = pitch(true);
    assert_eq!(project_file::version_for(&one), 3);
    let mut one = project(false);
    one.morphers[0].patch.audio_fx = vec![pitch(true)];
    assert_eq!(project_file::version_for(&one), 3);

    // a build that reads up to 2 sees a version above its ceiling: the same check as `from_text`
    // (sim-audio-pairs raised the ceiling to 4: a version above THIS build's ceiling is refused)
    let above = project_file::PROJECT_FILE_READS + 1;
    let newer = text.replace("\"kinetren\": 3", &format!("\"kinetren\": {above}"));
    assert!(matches!(project_file::from_text(&newer), Err(project_file::ProjectFileError::TooNew { version }) if version == above));
    const { assert!(3 > project_file::PROJECT_FILE_VERSION_MASKS && project_file::PROJECT_FILE_READS >= 3) };
}
