//! Camera simulations (camera-isp-simulation; F-FX-010): `camera_isp` takes the frame as a picture
//! another camera DEVELOPED, unprocesses it back into the scene, and films that scene with a
//! simulated lens and sensor developed by a simulated ISP. A SIMULATION, not an imitation (root
//! CLAUDE.md, 2026-11-01): every defect of the optics and the sensor is a physical step, and every
//! one of them has the ISP block that real cameras use to remove it — and that block can be
//! bypassed or mis-tuned (the owner, 2026-11-01: "what is hidden in real ISPs — the distortions and
//! the ways to remove them — you had to put into the fx").
//!
//! | defect (lens, sensor) | its ISP block |
//! |---|---|
//! | the source camera's curve, matrix, white balance | the unprocessing into a scene |
//! | defocus / lens MTF, the OLPF | sharpening |
//! | geometric distortion | dewarp |
//! | lateral chromatic aberration | CA correction |
//! | vignetting (cos⁴) | lens shading correction |
//! | colour shading (the micro-lens chief ray angle) | colour lens shading correction |
//! | spectral crosstalk | the colour correction matrix |
//! | Gr/Gb imbalance | green equalisation |
//! | blooming, clipping | highlight handling, purple-fringe suppression |
//! | row noise, column fixed pattern | the optical-black row/column correction |
//! | dark current, the pedestal | black-level subtraction |
//! | hot and dead pixels | defect-pixel correction |
//! | shot and read noise | raw NR by the noise model, YUV NR |
//! | demosaic false colour | false-colour suppression |
//! | the near-infrared leak | the IR-cut filter, RGB-IR subtraction |
//!
//! A barrier plugin: a GPU graph of up to twelve passes (the f32 model of the same passes for the
//! consequence tests lives in `tests/model/cpu.rs`)
//! (design D1). `view` shows the picture after any stage.

use std::sync::Arc;

use kt_plugin_api::barrier::{AlphaRel, BarrierKernel, GpuCtx, Node, Slot};
use kt_plugin_api::math::FrozenMath;
use kt_plugin_api::{
    Cooked, Decl, Gpu, Manifest, Media, ParamKind, ParamSpec, ParamUnit, ParamValues, Plugin,
    RowDecl, Store, UiDecl, UiVariant, VideoClass,
};

pub mod cfa;
pub mod mic;

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![Arc::new(CameraIsp::new()), Arc::new(CameraIspSound::new())]
}

// ---- the passport ------------------------------------------------------------------------------

/// The choice rows' keys (the file holds the key).
pub const SRC_CURVES: &[&str] = &["srgb", "bt709", "gamma22", "linear"];
pub const ISP_PHASES: &[&str] = &["exact", "shift_x", "shift_y", "shift_xy"];
pub const DEMOSAICS: &[&str] = &["bilinear", "mhc", "edge"];
pub const CURVES: &[&str] = &["srgb", "bt709", "gamma22"];
pub const MATRICES: &[&str] = &["bt601", "bt709"];
pub const CHROMAS: &[&str] = &["yuv444", "yuv422", "yuv420"];
pub const DECODE_MATRICES: &[&str] = &["same", "bt601", "bt709"];
/// What the effect outputs: the camera's picture, or the picture after one stage of the chain.
pub const VIEWS: &[&str] = &[
    "output",
    "scene",
    "optics",
    "pixel",
    "raw",
    "raw_front",
    "raw_nr",
    "raw_corrected",
    "demosaic",
    "lens_corrected",
    "luma",
    "chroma",
];

fn num(
    name: &'static str,
    min: f64,
    max: f64,
    default: f64,
    step: f64,
    unit: ParamUnit,
) -> (ParamSpec, RowDecl) {
    (
        ParamSpec::num(name, min, max, default, step, unit, ParamKind::Smooth),
        RowDecl { store: Store::F32, omit_default: true },
    )
}

fn int(name: &'static str, min: f64, max: f64, default: f64, store: Store) -> (ParamSpec, RowDecl) {
    (
        ParamSpec::num(name, min, max, default, 1.0, ParamUnit::Count, ParamKind::Structural),
        RowDecl { store, omit_default: true },
    )
}

fn flag(name: &'static str, default: bool) -> (ParamSpec, RowDecl) {
    (ParamSpec::flag_row(name, default), RowDecl { store: Store::Bool, omit_default: true })
}

fn choice(
    name: &'static str,
    keys: &'static [&'static str],
    default: usize,
) -> (ParamSpec, RowDecl) {
    (
        ParamSpec::choice_row(name, keys, default),
        RowDecl { store: Store::Choice(keys), omit_default: true },
    )
}

