//! Common widgets for input fields (R.33 C2).
//!
//! Until this round each window drew its parameters itself: the same
//! values lived under a slider in one place and under a drag field in another, and
//! their ranges diverged between the FX window, the morpher window and the model spec
//! (where they are the real ones). The user’s requirement: “the ui of the fields
//! limps everywhere”, “a revision across all input fields”.
//!
//! Hence the rule: a numeric effect parameter is drawn ONLY by these
//! functions, and the range, step and unit are taken from `ParamSpec` — one
//! source of truth in kt-model. There is now no room for them to diverge.

use crate::i18n::{k, tr};
use eframe::egui;
use kt_model::{FxInstance, HasParams, ParamKind, ParamSpec, ParamType, ParamUnit};

/// Unit suffix: a value with a unit explains itself, so the
/// caption stays short (the rule “do not make obvious captions”).
fn unit_suffix(u: ParamUnit) -> &'static str {
    match u {
        ParamUnit::Db => " dB",
        ParamUnit::Ms => " ms",
        ParamUnit::Hz => " Hz",
        ParamUnit::Deg => "°",
        ParamUnit::Px => " px",
        ParamUnit::St => " st",
        ParamUnit::Sec => " s",
        ParamUnit::Percent => " %",
        ParamUnit::None
        | ParamUnit::Norm
        | ParamUnit::Ratio
        | ParamUnit::Rgb8
        | ParamUnit::Count => "",
    }
}

/// The localised caption of a parameter of ITS OWN area (R.53, item 414).
///
/// One name means different things on different carriers: `size` is the grain in the
/// Noise effect and the FONT SIZE of a title. The global map lied about this, so
/// the area is asked first, and only then the common table.
pub fn param_label_scoped(scope: kt_model::ParamScope, name: &str) -> String {
    if scope == kt_model::ParamScope::Generator
        && let Some(s) = generator_param_label(name)
    {
        return s.to_string();
    }
    param_label(name)
}

/// Captions of GENERATOR parameters. `None` — the name is shared with effects and
/// is taken from the common table.
fn generator_param_label(name: &str) -> Option<&'static str> {
    Some(match name {
        "size" => tr(k::genp::SIZE),
        "line_spacing" => tr(k::genp::LINE_SPACING),
        "tracking" => tr(k::genp::TRACKING),
        "stroke_w" => tr(k::genp::STROKE_W),
        "shadow_dx" => tr(k::genp::SHADOW_DX),
        "shadow_dy" => tr(k::genp::SHADOW_DY),
        "shadow_blur" => tr(k::genp::SHADOW_BLUR),
        "plate_pad_x" => tr(k::genp::PLATE_PAD_X),
        "plate_pad_y" => tr(k::genp::PLATE_PAD_Y),
        "angle_deg" => tr(k::genp::ANGLE_DEG),
        "cols" => tr(k::genp::COLS),
        "tone_db" => tr(k::genp::TONE_DB),
        "fps" => tr(k::genp::FPS),
        // R.53 (items 419-422): shapes, countdown, level window
        "cx" => tr(k::genp::CX),
        "cy" => tr(k::genp::CY),
        "w" => tr(k::genp::W),
        "h" => tr(k::genp::H),
        "radius" => tr(k::genp::RADIUS),
        "feather" => tr(k::genp::FEATHER),
        "x1" => tr(k::genp::X1),
        "y1" => tr(k::genp::Y1),
        "x2" => tr(k::genp::X2),
        "y2" => tr(k::genp::Y2),
        "thickness" => tr(k::genp::THICKNESS),
        "seconds" => tr(k::genp::SECONDS),
        "beep_db" => tr(k::genp::BEEP_DB),
        "area" => tr(k::genp::AREA),
        "level" => tr(k::genp::LEVEL),
        // R.59: the title text is the same kind of property as the others
        "text" => tr(k::genp::TEXT),
        // R.59, stage 2: title animation — ordinary numbers of the kind
        "reveal" => tr(k::genp::REVEAL),
        "reveal_soft" => tr(k::genp::REVEAL_SOFT),
        "offset_x" => tr(k::genp::OFFSET_X),
        "offset_y" => tr(k::genp::OFFSET_Y),
        _ => return None,
    })
}

/// Colour eyedropper + HEX field for the DIRECT fields of the carrier (R.53, item 415).
///
/// For a title, colours are not a `ParamSpec` and never were (“the first lawful exception”,
/// F-TXT-001), so the path through `get_param` silently returns zero for them: this is how in R.53
/// the title fill became transparent black, and the title stopped being drawn at all.
/// A live scenario saw this,
/// the harness did not.
pub fn color_row_rgba(
    ui: &mut egui::Ui,
    label: &str,
    rgba: (&mut u8, &mut u8, &mut u8, &mut u8),
    probe: Option<&str>,
) -> bool {
    let (r, g, b, a) = rgba;
    let mut changed = false;
    let mut col = egui::Color32::from_rgba_unmultiplied(*r, *g, *b, *a);
    ui.horizontal(|ui| {
        ui.label(label);
        let resp = ui.color_edit_button_srgba(&mut col);
        if let Some(name) = probe {
            field_probe::note(name, resp.rect);
        }
        if resp.changed() {
            changed = true;
        }
        // HEX field: the canonical path form (rrggbbaa) and the same technique.
        // DIRECT components: `Color32` is premultiplied inside, and
        // `col.r()` on a semi-transparent colour would return a darkened byte —
        // the colour would darken on every opening of the window.
        let s = col.to_srgba_unmultiplied();
        let shown = format!("{:02x}{:02x}{:02x}{:02x}", s[0], s[1], s[2], s[3]);
        let id = ui.id().with(("hexedit", label.len(), probe.unwrap_or("")));
        let mut text = ui
            .memory_mut(|m| m.data.get_temp::<String>(id))
            .filter(|_| ui.memory(|m| m.has_focus(id)))
            .unwrap_or_else(|| shown.clone());
        let te =
            ui.add(egui::TextEdit::singleline(&mut text).id(id).desired_width(72.0).char_limit(8));
        if let Some(name) = probe {
            field_probe::note(&format!("{name}.hex"), te.rect);
        }
        if te.changed() {
            let hex = text.trim().trim_start_matches('#');
            if hex.len() == 8
                && hex.bytes().all(|c| c.is_ascii_hexdigit())
                && let Ok(v) = u32::from_str_radix(hex, 16)
            {
                col = egui::Color32::from_rgba_unmultiplied(
                    (v >> 24) as u8,
                    (v >> 16) as u8,
                    (v >> 8) as u8,
                    v as u8,
                );
                changed = true;
            }
            ui.memory_mut(|m| m.data.insert_temp(id, text.clone()));
        }
        if changed {
            let s = col.to_srgba_unmultiplied();
            *r = s[0];
            *g = s[1];
            *b = s[2];
            *a = s[3];
        }
    });
    changed
}

