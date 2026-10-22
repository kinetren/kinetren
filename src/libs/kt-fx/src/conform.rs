//! Conforming the source frame to the canvas (R.62, F-MED-004): ONE function
//! for the decode size, the placement frame, thumbnails, preview and export.
//!
//! The rule (“fit / fill / stretch / as is”) is a property
//! of the PROJECT with an exception for the event; here it is already resolved
//! (`ConformRule`). The source view — native × pixel aspect ratio
//! (`MediaInfo::view_size`); the canvas — the logical output size.
//!
//! The decoder contract “a frame of exactly the requested quality” does NOT change —
//! what changes is who computes the requested size: `conform_geometry(...).decode` gives
//! the size to which the decoder pulls the frame (at step 1), and the leading node
//! “Geometry” in the geo program (`geom.rs`) puts it into the `dest` frame
//! of the canvas; outside the frame — transparent.

use crate::plan::MAX_INTER_DIM;
pub use kt_model::ConformRule;

/// Context of the leading node: the source view (square pixels) and the
/// allowed rule. It travels from kt-engine to kt-fx via `VideoFxCtx`
/// and to kt-gpu via `chain_ops`; both backends receive the same numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeadCtx {
    /// The view of the source frame (width × SAR, height). `None` — the view is
    /// unknown: the rule degenerates to “stretch”, deterministically.
    pub view: Option<(f64, f64)>,
    pub rule: ConformRule,
}

impl LeadCtx {
    pub fn new(view: Option<(f64, f64)>, rule: ConformRule) -> LeadCtx {
        LeadCtx { view, rule }
    }
}

/// Layer geometry: the placement frame on the canvas (logical pixels, at
/// zoom 1 and position 0) and the decode size of the full frame (without a step).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerGeom {
    /// Size of the cropped source frame on the canvas.
    pub dest: (f64, f64),
    /// The size to which the decoder pulls the FULL frame (even, ≥ 2, with a
    /// ceiling `MAX_INTER_DIM` proportionally).
    pub decode: (u32, u32),
}

/// Frame size for the view `view` on the canvas `canvas` by the rule (zoom 1).
/// A zero/unknown view — “stretch”.
pub fn conform_dest(view: Option<(f64, f64)>, canvas: (f64, f64), rule: ConformRule) -> (f64, f64) {
    let (cw, ch) = (canvas.0.max(2.0), canvas.1.max(2.0));
    let Some((vw, vh)) = view.filter(|(w, h)| *w > 0.0 && *h > 0.0) else {
        return (cw, ch);
    };
    match rule {
        ConformRule::Stretch => (cw, ch),
        ConformRule::AsIs => (vw, vh),
        ConformRule::Fit => {
            let k = (cw / vw).min(ch / vh);
            (vw * k, vh * k)
        }
        ConformRule::Fill => {
            let k = (cw / vw).max(ch / vh);
            (vw * k, vh * k)
        }
    }
}

/// Full geometry: the source view, the canvas, the rule, the crop (fractions of the source:
/// left, top, right, bottom). The crop cuts the view BEFORE the rule: “fill”
/// fills with the cropped one; the decode size is the frame, stretched back to
/// the whole frame (the decoder gives the whole frame, the node cuts it out).
pub fn conform_geometry(
    view: Option<(f64, f64)>,
    canvas: (u32, u32),
    rule: ConformRule,
    crop: [f64; 4],
) -> LayerGeom {
    let kx = (1.0 - crop[0].clamp(0.0, 0.95) - crop[2].clamp(0.0, 0.95)).max(0.05);
    let ky = (1.0 - crop[1].clamp(0.0, 0.95) - crop[3].clamp(0.0, 0.95)).max(0.05);
    let cropped = view.map(|(w, h)| (w * kx, h * ky));
    let canvas_f = (canvas.0.max(2) as f64, canvas.1.max(2) as f64);
    let dest = conform_dest(cropped, canvas_f, rule);
    let (mut dw, mut dh) = (dest.0 / kx, dest.1 / ky);
    // ceiling of intermediate buffers — a project constant (item 314), proportionally
    let cap = MAX_INTER_DIM as f64;
    if dw > cap || dh > cap {
        let k = (cap / dw).min(cap / dh);
        dw *= k;
        dh *= k;
    }
    let even = |v: f64| ((v.round() as u32).max(2)) & !1;
    LayerGeom { dest, decode: (even(dw), even(dh)) }
}