/// The passport in file order, in the order of the chain. The defaults are a decent camera: ISO 1,
/// a 12-bit ADC, mild noise, every correction on and calibrated.
pub fn passport() -> Vec<(ParamSpec, RowDecl)> {
    use ParamUnit::*;
    vec![
        // the source: the camera that developed the frame, undone
        choice("src_curve", SRC_CURVES, 0),
        num("src_saturation", 0.3, 2.0, 1.0, 0.01, Ratio),
        num("src_wb_r", 0.5, 2.0, 1.0, 0.01, Ratio),
        num("src_wb_b", 0.5, 2.0, 1.0, 0.01, Ratio),
        // the lens
        flag("optics", true),
        num("lens_blur", 0.0, 8.0, 0.0, 0.05, Px),
        num("distortion", -0.5, 0.5, 0.0, 0.01, None),
        num("lateral_ca", -8.0, 8.0, 0.0, 0.1, Px),
        num("vignetting", 0.0, 4.0, 0.3, 0.05, None),
        num("color_shading", -0.5, 0.5, 0.0, 0.01, None),
        num("olpf", 0.0, 2.0, 0.5, 0.05, Px),
        // the near infrared
        flag("ir_cut", true),
        num("scene_ir", 0.0, 4.0, 0.4, 0.05, Ratio),
        num("qe_ir", 0.0, 1.0, 0.3, 0.01, Percent),
        // the colour filter array and the pixel
        choice("cfa", cfa::LAYOUTS, 0),
        num("crosstalk", 0.0, 0.5, 0.1, 0.01, Percent),
        num("green_imbalance", 0.0, 0.1, 0.0, 0.001, Percent),
        num("exposure_ev", -8.0, 4.0, 0.0, 0.1, None),
        num("full_well", 500.0, 100_000.0, 12_000.0, 100.0, None),
        num("qe_r", 0.05, 1.0, 0.42, 0.01, Percent),
        num("qe_g", 0.05, 1.0, 0.55, 0.01, Percent),
        num("qe_b", 0.05, 1.0, 0.38, 0.01, Percent),
        num("blooming", 0.0, 1.0, 0.0, 0.01, Percent),
        // noise
        flag("noise", true),
        num("read_noise", 0.0, 50.0, 2.5, 0.1, None),
        num("row_noise", 0.0, 20.0, 0.5, 0.1, None),
        num("column_fpn", 0.0, 20.0, 0.5, 0.1, None),
        num("dark_current", 0.0, 500.0, 1.0, 0.5, None),
        num("dsnu", 0.0, 50.0, 0.5, 0.1, None),
        num("prnu", 0.0, 0.1, 0.01, 0.001, Percent),
        num("hot_pixels", 0.0, 2000.0, 20.0, 1.0, Count),
        num("dead_pixels", 0.0, 2000.0, 10.0, 1.0, Count),
        int("sensor_seed", 0.0, 65535.0, 1.0, Store::U32),
        // the readout
        num("analog_gain", 1.0, 64.0, 1.0, 0.1, Ratio),
        int("black_level", 0.0, 2048.0, 256.0, Store::U32),
        int("adc_bits", 8.0, 14.0, 12.0, Store::U32),
        // the ISP: raw front
        flag("blc", true),
        int("black_error", -512.0, 512.0, 0.0, Store::I32),
        flag("rowcol", true),
        num("rowcol_strength", 0.0, 2.0, 1.0, 0.01, Ratio),
        flag("dpc", true),
        num("dpc_threshold", 0.0, 1.0, 0.15, 0.01, Norm),
        flag("geq", true),
        num("geq_strength", 0.0, 1.0, 1.0, 0.01, Norm),
        flag("raw_nr", true),
        num("raw_nr_strength", 0.0, 4.0, 1.0, 0.01, Ratio),
        flag("ir_sub", true),
        num("ir_sub_strength", 0.0, 2.0, 1.0, 0.01, Ratio),
        flag("lsc", true),
        num("lsc_strength", 0.0, 2.0, 1.0, 0.01, Ratio),
        flag("color_lsc", true),
        num("color_lsc_strength", 0.0, 2.0, 1.0, 0.01, Ratio),
        flag("wb", true),
        num("wb_r_error", 0.25, 4.0, 1.0, 0.01, Ratio),
        num("wb_b_error", 0.25, 4.0, 1.0, 0.01, Ratio),
        num("digital_gain_ev", -4.0, 8.0, 0.0, 0.1, None),
        // demosaic
        choice("isp_phase", ISP_PHASES, 0),
        choice("demosaic", DEMOSAICS, 1),
        // lens correction
        flag("dewarp", true),
        num("dewarp_strength", 0.0, 2.0, 1.0, 0.01, Ratio),
        flag("ca_corr", true),
        num("ca_corr_strength", 0.0, 2.0, 1.0, 0.01, Ratio),
        // colour
        flag("highlight", true),
        num("highlight_strength", 0.0, 1.0, 1.0, 0.01, Norm),
        flag("ccm", true),
        num("ccm_strength", 0.0, 3.0, 1.0, 0.01, Ratio),
        flag("tone", true),
        choice("curve", CURVES, 0),
        // YUV
        flag("denoise", true),
        num("luma_nr", 0.0, 1.0, 0.1, 0.01, Norm),
        num("chroma_nr", 0.0, 1.0, 0.4, 0.01, Norm),
        flag("fcs", true),
        num("fcs_strength", 0.0, 1.0, 0.5, 0.01, Norm),
        flag("purple", true),
        num("purple_strength", 0.0, 1.0, 0.5, 0.01, Norm),
        flag("sharpen", true),
        num("sharpen_amount", 0.0, 3.0, 0.4, 0.01, Ratio),
        flag("yuv", true),
        choice("matrix", MATRICES, 1),
        choice("chroma", CHROMAS, 2),
        choice("decode_matrix", DECODE_MATRICES, 0),
        // what the effect shows
        choice("view", VIEWS, 0),
        // the device's microphone (sim-audio-pairs, F-FX-014): read by the audio half only
        choice("mic_channels", mic::CHANNELS, 1),
        num("mic_scene_spl", 40.0, 130.0, 94.0, 0.5, Db),
        num("mic_wind", 0.0, 15.0, 0.0, 0.1, None),
        num("mic_port_d", 0.2, 3.0, 1.0, 0.05, None),
        num("mic_port_l", 0.2, 5.0, 0.8, 0.05, None),
        num("mic_front_v", 1.0, 60.0, 6.0, 0.5, None),
        num("mic_hpf", 20.0, 400.0, 80.0, 1.0, Hz),
        num("mic_self_noise", 15.0, 50.0, 29.0, 0.5, Db),
        num("mic_aop", 100.0, 140.0, 120.0, 0.5, Db),
        flag("mic_agc", true),
        num("mic_agc_target", -30.0, -3.0, -12.0, 0.5, Db),
        num("mic_agc_max", 0.0, 40.0, 18.0, 0.5, Db),
        flag("mic_ns", true),
        num("mic_ns_strength", 0.0, 1.0, 0.3, 0.01, Norm),
        choice("mic_codec", mic::CODECS, 0),
        choice("mic_kbps", mic::AAC_KBPS, 2),
    ]
}

/// The microphone rows: the picture never reads them (`Decl.foreign_rows` of the video half).
pub const MIC_ROWS: &[&str] = &[
    "mic_channels",
    "mic_scene_spl",
    "mic_wind",
    "mic_port_d",
    "mic_port_l",
    "mic_front_v",
    "mic_hpf",
    "mic_self_noise",
    "mic_aop",
    "mic_agc",
    "mic_agc_target",
    "mic_agc_max",
    "mic_ns",
    "mic_ns_strength",
    "mic_codec",
    "mic_kbps",
];

/// The passport's row names, in order (`ix` finds a row; built once).
fn names() -> &'static [&'static str] {
    static NAMES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| {
        passport()
            .into_iter()
            .map(|(s, _)| match s.name {
                std::borrow::Cow::Borrowed(n) => n,
                std::borrow::Cow::Owned(_) => unreachable!("built-in rows are borrowed"),
            })
            .collect()
    })
}

/// The index of a row by its name (a typo is a panic in every test).
pub fn ix(name: &str) -> usize {
    names()
        .iter()
        .position(|n| *n == name)
        .unwrap_or_else(|| panic!("camera_isp has no row {name}"))
}

