//! Tests 181–183 (R.29): conformance of the GPU executor against the CPU reference.
//!
//! - t181: each effect and chains: tolerances by kind of arithmetic
//!   (integer kernels byte for byte, bilinear ±LSB, threshold ones with
//!   legitimate outliers at the threshold edge);
//! - t182: composite scenes (over/scale/solo/dissolve/missing layers):
//!   byte for byte (the composite is integer);
//! - t183: determinism on one device + a “dirty” texture pool.
//!
//! Without a wgpu adapter: skip with a printout; KT_REQUIRE_GPU=1 requires an adapter.

use std::sync::Arc;

use kt_cache::{BurstCache, CacheConfig, SourceId};
use kt_engine::{FramePlan, LayerSrc, PlanLayer, VideoGpuExec, compose_plan_cpu};
use kt_gpu::{FrameTol, assert_frames_close, test_engine};
use kt_media::VideoFrame;
use kt_model::{CamKeyframe, Dir, FxInstance, IrisShape, PanMode, TransitionKind};
use kt_time::Tick;

const W: u32 = 64;
const H: u32 = 36;

/// A deterministic frame: gradients + a green corner (as in t26 of kt-fx),
/// optionally with a gradient alpha.
fn frame(w: u32, h: u32, alpha: bool, tint: u8) -> Arc<VideoFrame> {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            if x > w / 2 && y > h / 2 {
                rgba[i] = 0;
                rgba[i + 1] = 255;
                rgba[i + 2] = 0;
            } else {
                rgba[i] = (x * 4) as u8 ^ tint;
                rgba[i + 1] = (y * 7) as u8;
                rgba[i + 2] = ((x + y) * 3) as u8;
            }
            rgba[i + 3] = if alpha { ((x * 5 + y * 3) % 256) as u8 } else { 255 };
        }
    }
    Arc::new(VideoFrame {
        pts: Tick(0),
        width: w,
        height: h,
        rgba: rgba.into_boxed_slice(),
        has_alpha: alpha,
    })
}

struct Src {
    source: SourceId,
    frame: Option<Arc<VideoFrame>>,
    fx: Vec<FxInstance>,
    fx_hash: u64,
    u: f64,
    cam3d: Option<kt_engine::Cam3d>,
    /// The decode scale step of the layer (R.46): a frame src_scale times larger than Quality.
    src_scale: u32,
    /// Context of the leading “Geometry” node (R.62).
    lead: Option<kt_fx::conform::LeadCtx>,
}

impl Src {
    fn new(id: u64, frame: Option<Arc<VideoFrame>>, fx: Vec<FxInstance>) -> Src {
        // fx_hash MUST be unique per set of parameters: the CPU path
        // memoizes the post-FX by it (like fx_chain_hash in kt-engine)
        let repr = format!("{fx:?}|{id}");
        Src {
            source: SourceId(id),
            frame,
            fx,
            fx_hash: xxhash_rust::xxh3::xxh3_64(repr.as_bytes()),
            u: 0.5,
            cam3d: None,
            src_scale: 1,
            lead: None,
        }
    }

    fn layer(&self) -> LayerSrc<'_> {
        LayerSrc {
            source: self.source,
            cache_idx: 0,
            // the serial number of the content (R.53, item 409): in the product
            // BurstCache issues it; here it is a hash of the frame’s own bytes, so that
            // different frames do not share a texture in UploadCache
            content: self.frame.as_ref().map(|f| xxhash_rust::xxh3::xxh3_64(&f.rgba)).unwrap_or(0),
            src_scale: self.src_scale,
            input: kt_engine::video_plan::LayerInput::Frame(self.frame.clone()),
            fx: std::borrow::Cow::Borrowed(&self.fx[..]),
            fx_hash: self.fx_hash,
            u: self.u,
            cam3d: self.cam3d,
            blend: None,
            blend_mode: kt_model::BlendMode::Normal,
            lead: self.lead,
        }
    }
}

fn one(src: &Src, alpha_k: u32, scale_alpha: bool) -> PlanLayer<'_> {
    PlanLayer::One { src: src.layer(), alpha_k, scale_alpha, fade: None }
}

fn plan<'a>(t: i64, layers: Vec<PlanLayer<'a>>) -> FramePlan<'a> {
    FramePlan { t: Tick(t), out_w: W, out_h: H, layers, memoize_postfx: true }
}

/// CPU against GPU on one plan; returns (cpu, gpu, complete_cpu,
/// complete_gpu).
fn run_both(engine: &dyn VideoGpuExec, p: &FramePlan<'_>) -> (Vec<u8>, Vec<u8>, bool, bool) {
    let cache = BurstCache::new(CacheConfig::default());
    let mut cpu = Vec::new();
    let c_cpu = compose_plan_cpu(p, &cache, &mut cpu);
    let mut gpu = Vec::new();
    let c_gpu = engine.execute_into(p, &mut gpu).expect("GPU execute");
    (cpu, gpu, c_cpu, c_gpu)
}

