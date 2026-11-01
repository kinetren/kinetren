//! Camera simulations (camera-isp-simulation; F-FX-010): `camera_isp` takes the frame as the SCENE
//! and films it with a simulated sensor developed by a simulated ISP — optics, the colour filter
//! array, the sensor in electrons and its noise, the ADC, then black level, defect pixels, IR
//! subtraction, lens shading, white balance, demosaic, CCM, tone, YUV with denoise and sharpening
//! and chroma subsampling. A SIMULATION, not an imitation (root CLAUDE.md, 2026-11-01): every look
//! is a consequence of physical and ISP settings; the presets are only such settings.
//!
//! A barrier plugin with all three sides of the contract: the CPU kernel (`cpu`, the byte
//! reference, f32), its AVX2 twin (the same row loops compiled for AVX2, `simd`), and a six-pass GPU
//! graph whose WGSL mirrors the CPU pass by pass at the same quantization points (design D1).

use std::sync::Arc;

use kt_plugin_api::barrier::{
    AlphaRel, BarrierCtx, BarrierKernel, Frame, GpuCtx, Node, Scratch, Slot,
};
use kt_plugin_api::math::FrozenMath;
use kt_plugin_api::{
    Cooked, Decl, Gpu, Manifest, Media, ParamKind, ParamSpec, ParamUnit, ParamValues, Plugin,
    RowDecl, Store, UiDecl, UiVariant, VideoClass,
};

pub mod cfa;
pub mod cpu;
#[cfg(target_arch = "x86_64")]
mod simd;

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<Arc<dyn Plugin>> {
    vec![Arc::new(CameraIsp::new())]
}

// ---- the passport ------------------------------------------------------------------------------

/// The choice rows' keys (the file holds the key).
pub const ISP_PHASES: &[&str] = &["exact", "shift_x", "shift_y", "shift_xy"];
pub const DEMOSAICS: &[&str] = &["bilinear", "mhc", "edge"];
pub const CURVES: &[&str] = &["srgb", "bt709", "gamma22"];
pub const MATRICES: &[&str] = &["bt601", "bt709"];
pub const CHROMAS: &[&str] = &["yuv444", "yuv422", "yuv420"];
pub const DECODE_MATRICES: &[&str] = &["same", "bt601", "bt709"];

/// One stage of the chain as the window draws it (design "The chain"): its catalog key, its bypass
/// row (if it has one) and its rows in passport order.
pub struct Stage {
    pub key: &'static str,
    pub bypass: Option<&'static str>,
    pub rows: &'static [&'static str],
}

