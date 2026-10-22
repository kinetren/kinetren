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
    FlagSpec, HasParams, ParamCurve, ParamKind, ParamScope, ParamSpec, ParamUnit, ParamValue,
    apply_curves, choice_spec, default_get_value, default_set_value, flags_for, fx_cast,
    params_for, text_spec,
};

/// A generator kind with parameters. The serde representation (internally tagged
/// `kind`) is needed by kt-otio: the fields go into a `parameters` dictionary in
/// `GeneratorReference.1`, and `kind` itself — as the string `generator_kind`
/// (`kt.<tag>`). Unknown fields are ignored, missing ones take defaults — a file
/// from a future build is read without refusal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GeneratorKind {
    /// A solid color — the only generator that is used
    /// daily (F-TXT-003 reconnaissance). The default is black (UI rule 4).
    Solid {
        #[serde(default)]
        color_r: u8,
        #[serde(default)]
        color_g: u8,
        #[serde(default)]
        color_b: u8,
    },
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
    /// A linear gradient between two colors (angle in degrees).
    Gradient {
        #[serde(default = "default_angle")]
        angle_deg: f64,
        #[serde(default)]
        a_r: u8,
        #[serde(default)]
        a_g: u8,
        #[serde(default)]
        a_b: u8,
        #[serde(default = "default_255")]
        b_r: u8,
        #[serde(default = "default_255")]
        b_g: u8,
        #[serde(default = "default_255")]
        b_b: u8,
    },
    /// A four-color gradient across the corners (bilinear).
    Corners {
        #[serde(default = "default_255")]
        tl_r: u8,
        #[serde(default)]
        tl_g: u8,
        #[serde(default)]
        tl_b: u8,
        #[serde(default)]
        tr_r: u8,
        #[serde(default = "default_255")]
        tr_g: u8,
        #[serde(default)]
        tr_b: u8,
        #[serde(default)]
        bl_r: u8,
        #[serde(default)]
        bl_g: u8,
        #[serde(default = "default_255")]
        bl_b: u8,
        #[serde(default = "default_255")]
        br_r: u8,
        #[serde(default = "default_255")]
        br_g: u8,
        #[serde(default = "default_255")]
        br_b: u8,
    },
    /// Checkerboard: the cell is given by a COUNT of columns (resolution-independent).
    Checker {
        #[serde(default = "default_cols")]
        cols: u32,
        #[serde(default)]
        a_r: u8,
        #[serde(default)]
        a_g: u8,
        #[serde(default)]
        a_b: u8,
        #[serde(default = "default_255")]
        b_r: u8,
        #[serde(default = "default_255")]
        b_g: u8,
        #[serde(default = "default_255")]
        b_b: u8,
    },
    /// Noise field: a counter-based seed from (x, y, pts of the MATERIAL) —
    /// a clip shift does not change the seed; a full brightness range (a “gray
    /// field” that the Noise EFFECT cannot give — F-TXT-003).
    #[serde(rename = "noise")]
    NoiseField {
        #[serde(default = "default_one")]
        amount: f64,
        #[serde(default = "default_true_flag")]
        mono: bool,
    },
    /// SMPTE bars at 75% (+ reference marks in the bottom third); a 1 kHz tone — a parameter:
    /// enabling it makes the media audible (level in dBFS).
    #[serde(rename = "smpte")]
    SmpteBars {
        #[serde(default)]
        tone: bool,
        #[serde(default = "default_tone_db")]
        tone_db: f64,
    },
    /// Timecode with RUNNING digits. Shows the time of the MATERIAL (the source
    /// does not see the “timeline clock” by construction): a clip from zero with
    /// source_in = 0 coincides with the timeline clock; trim/shift show the
    /// material. Burn-in of the output is another future object (F-TXT-003).
    Timecode {
        #[serde(default = "default_tc_fps")]
        fps: u32,
    },
    // --- R.53, items 419-422: shapes, backing, countdown, tables ----------
    /// a TRANSPARENT backing (item 420): a frame of zeros. A placeholder on the
    /// track and a matte blank. A separate kind, not “Solid with alpha”: a
    /// semi-transparent fill is already expressed by the event's level, and a second switch for
    /// the same meaning is forbidden (UI rule 9).
    Transparent,
    /// A rectangle with rounding (item 419). Sizes are in fractions of ITS OWN
    /// axis, so the backing survives a change of project format;
    /// isotropic lengths (rounding, feathering) are in fractions of the HEIGHT.
    Rect {
        #[serde(default = "default_half")]
        cx: f64,
        #[serde(default = "default_half")]
        cy: f64,
        #[serde(default = "default_shape_w")]
        w: f64,
        #[serde(default = "default_shape_h")]
        h: f64,
        #[serde(default)]
        radius: f64,
        #[serde(default = "default_feather")]
        feather: f64,
        #[serde(default = "default_255")]
        color_r: u8,
        #[serde(default = "default_255")]
        color_g: u8,
        #[serde(default = "default_255")]
        color_b: u8,
        #[serde(default = "default_255")]
        color_a: u8,
    },
    /// An ellipse (item 419). A separate kind from the rectangle: it has no
    /// rounding, and merging would give a dead control (UI rule 9).
    Ellipse {
        #[serde(default = "default_half")]
        cx: f64,
        #[serde(default = "default_half")]
        cy: f64,
        #[serde(default = "default_shape_w")]
        w: f64,
        #[serde(default = "default_shape_h")]
        h: f64,
        #[serde(default = "default_feather")]
        feather: f64,
        #[serde(default = "default_255")]
        color_r: u8,
        #[serde(default = "default_255")]
        color_g: u8,
        #[serde(default = "default_255")]
        color_b: u8,
        #[serde(default = "default_255")]
        color_a: u8,
    },
    /// A line segment (item 419): the ends, not a center with dimensions — its own
    /// pair of parameters, hence its own kind.
    Line {
        #[serde(default = "default_line_x1")]
        x1: f64,
        #[serde(default = "default_half")]
        y1: f64,
        #[serde(default = "default_line_x2")]
        x2: f64,
        #[serde(default = "default_half")]
        y2: f64,
        #[serde(default = "default_thickness")]
        thickness: f64,
        #[serde(default = "default_feather")]
        feather: f64,
        #[serde(default = "default_255")]
        color_r: u8,
        #[serde(default = "default_255")]
        color_g: u8,
        #[serde(default = "default_255")]
        color_b: u8,
        #[serde(default = "default_255")]
        color_a: u8,
    },
    /// A countdown (item 421): a cross, a circle, a running sector and a
    /// digit. Time is the pts of the MATERIAL, as for the timecode. The “2-pop” is
    /// exactly `1/fps` long, and the 1 kHz beep is by the same code as the SMPTE tone.
    Leader {
        #[serde(default = "default_leader_secs")]
        seconds: u32,
        #[serde(default = "default_tc_fps")]
        fps: u32,
        #[serde(default)]
        beep: bool,
        #[serde(default = "default_tone_db")]
        beep_db: f64,
    },
    /// An engineering table (item 422). Tables have no numbers of their own, so the kind is
    /// ONE, and the variety is a field: five enum variants with an empty set of
    /// parameters would be five copies of one code.
    #[serde(rename = "table")]
    TestPattern {
        #[serde(default)]
        table: TestTable,
    },
    /// A level window (item 422): a white window of a given SHARE OF THE FRAME AREA
    /// on black — measurement of APL and panel warm-up. Its numbers are its own, hence the kind
    /// is its own.
    #[serde(rename = "window")]
    WindowPattern {
        #[serde(default = "default_window_area")]
        area: f64,
        #[serde(default = "default_one")]
        level: f64,
    },
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