#[test]
fn t181_effect_parity_gpu_vs_cpu() {
    let Some(engine) = test_engine() else { return };
    let cases: Vec<(&str, Vec<FxInstance>, FrameTol)> = vec![
        ("pan", vec![FxInstance::geometry_at(-0.05, 0.03333, 1.5)], FrameTol::BILINEAR),
        (
            "cammotion",
            vec![FxInstance::CamMotion {
                mode: PanMode::Fit,
                ease: true,
                time_scale: 1.0,
                keyframes: vec![
                    CamKeyframe { t: 0.0, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 },
                    CamKeyframe { t: 1.0, pan_x: 0.5, pan_y: 0.0, zoom: 2.0 },
                ],
            }],
            FrameTol::BILINEAR,
        ),
        ("mirror", vec![FxInstance::Mirror { horizontal: true, vertical: false }], FrameTol::EXACT),
        ("blur", vec![FxInstance::blur(8)], FrameTol::BILINEAR),
        ("greenscreen", vec![FxInstance::greenscreen([0, 255, 0], 70, 25)], FrameTol::THRESHOLD),
        ("edges", vec![FxInstance::edges(48)], FrameTol::THRESHOLD),
        ("noise", vec![FxInstance::noise(0.3)], FrameTol::EXACT),
        // the new parameters of R.33 S5: the mirrors MUST converge on them too
        ("blur-xy", vec![FxInstance::Blur { radius: 24, radius_y: 4 }], FrameTol::BILINEAR),
        (
            "edges-thick-color",
            vec![FxInstance::Edges {
                threshold: 48,
                thickness: 3,
                color_r: 255,
                color_g: 90,
                color_b: 0,
            }],
            FrameTol::THRESHOLD,
        ),
        (
            "noise-coarse",
            vec![FxInstance::Noise { amount: 0.3, size: 8, color: false }],
            FrameTol::EXACT,
        ),
        (
            "noise-color",
            vec![FxInstance::Noise { amount: 0.3, size: 3, color: true }],
            FrameTol::EXACT,
        ),
        (
            "vhs-scan-jitter",
            vec![FxInstance::Vhs { rgb_shift: 4, bleed: 0.4, scan_step: 7, jitter: 0.5 }],
            FrameTol::BILINEAR,
        ),
        (
            "vhs-no-scan",
            vec![FxInstance::Vhs { rgb_shift: 2, bleed: 0.2, scan_step: 0, jitter: 0.0 }],
            FrameTol::BILINEAR,
        ),
        (
            "glow-radius-tint",
            vec![FxInstance::Glow {
                strength: 0.9,
                radius: 8,
                threshold: 0,
                tint_r: 255,
                tint_g: 120,
                tint_b: 40,
            }],
            FrameTol::BILINEAR,
        ),
        (
            "glow-threshold",
            vec![FxInstance::Glow {
                strength: 1.2,
                radius: 4,
                threshold: 90,
                tint_r: 255,
                tint_g: 255,
                tint_b: 255,
            }],
            FrameTol::BILINEAR,
        ),
        (
            "greenscreen-spill",
            vec![FxInstance::Greenscreen {
                key_r: 0,
                key_g: 255,
                key_b: 0,
                tolerance: 70,
                softness: 25,
                spill: 0.8,
            }],
            FrameTol::THRESHOLD,
        ),
        ("vhs", vec![FxInstance::vhs(4, 0.4)], FrameTol::BILINEAR),
        ("glow", vec![FxInstance::glow(0.65)], FrameTol::BILINEAR),
        (
            "eventgrid",
            vec![FxInstance::EventGrid { cols: 2, rows: 2, center: true }],
            FrameTol::BILINEAR,
        ),
        // Merged geometry series (R.44): one pass of inverse mappings on
        // both backends (the program is shared, kt-fx::geom); the tolerance is bilinear:
        // f32 versus f64 in the composition of maps and in the taps.
        (
            "geo-mirror-grid",
            vec![
                FxInstance::Mirror { horizontal: true, vertical: false },
                FxInstance::EventGrid { cols: 2, rows: 2, center: false },
            ],
            FrameTol::BILINEAR,
        ),
        (
            "geo-grid-mirror",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: true },
                FxInstance::Mirror { horizontal: true, vertical: true },
            ],
            FrameTol::BILINEAR,
        ),
        (
            "geo-pan-pan",
            vec![
                FxInstance::geometry_at(-0.05, 0.03333, 1.5),
                FxInstance::geometry_at(0.008333, -0.008333, 1.2),
            ],
            FrameTol::BILINEAR,
        ),
        // a scenario core of R.44: the grid shrank, zoom brought it back; the merged pass
        // samples the SOURCE, not the raster of the grid
        (
            "geo-grid-pan-zoomback",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: true },
                FxInstance::geometry_zoom(2.0),
            ],
            FrameTol::BILINEAR,
        ),
        // zoom<1: the frame is wider than the input, a “plateau” (Clamp prim) on both paths
        (
            "geo-pan-shrink-clamp",
            vec![
                FxInstance::Mirror { horizontal: false, vertical: true },
                FxInstance::geometry_zoom(0.6),
            ],
            FrameTol::BILINEAR,
        ),
        // Absorbed pointwise ops (R.45, item 312): Greenscreen/Noise on a
        // subtap inside a series. Threshold (a soft key) and cellular (a grain grid)
        // decisions legitimately flip at the f32/f64 boundaries: THRESHOLD
        // (a precedent of edges/greenscreen).
        (
            "tap-gs-in-run",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: false },
                FxInstance::greenscreen([0, 255, 0], 70, 25),
                FxInstance::geometry_zoom(2.0),
            ],
            FrameTol::THRESHOLD,
        ),
        (
            "tap-noise-in-run",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: true },
                FxInstance::Noise { amount: 0.6, size: 2, color: true },
                FxInstance::geometry_at(-0.02222, -0.0, 1.8),
            ],
            FrameTol::THRESHOLD,
        ),
        // K×-barriers (R.45, item 314): the grid squeezed, a barrier at K=2,
        // zoom brought it back. The CPU kernel and the GPU pass work on K×-buffers.
        (
            "k2-vhs",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: false },
                FxInstance::Vhs { rgb_shift: 2, bleed: 0.3, scan_step: 5, jitter: 0.4 },
                FxInstance::geometry_zoom(2.0),
            ],
            FrameTol::BILINEAR,
        ),
        (
            "k2-blur",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: true },
                FxInstance::blur(8),
                FxInstance::geometry_zoom(2.0),
            ],
            FrameTol::BILINEAR,
        ),
        (
            "k2-glow",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: false },
                FxInstance::glow(0.9),
                FxInstance::geometry_at(-0.02222, -0.0, 1.8),
            ],
            FrameTol::BILINEAR,
        ),
        (
            "k2-edges",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: false },
                FxInstance::edges(48),
                FxInstance::geometry_zoom(2.0),
            ],
            FrameTol::THRESHOLD,
        ),
        // a Noise BARRIER at K× (not absorbed: it stands right after a hard barrier,
        // the leader for the next run)
        (
            "k2-noise-barrier",
            vec![
                FxInstance::EventGrid { cols: 2, rows: 2, center: false },
                FxInstance::blur(4),
                FxInstance::Noise { amount: 0.5, size: 2, color: true },
                FxInstance::geometry_zoom(2.0),
            ],
            FrameTol::THRESHOLD,
        ),
        (
            "chain-user",
            vec![
                FxInstance::Mirror { horizontal: true, vertical: false },
                FxInstance::EventGrid { cols: 2, rows: 2, center: false },
                FxInstance::glow(1.0),
                FxInstance::vhs(2, 0.5),
            ],
            FrameTol::BILINEAR,
        ),
        (
            "chain-key",
            vec![FxInstance::greenscreen([0, 255, 0], 70, 25), FxInstance::noise(0.5)],
            FrameTol::THRESHOLD,
        ),
        // LAYER STRENGTH (R.38 item 12): mixing of the dry and wet sides. The kernel
        // is integer (a mirror of dissolve_into), so it is byte for byte,
        // but the input of the wet side is bilinear, hence the common tolerance.
        (
            "layer-mix-half",
            vec![
                FxInstance::Mirror { horizontal: true, vertical: false },
                FxInstance::blur(8),
                FxInstance::LayerMix { amount: 0.5, from: 1 },
            ],
            FrameTol::BILINEAR,
        ),
        (
            "layer-mix-zero",
            vec![FxInstance::edges(48), FxInstance::LayerMix { amount: 0.0, from: 1 }],
            FrameTol::THRESHOLD,
        ),
        // the strength AFTER a resampling effect: the dry side MUST be
        // fitted by the same bilinear as on the CPU
        (
            "layer-mix-after-resize",
            vec![
                FxInstance::geometry_at(-0.02857, -0.0, 1.4),
                FxInstance::blur(6),
                FxInstance::LayerMix { amount: 0.35, from: 1 },
            ],
            FrameTol::BILINEAR,
        ),
        // strength ZERO on top of everything: MUST return the state before the layer
        (
            "layer-mix-whole-chain",
            vec![FxInstance::vhs(4, 0.4), FxInstance::LayerMix { amount: 0.25, from: 0 }],
            FrameTol::BILINEAR,
        ),
    ];
    for (i, (name, fx, tol)) in cases.into_iter().enumerate() {
        let src = Src::new(1000 + i as u64, Some(frame(W, H, false, 0)), fx);
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "{name}: complete diverged");
        assert_frames_close(&cpu, &gpu, W, H, tol, name);
    }
    // ALPHA RULE (R.53, item 418): input WITH ALPHA. The backends diverge
    // most easily here: the premultiplication before the mip and the raising of alpha in
    // edges live TWICE (kt-fx and WGSL), and were they to diverge, a title with
    // blur would look different on the GPU than in the CPU export.
    let alpha_cases: Vec<(&str, Vec<FxInstance>, FrameTol)> = vec![
        ("alpha-blur", vec![FxInstance::blur(8)], FrameTol::BILINEAR),
        ("alpha-blur-xy", vec![FxInstance::Blur { radius: 24, radius_y: 4 }], FrameTol::BILINEAR),
        ("alpha-glow", vec![FxInstance::glow(1.2)], FrameTol::BILINEAR),
        (
            "alpha-glow-tinted",
            vec![FxInstance::Glow {
                strength: 0.8,
                radius: 9,
                threshold: 40,
                tint_r: 200,
                tint_g: 120,
                tint_b: 255,
            }],
            FrameTol::BILINEAR,
        ),
        ("alpha-edges", vec![FxInstance::edges(48)], FrameTol::THRESHOLD),
        (
            "alpha-gs-blur",
            vec![FxInstance::greenscreen([0, 255, 0], 70, 25), FxInstance::blur(8)],
            FrameTol::THRESHOLD,
        ),
        (
            "alpha-grid-blur",
            vec![FxInstance::EventGrid { cols: 2, rows: 2, center: true }, FxInstance::blur(12)],
            FrameTol::BILINEAR,
        ),
    ];
    for (i, (name, fx, tol)) in alpha_cases.into_iter().enumerate() {
        let src = Src::new(3000 + i as u64, Some(frame(W, H, true, 0)), fx);
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "{name}: complete diverged");
        assert_frames_close(&cpu, &gpu, W, H, tol, name);
    }

    // Pan with a frame of a FOREIGN size (the resize to Quality MUST match)
    let src = Src::new(
        2000,
        Some(frame(96, 54, false, 0)),
        vec![FxInstance::geometry_at(-0.008333, -0.0, 1.2)],
    );
    let p = plan(90_000, vec![one(&src, 4096, false)]);
    let (cpu, gpu, _, _) = run_both(engine.as_ref(), &p);
    assert_frames_close(&cpu, &gpu, W, H, FrameTol::BILINEAR, "pan-resize");

    // Pan3d series (R.44): the CPU reference has appeared (the inverse homography);
    // tolerance GEOM: the silhouette of the quad legitimately flips single pixels
    // (f32 predicates versus f64), inside it is bilinear precision.
    let p3 = |pos_x: f64, rot_y: f64, scale: f64| FxInstance::Pan3d {
        pos_x,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale,
        camera: None,
    };
    let grid2 = FxInstance::EventGrid { cols: 2, rows: 2, center: true };
    let geo3d: Vec<(&str, Vec<FxInstance>)> = vec![
        ("pan3d-alone", vec![p3(4.0, 20.0, 0.9)]),
        ("grid-pan3d", vec![grid2.clone(), p3(0.0, 35.0, 1.0)]),
        ("pan3d-grid", vec![p3(0.0, 0.0, 0.7), grid2.clone()]),
        // THE USER’S CHAIN (R.44): grid → pan3d → grid → pan3d-zoom,
        // net scale ≈ 1: the merged pass samples the source
        ("user-grid-pan3d-x2", vec![grid2.clone(), p3(2.0, 15.0, 1.0), grid2, p3(0.0, 0.0, 4.0)]),
    ];
    for (i, (name, fx)) in geo3d.into_iter().enumerate() {
        let mut src = Src::new(3000 + i as u64, Some(frame(W, H, false, 0)), fx);
        src.cam3d =
            Some(kt_engine::Cam3d { cam: kt_model::CameraParams::default(), world_scale: 1.0 });
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "{name}: complete diverged");
        assert_frames_close(&cpu, &gpu, W, H, FrameTol::GEOM, name);
    }
}