/// Colour eyedropper + HEX field (R.53, item 415). The canonical path form is hex
/// (`c=rrggbb`), and until this item the person saw it in the model but
/// could not enter it: the egui picker does not contain a hex field.
///
/// `alpha` — the name of the fourth component, if the colour has transparency.
pub fn color_row<P: HasParams + ?Sized>(
    ui: &mut egui::Ui,
    label: &str,
    f: &mut P,
    names: [&str; 3],
    alpha: Option<&str>,
    probe: Option<&str>,
    pubc: Option<&PublishCtx>,
) -> bool {
    let mut changed = false;
    // The base of a colour’s name — ITS ADDRESS as a property (R.59): the channels
    // `<stem>_r/_g/_b/_a` are ordinary numeric series, and the colour is derived from
    // them (`derive_color_groups`), which is why it is published by its stem.
    let stem = names[0].strip_suffix("_r").unwrap_or(names[0]).to_string();
    // NO DOOR WHERE ONE MUST NOT ENTER (rule 9). For a title, colours are not
    // properties at all (“the first lawful exception” F-TXT-001): they lie
    // right in the variant, bypassing the spec, and publishing on them would die in
    // validation. We ask the carrier itself, not the list of exceptions.
    let pubc = pubc.filter(|_| f.value_type(&stem).is_some());
    ui.horizontal(|ui| {
        pub_label(ui, label, pubc, &stem, kt_model::ParamType::Color);
        let get = |n: &str| f.get_param(n).unwrap_or(0.0).round().clamp(0.0, 255.0) as u8;
        let (r, g, b) = (get(names[0]), get(names[1]), get(names[2]));
        let a = alpha.map(&get).unwrap_or(255);
        let mut col = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
        let resp = match alpha {
            Some(_) => ui.color_edit_button_srgba(&mut col),
            None => {
                let mut rgb = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0];
                let resp = ui.color_edit_button_rgb(&mut rgb);
                col = egui::Color32::from_rgb(
                    (rgb[0] * 255.0).round() as u8,
                    (rgb[1] * 255.0).round() as u8,
                    (rgb[2] * 255.0).round() as u8,
                );
                resp
            }
        };
        if let Some(name) = probe {
            field_probe::note(name, resp.rect);
        }
        // DIRECT components: `Color32` is premultiplied inside (see
        // `color_row_rgba`), and on a semi-transparent colour `col.r()` would return
        // a darkened byte
        let straight = col.to_srgba_unmultiplied();
        if resp.changed() {
            for (n, v) in names.iter().zip(straight) {
                f.set_param(n, v as f64);
            }
            if let Some(n) = alpha {
                f.set_param(n, straight[3] as f64);
            }
            changed = true;
        }
        // HEX field: shows the canonical path form and accepts the same
        let shown = match alpha {
            Some(_) => format!(
                "{:02x}{:02x}{:02x}{:02x}",
                straight[0], straight[1], straight[2], straight[3]
            ),
            None => format!("{:02x}{:02x}{:02x}", straight[0], straight[1], straight[2]),
        };
        let mut text = ui
            .memory_mut(|m| m.data.get_temp::<String>(ui.id().with(("hex", names[0]))))
            .unwrap_or_else(|| shown.clone());
        if !ui.memory(|m| m.has_focus(ui.id().with(("hexedit", names[0])))) {
            text = shown.clone();
        }
        let te = ui.add(
            egui::TextEdit::singleline(&mut text)
                .id(ui.id().with(("hexedit", names[0])))
                .desired_width(72.0)
                .char_limit(8),
        );
        if let Some(name) = probe {
            // a hex field is a separate probe — the live scenario has to TYPE
            // a colour, not poke at the system picker
            field_probe::note(&format!("{name}.hex"), te.rect);
        }
        if te.changed() {
            let hex = text.trim().trim_start_matches('#');
            let ok = hex.len() == names.len() * 2 || (alpha.is_some() && hex.len() == 8);
            if ok
                && hex.bytes().all(|b| b.is_ascii_hexdigit())
                && let Ok(v) = u32::from_str_radix(hex, 16)
            {
                let (r, g, b, a) = if hex.len() == 8 {
                    ((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8)
                } else {
                    ((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
                };
                for (n, x) in names.iter().zip([r, g, b]) {
                    f.set_param(n, x as f64);
                }
                if let Some(n) = alpha {
                    f.set_param(n, a as f64);
                }
                changed = true;
            }
            ui.memory_mut(|m| m.data.insert_temp(ui.id().with(("hex", names[0])), text.clone()));
        }
    });
    changed
}

/// Flag caption (R.53, item 414).
pub fn flag_label(name: &str) -> String {
    let s: &str = match name {
        "mono" => tr(k::genp::MONO),
        "tone" => tr(k::genp::TONE),
        "beep" => tr(k::genp::BEEP),
        "bold" => tr(k::titlew::BOLD),
        "reverse" => tr(k::trn::REVERSE),
        "vertical" => tr(k::trn::VERTICAL),
        "invert" => tr(k::trn::INVERT),
        other => return other.to_string(),
    };
    s.to_string()
}

/// The carrier’s flag row — ONE widget for all windows (R.53, item 414). Until it,
/// “yes/no” were drawn by hand in each window and so were forgotten:
/// there was no way to switch on the 1 kHz tone or the coloured noise.
pub fn flags_row<P: HasParams + ?Sized>(
    ui: &mut egui::Ui,
    _id: impl std::hash::Hash + std::fmt::Debug,
    f: &mut P,
) -> bool {
    let specs = f.flag_specs();
    if specs.is_empty() {
        return false;
    }
    let mut changed = false;
    ui.horizontal(|ui| {
        for spec in specs {
            let mut v = f.get_flag(spec.name).unwrap_or(spec.default);
            let r = ui.checkbox(&mut v, flag_label(spec.name));
            field_probe::note(spec.name, r.rect);
            if r.changed() {
                f.set_flag(spec.name, v);
                changed = true;
            }
        }
    });
    changed
}

/// The localised parameter caption. The field name is the key, the translation lives in
/// the catalogues: kt-model knows nothing about the interface language.
pub fn param_label(name: &str) -> String {
    let s: &str = match name {
        "db" => tr(k::fxp::DB),
        "rate" => tr(k::fxp::RATE),
        "mix" => tr(k::fxp::MIX),
        "reverb_delay_ms" => tr(k::fxp::DELAY),
        "damp" => tr(k::fxp::DAMP),
        "decay" => tr(k::fxp::DECAY),
        "wet_gain" => tr(k::fxp::WET_GAIN),
        // R.47 audio effects (Ka2)
        "cutoff_hz" => tr(k::fxp::CUTOFF_HZ),
        "resonance" => tr(k::fxp::RESONANCE),
        "low_gain_db" => tr(k::fxp::LOW_GAIN_DB),
        "low_freq_hz" => tr(k::fxp::LOW_FREQ_HZ),
        "mid_gain_db" => tr(k::fxp::MID_GAIN_DB),
        "mid_freq_hz" => tr(k::fxp::MID_FREQ_HZ),
        "mid_q" => tr(k::fxp::MID_Q),
        "high_gain_db" => tr(k::fxp::HIGH_GAIN_DB),
        "high_freq_hz" => tr(k::fxp::HIGH_FREQ_HZ),
        "drive_db" => tr(k::fxp::DRIVE_DB),
        "tone" => tr(k::fxp::TONE),
        "bits" => tr(k::fxp::BITS),
        "rate_hz" => tr(k::fxp::RATE_HZ),
        "depth" => tr(k::fxp::DEPTH),
        "depth_ms" => tr(k::fxp::DEPTH_MS),
        "feedback" => tr(k::fxp::FEEDBACK),
        "voices" => tr(k::fxp::VOICES),
        "stages" => tr(k::fxp::STAGES),
        "time_ms" => tr(k::fxp::TIME_MS),
        "threshold_db" => tr(k::fxp::THRESHOLD_DB),
        "ratio" => tr(k::fxp::RATIO),
        "attack_ms" => tr(k::fxp::ATTACK_MS),
        "release_ms" => tr(k::fxp::RELEASE_MS),
        "hold_ms" => tr(k::fxp::HOLD_MS),
        "makeup_db" => tr(k::fxp::MAKEUP_DB),
        // R.48 limiter
        "ceiling_db" => tr(k::fxp::CEILING_DB),
        "lookahead_ms" => tr(k::fxp::LOOKAHEAD_MS),
        // R.48 pitch
        "semitones" => tr(k::fxp::SEMITONES),
        "cents" => tr(k::fxp::CENTS),
        "pan_x" => tr(k::fxp::PAN_X),
        "pan_y" => tr(k::fxp::PAN_Y),
        "zoom" => tr(k::fxp::ZOOM),
        "time_scale" => tr(k::fxp::TIME_SCALE),
        "radius" => tr(k::fx::RADIUS),
        // new parameters of R.33 C5
        "radius_y" => tr(k::fxp::RADIUS_Y),
        "thickness" => tr(k::fxp::THICKNESS),
        "size" => tr(k::fxp::SIZE),
        "scan_step" => tr(k::fxp::SCAN_STEP),
        "jitter" => tr(k::fxp::JITTER),
        "spill" => tr(k::fxp::SPILL),
        "pan" => tr(k::fxp::PAN),
        "predelay_ms" => tr(k::fxp::PREDELAY_MS),
        "width" => tr(k::fxp::WIDTH),
        "key_r" => tr(k::fxp::KEY_R),
        "key_g" => tr(k::fxp::KEY_G),
        "key_b" => tr(k::fxp::KEY_B),
        "tolerance" => tr(k::fx::TOLERANCE),
        "softness" => tr(k::fx::SOFTNESS),
        "threshold" => tr(k::fx::THRESHOLD),
        "amount" => tr(k::fxp::AMOUNT),
        // parameters of the transition kinds (R.33 C4)
        "angle" => tr(k::fxp::ANGLE),
        "turns" => tr(k::fxp::TURNS),
        "blur" => tr(k::fxp::BLUR),
        "rgb_shift" => tr(k::fx::RGB_SHIFT),
        "bleed" => tr(k::fxp::BLEED),
        "strength" => tr(k::fx::STRENGTH),
        "cols" => tr(k::fx::COLUMNS),
        "rows" => tr(k::fx::ROWS),
        "pos_x" => tr(k::fxp::POS_X),
        "pos_y" => tr(k::fxp::POS_Y),
        "pos_z" => tr(k::fxp::POS_Z),
        "rot_x" => tr(k::fxp::ROT_X),
        "rot_y" => tr(k::fxp::ROT_Y),
        "rot_z" => tr(k::fxp::ROT_Z),
        "anchor_x" => tr(k::fxp::ANCHOR_X),
        "anchor_y" => tr(k::fxp::ANCHOR_Y),
        // the “Geometry” node (R.62)
        "crop_l" => tr(k::fxp::CROP_L),
        "crop_t" => tr(k::fxp::CROP_T),
        "crop_r" => tr(k::fxp::CROP_R),
        "crop_b" => tr(k::fxp::CROP_B),
        "rot" => tr(k::fxp::ROT),
        "scale" => tr(k::fxp::SCALE),
        "fov_deg" => tr(k::fxp::FOV_DEG),
        // R.47 video effects (Kv2)
        "brightness" => tr(k::fxp::BRIGHTNESS),
        "contrast" => tr(k::fxp::CONTRAST),
        "saturation" => tr(k::fxp::SATURATION),
        "hue" => tr(k::fxp::HUE),
        "levels" => tr(k::fxp::LEVELS),
        "x0" => tr(k::fxp::X0),
        "y0" => tr(k::fxp::Y0),
        "x1" => tr(k::fxp::X1),
        "y1" => tr(k::fxp::Y1),
        "x2" => tr(k::fxp::X2),
        "y2" => tr(k::fxp::Y2),
        "x3" => tr(k::fxp::X3),
        "y3" => tr(k::fxp::Y3),
        // R.47 barriers (Kv3)
        "offset_x" => tr(k::fxp::OFFSET_X),
        "offset_y" => tr(k::fxp::OFFSET_Y),
        "opacity" => tr(k::fxp::OPACITY),
        "center_x" => tr(k::fxp::CENTER_X),
        "center_y" => tr(k::fxp::CENTER_Y),
        // R.47 geo (Kv4)
        "k1" => tr(k::fxp::K1),
        "k2" => tr(k::fxp::K2),
        "freq" => tr(k::fxp::FREQ),
        "phase" => tr(k::fxp::PHASE),
        "segments" => tr(k::fxp::SEGMENTS),
        "curvature" => tr(k::fxp::CURVATURE),
        // parameters of the R.51 transition kinds: the gesture centre, volumetric panels
        // (caught by t508 — they were shown as raw English names)
        "cx" => tr(k::fxp::CX),
        "cy" => tr(k::fxp::CY),
        "count" => tr(k::fxp::COUNT),
        "persp" => tr(k::fxp::PERSP),
        "shade" => tr(k::fxp::SHADE),
        "curl" => tr(k::fxp::CURL),
        "shadow" => tr(k::fxp::SHADOW),
        "steps" => tr(k::fxp::STEPS),
        // a parameter without a caption in the catalogue — visible at once, not silently
        other => return other.to_string(),
    };
    s.to_string()
}

/// THE WINDOW BODY IN A SCROLL, the buttons — below it (R.66, form contract).
///
/// A window that has outgrown the screen has no right to hide its actions: in R.65
/// this is exactly how the track window’s “Close” and “Cancel” went past the edge, and
/// a test caught it, not the eye. The scroll height is a fraction of the screen, so that on
/// any laptop the buttons remain visible.
pub fn window_body<R>(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    // The screen fraction is the ceiling; a window with a SET height, in addition,
    // leaves room for the buttons below it (R.67): otherwise the window scrolled
    // itself too, and “Done” went past the bottom edge — the curve block had nothing
    // to close it with (live scenario `curves`).
    let screen = (ui.ctx().content_rect().height() * 0.62).max(240.0);
    let in_window = ui.available_height() - ACTIONS_H;
    let max_h = if in_window.is_finite() { screen.min(in_window).max(160.0) } else { screen };
    egui::ScrollArea::vertical()
        .max_height(max_h)
        .auto_shrink([true, true])
        .show(ui, body)
        .inner
}

/// How much of the window height the action button row keeps for itself: a
/// separator plus the row of buttons. One number per programme — `window_body` subtracts it,
/// and `window_actions` draws within it.
const ACTIONS_H: f32 = 34.0;

/// A COLLAPSIBLE form group — the sample is “Transform” (R.64/R.66).
///
/// The collapse state lives in egui memory by `id`, so it is the state of the WINDOW,
/// not of the project: it does not go into the file and does not go through undo.
pub fn group<R>(
    ui: &mut egui::Ui,
    id: &str,
    title: &str,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    let r = egui::CollapsingHeader::new(title)
        .id_salt(("form_group", id))
        .default_open(true)
        .show(ui, body);
    // the group header is the GESTURE target: the live scenario cannot find a `CollapsingHeader`
    // in the accessibility tree, and it must not guess offsets (R.27)
    field_probe::note(&format!("grp.{id}"), r.header_response.rect);
    r.body_returned
}

/// ACTIONS row of the window — BELOW the scrolling body. A separate call, not
/// a second closure: both closures would hold the same borrow
/// (the hint accumulator), and the borrow checker is right.
pub fn window_actions(ui: &mut egui::Ui, actions: impl FnOnce(&mut egui::Ui)) {
    ui.separator();
    actions(ui);
}

/// Row of a NUMBER WITHOUT A SPEC (R.66).
///
/// Not every window quantity has a `ParamSpec`: morpher shift in ms,
/// camera duration in seconds, timeline fields — these are model and
/// UI properties, not effect parameters, and inventing a spec for them would
/// misstate their nature. Yet they MUST be drawn in the SAME row, while the limits,
/// step and unit MUST come from ONE place next to the quantity’s definition,
/// not from the drawing code (otherwise one and the same number in two
/// windows would get different bounds — the R.33 defect with Gain −24…24 versus −48…24).
pub struct NumRow<'a> {
    /// Label on the left. Empty — a row without a label (for example the second in a pair).
    pub label: &'a str,
    /// Rectangle name for live scenarios (`field_probe`).
    pub probe: &'a str,
    pub unit: ParamUnit,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// Width of the slider next to the number; `None` — number only.
    pub slider: Option<f32>,
}

impl NumRow<'_> {
    fn spec(&self) -> ParamSpec {
        ParamSpec {
            name: std::borrow::Cow::Borrowed(""),
            ty: ParamType::Num,
            min: self.min,
            max: self.max,
            default: self.min,
            step: self.step,
            unit: self.unit,
            kind: ParamKind::Smooth,
            label: std::borrow::Cow::Borrowed(""),
        }
    }
}

