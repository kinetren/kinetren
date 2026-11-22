//! The audio-reactive generators (gen-audio-reactive, F-TXT-011, `v26.10` G5): a waveform, a
//! spectrum and level rings. Each hears an audio TRACK (its `track` row, 1-based): the host hands the
//! frame the values of the track's files at its moment (`audio_series::frame_values`), and the kind
//! draws them on the compute device from one storage buffer (`shader_frame_audio`). Heard nothing,
//! a kind draws silence: a flat line, empty bars, the rings at rest.

use std::sync::Arc;

use kt_plugin_api::audio_series::at;
use kt_plugin_api::generator::{
    count, kind, rgb, shader_frame_audio, smooth, u8v, GenDecl, PathTok, PathVal, Row, Tick,
};
use kt_plugin_api::{ParamUnit, Plugin};

/// The creation menu's group.
const REACT: &str = "genkind.group_reactive";

const COMMON: &str = include_str!("wgsl/common.wgsl");
const WAVEFORM: &str = concat!(include_str!("wgsl/common.wgsl"), include_str!("wgsl/waveform.wgsl"));
const SPECTRUM: &str = concat!(include_str!("wgsl/common.wgsl"), include_str!("wgsl/spectrum.wgsl"));
const RINGS: &str = concat!(include_str!("wgsl/common.wgsl"), include_str!("wgsl/levelrings.wgsl"));

/// The `pixel` texts of the family's kinds.
pub const SHADERS: [&str; 3] = [WAVEFORM, SPECTRUM, RINGS];

const WAVE_STYLES: &[&str] = &["line", "filled", "mirror"];
const SPECTRUM_STYLES: &[&str] = &["bars", "mirror"];

/// A fraction of the frame shown as a percentage.
fn pct(n: &'static str, lo: f64, hi: f64, d: f64) -> Row {
    smooth(n, lo, hi, d, 0.001, ParamUnit::Percent)
}

/// The audio track a kind hears (1-based).
fn track() -> Row {
    count("track", 1.0, 16.0, 1.0)
}

