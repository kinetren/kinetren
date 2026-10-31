//! Media generators (R.52, F-TXT-003 + F-TXT-001): media that has no
//! file — the source computes the picture.
//!
//! **Parameter values live on the EVENT (R.59, stage 2).** Until this
//! round they lay as a string in `MediaRef.path`, that is, they were a function of
//! the MEDIA: two titles on one bin record physically could not carry different
//! text, and there was nowhere to write “change the heading in the second issue”.
//! Now only the KIND remains in the path (`gen_kind_path`: `gen:text`,
//! `gen:table:tb=grey`) — it is “what kind of media this is” and the cache strip
//! (the R.53 decision stands), while the values and their curves are `ClipEvent.gen` and
//! `ClipEvent.gen_auto`. Parsing of old full paths stays here too
//! (`parse_gen_path_full`) and serves migration.
//!
//! The ONLY vocabulary of kinds and the ONLY parse/assembly of the path are here:
//! parsing of `synth:` was duplicated in kt-app and kt-cli and already diverged in
//! case (footnote item 211), so parsing is case-insensitive, and there are no second
//! parsers. kt-media renders the frames (`GeneratorSource`); the exchange format is
//! kt-otio (`GeneratorReference.1`).

use std::borrow::Cow;

use kt_time::Tick;
use serde::{Deserialize, Serialize};

use crate::project::{
    FlagSpec, HasParams, ParamCurve, ParamScope, ParamSpec, ParamValue, apply_curves, choice_spec,
    default_get_value, default_set_value, flags_for, params_for, text_spec,
};

/// A generator kind with parameters. The serde representation (internally tagged
/// `kind`) is needed by kt-otio: the fields go into a `parameters` dictionary in
/// `GeneratorReference.1`, and `kind` itself — as the string `generator_kind`
/// (`kt.<tag>`). Unknown fields are ignored, missing ones take defaults — a file
/// from a future build is read without refusal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GeneratorKind {
    // solid (src/plugins/generators-basic) is a plugin since plugin-api-stage1 (D13).
    /// Title (F-TXT-001): the text is drawn by kt-text (vendored font,
    /// the determinism rule — kt-text/CLAUDE.md). Everything is relative:
    /// the size is a fraction of the frame height, tracking/outline/shadow/padding are fractions of
    /// an em. The styling fields (F-TXT-005) and `order` are IN THE FORMAT from day one
    /// (a Kdenlive lesson: changing the order with a constant redrew other people's projects),
    /// even though their rendering (stage 3) and window are not ready yet.
    Text {
        #[serde(default)]
        text: String,
        #[serde(default = "default_family")]
        family: String,
        #[serde(default)]
        bold: bool,
        #[serde(default = "default_text_size")]
        size: f64,
        #[serde(default = "default_one")]
        line_spacing: f64,
        #[serde(default)]
        tracking: f64,
        #[serde(default)]
        align: TextAlign,
        #[serde(default)]
        anchor: TextAnchor,
        #[serde(default = "default_half")]
        pos_x: f64,
        #[serde(default = "default_half")]
        pos_y: f64,
        #[serde(default = "default_255")]
        fill_r: u8,
        #[serde(default = "default_255")]
        fill_g: u8,
        #[serde(default = "default_255")]
        fill_b: u8,
        #[serde(default = "default_255")]
        fill_a: u8,
        #[serde(default)]
        stroke_w: f64,
        #[serde(default)]
        stroke_r: u8,
        #[serde(default)]
        stroke_g: u8,
        #[serde(default)]
        stroke_b: u8,
        #[serde(default = "default_255")]
        stroke_a: u8,
        #[serde(default = "default_shadow_off")]
        shadow_dx: f64,
        #[serde(default = "default_shadow_off")]
        shadow_dy: f64,
        #[serde(default = "default_shadow_blur")]
        shadow_blur: f64,
        #[serde(default)]
        shadow_r: u8,
        #[serde(default)]
        shadow_g: u8,
        #[serde(default)]
        shadow_b: u8,
        #[serde(default)]
        shadow_a: u8,
        #[serde(default = "default_plate_px")]
        plate_pad_x: f64,
        #[serde(default = "default_plate_py")]
        plate_pad_y: f64,
        #[serde(default)]
        plate_r: u8,
        #[serde(default)]
        plate_g: u8,
        #[serde(default)]
        plate_b: u8,
        #[serde(default)]
        plate_a: u8,
        #[serde(default = "default_order")]
        order: [TextLayer; 4],
        /// The opening share of clusters, 0..1 (R.53, item 423 — now a NUMBER, not
        /// a mode). The five former hard-wired animations became presets in the
        /// application: it puts ordinary curves on these four numbers, and the
        /// “typewriter” stopped being a special case of the engine.
        #[serde(default = "default_one")]
        reveal: f64,
        /// The width of the opening front IN CLUSTERS: 0 — a hard cut,
        /// 1 — a letter has time to fade in (the former “soft” mode).
        #[serde(default)]
        reveal_soft: f64,
        /// The offset of the whole block in fractions of the frame WIDTH (and height for `y`):
        /// a fraction, not pixels, because the title survives a change of project format —
        /// the same rule as for `pos_x`/`pos_y`.
        #[serde(default)]
        offset_x: f64,
        #[serde(default)]
        offset_y: f64,
    },
    /// A plugin-backed kind (plugin-api-stage1, D13): `kind` is its id, the rows follow its
    /// passport. Untagged and LAST: a `kind` no variant above names is tried here, in the
    /// GENERATORS' family (the transition `checker` is another plugin).
    #[serde(untagged, deserialize_with = "crate::plugin_slot::de_generator")]
    Plugin(crate::plugin_slot::PluginSlot),
    /// A kind whose plugin this build lacks: kept and written back unchanged; its source is a
    /// named placeholder (kt-media's factory).
    #[serde(untagged)]
    Missing(crate::plugin_slot::MissingSlot),
}