/// Test 309 (item 319, R.46): the DECODE SCALE STEP on the GPU. The pre-FX frame arrives
/// twice as large as Quality (`src_scale = 2`), the plan is built in logic, and
/// it is executed on the large buffer: conjugation, leading barriers at 2×,
/// absorbed pointwise ops and the Legacy mirror MUST match the CPU within the same
/// tolerances as the K×-barriers of R.45.
#[test]
fn t309_decode_stage_parity_gpu_vs_cpu() {
    let Some(engine) = test_engine() else { return };
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let grid2 = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let cases: Vec<(&str, Vec<FxInstance>, FrameTol)> = vec![
        // pure conjugation: the 2× input is brought back exactly to Quality by zoom
        ("s2-zoom", vec![zoom2.clone()], FrameTol::BILINEAR),
        // the leading barrier runs at 2× (there was no resample before it)
        ("s2-vhs-zoom", vec![FxInstance::vhs(4, 0.4), zoom2.clone()], FrameTol::BILINEAR),
        // step + grid above the barrier: K is taken from both sources
        (
            "s2-grid-vhs-zoom",
            vec![grid2.clone(), FxInstance::vhs(2, 0.3), zoom2.clone()],
            FrameTol::BILINEAR,
        ),
        // mirror on equal edges: the old kernel directly on the 2× buffer
        (
            "s2-mirror-zoom",
            vec![
                FxInstance::Mirror { horizontal: true, vertical: false },
                FxInstance::vhs(4, 0.4),
                zoom2.clone(),
            ],
            FrameTol::BILINEAR,
        ),
        // absorbed pointwise ops on a subtap of the large input
        (
            "s2-tap-gs",
            vec![grid2, FxInstance::greenscreen([0, 255, 0], 70, 25), zoom2.clone()],
            FrameTol::THRESHOLD,
        ),
        // the scenario core of the feature: zoom ×2 takes details that were absent at 1×
        ("s2-zoom-edges", vec![zoom2, FxInstance::edges(48)], FrameTol::THRESHOLD),
    ];
    for (i, (name, fx, tol)) in cases.into_iter().enumerate() {
        let mut src = Src::new(4000 + i as u64, Some(frame(W * 2, H * 2, false, 0)), fx);
        src.src_scale = 2;
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "{name}: complete diverged");
        assert_frames_close(&cpu, &gpu, W, H, tol, name);
    }
    // 3D on the step: the inverse homography is computed in logic, the taps on the
    // large buffer (the GEOM tolerance: the quad silhouette, as in t181)
    let mut src = Src::new(
        4100,
        Some(frame(W * 2, H * 2, false, 0)),
        vec![FxInstance::Pan3d {
            pos_x: 2.0,
            pos_y: 0.0,
            pos_z: 0.0,
            rot_x: 0.0,
            rot_y: 15.0,
            rot_z: 0.0,
            anchor_x: 0.5,
            anchor_y: 0.5,
            scale: 2.0,
            camera: None,
        }],
    );
    src.src_scale = 2;
    src.cam3d = Some(kt_engine::Cam3d { cam: kt_model::CameraParams::default(), world_scale: 1.0 });
    let p = plan(90_000, vec![one(&src, 4096, false)]);
    let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
    assert_eq!(cc, cg, "s2-pan3d: complete diverged");
    assert_frames_close(&cpu, &gpu, W, H, FrameTol::GEOM, "s2-pan3d");
}

