//! Test 297 (item 307, R.44): the fusion of geometric series — the routing rule
//! and the byte guarantee. A series of length 1 of Pan/CamMotion/Mirror
//! must go through the OLD kernels (the goldens t26 guard the bytes from before R.44);
//! fused series must match the exact permutation where it is mathematically exact,
//! and not depend on transparent audio FX in the chain.

use kt_fx::geom::{GeomExec, build_geom_exec, is_geo};
use kt_fx::video::{Frame, VideoFxCtx, apply_video_chain};
use kt_model::FxInstance;

const W: u32 = 64;
const H: u32 = 36;

fn ctx() -> VideoFxCtx {
    VideoFxCtx {
        out_w: W,
        out_h: H,
        u: 0.5,
        burst_index: 90_000,
        cam3d: None,
        in_scale: 1,
        lead: None,
    }
}

/// Frame as in t26: gradients + green corner.
fn test_frame() -> Frame {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            if x > W / 2 && y > H / 2 {
                rgba[i + 1] = 255;
            } else {
                rgba[i] = (x * 4) as u8;
                rgba[i + 1] = (y * 7) as u8;
                rgba[i + 2] = ((x + y) * 3) as u8;
            }
            rgba[i + 3] = 255;
        }
    }
    Frame { w: W, h: H, rgba, has_alpha: false }
}

fn run(fx: &[FxInstance]) -> Frame {
    apply_video_chain(fx, test_frame(), &ctx())
}

/// Exact mirror of the frame (pixel permutation) — the reference for t297b.
fn mirror_h(f: &Frame) -> Vec<u8> {
    let (w, h) = (f.w as usize, f.h as usize);
    let mut out = vec![0u8; f.rgba.len()];
    for y in 0..h {
        for x in 0..w {
            let s = (y * w + x) * 4;
            let d = (y * w + (w - 1 - x)) * 4;
            out[d..d + 4].copy_from_slice(&f.rgba[s..s + 4]);
        }
    }
    out
}

/// Routing rule: single Pan/CamMotion/Mirror — the old kernels,
/// a single EventGrid and any series ≥ 2 — the fused pass. The decision lives
/// in EXACTLY one place (kt-fx), both backends ask it.
#[test]
fn t297_routing_single_legacy_multi_fused() {
    // R.62: Pan is abolished; the old kernel stayed with a single CamMotion, and the
    // “Geometry” node is ALWAYS fused (it has transparent margins)
    let cam = FxInstance::cam_motion();
    let pan = FxInstance::geometry_at(-0.05, 0.03333, 1.5);
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: true };
    assert!(matches!(
        build_geom_exec(&[&cam], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Legacy
    ));
    assert!(matches!(
        build_geom_exec(&[&pan], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Fused(_)
    ));
    assert!(matches!(
        build_geom_exec(&[&mir], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Legacy
    ));
    assert!(matches!(
        build_geom_exec(&[&grid], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Fused(_)
    ));
    assert!(matches!(
        build_geom_exec(&[&grid, &pan], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Fused(_)
    ));
    assert!(matches!(
        build_geom_exec(&[&mir, &grid], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Fused(_)
    ));
}

/// The mirror AFTER the grid — an exact permutation: a fused series must give
/// byte-for-byte mirror(grid) — proof that the fusion does not rasterize gaps and
/// coincides with the step-by-step path where the latter is exact.
#[test]
fn t297b_grid_then_mirror_is_exact_permutation() {
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: true };
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let fused = run(&[grid.clone(), mir]);
    let reference = run(&[grid]);
    assert_eq!(fused.rgba, mirror_h(&reference), "grid→mirror in one merged pass");
}

/// Audio FX inside a series is transparent: it does not break the fusion and does not change the bytes
/// (the GPU mirror skips it too — a divergence of segmentation is forbidden).
#[test]
fn t297c_audio_fx_inside_run_is_transparent() {
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let gain = FxInstance::Gain { db: -6.0, pan: 0.0 };
    let with_audio = run(&[grid.clone(), gain, mir.clone()]);
    let without = run(&[grid, mir]);
    assert_eq!(with_audio.rgba, without.rgba);
}

/// Two mirrors fuse into an identity: the composition of affine maps in f64 gives
/// identity, the tap in integer coordinates is exact — the input byte-for-byte.
#[test]
fn t297d_double_mirror_fuses_to_identity() {
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let out = run(&[mir.clone(), mir]);
    assert_eq!(out.rgba, test_frame().rgba, "mirror∘mirror ≠ identity");
}

/// A series without resampling steps (only mirrors) does NOT change the frame size:
/// [Mirror, Mirror] on a frame of a foreign size must return a frame of the SAME
/// size (the old path worked in-place; a layer of a foreign size is then silently
/// skipped by the composite — the semantics are kept).
#[test]
fn t297f_mirror_only_run_keeps_dims() {
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let src = Frame { w: 96, h: 54, rgba: vec![7u8; 96 * 54 * 4], has_alpha: false };
    let out = apply_video_chain(&[mir.clone(), mir], src, &ctx());
    assert_eq!((out.w, out.h), (96, 54), "mirrors do not resample");
    assert!(out.rgba.iter().all(|&b| b == 7), "and do not touch the bytes");
}

/// Test 298 (item 308, R.44): Pan3d on the CPU — the edge rules of the inverse
/// homography. Identity at the defaults is byte-for-byte; scale<1 — transparent
/// margins and an opaque centre; rotation to an edge (points behind the camera, w≤0) —
/// transparency without panic or garbage; without a camera — passthrough.
#[test]
fn t298_pan3d_cpu_edges() {
    use kt_fx::geom::Cam3d;
    let p3 = |rot_x: f64, scale: f64| FxInstance::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x,
        rot_y: 0.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale,
        camera: None,
    };
    let cam = Some(Cam3d { cam: kt_model::CameraParams::default(), world_scale: 1.0 });
    let ctx3d = VideoFxCtx { cam3d: cam, ..ctx() };

    // identity: defaults + camera “50 mm” at auto-distance = 1:1
    let ident = apply_video_chain(&[p3(0.0, 1.0)], test_frame(), &ctx3d);
    assert_eq!(ident.rgba, test_frame().rgba, "pan3d identity on CPU");

    // scale 0.5: corners transparent, centre opaque, alpha declared
    let half = apply_video_chain(&[p3(0.0, 0.5)], test_frame(), &ctx3d);
    assert!(half.has_alpha);
    let a = |f: &Frame, x: u32, y: u32| f.rgba[((y * W + x) * 4 + 3) as usize];
    assert_eq!(a(&half, 1, 1), 0, "the corner is transparent");
    assert_eq!(a(&half, W / 2, H / 2), 255, "the centre is opaque");

    // rotation to an edge: part of the plane behind the camera (w ≤ 0) — transparent,
    // without NaN/panic and deterministically. The silhouette edge with item 309
    // is anti-aliased (subtaps mix transparent with content), therefore the
    // alpha on the contour is fractional — but there is also clean transparency, and
    // clean content.
    let edge = apply_video_chain(&[p3(110.0, 1.0)], test_frame(), &ctx3d);
    assert!(edge.rgba.chunks_exact(4).any(|p| p[3] == 0), "behind the camera — transparency");
    assert!(edge.rgba.chunks_exact(4).any(|p| p[3] == 255), "the content remained");
    let again = apply_video_chain(&[p3(110.0, 1.0)], test_frame(), &ctx3d);
    assert_eq!(edge.rgba, again.rgba, "determinism");

    // without a camera Pan3d is skipped: passthrough byte-for-byte
    let noop = apply_video_chain(&[p3(30.0, 0.5)], test_frame(), &ctx());
    assert_eq!(noop.rgba, test_frame().rgba, "without the camera — as before");
}

/// Segmentation: a hard barrier (blur) breaks the series, audio does not; the stop index of the
/// layer strength breaks it (since R.45 segmentation lives in plan_chain — item 311/t306).
#[test]
fn t297e_run_end_stops_at_barriers_only() {
    use kt_fx::plan::{PlanOp, plan_chain};
    let fx = vec![
        FxInstance::EventGrid { cols: 2, rows: 2, center: false },
        FxInstance::Gain { db: 0.0, pan: 0.0 },
        FxInstance::Mirror { horizontal: true, vertical: false },
        FxInstance::blur(8),
        FxInstance::EventGrid { cols: 3, rows: 3, center: false },
    ];
    assert!(is_geo(&fx[0]));
    // a run covers grid + (transparent gain) + mirror, but not blur
    let p = plan_chain(&fx, W, H, 1, W, H, 0.5, None, 90_000, &[]);
    assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }));
    assert!(matches!(p.ops[1], PlanOp::Barrier { idx: 3, .. }));
    assert!(matches!(p.ops[2], PlanOp::Run { i: 4, j: 5, .. }));
    // the stop index (snapshot of the layer strength) breaks the run
    let ps = plan_chain(&fx, W, H, 1, W, H, 0.5, None, 90_000, &[2]);
    assert!(matches!(ps.ops[0], PlanOp::Run { i: 0, j: 1, .. }));
    assert!(matches!(ps.ops[1], PlanOp::Snapshot));
    assert!(matches!(ps.ops[2], PlanOp::Run { i: 2, j: 3, .. }));
}

