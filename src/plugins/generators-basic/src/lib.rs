//! The basic generators (plugin-api-stage1, stage 6, D13): backgrounds and shapes. Each kind is its
//! former `GeneratorKind` variant: the passport rows in the variant's field order with its stores
//! (the file text does not move — t973), the `gen:` path tokens of the former `format_gen_path` /
//! `apply_token` arms, and the frame of the former `render_at` arm, verbatim (t1048).

use kt_plugin_api::math::FrozenMath;
use std::sync::Arc;

use kt_plugin_api::generator::{
    GenDecl, PathTok, PathVal, Row, Tick, count, fill, kind, lowbias32, rgb, shader_frame, smooth, u8v,
};
use kt_plugin_api::{ParamUnit, Plugin};

/// The creation menu's groups.
const BG: &str = "genkind.group_bg";
const SHAPES: &str = "genkind.group_shapes";
const CHECK: &str = "genkind.group_check";

/// A kind's declarations: no address, one menu row of defaults, no value-named row.
const fn decl(
    alpha: bool,
    live: bool,
    blank: bool,
    path: &'static [PathTok],
    colors: &'static [(&'static str, &'static str)],
) -> GenDecl {
    GenDecl { alpha, live, blank, path, address: &[], menu: &[], names: None, colors }
}


/// An `rgb` triple of rows (`<stem>_r/_g/_b`) at a default colour.
fn rgb3(n: [&'static str; 3], d: [f64; 3]) -> Vec<Row> {
    vec![rgb(n[0], d[0]), rgb(n[1], d[1]), rgb(n[2], d[2])]
}

/// A fraction of the frame shown as a percentage.
fn pct(n: &'static str, lo: f64, hi: f64, d: f64) -> Row {
    smooth(n, lo, hi, d, 0.001, ParamUnit::Percent)
}

/// The shapes' colour WITH alpha (R.53, item 419).
fn rgba(d: [f64; 4]) -> Vec<Row> {
    vec![rgb("color_r", d[0]), rgb("color_g", d[1]), rgb("color_b", d[2]), rgb("color_a", d[3])]
}

// ---- the frames: the former `render_at` arms ---------------------------------------------------

fn solid(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    fill(w, h, [u8v(p[0]), u8v(p[1]), u8v(p[2]), 255])
}

fn checker(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let n = (w as usize) * (h as usize);
    let cols = p[0] as u32;
    // a cell is set by the COUNT of columns: the picture does not depend on
    // Quality (or on the decode step): the cells are square in width
    let cell = (w as f64 / cols.max(1) as f64).max(1.0);
    let a = [u8v(p[1]), u8v(p[2]), u8v(p[3]), 255];
    let b = [u8v(p[4]), u8v(p[5]), u8v(p[6]), 255];
    let mut buf = vec![0u8; n * 4];
    for y in 0..h as usize {
        let cy = (y as f64 / cell) as u64;
        for x in 0..w as usize {
            let cx = (x as f64 / cell) as u64;
            let o = (y * w as usize + x) * 4;
            buf[o..o + 4].copy_from_slice(if (cx + cy).is_multiple_of(2) { &a } else { &b });
        }
    }
    buf.into_boxed_slice()
}

fn gradient(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    // a projection of the normalised coordinates onto the direction of the angle,
    // stretched over the whole frame: at any angle both colours are visible
    let rad = p[0].to_radians();
    let (dx, dy) = (rad.fcos(), rad.fsin());
    let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)].map(|(x, y): (f64, f64)| x * dx + y * dy);
    let lo = corners.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = corners.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = (hi - lo).max(1e-9);
    let colours = [color(&p[1..4]), color(&p[4..7]), [dx as f32, dy as f32, lo as f32, span as f32], [0.0; 4]];
    shader_frame(include_str!("wgsl/gradient.wgsl"), w, h, [0, 0], colours)
}

fn corners(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let c = [color(&p[0..3]), color(&p[3..6]), color(&p[6..9]), color(&p[9..12])];
    shader_frame(include_str!("wgsl/corners.wgsl"), w, h, [0, 0], c)
}