/// A variety of engineering table (R.53, item 422).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestTable {
    /// 75% color bars (eight, including black).
    #[default]
    Ebu75,
    /// The same at full amplitude.
    Ebu100,
    /// A continuous horizontal gray scale 0..255.
    Grayscale,
    /// Ten gray steps.
    Steps10,
    /// The BT.709 chroma plane: Cb along X, Cr along Y at constant Y.
    RampYcbcr,
}

impl TestTable {
    pub const ALL: [TestTable; 5] = [
        TestTable::Ebu75,
        TestTable::Ebu100,
        TestTable::Grayscale,
        TestTable::Steps10,
        TestTable::RampYcbcr,
    ];

    pub fn token(self) -> &'static str {
        match self {
            TestTable::Ebu75 => "ebu75",
            TestTable::Ebu100 => "ebu100",
            TestTable::Grayscale => "grey",
            TestTable::Steps10 => "steps",
            TestTable::RampYcbcr => "ycbcr",
        }
    }

    pub fn from_token(s: &str) -> Option<TestTable> {
        TestTable::ALL.into_iter().find(|t| t.token() == s)
    }
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
fn default_angle() -> f64 {
    90.0
}
fn default_cols() -> u32 {
    8
}
fn default_true_flag() -> bool {
    true
}
fn default_tone_db() -> f64 {
    -20.0
}
fn default_tc_fps() -> u32 {
    30
}
fn default_anim_secs() -> f64 {
    1.0
}
fn default_shape_w() -> f64 {
    0.5
}
fn default_shape_h() -> f64 {
    0.25
}
/// The default edge feathering — half a percent of the height: without it the
/// analytic edge looks “staircased” on a large frame.
fn default_feather() -> f64 {
    0.005
}
fn default_thickness() -> f64 {
    0.01
}
fn default_line_x1() -> f64 {
    0.2
}
fn default_line_x2() -> f64 {
    0.8
}
fn default_leader_secs() -> u32 {
    8
}
/// The share of the frame area for the level window: the classic 10% APL.
fn default_window_area() -> f64 {
    0.1
}
fn default_order() -> [TextLayer; 4] {
    [TextLayer::Plate, TextLayer::Shadow, TextLayer::Stroke, TextLayer::Fill]
}