/// 1×1 px checkerboard — the pattern most fragile to resampling.
fn checkerboard() -> Frame {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            let v = if (x + y) % 2 == 0 { 0 } else { 255 };
            rgba[i] = v;
            rgba[i + 1] = v;
            rgba[i + 2] = v;
            rgba[i + 3] = 255;
        }
    }
    Frame { w: W, h: H, rgba, has_alpha: false }
}

/// Average horizontal contrast of the red channel (0..255).
fn contrast(f: &Frame) -> f64 {
    let mut sum = 0u64;
    let mut n = 0u64;
    for y in 0..f.h {
        for x in 1..f.w {
            let i = ((y * f.w + x) * 4) as usize;
            let j = ((y * f.w + x - 1) * 4) as usize;
            sum += (f.rgba[i] as i64 - f.rgba[j] as i64).unsigned_abs();
            n += 1;
        }
    }
    sum as f64 / n as f64
}

/// Test 296 (items 307/309, R.44): a net-unit geo chain RETURNS the detail.
/// The 2×2 grid halved it, the zoom ×2 brought it back — the fused pass
/// samples the SOURCE (positions land exactly on texels), and the checkerboard
/// comes back whole. The step-by-step path (before R.44) rasterized the grid in the
/// half-texel phase: the checkerboard averaged into flat grey, and the zoom
/// enlarged the mush — the contrast fell to zero. The test MUST fail on the
/// old code (checked on commit K2 in a worktree — contrast 0.0).
#[test]
fn t296_net_identity_chain_preserves_detail() {
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let out = apply_video_chain(&[grid, zoom2], checkerboard(), &ctx());
    let c = contrast(&out);
    assert!(c >= 250.0, "the checkerboard must come back whole, contrast {c:.1}");

    // The same principle with perspective (PerPixel supersampling): grid →
    // pan3d identity → grid → pan3d zoom ×4; net ≈ 1, the centre of the frame
    // must keep high contrast (the grid borders blur the edges)
    use kt_fx::geom::Cam3d;
    let ctx3d = VideoFxCtx {
        cam3d: Some(Cam3d { cam: kt_model::CameraParams::default(), world_scale: 1.0 }),
        ..ctx()
    };
    let p3 = |scale: f64| FxInstance::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 0.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale,
        camera: None,
    };
    let grid_c = FxInstance::EventGrid { cols: 2, rows: 2, center: true };
    let out3d =
        apply_video_chain(&[grid_c.clone(), p3(1.0), grid_c, p3(4.0)], checkerboard(), &ctx3d);
    // contrast over the central third of the frame
    let mut sum = 0u64;
    let mut n = 0u64;
    for y in H / 3..2 * H / 3 {
        for x in W / 3 + 1..2 * W / 3 {
            let i = ((y * W + x) * 4) as usize;
            let j = ((y * W + x - 1) * 4) as usize;
            sum += (out3d.rgba[i] as i64 - out3d.rgba[j] as i64).unsigned_abs();
            n += 1;
        }
    }
    let c3 = sum as f64 / n as f64;
    assert!(c3 >= 150.0, "the net unit with pan3d keeps detail, contrast {c3:.1}");
}

/// Supersampling is really enabled: a 4×4 grid (minification ×4) on a noise
/// frame must give LOW variance (the 16-tap box average), not the variance of point bilinear
/// (2 taps out of 4 columns).
#[test]
fn t296b_supersampling_tames_minification_noise() {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for (p, px) in rgba.chunks_exact_mut(4).enumerate() {
        // deterministic “noise” — a multiplicative hash of the position
        let v = ((p as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 56) as u8;
        px[0] = v;
        px[1] = v;
        px[2] = v;
        px[3] = 255;
    }
    let src = Frame { w: W, h: H, rgba, has_alpha: false };
    let var = |f: &Frame, x0: u32, x1: u32, y0: u32, y1: u32| {
        let mut vals = Vec::new();
        for y in y0..y1 {
            for x in x0..x1 {
                vals.push(f.rgba[((y * f.w + x) * 4) as usize] as f64);
            }
        }
        let m = vals.iter().sum::<f64>() / vals.len() as f64;
        vals.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / vals.len() as f64
    };
    let src_var = var(&src, 0, W, 0, H);
    let out = apply_video_chain(
        &[FxInstance::EventGrid { cols: 4, rows: 4, center: false }],
        src,
        &ctx(),
    );
    // the first tile (16×9) — the frame reduced four times
    let out_var = var(&out, 0, 16, 0, 9);
    assert!(
        out_var < src_var / 6.0,
        "box supersampling must suppress noise: {out_var:.0} against {src_var:.0} at the source"
    );
}

/// Test 300 (item 309): the tap formula and the supersampling enabling threshold.
#[test]
fn t300_ss_taps_and_routing_boundary() {
    use kt_fx::geom::ss_taps;
    // quantum 1/32: enabled strictly when len > 1 + 1/64
    assert_eq!(ss_taps(1.0), 1);
    assert_eq!(ss_taps(1.01), 1);
    assert_eq!(ss_taps(1.015624), 1);
    assert_eq!(ss_taps(1.015625), 2); // exactly 1 + 1/64
    assert_eq!(ss_taps(2.0), 2);
    assert_eq!(ss_taps(2.04), 3);
    assert_eq!(ss_taps(3.97), 4);
    assert_eq!(ss_taps(100.0), 4); // the ceiling

    // CamMotion without minification — the old kernel (bytes from before R.44); the
    // “Geometry” node with minification — the fused pass for the sake of supersampling
    let pan1 = FxInstance::cam_motion();
    let pan_min = FxInstance::geometry_zoom(0.9);
    assert!(matches!(
        build_geom_exec(&[&pan1], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Legacy
    ));
    assert!(matches!(
        build_geom_exec(&[&pan_min], W, H, W, H, 0.5, None, 90_000, 1, 1),
        GeomExec::Fused(_)
    ));

    // determinism of the fused supersampling
    let a = run(std::slice::from_ref(&pan_min));
    let b = run(std::slice::from_ref(&pan_min));
    assert_eq!(a.rgba, b.rgba);
}

/// Measurement (not a test): ns/px of the fused pass at 1080p. Run:
/// `cargo test -p kt-fx --release --test geo_fuse -- --ignored --nocapture`
/// judge speed ONLY by the release build (item 301).
#[test]
#[ignore]
fn bench_geom_1080p() {
    use kt_fx::geom::Cam3d;
    use std::time::Instant;
    let (bw, bh) = (1920u32, 1080u32);
    let mut rgba = vec![0u8; (bw * bh * 4) as usize];
    for (p, px) in rgba.chunks_exact_mut(4).enumerate() {
        px[0] = (p % 251) as u8;
        px[1] = (p % 241) as u8;
        px[2] = (p % 239) as u8;
        px[3] = 255;
    }
    let mk = || Frame { w: bw, h: bh, rgba: rgba.clone(), has_alpha: false };
    let bctx = VideoFxCtx {
        out_w: bw,
        out_h: bh,
        u: 0.5,
        burst_index: 90_000,
        cam3d: None,
        in_scale: 1,
        lead: None,
    };
    let ctx3d = VideoFxCtx {
        cam3d: Some(Cam3d { cam: kt_model::CameraParams::default(), world_scale: 1.0 }),
        ..bctx
    };
    let p3 = |scale: f64| FxInstance::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 10.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale,
        camera: None,
    };
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: true };
    let npx = (bw * bh) as f64;
    let bench = |name: &str, fx: &[FxInstance], c: &VideoFxCtx| {
        let t0 = Instant::now();
        let out = apply_video_chain(fx, mk(), c);
        let dt = t0.elapsed();
        println!(
            "{name}: {:.1} ms/frame 1080p, {:.1} ns/px (output {}x{})",
            dt.as_secs_f64() * 1e3,
            dt.as_secs_f64() * 1e9 / npx,
            out.w,
            out.h
        );
    };
    bench("pan legacy (zoom 1.5)", &[FxInstance::geometry_zoom(1.5)], &bctx);
    bench("grid 2x2 fused + SS 2x2", std::slice::from_ref(&grid), &bctx);
    bench(
        "user chain grid→pan3d→grid→pan3d(zoom)",
        &[grid.clone(), p3(1.0), grid.clone(), p3(4.0)],
        &ctx3d,
    );
    bench("single pan3d (perpixel probes)", &[p3(0.9)], &ctx3d);
    // K× barriers (item 314): at 1080p k_cap = 4096/1920 = 2 — the barrier segment
    // runs at 3840×2160
    let zoom2 = FxInstance::geometry_zoom(2.0);
    bench(
        "grid→vhs(K=2)→zoom, barrier at 3840×2160",
        &[
            grid.clone(),
            FxInstance::Vhs { rgb_shift: 2, bleed: 0.3, scan_step: 5, jitter: 0.4 },
            zoom2.clone(),
        ],
        &bctx,
    );
    bench("grid→blur(K=2)→zoom", &[grid.clone(), FxInstance::blur(8), zoom2.clone()], &bctx);
    bench(
        "grid→greenscreen(tap)→zoom",
        &[grid, FxInstance::greenscreen([0, 255, 0], 70, 25), zoom2],
        &bctx,
    );
}

