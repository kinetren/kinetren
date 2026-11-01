//! camera_isp's window (camera-isp-simulation, task 3.1): the kind drawn as its PIPELINE — one block
//! per part of the camera (lens, sensor, the ISP's raw front, demosaic, colour, YUV), inside each
//! the stages in the order of the chain, each with its bypass and its rows. The numeric rows are the
//! shared grid (`widgets::params_grid_bound`, curves and publication included), the choices and flags
//! the kind's own combos and checkboxes; nothing here computes a pixel. The owner put the physics
//! first (p.4 of 2026-11-01): this panel is the chain made visible, not a device skin.

use crate::i18n::{k, tr};
use crate::widgets;
use eframe::egui;
use kt_model::{FxInstance, ParamType};

/// One stage: its key, its bypass row and its rows, in the order of the chain.
pub(crate) struct Stage {
    pub key: &'static str,
    pub bypass: Option<&'static str>,
    pub rows: &'static [&'static str],
}

/// The blocks of the window and their stages, in the order of the chain (the plugin's design,
/// "The chain"). A correcting stage names the defect it removes (`pair_note`).
pub(crate) const BLOCKS: &[(&str, &[Stage])] = &[
    (
        "source",
        &[Stage { key: "source", bypass: None, rows: &["src_curve", "src_saturation", "src_wb_r", "src_wb_b"] }],
    ),
    (
        "lens",
        &[
            Stage {
                key: "optics",
                bypass: Some("optics"),
                rows: &["lens_blur", "distortion", "lateral_ca", "vignetting", "color_shading", "olpf"],
            },
            Stage { key: "infrared", bypass: None, rows: &["ir_cut", "scene_ir", "qe_ir"] },
        ],
    ),
    (
        "sensor",
        &[
            Stage {
                key: "sensor",
                bypass: None,
                rows: &["cfa", "crosstalk", "green_imbalance", "exposure_ev", "full_well", "qe_r", "qe_g", "qe_b", "blooming"],
            },
            Stage {
                key: "noise",
                bypass: Some("noise"),
                rows: &[
                    "read_noise",
                    "row_noise",
                    "column_fpn",
                    "dark_current",
                    "dsnu",
                    "prnu",
                    "hot_pixels",
                    "dead_pixels",
                    "sensor_seed",
                ],
            },
            Stage { key: "readout", bypass: None, rows: &["analog_gain", "black_level", "adc_bits"] },
        ],
    ),
    (
        "raw",
        &[
            Stage { key: "blc", bypass: Some("blc"), rows: &["black_error"] },
            Stage { key: "rowcol", bypass: Some("rowcol"), rows: &["rowcol_strength"] },
            Stage { key: "dpc", bypass: Some("dpc"), rows: &["dpc_threshold"] },
            Stage { key: "geq", bypass: Some("geq"), rows: &["geq_strength"] },
            Stage { key: "raw_nr", bypass: Some("raw_nr"), rows: &["raw_nr_strength"] },
            Stage { key: "ir_sub", bypass: Some("ir_sub"), rows: &["ir_sub_strength"] },
            Stage { key: "lsc", bypass: Some("lsc"), rows: &["lsc_strength"] },
            Stage { key: "color_lsc", bypass: Some("color_lsc"), rows: &["color_lsc_strength"] },
            Stage { key: "wb", bypass: Some("wb"), rows: &["wb_r_error", "wb_b_error", "digital_gain_ev"] },
        ],
    ),
    ("demosaic", &[Stage { key: "demosaic", bypass: None, rows: &["isp_phase", "demosaic"] }]),
    (
        "lens_correction",
        &[
            Stage { key: "dewarp", bypass: Some("dewarp"), rows: &["dewarp_strength"] },
            Stage { key: "ca_corr", bypass: Some("ca_corr"), rows: &["ca_corr_strength"] },
        ],
    ),
    (
        "colour",
        &[
            Stage { key: "highlight", bypass: Some("highlight"), rows: &["highlight_strength"] },
            Stage { key: "ccm", bypass: Some("ccm"), rows: &["ccm_strength"] },
            Stage { key: "tone", bypass: Some("tone"), rows: &["curve"] },
        ],
    ),
    (
        "yuv",
        &[
            Stage { key: "denoise", bypass: Some("denoise"), rows: &["luma_nr", "chroma_nr"] },
            Stage { key: "fcs", bypass: Some("fcs"), rows: &["fcs_strength"] },
            Stage { key: "purple", bypass: Some("purple"), rows: &["purple_strength"] },
            Stage { key: "sharpen", bypass: Some("sharpen"), rows: &["sharpen_amount"] },
            Stage { key: "yuv", bypass: Some("yuv"), rows: &["matrix", "chroma", "decode_matrix"] },
        ],
    ),
];