#[test]
fn t182_composite_parity_byte_exact() {
    let Some(engine) = test_engine() else { return };
    let bg = Src::new(1, Some(frame(W, H, false, 0)), vec![]);
    let top_alpha = Src::new(2, Some(frame(W, H, true, 0x55)), vec![]);
    let top_opaque = Src::new(3, Some(frame(W, H, false, 0xaa)), vec![]);
    let missing = Src::new(4, None, vec![]);
    let misfit = Src::new(5, Some(frame(32, 18, false, 0)), vec![]);

    // (name, layers, expected complete)
    let cases: Vec<(&str, Vec<PlanLayer<'_>>, bool)> = vec![
        ("two-opaque", vec![one(&bg, 4096, false), one(&top_opaque, 4096, false)], true),
        ("alpha-over", vec![one(&bg, 4096, false), one(&top_alpha, 4096, false)], true),
        ("opacity", vec![one(&bg, 4096, false), one(&top_alpha, 2048, true)], true),
        ("solo-fade-full", vec![one(&bg, 4096, false), one(&top_opaque, 4096, true)], true),
        ("solo-fade-mid", vec![one(&top_opaque, 1365, true)], true),
        (
            "dissolve",
            vec![
                one(&bg, 4096, false),
                PlanLayer::Dissolve {
                    a: top_opaque.layer(),
                    b: top_alpha.layer(),
                    k: 1000,
                    alpha_k: 4096,
                    mix: Default::default(),
                },
            ],
            true,
        ),
        (
            "dissolve-level",
            vec![
                one(&bg, 4096, false),
                PlanLayer::Dissolve {
                    a: top_opaque.layer(),
                    b: top_alpha.layer(),
                    k: 3000,
                    alpha_k: 2000,
                    mix: Default::default(),
                },
            ],
            true,
        ),
        (
            "dissolve-missing-half",
            vec![
                one(&bg, 4096, false),
                PlanLayer::Dissolve {
                    a: top_opaque.layer(),
                    b: missing.layer(),
                    k: 2048,
                    alpha_k: 3000,
                    mix: Default::default(),
                },
            ],
            false,
        ),
        ("missing-layer", vec![one(&bg, 4096, false), one(&missing, 4096, false)], false),
        // R.58, item 459: a layer of a FOREIGN SIZE no longer passes silently. Before
        // this round the frame was declared COMPLETE without it, so the embedding
        // disappeared from the finished frame, and the export wrote it as good.
        // The bytes still converge between both executors (both do not draw the layer),
        // but “complete” became a shared and honest `false`.
        ("misfit-not-silent", vec![one(&bg, 4096, false), one(&misfit, 4096, false)], false),
        ("empty", vec![one(&missing, 4096, false)], false),
    ];
    for (name, layers, want_complete) in cases {
        let p = plan(90_000, layers);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, want_complete, "{name}: complete CPU");
        assert_eq!(cg, want_complete, "{name}: complete GPU");
        assert_frames_close(&cpu, &gpu, W, H, FrameTol::EXACT, name);
    }
}

#[test]
fn t183_gpu_deterministic_on_device_and_dirty_pool() {
    let Some(e1) = test_engine() else { return };
    let Some(e2) = test_engine() else { return };
    // a chain WITHOUT t-dependent effects: a change of t alters the engine’s cache key
    // but not the bytes, so it forces a recalculation with a “dirty” pool
    let src = Src::new(
        10,
        Some(frame(96, 54, false, 3)),
        vec![FxInstance::geometry_at(-0.02308, -0.01154, 1.3), FxInstance::glow(1.2)],
    );
    let bg = Src::new(11, Some(frame(W, H, false, 0)), vec![]);
    let mk = |t: i64| plan(t, vec![one(&bg, 4096, false), one(&src, 3000, true)]);

    let mut a1 = Vec::new();
    e1.execute_into(&mk(90_000), &mut a1).unwrap();
    // two independent engines on one adapter: byte for byte
    let mut b1 = Vec::new();
    e2.execute_into(&mk(90_000), &mut b1).unwrap();
    assert_eq!(a1, b1, "two engines on one device diverged");

    // dirty the pools of e1 with other plans (including the merged geometry series of R.44 and
    // the K×-barrier of R.45: the grid squeezed → blur at 2× → zoom)
    let junk = Src::new(
        12,
        Some(frame(W, H, false, 9)),
        vec![
            FxInstance::EventGrid { cols: 3, rows: 2, center: true },
            FxInstance::blur(12),
            FxInstance::Mirror { horizontal: true, vertical: false },
            FxInstance::geometry_zoom(2.0),
        ],
    );
    let mut _j = Vec::new();
    e1.execute_into(&plan(1920, vec![one(&junk, 4096, false)]), &mut _j).unwrap();
    e1.execute_into(&plan(3840, vec![one(&junk, 2000, true)]), &mut _j).unwrap();

    // the same plan with another t (bypassing the ReadbackCache) → recalculation with a dirty
    // pool MUST give the same bytes
    let mut a2 = Vec::new();
    e1.execute_into(&mk(180_000), &mut a2).unwrap();
    assert_eq!(a1, a2, "a dirty texture pool changed the result");

    // and a repeat with the original t is a ReadbackCache hit with the same bytes
    let mut a3 = Vec::new();
    e1.execute_into(&mk(90_000), &mut a3).unwrap();
    assert_eq!(a1, a3, "ReadbackCache returned the wrong bytes");
}

