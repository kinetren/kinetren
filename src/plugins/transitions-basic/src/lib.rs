//! The basic transition kinds as plugins (plugin-api-stage1, stage 5, D12): the wipes, the motions
//! and the patterns. Each kernel is the former `TransMix::of` arm (its `pack`) and `kernel()` arm
//! (its `pixel`) of kt-engine's `transitions.rs`, verbatim; its WGSL is the former `trans()` arm of
//! composite.wgsl. t1019 holds the pixels against the enum, t224 the GPU against the CPU, t973 the
//! file text.
//!
//! The passport lists the rows in the former variant's FIELD order (the file writes them in that
//! order); the number rows keep the former specs, so the window's grid is the same.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use kt_plugin_api::trans::{Canvas, Img, fracf, front_mask, hash_u32, hash01, mix4, rot_sample};
use kt_plugin_api::{
    Decl, Gpu, Manifest, Media, ParamKind, ParamSpec, ParamUnit, Plugin, RowDecl, Store, TransKernel, UiDecl,
};

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

/// The direction row's keys (the file holds the former `Dir` text).
pub const DIRS: &[&str] = &["left", "right", "up", "down"];
/// The iris shape row's keys (the former `IrisShape` text).
pub const SHAPES: &[&str] = &["circle", "diamond", "box"];

/// The unit vector of a direction row's index in frame coordinates (X right, Y down) — `Dir::vec`.
fn dir_vec(i: f64) -> (f32, f32) {
    match i as u32 {
        1 => (1.0, 0.0),
        2 => (0.0, -1.0),
        3 => (0.0, 1.0),
        _ => (-1.0, 0.0),
    }
}

fn flag(v: f64) -> f32 {
    if v > 0.5 { 1.0 } else { 0.0 }
}

type Pack = fn(&[f64]) -> [f32; 8];
type Pixel = fn(&Img<'_>, &Img<'_>, f32, f32, f32, &[f32; 8], &Canvas) -> [f32; 4];

/// One basic kind: its manifest and the two former arms.
pub struct Kind {
    m: Manifest,
    pack: Pack,
    pixel: Pixel,
    symmetric: bool,
}

impl Plugin for Kind {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn trans(&self) -> Option<&dyn TransKernel> {
        Some(self)
    }
}

impl TransKernel for Kind {
    fn pack(&self, v: &[f64]) -> [f32; 8] {
        (self.pack)(v)
    }
    fn pixel(&self, a: &Img<'_>, b: &Img<'_>, u: f32, v: f32, k: f32, p: &[f32; 8], cx: &Canvas, _ext: &[f32; 16]) -> [f32; 4] {
        (self.pixel)(a, b, u, v, k, p, cx)
    }
    fn symmetric(&self) -> bool {
        self.symmetric
    }
}