/// PRESET of title animation (R.53 items 423-424 → R.59 stage 2).
///
/// Earlier this was a MODE IN THE MODEL, and the engine knew about the “typewriter”.
/// Now it is the name of a SET OF CURVES (`text_anim_curves`): the model stores only
/// numbers, and the preset is the way to lay them out. The type remains for two
/// things: the menu items of the presets in the title window, and migration of old files
/// where the mode lay as a field (`am=`/`ams=` in the path, `anim` in `parameters`).
///
/// The two branches are still DIFFERENT in essence: the opening changes the VISIBILITY of
/// clusters, motion changes the OFFSET of the whole block. They cannot be mixed into one
/// “speed” field: the opening has an end, while for motion it is off-frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAnim {
    #[default]
    None,
    /// Typewriter: clusters appear one at a time, with a hard cut.
    Typewriter,
    /// Soft fade-in: the front is one cluster wide.
    Fade,
    /// Ticker: the block moves right to left, one full pass.
    Crawl,
    /// Credits crawl: from bottom to top.
    Roll,
}

impl TextAnim {
    pub const ALL: [TextAnim; 5] =
        [TextAnim::None, TextAnim::Typewriter, TextAnim::Fade, TextAnim::Crawl, TextAnim::Roll];

    pub fn token(self) -> &'static str {
        match self {
            TextAnim::None => "no",
            TextAnim::Typewriter => "tw",
            TextAnim::Fade => "fd",
            TextAnim::Crawl => "cr",
            TextAnim::Roll => "rl",
        }
    }

    pub fn from_token(s: &str) -> Option<TextAnim> {
        TextAnim::ALL.into_iter().find(|a| a.token() == s)
    }

    /// Whether the FRAME depends on time. By this very attribute a title's
    /// Quality cache is disabled (`GeneratorSource`): static titles are not
    /// affected by the appearance of animation.
    pub fn animated(self) -> bool {
        self != TextAnim::None
    }
}

/// The curves of the title animation preset — ONE place where the formulas live
/// (R.59, stage 2).
///
/// Why the function is in the model and not in the window: the same formulas read
/// old files (`am=tw:ams=2` and `anim` in `parameters`), and if the migration diverged
/// from the preset, a project saved by the previous build would move differently
/// from the same preset set by hand.
///
/// `secs` — the former pass duration; `span` — the full path of the block in fractions
/// of the frame (`1 + block size/frame size`): the window knows the layout and passes the exact one,
/// migration passes 2.0 (“the block is no wider than the frame”). The curves are in the
/// MATERIAL TICK domain, with the anchor at the event's `source_in`,
/// so the caller adds it itself.
pub fn text_anim_curves(anim: TextAnim, secs: f64, span: f64) -> Vec<ParamCurve> {
    let end = Tick((secs.max(1.0 / 1000.0) * 90_000.0).round() as i64);
    let ramp = |param: &str, a: f64, b: f64| ParamCurve {
        param: param.to_string(),
        // LINEAR, not ease: the former code computed `prog` without smoothing,
        // and a “typewriter” with smoothstep would print in jerks.
        keys: crate::AutoKey::linear(vec![
            crate::AutoKey::at(Tick::ZERO, a),
            crate::AutoKey::at(end, b),
        ]),
        source: crate::CurveSource::Keys,
    };
    match anim {
        TextAnim::None => Vec::new(),
        TextAnim::Typewriter | TextAnim::Fade => vec![ramp("reveal", 0.0, 1.0)],
        TextAnim::Crawl => vec![ramp("offset_x", span / 2.0, -span / 2.0)],
        TextAnim::Roll => vec![ramp("offset_y", span / 2.0, -span / 2.0)],
    }
}

/// Values that the preset sets BESIDES the curves. For a soft fade-in
/// this is the front width: it is constant in time, and a curve of one key
/// would be a second way to write the same thing.
pub fn text_anim_values(kind: &mut GeneratorKind, anim: TextAnim) {
    let GeneratorKind::Text { reveal, reveal_soft, offset_x, offset_y, .. } = kind else { return };
    *reveal = 1.0;
    *reveal_soft = if anim == TextAnim::Fade { 1.0 } else { 0.0 };
    *offset_x = 0.0;
    *offset_y = 0.0;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    Left,
    #[default]
    Center,
    Right,
}

/// A 3×3 anchor: which point of the text block sits at (pos_x, pos_y).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAnchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    #[default]
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// A title's styling layer; drawing order from bottom to top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextLayer {
    Plate,
    Shadow,
    Stroke,
    Fill,
}

fn default_family() -> String {
    "Noto Sans".into()
}
fn default_text_size() -> f64 {
    0.07
}
fn default_one() -> f64 {
    1.0
}
fn default_half() -> f64 {
    0.5
}
fn default_255() -> u8 {
    255
}
fn default_shadow_off() -> f64 {
    0.04
}
fn default_shadow_blur() -> f64 {
    0.06
}
fn default_plate_px() -> f64 {
    0.25
}
fn default_plate_py() -> f64 {
    0.12
}
fn default_anim_secs() -> f64 {
    1.0
}
fn default_order() -> [TextLayer; 4] {
    [TextLayer::Plate, TextLayer::Shadow, TextLayer::Stroke, TextLayer::Fill]
}

