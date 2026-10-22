//! Test 188 (R.29): camera events and camera tracks in the model.
//! The active-camera rule is deterministic (rendering depends on it),
//! commands are undoable, validate catches broken events.

use kt_model::*;
use kt_time::Tick;

fn cam(id: u64, lane: u32, start: i64, dur: i64) -> CameraEvent {
    CameraEvent {
        ease: true,
        id: CameraId(id),
        lane,
        start: Tick(start),
        duration: Tick(dur),
        name: String::new(),
        cam: CameraParams::default(),
        keyframes: vec![],
        auto: vec![],
    }
}

fn project() -> Project {
    Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    })
}

#[test]
fn t188_active_camera_rule() {
    let mut p = project();
    p.ensure_tracks(LaneKind::Camera, 2);
    // three cameras: time overlap on different tracks and on one
    p.cameras = vec![
        cam(1, 1, 0, 200_000),      // bottom track, early
        cam(2, 0, 50_000, 100_000), // top track — priority
        cam(3, 1, 100_000, 80_000), // same bottom track, later
    ];
    p.validate().unwrap();
    // outside all events — None (the engine takes the default “50 mm”)
    assert!(p.active_camera_at(Tick(300_000)).is_none());
    // only camera 1 covers it
    assert_eq!(p.active_camera_at(Tick(10_000)).unwrap().id, CameraId(1));
    // the smaller lane wins (like video z-order)
    assert_eq!(p.active_camera_at(Tick(120_000)).unwrap().id, CameraId(2));
    // with equal lane — the larger start (“on top is the one that comes later”, Vegas)
    assert_eq!(p.active_camera_at(Tick(160_000)).unwrap().id, CameraId(3));
    // bounds: start is included, end is excluded
    assert_eq!(p.active_camera_at(Tick(50_000)).unwrap().id, CameraId(2));
    assert_eq!(p.active_camera_at(Tick(150_000)).unwrap().id, CameraId(3));
    assert_eq!(p.active_camera_at(Tick(180_000)).unwrap().id, CameraId(1));
}

#[test]
fn t188_camera_commands_undo() {
    let mut m = TimelineModel::new(project());
    assert!(m.apply(Command::InsertTrack { kind: LaneKind::Camera, at: 0 }).is_ok());
    assert!(m.apply(Command::AddCamera(cam(1, 0, 0, 90_000))).is_ok());
    assert_eq!(m.project().lanes(LaneKind::Camera), 1);
    assert_eq!(m.project().cameras.len(), 1);
    // duplicate id — rejected
    assert!(m.apply(Command::AddCamera(cam(1, 0, 10, 100))).is_err());
    // parameter replacement
    let mut c2 = cam(1, 0, 0, 90_000);
    c2.cam.fov_deg = 60.0;
    c2.name = "wide".into();
    assert!(m.apply(Command::SetCamera(c2.clone())).is_ok());
    assert_eq!(m.project().cameras[0].cam.fov_deg, 60.0);
    // broken events do not pass
    let mut bad = cam(9, 0, 0, 0);
    assert!(m.apply(Command::AddCamera(bad.clone())).is_err(), "zero duration");
    bad.duration = Tick(100);
    bad.cam.fov_deg = 200.0;
    assert!(m.apply(Command::AddCamera(bad.clone())).is_err(), "fov outside (0,180)");
    bad.cam.fov_deg = 50.0;
    bad.keyframes = vec![
        CameraKeyframe { t: 0.9, cam: CameraParams::default() },
        CameraKeyframe { t: 0.1, cam: CameraParams::default() },
    ];
    assert!(m.apply(Command::AddCamera(bad)).is_err(), "keyframes not sorted");
    // deleting a track takes its events with it; undo brings everything back
    assert!(m.apply(Command::RemoveTrack { kind: LaneKind::Camera, at: 0 }).is_ok());
    assert_eq!(m.project().lanes(LaneKind::Camera), 0);
    assert!(m.project().cameras.is_empty());
    m.undo().unwrap();
    assert_eq!(m.project().cameras.len(), 1);
    assert_eq!(m.project().cameras[0].name, "wide");
    m.undo().unwrap();
    assert_eq!(m.project().cameras[0].name, "");
}