/// The picker's presets (design: only physical settings). Indices of choice rows are their keys'.
const VARIANTS: &[UiVariant] = &[
    // a small pixel in the dark: a small well, high analog gain, digital gain on top, Quad Bayer,
    // hot pixels of a warm sensor, strong NR and sharpening to hide it
    UiVariant {
        name_key: "fxreg.var.phone_night",
        sets: &[
            ("cfa", 4.0),
            ("olpf", 0.0),
            ("exposure_ev", -5.0),
            ("full_well", 4000.0),
            ("analog_gain", 16.0),
            ("digital_gain_ev", 1.0),
            ("read_noise", 2.0),
            ("dark_current", 20.0),
            ("hot_pixels", 300.0),
            ("luma_nr", 0.6),
            ("chroma_nr", 1.0),
            ("sharpen_amount", 1.2),
            ("row_noise", 3.0),
            ("blooming", 0.4),
            ("green_imbalance", 0.02),
            ("raw_nr_strength", 2.0),
            ("distortion", -0.06),
            ("color_shading", 0.25),
            ("color_lsc_strength", 0.7),
            // its microphones: a small port in a thin case, a quiet street, a hard-working AGC and
            // noise suppressor, AAC at a phone's bitrate
            ("mic_scene_spl", 86.0),
            ("mic_port_d", 0.8),
            ("mic_port_l", 1.0),
            ("mic_front_v", 4.0),
            ("mic_wind", 1.5),
            ("mic_agc_max", 30.0),
            ("mic_ns_strength", 0.7),
            ("mic_kbps", 1.0),
        ],
    },
    // a cheap module: bilinear demosaic, a weak CCM, a mis-tuned WB, strong vignetting barely
    // corrected, lateral CA, a 10-bit ADC, crude sharpening, BT.601
    UiVariant {
        name_key: "fxreg.var.cheap_webcam",
        sets: &[
            ("vignetting", 1.5),
            ("lateral_ca", 2.5),
            ("olpf", 0.0),
            ("exposure_ev", -2.0),
            ("full_well", 6000.0),
            ("analog_gain", 4.0),
            ("read_noise", 6.0),
            ("black_level", 64.0),
            ("adc_bits", 10.0),
            ("lsc_strength", 0.4),
            ("wb_r_error", 1.15),
            ("wb_b_error", 0.85),
            ("demosaic", 0.0),
            ("ccm_strength", 0.5),
            ("luma_nr", 0.0),
            ("chroma_nr", 0.2),
            ("sharpen_amount", 1.5),
            ("matrix", 0.0),
            ("lens_blur", 1.2),
            ("distortion", 0.12),
            ("dewarp", 0.0),
            ("ca_corr", 0.0),
            ("color_shading", 0.3),
            ("color_lsc", 0.0),
            ("column_fpn", 4.0),
            ("rowcol", 0.0),
            ("fcs", 0.0),
            ("purple", 0.0),
            ("raw_nr", 0.0),
            // one electret capsule behind a long channel into a hollow case, USB audio at 16 kHz,
            // an AGC and no noise suppressor
            ("mic_channels", 0.0),
            ("mic_codec", 1.0),
            ("mic_port_d", 0.6),
            ("mic_port_l", 3.0),
            ("mic_front_v", 20.0),
            ("mic_hpf", 150.0),
            ("mic_self_noise", 38.0),
            ("mic_aop", 112.0),
            ("mic_ns", 0.0),
        ],
    },
    // a security camera at night: RGB-IR without the IR-cut filter under its IR illuminator,
    // the subtraction tuned for daylight, gain up
    UiVariant {
        name_key: "fxreg.var.security_ir",
        sets: &[
            ("cfa", 7.0),
            ("ir_cut", 0.0),
            ("scene_ir", 2.5),
            ("exposure_ev", -4.0),
            ("analog_gain", 8.0),
            ("digital_gain_ev", 1.0),
            ("read_noise", 4.0),
            ("ir_sub_strength", 0.6),
            ("ccm_strength", 0.6),
            ("chroma_nr", 0.8),
            ("distortion", 0.25),
            ("dewarp_strength", 0.6),
            ("vignetting", 1.2),
            ("lsc_strength", 0.7),
            ("raw_nr_strength", 1.5),
            // one cheap capsule outdoors: wind at the port, a hard AGC, G.711 at 8 kHz
            ("mic_channels", 0.0),
            ("mic_codec", 2.0),
            ("mic_port_d", 0.5),
            ("mic_port_l", 2.0),
            ("mic_front_v", 30.0),
            ("mic_hpf", 250.0),
            ("mic_self_noise", 42.0),
            ("mic_wind", 3.0),
            ("mic_agc_max", 30.0),
            ("mic_ns", 0.0),
        ],
    },
    // a large clean pixel: deep well, 14 bits, OLPF, edge-directed demosaic, no NR or sharpening, 4:4:4
    UiVariant {
        name_key: "fxreg.var.cinema_raw",
        sets: &[
            ("olpf", 0.7),
            ("full_well", 60_000.0),
            ("read_noise", 1.5),
            ("hot_pixels", 0.0),
            ("dead_pixels", 0.0),
            ("black_level", 1024.0),
            ("adc_bits", 14.0),
            ("demosaic", 2.0),
            ("denoise", 0.0),
            ("sharpen", 0.0),
            ("chroma", 0.0),
            ("raw_nr", 0.0),
            ("fcs", 0.0),
            ("row_noise", 0.2),
            ("column_fpn", 0.2),
        ],
    },
    // the ISP reads the mosaic one pixel off: RGGB developed as GRBG — green turns magenta
    UiVariant { name_key: "fxreg.var.wrong_cfa_phase", sets: &[("isp_phase", 1.0)] },
    // the IR-cut filter taken out: the dyes pass the near infrared, WB amplifies it in R and B
    UiVariant { name_key: "fxreg.var.no_ir_cut", sets: &[("ir_cut", 0.0), ("scene_ir", 1.0)] },
];

// ---- the plan: every per-frame constant, computed ONCE in f64 and rounded to f32 (D3) ----------

/// The near-infrared transmission of the dyes by code (R, G, B, W, IR): the dyes are transparent
/// above ~700 nm (design step 8). `dye_nir` in camera_common.wgsl holds the same five numbers.
pub const DYE_NIR: [f32; 5] = [1.0, 0.9, 0.95, 1.0, 1.0];
/// The visible leak of an IR pixel's filter.
pub const IR_VIS_LEAK: f32 = 0.03;
/// The NIR estimate's weights over the scene's linear RGB (design step 2).
pub const IR_FROM_RGB: [f32; 3] = [0.5, 0.35, 0.15];

