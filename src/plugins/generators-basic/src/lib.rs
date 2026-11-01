//! The basic generators (plugin-api-stage1, stage 6, D13): backgrounds and shapes. Each kind is its
//! former `GeneratorKind` variant: the passport rows in the variant's field order with its stores
//! (the file text does not move — t973), the `gen:` path tokens of the former `format_gen_path` /
//! `apply_token` arms, and the frame of the former `render_at` arm, verbatim (t1048).

use kt_plugin_api::math::FrozenMath;
use std::sync::Arc;

use kt_plugin_api::generator::{
    GenDecl, PathTok, PathVal, Row, Tick, count, coverage, fill, isqrt, kind, lowbias32, rgb, smooth, u8v,
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

fn gradient(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let n = (w as usize) * (h as usize);
    let angle_deg = p[0];
    let (a_r, a_g, a_b, b_r, b_g, b_b) = (u8v(p[1]), u8v(p[2]), u8v(p[3]), u8v(p[4]), u8v(p[5]), u8v(p[6]));
    // a projection of the normalised coordinates onto the direction of the angle,
    // stretched over the whole frame: at any angle both colours are visible
    let rad = angle_deg.to_radians();
    let (dx, dy) = (rad.fcos(), rad.fsin());
    let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)].map(|(x, y): (f64, f64)| x * dx + y * dy);
    let lo = corners.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = corners.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = (hi - lo).max(1e-9);
    let mut buf = vec![0u8; n * 4];
    for y in 0..h as usize {
        let ny = (y as f64 + 0.5) / h as f64;
        for x in 0..w as usize {
            let nx = (x as f64 + 0.5) / w as f64;
            let t = ((nx * dx + ny * dy - lo) / span).clamp(0.0, 1.0);
            let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t).round().clamp(0.0, 255.0) as u8;
            let o = (y * w as usize + x) * 4;
            buf[o] = mix(a_r, b_r);
            buf[o + 1] = mix(a_g, b_g);
            buf[o + 2] = mix(a_b, b_b);
            buf[o + 3] = 255;
        }
    }
    buf.into_boxed_slice()
}

