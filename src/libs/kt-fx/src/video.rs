//! Video FX (port of bc_fx.cpp, item 25): 10 v1 effects, RGBA8 in/out, CPU.
//! The order of application = the order in the fx array (the first — the nearest to the
//! decoder). One code for preview and export (invariant No. 7).
//! Determinism: noise/vhs depend only on timeline_burst_index.

use kt_model::{CamKeyframe, FxInstance, PanMode};

/// Frame context for the chain.
#[derive(Debug, Clone, Copy)]
pub struct VideoFxCtx {
    /// Target size (project/preview resolution).
    pub out_w: u32,
    pub out_h: u32,
    /// Clip progress 0..1 (for CamMotion).
    pub u: f64,
    /// Absolute timeline position in ticks (for noise/vhs).
    pub burst_index: i64,
    /// The resolved 3D camera of the layer (R.44: Pan3d is also computed on the CPU).
    /// None — Pan3d in the chain is skipped, as before without a camera.
    pub cam3d: Option<crate::geom::Cam3d>,
    /// Decode step of the frame (R.46, item 318): the input frame is in_scale times
    /// larger than the logical size of the chain. 1 — as it always was.
    pub in_scale: u32,
    /// Context of the leading “Geometry” node (R.62): the source view and the
    /// conform rule. `None` — the node is considered “stretch” (frame = canvas).
    pub lead: Option<crate::conform::LeadCtx>,
}

/// Frame in progress.
pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
    pub has_alpha: bool,
}

/// Pool of RGBA buffers of the chain (item 2 of round 7): previously each effect allocated
/// a full-size buffer for EACH frame (Mirror+Grid+Glow+VHS ≈ 6 allocations
/// and 5 full passes per compose). take() returns a zeroed buffer, because
/// edges/event_grid/pan_zoom rely on zero initialization.
/// The effects' arithmetic is unchanged: golden test 26 byte-for-byte.
#[derive(Default)]
pub struct FxScratch {
    bufs: Vec<Vec<u8>>,
}

impl FxScratch {
    pub fn take(&mut self, len: usize) -> Vec<u8> {
        let mut b = self.bufs.pop().unwrap_or_default();
        b.clear();
        b.resize(len, 0);
        b
    }
    pub fn put(&mut self, b: Vec<u8>) {
        if self.bufs.len() < 8 {
            self.bufs.push(b);
        }
    }
}

/// Built-in sample for the effect THUMBNAILS (R.54): what to show in the
/// icon when the frame of its own does not exist yet — the picker is open on a nested clip,
/// the source thumb has not arrived, or the question is about the icon of the effect itself.
///
/// The properties are chosen for what a thumbnail must show:
/// * ASYMMETRY on both axes (a diagonal + a circle in one corner) — otherwise
///   `Mirror` is indistinguishable from the input, and that is exactly what must be shown;
/// * a green plate — `Greenscreen` must key something;
/// * colour bars and a luma ramp — `ColorGrade`/`Duotone`/
///   `Posterize`/`VHS` change exactly these;
/// * a fine grid and hard edges — `Blur`/`Sharpen`/`Edges`/`Glow`/
///   `Pixelate`/`Halftone` without high frequencies would show an empty place.
///
/// `alpha_margin` — the content in a frame with TRANSPARENT margins: `DropShadow` on an
/// opaque frame is identical by construction (the shadow falls from under the
/// alpha), and `Pan3d`/`CornerPin` without margins push the quad to the edge.
///
/// Deterministic (no time, no RNG) and frozen by the golden t544: this is
/// the REFERENCE, not decoration — the icons are checked against it.
pub fn test_pattern(w: u32, h: u32, alpha_margin: bool) -> Frame {
    let (w, h) = (w.max(4), h.max(4));
    let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
    // 8 colour bars at the top (hard vertical edges)
    const BARS: [[u8; 3]; 8] = [
        [235, 235, 235],
        [235, 235, 40],
        [40, 235, 235],
        [40, 200, 60],
        [235, 60, 200],
        [220, 50, 50],
        [50, 70, 220],
        [20, 20, 20],
    ];
    let grid = (w / 40).max(2);
    let (fw, fh) = (w as f64, h as f64);
    for y in 0..h {
        let v = (y as f64 + 0.5) / fh;
        for x in 0..w {
            let u = (x as f64 + 0.5) / fw;
            let i = ((y as usize) * (w as usize) + x as usize) * 4;
            let (mut r, mut g, mut b) = if v < 0.34 {
                let bar = ((u * BARS.len() as f64) as usize).min(BARS.len() - 1);
                (BARS[bar][0], BARS[bar][1], BARS[bar][2])
            } else if v < 0.67 {
                // luma ramp; in the left half — a fine checkerboard
                let l = (u * 255.0) as u8;
                if u < 0.5 && ((x / grid) + (y / grid)) % 2 == 0 {
                    (l.saturating_add(60), l.saturating_add(60), l.saturating_add(60))
                } else {
                    (l, l, l)
                }
            } else if u > 0.62 {
                (0, 255, 0) // a plate for keying
            } else {
                (96, 96, 104)
            };
            // diagonal (asymmetry on both axes)
            if ((u - v).abs() * fw) < grid as f64 {
                (r, g, b) = (255, 255, 255);
            }
            // a circle in the LEFT bottom corner: a mirror on any axis would shift it
            let (dx, dy) = (u - 0.22, v - 0.80);
            if dx * dx + dy * dy < 0.012 {
                (r, g, b) = (250, 170, 30);
            }
            let inside = !alpha_margin || (0.11..0.89).contains(&u) && (0.11..0.89).contains(&v);
            rgba[i] = if inside { r } else { 0 };
            rgba[i + 1] = if inside { g } else { 0 };
            rgba[i + 2] = if inside { b } else { 0 };
            rgba[i + 3] = if inside { 255 } else { 0 };
        }
    }
    Frame { w, h, rgba, has_alpha: alpha_margin }
}

/// Thumbnail of the chain on a finished frame (R.54): fit `base` into w×h and
/// run the chain. The ONLY entry point for thumbnails — for the effect icons, the
/// variants strip and the library presets alike.
///
/// The context semantics are the engine's semantics, so that the picture does not diverge
/// from what results after a click: `u` — the clip progress at the point of the
/// cursor, `burst_index` — the absolute timeline tick (noise/vhs grain),
/// `cam3d` — the layer's resolved camera (without it Pan3d is a passthrough).
/// `in_scale` is always 1 here: the decode step (R.46) says “the frame came
/// LARGER than the logical size”, and for a thumbnail it is the opposite.
///
/// ABOUT SIZE. Some parameters are set in PIXELS (`Pixelate.size` up to 128,
/// `Halftone.size`, blur radii, `Vhs.rgb_shift`), so the same chain at 96×54 and
/// at 1080p shows a different STRENGTH. A thumbnail is about the look, not the strength;
/// call it meaningfully at the reference size (for us this is the thumb size,
/// 320×180) and shrink the result with `fit_into`, rather than computing it at the
/// size of the cell directly.
#[allow(clippy::too_many_arguments)] // these are the parameters of VideoFxCtx, not “a lot of stuff”
pub fn render_fx_thumb(
    base: &Frame,
    fx: &[FxInstance],
    w: u32,
    h: u32,
    u: f64,
    burst_index: i64,
    cam3d: Option<crate::geom::Cam3d>,
    scratch: &mut FxScratch,
) -> Frame {
    let (w, h) = (w.max(1), h.max(1));
    let src = if base.w == w && base.h == h {
        Frame { w, h, rgba: base.rgba.clone(), has_alpha: base.has_alpha }
    } else {
        let mut buf = scratch.take((w as usize) * (h as usize) * 4);
        fit_into(base, w, h, &mut buf);
        Frame { w, h, rgba: buf, has_alpha: base.has_alpha }
    };
    let ctx = VideoFxCtx { out_w: w, out_h: h, u, burst_index, cam3d, in_scale: 1, lead: None };
    apply_video_chain_pooled(fx, src, &ctx, scratch)
}