impl GeneratorKind {
    /// All kinds with the parameter DEFAULTS — the only registry (the role of
    /// `TransitionKind::ALL`; a function, not a const: the title has a String):
    /// it drives the creation menu in the UI and the completeness of the specs in tests. A new kind
    /// is added AT THE END.
    pub fn all() -> Vec<GeneratorKind> {
        vec![
            GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 },
            GeneratorKind::default_text(),
            GeneratorKind::Gradient {
                angle_deg: 90.0,
                a_r: 0,
                a_g: 0,
                a_b: 0,
                b_r: 255,
                b_g: 255,
                b_b: 255,
            },
            GeneratorKind::Corners {
                tl_r: 255,
                tl_g: 0,
                tl_b: 0,
                tr_r: 0,
                tr_g: 255,
                tr_b: 0,
                bl_r: 0,
                bl_g: 0,
                bl_b: 255,
                br_r: 255,
                br_g: 255,
                br_b: 255,
            },
            GeneratorKind::Checker {
                cols: 8,
                a_r: 0,
                a_g: 0,
                a_b: 0,
                b_r: 255,
                b_g: 255,
                b_b: 255,
            },
            GeneratorKind::NoiseField { amount: 1.0, mono: true },
            GeneratorKind::SmpteBars { tone: false, tone_db: -20.0 },
            GeneratorKind::Timecode { fps: 30 },
            // R.53: new kinds — AT THE END of the registry. Engineering tables
            // stand as FIVE rows of one kind: the menu shows each variety,
            // and they share one code.
            GeneratorKind::Transparent,
            GeneratorKind::Rect {
                cx: 0.5,
                cy: 0.5,
                w: default_shape_w(),
                h: default_shape_h(),
                radius: 0.0,
                feather: default_feather(),
                color_r: 255,
                color_g: 255,
                color_b: 255,
                color_a: 255,
            },
            GeneratorKind::Ellipse {
                cx: 0.5,
                cy: 0.5,
                w: default_shape_w(),
                h: default_shape_h(),
                feather: default_feather(),
                color_r: 255,
                color_g: 255,
                color_b: 255,
                color_a: 255,
            },
            GeneratorKind::Line {
                x1: default_line_x1(),
                y1: 0.5,
                x2: default_line_x2(),
                y2: 0.5,
                thickness: default_thickness(),
                feather: default_feather(),
                color_r: 255,
                color_g: 255,
                color_b: 255,
                color_a: 255,
            },
            GeneratorKind::Leader {
                seconds: default_leader_secs(),
                fps: default_tc_fps(),
                beep: false,
                beep_db: default_tone_db(),
            },
            GeneratorKind::TestPattern { table: TestTable::Ebu75 },
            GeneratorKind::TestPattern { table: TestTable::Ebu100 },
            GeneratorKind::TestPattern { table: TestTable::Grayscale },
            GeneratorKind::TestPattern { table: TestTable::Steps10 },
            GeneratorKind::TestPattern { table: TestTable::RampYcbcr },
            GeneratorKind::WindowPattern { area: default_window_area(), level: 1.0 },
        ]
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
            GeneratorKind::Solid { .. } => "solid",
            GeneratorKind::Text { .. } => "text",
            GeneratorKind::Gradient { .. } => "gradient",
            GeneratorKind::Corners { .. } => "corners",
            GeneratorKind::Checker { .. } => "checker",
            GeneratorKind::NoiseField { .. } => "noise",
            GeneratorKind::SmpteBars { .. } => "smpte",
            GeneratorKind::Timecode { .. } => "timecode",
            GeneratorKind::Transparent => "transparent",
            GeneratorKind::Rect { .. } => "rect",
            GeneratorKind::Ellipse { .. } => "ellipse",
            GeneratorKind::Line { .. } => "line",
            GeneratorKind::Leader { .. } => "leader",
            GeneratorKind::TestPattern { .. } => "table",
            GeneratorKind::WindowPattern { .. } => "window",
        }
    }

    /// A kind with defaults by tag (the tag's case is the caller's concern:
    /// `parse_gen_path` normalizes it itself).
    pub fn default_for_tag(tag: &str) -> Option<GeneratorKind> {
        GeneratorKind::all().into_iter().find(|k| k.tag() == tag)
    }
}