/// A row of the passport with its storage: a number row is always written, `center` rows are
/// omitted at 0.5 (the former `skip_serializing_if = "is_center"`).
enum R {
    Num(ParamSpec, Store),
    Center(&'static str),
    Dir,
    Shape,
    Flag(&'static str),
}

fn smooth(n: &'static str, lo: f64, hi: f64, d: f64, st: f64, u: ParamUnit, store: Store) -> R {
    R::Num(ParamSpec::num(n, lo, hi, d, st, u, ParamKind::Smooth), store)
}

fn soft(d: f64) -> R {
    smooth("softness", 0.0, 1.0, d, 0.01, ParamUnit::Norm, Store::F64)
}

fn rgb(n: &'static str, d: f64) -> R {
    smooth(n, 0.0, 255.0, d, 1.0, ParamUnit::Rgb8, Store::U8)
}

fn count(n: &'static str, lo: f64, d: f64) -> R {
    smooth(n, lo, 64.0, d, 1.0, ParamUnit::Count, Store::U32)
}

#[allow(clippy::too_many_arguments)]
fn kind(
    id: &'static str,
    name: &'static str,
    category: &'static str,
    symmetric: bool,
    rows: Vec<R>,
    wgsl: &'static str,
    pack: Pack,
    pixel: Pixel,
) -> Kind {
    let mut passport = Vec::new();
    let mut decls = Vec::new();
    for r in rows {
        let (spec, store, omit) = match r {
            R::Num(s, st) => (s, st, false),
            R::Center(n) => (ParamSpec::num(n, 0.0, 1.0, 0.5, 0.01, ParamUnit::Norm, ParamKind::Smooth), Store::F64, true),
            R::Dir => (ParamSpec::choice_row("dir", DIRS, 0), Store::Choice(DIRS), false),
            R::Shape => (ParamSpec::choice_row("shape", SHAPES, 0), Store::Choice(SHAPES), false),
            R::Flag(n) => (ParamSpec::flag_row(n, false), Store::Bool, false),
        };
        passport.push(spec);
        decls.push(RowDecl { store, omit_default: omit });
    }
    // the name's catalog key is `trn.kind.<id>` (blur dissolve's is `trn.kind.blur`)
    let name_key: &'static str = Box::leak(format!("trn.kind.{id}").into_boxed_str());
    Kind {
        m: Manifest {
            id: id.into(),
            media: Media::Transition,
            passport,
            rows: decls,
            decl: Decl { gpu: Gpu::Wgsl(wgsl), ..Decl::default() },
            ui: UiDecl { name, category_key: category, name_key, ..UiDecl::default() },
        },
        pack,
        pixel,
        symmetric,
    }
}

/// The values as they are (the kinds whose numbers map one to one onto the uniform).
fn direct(v: &[f64]) -> [f32; 8] {
    let mut p = [0.0f32; 8];
    for (o, x) in p.iter_mut().zip(v) {
        *o = *x as f32;
    }
    p
}

const WIPE: &str = "trn.cat.wipe";
const MOTION: &str = "trn.cat.motion";
const PATTERN: &str = "trn.cat.pattern";
const STYLE: &str = "trn.cat.style";

// ---------------------------------------------------------------------------
// wipes
// ---------------------------------------------------------------------------

/// The edge moves across the frame: a linear wipe, `angle` gives a diagonal.
pub fn wipe() -> Kind {
    kind(
        "wipe",
        "Wipe",
        WIPE,
        false,
        vec![R::Dir, smooth("angle", -180.0, 180.0, 0.0, 1.0, ParamUnit::Deg, Store::F64), soft(0.15)],
        include_str!("wipe.wgsl"),
        |v| {
            let mut p = [0.0f32; 8];
            let (dx, dy) = dir_vec(v[0]);
            let a = (v[1] as f32).to_radians();
            let (s, c) = (a.sin(), a.cos());
            // the angle turns the direction — the diagonal is obtained by it
            p[0] = dx * c - dy * s;
            p[1] = dx * s + dy * c;
            p[2] = v[2] as f32;
            p
        },
        |a, b, u, v, k, p, _| {
            // projection onto the direction, normalized by the square's span
            let (dx, dy) = (p[0], p[1]);
            let ext = (dx.abs() + dy.abs()).max(1e-6);
            let s = ((u - 0.5) * dx + (v - 0.5) * dy + ext * 0.5) / ext;
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[2]))
        },
    )
}

/// Opening from the centre: circle/diamond/rectangle; the centre (R.51) in fractions of the frame.
pub fn iris() -> Kind {
    kind(
        "iris",
        "Iris",
        WIPE,
        false,
        vec![R::Shape, R::Center("cx"), R::Center("cy"), soft(0.15)],
        include_str!("iris.wgsl"),
        |v| {
            let mut p = [0.0f32; 8];
            p[0] = v[0] as f32;
            p[1] = v[3] as f32;
            p[2] = v[1] as f32;
            p[3] = v[2] as f32;
            p
        },
        |a, b, u, v, k, p, cx| {
            // the centre (p2, p3) is arbitrary (R.51); rmax — the distance to the
            // FARTHEST corner of the frame in the same metric, so that the front
            // always covers the frame entirely.
            let aspect = cx.aspect;
            let (cx, cy) = ((u - p[2]) * aspect, v - p[3]);
            let metric = |dx: f32, dy: f32| match p[0] as i32 {
                1 => dx.abs() + dy.abs(),
                2 => dx.abs().max(dy.abs()),
                _ => (dx * dx + dy * dy).sqrt(),
            };
            let r = metric(cx, cy);
            let ex = (p[2].max(1.0 - p[2])) * aspect;
            let ey = p[3].max(1.0 - p[3]);
            let rmax = metric(ex, ey).max(1e-6);
            mix4(a.sample(u, v), b.sample(u, v), front_mask(r / rmax, k, p[1]))
        },
    )
}