/// Draw a row of a number without a spec. Returns a RESPONSE, not “touched”:
/// the caller sometimes needs “released” too (sorting keys on release,
/// R.33), and “focus left”.
pub fn plain_row(ui: &mut egui::Ui, row: &NumRow<'_>, v: &mut f64) -> egui::Response {
    if !row.label.is_empty() {
        ui.label(row.label);
    }
    let spec = row.spec();
    let r = param_edit(ui, &spec, v, row.slider);
    field_probe::note(row.probe, r.rect);
    r
}

/// A PAIR of values on one line (X/Y, width/height) — the model for
/// “Transform” (R.64): two numbers of one quantity are read together, and
/// splitting them across lines wastes space (rule 13).
pub fn pair_row(
    ui: &mut egui::Ui,
    label: &str,
    a: (&NumRow<'_>, &mut f64),
    b: (&NumRow<'_>, &mut f64),
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        if !label.is_empty() {
            ui.label(label);
        }
        changed |= plain_row(ui, a.0, a.1).changed();
        changed |= plain_row(ui, b.0, b.1).changed();
    });
    changed
}

/// Unified numeric control of a parameter. Range, step and unit — from the spec.
pub fn param_value(ui: &mut egui::Ui, spec: &ParamSpec, v: &mut f64) -> egui::Response {
    // Fraction shown as a percentage (R.53, item 415): the conversion lives HERE and
    // only here — in the model, in the file and in the curves the value stays a fraction.
    if spec.unit == ParamUnit::Percent {
        let mut pct = *v * 100.0;
        let step = spec.step * 100.0;
        let mut dv = egui::DragValue::new(&mut pct)
            .range(spec.min * 100.0..=spec.max * 100.0)
            .speed(step)
            .suffix(" %");
        if step >= 1.0 {
            dv = dv.max_decimals(0);
        } else {
            dv = dv.max_decimals(1);
        }
        let r = ui.add(dv);
        if r.changed() {
            *v = pct / 100.0;
        }
        return r;
    }
    let mut dv = egui::DragValue::new(v).range(spec.min..=spec.max).speed(spec.step);
    let suffix = unit_suffix(spec.unit);
    if !suffix.is_empty() {
        dv = dv.suffix(suffix);
    }
    // integer parameters do not show the fractional part
    if spec.step >= 1.0 {
        dv = dv.max_decimals(0);
    }
    ui.add(dv)
}

/// A number plus an optional slider — ONE value and one answer
/// “touched”. The order (number first, then slider) is fixed: the field stays
/// where it is for parameters without a slider, and the form does not “jump”
/// (rule 3 of the checklist).
fn param_edit(
    ui: &mut egui::Ui,
    spec: &ParamSpec,
    v: &mut f64,
    slider: Option<f32>,
) -> egui::Response {
    let r = param_value(ui, spec, v);
    match slider {
        None => r,
        Some(w) => {
            let h = r.rect.height();
            let s = ui.add_sized(
                [w, h],
                egui::Slider::new(v, spec.min..=spec.max).show_value(false),
            );
            r | s
        }
    }
}

/// Link between parameter fields and curves (R.38 items 15-16).
///
/// The claims, verbatim: “when pressing a keyframe — so that the values written in the fields
/// have a link with the keyframe” and “a feeling that effects have some separate
/// “default” state of values.”
///
/// The second is a consequence of the first: with a live curve the field was
/// the base rule, while the engine took the curve value. The base stayed invisible,
/// surfacing when the curve was removed and looking like a “separate state”.
pub struct CurveBind<'a> {
    pub curves: &'a mut Vec<kt_model::ParamCurve>,
    /// Selected key of the automation widget: (curve, index).
    pub sel: Option<(usize, usize)>,
    /// Cursor position IN THE CURVE DOMAIN — the key lands here when none
    /// is selected.
    pub at: kt_time::Tick,
}