fn corners(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let n = (w as usize) * (h as usize);
    let c: Vec<u8> = p.iter().map(|v| u8v(*v)).collect();
    let (tl, tr, bl, br) = ([c[0], c[1], c[2]], [c[3], c[4], c[5]], [c[6], c[7], c[8]], [c[9], c[10], c[11]]);
    let mut buf = vec![0u8; n * 4];
    let bilerp = |tl: u8, tr: u8, bl: u8, br: u8, fx: f64, fy: f64| {
        let top = tl as f64 + (tr as f64 - tl as f64) * fx;
        let bot = bl as f64 + (br as f64 - bl as f64) * fx;
        (top + (bot - top) * fy).round().clamp(0.0, 255.0) as u8
    };
    for y in 0..h as usize {
        let fy = (y as f64 + 0.5) / h as f64;
        for x in 0..w as usize {
            let fx = (x as f64 + 0.5) / w as f64;
            let o = (y * w as usize + x) * 4;
            for i in 0..3 {
                buf[o + i] = bilerp(tl[i], tr[i], bl[i], br[i], fx, fy);
            }
            buf[o + 3] = 255;
        }
    }
    buf.into_boxed_slice()
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

fn noise(p: &[f64], pts: Tick, w: u32, h: u32) -> Box<[u8]> {
    let n = (w as usize) * (h as usize);
    let (amount, mono) = (p[0], p[1] >= 0.5);
    // counter-based (the same lowbias32 as the Noise effect in
    // kt-fx): grain = f(cell of the BASE grid 1920×1080, pts of the MATERIAL)
    // — the picture is the same at any Quality/step, and a clip shift does not change
    // the grain (invariant 11 in the generator’s edition).
    // The full brightness range: “grey field”.
    let frame_seed = lowbias32(pts.0 as u32 ^ 0x9e37_79b9);
    let mut buf = vec![0u8; n * 4];
    for y in 0..h as usize {
        let ny = (y as u64 * 1080 / h.max(1) as u64) as u32;
        let row_seed = lowbias32(ny ^ frame_seed.rotate_left(11));
        for x in 0..w as usize {
            let nx = (x as u64 * 1920 / w.max(1) as u64) as u32;
            let s = lowbias32(nx ^ row_seed);
            let mixc = |v: u8| (128.0 + (v as f64 - 128.0) * amount).round().clamp(0.0, 255.0) as u8;
            let o = (y * w as usize + x) * 4;
            if mono {
                let v = mixc(s as u8);
                buf[o] = v;
                buf[o + 1] = v;
                buf[o + 2] = v;
            } else {
                buf[o] = mixc(s as u8);
                buf[o + 1] = mixc((s >> 8) as u8);
                buf[o + 2] = mixc((s >> 16) as u8);
            }
            buf[o + 3] = 255;
        }
    }
    buf.into_boxed_slice()
}

fn transparent(_: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    vec![0u8; (w as usize) * (h as usize) * 4].into_boxed_slice()
}

/// The shape's colour: the last four rows of the three shapes.
fn shape_color(p: &[f64]) -> (u8, u8, u8, u8) {
    let k = p.len();
    (u8v(p[k - 4]), u8v(p[k - 3]), u8v(p[k - 2]), u8v(p[k - 1]))
}

fn rect(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let n = (w as usize) * (h as usize);
    let (cx, cy, rw, rh, radius, feather) = (p[0], p[1], p[2], p[3], p[4], p[5]);
    let (r, g, b, a) = shape_color(p);
    let (fw, fh) = (w.max(1) as i64, h.max(1) as i64);
    // everything is in whole 1/256 px: float lives ONLY in the preamble
    // of the frame (the kt-media/CLAUDE.md rule), the pixel loop is integer
    let q = |v: f64| (v * 256.0).round() as i64;
    let (ccx, ccy) = (q(cx * fw as f64), q(cy * fh as f64));
    let (hx, hy) = (q(rw * fw as f64 / 2.0), q(rh * fh as f64 / 2.0));
    let rad = q(radius * fh as f64).clamp(0, hx.min(hy));
    let fe = q(feather * fh as f64).max(1);
    let mut buf = vec![0u8; n * 4];
    for y in 0..fh {
        let py = (y * 256 + 128) - ccy;
        for x in 0..fw {
            let px = (x * 256 + 128) - ccx;
            // signed distance to a rounded rectangle
            let dx = px.abs() - (hx - rad);
            let dy = py.abs() - (hy - rad);
            let d = if dx > 0 && dy > 0 { isqrt(dx * dx + dy * dy) - rad } else { dx.max(dy) - rad };
            let cov = coverage(d, fe);
            let o = ((y * fw + x) * 4) as usize;
            buf[o] = r;
            buf[o + 1] = g;
            buf[o + 2] = b;
            buf[o + 3] = ((a as i64 * cov) / 255) as u8;
        }
    }
    buf.into_boxed_slice()
}

fn ellipse(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let n = (w as usize) * (h as usize);
    let (cx, cy, rw, rh, feather) = (p[0], p[1], p[2], p[3], p[4]);
    let (r, g, b, a) = shape_color(p);
    let (fw, fh) = (w.max(1) as i64, h.max(1) as i64);
    let q = |v: f64| (v * 256.0).round() as i64;
    let (ccx, ccy) = (q(cx * fw as f64), q(cy * fh as f64));
    let (hx, hy) = (q(rw * fw as f64 / 2.0).max(1), q(rh * fh as f64 / 2.0).max(1));
    let fe = q(feather * fh as f64).max(1);
    let mut buf = vec![0u8; n * 4];
    for y in 0..fh {
        let py = (y * 256 + 128) - ccy;
        for x in 0..fw {
            let px = (x * 256 + 128) - ccx;
            // normalised distance ×min(hx,hy): an approximation of the
            // signed distance, exact on the circle
            let m = hx.min(hy);
            let ex = px * m / hx;
            let ey = py * m / hy;
            let d = isqrt(ex * ex + ey * ey) - m;
            let cov = coverage(d, fe);
            let o = ((y * fw + x) * 4) as usize;
            buf[o] = r;
            buf[o + 1] = g;
            buf[o + 2] = b;
            buf[o + 3] = ((a as i64 * cov) / 255) as u8;
        }
    }
    buf.into_boxed_slice()
}

fn line(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let n = (w as usize) * (h as usize);
    let (x1, y1, x2, y2, thickness, feather) = (p[0], p[1], p[2], p[3], p[4], p[5]);
    let (r, g, b, a) = shape_color(p);
    let (fw, fh) = (w.max(1) as i64, h.max(1) as i64);
    let q = |v: f64| (v * 256.0).round() as i64;
    let (ax, ay) = (q(x1 * fw as f64), q(y1 * fh as f64));
    let (bx, by) = (q(x2 * fw as f64), q(y2 * fh as f64));
    let half = q(thickness * fh as f64 / 2.0).max(1);
    let fe = q(feather * fh as f64).max(1);
    let (vx, vy) = (bx - ax, by - ay);
    let len2 = vx * vx + vy * vy;
    let mut buf = vec![0u8; n * 4];
    for y in 0..fh {
        let py = (y * 256 + 128) - ay;
        for x in 0..fw {
            let px = (x * 256 + 128) - ax;
            // projection onto a segment with a clamp: a capsule
            let t = if len2 == 0 { 0 } else { ((px * vx + py * vy) * 256 / len2).clamp(0, 256) };
            let qx = px - vx * t / 256;
            let qy = py - vy * t / 256;
            let d = isqrt(qx * qx + qy * qy) - half;
            let cov = coverage(d, fe);
            let o = ((y * fw + x) * 4) as usize;
            buf[o] = r;
            buf[o + 1] = g;
            buf[o + 2] = b;
            buf[o + 3] = ((a as i64 * cov) / 255) as u8;
        }
    }
    buf.into_boxed_slice()
}

// ---- the path tables: the former `format_gen_path` / `apply_token` arms ------------------------

const AB_COLORS: &[(&str, &str)] = &[("a", "genw.color_a"), ("b", "genw.color_b")];
const SHAPE_COLOR: &[(&str, &str)] = &[("color", "trn.color")];

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