/// Test 224 (R.33 S4): EVERY kind of transition is computed on the GPU the same way as on the
/// CPU. The transition kernels are the only place where one formula had to be written twice
/// (Rust and WGSL), so a divergence here means that the user’s preview and export
/// would show DIFFERENT things: exactly
/// what invariant 7 forbids.
#[test]
fn t224_transition_parity_gpu_vs_cpu() {
    let Some(engine) = test_engine() else { return };
    let a = Src::new(1, Some(frame(W, H, false, 0)), vec![]);
    let b = Src::new(2, Some(frame(W, H, false, 0x5a)), vec![]);
    // tolerance: the transition kernels are computed in f32 on both paths, but the sampling
    // and the trigonometry are the driver’s own, so we take the bilinear tolerance, and for
    // masks with a sharp edge we allow a band of pixels at the boundary
    let soft = FrameTol { max_lsb: 2, outlier_frac: 0.01, outlier_max: 4, mean_lsb: 0.05 };
    // VARIATIONS, not just the defaults: for wipe/shift the default is Dir::Left,
    // that is the vector (−1, 0). On it the error in the Y term of the formula is not visible
    // at all: that is exactly how the first version of this test missed a deliberately
    // broken mirror. Every axis and every flag MUST be in the list.
    let cases: Vec<TransitionKind> = vec![
        TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.15 },
        TransitionKind::Wipe { dir: Dir::Up, angle: 0.0, softness: 0.0 },
        TransitionKind::Wipe { dir: Dir::Right, angle: 37.0, softness: 0.3 },
        TransitionKind::Iris { shape: IrisShape::Circle, cx: 0.5, cy: 0.5, softness: 0.15 },
        TransitionKind::Iris { shape: IrisShape::Diamond, cx: 0.5, cy: 0.5, softness: 0.0 },
        TransitionKind::Iris { shape: IrisShape::Box, cx: 0.5, cy: 0.5, softness: 0.4 },
        // an offset centre (R.51): without it the error in the centre term is not visible
        TransitionKind::Iris { shape: IrisShape::Circle, cx: 0.2, cy: 0.7, softness: 0.1 },
        TransitionKind::Clock { reverse: false, cx: 0.5, cy: 0.5, softness: 0.1 },
        TransitionKind::Clock { reverse: true, cx: 0.5, cy: 0.5, softness: 0.0 },
        TransitionKind::Clock { reverse: false, cx: 0.7, cy: 0.3, softness: 0.1 },
        TransitionKind::Barn { vertical: false, softness: 0.2 },
        TransitionKind::Barn { vertical: true, softness: 0.0 },
        TransitionKind::Luma { invert: false, softness: 0.2 },
        TransitionKind::Luma { invert: true, softness: 0.05 },
        TransitionKind::Push { dir: Dir::Left },
        TransitionKind::Push { dir: Dir::Down },
        TransitionKind::Slide { dir: Dir::Right },
        TransitionKind::Slide { dir: Dir::Up },
        TransitionKind::Zoom { amount: 0.4, cx: 0.5, cy: 0.5 },
        TransitionKind::Zoom { amount: 1.5, cx: 0.25, cy: 0.6 },
        TransitionKind::Spin { turns: 1.0, amount: 0.4, cx: 0.5, cy: 0.5 },
        TransitionKind::Spin { turns: -0.5, amount: 0.0, cx: 0.8, cy: 0.2 },
        TransitionKind::Whip { dir: Dir::Left, blur: 0.6 },
        TransitionKind::Whip { dir: Dir::Down, blur: 1.0 },
        TransitionKind::BlurDissolve { amount: 24.0 },
        TransitionKind::Pixelate { amount: 48.0 },
        TransitionKind::Glitch { amount: 0.5 },
        TransitionKind::Flash { color_r: 255, color_g: 200, color_b: 40, strength: 1.0 },
        TransitionKind::Dip { color_r: 0, color_g: 0, color_b: 0 },
        // R.51 (item 400): patterns, symbiotic shifts, stylised ones: both axes
        // and both flags for each directional one, as the list header requires
        TransitionKind::Blinds { count: 8, angle: 0.0, softness: 0.1 },
        TransitionKind::Blinds { count: 5, angle: 30.0, softness: 0.0 },
        TransitionKind::Checker { cols: 8, rows: 6, softness: 0.1 },
        TransitionKind::Checker { cols: 5, rows: 7, softness: 0.0 },
        TransitionKind::Blocks { cols: 16, rows: 9, softness: 0.25 },
        TransitionKind::Blocks { cols: 7, rows: 5, softness: 0.05 },
        TransitionKind::Spiral { turns: 2.0, cx: 0.5, cy: 0.5, softness: 0.1 },
        TransitionKind::Spiral { turns: 4.0, cx: 0.3, cy: 0.6, softness: 0.05 },
        TransitionKind::Split { vertical: false },
        TransitionKind::Split { vertical: true },
        TransitionKind::Squeeze { dir: Dir::Left },
        TransitionKind::Squeeze { dir: Dir::Right },
        TransitionKind::Squeeze { dir: Dir::Up },
        TransitionKind::Squeeze { dir: Dir::Down },
        TransitionKind::Stretch { vertical: false },
        TransitionKind::Stretch { vertical: true },
        TransitionKind::Swap { vertical: false, depth: 0.3 },
        TransitionKind::Swap { vertical: true, depth: 0.7 },
        TransitionKind::Shake { amount: 0.5, steps: 12 },
        TransitionKind::Shake { amount: 1.0, steps: 3 },
        TransitionKind::FilmBurn { scale: 6.0, softness: 0.2, tint_r: 255, tint_g: 120, tint_b: 0 },
        TransitionKind::FilmBurn {
            scale: 14.0,
            softness: 0.05,
            tint_r: 200,
            tint_g: 40,
            tint_b: 20,
        },
        TransitionKind::LightLeaks { strength: 0.6, tint_r: 255, tint_g: 90, tint_b: 40 },
        TransitionKind::LightLeaks { strength: 1.0, tint_r: 80, tint_g: 160, tint_b: 255 },
        TransitionKind::ZoomBlur { amount: 0.6, cx: 0.5, cy: 0.5 },
        TransitionKind::ZoomBlur { amount: 1.0, cx: 0.2, cy: 0.8 },
    ];
    let mut seen = std::collections::BTreeSet::new();
    for kind in &cases {
        if matches!(kind, TransitionKind::Crossfade) {
            continue; // the integer path, held by t182
        }
        let mix = kt_engine::TransMix::of(kind);
        seen.insert(mix.mode);
        // several phases: for the transitions the formula depends non-linearly on k.
        // 1800 = 0.44 is a phase BETWEEN a quarter and the middle: without it
        // the mutation “the dip phase threshold shifted from 0.5 to 0.4” got past.
        for k in [200u32, 1024, 1800, 2048, 3600] {
            let p = plan(
                k as i64,
                vec![PlanLayer::Dissolve { a: a.layer(), b: b.layer(), k, alpha_k: 4096, mix }],
            );
            let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
            assert!(cc && cg, "{}: the plan must be complete", kind.tag());
            assert_frames_close(&cpu, &gpu, W, H, soft, &format!("{}@{k}", kind.tag()));
        }
    }
    // volumetric (R.51, item 401): the hard edges of the panels and the perspective
    // division: the GEOM tolerance, as for pan3d (a pixel on an edge may
    // flip over entirely)
    let hard: Vec<TransitionKind> = vec![
        TransitionKind::Flip { vertical: false, persp: 0.5, shade: 0.5 },
        TransitionKind::Flip { vertical: true, persp: 1.0, shade: 0.0 },
        TransitionKind::Cube { dir: Dir::Left, persp: 0.5, shade: 0.5 },
        TransitionKind::Cube { dir: Dir::Right, persp: 0.2, shade: 0.0 },
        TransitionKind::Cube { dir: Dir::Up, persp: 0.8, shade: 0.3 },
        TransitionKind::Door { vertical: false, persp: 0.5, shade: 0.5 },
        TransitionKind::Door { vertical: true, persp: 0.9, shade: 0.2 },
        TransitionKind::PagePeel { dir: Dir::Left, curl: 0.18, shadow: 0.5 },
        TransitionKind::PagePeel { dir: Dir::Down, curl: 0.35, shadow: 1.0 },
        TransitionKind::PageRoll { dir: Dir::Right, curl: 0.18, shadow: 0.5 },
        TransitionKind::PageRoll { dir: Dir::Up, curl: 0.3, shadow: 0.0 },
    ];
    for kind in &hard {
        let mix = kt_engine::TransMix::of(kind);
        seen.insert(mix.mode);
        for k in [200u32, 1024, 1800, 2048, 3600] {
            let p = plan(
                k as i64,
                vec![PlanLayer::Dissolve { a: a.layer(), b: b.layer(), k, alpha_k: 4096, mix }],
            );
            let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
            assert!(cc && cg, "{}: the plan must be complete", kind.tag());
            assert_frames_close(&cpu, &gpu, W, H, FrameTol::GEOM, &format!("{}@{k}", kind.tag()));
        }
    }
    assert_eq!(seen.len(), 32, "all 32 non-crossfade modes checked: {seen:?}");
    assert_eq!(
        seen.len() + 1,
        TransitionKind::ALL.len(),
        "a new kind of transition must appear in this list too"
    );

    // solo fade WITH A VIEW: the second side is empty, the same code
    for (incoming, kind) in [
        (true, TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.2 }),
        (false, TransitionKind::Iris { shape: IrisShape::Circle, cx: 0.5, cy: 0.5, softness: 0.1 }),
        (true, TransitionKind::Push { dir: Dir::Up }),
        // R.51 representatives: the pattern and the symbiotic shift with a fade “out of emptiness”
        (true, TransitionKind::Blinds { count: 8, angle: 0.0, softness: 0.1 }),
        (false, TransitionKind::Squeeze { dir: Dir::Right }),
    ] {
        let mix = kt_engine::TransMix::of(&kind);
        let p = plan(
            7,
            vec![PlanLayer::One {
                src: a.layer(),
                alpha_k: 4096,
                scale_alpha: true,
                fade: Some(kt_engine::video_plan::SoloFade { k: 1500, mix, incoming }),
            }],
        );
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert!(cc && cg, "{}: the solo fade must be complete", kind.tag());
        assert_frames_close(&cpu, &gpu, W, H, soft, &format!("solo-{}", kind.tag()));
        // the fade MUST hide SOMETHING: otherwise the test is green on “no view”
        let opaque = cpu.chunks_exact(4).filter(|p| p[3] > 250).count();
        let clear = cpu.chunks_exact(4).filter(|p| p[3] < 5).count();
        assert!(
            opaque > 100 && clear > 100,
            "{}: the solo fade must have both an opaque and an empty part ({opaque}/{clear})",
            kind.tag()
        );
    }
}