/// The passes, in order; a pass's index is its shader's.
pub mod pass {
    pub const UNPROCESS: usize = 0;
    pub const OPTICS: usize = 1;
    pub const PIXEL: usize = 2;
    pub const READOUT: usize = 3;
    pub const RAW_FRONT: usize = 4;
    pub const RAW_NR: usize = 5;
    pub const RAW_CORRECT: usize = 6;
    pub const DEMOSAIC: usize = 7;
    pub const LENS_CORRECT: usize = 8;
    pub const YUV: usize = 9;
    pub const CHROMA: usize = 10;
    pub const VIEW: usize = 11;
}

/// A pass's input is on the sensor's grid: a texel is a sensor pixel (local-cost-queue, row 136).
pub const SENSOR_IN: u32 = 1 << 6;
/// A pass's output is on the sensor's grid: rendered at ⌈frame/K⌉, a fragment is a sensor pixel.
pub const SENSOR_OUT: u32 = 1 << 7;

/// How the view pass reads the texel it shows.
pub mod fmt {
    /// The unprocessed scene (packed 11/11/10, square-root coded).
    pub const SCENE: u32 = 0;
    /// A fraction of the full well, 16-bit fixed, in the sensor's filter colours.
    pub const WELL: u32 = 1;
    /// ADC numbers, in the sensor's filter colours.
    pub const DN: u32 = 2;
    /// The ISP's normalised raw, 16-bit fixed, in the ISP's filter colours.
    pub const NORM: u32 = 3;
    pub const LUMA: u32 = 4;
    pub const CHROMA: u32 = 5;
}

/// The camera for one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// K: the frame is K× the sensor's pixel grid (D6).
    pub k: u32,
    pub period: u32,
    pub sensor_tab: [u32; 4],
    pub isp_tab: [u32; 4],
    /// The ISP's layout is a 2×2 Bayer (MHC's and Hamilton–Adams' exact forms).
    pub bayer: bool,
    // U: the unprocessing
    /// The source's transfer: 0 sRGB, 1 BT.709, 2 gamma 2.2, 3 linear.
    pub src_curve: u32,
    /// The inverse of the source's saturation matrix.
    pub src_m: [f32; 9],
    /// The inverse of the source's white-balance gains (R, B).
    pub src_wb_inv: [f32; 2],
    /// The defocus disc radius in FRAME pixels (sensor px × K).
    pub blur: f32,
    // P0 the lens and the photons
    pub optics: bool,
    /// The lens's sampling map `x(1 + k r²)`: k > 0 barrel.
    pub k_lens: f32,
    /// Lateral CA in sensor px at the corner.
    pub ca: f32,
    /// The OLPF split distance in sensor pixels.
    pub olpf: f32,
    /// cos⁴ vignetting: `E = 1/(1 + t² r²)²`.
    pub vig_t2: f32,
    /// Colour shading: R × (1 − cs r²), B × (1 + cs r²/2).
    pub cshade: f32,
    /// Mean electrons at scene 1.0 as a fraction of the full well, per band (R, G, B).
    pub a: [f32; 3],
    pub xtalk: f32,
    /// NIR electrons per unit NIR radiance as a fraction of the well (0 with the IR-cut filter in).
    pub cir: f32,
    pub scene_ir: f32,
    // P1a the pixel
    pub noise: bool,
    pub seed_t: u32,
    pub seed_f: u32,
    pub dead_thr: u32,
    pub hot_thr: u32,
    pub fw: f32,
    /// 1/fw, so both sides multiply (a WGSL division is not correctly rounded).
    pub inv_fw: f32,
    pub dark: f32,
    pub dsnu: f32,
    pub prnu: f32,
    /// Gr pixels × (1 + δ), Gb × (1 − δ).
    pub gimb: f32,
    // P1b the readout
    pub adc_max: u32,
    pub black: u32,
    /// DN per electron (analog gain included).
    pub cg: f32,
    pub read: f32,
    /// The share of the charge above the well that spills into the four neighbours.
    pub bloom: f32,
    /// Row noise (temporal) and column fixed pattern, electrons rms.
    pub row_e: f32,
    pub col_e: f32,
    // P2a the raw front: BLC, row/column correction, DPC, GEQ
    pub b_isp: f32,
    pub inv_range: f32,
    /// The row and column offsets the optical-black correction removes, in normalised units, × strength.
    pub row_corr: f32,
    pub col_corr: f32,
    pub dpc: bool,
    pub dpc_thr: f32,
    pub geq: f32,
    // P2n raw NR: σ²(v) = a·v + b
    pub nr_a: f32,
    pub nr_b: f32,
    pub raw_nr: f32,
    // P2b IR subtraction, lens shading (luma and colour), WB
    pub ir_sub: f32,
    pub lsc_pow: f32,
    pub clsc: f32,
    /// White balance × digital gain by code (R, G, B, W).
    pub gains: [f32; 4],
    // P3 demosaic, highlights, CCM, tone
    pub method: u32,
    pub hl: f32,
    /// The post-WB level where the first channel clips.
    pub hl_clip: f32,
    pub ccm: [f32; 9],
    /// 0 linear, 1 sRGB, 2 BT.709, 3 gamma 2.2.
    pub tone: u32,
    // P3b lens correction
    pub k_corr: f32,
    pub ca_corr: f32,
    // P4 YUV
    pub p4: bool,
    /// P4 writes Y'CbCr (YUV on, or a luma/chroma view).
    pub yuv: bool,
    /// The encoder: (Kr, Kg, Kb, 0.5/(1−Kb), 0.5/(1−Kr)).
    pub enc: [f32; 5],
    /// The decoder of the SAME matrix: (Kr, Kb, 1/Kg, 2(1−Kr), 2(1−Kb)).
    pub dec_same: [f32; 5],
    /// The luma NR's 1/σ² (0: off).
    pub luma_inv_s2: f32,
    pub chroma_amt: f32,
    pub fcs: f32,
    pub purple: f32,
    pub sharpen: f32,
    // P5 chroma subsampling and decode
    pub p5: bool,
    /// 0 4:4:4, 1 4:2:2, 2 4:2:0.
    pub chroma: u32,
    /// The decoder's matrix, as `dec_same`.
    pub dec: [f32; 5],
    /// The view: the last pass that runs and the format the view pass reads (`None`: the output).
    pub view: Option<(usize, u32)>,
}