/// The view row: drawn above the blocks (what the effect shows), not a stage.
const VIEW: Stage = Stage { key: "view", bypass: None, rows: &["view"] };

/// The pipeline: blocks, stages, bypasses, rows. Returns what changed (base or curve).
pub(crate) fn draw(
    ui: &mut egui::Ui,
    f: &mut FxInstance,
    mut bind: Option<&mut widgets::CurveBind<'_>>,
    pubc: Option<&widgets::PublishCtx>,
) -> widgets::ParamsChanged {
    let mut ch = widgets::ParamsChanged::default();
    ui.label(egui::RichText::new(tr(k::isp::LEAD)).weak().small());
    let names: Vec<String> = f.param_specs().iter().map(|s| s.name.to_string()).collect();
    // what the effect shows: the output, or the picture after any stage of the chain
    ch |= stage_rows(ui, f, &VIEW, &names, None, pubc);
    for (block, stages) in BLOCKS {
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.strong(block_title(block));
            for st in *stages {
                let on = st.bypass.is_none_or(|b| f.get_param(b) == Some(1.0));
                ui.horizontal(|ui| match st.bypass {
                    Some(b) => {
                        let mut v = on;
                        let r = ui.checkbox(&mut v, egui::RichText::new(stage_title(st.key)).strong());
                        // the live scenario aims at the bypass by the program's own rectangle
                        widgets::field_probe::note(&format!("isp.stage.{}", st.key), r.rect);
                        if r.changed() {
                            f.set_param(b, if v { 1.0 } else { 0.0 });
                            ch.base = true;
                        }
                    }
                    None => {
                        ui.label(egui::RichText::new(stage_title(st.key)).strong());
                    }
                });
                if let Some(note) = pair_note(st.key) {
                    ui.label(egui::RichText::new(note).weak().small());
                }
                ui.add_enabled_ui(on, |ui| {
                    ui.indent(st.key, |ui| {
                        ch |= stage_rows(ui, f, st, &names, bind.as_deref_mut(), pubc);
                    });
                });
            }
        });
    }
    ch
}

/// A stage's choices and flags (the kind's own controls), then its numbers (the shared grid).
fn stage_rows(
    ui: &mut egui::Ui,
    f: &mut FxInstance,
    st: &Stage,
    names: &[String],
    bind: Option<&mut widgets::CurveBind<'_>>,
    pubc: Option<&widgets::PublishCtx>,
) -> widgets::ParamsChanged {
    let mut ch = widgets::ParamsChanged::default();
    let specs: Vec<_> = f.param_specs().iter().filter(|s| st.rows.contains(&&*s.name)).cloned().collect();
    for spec in &specs {
        match &spec.ty {
            ParamType::Choice { variants } => {
                let cur = f.get_param(&spec.name).unwrap_or(0.0).round() as usize;
                let cap = |i: usize| variants.get(i).and_then(|v| choice_label(&spec.name, v)).unwrap_or("?");
                ui.horizontal(|ui| {
                    ui.label(row_label(&spec.name).unwrap_or(""));
                    let combo = egui::ComboBox::from_id_salt(("isp", &*spec.name)).selected_text(cap(cur)).show_ui(ui, |ui| {
                        for i in 0..variants.len() {
                            let r = ui.selectable_label(i == cur, cap(i));
                            // the live scenario picks a variant by the program's own rectangle
                            widgets::field_probe::note(&format!("isp.choice.{}.{}", spec.name, variants[i]), r.rect);
                            if r.clicked() && i != cur {
                                f.set_param(&spec.name, i as f64);
                                ch.base = true;
                            }
                        }
                    });
                    widgets::field_probe::note(&format!("isp.combo.{}", spec.name), combo.response.rect);
                });
            }
            ParamType::Flag => {
                let mut v = f.get_param(&spec.name) == Some(1.0);
                if ui.checkbox(&mut v, row_label(&spec.name).unwrap_or("")).changed() {
                    f.set_param(&spec.name, if v { 1.0 } else { 0.0 });
                    ch.base = true;
                }
            }
            _ => {}
        }
    }
    let skip: Vec<&str> = names.iter().map(String::as_str).filter(|n| !st.rows.contains(n)).collect();
    ch |= widgets::params_grid_bound(ui, ("isp_grid", st.key), f, &skip, bind, pubc);
    ch
}