/// The stages in the order of the chain — the window's blocks (t1219).
pub const STAGES: &[Stage] = &[
    Stage { key: "optics", bypass: Some("optics"), rows: &["vignetting", "lateral_ca", "olpf"] },
    Stage { key: "infrared", bypass: None, rows: &["ir_cut", "scene_ir", "qe_ir"] },
    Stage {
        key: "sensor",
        bypass: None,
        rows: &[
            "cfa",
            "crosstalk",
            "exposure_ev",
            "full_well",
            "qe_r",
            "qe_g",
            "qe_b",
            "analog_gain",
            "black_level",
            "adc_bits",
        ],
    },
    Stage {
        key: "noise",
        bypass: Some("noise"),
        rows: &[
            "read_noise",
            "dark_current",
            "dsnu",
            "prnu",
            "hot_pixels",
            "dead_pixels",
            "sensor_seed",
        ],
    },
    Stage { key: "blc", bypass: Some("blc"), rows: &["black_error"] },
    Stage { key: "dpc", bypass: Some("dpc"), rows: &["dpc_threshold"] },
    Stage { key: "ir_sub", bypass: Some("ir_sub"), rows: &["ir_sub_strength"] },
    Stage { key: "lsc", bypass: Some("lsc"), rows: &["lsc_strength"] },
    Stage { key: "wb", bypass: Some("wb"), rows: &["wb_r_error", "wb_b_error", "digital_gain_ev"] },
    Stage { key: "demosaic", bypass: None, rows: &["isp_phase", "demosaic"] },
    Stage { key: "ccm", bypass: Some("ccm"), rows: &["ccm_strength"] },
    Stage { key: "tone", bypass: Some("tone"), rows: &["curve"] },
    Stage { key: "denoise", bypass: Some("denoise"), rows: &["luma_nr", "chroma_nr"] },
    Stage { key: "sharpen", bypass: Some("sharpen"), rows: &["sharpen_amount"] },
    Stage { key: "yuv", bypass: Some("yuv"), rows: &["matrix", "chroma", "decode_matrix"] },
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

/// The passport in file order. The defaults are a decent camera: ISO 1, a 12-bit ADC, mild noise.
fn passport() -> Vec<(ParamSpec, RowDecl)> {
    use ParamUnit::*;
    vec![
        // optics
        flag("optics", true),
        num("vignetting", 0.0, 4.0, 0.3, 0.05, None),
        num("lateral_ca", -8.0, 8.0, 0.0, 0.1, Px),
        num("olpf", 0.0, 2.0, 0.5, 0.05, Px),
        // the near infrared
        num("scene_ir", 0.0, 4.0, 0.4, 0.05, Ratio),
        flag("ir_cut", true),
        num("qe_ir", 0.0, 1.0, 0.3, 0.01, Percent),
        // the colour filter array and the sensor
        choice("cfa", cfa::LAYOUTS, 0),
        num("crosstalk", 0.0, 0.5, 0.1, 0.01, Percent),
        num("exposure_ev", -8.0, 4.0, 0.0, 0.1, None),
        num("full_well", 500.0, 100_000.0, 12_000.0, 100.0, None),
        num("qe_r", 0.05, 1.0, 0.42, 0.01, Percent),
        num("qe_g", 0.05, 1.0, 0.55, 0.01, Percent),
        num("qe_b", 0.05, 1.0, 0.38, 0.01, Percent),
        num("analog_gain", 1.0, 64.0, 1.0, 0.1, Ratio),
        flag("noise", true),
        num("read_noise", 0.0, 50.0, 2.5, 0.1, None),
        num("dark_current", 0.0, 500.0, 1.0, 0.5, None),
        num("dsnu", 0.0, 50.0, 0.5, 0.1, None),
        num("prnu", 0.0, 0.1, 0.01, 0.001, Percent),
        num("hot_pixels", 0.0, 2000.0, 20.0, 1.0, Count),
        num("dead_pixels", 0.0, 2000.0, 10.0, 1.0, Count),
        int("black_level", 0.0, 2048.0, 256.0, Store::U32),
        int("adc_bits", 8.0, 14.0, 12.0, Store::U32),
        int("sensor_seed", 0.0, 65535.0, 1.0, Store::U32),
        // the ISP: raw front
        flag("blc", true),
        int("black_error", -512.0, 512.0, 0.0, Store::I32),
        flag("dpc", true),
        num("dpc_threshold", 0.0, 1.0, 0.15, 0.01, Norm),
        flag("ir_sub", true),
        num("ir_sub_strength", 0.0, 2.0, 1.0, 0.01, Ratio),
        flag("lsc", true),
        num("lsc_strength", 0.0, 2.0, 1.0, 0.01, Ratio),
        flag("wb", true),
        num("wb_r_error", 0.25, 4.0, 1.0, 0.01, Ratio),
        num("wb_b_error", 0.25, 4.0, 1.0, 0.01, Ratio),
        num("digital_gain_ev", -4.0, 8.0, 0.0, 0.1, None),
        // demosaic and colour
        choice("isp_phase", ISP_PHASES, 0),
        choice("demosaic", DEMOSAICS, 1),
        flag("ccm", true),
        num("ccm_strength", 0.0, 3.0, 1.0, 0.01, Ratio),
        flag("tone", true),
        choice("curve", CURVES, 0),
        // YUV
        flag("denoise", true),
        num("luma_nr", 0.0, 1.0, 0.1, 0.01, Norm),
        num("chroma_nr", 0.0, 1.0, 0.4, 0.01, Norm),
        flag("sharpen", true),
        num("sharpen_amount", 0.0, 3.0, 0.4, 0.01, Ratio),
        flag("yuv", true),
        choice("matrix", MATRICES, 1),
        choice("chroma", CHROMAS, 2),
        choice("decode_matrix", DECODE_MATRICES, 0),
    ]
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
        ],
    },
    // the ISP reads the mosaic one pixel off: RGGB developed as GRBG — green turns magenta
    UiVariant { name_key: "fxreg.var.wrong_cfa_phase", sets: &[("isp_phase", 1.0)] },
    // the IR-cut filter taken out: the dyes pass the near infrared, WB amplifies it in R and B
    UiVariant { name_key: "fxreg.var.no_ir_cut", sets: &[("ir_cut", 0.0), ("scene_ir", 1.0)] },
];