/// R.47 perf print (item 336): the cost of the new effects at 1080p, ns/px.
/// `cargo test -p kt-fx --test geo_fuse --release -- --ignored bench_fx`
#[test]
#[ignore]
fn bench_fx_1080p() {
    use std::time::Instant;
    let (bw, bh) = (1920u32, 1080u32);
    let mut rgba = vec![0u8; (bw * bh * 4) as usize];
    for (p, px) in rgba.chunks_exact_mut(4).enumerate() {
        px[0] = (p % 251) as u8;
        px[1] = (p % 241) as u8;
        px[2] = (p % 239) as u8;
        px[3] = 255;
    }
    let mk = || Frame { w: bw, h: bh, rgba: rgba.clone(), has_alpha: false };
    let bctx = VideoFxCtx {
        out_w: bw,
        out_h: bh,
        u: 0.5,
        burst_index: 90_000,
        cam3d: None,
        in_scale: 1,
        lead: None,
    };
    let npx = (bw * bh) as f64;
    let bench = |name: &str, fx: &[FxInstance]| {
        let t0 = Instant::now();
        let out = apply_video_chain(fx, mk(), &bctx);
        let dt = t0.elapsed();
        println!(
            "{name}: {:.1} ms/frame 1080p, {:.1} ns/px (output {}x{})",
            dt.as_secs_f64() * 1e3,
            dt.as_secs_f64() * 1e9 / npx,
            out.w,
            out.h
        );
    };
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: true };
    bench(
        "colorgrade+vignette (2 barriers)",
        &[
            FxInstance::ColorGrade { brightness: 0.05, contrast: 0.3, saturation: 1.4, hue: 20.0 },
            FxInstance::vignette(),
        ],
    );
    bench("sharpen r=3", &[FxInstance::Sharpen { amount: 1.5, radius: 3 }]);
    bench("pixelate 12", &[FxInstance::Pixelate { size: 12 }]);
    bench("chroma_ab 0.02", &[FxInstance::ChromaticAberration { amount: 0.02 }]);
    bench(
        "radial_blur zoom (16 taps)",
        &[FxInstance::RadialBlur { amount: 0.5, center_x: 0.0, center_y: 0.0, spin: false }],
    );
    bench(
        "radial_blur spin",
        &[FxInstance::RadialBlur { amount: 0.5, center_x: 0.0, center_y: 0.0, spin: true }],
    );
    bench("halftone 8/45", &[FxInstance::Halftone { size: 8, angle: 45.0 }]);
    bench(
        "greenscreen→drop_shadow (shadow with alpha)",
        &[FxInstance::greenscreen([0, 255, 0], 70, 25), FxInstance::drop_shadow()],
    );
    bench(
        "grid→lens→twirl→zoom (fused run, perpixel-SS)",
        &[grid, FxInstance::lens_distort(), FxInstance::twirl(), zoom2],
    );
    bench("kaleidoscope solo", &[FxInstance::kaleidoscope()]);
    bench("cylinder 0.8", &[FxInstance::Cylinder3d { curvature: 0.8 }]);
}

/// Test 306 (item 311, R.45): the chain plan — the single place of segmentation.
/// Runs/barriers/layer-strength markers, the step budget, and “slice plan ≡ piece of the
/// whole plan” (the CPU cuts at the stops one level up, the GPU plans the whole thing with
/// stops — the pictures must match).
#[test]
fn t306_chain_plan_segmentation() {
    use kt_fx::plan::{PlanOp, plan_chain};
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let gain = FxInstance::Gain { db: 0.0, pan: 0.0 };
    let blur = FxInstance::blur(8);

    // run (grid + transparent audio + mirror), barrier, run
    let fx = vec![grid.clone(), gain.clone(), mir.clone(), blur.clone(), grid.clone()];
    let p = plan_chain(&fx, W, H, 1, W, H, 0.5, None, 90_000, &[]);
    assert_eq!(p.ops.len(), 3);
    assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }));
    assert!(matches!(p.ops[1], PlanOp::Barrier { idx: 3, res_scale: 1 }));
    assert!(matches!(p.ops[2], PlanOp::Run { i: 4, j: 5, .. }));

    // layer strength: the snapshot point breaks the run, the slot is a Mix marker
    let fxm = vec![grid.clone(), FxInstance::LayerMix { amount: 0.5, from: 0 }, mir.clone()];
    let pm = plan_chain(&fxm, W, H, 1, W, H, 0.5, None, 90_000, &[0]);
    assert!(matches!(pm.ops[0], PlanOp::Snapshot));
    assert!(matches!(pm.ops[1], PlanOp::Run { i: 0, j: 1, .. }));
    assert!(matches!(pm.ops[2], PlanOp::Mix { idx: 1 }));
    assert!(matches!(pm.ops[3], PlanOp::Run { i: 2, j: 3, .. }));
    assert_eq!(pm.ops.len(), 4);

    // “slice plan ≡ piece of the whole plan”: the non-marker ops match
    let s1 = plan_chain(&fxm[0..1], W, H, 1, W, H, 0.5, None, 90_000, &[]);
    let s2 = plan_chain(&fxm[2..3], W, H, 1, W, H, 0.5, None, 90_000, &[]);
    assert!(matches!(s1.ops[..], [PlanOp::Run { i: 0, j: 1, .. }]));
    assert!(matches!(s2.ops[..], [PlanOp::Run { i: 0, j: 1, .. }]));

    // geo step budget: 7 in a row are cut into 6 + 1 (at the cut point the quality
    // is today's)
    let seven: Vec<FxInstance> = (0..7).map(|_| mir.clone()).collect();
    let p7 = plan_chain(&seven, W, H, 1, W, H, 0.5, None, 90_000, &[]);
    assert_eq!(p7.ops.len(), 2);
    assert!(matches!(p7.ops[0], PlanOp::Run { i: 0, j: 6, .. }));
    assert!(matches!(p7.ops[1], PlanOp::Run { i: 6, j: 7, .. }));
}

/// Test 305 (item 312, R.45): per-pixel Greenscreen/Noise INSIDE a geo series
/// no longer break the fusion. (a) grid → greenscreen → zoom: the key works
/// (the green is cut out), and the sharpness of the checkerboard is not lost — before R.45 the barrier
/// rasterized the grid at 1× and the zoom smeared it. (b) grid → noise → zoom: the grain
/// is deterministic and pinned to the grid of ITS OWN step (it zooms with the picture).
#[test]
fn t305_pointwise_inside_run() {
    use kt_fx::plan::{PlanOp, plan_chain};
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let gs = FxInstance::greenscreen([0, 255, 0], 70, 25);

    // (0) segmentation: an internal per-pixel op is absorbed — the run is ONE
    let fx = vec![grid.clone(), gs.clone(), zoom2.clone()];
    let p = plan_chain(&fx, W, H, 1, W, H, 0.5, None, 90_000, &[]);
    assert_eq!(p.ops.len(), 1, "inner greenscreen absorbed into the run");
    assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }));
    // a leading per-pixel op is NOT absorbed (the bytes of K=1 chains)
    let lead = vec![gs.clone(), zoom2.clone()];
    let pl = plan_chain(&lead, W, H, 1, W, H, 0.5, None, 90_000, &[]);
    assert!(matches!(pl.ops[0], PlanOp::Barrier { idx: 0, .. }));

    // (a) checkerboard with green: the key cuts out the green cells, the sharpness
    // of the rest is kept (net unit grid ×0.5 → zoom ×2)
    let mut cb = checkerboard();
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            if cb.rgba[i] == 255 {
                cb.rgba[i] = 0;
                cb.rgba[i + 1] = 255; // white cells become the green key
                cb.rgba[i + 2] = 0;
            }
        }
    }
    let out = apply_video_chain(&[grid.clone(), gs, zoom2.clone()], cb, &ctx());
    assert!(out.has_alpha, "greenscreen declared alpha also in the fused pass");
    let mut keyed = 0usize;
    let mut kept = 0usize;
    for px in out.rgba.chunks_exact(4) {
        if px[3] == 0 {
            keyed += 1;
        } else if px[3] == 255 {
            kept += 1;
        }
    }
    let total = (W * H) as usize;
    assert!(keyed > total / 3, "green cells keyed out ({keyed}/{total})");
    assert!(kept > total / 3, "dark cells intact ({kept}/{total})");

    // (b) grain: determinism and binding to the step — grain size=1 at the stage
    // BEFORE the zoom is stretched by it into 2×2 blocks of output pixels
    let noise = FxInstance::Noise { amount: 0.9, size: 1, color: false };
    let grey = Frame { w: W, h: H, rgba: vec![128u8; (W * H * 4) as usize], has_alpha: false };
    let mk = || Frame { w: W, h: H, rgba: grey.rgba.clone(), has_alpha: false };
    let a = apply_video_chain(&[grid.clone(), noise.clone(), zoom2.clone()], mk(), &ctx());
    let b = apply_video_chain(&[grid.clone(), noise.clone(), zoom2.clone()], mk(), &ctx());
    assert_eq!(a.rgba, b.rgba, "grain is deterministic");
    // ~2×2 blockiness (the block phase is shifted by the zoom crop — we compare
    // NEIGHBOURS): the grain of the stage before the zoom is stretched ×2, so about half of the
    // horizontal and vertical pixel pairs coincide; the screen-space grain size=1 has almost no
    // matches
    let px = |f: &Frame, x: u32, y: u32| f.rgba[((y * W + x) * 4) as usize];
    let (mut eq_h, mut n_h) = (0usize, 0usize);
    let (mut eq_v, mut n_v) = (0usize, 0usize);
    for y in 0..H {
        for x in 0..W - 1 {
            n_h += 1;
            if px(&a, x, y) == px(&a, x + 1, y) {
                eq_h += 1;
            }
        }
    }
    for y in 0..H - 1 {
        for x in 0..W {
            n_v += 1;
            if px(&a, x, y) == px(&a, x, y + 1) {
                eq_v += 1;
            }
        }
    }
    assert!(
        eq_h * 100 >= n_h * 35 && eq_v * 100 >= n_v * 35,
        "grain pinned to the step (stretched by zoom): equal neighbours {eq_h}/{n_h} along X, {eq_v}/{n_v} along Y"
    );
    // and the grain really IS there (not eaten by averaging)
    assert!(a.rgba.chunks_exact(4).any(|p| p[0] != 128), "grain is visible");
}

