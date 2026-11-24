//! The one key of a moment (durable-moment-key D2): what the display's memo, the render-ahead producer,
//! the kept-moment recheck and the disk tier find a finished frame by. It names each frame by its source
//! ([`Leaf`]), never by whether it is decoded now; where nothing names it (a private cache) the content
//! serial does, as the executor's keys always did.

use crate::TransMix;
use crate::video_plan::{Cam3d, FramePlan, LayerInput, LayerSrc, Leaf, PlanLayer};
use xxhash_rust::xxh3::Xxh3;

/// The moment's key. A field that makes another picture of the same plan is hashed here, and nowhere
/// else.
pub fn moment_key(plan: &FramePlan<'_>) -> u64 {
    let mut h = Xxh3::new();
    put_plan(&mut h, plan);
    h.digest()
}

/// Debug text streamed into the hash (frame-work-by-edit, row 364): no string built per frame.
pub(crate) fn put_debug(h: &mut Xxh3, v: &dyn std::fmt::Debug) {
    struct Into<'h>(&'h mut Xxh3);
    impl std::fmt::Write for Into<'_> {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            self.0.update(s.as_bytes());
            Ok(())
        }
    }
    let _ = std::fmt::Write::write_fmt(&mut Into(h), format_args!("{v:?}"));
}

fn put_leaf(h: &mut Xxh3, leaf: Leaf, content: u64) {
    if leaf.key != 0 {
        h.update(&[1u8]);
        h.update(&leaf.key.to_le_bytes());
    } else {
        h.update(&[0u8]);
        h.update(&content.to_le_bytes());
    }
}

fn put_cam3d(h: &mut Xxh3, c: &Option<Cam3d>) {
    match c {
        None => h.update(&[0u8]),
        Some(c) => {
            h.update(&[1u8, c.cam.ortho as u8]);
            for v in [c.cam.pos_x, c.cam.pos_y, c.cam.pos_z, c.cam.rot_x, c.cam.rot_y, c.cam.rot_z, c.cam.fov_deg, c.world_scale] {
                h.update(&v.to_bits().to_le_bytes());
            }
            if let Some((x, y, z)) = c.cam.target {
                for v in [x, y, z] {
                    h.update(&v.to_bits().to_le_bytes());
                }
            }
        }
    }
}

/// The context of the leading “Geometry” node (R.62): the same frame with the same chain but another
/// rule or view gives another picture; the raster moves the "as is" frame (still-image-frame-rule).
fn put_lead(h: &mut Xxh3, lead: &Option<kt_fx::conform::LeadCtx>) {
    match lead {
        None => h.update(&[0u8]),
        Some(l) => {
            h.update(&[1u8, l.rule as u8]);
            for v in [l.view, l.raster] {
                match v {
                    None => h.update(&[0u8]),
                    Some((w, hh)) => {
                        h.update(&[1u8]);
                        h.update(&w.to_bits().to_le_bytes());
                        h.update(&hh.to_bits().to_le_bytes());
                    }
                }
            }
        }
    }
}

/// Transition parameters (R.33 S4). The ext scalars of volumetric kinds (R.51) are NOT here and must not
/// be: they are a function of (kind, p, k). A plugin kind is named by its id, not by its mode: the mode
/// is a function of the registry's order (D12).
fn put_mix(h: &mut Xxh3, m: &TransMix) {
    match crate::transitions::plugin_id(m.mode) {
        Some(id) => {
            h.update(&[1u8]);
            h.update(id.as_bytes());
            h.update(&[0u8]);
        }
        None => h.update(&m.mode.to_le_bytes()),
    }
    for v in m.p {
        h.update(&v.to_bits().to_le_bytes());
    }
}