/// Row indices, in passport order (`passport()` is the one list; `t1218` holds the two together).
pub mod row {
    pub const OPTICS: usize = 0;
    pub const VIGNETTING: usize = 1;
    pub const LATERAL_CA: usize = 2;
    pub const OLPF: usize = 3;
    pub const SCENE_IR: usize = 4;
    pub const IR_CUT: usize = 5;
    pub const QE_IR: usize = 6;
    pub const CFA: usize = 7;
    pub const CROSSTALK: usize = 8;
    pub const EXPOSURE_EV: usize = 9;
    pub const FULL_WELL: usize = 10;
    pub const QE_R: usize = 11;
    pub const QE_G: usize = 12;
    pub const QE_B: usize = 13;
    pub const ANALOG_GAIN: usize = 14;
    pub const NOISE: usize = 15;
    pub const READ_NOISE: usize = 16;
    pub const DARK_CURRENT: usize = 17;
    pub const DSNU: usize = 18;
    pub const PRNU: usize = 19;
    pub const HOT_PIXELS: usize = 20;
    pub const DEAD_PIXELS: usize = 21;
    pub const BLACK_LEVEL: usize = 22;
    pub const ADC_BITS: usize = 23;
    pub const SENSOR_SEED: usize = 24;
    pub const BLC: usize = 25;
    pub const BLACK_ERROR: usize = 26;
    pub const DPC: usize = 27;
    pub const DPC_THRESHOLD: usize = 28;
    pub const IR_SUB: usize = 29;
    pub const IR_SUB_STRENGTH: usize = 30;
    pub const LSC: usize = 31;
    pub const LSC_STRENGTH: usize = 32;
    pub const WB: usize = 33;
    pub const WB_R_ERROR: usize = 34;
    pub const WB_B_ERROR: usize = 35;
    pub const DIGITAL_GAIN_EV: usize = 36;
    pub const ISP_PHASE: usize = 37;
    pub const DEMOSAIC: usize = 38;
    pub const CCM: usize = 39;
    pub const CCM_STRENGTH: usize = 40;
    pub const TONE: usize = 41;
    pub const CURVE: usize = 42;
    pub const DENOISE: usize = 43;
    pub const LUMA_NR: usize = 44;
    pub const CHROMA_NR: usize = 45;
    pub const SHARPEN: usize = 46;
    pub const SHARPEN_AMOUNT: usize = 47;
    pub const YUV: usize = 48;
    pub const MATRIX: usize = 49;
    pub const CHROMA: usize = 50;
    pub const DECODE_MATRIX: usize = 51;
    pub const COUNT: usize = 52;
}

// ---- the plan: every per-frame constant, computed ONCE in f64 and rounded to f32 (D3) ----------

