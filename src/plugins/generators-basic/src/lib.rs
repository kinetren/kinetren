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
    shader_frame("generator:gradient", GRADIENT, w, h, [0, 0], colours)
}

fn corners(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let c = [color(&p[0..3]), color(&p[3..6]), color(&p[6..9]), color(&p[9..12])];
    shader_frame("generator:corners", CORNERS, w, h, [0, 0], c)
}

fn noise(p: &[f64], pts: Tick, w: u32, h: u32) -> Box<[u8]> {
    // counter-based (the same lowbias32 as the Noise effect): grain = f(cell of the BASE grid
    // 1920×1080, pts of the MATERIAL): the picture is the same at any Quality/step, and a clip shift
    // does not change the grain (invariant 11 in the generator's edition).
    // The full brightness range: "grey field".
    let seed = lowbias32(pts.0 as u32 ^ 0x9e37_79b9).rotate_left(11);
    let values = [[p[0] as f32, 0.0, 0.0, 0.0], [0.0; 4], [0.0; 4], [0.0; 4]];
    shader_frame("generator:noise", NOISE, w, h, [seed, (p[1] >= 0.5) as u32], values)
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
    shader_frame("generator:rect", RECT, w, h, [0, 0], v)
}

fn ellipse(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let (cx, cy, rw, rh, feather) = (p[0], p[1], p[2], p[3], p[4]);
    let (fw, fh) = (w.max(1) as f64, h.max(1) as f64);
    let (hx, hy) = ((rw * fw / 2.0).max(1.0 / 256.0), (rh * fh / 2.0).max(1.0 / 256.0));
    let v = shape_values(p, [cx * fw, cy * fh, hx, hy], [0.0, (feather * fh).max(1.0 / 256.0)]);
    shader_frame("generator:ellipse", ELLIPSE, w, h, [0, 0], v)
}