/// (Kr, Kb) of a YUV matrix index (0 BT.601, 1 BT.709).
fn matrix_k(i: usize) -> (f64, f64) {
    if i == 0 { (0.299, 0.114) } else { (0.2126, 0.0722) }
}

/// A decoder's constants (Kr, Kb, 1/Kg, 2(1−Kr), 2(1−Kb)).
fn decoder(kr: f64, kb: f64) -> [f32; 5] {
    [
        kr as f32,
        kb as f32,
        (1.0 / (1.0 - kr - kb)) as f32,
        (2.0 * (1.0 - kr)) as f32,
        (2.0 * (1.0 - kb)) as f32,
    ]
}

impl Plan {
    /// The plan of the values `p` (passport order) at K and the timeline position.
    pub fn new(p: &[f64], k: u32, burst_index: i64) -> Plan {
        let v = |n: &str| p.get(ix(n)).copied().unwrap_or(0.0);
        let on = |n: &str| v(n) >= 0.5;
        let ch = |n: &str, len: usize| (v(n).round().max(0.0) as usize).min(len - 1);
        let f = |x: f64| x as f32;
        let layout = cfa::layout(ch("cfa", cfa::LAYOUTS.len()));
        let (dx, dy) = match ch("isp_phase", ISP_PHASES.len()) {
            0 => (0, 0),
            1 => (1, 0),
            2 => (0, 1),
            _ => (1, 1),
        };
        // the source: S(s) = s·I + (1−s)·1·wᵀ (w the BT.709 luma), so S⁻¹ = I/s + (1 − 1/s)·1·wᵀ
        let s = v("src_saturation").clamp(0.3, 2.0);
        let w = [0.2126, 0.7152, 0.0722];
        let mut src_m = [0.0f32; 9];
        for r in 0..3 {
            for c in 0..3 {
                let id = if r == c { 1.0 / s } else { 0.0 };
                src_m[r * 3 + c] = f(id + (1.0 - 1.0 / s) * w[c]);
            }
        }
        let qe =
            [v("qe_r").clamp(0.05, 1.0), v("qe_g").clamp(0.05, 1.0), v("qe_b").clamp(0.05, 1.0)];
        let ev = 2f64.fpowf(v("exposure_ev").clamp(-8.0, 4.0));
        let fw = v("full_well").clamp(500.0, 100_000.0);
        let bits = v("adc_bits").round().clamp(8.0, 14.0) as u32;
        let adc_max = (1u32 << bits) - 1;
        let black = (v("black_level").round().max(0.0) as u32).min(adc_max / 2);
        let gain = v("analog_gain").clamp(1.0, 64.0);
        let cg = gain * (adc_max - black) as f64 / fw;
        let dark = v("dark_current").clamp(0.0, 500.0);
        let optics = on("optics");
        let vig_t2 = 2f64.fpowf(v("vignetting").clamp(0.0, 4.0) / 2.0) - 1.0;
        let noise = on("noise");
        let read = v("read_noise").clamp(0.0, 50.0);
        let row_e = if noise { v("row_noise").clamp(0.0, 20.0) } else { 0.0 };
        let col_e = v("column_fpn").clamp(0.0, 20.0);
        // the ISP's black: the pedestal and the dark mean the optical-black rows measure, plus its error
        let blc = on("blc");
        let b_isp = if blc { black as f64 + dark * cg + v("black_error").round() } else { 0.0 };
        let inv_range = 1.0 / (adc_max as f64 - b_isp).max(1.0);
        let rowcol = if on("rowcol") { v("rowcol_strength").clamp(0.0, 2.0) } else { 0.0 };
        let dg = 2f64.fpowf(v("digital_gain_ev").clamp(-4.0, 8.0));
        let gains = if on("wb") {
            let (er, eb) = (v("wb_r_error").clamp(0.25, 4.0), v("wb_b_error").clamp(0.25, 4.0));
            [
                qe[1] / qe[0] * er * dg,
                dg,
                qe[1] / qe[2] * eb * dg,
                qe[1] / (qe[0] + qe[1] + qe[2]) * dg,
            ]
        } else {
            [dg; 4]
        };
        let x = v("crosstalk").clamp(0.0, 0.5);
        // CCM = I + s·(M⁻¹ − I), M = αI + βJ with α = 1 − 1.5x, β = x/2 (rows sum to 1):
        // M⁻¹ = (I − β/(α+3β)·J)/α
        let ccm = if on("ccm") {
            let s = v("ccm_strength").clamp(0.0, 3.0);
            let (al, be) = (1.0 - 1.5 * x, 0.5 * x);
            let off = -be / (al + 3.0 * be) / al;
            let d = 1.0 + s * (1.0 / al + off - 1.0);
            let o = s * off;
            [d, o, o, o, d, o, o, o, d]
        } else {
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
        };
        let (kr, kb) = matrix_k(ch("matrix", MATRICES.len()));
        let (dkr, dkb) = match ch("decode_matrix", DECODE_MATRICES.len()) {
            0 => (kr, kb),
            i => matrix_k(i - 1),
        };
        let denoise = on("denoise");
        let luma_sigma = if denoise { v("luma_nr").clamp(0.0, 1.0) * 0.08 } else { 0.0 };
        let chroma_amt = if denoise { v("chroma_nr").clamp(0.0, 1.0) } else { 0.0 };
        let sharpen = if on("sharpen") { v("sharpen_amount").clamp(0.0, 3.0) } else { 0.0 };
        let fcs = if on("fcs") { v("fcs_strength").clamp(0.0, 1.0) } else { 0.0 };
        let purple = if on("purple") { v("purple_strength").clamp(0.0, 1.0) } else { 0.0 };
        let k_lens = if optics { v("distortion").clamp(-0.5, 0.5) } else { 0.0 };
        let ca = if optics { v("lateral_ca").clamp(-8.0, 8.0) } else { 0.0 };
        let k_corr = if on("dewarp") {
            v("distortion").clamp(-0.5, 0.5) * v("dewarp_strength").clamp(0.0, 2.0)
        } else {
            0.0
        };
        let ca_corr = if on("ca_corr") {
            v("lateral_ca").clamp(-8.0, 8.0) * v("ca_corr_strength").clamp(0.0, 2.0)
        } else {
            0.0
        };
        let cshade = if optics { v("color_shading").clamp(-0.5, 0.5) } else { 0.0 };
        let raw_nr = if on("raw_nr") { v("raw_nr_strength").clamp(0.0, 4.0) } else { 0.0 };
        let ir = on("ir_cut");
        let thr = |per_million: f64| (per_million.clamp(0.0, 2000.0) * 4294.967296).round() as u32;
        let seed_t = kt_plugin_api::barrier::noise_seed(burst_index);
        let seed_f = kt_plugin_api::barrier::noise_hash(
            v("sensor_seed").round().clamp(0.0, 65535.0) as u32 ^ 0x5EED_CA11,
        );
        // the view: the last pass that runs and what its texel is
        let view_i = ch("view", VIEWS.len());
        let luma_view = view_i == 10 || view_i == 11;
        let yuv = on("yuv") || luma_view;
        let p4 = yuv
            || luma_sigma > 0.0
            || chroma_amt > 0.0
            || sharpen > 0.0
            || fcs > 0.0
            || purple > 0.0;
        let view = match view_i {
            1 => Some((pass::UNPROCESS, fmt::SCENE)),
            2 => Some((pass::OPTICS, fmt::WELL)),
            3 => Some((pass::PIXEL, fmt::WELL)),
            4 => Some((pass::READOUT, fmt::DN)),
            5 => Some((pass::RAW_FRONT, fmt::NORM)),
            6 => Some((pass::RAW_NR, fmt::NORM)),
            7 => Some((pass::RAW_CORRECT, fmt::NORM)),
            8 => Some((pass::DEMOSAIC, u32::MAX)),
            9 => Some((pass::LENS_CORRECT, u32::MAX)),
            10 => Some((pass::YUV, fmt::LUMA)),
            11 => Some((pass::YUV, fmt::CHROMA)),
            _ => None,
        };
        Plan {
            k: k.max(1),
            period: layout.period,
            sensor_tab: cfa::pack(&layout, 0, 0),
            isp_tab: cfa::pack(&layout, dx, dy),
            bayer: cfa::is_bayer(&layout),
            src_curve: ch("src_curve", SRC_CURVES.len()) as u32,
            src_m,
            src_wb_inv: [
                f(1.0 / v("src_wb_r").clamp(0.5, 2.0)),
                f(1.0 / v("src_wb_b").clamp(0.5, 2.0)),
            ],
            blur: if optics { f(v("lens_blur").clamp(0.0, 8.0) * k.max(1) as f64) } else { 0.0 },
            optics,
            k_lens: f(k_lens),
            ca: f(ca),
            olpf: f(v("olpf").clamp(0.0, 2.0)),
            vig_t2: f(vig_t2),
            cshade: f(cshade),
            a: [f(ev * qe[0] / qe[1]), f(ev), f(ev * qe[2] / qe[1])],
            xtalk: f(x),
            cir: if ir { 0.0 } else { f(ev * v("qe_ir").clamp(0.0, 1.0) / qe[1]) },
            scene_ir: f(v("scene_ir").clamp(0.0, 4.0)),
            noise,
            seed_t,
            seed_f,
            dead_thr: thr(v("dead_pixels")),
            hot_thr: thr(v("hot_pixels")),
            fw: f(fw),
            inv_fw: f(1.0 / fw),
            dark: f(dark),
            dsnu: f(v("dsnu").clamp(0.0, 50.0)),
            prnu: f(v("prnu").clamp(0.0, 0.1)),
            gimb: f(v("green_imbalance").clamp(0.0, 0.1)),
            adc_max,
            black,
            cg: f(cg),
            read: f(read),
            bloom: f(v("blooming").clamp(0.0, 1.0)),
            row_e: f(row_e),
            col_e: f(col_e),
            b_isp: f(b_isp),
            inv_range: f(inv_range),
            row_corr: f(rowcol * row_e * cg * inv_range),
            col_corr: f(rowcol * col_e * cg * inv_range),
            dpc: on("dpc"),
            dpc_thr: f(v("dpc_threshold").clamp(0.0, 1.0)),
            geq: if on("geq") { f(v("geq_strength").clamp(0.0, 1.0)) } else { 0.0 },
            // the noise model in normalised raw: Var = cg·inv·v + (cg·σ_read·inv)²
            nr_a: f(cg * inv_range),
            nr_b: f((cg * read * inv_range).powi(2)),
            raw_nr: f(raw_nr),
            ir_sub: if on("ir_sub") && cfa::has_ir(&layout) {
                f(v("ir_sub_strength").clamp(0.0, 2.0))
            } else {
                0.0
            },
            lsc_pow: if on("lsc") { f(2.0 * v("lsc_strength").clamp(0.0, 2.0)) } else { 0.0 },
            clsc: if on("color_lsc") { f(v("color_lsc_strength").clamp(0.0, 2.0)) } else { 0.0 },
            gains: gains.map(f),
            method: ch("demosaic", DEMOSAICS.len()) as u32,
            hl: if on("highlight") { f(v("highlight_strength").clamp(0.0, 1.0)) } else { 0.0 },
            hl_clip: f(gains[0].min(gains[1]).min(gains[2])),
            ccm: ccm.map(f),
            tone: if on("tone") { 1 + ch("curve", CURVES.len()) as u32 } else { 0 },
            k_corr: f(k_corr),
            ca_corr: f(ca_corr),
            p4,
            yuv,
            enc: [f(kr), f(1.0 - kr - kb), f(kb), f(0.5 / (1.0 - kb)), f(0.5 / (1.0 - kr))],
            dec_same: decoder(kr, kb),
            luma_inv_s2: if luma_sigma > 0.0 { f(1.0 / (luma_sigma * luma_sigma)) } else { 0.0 },
            chroma_amt: f(chroma_amt),
            fcs: f(fcs),
            purple: f(purple),
            sharpen: f(sharpen),
            p5: yuv && !luma_view,
            chroma: ch("chroma", CHROMAS.len()) as u32,
            dec: decoder(dkr, dkb),
            view,
        }
    }