impl GeneratorKind {
    /// All kinds with the parameter DEFAULTS — the only registry (the role of
    /// `TransitionKind::all()`; a function, not a const: the title has a String):
    /// it drives the creation menu in the UI and the completeness of the specs in tests. A new kind
    /// is added AT THE END.
    pub fn all() -> Vec<GeneratorKind> {
        // R.53: new kinds — AT THE END of the registry. Engineering tables stand as FIVE rows of
        // one kind (its declared menu): the menu shows each variety, and they share one code.
        const ORDER: &[&str] = &[
            "solid", "text", "gradient", "corners", "checker", "noise", "smpte", "timecode", "transparent", "rect",
            "ellipse", "line", "leader", "table", "window",
        ];
        let mut v = Vec::new();
        for id in ORDER {
            if *id == "text" {
                v.push(GeneratorKind::default_text());
            } else {
                v.extend(GeneratorKind::menu_rows(id));
            }
        }
        v
    }

    /// The creation-menu rows of a built-in plugin kind: one row of defaults, or the values each
    /// row of its declared menu sets (the five tables).
    pub fn menu_rows(id: &str) -> Vec<GeneratorKind> {
        let base = GeneratorKind::builtin(id);
        let menu = base.kernel().map_or(&[][..], |g| g.decl().menu);
        if menu.is_empty() {
            return vec![base];
        }
        menu.iter()
            .map(|sets| {
                let mut k = base.clone();
                for (n, v) in *sets {
                    k.set_param(n, *v);
                }
                k
            })
            .collect()
    }

    /// The path token of a plugin kind's FIRST address row (the table's sort, `grey`): the creation
    /// menu's probes tell the five tables apart by it.
    pub fn address_token(&self) -> Option<&'static str> {
        use kt_plugin_api::generator::PathVal;
        let GeneratorKind::Plugin(p) = self else { return None };
        let g = self.kernel()?;
        let row = g.decl().address.first()?;
        g.decl().path.iter().find_map(|t| match t.val {
            PathVal::Choice(n, toks) if n == *row => toks.get(p.get(n)?.round().max(0.0) as usize).copied(),
            _ => None,
        })
    }

    /// A built-in plugin kind at its defaults; the set must be installed (the registry's kinds).
    pub fn builtin(id: &str) -> GeneratorKind {
        GeneratorKind::plugin(id)
            .unwrap_or_else(|| panic!("{id} is not registered: install the built-in plugin set first"))
    }

    /// A plugin kind at its passport's defaults; `None` — no such generator is registered.
    pub fn plugin(id: &str) -> Option<GeneratorKind> {
        crate::plugin_slot::PluginSlot::new(kt_plugin_api::Family::Generator, id).map(GeneratorKind::Plugin)
    }

    /// A plugin kind with the given values by row name (clamped and cast like any write).
    pub fn plugin_with(id: &str, values: &[(&str, f64)]) -> Option<GeneratorKind> {
        let mut k = GeneratorKind::plugin(id)?;
        for (n, v) in values {
            k.set_param(n, *v);
        }
        Some(k)
    }

    /// A solid colour — the built-in plugin (`generators-basic`) at these values.
    pub fn solid(r: u8, g: u8, b: u8) -> GeneratorKind {
        let mut k = GeneratorKind::builtin("solid");
        for (n, v) in [("color_r", r), ("color_g", g), ("color_b", b)] {
            k.set_param(n, v as f64);
        }
        k
    }

    /// The generator kernel of a plugin kind.
    pub fn kernel(&self) -> Option<&'static dyn kt_plugin_api::generator::GenKernel> {
        match self {
            GeneratorKind::Plugin(p) => p.plugin()?.generator(),
            _ => None,
        }
    }

    /// A title with defaults (empty text — it is typed in the window).
    pub fn default_text() -> GeneratorKind {
        GeneratorKind::Text {
            text: String::new(),
            family: default_family(),
            bold: false,
            size: default_text_size(),
            line_spacing: 1.0,
            tracking: 0.0,
            align: TextAlign::Center,
            anchor: TextAnchor::Center,
            pos_x: 0.5,
            pos_y: 0.5,
            fill_r: 255,
            fill_g: 255,
            fill_b: 255,
            fill_a: 255,
            stroke_w: 0.0,
            stroke_r: 0,
            stroke_g: 0,
            stroke_b: 0,
            stroke_a: 255,
            shadow_dx: default_shadow_off(),
            shadow_dy: default_shadow_off(),
            shadow_blur: default_shadow_blur(),
            shadow_r: 0,
            shadow_g: 0,
            shadow_b: 0,
            shadow_a: 0,
            plate_pad_x: default_plate_px(),
            plate_pad_y: default_plate_py(),
            plate_r: 0,
            plate_g: 0,
            plate_b: 0,
            plate_a: 0,
            order: default_order(),
            reveal: 1.0,
            reveal_soft: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    /// The kind tag — it is also the name in the `gen:` path and the `generator_kind`
    /// suffix (`kt.<tag>`) in the exchange file.
    pub fn tag(&self) -> &'static str {
        match self {
            GeneratorKind::Plugin(p) => p.tag(),
            GeneratorKind::Missing(_) => "missing",
            GeneratorKind::Text { .. } => "text",
        }
    }

    /// A kind with defaults by tag (the tag's case is the caller's concern:
    /// `parse_gen_path` normalizes it itself).
    pub fn default_for_tag(tag: &str) -> Option<GeneratorKind> {
        GeneratorKind::all().into_iter().find(|k| k.tag() == tag)
    }
}