fn noise(p: &[f64], pts: Tick, w: u32, h: u32) -> Box<[u8]> {
    // counter-based (the same lowbias32 as the Noise effect): grain = f(cell of the BASE grid
    // 1920×1080, pts of the MATERIAL): the picture is the same at any Quality/step, and a clip shift
    // does not change the grain (invariant 11 in the generator's edition).
    // The full brightness range: "grey field".
    let seed = lowbias32(pts.0 as u32 ^ 0x9e37_79b9).rotate_left(11);
    let values = [[p[0] as f32, 0.0, 0.0, 0.0], [0.0; 4], [0.0; 4], [0.0; 4]];
    shader_frame(include_str!("wgsl/noise.wgsl"), w, h, [seed, (p[1] >= 0.5) as u32], values)
}

fn shape_values(p: &[f64], a: [f64; 4], b: [f64; 2]) -> [[f32; 4]; 4] {
    let (r, g, bl, al) = shape_color(p);
    [[r as f32, g as f32, bl as f32, al as f32], a.map(|v| v as f32), [b[0] as f32, b[1] as f32, 0.0, 0.0], [0.0; 4]]
}

fn rect(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let (cx, cy, rw, rh, radius, feather) = (p[0], p[1], p[2], p[3], p[4], p[5]);
    let (fw, fh) = (w.max(1) as f64, h.max(1) as f64);
    let (hx, hy) = (rw * fw / 2.0, rh * fh / 2.0);
    let rad = (radius * fh).clamp(0.0, hx.min(hy));
    let v = shape_values(p, [cx * fw, cy * fh, hx, hy], [rad, (feather * fh).max(1.0 / 256.0)]);
    shader_frame(include_str!("wgsl/rect.wgsl"), w, h, [0, 0], v)
}

fn ellipse(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let (cx, cy, rw, rh, feather) = (p[0], p[1], p[2], p[3], p[4]);
    let (fw, fh) = (w.max(1) as f64, h.max(1) as f64);
    let (hx, hy) = ((rw * fw / 2.0).max(1.0 / 256.0), (rh * fh / 2.0).max(1.0 / 256.0));
    let v = shape_values(p, [cx * fw, cy * fh, hx, hy], [0.0, (feather * fh).max(1.0 / 256.0)]);
    shader_frame(include_str!("wgsl/ellipse.wgsl"), w, h, [0, 0], v)
}

fn line(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let (x1, y1, x2, y2, thickness, feather) = (p[0], p[1], p[2], p[3], p[4], p[5]);
    let (fw, fh) = (w.max(1) as f64, h.max(1) as f64);
    let half = (thickness * fh / 2.0).max(1.0 / 256.0);
    let v = shape_values(p, [x1 * fw, y1 * fh, x2 * fw, y2 * fh], [half, (feather * fh).max(1.0 / 256.0)]);
    shader_frame(include_str!("wgsl/line.wgsl"), w, h, [0, 0], v)
}

fn transparent(_: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    vec![0u8; (w as usize) * (h as usize) * 4].into_boxed_slice()
}

/// The shape's colour: the last four rows of the three shapes.
/// A colour of three rows, as the shaders read it.
fn color(p: &[f64]) -> [f32; 4] {
    [u8v(p[0]) as f32, u8v(p[1]) as f32, u8v(p[2]) as f32, 0.0]
}

fn shape_color(p: &[f64]) -> (u8, u8, u8, u8) {
    let k = p.len();
    (u8v(p[k - 4]), u8v(p[k - 3]), u8v(p[k - 2]), u8v(p[k - 1]))
}

// ---- the path tables: the former `format_gen_path` / `apply_token` arms ------------------------

const AB_COLORS: &[(&str, &str)] = &[("a", "genw.color_a"), ("b", "genw.color_b")];
const SHAPE_COLOR: &[(&str, &str)] = &[("color", "trn.color")];

/// The `pixel` texts of the family's shader kinds.
pub const SHADERS: [&str; 6] = [
    include_str!("wgsl/gradient.wgsl"),
    include_str!("wgsl/corners.wgsl"),
    include_str!("wgsl/noise.wgsl"),
    include_str!("wgsl/rect.wgsl"),
    include_str!("wgsl/ellipse.wgsl"),
    include_str!("wgsl/line.wgsl"),
];