/// Apply the video FX chain of the clip. Audio variants are ignored.
/// A wrapper with a local pool — the result is identical to the pooled variant.
pub fn apply_video_chain(fx: &[FxInstance], frame: Frame, ctx: &VideoFxCtx) -> Frame {
    let mut scratch = FxScratch::default();
    apply_video_chain_pooled(fx, frame, ctx, &mut scratch)
}

/// Variant with a reusable buffer pool (the preview calls the chain for each
/// new frame — the pool holder lives in VideoRenderer).
///
/// Since R.44, consecutive GEOMETRIC effects (Pan/CamMotion/Mirror/
/// EventGrid) run as one fused pass of inverse mappings
/// (`crate::geom`, concatenation of transforms) — without intermediate rasters.
/// A series of length 1 goes through the old kernels byte-for-byte (t297); barriers
/// (Blur/Glow/Edges/VHS/Noise/Greenscreen) — as before.
pub fn apply_video_chain_pooled(
    fx: &[FxInstance],
    mut frame: Frame,
    ctx: &VideoFxCtx,
    scratch: &mut FxScratch,
) -> Frame {
    // The plan is built by the COMMON code (crate::plan, item 311) — the same one that the
    // GPU path calls; here only the execution of the ops.
    let plan = crate::plan::plan_chain_in(
        fx,
        frame.w,
        frame.h,
        ctx.in_scale,
        ctx.out_w,
        ctx.out_h,
        ctx.u,
        ctx.cam3d,
        ctx.burst_index,
        &[],
        ctx.lead,
    );
    for op in plan.ops {
        frame = match op {
            crate::plan::PlanOp::Run { i, j, exec } => match exec {
                crate::geom::GeomExec::Legacy => {
                    let step = fx[i..j]
                        .iter()
                        .find(|f| crate::geom::is_geo_active(f, ctx.lead, ctx.out_w, ctx.out_h))
                        .expect("legacy run has a geo step");
                    apply_legacy_geo(step, frame, ctx, scratch)
                }
                crate::geom::GeomExec::Fused(prog) => {
                    let nf = crate::geom::exec_geom_cpu(&prog, &frame, scratch);
                    scratch.put(frame.rgba);
                    nf
                }
            },
            crate::plan::PlanOp::Barrier { idx, res_scale } => {
                apply_barrier(&fx[idx], frame, ctx, scratch, res_scale)
            }
            // The layer strength is applied one level up (apply_chain_with_mixes in
            // kt-engine); in a flat chain the slot is a passthrough, as before
            crate::plan::PlanOp::Snapshot | crate::plan::PlanOp::Mix { .. } => frame,
        };
    }
    frame
}

/// One barrier effect (non-local/per-pixel outside the series) — the former
/// kernels; `res_scale` — K× execution (R.45 item 313: the kernel's pixel geometry
/// ×K, the look is invariant; K=1 — verbatim the former code). Pub — for
/// t303 (kernel invariance of the look without chains); the product call is
/// only the plan executor.
pub fn apply_barrier(
    f: &FxInstance,
    frame: Frame,
    ctx: &VideoFxCtx,
    scratch: &mut FxScratch,
    res_scale: u32,
) -> Frame {
    let rs = res_scale.max(1);
    match f {
        FxInstance::Blur { radius, radius_y } => blur(frame, *radius, *radius_y, scratch, rs),
        // greenscreen per-pixel — the resolution does not touch it
        FxInstance::Greenscreen { key_r, key_g, key_b, tolerance, softness, spill } => {
            greenscreen(frame, [*key_r, *key_g, *key_b], *tolerance, *softness, *spill)
        }
        FxInstance::Edges { threshold, thickness, color_r, color_g, color_b } => {
            edges(frame, *threshold, *thickness, [*color_r, *color_g, *color_b], scratch, rs)
        }
        FxInstance::Noise { amount, size, color } => {
            noise(frame, *amount, *size, *color, ctx.burst_index, rs)
        }
        FxInstance::Vhs { rgb_shift, bleed, scan_step, jitter } => {
            vhs(frame, *rgb_shift, *bleed, *scan_step, *jitter, ctx.burst_index, scratch, rs)
        }
        FxInstance::Glow { strength, radius, threshold, tint_r, tint_g, tint_b } => {
            glow(frame, *strength, *radius, *threshold, [*tint_r, *tint_g, *tint_b], scratch, rs)
        }
        // per-pixel R.47 effects in a BARRIER position (leading/trailing slot):
        // the same f64 text of the formula as the tap of the fused pass — one home
        FxInstance::ColorGrade { brightness, contrast, saturation, hue } => {
            let h = (*hue as f64).to_radians();
            let (hc, hs) = (h.cos(), h.sin());
            pointwise_barrier(frame, |v, _| {
                crate::geom::tap_grade(
                    v,
                    *brightness as f64,
                    *contrast as f64,
                    *saturation as f64,
                    hc,
                    hs,
                )
            })
        }
        FxInstance::Duotone { shadow_r, shadow_g, shadow_b, high_r, high_g, high_b, amount } => {
            let sh = [*shadow_r as f64, *shadow_g as f64, *shadow_b as f64];
            let hi = [*high_r as f64, *high_g as f64, *high_b as f64];
            let a = (*amount as f64).clamp(0.0, 1.0);
            pointwise_barrier(frame, |v, _| crate::geom::tap_duo(v, &sh, &hi, a))
        }
        FxInstance::Posterize { levels } => {
            let n = (*levels).clamp(2, 32);
            pointwise_barrier(frame, |v, _| crate::geom::tap_poster(v, n))
        }
        FxInstance::Vignette { strength, radius, softness } => {
            let (st, ra, so) = (
                (*strength as f64).clamp(0.0, 1.0),
                (*radius as f64).clamp(0.0, 1.5),
                (*softness as f64).clamp(0.05, 1.0),
            );
            let (w, h) = (frame.w, frame.h);
            pointwise_barrier(frame, move |v, xy| crate::geom::tap_vig(v, xy, st, ra, so, w, h))
        }
        // R.47 barriers Kv3
        FxInstance::Sharpen { amount, radius } => {
            sharpen(frame, (*amount as f64).clamp(0.0, 4.0), *radius, scratch, rs)
        }
        FxInstance::Pixelate { size } => pixelate(frame, *size, scratch, rs),
        FxInstance::ChromaticAberration { amount } => {
            chroma_ab(frame, (*amount as f64).clamp(0.0, 0.05), scratch)
        }
        FxInstance::RadialBlur { amount, center_x, center_y, spin } => radial_blur(
            frame,
            (*amount as f64).clamp(0.0, 1.0),
            *center_x as f64,
            *center_y as f64,
            *spin,
            scratch,
        ),
        FxInstance::DropShadow { offset_x, offset_y, radius, opacity, tint_r, tint_g, tint_b } => {
            drop_shadow(
                frame,
                (*offset_x, *offset_y),
                *radius,
                (*opacity as f64).clamp(0.0, 1.0),
                [*tint_r, *tint_g, *tint_b],
                scratch,
                rs,
            )
        }
        FxInstance::Halftone { size, angle } => halftone(frame, *size, *angle as f64, scratch, rs),
        _ => {
            debug_assert!(false, "barrier op on non-barrier fx");
            frame
        }
    }
}