/// Test 329 (R.47, item 331): a CPU↔GPU conformance cycle over the VIDEO subset of the
/// registry FxInstance::all(): for each effect, a single case and a sandwich
/// `[grid 2×2, effect, zoom ×2]` (for barriers the sandwich gives K=2). The tolerance classes
/// come from the table; the completeness lock is as in t224: a kind without a row fails the
/// test. Slowed+Reverb on video is a time warp, not a pixel filter, and is
/// excluded explicitly.
#[test]
fn t329_parity_cycle_over_the_registry() {
    let Some(engine) = test_engine() else { return };
    // (tag, single tolerance, sandwich tolerance)
    let classes: &[(&str, FrameTol, FrameTol)] = &[
        // R.62: Pan is retired; the “Geometry” node is the affine/Proj of the merged pass
        ("geometry", FrameTol::GEOM, FrameTol::GEOM),
        ("cam_motion", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("mirror", FrameTol::EXACT, FrameTol::BILINEAR),
        ("blur", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("greenscreen", FrameTol::THRESHOLD, FrameTol::THRESHOLD),
        ("edges", FrameTol::THRESHOLD, FrameTol::THRESHOLD),
        ("noise", FrameTol::EXACT, FrameTol::THRESHOLD),
        ("vhs", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("glow", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("event_grid", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("pan3d", FrameTol::GEOM, FrameTol::GEOM),
        // R.47 Kv2: pointwise ops: f32 continuum against f64 (in a sandwich they
        // travel as taps of the merged geometry pass); Posterize: a threshold quantization;
        // CornerPin: projective, like pan3d
        ("color_grade", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("duotone", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("posterize", FrameTol::THRESHOLD, FrameTol::THRESHOLD),
        ("vignette", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("corner_pin", FrameTol::GEOM, FrameTol::GEOM),
        // R.47 Kv3: barriers. Pixelate: an integer copy (EXACT
        // single); drop_shadow on an opaque input: an identity by the formula;
        // halftone: the hard edge of a dot (a threshold class)
        ("sharpen", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("pixelate", FrameTol::EXACT, FrameTol::BILINEAR),
        ("chromatic_aberration", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("radial_blur", FrameTol::BILINEAR, FrameTol::BILINEAR),
        ("drop_shadow", FrameTol::EXACT, FrameTol::BILINEAR),
        ("halftone", FrameTol::THRESHOLD, FrameTol::THRESHOLD),
        // R.47 Kv4: the geometric Warp prim
        ("lens_distort", FrameTol::GEOM, FrameTol::GEOM),
        ("twirl", FrameTol::GEOM, FrameTol::GEOM),
        ("ripple", FrameTol::GEOM, FrameTol::GEOM),
        ("kaleidoscope", FrameTol::GEOM, FrameTol::GEOM),
        ("cylinder3d", FrameTol::GEOM, FrameTol::GEOM),
    ];
    let table: std::collections::BTreeMap<&str, (FrameTol, FrameTol)> =
        classes.iter().map(|(t, a, b)| (*t, (*a, *b))).collect();
    let video: Vec<&FxInstance> = FxInstance::all()
        .iter()
        .filter(|f| {
            !f.is_audio() && !f.is_layer_mix() && !matches!(f, FxInstance::SlowedReverb { .. })
        })
        .collect();
    let tags: std::collections::BTreeSet<&str> = video.iter().map(|f| f.tag()).collect();
    assert_eq!(
        table.keys().copied().collect::<std::collections::BTreeSet<_>>(),
        tags,
        "t329 class table and the video registry must cover each other"
    );
    let grid2 = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let zoom2 = FxInstance::geometry_zoom(2.0);
    for (i, f) in video.iter().enumerate() {
        let (tol_single, tol_sand) = table[f.tag()];
        let cam = matches!(f, FxInstance::Pan3d { .. })
            .then(|| kt_engine::Cam3d { cam: kt_model::CameraParams::default(), world_scale: 1.0 });
        // single
        let mut src =
            Src::new(9000 + i as u64 * 2, Some(frame(W, H, false, 0)), vec![(*f).clone()]);
        src.cam3d = cam;
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "{}: complete diverged (single)", f.tag());
        assert_frames_close(&cpu, &gpu, W, H, tol_single, f.tag());
        // sandwich
        let mut src = Src::new(
            9001 + i as u64 * 2,
            Some(frame(W, H, false, 0)),
            vec![grid2.clone(), (*f).clone(), zoom2.clone()],
        );
        src.cam3d = cam;
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "{}: complete diverged (sandwich)", f.tag());
        assert_frames_close(&cpu, &gpu, W, H, tol_sand, &format!("sandwich-{}", f.tag()));
    }
    // R.47 Kv2: for color_grade and corner_pin the default is an identity, and the cycle above
    // does not touch their formulas.
    // The configured instances check the maths: single = a barrier pass (or a Proj run),
    // sandwich = a tap of the merged geometry pass (kind 2..5 in geom_fused.wgsl).
    let tuned: &[(FxInstance, FrameTol, FrameTol)] = &[
        (
            FxInstance::ColorGrade { brightness: 0.12, contrast: 0.35, saturation: 1.6, hue: 60.0 },
            FrameTol::BILINEAR,
            FrameTol::BILINEAR,
        ),
        (
            FxInstance::Duotone {
                shadow_r: 40,
                shadow_g: 0,
                shadow_b: 80,
                high_r: 255,
                high_g: 220,
                high_b: 160,
                amount: 0.7,
            },
            FrameTol::BILINEAR,
            FrameTol::BILINEAR,
        ),
        (FxInstance::Posterize { levels: 4 }, FrameTol::THRESHOLD, FrameTol::THRESHOLD),
        // R.62: an additional “Geometry” node with a crop, shift, zoom and
        // rotation (Proj-affine + Bounds)
        (
            FxInstance::Geometry {
                lead: false,
                crop_l: 0.1,
                crop_t: 0.05,
                crop_r: 0.15,
                crop_b: 0.1,
                pos_x: 0.08,
                pos_y: -0.05,
                zoom: 1.3,
                rot: 20.0,
                anchor_x: 0.4,
                anchor_y: 0.6,
            },
            FrameTol::GEOM,
            FrameTol::GEOM,
        ),
        (
            FxInstance::Vignette { strength: 0.9, radius: 0.3, softness: 0.2 },
            FrameTol::BILINEAR,
            FrameTol::BILINEAR,
        ),
        (
            FxInstance::CornerPin {
                x0: 0.08,
                y0: 0.06,
                x1: 0.92,
                y1: -0.04,
                x2: -0.05,
                y2: 0.95,
                x3: 1.05,
                y3: 1.1,
            },
            FrameTol::GEOM,
            FrameTol::GEOM,
        ),
    ];
    for (i, (f, tol_single, tol_sand)) in tuned.iter().enumerate() {
        let src = Src::new(9600 + i as u64 * 2, Some(frame(W, H, false, 0)), vec![f.clone()]);
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "tuned {}: complete diverged (single)", f.tag());
        assert_frames_close(&cpu, &gpu, W, H, *tol_single, &format!("tuned-{}", f.tag()));
        let src = Src::new(
            9601 + i as u64 * 2,
            Some(frame(W, H, false, 0)),
            vec![grid2.clone(), f.clone(), zoom2.clone()],
        );
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert_eq!(cc, cg, "tuned {}: complete diverged (sandwich)", f.tag());
        assert_frames_close(&cpu, &gpu, W, H, *tol_sand, &format!("tuned-sandwich-{}", f.tag()));
    }
    // R.47 Kv4: hard warps: f32 trigonometry under maximum load
    // (measurement: both backends of the session hold GEOM; a failure here means
    // an investigation and, upon confirmation, the introduction of a WARP ×2 class based on the measurement,
    // NOT a tweak of GEOM)
    for (i, f) in [
        FxInstance::LensDistort { k1: -0.8, k2: 0.5 },
        FxInstance::Twirl { angle: 540.0, radius: 1.2 },
        FxInstance::Ripple { amount: 0.3, freq: 30.0, phase: 137.0 },
        FxInstance::Kaleidoscope { segments: 12, angle: 30.0 },
        FxInstance::Cylinder3d { curvature: 0.95 },
    ]
    .into_iter()
    .enumerate()
    {
        let src = Src::new(9800 + i as u64 * 2, Some(frame(W, H, false, 0)), vec![f.clone()]);
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, ..) = run_both(engine.as_ref(), &p);
        assert_frames_close(&cpu, &gpu, W, H, FrameTol::GEOM, &format!("warp-hard-{}", f.tag()));
        let src = Src::new(
            9801 + i as u64 * 2,
            Some(frame(W, H, false, 0)),
            vec![grid2.clone(), f.clone(), zoom2.clone()],
        );
        let p = plan(90_000, vec![one(&src, 4096, false)]);
        let (cpu, gpu, ..) = run_both(engine.as_ref(), &p);
        assert_frames_close(
            &cpu,
            &gpu,
            W,
            H,
            FrameTol::GEOM,
            &format!("warp-hard-sandwich-{}", f.tag()),
        );
    }
    // R.47 Kv3: the spin branch of radial blur (an arc: f32 against f64 trigonometry)
    // and the shadow on REAL alpha (after greenscreen: THRESHOLD, the key’s threshold
    // breaks the bilinear smoothness)
    let spin = FxInstance::RadialBlur { amount: 0.6, center_x: 0.2, center_y: -0.1, spin: true };
    let src = Src::new(9700, Some(frame(W, H, false, 0)), vec![spin.clone()]);
    let p = plan(90_000, vec![one(&src, 4096, false)]);
    let (cpu, gpu, ..) = run_both(engine.as_ref(), &p);
    assert_frames_close(&cpu, &gpu, W, H, FrameTol::THRESHOLD, "tuned-radial-spin");
    let shadow_chain =
        vec![FxInstance::greenscreen([0, 255, 0], 70, 25), FxInstance::drop_shadow()];
    let src = Src::new(9702, Some(frame(W, H, false, 0)), shadow_chain.clone());
    let p = plan(90_000, vec![one(&src, 4096, false)]);
    let (cpu, gpu, ..) = run_both(engine.as_ref(), &p);
    assert_frames_close(&cpu, &gpu, W, H, FrameTol::THRESHOLD, "tuned-shadow-alpha");
    let mut with_zoom = shadow_chain;
    with_zoom.push(zoom2.clone());
    let src = Src::new(9704, Some(frame(W, H, false, 0)), with_zoom);
    let p = plan(90_000, vec![one(&src, 4096, false)]);
    let (cpu, gpu, ..) = run_both(engine.as_ref(), &p);
    assert_frames_close(&cpu, &gpu, W, H, FrameTol::THRESHOLD, "tuned-shadow-alpha-zoom");
}

/// Test 714b (R.62): placement by the leading “Geometry” node: GPU against
/// CPU. A vertical frame in 16:9 by the “fit” rule (transparent margins),
/// “as is” (past the edge) and with a rotation; the tolerance class is bilinear, and the margins
/// (alpha 0 against 255) MUST match exactly.
#[test]
fn t714b_lead_geometry_placement_parity() {
    use kt_fx::conform::{ConformRule, LeadCtx};
    let Some(engine) = test_engine() else { return };
    let vertical = frame(36, 64, false, 0);
    let rot = {
        let mut g = kt_model::EventGeometry { rot: 30.0, zoom: 1.2, ..Default::default() };
        g.rule = kt_model::Conform::Fit;
        g.node()
    };
    let cases: Vec<(&str, ConformRule, FxInstance)> = vec![
        ("fit", ConformRule::Fit, FxInstance::geometry()),
        ("fill", ConformRule::Fill, FxInstance::geometry()),
        ("as_is", ConformRule::AsIs, FxInstance::geometry()),
        // “stretch” is absent here: the frame = the canvas, the node is an identity, and the frame
        // MUST arrive already at the canvas size (the decoder contract): this is
        // the former path without a single resample, guarded by t182
        ("rot", ConformRule::Fit, rot),
    ];
    for (name, rule, node) in cases {
        let mut src = Src::new(7, Some(vertical.clone()), vec![node]);
        src.lead = Some(LeadCtx::new(Some((36.0, 64.0)), rule));
        let p = plan(0, vec![one(&src, 4096, false)]);
        let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
        assert!(cc && cg, "{name}: complete on both");
        assert_frames_close(&cpu, &gpu, W, H, FrameTol::BILINEAR, name);
        if name == "fit" {
            // margins: on the left and right the composite is transparent → background (0,0,0,0) on both
            for y in 0..H {
                let i = ((y * W) * 4) as usize;
                assert_eq!(&cpu[i..i + 4], &[0, 0, 0, 0], "cpu left field y={y}");
                assert_eq!(&gpu[i..i + 4], &[0, 0, 0, 0], "gpu left field y={y}");
            }
        }
    }
}

/// Test 737 (R.62, change B): BLEND MODES of a track: conformance GPU
/// against the CPU reference. The `blend_over` kernel lives in two files (CPU in
/// kt-engine, WGSL in composite.wgsl) and is edited in one commit: this
/// test is the guard of their agreement (invariant 7).
///
/// The tolerance class is that of transitions: both sides compute in f32/f64 and
/// round the same way, but the order of operations on the GPU is its own.
#[test]
fn t737_blend_modes_gpu_matches_cpu() {
    use kt_model::BlendMode;
    let Some(engine) = test_engine() else { return };
    // the BOTTOM layer is an opaque background, the TOP one has alpha: this shows both
    // the formula of the mode and that a hole leaves the bottom untouched
    for (i, mode) in BlendMode::ALL.into_iter().enumerate() {
        for level_k in [4096u32, 2048] {
            let below = Src::new(7000 + i as u64, Some(frame(W, H, false, 3)), vec![]);
            let mut above = Src::new(7100 + i as u64, Some(frame(W, H, true, 11)), vec![]);
            above.src_scale = 1;
            let mut layers = vec![one(&below, 4096, false)];
            let mut top = one(&above, level_k, level_k < 4096);
            if let PlanLayer::One { src, .. } = &mut top {
                src.blend_mode = mode;
            }
            layers.push(top);
            let p = plan(90_000, layers);
            let (cpu, gpu, cc, cg) = run_both(engine.as_ref(), &p);
            let name = format!("{mode:?}@{level_k}");
            assert_eq!(cc, cg, "{name}: complete diverged");
            assert_frames_close(&cpu, &gpu, W, H, FrameTol::THRESHOLD, &name);
        }
    }
}