impl CurveBind<'_> {
    fn index_of(&self, param: &str) -> Option<usize> {
        self.curves.iter().position(|c| c.param == param)
    }

    /// Whether the parameter has a LIVE curve — this decides where both the
    /// field and its paired frequency-response graph (R.48) write.
    pub fn has_curve<P: HasParams + ?Sized>(&self, _f: &P, param: &str) -> bool {
        self.index_of(param).is_some()
    }

    /// What the field SHOWS for this parameter: the value of the selected key,
    /// the value of the curve under the cursor, or the base (R.38 item 15).
    pub fn shown<P: HasParams + ?Sized>(&self, f: &P, param: &str) -> f64 {
        let base = f.get_param(param).unwrap_or(0.0);
        match self.index_of(param) {
            Some(ci) => {
                let key = match self.sel {
                    Some((c, k)) if c == ci && k < self.curves[ci].keys.len() => Some(k),
                    _ => None,
                };
                bound_value(&self.curves[ci], key, self.at)
            }
            None => base,
        }
    }

    /// What the field EDITS — exactly what it shows.
    pub fn apply<P: HasParams + ?Sized>(&mut self, _f: &mut P, param: &str, v: f64) {
        let Some(ci) = self.index_of(param) else { return };
        let key = match self.sel {
            Some((c, k)) if c == ci && k < self.curves[ci].keys.len() => Some(k),
            _ => None,
        };
        let at = self.at;
        bound_apply(&mut self.curves[ci], key, at, v);
    }
}

/// What exactly touched the parameter field (R.43, item 291).
///
/// Previously both edits returned ONE `bool`, and the caller could not know
/// where to write the result: to `SetFx` or `SetFxAuto`.
/// `fx_editor` wrote only to `SetFx`, so a key edit by NUMBER died with the
/// frame and the field “reset to the previous value” — the user’s claim of
/// R.43 item 1. The morpher had one flag for both edit sources, so it worked
/// there.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ParamsChanged {
    /// Base of the effect touched (parameter without a live curve) → `SetFx`.
    pub base: bool,
    /// Curve touched (key of a parameter with a live curve) → `SetFxAuto`.
    pub curve: bool,
}

impl ParamsChanged {
    pub fn any(self) -> bool {
        self.base || self.curve
    }
    fn base() -> ParamsChanged {
        ParamsChanged { base: true, curve: false }
    }
    fn curve() -> ParamsChanged {
        ParamsChanged { base: false, curve: true }
    }
}

impl std::ops::BitOrAssign for ParamsChanged {
    fn bitor_assign(&mut self, o: ParamsChanged) {
        self.base |= o.base;
        self.curve |= o.curve;
    }
}

/// Menu item with a CHORD (R.49, item 362).
///
/// The chord is an argument, not part of the translatable phrase.
/// While it lived in the catalog value (“Delete (Del)”), one string served different
/// contexts, and in half of them the promise was false: in a NAMED REGION the Del key
/// deletes nothing (it deletes the selected events), yet the label promised it.
/// The translator also had it worse: the chord is not translated, yet it had to be
/// edited in five catalogs.
///
/// The separator is `\t`: egui places the label and the chord at the edges of the item,
/// as system menus do.
pub fn menu_item(ui: &mut egui::Ui, label: &str, accel: Option<&str>) -> bool {
    menu_item_probed(ui, label, accel).0
}

/// The same PLUS the item’s RECTANGLE: live scenarios have no way to find an
/// egui menu item (it is absent from the accessibility tree), and they must not guess offsets.
pub fn menu_item_probed(ui: &mut egui::Ui, label: &str, accel: Option<&str>) -> (bool, egui::Rect) {
    let r = match accel {
        Some(a) => ui.button(format!("{label}\t{a}")),
        None => ui.button(label),
    };
    (r.clicked(), r.rect)
}

/// Rectangles of the parameter fields of the last frame — ONLY for live scenarios
/// (`KT_GEOM_DUMP`, rule R.27 “coordinates are taken from the program ITSELF”).
/// `egui::DragValue` has no node in the accessibility tree, and its label cannot be
/// used to find it, neither by the harness nor by a script: without this list a scenario
/// on a parameter field would have to be written with guessed offsets inside the window.
/// In a normal run the branch costs one `OnceLock` check.
pub(crate) mod field_probe {
    use eframe::egui;
    use std::cell::RefCell;
    use std::sync::OnceLock;

    thread_local! {
        static RECTS: RefCell<Vec<(String, egui::Rect)>> = const { RefCell::new(Vec::new()) };
    }

    pub(crate) fn on() -> bool {
        static ON: OnceLock<bool> = OnceLock::new();
        *ON.get_or_init(|| std::env::var("KT_GEOM_DUMP").is_ok())
    }

    pub(crate) fn note(name: &str, r: egui::Rect) {
        if !on() {
            return;
        }
        RECTS.with(|v| {
            let mut v = v.borrow_mut();
            v.retain(|(n, _)| n != name);
            v.push((name.to_string(), r));
        });
    }

    /// Take the accumulated data (the geometry dump is written every 100 ms).
    pub(crate) fn take() -> Vec<(String, egui::Rect)> {
        RECTS.with(|v| std::mem::take(&mut *v.borrow_mut()))
    }
}

/// Hints from CLOSURES (R.53, item 415). Rule 10 requires exactly one place for a hint —
/// the bottom right of the status line — but the shared widgets (`param_row_bound`,
/// `color_row`) are drawn inside `Window::show`, where `&mut KtApp` is not available, and
/// threading the accumulator through seven signatures would mean changing every call just
/// for a line of text. The mechanism is the same as for `field_probe`:
/// the widget puts it in, and `KtApp` takes it out at the end of
/// the frame.
pub(crate) mod hint_sink {
    use std::cell::RefCell;

    thread_local! {
        static TEXT: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    pub(crate) fn put(text: &str) {
        TEXT.with(|t| *t.borrow_mut() = Some(text.to_string()));
    }

    /// Take the accumulated data (called by `draw_status_hint` at the end of the frame).
    pub(crate) fn take() -> Option<String> {
        TEXT.with(|t| t.borrow_mut().take())
    }
}

/// Human-readable name of the segment shape.
pub fn shape_label(s: kt_model::SegShape) -> &'static str {
    use kt_model::SegShape as S;
    match s {
        S::Linear => tr(k::fx::SHAPE_LINEAR),
        S::Smooth { .. } => tr(k::fx::SHAPE_SMOOTH),
        S::Hold => tr(k::fx::SHAPE_HOLD),
        S::EaseIn { .. } => tr(k::fx::SHAPE_EASE_IN),
        S::EaseOut { .. } => tr(k::fx::SHAPE_EASE_OUT),
        S::Zigzag { .. } => tr(k::fx::SHAPE_ZIGZAG),
        S::Formula { .. } => tr(k::fx::SHAPE_FORMULA),
    }
}

/// A closed set of shapes in display order. ONE list for the whole program:
/// two lists would diverge, and a shape forgotten in one would become unreachable.
pub fn all_shapes() -> [kt_model::SegShape; 6] {
    use kt_model::SegShape as S;
    [
        S::Linear,
        S::DEFAULT,
        S::Hold,
        S::EaseIn { tension: 1.0 },
        S::EaseOut { tension: 1.0 },
        S::Zigzag { periods: 4.0, amplitude: 0.1, profile: Default::default() },
    ]
}