/// A row's caption (`widgets::param_label` falls back here).
pub(crate) fn row_label(name: &str) -> Option<&'static str> {
    Some(match name {
        "src_curve" => tr(k::isp::row::SRC_CURVE),
        "src_saturation" => tr(k::isp::row::SRC_SATURATION),
        "src_wb_r" => tr(k::isp::row::SRC_WB_R),
        "src_wb_b" => tr(k::isp::row::SRC_WB_B),
        "lens_blur" => tr(k::isp::row::LENS_BLUR),
        "distortion" => tr(k::isp::row::DISTORTION),
        "lateral_ca" => tr(k::isp::row::LATERAL_CA),
        "vignetting" => tr(k::isp::row::VIGNETTING),
        "color_shading" => tr(k::isp::row::COLOR_SHADING),
        "olpf" => tr(k::isp::row::OLPF),
        "ir_cut" => tr(k::isp::row::IR_CUT),
        "scene_ir" => tr(k::isp::row::SCENE_IR),
        "qe_ir" => tr(k::isp::row::QE_IR),
        "cfa" => tr(k::isp::row::CFA),
        "crosstalk" => tr(k::isp::row::CROSSTALK),
        "green_imbalance" => tr(k::isp::row::GREEN_IMBALANCE),
        "exposure_ev" => tr(k::isp::row::EXPOSURE_EV),
        "full_well" => tr(k::isp::row::FULL_WELL),
        "qe_r" => tr(k::isp::row::QE_R),
        "qe_g" => tr(k::isp::row::QE_G),
        "qe_b" => tr(k::isp::row::QE_B),
        "blooming" => tr(k::isp::row::BLOOMING),
        "read_noise" => tr(k::isp::row::READ_NOISE),
        "row_noise" => tr(k::isp::row::ROW_NOISE),
        "column_fpn" => tr(k::isp::row::COLUMN_FPN),
        "dark_current" => tr(k::isp::row::DARK_CURRENT),
        "dsnu" => tr(k::isp::row::DSNU),
        "prnu" => tr(k::isp::row::PRNU),
        "hot_pixels" => tr(k::isp::row::HOT_PIXELS),
        "dead_pixels" => tr(k::isp::row::DEAD_PIXELS),
        "sensor_seed" => tr(k::isp::row::SENSOR_SEED),
        "analog_gain" => tr(k::isp::row::ANALOG_GAIN),
        "black_level" => tr(k::isp::row::BLACK_LEVEL),
        "adc_bits" => tr(k::isp::row::ADC_BITS),
        "black_error" => tr(k::isp::row::BLACK_ERROR),
        "rowcol_strength" => tr(k::isp::row::ROWCOL_STRENGTH),
        "dpc_threshold" => tr(k::isp::row::DPC_THRESHOLD),
        "geq_strength" => tr(k::isp::row::GEQ_STRENGTH),
        "raw_nr_strength" => tr(k::isp::row::RAW_NR_STRENGTH),
        "ir_sub_strength" => tr(k::isp::row::IR_SUB_STRENGTH),
        "lsc_strength" => tr(k::isp::row::LSC_STRENGTH),
        "color_lsc_strength" => tr(k::isp::row::COLOR_LSC_STRENGTH),
        "wb_r_error" => tr(k::isp::row::WB_R_ERROR),
        "wb_b_error" => tr(k::isp::row::WB_B_ERROR),
        "digital_gain_ev" => tr(k::isp::row::DIGITAL_GAIN_EV),
        "isp_phase" => tr(k::isp::row::ISP_PHASE),
        "demosaic" => tr(k::isp::row::DEMOSAIC),
        "dewarp_strength" => tr(k::isp::row::DEWARP_STRENGTH),
        "ca_corr_strength" => tr(k::isp::row::CA_CORR_STRENGTH),
        "highlight_strength" => tr(k::isp::row::HIGHLIGHT_STRENGTH),
        "ccm_strength" => tr(k::isp::row::CCM_STRENGTH),
        "curve" => tr(k::isp::row::CURVE),
        "luma_nr" => tr(k::isp::row::LUMA_NR),
        "chroma_nr" => tr(k::isp::row::CHROMA_NR),
        "fcs_strength" => tr(k::isp::row::FCS_STRENGTH),
        "purple_strength" => tr(k::isp::row::PURPLE_STRENGTH),
        "sharpen_amount" => tr(k::isp::row::SHARPEN_AMOUNT),
        "matrix" => tr(k::isp::row::MATRIX),
        "chroma" => tr(k::isp::row::CHROMA),
        "decode_matrix" => tr(k::isp::row::DECODE_MATRIX),
        "view" => tr(k::isp::row::VIEW),
        _ => return None,
    })
}