/// Test 303 (item 313, R.45): kernel invariance of the barrier look to resolution, WITHOUT chains:
/// `down_K(kernel_K(up_K(F))) ≈ kernel_1(F)`
/// (up/down — the same bilinear fit_into as all the resamples). Thresholds ×2 from the
/// measurement (rule of kt-gpu/CLAUDE.md); edges — a threshold class (binary
/// decisions on interpolated luma legitimately flip).
#[test]
fn t303_barrier_kernels_res_invariant() {
    use kt_fx::video::{FxScratch, apply_barrier, fit_into};
    // Measurement (K=2 and K=4, frame t26): worst mean 1.04 (vhs), worst p99 24
    // (vhs K=2); thresholds ×2.
    let cases: Vec<(&str, FxInstance)> = vec![
        ("blur", FxInstance::blur(8)),
        ("glow", FxInstance::glow(0.9)),
        ("vhs", FxInstance::Vhs { rgb_shift: 4, bleed: 0.4, scan_step: 5, jitter: 0.5 }),
        ("noise", FxInstance::Noise { amount: 0.5, size: 2, color: true }),
        ("noise-special", FxInstance::noise(0.5)),
    ];
    let mut scratch = FxScratch::default();
    for k in [2u32, 4] {
        for (name, fx) in &cases {
            let base = apply_barrier(fx, test_frame(), &ctx(), &mut scratch, 1);
            let (kw, kh) = (W * k, H * k);
            let mut up = vec![0u8; (kw * kh * 4) as usize];
            fit_into(&test_frame(), kw, kh, &mut up);
            let upf = Frame { w: kw, h: kh, rgba: up, has_alpha: false };
            let kf = apply_barrier(fx, upf, &ctx(), &mut scratch, k);
            assert_eq!((kf.w, kf.h), (kw, kh), "{name}: kernel does not change size");
            let mut down = vec![0u8; (W * H * 4) as usize];
            fit_into(&kf, W, H, &mut down);
            let mut diffs: Vec<u32> = base
                .rgba
                .iter()
                .zip(&down)
                .map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs())
                .collect();
            diffs.sort_unstable();
            let mean = diffs.iter().map(|&d| d as u64).sum::<u64>() as f64 / diffs.len() as f64;
            let p99 = diffs[diffs.len() * 99 / 100] as f64;
            println!("t303 {name} K={k}: mean {mean:.2}, p99 {p99}");
            assert!(
                mean <= 2.5 && p99 <= 48.0,
                "{name} K={k}: view moved (mean {mean:.2}, p99 {p99})"
            );
        }
        // R.47 Kv3 (smooth): thresholds ×2 from the measurement (see printout)
        for (name, fx) in [
            ("sharpen", FxInstance::Sharpen { amount: 2.0, radius: 4 }),
            ("chroma", FxInstance::ChromaticAberration { amount: 0.03 }),
            (
                "radial-zoom",
                FxInstance::RadialBlur { amount: 0.5, center_x: 0.0, center_y: 0.0, spin: false },
            ),
            (
                "radial-spin",
                FxInstance::RadialBlur { amount: 0.5, center_x: 0.2, center_y: 0.1, spin: true },
            ),
        ] {
            let base = apply_barrier(&fx, test_frame(), &ctx(), &mut scratch, 1);
            let (kw, kh) = (W * k, H * k);
            let mut up = vec![0u8; (kw * kh * 4) as usize];
            fit_into(&test_frame(), kw, kh, &mut up);
            let upf = Frame { w: kw, h: kh, rgba: up, has_alpha: false };
            let kf = apply_barrier(&fx, upf, &ctx(), &mut scratch, k);
            let mut down = vec![0u8; (W * H * 4) as usize];
            fit_into(&kf, W, H, &mut down);
            let mut diffs: Vec<u32> = base
                .rgba
                .iter()
                .zip(&down)
                .map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs())
                .collect();
            diffs.sort_unstable();
            let mean = diffs.iter().map(|&d| d as u64).sum::<u64>() as f64 / diffs.len() as f64;
            let p99 = diffs[diffs.len() * 99 / 100] as f64;
            println!("t303 {name} K={k}: mean {mean:.2}, p99 {p99}");
            assert!(
                mean <= 2.5 && p99 <= 48.0,
                "{name} K={k}: view moved (mean {mean:.2}, p99 {p99})"
            );
        }
        // the shadow needs real alpha: a hard circle in the centre
        {
            let alpha_frame = || {
                let mut f = test_frame();
                f.has_alpha = true;
                for y in 0..H {
                    for x in 0..W {
                        let (dx, dy) = (x as f64 - W as f64 / 2.0, y as f64 - H as f64 / 2.0);
                        let i = ((y * W + x) * 4 + 3) as usize;
                        f.rgba[i] = if dx * dx + dy * dy < 144.0 { 255 } else { 0 };
                    }
                }
                f
            };
            let fx = FxInstance::drop_shadow();
            let base = apply_barrier(&fx, alpha_frame(), &ctx(), &mut scratch, 1);
            let (kw, kh) = (W * k, H * k);
            let mut up = vec![0u8; (kw * kh * 4) as usize];
            fit_into(&alpha_frame(), kw, kh, &mut up);
            let upf = Frame { w: kw, h: kh, rgba: up, has_alpha: true };
            let kf = apply_barrier(&fx, upf, &ctx(), &mut scratch, k);
            let mut down = vec![0u8; (W * H * 4) as usize];
            fit_into(&kf, W, H, &mut down);
            let mut diffs: Vec<u32> = base
                .rgba
                .iter()
                .zip(&down)
                .map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs())
                .collect();
            diffs.sort_unstable();
            let mean = diffs.iter().map(|&d| d as u64).sum::<u64>() as f64 / diffs.len() as f64;
            let p99 = diffs[diffs.len() * 99 / 100] as f64;
            println!("t303 drop-shadow K={k}: mean {mean:.2}, p99 {p99}");
            // the shadow edge on HARD alpha (a binary circle) shifts by the mip
            // phase — its own thresholds, ×2 from the measurement (K=2: 2.14/56)
            assert!(
                mean <= 4.5 && p99 <= 112.0,
                "drop-shadow K={k}: view moved (mean {mean:.2}, p99 {p99})"
            );
        }
        // halftone: the K× execution antialiases the dot edges (that is its
        // gain, not a defect) — a per-pixel metric is unsuitable.
        // What is compared is the COVERAGE: the mean luma of 8×8 blocks (the position and size of the
        // dots), thresholds ×2 from the measurement (see printout).
        {
            let fx = FxInstance::Halftone { size: 8, angle: 45.0 };
            let base = apply_barrier(&fx, test_frame(), &ctx(), &mut scratch, 1);
            let (kw, kh) = (W * k, H * k);
            let mut up = vec![0u8; (kw * kh * 4) as usize];
            fit_into(&test_frame(), kw, kh, &mut up);
            let upf = Frame { w: kw, h: kh, rgba: up, has_alpha: false };
            let kf = apply_barrier(&fx, upf, &ctx(), &mut scratch, k);
            let mut down = vec![0u8; (W * H * 4) as usize];
            fit_into(&kf, W, H, &mut down);
            let block = 8u32;
            let mut worst = 0.0f64;
            for by in 0..H / block {
                for bx in 0..W / block {
                    let (mut sa, mut sb) = (0u64, 0u64);
                    for y in by * block..(by + 1) * block {
                        for x in bx * block..(bx + 1) * block {
                            let i = ((y * W + x) * 4) as usize;
                            sa += base.rgba[i] as u64;
                            sb += down[i] as u64;
                        }
                    }
                    let n = (block * block) as f64;
                    worst = worst.max((sa as f64 / n - sb as f64 / n).abs());
                }
            }
            println!("t303 halftone K={k}: worst block delta {worst:.1}");
            // measurement: 14.0 (K=2), 6.0 (K=4); threshold ×2
            assert!(worst <= 28.0, "halftone K={k}: pattern moved (block {worst:.1})");
        }
        // edges — a threshold class: we compare the FRACTION of pixels that diverged
        for (name, fx) in [
            ("edges", FxInstance::edges(48)),
            (
                "edges-thick",
                FxInstance::Edges {
                    threshold: 48,
                    thickness: 2,
                    color_r: 255,
                    color_g: 255,
                    color_b: 255,
                },
            ),
            // threshold Kv3: the cell phase at the boundary legitimately flips
            ("pixelate", FxInstance::Pixelate { size: 8 }),
        ] {
            let base = apply_barrier(&fx, test_frame(), &ctx(), &mut scratch, 1);
            let (kw, kh) = (W * k, H * k);
            let mut up = vec![0u8; (kw * kh * 4) as usize];
            fit_into(&test_frame(), kw, kh, &mut up);
            let upf = Frame { w: kw, h: kh, rgba: up, has_alpha: false };
            let kf = apply_barrier(&fx, upf, &ctx(), &mut scratch, k);
            let mut down = vec![0u8; (W * H * 4) as usize];
            fit_into(&kf, W, H, &mut down);
            let over = base
                .rgba
                .iter()
                .zip(&down)
                .filter(|(a, b)| (**a as i32 - **b as i32).unsigned_abs() > 16)
                .count();
            let frac = over as f64 / base.rgba.len() as f64;
            println!("t303 {name} K={k}: |Δ|>16 at {:.2}%", frac * 100.0);
            // measurement: 0.88 % (K=2); threshold ×2
            assert!(frac <= 0.02, "{name} K={k}: outlines diverged ({:.2}%)", frac * 100.0);
        }
    }
}