params_for! { GeneratorKind, gen_specs, gen_set_param, gen_get_param, gen_set_idx, gen_get_idx, {
    // numeric parameters of the title; text/family/alignment/colors are not
    // specs (their own window controls, the “legal exception” F-TXT-001)
    // everything that is in FRACTIONS OF THE FRAME is shown as percentages (R.53, item 415):
    // the storage does not change, readability changes entirely. `line_spacing`
    // and `plate_pad_*` are fractions of ITS OWN text height, these are multipliers.
    Text {
        size: f64, 0.005, 0.5, 0.07, 0.001, Percent, Smooth;
        line_spacing: f64, 0.5, 3.0, 1.0, 0.05, Ratio, Smooth;
        tracking: f64, -0.05, 0.5, 0.0, 0.005, Percent, Smooth;
        pos_x: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        pos_y: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        stroke_w: f64, 0.0, 0.25, 0.0, 0.005, Percent, Smooth;
        shadow_dx: f64, -0.5, 0.5, 0.04, 0.005, Percent, Smooth;
        shadow_dy: f64, -0.5, 0.5, 0.04, 0.005, Percent, Smooth;
        shadow_blur: f64, 0.0, 0.5, 0.06, 0.005, Percent, Smooth;
        plate_pad_x: f64, 0.0, 2.0, 0.25, 0.01, Ratio, Smooth;
        plate_pad_y: f64, 0.0, 2.0, 0.12, 0.01, Ratio, Smooth;
        // R.59: title animation — ordinary numbers, the new rows AT THE END
        // (the index addressing of R.59 stage 1 is stable by construction).
        reveal: f64, 0.0, 1.0, 1.0, 0.01, Norm, Smooth;
        reveal_soft: f64, 0.0, 1.0, 0.0, 0.01, Norm, Smooth;
        offset_x: f64, -4.0, 4.0, 0.0, 0.001, Percent, Smooth;
        offset_y: f64, -4.0, 4.0, 0.0, 0.001, Percent, Smooth;
    },
    // R.53 (item 419): fractions of ITS OWN axis for sizes and positions, fractions of the HEIGHT for
    // isotropic lengths — the backing survives a change of project format
}}

flags_for! { GeneratorKind, gen_flag_specs, gen_set_flag, gen_get_flag, {
    // “yes/no” is not dragged by a slider and not automated by a curve, but it must not
    // be forgotten either: the 1 kHz tone and colored noise existed in the format since R.52, and
    // there was nothing to enable them with (R.53, item 414).
    Text { bold: false },
}}

impl HasParams for GeneratorKind {
    fn param_specs(&self) -> &'static [ParamSpec] {
        match self {
            GeneratorKind::Plugin(p) => p.specs(),
            GeneratorKind::Missing(_) => &[],
            _ => gen_specs(self),
        }
    }
    fn get_param(&self, name: &str) -> Option<f64> {
        match self {
            GeneratorKind::Plugin(p) => p.get(name),
            GeneratorKind::Missing(_) => None,
            _ => gen_get_param(self, name),
        }
    }
    fn set_param(&mut self, name: &str, v: f64) -> bool {
        match self {
            GeneratorKind::Plugin(p) => p.set(name, v),
            GeneratorKind::Missing(_) => false,
            _ => gen_set_param(self, name, v),
        }
    }
    fn params_scope(&self) -> ParamScope {
        ParamScope::Generator
    }
    /// A plugin kind's flags are its passport's flag rows (D13).
    fn flag_specs(&self) -> &'static [FlagSpec] {
        match self {
            GeneratorKind::Plugin(p) => crate::plugin_slot::flag_specs(p.manifest()),
            GeneratorKind::Missing(_) => &[],
            _ => gen_flag_specs(self),
        }
    }
    fn get_flag(&self, name: &str) -> Option<bool> {
        match self {
            GeneratorKind::Plugin(p) => is_flag_row(p, name).then(|| p.get(name).is_some_and(|v| v >= 0.5)),
            GeneratorKind::Missing(_) => None,
            _ => gen_get_flag(self, name),
        }
    }
    fn set_flag(&mut self, name: &str, v: bool) -> bool {
        match self {
            GeneratorKind::Plugin(p) => is_flag_row(p, name) && p.set(name, if v { 1.0 } else { 0.0 }),
            GeneratorKind::Missing(_) => false,
            _ => gen_set_flag(self, name, v),
        }
    }

    /// Non-numeric properties of the title (R.59, stage 3). The round was
    /// undertaken for their sake: a “release heading” is TEXT, and it is
    /// precisely that which must be published, not the size.
    fn extra_specs(&self) -> Vec<ParamSpec> {
        match self {
            GeneratorKind::Text { .. } => vec![
                text_spec("text", true),
                text_spec("family", false),
                choice_spec("align", TEXT_ALIGN_KEYS),
                choice_spec("anchor", TEXT_ANCHOR_KEYS),
            ],
            _ => Vec::new(),
        }
    }

    fn get_value(&self, name: &str) -> Option<ParamValue> {
        if let GeneratorKind::Text { text, family, align, anchor, .. } = self {
            match name {
                "text" => return Some(ParamValue::Text(text.clone())),
                "family" => return Some(ParamValue::Text(family.clone())),
                "align" => return Some(ParamValue::Choice(align_key(*align).into())),
                "anchor" => return Some(ParamValue::Choice(anchor_key(*anchor).into())),
                _ => {}
            }
        }
        default_get_value(self, name)
    }

    fn set_value(&mut self, name: &str, v: &ParamValue) -> bool {
        if let GeneratorKind::Text { text, family, align, anchor, .. } = self {
            match (name, v) {
                ("text", ParamValue::Text(s)) => {
                    *text = s.clone();
                    return true;
                }
                ("family", ParamValue::Text(s)) => {
                    *family = s.clone();
                    return true;
                }
                ("align", ParamValue::Choice(k)) => {
                    let Some(a) = align_of_key(k) else { return false };
                    *align = a;
                    return true;
                }
                ("anchor", ParamValue::Choice(k)) => {
                    let Some(a) = anchor_of_key(k) else { return false };
                    *anchor = a;
                    return true;
                }
                _ => {}
            }
        }
        default_set_value(self, name, v)
    }
}