/// The family's kinds, in `BUILTIN_IDS` order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    let gradient_rows = [vec![smooth("angle_deg", 0.0, 360.0, 90.0, 1.0, ParamUnit::Deg)], rgb3(["a_r", "a_g", "a_b"], [0.0; 3]), rgb3(["b_r", "b_g", "b_b"], [255.0; 3])].into_iter().flatten().collect::<Vec<_>>();
    let corner_rows = [
        rgb3(["tl_r", "tl_g", "tl_b"], [255.0, 0.0, 0.0]),
        rgb3(["tr_r", "tr_g", "tr_b"], [0.0, 255.0, 0.0]),
        rgb3(["bl_r", "bl_g", "bl_b"], [0.0, 0.0, 255.0]),
        rgb3(["br_r", "br_g", "br_b"], [255.0, 255.0, 255.0]),
    ]
    .into_iter().flatten().collect::<Vec<_>>();
    let checker_rows = [vec![count("cols", 1.0, 256.0, 8.0)], rgb3(["a_r", "a_g", "a_b"], [0.0; 3]), rgb3(["b_r", "b_g", "b_b"], [255.0; 3])].into_iter().flatten().collect::<Vec<_>>();
    let rect_rows = [
        vec![
            pct("cx", 0.0, 1.0, 0.5),
            pct("cy", 0.0, 1.0, 0.5),
            pct("w", 0.0, 2.0, 0.5),
            pct("h", 0.0, 2.0, 0.25),
            pct("radius", 0.0, 1.0, 0.0),
            pct("feather", 0.0, 0.5, 0.005),
        ],
        rgba([255.0; 4]),
    ]
    .into_iter().flatten().collect::<Vec<_>>();
    let ellipse_rows = [
        vec![
            pct("cx", 0.0, 1.0, 0.5),
            pct("cy", 0.0, 1.0, 0.5),
            pct("w", 0.0, 2.0, 0.5),
            pct("h", 0.0, 2.0, 0.25),
            pct("feather", 0.0, 0.5, 0.005),
        ],
        rgba([255.0; 4]),
    ]
    .into_iter().flatten().collect::<Vec<_>>();
    let line_rows = [
        vec![
            pct("x1", 0.0, 1.0, 0.2),
            pct("y1", 0.0, 1.0, 0.5),
            pct("x2", 0.0, 1.0, 0.8),
            pct("y2", 0.0, 1.0, 0.5),
            pct("thickness", 0.0, 0.5, 0.01),
            pct("feather", 0.0, 0.5, 0.005),
        ],
        rgba([255.0; 4]),
    ]
    .into_iter().flatten().collect::<Vec<_>>();
    vec![
        // a solid colour — the only generator used daily (F-TXT-003); the default is black (UI rule 4)
        Arc::new(kind(
            "solid",
            "genkind.solid",
            BG,
            rgb3(["color_r", "color_g", "color_b"], [0.0; 3]),
            decl(false, false, false, &[PathTok { key: "c", val: PathVal::Hex3("color") }], &[("color", "trn.color")]),
            solid,
        )),
        // a linear gradient between two colours (the angle in degrees)
        Arc::new(kind(
            "gradient",
            "genkind.gradient",
            BG,
            gradient_rows,
            decl(
                false,
                false,
                false,
                &[PathTok { key: "ang", val: PathVal::Num("angle_deg") }, PathTok { key: "a", val: PathVal::Hex3("a") }, PathTok { key: "b", val: PathVal::Hex3("b") }],
                AB_COLORS,
            ),
            gradient,
        )),
        // a four-colour gradient across the corners (bilinear)
        Arc::new(kind(
            "corners",
            "genkind.corners",
            BG,
            corner_rows,
            decl(
                false,
                false,
                false,
                &[
                    PathTok { key: "tl", val: PathVal::Hex3("tl") },
                    PathTok { key: "tr", val: PathVal::Hex3("tr") },
                    PathTok { key: "bl", val: PathVal::Hex3("bl") },
                    PathTok { key: "br", val: PathVal::Hex3("br") },
                ],
                &[
                    ("tl", "genw.corner_tl"),
                    ("tr", "genw.corner_tr"),
                    ("bl", "genw.corner_bl"),
                    ("br", "genw.corner_br"),
                ],
            ),
            corners,
        )),
        // a checkerboard: the cell is given by a COUNT of columns (resolution-independent)
        Arc::new(kind(
            "checker",
            "genkind.checker",
            CHECK,
            checker_rows,
            decl(
                false,
                false,
                false,
                &[PathTok { key: "n", val: PathVal::Int("cols", 1, 256) }, PathTok { key: "a", val: PathVal::Hex3("a") }, PathTok { key: "b", val: PathVal::Hex3("b") }],
                AB_COLORS,
            ),
            checker,
        )),
        // a noise field: a counter-based seed from (x, y, pts of the MATERIAL) — a clip shift does
        // not change the grain; the full brightness range (a "grey field" the Noise EFFECT cannot give)
        Arc::new(kind(
            "noise",
            "genkind.noise",
            BG,
            vec![smooth("amount", 0.0, 1.0, 1.0, 0.01, ParamUnit::Norm), Row::Flag("mono", true)],
            decl(false, true, false, &[PathTok { key: "amt", val: PathVal::Num("amount") }, PathTok { key: "mono", val: PathVal::Flag0("mono") }], &[]),
            noise,
        )),
        // a TRANSPARENT backing (R.53, item 420): a frame of zeros — a placeholder on the track and
        // a matte blank; empty by meaning, so the bin card writes its name
        Arc::new(kind("transparent", "genkind.transparent", BG, Vec::new(), decl(true, false, true, &[], &[]), transparent)),
        // a rectangle with rounding (item 419): sizes in fractions of ITS OWN axis, isotropic
        // lengths (rounding, feathering) in fractions of the HEIGHT
        Arc::new(kind(
            "rect",
            "genkind.rect",
            SHAPES,
            rect_rows,
            decl(
                true,
                false,
                false,
                &[
                    PathTok { key: "cx", val: PathVal::Unit("cx") },
                    PathTok { key: "cy", val: PathVal::Unit("cy") },
                    PathTok { key: "w", val: PathVal::Span("w") },
                    PathTok { key: "h", val: PathVal::Span("h") },
                    PathTok { key: "rad", val: PathVal::Unit("radius") },
                    PathTok { key: "fe", val: PathVal::Unit("feather") },
                    PathTok { key: "c", val: PathVal::Hex4("color") },
                ],
                SHAPE_COLOR,
            ),
            rect,
        )),
        // an ellipse (item 419): a kind of its own — no rounding, a merged kind would give a dead control
        Arc::new(kind(
            "ellipse",
            "genkind.ellipse",
            SHAPES,
            ellipse_rows,
            decl(
                true,
                false,
                false,
                &[
                    PathTok { key: "cx", val: PathVal::Unit("cx") },
                    PathTok { key: "cy", val: PathVal::Unit("cy") },
                    PathTok { key: "w", val: PathVal::Span("w") },
                    PathTok { key: "h", val: PathVal::Span("h") },
                    PathTok { key: "fe", val: PathVal::Unit("feather") },
                    PathTok { key: "c", val: PathVal::Hex4("color") },
                ],
                SHAPE_COLOR,
            ),
            ellipse,
        )),
        // a line segment (item 419): the ends, not a centre with dimensions
        Arc::new(kind(
            "line",
            "genkind.line",
            SHAPES,
            line_rows,
            decl(
                true,
                false,
                false,
                &[
                    PathTok { key: "x1", val: PathVal::Unit("x1") },
                    PathTok { key: "y1", val: PathVal::Unit("y1") },
                    PathTok { key: "x2", val: PathVal::Unit("x2") },
                    PathTok { key: "y2", val: PathVal::Unit("y2") },
                    PathTok { key: "th", val: PathVal::Unit("thickness") },
                    PathTok { key: "fe", val: PathVal::Unit("feather") },
                    PathTok { key: "c", val: PathVal::Hex4("color") },
                ],
                SHAPE_COLOR,
            ),
            line,
        )),
    ]
}
