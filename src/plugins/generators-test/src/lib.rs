//! The signal-check and countdown generators (plugin-api-stage1, stage 6, D13): SMPTE bars with a
//! tone, the running timecode, the countdown leader with its beep, the five engineering tables and
//! the level window. Each kind is its former `GeneratorKind` variant: the passport rows in the
//! variant's field order with its stores (the file text does not move — t973), the `gen:` path
//! tokens of the former `format_gen_path` / `apply_token` arms, the frame of the former `render_at`
//! arm and the sound of the former `read_audio_burst` arm, verbatim (t1048). The sine is the host's
//! one `tone_1k`: a kind gives its level and its gate.

use std::sync::Arc;

use kt_plugin_api::generator::{shader_frame, GenDecl, PathTok, PathVal, Row, Tick, count, kind, smooth};
use kt_plugin_api::pixfont::{Canvas, text_width};
use kt_plugin_api::{ParamUnit, Plugin};

/// The creation menu's groups.
const CHECK: &str = "genkind.group_check";
const TITLES: &str = "genkind.group_titles";

/// A kind's declarations: no address, one menu row of defaults, no value-named row, no colours.
const fn decl(live: bool, path: &'static [PathTok]) -> GenDecl {
    GenDecl { alpha: false, live, blank: false, path, address: &[], menu: &[], names: None, colors: &[] }
}

// ---- SMPTE bars ---------------------------------------------------------------------------------

/// The `pixel` text of the leader's shader.
pub const LEADER_SHADER: &str = include_str!("wgsl/leader.wgsl");