/// A clock hand around the centre.
pub fn clock() -> Kind {
    kind(
        "clock",
        "Clock",
        WIPE,
        false,
        vec![R::Flag("reverse"), R::Center("cx"), R::Center("cy"), soft(0.15)],
        include_str!("clock.wgsl"),
        |v| {
            let mut p = [0.0f32; 8];
            p[0] = flag(v[0]);
            p[1] = v[3] as f32;
            p[2] = v[1] as f32;
            p[3] = v[2] as f32;
            p
        },
        |a, b, u, v, k, p, _| {
            let (cx, cy) = (u - p[2], v - p[3]);
            // 0 at the top, clockwise; reverse — counter-clockwise
            let mut ang = cx.atan2(-cy);
            if ang < 0.0 {
                ang += TAU;
            }
            if p[0] > 0.5 {
                ang = TAU - ang;
            }
            let s = ang / TAU;
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[1]))
        },
    )
}

/// Shutters: two leaves diverge from the centre.
pub fn barn() -> Kind {
    kind(
        "barn",
        "Barn doors",
        WIPE,
        false,
        vec![R::Flag("vertical"), soft(0.15)],
        include_str!("barn.wgsl"),
        |v| {
            let mut p = [0.0f32; 8];
            p[0] = flag(v[0]);
            p[1] = v[1] as f32;
            p
        },
        |a, b, u, v, k, p, _| {
            let s = if p[0] > 0.5 { (u - 0.5).abs() * 2.0 } else { (v - 0.5).abs() * 2.0 };
            // the leaves go OUT FROM the centre: the front is computed from it outward
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[1]))
        },
    )
}

/// A wipe by the BRIGHTNESS of the outgoing frame — a “reveal” by the picture.
pub fn luma() -> Kind {
    kind(
        "luma",
        "Luma wipe",
        WIPE,
        false,
        vec![R::Flag("invert"), soft(0.15)],
        include_str!("luma.wgsl"),
        |v| {
            let mut p = [0.0f32; 8];
            p[0] = flag(v[0]);
            p[1] = v[1] as f32;
            p
        },
        |a, b, u, v, k, p, _| {
            let c = a.sample(u, v);
            let mut l = (0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]) / 255.0;
            if p[0] > 0.5 {
                l = 1.0 - l;
            }
            mix4(c, b.sample(u, v), front_mask(l, k, p[1]))
        },
    )
}

// ---------------------------------------------------------------------------
// motion
// ---------------------------------------------------------------------------

fn dir_pack(v: &[f64]) -> [f32; 8] {
    let mut p = [0.0f32; 8];
    (p[0], p[1]) = dir_vec(v[0]);
    p
}

/// The incoming one pushes the outgoing one off the edge.
pub fn push() -> Kind {
    kind("push", "Push", MOTION, false, vec![R::Dir], include_str!("push.wgsl"), dir_pack, |a, b, u, v, k, p, _| {
        // the content MOVES toward dir, so the screen pixel
        // shows the source BEHIND itself at dir·k; B stands right
        // BEHIND the outgoing A — a full screen further ALONG the direction
        // of motion (`+dir`, item 396: with `−dir` the coordinate went past
        // [0,1) and the clamp drew a smeared edge instead of B).
        let (dx, dy) = (p[0], p[1]);
        let (au, av) = (u - dx * k, v - dy * k);
        if (0.0..1.0).contains(&au) && (0.0..1.0).contains(&av) {
            a.sample(au, av)
        } else {
            b.sample(au + dx, av + dy)
        }
    })
}

