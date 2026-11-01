//! Test 1274 (shape-masks): the GPU applies the CPU rasterizer's coverage bytes in integer
//! passes — a mask adds no divergence: a chain of masks and the exact Geometry node agrees
//! EXACTLY, a masked effect within that effect's own class (t181).

use std::sync::Arc;

use kt_cache::{BurstCache, CacheConfig, SourceId};
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec, compose_plan_cpu};
use kt_fx::mask::{MaskOp, MaskOpKind};
use kt_gpu::{FrameTol, assert_frames_close, test_engine};
use kt_media::VideoFrame;
use kt_model::{FxInstance, Mask, MaskMode, MaskShape, Vertex};
use kt_time::Tick;

const W: u32 = 96;
const H: u32 = 54;

fn frame(alpha: bool, tint: u8) -> Arc<VideoFrame> {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            rgba[i] = (x * 3) as u8 ^ tint;
            rgba[i + 1] = (y * 5) as u8;
            rgba[i + 2] = ((x + y) * 2) as u8;
            rgba[i + 3] = if alpha { ((x * 5 + y * 3) % 256) as u8 } else { 255 };
        }
    }
    Arc::new(VideoFrame { pts: Tick(0), width: W, height: H, rgba: rgba.into_boxed_slice(), has_alpha: alpha })
}

fn masks() -> Vec<Mask> {
    let mut a = Mask::new(1, "a", MaskShape::Ellipse { cx: 0.45, cy: 0.5, w: 0.5, h: 0.6, rot: 20.0 });
    a.feather = 6.0;
    let mut b = Mask::new(
        2,
        "b",
        MaskShape::Path {
            verts: vec![
                Vertex::corner(0, 0.5, 0.1),
                Vertex { id: 1, x: 0.9, y: 0.5, ix: 0.0, iy: -0.2, ox: 0.0, oy: 0.2 },
                Vertex::corner(2, 0.55, 0.9),
            ],
        },
    );
    b.mode = MaskMode::Difference;
    b.feather = 18.0;
    vec![a, b]
}

/// (name, chain, masks, alpha source, the class of the chain itself)
type Case = (&'static str, Vec<FxInstance>, Vec<MaskOp>, bool, FrameTol);

fn op(at: usize, kind: MaskOpKind, follow_lead: bool) -> MaskOp {
    MaskOp { at, kind, masks: Arc::new(masks()), follow_lead, project_h: H as f64 }
}

#[test]
fn t1274_masked_frames_agree_exactly_on_both_backends() {
    kt_plugins_builtin::install();
    let Some(engine) = test_engine() else { return };
    let node = kt_model::EventGeometry { pos_x: 0.1, zoom: 1.3, rot: 12.0, ..Default::default() }.node();
    let lead = Some(kt_fx::conform::LeadCtx::new(Some((W as f64, H as f64)), kt_fx::conform::ConformRule::Fit));
    let blur = FxInstance::plugin_with("blur", &[4.0, 0.0]).unwrap();
    let duo = FxInstance::plugin("duotone").unwrap();
    // (name, chain, masks, alpha source, the class of the chain itself)
    let (exact, bil) = (FrameTol::EXACT, FrameTol::BILINEAR);
    let cases: Vec<Case> = vec![
        ("picture after the node", vec![node.clone()], vec![op(1, MaskOpKind::Alpha, true)], false, exact),
        ("picture, alpha source", vec![node.clone()], vec![op(1, MaskOpKind::Alpha, true)], true, exact),
        ("track, masks only", vec![], vec![op(0, MaskOpKind::Alpha, false)], true, exact),
        ("picture + effect", vec![node.clone(), duo.clone()], vec![op(1, MaskOpKind::Alpha, true)], true, bil),
        ("slot before the node", vec![blur.clone(), node.clone()], vec![op(1, MaskOpKind::Mix { from: 0 }, false)], false, bil),
        (
            "slot after the node + picture",
            vec![node.clone(), duo.clone()],
            vec![op(2, MaskOpKind::Mix { from: 1 }, true), op(1, MaskOpKind::Alpha, true)],
            true,
            bil,
        ),
        ("track (chain end)", vec![duo.clone()], vec![op(1, MaskOpKind::Alpha, false)], false, bil),
    ];
    for (i, (name, fx, ops, alpha, tol)) in cases.into_iter().enumerate() {
        let f = frame(alpha, i as u8 * 17);
        let src = LayerSrc {
            source: SourceId(9000 + i as u64),
            cache_idx: 0,
            content: xxhash_rust::xxh3::xxh3_64(&f.rgba),
            src_scale: 1,
            input: kt_engine::video_plan::LayerInput::Frame(Some(f)),
            fx: std::borrow::Cow::Borrowed(&fx[..]),
            fx_hash: xxhash_rust::xxh3::xxh3_64(format!("{fx:?}|{ops:?}|{i}").as_bytes()),
            u: 0.5,
            cam3d: None,
            blend: None,
            blend_mode: kt_model::BlendMode::Normal,
            lead,
            masks: ops,
        };
        let p = FramePlan {
            t: Tick(90_000),
            out_w: W,
            out_h: H,
            layers: vec![PlanLayer::One { src, alpha_k: 4096, scale_alpha: false, fade: None }],
            memoize_postfx: true,
        };
        let cache = BurstCache::new(CacheConfig::default());
        let mut cpu = Vec::new();
        let cc = compose_plan_cpu(&p, &cache, &mut cpu);
        let mut gpu = Vec::new();
        let cg = engine.execute_into(&p, &mut gpu).expect("GPU execute");
        assert_eq!(cc, cg, "{name}: complete diverged");
        assert_frames_close(&cpu, &gpu, W, H, tol, name);
        // and the mask did something
        let mut none = p;
        if let PlanLayer::One { src, .. } = &mut none.layers[0] {
            src.masks.clear();
            src.fx_hash ^= 1;
        }
        let mut bare = Vec::new();
        compose_plan_cpu(&none, &cache, &mut bare);
        assert_ne!(bare, cpu, "{name}: the masks changed nothing");
    }
}