fn smpte(_: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let n = (w as usize) * (h as usize);
    // a simplified table: the upper 2/3 are seven 75% bars,
    // the lower third are reference blocks (−I, white 100%, +Q, black, PLUGE);
    // the values are deterministic and frozen by the test
    const TOP: [[u8; 3]; 7] =
        [[191, 191, 191], [191, 191, 0], [0, 191, 191], [0, 191, 0], [191, 0, 191], [191, 0, 0], [0, 0, 191]];
    const BOTTOM: [[u8; 3]; 5] = [[0, 33, 76], [255, 255, 255], [50, 0, 106], [0, 0, 0], [16, 16, 16]];
    let mut buf = vec![0u8; n * 4];
    let split = (h as usize * 2) / 3;
    for y in 0..h as usize {
        let (bands, m): (&[[u8; 3]], usize) = if y < split { (&TOP, 7) } else { (&BOTTOM, 5) };
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

/// The 1 kHz tone at `tone_db` while `tone` is on (rows: tone, tone_db).
fn smpte_tone(p: &[f64]) -> Option<f64> {
    (p[0] >= 0.5).then_some(p[1])
}

// ---- timecode -----------------------------------------------------------------------------------

/// Timecode with RUNNING digits: the time of the MATERIAL (the source does not see the timeline
/// clock by construction).
fn timecode(p: &[f64], pts: Tick, w: u32, h: u32) -> Box<[u8]> {
    let fps = (p[0] as u32).max(1) as i64;
    let t = pts.0.max(0);
    let secs = t / 90_000;
    let ff = (t % 90_000) * fps / 90_000;
    let label = format!("{:02}:{:02}:{:02}:{:02}", secs / 3600, secs % 3600 / 60, secs % 60, ff);
    let mut c = Canvas::new(w.max(1), h.max(1), [0, 0, 0, 255]);
    let chars = label.chars().count();
    let s = ((c.w * 4 / 5) / text_width(chars, 1).max(1)).clamp(1, 40);
    let tx = (c.w - text_width(chars, s)) / 2;
    let ty = (c.h - 5 * s) / 2;
    c.text(tx, ty, s, &label, [235, 235, 235, 255]);
    c.buf.into_boxed_slice()
}

// ---- the countdown leader -----------------------------------------------------------------------

/// A countdown frame (R.53, item 421): a cross, a circle, a sweeping
/// sector and the digit of the seconds. The time is the MATERIAL’s pts, as for timecode.
///
/// The last frame before zero is a “2-pop”: a white field EXACTLY `1/fps` long.
fn leader(p: &[f64], pts: Tick, w: u32, h: u32) -> Box<[u8]> {
    let (seconds, fps) = (p[0] as u32, (p[1] as u32).max(1));
    let (w, h) = (w.max(1), h.max(1));
    let total = seconds.max(1) as i64 * 90_000;
    let t = pts.0.clamp(0, total);
    let left = total - t; // how much is left until zero, in ticks
    let frame_ticks = 90_000 / fps as i64;
    let (fw, fh) = (w as i64, h as i64);
    let mut c = Canvas::new(w, h, [16, 16, 16, 255]);
    // 2-pop: exactly one frame of the white field before zero
    if left <= frame_ticks {
        for px in c.buf.chunks_exact_mut(4) {
            px.copy_from_slice(&[235, 235, 235, 255]);
        }
        return c.buf.into_boxed_slice();
    }
    let radius = fw.min(fh) * 2 / 5;
    // a cross across the whole frame: two bars 1/200 of the smaller side thick; the ring of the
    // same thickness; the sector sweeping clockwise (the quarter and how far into it)
    let bar = (fw.min(fh) / 200).max(1);
    let phase = ((left % 90_000) * 4) / 90_000;
    let rem = (left % 90_000) * 4 - phase * 90_000;
    let geometry = [(fw / 2) as f32, (fh / 2) as f32, radius as f32, bar as f32];
    let frame = shader_frame("generator:leader", LEADER_SHADER, w, h, [phase as u32, rem as u32], [geometry, [0.0; 4], [0.0; 4], [0.0; 4]]);
    c.buf = frame.into_vec();
    // the seconds digit: how many FULL seconds are left, at least 1
    let digit = ((left + 89_999) / 90_000).max(1).to_string();
    let chars = digit.chars().count();
    let s = ((c.h / 3) / 5).clamp(1, 60);
    let tx = (c.w - text_width(chars, s)) / 2;
    let ty = (c.h - 5 * s) / 2;
    c.text(tx, ty, s, &digit, [245, 245, 245, 255]);
    c.buf.into_boxed_slice()
}

/// The countdown beep at `beep_db` while `beep` is on (rows: seconds, fps, beep, beep_db).
fn leader_tone(p: &[f64]) -> Option<f64> {
    (p[2] >= 0.5).then_some(p[3])
}

/// The countdown beep (R.53, item 421): the SAME tone as SMPTE's, only with a gate — it sounds
/// exactly on whole seconds, 1/fps long, and is silent the rest of the time. The phase is the
/// host's (the GLOBAL sample number), so seek does not click.
fn leader_gate(p: &[f64], n: i64) -> bool {
    let total = (p[0] as u32).max(1) as i64 * 90_000;
    let step = 90_000 / (p[1] as u32).max(1) as i64;
    // sample number → material tick (48 kHz → 90 kHz, exact)
    let t = n * 90_000 / 48_000;
    if t < 0 || t > total {
        return false;
    }
    // a beep at the START of each second of the material, 1/fps long,
    // and the same one on the “2-pop” (the end of the countdown is also a whole second)
    t % 90_000 < step || total - t <= step
}

// ---- the engineering tables ---------------------------------------------------------------------

/// The table sorts as the file writes them, and as the path names them.
const TABLES: &[&str] = &["ebu75", "ebu100", "grayscale", "steps10", "ramp_ycbcr"];
const TABLE_TOKENS: &[&str] = &["ebu75", "ebu100", "grey", "steps", "ycbcr"];

/// Engineering table (R.53, item 422). The numbers are frozen by test t531.
///
/// 75% bars: eight vertical columns in descending order of brightness
/// (white, yellow, cyan, green, magenta, red, blue, black);
/// 75% of the 8-bit scale is 191. `[NOT VERIFIED]` against the paper
/// EBU 3213: the original source is paid, so the values are taken from the common
/// table and frozen here.
fn table(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let t = p[0].round().clamp(0.0, 4.0) as usize;
    let (fw, fh) = (w.max(1) as usize, h.max(1) as usize);
    let mut buf = vec![0u8; fw * fh * 4];
    match t {
        // ebu75, ebu100
        0 | 1 => {
            let v: u8 = if t == 0 { 191 } else { 255 };
            let bars: [[u8; 3]; 8] =
                [[v, v, v], [v, v, 0], [0, v, v], [0, v, 0], [v, 0, v], [v, 0, 0], [0, 0, v], [0, 0, 0]];
            for y in 0..fh {
                for x in 0..fw {
                    let c = bars[(x * 8 / fw).min(7)];
                    let o = (y * fw + x) * 4;
                    buf[o..o + 3].copy_from_slice(&c);
                    buf[o + 3] = 255;
                }
            }
        }
        // the grey ramp, ten grey steps
        2 | 3 => {
            for y in 0..fh {
                for x in 0..fw {
                    let g = if t == 2 {
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
        _ => {
            // the BT.709 chroma plane at constant luminance: Cb along X,
            // Cr along Y. The coefficients are in the preamble (tables by columns and
            // rows), the pixel loop is integer.
            let cb: Vec<i32> = (0..fw).map(|x| (16 + (x * 224 / (fw - 1).max(1))) as i32 - 128).collect();
            let cr: Vec<i32> = (0..fh).map(|y| (16 + (y * 224 / (fh - 1).max(1))) as i32 - 128).collect();
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

// ---- the level window ---------------------------------------------------------------------------

/// A white window of a given SHARE OF THE FRAME AREA on black — APL measurement and panel warm-up.
fn window(p: &[f64], _: Tick, w: u32, h: u32) -> Box<[u8]> {
    let n = (w as usize) * (h as usize);
    let (area, level) = (p[0], p[1]);
    let (fw, fh) = (w.max(1) as i64, h.max(1) as i64);
    let lv = (level.clamp(0.0, 1.0) * 255.0).round() as u8;
    // the share of AREA → the side in fractions of the frame: the window is similar to the frame
    let side = area.clamp(0.0, 1.0).sqrt();
    let (hw, hh) = (((side * fw as f64) / 2.0).round() as i64, ((side * fh as f64) / 2.0).round() as i64);
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

/// A level in dBFS (the tone, the beep).
fn db(n: &'static str) -> Row {
    smooth(n, -60.0, 0.0, -20.0, 0.5, ParamUnit::Db)
}

/// The family's kinds, in `BUILTIN_IDS` order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        // SMPTE bars at 75% (+ reference marks in the bottom third); the 1 kHz tone makes the media
        // audible (level in dBFS)
        Arc::new(
            kind(
                "smpte",
                "genkind.smpte",
                CHECK,
                vec![Row::Flag("tone", false), db("tone_db")],
                decl(
                    false,
                    &[
                        PathTok { key: "tone", val: PathVal::Flag1("tone") },
                        PathTok { key: "db", val: PathVal::Num("tone_db") },
                    ],
                ),
                smpte,
            )
            .with_tone(smpte_tone, None),
        ),
        // timecode with RUNNING digits — the time of the MATERIAL
        Arc::new(kind(
            "timecode",
            "genkind.timecode",
            TITLES,
            vec![count("fps", 1.0, 120.0, 30.0)],
            decl(true, &[PathTok { key: "fps", val: PathVal::Int("fps", 1, 120) }]),
            timecode,
        )),
        // the countdown (item 421): the "2-pop" is exactly 1/fps long, the beep is the SMPTE tone's code
        Arc::new(
            kind(
                "leader",
                "genkind.leader",
                TITLES,
                vec![count("seconds", 1.0, 60.0, 8.0), count("fps", 1.0, 120.0, 30.0), Row::Flag("beep", false), db("beep_db")],
                decl(
                    true,
                    &[
                        PathTok { key: "n", val: PathVal::Int("seconds", 1, 60) },
                        PathTok { key: "fps", val: PathVal::Int("fps", 1, 120) },
                        PathTok { key: "beep", val: PathVal::Flag1("beep") },
                        PathTok { key: "db", val: PathVal::Num("beep_db") },
                    ],
                ),
                leader,
            )
            .with_tone(leader_tone, Some(leader_gate)),
        ),
        // the engineering tables (item 422): ONE kind, the sort is a field — part of the media
        // address and of the name, five rows of the creation menu
        Arc::new(kind(
            "table",
            "genkind.ebu75",
            CHECK,
            vec![Row::Choice("table", TABLES)],
            GenDecl {
                alpha: false,
                live: false,
                blank: false,
                path: &[PathTok { key: "tb", val: PathVal::Choice("table", TABLE_TOKENS) }],
                address: &["table"],
                menu: &[&[("table", 0.0)], &[("table", 1.0)], &[("table", 2.0)], &[("table", 3.0)], &[("table", 4.0)]],
                names: Some((
                    "table",
                    &["genkind.ebu75", "genkind.ebu100", "genkind.grayscale", "genkind.steps10", "genkind.ycbcr"],
                )),
                colors: &[],
            },
            table,
        )),
        // the level window (item 422): its numbers are its own, hence the kind is its own
        Arc::new(kind(
            "window",
            "genkind.window",
            CHECK,
            vec![
                smooth("area", 0.0, 1.0, 0.1, 0.005, ParamUnit::Percent),
                smooth("level", 0.0, 1.0, 1.0, 0.005, ParamUnit::Percent),
            ],
            decl(
                false,
                &[
                    PathTok { key: "ar", val: PathVal::Unit("area") },
                    PathTok { key: "lv", val: PathVal::Unit("level") },
                ],
            ),
            window,
        )),
    ]
}
