//! t2624 (stream-front-on-device, found-defects 561, capability `video-fx`): a geometry shape's
//! readiness moves the run forms' revision, which the memo keys carry. Its own test binary: the
//! revision is process-wide and another test's runs would move it too. Without a wgpu adapter: skip.

use std::sync::Arc;

use kt_engine::VideoGpuExec;
use kt_media::VideoFrame;
use kt_model::FxInstance;
use kt_time::Tick;

/// Test t2624: the first frame of a geometry shape runs the interpreter while the shape compiles; once
/// the shape is ready the forms' revision has moved, so a composite kept from the interpreter's frame
/// is never served for the frame that now runs the specialized text.
#[test]
fn t2624_a_ready_geometry_shape_moves_the_forms_revision() {
    kt_plugins_builtin::install();
    let Some(engine) = kt_gpu::test_engine() else { return };
    let rgba: Vec<u8> = (0..160u32 * 90 * 4).map(|i| (i * 13 % 251) as u8).collect();
    let f = Arc::new(VideoFrame { pts: Tick(0), width: 160, height: 90, rgba: rgba.into_boxed_slice(), has_alpha: false });
    let chain = [FxInstance::geometry_at(0.07, -0.03, 1.3)];
    let plan = kt_engine::chain_plan(f, &chain, 2624, 0.0, Tick(0), None);
    let mut out = Vec::new();
    engine.execute_once(&plan, &mut out).expect("the first frame");
    let shapes = engine.geometry_shapes();
    let before = kt_gpu::forms_revision();
    engine.settle();
    assert!(engine.geometry_shapes() > shapes, "the shape compiled");
    assert!(kt_gpu::forms_revision() > before, "the shape's readiness moved the forms' revision");
}