fn rgba4(st: [&'static str; 4], d: [f64; 4]) -> Vec<Row> {
    vec![rgb(st[0], d[0]), rgb(st[1], d[1]), rgb(st[2], d[2]), rgb(st[3], d[3])]
}

fn rgba_at(p: &[f64], i: usize) -> [f32; 4] {
    [u8v(p[i]) as f32, u8v(p[i + 1]) as f32, u8v(p[i + 2]) as f32, u8v(p[i + 3]) as f32]
}

/// A feather in pixels: a fraction of the height, at least 1/256 px.
fn feather_px(f: f64, h: u32) -> f32 {
    (f * h.max(1) as f64).max(1.0 / 256.0) as f32
}

const fn decl(path: &'static [PathTok], colors: &'static [(&'static str, &'static str)]) -> GenDecl {
    // transparent over what lies below, and moving by itself with the sound
    GenDecl { alpha: true, live: true, blank: false, path, address: &[], menu: &[], names: None, colors }
}

const SILENT: [f32; at::LEN] = [0.0; at::LEN];

/// A waveform (rows: track, cy, height, thickness, gain, style, feather, colour×4).
fn waveform_heard(p: &[f64], _: Tick, w: u32, h: u32, au: &[f32; at::LEN]) -> Box<[u8]> {
    let fh = h.max(1) as f32;
    let v = [
        rgba_at(p, 7),
        [p[1] as f32 * fh, p[2] as f32 * fh * 0.5, (p[3] as f32 * fh).max(1.0), p[4] as f32],
        [p[5] as f32, feather_px(p[6], h), 0.0, 0.0],
        [0.0; 4],
    ];
    shader_frame_audio("generator:waveform", WAVEFORM, w, h, [0, 0], v, au)
}

/// A spectrum (rows: track, left, bottom, width, height, gap, hold, style, feather, colour×4,
/// top colour×4).
fn spectrum_heard(p: &[f64], _: Tick, w: u32, h: u32, au: &[f32; at::LEN]) -> Box<[u8]> {
    let (fw, fh) = (w.max(1) as f32, h.max(1) as f32);
    let v = [
        rgba_at(p, 9),
        [p[1] as f32 * fw, p[2] as f32 * fh, p[3] as f32 * fw, p[4] as f32 * fh],
        [p[5] as f32, p[6] as f32, p[7] as f32, feather_px(p[8], h)],
        rgba_at(p, 13),
    ];
    shader_frame_audio("generator:spectrum", SPECTRUM, w, h, [0, 0], v, au)
}

/// Level rings (rows: track, cx, cy, radius, react, rings, spacing, thickness, feather, colour×4).
fn rings_heard(p: &[f64], _: Tick, w: u32, h: u32, au: &[f32; at::LEN]) -> Box<[u8]> {
    let (fw, fh) = (w.max(1) as f32, h.max(1) as f32);
    let v = [
        rgba_at(p, 9),
        [p[1] as f32 * fw, p[2] as f32 * fh, p[3] as f32 * fh, p[4] as f32 * fh],
        [p[5] as f32, p[6] as f32 * fh, (p[7] as f32 * fh).max(1.0), feather_px(p[8], h)],
        [0.0; 4],
    ];
    shader_frame_audio("generator:levelrings", RINGS, w, h, [0, 0], v, au)
}

fn waveform(p: &[f64], t: Tick, w: u32, h: u32) -> Box<[u8]> {
    waveform_heard(p, t, w, h, &SILENT)
}
fn spectrum(p: &[f64], t: Tick, w: u32, h: u32) -> Box<[u8]> {
    spectrum_heard(p, t, w, h, &SILENT)
}
fn rings(p: &[f64], t: Tick, w: u32, h: u32) -> Box<[u8]> {
    rings_heard(p, t, w, h, &SILENT)
}

const SPECTRUM_COLORS: &[(&str, &str)] = &[("color", "genp.bottom_color"), ("color2", "genp.top_color")];
const ONE_COLOR: &[(&str, &str)] = &[("color", "trn.color")];

pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    let _ = COMMON;
    vec![
        Arc::new(
            kind(
                "waveform",
                "genkind.waveform",
                REACT,
                [
                    vec![
                        track(),
                        pct("cy", 0.0, 1.0, 0.5),
                        pct("height", 0.02, 1.0, 0.3),
                        pct("thickness", 0.001, 0.05, 0.006),
                        smooth("gain", 0.25, 8.0, 1.0, 0.05, ParamUnit::Ratio),
                        Row::Choice("style", WAVE_STYLES),
                        pct("feather", 0.0, 0.05, 0.001),
                    ],
                    rgba4(["color_r", "color_g", "color_b", "color_a"], [255.0, 255.0, 255.0, 255.0]),
                ]
                .into_iter()
                .flatten()
                .collect(),
                decl(
                    &[
                        PathTok { key: "tr", val: PathVal::Int("track", 1, 16) },
                        PathTok { key: "y", val: PathVal::Unit("cy") },
                        PathTok { key: "h", val: PathVal::Unit("height") },
                        PathTok { key: "th", val: PathVal::Unit("thickness") },
                        PathTok { key: "g", val: PathVal::Num("gain") },
                        PathTok { key: "st", val: PathVal::Choice("style", WAVE_STYLES) },
                        PathTok { key: "fe", val: PathVal::Unit("feather") },
                        PathTok { key: "c", val: PathVal::Hex4("color") },
                    ],
                    ONE_COLOR,
                ),
                waveform,
            )
            .with_listen(waveform_heard, WAVEFORM),
        ),
        Arc::new(
            kind(
                "spectrum",
                "genkind.spectrum",
                REACT,
                [
                    vec![
                        track(),
                        pct("left", 0.0, 1.0, 0.1),
                        pct("bottom", 0.0, 1.0, 0.85),
                        pct("w", 0.05, 1.0, 0.8),
                        pct("h", 0.02, 1.0, 0.4),
                        smooth("gap", 0.0, 0.9, 0.25, 0.01, ParamUnit::Norm),
                        Row::Flag("hold", true),
                        Row::Choice("style", SPECTRUM_STYLES),
                        pct("feather", 0.0, 0.05, 0.001),
                    ],
                    rgba4(["color_r", "color_g", "color_b", "color_a"], [40.0, 200.0, 255.0, 255.0]),
                    rgba4(["color2_r", "color2_g", "color2_b", "color2_a"], [255.0, 60.0, 200.0, 255.0]),
                ]
                .into_iter()
                .flatten()
                .collect(),
                decl(
                    &[
                        PathTok { key: "tr", val: PathVal::Int("track", 1, 16) },
                        PathTok { key: "l", val: PathVal::Unit("left") },
                        PathTok { key: "b", val: PathVal::Unit("bottom") },
                        PathTok { key: "w", val: PathVal::Span("w") },
                        PathTok { key: "h", val: PathVal::Span("h") },
                        PathTok { key: "gap", val: PathVal::Unit("gap") },
                        PathTok { key: "hold", val: PathVal::Flag0("hold") },
                        PathTok { key: "st", val: PathVal::Choice("style", SPECTRUM_STYLES) },
                        PathTok { key: "fe", val: PathVal::Unit("feather") },
                        PathTok { key: "c", val: PathVal::Hex4("color") },
                        PathTok { key: "c2", val: PathVal::Hex4("color2") },
                    ],
                    SPECTRUM_COLORS,
                ),
                spectrum,
            )
            .with_listen(spectrum_heard, SPECTRUM),
        ),
        Arc::new(
            kind(
                "levelrings",
                "genkind.levelrings",
                REACT,
                [
                    vec![
                        track(),
                        pct("cx", 0.0, 1.0, 0.5),
                        pct("cy", 0.0, 1.0, 0.5),
                        pct("radius", 0.0, 1.0, 0.12),
                        pct("react", 0.0, 1.0, 0.2),
                        count("rings", 1.0, 8.0, 3.0),
                        pct("spacing", 0.0, 0.3, 0.05),
                        pct("thickness", 0.001, 0.1, 0.008),
                        pct("feather", 0.0, 0.05, 0.002),
                    ],
                    rgba4(["color_r", "color_g", "color_b", "color_a"], [255.0, 210.0, 60.0, 255.0]),
                ]
                .into_iter()
                .flatten()
                .collect(),
                decl(
                    &[
                        PathTok { key: "tr", val: PathVal::Int("track", 1, 16) },
                        PathTok { key: "x", val: PathVal::Unit("cx") },
                        PathTok { key: "y", val: PathVal::Unit("cy") },
                        PathTok { key: "r", val: PathVal::Unit("radius") },
                        PathTok { key: "re", val: PathVal::Unit("react") },
                        PathTok { key: "n", val: PathVal::Int("rings", 1, 8) },
                        PathTok { key: "sp", val: PathVal::Unit("spacing") },
                        PathTok { key: "th", val: PathVal::Unit("thickness") },
                        PathTok { key: "fe", val: PathVal::Unit("feather") },
                        PathTok { key: "c", val: PathVal::Hex4("color") },
                    ],
                    ONE_COLOR,
                ),
                rings,
            )
            .with_listen(rings_heard, RINGS),
        ),
    ]
}
