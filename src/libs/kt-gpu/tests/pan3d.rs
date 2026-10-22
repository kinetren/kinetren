//! Test 187 (R.29, ed. R.44): Pan 3D. Properties:
//! - identity: default parameters + the default “50 mm” camera at auto distance
//!   give a 1:1 frame (within the f32 matrix tolerance);
//! - geometry: a pos_x shift moves the marker by exactly that many px;
//! - scale < 1 opens transparent margins; the background shows through below;
//! - determinism on the device (two engines byte for byte);
//! - with R.44 the CPU is the REFERENCE again, also for Pan3d (the inverse homography in
//!   the merged geometry pass): each GPU frame is checked against compose_plan_cpu in GEOM.

use std::sync::Arc;

use kt_cache::{BurstCache, CacheConfig, SourceId};
use kt_engine::{Cam3d, FramePlan, LayerSrc, PlanLayer, VideoGpuExec, compose_plan_cpu};
use kt_gpu::{FrameTol, assert_frames_close, test_engine};
use kt_media::VideoFrame;
use kt_model::{CameraParams, FxInstance};
use kt_time::Tick;

const W: u32 = 64;
const H: u32 = 36;

fn frame_with_marker(mx: u32, my: u32) -> Arc<VideoFrame> {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            let v = if (x, y) == (mx, my) { 255 } else { 24 + ((x + y) % 8) as u8 };
            rgba[i] = v;
            rgba[i + 1] = v;
            rgba[i + 2] = v;
            rgba[i + 3] = 255;
        }
    }
    Arc::new(VideoFrame {
        pts: Tick(0),
        width: W,
        height: H,
        rgba: rgba.into_boxed_slice(),
        has_alpha: false,
    })
}

fn pan3d(pos_x: f64, scale: f64) -> FxInstance {
    FxInstance::Pan3d {
        pos_x,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 0.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale,
        camera: None,
    }
}

fn layer<'a>(fx: &'a [FxInstance], frame: &Arc<VideoFrame>, id: u64) -> PlanLayer<'a> {
    PlanLayer::One {
        src: LayerSrc {
            source: SourceId(id),
            cache_idx: 0,
            content: xxhash_rust::xxh3::xxh3_64(&frame.rgba),
            src_scale: 1,
            input: kt_engine::video_plan::LayerInput::Frame(Some(frame.clone())),
            fx: std::borrow::Cow::Borrowed(fx),
            fx_hash: id,
            u: 0.5,
            cam3d: Some(Cam3d { cam: CameraParams::default(), world_scale: 1.0 }),
            blend: None,
            blend_mode: kt_model::BlendMode::Normal,
            lead: None,
        },
        alpha_k: 4096,
        scale_alpha: false,
        fade: None,
    }
}

fn run(engine: &dyn VideoGpuExec, layers: Vec<PlanLayer<'_>>) -> Vec<u8> {
    let plan = FramePlan { t: Tick(90_000), out_w: W, out_h: H, layers, memoize_postfx: true };
    let mut out = Vec::new();
    let complete = engine.execute_into(&plan, &mut out).expect("GPU execute");
    assert!(complete);
    // R.44: the CPU reference computes THE SAME plan (Pan3d is the inverse homography);
    // checked on every run, so the GPU frames of pan3d are no longer orphaned
    let cache = BurstCache::new(CacheConfig::default());
    let mut cpu = Vec::new();
    let complete_cpu = compose_plan_cpu(&plan, &cache, &mut cpu);
    assert_eq!(complete_cpu, complete, "complete CPU vs GPU");
    assert_frames_close(&cpu, &out, W, H, FrameTol::GEOM, "pan3d-cpu-vs-gpu");
    out
}

fn brightest(rgba: &[u8]) -> (u32, u32) {
    let mut best = (0u32, 0u32);
    let mut bv = 0u32;
    for (p, px) in rgba.chunks_exact(4).enumerate() {
        let v = px[0] as u32 + px[1] as u32 + px[2] as u32;
        if v > bv {
            bv = v;
            best = (p as u32 % W, p as u32 / W);
        }
    }
    best
}

#[test]
fn t187_pan3d_identity_shift_scale() {
    let Some(e) = test_engine() else { return };
    let f = frame_with_marker(20, 12);

    // identity: default Pan3d == a frame without the effect
    let fx_id = [pan3d(0.0, 1.0)];
    let ident = run(e.as_ref(), vec![layer(&fx_id, &f, 1)]);
    let none: [FxInstance; 0] = [];
    let plain = run(e.as_ref(), vec![layer(&none, &f, 2)]);
    assert_frames_close(&plain, &ident, W, H, FrameTol::BILINEAR, "pan3d-identity");

    // shift: the marker moved by exactly pos_x (±1 px of rasterization)
    let fx_shift = [pan3d(8.0, 1.0)];
    let shifted = run(e.as_ref(), vec![layer(&fx_shift, &f, 3)]);
    let (mx, my) = brightest(&shifted);
    assert!(
        (mx as i64 - 28).abs() <= 1 && (my as i64 - 12).abs() <= 1,
        "marker (20,12) at pos_x=8 must end up at (28,12), but it is at ({mx},{my})"
    );

    // scale 0.5: the corners of the frame are transparent (the background shows), the centre is the content
    let fx_half = [pan3d(0.0, 0.5)];
    let bg = frame_with_marker(999, 999); // without a marker, grey
    let out = run(e.as_ref(), vec![layer(&none, &bg, 4), layer(&fx_half, &f, 5)]);
    let px = |x: u32, y: u32| {
        let i = ((y * W + x) * 4) as usize;
        (out[i], out[i + 3])
    };
    // centre: the layer on top (the marker is not in the centre here, just the layer’s content)
    let (_, ca) = px(W / 2, H / 2);
    assert_eq!(ca, 255, "the centre is opaque");
    // corner: the quad is squeezed twice, so the corner is transparent and the lower layer is visible (~24+)
    let (cr, _) = px(1, 1);
    assert!(cr >= 24, "the background shows through in the corner, not black emptiness: {cr}");
    // and the layer’s marker stands in the squeezed position: (20-32)*0.5+32 = 26, (12-18)*0.5+18 = 15
    let (mx2, my2) = brightest(&out);
    assert!(
        (mx2 as i64 - 26).abs() <= 1 && (my2 as i64 - 15).abs() <= 1,
        "the marker at scale 0.5 is at (26,15), but it is at ({mx2},{my2})"
    );
}

#[test]
fn t187b_pan3d_deterministic_on_device() {
    let Some(e1) = test_engine() else { return };
    let Some(e2) = test_engine() else { return };
    let f = frame_with_marker(20, 12);
    let fx = [pan3d(3.0, 0.8)];
    let a = run(e1.as_ref(), vec![layer(&fx, &f, 1)]);
    let b = run(e2.as_ref(), vec![layer(&fx, &f, 1)]);
    assert_eq!(a, b, "Pan3d is non-deterministic on one device");
}