fn put_src(h: &mut Xxh3, s: &LayerSrc<'_>) {
    h.update(&s.source.0.to_le_bytes());
    h.update(&s.cache_idx.to_le_bytes());
    put_leaf(h, s.leaf, s.content);
    // frame blending (R.37 S5): the same two frames with another weight give a DIFFERENT picture
    match &s.blend {
        None => h.update(&[0u8]),
        Some(b) => {
            h.update(&[1u8]);
            h.update(&b.cache_idx.to_le_bytes());
            h.update(&b.k.to_le_bytes());
            put_leaf(h, b.leaf, b.content);
        }
    }
    h.update(&s.fx_hash.to_le_bytes());
    // the chain's wiring (fx-graph-dag D3): a line adds nothing
    if !s.fx_in.is_empty() {
        put_debug(h, &s.fx_in);
    }
    // each read is a layer of its own frame (fx-graph-dag D2)
    for (slot, r) in &s.reads {
        h.update(&(*slot as u64).to_le_bytes());
        put_src(h, r);
    }
    // the masks evaluated at THIS frame (shape-masks): maskless layers add nothing
    if !s.masks.is_empty() {
        put_debug(h, &s.masks);
    }
    // decode scale step (R.46, item 318): a frame of the same key in another size gives another picture
    h.update(&s.src_scale.to_le_bytes());
    h.update(&s.u.to_bits().to_le_bytes());
    put_cam3d(h, &s.cam3d);
    put_lead(h, &s.lead);
    h.update(&[s.blend_mode as u8]);
    match &s.input {
        // named by its leaf: neither its presence nor its size says more
        LayerInput::Frame(_) | LayerInput::Held(_) if s.leaf.key != 0 => h.update(&[1u8]),
        LayerInput::Frame(None) => h.update(&[0u8]),
        LayerInput::Frame(Some(f)) => {
            h.update(&[1u8, f.has_alpha as u8]);
            h.update(&f.width.to_le_bytes());
            h.update(&f.height.to_le_bytes());
        }
        // the frame it stands for (stream-picture-stays D3): the same key either way
        LayerInput::Held(hl) => {
            h.update(&[1u8, 0u8]);
            h.update(&hl.out.w.to_le_bytes());
            h.update(&hl.out.h.to_le_bytes());
        }
        // the subtree goes ENTIRELY into the key (R.50): a reel has neither a frame nor an index by
        // which it could be told apart
        LayerInput::Nest(n) => {
            h.update(&[2u8]);
            h.update(&n.seq.0.to_le_bytes());
            // the same reel at the same time in “blend” and in “nearest” (R.56, item 448)
            h.update(&[n.blend as u8]);
            // the REEL tier (R.58, item 459): above the chain input a fit is placed before the chain
            h.update(&n.nest_scale.to_le_bytes());
            h.update(&n.content_hash.to_le_bytes());
            h.update(&n.inner_t.0.to_le_bytes());
            put_plan(h, &n.plan);
        }
    }
}

fn put_plan(h: &mut Xxh3, plan: &FramePlan<'_>) {
    h.update(&plan.t.0.to_le_bytes());
    h.update(&plan.out_w.to_le_bytes());
    h.update(&plan.out_h.to_le_bytes());
    for layer in &plan.layers {
        match layer {
            PlanLayer::One { src, alpha_k, scale_alpha, fade } => {
                h.update(&[1u8, *scale_alpha as u8]);
                h.update(&alpha_k.to_le_bytes());
                match fade {
                    None => h.update(&[0u8]),
                    Some(f) => {
                        h.update(&[1u8, f.incoming as u8]);
                        h.update(&f.k.to_le_bytes());
                        put_mix(h, &f.mix);
                    }
                }
                put_src(h, src);
            }
            PlanLayer::Dissolve { a, b, k, alpha_k, mix } => {
                h.update(&[2u8]);
                h.update(&k.to_le_bytes());
                h.update(&alpha_k.to_le_bytes());
                put_mix(h, mix);
                put_src(h, a);
                put_src(h, b);
            }
        }
    }
}