/// Whether `name` is a flag row of the plugin kind's passport.
fn is_flag_row(p: &crate::plugin_slot::PluginSlot, name: &str) -> bool {
    p.specs().iter().any(|s| s.name == name && s.ty == crate::project::ParamType::Flag)
}

const TEXT_ALIGN_KEYS: &[&str] = &["left", "center", "right"];
const TEXT_ANCHOR_KEYS: &[&str] = &[
    "top_left",
    "top_center",
    "top_right",
    "center_left",
    "center",
    "center_right",
    "bottom_left",
    "bottom_center",
    "bottom_right",
];

fn align_key(a: TextAlign) -> &'static str {
    match a {
        TextAlign::Left => "left",
        TextAlign::Center => "center",
        TextAlign::Right => "right",
    }
}

fn align_of_key(k: &str) -> Option<TextAlign> {
    match k {
        "left" => Some(TextAlign::Left),
        "center" => Some(TextAlign::Center),
        "right" => Some(TextAlign::Right),
        _ => None,
    }
}

fn anchor_key(a: TextAnchor) -> &'static str {
    match a {
        TextAnchor::TopLeft => "top_left",
        TextAnchor::TopCenter => "top_center",
        TextAnchor::TopRight => "top_right",
        TextAnchor::CenterLeft => "center_left",
        TextAnchor::Center => "center",
        TextAnchor::CenterRight => "center_right",
        TextAnchor::BottomLeft => "bottom_left",
        TextAnchor::BottomCenter => "bottom_center",
        TextAnchor::BottomRight => "bottom_right",
    }
}

fn anchor_of_key(k: &str) -> Option<TextAnchor> {
    TEXT_ANCHOR_KEYS.iter().position(|x| *x == k).map(|i| {
        [
            TextAnchor::TopLeft,
            TextAnchor::TopCenter,
            TextAnchor::TopRight,
            TextAnchor::CenterLeft,
            TextAnchor::Center,
            TextAnchor::CenterRight,
            TextAnchor::BottomLeft,
            TextAnchor::BottomCenter,
            TextAnchor::BottomRight,
        ][i]
    })
}

/// A pseudo-scheme path behind which there is no file: `synth:` (test sources)
/// or `gen:` (generators). A single predicate for all the places that today wrote
/// `starts_with("synth:")` by hand: path resolution, the offline watchdog,
/// the index service, sidecar DB.
pub fn is_pseudo_path(path: &str) -> bool {
    starts_ci(path, "synth:") || starts_ci(path, "gen:")
}

/// Parsing of a `gen:` path. Case-insensitive for the scheme, tag and keys
/// (string values are not touched). Any unknown tag, key or broken
/// value — `None`: upstream this is an honest “file missing” error, not a
/// silent default (a lesson of footnote item 211).
pub fn parse_gen_path(path: &str) -> Option<GeneratorKind> {
    parse_gen_path_full(path).map(|(k, _)| k)
}

/// The title animation mode from the OLD path (R.53). A separate parse output,
/// because in the kind it no longer exists: migration turns it into curves
/// (`text_anim_curves`), not into a field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LegacyTextAnim {
    pub anim: TextAnim,
    pub secs: f64,
}

/// Parsing of a `gen:` path TOGETHER WITH the legacy animation mode.
///
/// The only parsing door: `parse_gen_path` is a wrapper that discards the
/// mode. The keys `am`/`ams` occur only in files of R.53..R.58, and
/// exactly one parser must understand them — otherwise migration and reading
/// would diverge on what counts as a broken path.
pub fn parse_gen_path_full(path: &str) -> Option<(GeneratorKind, Option<LegacyTextAnim>)> {
    let rest = strip_ci(path, "gen:")?;
    let mut toks = rest.split(':');
    let tag = toks.next()?.to_ascii_lowercase();
    let mut kind = GeneratorKind::default_for_tag(&tag)?;
    let mut anim: Option<TextAnim> = None;
    let mut secs = default_anim_secs();
    for t in toks {
        let (key, val) = t.split_once('=')?;
        let key = key.to_ascii_lowercase();
        if matches!(kind, GeneratorKind::Text { .. }) {
            match key.as_str() {
                "am" => {
                    anim = Some(TextAnim::from_token(val)?);
                    continue;
                }
                "ams" => {
                    secs = val.parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0)?;
                    continue;
                }
                _ => {}
            }
        }
        apply_token(&mut kind, &key, val)?;
    }
    let legacy = anim.filter(|a| a.animated()).map(|a| LegacyTextAnim { anim: a, secs });
    Some((kind, legacy))
}

/// A `gen:` path whose tag names no kind of this build (D13): the media of a plugin kind this build
/// lacks. Its events carry the kind as `Missing`, and the source is a named placeholder.
pub fn is_missing_gen_path(path: &str) -> bool {
    let Some(rest) = strip_ci(path, "gen:") else { return false };
    let tag = rest.split(':').next().unwrap_or("").to_ascii_lowercase();
    !tag.is_empty() && GeneratorKind::default_for_tag(&tag).is_none()
}

/// A media path carrying ONLY THE KIND (R.59, stage 2).
///
/// The values moved to the event, and what remained in `MediaRef.path` is “what kind of
/// media this is” — the kind tag. The single exception is the kind's “sort” of a variant,
/// for which the kind name is a function of the VALUE (item 419 rule: five
/// engineering tables — one kind with a `table` field, but five menu rows).
/// A sort is not a number and is not automated by a curve, so it is
/// part of the address, not a value of the event.
pub fn gen_kind_path(k: &GeneratorKind) -> String {
    match k {
        // a plugin kind's address rows are its declaration's (D13): the path token of each
        GeneratorKind::Plugin(p) => {
            let mut out = format!("gen:{}", p.tag());
            let address = k.kernel().map_or(&[][..], |g| g.decl().address);
            plugin_tokens(p, Some(address), &mut |key, val| {
                out.push(':');
                out.push_str(key);
                out.push('=');
                out.push_str(&val);
            });
            out
        }
        GeneratorKind::Missing(m) => format!("gen:{}", m.id()),
        _ => format!("gen:{}", k.tag()),
    }
}

