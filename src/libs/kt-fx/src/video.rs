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

// a per-pixel effect as a barrier is the plugin's `frame` (`kt_plugin_api::frame_with_at`, the one
// text of that loop since plugin-api-stage1: the f64 tap per pixel, clamp+round ONCE at the output)

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

/// The bilinear of a CPU-side resample (a stream stage's output, an effect thumbnail's base).
#[allow(clippy::too_many_arguments)]
pub(crate) fn bilinear_into(
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