/// The incoming one slides over the top, the outgoing one stays.
pub fn slide() -> Kind {
    kind("slide", "Slide", MOTION, false, vec![R::Dir], include_str!("slide.wgsl"), dir_pack, |a, b, u, v, k, p, _| {
        let (bu, bv) = (u + p[0] * (1.0 - k), v + p[1] * (1.0 - k));
        if (0.0..1.0).contains(&bu) && (0.0..1.0).contains(&bv) {
            b.sample(bu, bv)
        } else {
            a.sample(u, v)
        }
    })
}

/// A surge with approach: the outgoing one flies apart, the incoming one rushes in.
pub fn zoom() -> Kind {
    kind(
        "zoom",
        "Cross zoom",
        MOTION,
        // both sides are computed by the same formula from their own role, and the mix identity
        // `mix(y, x, 1−t) == mix(x, y, t)` makes the inversion an exact identity
        true,
        vec![smooth("amount", 0.0, 4.0, 0.4, 0.01, ParamUnit::Ratio, Store::F64), R::Center("cx"), R::Center("cy")],
        include_str!("zoom.wgsl"),
        direct,
        |a, b, u, v, k, p, _| {
            let amt = p[0];
            let (ox, oy) = (p[1], p[2]);
            let za = 1.0 + amt * k; // the outgoing one spreads apart
            let zb = 1.0 + amt * (1.0 - k); // the incoming one rushes in
            let sa = a.sample((u - ox) / za + ox, (v - oy) / za + oy);
            let sb = b.sample((u - ox) / zb + ox, (v - oy) / zb + oy);
            mix4(sa, sb, k)
        },
    )
}

/// A rotation with a surge (not symmetric: the sign of the rotation changes on inversion).
pub fn spin() -> Kind {
    kind(
        "spin",
        "Spin",
        MOTION,
        false,
        vec![
            smooth("turns", -4.0, 4.0, 1.0, 0.05, ParamUnit::Ratio, Store::F64),
            smooth("amount", 0.0, 4.0, 0.4, 0.01, ParamUnit::Ratio, Store::F64),
            R::Center("cx"),
            R::Center("cy"),
        ],
        include_str!("spin.wgsl"),
        direct,
        |a, b, u, v, k, p, cx| {
            let (turns, amt) = (p[0], p[1]);
            let (ox, oy) = (p[2], p[3]);
            let sa = rot_sample(a, u, v, turns * k * TAU, 1.0 + amt * k, cx.aspect, ox, oy);
            let sb = rot_sample(b, u, v, -turns * (1.0 - k) * TAU, 1.0 + amt * (1.0 - k), cx.aspect, ox, oy);
            mix4(sa, sb, k)
        },
    )
}

// ---------------------------------------------------------------------------
// colour
// ---------------------------------------------------------------------------

/// Flash: the frame goes into the colour and comes back.
pub fn flash() -> Kind {
    kind(
        "flash",
        "Flash",
        STYLE,
        true,
        vec![
            rgb("color_r", 255.0),
            rgb("color_g", 255.0),
            rgb("color_b", 255.0),
            smooth("strength", 0.0, 1.0, 1.0, 0.01, ParamUnit::Norm, Store::F64),
        ],
        include_str!("flash.wgsl"),
        direct,
        |a, b, u, v, k, p, _| {
            let base = mix4(a.sample(u, v), b.sample(u, v), k);
            let w = (p[3] * (PI * k).sin()).clamp(0.0, 1.0);
            [
                base[0] + (p[0] - base[0]) * w,
                base[1] + (p[1] - base[1]) * w,
                base[2] + (p[2] - base[2]) * w,
                base[3],
            ]
        },
    )
}