/// Generator state for the SOURCE: the kind with values plus curves
/// (R.59, stage 2).
///
/// The curves are resolved by the SOURCE from its `pts` (the material tick), not by the plan:
/// the order has the form `(address, tick, quality)`, and a hash of the values in the address
/// would mean rebuilding the plan on every keystroke. Hence the hard
/// constraint: the compiled automation plan (`AutoPlan`) does NOT TOUCH the
/// generator's curves — otherwise the plan and the source would diverge, and the
/// cache key would stop determining the frame.
#[derive(Debug, Clone, PartialEq)]
pub struct GenState {
    pub kind: GeneratorKind,
    pub auto: Vec<ParamCurve>,
}

impl GenState {
    pub fn new(kind: GeneratorKind) -> GenState {
        GenState { kind, auto: Vec::new() }
    }

    /// The kind WITH VALUES at the moment `pts` of the material. Without curves —
    /// `Borrowed`: a non-animated generator pays no allocation
    /// (the zero-cost rule, the owner's claim item 4 of R.59).
    pub fn kind_at(&self, pts: Tick) -> Cow<'_, GeneratorKind> {
        if self.auto.is_empty() {
            return Cow::Borrowed(&self.kind);
        }
        let mut k = self.kind.clone();
        apply_curves(&mut k, &self.auto, pts);
        Cow::Owned(k)
    }

    /// Whether the frame depends on time BECAUSE OF the curves. A kind that depends on time
    /// by itself (noise, timecode) is known to kt-media itself.
    pub fn animated(&self) -> bool {
        !self.auto.is_empty()
    }
}

/// The canonical `gen:` path: lowercase, defaults are not written, the color —
/// as a single token `c=rrggbb`. `format(parse(s))` canonicalizes;
/// the model always stores the canon (it is produced by the window and the kt-otio reader).
pub fn format_gen_path(k: &GeneratorKind) -> String {
    let mut out = match k {
        GeneratorKind::Missing(m) => format!("gen:{}", m.id()),
        _ => format!("gen:{}", k.tag()),
    };
    let mut push = |key: &str, val: String| {
        out.push(':');
        out.push_str(key);
        out.push('=');
        out.push_str(&val);
    };
    match k {
        GeneratorKind::Plugin(p) => plugin_tokens(p, None, &mut |key, val| push(key, val)),
        GeneratorKind::Missing(_) => {}
        GeneratorKind::Text {
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
            plate_pad_x,
            plate_pad_y,
            plate_r,
            plate_g,
            plate_b,
            plate_a,
            order,
            reveal,
            reveal_soft,
            offset_x,
            offset_y,
        } => {
            // defaults are not written; f64 — the shortest exact Display form
            if !text.is_empty() {
                push("t", escape_gen_value(text));
            }
            if family != &default_family() {
                push("f", escape_gen_value(family));
            }
            if *bold {
                push("b", "1".into());
            }
            let f = |v: &f64| format!("{v}");
            if *size != default_text_size() {
                push("s", f(size));
            }
            if *line_spacing != 1.0 {
                push("ls", f(line_spacing));
            }
            if *tracking != 0.0 {
                push("tr", f(tracking));
            }
            if *align != TextAlign::default() {
                push("al", align_token(*align).into());
            }
            if *anchor != TextAnchor::default() {
                push("an", anchor_token(*anchor).into());
            }
            if *pos_x != 0.5 {
                push("px", f(pos_x));
            }
            if *pos_y != 0.5 {
                push("py", f(pos_y));
            }
            let rgba = |r: &u8, g: &u8, b: &u8, a: &u8| format!("{r:02x}{g:02x}{b:02x}{a:02x}");
            if (*fill_r, *fill_g, *fill_b, *fill_a) != (255, 255, 255, 255) {
                push("c", rgba(fill_r, fill_g, fill_b, fill_a));
            }
            if *stroke_w != 0.0 {
                push("sw", f(stroke_w));
            }
            if (*stroke_r, *stroke_g, *stroke_b, *stroke_a) != (0, 0, 0, 255) {
                push("sc", rgba(stroke_r, stroke_g, stroke_b, stroke_a));
            }
            if *shadow_dx != default_shadow_off() {
                push("sdx", f(shadow_dx));
            }
            if *shadow_dy != default_shadow_off() {
                push("sdy", f(shadow_dy));
            }
            if *shadow_blur != default_shadow_blur() {
                push("sb", f(shadow_blur));
            }
            if (*shadow_r, *shadow_g, *shadow_b, *shadow_a) != (0, 0, 0, 0) {
                push("shc", rgba(shadow_r, shadow_g, shadow_b, shadow_a));
            }
            if *plate_pad_x != default_plate_px() {
                push("ppx", f(plate_pad_x));
            }
            if *plate_pad_y != default_plate_py() {
                push("ppy", f(plate_pad_y));
            }
            if (*plate_r, *plate_g, *plate_b, *plate_a) != (0, 0, 0, 0) {
                push("plc", rgba(plate_r, plate_g, plate_b, plate_a));
            }
            if *order != default_order() {
                push("or", order.iter().map(|l| layer_letter(*l)).collect());
            }
            if *reveal != 1.0 {
                push("rv", f(reveal));
            }
            if *reveal_soft != 0.0 {
                push("rvs", f(reveal_soft));
            }
            if *offset_x != 0.0 {
                push("ox", f(offset_x));
            }
            if *offset_y != 0.0 {
                push("oy", f(offset_y));
            }
        }
        // --- R.53 (items 419-422) -------------------------------------------
    }
    out
}