    /// The flags word the passes share: K in bits 0..5, the grids in 6 and 7 (`SENSOR_IN`,
    /// `SENSOR_OUT`, set per pass by `gpu_values`), the period in 8..11, then one bit each.
    pub fn flags(&self) -> u32 {
        self.k.min(63)
            | (self.period << 8)
            | (self.optics as u32) << 12
            | (self.noise as u32) << 13
            | (self.dpc as u32) << 14
            | (self.bayer as u32) << 15
            | (self.yuv as u32) << 16
            | self.src_curve << 17
            | self.method << 20
            | self.tone << 24
            | self.chroma << 28
    }

    /// Whether the lens-correction pass has anything to do.
    pub fn lens_correct(&self) -> bool {
        self.k_corr != 0.0 || self.ca_corr != 0.0
    }

    /// The passes that run, in order (the view pass last when a stage is viewed).
    pub fn passes(&self) -> Vec<usize> {
        let mut v =
            vec![pass::UNPROCESS, pass::OPTICS, pass::PIXEL, pass::READOUT, pass::RAW_FRONT];
        if self.raw_nr > 0.0 {
            v.push(pass::RAW_NR);
        }
        v.extend([pass::RAW_CORRECT, pass::DEMOSAIC]);
        if self.lens_correct() {
            v.push(pass::LENS_CORRECT);
        }
        if self.p4 {
            v.push(pass::YUV);
        }
        if self.p5 {
            v.push(pass::CHROMA);
        }
        if let Some((last, f)) = self.view {
            // the viewed stage, or the last one before it that runs (a bypassed stage shows its input)
            let cut = v.iter().rposition(|p| *p <= last).unwrap_or(0);
            v.truncate(cut + 1);
            if f != u32::MAX {
                v.push(pass::VIEW);
            }
        }
        v
    }