params_for! { GeneratorKind, gen_specs, gen_set_param, gen_get_param, gen_set_idx, gen_get_idx, {
    Solid {
        color_r: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        color_g: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        color_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
    },
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
    Gradient {
        angle_deg: f64, 0.0, 360.0, 90.0, 1.0, Deg, Smooth;
        a_r: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        a_g: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        a_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        b_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        b_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        b_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
    },
    Corners {
        tl_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        tl_g: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        tl_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        tr_r: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        tr_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        tr_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        bl_r: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        bl_g: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        bl_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        br_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        br_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        br_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
    },
    Checker {
        cols: u32, 1.0, 256.0, 8.0, 1.0, Count, Structural;
        a_r: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        a_g: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        a_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        b_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        b_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        b_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
    },
    NoiseField {
        amount: f64, 0.0, 1.0, 1.0, 0.01, Norm, Smooth;
    },
    SmpteBars {
        tone_db: f64, -60.0, 0.0, -20.0, 0.5, Db, Smooth;
    },
    Timecode {
        fps: u32, 1.0, 120.0, 30.0, 1.0, Count, Structural;
    },
    // R.53 (item 419): fractions of ITS OWN axis for sizes and positions, fractions of the HEIGHT for
    // isotropic lengths — the backing survives a change of project format
    Rect {
        cx: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        cy: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        w: f64, 0.0, 2.0, 0.5, 0.001, Percent, Smooth;
        h: f64, 0.0, 2.0, 0.25, 0.001, Percent, Smooth;
        radius: f64, 0.0, 1.0, 0.0, 0.001, Percent, Smooth;
        feather: f64, 0.0, 0.5, 0.005, 0.001, Percent, Smooth;
        color_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_a: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
    },
    Ellipse {
        cx: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        cy: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        w: f64, 0.0, 2.0, 0.5, 0.001, Percent, Smooth;
        h: f64, 0.0, 2.0, 0.25, 0.001, Percent, Smooth;
        feather: f64, 0.0, 0.5, 0.005, 0.001, Percent, Smooth;
        color_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_a: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
    },
    Line {
        x1: f64, 0.0, 1.0, 0.2, 0.001, Percent, Smooth;
        y1: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        x2: f64, 0.0, 1.0, 0.8, 0.001, Percent, Smooth;
        y2: f64, 0.0, 1.0, 0.5, 0.001, Percent, Smooth;
        thickness: f64, 0.0, 0.5, 0.01, 0.001, Percent, Smooth;
        feather: f64, 0.0, 0.5, 0.005, 0.001, Percent, Smooth;
        color_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_a: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
    },
    Leader {
        seconds: u32, 1.0, 60.0, 8.0, 1.0, Count, Structural;
        fps: u32, 1.0, 120.0, 30.0, 1.0, Count, Structural;
        beep_db: f64, -60.0, 0.0, -20.0, 0.5, Db, Smooth;
    },
    WindowPattern {
        area: f64, 0.0, 1.0, 0.1, 0.005, Percent, Smooth;
        level: f64, 0.0, 1.0, 1.0, 0.005, Percent, Smooth;
    },
}}