/// Test 302 (item 314, R.45): the BARRIER between the grid and the zoom no longer
/// kills the detail — vhs with zero parameters (the identity kernel) runs
/// at K×, and the checkerboard passes the chain [grid 2×2 → vhs → zoom ×2] whole.
/// Before K was enabled (items 311-313) the same test gave a contrast of ~0 — the grid
/// was rasterized at 1× before the barrier
/// (red-proof run on this same tree before the commit of item 314).
#[test]
fn t302_barrier_no_longer_kills_detail() {
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let vhs0 = FxInstance::Vhs { rgb_shift: 0, bleed: 0.0, scan_step: 0, jitter: 0.0 };
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let out = apply_video_chain(&[grid, vhs0, zoom2], checkerboard(), &ctx());
    let c = contrast(&out);
    assert!(c >= 200.0, "checkerboard through the K× barrier, contrast {c:.1}");
}

/// Test 304 (item 314): the rule of K. Without a net zoom of the tail OR without compression above
/// in the chain — K=1 (the bytes of items 311-313: enabling K does not touch them); compression +
/// zoom — K>1; neighbouring barriers share one K; a strength slot breaks it.
#[test]
fn t304_k_only_with_squeeze_and_zoom() {
    use kt_fx::plan::{PlanOp, plan_chain};
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let vhs = FxInstance::vhs(4, 0.4);
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let zoom05 = FxInstance::geometry_zoom(0.5);
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let k_of = |fx: &[FxInstance]| -> Vec<u32> {
        plan_chain(fx, W, H, 1, W, H, 0.5, None, 90_000, &[])
            .ops
            .iter()
            .filter_map(|op| match op {
                PlanOp::Barrier { res_scale, .. } => Some(*res_scale),
                _ => None,
            })
            .collect()
    };
    assert_eq!(k_of(&[grid.clone(), vhs.clone()]), vec![1], "tail is empty");
    assert_eq!(k_of(&[grid.clone(), vhs.clone(), mir.clone()]), vec![1], "tail without zoom");
    assert_eq!(k_of(&[grid.clone(), vhs.clone(), zoom05.clone()]), vec![1], "tail compresses");
    // squeeze-cap: with 1× input there is nowhere to take detail above — K=1, the extra
    // buffer is not bought (the source of detail is the decode step — t308)
    assert_eq!(k_of(&[vhs.clone(), zoom2.clone()]), vec![1], "no more compression above");
    assert_eq!(k_of(&[grid.clone(), vhs.clone(), zoom2.clone()]), vec![2], "compression+zoom → K=2");
    // neighbouring barriers — one K (there are no resamples between barriers)
    assert_eq!(k_of(&[grid.clone(), vhs.clone(), FxInstance::blur(8), zoom2.clone()]), vec![2, 2]);
    // t68 user chain: the tail after barriers does not zoom — K=1
    assert_eq!(
        k_of(&[mir.clone(), grid.clone(), FxInstance::glow(1.0), FxInstance::vhs(2, 0.5)]),
        vec![1, 1]
    );
    // a strength slot breaks both accumulators: the barrier before Mix stays 1×
    assert_eq!(
        k_of(&[grid.clone(), vhs.clone(), FxInstance::LayerMix { amount: 0.5, from: 0 }, zoom2]),
        vec![1]
    );
    // byte anchor: a K=1 chain with a barrier byte-for-byte with items 311-313
    // (enabling K did not touch it); on a deliberate change — a new hash from the
    // output, as for t26
    let f = run(&[grid, vhs]);
    let hash = xxhash_rust::xxh3::xxh3_64(&f.rgba);
    println!("t304 anchor: 0x{hash:016x}");
    assert_eq!(hash, 0x1a3aad166d0a0c18, "K=1 bytes moved");
}

/// Test 308 (item 317, R.46): the decode step feeds the compression budget. A frame
/// that came at S× the logical size is REALLY squeezed detail: the squeeze-cap stops
/// cutting K where there is no grid higher up the chain, and the leading barrier
/// (there was no resample before it) is computed directly at S×.
#[test]
fn t308_in_scale_feeds_the_squeeze_budget() {
    use kt_fx::plan::{PlanOp, plan_chain};
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let vhs = FxInstance::vhs(4, 0.4);
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let zoom12 = FxInstance::geometry_zoom(1.2);
    let k_of = |fx: &[FxInstance], s: u32| -> Vec<u32> {
        plan_chain(fx, W * s, H * s, s, W, H, 0.5, None, 90_000, &[])
            .ops
            .iter()
            .filter_map(|op| match op {
                PlanOp::Barrier { res_scale, .. } => Some(*res_scale),
                _ => None,
            })
            .collect()
    };
    // exactly the case the round was made for: a zoom with no grid above
    assert_eq!(k_of(&[vhs.clone(), zoom2.clone()], 2), vec![2], "step = live detail");
    assert_eq!(k_of(&[vhs.clone(), zoom2.clone()], 1), vec![1], "without a step — the prior K");
    // the leading barrier runs at the frame's resolution, not by benefit:
    // a zoom of 1.2 alone would not give K (the quantum), but the frame is physically 2×
    assert_eq!(k_of(&[vhs.clone(), zoom12], 2), vec![2], "the leading barrier by the physics of the frame");
    // a grid higher up the chain does not cancel the step nor double it beyond the ceiling
    assert_eq!(k_of(&[grid.clone(), vhs.clone(), zoom2.clone()], 1), vec![2], "the prior path is intact");
    // a mirror at equal edges stays the old kernel at the step too
    assert_eq!(k_of(&[mir, vhs, zoom2], 2), vec![2], "Mirror run does not break the segment");
}

/// Test 308b (item 317): a geo run BETWEEN two K× barriers. A single mirror was routed
/// to Legacy unconditionally, while the plan assembly requires 1×-edges
/// — the chain “grid → vhs → mirror → blur → zoom” tripped debug_assert even
/// before R.46 (a latent defect of R.45). Rule: Legacy only when k_in==k_out.
#[test]
fn t308b_mirror_run_between_k_segments() {
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let vhs = FxInstance::vhs(4, 0.4);
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let out = run(&[grid, vhs, mir, FxInstance::blur(8), zoom2]);
    assert_eq!((out.w, out.h), (W, H), "chain must land in Quality");
}

/// pixel luma (the same formula 77/150/29 as in the tap kernels)
fn luma(f: &Frame, x: u32, y: u32) -> f64 {
    let i = ((y * f.w + x) * 4) as usize;
    (77.0 * f.rgba[i] as f64 + 150.0 * f.rgba[i + 1] as f64 + 29.0 * f.rgba[i + 2] as f64) / 256.0
}