fn align_token(a: TextAlign) -> &'static str {
    match a {
        TextAlign::Left => "l",
        TextAlign::Center => "c",
        TextAlign::Right => "r",
    }
}

fn align_from(s: &str) -> Option<TextAlign> {
    Some(match s {
        "l" => TextAlign::Left,
        "c" => TextAlign::Center,
        "r" => TextAlign::Right,
        _ => return None,
    })
}

fn anchor_token(a: TextAnchor) -> &'static str {
    match a {
        TextAnchor::TopLeft => "tl",
        TextAnchor::TopCenter => "tc",
        TextAnchor::TopRight => "tr",
        TextAnchor::CenterLeft => "cl",
        TextAnchor::Center => "cc",
        TextAnchor::CenterRight => "cr",
        TextAnchor::BottomLeft => "bl",
        TextAnchor::BottomCenter => "bc",
        TextAnchor::BottomRight => "br",
    }
}

fn anchor_from(s: &str) -> Option<TextAnchor> {
    Some(match s {
        "tl" => TextAnchor::TopLeft,
        "tc" => TextAnchor::TopCenter,
        "tr" => TextAnchor::TopRight,
        "cl" => TextAnchor::CenterLeft,
        "cc" => TextAnchor::Center,
        "cr" => TextAnchor::CenterRight,
        "bl" => TextAnchor::BottomLeft,
        "bc" => TextAnchor::BottomCenter,
        "br" => TextAnchor::BottomRight,
        _ => return None,
    })
}

fn layer_letter(l: TextLayer) -> char {
    match l {
        TextLayer::Plate => 'p',
        TextLayer::Shadow => 'h',
        TextLayer::Stroke => 's',
        TextLayer::Fill => 'f',
    }
}

fn order_from(s: &str) -> Option<[TextLayer; 4]> {
    let mut out = [TextLayer::Plate; 4];
    let chars: Vec<char> = s.chars().collect();
    if chars.len() != 4 {
        return None;
    }
    for (i, c) in chars.iter().enumerate() {
        out[i] = match c {
            'p' => TextLayer::Plate,
            'h' => TextLayer::Shadow,
            's' => TextLayer::Stroke,
            'f' => TextLayer::Fill,
            _ => return None,
        };
    }
    // each layer exactly once
    for l in [TextLayer::Plate, TextLayer::Shadow, TextLayer::Stroke, TextLayer::Fill] {
        if !out.contains(&l) {
            return None;
        }
    }
    Some(out)
}

/// One `key=value` token of the path. The key is already in lowercase.
fn apply_token(k: &mut GeneratorKind, key: &str, val: &str) -> Option<()> {
    match k {
        GeneratorKind::Plugin(p) => plugin_apply(p, key, val),
        GeneratorKind::Missing(_) => None,
        GeneratorKind::Text {
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
            plate_pad_x,
            plate_pad_y,
            plate_r,
            plate_g,
            plate_b,
            plate_a,
            order,
            reveal,
            reveal_soft,
            offset_x,
            offset_y,
        } => {
            let n = || val.parse::<f64>().ok().filter(|v| v.is_finite());
            match key {
                // LEGACY keys of the animation mode (R.53): the kind no longer
                // carries them, so they are caught by the full parse
                // (`parse_gen_path_full`), and they do not reach here.
                "am" | "ams" => return None,
                "rv" => *reveal = n()?,
                "rvs" => *reveal_soft = n()?,
                "ox" => *offset_x = n()?,
                "oy" => *offset_y = n()?,
                "t" => *text = unescape_gen_value(val)?,
                "f" => *family = unescape_gen_value(val)?,
                "b" => *bold = val == "1",
                "s" => *size = n()?,
                "ls" => *line_spacing = n()?,
                "tr" => *tracking = n()?,
                "al" => *align = align_from(val)?,
                "an" => *anchor = anchor_from(val)?,
                "px" => *pos_x = n()?,
                "py" => *pos_y = n()?,
                "c" => (*fill_r, *fill_g, *fill_b, *fill_a) = parse_rgba(val)?,
                "sw" => *stroke_w = n()?,
                "sc" => (*stroke_r, *stroke_g, *stroke_b, *stroke_a) = parse_rgba(val)?,
                "sdx" => *shadow_dx = n()?,
                "sdy" => *shadow_dy = n()?,
                "sb" => *shadow_blur = n()?,
                "shc" => (*shadow_r, *shadow_g, *shadow_b, *shadow_a) = parse_rgba(val)?,
                "ppx" => *plate_pad_x = n()?,
                "ppy" => *plate_pad_y = n()?,
                "plc" => (*plate_r, *plate_g, *plate_b, *plate_a) = parse_rgba(val)?,
                "or" => *order = order_from(val)?,
                _ => return None,
            }
            Some(())
        }
        // --- R.53 (items 421-422) -------------------------------------------
    }
}

/// A 0..1 fraction from the token. Out of range or not a number — a broken path, not a
/// silent clamp (the parse rule: an error, not a guess).
fn unit(val: &str) -> Option<f64> {
    val.parse::<f64>().ok().filter(|v| v.is_finite() && (0.0..=1.0).contains(v))
}

/// The figure's size: up to TWO axis fractions — the figure legally extends beyond the frame
/// (a backing under a title wider than the frame is a common thing).
fn span(val: &str) -> Option<f64> {
    val.parse::<f64>().ok().filter(|v| v.is_finite() && (0.0..=2.0).contains(v))
}