    /// The view pass's format and table: the stage it shows decides both.
    pub fn view_fmt(&self) -> u32 {
        let passes = self.passes();
        let shown = passes.iter().rev().find(|p| **p != pass::VIEW).copied().unwrap_or(0);
        match shown {
            pass::UNPROCESS => fmt::SCENE,
            pass::OPTICS | pass::PIXEL => fmt::WELL,
            pass::READOUT => fmt::DN,
            pass::YUV => self.view.map_or(fmt::LUMA, |v| v.1),
            _ => fmt::NORM,
        }
    }
}

// ---- the plugin ----------------------------------------------------------------------------------

pub struct CameraIsp {
    m: Manifest,
}

impl Default for CameraIsp {
    fn default() -> CameraIsp {
        CameraIsp::new()
    }
}

impl CameraIsp {
    pub fn new() -> CameraIsp {
        let (passport, rows): (Vec<ParamSpec>, Vec<RowDecl>) = passport().into_iter().unzip();
        CameraIsp {
            m: Manifest {
                id: "camera_isp".into(),
                media: Media::VideoFx,
                passport,
                rows,
                decl: Decl {
                    video_class: Some(VideoClass::Barrier),
                    gpu: Gpu::Barrier,
                    alpha: AlphaRel::Preserves,
                    // the device's microphone (sim-audio-pairs): the picture never reads its rows
                    pair: Some("camera_isp_sound"),
                    foreign_rows: MIC_ROWS,
                    ..Decl::default()
                },
                ui: UiDecl {
                    name: "Camera ISP",
                    category_key: "fxreg.cat.camera",
                    keywords_key: "fxreg.kw.camera_isp",
                    variants: VARIANTS,
                    ..UiDecl::default()
                },
            },
        }
    }

    fn defaults(&self) -> Vec<f64> {
        ParamValues::defaults(&self.m.passport).0
    }
}

/// The passport's defaults.
pub fn defaults() -> Vec<f64> {
    static D: std::sync::OnceLock<Vec<f64>> = std::sync::OnceLock::new();
    D.get_or_init(|| passport().iter().map(|(s, _)| s.default).collect()).clone()
}

/// The sensor's and the ISP's rows: the microphone never reads them (the seed it does).
fn picture_rows() -> &'static [&'static str] {
    static R: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    R.get_or_init(|| names().iter().copied().filter(|n| !MIC_ROWS.contains(n) && *n != "sensor_seed").collect())
}

/// The camera's microphone (sim-audio-pairs, F-FX-014): the audio half of `camera_isp`, set by the
/// same looks (`mic`).
pub struct CameraIspSound {
    m: Manifest,
}

impl Default for CameraIspSound {
    fn default() -> CameraIspSound {
        CameraIspSound::new()
    }
}

impl CameraIspSound {
    pub fn new() -> CameraIspSound {
        let (passport, rows): (Vec<ParamSpec>, Vec<RowDecl>) = passport().into_iter().unzip();
        CameraIspSound {
            m: Manifest {
                id: "camera_isp_sound".into(),
                media: Media::AudioFx,
                passport,
                rows,
                decl: Decl {
                    state: true,
                    lookahead: Some(|_, _| mic::LA),
                    foreign_rows: picture_rows(),
                    ..Decl::default()
                },
                ui: UiDecl {
                    name: "Camera ISP",
                    category_key: "fxreg.cat.camera",
                    keywords_key: "fxreg.kw.camera_isp_sound",
                    // an audio entry has no variants: a sound has no thumbnail (R.54)
                    ..UiDecl::default()
                },
            },
        }
    }
}

impl Plugin for CameraIspSound {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn audio(&self) -> Option<&dyn kt_plugin_api::AudioKernel> {
        Some(self)
    }
}

impl kt_plugin_api::AudioKernel for CameraIspSound {
    fn build(&self, _opts: &kt_plugin_api::HostOpts) -> Box<dyn kt_plugin_api::AudioState> {
        Box::new(mic::MicSound::new())
    }
}

impl Plugin for CameraIsp {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn barrier(&self) -> Option<&dyn BarrierKernel> {
        Some(self)
    }
}

macro_rules! shader {
    ($f:literal) => {
        concat!(include_str!("camera_common.wgsl"), include_str!($f))
    };
}

const SHADERS: &[&str] = &[
    shader!("s00_unprocess.wgsl"),
    shader!("s01_optics.wgsl"),
    shader!("s02_pixel.wgsl"),
    shader!("s03_readout.wgsl"),
    shader!("s04_raw_front.wgsl"),
    shader!("s05_raw_nr.wgsl"),
    shader!("s06_raw_correct.wgsl"),
    shader!("s07_demosaic.wgsl"),
    shader!("s08_lens_correct.wgsl"),
    shader!("s09_yuv.wgsl"),
    shader!("s10_chroma.wgsl"),
    shader!("s11_view.wgsl"),
];

fn bits(x: f32) -> u32 {
    x.to_bits()
}

impl Plan {
    /// One pass's uniforms (`u`, `f`), the mirror of what its WGSL reads — the ONE text of the
    /// packing; the model in the tests reads the plan directly.
    pub fn uniforms(&self, pass_ix: usize) -> ([u32; 8], [f32; 8]) {
        self.uniforms_fl(pass_ix, self.flags())
    }