/// Test 330 (R.47 Kv2): the new per-pixel effects — ColorGrade/Duotone/Posterize/
/// Vignette. (a) an internal per-pixel effect is absorbed into the geo run (a plan of length
/// 1); (b) the meaning of each at a barrier position; (c) Vignette computes the coordinate of ITS
/// OWN STAGE, not the output: a zoom after it cuts the corners.
#[test]
fn t330_new_pointwise_absorb_and_meaning() {
    use kt_fx::plan::{PlanOp, plan_chain};
    let grid2 = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let zoom2 = FxInstance::geometry_zoom(2.0);

    // (a) absorption: each new per-pixel effect between geo steps — the run is ONE
    for f in [
        FxInstance::color_grade(),
        FxInstance::duotone(),
        FxInstance::posterize(),
        FxInstance::vignette(),
    ] {
        let fx = vec![grid2.clone(), f.clone(), zoom2.clone()];
        let p = plan_chain(&fx, W, H, 1, W, H, 0.5, None, 90_000, &[]);
        assert_eq!(p.ops.len(), 1, "{}: must absorb into the run", f.tag());
        assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }), "{}", f.tag());
    }

    // (b-1) ColorGrade: saturation 0 — the frame is desaturated (r==g==b)
    let gray = run(&[FxInstance::ColorGrade {
        brightness: 0.0,
        contrast: 0.0,
        saturation: 0.0,
        hue: 0.0,
    }]);
    let src = test_frame();
    assert_ne!(gray.rgba, src.rgba, "saturation 0 must change a colored frame");
    for px in gray.rgba.chunks_exact(4) {
        assert!(px[0] == px[1] && px[1] == px[2], "saturation 0 must produce gray: {px:?}");
    }

    // (b-2) Duotone: two colours — the output is only their segment
    let duo = run(&[FxInstance::Duotone {
        shadow_r: 0,
        shadow_g: 0,
        shadow_b: 255,
        high_r: 255,
        high_g: 0,
        high_b: 0,
        amount: 1.0,
    }]);
    for px in duo.rgba.chunks_exact(4) {
        assert_eq!(px[1], 0, "duotone (blue..red) leaves no green");
        let sum = px[0] as i32 + px[2] as i32;
        assert!((254..=256).contains(&sum), "off the shadow..high segment: {px:?}");
    }

    // (b-3) Posterize: no more than levels unique values per channel
    let post = run(&[FxInstance::Posterize { levels: 4 }]);
    for c in 0..3 {
        let uniq: std::collections::BTreeSet<u8> =
            post.rgba.chunks_exact(4).map(|px| px[c]).collect();
        assert!(uniq.len() <= 4, "channel {c}: {} levels out of 4", uniq.len());
        let before: std::collections::BTreeSet<u8> =
            src.rgba.chunks_exact(4).map(|px| px[c]).collect();
        assert!(before.len() > 4, "source must have had detail to lose");
    }

    // (b-4) Vignette: corners darker, centre untouched (radius > r at the centre).
    // The frame is flat grey: in test_frame the corner (0,0) is itself black, and there is
    // nothing to darken there.
    // radius 0.5: the frame corner (r=1) fades to zero, and the stage point under the
    // ZOOM corner (r≈0.48) stays inside the radius — the contrast (c) is full
    let vig = FxInstance::Vignette { strength: 1.0, radius: 0.5, softness: 0.3 };
    let flat = || {
        let mut rgba = vec![200u8; (W * H * 4) as usize];
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255;
        }
        Frame { w: W, h: H, rgba, has_alpha: false }
    };
    let v = apply_video_chain(std::slice::from_ref(&vig), flat(), &ctx());
    let (cx, cy) = (W / 2, H / 2);
    assert!(luma(&v, 0, 0) < 100.0, "corner must darken: {}", luma(&v, 0, 0));
    assert_eq!(
        v.rgba[((cy * W + cx) * 4) as usize..((cy * W + cx) * 4 + 4) as usize],
        [200, 200, 200, 255],
        "inside radius the pixel is untouched"
    );

    // (c) the stage coordinate: [grid, vig, zoom2] vignettes BEFORE the zoom —
    // the zoom cuts the dark corners, and the output corner is lighter than a direct vignette.
    let direct = apply_video_chain(std::slice::from_ref(&vig), flat(), &ctx());
    let fused = apply_video_chain(&[grid2.clone(), vig, zoom2.clone()], flat(), &ctx());
    assert!(
        luma(&fused, 0, 0) > luma(&direct, 0, 0) + 20.0,
        "vignette must use STAGE coords (zoom crops the dark corners): fused {} direct {}",
        luma(&fused, 0, 0),
        luma(&direct, 0, 0)
    );
}

/// Test 331 (R.47 Kv2): CornerPin. The default is identity byte-for-byte (the step
/// is skipped); a compression into the right half puts the marker by the formula and
/// leaves the left one transparent; a perspective quad moves the corner marker
/// to the assigned corner; a degenerate quad — transparency, deterministically;
/// in a series with geo CornerPin stays ONE run.
#[test]
fn t331_corner_pin() {
    use kt_fx::plan::{PlanOp, plan_chain};
    // (1) default: identity byte-for-byte, alpha is not declared
    let out = run(&[FxInstance::corner_pin()]);
    let src = test_frame();
    assert_eq!(out.rgba, src.rgba, "default corners are a byte-exact identity");
    assert!(!out.has_alpha, "skipped step must not declare alpha");

    // marker frame: white field, black 4×4 block in the centre
    let marker = || {
        let mut m = Frame { w: W, h: H, rgba: vec![255u8; (W * H * 4) as usize], has_alpha: false };
        for y in H / 2 - 2..H / 2 + 2 {
            for x in W / 2 - 2..W / 2 + 2 {
                let i = ((y * W + x) * 4) as usize;
                m.rgba[i] = 0;
                m.rgba[i + 1] = 0;
                m.rgba[i + 2] = 0;
            }
        }
        m
    };
    let centroid = |f: &Frame| {
        let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0.0f64);
        for y in 0..f.h {
            for x in 0..f.w {
                let i = ((y * f.w + x) * 4) as usize;
                if f.rgba[i + 3] == 255 && f.rgba[i] < 128 {
                    sx += x as f64;
                    sy += y as f64;
                    n += 1.0;
                }
            }
        }
        assert!(n > 0.0, "marker must survive");
        (sx / n, sy / n)
    };

    // (2) compression into the right half: uv-centre → x = 0.75·W
    let right = FxInstance::CornerPin {
        x0: 0.5,
        y0: 0.0,
        x1: 1.0,
        y1: 0.0,
        x2: 0.5,
        y2: 1.0,
        x3: 1.0,
        y3: 1.0,
    };
    let o = apply_video_chain(std::slice::from_ref(&right), marker(), &ctx());
    assert!(o.has_alpha, "corner pin declares alpha");
    for y in 0..H {
        for x in 0..W / 2 - 1 {
            assert_eq!(o.rgba[((y * W + x) * 4 + 3) as usize], 0, "left half is off-quad");
        }
    }
    let (mx, my) = centroid(&o);
    assert!(
        (mx - 0.75 * W as f64).abs() <= 2.0 && (my - 0.5 * H as f64).abs() <= 2.0,
        "marker must land at (0.75W, 0.5H): got ({mx:.1}, {my:.1})"
    );

    // (3) perspective: the TL corner is pulled inward — the corner marker follows it
    let mut tlm = Frame { w: W, h: H, rgba: vec![255u8; (W * H * 4) as usize], has_alpha: false };
    for y in 0..4u32 {
        for x in 0..4u32 {
            let i = ((y * W + x) * 4) as usize;
            tlm.rgba[i] = 0;
            tlm.rgba[i + 1] = 0;
            tlm.rgba[i + 2] = 0;
        }
    }
    let persp = FxInstance::CornerPin {
        x0: 0.25,
        y0: 0.25,
        x1: 1.0,
        y1: 0.0,
        x2: 0.0,
        y2: 1.0,
        x3: 1.0,
        y3: 1.0,
    };
    let o = apply_video_chain(std::slice::from_ref(&persp), tlm, &ctx());
    let (mx, my) = centroid(&o);
    assert!(
        (mx - 0.25 * W as f64).abs() <= 4.0 && (my - 0.25 * H as f64).abs() <= 4.0,
        "TL marker must follow the pinned corner: got ({mx:.1}, {my:.1})"
    );

    // (4) degenerate quad: transparent and deterministic
    let degen = FxInstance::CornerPin {
        x0: 0.3,
        y0: 0.3,
        x1: 0.3,
        y1: 0.3,
        x2: 0.3,
        y2: 0.3,
        x3: 0.3,
        y3: 0.3,
    };
    let a = apply_video_chain(std::slice::from_ref(&degen), marker(), &ctx());
    let b = apply_video_chain(std::slice::from_ref(&degen), marker(), &ctx());
    assert!(a.rgba.chunks_exact(4).all(|px| px[3] == 0), "degenerate quad is transparent");
    assert_eq!(a.rgba, b.rgba, "and deterministic");

    // (5) a series with geo — one run
    let grid2 = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let zoom2 = FxInstance::geometry_zoom(2.0);
    let fx = vec![grid2, right, zoom2];
    let p = plan_chain(&fx, W, H, 1, W, H, 0.5, None, 90_000, &[]);
    assert_eq!(p.ops.len(), 1, "corner pin is geo: the series stays one run");
    assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }));
}