fn line(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let (x1, y1, x2, y2, thickness, feather) = (p[0], p[1], p[2], p[3], p[4], p[5]);
    let (fw, fh) = (w.max(1) as f64, h.max(1) as f64);
    let half = (thickness * fh / 2.0).max(1.0 / 256.0);
    let v = shape_values(p, [x1 * fw, y1 * fh, x2 * fw, y2 * fh], [half, (feather * fh).max(1.0 / 256.0)]);
    shader_frame("generator:line", LINE, w, h, [0, 0], v)
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

// the frame functions, the kinds' `with_shader` (pipeline-prewarm D6) and `SHADERS` read the same text
const GRADIENT: &str = include_str!("wgsl/gradient.wgsl");
const CORNERS: &str = include_str!("wgsl/corners.wgsl");
const NOISE: &str = include_str!("wgsl/noise.wgsl");

// ---- the moving backgrounds (gen-backgrounds, `v26.10` G5) ----------------------------------------

const MESHGRAD: &str = include_str!("wgsl/meshgrad.wgsl");
const PLASMA: &str = include_str!("wgsl/plasma.wgsl");
const PARTICLES: &str = include_str!("wgsl/particles.wgsl");
const PATTERN: &str = include_str!("wgsl/pattern.wgsl");

/// The phase of a moving background: material seconds × speed (the preamble's one float).
fn phase(pts: Tick, speed: f64) -> f32 {
    (pts.0 as f64 / 90_000.0 * speed) as f32
}

fn rgbf(p: &[f64], i: usize) -> [f32; 4] {
    [p[i] as f32, p[i + 1] as f32, p[i + 2] as f32, 0.0]
}

fn aspect(w: u32, h: u32) -> f32 {
    w.max(1) as f32 / h.max(1) as f32
}

/// A moving mesh gradient (rows: color×3, color2×3, color3×3, speed, softness): three colour blobs on
/// slow orbits, blended by soft weights — the "modern" backdrop, not a linear ramp.
fn meshgrad(p: &[f64], pts: Tick, w: u32, h: u32) -> Box<[u8]> {
    let v = [rgbf(p, 0), rgbf(p, 3), rgbf(p, 6), [phase(pts, p[9]), p[10] as f32, aspect(w, h), 0.0]];
    shader_frame("generator:meshgrad", MESHGRAD, w, h, [0, 0], v)
}

/// Plasma (rows: color×3, color2×3, color3×3, speed, scale): travelling sine fields through a palette.
fn plasma(p: &[f64], pts: Tick, w: u32, h: u32) -> Box<[u8]> {
    let v = [rgbf(p, 0), rgbf(p, 3), rgbf(p, 6), [phase(pts, p[9]), p[10] as f32, aspect(w, h), 0.0]];
    shader_frame("generator:plasma", PLASMA, w, h, [0, 0], v)
}

/// Floating particles (rows: bg×3, color×3, strength, count, size, softness, speed, seed): soft bokeh
/// discs drifting up over a backdrop.
fn particles(p: &[f64], pts: Tick, w: u32, h: u32) -> Box<[u8]> {
    let mut c = rgbf(p, 3);
    c[3] = p[6] as f32;
    let v = [rgbf(p, 0), c, [phase(pts, p[10]), p[8] as f32, p[9] as f32, aspect(w, h)], [0.0; 4]];
    shader_frame("generator:particles", PARTICLES, w, h, [p[7] as u32, p[11] as u32], v)
}

/// Moving patterns (rows: pattern, color×3, color2×3, cells, angle_deg, ratio, speed, softness).
fn pattern(p: &[f64], pts: Tick, w: u32, h: u32) -> Box<[u8]> {
    let v = [
        rgbf(p, 1),
        rgbf(p, 4),
        [phase(pts, p[10]), p[7] as f32, p[8].to_radians() as f32, p[9] as f32],
        [aspect(w, h), p[11] as f32, 0.0, 0.0],
    ];
    shader_frame("generator:pattern", PATTERN, w, h, [p[0] as u32, 0], v)
}

/// The patterns, as the file writes them.
const PATTERNS: &[&str] = &["stripes", "dots", "grid", "waves"];

/// The eyedropper groups of the moving backgrounds.
const THREE_COLORS: &[(&str, &str)] = &[("color", "genp.color1"), ("color2", "genp.color2"), ("color3", "genp.color3")];
const PARTICLE_COLORS: &[(&str, &str)] = &[("bg", "genp.bg_color"), ("color", "genp.color")];
const TWO_COLORS: &[(&str, &str)] = &[("color", "genp.color1"), ("color2", "genp.color2")];
const RECT: &str = include_str!("wgsl/rect.wgsl");
const ELLIPSE: &str = include_str!("wgsl/ellipse.wgsl");
const LINE: &str = include_str!("wgsl/line.wgsl");

// ---- the shapes and plates (gen-shapes, `v26.10` G5) -----------------------------------------------

const POLYGON: &str = concat!(include_str!("wgsl/sdf.wgsl"), include_str!("wgsl/polygon.wgsl"));
const STAR: &str = concat!(include_str!("wgsl/sdf.wgsl"), include_str!("wgsl/star.wgsl"));
const ARROW: &str = concat!(include_str!("wgsl/sdf.wgsl"), include_str!("wgsl/arrow.wgsl"));
const FRAME: &str = concat!(include_str!("wgsl/sdf.wgsl"), include_str!("wgsl/frame.wgsl"));
const PLATE: &str = concat!(include_str!("wgsl/sdf.wgsl"), include_str!("wgsl/plate.wgsl"));
const PROGRESS: &str = concat!(include_str!("wgsl/sdf.wgsl"), include_str!("wgsl/progress.wgsl"));

/// The colour of `rows[i..i+4]` as the shaders read it (0..255, alpha 0..255).
fn rgba_at(p: &[f64], i: usize) -> [f32; 4] {
    [u8v(p[i]) as f32, u8v(p[i + 1]) as f32, u8v(p[i + 2]) as f32, u8v(p[i + 3]) as f32]
}

/// A feather in pixels: a fraction of the height, at least 1/256 px.
fn feather_px(f: f64, h: u32) -> f32 {
    (f * h.max(1) as f64).max(1.0 / 256.0) as f32
}

/// A regular polygon (rows: cx, cy, size, sides, angle_deg, feather, color×4).
fn polygon(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let (fw, fh) = (w.max(1) as f32, h.max(1) as f32);
    let v = [
        rgba_at(p, 6),
        [p[0] as f32 * fw, p[1] as f32 * fh, p[2] as f32 * fh, p[3] as f32],
        [p[4].to_radians() as f32, feather_px(p[5], h), 0.0, 0.0],
        [0.0; 4],
    ];
    shader_frame("generator:polygon", POLYGON, w, h, [0, 0], v)
}

/// A star (rows: cx, cy, size, points, inner, angle_deg, feather, color×4).
fn star(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let (fw, fh) = (w.max(1) as f32, h.max(1) as f32);
    let v = [
        rgba_at(p, 7),
        [p[0] as f32 * fw, p[1] as f32 * fh, p[2] as f32 * fh, p[3] as f32],
        [p[5].to_radians() as f32, feather_px(p[6], h), p[4] as f32, 0.0],
        [0.0; 4],
    ];
    shader_frame("generator:star", STAR, w, h, [0, 0], v)
}

/// An arrow (rows: x1, y1, x2, y2, thickness, head, feather, color×4).
fn arrow(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let (fw, fh) = (w.max(1) as f32, h.max(1) as f32);
    let head = p[5] as f32 * fh;
    let v = [
        rgba_at(p, 7),
        [p[0] as f32 * fw, p[1] as f32 * fh, p[2] as f32 * fw, p[3] as f32 * fh],
        [p[4] as f32 * fh / 2.0, head, head * 0.6, feather_px(p[6], h)],
        [0.0; 4],
    ];
    shader_frame("generator:arrow", ARROW, w, h, [0, 0], v)
}

/// A border frame (rows: inset, thickness, radius, feather, color×4).
fn border(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let (fw, fh) = (w.max(1) as f32, h.max(1) as f32);
    let inset = p[0] as f32 * fh;
    let half = [(fw / 2.0 - inset).max(1.0), (fh / 2.0 - inset).max(1.0)];
    let v = [
        rgba_at(p, 4),
        [fw / 2.0, fh / 2.0, half[0], half[1]],
        [(p[2] as f32 * fh).min(half[0].min(half[1])), (p[1] as f32 * fh).max(1.0), feather_px(p[3], h), 0.0],
        [0.0; 4],
    ];
    shader_frame("generator:frame", FRAME, w, h, [0, 0], v)
}

/// A lower-third plate (rows: left, top, w, h, radius, accent, reveal, feather, color×4, color2×4).
fn plate(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let (fw, fh) = (w.max(1) as f32, h.max(1) as f32);
    let (pw, ph) = (p[2] as f32 * fw, p[3] as f32 * fh);
    let v = [
        rgba_at(p, 8),
        [p[0] as f32 * fw, p[1] as f32 * fh, pw, ph],
        [(p[4] as f32 * fh).min(pw.min(ph) / 2.0), p[5] as f32 * fh, p[6] as f32, feather_px(p[7], h)],
        rgba_at(p, 12),
    ];
    shader_frame("generator:plate", PLATE, w, h, [0, 0], v)
}

/// A progress bar filling with the material time (rows: left, top, w, h, radius, fill_time, feather,
/// color×4 — the track, color2×4 — the fill).
fn progress(p: &[f64], pts: Tick, w: u32, h: u32) -> Box<[u8]> {
    let (fw, fh) = (w.max(1) as f32, h.max(1) as f32);
    let (pw, ph) = (p[2] as f32 * fw, p[3] as f32 * fh);
    let done = (pts.0.max(0) as f64 / 90_000.0 / p[5].max(0.01)).clamp(0.0, 1.0) as f32;
    let v = [
        rgba_at(p, 7),
        [p[0] as f32 * fw, p[1] as f32 * fh, pw, ph],
        [(p[4] as f32 * fh).min(pw.min(ph) / 2.0), done, feather_px(p[6], h), 0.0],
        rgba_at(p, 11),
    ];
    shader_frame("generator:progress", PROGRESS, w, h, [0, 0], v)
}

/// The colour rows of a stem with alpha.
fn rgba4(st: [&'static str; 4], d: [f64; 4]) -> Vec<Row> {
    vec![rgb(st[0], d[0]), rgb(st[1], d[1]), rgb(st[2], d[2]), rgb(st[3], d[3])]
}

const PLATE_COLORS: &[(&str, &str)] = &[("color", "genp.plate_color"), ("color2", "genp.accent_color")];
const PROGRESS_COLORS: &[(&str, &str)] = &[("color", "genp.track_color"), ("color2", "genp.fill_color")];

/// The `pixel` texts of the family's shader kinds.
pub const SHADERS: [&str; 6] = [
    GRADIENT,
    CORNERS,
    NOISE,
    RECT,
    ELLIPSE,
    LINE,
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
        )
        .with_shader(GRADIENT)),
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
        )
        .with_shader(CORNERS)),
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
        )
        .with_shader(NOISE)),
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
        )
        .with_shader(RECT)),
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
        )
        .with_shader(ELLIPSE)),
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
        )
        .with_shader(LINE)),
        // ---- gen-shapes (`v26.10` G5): shapes and plates --------------------------------------------
        // a regular polygon: sides, size (a fraction of the height), rotation
        Arc::new(kind(
            "polygon",
            "genkind.polygon",
            SHAPES,
            [
                vec![
                    pct("cx", 0.0, 1.0, 0.5),
                    pct("cy", 0.0, 1.0, 0.5),
                    pct("size", 0.01, 1.0, 0.25),
                    count("sides", 3.0, 12.0, 6.0),
                    smooth("angle_deg", -180.0, 180.0, 0.0, 1.0, ParamUnit::Deg),
                    pct("feather", 0.0, 0.05, 0.002),
                ],
                rgba([255.0, 255.0, 255.0, 255.0]),
            ]
            .into_iter()
            .flatten()
            .collect(),
            decl(
                true,
                false,
                false,
                &[
                    PathTok { key: "cx", val: PathVal::Unit("cx") },
                    PathTok { key: "cy", val: PathVal::Unit("cy") },
                    PathTok { key: "sz", val: PathVal::Unit("size") },
                    PathTok { key: "n", val: PathVal::Int("sides", 3, 12) },
                    PathTok { key: "an", val: PathVal::Num("angle_deg") },
                    PathTok { key: "fe", val: PathVal::Unit("feather") },
                    PathTok { key: "c", val: PathVal::Hex4("color") },
                ],
                SHAPE_COLOR,
            ),
            polygon,
        )
        .with_shader(POLYGON)),
        // a star: its points and how deep the valleys go (its own number: a kind of its own)
        Arc::new(kind(
            "star",
            "genkind.star",
            SHAPES,
            [
                vec![
                    pct("cx", 0.0, 1.0, 0.5),
                    pct("cy", 0.0, 1.0, 0.5),
                    pct("size", 0.01, 1.0, 0.25),
                    count("points", 3.0, 12.0, 5.0),
                    smooth("inner", 0.1, 0.95, 0.45, 0.01, ParamUnit::Norm),
                    smooth("angle_deg", -180.0, 180.0, 0.0, 1.0, ParamUnit::Deg),
                    pct("feather", 0.0, 0.05, 0.002),
                ],
                rgba([255.0, 210.0, 60.0, 255.0]),
            ]
            .into_iter()
            .flatten()
            .collect(),
            decl(
                true,
                false,
                false,
                &[
                    PathTok { key: "cx", val: PathVal::Unit("cx") },
                    PathTok { key: "cy", val: PathVal::Unit("cy") },
                    PathTok { key: "sz", val: PathVal::Unit("size") },
                    PathTok { key: "n", val: PathVal::Int("points", 3, 12) },
                    PathTok { key: "in", val: PathVal::Unit("inner") },
                    PathTok { key: "an", val: PathVal::Num("angle_deg") },
                    PathTok { key: "fe", val: PathVal::Unit("feather") },
                    PathTok { key: "c", val: PathVal::Hex4("color") },
                ],
                SHAPE_COLOR,
            ),
            star,
        )
        .with_shader(STAR)),
        // an arrow from (x1, y1) to (x2, y2) with its head
        Arc::new(kind(
            "arrow",
            "genkind.arrow",
            SHAPES,
            [
                vec![
                    pct("x1", 0.0, 1.0, 0.3),
                    pct("y1", 0.0, 1.0, 0.6),
                    pct("x2", 0.0, 1.0, 0.7),
                    pct("y2", 0.0, 1.0, 0.4),
                    pct("thickness", 0.002, 0.1, 0.02),
                    pct("head", 0.01, 0.3, 0.08),
                    pct("feather", 0.0, 0.05, 0.002),
                ],
                rgba([255.0, 255.0, 255.0, 255.0]),
            ]
            .into_iter()
            .flatten()
            .collect(),
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
                    PathTok { key: "hd", val: PathVal::Unit("head") },
                    PathTok { key: "fe", val: PathVal::Unit("feather") },
                    PathTok { key: "c", val: PathVal::Hex4("color") },
                ],
                SHAPE_COLOR,
            ),
            arrow,
        )
        .with_shader(ARROW)),
        // a border frame round the picture
        Arc::new(kind(
            "frame",
            "genkind.frame",
            SHAPES,
            [
                vec![
                    pct("inset", 0.0, 0.45, 0.04),
                    pct("thickness", 0.001, 0.2, 0.01),
                    pct("radius", 0.0, 0.5, 0.02),
                    pct("feather", 0.0, 0.05, 0.002),
                ],
                rgba([255.0, 255.0, 255.0, 255.0]),
            ]
            .into_iter()
            .flatten()
            .collect(),
            decl(
                true,
                false,
                false,
                &[
                    PathTok { key: "in", val: PathVal::Unit("inset") },
                    PathTok { key: "th", val: PathVal::Unit("thickness") },
                    PathTok { key: "rad", val: PathVal::Unit("radius") },
                    PathTok { key: "fe", val: PathVal::Unit("feather") },
                    PathTok { key: "c", val: PathVal::Hex4("color") },
                ],
                SHAPE_COLOR,
            ),
            border,
        )
        .with_shader(FRAME)),
        // a lower-third plate: a bar with an accent stripe, its reveal a number a curve can animate
        Arc::new(kind(
            "plate",
            "genkind.plate",
            SHAPES,
            [
                vec![
                    pct("left", 0.0, 1.0, 0.06),
                    pct("top", 0.0, 1.0, 0.76),
                    pct("w", 0.05, 1.0, 0.5),
                    pct("h", 0.02, 0.5, 0.1),
                    pct("radius", 0.0, 0.1, 0.008),
                    pct("accent", 0.0, 0.1, 0.012),
                    smooth("reveal", 0.0, 1.0, 1.0, 0.01, ParamUnit::Norm),
                    pct("feather", 0.0, 0.05, 0.001),
                ],
                rgba4(["color_r", "color_g", "color_b", "color_a"], [20.0, 24.0, 32.0, 220.0]),
                rgba4(["color2_r", "color2_g", "color2_b", "color2_a"], [255.0, 170.0, 40.0, 255.0]),
            ]
            .into_iter()
            .flatten()
            .collect(),
            decl(
                true,
                false,
                false,
                &[
                    PathTok { key: "l", val: PathVal::Unit("left") },
                    PathTok { key: "t", val: PathVal::Unit("top") },
                    PathTok { key: "w", val: PathVal::Unit("w") },
                    PathTok { key: "h", val: PathVal::Unit("h") },
                    PathTok { key: "rad", val: PathVal::Unit("radius") },
                    PathTok { key: "ac", val: PathVal::Unit("accent") },
                    PathTok { key: "rv", val: PathVal::Unit("reveal") },
                    PathTok { key: "fe", val: PathVal::Unit("feather") },
                    PathTok { key: "c", val: PathVal::Hex4("color") },
                    PathTok { key: "c2", val: PathVal::Hex4("color2") },
                ],
                PLATE_COLORS,
            ),
            plate,
        )
        .with_shader(PLATE)),
        // a progress bar that fills with the material time over its fill time
        Arc::new(kind(
            "progress",
            "genkind.progress",
            SHAPES,
            [
                vec![
                    pct("left", 0.0, 1.0, 0.1),
                    pct("top", 0.0, 1.0, 0.9),
                    pct("w", 0.05, 1.0, 0.8),
                    pct("h", 0.002, 0.2, 0.012),
                    pct("radius", 0.0, 0.1, 0.006),
                    smooth("fill_time", 0.5, 3600.0, 10.0, 0.5, ParamUnit::Sec),
                    pct("feather", 0.0, 0.05, 0.001),
                ],
                rgba4(["color_r", "color_g", "color_b", "color_a"], [255.0, 255.0, 255.0, 60.0]),
                rgba4(["color2_r", "color2_g", "color2_b", "color2_a"], [255.0, 255.0, 255.0, 235.0]),
            ]
            .into_iter()
            .flatten()
            .collect(),
            decl(
                true,
                true,
                false,
                &[
                    PathTok { key: "l", val: PathVal::Unit("left") },
                    PathTok { key: "t", val: PathVal::Unit("top") },
                    PathTok { key: "w", val: PathVal::Unit("w") },
                    PathTok { key: "h", val: PathVal::Unit("h") },
                    PathTok { key: "rad", val: PathVal::Unit("radius") },
                    PathTok { key: "ft", val: PathVal::Num("fill_time") },
                    PathTok { key: "fe", val: PathVal::Unit("feather") },
                    PathTok { key: "c", val: PathVal::Hex4("color") },
                    PathTok { key: "c2", val: PathVal::Hex4("color2") },
                ],
                PROGRESS_COLORS,
            ),
            progress,
        )
        .with_shader(PROGRESS)),
        // ---- gen-backgrounds (`v26.10` G5): the moving backgrounds -------------------------------
        // a moving mesh gradient: three colour blobs on slow orbits
        Arc::new(kind(
            "meshgrad",
            "genkind.meshgrad",
            BG,
            [
                rgb3(["color_r", "color_g", "color_b"], [255.0, 94.0, 98.0]),
                rgb3(["color2_r", "color2_g", "color2_b"], [255.0, 195.0, 113.0]),
                rgb3(["color3_r", "color3_g", "color3_b"], [104.0, 58.0, 183.0]),
                vec![smooth("speed", 0.0, 2.0, 0.08, 0.01, ParamUnit::Ratio), smooth("softness", 0.1, 1.0, 0.45, 0.01, ParamUnit::Norm)],
            ]
            .into_iter()
            .flatten()
            .collect(),
            decl(
                false,
                true,
                false,
                &[
                    PathTok { key: "c", val: PathVal::Hex3("color") },
                    PathTok { key: "c2", val: PathVal::Hex3("color2") },
                    PathTok { key: "c3", val: PathVal::Hex3("color3") },
                    PathTok { key: "sp", val: PathVal::Num("speed") },
                    PathTok { key: "so", val: PathVal::Num("softness") },
                ],
                THREE_COLORS,
            ),
            meshgrad,
        )
        .with_shader(MESHGRAD)),
        // plasma through a palette of three colours
        Arc::new(kind(
            "plasma",
            "genkind.plasma",
            BG,
            [
                rgb3(["color_r", "color_g", "color_b"], [18.0, 24.0, 60.0]),
                rgb3(["color2_r", "color2_g", "color2_b"], [0.0, 170.0, 200.0]),
                rgb3(["color3_r", "color3_g", "color3_b"], [240.0, 90.0, 160.0]),
                vec![smooth("speed", 0.0, 2.0, 0.08, 0.01, ParamUnit::Ratio), smooth("scale", 1.0, 30.0, 8.0, 0.1, ParamUnit::Ratio)],
            ]
            .into_iter()
            .flatten()
            .collect(),
            decl(
                false,
                true,
                false,
                &[
                    PathTok { key: "c", val: PathVal::Hex3("color") },
                    PathTok { key: "c2", val: PathVal::Hex3("color2") },
                    PathTok { key: "c3", val: PathVal::Hex3("color3") },
                    PathTok { key: "sp", val: PathVal::Num("speed") },
                    PathTok { key: "sc", val: PathVal::Num("scale") },
                ],
                THREE_COLORS,
            ),
            plasma,
        )
        .with_shader(PLASMA)),
        // floating particles: soft bokeh discs drifting up over a backdrop
        Arc::new(kind(
            "particles",
            "genkind.particles",
            BG,
            [
                rgb3(["bg_r", "bg_g", "bg_b"], [10.0, 12.0, 24.0]),
                rgb3(["color_r", "color_g", "color_b"], [255.0, 214.0, 150.0]),
                vec![
                    smooth("strength", 0.0, 1.0, 0.8, 0.01, ParamUnit::Norm),
                    count("count", 4.0, 200.0, 60.0),
                    smooth("size", 0.005, 0.1, 0.02, 0.001, ParamUnit::Percent),
                    smooth("softness", 0.0, 1.0, 0.5, 0.01, ParamUnit::Norm),
                    smooth("speed", 0.0, 1.0, 0.05, 0.005, ParamUnit::Ratio),
                    count("seed", 0.0, 9999.0, 1.0),
                ],
            ]
            .into_iter()
            .flatten()
            .collect(),
            decl(
                false,
                true,
                false,
                &[
                    PathTok { key: "bg", val: PathVal::Hex3("bg") },
                    PathTok { key: "c", val: PathVal::Hex3("color") },
                    PathTok { key: "st", val: PathVal::Num("strength") },
                    PathTok { key: "n", val: PathVal::Int("count", 4, 200) },
                    PathTok { key: "sz", val: PathVal::Num("size") },
                    PathTok { key: "so", val: PathVal::Num("softness") },
                    PathTok { key: "sp", val: PathVal::Num("speed") },
                    PathTok { key: "seed", val: PathVal::Int("seed", 0, 9999) },
                ],
                PARTICLE_COLORS,
            ),
            particles,
        )
        .with_shader(PARTICLES)),
        // moving patterns: stripes, dots, a grid or waves, at an angle, drifting
        Arc::new(kind(
            "pattern",
            "genkind.pattern",
            BG,
            [
                vec![Row::Choice("pattern", PATTERNS)],
                rgb3(["color_r", "color_g", "color_b"], [240.0, 240.0, 240.0]),
                rgb3(["color2_r", "color2_g", "color2_b"], [30.0, 30.0, 40.0]),
                vec![
                    count("cells", 2.0, 80.0, 12.0),
                    smooth("angle_deg", -180.0, 180.0, 30.0, 1.0, ParamUnit::Deg),
                    smooth("ratio", 0.05, 0.95, 0.5, 0.01, ParamUnit::Norm),
                    smooth("speed", -2.0, 2.0, 0.1, 0.01, ParamUnit::Ratio),
                    smooth("softness", 0.0, 0.5, 0.04, 0.005, ParamUnit::Norm),
                ],
            ]
            .into_iter()
            .flatten()
            .collect(),
            decl(
                false,
                true,
                false,
                &[
                    PathTok { key: "pt", val: PathVal::Choice("pattern", PATTERNS) },
                    PathTok { key: "c", val: PathVal::Hex3("color") },
                    PathTok { key: "c2", val: PathVal::Hex3("color2") },
                    PathTok { key: "cl", val: PathVal::Int("cells", 2, 80) },
                    PathTok { key: "an", val: PathVal::Num("angle_deg") },
                    PathTok { key: "ra", val: PathVal::Num("ratio") },
                    PathTok { key: "sp", val: PathVal::Num("speed") },
                    PathTok { key: "so", val: PathVal::Num("softness") },
                ],
                TWO_COLORS,
            ),
            pattern,
        )
        .with_shader(PATTERN)),
    ]
}