/// “Shape of all segments of the curve” is a command over the curve, not a machine setting
/// (R.61): a machine setting would give “one file, two behaviors”.
///
/// The warp speed has only the straight shape: on a piecewise-linear profile
/// a closed stretch map stands. The refusal is LOUD — via the status line,
/// not a silently disabled control.
pub fn shape_all_control(
    ui: &mut egui::Ui,
    curve: &mut kt_model::ParamCurve,
    hover: &mut Option<String>,
) -> bool {
    let mut changed = false;
    let rate = curve.param == "rate";
    let cur = curve.keys.first().map(|k| k.shape).unwrap_or_default();
    ui.label(tr(k::fx::SHAPE_ALL));
    // width — measured by the LONGEST shape name (rule 3): the field
    // does not grow past the window and does not jump when the shape changes.
    let widest = all_shapes()
        .iter()
        .map(|s| {
            ui.painter()
                .layout_no_wrap(
                    shape_label(*s).to_string(),
                    egui::TextStyle::Button.resolve(ui.style()),
                    egui::Color32::WHITE,
                )
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    let r = egui::ComboBox::from_id_salt(("shape_all", curve.param.clone()))
        .width(widest + 28.0)
        .selected_text(shape_label(cur))
        .show_ui(ui, |ui| {
            for s in all_shapes() {
                if ui.selectable_label(s == cur, shape_label(s)).clicked() {
                    if rate && s != kt_model::SegShape::Linear {
                        *hover = Some(tr(k::fx::SHAPE_RATE_LINEAR).to_string());
                        continue;
                    }
                    for k in &mut curve.keys {
                        k.shape = s;
                    }
                    changed = true;
                }
            }
        });
    crate::app::hint_into(hover, r.response, tr(k::fx::SHAPE_ALL_HINT));
    // PARAMETERS of the shape — next to it: each kind has its own, and there is no common
    // “stiffness” number here (R.61, D2). The edit lands on all segments —
    // that is what “all segments of the shape” means.
    let mut next = cur;
    match &mut next {
        kt_model::SegShape::Smooth { tension }
        | kt_model::SegShape::EaseIn { tension }
        | kt_model::SegShape::EaseOut { tension } => {
            ui.label(tr(k::fx::SHAPE_TENSION));
            if ui.add(egui::DragValue::new(tension).range(0.0..=4.0).speed(0.02)).changed() {
                changed = true;
            }
        }
        kt_model::SegShape::Zigzag { periods, amplitude, profile } => {
            ui.label(tr(k::fx::SHAPE_PERIODS));
            if ui.add(egui::DragValue::new(periods).range(0.25..=256.0).speed(0.1)).changed() {
                changed = true;
            }
            ui.label(tr(k::fx::SHAPE_AMP));
            // amplitude in UNITS OF THE PARAMETER: a zigzag around a constant
            // value is the most frequent — and a fraction of the swing would collapse it.
            if ui.add(egui::DragValue::new(amplitude).speed(0.01)).changed() {
                changed = true;
            }
            ui.label(tr(k::fx::SHAPE_PROFILE));
            let name = |p: kt_model::ZigProfile| match p {
                kt_model::ZigProfile::Triangle => tr(k::fx::ZIG_TRIANGLE),
                kt_model::ZigProfile::Square => tr(k::fx::ZIG_SQUARE),
                kt_model::ZigProfile::Sine => tr(k::fx::ZIG_SINE),
            };
            egui::ComboBox::from_id_salt(("zig_profile", curve.param.clone()))
                .selected_text(name(*profile))
                .show_ui(ui, |ui| {
                    for pr in [
                        kt_model::ZigProfile::Triangle,
                        kt_model::ZigProfile::Square,
                        kt_model::ZigProfile::Sine,
                    ] {
                        if ui.selectable_label(*profile == pr, name(pr)).clicked() {
                            *profile = pr;
                            changed = true;
                        }
                    }
                });
        }
        // text of the segment formula is edited in the segment’s menu (R.67), not here
        kt_model::SegShape::Linear | kt_model::SegShape::Hold | kt_model::SegShape::Formula { .. } => {}
    }
    if next != cur && !rate {
        for k in &mut curve.keys {
            k.shape = next;
        }
    }
    changed
}

/// Hint for a widget drawn inside a closure.
pub fn hint(r: &egui::Response, text: &str) {
    if r.hovered() {
        hint_sink::put(text);
    }
}

// ---- publication of a property outward (R.59, stage 3) -------------------------

/// Context of PUBLICATION at the field’s label.
///
/// There is one door: a property can be published outward only by right-clicking
/// the LABEL of its field. A second entry is deliberately not added — otherwise “where is
/// this published” becomes a question, and the public set of a reel must be assembled
/// where the author looks at the property (reconnaissance F-TL-012:
/// the Essential Graphics panel is assembled BY HAND,
/// not derived from the content).
///
/// Whether a window offers a context at all is decided by its host: a morpher patch has no
/// address (`ParamOwner` does not know it), so it passes
/// `None`.
pub struct PublishCtx {
    /// The track that holds the carrier. `None` — we are at the ROOT: the root
    /// is not anyone’s instance, there is nothing to publish from it, and the menu item
    /// is ABSENT ENTIRELY (rule 9, “there are no dead controls”). The reason is given by
    /// the label’s hint (rule 16).
    pub seq: Option<kt_model::MediaId>,
    pub clip: kt_model::ClipId,
    pub owner: kt_model::ParamOwner,
    /// Handles of THIS carrier that are already declared: property name → handle number.
    /// By it the row recognizes itself, draws a pin and offers removal.
    pub published: std::collections::HashMap<String, kt_model::PubId>,
    /// Whether the hint should explain WHY there is no item (rule 16).
    ///
    /// Only when the composition document has one: in a project without a single
    /// reel, the phrase “publication lives inside a composition” would hang over
    /// EVERY label and talk about a mechanism the person has not set up yet —
    /// that is noise (rule 9 and rule 10).
    pub explain_outside: bool,
}

/// What the field label asked for. `KtApp` executes it at the end of the frame — inside
/// `Window::show` the model is out of reach.
#[derive(Debug, Clone)]
pub enum PublishReq {
    /// Open the name dialog for this target. The default name is the LABEL
    /// of the field in the interface language; from then on it is the AUTHOR’s text.
    Publish {
        seq: kt_model::MediaId,
        addr: kt_model::ParamAddr,
        ty: kt_model::ParamType,
        label: String,
    },
    Unpublish {
        seq: kt_model::MediaId,
        id: kt_model::PubId,
    },
}

/// The label request sink — the same mechanism as `hint_sink`: the widget puts it,
/// `KtApp` takes it at the end of the frame.
pub(crate) mod publish_sink {
    use std::cell::RefCell;

    thread_local! {
        static REQ: RefCell<Option<super::PublishReq>> = const { RefCell::new(None) };
    }

    pub(crate) fn put(r: super::PublishReq) {
        REQ.with(|t| *t.borrow_mut() = Some(r));
    }

    pub(crate) fn take() -> Option<super::PublishReq> {
        REQ.with(|t| t.borrow_mut().take())
    }
}

/// The pin before the label of a declared property.
fn pin_prefix(pubc: Option<&PublishCtx>, param: &str) -> &'static str {
    match pubc {
        Some(c) if c.published.contains_key(param) => crate::icons::PIN,
        _ => "",
    }
}

/// Right-click on the label: “Publish…” or “Remove publication”.
///
/// There is NO item outside a reel (rule 9), and this is not silence: the hint of
/// the label itself explains where publication lives at all (rule 16).
fn publish_menu(
    r: &egui::Response,
    pubc: Option<&PublishCtx>,
    param: &str,
    label: &str,
    ty: kt_model::ParamType,
) {
    let Some(c) = pubc else { return };
    let Some(seq) = c.seq else { return };
    let existing = c.published.get(param).copied();
    let (clip, owner, param) = (c.clip, c.owner.clone(), param.to_string());
    // DEFAULT NAME — the label WITHOUT a colon: for fields the colon is part of the layout
    // (“Size:”), while for a composition handle it is its name, and “Size:” in the list
    // looks like a typo (noticed by a live scenario, R.63)
    let label = label.trim().trim_end_matches(':').trim().to_string();
    r.clone().context_menu(|ui| {
        match existing {
            Some(id) => {
                let resp = ui.button(tr(k::pubp::UNPUBLISH));
                field_probe::note(&format!("pub.unpublish.{param}"), resp.rect);
                if resp.clicked() {
                    publish_sink::put(PublishReq::Unpublish { seq, id });
                    ui.close();
                }
            }
            None => {
                let resp = ui.button(tr(k::pubp::PUBLISH));
                field_probe::note(&format!("pub.publish.{param}"), resp.rect);
                if resp.clicked() {
                    publish_sink::put(PublishReq::Publish {
                        seq,
                        addr: kt_model::ParamAddr::new(clip, owner.clone(), param.clone()),
                        ty: ty.clone(),
                        label: label.clone(),
                    });
                    ui.close();
                }
            }
        };
    });
}

/// The publication door on the FIELD ITSELF — where there is no label and adding one
/// for a single caption is not allowed (rule 13). The only such case is
/// the multi-line title text: it takes the full width of the form.
pub fn publish_on_field(
    r: &egui::Response,
    pubc: Option<&PublishCtx>,
    param: &str,
    label: &str,
    ty: kt_model::ParamType,
) {
    publish_menu(r, pubc, param, label, ty);
}

/// The label of a NON-NUMERIC property (text, color, choice, media) — the same publication
/// door as for a number. Returns a `Response`, so the carrier can
/// attach its own hint.
pub fn pub_label(
    ui: &mut egui::Ui,
    text: &str,
    pubc: Option<&PublishCtx>,
    param: &str,
    ty: kt_model::ParamType,
) -> egui::Response {
    let shown = format!("{}{text}", pin_prefix(pubc, param));
    let r = ui.add(egui::Label::new(shown).sense(egui::Sense::click()));
    field_probe::note(&format!("lbl.{param}"), r.rect);
    publish_menu(&r, pubc, param, text, ty);
    if r.hovered()
        && let Some(c) = pubc
        && c.seq.is_none()
        && c.explain_outside
    {
        hint_sink::put(tr(k::pubp::OUTSIDE_REEL));
    }
    r
}

/// The parameter label is CLICKABLE: a double click resets the parameter to the
/// spec default (R.53, item 415). “Restore as it was” must not require
/// the user to remember a number; the hint about it hangs on the label
/// itself, so it is found in the same place as the gesture.
fn param_label_widget(
    ui: &mut egui::Ui,
    text: impl Into<egui::RichText>,
    pubc: Option<&PublishCtx>,
    param: &str,
    plain: &str,
) -> bool {
    let r = ui.add(egui::Label::new(text.into()).sense(egui::Sense::click()));
    // The LABEL is also a gesture target (double click, right click), so its
    // rectangle MUST be exported outward: a live scenario has no way to find the
    // `Label` in the accessibility tree, and it must not guess offsets.
    field_probe::note(&format!("lbl.{param}"), r.rect);
    // R.59, stage 3: the same label is the publication door. One hint for
    // both roles (rule 10), and outside a reel it honestly says why there is no
    // item (rule 16).
    publish_menu(&r, pubc, param, plain, kt_model::ParamType::Num);
    match pubc {
        Some(c) if c.seq.is_none() && c.explain_outside => {
            hint(&r, &format!("{}\n{}", tr(k::fx::HINT_RESET), tr(k::pubp::OUTSIDE_REEL)))
        }
        Some(c) if c.seq.is_some() => {
            hint(&r, &format!("{}\n{}", tr(k::fx::HINT_RESET), tr(k::pubp::HINT_LABEL)))
        }
        _ => hint(&r, tr(k::fx::HINT_RESET)),
    }
    r.double_clicked()
}

/// The same row, but aware of curves.
///
/// The rule is one: **the field edits exactly what it shows.**
/// - no curve → the effect’s base, as before;
/// - a curve exists and ITS key is selected → that key;
/// - a curve exists, no key selected → the curve value under the cursor, and an edit
///   places a key there (the “+” in the automation panel does the same).
pub fn param_row_bound<P: HasParams + ?Sized>(
    ui: &mut egui::Ui,
    f: &mut P,
    spec: &ParamSpec,
    bind: Option<&mut CurveBind<'_>>,
    pubc: Option<&PublishCtx>,
) -> ParamsChanged {
    param_row_bound_ex(ui, f, spec, bind, pubc, None)
}

/// A parameter field WITH A SLIDER next to it (R.64): the slider is the SAME row, and
/// not a second widget with its own bounds (rule 24). The number and the slider
/// edit one value and write by one rule, `CurveBind`; the slider
/// width is an argument, because the space in the form differs.
pub fn param_row_bound_ex<P: HasParams + ?Sized>(
    ui: &mut egui::Ui,
    f: &mut P,
    spec: &ParamSpec,
    bind: Option<&mut CurveBind<'_>>,
    pubc: Option<&PublishCtx>,
    slider: Option<f32>,
) -> ParamsChanged {
    let none = ParamsChanged::default();
    let Some(base) = f.get_param(&spec.name) else { return none };
    // the label — in the carrier’s OWN area (R.53, item 414): “size” of a title is
    // the font size, not the grain of the Noise effect
    let label = param_label_scoped(f.params_scope(), &spec.name);
    let pin = format!("{}{label}", pin_prefix(pubc, &spec.name));
    let Some(bind) = bind else {
        let reset = param_label_widget(ui, pin, pubc, &spec.name, &label);
        let mut v = if reset { spec.default } else { base };
        let r = param_edit(ui, spec, &mut v, slider);
        field_probe::note(&spec.name, r.rect);
        if r.changed() || reset {
            f.set_param(&spec.name, v);
            return ParamsChanged::base();
        }
        return none;
    };
    let Some(ci) = bind.index_of(&spec.name) else {
        let reset = param_label_widget(ui, pin, pubc, &spec.name, &label);
        let mut v = if reset { spec.default } else { base };
        let r = param_edit(ui, spec, &mut v, slider);
        field_probe::note(&spec.name, r.rect);
        if r.changed() || reset {
            f.set_param(&spec.name, v);
            return ParamsChanged::base();
        }
        return none;
    };
    // the label in the COLOR of the curve — it is visible which line the field edits
    // (and which line on the graph to look for with the eyes)
    let key = match bind.sel {
        Some((c, k)) if c == ci && k < bind.curves[ci].keys.len() => Some(k),
        _ => None,
    };
    let reset = param_label_widget(
        ui,
        egui::RichText::new(pin).color(crate::automation::curve_color(ci)),
        pubc,
        &spec.name,
        &label,
    );
    let c = &mut bind.curves[ci];
    // DISPLAY — already within the spec bounds (R.67): `DragValue`/`Slider` with
    // bounds clamp the incoming value and mark the answer “changed” —
    // a zigzag that left the spec under the cursor placed a key on every
    // frame of playback (“recorder keys” in the owner’s project)
    let shown = if reset { spec.default } else { bound_value(c, key, bind.at) };
    let mut v = shown.clamp(spec.min.min(spec.max), spec.max.max(spec.min));
    let r = param_edit(ui, spec, &mut v, slider);
    field_probe::note(&spec.name, r.rect);
    // A key — ONLY from a human gesture: drag, keyboard input (focus /
    // confirmation), a click on the slider or a reset; “changed” without a gesture
    // is not an edit
    let gesture = r.dragged() || r.has_focus() || r.lost_focus() || r.clicked();
    if (r.changed() && gesture) || reset {
        bound_apply(c, key, bind.at, v);
        // the edit went to the CURVE — so the command MUST also be SetFxAuto
        return ParamsChanged::curve();
    }
    none
}

/// What the field of a parameter with a live curve SHOWS: the value of the selected
/// key, and if none is selected — the curve value under the cursor. There is no
/// effect base here at all: its invisibility was precisely the “separate default
/// state” (R.38 item 16).
pub fn bound_value(c: &kt_model::ParamCurve, key: Option<usize>, at: kt_time::Tick) -> f64 {
    match key.filter(|k| *k < c.keys.len()) {
        Some(k) => c.keys[k].v,
        None => kt_model::curve_value(&c.keys, at),
    }
}

/// What the field EDITS. Exactly what it shows (R.38 item 15): the selected
/// key or a new key under the cursor.
pub fn bound_apply(c: &mut kt_model::ParamCurve, key: Option<usize>, at: kt_time::Tick, v: f64) {
    match key.filter(|k| *k < c.keys.len()) {
        Some(k) => c.keys[k].v = v,
        // the segment shape is INHERITED: an edit of the field may not change the view of the
        // curve, and for warp speed a smoothed key would be a refusal
        None => kt_model::insert_key(&mut c.keys, at, v, kt_model::SegShape::Linear),
    }
}

/// The grid of ALL numeric parameters of the effect in two pairs “label/value”.
/// `skip` — parameters that have their own control (the key color is drawn with
/// an eyedropper, not with three numbers).
pub fn params_grid<P: HasParams + ?Sized>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    f: &mut P,
    skip: &[&str],
    pubc: Option<&PublishCtx>,
) -> bool {
    params_grid_bound(ui, id, f, skip, None, pubc).base
}