    /// `uniforms` with the flags word `fl` (the plan's, plus a pass's grid bits).
    pub fn uniforms_fl(&self, pass_ix: usize, fl: u32) -> ([u32; 8], [f32; 8]) {
        let t = self.sensor_tab;
        let it = self.isp_tab;
        match pass_ix {
            pass::UNPROCESS => {
                let m = self.src_m;
                (
                    [
                        fl,
                        bits(m[8]),
                        bits(self.src_wb_inv[0]),
                        bits(self.src_wb_inv[1]),
                        bits(self.blur),
                        0,
                        0,
                        0,
                    ],
                    [m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7]],
                )
            }
            pass::OPTICS => (
                [
                    t[0],
                    t[1],
                    t[2],
                    t[3],
                    fl,
                    bits(self.cir),
                    bits(self.scene_ir),
                    bits(self.k_lens),
                ],
                [
                    self.vig_t2,
                    self.ca,
                    self.olpf,
                    self.a[0],
                    self.a[1],
                    self.a[2],
                    self.xtalk,
                    self.cshade,
                ],
            ),
            pass::PIXEL => (
                [t[0], t[1], t[2], t[3], fl, self.seed_t, self.seed_f, 0],
                [
                    self.fw,
                    self.dark,
                    self.dsnu,
                    self.prnu,
                    self.dead_thr as f32,
                    self.hot_thr as f32,
                    self.gimb,
                    self.inv_fw,
                ],
            ),
            pass::READOUT => (
                [self.seed_t, self.seed_f, self.adc_max | self.black << 16, fl, 0, 0, 0, 0],
                [self.fw, self.cg, self.read, self.bloom, self.row_e, self.col_e, 0.0, 0.0],
            ),
            pass::RAW_FRONT => (
                [it[0], it[1], it[2], it[3], fl, self.seed_t, self.seed_f, 0],
                [
                    self.b_isp,
                    self.inv_range,
                    self.row_corr,
                    self.col_corr,
                    self.dpc_thr,
                    self.geq,
                    0.0,
                    0.0,
                ],
            ),
            pass::RAW_NR => (
                [it[0], it[1], it[2], it[3], fl, 0, 0, 0],
                [self.nr_a, self.nr_b, self.raw_nr, 0.0, 0.0, 0.0, 0.0, 0.0],
            ),
            pass::RAW_CORRECT => (
                [
                    it[0],
                    it[1],
                    it[2],
                    it[3],
                    fl,
                    bits(self.ir_sub),
                    bits(self.lsc_pow),
                    bits(self.clsc),
                ],
                [
                    self.vig_t2,
                    self.cshade,
                    0.0,
                    0.0,
                    self.gains[0],
                    self.gains[1],
                    self.gains[2],
                    self.gains[3],
                ],
            ),
            pass::DEMOSAIC => {
                let m = self.ccm;
                (
                    [it[0], it[1], it[2], it[3], fl, bits(m[8]), bits(self.hl), bits(self.hl_clip)],
                    [m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7]],
                )
            }
            pass::LENS_CORRECT => (
                [fl, 0, 0, 0, 0, 0, 0, 0],
                [self.k_corr, self.ca_corr, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            ),
            pass::YUV => {
                let (e, d) = (self.enc, self.dec_same);
                (
                    [
                        fl,
                        bits(d[0]),
                        bits(d[1]),
                        bits(d[2]),
                        bits(d[3]),
                        bits(d[4]),
                        bits(self.fcs),
                        bits(self.purple),
                    ],
                    [e[0], e[1], e[2], e[3], e[4], self.luma_inv_s2, self.chroma_amt, self.sharpen],
                )
            }
            pass::CHROMA => {
                let d = self.dec;
                ([fl, 0, 0, 0, 0, 0, 0, 0], [d[0], d[1], d[2], d[3], d[4], 0.0, 0.0, 0.0])
            }
            _ => {
                // the view: the table of the stage shown (the sensor's before the ISP, the ISP's after)
                let vf = self.view_fmt();
                let tab = if vf == fmt::NORM { it } else { t };
                let d = self.dec_same;
                (
                    [fl, vf, self.adc_max | self.black << 16, 0, tab[0], tab[1], tab[2], tab[3]],
                    [d[0], d[1], d[2], d[3], d[4], 0.0, 0.0, 0.0],
                )
            }
        }
    }
}

impl BarrierKernel for CameraIsp {

    fn gpu(&self, _k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        self.gpu_values(&self.defaults(), g, out)
    }

    fn shaders(&self) -> &'static [&'static str] {
        SHADERS
    }

    /// The passes by their files (gpu-pass-cost).
    fn pass_names(&self) -> &'static [&'static str] {
        &[
            "s00-unprocess", "s01-optics", "s02-pixel", "s03-readout", "s04-raw-front", "s05-raw-nr", "s06-raw-correct",
            "s07-demosaic", "s08-lens-correct", "s09-yuv", "s10-chroma", "s11-view",
        ]
    }

    /// The passes of `Plan::passes`: the first reads the source, each next one the previous (and
    /// the source for its alpha).
    ///
    /// At K > 1 a pass that works on the sensor's grid renders at ⌈frame/K⌉ (local-cost-queue, row
    /// 136): every pass after the unprocessing computes per sensor pixel and reads its input at the
    /// block's top-left texel, so its K² fragments of a block were one value computed K² times. The
    /// graph's last pass stays at the frame's size and reads the sensor-sized input at the pixel's
    /// sensor position: the same values, the same bytes. `cpu::with_general` keeps the frame-grid form
    /// (the reference of t2310).
    fn gpu_values(&self, p: &[f64], g: &GpuCtx, out: &mut Vec<Node>) {
        let c = Plan::new(p, g.res_scale, g.burst_index);
        let passes = c.passes();
        let sensor = c.k > 1 && kt_plugin_api::cpu::specialized();
        let (sw, sh) = (g.w.div_ceil(c.k), g.h.div_ceil(c.k));
        let mut prev_sensor = false;
        for (i, &ps) in passes.iter().enumerate() {
            let on_sensor = sensor && i + 1 < passes.len() && ps != pass::UNPROCESS;
            let fl = c.flags() | if prev_sensor { SENSOR_IN } else { 0 } | if on_sensor { SENSOR_OUT } else { 0 };
            let (u, f) = c.uniforms_fl(ps, fl);
            let first = if i == 0 { Slot::Src } else { Slot::Prev };
            let inputs = [first, Slot::Src, Slot::None];
            out.push(if on_sensor {
                Node::PassAt { shader: ps as u8, inputs, u, f, w: sw, h: sh }
            } else {
                Node::Pass { shader: ps as u8, inputs, u, f }
            });
            prev_sensor = on_sensor;
        }
    }
}