/// Through the colour: the outgoing one fades into the colour, the incoming one emerges from it.
pub fn dip() -> Kind {
    kind(
        "dip",
        "Dip to color",
        STYLE,
        true,
        vec![rgb("color_r", 0.0), rgb("color_g", 0.0), rgb("color_b", 0.0)],
        include_str!("dip.wgsl"),
        direct,
        |a, b, u, v, k, p, _| {
            // two phases: A → colour, colour → B (classic dip-to-colour)
            let col = [p[0], p[1], p[2]];
            let (src, t) = if k < 0.5 { (a.sample(u, v), k * 2.0) } else { (b.sample(u, v), 2.0 - k * 2.0) };
            [
                src[0] + (col[0] - src[0]) * t,
                src[1] + (col[1] - src[1]) * t,
                src[2] + (col[2] - src[2]) * t,
                src[3],
            ]
        },
    )
}

// ---------------------------------------------------------------------------
// patterns (R.51, item 400)
// ---------------------------------------------------------------------------

/// Blinds: the strips open in a fan, the angle gives a diagonal.
pub fn blinds() -> Kind {
    kind(
        "blinds",
        "Blinds",
        PATTERN,
        false,
        vec![count("count", 2.0, 8.0), smooth("angle", -90.0, 90.0, 0.0, 1.0, ParamUnit::Deg, Store::F64), soft(0.1)],
        include_str!("blinds.wgsl"),
        |v| {
            // the angle rotates the axis of the stripes — the same scheme as wipe
            let mut p = [0.0f32; 8];
            let a = (v[1] as f32).to_radians();
            let (sn, cs) = (a.sin(), a.cos());
            p[0] = cs;
            p[1] = sn;
            p[2] = v[0] as f32;
            p[3] = v[2] as f32;
            p
        },
        |a, b, u, v, k, p, _| {
            // projection onto the rotated axis (as wipe), the fraction INSIDE the band:
            // all bands open simultaneously
            let (dx, dy) = (p[0], p[1]);
            let ext = (dx.abs() + dy.abs()).max(1e-6);
            let c = ((u - 0.5) * dx + (v - 0.5) * dy + ext * 0.5) / ext;
            let f = fracf(c * p[2].max(1.0));
            mix4(a.sample(u, v), b.sample(u, v), front_mask(f, k, p[3]))
        },
    )
}

/// Checkers: the cells open in two opposing waves.
pub fn checker() -> Kind {
    kind(
        "checker",
        "Checkerboard",
        PATTERN,
        false,
        vec![count("cols", 1.0, 8.0), count("rows", 1.0, 6.0), soft(0.1)],
        include_str!("checker.wgsl"),
        direct,
        |a, b, u, v, k, p, _| {
            // local wipe in a cell, even and odd ones — toward each other
            let (cols, rows) = (p[0].max(1.0), p[1].max(1.0));
            let (i, j) = ((u * cols).floor(), (v * rows).floor());
            let fx = fracf(u * cols);
            let s = if ((i + j) as i32) % 2 == 0 { fx } else { 1.0 - fx };
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[2]))
        },
    )
}

/// Random blocks: the cells flash up in a pseudo-random order.
pub fn blocks() -> Kind {
    kind(
        "blocks",
        "Random blocks",
        PATTERN,
        false,
        vec![count("cols", 2.0, 16.0), count("rows", 2.0, 9.0), soft(0.25)],
        include_str!("blocks.wgsl"),
        direct,
        |a, b, u, v, k, p, _| {
            // the order of cells — the hash of the cell: determinism without a seed, the same
            // trick as glitch. softness — a per-cell crossfade
            let (cols, rows) = (p[0].max(1.0), p[1].max(1.0));
            let (i, j) = ((u * cols).floor() as u32, (v * rows).floor() as u32);
            let s = hash01(hash_u32(i.wrapping_mul(1973)) ^ hash_u32(j.wrapping_mul(9277)));
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[2]))
        },
    )
}