/// A stage's title.
fn stage_title(key: &str) -> &'static str {
    match key {
        "source" => tr(k::isp::stage::SOURCE),
        "optics" => tr(k::isp::stage::OPTICS),
        "infrared" => tr(k::isp::stage::INFRARED),
        "sensor" => tr(k::isp::stage::SENSOR),
        "noise" => tr(k::isp::stage::NOISE),
        "readout" => tr(k::isp::stage::READOUT),
        "blc" => tr(k::isp::stage::BLC),
        "rowcol" => tr(k::isp::stage::ROWCOL),
        "dpc" => tr(k::isp::stage::DPC),
        "geq" => tr(k::isp::stage::GEQ),
        "raw_nr" => tr(k::isp::stage::RAW_NR),
        "ir_sub" => tr(k::isp::stage::IR_SUB),
        "lsc" => tr(k::isp::stage::LSC),
        "color_lsc" => tr(k::isp::stage::COLOR_LSC),
        "wb" => tr(k::isp::stage::WB),
        "demosaic" => tr(k::isp::stage::DEMOSAIC),
        "dewarp" => tr(k::isp::stage::DEWARP),
        "ca_corr" => tr(k::isp::stage::CA_CORR),
        "highlight" => tr(k::isp::stage::HIGHLIGHT),
        "ccm" => tr(k::isp::stage::CCM),
        "tone" => tr(k::isp::stage::TONE),
        "denoise" => tr(k::isp::stage::DENOISE),
        "fcs" => tr(k::isp::stage::FCS),
        "purple" => tr(k::isp::stage::PURPLE),
        "sharpen" => tr(k::isp::stage::SHARPEN),
        "yuv" => tr(k::isp::stage::YUV),
        _ => "",
    }
}

/// A block's title.
fn block_title(key: &str) -> &'static str {
    match key {
        "source" => tr(k::isp::block::SOURCE),
        "lens" => tr(k::isp::block::LENS),
        "sensor" => tr(k::isp::block::SENSOR),
        "raw" => tr(k::isp::block::RAW),
        "demosaic" => tr(k::isp::block::DEMOSAIC),
        "lens_correction" => tr(k::isp::block::LENS_CORRECTION),
        "colour" => tr(k::isp::block::COLOUR),
        "yuv" => tr(k::isp::block::YUV),
        _ => "",
    }
}

/// What a correcting stage removes (its pair among the defects), shown under its title.
fn pair_note(key: &str) -> Option<&'static str> {
    Some(match key {
        "blc" => tr(k::isp::pair::BLC),
        "rowcol" => tr(k::isp::pair::ROWCOL),
        "dpc" => tr(k::isp::pair::DPC),
        "geq" => tr(k::isp::pair::GEQ),
        "raw_nr" => tr(k::isp::pair::RAW_NR),
        "ir_sub" => tr(k::isp::pair::IR_SUB),
        "lsc" => tr(k::isp::pair::LSC),
        "color_lsc" => tr(k::isp::pair::COLOR_LSC),
        "dewarp" => tr(k::isp::pair::DEWARP),
        "ca_corr" => tr(k::isp::pair::CA_CORR),
        "highlight" => tr(k::isp::pair::HIGHLIGHT),
        "ccm" => tr(k::isp::pair::CCM),
        "denoise" => tr(k::isp::pair::DENOISE),
        "fcs" => tr(k::isp::pair::FCS),
        "purple" => tr(k::isp::pair::PURPLE),
        "sharpen" => tr(k::isp::pair::SHARPEN),
        _ => return None,
    })
}