/// Unsharp masking (R.47): src + (src − mip base)·amount. The mip step
/// = radius·K (radius ≥ 2: step 1 would give a base == the source and a zero
/// effect — the bilinear 1:1 is exact). Alpha is untouched.
fn sharpen(f: Frame, amount: f64, radius: u32, scratch: &mut FxScratch, res_scale: u32) -> Frame {
    let step = radius.clamp(2, 16) * res_scale;
    let (dw, dh) = ((f.w / step).max(1), (f.h / step).max(1));
    let mut small = scratch.take((dw * dh * 4) as usize);
    bilinear_into(&f, 0.0, 0.0, f.w as f64, f.h as f64, dw, dh, &mut small);
    let small_f = Frame { w: dw, h: dh, rgba: small, has_alpha: f.has_alpha };
    let mut base = scratch.take(f.rgba.len());
    bilinear_into(&small_f, 0.0, 0.0, dw as f64, dh as f64, f.w, f.h, &mut base);
    scratch.put(small_f.rgba);
    let mut out = f;
    for (px, bp) in out.rgba.chunks_exact_mut(4).zip(base.chunks_exact(4)) {
        for c in 0..3 {
            let s = px[c] as f64;
            px[c] = (s + (s - bp[c] as f64) * amount).round().clamp(0.0, 255.0) as u8;
        }
    }
    scratch.put(base);
    out
}

/// Mosaic (R.47): an integer copy of the cell centre C = size·K —
/// EXACT parity with the GPU by construction.
fn pixelate(f: Frame, size: u32, scratch: &mut FxScratch, res_scale: u32) -> Frame {
    let c = size.clamp(2, 128) * res_scale;
    let mut out = scratch.take(f.rgba.len());
    for y in 0..f.h {
        let sy = (y / c * c + c / 2).min(f.h - 1);
        for x in 0..f.w {
            let sx = (x / c * c + c / 2).min(f.w - 1);
            let d = ((y * f.w + x) * 4) as usize;
            let s = ((sy * f.w + sx) * 4) as usize;
            out[d..d + 4].copy_from_slice(&f.rgba[s..s + 4]);
        }
    }
    let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha };
    scratch.put(f.rgba);
    res
}

/// Chromatic aberration (R.47): R is sampled closer to the centre (×(1−a)),
/// B further out (×(1+a)), G and alpha in place. Positions are frame fractions,
/// the K× execution scales itself.
fn chroma_ab(f: Frame, amount: f64, scratch: &mut FxScratch) -> Frame {
    let (cw, ch) = (f.w as f64 / 2.0, f.h as f64 / 2.0);
    let mut out = scratch.take(f.rgba.len());
    for y in 0..f.h {
        for x in 0..f.w {
            let d = ((y * f.w + x) * 4) as usize;
            let (px, py) = (x as f64 + 0.5 - cw, y as f64 + 0.5 - ch);
            let r = crate::geom::sample_bilinear_f64(
                &f,
                cw + px * (1.0 - amount) - 0.5,
                ch + py * (1.0 - amount) - 0.5,
            )[0];
            let b = crate::geom::sample_bilinear_f64(
                &f,
                cw + px * (1.0 + amount) - 0.5,
                ch + py * (1.0 + amount) - 0.5,
            )[2];
            out[d] = r.round().clamp(0.0, 255.0) as u8;
            out[d + 1] = f.rgba[d + 1];
            out[d + 2] = b.round().clamp(0.0, 255.0) as u8;
            out[d + 3] = f.rgba[d + 3];
        }
    }
    let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha };
    scratch.put(f.rgba);
    res
}