/// Spiral: an Archimedean front spirals around the centre.
pub fn spiral() -> Kind {
    kind(
        "spiral",
        "Spiral",
        PATTERN,
        false,
        vec![
            smooth("turns", 0.25, 6.0, 2.0, 0.05, ParamUnit::Ratio, Store::F64),
            R::Center("cx"),
            R::Center("cy"),
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
        },
        |a, b, u, v, k, p, cx| {
            // Archimedean front: the angle (as clock) + turns along the radius
            let aspect = cx.aspect;
            let (cx, cy) = (u - p[2], v - p[3]);
            let mut ang = cx.atan2(-cy);
            if ang < 0.0 {
                ang += TAU;
            }
            let theta = ang / TAU;
            let ex = (p[2].max(1.0 - p[2])) * aspect;
            let ey = p[3].max(1.0 - p[3]);
            let rmax = (ex * ex + ey * ey).sqrt().max(1e-6);
            let r = ((cx * aspect) * (cx * aspect) + cy * cy).sqrt() / rmax;
            let turns = p[0].max(0.25);
            let sfrac = (theta + turns * r) / (1.0 + turns);
            mix4(a.sample(u, v), b.sample(u, v), front_mask(sfrac, k, p[1]))
        },
    )
}

// ---------------------------------------------------------------------------
// shifts (symbiosis: BOTH frames move)
// ---------------------------------------------------------------------------

fn flag_pack(v: &[f64]) -> [f32; 8] {
    let mut p = [0.0f32; 8];
    p[0] = flag(v[0]);
    p
}

/// Split: the halves of the outgoing one diverge, the incoming one opens.
pub fn split() -> Kind {
    kind("split", "Split", MOTION, false, vec![R::Flag("vertical")], include_str!("split.wgsl"), flag_pack, |a, b, u, v, k, p, _| {
        // symbiosis: the halves of A part, B opens in the gap
        let c = if p[0] > 0.5 { v - 0.5 } else { u - 0.5 };
        let h = k * 0.5;
        if c.abs() >= h {
            let off = h * c.signum();
            if p[0] > 0.5 { a.sample(u, v - off) } else { a.sample(u - off, v) }
        } else {
            b.sample(u, v)
        }
    })
}

/// Squeeze: the outgoing one is compressed toward the far edge, the incoming one unfolds.
pub fn squeeze() -> Kind {
    kind("squeeze", "Squeeze", MOTION, false, vec![R::Dir], include_str!("squeeze.wgsl"), dir_pack, |a, b, u, v, k, p, _| {
        // symbiosis: A compresses toward the far edge, B straightens out from the
        // near one; movement along dir
        let (dx, dy) = (p[0], p[1]);
        let c = if dx > 0.5 {
            u
        } else if dx < -0.5 {
            1.0 - u
        } else if dy > 0.5 {
            v
        } else {
            1.0 - v
        };
        let back = |c2: f32| -> (f32, f32) {
            if dx > 0.5 {
                (c2, v)
            } else if dx < -0.5 {
                (1.0 - c2, v)
            } else if dy > 0.5 {
                (u, c2)
            } else {
                (u, 1.0 - c2)
            }
        };
        if c < k {
            let (su, sv) = back(c / k.max(1e-4));
            b.sample(su, sv)
        } else {
            let (su, sv) = back((c - k) / (1.0 - k).max(1e-4));
            a.sample(su, sv)
        }
    })
}

/// Stretch: the incoming one stretches out from the central line.
pub fn stretch() -> Kind {
    kind("stretch", "Stretch", MOTION, false, vec![R::Flag("vertical")], include_str!("stretch.wgsl"), flag_pack, |a, b, u, v, k, p, _| {
        // symbiosis: B stretches out from the central line over A
        let c = if p[0] > 0.5 { v - 0.5 } else { u - 0.5 };
        let h = k * 0.5;
        if c.abs() < h {
            let c2 = c / k.max(1e-4) + 0.5;
            if p[0] > 0.5 { b.sample(u, c2) } else { b.sample(c2, v) }
        } else {
            a.sample(u, v)
        }
    })
}