/// A choice row's variant caption.
pub(crate) fn choice_label(row: &str, key: &str) -> Option<&'static str> {
    Some(match (row, key) {
        ("src_curve", "srgb") => tr(k::isp::choice::SRC_CURVE_SRGB),
        ("src_curve", "bt709") => tr(k::isp::choice::SRC_CURVE_BT709),
        ("src_curve", "gamma22") => tr(k::isp::choice::SRC_CURVE_GAMMA22),
        ("src_curve", "linear") => tr(k::isp::choice::SRC_CURVE_LINEAR),
        ("cfa", "rggb") => tr(k::isp::choice::CFA_RGGB),
        ("cfa", "bggr") => tr(k::isp::choice::CFA_BGGR),
        ("cfa", "grbg") => tr(k::isp::choice::CFA_GRBG),
        ("cfa", "gbrg") => tr(k::isp::choice::CFA_GBRG),
        ("cfa", "quad_bayer") => tr(k::isp::choice::CFA_QUAD_BAYER),
        ("cfa", "xtrans") => tr(k::isp::choice::CFA_XTRANS),
        ("cfa", "rgbw") => tr(k::isp::choice::CFA_RGBW),
        ("cfa", "rgbir") => tr(k::isp::choice::CFA_RGBIR),
        ("isp_phase", "exact") => tr(k::isp::choice::ISP_PHASE_EXACT),
        ("isp_phase", "shift_x") => tr(k::isp::choice::ISP_PHASE_SHIFT_X),
        ("isp_phase", "shift_y") => tr(k::isp::choice::ISP_PHASE_SHIFT_Y),
        ("isp_phase", "shift_xy") => tr(k::isp::choice::ISP_PHASE_SHIFT_XY),
        ("demosaic", "bilinear") => tr(k::isp::choice::DEMOSAIC_BILINEAR),
        ("demosaic", "mhc") => tr(k::isp::choice::DEMOSAIC_MHC),
        ("demosaic", "edge") => tr(k::isp::choice::DEMOSAIC_EDGE),
        ("curve", "srgb") => tr(k::isp::choice::CURVE_SRGB),
        ("curve", "bt709") => tr(k::isp::choice::CURVE_BT709),
        ("curve", "gamma22") => tr(k::isp::choice::CURVE_GAMMA22),
        ("matrix", "bt601") => tr(k::isp::choice::MATRIX_BT601),
        ("matrix", "bt709") => tr(k::isp::choice::MATRIX_BT709),
        ("chroma", "yuv444") => tr(k::isp::choice::CHROMA_YUV444),
        ("chroma", "yuv422") => tr(k::isp::choice::CHROMA_YUV422),
        ("chroma", "yuv420") => tr(k::isp::choice::CHROMA_YUV420),
        ("decode_matrix", "same") => tr(k::isp::choice::DECODE_MATRIX_SAME),
        ("decode_matrix", "bt601") => tr(k::isp::choice::DECODE_MATRIX_BT601),
        ("decode_matrix", "bt709") => tr(k::isp::choice::DECODE_MATRIX_BT709),
        ("view", "output") => tr(k::isp::choice::VIEW_OUTPUT),
        ("view", "scene") => tr(k::isp::choice::VIEW_SCENE),
        ("view", "optics") => tr(k::isp::choice::VIEW_OPTICS),
        ("view", "pixel") => tr(k::isp::choice::VIEW_PIXEL),
        ("view", "raw") => tr(k::isp::choice::VIEW_RAW),
        ("view", "raw_front") => tr(k::isp::choice::VIEW_RAW_FRONT),
        ("view", "raw_nr") => tr(k::isp::choice::VIEW_RAW_NR),
        ("view", "raw_corrected") => tr(k::isp::choice::VIEW_RAW_CORRECTED),
        ("view", "demosaic") => tr(k::isp::choice::VIEW_DEMOSAIC),
        ("view", "lens_corrected") => tr(k::isp::choice::VIEW_LENS_CORRECTED),
        ("view", "luma") => tr(k::isp::choice::VIEW_LUMA),
        ("view", "chroma") => tr(k::isp::choice::VIEW_CHROMA),
        _ => return None,
    })
}

/// Every row the window draws (bypasses and stage rows), for t1219.
#[cfg(test)]
pub(crate) fn all_rows() -> Vec<&'static str> {
    let mut v = vec!["view"];
    for (_, stages) in BLOCKS {
        for st in *stages {
            v.extend(st.bypass);
            v.extend(st.rows.iter().copied());
        }
    }
    v
}