/// The same grid, but linked with curves (R.38 items 15-16).
pub fn params_grid_bound<P: HasParams + ?Sized>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    f: &mut P,
    skip: &[&str],
    mut bind: Option<&mut CurveBind<'_>>,
    pubc: Option<&PublishCtx>,
) -> ParamsChanged {
    // The spec is COPIED, not borrowed from the owner (R.59): it is borrowed from `self`, and a live
    // borrow would not allow writing into it in the same
    // cycle. The clone is cheap — for built-in rows the name and the label are
    // `Cow::Borrowed`, no allocations arise at all, and the Vec itself is one per
    // window drawing (not per timeline frame — rule 12 is not touched).
    let specs: Vec<ParamSpec> =
        f.param_specs().iter().filter(|s| !skip.contains(&&*s.name)).cloned().collect();
    if specs.is_empty() {
        return ParamsChanged::default();
    }
    let mut changed = ParamsChanged::default();
    egui::Grid::new(id).num_columns(4).spacing([8.0, 4.0]).show(ui, |ui| {
        for pair in specs.chunks(2) {
            for spec in pair {
                changed |= param_row_bound(ui, f, spec, bind.as_deref_mut(), pubc);
            }
            ui.end_row();
        }
    });
    changed
}

// ---- single strip of the FX chain (R.33 S2) ---------------------------------
// “the same meaning in two places, and done only halfway in both”:
// events had checkboxes and up/down arrows, morphers had crosses, and
// neither had a mouse, a master checkbox or copy-paste. Now there is one strip
// and it does everything; the event window and both morpher sections call it.

/// What the chain hands over for editing. `bypass` and `auto` are parallel to
/// `fx`.
///
/// Curves exist ON BOTH carriers (R.38 item 13 gave them to the morpher), and `auto`
/// here is NOT an `Option` on purpose: while it was optional, inserting a
/// chain with `None` silently dropped the curves on the floor — a defect that lived
/// unnoticed, because the “quiet branch” looked like care for
/// a carrier without automation. The domains differ, and that is normal: for an
/// event the key lives in the ticks of the MATERIAL (`src_in` = `source_in`),
/// for a morpher — in the time of the BLOCK (`src_in` = 0). Insertion converts keys
/// “input to input” in both directions (`ops::rebase_curves`).
pub struct ChainEdit<'a> {
    pub fx: &'a mut Vec<FxInstance>,
    pub bypass: &'a mut Vec<bool>,
    pub auto: &'a mut Vec<Vec<kt_model::ParamCurve>>,
    pub selected: &'a mut ChainSel,
    /// Preset library (R.48): rows for the picker and the “mailbox” of the
    /// save request — the record itself is written by the window’s owner.
    pub presets: ChainPresets<'a>,
    /// Master switch of the chain (for a morpher — the switch of the event).
    pub master_on: &'a mut bool,
    /// The pinned FIRST slot, which is not an effect (R.35): for a morpher
    /// it is a source substitution. Its own checkbox, a purple label, it is not
    /// rearranged with the mouse and has no cross.
    pub head: Option<HeadSlot<'a>>,
}

/// Pseudo-slot at the start of the strip.
pub struct HeadSlot<'a> {
    pub label: String,
    /// Whether it is enabled (for a morpher — `!patch.src_off`).
    ///
    /// `None` — the head has NO switch and cannot have one: for an event with a
    /// generator the head is its SOURCE (R.59, stage 2), and a source is not switched off —
    /// a disabled one would leave the event with no picture at all. A dead checkbox here
    /// would be a violation of rule 9.
    pub on: Option<&'a mut bool>,
    pub hint: &'a str,
    /// Color of the label: for a morpher purple (the layer color), for a generator
    /// source — the program accent.
    pub color: egui::Color32,
}