/// Test 332 (R.47 Kv3): the meaning of six barriers + the alpha contract of the shadow.
#[test]
fn t332_barrier_semantics() {
    use kt_fx::video::{FxScratch, apply_barrier};
    let mut scratch = FxScratch::default();

    // Sharpen: local contrast grows
    let sharp = run(&[FxInstance::Sharpen { amount: 2.0, radius: 4 }]);
    assert!(
        contrast(&sharp) > contrast(&test_frame()) * 1.2,
        "sharpen must raise local contrast: {} vs {}",
        contrast(&sharp),
        contrast(&test_frame())
    );

    // Pixelate: each 12×12 cell is constant
    let pix = run(&[FxInstance::Pixelate { size: 12 }]);
    for cy in 0..H / 12 {
        for cx in 0..W / 12 {
            let base = ((cy * 12 * W + cx * 12) * 4) as usize;
            for y in cy * 12..(cy + 1) * 12 {
                for x in cx * 12..(cx + 1) * 12 {
                    let i = ((y * W + x) * 4) as usize;
                    assert_eq!(
                        pix.rgba[i..i + 3],
                        pix.rgba[base..base + 3],
                        "pixelate cell must be flat at ({x},{y})"
                    );
                }
            }
        }
    }

    // ChromaticAberration: the centre is untouched, at the edge the channels have split
    let ca = run(&[FxInstance::ChromaticAberration { amount: 0.04 }]);
    let src = test_frame();
    let c = (((H / 2) * W + W / 2) * 4) as usize;
    assert_eq!(ca.rgba[c..c + 4], src.rgba[c..c + 4], "chroma: center pixel is a fixed point");
    let edge = ((H / 2 * W + 2) * 4) as usize;
    assert_ne!(ca.rgba[edge], src.rgba[edge], "chroma: red must shift near the edge");

    // RadialBlur: the centre is the fixed point of the ray, the edge is smeared
    let rb =
        run(&[FxInstance::RadialBlur { amount: 0.8, center_x: 0.0, center_y: 0.0, spin: false }]);
    let mut edge_moved = 0;
    for x in 0..W {
        let i = ((H / 4 * W + x) * 4) as usize;
        if rb.rgba[i] != src.rgba[i] {
            edge_moved += 1;
        }
    }
    assert!(edge_moved > (W / 4) as usize, "radial blur must smear off-center rows");

    // DropShadow on an opaque input — passthrough byte-for-byte
    let ds = run(&[FxInstance::drop_shadow()]);
    assert_eq!(ds.rgba, src.rgba, "opaque input: drop shadow is a byte passthrough");

    // [greenscreen, drop_shadow]: in the cut-out zone at +offset from the opaque edge a SHADOW
    // appears (alpha > 0, colour — tint)
    let keyed = apply_barrier(
        &FxInstance::greenscreen([0, 255, 0], 70, 25),
        test_frame(),
        &ctx(),
        &mut scratch,
        1,
    );
    assert!(keyed.has_alpha);
    let shadowed = apply_barrier(
        &FxInstance::DropShadow {
            offset_x: 6,
            offset_y: 6,
            radius: 2,
            opacity: 1.0,
            tint_r: 0,
            tint_g: 0,
            tint_b: 0,
        },
        keyed,
        &ctx(),
        &mut scratch,
        1,
    );
    // the green corner is switched off by the key; the edge of the opaque zone is at
    // x == W/2 (and y == H/2); a point slightly right+below the edge in the cut-out
    // zone must get the shadow
    let px = (W / 2 + 4, H / 2 + 4);
    let i = ((px.1 * W + px.0) * 4) as usize;
    assert!(
        shadowed.rgba[i + 3] > 32,
        "shadow must appear in the keyed-out zone: alpha {}",
        shadowed.rgba[i + 3]
    );
    assert!(
        shadowed.rgba[i] < 32 && shadowed.rgba[i + 1] < 32 && shadowed.rgba[i + 2] < 32,
        "shadow colour is the tint: {:?}",
        &shadowed.rgba[i..i + 3]
    );

    // Halftone: the output is two-level; there are more dots in the dark zone
    let ht = run(&[FxInstance::Halftone { size: 8, angle: 45.0 }]);
    for pxl in ht.rgba.chunks_exact(4) {
        assert!(
            (pxl[0] == 0 || pxl[0] == 255) && pxl[0] == pxl[1] && pxl[1] == pxl[2],
            "halftone output is binary mono: {pxl:?}"
        );
    }
    let dark_dots =
        ht.rgba.chunks_exact(4).take((W * H / 4) as usize).filter(|p| p[0] == 0).count();
    let light_dots =
        ht.rgba.chunks_exact(4).skip((W * H * 3 / 4) as usize).filter(|p| p[0] == 0).count();
    assert!(
        dark_dots > light_dots,
        "darker area must grow bigger dots: {dark_dots} vs {light_dots}"
    );
}

