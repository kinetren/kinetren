//! Test 545 (docs/porting-notes.md, item 429, R.54): the rule for resolving the
//! 3D camera — ONE for the whole program.
//!
//! Before R.54 it lived inline in the private `layer_src`, and the second consumer
//! (the picker's thumbnails) physically could not call it — only rewrite it.
//! A rewritten rule would silently diverge: the `Pan 3D` variant in the picker
//! would look different from the same variant after a click.

use kt_engine::resolve_cam3d;
use kt_model::{CameraEvent, CameraId, CameraParams, FxInstance};
use kt_time::Tick;

fn cam(id: u64, lane: u32, start: i64, dur: i64, rot_y: f64) -> CameraEvent {
    CameraEvent {
        ease: true,
        id: CameraId(id),
        lane,
        start: Tick(start),
        duration: Tick(dur),
        name: String::new(),
        cam: CameraParams { rot_y, ..CameraParams::default() },
        keyframes: vec![],
        auto: vec![],
    }
}

fn pan3d(camera: Option<CameraId>) -> FxInstance {
    FxInstance::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 0.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale: 1.0,
        camera,
    }
}

#[test]
fn t545_camera_resolution_has_one_rule() {
    let cams = [cam(1, 0, 0, 90_000, 11.0), cam(2, 0, 90_000, 90_000, 22.0)];

    // 1) no Pan3d in the chain — no camera either: Pan3d is skipped, and the other
    // effects do not need a camera
    assert!(
        resolve_cam3d(&[FxInstance::blur(8)], &cams, Tick(0), 1920, 1920).is_none(),
        "a chain without Pan3d asked for a camera"
    );
    assert!(resolve_cam3d(&[], &cams, Tick(0), 1920, 1920).is_none());

    // 2) a PINNED camera (by id) beats time: event No. 1 sits at [0, 1 s),
    // but the pinned camera is still in effect in the second second
    let pinned = resolve_cam3d(&[pan3d(Some(CameraId(1)))], &cams, Tick(120_000), 1920, 1920)
        .expect("pinned camera");
    assert_eq!(pinned.cam.rot_y, 11.0, "pinned camera lost to the active one");

    // 3) not pinned — the ACTIVE one, chosen BY TIME
    let active =
        resolve_cam3d(&[pan3d(None)], &cams, Tick(120_000), 1920, 1920).expect("active camera");
    assert_eq!(active.cam.rot_y, 22.0);
    let first = resolve_cam3d(&[pan3d(None)], &cams, Tick(0), 1920, 1920).expect("active camera");
    assert_eq!(first.cam.rot_y, 11.0);

    // 4) pinned to a NON-EXISTENT id — fall back to the active one, not to the
    // default: the camera could have been deleted while the layer survived it
    let gone = resolve_cam3d(&[pan3d(Some(CameraId(99)))], &cams, Tick(0), 1920, 1920)
        .expect("fallback camera");
    assert_eq!(gone.cam.rot_y, 11.0);

    // 5) no cameras at all — the default “50 mm”, that is an identity for Pan3d
    // with defaults (t298 and the goldens rest on this)
    let def = resolve_cam3d(&[pan3d(None)], &[], Tick(0), 1920, 1920).expect("default camera");
    assert_eq!(def.cam, CameraParams::default());

    // 6) world_scale = size of the TARGET / size of the PROJECT: the camera's world is frozen
    // by the format in the project's world pixels (R.29), so the target's scale
    // enters it, not the camera itself. A 320 px thumbnail in a project of
    // 1920 px must get 1/6, otherwise the perspective in the picker would differ
    // from the one after a click.
    let small = resolve_cam3d(&[pan3d(None)], &cams, Tick(0), 320, 1920).expect("camera");
    assert!((small.world_scale - 320.0 / 1920.0).abs() < 1e-12, "{}", small.world_scale);
    let full = resolve_cam3d(&[pan3d(None)], &cams, Tick(0), 1920, 1920).expect("camera");
    assert_eq!(full.world_scale, 1.0);
    // a zero project size does not divide by zero (clamp .max(1))
    let zero = resolve_cam3d(&[pan3d(None)], &cams, Tick(0), 320, 0).expect("camera");
    assert_eq!(zero.world_scale, 320.0);
}