flags_for! { GeneratorKind, gen_flag_specs, gen_set_flag, gen_get_flag, {
    // “yes/no” is not dragged by a slider and not automated by a curve, but it must not
    // be forgotten either: the 1 kHz tone and colored noise existed in the format since R.52, and
    // there was nothing to enable them with (R.53, item 414).
    NoiseField { mono: true },
    SmpteBars { tone: false },
    Text { bold: false },
    Leader { beep: false },
}}

impl HasParams for GeneratorKind {
    fn param_specs(&self) -> &'static [ParamSpec] {
        gen_specs(self)
    }
    fn get_param(&self, name: &str) -> Option<f64> {
        gen_get_param(self, name)
    }
    fn set_param(&mut self, name: &str, v: f64) -> bool {
        gen_set_param(self, name, v)
    }
    fn params_scope(&self) -> ParamScope {
        ParamScope::Generator
    }
    fn flag_specs(&self) -> &'static [FlagSpec] {
        gen_flag_specs(self)
    }
    fn get_flag(&self, name: &str) -> Option<bool> {
        gen_get_flag(self, name)
    }
    fn set_flag(&mut self, name: &str, v: bool) -> bool {
        gen_set_flag(self, name, v)
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
        GeneratorKind::TestPattern { table } if *table != TestTable::default() => {
            format!("gen:table:tb={}", table.token())
        }
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
    let mut out = format!("gen:{}", k.tag());
    let mut push = |key: &str, val: String| {
        out.push(':');
        out.push_str(key);
        out.push('=');
        out.push_str(&val);
    };
    match k {
        GeneratorKind::Solid { color_r, color_g, color_b } => {
            if (*color_r, *color_g, *color_b) != (0, 0, 0) {
                push("c", format!("{color_r:02x}{color_g:02x}{color_b:02x}"));
            }
        }
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
        GeneratorKind::Gradient { angle_deg, a_r, a_g, a_b, b_r, b_g, b_b } => {
            if *angle_deg != default_angle() {
                push("ang", format!("{angle_deg}"));
            }
            if (*a_r, *a_g, *a_b) != (0, 0, 0) {
                push("a", format!("{a_r:02x}{a_g:02x}{a_b:02x}"));
            }
            if (*b_r, *b_g, *b_b) != (255, 255, 255) {
                push("b", format!("{b_r:02x}{b_g:02x}{b_b:02x}"));
            }
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
            let hex = |r: &u8, g: &u8, b: &u8| format!("{r:02x}{g:02x}{b:02x}");
            if (*tl_r, *tl_g, *tl_b) != (255, 0, 0) {
                push("tl", hex(tl_r, tl_g, tl_b));
            }
            if (*tr_r, *tr_g, *tr_b) != (0, 255, 0) {
                push("tr", hex(tr_r, tr_g, tr_b));
            }
            if (*bl_r, *bl_g, *bl_b) != (0, 0, 255) {
                push("bl", hex(bl_r, bl_g, bl_b));
            }
            if (*br_r, *br_g, *br_b) != (255, 255, 255) {
                push("br", hex(br_r, br_g, br_b));
            }
        }
        GeneratorKind::Checker { cols, a_r, a_g, a_b, b_r, b_g, b_b } => {
            if *cols != default_cols() {
                push("n", format!("{cols}"));
            }
            if (*a_r, *a_g, *a_b) != (0, 0, 0) {
                push("a", format!("{a_r:02x}{a_g:02x}{a_b:02x}"));
            }
            if (*b_r, *b_g, *b_b) != (255, 255, 255) {
                push("b", format!("{b_r:02x}{b_g:02x}{b_b:02x}"));
            }
        }
        GeneratorKind::NoiseField { amount, mono } => {
            if *amount != 1.0 {
                push("amt", format!("{amount}"));
            }
            if !*mono {
                push("mono", "0".into());
            }
        }
        GeneratorKind::SmpteBars { tone, tone_db } => {
            if *tone {
                push("tone", "1".into());
            }
            if *tone_db != default_tone_db() {
                push("db", format!("{tone_db}"));
            }
        }
        GeneratorKind::Timecode { fps } => {
            if *fps != default_tc_fps() {
                push("fps", format!("{fps}"));
            }
        }
        // --- R.53 (items 419-422) -------------------------------------------
        GeneratorKind::Transparent => {}
        GeneratorKind::Rect {
            cx,
            cy,
            w,
            h,
            radius,
            feather,
            color_r,
            color_g,
            color_b,
            color_a,
        } => {
            let f = |v: &f64| format!("{v}");
            if *cx != 0.5 {
                push("cx", f(cx));
            }
            if *cy != 0.5 {
                push("cy", f(cy));
            }
            if *w != default_shape_w() {
                push("w", f(w));
            }
            if *h != default_shape_h() {
                push("h", f(h));
            }
            if *radius != 0.0 {
                push("rad", f(radius));
            }
            if *feather != default_feather() {
                push("fe", f(feather));
            }
            if (*color_r, *color_g, *color_b, *color_a) != (255, 255, 255, 255) {
                push("c", format!("{color_r:02x}{color_g:02x}{color_b:02x}{color_a:02x}"));
            }
        }
        GeneratorKind::Ellipse { cx, cy, w, h, feather, color_r, color_g, color_b, color_a } => {
            let f = |v: &f64| format!("{v}");
            if *cx != 0.5 {
                push("cx", f(cx));
            }
            if *cy != 0.5 {
                push("cy", f(cy));
            }
            if *w != default_shape_w() {
                push("w", f(w));
            }
            if *h != default_shape_h() {
                push("h", f(h));
            }
            if *feather != default_feather() {
                push("fe", f(feather));
            }
            if (*color_r, *color_g, *color_b, *color_a) != (255, 255, 255, 255) {
                push("c", format!("{color_r:02x}{color_g:02x}{color_b:02x}{color_a:02x}"));
            }
        }
        GeneratorKind::Line {
            x1,
            y1,
            x2,
            y2,
            thickness,
            feather,
            color_r,
            color_g,
            color_b,
            color_a,
        } => {
            let f = |v: &f64| format!("{v}");
            if *x1 != default_line_x1() {
                push("x1", f(x1));
            }
            if *y1 != 0.5 {
                push("y1", f(y1));
            }
            if *x2 != default_line_x2() {
                push("x2", f(x2));
            }
            if *y2 != 0.5 {
                push("y2", f(y2));
            }
            if *thickness != default_thickness() {
                push("th", f(thickness));
            }
            if *feather != default_feather() {
                push("fe", f(feather));
            }
            if (*color_r, *color_g, *color_b, *color_a) != (255, 255, 255, 255) {
                push("c", format!("{color_r:02x}{color_g:02x}{color_b:02x}{color_a:02x}"));
            }
        }
        GeneratorKind::Leader { seconds, fps, beep, beep_db } => {
            if *seconds != default_leader_secs() {
                push("n", format!("{seconds}"));
            }
            if *fps != default_tc_fps() {
                push("fps", format!("{fps}"));
            }
            if *beep {
                push("beep", "1".into());
            }
            if *beep_db != default_tone_db() {
                push("db", format!("{beep_db}"));
            }
        }
        GeneratorKind::TestPattern { table } => {
            if *table != TestTable::default() {
                push("tb", table.token().into());
            }
        }
        GeneratorKind::WindowPattern { area, level } => {
            if *area != default_window_area() {
                push("ar", format!("{area}"));
            }
            if *level != 1.0 {
                push("lv", format!("{level}"));
            }
        }
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
        GeneratorKind::Solid { color_r, color_g, color_b } => match key {
            "c" => {
                (*color_r, *color_g, *color_b) = parse_rgb(val)?;
                Some(())
            }
            _ => None,
        },
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
        GeneratorKind::Gradient { angle_deg, a_r, a_g, a_b, b_r, b_g, b_b } => {
            match key {
                "ang" => *angle_deg = val.parse::<f64>().ok().filter(|v| v.is_finite())?,
                "a" => (*a_r, *a_g, *a_b) = parse_rgb(val)?,
                "b" => (*b_r, *b_g, *b_b) = parse_rgb(val)?,
                _ => return None,
            }
            Some(())
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
            match key {
                "tl" => (*tl_r, *tl_g, *tl_b) = parse_rgb(val)?,
                "tr" => (*tr_r, *tr_g, *tr_b) = parse_rgb(val)?,
                "bl" => (*bl_r, *bl_g, *bl_b) = parse_rgb(val)?,
                "br" => (*br_r, *br_g, *br_b) = parse_rgb(val)?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::Checker { cols, a_r, a_g, a_b, b_r, b_g, b_b } => {
            match key {
                "n" => *cols = val.parse::<u32>().ok().filter(|v| (1..=256).contains(v))?,
                "a" => (*a_r, *a_g, *a_b) = parse_rgb(val)?,
                "b" => (*b_r, *b_g, *b_b) = parse_rgb(val)?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::NoiseField { amount, mono } => {
            match key {
                "amt" => *amount = val.parse::<f64>().ok().filter(|v| v.is_finite())?,
                "mono" => *mono = val != "0",
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::SmpteBars { tone, tone_db } => {
            match key {
                "tone" => *tone = val == "1",
                "db" => *tone_db = val.parse::<f64>().ok().filter(|v| v.is_finite())?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::Timecode { fps } => {
            match key {
                "fps" => *fps = val.parse::<u32>().ok().filter(|v| (1..=120).contains(v))?,
                _ => return None,
            }
            Some(())
        }
        // --- R.53 (items 419-422) -------------------------------------------
        // the backing has no parameters: any token is a broken path
        GeneratorKind::Transparent => None,
        GeneratorKind::Rect {
            cx,
            cy,
            w,
            h,
            radius,
            feather,
            color_r,
            color_g,
            color_b,
            color_a,
        } => {
            match key {
                "cx" => *cx = unit(val)?,
                "cy" => *cy = unit(val)?,
                "w" => *w = span(val)?,
                "h" => *h = span(val)?,
                "rad" => *radius = unit(val)?,
                "fe" => *feather = unit(val)?,
                "c" => (*color_r, *color_g, *color_b, *color_a) = parse_rgba(val)?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::Ellipse { cx, cy, w, h, feather, color_r, color_g, color_b, color_a } => {
            match key {
                "cx" => *cx = unit(val)?,
                "cy" => *cy = unit(val)?,
                "w" => *w = span(val)?,
                "h" => *h = span(val)?,
                "fe" => *feather = unit(val)?,
                "c" => (*color_r, *color_g, *color_b, *color_a) = parse_rgba(val)?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::Line {
            x1,
            y1,
            x2,
            y2,
            thickness,
            feather,
            color_r,
            color_g,
            color_b,
            color_a,
        } => {
            match key {
                "x1" => *x1 = unit(val)?,
                "y1" => *y1 = unit(val)?,
                "x2" => *x2 = unit(val)?,
                "y2" => *y2 = unit(val)?,
                "th" => *thickness = unit(val)?,
                "fe" => *feather = unit(val)?,
                "c" => (*color_r, *color_g, *color_b, *color_a) = parse_rgba(val)?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::Leader { seconds, fps, beep, beep_db } => {
            match key {
                "n" => *seconds = val.parse::<u32>().ok().filter(|v| (1..=60).contains(v))?,
                "fps" => *fps = val.parse::<u32>().ok().filter(|v| (1..=120).contains(v))?,
                "beep" => *beep = val == "1",
                "db" => *beep_db = val.parse::<f64>().ok().filter(|v| v.is_finite())?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::TestPattern { table } => {
            match key {
                "tb" => *table = TestTable::from_token(val)?,
                _ => return None,
            }
            Some(())
        }
        GeneratorKind::WindowPattern { area, level } => {
            match key {
                "ar" => *area = unit(val)?,
                "lv" => *level = unit(val)?,
                _ => return None,
            }
            Some(())
        }
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
