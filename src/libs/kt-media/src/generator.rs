//! Media generator (R.52, F-TXT-003): the third kind of source without a file:
//! after `SyntheticSource` (tests) and `PlaceholderSource` (a missing
//! file). It draws the picture itself.
//!
//! **Values and their curves arrive as STATE (`GenState`, R.59 stage
//! 2), not as a path string.** They are a property of the EVENT, so the source
//! is raised by the event’s address (`GEN_TAG | id`), and an edit of the values
//! reaches a live source by a reconfiguration (`retune_gen`), without
//! recreating the stream. The curves are resolved BY THE SOURCE itself, by its own
//! `pts`: the order holds `(address, tick, quality)`, and the frame MUST be a
//! function of this triple, otherwise the cache key stops determining the frame.
//!
//! Contracts on which everything rests:
//! - the frame is EXACTLY the requested Quality: the composite silently drops a layer
//!   of a foreign size (kt-engine/video.rs, `fits`);
//! - the frame is a deterministic function (parameters, pts, Quality): the engine’s
//!   cache keys and the GPU upload will not survive anything else (invariant 11);
//! - “no sound” = `has_audio: false` AND `Ok(AudioBurst::silence)`:
//!   an `Err` piles up in the worker’s negative cache as false underruns.

use std::borrow::Cow;

use crate::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_model::{GenState, GeneratorKind};
use kt_time::Tick;

/// “Detail reserve” of a vector source: the generator draws sharply at any size,
/// and its native size is the permission for a decode step under a zooming
/// FX chain (`clip_decode_scale`: min(chain request, native/Quality,
/// buffer cap)). The number equals `kt_fx::plan::MAX_INTER_DIM`: above it
/// there are never intermediate buffers anyway; kt-media does not depend on kt-fx,
/// so the constant is its own, with this linking comment.
/// In the “Max (by sources)” render menu the pseudo-paths do not take part.
pub const GEN_NATIVE: u32 = 4096;

/// The frame size for `quality = None` (“the full resolution of the source”:
/// a generator has none of it). The product always sends `Some`; the branch is
/// needed only so that the trait contract has no hole.
const DEFAULT_Q: Quality = Quality { width: 1920, height: 1080 };

/// Generator source: the frame is computed from `GenState`.
pub struct GeneratorSource {
    state: GenState,
    info: MediaInfo,
    /// A frame of the kinds that do not depend on time is built once for
    /// the requested quality (the pattern of `PlaceholderSource`).
    cache: Option<(Quality, Box<[u8]>)>,
}

impl GeneratorSource {
    pub fn new(kind: GeneratorKind) -> GeneratorSource {
        GeneratorSource::from_state(GenState::new(kind))
    }

    /// A source from the state of an event (R.59, stage 2).
    pub fn from_state(state: GenState) -> GeneratorSource {
        let info = Self::info_for(&state.kind);
        GeneratorSource { state, info, cache: None }
    }

    /// A kind WITH VALUES at the moment `pts` of the material. Without curves,
    /// it is a borrow: a non-animated generator pays no allocation.
    pub fn kind_at(&self, pts: Tick) -> Cow<'_, GeneratorKind> {
        self.state.kind_at(pts)
    }

    /// The passport of the source for a kind: the ONLY place where it is computed
    /// (R.53, item 410). The factory also decides by it whether a
    /// live source can be reconfigured: `retune` MUST not change `info()`.
    pub fn info_for(kind: &GeneratorKind) -> MediaInfo {
        // the only sounding kind: SMPTE with the tone enabled
        let has_audio = matches!(
            kind,
            GeneratorKind::SmpteBars { tone: true, .. } | GeneratorKind::Leader { beep: true, .. }
        );
        MediaInfo {
            // “no duration”: when dropped, the UI substitutes its own default
            // (5 s), the trim is not limited, and there is no EOF.
            duration: Tick(0),
            has_video: true,
            // The source gives silence anyway (see the header), but the
            // audio CLIP is created by place_media_at only on this flag.
            has_audio,
            // it has no grid of its own frames: the engine schedules on the PROJECT’s grid
            fps: None,
            width: GEN_NATIVE,
            height: GEN_NATIVE,
            sar: (1, 1),
            still: false,
        }
    }

    pub fn kind(&self) -> &GeneratorKind {
        &self.state.kind
    }

    pub fn state(&self) -> &GenState {
        &self.state
    }

    /// The frame buffer FOR OUTPUT. Live kinds (noise, timecode) are drawn at once
    /// into the returned buffer: the cache by Quality does not suit them; static
    /// ones are drawn once per quality and copied (R.53, item 413: there were TWO
    /// copies per request, into the cache and out of it; now there is one).
    fn frame_bytes(&mut self, pts: Tick, q: Quality) -> Box<[u8]> {
        // Curves make ANY kind time-dependent: a single-element cache by Quality would then
        // return the first frame all the way.
        if self.state.animated() || depends_on_time(&self.state.kind) {
            let kind = self.state.kind_at(pts);
            return render_at(&kind, pts, q.width, q.height);
        }
        if self.cache.as_ref().is_none_or(|(cq, _)| *cq != q) {
            self.cache = Some((q, render_at(&self.state.kind, Tick(0), q.width, q.height)));
        }
        self.cache.as_ref().expect("frame was just built").1.clone()
    }
}

/// Whether the frame of the KIND depends on time by itself. The title is no longer
/// included here: its animation became ordinary curves (R.59, stage 2), and the
/// curves are seen by `GenState::animated`.
fn depends_on_time(kind: &GeneratorKind) -> bool {
    matches!(
        kind,
        GeneratorKind::NoiseField { .. }
            | GeneratorKind::Timecode { .. }
            | GeneratorKind::Leader { .. }
    )
}

/// Whether the kind’s alpha is honest: an EXHAUSTIVE match (R.53, item 419): a new
/// kind will not build until its alpha has been stated. Before, the rule was a
/// `matches!(…Text…)` check, past which a new transparent kind would
/// slip through silently and land in the composite as a black slab.
pub fn kind_has_alpha(kind: &GeneratorKind) -> bool {
    match kind {
        GeneratorKind::Text { .. }
        | GeneratorKind::Transparent
        | GeneratorKind::Rect { .. }
        | GeneratorKind::Ellipse { .. }
        | GeneratorKind::Line { .. } => true,
        GeneratorKind::Solid { .. }
        | GeneratorKind::Gradient { .. }
        | GeneratorKind::Corners { .. }
        | GeneratorKind::Checker { .. }
        | GeneratorKind::NoiseField { .. }
        | GeneratorKind::SmpteBars { .. }
        | GeneratorKind::Timecode { .. }
        | GeneratorKind::Leader { .. }
        | GeneratorKind::TestPattern { .. }
        | GeneratorKind::WindowPattern { .. } => false,
    }
}