/// What is selected in the strip. A separate type, not “head = 0” — otherwise
/// shifting effect indices by one is guaranteed to give an off-by-one error.
/// The link of the strip with the preset library (R.48).
pub struct ChainPresets<'a> {
    /// What to show in the picker (already filtered by the event kind).
    pub rows: &'a [crate::user_store::PresetRow],
    /// The request “save this chain”: the strip puts a block in, and the owner of the window
    /// opens the name dialog and writes the file.
    pub save: &'a mut Option<kt_model::ops::FxChainBlock>,
    /// Request to manage a preset (rename/delete).
    pub action: &'a mut Option<PresetAction>,
}

/// What to do with a user preset.
#[derive(Debug, Clone, PartialEq)]
pub enum PresetAction {
    Rename(String),
    Delete(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChainSel {
    #[default]
    Head,
    Fx(usize),
    /// The leading “Geometry” node of the event (R.62): in the strip it is a slot, in the
    /// model it is a field of the event, so it has no index among `fx`.
    Lead,
}

impl ChainSel {
    /// Index of the effect slot, if exactly that one is selected.
    pub fn fx(self) -> Option<usize> {
        match self {
            ChainSel::Fx(i) => Some(i),
            ChainSel::Head | ChainSel::Lead => None,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct ChainChanged {
    pub fx: bool,
    pub bypass: bool,
    pub auto: bool,
    pub master: bool,
    /// The pinned slot was touched (the checkbox of the morph slot).
    pub head: bool,
}

impl ChainChanged {
    pub fn any(self) -> bool {
        self.fx || self.bypass || self.auto || self.master || self.head
    }
}

/// Ceiling of the slot label in characters (R.38 items 7-8, verbatim: “if in the Source
/// crumb you pick a source with a long name, the whole chain flies apart by
/// the length” and “if you toss in many crumbs, the last label runs
/// into the edge”).
///
/// The cause was not the length as such: the strip lives in
/// `horizontal_wrapped`, and it sets `TextWrapMode::Wrap`, so the label wraps
/// INSIDE ITSELF — a file name has no spaces, so it wrapped by letters. The crumb
/// did not move to a new line as a whole: it was shrunk in advance
/// to the remaining width.
pub const SLOT_NAME_CHARS: usize = 22;
/// The grab area of the slot label above and below the drawing (rule 1 of the checklist).
pub const SLOT_GRAB_PAD: f32 = 2.0;

/// A name truncated to one line with an ellipsis, not wrapped.
///
/// ONE function for all places where a user name is shown: the slot label,
/// the strip head, the morpher source field. If they diverged, half of the
/// places would start drifting apart again, and that was the claim of items 7-8.
pub fn elided_name(ui: &egui::Ui, name: &str, color: egui::Color32, off: bool) -> egui::WidgetText {
    let mut job = egui::text::LayoutJob::single_section(
        name.to_string(),
        egui::TextFormat {
            font_id: egui::TextStyle::Button.resolve(ui.style()),
            color,
            strikethrough: if off { egui::Stroke::new(1.0, color) } else { egui::Stroke::NONE },
            ..Default::default()
        },
    );
    // the limit is by WIDTH, not “however much fits in the rest of the line”: otherwise
    // the last crumb is again shrunk to the edge instead of moving whole to the next line
    job.wrap = egui::text::TextWrapping {
        max_width: ui.fonts_mut(|f| {
            let fid = egui::TextStyle::Button.resolve(ui.style());
            f.glyph_width(&fid, 'M') * SLOT_NAME_CHARS as f32
        }),
        max_rows: 1,
        overflow_character: Some('…'),
        ..Default::default()
    };
    // A FINISHED galley, not a job: the widget would lay the job out to the available
    // line width and return a wrap — the very “flying apart”
    egui::WidgetText::Galley(ui.fonts_mut(|f| f.layout_job(job)))
}

fn slot_text(ui: &egui::Ui, name: &str, color: egui::Color32, off: bool) -> egui::WidgetText {
    elided_name(ui, name, color, off)
}

/// The chain strip: master checkbox, slots (on/off checkbox, name, cross),
/// mouse rearrangement, picker and copy-paste. `audio` — the picker filter.
#[allow(clippy::too_many_arguments)]
/// Who owns the strip and where to report a right click (R.49, item 363).
/// The strip does not know `KtApp` and must not know it — it is a widget; it accepts
/// the target address from outside and gives the result back outside as well.
pub struct ChainCtx<'a> {
    pub chain: kt_model::ClipId,
    pub picked: &'a mut Option<crate::ctx_menu::PanelTarget>,
}

#[allow(clippy::too_many_arguments)]
pub fn chain_strip(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    mut edit: ChainEdit<'_>,
    audio: bool,
    query: &mut String,
    clipboard: &mut Option<kt_model::ops::FxChainBlock>,
    src_in: kt_time::Tick,
    hover: &mut Option<String>,
    ctx: &mut ChainCtx<'_>,
    // Context of picker thumbnails (R.54). `None` — we draw no pictures:
    // the audio chain (a sound thumbnail cannot be drawn) and harness tests without a
    // service. The strip knows nothing else about thumbnails — it is a widget.
    preview: Option<&mut crate::fx_thumbs::PickerCtx<'_>>,
) -> ChainChanged {
    let mut ch = ChainChanged::default();
    edit.bypass.resize(edit.fx.len(), false);
    edit.auto.resize(edit.fx.len(), Vec::new());
    let mut remove: Option<usize> = None;
    let mut swap: Option<(usize, usize)> = None;

    ui.horizontal_wrapped(|ui| {
        // master checkbox: switches off the WHOLE chain, without touching the slot states
        if crate::app::hint_into(
            hover,
            ui.checkbox(edit.master_on, tr(k::fx::CHAIN_ON)),
            tr(k::fx::CHAIN_ON_HINT),
        )
        .changed()
        {
            ch.master = true;
        }
        ui.separator();
        // the pinned head: its own checkbox, a purple label, no drag
        // and no cross — it is not an effect
        if let Some(h) = edit.head.as_mut() {
            if let Some(on) = h.on.as_deref_mut()
                && crate::app::hint_into(hover, ui.checkbox(on, ""), h.hint).changed()
            {
                ch.head = true;
            }
            let head_on = h.on.as_deref().copied().unwrap_or(true);
            let text = slot_text(ui, &h.label, h.color, !head_on);
            let resp = crate::app::hint_into(
                hover,
                ui.selectable_label(*edit.selected == ChainSel::Head, text),
                h.hint,
            );
            crate::widgets::field_probe::note("fx.head", resp.rect);
            if resp.clicked() {
                *edit.selected = ChainSel::Head;
            }
            // the head also responds to the right button (item 363): it has
            // exactly one action — its own checkbox, and staying silent about it is not allowed.
            // There is no checkbox (a generator source) — the menu does not lie with an item,
            // it names the head: the name is truncated, and the full one must be visible somewhere.
            let head_label = h.label.clone();
            let switchable = h.on.is_some();
            let mut head_toggle = false;
            resp.clone().context_menu(|ui| {
                *ctx.picked = Some(crate::ctx_menu::PanelTarget::FxHead { chain: ctx.chain });
                ui.label(egui::RichText::new(head_label.clone()).strong());
                if switchable {
                    let label = if head_on { tr(k::fx::DISABLE) } else { tr(k::fx::ENABLE) };
                    if ui.button(label).clicked() {
                        head_toggle = true;
                        ui.close();
                    }
                }
            });
            if head_toggle && let Some(on) = h.on.as_deref_mut() {
                *on = !head_on;
                ch.head = true;
            }
            // the full name — in the hint: the truncated one is visible, but which file
            // is substituted can be learned in no other way
            if h.label.chars().count() > SLOT_NAME_CHARS {
                resp.on_hover_text(h.label.clone());
            }
            if !edit.fx.is_empty() {
                ui.label(crate::icons::CARET_RIGHT);
            }
        }
        for i in 0..edit.fx.len() {
            let off = edit.bypass[i];
            // the enable checkbox of the slot (morphers had none at all)
            let mut on = !off;
            if crate::app::hint_into(
                hover,
                ui.checkbox(&mut on, ""),
                if off { tr(k::fx::ENABLE) } else { tr(k::fx::DISABLE) },
            )
            .changed()
            {
                edit.bypass[i] = !on;
                ch.bypass = true;
            }
            // The slot name: both the selection by click and the drag source.
            //
            // ONE interaction `click_and_drag`, not `dnd_drag_source`
            // around the label (R.38 item 6, verbatim: “in fx-chain widgets
            // it is troublesome to activate the input of parameters”). In egui 0.36
            // `dnd_drag_source` registers OVER the content a
            // rectangle with `Sense::drag()` and does it AFTER the content —
            // `hit_test` gives `click: None`, and `.clicked()` inside
            // NEVER fires. The repository idiom (bin_panel.rs,
            // automation.rs, meters.rs, timeline.rs) — to parse
            // `drag_started()`/`clicked()` on one interaction.
            let slot_id = egui::Id::new((&id, "slot", i));
            let name = crate::fx_editor::fx_display_name(&edit.fx[i]);
            let text = slot_text(ui, name, ui.visuals().text_color(), off);
            let drawn = ui.selectable_label(*edit.selected == ChainSel::Fx(i), text);
            // the grab area is wider than the drawing (rule 1 of the checklist): one should not
            // have to aim at a label one line high
            let resp = crate::app::hint_into(
                hover,
                ui.interact(
                    drawn.rect.expand2(egui::vec2(0.0, SLOT_GRAB_PAD)),
                    slot_id,
                    egui::Sense::click_and_drag(),
                ),
                tr(k::fx::SLOT_HINT),
            );
            // the slot hands its rectangle to live scenarios: the strip is the
            // only way to select a slot, and a scenario must not guess offsets
            // inside the window (R.27)
            if crate::widgets::field_probe::on() {
                crate::widgets::field_probe::note(&format!("fx.slot.{i}"), resp.rect);
            }
            if resp.clicked() {
                *edit.selected = ChainSel::Fx(i);
            }
            if resp.drag_started() {
                egui::DragAndDrop::set_payload(ui.ctx(), i);
            }
            if name.chars().count() > SLOT_NAME_CHARS {
                resp.clone().on_hover_text(name.to_string());
            }
            // released another slot over this one — we rearrange
            if let Some(from) = resp.dnd_release_payload::<usize>()
                && *from != i
            {
                swap = Some((*from, i));
            }
            // Right click on a slot (R.49, item 363): before this round the slot
            // answered with a hover hint and was SILENT on the right button —
            // and a user who has learned the right button on the timeline presses
            // it everywhere. No new keys: the items are the same actions that
            // already exist as a checkbox and a cross.
            resp.context_menu(|ui| {
                *ctx.picked =
                    Some(crate::ctx_menu::PanelTarget::FxSlot { chain: ctx.chain, slot: i });
                ui.label(egui::RichText::new(name).strong());
                let label = if off { tr(k::fx::ENABLE) } else { tr(k::fx::DISABLE) };
                if ui.button(label).clicked() {
                    edit.bypass[i] = !off;
                    ch.bypass = true;
                    ui.close();
                }
                if ui.button(tr(k::fx::REMOVE_SLOT)).clicked() {
                    remove = Some(i);
                    ui.close();
                }
            });
            if crate::app::hint_into(
                hover,
                ui.small_button(crate::icons::CLEAR),
                tr(k::fx::REMOVE_SLOT),
            )
            .clicked()
            {
                remove = Some(i);
            }
            if i + 1 < edit.fx.len() {
                ui.label(crate::icons::CARET_RIGHT);
            }
        }
    });

    ui.horizontal_wrapped(|ui| {
        let head_name = edit.head.as_ref().map(|h| h.label.clone());
        match crate::fx_registry::fx_picker_with_head(
            ui,
            (&id, "add"),
            audio,
            query,
            head_name.as_deref(),
            edit.presets.rows,
            hover,
            preview,
        ) {
            Some(crate::fx_registry::Picked::Preset(i)) => {
                if let Some(row) = edit.presets.rows.get(i) {
                    // insertion of a preset — the SAME function as insertion from the
                    // clipboard: keys move “input to input”
                    kt_model::ops::append_chain_block(
                        edit.fx,
                        edit.bypass,
                        edit.auto,
                        &row.block,
                        src_in,
                    );
                    *edit.selected = ChainSel::Fx(edit.fx.len().saturating_sub(1));
                    ch.fx = true;
                    ch.bypass = true;
                    ch.auto = true;
                }
            }
            Some(crate::fx_registry::Picked::PresetAction(a)) => {
                *edit.presets.action = Some(a);
            }
            Some(crate::fx_registry::Picked::Fx(tmpl)) => {
                edit.fx.push(tmpl);
                edit.bypass.push(false);
                edit.auto.push(Vec::new());
                *edit.selected = ChainSel::Fx(edit.fx.len() - 1);
                ch.fx = true;
            }
            // the head is not “added” — it is pinned; the picker only helps
            // to FIND it by search, on a par with the effects
            Some(crate::fx_registry::Picked::Head) => *edit.selected = ChainSel::Head,
            None => {}
        }
        // save the chain as a preset (R.48): the file itself is written by the owner of the
        // window — the strip only passes it the block
        if !edit.fx.is_empty()
            && crate::app::hint_into(
                hover,
                ui.button(tr(k::fx::SAVE_PRESET)),
                tr(k::fx::SAVE_PRESET_HINT),
            )
            .clicked()
        {
            *edit.presets.save = Some(kt_model::ops::FxChainBlock {
                slots: edit
                    .fx
                    .iter()
                    .enumerate()
                    .map(|(i, f)| kt_model::ops::FxSlotBlock {
                        fx: f.clone(),
                        bypass: edit.bypass[i],
                        curves: edit.auto.get(i).cloned().unwrap_or_default(),
                    })
                    .collect(),
                src_in,
                fx_off: !*edit.master_on,
            });
        }
        // copy-paste of the chain (the model — kt_model::ops, R.33)
        if !edit.fx.is_empty()
            && crate::app::hint_into(
                hover,
                ui.button(tr(k::fx::COPY_CHAIN)),
                tr(k::fx::COPY_CHAIN_HINT),
            )
            .clicked()
        {
            let block = kt_model::ops::FxChainBlock {
                slots: edit
                    .fx
                    .iter()
                    .enumerate()
                    .map(|(i, f)| kt_model::ops::FxSlotBlock {
                        fx: f.clone(),
                        bypass: edit.bypass[i],
                        curves: edit.auto.get(i).cloned().unwrap_or_default(),
                    })
                    .collect(),
                src_in,
                fx_off: !*edit.master_on,
            };
            if let Ok(json) = serde_json::to_string_pretty(&block) {
                ui.ctx().copy_text(json);
            }
            *clipboard = Some(block);
        }
        let can_paste = clipboard.is_some();
        if crate::app::hint_into(
            hover,
            ui.add_enabled(can_paste, egui::Button::new(tr(k::fx::PASTE_CHAIN))),
            tr(k::fx::PASTE_CHAIN_HINT),
        )
        .clicked()
            && let Some(b) = clipboard.clone()
        {
            // ONE truth of insertion: the same function that the presets call
            // (R.48). Keys move “input to input” — for an event these are
            // the ticks of the material, for a morpher the time of the block.
            kt_model::ops::append_chain_block(edit.fx, edit.bypass, edit.auto, &b, src_in);
            *edit.selected = ChainSel::Fx(edit.fx.len().saturating_sub(1));
            ch.fx = true;
            ch.bypass = true;
            ch.auto = true;
        }
    });

    if let Some((from, to)) = swap {
        let slot = edit.fx.remove(from);
        edit.fx.insert(to, slot);
        let b = edit.bypass.remove(from);
        edit.bypass.insert(to, b);
        {
            let a = &mut *edit.auto;
            let c = a.remove(from);
            a.insert(to, c);
            ch.auto = true;
        }
        *edit.selected = ChainSel::Fx(to);
        ch.fx = true;
        ch.bypass = true;
    }
    if let Some(i) = remove {
        edit.fx.remove(i);
        edit.bypass.remove(i);
        if i < edit.auto.len() {
            edit.auto.remove(i);
            ch.auto = true;
        }
        // the selection moves to the surviving slot, and the head stays the head
        if let Some(sel) = edit.selected.fx() {
            *edit.selected = if edit.fx.is_empty() {
                ChainSel::Head
            } else {
                ChainSel::Fx(sel.min(edit.fx.len() - 1))
            };
        }
        ch.fx = true;
        ch.bypass = true;
    }
    if edit.fx.is_empty() {
        ui.weak(tr(k::fx::CHAIN_EMPTY));
    }
    ch
}

/// The media picker of the project with search — the same gesture as the effect picker.
/// `media` — (id, path). Returns the choice: `Some(None)` — “keep
/// the source”, `Some(Some(id))` — a concrete media.
pub fn media_picker(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    media: &[(kt_model::MediaId, String)],
    query: &mut String,
) -> Option<Option<kt_model::MediaId>> {
    let mut picked = None;
    ui.menu_button(tr(k::mrph::CHOOSE), |ui| {
        let resp = ui.add(
            egui::TextEdit::singleline(query)
                .hint_text(tr(k::mrph::SEARCH_MEDIA))
                .desired_width(220.0),
        );
        if ui.memory(|m| m.focused().is_none()) {
            resp.request_focus();
        }
        ui.separator();
        if ui.button(tr(k::mrph::KEEP_SOURCE)).clicked() {
            picked = Some(None);
            ui.close();
        }
        let q = query.to_lowercase();
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for (mid, path) in media {
                if !q.is_empty() && !path.to_lowercase().contains(&q) {
                    continue;
                }
                let name = std::path::Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());
                if ui.button(name).clicked() {
                    picked = Some(Some(*mid));
                    ui.close();
                }
            }
        });
    });
    let _ = id;
    picked
}

/// The same popup with search, but over an arbitrary list of “id → label”
/// (R.35 item 2: morpher references to areas and range markers). The labels are
/// ready names, there are no file paths here.
pub fn media_picker_raw(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    items: &[(u64, String)],
    query: &mut String,
    label: &str,
) -> Option<u64> {
    let mut picked = None;
    ui.menu_button(label, |ui| {
        let resp = ui.add(
            egui::TextEdit::singleline(query)
                .hint_text(tr(k::mrph::SEARCH_MEDIA))
                .desired_width(220.0),
        );
        if ui.memory(|m| m.focused().is_none()) {
            resp.request_focus();
        }
        ui.separator();
        let q = query.to_lowercase();
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for (i, name) in items {
                if !q.is_empty() && !name.to_lowercase().contains(&q) {
                    continue;
                }
                if ui.button(name).clicked() {
                    picked = Some(*i);
                    query.clear();
                    ui.close();
                }
            }
        });
    });
    let _ = id;
    picked
}
