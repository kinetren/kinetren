//! t973 (plugin-api-stage1, D3; capability `plugins`): moving a kind into a plugin does not change a
//! byte of the project file, and an effect this build does not register is kept and written back.
//!
//! The FROZEN ORACLE below is a verbatim copy of the derive of every kind moved so far (the t643
//! technique): the build after the move must write exactly the text the build before wrote.

use kt_model::project_file::{from_text, to_text};
use kt_model::*;
use kt_time::Tick;
use serde::Serialize;

/// Today's derive output of the moved kinds — copied from `FxInstance` before each move, never
/// edited afterwards. Stage 2.4 adds `posterize`; each later stage adds its kinds.
/// `FxInstance::CornerPin`'s fields before 4.1c (f64 corners, each skipped at its default) — a
/// struct, so a case can change one corner.
#[derive(Serialize, Clone, Copy)]
struct Pin {
    #[serde(skip_serializing_if = "is_zero_f64")]
    x0: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    y0: f64,
    #[serde(skip_serializing_if = "is_one_f64")]
    x1: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    y1: f64,
    #[serde(skip_serializing_if = "is_zero_f64")]
    x2: f64,
    #[serde(skip_serializing_if = "is_one_f64")]
    y2: f64,
    #[serde(skip_serializing_if = "is_one_f64")]
    x3: f64,
    #[serde(skip_serializing_if = "is_one_f64")]
    y3: f64,
}

const PIN: Pin = Pin { x0: 0.0, y0: 0.0, x1: 1.0, y1: 0.0, x2: 0.0, y2: 1.0, x3: 1.0, y3: 1.0 };

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Frozen {
    /// `FxInstance::Posterize` before 2.4 (`#[serde(default = "default_levels")] levels: u32`).
    Posterize { levels: u32 },
    /// `FxInstance::StereoWidth` before stage 3 (`#[serde(default = "default_one_f32")] width: f32`).
    StereoWidth { width: f32 },
    /// The three below before stage 3, field for field (`skip_serializing_if` copied as is).
    Eq {
        #[serde(skip_serializing_if = "is_zero")]
        low_gain_db: f32,
        low_freq_hz: f32,
        #[serde(skip_serializing_if = "is_zero")]
        mid_gain_db: f32,
        mid_freq_hz: f32,
        #[serde(skip_serializing_if = "is_one")]
        mid_q: f32,
        #[serde(skip_serializing_if = "is_zero")]
        high_gain_db: f32,
        high_freq_hz: f32,
    },
    Distortion {
        drive_db: f32,
        tone: f32,
        #[serde(skip_serializing_if = "is_one")]
        mix: f32,
    },
    Bitcrusher {
        bits: f32,
        rate_hz: f32,
        #[serde(skip_serializing_if = "is_one")]
        mix: f32,
    },
    /// The dynamics family before stage 3 (`Gain.db` had no serde default: a file always wrote it).
    Gain {
        db: f32,
        #[serde(skip_serializing_if = "is_zero")]
        pan: f32,
    },
    Gate { threshold_db: f32, attack_ms: f32, hold_ms: f32, release_ms: f32 },
    Limiter { threshold_db: f32, ceiling_db: f32, lookahead_ms: f32, release_ms: f32 },
    /// The modulation family before stage 3 (every field always written).
    Tremolo { rate_hz: f32, depth: f32 },
    AutoPan { rate_hz: f32, depth: f32 },
    Vibrato { rate_hz: f32, depth_ms: f32 },
    Chorus { rate_hz: f32, depth_ms: f32, mix: f32, voices: u32 },
    Flanger { rate_hz: f32, depth_ms: f32, feedback: f32, mix: f32 },
    Phaser { rate_hz: f32, depth: f32, feedback: f32, stages: u32, mix: f32 },
    /// `PitchShift` before stage 3 (both fields `skip_serializing_if = "is_zero"`).
    PitchShift {
        #[serde(skip_serializing_if = "is_zero")]
        semitones: f32,
        #[serde(skip_serializing_if = "is_zero")]
        cents: f32,
    },
    /// `Vignette` before stage 4 (no field was skipped).
    Vignette { strength: f32, radius: f32, softness: f32 },
    /// The five warps before 4.1c (`skip_serializing_if` copied as is).
    LensDistort {
        k1: f32,
        #[serde(skip_serializing_if = "is_zero")]
        k2: f32,
    },
    Twirl { angle: f32, radius: f32 },
    Ripple {
        amount: f32,
        freq: f32,
        #[serde(skip_serializing_if = "is_zero")]
        phase: f32,
    },
    Kaleidoscope {
        segments: u32,
        #[serde(skip_serializing_if = "is_zero")]
        angle: f32,
    },
    Cylinder3d { curvature: f32 },
    CornerPin(Pin),
    /// `Mirror` and `EventGrid` before 4.1c (every field always written).
    Mirror { horizontal: bool, vertical: bool },
    /// `Pan3d` before 4.1c-4: nine f64 rows always written, the pinned camera a bare id, omitted at none.
    Pan3d {
        pos_x: f64,
        pos_y: f64,
        pos_z: f64,
        rot_x: f64,
        rot_y: f64,
        rot_z: f64,
        anchor_x: f64,
        anchor_y: f64,
        scale: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        camera: Option<u64>,
    },
    /// `SlowedReverb` before 3b.2 (its legacy `automation` is empty in every file we write).
    SlowedReverb {
        rate: f64,
        mix: f32,
        reverb_delay_ms: f32,
        damp: f32,
        decay: f32,
        wet_gain: f32,
        #[serde(skip_serializing_if = "is_zero")]
        predelay_ms: f32,
        #[serde(skip_serializing_if = "Option::is_none")]
        frames: Option<&'static str>,
        #[serde(skip_serializing_if = "is_one")]
        width: f32,
    },
    /// `Compressor` before 3b.1: the key is a `TrackId` (a bare number), omitted when none.
    Compressor {
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        #[serde(skip_serializing_if = "is_zero")]
        makeup_db: f32,
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<u64>,
    },
    EventGrid { cols: u32, rows: u32, center: bool },
    /// `ColorGrade` and `Duotone` before stage 4.
    ColorGrade {
        #[serde(skip_serializing_if = "is_zero")]
        brightness: f32,
        #[serde(skip_serializing_if = "is_zero")]
        contrast: f32,
        #[serde(skip_serializing_if = "is_one")]
        saturation: f32,
        #[serde(skip_serializing_if = "is_zero")]
        hue: f32,
    },
    Duotone { shadow_r: u8, shadow_g: u8, shadow_b: u8, high_r: u8, high_g: u8, high_b: u8, amount: f32 },
    /// `Filter` before stage 3: its `mode` was the enum `FilterMode` (snake_case keys), always written.
    Filter { mode: FrozenFilterMode, cutoff_hz: f32, resonance: f32 },
    /// `Delay` before stage 3: `pingpong` was `skip_serializing_if = "Not::not"`.
    Delay {
        time_ms: f32,
        feedback: f32,
        mix: f32,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        pingpong: bool,
    },
}

