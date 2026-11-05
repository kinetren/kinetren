//! The basic transition kinds as plugins (plugin-api-stage1, stage 5, D12): the wipes, the motions
//! and the patterns. Each kernel is the former `TransMix::of` arm (its `pack`) and `kernel()` arm
//! (its `pixel`) of kt-engine's `transitions.rs`, verbatim; its WGSL is the former `trans()` arm of
//! composite.wgsl. t1019 holds the pixels against the enum, t224 the GPU against the CPU, t973 the
//! file text.
//!
//! The passport lists the rows in the former variant's FIELD order (the file writes them in that
//! order); the number rows keep the former specs, so the window's grid is the same.

use kt_plugin_api::math::FrozenMath;
use std::sync::Arc;

use kt_plugin_api::trans::{
    Kind, Row, count, dir_pack, dir_vec, direct, flag, flag_pack, kind,
    rgb, smooth, soft,
};
use kt_plugin_api::{ParamUnit, Plugin, Store};

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(wipe()),
        Arc::new(iris()),
        Arc::new(clock()),
        Arc::new(barn()),
        Arc::new(luma()),
        Arc::new(push()),
        Arc::new(slide()),
        Arc::new(zoom()),
        Arc::new(spin()),
        Arc::new(flash()),
        Arc::new(dip()),
        Arc::new(blinds()),
        Arc::new(checker()),
        Arc::new(blocks()),
        Arc::new(spiral()),
        Arc::new(split()),
        Arc::new(squeeze()),
        Arc::new(stretch()),
    ]
}

/// The iris shape row's keys (the former `IrisShape` text).
pub const SHAPES: &[&str] = &["circle", "diamond", "box"];

const WIPE: &str = "trn.cat.wipe";
const MOTION: &str = "trn.cat.motion";
const PATTERN: &str = "trn.cat.pattern";
const STYLE: &str = "trn.cat.style";

// ---------------------------------------------------------------------------
// wipes
// ---------------------------------------------------------------------------

/// The edge moves across the frame: a linear wipe, `angle` gives a diagonal.
pub fn wipe() -> Kind {
    kind("wipe", "Wipe", "trn.kind.wipe",
        WIPE,
        false,
        vec![Row::Dir, smooth("angle", -180.0, 180.0, 0.0, 1.0, ParamUnit::Deg, Store::F64), soft(0.15)],
        include_str!("wipe.wgsl"),
        |v| {
            let mut p = [0.0f32; 8];
            let (dx, dy) = dir_vec(v[0]);
            let a = (v[1] as f32).to_radians();
            let (s, c) = (a.fsin(), a.fcos());
            // the angle turns the direction — the diagonal is obtained by it
            p[0] = dx * c - dy * s;
            p[1] = dx * s + dy * c;
            p[2] = v[2] as f32;
            p
        })
}

/// Opening from the centre: circle/diamond/rectangle; the centre (R.51) in fractions of the frame.
pub fn iris() -> Kind {
    kind("iris", "Iris", "trn.kind.iris",
        WIPE,
        false,
        vec![Row::Choice("shape", SHAPES), Row::Center("cx"), Row::Center("cy"), soft(0.15)],
        include_str!("iris.wgsl"),
        |v| {
            let mut p = [0.0f32; 8];
            p[0] = v[0] as f32;
            p[1] = v[3] as f32;
            p[2] = v[1] as f32;
            p[3] = v[2] as f32;
            p
        })
}

/// A clock hand around the centre.
pub fn clock() -> Kind {
    kind("clock", "Clock", "trn.kind.clock",
        WIPE,
        false,
        vec![Row::Flag("reverse"), Row::Center("cx"), Row::Center("cy"), soft(0.15)],
        include_str!("clock.wgsl"),
        |v| {
            let mut p = [0.0f32; 8];
            p[0] = flag(v[0]);
            p[1] = v[3] as f32;
            p[2] = v[1] as f32;
            p[3] = v[2] as f32;
            p
        })
}

/// Shutters: two leaves diverge from the centre.
pub fn barn() -> Kind {
    kind("barn", "Barn doors", "trn.kind.barn",
        WIPE,
        false,
        vec![Row::Flag("vertical"), soft(0.15)],
        include_str!("barn.wgsl"),
        |v| {
            let mut p = [0.0f32; 8];
            p[0] = flag(v[0]);
            p[1] = v[1] as f32;
            p
        })
}

/// A wipe by the BRIGHTNESS of the outgoing frame — a “reveal” by the picture.
pub fn luma() -> Kind {
    kind("luma", "Luma wipe", "trn.kind.luma",
        WIPE,
        false,
        vec![Row::Flag("invert"), soft(0.15)],
        include_str!("luma.wgsl"),
        |v| {
            let mut p = [0.0f32; 8];
            p[0] = flag(v[0]);
            p[1] = v[1] as f32;
            p
        })
}

// ---------------------------------------------------------------------------
// motion
// ---------------------------------------------------------------------------

/// The incoming one pushes the outgoing one off the edge.
pub fn push() -> Kind {
    kind("push", "Push", "trn.kind.push", MOTION, false, vec![Row::Dir], include_str!("push.wgsl"), dir_pack)
}

/// The incoming one slides over the top, the outgoing one stays.
pub fn slide() -> Kind {
    kind("slide", "Slide", "trn.kind.slide", MOTION, false, vec![Row::Dir], include_str!("slide.wgsl"), dir_pack)
}

