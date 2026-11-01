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

/// Frame in progress — the plugin API's type (plugin-api-barriers, D1), so a barrier plugin takes
/// and hands back the host's frame without a copy.
pub use kt_plugin_api::barrier::Frame;

/// Pool of RGBA buffers of the chain (item 2 of round 7): previously each effect allocated
/// a full-size buffer for EACH frame (Mirror+Grid+Glow+VHS ≈ 6 allocations
/// and 5 full passes per compose). take() returns a zeroed buffer, because
/// edges/event_grid/pan_zoom rely on zero initialization.
/// The effects' arithmetic is unchanged: golden test 26 byte-for-byte.
#[derive(Default)]
pub struct FxScratch {
    bufs: Vec<Vec<u8>>,
    /// What the executor RAN since the caller last reset it (morpher-event-grid-cost, D1).
    /// Plain integers: the frame path neither allocates nor locks for it (invariant 12).
    pub cost: FrameCost,
}

/// The work of composing a frame, counted by the plan executor that did it — the same struct on
/// both backends (morpher-event-grid-cost, D1). A memoized layer costs nothing, honestly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FrameCost {
    /// Passes over the frame: a fused geometry run, a legacy geometry step, a barrier kernel.
    pub passes: u32,
    /// Sub-samples of the fused geometry passes (`GeomProgram::sample_cost`); a program whose
    /// tap count varies per pixel (perspective) adds nothing and sets `subsamples_unknown`.
    pub subsamples: u64,
    pub subsamples_unknown: bool,
    /// Copies of the dry side of a layer strength, and the mixes that use them.
    pub snapshots: u32,
    pub mixes: u32,
}

impl FrameCost {
    pub fn add(&mut self, o: FrameCost) {
        self.passes += o.passes;
        self.subsamples += o.subsamples;
        self.subsamples_unknown |= o.subsamples_unknown;
        self.snapshots += o.snapshots;
        self.mixes += o.mixes;
    }

    /// The cost of one fused geometry pass.
    pub fn geom(prog: &crate::geom::GeomProgram) -> FrameCost {
        let sc = prog.sample_cost();
        FrameCost {
            passes: 1,
            subsamples: sc.unwrap_or(0),
            subsamples_unknown: sc.is_none(),
            ..FrameCost::default()
        }
    }
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

/// A barrier plugin draws its buffers from the same pool (invariant 12).
impl kt_plugin_api::barrier::Scratch for FxScratch {
    fn take(&mut self, len: usize) -> Vec<u8> {
        FxScratch::take(self, len)
    }
    fn put(&mut self, b: Vec<u8>) {
        FxScratch::put(self, b)
    }
}

/// The raster code barrier plugins share with the host (plugin-api-barriers, D3): the same
/// functions the core's own paths call, so each keeps ONE text and its SIMD twin.
pub struct Raster;

impl kt_plugin_api::barrier::RasterHost for Raster {
    fn resample(&self, src: &Frame, tw: u32, th: u32, scratch: &mut dyn kt_plugin_api::barrier::Scratch) -> Frame {
        let mut out = scratch.take((tw * th * 4) as usize);
        bilinear_into(src, 0.0, 0.0, src.w as f64, src.h as f64, tw, th, &mut out);
        Frame { w: tw, h: th, rgba: out, has_alpha: src.has_alpha }
    }
    fn sample(&self, f: &Frame, x: f64, y: f64) -> [f64; 4] {
        crate::geom::sample_bilinear_f64(f, x, y)
    }
    fn premultiply(&self, rgba: &mut [u8]) {
        premultiply(rgba)
    }
    fn unpremultiply(&self, rgba: &mut [u8]) {
        unpremultiply(rgba)
    }
}

/// A barrier plugin's kernel and the slot's values (plugin-api-barriers, D1): `None` — the slot is
/// not a plugin with a barrier kernel.
pub fn plugin_barrier(f: &FxInstance) -> Option<(&'static dyn kt_plugin_api::barrier::BarrierKernel, &[f64])> {
    let FxInstance::Plugin(slot) = f else { return None };
    Some((slot.plugin()?.barrier()?, &slot.values))
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
                    scratch.cost.passes += 1;
                    apply_legacy_geo(step, frame, ctx, scratch)
                }
                crate::geom::GeomExec::Fused(prog) => {
                    let c = FrameCost::geom(&prog);
                    scratch.cost.add(c);
                    let nf = crate::geom::exec_geom_cpu(&prog, &frame, scratch);
                    scratch.put(frame.rgba);
                    nf
                }
            },
            crate::plan::PlanOp::Barrier { idx, res_scale } => {
                scratch.cost.passes += 1;
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
    // a barrier plugin (plugin-api-barriers): one dynamic call per frame, the loop inside the kernel;
    // SIMD is the host's ONE switch (R.32)
    if let Some((b, values)) = plugin_barrier(f) {
        let cx = kt_plugin_api::barrier::BarrierCtx {
            res_scale: rs,
            burst_index: ctx.burst_index,
            simd: crate::simd::simd_level() == crate::simd::SimdLevel::Avx2,
            raster: &Raster,
        };
        return b.frame_values(values, frame, &cx, scratch);
    }
    match f {
        // per-pixel R.47 effects in a BARRIER position (leading/trailing slot):
        // the same f64 text of the formula as the tap of the fused pass — one home
        // a point-wise plugin at the barrier position: the same tap as inside the fused pass
        f @ FxInstance::Plugin(_) if crate::geom::PluginTap::of(f).is_some() => {
            // the barrier's stage is the frame itself (D11.1)
            let stage = kt_plugin_api::TapCtx { stage_w: frame.w, stage_h: frame.h, burst_index: ctx.burst_index };
            let t = crate::geom::PluginTap::of_at(f, stage).expect("checked");
            let mut frame = frame;
            let w = frame.w;
            t.apply_frame(&mut frame.rgba, w);
            frame
        }
        // R.47 barriers Kv3
        _ => {
            debug_assert!(false, "barrier op on non-barrier fx");
            frame
        }
    }
}

// a per-pixel effect as a barrier is the plugin's `frame` (`kt_plugin_api::frame_with_at`, the one
// text of that loop since plugin-api-stage1: the f64 tap per pixel, clamp+round ONCE at the output)

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
        f @ FxInstance::Plugin(_) => match crate::geom::plugin_lone(f) {
            Some(kt_plugin_api::Lone::Mirror { h, v }) => mirror(frame, h, v),
            None => {
                debug_assert!(false, "apply_legacy_geo: a plugin without a lone path");
                frame
            }
        },
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
            // a stream stage ran on the source before the chain (datamosh-codec-path, D1)
            && !f.is_stream()
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

/// The lowbias32 finalizer (Chris Wellons): an integer avalanche hash, bit-reproducible in WGSL
/// (u32 arithmetic without float) — one text in the plugin API since plugin-api-barriers 4.2.
pub use kt_plugin_api::barrier::noise_hash;

/// Frame grain for noise: one value per frame, goes into the GPU path as a uniform.
pub use kt_plugin_api::barrier::noise_seed;

// The EventGrid mosaic with R.44 is computed by the fused geo pass (`crate::geom`,
// the Fold primitive): the tile-local phase reproduces the former kernel verbatim —
// the golden t26 “eventgrid” did not change. The guard “grid larger than the frame” (item 306)
// lives in the program's builder.