/// The kind’s frame is exactly `w`×`h`; `pts` is the MATERIAL tick (it matters only
/// to live kinds: noise and timecode; a clip shift does not change them).
fn render_at(kind: &GeneratorKind, pts: Tick, w: u32, h: u32) -> Box<[u8]> {
    let n = (w as usize) * (h as usize);
    match kind {
        GeneratorKind::Solid { color_r, color_g, color_b } => {
            let mut buf = vec![0u8; n * 4];
            for px in buf.chunks_exact_mut(4) {
                px.copy_from_slice(&[*color_r, *color_g, *color_b, 255]);
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::Text { .. } => kt_text::render_rgba(&text_params(kind, w, h), w, h),
        GeneratorKind::Gradient { angle_deg, a_r, a_g, a_b, b_r, b_g, b_b } => {
            // a projection of the normalised coordinates onto the direction of the angle,
            // stretched over the whole frame: at any angle both colours are visible
            let rad = angle_deg.to_radians();
            let (dx, dy) = (rad.cos(), rad.sin());
            let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
                .map(|(x, y): (f64, f64)| x * dx + y * dy);
            let lo = corners.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = corners.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let span = (hi - lo).max(1e-9);
            let mut buf = vec![0u8; n * 4];
            for y in 0..h as usize {
                let ny = (y as f64 + 0.5) / h as f64;
                for x in 0..w as usize {
                    let nx = (x as f64 + 0.5) / w as f64;
                    let t = ((nx * dx + ny * dy - lo) / span).clamp(0.0, 1.0);
                    let mix = |a: u8, b: u8| {
                        (a as f64 + (b as f64 - a as f64) * t).round().clamp(0.0, 255.0) as u8
                    };
                    let o = (y * w as usize + x) * 4;
                    buf[o] = mix(*a_r, *b_r);
                    buf[o + 1] = mix(*a_g, *b_g);
                    buf[o + 2] = mix(*a_b, *b_b);
                    buf[o + 3] = 255;
                }
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::Corners {
            tl_r,
            tl_g,
            tl_b,
            tr_r,
            tr_g,
            tr_b,
            bl_r,
            bl_g,
            bl_b,
            br_r,
            br_g,
            br_b,
        } => {
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
                    buf[o] = bilerp(*tl_r, *tr_r, *bl_r, *br_r, fx, fy);
                    buf[o + 1] = bilerp(*tl_g, *tr_g, *bl_g, *br_g, fx, fy);
                    buf[o + 2] = bilerp(*tl_b, *tr_b, *bl_b, *br_b, fx, fy);
                    buf[o + 3] = 255;
                }
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::Checker { cols, a_r, a_g, a_b, b_r, b_g, b_b } => {
            // a cell is set by the COUNT of columns: the picture does not depend on
            // Quality (or on the decode step): the cells are square in width
            let cell = (w as f64 / (*cols).max(1) as f64).max(1.0);
            let a = [*a_r, *a_g, *a_b, 255];
            let b = [*b_r, *b_g, *b_b, 255];
            let mut buf = vec![0u8; n * 4];
            for y in 0..h as usize {
                let cy = (y as f64 / cell) as u64;
                for x in 0..w as usize {
                    let cx = (x as f64 / cell) as u64;
                    let o = (y * w as usize + x) * 4;
                    buf[o..o + 4].copy_from_slice(if (cx + cy).is_multiple_of(2) {
                        &a
                    } else {
                        &b
                    });
                }
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::NoiseField { amount, mono } => {
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
                    let mixc = |v: u8| {
                        (128.0 + (v as f64 - 128.0) * amount).round().clamp(0.0, 255.0) as u8
                    };
                    let o = (y * w as usize + x) * 4;
                    if *mono {
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
        GeneratorKind::SmpteBars { .. } => {
            // a simplified table: the upper 2/3 are seven 75% bars,
            // the lower third are reference blocks (−I, white 100%, +Q, black, PLUGE);
            // the values are deterministic and frozen by the test
            const TOP: [[u8; 3]; 7] = [
                [191, 191, 191],
                [191, 191, 0],
                [0, 191, 191],
                [0, 191, 0],
                [191, 0, 191],
                [191, 0, 0],
                [0, 0, 191],
            ];
            const BOTTOM: [[u8; 3]; 5] =
                [[0, 33, 76], [255, 255, 255], [50, 0, 106], [0, 0, 0], [16, 16, 16]];
            let mut buf = vec![0u8; n * 4];
            let split = (h as usize * 2) / 3;
            for y in 0..h as usize {
                let (bands, m): (&[[u8; 3]], usize) =
                    if y < split { (&TOP, 7) } else { (&BOTTOM, 5) };
                for x in 0..w as usize {
                    let i = (x * m / w.max(1) as usize).min(m - 1);
                    let c = bands[i];
                    let o = (y * w as usize + x) * 4;
                    buf[o] = c[0];
                    buf[o + 1] = c[1];
                    buf[o + 2] = c[2];
                    buf[o + 3] = 255;
                }
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::Timecode { fps } => {
            let fps = (*fps).max(1) as i64;
            let t = pts.0.max(0);
            let secs = t / 90_000;
            let ff = (t % 90_000) * fps / 90_000;
            let label =
                format!("{:02}:{:02}:{:02}:{:02}", secs / 3600, secs % 3600 / 60, secs % 60, ff);
            let mut c = crate::pixfont::Canvas::new(w.max(1), h.max(1), [0, 0, 0, 255]);
            let chars = label.chars().count();
            let s = ((c.w * 4 / 5) / crate::pixfont::text_width(chars, 1).max(1)).clamp(1, 40);
            let tx = (c.w - crate::pixfont::text_width(chars, s)) / 2;
            let ty = (c.h - 5 * s) / 2;
            c.text(tx, ty, s, &label, [235, 235, 235, 255]);
            c.buf.into_boxed_slice()
        }
        // --- R.53 (items 419-422): shapes, backdrop, countdown, tables ------
        GeneratorKind::Transparent => vec![0u8; n * 4].into_boxed_slice(),
        GeneratorKind::Rect { cx, cy, w: rw, h: rh, radius, feather, .. } => {
            let (r, g, b, a) = shape_color(kind);
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
                    let d = if dx > 0 && dy > 0 {
                        isqrt(dx * dx + dy * dy) - rad
                    } else {
                        dx.max(dy) - rad
                    };
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
        GeneratorKind::Ellipse { cx, cy, w: rw, h: rh, feather, .. } => {
            let (r, g, b, a) = shape_color(kind);
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
        GeneratorKind::Line { x1, y1, x2, y2, thickness, feather, .. } => {
            let (r, g, b, a) = shape_color(kind);
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
                    let t = if len2 == 0 {
                        0
                    } else {
                        ((px * vx + py * vy) * 256 / len2).clamp(0, 256)
                    };
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
        GeneratorKind::Leader { seconds, fps, .. } => {
            leader_frame(pts, *seconds, (*fps).max(1), w.max(1), h.max(1))
        }
        GeneratorKind::TestPattern { table } => table_frame(*table, w.max(1), h.max(1)),
        GeneratorKind::WindowPattern { area, level } => {
            let (fw, fh) = (w.max(1) as i64, h.max(1) as i64);
            let lv = (level.clamp(0.0, 1.0) * 255.0).round() as u8;
            // the share of AREA → the side in fractions of the frame: the window is similar to the frame
            let side = area.clamp(0.0, 1.0).sqrt();
            let (hw, hh) = (
                ((side * fw as f64) / 2.0).round() as i64,
                ((side * fh as f64) / 2.0).round() as i64,
            );
            let mut buf = vec![0u8; n * 4];
            for y in 0..fh {
                let inside_y = (y - fh / 2).abs() < hh;
                for x in 0..fw {
                    let v = if inside_y && (x - fw / 2).abs() < hw { lv } else { 0 };
                    let o = ((y * fw + x) * 4) as usize;
                    buf[o] = v;
                    buf[o + 1] = v;
                    buf[o + 2] = v;
                    buf[o + 3] = 255;
                }
            }
            buf.into_boxed_slice()
        }
    }
}

/// a 1 kHz sine at the level `db`, sample-accurate from the GLOBAL sample number
/// (byte for byte, as `AudioPattern::Sine`). `gate` decides whether a given sample
/// sounds: for SMPTE always, for the countdown by the gate.
/// One code for both kinds: a second sine would diverge in phase.
fn tone_1k(idx: i64, db: f64, gate: impl Fn(i64) -> bool) -> AudioBurst {
    let amp = 10f64.powf(db / 20.0);
    let mut b = AudioBurst::silence(idx);
    let base = idx * kt_time::AUDIO_BURST_FRAMES as i64;
    for f in 0..kt_time::AUDIO_BURST_FRAMES as i64 {
        let n = base + f;
        if !gate(n) {
            continue;
        }
        let v = ((n as f64) * 1000.0 * std::f64::consts::TAU / 48_000.0).sin() * amp;
        let v = v as f32;
        b.data[(f * 2) as usize] = v;
        b.data[(f * 2 + 1) as usize] = v;
    }
    b
}

/// An integer square root (Newton on u64): the analytic
/// distance field of the shapes is computed IN WHOLE 1/256 px, and `f64::sqrt` in the
/// pixel loop would break this rule for the sake of the same precision.
fn isqrt(v: i64) -> i64 {
    if v <= 0 {
        return 0;
    }
    let mut x = v as u64;
    let mut r = x;
    let mut t = x.div_ceil(2);
    while t < r {
        r = t;
        t = (r + x / r) / 2;
    }
    x = r;
    x as i64
}

/// Pixel coverage from the signed distance `d` (1/256 px) and the width of the
/// ramp `fe`: inside 255, outside 0, between linear. The same ramp as the
/// outline of a title (kt-text), so the edges of a shape and the edges of a glyph
/// look the same.
fn coverage(d: i64, fe: i64) -> i64 {
    let half = fe.max(1) / 2;
    if d <= -half {
        255
    } else if d >= half {
        0
    } else {
        255 - ((d + half) * 255) / fe.max(1)
    }
}

/// The shape’s colour: in one place for the three kinds (all three fields are named
/// the same way, but the enum variants differ).
fn shape_color(kind: &GeneratorKind) -> (u8, u8, u8, u8) {
    match kind {
        GeneratorKind::Rect { color_r, color_g, color_b, color_a, .. }
        | GeneratorKind::Ellipse { color_r, color_g, color_b, color_a, .. }
        | GeneratorKind::Line { color_r, color_g, color_b, color_a, .. } => {
            (*color_r, *color_g, *color_b, *color_a)
        }
        _ => (255, 255, 255, 255),
    }
}

/// Engineering table (R.53, item 422). The numbers are frozen by test t531.
///
/// 75% bars: eight vertical columns in descending order of brightness
/// (white, yellow, cyan, green, magenta, red, blue, black);
/// 75% of the 8-bit scale is 191. `[NOT VERIFIED]` against the paper
/// EBU 3213: the original source is paid, so the values are taken from the common
/// table and frozen here.
fn table_frame(t: kt_model::TestTable, w: u32, h: u32) -> Box<[u8]> {
    use kt_model::TestTable as T;
    let (fw, fh) = (w as usize, h as usize);
    let mut buf = vec![0u8; fw * fh * 4];
    match t {
        T::Ebu75 | T::Ebu100 => {
            let v: u8 = if t == T::Ebu75 { 191 } else { 255 };
            let bars: [[u8; 3]; 8] = [
                [v, v, v],
                [v, v, 0],
                [0, v, v],
                [0, v, 0],
                [v, 0, v],
                [v, 0, 0],
                [0, 0, v],
                [0, 0, 0],
            ];
            for y in 0..fh {
                for x in 0..fw {
                    let c = bars[(x * 8 / fw).min(7)];
                    let o = (y * fw + x) * 4;
                    buf[o..o + 3].copy_from_slice(&c);
                    buf[o + 3] = 255;
                }
            }
        }
        T::Grayscale | T::Steps10 => {
            for y in 0..fh {
                for x in 0..fw {
                    let g = if t == T::Grayscale {
                        (x * 255 / (fw - 1).max(1)) as u8
                    } else {
                        let step = (x * 10 / fw).min(9);
                        (step * 255 / 9) as u8
                    };
                    let o = (y * fw + x) * 4;
                    buf[o] = g;
                    buf[o + 1] = g;
                    buf[o + 2] = g;
                    buf[o + 3] = 255;
                }
            }
        }
        T::RampYcbcr => {
            // the BT.709 chroma plane at constant luminance: Cb along X,
            // Cr along Y. The coefficients are in the preamble (tables by columns and
            // rows), the pixel loop is integer.
            let cb: Vec<i32> =
                (0..fw).map(|x| (16 + (x * 224 / (fw - 1).max(1))) as i32 - 128).collect();
            let cr: Vec<i32> =
                (0..fh).map(|y| (16 + (y * 224 / (fh - 1).max(1))) as i32 - 128).collect();
            const Y: i32 = 128;
            for (y, cr_y) in cr.iter().enumerate() {
                let (r_cr, g_cr) = ((1_5748 * cr_y) / 1_0000, (-4681 * cr_y) / 1_0000);
                for (x, cb_x) in cb.iter().enumerate() {
                    let g_cb = (-1873 * cb_x) / 1_0000;
                    let b_cb = (1_8556 * cb_x) / 1_0000;
                    let o = (y * fw + x) * 4;
                    buf[o] = (Y + r_cr).clamp(0, 255) as u8;
                    buf[o + 1] = (Y + g_cb + g_cr).clamp(0, 255) as u8;
                    buf[o + 2] = (Y + b_cb).clamp(0, 255) as u8;
                    buf[o + 3] = 255;
                }
            }
        }
    }
    buf.into_boxed_slice()
}

/// A countdown frame (R.53, item 421): a cross, a circle, a sweeping
/// sector and the digit of the seconds. The time is the MATERIAL’s pts, as for timecode.
///
/// The last frame before zero is a “2-pop”: a white field EXACTLY `1/fps` long.
fn leader_frame(pts: Tick, seconds: u32, fps: u32, w: u32, h: u32) -> Box<[u8]> {
    let total = seconds.max(1) as i64 * 90_000;
    let t = pts.0.clamp(0, total);
    let left = total - t; // how much is left until zero, in ticks
    let frame_ticks = 90_000 / fps as i64;
    let (fw, fh) = (w as i64, h as i64);
    let mut c = crate::pixfont::Canvas::new(w, h, [16, 16, 16, 255]);
    // 2-pop: exactly one frame of the white field before zero
    if left <= frame_ticks {
        for px in c.buf.chunks_exact_mut(4) {
            px.copy_from_slice(&[235, 235, 235, 255]);
        }
        return c.buf.into_boxed_slice();
    }
    let (cx, cy) = (fw / 2, fh / 2);
    let radius = fw.min(fh) * 2 / 5;
    let white = [220u8, 220, 220, 255];
    // a cross across the whole frame: two bars 1/200 of the smaller side thick
    let bar = (fw.min(fh) / 200).max(1);
    for y in 0..fh {
        for x in 0..fw {
            let on_cross = (x - cx).abs() <= bar || (y - cy).abs() <= bar;
            // circle: a ring of the same thickness
            let d = isqrt((x - cx) * (x - cx) + (y - cy) * (y - cy));
            let on_ring = (d - radius).abs() <= bar;
            // sweeping sector: the sign of the vector product (no atan2)
            let phase = ((left % 90_000) * 4) / 90_000; // a quarter, 0..3
            let rem = (left % 90_000) * 4 - phase * 90_000; // 0..90_000
            let (dx, dy) = (x - cx, y - cy);
            let swept = d <= radius && sector_covers(dx, dy, phase, rem);
            let o = ((y * fw + x) * 4) as usize;
            if on_cross || on_ring {
                c.buf[o..o + 4].copy_from_slice(&white);
            } else if swept {
                c.buf[o..o + 4].copy_from_slice(&[60, 60, 60, 255]);
            }
        }
    }
    // the seconds digit: how many FULL seconds are left, at least 1
    let digit = ((left + 89_999) / 90_000).max(1).to_string();
    let chars = digit.chars().count();
    let s = ((c.h / 3) / 5).clamp(1, 60);
    let tx = (c.w - crate::pixfont::text_width(chars, s)) / 2;
    let ty = (c.h - 5 * s) / 2;
    c.text(tx, ty, s, &digit, [245, 245, 245, 255]);
    c.buf.into_boxed_slice()
}

/// Whether the ray (dx, dy) is covered by the sweeping sector. Integer signs of
/// vector products instead of atan2 (the rule “float only in the preamble”):
/// the sector grows clockwise from “12 o’clock”.
fn sector_covers(dx: i64, dy: i64, phase: i64, rem: i64) -> bool {
    // the quadrant of a point, counting from “12 o’clock” clockwise: 0 = right-up
    let quad = match (dx >= 0, dy < 0) {
        (true, true) => 0,
        (true, false) => 1,
        (false, false) => 2,
        (false, true) => 3,
    };
    if quad < phase {
        return true;
    }
    if quad > phase {
        return false;
    }
    // inside the current quadrant: the share traversed along the chord: a comparison of
    // |across| and |along| via a vector product with the boundary
    let (a, b) = match quad {
        0 => (dx, -dy),
        1 => (dy, dx),
        2 => (-dx, dy),
        _ => (-dy, -dx),
    };
    a * 90_000 >= rem * (a + b).max(1)
}

/// lowbias32 (Chris Wellons): the same avalanche hash as in kt-fx::video
/// (`noise_hash`): integer, deterministic, stateless.
fn lowbias32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// The layout of a title in the pixels of a `w`×`h` frame, for the drag frame in the
/// preview (R.52 S6). Returns `None` for a non-title. It goes through the same
/// `text_params`: the window and the frame MUST see the same values.
pub fn title_layout(kind: &GeneratorKind, w: u32, h: u32) -> Option<kt_text::LayoutInfo> {
    matches!(kind, GeneratorKind::Text { .. })
        .then(|| kt_text::layout_info(&text_params(kind, w, h), w, h))
}

/// THE ONLY function that converts a stored title (kt-model) into the input of the
/// rasterizer (kt-text): there is no second parser (kt-text/CLAUDE.md):
/// the window and the frame MUST see the same values.
fn text_params(kind: &GeneratorKind, w: u32, h: u32) -> kt_text::TextParams {
    let GeneratorKind::Text {
        text,
        family,
        bold,
        size,
        line_spacing,
        tracking,
        align,
        anchor,
        pos_x,
        pos_y,
        fill_r,
        fill_g,
        fill_b,
        fill_a,
        stroke_w,
        stroke_r,
        stroke_g,
        stroke_b,
        stroke_a,
        shadow_dx,
        shadow_dy,
        shadow_blur,
        shadow_r,
        shadow_g,
        shadow_b,
        shadow_a,
        reveal,
        reveal_soft,
        offset_x,
        offset_y,
        plate_pad_x,
        plate_pad_y,
        plate_r,
        plate_g,
        plate_b,
        plate_a,
        order,
    } = kind
    else {
        unreachable!("text_params is only called for GeneratorKind::Text");
    };
    let align = match align {
        kt_model::TextAlign::Left => kt_text::Align::Left,
        kt_model::TextAlign::Center => kt_text::Align::Center,
        kt_model::TextAlign::Right => kt_text::Align::Right,
    };
    let anchor = match anchor {
        kt_model::TextAnchor::TopLeft => kt_text::Anchor::TopLeft,
        kt_model::TextAnchor::TopCenter => kt_text::Anchor::TopCenter,
        kt_model::TextAnchor::TopRight => kt_text::Anchor::TopRight,
        kt_model::TextAnchor::CenterLeft => kt_text::Anchor::CenterLeft,
        kt_model::TextAnchor::Center => kt_text::Anchor::Center,
        kt_model::TextAnchor::CenterRight => kt_text::Anchor::CenterRight,
        kt_model::TextAnchor::BottomLeft => kt_text::Anchor::BottomLeft,
        kt_model::TextAnchor::BottomCenter => kt_text::Anchor::BottomCenter,
        kt_model::TextAnchor::BottomRight => kt_text::Anchor::BottomRight,
    };
    let layer = |l: &kt_model::TextLayer| match l {
        kt_model::TextLayer::Plate => kt_text::Layer::Plate,
        kt_model::TextLayer::Shadow => kt_text::Layer::Shadow,
        kt_model::TextLayer::Stroke => kt_text::Layer::Stroke,
        kt_model::TextLayer::Fill => kt_text::Layer::Fill,
    };
    kt_text::TextParams {
        text: text.clone(),
        family: family.clone(),
        style: if *bold { kt_text::FaceStyle::Bold } else { kt_text::FaceStyle::Regular },
        size: *size,
        line_spacing: *line_spacing,
        tracking: *tracking,
        align,
        anchor,
        pos: (*pos_x, *pos_y),
        fill: [*fill_r, *fill_g, *fill_b, *fill_a],
        stroke_w: *stroke_w,
        stroke: [*stroke_r, *stroke_g, *stroke_b, *stroke_a],
        shadow_dx: *shadow_dx,
        shadow_dy: *shadow_dy,
        shadow_blur: *shadow_blur,
        shadow: [*shadow_r, *shadow_g, *shadow_b, *shadow_a],
        plate_pad: (*plate_pad_x, *plate_pad_y),
        plate: [*plate_r, *plate_g, *plate_b, *plate_a],
        order: [layer(&order[0]), layer(&order[1]), layer(&order[2]), layer(&order[3])],
        // The animation of a title is ORDINARY NUMBERS of the kind (R.59, stage 2): there are
        // no modes any more; the curves are resolved before the call (`GenState::kind_at`).
        // The offset is stored in FRACTIONS OF THE FRAME (a title survives a change of the
        // project format), it is converted to pixels here; the 1/64 px quantum stays
        // where it was, in kt-text.
        reveal: *reveal,
        reveal_soft: *reveal_soft,
        offset: (offset_x * w as f64, offset_y * h as f64),
    }
}

impl MediaSource for GeneratorSource {
    fn info(&self) -> &MediaInfo {
        &self.info
    }

    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        // SMPTE tone: a 1 kHz sine at the level tone_db, sample-accurate from the
        // global sample number (byte for byte, as AudioPattern::Sine)
        if let GeneratorKind::SmpteBars { tone: true, tone_db } = &self.state.kind {
            return Ok(tone_1k(idx, *tone_db, |_| true));
        }
        // The countdown beep (R.53, item 421): the SAME code as the SMPTE tone, only
        // with a gate: it sounds exactly on whole seconds, 1/fps long, and
        // is silent the rest of the time. The phase is taken from the GLOBAL sample
        // number, so seek does not click (as with the tone).
        if let GeneratorKind::Leader { seconds, fps, beep: true, beep_db } = &self.state.kind {
            let total = (*seconds).max(1) as i64 * 90_000;
            let step = 90_000 / (*fps).max(1) as i64;
            return Ok(tone_1k(idx, *beep_db, |n| {
                // sample number → material tick (48 kHz → 90 kHz, exact)
                let t = n * 90_000 / 48_000;
                if t < 0 || t > total {
                    return false;
                }
                // a beep at the START of each second of the material, 1/fps long,
                // and the same one on the “2-pop” (the end of the countdown is also a whole second)
                t % 90_000 < step || total - t <= step
            }));
        }
        Ok(AudioBurst::silence(idx))
    }

    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
        let q = quality.unwrap_or(DEFAULT_Q);
        let rgba = self.frame_bytes(pts, q);
        let has_alpha = kind_has_alpha(&self.state.kind);
        Ok(VideoFrame { pts, width: q.width, height: q.height, rgba, has_alpha })
    }

    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        Some(Tick(pts.0.max(0)))
    }

    /// In-place reconfiguration (R.53, item 410): the worker's stream stays alive and
    /// it drops the frames of the old parameters itself. Refusal happens when the source
    /// passport would change (SMPTE with tone enabled becomes audible media, which is
    /// already another set of clips) or the path cannot be
    /// parsed.
    fn retune(&mut self, path: &str) -> bool {
        let Some(kind) = kt_model::parse_gen_path(path) else { return false };
        self.retune_gen(&kt_model::GenState::new(kind))
    }

    /// Reconfiguration to NEW EVENT VALUES (R.59, stage 2) — the same
    /// band as the path, only without the middle line.
    fn retune_gen(&mut self, state: &kt_model::GenState) -> bool {
        if Self::info_for(&state.kind) != self.info {
            return false;
        }
        self.state = state.clone();
        self.cache = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(r: u8, g: u8, b: u8) -> GeneratorSource {
        GeneratorSource::new(GeneratorKind::Solid { color_r: r, color_g: g, color_b: b })
    }

    /// t471 (kin of t208): the generator frame is deterministic, exactly
    /// the requested size, does not depend on time, color is the given one.
    #[test]
    fn t471_generator_frame_is_deterministic_and_exact() {
        let q = Quality { width: 320, height: 180 };
        let mut s = solid(255, 136, 0);
        let f1 = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let f2 = s.read_video_frame(Tick(90_000), Some(q)).unwrap();
        assert_eq!((f1.width, f1.height), (320, 180), "the frame must be exactly Quality");
        assert_eq!(f1.rgba, f2.rgba, "the frame must not depend on time");
        assert_eq!(f2.pts.0, 90_000, "pts must be the requested one");
        assert!(!f1.has_alpha);
        assert!(f1.rgba.chunks_exact(4).all(|p| p == [255, 136, 0, 255]), "wrong colour");

        // a different size — a different buffer, also exact
        let q2 = Quality { width: 64, height: 36 };
        let f3 = s.read_video_frame(Tick(0), Some(q2)).unwrap();
        assert_eq!((f3.width, f3.height), (64, 36));
        assert_eq!(f3.rgba.len(), 64 * 36 * 4);

        // two independent sources with the same parameters — byte-for-byte
        let g = solid(255, 136, 0).read_video_frame(Tick(7), Some(q)).unwrap();
        assert_eq!(g.rgba, f1.rgba);
        // other parameters — other frame
        let other = solid(0, 0, 1).read_video_frame(Tick(7), Some(q)).unwrap();
        assert_ne!(other.rgba, f1.rgba);
    }

    /// t471: source passport — “no duration”, native resolution for the
    /// decode scale step, project grid.
    #[test]
    fn t471_generator_info_contract() {
        let s = solid(0, 0, 0);
        let i = s.info();
        assert_eq!(i.duration.0, 0, "the generator has no duration");
        assert!(i.has_video && !i.has_audio);
        assert_eq!(i.fps, None, "it has no frame grid of its own");
        assert_eq!((i.width, i.height), (GEN_NATIVE, GEN_NATIVE));
        assert_eq!(s.keyframe_before(Tick(-5)), Some(Tick(0)));
        assert_eq!(s.keyframe_before(Tick(90_000)), Some(Tick(90_000)));
    }

    /// t498 (R.53, item 410): the generator is reconfigured IN PLACE. The frame
    /// after `retune` is byte-for-byte equal to the frame of a fresh source (cache
    /// reset), a refusal changes nothing.
    #[test]
    fn t498_generator_retunes_in_place() {
        let q = Quality { width: 64, height: 36 };
        let orange = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
        let mut s =
            GeneratorSource::new(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 });
        // we pre-warm the internal frame cache with the old parameters
        let before = s.read_video_frame(Tick(0), Some(q)).unwrap();
        assert_eq!(&before.rgba[..4], &[0, 0, 0, 255]);

        assert!(s.retune(&kt_model::format_gen_path(&orange)), "solid → solid is reconfigured");
        let after = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let fresh =
            GeneratorSource::new(orange.clone()).read_video_frame(Tick(0), Some(q)).unwrap();
        assert_eq!(after.rgba, fresh.rgba, "the cache of old parameters must be reset");

        // refusals: broken path, foreign schema, passport change (SMPTE tone)
        let mut smpte =
            GeneratorSource::new(GeneratorKind::SmpteBars { tone: false, tone_db: -20.0 });
        let paspt = smpte.info().clone();
        let with_tone =
            kt_model::format_gen_path(&GeneratorKind::SmpteBars { tone: true, tone_db: -20.0 });
        assert!(!smpte.retune(&with_tone), "enabling the tone changes the passport — only recreation");
        assert_eq!(smpte.info(), &paspt, "refusal changes nothing");
        assert!(!smpte.retune("gen:unknown"));
        assert!(!smpte.retune("media/clip.mp4"));
        assert_eq!(smpte.info(), &paspt);

        // changing the KIND under the same passport is legitimate: the clips are the same
        let mut s2 = GeneratorSource::new(orange);
        assert!(s2.retune("gen:checker"), "solid → checker: the passport is the same");
    }

    /// t507 (R.53, item 413): a static frame is drawn ONCE per quality and handed
    /// out as a single copy. Before this item there were two copies per request —
    /// into the cache and out of it, i.e. 16 MB of extra memcpy on
    /// every 1080p frame.
    #[test]
    fn t507_generator_frame_is_rendered_once_per_quality() {
        let q1 = Quality { width: 64, height: 36 };
        let q2 = Quality { width: 32, height: 18 };
        let mut s =
            GeneratorSource::new(GeneratorKind::Solid { color_r: 9, color_g: 9, color_b: 9 });
        let a = s.read_video_frame(Tick(0), Some(q1)).unwrap();
        let b = s.read_video_frame(Tick(90_000), Some(q1)).unwrap();
        assert_eq!(a.rgba, b.rgba, "a static frame does not depend on time");
        // the buffers are DIFFERENT (ownership is handed outward), but the content is one
        assert!(!std::ptr::eq(a.rgba.as_ptr(), b.rgba.as_ptr()));

        // switching quality back and forth does not spoil the content
        let small = s.read_video_frame(Tick(0), Some(q2)).unwrap();
        assert_eq!(small.rgba.len(), (q2.width * q2.height * 4) as usize);
        let again = s.read_video_frame(Tick(0), Some(q1)).unwrap();
        assert_eq!(again.rgba, a.rgba);

        // the live view (noise) does not use the quality cache at all
        let mut n = GeneratorSource::new(GeneratorKind::NoiseField { amount: 1.0, mono: true });
        let f1 = n.read_video_frame(Tick(0), Some(q1)).unwrap();
        let f2 = n.read_video_frame(Tick(3000), Some(q1)).unwrap();
        assert_ne!(f1.rgba, f2.rgba, "noise must depend on time");
    }

    /// t491 (R.52 stage 5): noise is counter-based — frame = f(pts, x, y): repeat
    /// byte-for-byte, different pts — different frames, mono gray, amount 0 —
    /// exactly 128; the grid is the BASE (1920×1080): a half-Quality frame
    /// has the same cells (independence from Quality and the scale step).
    #[test]
    fn t491_noise_field_is_counter_based() {
        let mk = |amount: f64, mono: bool| GeneratorKind::NoiseField { amount, mono };
        let q = Quality { width: 1920, height: 1080 };
        let mut s = GeneratorSource::new(mk(1.0, true));
        let a = s.read_video_frame(Tick(3000), Some(q)).unwrap();
        let b = s.read_video_frame(Tick(3000), Some(q)).unwrap();
        assert_eq!(a.rgba, b.rgba, "the same pts — the same bytes");
        let c = s.read_video_frame(Tick(6000), Some(q)).unwrap();
        assert_ne!(a.rgba, c.rgba, "another frame of the material — another grain");
        assert!(a.rgba.chunks_exact(4).all(|p| p[0] == p[1] && p[1] == p[2]), "mono grey");
        // full range: both dark and light values are present
        assert!(a.rgba.chunks_exact(4).any(|p| p[0] < 32));
        assert!(a.rgba.chunks_exact(4).any(|p| p[0] > 224));

        // amount 0 — a flat field of 128
        let z = GeneratorSource::new(mk(0.0, true)).read_video_frame(Tick(3000), Some(q)).unwrap();
        assert!(z.rgba.chunks_exact(4).all(|p| p[0] == 128));

        // independence from Quality: the half frame has the same cells
        let half =
            s.read_video_frame(Tick(3000), Some(Quality { width: 960, height: 540 })).unwrap();
        for (hx, hy) in [(10usize, 7usize), (123, 400), (900, 100)] {
            let hpx = &half.rgba[(hy * 960 + hx) * 4];
            let fpx = &a.rgba[((hy * 2) * 1920 + hx * 2) * 4];
            assert_eq!(hpx, fpx, "({hx},{hy}): the grain must match the full frame");
        }
    }

    /// t492 (R.52 stage 5): SMPTE — has_audio is driven by the `tone` parameter; the
    /// 1 kHz tone is byte-for-byte and audible at its level; the top band is 75% gray.
    #[test]
    fn t492_smpte_tone_and_bars() {
        let silent = GeneratorSource::new(GeneratorKind::SmpteBars { tone: false, tone_db: -20.0 });
        assert!(!silent.info().has_audio, "without a tone no audio clip is created");
        let mut loud =
            GeneratorSource::new(GeneratorKind::SmpteBars { tone: true, tone_db: -20.0 });
        assert!(loud.info().has_audio);
        let a = loud.read_audio_burst(3).unwrap();
        let b = loud.read_audio_burst(3).unwrap();
        assert_eq!(a.data.as_slice(), b.data.as_slice(), "the tone is byte for byte");
        let peak = a.data.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!((peak - 0.1).abs() < 0.01, "−20 dBFS ≈ 0.1: {peak}");
        let mut hot = GeneratorSource::new(GeneratorKind::SmpteBars { tone: true, tone_db: -6.0 });
        let h = hot.read_audio_burst(3).unwrap();
        let hpeak = h.data.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(hpeak > peak * 3.0, "the level obeys the parameter");

        let f = GeneratorSource::new(GeneratorKind::SmpteBars { tone: false, tone_db: -20.0 })
            .read_video_frame(Tick(0), Some(Quality { width: 700, height: 300 }))
            .unwrap();
        assert_eq!(&f.rgba[..4], &[191, 191, 191, 255], "first bar — 75 % grey");
    }

    /// t493 (R.52 stage 5): timecode — frame = f(source pts) and the fps parameter.
    #[test]
    fn t493_timecode_frames_follow_material() {
        let q = Quality { width: 640, height: 360 };
        let mut s = GeneratorSource::new(GeneratorKind::Timecode { fps: 30 });
        let a = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let a2 = s.read_video_frame(Tick(0), Some(q)).unwrap();
        assert_eq!(a.rgba, a2.rgba);
        let b = s.read_video_frame(Tick(90_000), Some(q)).unwrap();
        assert_ne!(a.rgba, b.rgba, "the second changed — the digits differ");
        // within one frame (30 fps => 3000 ticks) the picture is one
        let c = s.read_video_frame(Tick(90_000 + 2_999), Some(q)).unwrap();
        assert_eq!(b.rgba, c.rgba, "within a frame the timecode does not jitter");
        // fps is a parameter: same pts, different frame counter
        let mut s60 = GeneratorSource::new(GeneratorKind::Timecode { fps: 60 });
        let d = s60.read_video_frame(Tick(90_000 + 2_999), Some(q)).unwrap();
        assert_ne!(c.rgba, d.rgba, "fps changes the FF field");
    }

    /// t484 (R.52 stage 2): title source — frame EXACTLY Quality, with real
    /// ALPHA (the title lives over other layers), deterministic, does not depend
    /// on pts; style changes the bytes; the frame is drawn by kt-text (vendored
    /// font — system fonts are irrelevant here).
    #[test]
    fn t484_text_source_renders_title() {
        let title = |bold: bool| {
            let mut k = GeneratorKind::default_text();
            if let GeneratorKind::Text { text, bold: b, .. } = &mut k {
                *text = "Kinetren\nR.52".into();
                *b = bold;
            }
            GeneratorSource::new(k)
        };
        let q = Quality { width: 320, height: 180 };
        let mut s = title(false);
        let f1 = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let f2 = s.read_video_frame(Tick(90_000), Some(q)).unwrap();
        assert_eq!((f1.width, f1.height), (320, 180), "the frame must be exactly Quality");
        assert!(f1.has_alpha, "the title has honest alpha");
        assert_eq!(f1.rgba, f2.rgba, "the frame does not depend on time");
        let ink = f1.rgba.chunks_exact(4).filter(|px| px[3] > 0).count();
        assert!(ink > 200, "there is no text on the frame: {ink}");
        // background is transparent, not black: uncovered pixels with zero alpha
        assert!(f1.rgba.chunks_exact(4).any(|px| px[3] == 0));
        // bold style — other bytes (the second face really loads)
        let fb = title(true).read_video_frame(Tick(0), Some(q)).unwrap();
        assert_ne!(fb.rgba, f1.rgba, "Bold must differ from Regular");
        // same passport as the other generators
        let i = title(false).info().clone();
        assert!(i.has_video && !i.has_audio);
        assert_eq!((i.width, i.height), (GEN_NATIVE, GEN_NATIVE));
        assert_eq!(i.duration.0, 0);
    }

    // ---- R.53 (items 419-422): shapes, backing, countdown, tables --------

    /// t526: all new kinds are deterministic and return EXACTLY Quality.
    /// Invariant 11 is common to all generators, and a new kind must fall
    /// under it, not “probably also”.
    #[test]
    fn t526_new_kinds_are_deterministic_and_exact() {
        let q = Quality { width: 96, height: 54 };
        for k in GeneratorKind::all() {
            let mut a = GeneratorSource::new(k.clone());
            let mut b = GeneratorSource::new(k.clone());
            for pts in [Tick(0), Tick(45_000), Tick(300_000)] {
                let fa = a.read_video_frame(pts, Some(q)).unwrap();
                let fb = b.read_video_frame(pts, Some(q)).unwrap();
                assert_eq!((fa.width, fa.height), (96, 54), "{}: frame is not Quality", k.tag());
                assert_eq!(fa.rgba, fb.rgba, "{} @{}: frame is not deterministic", k.tag(), pts.0);
            }
        }
    }

    /// t527: SHAPES and the backing have honest alpha, the others do not, and
    /// the rule is EXHAUSTIVE (a new kind will not build without an answer).
    #[test]
    fn t527_alpha_is_declared_by_every_kind() {
        let q = Quality { width: 64, height: 36 };
        for k in GeneratorKind::all() {
            let want = kind_has_alpha(&k);
            let mut s = GeneratorSource::new(k.clone());
            let f = s.read_video_frame(Tick(0), Some(q)).unwrap();
            assert_eq!(f.has_alpha, want, "{}: the frame and the alpha rule diverged", k.tag());
        }
        // backing — EXACTLY zeros: a placeholder paints nothing
        let mut t = GeneratorSource::new(GeneratorKind::Transparent);
        let f = t.read_video_frame(Tick(0), Some(q)).unwrap();
        assert!(f.rgba.iter().all(|v| *v == 0), "the backing must be empty");
        assert!(f.has_alpha);
    }

    /// t528: the rectangle stands where stated, and beyond its edge it is
    /// transparent. The assertion is IN PIXELS, not “the field got set”.
    #[test]
    fn t528_rect_covers_what_it_says() {
        let q = Quality { width: 200, height: 100 };
        let k = kt_model::parse_gen_path("gen:rect:cx=0.5:cy=0.5:w=0.5:h=0.5:c=ff8800ff")
            .expect("shape path");
        let mut s = GeneratorSource::new(k);
        let f = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let at = |x: u32, y: u32| {
            let o = ((y * 200 + x) * 4) as usize;
            [f.rgba[o], f.rgba[o + 1], f.rgba[o + 2], f.rgba[o + 3]]
        };
        assert_eq!(at(100, 50), [255, 136, 0, 255], "the centre must be filled");
        // half-width = 50 px from center: 100±50 — the edge
        assert_eq!(at(60, 50)[3], 255, "inside the rectangle");
        assert_eq!(at(10, 50)[3], 0, "left of the shape must be transparent");
        assert_eq!(at(190, 50)[3], 0, "right of the shape must be transparent");
        assert_eq!(at(100, 5)[3], 0, "above must be transparent");
    }

    /// t529: a line segment is a CAPSULE between its ends: opaque under the line,
    /// transparent to the side, and also transparent beyond the end (projection clamp).
    #[test]
    fn t529_line_is_a_capsule_between_its_ends() {
        let q = Quality { width: 200, height: 100 };
        let k = kt_model::parse_gen_path("gen:line:x1=0.25:y1=0.5:x2=0.75:y2=0.5:th=0.1")
            .expect("segment path");
        let mut s = GeneratorSource::new(k);
        let f = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let a = |x: u32, y: u32| f.rgba[(((y * 200 + x) * 4) + 3) as usize];
        assert_eq!(a(100, 50), 255, "middle of the segment");
        assert_eq!(a(60, 50), 255, "at the left end");
        assert_eq!(a(10, 50), 0, "left of the start — empty");
        assert_eq!(a(190, 50), 0, "right of the end — empty");
        assert_eq!(a(100, 10), 0, "away from the line — empty");
    }

    /// t530: the countdown runs over the MATERIAL, the digit decreases, and the
    /// last frame before zero is a white field “2-pop” exactly 1/fps long.
    #[test]
    fn t530_leader_counts_down_and_pops() {
        let q = Quality { width: 120, height: 90 };
        let k = kt_model::parse_gen_path("gen:leader:n=3:fps=30").expect("countdown path");
        let mut s = GeneratorSource::new(k);
        let mean = |f: &VideoFrame| {
            f.rgba.chunks_exact(4).map(|p| p[0] as u64).sum::<u64>() / (f.rgba.len() / 4) as u64
        };
        // frames of different seconds differ (the digit and the sector move)
        let a = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let b = s.read_video_frame(Tick(90_000), Some(q)).unwrap();
        assert_ne!(a.rgba, b.rgba, "the countdown must run along the material");
        // 2-pop: the last 1/30 s is a light field, before it is dark
        let total = 3 * 90_000;
        let pop = s.read_video_frame(Tick(total - 100), Some(q)).unwrap();
        let before = s.read_video_frame(Tick(total - 90_000 / 30 - 100), Some(q)).unwrap();
        assert!(mean(&pop) > 200, "2-pop must be a white field: {}", mean(&pop));
        assert!(mean(&before) < 100, "there must be no frame before the white field");
    }

    /// t531: the numbers of engineering tables are FROZEN. A table without fixed
    /// values is a picture, not a measurement signal.
    #[test]
    fn t531_table_numbers_are_frozen() {
        use kt_model::TestTable;
        let q = Quality { width: 160, height: 40 };
        // the middle of band number `i` of `n` is the pad center, not the edge
        let band = |t: TestTable, i: u32, n: u32| {
            let mut s = GeneratorSource::new(GeneratorKind::TestPattern { table: t });
            let f = s.read_video_frame(Tick(0), Some(q)).unwrap();
            let x = (i * 2 + 1) * 160 / (n * 2);
            let o = ((20 * 160 + x) * 4) as usize;
            [f.rgba[o], f.rgba[o + 1], f.rgba[o + 2]]
        };
        let bar = |t: TestTable, i: u32| band(t, i, 8);
        // 75% of the 8-bit scale — 191; order of decreasing brightness
        assert_eq!(bar(TestTable::Ebu75, 0), [191, 191, 191], "white bar 75 %");
        assert_eq!(bar(TestTable::Ebu75, 1), [191, 191, 0], "yellow");
        assert_eq!(bar(TestTable::Ebu75, 2), [0, 191, 191], "cyan");
        assert_eq!(bar(TestTable::Ebu75, 7), [0, 0, 0], "black");
        assert_eq!(bar(TestTable::Ebu100, 0), [255, 255, 255], "white bar 100 %");
        assert_eq!(bar(TestTable::Ebu100, 6), [0, 0, 255], "blue at full");
        // steps: ten even plateaus from 0 to 255
        assert_eq!(band(TestTable::Steps10, 0, 10), [0, 0, 0]);
        assert_eq!(band(TestTable::Steps10, 7, 10), [198, 198, 198], "seventh of ten steps");
        assert_eq!(band(TestTable::Steps10, 9, 10), [255, 255, 255]);
        // the gray scale is continuous and monotonic along X
        let mut g =
            GeneratorSource::new(GeneratorKind::TestPattern { table: TestTable::Grayscale });
        let f = g.read_video_frame(Tick(0), Some(q)).unwrap();
        let v = |x: u32| f.rgba[((20 * 160 + x) * 4) as usize];
        assert_eq!(v(0), 0);
        assert_eq!(v(159), 255);
        assert!((1..160).all(|x| v(x) >= v(x - 1)), "the grey scale must be monotonic");
    }

    /// t532: the level window occupies a given SHARE OF THE FRAME AREA. That is the
    /// point of the signal: it is used to measure APL and panel warm-up.
    #[test]
    fn t532_level_window_covers_the_declared_area() {
        let q = Quality { width: 400, height: 200 };
        for (area, tol) in [(0.1_f64, 0.01), (0.25, 0.01), (1.0, 0.01)] {
            let path = format!("gen:window:ar={area}");
            let k = kt_model::parse_gen_path(&path).expect("window path");
            let mut s = GeneratorSource::new(k);
            let f = s.read_video_frame(Tick(0), Some(q)).unwrap();
            let lit = f.rgba.chunks_exact(4).filter(|p| p[0] > 0).count() as f64;
            let frac = lit / (400.0 * 200.0);
            assert!((frac - area).abs() < tol, "{path}: area {frac:.3} instead of {area}");
        }
    }

    /// t533: the countdown beep is the SAME 1 kHz tone as SMPTE, and it sounds
    /// only on whole seconds. Without a gate the countdown would hum continuously.
    #[test]
    fn t533_leader_beep_is_gated_to_whole_seconds() {
        let silent =
            GeneratorSource::new(kt_model::parse_gen_path("gen:leader:n=3").expect("path"));
        assert!(!silent.info().has_audio, "without a beep no audio clip is created");
        let mut loud =
            GeneratorSource::new(kt_model::parse_gen_path("gen:leader:n=3:beep=1").expect("path"));
        assert!(loud.info().has_audio);
        let peak = |s: &mut GeneratorSource, idx: i64| {
            s.read_audio_burst(idx).unwrap().data.iter().fold(0f32, |m, v| m.max(v.abs()))
        };
        // burst 1024 frames = 1920 ticks; the zero second of material — the gate is
        // open (exactly 3 s remain until the end)
        assert!(peak(&mut loud, 0) > 0.01, "on a whole second the beep must sound");
        // the middle between seconds — silence
        let mid = 48_000 / 2 / 1024;
        assert_eq!(peak(&mut loud, mid), 0.0, "between seconds there must be silence");
    }

    /// t539 (R.53, items 423-424; rewritten in R.59, stage 2): title animation runs
    /// over the MATERIAL's pts, and the Quality cache is disabled ONLY for the
    /// animated one — static titles do not suffer from the animation's appearance
    /// (they are not affected).
    ///
    /// What changed in the plan: the `am=tw`/`am=cr` modes no longer exist in the
    /// model (owner's decision R.59 “generator parameters — on the event”).
    /// The same five animations are now built from curves — `text_anim_curves` —
    /// and the same behavior is verified here, just assembled from
    /// ordinary properties.
    #[test]
    fn t539_title_animation_follows_material_time() {
        use kt_model::{GenState, TextAnim, text_anim_curves, text_anim_values};
        let q = Quality { width: 240, height: 120 };
        let ink = |f: &VideoFrame| f.rgba.chunks_exact(4).map(|p| p[3] as u64).sum::<u64>();
        let titled = |anim: TextAnim| {
            let mut kind = kt_model::parse_gen_path("gen:text:t=ABCD:s=0.3").expect("title path");
            text_anim_values(&mut kind, anim);
            GeneratorSource::from_state(GenState { kind, auto: text_anim_curves(anim, 2.0, 2.0) })
        };
        // machine: empty at the start, all the text by the end
        let mut s = titled(TextAnim::Typewriter);
        let a = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let mid = s.read_video_frame(Tick(90_000), Some(q)).unwrap();
        let end = s.read_video_frame(Tick(180_000), Some(q)).unwrap();
        assert_eq!(ink(&a), 0, "the typewriter printed no letter at zero");
        assert!(ink(&mid) > 0 && ink(&mid) < ink(&end), "by the middle part is typed");
        // past the animation's end the frame equals the static title
        let stat = kt_model::parse_gen_path("gen:text:t=ABCD:s=0.3").expect("path");
        let mut st = GeneratorSource::new(stat);
        let want = st.read_video_frame(Tick(0), Some(q)).unwrap();
        assert_eq!(end.rgba, want.rgba, "after the reveal the title must become ordinary");

        // ticker: the block MOVES, it does not fade in
        let mut c = titled(TextAnim::Crawl);
        let f0 = c.read_video_frame(Tick(45_000), Some(q)).unwrap();
        let f1 = c.read_video_frame(Tick(90_000), Some(q)).unwrap();
        assert_ne!(f0.rgba, f1.rgba, "the ticker must move");
        let cx = |f: &VideoFrame| -> f64 {
            // center of mass along X — where the block has moved
            let (mut sum, mut wsum) = (0f64, 0f64);
            for (i, p) in f.rgba.chunks_exact(4).enumerate() {
                let a = p[3] as f64;
                sum += a * (i % 240) as f64;
                wsum += a;
            }
            if wsum > 0.0 { sum / wsum } else { 0.0 }
        };
        assert!(cx(&f0) > cx(&f1), "the ticker must move right to left");

        // cache by Quality: a static title is computed ONCE, an animation —
        // for every frame (otherwise the motion would freeze)
        let plain = kt_model::parse_gen_path("gen:text:t=A").unwrap();
        assert!(!depends_on_time(&plain), "the title’s look does not depend on time");
        assert!(!GenState::new(plain.clone()).animated(), "without curves the title is static");
        assert!(
            GenState { kind: plain, auto: text_anim_curves(TextAnim::Typewriter, 2.0, 2.0) }
                .animated(),
            "a curve must turn off the single-element cache"
        );
    }

    /// t472: “no sound” is `Ok` silence, never `Err` (otherwise the worker's
    /// negative cache counts false underruns).
    #[test]
    fn t472_generator_audio_is_silence_not_err() {
        let mut s = solid(10, 20, 30);
        for idx in [0_i64, 7, 1_000_000] {
            let b = s.read_audio_burst(idx).unwrap();
            assert_eq!(b.idx, idx);
            assert!(b.data.iter().all(|v| *v == 0.0), "the generator must be silent");
        }
    }
}