/// The `gen:` tokens of a plugin kind, in its table's order: a token at its rows' defaults is not
/// written (D13). `only` — the address rows (`gen_kind_path`); `None` — every token.
fn plugin_tokens(p: &crate::plugin_slot::PluginSlot, only: Option<&[&str]>, push: &mut dyn FnMut(&str, String)) {
    use kt_plugin_api::generator::PathVal;
    let m = p.manifest();
    let Some(g) = p.plugin().and_then(|pl| pl.generator()) else { return };
    let at = |n: &str| -> (f64, f64) {
        let i = m.passport.iter().position(|s| s.name == n).expect("the registry checked the row");
        (p.values[i], m.row(i).store.cast(m.passport[i].default))
    };
    let hex = |st: &str, comps: &[&str]| -> Option<String> {
        let vals: Vec<(f64, f64)> = comps.iter().map(|c| at(&format!("{st}_{c}"))).collect();
        (vals.iter().any(|(v, d)| v != d)).then(|| vals.iter().map(|(v, _)| format!("{:02x}", *v as u8)).collect())
    };
    for t in g.decl().path {
        let row = match t.val {
            PathVal::Hex3(st) | PathVal::Hex4(st) => format!("{st}_r"),
            PathVal::Num(n)
            | PathVal::Unit(n)
            | PathVal::Span(n)
            | PathVal::Int(n, ..)
            | PathVal::Flag1(n)
            | PathVal::Flag0(n)
            | PathVal::Choice(n, _) => n.to_string(),
        };
        if let Some(rows) = only
            && !rows.iter().any(|r| *r == row)
        {
            continue;
        }
        let val = match t.val {
            PathVal::Hex3(st) => hex(st, &["r", "g", "b"]),
            PathVal::Hex4(st) => hex(st, &["r", "g", "b", "a"]),
            PathVal::Num(n) | PathVal::Unit(n) | PathVal::Span(n) => {
                let (v, d) = at(n);
                (v != d).then(|| format!("{v}"))
            }
            PathVal::Int(n, ..) => {
                let (v, d) = at(n);
                (v != d).then(|| format!("{}", v as u32))
            }
            PathVal::Flag1(n) => (at(n).0 >= 0.5).then(|| "1".to_string()),
            PathVal::Flag0(n) => (at(n).0 < 0.5).then(|| "0".to_string()),
            PathVal::Choice(n, toks) => {
                let (v, d) = at(n);
                (v != d).then(|| toks[v as usize].to_string())
            }
        };
        if let Some(v) = val {
            push(t.key, v);
        }
    }
}

/// One `gen:` token into a plugin kind — the validators of the former `apply_token` (D13).
fn plugin_apply(p: &mut crate::plugin_slot::PluginSlot, key: &str, val: &str) -> Option<()> {
    use kt_plugin_api::generator::PathVal;
    let g = p.plugin()?.generator()?;
    let t = g.decl().path.iter().find(|t| t.key == key)?;
    let mut set = |n: &str, v: f64| p.set(n, v).then_some(());
    match t.val {
        PathVal::Hex3(st) => {
            let (r, gg, b) = parse_rgb(val)?;
            set(&format!("{st}_r"), r as f64)?;
            set(&format!("{st}_g"), gg as f64)?;
            set(&format!("{st}_b"), b as f64)
        }
        PathVal::Hex4(st) => {
            let (r, gg, b, a) = parse_rgba(val)?;
            set(&format!("{st}_r"), r as f64)?;
            set(&format!("{st}_g"), gg as f64)?;
            set(&format!("{st}_b"), b as f64)?;
            set(&format!("{st}_a"), a as f64)
        }
        PathVal::Num(n) => set(n, val.parse::<f64>().ok().filter(|v| v.is_finite())?),
        PathVal::Unit(n) => set(n, unit(val)?),
        PathVal::Span(n) => set(n, span(val)?),
        PathVal::Int(n, lo, hi) => set(n, val.parse::<u32>().ok().filter(|v| (lo..=hi).contains(v))? as f64),
        PathVal::Flag1(n) => set(n, if val == "1" { 1.0 } else { 0.0 }),
        PathVal::Flag0(n) => set(n, if val != "0" { 1.0 } else { 0.0 }),
        PathVal::Choice(n, toks) => set(n, toks.iter().position(|x| *x == val)? as f64),
    }
}

/// `rrggbbaa` (8 hex digits) → a four-component tuple.
fn parse_rgba(s: &str) -> Option<(u8, u8, u8, u8)> {
    if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some((c(0)?, c(2)?, c(4)?, c(6)?))
}

/// `rrggbb` (6 hex digits, any case) → a three-component tuple.
fn parse_rgb(s: &str) -> Option<(u8, u8, u8)> {
    if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some((c(0)?, c(2)?, c(4)?))
}

/// Escaping of a string value for the `gen:` path token (title text):
/// the service characters `%`, `:`, `=` and control characters — in `%XX`. The path stays
/// one line with no line breaks, and `unescape(escape(s)) == s` for
/// any string (test t469b).
pub fn escape_gen_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '%' | ':' | '=' => out.push_str(&format!("%{:02X}", ch as u32)),
            c if (c as u32) < 0x20 => out.push_str(&format!("%{:02X}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// The inverse of [`escape_gen_value`]. A broken sequence — `None`
/// (the rule is the same as for the whole parse: an error, not a guess).
pub fn unescape_gen_value(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(ch) = it.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        let hi = it.next()?.to_digit(16)?;
        let lo = it.next()?.to_digit(16)?;
        out.push(char::from_u32(hi * 16 + lo)?);
    }
    Some(out)
}

fn starts_ci(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

fn strip_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    starts_ci(s, prefix).then(|| &s[prefix.len()..])
}