/// Test 333 (R.47 Kv4): the Warp primitive — LensDistort/Twirl/Ripple/
/// Kaleidoscope/Cylinder3d. Neutral parameters — the step is skipped
/// (identity bytes); the meaning of each mode; PerPixel supersampling;
/// determinism; a series of warps — one run.
#[test]
fn t333_warp_prims() {
    use kt_fx::geom::SsSpec;
    use kt_fx::plan::{PlanOp, plan_chain};
    let src = test_frame();

    // (1) the neutral parameters of each warp — identity byte-for-byte
    for f in [
        FxInstance::LensDistort { k1: 0.0, k2: 0.0 },
        FxInstance::Twirl { angle: 0.0, radius: 0.8 },
        FxInstance::Ripple { amount: 0.0, freq: 10.0, phase: 0.0 },
        FxInstance::Cylinder3d { curvature: 0.0 },
    ] {
        let out = run(std::slice::from_ref(&f));
        assert_eq!(out.rgba, src.rgba, "{}: neutral params are a byte identity", f.tag());
    }

    // (2) PerPixel supersampling is enabled for a program with a warp
    let twirl = FxInstance::twirl();
    match build_geom_exec(&[&twirl], W, H, W, H, 0.5, None, 90_000, 1, 1) {
        GeomExec::Fused(prog) => {
            assert_eq!(prog.ss, SsSpec::PerPixel, "warp jacobian varies per pixel");
        }
        GeomExec::Legacy => panic!("warp must go fused"),
    }

    // (3) determinism: two runs byte-for-byte
    let hard = [
        FxInstance::Twirl { angle: 540.0, radius: 1.2 },
        FxInstance::Ripple { amount: 0.3, freq: 30.0, phase: 137.0 },
    ];
    assert_eq!(run(&hard).rgba, run(&hard).rgba, "warp chain must be deterministic");

    // (4) twirl: the centre is a fixed point; a point on the radius is rotated.
    // Marker frame: white field, black block to the right of the centre.
    let marker_at = |mx: u32, my: u32| {
        let mut f = Frame { w: W, h: H, rgba: vec![255u8; (W * H * 4) as usize], has_alpha: false };
        for y in my.saturating_sub(1)..(my + 2).min(H) {
            for x in mx.saturating_sub(1)..(mx + 2).min(W) {
                let i = ((y * W + x) * 4) as usize;
                f.rgba[i] = 0;
                f.rgba[i + 1] = 0;
                f.rgba[i + 2] = 0;
            }
        }
        f
    };
    let centroid = |f: &Frame| {
        let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0.0f64);
        for y in 0..f.h {
            for x in 0..f.w {
                let i = ((y * f.w + x) * 4) as usize;
                if f.rgba[i + 3] == 255 && f.rgba[i] < 128 {
                    sx += x as f64;
                    sy += y as f64;
                    n += 1.0;
                }
            }
        }
        assert!(n > 0.0, "marker must survive");
        (sx / n, sy / n)
    };
    // the centre: the marker in the centre of the frame does not move
    let tw = FxInstance::Twirl { angle: 180.0, radius: 1.0 };
    let out = apply_video_chain(std::slice::from_ref(&tw), marker_at(W / 2, H / 2), &ctx());
    let (cx, cy) = centroid(&out);
    assert!(
        (cx - (W / 2) as f64).abs() <= 1.5 && (cy - (H / 2) as f64).abs() <= 1.5,
        "twirl center is a fixed point: ({cx:.1}, {cy:.1})"
    );
    // a point at half the radius: the inverse map rotates it BY θ — the marker visibly
    // moves from its original place
    let out = apply_video_chain(std::slice::from_ref(&tw), marker_at(W / 2 + 9, H / 2), &ctx());
    let (mx, my) = centroid(&out);
    let moved = ((mx - (W / 2 + 9) as f64).powi(2) + (my - (H / 2) as f64).powi(2)).sqrt();
    assert!(moved > 4.0, "twirl must move an off-center marker: ({mx:.1}, {my:.1})");

    // (5) lens: the marker shift by the formula. Inverse map: the output r
    // samples the source at r·(1+k1·r²) — a source marker at radius rs
    // is visible at the output where rs = r·(1+k1·r²).
    let k1 = 0.5f64;
    let lens = FxInstance::LensDistort { k1: k1 as f32, k2: 0.0 };
    let rr = (H / 2 - 4) as f64; // the marker radius in px (along Y, the short axis)
    let rn = rr / (H as f64 / 2.0); // normalized
    let out =
        apply_video_chain(std::slice::from_ref(&lens), marker_at(W / 2, H / 2 - rr as u32), &ctx());
    let (_, my) = centroid(&out);
    // solve rs = r·(1+k1·r²) for r (the output radius of the marker)
    let mut r_out = rn;
    for _ in 0..40 {
        r_out = rn / (1.0 + k1 * r_out * r_out);
    }
    let expect_y = H as f64 / 2.0 - r_out * H as f64 / 2.0;
    assert!(
        (my - expect_y).abs() <= 1.5,
        "lens marker at formula position: got y {my:.1}, expect {expect_y:.1}"
    );

    // (6) kaleidoscope: the points of the first wedge remain themselves (≤1 LSB)
    let kal = FxInstance::Kaleidoscope { segments: 6, angle: 0.0 };
    let out = apply_video_chain(std::slice::from_ref(&kal), test_frame(), &ctx());
    let tau = std::f64::consts::TAU / 6.0;
    let mut checked = 0;
    for y in 0..H {
        for x in 0..W {
            let (dx, dy) = (x as f64 + 0.5 - W as f64 / 2.0, y as f64 + 0.5 - H as f64 / 2.0);
            let phi = dy.atan2(dx);
            let r = (dx * dx + dy * dy).sqrt() / (H as f64 / 2.0);
            // deep inside the first HALF of the wedge (there m == q — the identity)
            if r < 0.9 && phi > 0.1 && phi < tau / 2.0 - 0.1 {
                let i = ((y * W + x) * 4) as usize;
                for c in 0..4 {
                    let d = (out.rgba[i + c] as i32 - src.rgba[i + c] as i32).abs();
                    assert!(d <= 1, "first wedge must be itself at ({x},{y}): d {d}");
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 20, "wedge probe must cover pixels: {checked}");

    // (7) cylinder: the edges and the centre column are in place, compression toward the edges
    let cyl = FxInstance::Cylinder3d { curvature: 0.9 };
    let out = apply_video_chain(std::slice::from_ref(&cyl), marker_at(W / 2, H / 4), &ctx());
    let (mx, _) = centroid(&out);
    assert!((mx - (W / 2) as f64).abs() <= 1.5, "cylinder center column fixed: {mx:.1}");
    // a marker at 3/4 of the width moves OUTWARD (compression at the edges: the source
    // is sampled there closer to the centre, the picture swells toward the edge)
    let out = apply_video_chain(std::slice::from_ref(&cyl), marker_at(3 * W / 4, H / 2), &ctx());
    let (mx, _) = centroid(&out);
    assert!(mx > (3 * W / 4) as f64 + 2.0, "cylinder must push mid-side content outward: {mx:.1}");

    // (8) a series of warps + zoom — ONE run
    let fx = vec![FxInstance::lens_distort(), FxInstance::twirl(), FxInstance::geometry_zoom(2.0)];
    let p = plan_chain(&fx, W, H, 1, W, H, 0.5, None, 90_000, &[]);
    assert_eq!(p.ops.len(), 1, "warp series stays one fused run");
    assert!(matches!(p.ops[0], PlanOp::Run { i: 0, j: 3, .. }));
}

/// Test 519 (R.53, item 417): the alpha rule is a convolution, not “the last
/// declarer wins” among ALL steps.
///
/// `EventGrid` declared itself the killer of alpha, although the tile is a
/// resampled input and the input alpha still lives in it. Because of this
/// the title or the greenscreen output under the grid landed in the composite as an
/// OPAQUE rectangle.
#[test]
fn t519_alpha_rule_folds_produces_and_destroys() {
    use kt_fx::geom::build_geom_exec;
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let gs = FxInstance::Greenscreen {
        key_r: 0,
        key_g: 255,
        key_b: 0,
        tolerance: 80,
        softness: 20,
        spill: 0.0,
    };
    let mir = FxInstance::Mirror { horizontal: true, vertical: false };

    let prog = |fx: &[FxInstance]| {
        let refs: Vec<&FxInstance> = fx.iter().collect();
        match build_geom_exec(&refs, W, H, W, H, 0.5, None, 90_000, 1, 1) {
            GeomExec::Fused(p) => p,
            _ => panic!("the series must merge"),
        }
    };

    // a lone grid: it has nothing to say about alpha — the SOURCE alpha is taken
    let p = prog(&[grid.clone(), mir.clone()]);
    assert!(!p.has_alpha_known, "EventGrid again declares alpha for the source");

    // greenscreen CREATES alpha, and the grid behind it does not erase it
    let p = prog(&[gs.clone(), grid.clone()]);
    assert!(p.has_alpha_known && p.has_alpha, "the grid erased the greenscreen alpha");

    // the order reversed — the same answer
    let p = prog(&[grid.clone(), gs.clone(), mir.clone()]);
    assert!(p.has_alpha_known && p.has_alpha);
}

/// Test 520 (R.53, item 417): the consequence IN PIXELS. A frame with a transparent
/// half passes the grid and stays transparent where it was.
#[test]
fn t520_grid_keeps_transparent_pixels_transparent() {
    let mut f = test_frame();
    f.has_alpha = true;
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            if x < W / 2 {
                f.rgba[i + 3] = 0;
            }
        }
    }
    let grid = FxInstance::EventGrid { cols: 2, rows: 2, center: false };
    let out = apply_video_chain(&[grid], f, &ctx());
    assert!(out.has_alpha, "frame declared opaque — the composite will flood it with a slab");
    // the left quarter of each tile comes from the transparent half of the input
    let a = |x: u32, y: u32| out.rgba[(((y * W + x) * 4) + 3) as usize];
    assert_eq!(a(2, 4), 0, "a transparent pixel became opaque");
    assert_eq!(a(W / 2 + 2, 4), 0, "the second tile lost the input transparency");
    assert_eq!(a(W / 4 + 2, 4), 255, "the opaque half of the input got spoiled");
}

/// Test 522 (R.53, item 418): blurring a frame WITH ALPHA does not drag the colour of
/// transparent pixels into the visible ones.
///
/// Without premultiplication the mip mixes the RGB of transparent neighbours (for a title
/// that is the black background around a glyph) with the visible ones, and a dark
/// halo appears along the edge. The CONSEQUENCE is checked: a bright opaque pixel next to a
/// transparent black area does not darken after blurring.
#[test]
fn t522_blur_of_an_alpha_frame_has_no_dark_halo() {
    // a white opaque stripe in a black TRANSPARENT field
    let mut f = Frame { w: W, h: H, rgba: vec![0u8; (W * H * 4) as usize], has_alpha: true };
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            let inside = (W / 4..W * 3 / 4).contains(&x);
            if inside {
                f.rgba[i] = 255;
                f.rgba[i + 1] = 255;
                f.rgba[i + 2] = 255;
                f.rgba[i + 3] = 255;
            }
        }
    }
    let out = apply_video_chain(&[FxInstance::blur(8)], f, &ctx());
    // where the alpha is still noticeable, the colour must stay white: the black from the
    // transparent field did not leak into it
    let mut worst = 255u8;
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            if out.rgba[i + 3] >= 200 {
                worst = worst.min(out.rgba[i]);
            }
        }
    }
    assert!(worst >= 250, "colour of transparent neighbours bled into visible ones: minimum {worst} instead of ~255");
}

/// Test 523 (R.53, item 418): the `has_alpha` gate — an OPAQUE frame goes through the same
/// kernels BYTE FOR BYTE.
///
/// Premultiplication at `a == 255` is an identity, and it is precisely on this that the promise
/// “not a single golden moved” rests: the rule is enabled only where
/// alpha exists.
#[test]
fn t523_opaque_frames_are_byte_identical() {
    for fx in [
        FxInstance::blur(8),
        FxInstance::blur(24),
        FxInstance::Glow {
            strength: 1.0,
            radius: 4,
            threshold: 0,
            tint_r: 255,
            tint_g: 255,
            tint_b: 255,
        },
        FxInstance::Glow {
            strength: 0.7,
            radius: 9,
            threshold: 40,
            tint_r: 200,
            tint_g: 120,
            tint_b: 255,
        },
    ] {
        // the same frame, but declared with alpha: all pixels a == 255
        let mut with = test_frame();
        with.has_alpha = true;
        let a = apply_video_chain(std::slice::from_ref(&fx), test_frame(), &ctx());
        let b = apply_video_chain(std::slice::from_ref(&fx), with, &ctx());
        assert_eq!(a.rgba, b.rgba, "{fx:?}: the alpha gate shifted the bytes on an opaque frame");
    }
}