#[test]
fn t188_project_without_cameras_is_byte_stable() {
    // serializing a project without cameras does not grow new fields
    let p = project();
    let json = serde_json::to_string(&p).unwrap();
    assert!(!json.contains("camera"), "empty camera fields are not written: {json}");
    // and with cameras — the round trip is honest
    let mut p2 = project();
    p2.ensure_tracks(LaneKind::Camera, 1);
    p2.cameras.push(cam(1, 0, 0, 90_000));
    let json2 = serde_json::to_string(&p2).unwrap();
    let back: Project = serde_json::from_str(&json2).unwrap();
    assert_eq!(back, p2);
}

/// Test 231 (R.34 K3): the camera is animated by the COMMON mechanism. Keyframes
/// of the old domain (fraction of the event) are converted into curves (ticks from the event start),
/// curves are evaluated by `params_at`, moving the camera carries the animation
/// along, and a broken curve fails validation instead of passing silently.
#[test]
fn t231_camera_curves_replace_keyframes() {
    let mut p = project();
    p.ensure_tracks(LaneKind::Camera, 1);
    let mut c = cam(1, 0, 90_000, 90_000);
    c.ease = false;
    c.keyframes = vec![
        CameraKeyframe { t: 0.0, cam: CameraParams { pos_x: 0.0, ..Default::default() } },
        CameraKeyframe {
            t: 1.0,
            cam: CameraParams { pos_x: 100.0, fov_deg: 39.6, ..Default::default() },
        },
    ];
    let mut legacy = c.clone();
    assert!(kt_model::migrate_legacy_camera(&mut legacy), "the migration must fire");
    assert!(legacy.keyframes.is_empty(), "legacy keyframes consumed");
    // flat parameter curves are not created: seven straight lines in the editor is garbage
    assert_eq!(legacy.auto.len(), 1, "curve only on the changed one: {:?}", legacy.auto);
    let px = &legacy.auto[0];
    assert_eq!(px.param, "pos_x");
    assert_eq!(
        px.keys.iter().map(|k| (k.t.0, k.v)).collect::<Vec<_>>(),
        vec![(0, 0.0), (90_000, 100.0)],
        "event share converted to ticks FROM THE START"
    );

    // values match the old mechanism at the same points
    for (t, want) in [(90_000i64, 0.0), (135_000, 50.0), (180_000, 100.0)] {
        let old = c.params_at(Tick(t)).pos_x;
        let new = legacy.params_at(Tick(t)).pos_x;
        assert!((old - want).abs() < 1e-9, "old mechanism at {t}: {old}");
        assert!((new - want).abs() < 1e-9, "new mechanism at {t}: {new}");
    }

    // MOVING the camera carries the animation along (the domain starts at the event start)
    let mut moved = legacy.clone();
    moved.start = Tick(270_000);
    assert!(
        (moved.params_at(Tick(315_000)).pos_x - 50.0).abs() < 1e-9,
        "animation midpoint moved together with the camera"
    );

    // a curve on a nonexistent parameter — validation failure, not silence
    p.cameras = vec![legacy.clone()];
    p.validate().expect("camera with curves is valid");
    p.cameras[0].auto[0].param = "no_such".into();
    assert!(p.validate().is_err(), "an orphan curve must fail the load");
    p.cameras[0].auto[0].param = "pos_x".into();
    p.cameras[0].auto[0].keys[0].t = Tick(-5);
    assert!(p.validate().is_err(), "keyframe before the start of the event — rejection");

    // curves of the NEW domain take precedence over legacy keyframes
    let mut both = c.clone();
    both.auto = vec![ParamCurve {
        param: "pos_x".into(),

        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 7.0)]),
        source: Default::default(),
    }];
    assert!(!kt_model::migrate_legacy_camera(&mut both), "migration does not touch finished curves");
    assert!((both.params_at(Tick(135_000)).pos_x - 7.0).abs() < 1e-9);
}