/// Radial blur (R.47): the average over EXACTLY 16 taps along the zoom ray or
/// the rotation arc — the tap count is a constant of both backends; the frame
/// fractions scale by K× themselves.
fn radial_blur(
    f: Frame,
    amount: f64,
    center_x: f64,
    center_y: f64,
    spin: bool,
    scratch: &mut FxScratch,
) -> Frame {
    const N: usize = 16;
    let (cx, cy) = (
        (0.5 + center_x.clamp(-1.0, 1.0) * 0.5) * f.w as f64,
        (0.5 + center_y.clamp(-1.0, 1.0) * 0.5) * f.h as f64,
    );
    let mut out = scratch.take(f.rgba.len());
    for y in 0..f.h {
        for x in 0..f.w {
            let d = ((y * f.w + x) * 4) as usize;
            let (px, py) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
            let mut acc = [0f64; 4];
            for i in 0..N {
                let t = i as f64 / (N - 1) as f64;
                let (sx, sy) = if spin {
                    let a = 0.6 * amount * (t - 0.5);
                    let (sa, ca) = a.sin_cos();
                    (px * ca - py * sa, px * sa + py * ca)
                } else {
                    let k = 1.0 - 0.5 * amount * t;
                    (px * k, py * k)
                };
                let s = crate::geom::sample_bilinear_f64(&f, cx + sx - 0.5, cy + sy - 0.5);
                for (a, v) in acc.iter_mut().zip(s) {
                    *a += v;
                }
            }
            for (c, a) in acc.iter().enumerate() {
                out[d + c] = (a / N as f64).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha };
    scratch.put(f.rgba);
    res
}

/// Shadow under the layer (R.47): the source alpha blurred (mip radius·K) and shifted
/// by offset·K; the source OVER the shadow (straight alpha, f64,
/// rounding once). An input without alpha — passthrough: the formula on an
/// opaque input degenerates into identity, the early exit only
/// saves work (the GPU side is consistent — see chain_ops).
fn drop_shadow(
    f: Frame,
    offset: (i32, i32),
    radius: u32,
    opacity: f64,
    tint: [u8; 3],
    scratch: &mut FxScratch,
    res_scale: u32,
) -> Frame {
    if !f.has_alpha {
        return f;
    }
    let step = radius.clamp(1, 32) * res_scale;
    let (dw, dh) = ((f.w / step).max(1), (f.h / step).max(1));
    let mut small = scratch.take((dw * dh * 4) as usize);
    bilinear_into(&f, 0.0, 0.0, f.w as f64, f.h as f64, dw, dh, &mut small);
    let small_f = Frame { w: dw, h: dh, rgba: small, has_alpha: true };
    let mut blurred = scratch.take(f.rgba.len());
    bilinear_into(&small_f, 0.0, 0.0, dw as f64, dh as f64, f.w, f.h, &mut blurred);
    scratch.put(small_f.rgba);
    let (ox, oy) =
        (offset.0.clamp(-64, 64) * res_scale as i32, offset.1.clamp(-64, 64) * res_scale as i32);
    let mut out = scratch.take(f.rgba.len());
    for y in 0..f.h as i64 {
        for x in 0..f.w as i64 {
            let d = ((y * f.w as i64 + x) * 4) as usize;
            let (bx, by) = (x - ox as i64, y - oy as i64);
            let ba = if bx >= 0 && by >= 0 && bx < f.w as i64 && by < f.h as i64 {
                blurred[((by * f.w as i64 + bx) * 4 + 3) as usize] as f64 / 255.0
            } else {
                0.0
            };
            let ash = ba * opacity;
            let asrc = f.rgba[d + 3] as f64 / 255.0;
            let oa = asrc + ash * (1.0 - asrc);
            if oa <= 0.0 {
                out[d..d + 4].copy_from_slice(&[0, 0, 0, 0]);
            } else {
                for c in 0..3 {
                    let v =
                        (f.rgba[d + c] as f64 * asrc + tint[c] as f64 * ash * (1.0 - asrc)) / oa;
                    out[d + c] = v.round().clamp(0.0, 255.0) as u8;
                }
                out[d + 3] = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    scratch.put(blurred);
    let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: true };
    scratch.put(f.rgba);
    res
}

/// Halftone raster (R.47): black dots on white, radius from the luma
/// of the cell centre. The pattern entirely in LOGICAL coordinates l = p/K —
/// the K× execution sees the same grid.
fn halftone(f: Frame, size: u32, angle: f64, scratch: &mut FxScratch, res_scale: u32) -> Frame {
    let s = size.clamp(2, 64) as f64;
    let k = res_scale as f64;
    let (sa, ca) = angle.clamp(-90.0, 90.0).to_radians().sin_cos();
    let mut out = scratch.take(f.rgba.len());
    for y in 0..f.h {
        for x in 0..f.w {
            let d = ((y * f.w + x) * 4) as usize;
            let (lx, ly) = ((x as f64 + 0.5) / k, (y as f64 + 0.5) / k);
            // rotation by −angle: the grid stands in the rotated space
            let (ux, uy) = (lx * ca + ly * sa, -lx * sa + ly * ca);
            let (ccx, ccy) = ((ux / s).floor() * s + 0.5 * s, (uy / s).floor() * s + 0.5 * s);
            // cell centre back into logic and into the physics of the frame
            let (gx, gy) = (ccx * ca - ccy * sa, ccx * sa + ccy * ca);
            let smp = crate::geom::sample_bilinear_f64(&f, gx * k - 0.5, gy * k - 0.5);
            let l = (77.0 * smp[0] + 150.0 * smp[1] + 29.0 * smp[2]) / 256.0;
            let rd = 0.5 * s * 1.25 * (1.0 - (l / 255.0).clamp(0.0, 1.0)).sqrt();
            let dist = ((ux - ccx) * (ux - ccx) + (uy - ccy) * (uy - ccy)).sqrt();
            let v = if dist <= rd { 0u8 } else { 255u8 };
            out[d] = v;
            out[d + 1] = v;
            out[d + 2] = v;
            out[d + 3] = f.rgba[d + 3];
        }
    }
    let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha };
    scratch.put(f.rgba);
    res
}

/// A per-pixel effect as a barrier: the f64 continuum of the tap formula on each
/// pixel, clamp+round ONCE at the output (the same discipline as
/// exec_geom_cpu).
fn pointwise_barrier(mut f: Frame, op: impl Fn(&mut [f64; 4], (f64, f64))) -> Frame {
    let w = f.w as usize;
    for (i, px) in f.rgba.chunks_exact_mut(4).enumerate() {
        let (x, y) = ((i % w) as f64, (i / w) as f64);
        let mut v = [px[0] as f64, px[1] as f64, px[2] as f64, px[3] as f64];
        op(&mut v, (x, y));
        for (c, out) in v.iter().zip(px.iter_mut()) {
            *out = c.round().clamp(0.0, 255.0) as u8;
        }
    }
    f
}

/// A single geo effect with the OLD kernels (`GeomExec::Legacy`): bytes as before
/// R.44 — the goldens pan/cammotion/mirror t26 and the rule “a series of length 1 —
/// the former pass” rest on this. Called only from `geom::apply_geo_run`.
pub(crate) fn apply_legacy_geo(
    f: &FxInstance,
    frame: Frame,
    ctx: &VideoFxCtx,
    scratch: &mut FxScratch,
) -> Frame {
    match f {
        FxInstance::CamMotion { mode, ease, time_scale, keyframes } => {
            let (px, py, z) = cam_at(keyframes, ctx.u, *ease, *time_scale);
            let nf = pan_zoom(&frame, ctx, *mode, px, py, z, scratch);
            scratch.put(frame.rgba);
            nf
        }
        FxInstance::Mirror { horizontal, vertical } => mirror(frame, *horizontal, *vertical),
        _ => {
            debug_assert!(false, "apply_legacy_geo: not a legacy geo fx");
            frame
        }
    }
}

/// Whether the chain has video effects (otherwise the composite runs without copies).
pub fn has_video_fx(fx: &[FxInstance]) -> bool {
    has_video_fx_in(fx, None, 0, 0)
}

/// The same with the context of the leading “Geometry” node (R.62): the leading node with
/// identity parameters is “not an effect” exactly when the conform frame equals the
/// canvas; otherwise it must place the frame in the box.
pub fn has_video_fx_in(
    fx: &[FxInstance],
    lead: Option<crate::conform::LeadCtx>,
    out_w: u32,
    out_h: u32,
) -> bool {
    // Layer strength (R.38 item 12) — not a pixel filter, but the rule of mixing the
    // chain input with the output; kt-engine applies it AFTER the chain.
    // A chain of only pixel strength does not touch it at all.
    fx.iter().any(|f| {
        !f.is_audio()
            && !f.is_layer_mix()
            && !crate::geom::geometry_is_identity_in(f, lead, out_w, out_h)
    })
}

/// Fit the frame into `ow`×`oh` with THE SAME bilinear as all the other
/// resamples (its mirror in kt-gpu is `Resample` with the frame covering the whole frame).
/// Needed for layer strength: the chain input can be mixed with its output only at
/// one size, and Pan/CamMotion change the size.
pub fn fit_into(src: &Frame, ow: u32, oh: u32, out: &mut [u8]) {
    bilinear_into(src, 0.0, 0.0, src.w as f64, src.h as f64, ow, oh, out);
}

// ---- pan/zoom ---------------------------------------------------------------

/// Interpolation of CamMotion keyframes (v1: smoothstep inside the segment).
/// Pub (R.29): the GPU path computes the frame with the same code, into the shader — uniforms.
pub fn cam_at(keys: &[CamKeyframe], u: f64, ease: bool, time_scale: f64) -> (f64, f64, f64) {
    // v1 default: zoom-in 1.0 → 1.35
    let default = [
        CamKeyframe { t: 0.0, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 },
        CamKeyframe { t: 1.0, pan_x: 0.0, pan_y: 0.0, zoom: 1.35 },
    ];
    let keys: &[CamKeyframe] = if keys.is_empty() { &default } else { keys };
    let u = (u * time_scale.clamp(0.05, 8.0)).clamp(0.0, 1.0);
    if u <= keys[0].t {
        let k = &keys[0];
        return (k.pan_x, k.pan_y, k.zoom);
    }
    for w in keys.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if u <= b.t {
            let span = (b.t - a.t).max(1e-9);
            let mut s = ((u - a.t) / span).clamp(0.0, 1.0);
            if ease {
                s = s * s * (3.0 - 2.0 * s); // smoothstep01
            }
            return (
                a.pan_x + (b.pan_x - a.pan_x) * s,
                a.pan_y + (b.pan_y - a.pan_y) * s,
                a.zoom + (b.zoom - a.zoom) * s,
            );
        }
    }
    let k = keys.last().unwrap();
    (k.pan_x, k.pan_y, k.zoom)
}

/// Crop frame of Pan/CamMotion in the source coordinates: (x0, y0, cw, ch).
/// Pub (R.29): the COMMON math of the CPU and GPU paths — the GPU computes the frame with
/// the same function and passes it to the shader as uniforms.
#[allow(clippy::too_many_arguments)]
pub fn pan_frame(
    src_w: u32,
    src_h: u32,
    out_w: u32,
    out_h: u32,
    mode: PanMode,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
) -> (f64, f64, f64, f64) {
    let (ow, oh) = (out_w.max(2), out_h.max(2));
    let zoom = zoom.clamp(0.05, 32.0);
    let (sw, sh) = (src_w as f64, src_h as f64);
    let ar_out = ow as f64 / oh as f64;

    // base frame in the source coordinates
    let (mut cw, mut ch) = match mode {
        PanMode::Stretch => (sw, sh),
        PanMode::Fill => {
            // crop by the short side to ar_out
            if sw / sh > ar_out { (sh * ar_out, sh) } else { (sw, sw / ar_out) }
        }
        PanMode::Fit => {
            // the largest rectangle with ar_out INSIDE the source
            if sw / sh > ar_out { (sh * ar_out, sh) } else { (sw, sw / ar_out) }
        }
    };
    cw /= zoom;
    ch /= zoom;
    // pan: −1..1 moves the frame within the free space
    let max_dx = (sw - cw) / 2.0;
    let max_dy = (sh - ch) / 2.0;
    let cx = sw / 2.0 + pan_x.clamp(-4.0, 4.0) * max_dx;
    let cy = sh / 2.0 + pan_y.clamp(-4.0, 4.0) * max_dy;
    let x0 = (cx - cw / 2.0).clamp(0.0, sw - cw.min(sw));
    let y0 = (cy - ch / 2.0).clamp(0.0, sh - ch.min(sh));
    (x0, y0, cw, ch)
}

/// Crop frame + bilinear resample into out (v1 composePanZoomOutput).
fn pan_zoom(
    src: &Frame,
    ctx: &VideoFxCtx,
    mode: PanMode,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    scratch: &mut FxScratch,
) -> Frame {
    let (ow, oh) = (ctx.out_w.max(2), ctx.out_h.max(2));
    let (x0, y0, cw, ch) = pan_frame(src.w, src.h, ctx.out_w, ctx.out_h, mode, pan_x, pan_y, zoom);
    let mut out = scratch.take((ow * oh * 4) as usize);
    bilinear_into(src, x0, y0, cw, ch, ow, oh, &mut out);
    Frame { w: ow, h: oh, rgba: out, has_alpha: src.has_alpha }
}

/// Bilinear sampling of the frame [x0,y0,cw,ch] of the source into a ow×oh buffer.
///
/// The SIMD level dispatcher (R.32): the AVX2 kernel must be byte-for-byte
/// equal to the scalar one, so the choice of path does not affect any golden.
/// The branch costs per CALL (frame/layer), not per pixel.
#[allow(clippy::too_many_arguments)]
fn bilinear_into(
    src: &Frame,
    x0: f64,
    y0: f64,
    cw: f64,
    ch: f64,
    ow: u32,
    oh: u32,
    out: &mut [u8],
) {
    #[cfg(target_arch = "x86_64")]
    if crate::simd::simd_level() == crate::simd::SimdLevel::Avx2
        // contract safeguard of the unsafe kernel: the buffers must be complete
        && src.rgba.len() >= (src.w as usize) * (src.h as usize) * 4
        && out.len() >= (ow as usize) * (oh as usize) * 4
    {
        // SAFETY: the level is checked by the runtime detector, the buffer lengths — above
        unsafe {
            crate::video_simd::bilinear_into_avx2(src, x0, y0, cw, ch, ow, oh, out);
        }
        return;
    }
    bilinear_into_scalar(src, x0, y0, cw, ch, ow, oh, out);
}

/// Scalar reference of the bilinear. NOT refactored together with the SIMD kernel:
/// it IS the definition of the bytes of the CPU path (invariant No. 11).
#[allow(clippy::too_many_arguments)]
pub(crate) fn bilinear_into_scalar(
    src: &Frame,
    x0: f64,
    y0: f64,
    cw: f64,
    ch: f64,
    ow: u32,
    oh: u32,
    out: &mut [u8],
) {
    let (sw, sh) = (src.w as i64, src.h as i64);
    for oy in 0..oh as i64 {
        let sy = y0 + (oy as f64 + 0.5) / oh as f64 * ch - 0.5;
        let y = sy.floor() as i64;
        let fy = sy - y as f64;
        for ox in 0..ow as i64 {
            let sx = x0 + (ox as f64 + 0.5) / ow as f64 * cw - 0.5;
            let x = sx.floor() as i64;
            let fx = sx - x as f64;
            let mut px = [0f64; 4];
            for (dy, wy) in [(0i64, 1.0 - fy), (1, fy)] {
                for (dx, wx) in [(0i64, 1.0 - fx), (1, fx)] {
                    let xi = (x + dx).clamp(0, sw - 1);
                    let yi = (y + dy).clamp(0, sh - 1);
                    let si = ((yi * sw + xi) * 4) as usize;
                    let w = wy * wx;
                    for (c, acc) in px.iter_mut().enumerate() {
                        *acc += src.rgba[si + c] as f64 * w;
                    }
                }
            }
            let oi = ((oy * ow as i64 + ox) * 4) as usize;
            for c in 0..4 {
                out[oi + c] = px[c].round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

// ---- simple ---------------------------------------------------------------

fn mirror(mut f: Frame, h: bool, v: bool) -> Frame {
    if !h && !v {
        return f;
    }
    // in-place swaps instead of a second buffer (item 2 of round 7); the result is byte-for-byte
    // equal to the former out[y][x] = src[sy][sx]
    let (w, hh) = (f.w as usize, f.h as usize);
    let px = |x: usize, y: usize| (y * w + x) * 4;
    if h && v {
        // rotation by 180°: pixel p ↔ total-1-p, the middle is fixed
        let total = w * hh;
        for p in 0..total / 2 {
            let (i, j) = (p * 4, (total - 1 - p) * 4);
            for c in 0..4 {
                f.rgba.swap(i + c, j + c);
            }
        }
    } else if h {
        for y in 0..hh {
            for x in 0..w / 2 {
                let (i, j) = (px(x, y), px(w - 1 - x, y));
                for c in 0..4 {
                    f.rgba.swap(i + c, j + c);
                }
            }
        }
    } else {
        for y in 0..hh / 2 {
            let (i, j) = (px(0, y), px(0, hh - 1 - y));
            for c in 0..w * 4 {
                f.rgba.swap(i + c, j + c);
            }
        }
    }
    f
}

/// “Cheap” blur v1: downscale by step times and back (bilinear).
/// Blur by downscaling. `radius_y == 0` — “as horizontally”: exactly the
/// former path (one step on both axes), therefore the goldens do not move;
/// different radii give a smear along one axis (R.33 C5).
/// Premultiply IN PLACE: `rgb' = rgb·a/255` with rounding to the
/// nearest (R.53, item 418).
///
/// Why: a mip and any convolution mix NEIGHBOURING pixels, and the colour of a transparent
/// pixel is garbage (usually black). With straight alpha this garbage
/// leaks into the visible pixels, and the title gets a dark halo along the
/// edge. Premultiplication makes the contribution of a transparent pixel zero by
/// construction.
///
/// Identity at `a == 255`: on an opaque frame the premultiplication does NOT
/// CHANGE A SINGLE BYTE — all the goldens rest on this, and therefore the step is gated by
/// `has_alpha` rather than always on.
pub fn premultiply(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a == 255 {
            continue;
        }
        for c in px[..3].iter_mut() {
            *c = ((*c as u32 * a + 127) / 255) as u8;
        }
    }
}

/// Straight alpha from the premultiplied one: `rgb' = rgb·255/a`.
///
/// Identity at `a == 255` (an opaque pixel is not touched at all) and
/// zero at `a == 0` (there is no colour there). Integer and mirrored exactly in
/// WGSL — all kt-gpu textures are `Rgba8Uint`. The function's home is HERE: it is called by
/// both the nesting boundary (kt-engine) and the premul gate of the kernels (item 418), and two
/// instances of one rule would diverge on rounding.
pub fn unpremultiply(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a == 255 {
            continue;
        }
        if a == 0 {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
            continue;
        }
        for c in px[..3].iter_mut() {
            *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
        }
    }
}

fn blur(
    mut f: Frame,
    radius: u32,
    radius_y: u32,
    scratch: &mut FxScratch,
    res_scale: u32,
) -> Frame {
    let radius = radius.clamp(1, 48);
    // R.45 (item 313): the K× execution scales the STEP — floor(K·w/(K·step))
    // == floor(w/step), the mip is the same, the look is invariant exactly; K=1 — ×1
    let step = ((radius / 3).max(1) + 1) * res_scale;
    let step_y =
        if radius_y == 0 { step } else { ((radius_y.clamp(1, 48) / 3).max(1) + 1) * res_scale };
    let (dw, dh) = ((f.w / step).max(1), (f.h / step_y).max(1));
    // ALPHA RULE (item 418): the premultiplied image is blurred, otherwise the colour
    // of transparent pixels leaks into the visible ones — a dark halo along the edge of the
    // title. On an opaque frame both conversions are identities, therefore the
    // gate by `has_alpha` leaves all the goldens untouched.
    let pm = f.has_alpha;
    if pm {
        premultiply(&mut f.rgba);
    }
    let mut small = scratch.take((dw * dh * 4) as usize);
    bilinear_into(&f, 0.0, 0.0, f.w as f64, f.h as f64, dw, dh, &mut small);
    let small_f = Frame { w: dw, h: dh, rgba: small, has_alpha: f.has_alpha };
    let mut out = scratch.take(f.rgba.len());
    bilinear_into(&small_f, 0.0, 0.0, dw as f64, dh as f64, f.w, f.h, &mut out);
    if pm {
        unpremultiply(&mut out);
    }
    scratch.put(small_f.rgba);
    scratch.put(f.rgba);
    Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha }
}

fn greenscreen(mut f: Frame, key: [u8; 3], tolerance: u32, softness: u32, spill: f64) -> Frame {
    let tol = tolerance.clamp(1, 200) as f64;
    let soft = softness.clamp(0, 100) as f64;
    let hard = (tol - soft).max(0.0);
    // Spill suppression (R.33 C5): the dominant channel of the KEY is pulled toward the
    // larger of the neighbours. spill = 0 — the former path byte-for-byte
    let spill = spill.clamp(0.0, 1.0);
    let kc = (0..3).max_by_key(|&i| key[i]).unwrap_or(1);
    // Greenscreen is deliberately left as SCALAR (R.32): measurement showed
    // only ×1.29 at a threshold of 1.3 — LLVM already vectorizes this loop
    // well by itself, and 60 lines of intrinsics in a place where byte-exact accuracy
    // matters are not worth it. The figures are in item 198.
    for px in f.rgba.chunks_exact_mut(4) {
        let d = ((px[0] as f64 - key[0] as f64).powi(2)
            + (px[1] as f64 - key[1] as f64).powi(2)
            + (px[2] as f64 - key[2] as f64).powi(2))
        .sqrt();
        if d <= hard {
            px[3] = 0;
        } else if d < tol {
            let a = (d - hard) / (tol - hard).max(1e-9);
            px[3] = (px[3] as f64 * a).round() as u8;
        }
        if spill > 0.0 && px[3] > 0 {
            let other = px[(kc + 1) % 3].max(px[(kc + 2) % 3]) as f64;
            let v = px[kc] as f64;
            if v > other {
                px[kc] = (v - (v - other) * spill).round() as u8;
            }
        }
    }
    f.has_alpha = true;
    f
}

/// Sobel by luma (v1: (77R+150G+29B)>>8, threshold, grey output).
fn edges(
    f: Frame,
    threshold: u32,
    thickness: u32,
    color: [u8; 3],
    scratch: &mut FxScratch,
    res_scale: u32,
) -> Frame {
    let th = threshold.clamp(1, 255) as i32;
    // thickness = the Sobel sampling step; 1 and white colour — the former 3×3 kernel
    // byte-for-byte (including the AVX2 path)
    // R.45 (item 313): the Sobel stride and the border ×K; the plain gate (d == 1)
    // switches off by itself at K > 1 — the effective stride ≥ 2
    let d = thickness.clamp(1, 8) as i64 * res_scale as i64;
    let plain = d == 1 && color == [255, 255, 255];
    let (w, h) = (f.w as usize, f.h as usize);
    #[cfg(target_arch = "x86_64")]
    let simd =
        crate::simd::simd_level() == crate::simd::SimdLevel::Avx2 && f.rgba.len() >= w * h * 4;
    #[cfg(not(target_arch = "x86_64"))]
    let simd = false;
    let luma: Vec<i32> = if simd {
        let mut v = vec![0i32; w * h];
        #[cfg(target_arch = "x86_64")]
        // SAFETY: the level is checked by the detector; the buffer lengths — above
        unsafe {
            crate::video_simd::luma_avx2(&mut v, &f.rgba, w * h)
        };
        v
    } else {
        f.rgba
            .chunks_exact(4)
            .map(|p| (77 * p[0] as i32 + 150 * p[1] as i32 + 29 * p[2] as i32) >> 8)
            .collect()
    };
    // ALPHA RULE (R.53, item 418): edges no longer fills alpha with 255, but RAISES it
    // to the strength of the line — `a_out = max(a_in, v)`. On an opaque
    // input (a_in = 255) this is an identity, and the bytes are the former ones; on an input with
    // alpha the outline has stopped being an opaque plate over the whole frame and
    // has become what it is asked to be — an overlay of contours.
    let keep_alpha = f.has_alpha;
    let mut out = scratch.take(f.rgba.len());
    for (o, i) in out.chunks_exact_mut(4).zip(f.rgba.chunks_exact(4)) {
        o[3] = if keep_alpha { i[3] } else { 255 };
    }
    let m = d as usize; // the border is wider with a thick line
    for y in m..h.saturating_sub(m) {
        // the vector part of the row; the scalar loop below finishes the tail
        let mut x_from = m;
        #[cfg(target_arch = "x86_64")]
        // The AVX2 kernel writes alpha 255 hard: on a frame WITH ALPHA we count with the
        // scalar path (it is not a hot path there anyway — titles, not 4K editing)
        if plain && simd && w > 2 && !keep_alpha {
            // SAFETY: the level is checked; rows y-1..y+1 exist, out is full
            x_from = unsafe { crate::video_simd::sobel_row_avx2(&mut out, &luma, w, y, th, w - 1) };
        }
        for x in x_from..w.saturating_sub(m) {
            let l = |dx: i64, dy: i64| {
                luma[(y as i64 + dy * d) as usize * w + (x as i64 + dx * d) as usize]
            };
            let gx = -l(-1, -1) - 2 * l(-1, 0) - l(-1, 1) + l(1, -1) + 2 * l(1, 0) + l(1, 1);
            let gy = -l(-1, -1) - 2 * l(0, -1) - l(1, -1) + l(-1, 1) + 2 * l(0, 1) + l(1, 1);
            let mag = (((gx * gx + gy * gy) as f64).sqrt() as i32).min(255);
            let v = if mag < th { 0 } else { mag as u8 };
            let i = (y * w + x) * 4;
            // colour of the lines: the grey value is multiplied by it (255 — the former)
            for c in 0..3 {
                out[i + c] = ((v as u32 * color[c] as u32) / 255) as u8;
            }
            if keep_alpha {
                out[i + 3] = out[i + 3].max(v);
            }
        }
    }
    scratch.put(f.rgba);
    Frame { w: f.w, h: f.h, rgba: out, has_alpha: keep_alpha }
}

/// The lowbias32 finalizer (Chris Wellons): an integer avalanche hash,
/// bit-reproducible in WGSL (u32 arithmetic without float).
#[inline]
pub fn noise_hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^= x >> 16;
    x
}

/// Frame grain for noise: one value per frame, goes into the GPU path as a uniform.
#[inline]
pub fn noise_seed(burst_index: i64) -> u32 {
    noise_hash(burst_index as u32)
}

/// Deterministic grain. R.29: a counter-based hash instead of the sequential LCG of v1 —
/// the grain of each pixel is computed from (x, y, seed) independently, so the traversal
/// order does not matter → the same formula in WGSL gives byte-for-byte the same noise
/// on the GPU. An authorised change of the noise picture (only the noise golden t26
/// was regenerated).
/// Grain. `size == 1` and `color == false` — the former formula byte-for-byte
/// (including the AVX2 path); coarse grain and coloured noise go through the scalar path.
fn noise(
    mut f: Frame,
    amount: f64,
    size: u32,
    color: bool,
    burst_index: i64,
    res_scale: u32,
) -> Frame {
    let k = (amount.clamp(0.0, 1.0) * 32.0) as i64;
    if k == 0 {
        return f;
    }
    let seed = noise_seed(burst_index);
    let w = f.w;
    let size = size.clamp(1, 64);
    // R.45 (item 313): the cell ×K — floor((K·p+i)/(K·size)) == floor(p/size),
    // the grain pattern matches 1× BIT FOR BIT. The formula branch (the special one at
    // size==1 && !color) is chosen by the USER size — the formulas have different grain,
    // and K must not silently substitute one for the other.
    let cell = size * res_scale;
    if size > 1 || color {
        let off_at = |x: u32, y: u32, ch: u32| -> i32 {
            let h = noise_hash(x ^ noise_hash(y ^ noise_hash(ch ^ seed)));
            let n = ((h >> 16) & 0x7fff) as i64 - 16384;
            (n * k / 32767) as i32
        };
        for (p, px) in f.rgba.chunks_exact_mut(4).enumerate() {
            let (x, y) = (p as u32 % w / cell, p as u32 / w / cell);
            if color {
                for (c, v) in px.iter_mut().take(3).enumerate() {
                    *v = (*v as i32 + off_at(x, y, c as u32 + 1)).clamp(0, 255) as u8;
                }
            } else {
                let off = off_at(x, y, 0);
                for v in px.iter_mut().take(3) {
                    *v = (*v as i32 + off).clamp(0, 255) as u8;
                }
            }
        }
        return f;
    }
    if res_scale > 1 {
        // special formula on the K× raster: the coordinates are divided by K — the 1× grain
        // pattern is reproduced block by block (the AVX2 path stays with K=1)
        for (p, px) in f.rgba.chunks_exact_mut(4).enumerate() {
            let (x, y) = (p as u32 % w / res_scale, p as u32 / w / res_scale);
            let h = noise_hash(x ^ noise_hash(y ^ seed));
            let n = ((h >> 16) & 0x7fff) as i64 - 16384;
            let off = (n * k / 32767) as i32;
            for c in px.iter_mut().take(3) {
                *c = (*c as i32 + off).clamp(0, 255) as u8;
            }
        }
        return f;
    }
    #[cfg(target_arch = "x86_64")]
    if crate::simd::simd_level() == crate::simd::SimdLevel::Avx2
        && w >= 8
        && f.rgba.len() >= (w as usize) * (f.h as usize) * 4
    {
        // SAFETY: the level is checked by the detector, the buffer length — above
        unsafe { crate::video_simd::noise_avx2(&mut f.rgba, w, f.h, seed, k) };
        // the column tail (w % 8) — with the same formula as the scalar below
        let vec_w = w & !7;
        if vec_w < w {
            for y in 0..f.h {
                for x in vec_w..w {
                    let i = ((y * w + x) * 4) as usize;
                    let h = noise_hash(x ^ noise_hash(y ^ seed));
                    let n = ((h >> 16) & 0x7fff) as i64 - 16384;
                    let off = (n * k / 32767) as i32;
                    for c in 0..3 {
                        f.rgba[i + c] = (f.rgba[i + c] as i32 + off).clamp(0, 255) as u8;
                    }
                }
            }
        }
        return f;
    }
    for (p, px) in f.rgba.chunks_exact_mut(4).enumerate() {
        let (x, y) = (p as u32 % w, p as u32 / w);
        let h = noise_hash(x ^ noise_hash(y ^ seed));
        let n = ((h >> 16) & 0x7fff) as i64 - 16384;
        let off = (n * k / 32767) as i32;
        for c in px.iter_mut().take(3) {
            *c = (*c as i32 + off).clamp(0, 255) as u8;
        }
    }
    f
}

/// VHS: RGB shift + scan lines (v1: lines every 4, jitter from time).
#[allow(clippy::too_many_arguments)]
fn vhs(
    mut f: Frame,
    rgb_shift: u32,
    bleed: f64,
    scan_step: u32,
    jitter: f64,
    burst_index: i64,
    scratch: &mut FxScratch,
    res_scale: u32,
) -> Frame {
    // R.45 (item 313): the shift ×K, the whole ROW context (scan lines and the jitter hash)
    // — from the logical row y/K: the scan condition itself colours the K
    // rows of the block; line_off and scan_step remain LOGICAL,
    // the jitter amplitude jx scales itself (it is in width fractions). K=1 — y/1.
    let rs = res_scale as usize;
    let shift = rgb_shift.clamp(0, 12) as i64 * res_scale as i64;
    let bleed = bleed.clamp(0.0, 1.0);
    let line_off = ((burst_index / 3000) % 7) as usize;
    let (w, h) = (f.w as usize, f.h as usize);
    // scan_step = 4 and jitter = 0 — the former behaviour byte-for-byte (including
    // the AVX2 path); everything else is computed by the scalar path
    let step = scan_step.clamp(0, 32) as usize;
    let jitter = jitter.clamp(0.0, 1.0);
    let plain = step == 4 && jitter == 0.0;
    let jq = (jitter * 1024.0).round() as i64;
    let seed = noise_seed(burst_index);
    // only the pixels of the CURRENT row are read → a row scratch instead of a full
    // clone (item 2 of round 7); the arithmetic is unchanged
    let mut row = scratch.take(w * 4);
    for y in 0..h {
        let scan = if step > 0 && (y / rs + line_off).is_multiple_of(step) { 1i64 } else { 0 };
        // the row jitter: a shift of the whole row by a width fraction,
        // INTEGER: GPU round() rounds halves to even, and on the fractional path the row
        // sometimes drifted a pixel past the CPU (t181).
        // The parameter quantization is the same for both paths.
        let jx = if jq == 0 {
            0i64
        } else {
            let hsh = noise_hash((y / rs) as u32 ^ noise_hash(seed));
            let n = ((hsh >> 16) & 0x7fff) as i64 - 16384;
            (n * jq / 32767) * w as i64 / 4096
        };
        let r0 = y * w * 4;
        row.copy_from_slice(&f.rgba[r0..r0 + w * 4]);
        // the vector interior of the row; the edges and the tail — by a scalar loop
        let mut x_from = 0usize;
        #[cfg(target_arch = "x86_64")]
        if plain && crate::simd::simd_level() == crate::simd::SimdLevel::Avx2 {
            let d = (shift + scan) as usize;
            // SAFETY: the level is checked; row of length w*4, the row slice too
            x_from = unsafe {
                crate::video_simd::vhs_row_avx2(
                    &mut f.rgba[r0..r0 + w * 4],
                    &row,
                    w,
                    d,
                    bleed,
                    scan == 1,
                    0,
                )
            };
        }
        // the left edge (the vector skipped it, starting at x = d)
        #[cfg(target_arch = "x86_64")]
        if x_from > 0 {
            for x in 0..(shift + scan) as usize {
                let i = r0 + x * 4;
                let xr = (x as i64 + shift + scan).clamp(0, w as i64 - 1) as usize;
                let xb = (x as i64 - shift - scan).clamp(0, w as i64 - 1) as usize;
                let r = row[xr * 4] as f64;
                let b = row[xb * 4 + 2] as f64;
                f.rgba[i] = (row[x * 4] as f64 * (1.0 - bleed) + r * bleed).round() as u8;
                f.rgba[i + 2] = (row[x * 4 + 2] as f64 * (1.0 - bleed) + b * bleed).round() as u8;
                if scan == 1 {
                    f.rgba[i + 1] = row[x * 4 + 1].saturating_sub(18);
                }
            }
        }
        for x in x_from..w {
            let i = r0 + x * 4;
            // the row base drifts by jx (0 — the former formula verbatim)
            let xj = (x as i64 + jx).clamp(0, w as i64 - 1) as usize;
            let xr = (xj as i64 + shift + scan).clamp(0, w as i64 - 1) as usize;
            let xb = (xj as i64 - shift - scan).clamp(0, w as i64 - 1) as usize;
            let r = row[xr * 4] as f64;
            let b = row[xb * 4 + 2] as f64;
            f.rgba[i] = (row[xj * 4] as f64 * (1.0 - bleed) + r * bleed).round() as u8;
            f.rgba[i + 2] = (row[xj * 4 + 2] as f64 * (1.0 - bleed) + b * bleed).round() as u8;
            if scan == 1 {
                f.rgba[i + 1] = row[xj * 4 + 1].saturating_sub(18);
            } else if jx != 0 {
                // with jitter the WHOLE row moves, including the green: without this
                // the green would remain from the unshifted pixel (an in-place artefact,
                // because of it CPU and GPU diverged)
                f.rgba[i + 1] = row[xj * 4 + 1];
            }
        }
    }
    scratch.put(row);
    f
}

/// Bloom: downscale ×1/4 → upscale, additive ×strength×0.35 (v1).
fn glow(
    mut f: Frame,
    strength: f64,
    radius: u32,
    threshold: u32,
    tint: [u8; 3],
    scratch: &mut FxScratch,
    res_scale: u32,
) -> Frame {
    let s = strength.clamp(0.0, 2.0) * 0.35;
    if s <= 0.0 {
        return f;
    }
    // radius 4, threshold 0 and white tint — the former path byte-for-byte
    // R.45 (item 313): the mip divisor ×K — the mip is the same, the look is invariant
    let r = radius.clamp(1, 32) * res_scale;
    let th = threshold.min(255) as i32;
    let plain = th == 0 && tint == [255, 255, 255];
    let (dw, dh) = ((f.w / r).max(1), (f.h / r).max(1));
    // ALPHA RULE (item 418): light does not come from transparent places. We mip
    // a COPY of the premultiplied frame — and add it as is,
    // without the reverse conversion: the premultiplied RGB IS the contribution of
    // the light. The addition base stays straight. On an opaque frame the copy is
    // byte-for-byte equal to the original — the former path is intact.
    let pm = f.has_alpha;
    let mut pmbuf = None;
    if pm {
        let mut b = scratch.take(f.rgba.len());
        b.copy_from_slice(&f.rgba);
        premultiply(&mut b);
        pmbuf = Some(Frame { w: f.w, h: f.h, rgba: b, has_alpha: true });
    }
    let mip_src = pmbuf.as_ref().unwrap_or(&f);
    let mut small = scratch.take((dw * dh * 4) as usize);
    bilinear_into(mip_src, 0.0, 0.0, f.w as f64, f.h as f64, dw, dh, &mut small);
    if let Some(fr) = pmbuf {
        scratch.put(fr.rgba);
    }
    let small_f = Frame { w: dw, h: dh, rgba: small, has_alpha: false };
    let mut up = scratch.take(f.rgba.len());
    bilinear_into(&small_f, 0.0, 0.0, dw as f64, dh as f64, f.w, f.h, &mut up);
    // in-place addition: only the pixel itself + up is read (item 2 of round 7)
    let pixels = f.rgba.len().min(up.len()) / 4;
    let mut done = 0usize;
    #[cfg(target_arch = "x86_64")]
    if plain && crate::simd::simd_level() == crate::simd::SimdLevel::Avx2 {
        // SAFETY: the level is checked by the detector; pixels do not go past either buffer
        unsafe { crate::video_simd::glow_add_avx2(&mut f.rgba, &up, s, pixels) };
        done = pixels;
    }
    for (o, u) in f.rgba.chunks_exact_mut(4).zip(up.chunks_exact(4)).skip(done) {
        // threshold by luma of the BLURRED frame: a soft knee of width th
        let w = if th == 0 {
            1.0
        } else {
            let luma = (77 * u[0] as i32 + 150 * u[1] as i32 + 29 * u[2] as i32) >> 8;
            (((luma - th) as f64) / th as f64).clamp(0.0, 1.0)
        };
        for c in 0..3 {
            let k = s * w * tint[c] as f64 / 255.0;
            o[c] = (o[c] as f64 + u[c] as f64 * k).min(255.0) as u8;
        }
    }
    scratch.put(small_f.rgba);
    scratch.put(up);
    f
}

// The EventGrid mosaic with R.44 is computed by the fused geo pass (`crate::geom`,
// the Fold primitive): the tile-local phase reproduces the former kernel verbatim —
// the golden t26 “eventgrid” did not change. The guard “grid larger than the frame” (item 306)
// lives in the program's builder.