/// The near-infrared transmission of the dyes by code (R, G, B, W, IR): the dyes are transparent
/// above ~700 nm (design step 8). The WGSL `T_IR` holds the same five numbers.
pub const DYE_NIR: [f32; 5] = [1.0, 0.9, 0.95, 1.0, 1.0];
/// The visible leak of an IR pixel's filter.
pub const IR_VIS_LEAK: f32 = 0.03;
/// The NIR estimate's weights over the scene's linear RGB (design step 2).
pub const IR_FROM_RGB: [f32; 3] = [0.5, 0.35, 0.15];

/// The camera for one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// K: the frame is K× the sensor's pixel grid (D6).
    pub k: u32,
    pub period: u32,
    pub sensor_tab: [u32; 4],
    pub isp_tab: [u32; 4],
    /// The ISP's layout is a 2×2 Bayer (MHC's exact kernels).
    pub bayer: bool,
    // P0 optics and photons
    pub optics: bool,
    /// cos⁴ vignetting: `E = 1/(1 + t² r²)²`.
    pub vig_t2: f32,
    /// Lateral CA: R at radius ×(1+ca), B at ×(1−ca).
    pub ca: f32,
    /// The OLPF split distance in sensor pixels.
    pub olpf: f32,
    /// Mean electrons at scene 1.0 as a fraction of the full well, per band (R, G, B).
    pub a: [f32; 3],
    pub xtalk: f32,
    /// NIR electrons per unit NIR radiance as a fraction of the well (0 with the IR-cut filter in).
    pub cir: f32,
    pub scene_ir: f32,
    // P1 the sensor's electronics
    pub noise: bool,
    pub seed_t: u32,
    pub seed_f: u32,
    pub adc_max: u32,
    pub black: u32,
    pub dead_thr: u32,
    pub hot_thr: u32,
    pub fw: f32,
    /// DN per electron (analog gain included).
    pub cg: f32,
    pub read: f32,
    pub dark: f32,
    pub dsnu: f32,
    pub prnu: f32,
    // P2 the raw front
    pub b_isp: f32,
    pub inv_range: f32,
    pub dpc: bool,
    pub dpc_thr: f32,
    /// The IR subtraction's strength (0: off, or no IR pixels).
    pub ir_sub: f32,
    /// LSC: `× (1 + t² r²)^lsc_pow` (0: off).
    pub lsc_pow: f32,
    /// White balance × digital gain by code (R, G, B, W).
    pub gains: [f32; 4],
    // P3 demosaic, CCM, tone
    pub method: u32,
    pub ccm: [f32; 9],
    /// 0 linear, 1 sRGB, 2 BT.709, 3 gamma 2.2.
    pub tone: u32,
    // P4 YUV
    pub p4: bool,
    pub yuv: bool,
    /// The encoder: (Kr, Kg, Kb, 0.5/(1−Kb), 0.5/(1−Kr)).
    pub enc: [f32; 5],
    /// The decoder of the SAME matrix (P4 when YUV is bypassed): (Kr, Kb, 1/Kg, 2(1−Kr), 2(1−Kb)).
    pub dec_same: [f32; 5],
    /// The luma NR's 1/σ² (0: off).
    pub luma_inv_s2: f32,
    pub chroma_amt: f32,
    pub sharpen: f32,
    // P5 chroma subsampling and decode
    /// 0 4:4:4, 1 4:2:2, 2 4:2:0.
    pub chroma: u32,
    /// The decoder's matrix, as `dec_same`.
    pub dec: [f32; 5],
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
        let v = |i: usize| p.get(i).copied().unwrap_or(0.0);
        let on = |i: usize| v(i) >= 0.5;
        let ch = |i: usize, n: usize| (v(i).round().max(0.0) as usize).min(n - 1);
        let layout = cfa::layout(ch(row::CFA, cfa::LAYOUTS.len()));
        let (dx, dy) = match ch(row::ISP_PHASE, ISP_PHASES.len()) {
            0 => (0, 0),
            1 => (1, 0),
            2 => (0, 1),
            _ => (1, 1),
        };
        let qe = [
            v(row::QE_R).clamp(0.05, 1.0),
            v(row::QE_G).clamp(0.05, 1.0),
            v(row::QE_B).clamp(0.05, 1.0),
        ];
        let ev = 2f64.fpowf(v(row::EXPOSURE_EV).clamp(-8.0, 4.0));
        let fw = v(row::FULL_WELL).clamp(500.0, 100_000.0);
        let bits = v(row::ADC_BITS).round().clamp(8.0, 14.0) as u32;
        let adc_max = (1u32 << bits) - 1;
        let black = (v(row::BLACK_LEVEL).round().max(0.0) as u32).min(adc_max / 2);
        let gain = v(row::ANALOG_GAIN).clamp(1.0, 64.0);
        let cg = gain * (adc_max - black) as f64 / fw;
        let dark = v(row::DARK_CURRENT).clamp(0.0, 500.0);
        let vig_ev = v(row::VIGNETTING).clamp(0.0, 4.0);
        let vig_t2 = 2f64.fpowf(vig_ev / 2.0) - 1.0;
        // the ISP's black: the pedestal and the dark mean the optical-black rows measure, plus its error
        let b_isp =
            if on(row::BLC) { black as f64 + dark * cg + v(row::BLACK_ERROR).round() } else { 0.0 };
        let range = (adc_max as f64 - b_isp).max(1.0);
        let dg = 2f64.fpowf(v(row::DIGITAL_GAIN_EV).clamp(-4.0, 8.0));
        let gains = if on(row::WB) {
            let (er, eb) =
                (v(row::WB_R_ERROR).clamp(0.25, 4.0), v(row::WB_B_ERROR).clamp(0.25, 4.0));
            [
                qe[1] / qe[0] * er * dg,
                dg,
                qe[1] / qe[2] * eb * dg,
                qe[1] / (qe[0] + qe[1] + qe[2]) * dg,
            ]
        } else {
            [dg; 4]
        };
        let x = v(row::CROSSTALK).clamp(0.0, 0.5);
        // CCM = I + s·(M⁻¹ − I), M = αI + βJ with α = 1 − 1.5x, β = x/2 (rows sum to 1):
        // M⁻¹ = (I − β/(α+3β)·J)/α
        let ccm = if on(row::CCM) {
            let s = v(row::CCM_STRENGTH).clamp(0.0, 3.0);
            let (al, be) = (1.0 - 1.5 * x, 0.5 * x);
            let off = -be / (al + 3.0 * be) / al;
            let diag = 1.0 / al + off;
            let d = 1.0 + s * (diag - 1.0);
            let o = s * off;
            [d, o, o, o, d, o, o, o, d]
        } else {
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
        };
        let (kr, kb) = matrix_k(ch(row::MATRIX, MATRICES.len()));
        let (dkr, dkb) = match ch(row::DECODE_MATRIX, DECODE_MATRICES.len()) {
            0 => (kr, kb),
            i => matrix_k(i - 1),
        };
        let denoise = on(row::DENOISE);
        let luma_sigma = if denoise { v(row::LUMA_NR).clamp(0.0, 1.0) * 0.08 } else { 0.0 };
        let chroma_amt = if denoise { v(row::CHROMA_NR).clamp(0.0, 1.0) } else { 0.0 };
        let sharpen = if on(row::SHARPEN) { v(row::SHARPEN_AMOUNT).clamp(0.0, 3.0) } else { 0.0 };
        let yuv = on(row::YUV);
        // the half-diagonal of the sensor normalises the radius; CA in px at the corner → a fraction
        let ir = on(row::IR_CUT);
        let thr = |per_million: f64| (per_million.clamp(0.0, 2000.0) * 4294.967296).round() as u32;
        let seed_t = kt_plugin_api::barrier::noise_seed(burst_index);
        let seed_f = kt_plugin_api::barrier::noise_hash(
            v(row::SENSOR_SEED).round().clamp(0.0, 65535.0) as u32 ^ 0x5EED_CA11,
        );
        let f = |x: f64| x as f32;
        Plan {
            k: k.max(1),
            period: layout.period,
            sensor_tab: cfa::pack(&layout, 0, 0),
            isp_tab: cfa::pack(&layout, dx, dy),
            bayer: cfa::is_bayer(&layout),
            optics: on(row::OPTICS),
            vig_t2: f(vig_t2),
            ca: f(v(row::LATERAL_CA).clamp(-8.0, 8.0)),
            olpf: f(v(row::OLPF).clamp(0.0, 2.0)),
            a: [f(ev * qe[0] / qe[1]), f(ev), f(ev * qe[2] / qe[1])],
            xtalk: f(x),
            cir: if ir { 0.0 } else { f(ev * v(row::QE_IR).clamp(0.0, 1.0) / qe[1]) },
            scene_ir: f(v(row::SCENE_IR).clamp(0.0, 4.0)),
            noise: on(row::NOISE),
            seed_t,
            seed_f,
            adc_max,
            black,
            dead_thr: thr(v(row::DEAD_PIXELS)),
            hot_thr: thr(v(row::HOT_PIXELS)),
            fw: f(fw),
            cg: f(cg),
            read: f(v(row::READ_NOISE).clamp(0.0, 50.0)),
            dark: f(dark),
            dsnu: f(v(row::DSNU).clamp(0.0, 50.0)),
            prnu: f(v(row::PRNU).clamp(0.0, 0.1)),
            b_isp: f(b_isp),
            inv_range: f(1.0 / range),
            dpc: on(row::DPC),
            dpc_thr: f(v(row::DPC_THRESHOLD).clamp(0.0, 1.0)),
            ir_sub: if on(row::IR_SUB) && cfa::has_ir(&layout) {
                f(v(row::IR_SUB_STRENGTH).clamp(0.0, 2.0))
            } else {
                0.0
            },
            lsc_pow: if on(row::LSC) { f(2.0 * v(row::LSC_STRENGTH).clamp(0.0, 2.0)) } else { 0.0 },
            gains: gains.map(f),
            method: ch(row::DEMOSAIC, DEMOSAICS.len()) as u32,
            ccm: ccm.map(f),
            tone: if on(row::TONE) { 1 + ch(row::CURVE, CURVES.len()) as u32 } else { 0 },
            p4: yuv || luma_sigma > 0.0 || chroma_amt > 0.0 || sharpen > 0.0,
            yuv,
            enc: [f(kr), f(1.0 - kr - kb), f(kb), f(0.5 / (1.0 - kb)), f(0.5 / (1.0 - kr))],
            dec_same: decoder(kr, kb),
            luma_inv_s2: if luma_sigma > 0.0 { f(1.0 / (luma_sigma * luma_sigma)) } else { 0.0 },
            chroma_amt: f(chroma_amt),
            sharpen: f(sharpen),
            chroma: if yuv { ch(row::CHROMA, CHROMAS.len()) as u32 } else { 0 },
            dec: decoder(dkr, dkb),
        }
    }

    /// The flags word the passes share: K in bits 0..7, the period in 8..11, then one bit each.
    pub fn flags(&self) -> u32 {
        self.k.min(255)
            | (self.period << 8)
            | (self.optics as u32) << 12
            | (self.noise as u32) << 13
            | (self.dpc as u32) << 14
            | (self.bayer as u32) << 15
            | (self.yuv as u32) << 16
            | self.method << 20
            | self.tone << 24
            | self.chroma << 28
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

impl Plugin for CameraIsp {
    fn manifest(&self) -> &Manifest {
        &self.m
    }
    fn barrier(&self) -> Option<&dyn BarrierKernel> {
        Some(self)
    }
}

const SHADERS: &[&str] = &[
    concat!(include_str!("camera_common.wgsl"), include_str!("p0_optics.wgsl")),
    concat!(include_str!("camera_common.wgsl"), include_str!("p1_sensor.wgsl")),
    concat!(include_str!("camera_common.wgsl"), include_str!("p2_raw.wgsl")),
    concat!(include_str!("camera_common.wgsl"), include_str!("p3_demosaic.wgsl")),
    concat!(include_str!("camera_common.wgsl"), include_str!("p4_yuv.wgsl")),
    concat!(include_str!("camera_common.wgsl"), include_str!("p5_chroma.wgsl")),
];

fn bits(x: f32) -> u32 {
    x.to_bits()
}

impl BarrierKernel for CameraIsp {
    /// The host never calls the cooked forms (D2): they answer the defaults.
    fn frame(&self, _k: &Cooked, f: Frame, cx: &BarrierCtx, scratch: &mut dyn Scratch) -> Frame {
        self.frame_values(&self.defaults(), f, cx, scratch)
    }

    fn gpu(&self, _k: &Cooked, g: &GpuCtx, out: &mut Vec<Node>) {
        self.gpu_values(&self.defaults(), g, out)
    }

    fn frame_values(
        &self,
        p: &[f64],
        f: Frame,
        cx: &BarrierCtx,
        scratch: &mut dyn Scratch,
    ) -> Frame {
        let plan = Plan::new(p, cx.res_scale, cx.burst_index);
        cpu::run(&plan, f, cx.simd, scratch)
    }

    fn shaders(&self) -> &'static [&'static str] {
        SHADERS
    }

    /// Six passes (D1): P0 reads the source; each next pass reads the previous one (and the source
    /// for its alpha). P4 runs only with NR, sharpening or YUV; P5 only with YUV.
    fn gpu_values(&self, p: &[f64], g: &GpuCtx, out: &mut Vec<Node>) {
        let c = Plan::new(p, g.res_scale, g.burst_index);
        let fl = c.flags();
        let t = c.sensor_tab;
        let it = c.isp_tab;
        let pass = |shader: u8, inputs: [Slot; 3], u: [u32; 8], f: [f32; 8]| Node::Pass {
            shader,
            inputs,
            u,
            f,
        };
        out.push(pass(
            0,
            [Slot::Src, Slot::None, Slot::None],
            [t[0], t[1], t[2], t[3], fl, bits(c.cir), bits(c.scene_ir), 0],
            [c.vig_t2, c.ca, c.olpf, c.a[0], c.a[1], c.a[2], c.xtalk, 0.0],
        ));
        out.push(pass(
            1,
            [Slot::Prev, Slot::None, Slot::None],
            [c.seed_t, c.seed_f, c.adc_max | c.black << 16, c.dead_thr, c.hot_thr, fl, 0, 0],
            [c.fw, c.cg, c.read, c.dark, c.dsnu, c.prnu, 0.0, 0.0],
        ));
        out.push(pass(
            2,
            [Slot::Prev, Slot::None, Slot::None],
            [it[0], it[1], it[2], it[3], fl, bits(c.dpc_thr), bits(c.ir_sub), bits(c.lsc_pow)],
            [c.b_isp, c.inv_range, c.vig_t2, 0.0, c.gains[0], c.gains[1], c.gains[2], c.gains[3]],
        ));
        let m = c.ccm;
        out.push(pass(
            3,
            [Slot::Prev, Slot::Src, Slot::None],
            [it[0], it[1], it[2], it[3], fl, bits(m[8]), 0, 0],
            [m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7]],
        ));
        if c.p4 {
            let (e, d) = (c.enc, c.dec_same);
            out.push(pass(
                4,
                [Slot::Prev, Slot::Src, Slot::None],
                [fl, bits(d[0]), bits(d[1]), bits(d[2]), bits(d[3]), bits(d[4]), 0, 0],
                [e[0], e[1], e[2], e[3], e[4], c.luma_inv_s2, c.chroma_amt, c.sharpen],
            ));
        }
        if c.yuv {
            let d = c.dec;
            out.push(pass(
                5,
                [Slot::Prev, Slot::Src, Slot::None],
                [fl, 0, 0, 0, 0, 0, 0, 0],
                [d[0], d[1], d[2], d[3], d[4], 0.0, 0.0, 0.0],
            ));
        }
    }
}