/// `kt_model::FilterMode` before stage 3, verbatim.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum FrozenFilterMode {
    Lowpass,
    Highpass,
    Bandpass,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

fn is_one_f64(v: &f64) -> bool {
    *v == 1.0
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}

/// A file that leaves every field out read the variant's serde defaults; the plugin reads the same.
fn empty_reads() -> Vec<(&'static str, String)> {
    vec![
        (r#"{"kind":"eq"}"#, frozen(Frozen::Eq {
            low_gain_db: 0.0, low_freq_hz: 200.0, mid_gain_db: 0.0, mid_freq_hz: 1000.0,
            mid_q: 1.0, high_gain_db: 0.0, high_freq_hz: 4000.0,
        })),
        (r#"{"kind":"distortion"}"#, frozen(Frozen::Distortion { drive_db: 12.0, tone: 0.5, mix: 1.0 })),
        (r#"{"kind":"bitcrusher"}"#, frozen(Frozen::Bitcrusher { bits: 8.0, rate_hz: 12000.0, mix: 1.0 })),
        (r#"{"kind":"stereo_width"}"#, frozen(Frozen::StereoWidth { width: 1.0 })),
        (r#"{"kind":"gate"}"#, frozen(Frozen::Gate { threshold_db: -40.0, attack_ms: 1.0, hold_ms: 40.0, release_ms: 100.0 })),
        (r#"{"kind":"limiter"}"#, frozen(Frozen::Limiter { threshold_db: -6.0, ceiling_db: -0.3, lookahead_ms: 5.0, release_ms: 80.0 })),
        // a file without `pan` read 0 (`#[serde(default)]`)
        (r#"{"kind":"gain","db":-3.0}"#, frozen(Frozen::Gain { db: -3.0, pan: 0.0 })),
        (r#"{"kind":"vignette"}"#, frozen(Frozen::Vignette { strength: 0.5, radius: 0.7, softness: 0.45 })),
        (r#"{"kind":"lens_distort"}"#, frozen(Frozen::LensDistort { k1: 0.35, k2: 0.0 })),
        (r#"{"kind":"twirl"}"#, frozen(Frozen::Twirl { angle: 120.0, radius: 0.8 })),
        (r#"{"kind":"ripple"}"#, frozen(Frozen::Ripple { amount: 0.08, freq: 10.0, phase: 0.0 })),
        (r#"{"kind":"kaleidoscope"}"#, frozen(Frozen::Kaleidoscope { segments: 6, angle: 0.0 })),
        (r#"{"kind":"cylinder3d"}"#, frozen(Frozen::Cylinder3d { curvature: 0.5 })),
        (r#"{"kind":"corner_pin"}"#, frozen(Frozen::CornerPin(PIN))),
        (r#"{"kind":"mirror"}"#, frozen(Frozen::Mirror { horizontal: true, vertical: false })),
        (
            r#"{"kind":"slowed_reverb","rate":0.82,"mix":0.1,"reverb_delay_ms":85.0}"#,
            frozen(Frozen::SlowedReverb { rate: 0.82, mix: 0.1, reverb_delay_ms: 85.0, damp: 0.2, decay: 0.84, wet_gain: 1.0, predelay_ms: 0.0, frames: None, width: 1.0 }),
        ),
        (r#"{"kind":"pan3d"}"#, frozen(pan3d_default(None))),
        (r#"{"kind":"pan3d","camera":3}"#, frozen(pan3d_default(Some(3)))),
        (
            r#"{"kind":"compressor"}"#,
            frozen(Frozen::Compressor { threshold_db: -18.0, ratio: 4.0, attack_ms: 10.0, release_ms: 150.0, makeup_db: 0.0, key: None }),
        ),
        (
            r#"{"kind":"compressor","key":7}"#,
            frozen(Frozen::Compressor { threshold_db: -18.0, ratio: 4.0, attack_ms: 10.0, release_ms: 150.0, makeup_db: 0.0, key: Some(7) }),
        ),
        (r#"{"kind":"mirror","vertical":true}"#, frozen(Frozen::Mirror { horizontal: true, vertical: true })),
        (r#"{"kind":"event_grid"}"#, frozen(Frozen::EventGrid { cols: 1, rows: 1, center: true })),
        (r#"{"kind":"event_grid","cols":3,"center":false}"#, frozen(Frozen::EventGrid { cols: 3, rows: 1, center: false })),
        (r#"{"kind":"corner_pin","x0":0.25,"y3":1.5}"#, frozen(Frozen::CornerPin(Pin { x0: 0.25, y3: 1.5, ..PIN }))),
        (r#"{"kind":"color_grade"}"#, frozen(Frozen::ColorGrade { brightness: 0.0, contrast: 0.0, saturation: 1.0, hue: 0.0 })),
        (r#"{"kind":"duotone"}"#, frozen(Frozen::Duotone { shadow_r: 24, shadow_g: 18, shadow_b: 64, high_r: 255, high_g: 236, high_b: 190, amount: 1.0 })),
        (r#"{"kind":"filter"}"#, frozen(Frozen::Filter { mode: FrozenFilterMode::Lowpass, cutoff_hz: 800.0, resonance: 0.707 })),
        (r#"{"kind":"delay"}"#, frozen(Frozen::Delay { time_ms: 350.0, feedback: 0.35, mix: 0.35, pingpong: false })),
        // an unknown mode key reads as the default, as `#[serde(default)]` would not (it refused):
        // the plugin is the more lenient reader, never a stricter one
        (r#"{"kind":"filter","mode":"bandpass","cutoff_hz":1200.0}"#, frozen(Frozen::Filter { mode: FrozenFilterMode::Bandpass, cutoff_hz: 1200.0, resonance: 0.707 })),
        (r#"{"kind":"delay","pingpong":true}"#, frozen(Frozen::Delay { time_ms: 350.0, feedback: 0.35, mix: 0.35, pingpong: true })),
        (r#"{"kind":"pitch_shift"}"#, frozen(Frozen::PitchShift { semitones: 0.0, cents: 0.0 })),
        (r#"{"kind":"tremolo"}"#, frozen(Frozen::Tremolo { rate_hz: 5.0, depth: 0.6 })),
        (r#"{"kind":"auto_pan"}"#, frozen(Frozen::AutoPan { rate_hz: 1.0, depth: 0.8 })),
        (r#"{"kind":"vibrato"}"#, frozen(Frozen::Vibrato { rate_hz: 5.0, depth_ms: 0.5 })),
        (r#"{"kind":"chorus"}"#, frozen(Frozen::Chorus { rate_hz: 0.8, depth_ms: 3.5, mix: 0.5, voices: 2 })),
        (r#"{"kind":"flanger"}"#, frozen(Frozen::Flanger { rate_hz: 0.25, depth_ms: 2.0, feedback: 0.5, mix: 0.5 })),
        (r#"{"kind":"phaser"}"#, frozen(Frozen::Phaser { rate_hz: 0.4, depth: 0.7, feedback: 0.3, stages: 4, mix: 0.5 })),
    ]
}

fn pan3d_default(camera: Option<u64>) -> Frozen {
    Frozen::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 0.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale: 1.0,
        camera,
    }
}

fn frozen(f: Frozen) -> String {
    serde_json::to_string(&f).unwrap()
}

/// The moved kinds, each with its default and a non-default value: (the slot, its frozen form).
// values with more digits than an f32 holds are the point: the file must keep the f32's form
#[allow(clippy::excessive_precision)]
fn moved_kinds() -> Vec<(FxInstance, String)> {
    kt_plugins_builtin::install();
    vec![
        (FxInstance::builtin("posterize"), frozen(Frozen::Posterize { levels: 6 })),
        (FxInstance::plugin_with("posterize", &[4.0]).unwrap(), frozen(Frozen::Posterize { levels: 4 })),
        (FxInstance::plugin_with("posterize", &[32.0]).unwrap(), frozen(Frozen::Posterize { levels: 32 })),
        (FxInstance::builtin("stereo_width"), frozen(Frozen::StereoWidth { width: 1.0 })),
        (FxInstance::builtin("eq"), frozen(Frozen::Eq {
            low_gain_db: 0.0, low_freq_hz: 200.0, mid_gain_db: 0.0, mid_freq_hz: 1000.0,
            mid_q: 1.0, high_gain_db: 0.0, high_freq_hz: 4000.0,
        })),
        (
            FxInstance::plugin_with("eq", &[-6.5, 120.0, 3.25, 2500.0, 0.7, 0.123456789, 8000.0]).unwrap(),
            frozen(Frozen::Eq {
                low_gain_db: -6.5, low_freq_hz: 120.0, mid_gain_db: 3.25, mid_freq_hz: 2500.0,
                mid_q: 0.7, high_gain_db: 0.123456789, high_freq_hz: 8000.0,
            }),
        ),
        (FxInstance::builtin("distortion"), frozen(Frozen::Distortion { drive_db: 12.0, tone: 0.5, mix: 1.0 })),
        (
            FxInstance::plugin_with("distortion", &[30.0, 0.25, 0.6]).unwrap(),
            frozen(Frozen::Distortion { drive_db: 30.0, tone: 0.25, mix: 0.6 }),
        ),
        (FxInstance::builtin("bitcrusher"), frozen(Frozen::Bitcrusher { bits: 8.0, rate_hz: 12000.0, mix: 1.0 })),
        (FxInstance::gain(0.0), frozen(Frozen::Gain { db: 0.0, pan: 0.0 })),
        (FxInstance::gain(-7.25), frozen(Frozen::Gain { db: -7.25, pan: 0.0 })),
        (
            FxInstance::plugin_with("gain", &[2.0, -0.123456789]).unwrap(),
            frozen(Frozen::Gain { db: 2.0, pan: -0.123456789 }),
        ),
        (FxInstance::builtin("gate"), frozen(Frozen::Gate { threshold_db: -40.0, attack_ms: 1.0, hold_ms: 40.0, release_ms: 100.0 })),
        (
            FxInstance::plugin_with("gate", &[-30.0, 0.5, 120.0, 250.0]).unwrap(),
            frozen(Frozen::Gate { threshold_db: -30.0, attack_ms: 0.5, hold_ms: 120.0, release_ms: 250.0 }),
        ),
        (FxInstance::builtin("vignette"), frozen(Frozen::Vignette { strength: 0.5, radius: 0.7, softness: 0.45 })),
        (FxInstance::builtin("lens_distort"), frozen(Frozen::LensDistort { k1: 0.35, k2: 0.0 })),
        (
            FxInstance::plugin_with("lens_distort", &[-0.5, 0.123456789]).unwrap(),
            frozen(Frozen::LensDistort { k1: -0.5, k2: 0.123456789 }),
        ),
        (FxInstance::builtin("twirl"), frozen(Frozen::Twirl { angle: 120.0, radius: 0.8 })),
        (FxInstance::plugin_with("twirl", &[-240.0, 1.2]).unwrap(), frozen(Frozen::Twirl { angle: -240.0, radius: 1.2 })),
        (FxInstance::builtin("ripple"), frozen(Frozen::Ripple { amount: 0.08, freq: 10.0, phase: 0.0 })),
        (
            FxInstance::plugin_with("ripple", &[0.15, 6.0, 137.5]).unwrap(),
            frozen(Frozen::Ripple { amount: 0.15, freq: 6.0, phase: 137.5 }),
        ),
        (FxInstance::builtin("kaleidoscope"), frozen(Frozen::Kaleidoscope { segments: 6, angle: 0.0 })),
        (
            FxInstance::plugin_with("kaleidoscope", &[20.0, 15.0]).unwrap(),
            frozen(Frozen::Kaleidoscope { segments: 20, angle: 15.0 }),
        ),
        (FxInstance::builtin("cylinder3d"), frozen(Frozen::Cylinder3d { curvature: 0.5 })),
        (FxInstance::plugin_with("cylinder3d", &[0.85]).unwrap(), frozen(Frozen::Cylinder3d { curvature: 0.85 })),
        (FxInstance::builtin("corner_pin"), frozen(Frozen::CornerPin(PIN))),
        (FxInstance::builtin("mirror"), frozen(Frozen::Mirror { horizontal: true, vertical: false })),
        (FxInstance::slowed_reverb(0.82, 0.1, 85.0), frozen(Frozen::SlowedReverb { rate: 0.82, mix: 0.1, reverb_delay_ms: 85.0, damp: 0.2, decay: 0.84, wet_gain: 1.0, predelay_ms: 0.0, frames: None, width: 1.0 })),
        (
            FxInstance::plugin_with("slowed_reverb", &[-0.5, 0.35, 120.0, 0.3, 0.9, 1.2, 15.0, 1.0, 0.123456789]).unwrap(),
            frozen(Frozen::SlowedReverb {
                rate: -0.5,
                mix: 0.35,
                reverb_delay_ms: 120.0,
                damp: 0.3,
                decay: 0.9,
                wet_gain: 1.2,
                predelay_ms: 15.0,
                frames: Some("blend"),
                width: 0.123456789,
            }),
        ),
        (FxInstance::builtin("pan3d"), frozen(pan3d_default(None))),
        (
            {
                let mut p = FxInstance::plugin_with("pan3d", &[10.5, -4.0, 120.0, 0.123456789, 25.0, -3.0, 0.25, 0.75, 1.1, 0.0])
                    .unwrap();
                assert!(p.set_pinned_camera(Some(kt_model::CameraId(9))), "pan3d has no camera row");
                p
            },
            frozen(Frozen::Pan3d {
                pos_x: 10.5,
                pos_y: -4.0,
                pos_z: 120.0,
                rot_x: 0.123456789,
                rot_y: 25.0,
                rot_z: -3.0,
                anchor_x: 0.25,
                anchor_y: 0.75,
                scale: 1.1,
                camera: Some(9),
            }),
        ),
        (
            FxInstance::builtin("compressor"),
            frozen(Frozen::Compressor { threshold_db: -18.0, ratio: 4.0, attack_ms: 10.0, release_ms: 150.0, makeup_db: 0.0, key: None }),
        ),
        (
            {
                let mut c = FxInstance::plugin_with("compressor", &[-30.0, 12.0, 3.0, 10.0, 0.123456789, 0.0]).unwrap();
                assert!(c.set_sidechain_key(Some(kt_model::TrackId(42))), "the compressor has no key row");
                c
            },
            frozen(Frozen::Compressor { threshold_db: -30.0, ratio: 12.0, attack_ms: 3.0, release_ms: 10.0, makeup_db: 0.123456789, key: Some(42) }),
        ),
        (FxInstance::plugin_with("mirror", &[0.0, 1.0]).unwrap(), frozen(Frozen::Mirror { horizontal: false, vertical: true })),
        (FxInstance::event_grid(), frozen(Frozen::EventGrid { cols: 2, rows: 2, center: true })),
        (
            FxInstance::plugin_with("event_grid", &[12.0, 30.0, 0.0]).unwrap(),
            frozen(Frozen::EventGrid { cols: 12, rows: 30, center: false }),
        ),
        (
            FxInstance::plugin_with("corner_pin", &[0.1, 0.0, 1.0, 0.123456789, 0.0, 1.0, 0.9, 1.0]).unwrap(),
            frozen(Frozen::CornerPin(Pin { x0: 0.1, y1: 0.123456789, x3: 0.9, ..PIN })),
        ),
        (
            FxInstance::plugin_with("vignette", &[0.9, 0.3, 0.123456789]).unwrap(),
            frozen(Frozen::Vignette { strength: 0.9, radius: 0.3, softness: 0.123456789 }),
        ),
        (FxInstance::builtin("color_grade"), frozen(Frozen::ColorGrade { brightness: 0.0, contrast: 0.0, saturation: 1.0, hue: 0.0 })),
        (
            FxInstance::plugin_with("color_grade", &[0.05, -0.3, 1.4, 0.123456789 * 100.0]).unwrap(),
            frozen(Frozen::ColorGrade { brightness: 0.05, contrast: -0.3, saturation: 1.4, hue: 12.345679 }),
        ),
        (FxInstance::builtin("duotone"), frozen(Frozen::Duotone { shadow_r: 24, shadow_g: 18, shadow_b: 64, high_r: 255, high_g: 236, high_b: 190, amount: 1.0 })),
        (
            FxInstance::plugin_with("duotone", &[40.0, 0.0, 80.4, 255.0, 220.6, 160.0, 0.7]).unwrap(),
            frozen(Frozen::Duotone { shadow_r: 40, shadow_g: 0, shadow_b: 80, high_r: 255, high_g: 221, high_b: 160, amount: 0.7 }),
        ),
        (FxInstance::builtin("filter"), frozen(Frozen::Filter { mode: FrozenFilterMode::Lowpass, cutoff_hz: 800.0, resonance: 0.707 })),
        (
            FxInstance::plugin_with("filter", &[1.0, 2500.0, 4.5]).unwrap(),
            frozen(Frozen::Filter { mode: FrozenFilterMode::Highpass, cutoff_hz: 2500.0, resonance: 4.5 }),
        ),
        (
            FxInstance::plugin_with("filter", &[2.0, 0.123456789 * 1000.0, 0.5]).unwrap(),
            frozen(Frozen::Filter { mode: FrozenFilterMode::Bandpass, cutoff_hz: 123.456789, resonance: 0.5 }),
        ),
        (FxInstance::builtin("delay"), frozen(Frozen::Delay { time_ms: 350.0, feedback: 0.35, mix: 0.35, pingpong: false })),
        (
            FxInstance::plugin_with("delay", &[120.0, 0.8, 0.5, 1.0]).unwrap(),
            frozen(Frozen::Delay { time_ms: 120.0, feedback: 0.8, mix: 0.5, pingpong: true }),
        ),
        (FxInstance::builtin("pitch_shift"), frozen(Frozen::PitchShift { semitones: 0.0, cents: 0.0 })),
        (FxInstance::plugin_with("pitch_shift", &[7.0, 0.0]).unwrap(), frozen(Frozen::PitchShift { semitones: 7.0, cents: 0.0 })),
        (FxInstance::plugin_with("pitch_shift", &[-12.0, 33.5]).unwrap(), frozen(Frozen::PitchShift { semitones: -12.0, cents: 33.5 })),
        (FxInstance::builtin("tremolo"), frozen(Frozen::Tremolo { rate_hz: 5.0, depth: 0.6 })),
        (FxInstance::plugin_with("tremolo", &[12.5, 0.95]).unwrap(), frozen(Frozen::Tremolo { rate_hz: 12.5, depth: 0.95 })),
        (FxInstance::builtin("auto_pan"), frozen(Frozen::AutoPan { rate_hz: 1.0, depth: 0.8 })),
        (FxInstance::plugin_with("auto_pan", &[0.123456789, 0.1]).unwrap(), frozen(Frozen::AutoPan { rate_hz: 0.123456789, depth: 0.1 })),
        (FxInstance::builtin("vibrato"), frozen(Frozen::Vibrato { rate_hz: 5.0, depth_ms: 0.5 })),
        (FxInstance::plugin_with("vibrato", &[7.0, 4.25]).unwrap(), frozen(Frozen::Vibrato { rate_hz: 7.0, depth_ms: 4.25 })),
        (FxInstance::builtin("chorus"), frozen(Frozen::Chorus { rate_hz: 0.8, depth_ms: 3.5, mix: 0.5, voices: 2 })),
        (
            FxInstance::plugin_with("chorus", &[2.0, 9.5, 0.75, 3.0]).unwrap(),
            frozen(Frozen::Chorus { rate_hz: 2.0, depth_ms: 9.5, mix: 0.75, voices: 3 }),
        ),
        (FxInstance::builtin("flanger"), frozen(Frozen::Flanger { rate_hz: 0.25, depth_ms: 2.0, feedback: 0.5, mix: 0.5 })),
        (
            FxInstance::plugin_with("flanger", &[1.5, 6.0, -0.75, 0.9]).unwrap(),
            frozen(Frozen::Flanger { rate_hz: 1.5, depth_ms: 6.0, feedback: -0.75, mix: 0.9 }),
        ),
        (FxInstance::builtin("phaser"), frozen(Frozen::Phaser { rate_hz: 0.4, depth: 0.7, feedback: 0.3, stages: 4, mix: 0.5 })),
        (
            FxInstance::plugin_with("phaser", &[3.0, 0.2, 0.85, 7.0, 1.0]).unwrap(),
            frozen(Frozen::Phaser { rate_hz: 3.0, depth: 0.2, feedback: 0.85, stages: 7, mix: 1.0 }),
        ),
        (FxInstance::builtin("limiter"), frozen(Frozen::Limiter { threshold_db: -6.0, ceiling_db: -0.3, lookahead_ms: 5.0, release_ms: 80.0 })),
        (
            FxInstance::plugin_with("limiter", &[-12.0, -1.0, 12.5, 200.0]).unwrap(),
            frozen(Frozen::Limiter { threshold_db: -12.0, ceiling_db: -1.0, lookahead_ms: 12.5, release_ms: 200.0 }),
        ),
        (
            FxInstance::plugin_with("bitcrusher", &[3.5, 4410.0, 0.5]).unwrap(),
            frozen(Frozen::Bitcrusher { bits: 3.5, rate_hz: 4410.0, mix: 0.5 }),
        ),
        (FxInstance::plugin_with("stereo_width", &[0.0]).unwrap(), frozen(Frozen::StereoWidth { width: 0.0 })),
        // more digits than an f32 holds (a dragged or automated value): the variant's f32 field
        // wrote "0.12345679"; an f64 row would write "0.123456789" (red proof of the row's Store)
        (
            FxInstance::plugin_with("stereo_width", &[0.123456789]).unwrap(),
            frozen(Frozen::StereoWidth { width: 0.123456789 }),
        ),
    ]
}

/// The frozen variant also READ files that left out a defaulted field: the plugin reads them the same.
#[test]
fn t973_a_moved_kind_reads_the_old_defaults() {
    kt_plugins_builtin::install();
    for (file, want) in empty_reads() {
        let read: FxInstance = serde_json::from_str(file).unwrap();
        assert_eq!(serde_json::to_string(&read).unwrap(), want, "{file} does not read the variant's defaults");
    }
    let read: FxInstance = serde_json::from_str(r#"{"kind":"posterize"}"#).unwrap();
    // against the FROZEN default (6), not `posterize()`: a moved default would agree with itself
    assert_eq!(serde_json::to_string(&read).unwrap(), frozen(Frozen::Posterize { levels: 6 }), "a posterize without `levels` reads as the default 6");
    let clamped: FxInstance = serde_json::from_str(r#"{"kind":"posterize","levels":99}"#).unwrap();
    assert_eq!(
        serde_json::to_string(&clamped).unwrap(),
        r#"{"kind":"posterize","levels":99}"#,
        "an out-of-range value in a file is kept as read, as the variant kept it"
    );
}

fn project_with(fx: Vec<FxInstance>) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    let n = fx.len();
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: Vec::new(),
        fx_auto: vec![Vec::new(); n],
        opacity: 1.0,
        fx_off: false,
    });
    p
}

#[test]
fn t973_moving_a_kind_does_not_change_a_byte() {
    kt_plugins_builtin::install();
    for (slot, frozen) in moved_kinds() {
        let ours = serde_json::to_string(&slot).unwrap();
        assert_eq!(ours, frozen, "{}: the plugin slot writes other text than the variant did", slot.tag());
        // and it reads back into the same slot
        let back: FxInstance = serde_json::from_str(&frozen).unwrap();
        assert_eq!(back, slot, "{}: the variant's text reads into another slot", slot.tag());
    }
    // the whole file: written, read and written again — byte-identical
    let fx: Vec<FxInstance> = moved_kinds().into_iter().map(|(s, _)| s).collect();
    let text = to_text(&project_with(fx), None);
    let again = to_text(&from_text(&text).expect("reads").project, None);
    assert_eq!(text, again, "the second write differs from the first");
}

/// A chain `[blur, wobble_x, glow]` where `wobble_x` is not registered: the file opens, the slot
/// is `Missing` with its raw content, and the save writes the same bytes.
#[test]
fn t973_a_missing_plugin_keeps_its_bytes() {
    kt_plugins_builtin::install();
    let wobble = r#"{"kind":"wobble_x","amp":0.25,"freq":3,"mode":"sine"}"#;
    let fx = vec![
        FxInstance::blur(4),
        serde_json::from_str(wobble).expect("an unknown kind still reads"),
        FxInstance::glow(0.5),
    ];
    assert!(matches!(&fx[1], FxInstance::Missing(m) if m.id() == "wobble_x"), "{:?}", fx[1]);
    let text = to_text(&project_with(fx), None);
    assert!(text.contains("\"kind\": \"wobble_x\""), "the missing slot was not written: {text}");
    let read = from_text(&text).expect("a file with a missing plugin opens");
    let FxInstance::Missing(m) = &read.project.clips[0].fx[1] else { panic!("not kept as missing") };
    assert_eq!(serde_json::to_string(&m.raw).unwrap(), wobble, "the raw content changed");
    assert_eq!(to_text(&read.project, None), text, "the save changed the bytes");
}

/// `TransitionKind`'s moved kinds before stage 5 — copied from the derive, never edited afterwards
/// (`Dir` and `IrisShape` are written by their snake_case keys; the centre is skipped at 0.5).
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum FrozenTrans {
    Wipe { dir: &'static str, angle: f64, softness: f64 },
    Iris {
        shape: &'static str,
        #[serde(skip_serializing_if = "is_center")]
        cx: f64,
        #[serde(skip_serializing_if = "is_center")]
        cy: f64,
        softness: f64,
    },
    Clock {
        reverse: bool,
        #[serde(skip_serializing_if = "is_center")]
        cx: f64,
        #[serde(skip_serializing_if = "is_center")]
        cy: f64,
        softness: f64,
    },
    Barn { vertical: bool, softness: f64 },
    Push { dir: &'static str },
    Zoom {
        amount: f64,
        #[serde(skip_serializing_if = "is_center")]
        cx: f64,
        #[serde(skip_serializing_if = "is_center")]
        cy: f64,
    },
    Flash { color_r: u8, color_g: u8, color_b: u8, strength: f64 },
    Blinds { count: u32, angle: f64, softness: f64 },
    Split { vertical: bool },
    Whip { dir: &'static str, blur: f64 },
    BlurDissolve { amount: f64 },
    Shake { amount: f64, steps: u32 },
    FilmBurn { scale: f64, softness: f64, tint_r: u8, tint_g: u8, tint_b: u8 },
    ZoomBlur {
        amount: f64,
        #[serde(skip_serializing_if = "is_center")]
        cx: f64,
        #[serde(skip_serializing_if = "is_center")]
        cy: f64,
    },
    Swap { vertical: bool, depth: f64 },
    Flip { vertical: bool, persp: f64, shade: f64 },
    Cube { dir: &'static str, persp: f64, shade: f64 },
    PagePeel { dir: &'static str, curl: f64, shadow: f64 },
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_center(v: &f64) -> bool {
    *v == 0.5
}

/// t973 for transitions (plugin-api-stage1, D12): a moved transition kind writes the text its
/// variant wrote, reads the variant's text (defaults for missing fields), and a kind this build
/// lacks is kept and written back.
#[test]
fn t973_a_moved_transition_kind_keeps_its_text() {
    kt_plugins_builtin::install();
    let frozen = |f: FrozenTrans| serde_json::to_string(&f).unwrap();
    let cases = [
        (TransitionKind::builtin("wipe"), frozen(FrozenTrans::Wipe { dir: "left", angle: 0.0, softness: 0.15 })),
        (
            TransitionKind::plugin_with("wipe", &[2.0, 12.0, 0.123456789]).unwrap(),
            frozen(FrozenTrans::Wipe { dir: "up", angle: 12.0, softness: 0.123456789 }),
        ),
        (
            TransitionKind::builtin("iris"),
            frozen(FrozenTrans::Iris { shape: "circle", cx: 0.5, cy: 0.5, softness: 0.15 }),
        ),
        (
            TransitionKind::plugin_with("iris", &[2.0, 0.25, 0.5, 0.4]).unwrap(),
            frozen(FrozenTrans::Iris { shape: "box", cx: 0.25, cy: 0.5, softness: 0.4 }),
        ),
        (
            TransitionKind::plugin_with("clock", &[1.0, 0.7, 0.3, 0.0]).unwrap(),
            frozen(FrozenTrans::Clock { reverse: true, cx: 0.7, cy: 0.3, softness: 0.0 }),
        ),
        (
            TransitionKind::plugin_with("barn", &[1.0, 0.2]).unwrap(),
            frozen(FrozenTrans::Barn { vertical: true, softness: 0.2 }),
        ),
        (TransitionKind::plugin_with("push", &[3.0]).unwrap(), frozen(FrozenTrans::Push { dir: "down" })),
        (TransitionKind::builtin("zoom"), frozen(FrozenTrans::Zoom { amount: 0.4, cx: 0.5, cy: 0.5 })),
        (
            TransitionKind::plugin_with("flash", &[255.0, 200.0, 40.0, 1.0]).unwrap(),
            frozen(FrozenTrans::Flash { color_r: 255, color_g: 200, color_b: 40, strength: 1.0 }),
        ),
        (
            TransitionKind::plugin_with("blinds", &[5.0, 30.0, 0.0]).unwrap(),
            frozen(FrozenTrans::Blinds { count: 5, angle: 30.0, softness: 0.0 }),
        ),
        (TransitionKind::builtin("split"), frozen(FrozenTrans::Split { vertical: false })),
        (
            TransitionKind::plugin_with("whip", &[3.0, 1.0]).unwrap(),
            frozen(FrozenTrans::Whip { dir: "down", blur: 1.0 }),
        ),
        (TransitionKind::builtin("blur_dissolve"), frozen(FrozenTrans::BlurDissolve { amount: 24.0 })),
        (
            TransitionKind::plugin_with("shake", &[1.0, 3.0]).unwrap(),
            frozen(FrozenTrans::Shake { amount: 1.0, steps: 3 }),
        ),
        (
            TransitionKind::builtin("film_burn"),
            frozen(FrozenTrans::FilmBurn { scale: 6.0, softness: 0.2, tint_r: 255, tint_g: 120, tint_b: 0 }),
        ),
        (
            TransitionKind::plugin_with("zoom_blur", &[1.0, 0.2, 0.8]).unwrap(),
            frozen(FrozenTrans::ZoomBlur { amount: 1.0, cx: 0.2, cy: 0.8 }),
        ),
        (
            TransitionKind::plugin_with("swap", &[1.0, 0.7]).unwrap(),
            frozen(FrozenTrans::Swap { vertical: true, depth: 0.7 }),
        ),
        (
            TransitionKind::builtin("flip"),
            frozen(FrozenTrans::Flip { vertical: false, persp: 0.5, shade: 0.5 }),
        ),
        (
            TransitionKind::plugin_with("cube", &[2.0, 0.8, 0.3]).unwrap(),
            frozen(FrozenTrans::Cube { dir: "up", persp: 0.8, shade: 0.3 }),
        ),
        (
            TransitionKind::plugin_with("page_peel", &[3.0, 0.35, 1.0]).unwrap(),
            frozen(FrozenTrans::PagePeel { dir: "down", curl: 0.35, shadow: 1.0 }),
        ),
    ];
    for (kind, want) in &cases {
        assert_eq!(&serde_json::to_string(kind).unwrap(), want, "the moved kind's text changed");
        let back: TransitionKind = serde_json::from_str(want).unwrap();
        assert_eq!(&back, kind, "the variant's text reads otherwise");
    }
    // the variant's defaults: a file without the fields
    for id in [
        "wipe", "iris", "clock", "barn", "luma", "push", "slide", "zoom", "spin", "flash", "dip", "blinds",
        "checker", "blocks", "spiral", "split", "squeeze", "stretch", "whip", "blur_dissolve", "pixelate",
        "glitch", "shake", "film_burn", "light_leaks", "zoom_blur", "swap", "flip", "cube", "door", "page_peel",
        "page_roll",
    ] {
        let bare: TransitionKind = serde_json::from_str(&format!(r#"{{"kind":"{id}"}}"#)).unwrap();
        assert_eq!(bare, TransitionKind::builtin(id), "{id}: the variant's serde defaults");
    }
    // a kind this build lacks stays as it was
    let raw = r#"{"kind":"warp_tunnel","depth":0.5}"#;
    let missing: TransitionKind = serde_json::from_str(raw).unwrap();
    assert!(matches!(&missing, TransitionKind::Missing(m) if m.id() == "warp_tunnel"), "{missing:?}");
    assert_eq!(serde_json::to_string(&missing).unwrap(), raw, "the missing kind's text changed");
}

/// Test 1049 (plugin-api-stage1, D13): an id is unique WITHIN its family, not across. The generator
/// `checker` and the transition `checker` are both the file's tag of their kind; a reader looks the
/// id up in ITS family, so an effect slot that names a transition's id is kept as a missing effect,
/// never read as the transition (before, the registry held one id for every media and the first
/// registered won).
#[test]
fn t1049_a_plugin_id_is_unique_within_its_family_only() {
    use kt_plugin_api::{Decl, Family, Manifest, Media, Plugin, Registry, UiDecl};
    struct Bare(Manifest);
    impl Plugin for Bare {
        fn manifest(&self) -> &Manifest {
            &self.0
        }
    }
    let bare = |media| {
        std::sync::Arc::new(Bare(Manifest {
            id: "same_id".into(),
            media,
            passport: Vec::new(),
            rows: Vec::new(),
            decl: Decl::default(),
            ui: UiDecl::default(),
        })) as std::sync::Arc<dyn Plugin>
    };
    // video and audio effects are ONE family (a chain holds both)
    let mut reg = Registry::new();
    reg.register(bare(Media::AudioFx)).expect("the first audio effect `same_id` is registered");
    let twice = reg.register(bare(Media::VideoFx)).expect_err("a video effect of the same family and id");
    assert!(twice.reason.contains("twice"), "{twice:?}");
    assert!(reg.get(Family::Fx, "same_id").is_some() && reg.get(Family::Transition, "same_id").is_none());

    kt_plugins_builtin::install();
    let wipe = serde_json::json!({ "kind": "wipe" });
    let as_fx: FxInstance = serde_json::from_value(wipe.clone()).expect("an unknown effect is kept");
    assert!(matches!(as_fx, FxInstance::Missing(_)), "an effect slot read a transition's id: {as_fx:?}");
    let as_trans: TransitionKind = serde_json::from_value(wipe).expect("the wipe transition");
    assert!(matches!(&as_trans, TransitionKind::Plugin(p) if p.family == Family::Transition), "{as_trans:?}");
}