/// A surge with approach: the outgoing one flies apart, the incoming one rushes in.
pub fn zoom() -> Kind {
    kind("zoom", "Cross zoom", "trn.kind.zoom",
        MOTION,
        // both sides are computed by the same formula from their own role, and the mix identity
        // `mix(y, x, 1−t) == mix(x, y, t)` makes the inversion an exact identity
        true,
        vec![smooth("amount", 0.0, 4.0, 0.4, 0.01, ParamUnit::Ratio, Store::F64), Row::Center("cx"), Row::Center("cy")],
        include_str!("zoom.wgsl"),
        direct)
}

/// A rotation with a surge (not symmetric: the sign of the rotation changes on inversion).
pub fn spin() -> Kind {
    kind("spin", "Spin", "trn.kind.spin",
        MOTION,
        false,
        vec![
            smooth("turns", -4.0, 4.0, 1.0, 0.05, ParamUnit::Ratio, Store::F64),
            smooth("amount", 0.0, 4.0, 0.4, 0.01, ParamUnit::Ratio, Store::F64),
            Row::Center("cx"),
            Row::Center("cy"),
        ],
        include_str!("spin.wgsl"),
        direct)
}

// ---------------------------------------------------------------------------
// colour
// ---------------------------------------------------------------------------

/// Flash: the frame goes into the colour and comes back.
pub fn flash() -> Kind {
    kind("flash", "Flash", "trn.kind.flash",
        STYLE,
        true,
        vec![
            rgb("color_r", 255.0),
            rgb("color_g", 255.0),
            rgb("color_b", 255.0),
            smooth("strength", 0.0, 1.0, 1.0, 0.01, ParamUnit::Norm, Store::F64),
        ],
        include_str!("flash.wgsl"),
        direct)
}

/// Through the colour: the outgoing one fades into the colour, the incoming one emerges from it.
pub fn dip() -> Kind {
    kind("dip", "Dip to color", "trn.kind.dip",
        STYLE,
        true,
        vec![rgb("color_r", 0.0), rgb("color_g", 0.0), rgb("color_b", 0.0)],
        include_str!("dip.wgsl"),
        direct)
}

// ---------------------------------------------------------------------------
// patterns (R.51, item 400)
// ---------------------------------------------------------------------------

/// Blinds: the strips open in a fan, the angle gives a diagonal.
pub fn blinds() -> Kind {
    kind("blinds", "Blinds", "trn.kind.blinds",
        PATTERN,
        false,
        vec![count("count", 2.0, 64.0, 8.0), smooth("angle", -90.0, 90.0, 0.0, 1.0, ParamUnit::Deg, Store::F64), soft(0.1)],
        include_str!("blinds.wgsl"),
        |v| {
            // the angle rotates the axis of the stripes — the same scheme as wipe
            let mut p = [0.0f32; 8];
            let a = (v[1] as f32).to_radians();
            let (sn, cs) = (a.fsin(), a.fcos());
            p[0] = cs;
            p[1] = sn;
            p[2] = v[0] as f32;
            p[3] = v[2] as f32;
            p
        })
}

/// Checkers: the cells open in two opposing waves.
pub fn checker() -> Kind {
    kind("checker", "Checkerboard", "trn.kind.checker",
        PATTERN,
        false,
        vec![count("cols", 1.0, 64.0, 8.0), count("rows", 1.0, 64.0, 6.0), soft(0.1)],
        include_str!("checker.wgsl"),
        direct)
}

/// Random blocks: the cells flash up in a pseudo-random order.
pub fn blocks() -> Kind {
    kind("blocks", "Random blocks", "trn.kind.blocks",
        PATTERN,
        false,
        vec![count("cols", 2.0, 64.0, 16.0), count("rows", 2.0, 64.0, 9.0), soft(0.25)],
        include_str!("blocks.wgsl"),
        direct)
}

/// Spiral: an Archimedean front spirals around the centre.
pub fn spiral() -> Kind {
    kind("spiral", "Spiral", "trn.kind.spiral",
        PATTERN,
        false,
        vec![
            smooth("turns", 0.25, 6.0, 2.0, 0.05, ParamUnit::Ratio, Store::F64),
            Row::Center("cx"),
            Row::Center("cy"),
            soft(0.1),
        ],
        include_str!("spiral.wgsl"),
        |v| {
            let mut p = [0.0f32; 8];
            p[0] = v[0] as f32;
            p[1] = v[3] as f32;
            p[2] = v[1] as f32;
            p[3] = v[2] as f32;
            p
        })
}

// ---------------------------------------------------------------------------
// shifts (symbiosis: BOTH frames move)
// ---------------------------------------------------------------------------

/// Split: the halves of the outgoing one diverge, the incoming one opens.
pub fn split() -> Kind {
    kind("split", "Split", "trn.kind.split", MOTION, false, vec![Row::Flag("vertical")], include_str!("split.wgsl"), flag_pack)
}

/// Squeeze: the outgoing one is compressed toward the far edge, the incoming one unfolds.
pub fn squeeze() -> Kind {
    kind("squeeze", "Squeeze", "trn.kind.squeeze", MOTION, false, vec![Row::Dir], include_str!("squeeze.wgsl"), dir_pack)
}

/// Stretch: the incoming one stretches out from the central line.
pub fn stretch() -> Kind {
    kind("stretch", "Stretch", "trn.kind.stretch", MOTION, false, vec![Row::Flag("vertical")], include_str!("stretch.wgsl"), flag_pack)
}
