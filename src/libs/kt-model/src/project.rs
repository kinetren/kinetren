//! Types of the project format. The source of truth for the schema of `.ktproj` v1.

use kt_time::{Tick, TickRange};
use serde::{Deserialize, Serialize};

use crate::published::{Override, ParamAddr, ParamOwner, PubId, PublishedParam, ResolvedReel};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClipId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MediaId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GroupId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MorphId(pub u64);

/// Identifier of the camera-event (R.29). serde(transparent) — in the file
/// simply a number, as MediaId.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CameraId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipKind {
    Video,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TransitionId(pub u64);

/// The minimal total width of the transition window (1 ms, as crate::MIN_CLIP_DURATION).
pub const MIN_TRANSITION_DURATION: Tick = Tick(90);

/// The direction of the transition gesture. One field instead of four kinds: the core for
/// “left/right/up/down” is one, while the presets in the UI are different.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dir {
    #[default]
    Left,
    Right,
    Up,
    Down,
}

impl Dir {
    /// The unit direction vector in the coordinates of the frame (X right, Y down).
    pub fn vec(self) -> (f64, f64) {
        match self {
            Dir::Left => (-1.0, 0.0),
            Dir::Right => (1.0, 0.0),
            Dir::Up => (0.0, -1.0),
            Dir::Down => (0.0, 1.0),
        }
    }
}

/// The kind of transition (R.33 item 1: “it would be nice to make the transitions different, toss in
/// 10-20 modern ones, their control should be unified like the
/// fx-chain”). Built like `FxInstance`: a tag + parameter fields, the parameter specs
/// are shared with the effects — therefore a transition gets both a unified UI
/// of fields and, when the hands get to it, automation by the same mechanism.
///
/// `Crossfade` is the default and exactly the former render path: files without
/// parameters are byte-stable, and GOLDEN does not move.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransitionKind {
    /// Linear dissolve (video) + crossfade (audio).
    #[default]
    Crossfade,
    /// The edge moves across the frame: a linear wipe, `angle` gives a diagonal.
    Wipe {
        #[serde(default)]
        dir: Dir,
        #[serde(default)]
        angle: f64,
        #[serde(default = "default_soft")]
        softness: f64,
    },
    /// Opening from the center: circle/diamond/rectangle. The center (R.51) —
    /// fractions of the frame, the default is the middle; at 0.5/0.5 the picture is byte-identical.
    Iris {
        #[serde(default)]
        shape: IrisShape,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cx: f64,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cy: f64,
        #[serde(default = "default_soft")]
        softness: f64,
    },
    /// A clock hand around the center.
    Clock {
        #[serde(default)]
        reverse: bool,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cx: f64,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cy: f64,
        #[serde(default = "default_soft")]
        softness: f64,
    },
    /// Shutters: two leaves diverge from the center.
    Barn {
        #[serde(default)]
        vertical: bool,
        #[serde(default = "default_soft")]
        softness: f64,
    },
    /// A wipe by the BRIGHTNESS of the outgoing frame — a “reveal” by the picture.
    Luma {
        #[serde(default)]
        invert: bool,
        #[serde(default = "default_soft")]
        softness: f64,
    },
    /// The incoming one pushes the outgoing one off the edge.
    Push {
        #[serde(default)]
        dir: Dir,
    },
    /// The incoming one zooms over the top, the outgoing one stays.
    Slide {
        #[serde(default)]
        dir: Dir,
    },
    /// A surge with approach: the outgoing one flies apart, the incoming one rushes in.
    Zoom {
        #[serde(default = "default_zoom_amount")]
        amount: f64,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cx: f64,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cy: f64,
    },
    /// A rotation with a surge.
    Spin {
        #[serde(default = "default_one")]
        turns: f64,
        #[serde(default = "default_zoom_amount")]
        amount: f64,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cx: f64,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cy: f64,
    },
    /// A sharp camera pass with a smear.
    Whip {
        #[serde(default)]
        dir: Dir,
        #[serde(default = "default_whip_blur")]
        blur: f64,
    },
    /// Dissolve through the blur of both frames.
    BlurDissolve {
        #[serde(default = "default_blur_amount")]
        amount: f64,
    },
    /// Dissolve through enlarging the pixels.
    Pixelate {
        #[serde(default = "default_pixelate")]
        amount: f64,
    },
    /// A digital glitch: row shifts and channel divergence.
    Glitch {
        #[serde(default = "default_glitch")]
        amount: f64,
    },
    /// Flash: the frame goes into the color and comes back.
    Flash {
        #[serde(default = "default_255")]
        color_r: u8,
        #[serde(default = "default_255")]
        color_g: u8,
        #[serde(default = "default_255")]
        color_b: u8,
        #[serde(default = "default_one")]
        strength: f64,
    },
    /// Through the color: the outgoing one fades into the color, the incoming one emerges from it.
    Dip {
        #[serde(default)]
        color_r: u8,
        #[serde(default)]
        color_g: u8,
        #[serde(default)]
        color_b: u8,
    },
    // ---- patterns (R.51, item 400) -------------------------------------
    /// Blinds: the strips open in a fan, the angle gives a diagonal.
    Blinds {
        #[serde(default = "default_count8")]
        count: u32,
        #[serde(default)]
        angle: f64,
        #[serde(default = "default_soft_thin")]
        softness: f64,
    },
    /// Checkers: the cells open in two opposing waves.
    Checker {
        #[serde(default = "default_count8")]
        cols: u32,
        #[serde(default = "default_count6")]
        rows: u32,
        #[serde(default = "default_soft_thin")]
        softness: f64,
    },
    /// Random blocks: the cells flash up in a pseudo-random order.
    Blocks {
        #[serde(default = "default_count16")]
        cols: u32,
        #[serde(default = "default_count9")]
        rows: u32,
        #[serde(default = "default_blocks_soft")]
        softness: f64,
    },
    /// Spiral: an Archimedean front spirals around the center.
    Spiral {
        #[serde(default = "default_spiral_turns")]
        turns: f64,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cx: f64,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cy: f64,
        #[serde(default = "default_soft_thin")]
        softness: f64,
    },
    // ---- shift (symbiosis: BOTH frames move) ----------------------------
    /// Split: the halves of the outgoing one diverge, the incoming one opens.
    Split {
        #[serde(default)]
        vertical: bool,
    },
    /// Squeeze: the outgoing one is compressed toward the far edge, the incoming one unfolds.
    Squeeze {
        #[serde(default)]
        dir: Dir,
    },
    /// Stretch: the incoming one stretches out from the central line.
    Stretch {
        #[serde(default)]
        vertical: bool,
    },
    /// Exchange: the frames swap places along an arc, the order of the sides
    /// switches in the middle (2.5D without depth).
    Swap {
        #[serde(default)]
        vertical: bool,
        #[serde(default = "default_swap_depth")]
        depth: f64,
    },
    // ---- stylized --------------------------------------------------
    /// Shake: jerks of the frame, a change of side in the middle.
    Shake {
        #[serde(default = "default_glitch")]
        amount: f64,
        #[serde(default = "default_shake_steps")]
        steps: u32,
    },
    /// Film burn: a front by large noise with a hot edge.
    FilmBurn {
        #[serde(default = "default_burn_scale")]
        scale: f64,
        #[serde(default = "default_burn_soft")]
        softness: f64,
        #[serde(default = "default_255")]
        tint_r: u8,
        #[serde(default = "default_burn_tint_g")]
        tint_g: u8,
        #[serde(default)]
        tint_b: u8,
    },
    /// Light flares: colored spots fly across the frame over the dissolve.
    LightLeaks {
        #[serde(default = "default_leaks_strength")]
        strength: f64,
        #[serde(default = "default_255")]
        tint_r: u8,
        #[serde(default = "default_leaks_tint_g")]
        tint_g: u8,
        #[serde(default = "default_leaks_tint_b")]
        tint_b: u8,
    },
    /// Radial smear: rays from the center, a change of side in the middle.
    ZoomBlur {
        #[serde(default = "default_whip_blur")]
        amount: f64,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cx: f64,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cy: f64,
    },
    // ---- volumetric (R.51, item 401): panels and cylinders, occlusion
    // analytic — there is no depth buffer in the composite and none is needed -------
    /// Flip: the panel turns around its middle, on the back
    /// the reverse side is visible with the incoming frame.
    Flip {
        #[serde(default)]
        vertical: bool,
        #[serde(default = "default_persp")]
        persp: f64,
        #[serde(default = "default_shade")]
        shade: f64,
    },
    /// Cube: the outgoing and the incoming — two faces, the cube turns.
    Cube {
        #[serde(default)]
        dir: Dir,
        #[serde(default = "default_persp")]
        persp: f64,
        #[serde(default = "default_shade")]
        shade: f64,
    },
    /// Doors: two leaves of the outgoing one swing open inward, behind them the incoming one.
    Door {
        #[serde(default)]
        vertical: bool,
        #[serde(default = "default_persp")]
        persp: f64,
        #[serde(default = "default_shade")]
        shade: f64,
    },
    /// Page turn: the sheet wraps back around a cylinder, its back-side tail is visible;
    /// under the sheet the incoming one opens.
    PagePeel {
        #[serde(default)]
        dir: Dir,
        #[serde(default = "default_curl")]
        curl: f64,
        #[serde(default = "default_shade")]
        shadow: f64,
    },
    /// Scroll: the sheet winds onto a roll, the top coil is visible.
    PageRoll {
        #[serde(default)]
        dir: Dir,
        #[serde(default = "default_curl")]
        curl: f64,
        #[serde(default = "default_shade")]
        shadow: f64,
    },
}

/// The opening shape of Iris.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IrisShape {
    #[default]
    Circle,
    Diamond,
    Box,
}

fn default_soft() -> f64 {
    0.15
}

fn default_center() -> f64 {
    0.5
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_center(v: &f64) -> bool {
    *v == 0.5
}

fn default_soft_thin() -> f64 {
    0.1
}

fn default_blocks_soft() -> f64 {
    0.25
}

fn default_count6() -> u32 {
    6
}

fn default_count8() -> u32 {
    8
}

fn default_count9() -> u32 {
    9
}

fn default_count16() -> u32 {
    16
}

fn default_spiral_turns() -> f64 {
    2.0
}

fn default_swap_depth() -> f64 {
    0.3
}

fn default_shake_steps() -> u32 {
    12
}

fn default_burn_scale() -> f64 {
    6.0
}

fn default_burn_soft() -> f64 {
    0.2
}

fn default_burn_tint_g() -> u8 {
    120
}

fn default_leaks_strength() -> f64 {
    0.6
}

fn default_leaks_tint_g() -> u8 {
    90
}

fn default_leaks_tint_b() -> u8 {
    40
}

fn default_persp() -> f64 {
    0.5
}

fn default_shade() -> f64 {
    0.5
}

fn default_curl() -> f64 {
    0.18
}

fn default_zoom_amount() -> f64 {
    0.4
}

fn default_whip_blur() -> f64 {
    0.6
}

fn default_blur_amount() -> f64 {
    24.0
}

fn default_pixelate() -> f64 {
    48.0
}

fn default_glitch() -> f64 {
    0.5
}

/// The common interface “numeric parameters with specs”. Both effects and
/// kinds of transitions implement it — therefore their field UI is ONE (requirement
/// R.33 item 1: “their control should be unified like the fx-chain”), and any
/// new carrier of parameters gets a ready grid of fields for free.
/// The passport of a FLAG — a parameter that has no range (R.53, item 414).
///
/// Numbers are described by `ParamSpec`, and “yes/no” is not a number: it cannot be dragged
/// by a slider and cannot be automated by a curve. Before this item such
/// fields were drawn as hand-written rows in each window — and for exactly that reason
/// they were forgotten: the 1 kHz tone of SMPTE and the color mode of the noise existed in
/// the format, but there was NOTHING with which to turn them on (class t252 “orphaned feature”).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagSpec {
    /// The name of the field, snake_case — it is also the key in the file.
    pub name: &'static str,
    pub default: bool,
}

/// Who owns the parameter (R.53, item 414). One and the same NAME means something different for
/// different carriers: `size` — grain in the Noise effect and the FONT SIZE of a title,
/// `amount` — strength for half a dozen effects. A global map “name → label” lied about this:
/// the title’s font size was labeled “Grain:”.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamScope {
    Fx,
    Generator,
}

pub trait HasParams {
    /// The passport of properties. Borrowed from `self` (rather than `&'static`), because
    /// for dynamic owners — the public parameter of a composition and
    /// a plugin — it is built at runtime (R.59, item 466).
    fn param_specs(&self) -> &[ParamSpec];
    fn get_param(&self, name: &str) -> Option<f64>;
    fn set_param(&mut self, name: &str, v: f64) -> bool;

    /// Reading by the INDEX of the passport row. The default is through the name (slow,
    /// but correct for any owner); the built-in kinds generate direct
    /// access by a macro.
    fn get_param_idx(&self, idx: usize) -> Option<f64> {
        let name = self.param_specs().get(idx)?.name.clone();
        self.get_param(&name)
    }
    fn set_param_idx(&mut self, idx: usize, v: f64) -> bool {
        let Some(name) = self.param_specs().get(idx).map(|s| s.name.clone()) else {
            return false;
        };
        self.set_param(&name, v)
    }

    /// Whose parameters these are — the label is looked up first in its own area
    /// (R.53, item 414).
    fn params_scope(&self) -> ParamScope {
        ParamScope::Fx
    }

    /// The flags of the kind (R.53, item 414). Empty — there are none; then the window draws
    /// not a single checkbox, and this is legitimate.
    fn flag_specs(&self) -> &'static [FlagSpec] {
        &[]
    }
    fn get_flag(&self, _name: &str) -> Option<bool> {
        None
    }
    fn set_flag(&mut self, _name: &str, _v: bool) -> bool {
        false
    }

    /// The spec of a parameter by name (an orphaned curve is visible at once).
    fn param_spec(&self, name: &str) -> Option<&ParamSpec> {
        self.param_specs().iter().find(|s| s.name == name)
    }

    /// Index of a row by name — a door from the named world (file, UI) into the
    /// index one (the compiled automation plan).
    fn param_idx(&self, name: &str) -> Option<usize> {
        self.param_specs().iter().position(|s| s.name == name)
    }

    /// Properties that are NOT among the numeric rows and flags: text, choice,
    /// a reference to media. Empty for the vast majority of owners —
    /// only the one who has them declares them (a title — its own text and
    /// the font family).
    fn extra_specs(&self) -> Vec<ParamSpec> {
        Vec::new()
    }

    /// Color groups are DERIVED from the passport (R.59, stage 3), rather than
    /// declared: a manual list would be a second source of the same fact.
    fn color_groups(&self) -> Vec<ColorGroup> {
        derive_color_groups(self.param_specs())
    }

    /// The type of a property by name. `None` — the owner has no such property,
    /// and this is exactly what validation uses to catch a publication pointing at a vanished target.
    fn value_type(&self, name: &str) -> Option<ParamType> {
        if self.color_groups().iter().any(|g| g.stem == name) {
            return Some(ParamType::Color);
        }
        if let Some(s) = self.param_spec(name) {
            return Some(s.ty.clone());
        }
        if self.flag_specs().iter().any(|f| f.name == name) {
            return Some(ParamType::Flag);
        }
        self.extra_specs().into_iter().find(|s| s.name == name).map(|s| s.ty)
    }

    /// The PASSPORT of a property by name — a mirror of `value_type` (R.60).
    ///
    /// The single source of limits, step and default for the publication and
    /// for the consumer’s window. There will be no copy of the passport in the publication, for the same
    /// reason that the value of the template is not stored: it would
    /// diverge from the target at the first edit inside the reel. The order
    /// of the branches must coincide with `value_type` — they would diverge, and for color
    /// the passport would come from the component while the type would come from the group.
    fn value_spec(&self, name: &str) -> Option<ParamSpec> {
        if self.color_groups().iter().any(|g| g.stem == name) {
            // the limits of color are taken from the component `_r`: it is the storage scale
            // (Rgb8), not an invention of the interface
            let comp = self.param_spec(&format!("{name}_r")).cloned();
            return Some(ParamSpec {
                name: std::borrow::Cow::Owned(name.to_string()),
                ty: ParamType::Color,
                min: comp.as_ref().map_or(0.0, |s| s.min),
                max: comp.as_ref().map_or(255.0, |s| s.max),
                default: comp.as_ref().map_or(0.0, |s| s.default),
                step: comp.as_ref().map_or(1.0, |s| s.step),
                unit: ParamUnit::Rgb8,
                kind: comp.as_ref().map_or(ParamKind::Smooth, |s| s.kind),
                label: std::borrow::Cow::Borrowed(""),
            });
        }
        if let Some(s) = self.param_spec(name) {
            return Some(s.clone());
        }
        if let Some(f) = self.flag_specs().iter().find(|f| f.name == name) {
            return Some(ParamSpec {
                name: std::borrow::Cow::Borrowed(f.name),
                ty: ParamType::Flag,
                min: 0.0,
                max: 1.0,
                default: f.default as u8 as f64,
                step: 1.0,
                unit: ParamUnit::None,
                kind: ParamKind::Structural,
                label: std::borrow::Cow::Borrowed(""),
            });
        }
        self.extra_specs().into_iter().find(|s| s.name == name)
    }

    /// TYPED reading. The default splits the name over three existing paths
    /// (number, flag, color components); owners with text and
    /// choice override it.
    fn get_value(&self, name: &str) -> Option<ParamValue> {
        default_get_value(self, name)
    }

    /// TYPED writing. Returns false if there is no such property or the type is wrong:
    /// a silent substitution of the type would be corruption.
    fn set_value(&mut self, name: &str, v: &ParamValue) -> bool {
        default_set_value(self, name, v)
    }
}

/// The body of the default `HasParams::get_value` as a FREE function: overriding
/// the method, the owner must be able to call the default, and from an overridden
/// trait method it is unavailable.
pub fn default_get_value<T: HasParams + ?Sized>(o: &T, name: &str) -> Option<ParamValue> {
    if let Some(g) = o.color_groups().into_iter().find(|g| g.stem == name) {
        let ch = |sfx: &str| {
            o.get_param(&format!("{name}_{sfx}")).unwrap_or(0.0).round().clamp(0.0, 255.0) as u8
        };
        let a = if g.alpha { ch("a") } else { 255 };
        return Some(ParamValue::Color([ch("r"), ch("g"), ch("b"), a]));
    }
    if o.param_spec(name).is_some() {
        return o.get_param(name).map(ParamValue::Num);
    }
    if o.flag_specs().iter().any(|f| f.name == name) {
        return o.get_flag(name).map(ParamValue::Flag);
    }
    None
}

/// The body of the default `HasParams::set_value`.
pub fn default_set_value<T: HasParams + ?Sized>(o: &mut T, name: &str, v: &ParamValue) -> bool {
    match v {
        ParamValue::Color(rgba) => {
            let Some(g) = o.color_groups().into_iter().find(|g| g.stem == name) else {
                return false;
            };
            let mut ok = true;
            for (i, sfx) in ["r", "g", "b"].iter().enumerate() {
                ok &= o.set_param(&format!("{name}_{sfx}"), rgba[i] as f64);
            }
            if g.alpha {
                ok &= o.set_param(&format!("{name}_a"), rgba[3] as f64);
            }
            ok
        }
        ParamValue::Num(x) => o.param_spec(name).is_some() && o.set_param(name, *x),
        ParamValue::Flag(b) => o.set_flag(name, *b),
        _ => false,
    }
}

/// The spec of a text property (the passport is built at runtime — R.59).
pub fn text_spec(name: &'static str, multiline: bool) -> ParamSpec {
    ParamSpec {
        name: std::borrow::Cow::Borrowed(name),
        ty: ParamType::Text { multiline },
        min: 0.0,
        max: 0.0,
        default: 0.0,
        step: 0.0,
        unit: ParamUnit::None,
        kind: ParamKind::Smooth,
        label: std::borrow::Cow::Borrowed(""),
    }
}

/// The spec of a choice property; the variants are KEYS, not numbers.
pub fn choice_spec(name: &'static str, variants: &'static [&'static str]) -> ParamSpec {
    ParamSpec {
        name: std::borrow::Cow::Borrowed(name),
        ty: ParamType::Choice { variants: std::borrow::Cow::Borrowed(variants) },
        min: 0.0,
        max: 0.0,
        default: 0.0,
        step: 0.0,
        unit: ParamUnit::None,
        kind: ParamKind::Smooth,
        label: std::borrow::Cow::Borrowed(""),
    }
}

impl HasParams for FxInstance {
    fn param_specs(&self) -> &[ParamSpec] {
        FxInstance::param_specs(self)
    }
    fn get_param_idx(&self, idx: usize) -> Option<f64> {
        fx_get_idx(self, idx)
    }
    fn set_param_idx(&mut self, idx: usize, v: f64) -> bool {
        fx_set_idx(self, idx, v)
    }
    fn get_param(&self, name: &str) -> Option<f64> {
        FxInstance::get_param(self, name)
    }
    fn set_param(&mut self, name: &str, v: f64) -> bool {
        FxInstance::set_param(self, name, v)
    }
}

impl HasParams for TransitionKind {
    fn param_specs(&self) -> &[ParamSpec] {
        TransitionKind::param_specs(self)
    }
    fn get_param(&self, name: &str) -> Option<f64> {
        TransitionKind::get_param(self, name)
    }
    fn set_param(&mut self, name: &str, v: f64) -> bool {
        TransitionKind::set_param(self, name, v)
    }
}

impl TransitionKind {
    /// All kinds with the DEFAULTS of parameters are the single registry: the selection menu
    /// in the UI is built from it, and the tests also check the completeness of the specs
    /// against it. A new kind that is not listed here shows up in a test at once.
    pub const ALL: &'static [TransitionKind] = &[
        TransitionKind::Crossfade,
        TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.15 },
        TransitionKind::Iris { shape: IrisShape::Circle, cx: 0.5, cy: 0.5, softness: 0.15 },
        TransitionKind::Clock { reverse: false, cx: 0.5, cy: 0.5, softness: 0.15 },
        TransitionKind::Barn { vertical: false, softness: 0.15 },
        TransitionKind::Luma { invert: false, softness: 0.15 },
        TransitionKind::Push { dir: Dir::Left },
        TransitionKind::Slide { dir: Dir::Left },
        TransitionKind::Zoom { amount: 0.4, cx: 0.5, cy: 0.5 },
        TransitionKind::Spin { turns: 1.0, amount: 0.4, cx: 0.5, cy: 0.5 },
        TransitionKind::Whip { dir: Dir::Left, blur: 0.6 },
        TransitionKind::BlurDissolve { amount: 24.0 },
        TransitionKind::Pixelate { amount: 48.0 },
        TransitionKind::Glitch { amount: 0.5 },
        TransitionKind::Flash { color_r: 255, color_g: 255, color_b: 255, strength: 1.0 },
        TransitionKind::Dip { color_r: 0, color_g: 0, color_b: 0 },
        TransitionKind::Blinds { count: 8, angle: 0.0, softness: 0.1 },
        TransitionKind::Checker { cols: 8, rows: 6, softness: 0.1 },
        TransitionKind::Blocks { cols: 16, rows: 9, softness: 0.25 },
        TransitionKind::Spiral { turns: 2.0, cx: 0.5, cy: 0.5, softness: 0.1 },
        TransitionKind::Split { vertical: false },
        TransitionKind::Squeeze { dir: Dir::Left },
        TransitionKind::Stretch { vertical: false },
        TransitionKind::Swap { vertical: false, depth: 0.3 },
        TransitionKind::Shake { amount: 0.5, steps: 12 },
        TransitionKind::FilmBurn { scale: 6.0, softness: 0.2, tint_r: 255, tint_g: 120, tint_b: 0 },
        TransitionKind::LightLeaks { strength: 0.6, tint_r: 255, tint_g: 90, tint_b: 40 },
        TransitionKind::ZoomBlur { amount: 0.6, cx: 0.5, cy: 0.5 },
        TransitionKind::Flip { vertical: false, persp: 0.5, shade: 0.5 },
        TransitionKind::Cube { dir: Dir::Left, persp: 0.5, shade: 0.5 },
        TransitionKind::Door { vertical: false, persp: 0.5, shade: 0.5 },
        TransitionKind::PagePeel { dir: Dir::Left, curl: 0.18, shadow: 0.5 },
        TransitionKind::PageRoll { dir: Dir::Left, curl: 0.18, shadow: 0.5 },
    ];

    /// The tag of the kind, as it is in the file (for the UI and comparison).
    pub fn tag(&self) -> &'static str {
        match self {
            TransitionKind::Crossfade => "crossfade",
            TransitionKind::Wipe { .. } => "wipe",
            TransitionKind::Iris { .. } => "iris",
            TransitionKind::Clock { .. } => "clock",
            TransitionKind::Barn { .. } => "barn",
            TransitionKind::Luma { .. } => "luma",
            TransitionKind::Push { .. } => "push",
            TransitionKind::Slide { .. } => "slide",
            TransitionKind::Zoom { .. } => "zoom",
            TransitionKind::Spin { .. } => "spin",
            TransitionKind::Whip { .. } => "whip",
            TransitionKind::BlurDissolve { .. } => "blur_dissolve",
            TransitionKind::Pixelate { .. } => "pixelate",
            TransitionKind::Glitch { .. } => "glitch",
            TransitionKind::Flash { .. } => "flash",
            TransitionKind::Dip { .. } => "dip",
            TransitionKind::Blinds { .. } => "blinds",
            TransitionKind::Checker { .. } => "checker",
            TransitionKind::Blocks { .. } => "blocks",
            TransitionKind::Spiral { .. } => "spiral",
            TransitionKind::Split { .. } => "split",
            TransitionKind::Squeeze { .. } => "squeeze",
            TransitionKind::Stretch { .. } => "stretch",
            TransitionKind::Swap { .. } => "swap",
            TransitionKind::Shake { .. } => "shake",
            TransitionKind::FilmBurn { .. } => "film_burn",
            TransitionKind::LightLeaks { .. } => "light_leaks",
            TransitionKind::ZoomBlur { .. } => "zoom_blur",
            TransitionKind::Flip { .. } => "flip",
            TransitionKind::Cube { .. } => "cube",
            TransitionKind::Door { .. } => "door",
            TransitionKind::PagePeel { .. } => "page_peel",
            TransitionKind::PageRoll { .. } => "page_roll",
        }
    }

    /// Symmetric kinds: `kernel(b, a, 1−k)` for them is identical to
    /// `kernel(a, b, k)`, the inversion changes nothing — the UI hides the checkbox
    /// (rule 16: a non-working control is not shown). The match
    /// is exhaustive on purpose: a new kind must decide whether it is symmetric.
    pub fn symmetric(&self) -> bool {
        match self {
            // for zoom both sides are computed by the same formula from their own role, and the
            // mix identity `mix(y, x, 1−t) == mix(x, y, t)` makes
            // the inversion an exact identity (spin does not: the sign of the rotation
            // changes on inversion, and this is meaningful).
            TransitionKind::Crossfade
            | TransitionKind::Zoom { .. }
            | TransitionKind::BlurDissolve { .. }
            | TransitionKind::Pixelate { .. }
            | TransitionKind::Glitch { .. }
            | TransitionKind::Flash { .. }
            | TransitionKind::Dip { .. }
            // the strength of the rays sin(πk) and the change of side in the middle are symmetric
            | TransitionKind::ZoomBlur { .. } => true,
            TransitionKind::Wipe { .. }
            | TransitionKind::Iris { .. }
            | TransitionKind::Clock { .. }
            | TransitionKind::Barn { .. }
            | TransitionKind::Luma { .. }
            | TransitionKind::Push { .. }
            | TransitionKind::Slide { .. }
            | TransitionKind::Spin { .. }
            | TransitionKind::Whip { .. }
            | TransitionKind::Blinds { .. }
            | TransitionKind::Checker { .. }
            | TransitionKind::Blocks { .. }
            | TransitionKind::Spiral { .. }
            | TransitionKind::Split { .. }
            | TransitionKind::Squeeze { .. }
            | TransitionKind::Stretch { .. }
            | TransitionKind::Swap { .. }
            | TransitionKind::Shake { .. }
            | TransitionKind::FilmBurn { .. }
            | TransitionKind::LightLeaks { .. }
            | TransitionKind::Flip { .. }
            | TransitionKind::Cube { .. }
            | TransitionKind::Door { .. }
            | TransitionKind::PagePeel { .. }
            | TransitionKind::PageRoll { .. } => false,
        }
    }

    /// The numeric parameters of the kind — the same spec as for effects, so the UI
    /// draws them with the same code.
    pub fn param_specs(&self) -> &'static [ParamSpec] {
        transition_specs(self)
    }

    pub fn param_spec(&self, name: &str) -> Option<&'static ParamSpec> {
        self.param_specs().iter().find(|s| s.name == name)
    }

    pub fn set_param(&mut self, name: &str, v: f64) -> bool {
        let Some(spec) = self.param_spec(name) else { return false };
        transition_set_param(self, name, v.clamp(spec.min, spec.max))
    }

    pub fn get_param(&self, name: &str) -> Option<f64> {
        transition_get_param(self, name)
    }
}

/// The curve of the audio crossfade (R.33 item 1, the user’s decision: “wipes/glitches” have
/// no sound meaning, so the sound always crossfades, but the curve can be chosen).
/// The default is equal-power:
/// exactly the former formula; GOLDEN_AUDIO does not move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioCurve {
    #[default]
    EqualPower,
    Linear,
    SCurve,
    Exponential,
}

impl AudioCurve {
    /// The weight of the RISING side at position t ∈ [0,1]. The falling side is
    /// computed by the same code from (1 − t) — otherwise the halves diverge.
    pub fn rise(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            AudioCurve::EqualPower => t.sqrt(),
            AudioCurve::Linear => t,
            // S-curve: a soft start and a soft end, the sum of powers is smoother
            AudioCurve::SCurve => (t * t * (3.0 - 2.0 * t)).sqrt(),
            // exponential: a long quiet entry, a sharp rise at the end
            AudioCurve::Exponential => t * t,
        }
    }
}

/// A transition at the joint of two ADJACENT clips of one track (R.24, OTIO
/// Transition). The position is NOT stored — it is derived from the clips: the joint == the end of `a`
/// == the start of `b`; the identity by the pair of ClipIds survives group moves
/// (the joint moved for both — the transition lives). Window: [joint − in_offset,
/// joint + out_offset); in_offset lies in the body of A (incoming plays the left
/// “handle” of the source), out_offset is in the body of B (outgoing plays the right one).
/// The material of the handles is reading of the source beyond the used piece;
/// beyond the edges of the material the engine gives silence/a clamped frame (the existing
/// policy, nothing is serialized).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionEvent {
    pub id: TransitionId,
    /// The outgoing clip (its end is the joint). `None` — a fade-in: on the left there is
    /// emptiness (the track edge or a Gap).
    pub a: Option<ClipId>,
    /// The incoming clip (its start is the joint). `None` — a fade-out: on the right there is emptiness.
    pub b: Option<ClipId>,
    pub in_offset: Tick,
    pub out_offset: Tick,
    #[serde(default)]
    pub kind: TransitionKind,
    /// The curve of the audio crossfade (R.33). The equal-power default is not
    /// serialized — files of earlier versions are byte-stable.
    #[serde(default, skip_serializing_if = "is_default_curve")]
    pub audio: AudioCurve,
    /// The curves of the PARAMETERS of the kind (R.34 K4) — the same mechanism as for
    /// effects and cameras. The key position is a tick from the START of the transition window
    /// (`joint − in_offset`): editing the edge of the window opens or hides keys,
    /// as with a clip trim, rather than stretching the animation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto: Vec<ParamCurve>,
    /// The inversion of a transition (R.51, item 398): the gesture is played backward —
    /// the executors swap the sides and reverse the progress
    /// (`kernel(b, a, 1−k)`), the transition itself still goes from A to B.
    /// The private flags of the kinds (`Clock.reverse` — the direction of the hand,
    /// `Luma.invert` — the brightness threshold) are OTHER axes, they remain.
    /// The default is not serialized — files of earlier versions are byte-stable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub invert: bool,
}

fn is_default_curve(c: &AudioCurve) -> bool {
    *c == AudioCurve::EqualPower
}

impl TransitionEvent {
    /// The kind WITH the curves taken into account at position `local` (a tick from the start of the window).
    /// The only point of resolution — both render executors call it.
    pub fn kind_at(&self, local: Tick) -> TransitionKind {
        if self.auto.is_empty() {
            return self.kind.clone();
        }
        let mut k = self.kind.clone();
        for c in &self.auto {
            if !c.keys.is_empty() {
                k.set_param(&c.param, curve_value(&c.keys, local));
            }
        }
        k
    }

    /// The joint: the end of `a`, and for a fade-in — the start of `b` (R.25). None — both ends
    /// have vanished; such a state lives only inside a mutation before
    /// reconcile_transitions.
    pub fn joint(&self, p: &dyn Reel) -> Option<Tick> {
        if let Some(a) = self.a.and_then(|id| p.clips().iter().find(|c| c.id == id)) {
            return Some(a.start + a.duration);
        }
        let b = self.b.and_then(|id| p.clips().iter().find(|c| c.id == id))?;
        Some(b.start)
    }

    /// A fade — a transition with ONE live end: on the side where `None` is, there is
    /// emptiness (the track edge or a Gap), and the layer goes to transparency/silence.
    /// The reference library treats such a transition the same way (dissolve with a Gap).
    pub fn is_fade(&self) -> bool {
        self.a.is_none() || self.b.is_none()
    }

    /// A clip to whose body a transition is adjacent on both sides of the window.
    pub fn touches(&self, id: ClipId) -> bool {
        self.a == Some(id) || self.b == Some(id)
    }

    /// The pair of an OVERLAPPED transition (R.51, item 399): both ends are live, one
    /// (kind, lane) and an overlap in a “staircase” — a started earlier and ended
    /// inside b. A full nesting and equal starts are not counted as a pair:
    /// such an arrangement has neither a direction nor a meaningful window.
    pub fn overlap_pair<'a>(&self, p: &'a dyn Reel) -> Option<(&'a ClipEvent, &'a ClipEvent)> {
        let a = p.clips().iter().find(|c| Some(c.id) == self.a)?;
        let b = p.clips().iter().find(|c| Some(c.id) == self.b)?;
        let (a_end, b_end) = (a.start + a.duration, b.start + b.duration);
        (a.kind == b.kind
            && a.lane == b.lane
            && a.start < b.start
            && b.start < a_end
            && a_end < b_end)
            .then_some((a, b))
    }

    /// The half-open window of the transition on the timeline. For a joint one — the offsets from the
    /// joint; for an overlapped one (R.51) the window is the overlap zone ITSELF: the mode is not
    /// stored, the window follows the trim of the clips, it has no handles of its own
    /// (the Kdenlive lesson from F-TL-010 — we do not introduce a second window subsystem).
    /// `in/out_offset` are not used in the overlap mode (they lie as dead weight
    /// and come back to life when the pair is joined again).
    pub fn window(&self, p: &dyn Reel) -> Option<TickRange> {
        if let Some((a, b)) = self.overlap_pair(p) {
            return Some(TickRange::new(b.start, a.start + a.duration));
        }
        let at = self.joint(p)?;
        Some(TickRange::new(at - self.in_offset, at + self.out_offset))
    }
}

/// THE EDIT REEL: events and the transitions between them (R.50, item 378).
///
/// Both the project root and a nested sequence implement it. The trait exists
/// for one thing: the whole geometry of transitions (`joint`, `window`,
/// `check_transition_full`, `forbidden_zones`) must ask THE
/// reel on which the transition stands. A transition whose ends lie in a sequence,
/// while it itself is looked up among the root clips, would return `None` — and
/// `reconcile_transitions` would kill it on the very first command.
///
/// A trait was taken, not a view structure: then the 29 existing calls of the kind
/// `t.joint(project)` continue to compile verbatim, while the new path
/// simply passes `&Sequence`.
pub trait Reel {
    fn clips(&self) -> &[ClipEvent];
    fn transitions(&self) -> &[TransitionEvent];
    /// Rows top to bottom. The only way to know what is “below”, and —
    /// since R.56 — what the NUMBER of the track must correspond to.
    fn rows(&self) -> Vec<(LaneKind, u32)>;
    /// Object tracks (R.62).
    fn tracks(&self) -> &[Track];
    /// The rest of the content of the reel. The defaults are empty: the implementer
    /// must override only what it has.
    fn morphers(&self) -> &[MorpherEvent] {
        &[]
    }
    fn cameras(&self) -> &[CameraEvent] {
        &[]
    }
    fn foreign(&self) -> &[ForeignEvent] {
        &[]
    }
    fn overlaps_allowed(&self) -> bool {
        true
    }
    /// Handles declared outward (R.59, stage 3). The root is empty by
    /// its meaning: the root is no one’s instance.
    fn published(&self) -> &[PublishedParam] {
        &[]
    }
}

/// The zones where the boundary of a morpher layer does not stand: the transition windows of this clip
/// and the overlaps of neighbors on the track. A cut there breaks not the target but the SURROUNDINGS (R.37).
///
/// A free function, not a trait method: `Reel` must remain
/// object-safe (the walk of the reel tree goes through `&dyn Reel`), while the body
/// calls `t.joint(reel)` — there is no way to do the coercion `&Self -> &dyn Reel` inside
/// a provided method.
pub fn forbidden_zones(reel: &dyn Reel, c: &ClipEvent) -> Vec<TickRange> {
    let body = TickRange::new(c.start, c.start + c.duration);
    let mut out = Vec::new();
    for t in reel.transitions() {
        if t.a != Some(c.id) && t.b != Some(c.id) {
            continue;
        }
        // for an overlapped transition (R.51) the window is the overlap zone
        let Some(w) = t.window(reel) else { continue };
        let z = w.intersect(&body);
        if !z.is_empty() {
            out.push(z);
        }
    }
    for o in reel.clips() {
        if o.id == c.id || o.kind != c.kind || o.lane != c.lane {
            continue;
        }
        let z = TickRange::new(o.start, o.start + o.duration).intersect(&body);
        if !z.is_empty() {
            out.push(z);
        }
    }
    out
}

// ---- EXPANDING A REEL IN PLACE (R.55, C5) -----------------------------------

/// How many tracks of each kind. It serves both as “where the band starts” and
/// “how big it is”.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LaneBases {
    pub video: u32,
    pub audio: u32,
    pub morph: u32,
    pub camera: u32,
}

impl LaneBases {
    pub fn get(&self, k: LaneKind) -> u32 {
        match k {
            LaneKind::Video => self.video,
            LaneKind::Audio => self.audio,
            LaneKind::Morph => self.morph,
            LaneKind::Camera => self.camera,
        }
    }

    pub fn get_mut(&mut self, k: LaneKind) -> &mut u32 {
        match k {
            LaneKind::Video => &mut self.video,
            LaneKind::Audio => &mut self.audio,
            LaneKind::Morph => &mut self.morph,
            LaneKind::Camera => &mut self.camera,
        }
    }

    pub fn of_root(p: &Project) -> LaneBases {
        LaneBases::of_tracks(&p.tracks)
    }

    pub fn of_seq(s: &Sequence) -> LaneBases {
        LaneBases::of_tracks(&s.tracks)
    }

    pub fn of_tracks(tracks: &[Track]) -> LaneBases {
        let mut n = LaneBases::default();
        for t in tracks {
            *n.get_mut(t.kind) += 1;
        }
        n
    }
}

/// THE BAND OF EXPANSION: a reel laid out onto the root tracks RIGHT BELOW its own
/// instance (R.55, C5).
///
/// The technique is the same as in `swap_reel` (item 392), and for the same reason: the context
/// of an edit is made by SWAPPING THE CONTENT, rather than by a parameter in
/// sixty hands. Only here the swap goes WITH AN OFFSET — in
/// time and in track number:
///
/// ```text
/// t_root = t_reel + dt,   dt = inst.start − inst.source_in
/// track = its own + base[kind], base[kind] = how many tracks of this kind the root has
/// ```
///
/// What follows from this and why it was all undertaken: **drawing, hit testing,
/// snapping, magnet, frame, selection, `rows()`, `MovePlan`, `forbidden_zones`
/// and `reconcile_transitions` do not change by a single line**. The events of the band
/// physically lie in the root vectors, therefore `TrimStart{id}` reaches
/// them by the ordinary path, and “we missed one place — the edit silently drifted into
/// the root” cannot happen: there is nothing to drift physically with, and the violation is caught by
/// `unmerge_reel`, which rejects the whole batch.
///
/// The band occupies the TAIL of the numeric space of the tracks, while the visible place
/// under the instance is set only through `lane_order`. Then the root
/// numbers are all `< base`, the band numbers are all `>= base`, and an insertion/deletion
/// of a track in the root does not touch the band — by construction, not by a check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Band {
    /// Through which instance it is expanded.
    pub inst: ClipId,
    /// Which reel is laid out.
    pub seq: MediaId,
    /// The time shift: `t_root = t_reel + dt`.
    pub dt: Tick,
    /// From which number the tracks of the band start.
    pub base: LaneBases,
    /// How many of them.
    pub len: LaneBases,
}

impl Band {
    /// The band for the instance `inst`. `None` — this is not an attachment.
    ///
    /// `dt` is invariant under a trim: `TrimStart`/`TrimEnd` move `start` and
    /// `source_in` by ONE delta (`clip_slice`), so when the edge of the instance is dragged
    /// the content of the band does not move on screen — the window
    /// opens over motionless material. Exactly this gesture the owner
    /// asked for: “the edge can be pulled right here”.
    pub fn of(p: &Project, inst: ClipId) -> Option<Band> {
        let c = p.clips.iter().find(|c| c.id == inst)?;
        let s = p.sequence(c.media)?;
        // The block of the reel’s tracks lands RIGHT BELOW the instance row (R.62,
        // change B: tracks are objects, number = rank of the vector). Therefore
        // the first track of the band of each kind gets the rank “how many
        // rows of this kind stand not below the instance”.
        let rows = p.rows();
        let at = rows.iter().position(|&k| k == lane_key(c)).map(|i| i + 1).unwrap_or(rows.len());
        let mut base = LaneBases::default();
        for &(k, _) in &rows[..at] {
            *base.get_mut(k) += 1;
        }
        Some(Band {
            inst,
            seq: c.media,
            dt: c.start - c.source_in,
            base,
            len: LaneBases::of_seq(s),
        })
    }

    /// The window of the instance in the coordinates of the ROOT: what of the band is visible, and what —
    /// the tail beyond the crop.
    pub fn window(&self, p: &Project) -> Option<TickRange> {
        let c = p.clips.iter().find(|c| c.id == self.inst)?;
        Some(TickRange::new(c.start, c.start + c.duration))
    }

    /// Whether the track belongs to the band.
    pub fn owns(&self, kind: LaneKind, lane: u32) -> bool {
        let b = self.base.get(kind);
        lane >= b && lane < b + self.len.get(kind)
    }

    /// The rows of the band in the ROOT numbers, top to bottom by kind.
    pub fn rows(&self) -> Vec<(LaneKind, u32)> {
        let mut out = Vec::new();
        for k in [LaneKind::Video, LaneKind::Audio, LaneKind::Morph, LaneKind::Camera] {
            let b = self.base.get(k);
            for i in 0..self.len.get(k) {
                out.push((k, b + i));
            }
        }
        out
    }
}

impl Project {
    /// Lay out the instance’s reel onto the root tracks WITH AN OFFSET (R.55, C5).
    ///
    /// The body of the reel is EMPTIED: the events moved rather than were copied —
    /// therefore `max_clip_id` and its kin continue to see each address exactly
    /// once (item 394), and `unmerge_reel` puts everything back in place.
    pub fn merge_reel(&mut self, b: &Band) {
        let Some(i) = self.sequences.iter().position(|s| s.id == b.seq) else { return };
        let (mut clips, mut trans, mut morphs, mut cams, tracks) = {
            let s = &mut self.sequences[i];
            (
                std::mem::take(&mut s.clips),
                std::mem::take(&mut s.transitions),
                std::mem::take(&mut s.morphers),
                std::mem::take(&mut s.cameras),
                std::mem::take(&mut s.tracks),
            )
        };
        // the root content below the band block is shifted by its length:
        // number = rank, and the block lands between
        let shift =
            |k: LaneKind, lane: u32| if lane >= b.base.get(k) { lane + b.len.get(k) } else { lane };
        for c in &mut self.clips {
            c.lane = shift(lane_key(c).0, c.lane);
        }
        for m in &mut self.morphers {
            m.lane = shift(LaneKind::Morph, m.lane);
        }
        for c in &mut self.cameras {
            c.lane = shift(LaneKind::Camera, c.lane);
        }
        for f in &mut self.foreign {
            f.lane = shift(lane_key_of(f.kind), f.lane);
        }
        for a in &mut self.areas {
            for e in a.lanes.iter_mut() {
                e.1 = shift(e.0, e.1);
            }
        }
        for c in &mut clips {
            c.start = c.start + b.dt;
            c.lane += b.base.get(lane_key(c).0);
        }
        for m in &mut morphs {
            m.start = m.start + b.dt;
            m.lane += b.base.morph;
        }
        for c in &mut cams {
            c.start = c.start + b.dt;
            c.lane += b.base.camera;
        }
        self.clips.append(&mut clips);
        self.transitions.append(&mut trans);
        self.morphers.append(&mut morphs);
        self.cameras.append(&mut cams);
        // THE ROWS OF THE BAND land as an unbroken block RIGHT BELOW the row
        // of the instance
        let rows = self.rows();
        let at = self
            .clips
            .iter()
            .find(|c| c.id == b.inst)
            .and_then(|c| rows.iter().position(|&k| k == lane_key(c)))
            .map(|p| p + 1)
            .unwrap_or(rows.len());
        for (j, t) in tracks.into_iter().enumerate() {
            self.tracks.insert(at + j, t);
        }
    }

    /// Assemble the reel back — WITH PROOF that no one changed the band.
    ///
    /// An error here is not a trifle but the guard of the whole construction: it means
    /// that a gesture carried an event across the boundary of reels, and the batch must be
    /// rejected ENTIRELY, rather than “patched up”. That is exactly why
    /// addressing of edits is not needed: there is nothing to drift away physically with,
    /// and the attempt is caught by exactly this place.
    pub fn unmerge_reel(&mut self, b: &Band) -> Result<(), ProjectError> {
        if !self.sequences.iter().any(|s| s.id == b.seq) {
            return Ok(()); // the reel no longer exists (undo) — there is nothing to unmerge
        }
        let mine_clip = |c: &ClipEvent| b.owns(lane_key(c).0, c.lane);
        let (mut inner, root): (Vec<ClipEvent>, Vec<ClipEvent>) =
            std::mem::take(&mut self.clips).into_iter().partition(mine_clip);
        self.clips = root;
        // The INSTANCE must remain in the root: drag it inside — the reel
        // would come to contain itself
        if inner.iter().any(|c| c.id == b.inst) {
            return Err(ProjectError::BandEscape(b.inst));
        }
        let (mut morphs, root_m): (Vec<MorpherEvent>, Vec<MorpherEvent>) =
            std::mem::take(&mut self.morphers)
                .into_iter()
                .partition(|m| b.owns(LaneKind::Morph, m.lane));
        self.morphers = root_m;
        let (mut cams, root_c): (Vec<CameraEvent>, Vec<CameraEvent>) =
            std::mem::take(&mut self.cameras)
                .into_iter()
                .partition(|c| b.owns(LaneKind::Camera, c.lane));
        self.cameras = root_c;
        // A TRANSITION is either entirely in the band or in the root: an end on one side of
        // the boundary is a transition across the boundary of reels, which does not occur in the
        // model (R.50, t425)
        let inner_ids: std::collections::HashSet<ClipId> = inner.iter().map(|c| c.id).collect();
        let mut trans = Vec::new();
        let mut root_t = Vec::new();
        for t in std::mem::take(&mut self.transitions) {
            let side = |id: Option<ClipId>| id.map(|i| inner_ids.contains(&i));
            match (side(t.a), side(t.b)) {
                (Some(true), Some(false)) | (Some(false), Some(true)) => {
                    return Err(ProjectError::BandEscape(t.a.or(t.b).unwrap_or(b.inst)));
                }
                (Some(true), _) | (_, Some(true)) => trans.push(t),
                _ => root_t.push(t),
            }
        }
        self.transitions = root_t;
        for c in &mut inner {
            c.start = c.start - b.dt;
            c.lane -= b.base.get(lane_key(c).0);
        }
        for m in &mut morphs {
            m.start = m.start - b.dt;
            m.lane -= b.base.morph;
        }
        for c in &mut cams {
            c.start = c.start - b.dt;
            c.lane -= b.base.camera;
        }
        // the root content below the block returns to its numbers
        let unshift = |k: LaneKind, lane: u32| {
            if lane >= b.base.get(k) + b.len.get(k) { lane - b.len.get(k) } else { lane }
        };
        for c in &mut self.clips {
            c.lane = unshift(lane_key(c).0, c.lane);
        }
        for m in &mut self.morphers {
            m.lane = unshift(LaneKind::Morph, m.lane);
        }
        for c in &mut self.cameras {
            c.lane = unshift(LaneKind::Camera, c.lane);
        }
        for f in &mut self.foreign {
            f.lane = unshift(lane_key_of(f.kind), f.lane);
        }
        for a in &mut self.areas {
            for e in a.lanes.iter_mut() {
                e.1 = unshift(e.0, e.1);
            }
        }
        // the TRACKS of the band are returned to the reel in the order of the rows; the root
        // keeps its vector without the block
        let rows = self.rows();
        let mut band_tracks = Vec::new();
        let mut root_tracks = Vec::new();
        for (idx, t) in std::mem::take(&mut self.tracks).into_iter().enumerate() {
            let (k, l) = rows[idx];
            if b.owns(k, l) {
                band_tracks.push(t);
            } else {
                root_tracks.push(t);
            }
        }
        self.tracks = root_tracks;
        let s =
            self.sequences.iter_mut().find(|s| s.id == b.seq).expect("reel presence checked above");
        s.clips = inner;
        s.transitions = trans;
        s.morphers = morphs;
        s.cameras = cams;
        s.tracks = band_tracks;
        s.ensure_tracks(LaneKind::Video, 1);
        s.ensure_tracks(LaneKind::Audio, 1);
        Ok(())
    }
}

/// The high bit of the media address is given to nested reels (R.50).
///
/// The engine needs ONE `SourceId` per reel — the cache key, the post-FX memoization key,
/// the address in the perf panel. It is taken as `NEST_TAG | seq`, therefore
/// the address of a real file must lie below: otherwise the file and the sequence would
/// silently share the cache. Checked in `validate`.
pub const NEST_TAG: u64 = 1 << 63;

/// The next bit of the media address is given to the GENERATOR ON THE EVENT (R.59, stage 2).
///
/// The values of the generator live on the clip, so the source of each clip is ITS OWN:
/// the address is taken as `GEN_TAG | id` of the event. The address is computed from the
/// EVENT, not from the values — it enters the plan, and a hash of the values in it would
/// mean rebuilding the plan on every keystroke (exactly what the owner called “terrible” in R.53).
/// The shape of the order `(address, tick, quality)` does not change,
/// therefore
/// “order == read”, the coverage bars, prefetch and export chunks work without edits.
///
/// The tag must lie BELOW `NEST_TAG` and ABOVE any clip id — including the
/// synthetic ids of the pieces of the morpher layer (`slice_id`). This is what keeps
/// the `const _` below.
pub const GEN_TAG: u64 = 1 << 62;

/// The next bit of the media address is given to the SUB-COMPOSITE OF A TRACK (R.62, change
/// B). A track with a chain, a level ≠ 1 or a blend mode ≠ Normal
/// enters the plan as ONE nesting layer, and its sub-composite needs its own
/// `SourceId` — the memoization key and the GPU plan key. It has no real reel,
/// so the address is synthetic: `NEST_TAG | TRACK_TAG | id of the track`.
///
/// The bit lies BELOW `NEST_TAG` and above any reel address: the reel addresses are
/// the `MediaId`s of the document, they are smaller than `TRACK_TAG` by construction
/// (`validate` rejects `MediaId >= NEST_TAG`, and track ids grow from 1).
pub const TRACK_TAG: u64 = 1 << 61;

/// The address of the track’s sub-composite in the media space.
/// SIDECHAIN KEYS of the chain (R.65): which devices have a second input assigned.
/// One home for the answer to “what is a key in the chain” — it is asked by both the check
/// of the command, and the engine, and the format.
pub fn sidechain_keys(fx: &[FxInstance]) -> impl Iterator<Item = TrackId> + '_ {
    fx.iter().filter_map(|f| match f {
        FxInstance::Compressor { key, .. } => *key,
        _ => None,
    })
}

/// Whether the key graph closes if the track `target` gets the chain `fx`.
///
/// A cycle cannot be executed: the frame of the track would depend on itself. Therefore
/// it is rejected AT WRITE TIME (the engine insures itself with its own recursion break).
pub fn sidechain_cycle(tracks: &[Track], target: TrackId, fx: &[FxInstance]) -> bool {
    // a walk from each key of the proposed chain: if it reaches target — a cycle
    let keys_of = |id: TrackId| -> Vec<TrackId> {
        if id == target {
            return sidechain_keys(fx).collect();
        }
        tracks
            .iter()
            .find(|t| t.id == id)
            .map(|t| sidechain_keys(&t.fx).collect())
            .unwrap_or_default()
    };
    let mut seen: Vec<TrackId> = Vec::new();
    let mut stack: Vec<TrackId> = keys_of(target);
    while let Some(id) = stack.pop() {
        if id == target {
            return true;
        }
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        stack.extend(keys_of(id));
    }
    false
}

pub fn track_source_media(id: TrackId) -> MediaId {
    MediaId(TRACK_TAG | id.0)
}

/// The depth limit of nesting (R.50, item 378).
///
/// The number is COMPUTED, not picked as a round one: at the moment of the composite of a level, two
/// accumulators of this level are alive in the video card’s memory, plus one
/// accumulator and one result per ancestor — about 25 MiB per level
/// at 1080p, against a temporary texture budget of 128 MiB (kt-gpu). The limit
/// stands here too (hard, with the reel names in the error text), and in
/// the executor (a graceful `TooLarge` with a fallback to the CPU).
pub const MAX_NEST_DEPTH: usize = 4;

/// Nested sequence — a reel that is REFERENCED (R.50, item 378).
///
/// It has NO settings of its own — no fps, no raster, no rate: a nested clip
/// inherits from its parent (a direct decision of the owner). It follows that
/// the round's main technical property is this: no intermediate raster arises,
/// so **a nested clip with an empty chain is identical to its expanded content**,
/// and the expansion in `effective_clips` serves as the reference
/// oracle for the real subtree of the plan.
///
/// The address is a `MediaId` in ONE space together with media: `ClipEvent.media`
/// points either to a file (`Project.media`) or to a reel
/// (`Project.sequences`). The sequence has NO `MediaRef` on purpose — otherwise
/// `build_player` would raise a `MediaWorker` for it, the watchdog would tint it
/// with the offline tint, and the index service would start building ffmpeg peaks for it.
/// The listed paths iterate `project.media`, where there are no sequences, and
/// remain correct BY CONSTRUCTION.
///
/// The name is a label, NOT an address (unlike `MarkedArea`): renaming
/// has no chance to break the link, and deduplication of bodies when writing to .otio
/// relies on a stable key that survives renaming.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sequence {
    pub id: MediaId,
    /// Label in the bin and `Stack.name` in the file. Non-empty (otherwise a card in the bin would have no label),
    /// but NOT required to be unique.
    pub name: String,
    pub clips: Vec<ClipEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<TransitionEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub morphers: Vec<MorpherEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<CameraEvent>,
    /// The reel's tracks are the same objects as the root's (R.62).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<Track>,
    #[serde(default, rename = "video_lanes", skip_serializing)]
    pub legacy_video_lanes: u32,
    #[serde(default, rename = "audio_lanes", skip_serializing)]
    pub legacy_audio_lanes: u32,
    #[serde(default, rename = "morph_lanes", skip_serializing)]
    pub legacy_morph_lanes: u32,
    #[serde(default, rename = "camera_lanes", skip_serializing)]
    pub legacy_camera_lanes: u32,
    #[serde(default, rename = "lane_order", skip_serializing)]
    pub legacy_lane_order: Vec<(LaneKind, u32)>,
    #[serde(default = "default_true")]
    pub overlap_layers: bool,
    /// Externally declared handles of the reel (R.59, stage 3). An empty list is not
    /// serialized — a reel without publications behaves and is written as it is today,
    /// byte for byte.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub published: Vec<PublishedParam>,
    /// The reel's OWN raster (R.62, a revision of R.50 at the owner's request).
    /// `None` — the project. The instance is conformed to the parent frame by the same
    /// rule as a file, with view = the reel's raster.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raster: Option<(u32, u32)>,
    /// The reel's OWN fps: quantization of its continuous time to its own grid
    /// (a frame is held, no blending). `None` — the project. Does not affect sound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<u32>,
}

impl Sequence {
    pub fn new(id: MediaId, name: impl Into<String>) -> Sequence {
        Sequence {
            id,
            name: name.into(),
            clips: Vec::new(),
            transitions: Vec::new(),
            morphers: Vec::new(),
            cameras: Vec::new(),
            // ids are assigned by `Project::assign_track_ids`: a reel has no access to
            // the document's id space
            tracks: vec![
                Track::new(TrackId(0), LaneKind::Video),
                Track::new(TrackId(0), LaneKind::Audio),
            ],
            legacy_video_lanes: 0,
            legacy_audio_lanes: 0,
            legacy_morph_lanes: 0,
            legacy_camera_lanes: 0,
            legacy_lane_order: Vec::new(),
            overlap_layers: true,
            published: Vec::new(),
            raster: None,
            fps: None,
        }
    }

    pub fn lanes(&self, kind: LaneKind) -> u32 {
        self.tracks.iter().filter(|t| t.kind == kind).count() as u32
    }

    pub fn track_at(&self, kind: LaneKind, lane: u32) -> Option<&Track> {
        track_index(&self.tracks, kind, lane).map(|i| &self.tracks[i])
    }

    pub fn track_at_mut(&mut self, kind: LaneKind, lane: u32) -> Option<&mut Track> {
        track_index(&self.tracks, kind, lane).map(move |i| &mut self.tracks[i])
    }

    pub fn track_label(&self, kind: LaneKind, lane: u32) -> String {
        self.track_at(kind, lane)
            .map(|t| t.label(lane))
            .unwrap_or_else(|| default_lane_label(kind, lane))
    }

    /// Append tracks of the kind up to `n` items (at the end of the rows).
    pub fn ensure_tracks(&mut self, kind: LaneKind, n: u32) {
        while self.lanes(kind) < n {
            let at = natural_insert_pos(&self.tracks, kind);
            self.tracks.insert(at, Track::new(TrackId(0), kind));
        }
    }

    /// Legacy counters and reel order → track vector (one-way).
    fn legacy(&self) -> LegacyLanes {
        LegacyLanes {
            video_lanes: self.legacy_video_lanes,
            audio_lanes: self.legacy_audio_lanes,
            morph_lanes: self.legacy_morph_lanes,
            camera_lanes: self.legacy_camera_lanes,
            lane_order: self.legacy_lane_order.clone(),
        }
    }

    fn clear_legacy(&mut self) {
        self.legacy_video_lanes = 0;
        self.legacy_audio_lanes = 0;
        self.legacy_morph_lanes = 0;
        self.legacy_camera_lanes = 0;
        self.legacy_lane_order = Vec::new();
    }

    /// How many tracks of each kind the content REQUIRES.
    fn needed(&self) -> LaneBases {
        let mut n = LaneBases::default();
        for c in &self.clips {
            let k = lane_key(c).0;
            *n.get_mut(k) = n.get(k).max(c.lane + 1);
        }
        for m in &self.morphers {
            n.morph = n.morph.max(m.lane + 1);
        }
        for c in &self.cameras {
            n.camera = n.camera.max(c.lane + 1);
        }
        n
    }

    /// Are its own settings legal? Raster — even, within 16…8192; fps —
    /// a divisor of 90 000 within 1…120 (the same check as for the project).
    pub fn settings_ok(&self) -> bool {
        let raster_ok = self.raster.is_none_or(|(w, h)| {
            [w, h].iter().all(|v| (16..=8192).contains(v) && v.is_multiple_of(2))
        });
        let fps_ok =
            self.fps.is_none_or(|f| (1..=120).contains(&f) && kt_time::TICK_HZ % f as i64 == 0);
        raster_ok && fps_ok
    }

    /// Reel rows top to bottom — the same rule as for the root.
    pub fn rows(&self) -> Vec<(LaneKind, u32)> {
        rows_of_tracks(&self.tracks)
    }

    /// End of the reel's content.
    pub fn end(&self) -> Tick {
        self.clips.iter().map(|c| c.start + c.duration).max().unwrap_or(Tick::ZERO)
    }

    /// Reel tracks from legacy fields (counters, `lane_order`) — on READ,
    /// one-way; then append up to the content. Names of service rows
    /// were not stored by the old format.
    pub fn canonicalize_lanes(&mut self) -> LanePerm {
        let need = self.needed();
        let perm = if self.tracks.is_empty() {
            let legacy = self.legacy();
            self.tracks = tracks_from_legacy(&legacy, need, &[], &[]);
            self.clear_legacy();
            self.reorder_rows(&legacy.lane_order)
        } else {
            self.clear_legacy();
            LanePerm::identity(LaneBases::of_seq(self))
        };
        self.ensure_tracks(LaneKind::Video, need.video.max(1));
        self.ensure_tracks(LaneKind::Audio, need.audio.max(1));
        self.ensure_tracks(LaneKind::Morph, need.morph);
        self.ensure_tracks(LaneKind::Camera, need.camera);
        perm
    }

    /// The only door for reordering reel rows — see `Project::set_lane_order`.
    pub fn reorder_rows(&mut self, order: &[(LaneKind, u32)]) -> LanePerm {
        let n = LaneBases::of_seq(self);
        let norm = normalize_lane_order(order, n.video, n.audio, n.morph, n.camera);
        let perm = LanePerm::from_rows(&norm, n);
        let old = std::mem::take(&mut self.tracks);
        self.tracks = norm
            .iter()
            .filter_map(|&(k, l)| track_index(&old, k, l).map(|i| old[i].clone()))
            .collect();
        if !perm.is_identity() {
            for c in &mut self.clips {
                c.lane = perm.map_clip(c.kind, c.lane);
            }
            for m in &mut self.morphers {
                m.lane = perm.map(LaneKind::Morph, m.lane);
            }
            for c in &mut self.cameras {
                c.lane = perm.map(LaneKind::Camera, c.lane);
            }
        }
        perm
    }
}

impl Reel for Project {
    fn clips(&self) -> &[ClipEvent] {
        &self.clips
    }
    fn rows(&self) -> Vec<(LaneKind, u32)> {
        Project::rows(self)
    }
    fn tracks(&self) -> &[Track] {
        &self.tracks
    }
    fn transitions(&self) -> &[TransitionEvent] {
        &self.transitions
    }
    fn morphers(&self) -> &[MorpherEvent] {
        &self.morphers
    }
    fn cameras(&self) -> &[CameraEvent] {
        &self.cameras
    }
    fn foreign(&self) -> &[ForeignEvent] {
        &self.foreign
    }
    fn overlaps_allowed(&self) -> bool {
        self.overlap_layers
    }
    fn published(&self) -> &[PublishedParam] {
        &self.published
    }
}

impl Reel for Sequence {
    fn clips(&self) -> &[ClipEvent] {
        &self.clips
    }
    fn rows(&self) -> Vec<(LaneKind, u32)> {
        Sequence::rows(self)
    }
    fn tracks(&self) -> &[Track] {
        &self.tracks
    }
    fn transitions(&self) -> &[TransitionEvent] {
        &self.transitions
    }
    fn morphers(&self) -> &[MorpherEvent] {
        &self.morphers
    }
    fn cameras(&self) -> &[CameraEvent] {
        &self.cameras
    }
    fn overlaps_allowed(&self) -> bool {
        self.overlap_layers
    }
    fn published(&self) -> &[PublishedParam] {
        &self.published
    }
}

/// What `ClipEvent.media` points to — the ONLY answer to the question
/// “file or reel” (R.50, item 378). Introduced after the pattern of `lane_key`:
/// a mapping written out in six places sooner or later drifts apart.
pub enum Source<'a> {
    File(&'a MediaRef),
    Seq(&'a Sequence),
}

/// A foreign OTIO construct that we do not render (unknown schema,
/// nested Stack, disabled clip) — a grey placeholder (R.24 item 5).
/// It takes a place on the track (takes part in the overlap check); it can be deleted
/// (undo works), but cannot be moved: the raw data is written back untouched.
/// `key` — the key of the raw JSON in Retained (kt-otio).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForeignEvent {
    pub key: u64,
    pub kind: ClipKind,
    pub lane: u32,
    pub start: Tick,
    pub duration: Tick,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub format_version: u32,
    pub settings: ProjectSettings,
    pub media: Vec<MediaRef>,
    pub clips: Vec<ClipEvent>,
    /// Markers (sorted by time, no duplicates). A marker may have a
    /// name (item 1 of round 10); old projects with an array of numbers are read as they are.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<Marker>,
    /// The selected timeline region (outside the undo history, as in the old editor).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
    /// Loop over the region (outside undo).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub loop_enabled: bool,
    /// TRACKS — objects top to bottom (R.62, change B): the vector order =
    /// row order; the event number = rank among the tracks of its kind.
    /// Empty tracks are full-fledged. On load they are appended upward up to
    /// max(lane of the content)+1 and not fewer than one video and one audio.
    /// Counters and `lane_order` of earlier rounds are `legacy_*`, read-only
    /// access.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<Track>,
    #[serde(default, rename = "video_lanes", skip_serializing)]
    pub legacy_video_lanes: u32,
    #[serde(default, rename = "audio_lanes", skip_serializing)]
    pub legacy_audio_lanes: u32,
    #[serde(default, rename = "morph_lanes", skip_serializing)]
    pub legacy_morph_lanes: u32,
    #[serde(default, rename = "lane_order", skip_serializing)]
    pub legacy_lane_order: Vec<(LaneKind, u32)>,
    /// Morph events. Applied to clips in `effective_clips()`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub morphers: Vec<MorpherEvent>,
    /// Transitions at clip joins (R.24: a crossfade is an explicit object, not an
    /// overlap).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<TransitionEvent>,
    /// Placeholders for foreign OTIO constructs (they appear only from import).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub foreign: Vec<ForeignEvent>,
    /// Nested reels (R.50, item 378). Empty — the project is flat, and ALL
    /// fast paths (validate, effective_clips, the format writer)
    /// remain exactly as before: GOLDEN and byte-level stability
    /// of files without nesting rest on this.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sequences: Vec<Sequence>,
    /// Editor viewport state (item 13, v1 port: timelineViewStartMs/
    /// zoomLevel/cursorMs). Outside the undo history (like region); old files
    /// without the section are read as None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiState>,
    /// Layering clips on one track (R.28, the user's decision:
    /// “a project option that allows several events of one
    /// track to be layered at the same time… turn it on by default”). When on, overlaps
    /// are legal and sound as in v1 (equal-power for two, 1/N for 3+); dragging over
    /// cuts NOTHING. When off — the former R.24 invariant: clips
    /// of one (kind, lane) do not overlap.
    #[serde(default = "default_true")]
    pub overlap_layers: bool,
    /// A trim STRETCHES the animation together with the event (R.40, the user's
    /// decision: “Well, it could be made NOT the default behaviour
    /// yes”). OFF by default, and that is the main point: the keyframe stays on its own
    /// frame of the material, wherever the event edge travels — exactly what
    /// the user asked for (“how to make it NOT slide
    /// anywhere, even if I move the left edge left or right”).
    ///
    /// The enabled option is needed for another way of working — “animation from
    /// the start of the event”: then the keyframe's share within the event is preserved, and
    /// the material frame under it changes.
    #[serde(default, skip_serializing_if = "is_false")]
    pub trim_stretches_auto: bool,
    /// Editing length of the timeline when it is LONGER than the material: foreign files
    /// set it with a trailing Gap after the last clip (the user's decision
    /// of R.25 — “keep the length, render to the end”, otherwise
    /// saving silently shortened someone else's edit). None — length by clips.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeline_end: Option<Tick>,
    #[serde(default, rename = "camera_lanes", skip_serializing)]
    pub legacy_camera_lanes: u32,
    /// Camera events. The active one at time t is chosen by `active_camera_at`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<CameraEvent>,
    /// Named areas “time × tracks” (R.35 item 2): a selection frame
    /// saved under a name. The morpher refers to them BY NAME.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub areas: Vec<MarkedArea>,
    /// PUBLIC parameters of the REEL whose content is currently standing in
    /// the root's place (R.59, stage 3).
    ///
    /// The field is here and on `Sequence` alike, for the same reason as
    /// why `morphers`, `cameras` and `lane_order` live here: entering
    /// a reel is done by SWAPPING its content (`swap_reel`), and the command
    /// edits the reel you are standing in, without knowing about nesting at all.
    /// The real root's set is always empty: the root is an instance of nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub published: Vec<PublishedParam>,
}

fn default_true() -> bool {
    true
}

fn default_scan_step() -> u32 {
    4
}

fn default_glow_radius() -> u32 {
    4
}

fn is_four_u32(v: &u32) -> bool {
    *v == 4
}

fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

fn is_one_u32_v(v: &u32) -> bool {
    *v == 1
}

fn is_255(v: &u8) -> bool {
    *v == 255
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

/// Morpher (R.35): an opaque “patch” on the purple morph track —
/// a LAYER, not a mark. It acts where its block covers the target
/// (`timed`), and by default — on everything BELOW its row.
///
/// Before R.35 it was a “patch on a frozen list of targets, entirely and outside
/// time”: `start`/`duration` only positioned the block. Files of that
/// era are read with `timed = false` and rendered exactly as
/// before (`migrate_legacy_morpher`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MorpherEvent {
    pub id: MorphId,
    /// Index of the morph track (0..morph_lanes).
    pub lane: u32,
    pub start: Tick,
    pub duration: Tick,
    /// What it covers. Default — “everything below the morpher row”.
    #[serde(default, skip_serializing_if = "MorphTargets::is_default")]
    pub targets: MorphTargets,
    /// What it does: replaces the source AND the effect chain — one event
    /// carries both (before R.35 these were mutually exclusive `MorphKind`).
    /// `alias = "kind"` — input of old files.
    #[serde(
        rename = "patch",
        alias = "kind",
        default,
        skip_serializing_if = "MorphPatch::is_empty"
    )]
    pub patch: MorphPatch,
    /// Switch for the whole event (R.33). A disabled morpher does not
    /// apply to targets, but lives on the track and is switched on as it was.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// The action is limited to the block's WINDOW (default, R.35 item 1: “if we play them —
    /// then we morph them too”). `false` — the former semantics:
    /// the patch applies to the targets entirely, the block's time affects nothing.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub timed: bool,
    /// Layer blend strength 0..1 (R.38 item 12: “the morpher has no way to
    /// apply fade and opacity”). 1.0 — the layer acts fully, and this is
    /// the former path BYTE-FOR-BYTE: the blend slot does not appear at all.
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub strength: f64,
    /// Ramp-in and ramp-out of the strength at the block edges. Zero — hard edges.
    #[serde(default, skip_serializing_if = "tick_is_zero")]
    pub fade_in: Tick,
    #[serde(default, skip_serializing_if = "tick_is_zero")]
    pub fade_out: Tick,
}

fn is_true(v: &bool) -> bool {
    *v
}

/// What the morpher covers (R.35). Space and time are separated:
/// `Below`/`Clips` choose the SPACE (the window is taken from the block),
/// `Marks`/`Areas` — named ranges and areas (R.35 item 2).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(tag = "on", rename_all = "snake_case")]
pub enum MorphTargets {
    /// Everything BELOW the morpher row in track order (adjustment layer).
    #[default]
    Below,
    /// A list frozen at creation — the former behaviour.
    Clips { ids: Vec<ClipId> },
    /// Named range markers: the windows are taken from them.
    Marks { names: Vec<String> },
    /// Named areas (time × tracks).
    Areas { names: Vec<String> },
}

impl MorphTargets {
    pub fn is_default(&self) -> bool {
        matches!(self, MorphTargets::Below)
    }

    /// Frozen ids — empty for all modes except `Clips`.
    pub fn clip_ids(&self) -> &[ClipId] {
        match self {
            MorphTargets::Clips { ids } => ids,
            _ => &[],
        }
    }

    /// Frozen ids for editing (RemoveClip/Split keep them alive).
    pub fn clip_ids_mut(&mut self) -> Option<&mut Vec<ClipId>> {
        match self {
            MorphTargets::Clips { ids } => Some(ids),
            _ => None,
        }
    }
}

/// A manual `Deserialize`: files before R.35 carry a BARE array of ids
/// (`"targets": [1, 2]`) — it is read as `Clips`. Precedent — `Marker`.
impl<'de> Deserialize<'de> for MorphTargets {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "on", rename_all = "snake_case")]
        enum Tagged {
            Below,
            Clips { ids: Vec<ClipId> },
            Marks { names: Vec<String> },
            Areas { names: Vec<String> },
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Legacy(Vec<ClipId>),
            Tagged(Tagged),
        }
        Ok(match Repr::deserialize(d)? {
            Repr::Legacy(ids) => MorphTargets::Clips { ids },
            Repr::Tagged(Tagged::Below) => MorphTargets::Below,
            Repr::Tagged(Tagged::Clips { ids }) => MorphTargets::Clips { ids },
            Repr::Tagged(Tagged::Marks { names }) => MorphTargets::Marks { names },
            Repr::Tagged(Tagged::Areas { names }) => MorphTargets::Areas { names },
        })
    }
}

/// What the morpher does to a target. All fields — “do nothing” by
/// default, so an empty patch is not serialized at all.
///
/// Source replacement and the effect chain live TOGETHER (R.35 item 3: “in the ui
/// of the morpher, merge them”) — in the interface this is one fx strip, where
/// the replacement is drawn as the first purple slot.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct MorphPatch {
    /// `None` = do not change the source (only shift the entry point).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<MediaId>,
    /// Shift of source_in (ticks); applied if `absolute` is not set.
    #[serde(default, skip_serializing_if = "tick_is_zero")]
    pub source_in_delta: Tick,
    /// Absolute source_in — takes priority over delta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absolute_source_in: Option<Tick>,
    /// Switch of the morph slot: the replacement settings are intact, but it does not
    /// act (the checkbox of the purple slot).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub src_off: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub video_fx: Vec<FxInstance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio_fx: Vec<FxInstance>,
    /// Slot switches, parallel to their chains.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub video_bypass: Vec<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio_bypass: Vec<bool>,
    /// Parameter curves of the layer (R.38 item 13: “for the morpher I cannot set
    /// effect automation”). Parallel to their chains, as are the
    /// switches.
    ///
    /// **Domain — the morpher BLOCK's time, 0..duration.** A morpher has no
    /// material: it is a layer of edits, not an event with a source. The same
    /// domain as the camera and the transition, and the same rule that R.37 set
    /// for events: “the time span cut out by this element”.
    /// In `apply_to` the keys are recalculated into the TARGET's material.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub video_auto: Vec<Vec<ParamCurve>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio_auto: Vec<Vec<ParamCurve>>,
}

/// Manual `Deserialize`: accepts three forms — the flat one (R.35) and both
/// old ones (`{"Replace": …}` / `{"EffectBatch": …}`), because before R.35
/// it was an externally tagged enum `MorphKind`.
impl<'de> Deserialize<'de> for MorphPatch {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        enum Legacy {
            Replace {
                #[serde(default)]
                media: Option<MediaId>,
                #[serde(default)]
                source_in_delta: Tick,
                #[serde(default)]
                absolute_source_in: Option<Tick>,
            },
            EffectBatch {
                #[serde(default)]
                video_fx: Vec<FxInstance>,
                #[serde(default)]
                audio_fx: Vec<FxInstance>,
                #[serde(default)]
                video_bypass: Vec<bool>,
                #[serde(default)]
                audio_bypass: Vec<bool>,
            },
        }
        #[derive(Deserialize)]
        struct Flat {
            #[serde(default)]
            media: Option<MediaId>,
            #[serde(default)]
            source_in_delta: Tick,
            #[serde(default)]
            absolute_source_in: Option<Tick>,
            #[serde(default)]
            src_off: bool,
            #[serde(default)]
            video_fx: Vec<FxInstance>,
            #[serde(default)]
            audio_fx: Vec<FxInstance>,
            #[serde(default)]
            video_bypass: Vec<bool>,
            #[serde(default)]
            audio_bypass: Vec<bool>,
            #[serde(default)]
            video_auto: Vec<Vec<ParamCurve>>,
            #[serde(default)]
            audio_auto: Vec<Vec<ParamCurve>>,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Legacy(Legacy),
            Flat(Flat),
        }
        Ok(match Repr::deserialize(d)? {
            Repr::Legacy(Legacy::Replace { media, source_in_delta, absolute_source_in }) => {
                MorphPatch { media, source_in_delta, absolute_source_in, ..Default::default() }
            }
            Repr::Legacy(Legacy::EffectBatch {
                video_fx,
                audio_fx,
                video_bypass,
                audio_bypass,
            }) => {
                MorphPatch { video_fx, audio_fx, video_bypass, audio_bypass, ..Default::default() }
            }
            Repr::Flat(f) => MorphPatch {
                media: f.media,
                source_in_delta: f.source_in_delta,
                absolute_source_in: f.absolute_source_in,
                src_off: f.src_off,
                video_fx: f.video_fx,
                audio_fx: f.audio_fx,
                video_bypass: f.video_bypass,
                audio_bypass: f.audio_bypass,
                video_auto: f.video_auto,
                audio_auto: f.audio_auto,
            },
        })
    }
}

impl MorphPatch {
    /// An empty patch does nothing — such a one is not serialized.
    pub fn is_empty(&self) -> bool {
        self.media.is_none()
            && self.source_in_delta.0 == 0
            && self.absolute_source_in.is_none()
            && self.video_fx.is_empty()
            && self.audio_fx.is_empty()
            // curves without slots are impossible, but an “empty” patch is NOT
            // serialized — they cannot be silently lost (R.38)
            && self.video_auto.iter().all(|a| a.is_empty())
            && self.audio_auto.iter().all(|a| a.is_empty())
    }

    /// Does the patch change THIS clip. The answer “no” is a reason not to cut it
    /// at all: a layer with video effects alone must not touch the audio under
    /// it (let alone leave a seam on it).
    pub fn affects(&self, c: &ClipEvent) -> bool {
        self.replaces_source() || !self.chain_for(c.kind).is_empty()
    }

    /// Does the patch chain pull TIME for a clip of this kind.
    ///
    /// It is asked where it is decided whether to CUT the target: a warp cannot be “blended
    /// back” by a strength slot — it changes not a value but how much material
    /// is consumed, and acts on the whole clip, no matter how you dim it.
    pub fn warps_time_for(&self, kind: ClipKind) -> bool {
        self.chain_for(kind).iter().any(|f| f.warps_time())
    }

    /// Does the patch replace the source (taking the morph-slot switch into account).
    pub fn replaces_source(&self) -> bool {
        !self.src_off
            && (self.media.is_some()
                || self.absolute_source_in.is_some()
                || self.source_in_delta.0 != 0)
    }

    /// ENABLED effects of the patch for a clip of this kind. A disabled slot
    /// is simply not appended: the clip has its own bypass numbering, and
    /// they must not be mixed.
    pub fn chain_for(&self, kind: ClipKind) -> Vec<FxInstance> {
        let (fx, off) = match kind {
            ClipKind::Video => (&self.video_fx, &self.video_bypass),
            ClipKind::Audio => (&self.audio_fx, &self.audio_bypass),
        };
        let mut out: Vec<FxInstance> = fx
            .iter()
            .enumerate()
            .filter(|(i, _)| !off.get(*i).copied().unwrap_or(false))
            .map(|(_, f)| f.clone())
            .collect();
        // The TIME WARP goes to BOTH kinds (R.37 item 2). It lives in
        // `audio_fx` — its canonical home there, `merge_chains`/
        // `split_chains` in the UI rest on this — but it is not a pixel filter,
        // but a rule for choosing the source frame. Without this the layer would slow
        // the sound and not the picture: a desync that nobody asked for.
        if kind == ClipKind::Video {
            out.extend(
                self.audio_fx
                    .iter()
                    .enumerate()
                    .filter(|(i, f)| {
                        f.warps_time() && !self.audio_bypass.get(*i).copied().unwrap_or(false)
                    })
                    .map(|(_, f)| f.clone()),
            );
        } else {
            // The LAYER STRENGTH goes to the sound too (R.38 item 12, the decision “video and
            // audio at once”). It lives in `video_fx` — its canonical
            // home there — but it must attenuate both kinds.
            out.extend(
                self.video_fx
                    .iter()
                    .enumerate()
                    .filter(|(i, f)| {
                        f.is_layer_mix() && !self.video_bypass.get(*i).copied().unwrap_or(false)
                    })
                    .map(|(_, f)| f.clone()),
            );
        }
        out
    }

    /// The curves in the same order in which `chain_for` hands out the slots:
    /// disabled ones skipped, for video at the tail — the warp slots from audio.
    /// If the order diverged, the curve would shift onto another effect.
    pub fn auto_for(&self, kind: ClipKind) -> Vec<Vec<ParamCurve>> {
        let (fx, off, auto) = match kind {
            ClipKind::Video => (&self.video_fx, &self.video_bypass, &self.video_auto),
            ClipKind::Audio => (&self.audio_fx, &self.audio_bypass, &self.audio_auto),
        };
        let pick = |i: usize| auto.get(i).cloned().unwrap_or_default();
        let mut out: Vec<Vec<ParamCurve>> = fx
            .iter()
            .enumerate()
            .filter(|(i, _)| !off.get(*i).copied().unwrap_or(false))
            .map(|(i, _)| pick(i))
            .collect();
        if kind == ClipKind::Video {
            out.extend(
                self.audio_fx
                    .iter()
                    .enumerate()
                    .filter(|(i, f)| {
                        f.warps_time() && !self.audio_bypass.get(*i).copied().unwrap_or(false)
                    })
                    .map(|(i, _)| self.audio_auto.get(i).cloned().unwrap_or_default()),
            );
        } else {
            out.extend(
                self.video_fx
                    .iter()
                    .enumerate()
                    .filter(|(i, f)| {
                        f.is_layer_mix() && !self.video_bypass.get(*i).copied().unwrap_or(false)
                    })
                    .map(|(i, _)| self.video_auto.get(i).cloned().unwrap_or_default()),
            );
        }
        out
    }

    /// All media held by the patch (guard `RemoveMedia`).
    pub fn media_refs(&self) -> impl Iterator<Item = MediaId> + '_ {
        self.media.into_iter()
    }

    /// Apply the patch to a copy of the clip (v1 applyMorpherPatchToClipCopy).
    /// There is deliberately no upper clamp of source_in by the source duration:
    /// the model does not know media durations (no FFmpeg in kt-model);
    /// reading past the source end, the engine returns silence / the last frame.
    /// `anchor` — the start of the morpher block on the timeline, `frame_step` — the step
    /// of a project frame: by them the layer's keys are converted from BLOCK time into
    /// the target's material by the same mapping as everything else
    /// (`clip_material_at` accounts for reverse).
    pub fn apply_to(&self, c: &mut ClipEvent, anchor: Tick, frame_step: i64) {
        if self.replaces_source() {
            if let Some(m) = self.media {
                // A source replacement changes the MEDIA, while generator values are
                // a property of the event on a SPECIFIC medium (R.59, stage 2).
                // The patch does not carry them and cannot invent them, so the target
                // stays without them: the source address degenerates to `media`,
                // and the frame is taken from the shared source of the kind — the same one that
                // the bin draws. Had we invented values, the layer would draw
                // “someone else's title”, and that is silent corruption.
                if c.media != m {
                    c.r#gen = None;
                    c.gen_auto.clear();
                }
                c.media = m;
            }
            let was = c.source_in;
            c.source_in = match self.absolute_source_in {
                Some(abs) => Tick(abs.0.max(0)),
                None => Tick((c.source_in + self.source_in_delta).0.max(0)),
            };
            // THE ANCHOR AND THE KEYS MOVE BY ONE DELTA (R.43, item 300) — the same
            // rule as for `clip_slice`.
            //
            // A keyframe is glued to the material (t212), and this holds while the material
            // is the SAME: trimming hides and reveals keys, a cut splits the curve.
            // A source replacement changes the coordinate system itself, and the curve
            // left in the old one slides out of the event window entirely: it
            // is clamped by the last key and looks like “the effect stopped
            // applying”. For the user `source_in` moved from 47.0 s to
            // 99.0 s with keys at 47.0..59.6 s — pan3d became
            // identical (rot_y = rot_z = 0).
            let shift = c.source_in - was;
            if shift.0 != 0 {
                for slot in &mut c.fx_auto {
                    for cur in slot {
                        for k in &mut cur.keys {
                            k.t = k.t + shift;
                        }
                    }
                }
            }
        }
        let add = self.chain_for(c.kind);
        if add.is_empty() {
            return;
        }
        // The master switch turns off the EVENT's chain, not the layer above it
        // (R.35): its own chain slides out entirely, the layer's effects apply.
        // Otherwise a disabled event chain would silently kill a layer
        // that has no relation to it at all.
        if c.fx_off {
            c.fx.clear();
            c.fx_bypass.clear();
            c.fx_auto.clear();
            c.fx_off = false;
        }
        // fx_bypass shorter than the chain = the appended slots are enabled
        // (active_fx treats a missing flag as false)
        let auto = self.auto_for(c.kind);
        c.fx.extend(add);
        // layer curves (R.38 item 13) — in the target's MATERIAL domain: the layer has its own
        // time (0..duration of the block), the event has the material, and the engine
        // samples exactly the material
        if auto.iter().any(|a| !a.is_empty()) {
            c.fx_auto.resize(c.fx.len(), Vec::new());
            let base = c.fx.len() - auto.len();
            for (j, curves) in auto.into_iter().enumerate() {
                c.fx_auto[base + j] = curves
                    .into_iter()
                    .map(|mut cur| {
                        for k in &mut cur.keys {
                            k.t = crate::clip_material_at(
                                c.source_in,
                                c.start,
                                c.duration,
                                c.reversed,
                                frame_step,
                                Tick(anchor.0 + k.t.0),
                            );
                        }
                        cur.keys.sort_by_key(|k| k.t.0);
                        cur
                    })
                    .collect();
            }
        }
    }
}

impl MorpherEvent {
    /// Is a blend slot needed: the strength is below one or there are fades.
    /// At the defaults (1.0 and zeros) there is NO slot — the path is byte-for-byte the former one, and
    /// GOLDEN_VIDEO and GOLDEN_AUDIO rest on this.
    pub fn mixes(&self) -> bool {
        self.strength < 1.0 || self.fade_in.0 > 0 || self.fade_out.0 > 0
    }

    /// Is a blend slot needed TAKING THE GATE INTO ACCOUNT (R.55, C2).
    ///
    /// `gate` — the windows in which the layer ACTS, in timeline coordinates,
    /// already intersected with the target's body. If the gate covers the body entirely,
    /// the answer is the former one (`mixes`) and the path stays byte-for-byte: at the defaults
    /// there is no slot, and GOLDEN_VIDEO and GOLDEN_AUDIO rest on this.
    pub fn mixes_on(&self, gate: &[TickRange], body: TickRange) -> bool {
        self.mixes() || !covers_whole(gate, body)
    }

    /// Strength curve in the BLOCK domain with GATES (R.55, C2).
    ///
    /// The gate is the “by time” for a layer that does NOT CUT the target:
    /// outside the window the strength is zero, inside it is the ordinary strength with fades. The step
    /// is built from two keys on ADJACENT ticks: on one tick
    /// `dedup_by_key` would collapse them into one, and instead of a step we would get
    /// a ramp over the whole body.
    ///
    /// A key beyond the edge of the body is not placed: `clip_material_at` clamps the edges, and
    /// two keys would collapse into one tick with DIFFERENT values. Where
    /// the gate runs up against the edge of the body there is nothing to fade — the layer acts
    /// right from the edge.
    pub fn mix_curve_on(&self, gate: &[TickRange], body: TickRange) -> Option<ParamCurve> {
        if covers_whole(gate, body) {
            return self.mix_curve();
        }
        let d = self.duration.0.max(1);
        let fi = self.fade_in.0.clamp(0, d);
        let fo = self.fade_out.0.clamp(0, d - fi);
        let s = self.strength;
        // strength at a TIMELINE point from the block's fades (without gates)
        let fade_at = |t: i64| -> f64 {
            let x = t - self.start.0;
            if fi > 0 && x < fi {
                s * (x.clamp(0, fi) as f64 / fi as f64)
            } else if fo > 0 && x > d - fo {
                s * ((d - x).clamp(0, fo) as f64 / fo as f64)
            } else {
                s
            }
        };
        let mut keys: Vec<AutoKey> = Vec::with_capacity(gate.len() * 4 + 2);
        let mut push = |t: i64, v: f64| keys.push(AutoKey::at(Tick(t - self.start.0), v));
        for w in gate {
            if w.start > body.start {
                push(w.start.0 - 1, 0.0);
            }
            push(w.start.0, fade_at(w.start.0));
            // fade breakpoints falling INSIDE the window
            for brk in [self.start.0 + fi, self.start.0 + d - fo] {
                if brk > w.start.0 && brk < w.end.0 {
                    push(brk, fade_at(brk));
                }
            }
            push(w.end.0, fade_at(w.end.0));
            if w.end < body.end {
                push(w.end.0 + 1, 0.0);
            }
        }
        keys.sort_by_key(|k| k.t.0);
        keys.dedup_by_key(|k| k.t.0);
        (!keys.is_empty()).then(|| ParamCurve {
            param: "amount".into(),
            keys: AutoKey::linear(keys),
            source: Default::default(),
        })
    }

    /// Strength curve in the BLOCK domain: 0 at the very edge, `strength` on the plateau.
    /// `None` — there are no fades; the base slot value is enough.
    pub fn mix_curve(&self) -> Option<ParamCurve> {
        if self.fade_in.0 <= 0 && self.fade_out.0 <= 0 {
            return None;
        }
        let d = self.duration.0.max(1);
        // fades do not overlap each other: the sum is clamped to the block's length
        let fi = self.fade_in.0.clamp(0, d);
        let fo = self.fade_out.0.clamp(0, d - fi);
        let s = self.strength;
        let mut keys = Vec::with_capacity(4);
        if fi > 0 {
            keys.push(AutoKey::at(Tick(0), 0.0));
        }
        keys.push(AutoKey::at(Tick(fi), s));
        keys.push(AutoKey::at(Tick(d - fo), s));
        if fo > 0 {
            keys.push(AutoKey::at(Tick(d), 0.0));
        }
        keys.dedup_by_key(|k| k.t.0);
        Some(ParamCurve {
            param: "amount".into(),
            keys: AutoKey::linear(keys),
            source: Default::default(),
        })
    }

    /// Apply the layer to a piece of the target: the patch plus the STRENGTH slot (R.38 item 12).
    ///
    /// Strength is the last slot of the chain: it mixes the chain's INPUT with its
    /// result, so it must come after everything the layer does.
    pub fn apply_to(&self, c: &mut ClipEvent, frame_step: i64, gate: &[TickRange]) {
        let body = TickRange::new(c.start, c.start + c.duration);
        self.patch.apply_to(c, self.start, frame_step);
        if !self.mixes_on(gate, body) {
            return;
        }
        // from which slot the contribution of THIS layer exactly starts
        let from = c.fx.len().saturating_sub(self.patch.chain_for(c.kind).len()) as u32;
        let mut mix = FxInstance::LayerMix { amount: self.strength, from };
        mix.set_param("amount", self.strength);
        let curve = self.mix_curve_on(gate, body).map(|mut cur| {
            for k in &mut cur.keys {
                k.t = crate::clip_material_at(
                    c.source_in,
                    c.start,
                    c.duration,
                    c.reversed,
                    frame_step,
                    Tick(self.start.0 + k.t.0),
                );
            }
            cur.keys.sort_by_key(|k| k.t.0);
            cur
        });
        c.fx.push(mix);
        c.fx_auto.resize(c.fx.len(), Vec::new());
        if let Some(cur) = curve {
            let last = c.fx.len() - 1;
            c.fx_auto[last] = vec![cur];
        }
    }

    /// All media held by the morpher.
    pub fn media_refs(&self) -> impl Iterator<Item = MediaId> + '_ {
        self.patch.media_refs()
    }

    /// The block's window on the timeline.
    pub fn window(&self) -> TickRange {
        TickRange::new(self.start, self.start + self.duration)
    }

    /// Is this clip a target. `Below` is the default: everything BELOW the row
    /// of the morpher in track order (adjustment layer); `Marks`/`Areas` —
    /// named ranges and areas (R.35 item 2), which also define the WINDOWS.
    pub fn covers(&self, c: &ClipEvent, rows: &[(LaneKind, u32)]) -> bool {
        match &self.targets {
            MorphTargets::Clips { ids } => ids.contains(&c.id),
            MorphTargets::Below => {
                let mine = rows.iter().position(|&k| k == (LaneKind::Morph, self.lane));
                let his = rows.iter().position(|&k| k == lane_key(c));
                matches!((mine, his), (Some(a), Some(b)) if b > a)
            }
            // the reference is resolved by `ref_windows`: a dead reference = an empty
            // set, not an error (the file does not lose it)
            MorphTargets::Marks { .. } | MorphTargets::Areas { .. } => true,
        }
    }

    /// Windows of named references, bounded by the clip's body. Empty — the mode
    /// is not reference-based or the references lead nowhere.
    pub fn ref_windows(&self, c: &ClipEvent, p: &Project) -> Vec<TickRange> {
        let body = TickRange::new(c.start, c.start + c.duration);
        let key = lane_key(c);
        let ranges: Vec<TickRange> = match &self.targets {
            MorphTargets::Marks { names } => {
                names.iter().filter_map(|n| p.named_range(n)).collect()
            }
            MorphTargets::Areas { names } => names
                .iter()
                .filter_map(|n| p.area(n))
                .filter(|a| a.covers_lane(key))
                .map(|a| a.range())
                .collect(),
            _ => return Vec::new(),
        };
        merge_ranges(ranges.into_iter().map(|r| r.intersect(&body)).filter(|r| !r.is_empty()))
    }

    /// Index of the morpher row — the key of the layer stacking order.
    pub fn row_in(&self, rows: &[(LaneKind, u32)]) -> usize {
        rows.iter().position(|&k| k == (LaneKind::Morph, self.lane)).unwrap_or(0)
    }

    /// Windows in which the morpher ACTUALLY changes THIS clip. Empty — the clip is not
    /// touched and (important!) not cut. The order of the checks explains all of
    /// the cost of the decision:
    ///
    /// 1. is the clip a target;
    /// 2. **does the patch change a clip of THIS kind** — a layer with video FX alone must not
    ///    leave a seam on the audio beneath it;
    /// 3. `timed = false` (files before v4) — the patch covers the whole body, as in v1;
    /// 4. a clip that must NOT be cut (a source-consumption warp —
    ///    finding 2 of R.35), is patched entirely;
    /// 5. forbidden zones (transition windows and neighbours' overlaps) push
    ///    the boundary outward: the layer “sticks” to the crossfade edge, but neighbours on
    ///    the track are not layered over by the cut (finding 1 of R.35);
    /// 6. a stub shorter than `crate::MIN_CLIP_DURATION` collapses to the body's edge.
    ///
    /// Forbidden zones are computed over the `reel` REEL, not over the root (R.50):
    /// the layer may lie inside a nested clip. `p` remains for the sake of the project
    /// settings — the nested clip inherits them, it has none of its own.
    pub fn windows_on(
        &self,
        c: &ClipEvent,
        p: &Project,
        reel: &dyn Reel,
        rows: &[(LaneKind, u32)],
    ) -> Vec<TickRange> {
        let body = TickRange::new(c.start, c.start + c.duration);
        if !self.enabled || !self.covers(c, rows) || !self.patch.affects(c) {
            return Vec::new();
        }
        // WARP OF TIME — a VETO on CUTTING (R.37): the `source_in` of a piece computed linearly
        // for a stretched clip points to the wrong place, so
        // such a clip is patched ENTIRELY. But “entirely” is about the BOUNDARIES, not about
        // REACH: the veto came FIRST and returned the whole body,
        // never asking whether the block reaches the clip IN TIME.
        //
        // Cost of the error (R.43, item 298): a morph block 0.6 s long at the 5020th
        // second patched EVERY event of the project that had
        // slowed&reverb — in the user's real project this swapped
        // the source of the audio at the 148th second (“sound from another event”),
        // and a morpher with slowed&reverb in its patch handed the warp to ALL events
        // below its row (`chain_for` hands the warp out to both kinds).
        // Therefore we first compute REACH, and only then the veto.
        // Whether the layer CUTS the target (R.55, C2). A patch of effects alone does not cut
        // at all: it lies over the WHOLE BODY, and “by time” is set by the GATES
        // of the strength. Then the boundary is exact to the tick, does not touch
        // neighbours or transitions, and no seam arises on the material at all.
        let cuts = self.cuts_target(c);
        let warps = |fx: &[FxInstance]| fx.iter().any(|f| f.warps_time());
        // The warp veto makes sense ONLY where cutting happens: a linearly
        // computed `source_in` of a piece of a stretched clip points not
        // there. Gates cut nothing, so the veto does not apply to them —
        // and “by time” starts working on stretched events too.
        let no_cut = cuts && (warps(&c.active_fx()) || warps(&self.patch.chain_for(c.kind)));
        // reference modes: the TIME is set by the reference itself, the block does not
        // limit it — otherwise the user would point at a distant range and
        // would not understand why nothing happened. The “by time” checkbox in these
        // modes is greyed out (the window already comes from the reference).
        if matches!(self.targets, MorphTargets::Marks { .. } | MorphTargets::Areas { .. }) {
            let refs = self.ref_windows(c, p);
            if refs.is_empty() {
                return Vec::new(); // the references did not reach here
            }
            if no_cut {
                return vec![body];
            }
            if !cuts {
                return refs; // gates: the reference windows as they are, to the tick
            }
            return refs.into_iter().filter_map(|w| self.fit_window(w, c, p, reel, body)).collect();
        }
        if !self.timed {
            return vec![body]; // v1: block time does not apply
        }
        let w = self.window().intersect(&body);
        if w.len() < crate::MIN_CLIP_DURATION {
            return Vec::new(); // the block did not reach here
        }
        if no_cut {
            return vec![body];
        }
        if !cuts {
            return vec![w]; // gates: the block window as it is, to the tick
        }
        match self.fit_window(w, c, p, reel, body) {
            Some(w) => vec![w],
            None => Vec::new(),
        }
    }

    /// Does the layer cut the target into pieces, or lie over the whole body with the gates
    /// of strength (R.55, C2).
    ///
    /// Exactly two kinds of patch cut, and both — because a strength slot cannot
    /// switch them off:
    ///
    /// * **source replacement** — “take the frame from another file” is not
    ///   expressed by a parameter value at all;
    /// * **time warp in the patch** — it changes not a value but how much
    ///   material is consumed, and acts on the whole clip, however it is dimmed.
    ///
    /// All the rest (and that is almost all) — the effect chain, whose contribution
    /// is damped to zero by the strength curve. So “by time” stops depending on
    /// whether the event has a transition: cutting inside a crossfade window is
    /// forbidden, but DAMPING is allowed anywhere.
    pub fn cuts_target(&self, c: &ClipEvent) -> bool {
        self.patch.replaces_source() || self.patch.warps_time_for(c.kind)
    }

    /// Snap the window to the material grid, push it out of the forbidden zones and
    /// collapse the stubs to the body's edges. `None` — no window remains.
    fn fit_window(
        &self,
        w: TickRange,
        c: &ClipEvent,
        p: &Project,
        reel: &dyn Reel,
        body: TickRange,
    ) -> Option<TickRange> {
        if w.len() < crate::MIN_CLIP_DURATION {
            return None;
        }
        // The boundary snaps to the clip's MATERIAL grid: a frame for video, 8
        // samples (15 ticks) for audio. Otherwise the entry point of the piece would fall
        // on a fractional frame: for a forward clip this would cost a sample at the
        // seam, for a reversed one — a frame (clamp of the mirrored anchor). Counting from the
        // start of the body, but for reversed from the END: there is the material anchor.
        let step = match c.kind {
            ClipKind::Video => kt_time::TICK_HZ / p.settings.fps.max(1) as i64,
            ClipKind::Audio => kt_time::TICK_HZ * 8 / kt_time::AUDIO_SAMPLE_RATE as i64,
        };
        let grid = |t: Tick| {
            if c.reversed {
                Tick(body.end.0 - (body.end.0 - t.0) / step * step)
            } else {
                Tick(body.start.0 + (t.0 - body.start.0) / step * step)
            }
        };
        // THE ORDER MATTERS: grid first, zones after. A zone edge may fall not
        // on the grid (a neighbour clip started between frames), and then the choice
        // is one: either a frame of material, or an uncut surrounding. More costly
        // is the surrounding — a piece with a larger `start` sticks out over the neighbour
        // and changes the crossfade weights, while the grid costs one frame at the seam.
        // WE PUSH ONLY INWARD (R.55, C2). Before this, the boundary moved
        // to the NEAREST edge of the zone — including OUTWARD, and for an event with
        // a fade on the left and a fade on the right the left boundary slid to the start
        // of the body and the right one to the end: “by time” turned into “on the whole
        // event”. The window now only SHRINKS; if no window is left —
        // the layer did not reach here, and that is an honest answer, not a silent
        // widening.
        let zones = forbidden_zones(reel, c);
        let mut s = snap_out_of(grid(w.start), &zones, true);
        let mut e = snap_out_of(grid(w.end), &zones, false);
        s = Tick(s.0.clamp(body.start.0, body.end.0));
        e = Tick(e.0.clamp(body.start.0, body.end.0));
        // a stub that could not be obtained by a command anyway
        // (Split forbids pieces shorter than the minimum) sticks to the body's edge
        if (s - body.start) < crate::MIN_CLIP_DURATION {
            s = body.start;
        }
        if (body.end - e) < crate::MIN_CLIP_DURATION {
            e = body.end;
        }
        (e - s >= crate::MIN_CLIP_DURATION).then_some(TickRange::new(s, e))
    }
}

/// Merge overlapping and adjacent ranges into a non-overlapping set
/// (a morpher may have several references, and they may overlap).
fn merge_ranges(it: impl Iterator<Item = TickRange>) -> Vec<TickRange> {
    let mut v: Vec<TickRange> = it.collect();
    v.sort_by_key(|r| r.start.0);
    let mut out: Vec<TickRange> = Vec::with_capacity(v.len());
    for r in v {
        match out.last_mut() {
            Some(last) if r.start.0 <= last.end.0 => {
                last.end = Tick(last.end.0.max(r.end.0));
            }
            _ => out.push(r),
        }
    }
    out
}

/// Push the point out of the forbidden zones IN THE GIVEN DIRECTION. Zones may
/// overlap and abut, so the pass repeats until convergence
/// (no more than the number of zones — each step moves the point out of the next one).
///
/// `forward` — where to move: the window start forward (to the zone's end), the window end
/// backward (to its start). The side is given, not chosen by proximity, and this is
/// essential: “to the nearest edge” widens the window where the boundary
/// fell in the zone a little beyond its middle, and the layer starts acting
/// WIDER than the person asked (R.55, C2).
fn snap_out_of(t: Tick, zones: &[TickRange], forward: bool) -> Tick {
    let mut t = t;
    for _ in 0..=zones.len() {
        let Some(z) = zones.iter().find(|z| z.start < t && t < z.end) else { return t };
        t = if forward { z.end } else { z.start };
    }
    t
}

/// Whether the gates cover the whole body (in which case they are, in a sense, absent).
fn covers_whole(gate: &[TickRange], body: TickRange) -> bool {
    gate.iter().any(|w| w.start <= body.start && body.end <= w.end)
}

/// Key of an event row: track kind and its number. The mapping was
/// duplicated four times in `timeline.rs` and twice in `convert.rs`.
pub fn lane_key_of(k: ClipKind) -> LaneKind {
    match k {
        ClipKind::Video => LaneKind::Video,
        ClipKind::Audio => LaneKind::Audio,
    }
}

pub fn lane_key(c: &ClipEvent) -> (LaneKind, u32) {
    match c.kind {
        ClipKind::Video => (LaneKind::Video, c.lane),
        ClipKind::Audio => (LaneKind::Audio, c.lane),
    }
}

/// The top bit of the id is the marker of a PIECE of a clip produced by a morpher layer.
/// Real clips do not fall into this range (checked by `validate`),
/// so the original is recovered from the id itself and side tables are not
/// needed.
///
/// **The bit was lowered from 63 to 61 in R.59 (stage 2).** The source address
/// of a generator is taken as `GEN_TAG | id` of the EVENT, and an event can also be
/// a piece of a layer: had the tag stayed at the top, `GEN_TAG | slice_id` would have spilled into
/// the `NEST_TAG` range, and a title piece would silently share the cache with a reel.
/// Below is a `const _` guard that prevents these three numbers from drifting apart.
pub const SLICE_TAG: u64 = 1 << 61;
/// This many low bits of the id are given to the piece number.
const SLICE_IDX_BITS: u32 = 16;
/// Ceiling of a real clip's id (everything above is reserved for pieces).
pub const MAX_REAL_CLIP_ID: u64 = 1 << (61 - SLICE_IDX_BITS);

/// The address space as a whole: real clip < piece < generator <
/// reel. Violating any inequality means two different objects with one
/// cache key, and it would show up as a stale picture, not as a crash.
const _: () = {
    // pieces fit under their own tag…
    assert!(MAX_REAL_CLIP_ID << SLICE_IDX_BITS <= SLICE_TAG);
    // …and the HIGHEST piece lies strictly below the generator tag
    assert!(SLICE_TAG + (MAX_REAL_CLIP_ID << SLICE_IDX_BITS) <= GEN_TAG);
    assert!(GEN_TAG < NEST_TAG);
};

/// A piece's id is a pure function of (original, number). Neither counters nor
/// hashes: the id must not move because of an edit of a neighbouring clip or a drag of
/// a morpher block with the mouse, otherwise audio FX would be reset on every sneeze
/// (invariant No. 3).
pub fn slice_id(origin: ClipId, idx: u32) -> ClipId {
    ClipId(SLICE_TAG | (origin.0 << SLICE_IDX_BITS) | idx as u64)
}

/// A piece's original — FROM the id itself. `None` — this is not a piece.
pub fn slice_origin(id: ClipId) -> Option<ClipId> {
    (id.0 & SLICE_TAG != 0).then_some(ClipId((id.0 & !SLICE_TAG) >> SLICE_IDX_BITS))
}

/// The same clip or its piece. Everything that addresses a clip by id AMONG
/// the effective clips (transition ends in the engine) must ask this.
pub fn same_clip(eff_id: ClipId, origin: ClipId) -> bool {
    eff_id == origin || slice_origin(eff_id) == Some(origin)
}

/// A piece of a clip over window `w`. The source_in rule is the SAME as for
/// `Command::Split` (which since R.35 calls this function itself): for a forward
/// clip the entry point travels with the start of the piece; for reversed — with its end,
/// because its material is mirrored (item 16).
/// The piece's pair of anchors: LINEAR and MATERIAL (R.39).
///
/// `source_in` has two jobs at once: it is the anchor of READING the MATERIAL and the start of the AUTOMATION AXIS
/// — the event time of a key is exactly `start + (k.t −
/// source_in)` (this is what `clip_timeline_at` computes). With a linear map both
/// coincide; under a warp they diverge, and the anchor must travel by the integral of
/// the speed, while the axis stays linear (R.37 decision: “WITHOUT accounting for
/// slowing-speeding-up”).
fn slice_anchors(c: &ClipEvent, w: TickRange) -> (Tick, Tick) {
    let linear = if c.reversed {
        c.source_in + ((c.start + c.duration) - w.end)
    } else {
        c.source_in + (w.start - c.start)
    };
    let Some(warp) = crate::clip_warp(c) else { return (linear, linear) };
    // WARP, THEN MIRROR (R.37 item 247) — by the same expression by which the frame
    // is chosen by the renderer: for a forward clip the anchor is at the START of the window, for a mirrored one
    // at its END, counted from the actually consumed `used`, not
    // from the length of the event.
    let tl = if c.reversed { w.end - c.start } else { w.start - c.start };
    (linear, c.source_in + Tick(warp.material_off(c.duration, c.reversed, tl.0)))
}

/// How far to the left the START of the clip can be dragged: beyond that the material ends.
///
/// For a stretched clip this is NOT `start − source_in`: widening to the left by δ
/// consumes `rate·δ` of material, so for a sped-up clip the limit is closer,
/// and for a slowed one it is further. The inverse of the map is `output_time_for_source`.
pub fn trim_min_start(c: &ClipEvent) -> Tick {
    if c.reversed {
        // for a mirrored clip the left trim does not touch source_in (item 16)
        return Tick(0);
    }
    let Some(warp) = crate::clip_warp(c) else {
        return Tick((c.start - c.source_in).0.max(0));
    };
    // how much OUTPUT time corresponds to material lying to the left of
    // the entry point: s < 0, the map extrapolates with the speed at zero
    let span = c.duration.0.max(1) as f64;
    // Speed at zero ≤ 0 (R.57): the material on the left is NOT consumed by this
    // event at all — neither backward nor on a hold. Extrapolation would divide
    // by zero and give `i64::MIN` (overflow in debug).
    if crate::eff_rate(warp.base(), warp.prof(), 0.0) <= 0.0 {
        return Tick(0);
    }
    let s = -(c.source_in.0 as f64) / span;
    let dt = crate::output_time_for_source(warp.base(), warp.prof(), s) * span;
    Tick((c.start.0 + dt.round() as i64).max(0))
}

/// Where IN THE EVENT'S TIME the material ends (R.63 item 2), if it ends at all
/// inside it. `None` — the length is unknown or the material is enough
/// for the whole event.
///
/// One function for all: the timeline strip draws the tail hatching from here,
/// `kt-cli probe` prints the moment from here too, and the trim limit is this very function
/// plus “not less than the current end”. If they diverged — the picture, the diagnostics
/// and the gesture would speak of different edges.
pub fn material_end_on_timeline(c: &ClipEvent, media_len: Option<Tick>) -> Option<i64> {
    let len = media_len.filter(|l| l.0 > 0)?;
    let left = (len.0 - c.source_in.0).max(0);
    let used = crate::clip_warp(c).map(|w| w.used()).unwrap_or(c.duration.0);
    if used <= left {
        return None; // the material is enough for the whole event
    }
    let end = trim_max_end_in(c, Some(len)).0;
    (end < (c.start + c.duration).0).then_some(end)
}

/// How far to the right the END of the clip can be dragged, KNOWING the material length
/// (R.63 item 2). `None` — the length is unknown (generator, reel, a file not yet
/// opened), and there is no limit, as there always was.
///
/// A forward event hits the material's end with its end exactly as it hits
/// the start with its start: the right edge is the same kind of edge as the left. The limit
/// is computed BY THE MATERIAL, so a slowed event hits it later
/// (at rate 0.5 a second of material is two seconds on the timeline), and
/// a sped-up one earlier — the warp map is queried by the same function as for
/// the left edge.
pub fn trim_max_end_in(c: &ClipEvent, media_len: Option<Tick>) -> Tick {
    let by_material = || -> Tick {
        let Some(len) = media_len.filter(|l| l.0 > 0) else {
            return Tick(i64::MAX / 4);
        };
        // how much material is left to the right of the entry point
        let left = (len.0 - c.source_in.0).max(0);
        let Some(warp) = crate::clip_warp(c) else {
            return Tick(c.start.0 + left);
        };
        let span = c.duration.0.max(1) as f64;
        if crate::eff_rate(warp.base(), warp.prof(), 0.0) <= 0.0 {
            return Tick(i64::MAX / 4);
        }
        // the output time over which `left` of material is consumed
        let s = left as f64 / span;
        let dt = crate::output_time_for_source(warp.base(), warp.prof(), s) * span;
        if !dt.is_finite() {
            return Tick(i64::MAX / 4);
        }
        Tick(c.start.0.saturating_add(dt.round() as i64))
    };
    if !c.reversed {
        return by_material();
    }
    // for a mirrored one the limit is set by the material on the LEFT (it moves down), and it is
    // stricter than the right one — we take both
    Tick(trim_max_end(c).0.min(by_material().0))
}

/// How far to the right the END of the clip can be dragged. Symmetrically: for a mirrored
/// clip, widening to the right pulls `source_in` down, and the limit is set by the material.
pub fn trim_max_end(c: &ClipEvent) -> Tick {
    if !c.reversed {
        return Tick(i64::MAX / 4);
    }
    let Some(warp) = crate::clip_warp(c) else {
        return c.start + c.duration + c.source_in;
    };
    let span = c.duration.0.max(1) as f64;
    if crate::eff_rate(warp.base(), warp.prof(), 0.0) <= 0.0 {
        return Tick(i64::MAX / 4);
    }
    let s = -(c.source_in.0 as f64) / span;
    let dt = crate::output_time_for_source(warp.base(), warp.prof(), s) * span;
    Tick(c.start.0 + c.duration.0 - dt.round() as i64)
}

/// The material entry point of a piece is the ONLY rule of the project.
pub fn slice_source_in(c: &ClipEvent, w: TickRange) -> Tick {
    slice_anchors(c, w).1
}

/// Is there enough material to the left of the window. ONE function per command, its
/// `check_on` and the pre-filters in `ops`: if they diverged, the gesture would sometimes fail and sometimes
/// silently stop short.
pub fn slice_fits_source(c: &ClipEvent, w: TickRange) -> bool {
    slice_source_in(c, w).0 >= 0
}

pub fn clip_slice(c: &ClipEvent, w: TickRange, id: ClipId) -> ClipEvent {
    let mut s = c.clone();
    s.id = id;
    s.start = w.start;
    s.duration = w.len();
    let (linear, material) = slice_anchors(c, w);
    s.source_in = material;
    // Keys travel by the SAME delta: the difference `(k.t − source_in)` must
    // stay the same, otherwise the whole curve would move off with the anchor. A check on
    // numbers: a 4 s clip, a ramp 1.0→2.0, a cut at 2 s — without compensation
    // the first point of the profile of the right piece lands at −1.25 instead of −1.0, and
    // its speed at the start reads 1.625 instead of 1.5: a joint without a jump in
    // the MATERIAL, but with a jump in SPEED. Only the material changes.
    //
    // R.33 promised that keyframe commands do not touch it at all. The promise held
    // because with a linear map the delta is EXACTLY ZERO — and so it remained:
    // on a clip without a warp this block is not executed.
    let shift = material - linear;
    if shift.0 != 0 {
        for slot in &mut s.fx_auto {
            for cur in slot {
                for k in &mut cur.keys {
                    k.t = k.t + shift;
                }
            }
        }
        // generator curves live in the same domain (material tick) and
        // must travel by the same delta — otherwise for a title piece the “typewriter”
        // starts not where it does for the neighbouring piece
        for cur in &mut s.gen_auto {
            for k in &mut cur.keys {
                k.t = k.t + shift;
            }
        }
    }
    s
}

/// Stretch the event's curves under the NEW material span (R.40, option
/// `Project.trim_stretches_auto`).
///
/// It is called ONLY from trims and ONLY with the option on: a cut and a slice
/// by a morpher layer split one animation into parts; there is nothing to stretch there.
///
/// A key's share is computed from the CONSUMED material (`ClipWarp::used`), not
/// from the length of the event: for a stretched clip these are different quantities, and measuring
/// the share by the event length would mean adding a third rule next to the two
/// existing ones.
/// A key's share is computed FROM THE OLD ANCHOR and carried over to the new one: the meaning
/// of the option is “the animation is pinned to the event”, so a key that stood at
/// the half of the former event stands at the half of the new one. Measuring the offset
/// from the NEW anchor would be a different operation: for a left trim the offset is already
/// reduced by the trim's own amount, and the keys would slide twice.
pub fn stretch_auto_span(s: &mut ClipEvent, old_anchor: Tick, span_old: i64, span_new: i64) {
    if span_old <= 0 || span_new <= 0 || span_old == span_new {
        return;
    }
    let (a0, a1) = (old_anchor.0, s.source_in.0);
    let k = span_new as f64 / span_old as f64;
    for slot in &mut s.fx_auto {
        for cur in slot {
            for key in &mut cur.keys {
                let off = (key.t.0 - a0) as f64;
                key.t = Tick(a1 + (off * k).round() as i64);
            }
            cur.keys.sort_by_key(|x| x.t.0);
        }
    }
}

/// How much MATERIAL the event actually occupies: for a stretched one — the SPAN
/// of position (R.57); for an ordinary one — its own length.
///
/// Precisely the span, not “how much is consumed”: a signed curve can go
/// backward and come back, having consumed twice as much as it occupied. On positive
/// curves the span equals the former `used`.
pub fn clip_material_span(c: &ClipEvent) -> i64 {
    crate::clip_warp(c).map(|w| w.span_len()).unwrap_or(c.duration.0).max(1)
}

/// A morpher from a file BEFORE R.35: the former semantics — the patch acts on targets
/// entirely, the block's time affects nothing. Called by the OTIO reader and
/// `ops::upgrade_legacy`; the same device as `migrate_legacy_cammotion`
/// (item 230) — a one-way upgrade, not “compatibility”.
pub fn migrate_legacy_morpher(m: &mut MorpherEvent) -> bool {
    if !m.timed {
        return false;
    }
    m.timed = false;
    true
}

/// Marker on the timeline: a moment and an (optional) name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Marker {
    /// The start of the marker. A marker's identity is by it (as before, by `t`).
    pub t: Tick,
    /// Range marker (R.24, OTIO marked_range): 0 = point marker.
    #[serde(default, skip_serializing_if = "tick_is_zero")]
    pub duration: Tick,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Color of OTIO Marker.2 — round-trip of foreign files; the UI draws in its own style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

fn tick_is_zero(t: &Tick) -> bool {
    t.0 == 0
}

/// A named timeline area: time × a set of tracks (R.35 item 2,
/// verbatim: “make the area-selection persistent as marked_region in
/// the form of marked_area, and also refer to it”).
///
/// The name is an ADDRESS: non-empty and unique in one namespace with
/// range markers (“area ‘final’ and marker ‘final’” in one picker
/// — a trap). Renaming updates the morpher references.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkedArea {
    pub name: String,
    pub start: Tick,
    pub duration: Tick,
    /// SEMANTIC track keys, not row numbers: reordering rows
    /// does not move the area. Empty = all tracks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lanes: Vec<(LaneKind, u32)>,
    /// Color of OTIO-compatible markers; the UI draws in its own style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

impl MarkedArea {
    pub fn range(&self) -> TickRange {
        TickRange::new(self.start, self.start + self.duration)
    }

    /// Whether the area covers this track (empty list = all).
    pub fn covers_lane(&self, key: (LaneKind, u32)) -> bool {
        self.lanes.is_empty() || self.lanes.contains(&key)
    }
}

/// A unique name in the occupied space: `wanted` if it is free, otherwise
/// “wanted 2”, “wanted 3”… ONE function for checking commands and for insertion —
/// if they diverged, an insert batch would fail on its own suffixes.
pub fn unique_name(taken: &[String], wanted: &str, stem: &str) -> String {
    let base = if wanted.trim().is_empty() { stem } else { wanted.trim() };
    if !taken.iter().any(|t| t == base) {
        return base.to_string();
    }
    for n in 2..10_000 {
        let cand = format!("{base} {n}");
        if !taken.contains(&cand) {
            return cand;
        }
    }
    format!("{base} {}", taken.len() + 1)
}

impl Marker {
    pub fn at(t: Tick) -> Marker {
        Marker { t, duration: Tick::ZERO, name: String::new(), color: None }
    }
    pub fn named(t: Tick, name: impl Into<String>) -> Marker {
        Marker { t, duration: Tick::ZERO, name: name.into(), color: None }
    }
    /// End of the range (== the start for a point marker).
    pub fn end(&self) -> Tick {
        self.t + self.duration
    }
    pub fn is_range(&self) -> bool {
        self.duration.0 > 0
    }
}

impl<'de> Deserialize<'de> for Marker {
    /// Accepts the old form (a number of ticks), the object {t, name} and the range
    /// {t, duration, name, color}: old projects keep opening.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Marker, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Bare(i64),
            Full {
                t: i64,
                #[serde(default)]
                duration: i64,
                #[serde(default)]
                name: String,
                #[serde(default)]
                color: Option<String>,
            },
        }
        Ok(match Repr::deserialize(d)? {
            Repr::Bare(t) => Marker::at(Tick(t)),
            Repr::Full { t, duration, name, color } => {
                Marker { t: Tick(t), duration: Tick(duration.max(0)), name, color }
            }
        })
    }
}

/// Timeline viewport, saved in the project.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UiState {
    /// Left edge of the view (ticks).
    pub view_start: f64,
    /// Zoom (ticks per pixel).
    pub ticks_per_px: f64,
    /// Cursor position (ticks).
    pub cursor: i64,
    /// LEGACY input (before R.35 the row order was purely visual and lived
    /// here). Now it is in the model (`Project.lane_order`) and goes through
    /// undo — the morpher layer acts “on everything below”, so the order
    /// affects the picture. The reader moves this field into the model; the writer
    /// no longer writes here.
    #[serde(default, skip_serializing)]
    pub lane_order: Vec<(LaneKind, u32)>,
    /// Media bin: free-layout mode (item 7 of round 8). By default
    /// it is on — the user groups the cards themselves.
    #[serde(default, skip_serializing_if = "is_false")]
    pub bin_free: bool,
    /// Card positions in free mode (media, x, y in points from the
    /// top-left corner of the bin). Switching modes does NOT touch the layout —
    /// it is simply not used in grid mode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bin_layout: Vec<(MediaId, f32, f32)>,
    /// Width of the meter panel (item 6 of round 13): it is dragged by its own strip,
    /// so egui does not remember it — we store it ourselves. 0 = default.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub meters_w: f32,
    /// Individual row height in pixels (R.38 item 11: “allow changing
    /// the height of each track individually”). A CONVENIENCE, not content:
    /// height does not affect the picture — unlike the ORDER of rows, which
    /// in R.35 became content, because the morpher acts “on everything
    /// below”. Absolute pixels: vertical zoom does not touch the override,
    /// otherwise “set a track's height” would not stick.
    /// LEGACY input (before R.62 the height lived here): moved onto the track
    /// on read (`normalize`); the writer no longer writes here.
    #[serde(default, skip_serializing)]
    pub lane_heights: Vec<(LaneKind, u32, f32)>,
    /// Bin card groups (item 13 of round 11): frame + name + lock. A single
    /// “lock” is a group of one member without a name; a separate field
    /// is deliberately absent (otherwise a class of defects appears: “a card is locked,
    /// but the group is not”). Outside undo, like the whole UiState.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bin_groups: Vec<BinGroup>,
    /// Track names (item 2 of round 10). Empty → label by the formula V1/A1/M1.
    /// Outside undo, like the whole UiState; the name is bound to (kind, index) and travels
    /// together with the content when tracks are reordered.
    /// LEGACY input (before R.62 the name lived here): moved onto the track on
    /// read (`normalize`); the writer no longer writes here.
    #[serde(default, skip_serializing)]
    pub lane_names: Vec<(LaneKind, u32, String)>,
    /// Lock of the automation graph axis (R.40 item 15). ONE FOR THE WHOLE PROGRAM
    /// and it is remembered in the project — the user's decision verbatim: “One for the whole
    /// program, remembered in the project”. Before this it lived in `AutoView`
    /// of each window and died together with the window.
    ///
    /// The default is ON (rule 4: a useful mode by default), and
    /// the serde default for `bool` is `false`, so the field stores the INVERSE
    /// (“lock released”): files without it are read with the lock on and
    /// remain byte-stable.
    #[serde(default, skip_serializing_if = "is_false")]
    pub auto_axis_unlinked: bool,
    /// A set of on-screen controls over the frame (R.64): 0 — off,
    /// 1 — transform (frame, scale handles, rotation, anchor),
    /// 2 — crop (four handles of its side).
    ///
    /// EXACTLY ONE SET IS ACTIVE — the Resolve rule (`View > Viewer
    /// Overlay`): two sets over one frame fight for the same mouse, and
    /// “where are my handles” becomes a steady source of complaints (reconnaissance
    /// F-UI-001). A convenience, not content: it lives outside undo, like the whole
    /// `UiState`, and the default “off” keeps files byte-stable.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub geom_controls: u32,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

/// Group of media-bin cards (item 13 of round 11).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BinGroup {
    /// Name in the gap of the frame's top line; empty — a frame without a label.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub members: Vec<MediaId>,
    /// Lock: the group's cards are not moved by the mouse.
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
}

/// Drop vanished media from the groups, and the groups that have become empty themselves
/// (media are removed from the project, while the layout/groups lived separately and drifted).
pub fn prune_bin_groups(groups: &mut Vec<BinGroup>, alive: &[MediaId]) {
    for g in groups.iter_mut() {
        g.members.retain(|m| alive.contains(m));
    }
    groups.retain(|g| !g.members.is_empty());
}

/// Track kind in display order (ClipKind does not fit: there is no Morph/Camera).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneKind {
    #[default]
    Video,
    Audio,
    Morph,
    /// Camera tracks (R.29): only camera events live here.
    Camera,
}

impl LaneKind {
    pub const ALL: [LaneKind; 4] =
        [LaneKind::Video, LaneKind::Audio, LaneKind::Morph, LaneKind::Camera];

    /// Whether a row of this kind has mute and solo: service ones (morph,
    /// camera) do not have them — there is nothing to mute there (UI rule 9).
    pub fn has_switches(self) -> bool {
        matches!(self, LaneKind::Video | LaneKind::Audio)
    }
}

/// The stable address of a TRACK (R.62, change B). The track number is the rank of the row
/// (R.56) and changes when a neighbour is inserted; the id does not. The track's
/// FX chain state (invariant 3) and property addressing rest on it.
/// `0` — “not yet assigned” (`Sequence::new`, the content of a collapse);
/// assigned by `Project::assign_track_ids` — on read and after a command.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
pub struct TrackId(pub u64);

/// Blend mode of the VIDEO TRACK composite over the layers below (R.62, F-CMP-003).
/// `out = lerp(below, blend(below, above), α_above × level)`; Normal —
/// the former alpha_over byte-for-byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Add,
    Subtract,
    Darken,
    Lighten,
    Overlay,
    Difference,
}

impl BlendMode {
    pub const ALL: [BlendMode; 9] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Add,
        BlendMode::Subtract,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::Overlay,
        BlendMode::Difference,
    ];
    pub fn tag(self) -> &'static str {
        match self {
            BlendMode::Normal => "normal",
            BlendMode::Multiply => "multiply",
            BlendMode::Screen => "screen",
            BlendMode::Add => "add",
            BlendMode::Subtract => "subtract",
            BlendMode::Darken => "darken",
            BlendMode::Lighten => "lighten",
            BlendMode::Overlay => "overlay",
            BlendMode::Difference => "difference",
        }
    }
}

/// Level limits of an audio track in dB (the track properties passport, R.62).
pub const TRACK_GAIN_MIN_DB: f64 = -60.0;
pub const TRACK_GAIN_MAX_DB: f64 = 24.0;

fn is_normal_blend(b: &BlendMode) -> bool {
    *b == BlendMode::Normal
}

fn is_zero_track_id(id: &TrackId) -> bool {
    id.0 == 0
}

/// A TRACK is a model object (R.62, change B). The order of the track vector =
/// the row order top to bottom; an event's track number is its rank among
/// the tracks of its kind (the R.56 invariant did not change). Name and height live
/// HERE and go through undo; switches, level, chain and blend mode —
/// are project content (the owner's decision: mute and solo
/// affect the export).
///
/// The level depends on the kind: for audio `gain_db` (dB, 0 — as it was), for video
/// `opacity` (fraction, 1 — as it was); a field that does not belong to the kind stays at its default.
/// Service rows (morph, camera) carry only a name, height and lock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Track {
    #[serde(skip_serializing_if = "is_zero_track_id")]
    pub id: TrackId,
    pub kind: LaneKind,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Row height in pixels; 0 — computed.
    #[serde(skip_serializing_if = "is_zero")]
    pub height: f32,
    #[serde(skip_serializing_if = "is_false")]
    pub mute: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub solo: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub lock: bool,
    #[serde(skip_serializing_if = "is_zero_f64")]
    pub gain_db: f64,
    #[serde(skip_serializing_if = "is_one")]
    pub opacity: f64,
    #[serde(skip_serializing_if = "is_normal_blend")]
    pub blend: BlendMode,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fx: Vec<FxInstance>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fx_bypass: Vec<bool>,
    #[serde(skip_serializing_if = "is_false")]
    pub fx_off: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fx_auto: Vec<Vec<ParamCurve>>,
}

impl Default for Track {
    fn default() -> Self {
        Track::new(TrackId(0), LaneKind::Video)
    }
}

/// Track properties as one piece — what the window “Track
/// Properties” and the `SetTrackProps` command edit (the chain — a separate command).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackProps {
    pub name: String,
    pub height: f32,
    pub mute: bool,
    pub solo: bool,
    pub lock: bool,
    pub gain_db: f64,
    pub opacity: f64,
    pub blend: BlendMode,
}

impl Track {
    pub fn new(id: TrackId, kind: LaneKind) -> Track {
        Track {
            id,
            kind,
            name: String::new(),
            height: 0.0,
            mute: false,
            solo: false,
            lock: false,
            gain_db: 0.0,
            opacity: 1.0,
            blend: BlendMode::Normal,
            fx: Vec::new(),
            fx_bypass: Vec::new(),
            fx_off: false,
            fx_auto: Vec::new(),
        }
    }

    /// Row label: the user's name or the formula V1/A1/M1/C1 by rank.
    pub fn label(&self, lane: u32) -> String {
        if self.name.is_empty() { default_lane_label(self.kind, lane) } else { self.name.clone() }
    }

    pub fn props(&self) -> TrackProps {
        TrackProps {
            name: self.name.clone(),
            height: self.height,
            mute: self.mute,
            solo: self.solo,
            lock: self.lock,
            gain_db: self.gain_db,
            opacity: self.opacity,
            blend: self.blend,
        }
    }

    pub fn set_props(&mut self, p: &TrackProps) {
        self.name = p.name.clone();
        self.height = p.height;
        self.mute = p.mute;
        self.solo = p.solo;
        self.lock = p.lock;
        self.gain_db = p.gain_db;
        self.opacity = p.opacity;
        self.blend = p.blend;
    }

    /// Active chain slots (bypass filtered out), as for an event.
    pub fn active_fx(&self) -> Vec<FxInstance> {
        if self.fx_off {
            return Vec::new();
        }
        self.fx
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.fx_bypass.get(*i).copied().unwrap_or(false))
            .map(|(_, f)| f.clone())
            .collect()
    }

    /// Does the track need a STAGE in the audio mix (sum → chain → level →
    /// muting): without it events are summed into the output in the former order —
    /// byte-for-byte (GOLDEN_AUDIO). Solo is decided by the caller (a set by kind).
    pub fn needs_audio_stage(&self) -> bool {
        self.kind == LaneKind::Audio
            && (self.mute || self.gain_db != 0.0 || !self.active_fx().is_empty())
    }

    /// Does a video track need a stage (an implicit reel in the plan): chain,
    /// level ≠ 1 or mode ≠ Normal. Video mute/solo is an exception from the
    /// composite and does not require a stage.
    pub fn needs_video_stage(&self) -> bool {
        self.kind == LaneKind::Video
            && (self.opacity != 1.0
                || self.blend != BlendMode::Normal
                || !self.active_fx().is_empty())
    }

    /// R.62 NOVELTY on the track: what earlier builds did not know at all and
    /// would silently lose (switches, level, blend mode, chain).
    /// Name and height are NOT included here: the name even before R.62 lived in `Track.name`
    /// of the file, the height in `ui`, and an old reader would lose at most
    /// a convenience. The file version depends on this predicate.
    pub fn has_novelty(&self) -> bool {
        self.mute
            || self.solo
            || self.lock
            || self.gain_db != 0.0
            || self.opacity != 1.0
            || self.blend != BlendMode::Normal
            || !self.fx.is_empty()
            || self.fx_off
    }

    /// Nothing except kind and id is set — only the envelope is written to the file.
    pub fn is_plain(&self) -> bool {
        self.name.is_empty()
            && self.height == 0.0
            && !self.mute
            && !self.solo
            && !self.lock
            && self.gain_db == 0.0
            && self.opacity == 1.0
            && self.blend == BlendMode::Normal
            && self.fx.is_empty()
            && self.fx_auto.iter().all(|a| a.is_empty())
            && !self.fx_off
    }
}

/// Blend-mode keys for the properties passport (R.62): `Choice` stores
/// the KEY of the variant, not a number — a number would shift on insertion in the middle.
pub const BLEND_KEYS: &[&str] = &[
    "normal",
    "multiply",
    "screen",
    "add",
    "subtract",
    "darken",
    "lighten",
    "overlay",
    "difference",
];

/// Row height limit in the passport: 0 — “computed”.
pub const TRACK_HEIGHT_MAX: f64 = 400.0;

const TRACK_FLAGS: &[FlagSpec] = &[
    FlagSpec { name: "mute", default: false },
    FlagSpec { name: "solo", default: false },
    FlagSpec { name: "lock", default: false },
];

const TRACK_HEIGHT_SPEC: ParamSpec =
    ParamSpec::num("height", 0.0, TRACK_HEIGHT_MAX, 0.0, 1.0, ParamUnit::Px, ParamKind::Structural);

/// TRACK PROPERTIES PASSPORT (R.62, change B). Level, opacity,
/// height, switches and blend mode — typed properties with
/// clamping: the properties window and the editor read the limits from here, not from their own
/// literals (precedent R.33 — Gain was −24…24 in one window and −48…24 in
/// another). A track has NO curves: its key is a timeline tick, and this is the second
/// automation domain (the next change), so the rows are declared
/// `ParamKind::Structural` — `auto_params()` does not return them, and there is nothing to hang
/// a curve on, neither for the command nor for the interface.
impl HasParams for Track {
    fn param_specs(&self) -> &[ParamSpec] {
        const A: &[ParamSpec] = &[
            ParamSpec::num(
                "gain_db",
                TRACK_GAIN_MIN_DB,
                TRACK_GAIN_MAX_DB,
                0.0,
                0.1,
                ParamUnit::Db,
                ParamKind::Structural,
            ),
            TRACK_HEIGHT_SPEC,
        ];
        const V: &[ParamSpec] = &[
            ParamSpec::num("opacity", 0.0, 1.0, 1.0, 0.01, ParamUnit::Norm, ParamKind::Structural),
            TRACK_HEIGHT_SPEC,
        ];
        const S: &[ParamSpec] = &[TRACK_HEIGHT_SPEC];
        // level — BY KIND: for audio dB, for video opacity; there is no foreign
        // row on the track, otherwise a dead
        // control would hang in the properties window (UI rule 9)
        match self.kind {
            LaneKind::Audio => A,
            LaneKind::Video => V,
            _ => S,
        }
    }

    fn get_param(&self, name: &str) -> Option<f64> {
        match name {
            "gain_db" if self.kind == LaneKind::Audio => Some(self.gain_db),
            "opacity" if self.kind == LaneKind::Video => Some(self.opacity),
            "height" => Some(self.height as f64),
            _ => None,
        }
    }

    fn set_param(&mut self, name: &str, v: f64) -> bool {
        let Some(spec) = self.param_spec(name) else { return false };
        let v = v.clamp(spec.min, spec.max);
        match name {
            "gain_db" => self.gain_db = v,
            "opacity" => self.opacity = v,
            "height" => self.height = v as f32,
            _ => return false,
        }
        true
    }

    fn get_param_idx(&self, idx: usize) -> Option<f64> {
        let name = self.param_specs().get(idx)?.name.clone();
        self.get_param(&name)
    }

    fn set_param_idx(&mut self, idx: usize, v: f64) -> bool {
        let Some(name) = self.param_specs().get(idx).map(|s| s.name.clone()) else { return false };
        self.set_param(&name, v)
    }

    fn flag_specs(&self) -> &'static [FlagSpec] {
        // for service rows only the lock of the switches: there is nothing to mute and
        // solo for a morph layer (UI rule 9)
        if self.kind.has_switches() { TRACK_FLAGS } else { &TRACK_FLAGS[2..] }
    }

    fn get_flag(&self, name: &str) -> Option<bool> {
        match name {
            "mute" if self.kind.has_switches() => Some(self.mute),
            "solo" if self.kind.has_switches() => Some(self.solo),
            "lock" => Some(self.lock),
            _ => None,
        }
    }

    fn set_flag(&mut self, name: &str, v: bool) -> bool {
        match name {
            "mute" if self.kind.has_switches() => self.mute = v,
            "solo" if self.kind.has_switches() => self.solo = v,
            "lock" => self.lock = v,
            _ => return false,
        }
        true
    }

    fn extra_specs(&self) -> Vec<ParamSpec> {
        match self.kind {
            LaneKind::Video => vec![choice_spec("blend", BLEND_KEYS)],
            _ => Vec::new(),
        }
    }

    fn get_value(&self, name: &str) -> Option<ParamValue> {
        if name == "blend" && self.kind == LaneKind::Video {
            return Some(ParamValue::Choice(self.blend.tag().into()));
        }
        default_get_value(self, name)
    }

    fn set_value(&mut self, name: &str, v: &ParamValue) -> bool {
        if name == "blend"
            && self.kind == LaneKind::Video
            && let ParamValue::Choice(key) = v
        {
            let Some(b) = BlendMode::ALL.into_iter().find(|b| b.tag() == key) else { return false };
            self.blend = b;
            return true;
        }
        default_set_value(self, name, v)
    }
}

/// WHAT DOES NOT GO INTO THE FILE (R.62): the names of muted tracks and the active
/// solo by kind. Mute and solo are project content and act on the
/// export (the owner's decision), so the program MUST say so
/// before starting — both the render window and kt-cli, in one wording.
pub struct SilencedTracks {
    /// Names of muted (mute) tracks, top to bottom.
    pub muted: Vec<String>,
    /// Names of tracks in SOLO (empty — solo is not on).
    pub solo: Vec<String>,
}

impl SilencedTracks {
    pub fn is_empty(&self) -> bool {
        self.muted.is_empty() && self.solo.is_empty()
    }
}

/// Collect the list over a reel (the root or a nested one).
pub fn silenced_tracks(reel: &dyn Reel) -> SilencedTracks {
    let rows = reel.rows();
    let solo_on = |k: LaneKind| reel.tracks().iter().any(|t| t.kind == k && t.solo);
    let mut muted = Vec::new();
    let mut solo = Vec::new();
    for (i, (kind, lane)) in rows.iter().enumerate() {
        let Some(t) = reel.tracks().get(i) else { continue };
        if !kind.has_switches() {
            continue;
        }
        if t.mute || (solo_on(*kind) && !t.solo) {
            muted.push(t.label(*lane));
        }
        if t.solo {
            solo.push(t.label(*lane));
        }
    }
    SilencedTracks { muted, solo }
}

/// Formula of the label of a track without a name: V1/A1/M1/C1 by rank.
pub fn default_lane_label(kind: LaneKind, lane: u32) -> String {
    let p = match kind {
        LaneKind::Video => "V",
        LaneKind::Audio => "A",
        LaneKind::Morph => "M",
        LaneKind::Camera => "C",
    };
    format!("{p}{}", lane + 1)
}

/// Rows by the track vector: `(kind, rank within the kind)` — the definition
/// of a track number (R.56 via R.62).
pub fn rows_of_tracks(tracks: &[Track]) -> Vec<(LaneKind, u32)> {
    let mut n = LaneBases::default();
    tracks
        .iter()
        .map(|t| {
            let c = n.get_mut(t.kind);
            let r = *c;
            *c += 1;
            (t.kind, r)
        })
        .collect()
}

/// Where a NEW track of kind `kind` goes without an explicit place: after the last
/// of its kind, otherwise after the block of preceding kinds (V..A..M..C) —
/// the natural block order of the former counters, on which the
/// test scenes and the default “morph rows under audio” rest.
pub fn natural_insert_pos(tracks: &[Track], kind: LaneKind) -> usize {
    let rank = |k: LaneKind| LaneKind::ALL.iter().position(|&x| x == k).unwrap_or(0);
    let mine = rank(kind);
    let mut pos = 0;
    for (i, t) in tracks.iter().enumerate() {
        if rank(t.kind) <= mine {
            pos = i + 1;
        }
    }
    pos
}

/// Index of the track of kind `kind` with rank `lane` in the vector.
pub fn track_index(tracks: &[Track], kind: LaneKind, lane: u32) -> Option<usize> {
    let mut rank = 0u32;
    for (i, t) in tracks.iter().enumerate() {
        if t.kind == kind {
            if rank == lane {
                return Some(i);
            }
            rank += 1;
        }
    }
    None
}

/// Tracks of an old file: counters, `lane_order`, names and heights in `ui`.
/// They are only READ (the migration is one-way, R.62): the writer no longer knows
/// these fields, and `normalize` turns them into the track vector.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct LegacyLanes {
    #[serde(default)]
    pub video_lanes: u32,
    #[serde(default)]
    pub audio_lanes: u32,
    #[serde(default)]
    pub morph_lanes: u32,
    #[serde(default)]
    pub camera_lanes: u32,
    #[serde(default)]
    pub lane_order: Vec<(LaneKind, u32)>,
}

impl LegacyLanes {
    pub fn is_empty(&self) -> bool {
        *self == LegacyLanes::default()
    }
}

/// Expand legacy counters into the track vector: natural order
/// V..A..M..C, then a permutation by `lane_order` (the content
/// is renumbered by the SAME mapping that is returned to the caller).
/// Names and heights (`ui`) are keyed by the OLD numbers — attached before
/// the permutation, then travel together with the object.
fn tracks_from_legacy(
    legacy: &LegacyLanes,
    need: LaneBases,
    names: &[(LaneKind, u32, String)],
    heights: &[(LaneKind, u32, f32)],
) -> Vec<Track> {
    let n = LaneBases {
        video: legacy.video_lanes.max(need.video).max(1),
        audio: legacy.audio_lanes.max(need.audio).max(1),
        morph: legacy.morph_lanes.max(need.morph),
        camera: legacy.camera_lanes.max(need.camera),
    };
    let mut out = Vec::new();
    for kind in LaneKind::ALL {
        for lane in 0..n.get(kind) {
            let mut t = Track::new(TrackId(0), kind);
            if let Some((_, _, nm)) = names.iter().find(|(k, l, _)| *k == kind && *l == lane) {
                t.name = nm.clone();
            }
            if let Some((_, _, h)) = heights.iter().find(|(k, l, _)| *k == kind && *l == lane) {
                t.height = *h;
            }
            out.push(t);
        }
    }
    out
}

/// Normalization of the display order of tracks to the actual counters (Test 49):
/// duplicates and non-existent (kind, lane) are dropped, missing ones
/// are appended at the end in the block order V..A..M..C. An empty input gives
/// the default block order.
pub fn normalize_lane_order(
    order: &[(LaneKind, u32)],
    video: u32,
    audio: u32,
    morph: u32,
    camera: u32,
) -> Vec<(LaneKind, u32)> {
    let valid = |&(k, l): &(LaneKind, u32)| match k {
        LaneKind::Video => l < video,
        LaneKind::Audio => l < audio,
        LaneKind::Morph => l < morph,
        LaneKind::Camera => l < camera,
    };
    let mut out: Vec<(LaneKind, u32)> =
        Vec::with_capacity((video + audio + morph + camera) as usize);
    for &e in order.iter().filter(|e| valid(e)) {
        if !out.contains(&e) {
            out.push(e);
        }
    }
    for (kind, n) in [
        (LaneKind::Video, video),
        (LaneKind::Audio, audio),
        (LaneKind::Morph, morph),
        (LaneKind::Camera, camera),
    ] {
        for l in 0..n {
            if !out.contains(&(kind, l)) {
                out.push((kind, l));
            }
        }
    }
    out
}

/// Rows go in ascending order of number WITHIN EACH KIND — the invariant
/// of R.56 (item 449). The order of the kinds among themselves is arbitrary: `lane_order`
/// encodes only the interleaving.
pub fn rows_are_canonical(rows: &[(LaneKind, u32)]) -> bool {
    for kind in [LaneKind::Video, LaneKind::Audio, LaneKind::Morph, LaneKind::Camera] {
        let mut prev: Option<u32> = None;
        for &(_k, l) in rows.iter().filter(|e| e.0 == kind) {
            if prev.is_some_and(|p| l <= p) {
                return false;
            }
            prev = Some(l);
        }
    }
    true
}

/// Renumbering of tracks by kind: `map(kind, old) = new` (R.56, item 449).
///
/// **The track number within a kind = ITS RANK among the rows of that kind.** Before R.56
/// the number and the row lived separately: the engine stacked video by NUMBER
/// (`(Reverse(lane), start)` in `VideoRenderer::build_reel`), while the screen
/// showed by `lane_order`. A track added from the morph-row menu
/// got the largest number — that is, the BOTTOM layer — and was drawn
/// third from the top: “put the video under the ceiling and it is not visible” (the complaint
/// of the owner, R.56 item 2). The rule removes the discrepancy in the model without touching
/// the engine; item 50 promised “z-order == screen” and was broken in two places of the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanePerm {
    pub video: Vec<u32>,
    pub audio: Vec<u32>,
    pub morph: Vec<u32>,
    pub camera: Vec<u32>,
}

impl LanePerm {
    /// The rank of each track among the rows of its kind. `rows` must be
    /// NORMALIZED (each track exactly once) — otherwise the ranks lie.
    pub fn from_rows(rows: &[(LaneKind, u32)], n: LaneBases) -> LanePerm {
        let mut p = LanePerm::identity(n);
        for kind in [LaneKind::Video, LaneKind::Audio, LaneKind::Morph, LaneKind::Camera] {
            let v = p.of_mut(kind);
            for (rank, &(_, old)) in rows.iter().filter(|e| e.0 == kind).enumerate() {
                if let Some(slot) = v.get_mut(old as usize) {
                    *slot = rank as u32;
                }
            }
        }
        p
    }

    pub fn identity(n: LaneBases) -> LanePerm {
        let id = |k: u32| (0..k).collect::<Vec<u32>>();
        LanePerm {
            video: id(n.video),
            audio: id(n.audio),
            morph: id(n.morph),
            camera: id(n.camera),
        }
    }

    pub fn is_identity(&self) -> bool {
        [&self.video, &self.audio, &self.morph, &self.camera]
            .iter()
            .all(|v| v.iter().enumerate().all(|(i, &l)| i as u32 == l))
    }

    fn of(&self, k: LaneKind) -> &[u32] {
        match k {
            LaneKind::Video => &self.video,
            LaneKind::Audio => &self.audio,
            LaneKind::Morph => &self.morph,
            LaneKind::Camera => &self.camera,
        }
    }

    fn of_mut(&mut self, k: LaneKind) -> &mut Vec<u32> {
        match k {
            LaneKind::Video => &mut self.video,
            LaneKind::Audio => &mut self.audio,
            LaneKind::Morph => &mut self.morph,
            LaneKind::Camera => &mut self.camera,
        }
    }

    /// New track number. A number outside the counter stays as it is: there is nothing to
    /// renumber, and the format reader must not panic on someone else's garbage.
    pub fn map(&self, k: LaneKind, lane: u32) -> u32 {
        self.of(k).get(lane as usize).copied().unwrap_or(lane)
    }

    /// Reorder the `(kind, track)` keys — areas, order rows.
    pub fn remap_keys(&self, keys: &mut [(LaneKind, u32)]) {
        for e in keys.iter_mut() {
            e.1 = self.map(e.0, e.1);
        }
    }

    /// Reorder the tables that carry payload: track names and heights.
    pub fn remap_keyed<T>(&self, items: &mut [(LaneKind, u32, T)]) {
        for e in items.iter_mut() {
            e.1 = self.map(e.0, e.1);
        }
    }

    /// Key `(ClipKind, lane)` — this is how foreign OTIO tracks are addressed.
    pub fn map_clip(&self, k: ClipKind, lane: u32) -> u32 {
        self.map(
            match k {
                ClipKind::Video => LaneKind::Video,
                ClipKind::Audio => LaneKind::Audio,
            },
            lane,
        )
    }
}

/// Synchronization of the display vector on track INSERTION (lane >= at of this
/// kind shifted by +1): the new row goes right above the former `at`
/// (now at+1); if there is none, at the end (Test 49).
pub fn lane_order_insert(rows: &mut Vec<(LaneKind, u32)>, kind: LaneKind, at: u32) {
    for e in rows.iter_mut() {
        if e.0 == kind && e.1 >= at {
            e.1 += 1;
        }
    }
    let pos = rows.iter().position(|&(k, l)| k == kind && l == at + 1).unwrap_or(rows.len());
    rows.insert(pos, (kind, at));
}

/// Synchronization of the display vector on track DELETION (Test 49).
pub fn lane_order_remove(rows: &mut Vec<(LaneKind, u32)>, kind: LaneKind, at: u32) {
    rows.retain(|&(k, l)| !(k == kind && l == at));
    for e in rows.iter_mut() {
        if e.0 == kind && e.1 > at {
            e.1 -= 1;
        }
    }
}

/// The track label by the formula — see `Project::track_label` for the user's
/// name (R.62: names live on the track object).
pub fn lane_label(kind: LaneKind, lane: u32) -> String {
    default_lane_label(kind, lane)
}

/// Half-open region [start, end), start < end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub start: Tick,
    pub end: Tick,
}

impl Region {
    pub fn normalized(a: Tick, b: Tick) -> Option<Region> {
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        let start = Tick(start.0.max(0));
        (end > start).then_some(Region { start, end })
    }

    pub fn contains(&self, t: Tick) -> bool {
        self.start <= t && t < self.end
    }

    pub fn intersects_clip(&self, c: &ClipEvent) -> bool {
        c.start < self.end && c.start + c.duration > self.start
    }
}

/// The pitch-shift engine (R.48, F-AUD-013). The choice is one for the whole edit:
/// the algorithm is a CHOICE FOR THE MATERIAL, and it is heard on transients.
/// `Wsola` — time domain: a hit stays a hit, dense polyphony gives
/// a light “rustle” at the grain joins. `Vocoder` — spectral: the backing parts and
/// the synths travel cleanly, a hit smears. Default — Wsola: it is cheaper and
/// safer on speech, which is what is worked with more often.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PitchEngine {
    #[default]
    Wsola,
    Vocoder,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub sample_rate: u32,
    /// The pitch engine (R.48): the default is not written to the file — old projects
    /// are byte-stable.
    #[serde(default, skip_serializing_if = "is_default_pitch_engine")]
    pub pitch_engine: PitchEngine,

    /// The segment shape of NEW keys (R.61).
    ///
    /// It lives in the PROJECT, not in the program settings: a machine setting
    /// would give “one file — two behaviours” (the lesson of `Lock Envelopes to
    /// Events` in Vegas), and the project would stop describing itself.
    /// Changing the default does NOT touch keys already placed — for this there is
    /// the command “apply the shape to all segments”.
    /// The default is not written to the file: former projects are byte-stable.
    #[serde(default, skip_serializing_if = "SegShape::is_default")]
    pub default_shape: SegShape,
    /// The frame-conform rule BY DEFAULT (R.62): applies to all
    /// events with `Conform::Project`, retroactively. The default “fit”
    /// is not written — former files are byte-stable.
    #[serde(default, skip_serializing_if = "is_default_rule")]
    pub conform: ConformRule,
    /// Settings have been set EXPLICITLY (the settings window applied). While `false`,
    /// the first clip into an empty project sets the raster and fps (rule 20: an explicit
    /// decision is not touched by placement). `false` is not written.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub settings_explicit: bool,
}

fn is_default_rule(r: &ConformRule) -> bool {
    *r == ConformRule::default()
}

fn is_default_pitch_engine(e: &PitchEngine) -> bool {
    *e == PitchEngine::default()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaRef {
    pub id: MediaId,
    /// Path relative to the project file.
    pub path: String,
    /// Material duration, IF IT IS KNOWN (R.63 item 2).
    ///
    /// The model does not obtain it itself and cannot (law P: what lies
    /// at an address is a property of the SOURCE). It is written by whoever OPENED
    /// the source: kt-app on placing and syncing media, kt-cli on probe,
    /// the file reader — from the envelope. Hence `Option`: a generator and
    /// a reel have no duration of their own, and a file not yet opened has had it
    /// measured by nobody yet, and in these cases there is no trim limit — as it
    /// always was.
    ///
    /// Why the model needs to know this. Before R.63 `trim_max_end` of a non-mirrored
    /// event returned `i64::MAX / 4`: the right edge could be dragged
    /// as far to the right as you like, past the end of the material, and nobody said
    /// anything about it. Exactly so for the owner, the event became 316.967 s long with a
    /// file of 226.268 s — the last 11.5 s of material “sounded” as silence and
    /// were counted as underruns (R.63 item 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<Tick>,
}

impl Project {
    /// Length of the material of this media, if someone has measured it (R.63).
    /// The only door: a reel has no duration of its own (its content
    /// ends by itself); a generator never has one at all.
    pub fn media_duration(&self, id: MediaId) -> Option<Tick> {
        self.media.iter().find(|m| m.id == id).and_then(|m| m.duration).filter(|d| d.0 > 0)
    }
}

impl MediaRef {
    /// Media without a known duration — the usual case on creation.
    pub fn new(id: MediaId, path: impl Into<String>) -> MediaRef {
        MediaRef { id, path: path.into(), duration: None }
    }

    /// The same, but the duration has already been measured by the one who opened the source.
    pub fn with_duration(id: MediaId, path: impl Into<String>, duration: Tick) -> MediaRef {
        MediaRef { id, path: path.into(), duration: Some(duration) }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipEvent {
    pub id: ClipId,
    #[serde(default)]
    pub link_group: Option<GroupId>,
    pub kind: ClipKind,
    /// z-order for video (lower = higher), mix track for audio.
    pub lane: u32,
    pub start: Tick,
    pub duration: Tick,
    pub source_in: Tick,
    pub media: MediaId,
    #[serde(default)]
    pub gain_db: f32,
    /// Reverse of the source (audio and video are read backwards; the FX chain
    /// is applied AFTER the reverse). serde default — old files are stable.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reversed: bool,
    #[serde(default)]
    pub fx: Vec<FxInstance>,
    /// Bypass flags of the slots (item 2 of v1.3): true = the slot is OFF. Parallel to
    /// fx (indices); shorter than fx = the tail is on. An empty one is not serialized.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fx_bypass: Vec<bool>,
    /// Video opacity 0..1 (R.28, item 12 — the event's “level” as in
    /// Vegas: a line along the top of the clip, dragged with the mouse). For audio the role of level
    /// is played by `gain_db`. The default 1.0 is not serialized — old files
    /// are byte-stable.
    #[serde(default = "one_f32", skip_serializing_if = "is_one_f32")]
    pub opacity: f32,
    /// Automation of video-FX parameters (R.30): curves per slot,
    /// parallel to `fx` (like fx_bypass; shorter than fx = the tail without curves).
    /// Domain t — clip progress 0..1 (like CamMotion and cameras). An empty
    /// vector is not serialized — old files are byte-stable; in .otio
    /// the curves travel inside the effect's kt envelope and raise the file to v2
    /// (an old reader would silently lose them — the R.29 rule).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fx_auto: Vec<Vec<ParamCurve>>,
    /// The master switch of the WHOLE event chain (R.33, the user's
    /// request: “there is no master checkbox to switch off the current
    /// fx-chain”). It differs from slot bypass in that it does not touch their
    /// state: switch it back on — the chain is the same. The default (on)
    /// is not serialized, files are byte-stable.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fx_off: bool,
    /// Generator kind WITH VALUES (R.59, stage 2).
    ///
    /// `Some` if and only if `media` is a generator; the kind itself
    /// must match the kind in the media path (`gen_kind_path`); this is
    /// checked by `validate`. Before this round the values lay in the media
    /// path, that is, they were shared by all places of use, and “a second
    /// release with a different title” had physically nowhere to be written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#gen: Option<crate::GeneratorKind>,
    /// Parameter curves of the generator. Domain `t` — the MATERIAL TICK, as in
    /// `fx_auto`: the key is glued to the material, not to the length of the event.
    /// They are resolved by the SOURCE through its own `pts`, not by the frame plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gen_auto: Vec<ParamCurve>,
    /// Overrides of the PUBLIC parameters of the REEL this event points to
    /// (R.59, stage 3).
    ///
    /// They are legal only on an instance of a nested clip and only for the PUBLISHED
    /// (the owner's decision: “an instance overrides only the
    /// published”). This is the only place where one can put the
    /// difference of an instance from the reel: a nested clip for us is a REFERENCE, and an edit
    /// of the reel reaches all places by itself.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<Override>,
    /// The leading “Geometry” node (R.62). Does not take part in an audio event.
    /// The default is not serialized — former files are byte-stable;
    /// an event without the node is identical to the node “follow the project”.
    #[serde(default, skip_serializing_if = "EventGeometry::is_default")]
    pub geometry: EventGeometry,
    /// Position of the leading node in the chain: before the slot `fx[geometry_pos]`
    /// (0 — first, `fx.len()` — last). Effects BEFORE it work in
    /// the source space, AFTER it — on the canvas.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub geometry_pos: usize,
    /// Parameter curves of the leading node (domain — the material tick, as in
    /// `fx_auto`). The engine puts them into the node's slot on materialization.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub geometry_auto: Vec<ParamCurve>,
}

fn is_zero_usize(v: &usize) -> bool {
    *v == 0
}

fn one_f32() -> f32 {
    1.0
}

fn is_default_frame_mode(m: &FrameMode) -> bool {
    *m == FrameMode::Nearest
}

fn default_cutoff_hz() -> f32 {
    800.0
}
fn default_resonance() -> f32 {
    0.707
}
fn default_low_freq() -> f32 {
    200.0
}
fn default_mid_freq() -> f32 {
    1000.0
}
fn default_high_freq() -> f32 {
    4000.0
}
fn default_drive_db() -> f32 {
    12.0
}
fn default_half_f32() -> f32 {
    0.5
}
fn default_bits() -> f32 {
    8.0
}
fn default_crush_rate() -> f32 {
    12000.0
}
fn default_trem_rate() -> f32 {
    5.0
}
fn default_trem_depth() -> f32 {
    0.6
}
fn default_autopan_depth() -> f32 {
    0.8
}
fn default_vib_rate() -> f32 {
    5.0
}
fn default_vib_depth() -> f32 {
    0.5
}
fn default_chorus_rate() -> f32 {
    0.8
}
fn default_chorus_depth() -> f32 {
    3.5
}
fn default_voices() -> u32 {
    2
}
fn default_flanger_rate() -> f32 {
    0.25
}
fn default_flanger_depth() -> f32 {
    2.0
}
fn default_flanger_fb() -> f32 {
    0.5
}
fn default_phaser_rate() -> f32 {
    0.4
}
fn default_phaser_depth() -> f32 {
    0.7
}
fn default_phaser_fb() -> f32 {
    0.3
}
fn default_stages() -> u32 {
    4
}
fn default_delay_time() -> f32 {
    350.0
}
fn default_delay_fb() -> f32 {
    0.35
}
fn default_delay_mix() -> f32 {
    0.35
}
fn default_comp_thr() -> f32 {
    -18.0
}
fn default_comp_ratio() -> f32 {
    4.0
}
fn default_comp_att() -> f32 {
    10.0
}
fn default_comp_rel() -> f32 {
    150.0
}
fn default_gate_thr() -> f32 {
    -40.0
}
fn default_gate_att() -> f32 {
    1.0
}
fn default_gate_hold() -> f32 {
    40.0
}
fn default_gate_rel() -> f32 {
    100.0
}
fn default_duo_sr() -> u8 {
    24
}
fn default_duo_sg() -> u8 {
    18
}
fn default_duo_sb() -> u8 {
    64
}
fn default_duo_hr() -> u8 {
    255
}
fn default_duo_hg() -> u8 {
    236
}
fn default_duo_hb() -> u8 {
    190
}
fn default_two_u32() -> u32 {
    2
}

fn default_twelve_u32() -> u32 {
    12
}

fn default_chroma_amount() -> f32 {
    0.012
}

fn default_quarter_f32() -> f32 {
    0.25
}

fn default_eight_i32() -> i32 {
    8
}

fn default_four_u32() -> u32 {
    4
}

fn default_shadow_opacity() -> f32 {
    0.6
}

fn default_eight_u32() -> u32 {
    8
}

fn default_halftone_angle() -> f32 {
    45.0
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

fn default_limiter_threshold() -> f32 {
    -6.0
}

fn default_limiter_ceiling() -> f32 {
    -0.3
}

fn default_limiter_lookahead() -> f32 {
    5.0
}

fn default_limiter_release() -> f32 {
    80.0
}

fn default_lens_k1() -> f32 {
    0.35
}

fn default_twirl_angle() -> f32 {
    120.0
}

fn default_twirl_radius() -> f32 {
    0.8
}

fn default_ripple_amount() -> f32 {
    0.08
}

fn default_ripple_freq() -> f32 {
    10.0
}

fn default_kaleido_segments() -> u32 {
    6
}

fn default_levels() -> u32 {
    6
}
fn default_vig_radius() -> f32 {
    0.7
}
fn default_vig_soft() -> f32 {
    0.45
}
fn default_one_f64() -> f64 {
    1.0
}
fn is_one_f64(v: &f64) -> bool {
    *v == 1.0
}
fn default_one_f32() -> f32 {
    1.0
}

fn is_one_f32(v: &f32) -> bool {
    *v == 1.0
}

impl ClipEvent {
    /// The active FX chain (without bypass slots) — the only kind of chain
    /// that the engine and the renderers see.
    pub fn active_fx(&self) -> Vec<FxInstance> {
        if self.fx_off {
            return Vec::new();
        }
        self.fx
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.fx_bypass.get(*i).copied().unwrap_or(false))
            .map(|(_, f)| f.clone())
            .collect()
    }

    /// Automation curves aligned with active_fx(): bypass slots
    /// drop out TOGETHER with their curves (R.30) — otherwise a curve would stick
    /// to someone else's slot.
    pub fn active_fx_auto(&self) -> Vec<Vec<ParamCurve>> {
        if self.fx_off {
            return Vec::new();
        }
        self.fx
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.fx_bypass.get(*i).copied().unwrap_or(false))
            .map(|(i, _)| self.fx_auto.get(i).cloned().unwrap_or_default())
            .collect()
    }
}

/// Automation keyframe of a parameter.
///
/// `t` — the position IN THE SOURCE MATERIAL (ticks in the coordinate system of
/// `source_in`), not the clip progress (R.33, the user's decision “keys
/// are glued to the material, as in Premiere/AE”). All the rest of the behaviour follows from this:
/// trimming hides and reveals keys without distorting the
/// animation; a cut gives each half its piece of the curve; a reverse
/// mirrors the animation together with the material; a change of speed in audio pulls the
/// keys along. No geometric command touches keys —
/// the gluing is ensured by the REPRESENTATION, not by the command code.
///
/// Before R.33 here was a 0..1 share of the event's length: trimming stretched the
/// animation, and `Split` copied the curve entirely into both halves.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AutoKey {
    pub t: Tick,
    pub v: f64,
    /// The segment shape AFTER this key (R.61). Before R.61 the shape was one for
    /// the whole curve — the boolean `ParamCurve::ease` — and neither “this section as a step”
    /// nor a zigzag could be expressed by it.
    ///
    /// For the last key the field has no effect: there is no segment to its right.
    /// The default (`Smooth { tension: 1.0 }`) computes EXACTLY the former
    /// `smoothstep01` and is not serialized — the bytes of old files and
    /// `GOLDEN_*` do not move.
    #[serde(default, skip_serializing_if = "SegShape::is_default")]
    pub shape: SegShape,
}

impl AutoKey {
    /// A key with the default shape. A constructor, not a literal: the shape field was
    /// added later, and rewriting a hundred and twenty literals across the tree
    /// for each new field is a second way to forget one of them.
    pub const fn at(t: Tick, v: f64) -> AutoKey {
        AutoKey { t, v, shape: SegShape::DEFAULT }
    }

    /// A key with an explicit segment shape after it.
    pub const fn shaped(t: Tick, v: f64, shape: SegShape) -> AutoKey {
        AutoKey { t, v, shape }
    }

    /// Set one shape on all keys — “apply to all segments”.
    pub fn all_shaped(mut keys: Vec<AutoKey>, shape: SegShape) -> Vec<AutoKey> {
        for k in &mut keys {
            k.shape = shape;
        }
        keys
    }

    /// A piecewise-linear curve in one line (a frequent case in tests and in
    /// paths where smoothing is forbidden — for example for the speed of a warp).
    pub fn linear(keys: Vec<AutoKey>) -> Vec<AutoKey> {
        AutoKey::all_shaped(keys, SegShape::Linear)
    }
}

/// Insert a key, preserving the SHAPE OF THE SEGMENT it divides (R.61).
///
/// The only insertion rule for the whole program. A key placed
/// in the middle of a zigzag must leave the zigzag on both sides: otherwise a click on the
/// graph would silently turn the curve into a smoothing. For the speed of a warp the
/// same rule saves from a validation failure — there all segments are straight, and
/// the new key is straight by inheritance, not by lucky chance.
///
/// `keys` stays sorted; a key at the same position is replaced.
pub fn insert_key(keys: &mut Vec<AutoKey>, at: Tick, v: f64, default: SegShape) {
    // shape of the divided segment = the shape of the nearest key on the LEFT; if none on the left —
    // the shape of the first; if there are no keys at all — the project default
    let shape =
        keys.iter().rev().find(|k| k.t <= at).or_else(|| keys.first()).map_or(default, |k| k.shape);
    keys.retain(|k| k.t != at);
    keys.push(AutoKey::shaped(at, v, shape));
    keys.sort_by_key(|k| k.t.0);
}

/// Zigzag profile: what exactly the segment jitters with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZigProfile {
    /// Triangle — the cheapest and the most “hand-made” looking.
    #[default]
    Triangle,
    /// Square: the value jumps between two levels.
    Square,
    /// Sine — the only smooth one of the three.
    Sine,
}

/// Shape of the segment between two adjacent keys (R.61).
///
/// The set is CLOSED, and each kind has ITS OWN parameters — not one common
/// “stiffness” number. This is because the zigzag is recognized as a segment shape
/// (the owner's decision, a revision of verdict F-AUT-009 item 3), and one number is too little for it:
/// periods, amplitude and profile are needed as well.
///
/// Each shape is a CLOSED function `(s ∈ [0,1], a, b, parameters)` without
/// state: both post-FX memoization and determinism rest on this.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "s", rename_all = "snake_case")]
pub enum SegShape {
    /// Straight: the value goes linearly from `a` to `b`.
    Linear,
    /// Third order. `tension = 1` — exactly the former `smoothstep01`;
    /// 0 degenerates into a straight line, above one — sharper at the ends.
    Smooth { tension: f64 },
    /// Step: until the next key the left one's value is held.
    Hold,
    /// Soft start, sharp arrival.
    EaseIn { tension: f64 },
    /// Sharp start, soft arrival.
    EaseOut { tension: f64 },
    /// Jitter around the line `a → b`.
    ///
    /// **The amplitude is in UNITS OF THE PARAMETER, not as a share of the difference `b − a`**:
    /// the most frequent zigzag is jitter around a CONSTANT value, where
    /// the difference is zero, and a share would collapse the feature exactly where it
    /// is needed. Clamping to the target limits is done by the caller — the property passport
    /// knows them, the shape does not.
    Zigzag { periods: f64, amplitude: f64, profile: ZigProfile },
    /// SEGMENT FORMULA (R.67): a text with its own evaluator and variables
    /// `s` (share of the path 0..1), `a` and `b` (end values), `t` (seconds from the
    /// left key). The ends are set by the formula itself — as for a step, it is allowed
    /// not to coincide with the key. An uncomputable text is rejected by
    /// validation; on a segment an evaluation error (division by zero at one
    /// point) gives a straight line between the ends, not a silent zero.
    ///
    /// The text lives in the interner (`ExprId`): the shape and the key stay `Copy`,
    /// as all the rest of the curve code holds them; the text travels to the file.
    Formula { expr: ExprId },
}

/// Interned text of the segment formula (R.67): a `Copy` handle instead of a
/// `String`, so that `SegShape`/`AutoKey` do not lose `Copy`. Identical text —
/// one handle (equality of shapes = equality of texts). The table only
/// grows — by the number of DIFFERENT texts a person has typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprId(u32);

static EXPRS: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> = std::sync::OnceLock::new();

impl ExprId {
    /// Handle to the text: the same text — the same handle.
    pub fn intern(text: &str) -> ExprId {
        let m = EXPRS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
        let mut v = m.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = v.iter().position(|s| s == text) {
            return ExprId(i as u32);
        }
        v.push(text.to_string());
        ExprId((v.len() - 1) as u32)
    }

    /// The formula text.
    pub fn text(self) -> String {
        let m = EXPRS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
        let v = m.lock().unwrap_or_else(|e| e.into_inner());
        v.get(self.0 as usize).cloned().unwrap_or_default()
    }
}

impl Serialize for ExprId {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.text())
    }
}

impl<'de> Deserialize<'de> for ExprId {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<ExprId, D::Error> {
        let s = String::deserialize(de)?;
        Ok(ExprId::intern(&s))
    }
}

impl Default for SegShape {
    fn default() -> SegShape {
        SegShape::DEFAULT
    }
}

impl SegShape {
    /// The default — the former smoothing of the curve (`ease = true`).
    pub const DEFAULT: SegShape = SegShape::Smooth { tension: 1.0 };

    pub fn is_default(&self) -> bool {
        *self == SegShape::DEFAULT
    }

    /// Value ON THE SEGMENT: `s` — the share of the path from the left key to the right one.
    ///
    /// The ends must coincide with the key values for all shapes except
    /// the step and the formula: otherwise the curve would tear at every key.
    /// A segment formula without time (`t` = 0) — for places where there are no seconds from
    /// the left key; the curve calls `at_t`.
    pub fn at(&self, s: f64, a: f64, b: f64) -> f64 {
        self.at_t(s, a, b, 0.0)
    }

    /// The same with `secs` — seconds from the left key (the variable `t` in
    /// the segment formula). For other shapes `secs` is not read.
    pub fn at_t(&self, s: f64, a: f64, b: f64, secs: f64) -> f64 {
        let s = s.clamp(0.0, 1.0);
        match self {
            SegShape::Linear => a + (b - a) * s,
            SegShape::Hold => {
                if s >= 1.0 {
                    b
                } else {
                    a
                }
            }
            // `tension = 1` must give EXACTLY `s*s*(3-2s)`: on this
            // rest GOLDEN_* and the frozen oracle t695b.
            SegShape::Smooth { tension } => {
                let k = tension.clamp(0.0, 4.0);
                let k: f64 = k;
                // the ends of the tension scale are computed EXACTLY, not by the
                // blending formula: `s + (w − s)·1` is arithmetically equal to `w`, but
                // in floating point it is not, and the default would diverge from the
                // former smoothing in the last bit, shifting GOLDEN_*
                let f = if k == 1.0 {
                    s * s * (3.0 - 2.0 * s)
                } else if k == 0.0 {
                    s
                } else {
                    let w = s * s * (3.0 - 2.0 * s);
                    s + (w - s) * k
                };
                a + (b - a) * f
            }
            SegShape::EaseIn { tension } => {
                let k = tension.clamp(0.0, 4.0).max(f64::MIN_POSITIVE);
                a + (b - a) * s.powf(1.0 + k)
            }
            SegShape::EaseOut { tension } => {
                let k = tension.clamp(0.0, 4.0).max(f64::MIN_POSITIVE);
                a + (b - a) * (1.0 - (1.0 - s).powf(1.0 + k))
            }
            SegShape::Formula { expr } => {
                let mut vars = std::collections::BTreeMap::new();
                vars.insert("s".to_string(), s);
                vars.insert("a".to_string(), a);
                vars.insert("b".to_string(), b);
                vars.insert("t".to_string(), secs);
                match crate::formula::cached(&expr.text()).and_then(|f| f.eval(&vars).ok()) {
                    Some(v) => v,
                    None => a + (b - a) * s,
                }
            }
            SegShape::Zigzag { periods, amplitude, profile } => {
                let (periods, amplitude, profile) = (*periods, *amplitude, *profile);
                let base = a + (b - a) * s;
                // the ends must coincide with the keys: the window damps the jitter
                // at the edges, otherwise the curve would tear at every key.
                // A parabola, not a sine: `sin(pi)` in floating point equals
                // 1.2e-16, and the right end would not converge with the key EXACTLY.
                let win = 4.0 * s * (1.0 - s);
                let ph = s * periods.abs().min(1024.0);
                let w = match profile {
                    ZigProfile::Triangle => {
                        let f = (ph * 2.0).rem_euclid(2.0);
                        if f <= 1.0 { f * 2.0 - 1.0 } else { 3.0 - f * 2.0 }
                    }
                    ZigProfile::Square => {
                        if ph.rem_euclid(1.0) < 0.5 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    ZigProfile::Sine => (2.0 * std::f64::consts::PI * ph).sin(),
                };
                base + amplitude * w * win
            }
        }
    }
}

/// A curve of one parameter of one FX slot. The shape of EACH segment lives
/// on its left key (`AutoKey::shape`, R.61); beyond the edges — the end
/// values. The parameter name is the snake_case of the variant's field (as in the file);
/// an orphan curve (the name does not fit the variant) silently does not act —
/// validation catches it on load/commands.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamCurve {
    pub param: String,
    pub keys: Vec<AutoKey>,
    /// WHERE the values come from (R.59, item 466). Today — keys only;
    /// a formula and a link to another property take up a place in the schema, but
    /// are rejected by validation: the place for them was introduced IN ADVANCE, so that
    /// a future evaluator would not require breaking the format, and a file with
    /// an uncomputable curve fails loudly rather than drawing the wrong thing.
    ///
    /// The field is not written while the source is keys: the bytes of old files do not
    /// move by a single character. The serialization of the curve is its own (see below),
    /// so there are no serde attributes on the fields.
    pub source: CurveSource,
}

/// Whether the formula text reads the name `name` (by parsing, not by substring:
/// `k` inside `clamp` is not a variable).
fn expr_reads(expr: &str, name: &str) -> bool {
    crate::formula::Formula::parse(expr).is_ok_and(|f| f.reads.iter().any(|r| r == name))
}

/// The curve's wire format in the file (R.61, D1b).
///
/// In the MODEL the shape lives on the key; in the FILE it is written BY CONTENT —
/// by the same rule as the envelope version. A curve whose keys all
/// carry one expressible shape is written with the FORMER `ease` field, and not a single
/// byte moves; a shape different from it travels on its own key and
/// raises the file to v10.
///
/// Without this, a simple removal of `ease` would shift the bytes of EVERY file with
/// curves — that is, it would break “a file without novelties stays the same”
/// (t433b across all fixtures and the golden `demo-3d.otio`, t189b).
#[derive(Serialize, Deserialize)]
struct KeyWire {
    t: Tick,
    v: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shape: Option<SegShape>,
}

#[derive(Serialize, Deserialize)]
struct CurveWire {
    param: String,
    #[serde(default = "default_true")]
    ease: bool,
    keys: Vec<KeyWire>,
    #[serde(default, skip_serializing_if = "CurveSource::is_keys")]
    source: CurveSource,
}

impl ParamCurve {
    /// The shape that the former boolean field `ease` expresses, TAKING INTO ACCOUNT THE NAME
    /// of the parameter (R.63): `true` — third order with the former smoothing,
    /// `false` — a straight line, and for the SPEED of a warp — always a straight line.
    ///
    /// About speed this is not a concession to the validator but a correct translation. Before R.61
    /// the warp map did not read the `ease` field AT ALL (only `cam_at` of the camera read it):
    /// the speed curve always sounded piecewise-linear, whatever the boolean was.
    /// By expanding `ease: true` into “smooth”, the R.61 reader
    /// invented a shape that had never existed in the sound — and the owner's file
    /// (14 `rate` curves) stopped opening at all
    /// (`RateShapeMustBeLinear`, R.63 item 1). The reader knows the parameter name,
    /// so it knows the one and only correct shape too.
    ///
    /// An EXPLICIT shape on a key (the `shape` field, files with v10) is not touched here:
    /// our writer does not produce it for speed, and one that comes from outside
    /// is still rejected loudly by validation.
    pub fn implied_shape_of(param: &str, ease: bool) -> SegShape {
        if param == "rate" {
            SegShape::Linear
        } else if ease {
            SegShape::DEFAULT
        } else {
            SegShape::Linear
        }
    }
}

impl ParamCurve {
    /// Whether the curve has a SEGMENT formula (R.67): such a curve the renderer
    /// does not evaluate itself — the resolver expands it into keys on the frame grid.
    pub fn has_formula_segments(&self) -> bool {
        self.keys.iter().any(|k| matches!(k.shape, SegShape::Formula { .. }))
    }

    /// Whether the curve needs the resolver's work: a derived source or a segment
    /// formula. A project without either pays nothing.
    pub fn needs_resolve(&self) -> bool {
        !self.source.is_keys() || self.has_formula_segments()
    }

    /// R.67 novelties in the FILE (version v13 by content): a segment formula, a layer
    /// over non-empty keys, or a formula reading `k`. An old build would read a segment
    /// formula as a straight line, and a layer would erase the keys.
    pub fn needs_v13(&self) -> bool {
        if self.has_formula_segments() {
            return true;
        }
        match &self.source {
            CurveSource::Keys => false,
            CurveSource::Formula { expr } => {
                !self.keys.is_empty() || expr_reads(expr, "k")
            }
            CurveSource::Link { expr, .. } => expr.as_deref().is_some_and(|e| expr_reads(e, "k")),
        }
    }

    /// Start curve — a CONSTANT for the parameter (R.61).
    ///
    /// The only place where the shape of a NEW curve is decided: the project's default,
    /// but for the speed of a warp it is ALWAYS a straight line — on the piecewise-linear profile
    /// a closed stretch map stands,
    /// and a smoothed start curve would be a validation failure in the very same second.
    pub fn constant(param: &str, from: Tick, to: Tick, v: f64, default: SegShape) -> ParamCurve {
        let shape = if param == "rate" { SegShape::Linear } else { default };
        ParamCurve {
            param: param.to_string(),
            keys: vec![AutoKey::shaped(from, v, shape), AutoKey::shaped(to, v, shape)],
            source: CurveSource::Keys,
        }
    }
}

impl Serialize for ParamCurve {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        // `ease = false` exactly when the WHOLE curve is straight: so a file of the
        // former kind is written with the former bytes.
        let ease = !self.keys.iter().all(|k| k.shape == SegShape::Linear);
        let implied = ParamCurve::implied_shape_of(&self.param, ease);
        let keys: Vec<KeyWire> = self
            .keys
            .iter()
            .map(|k| KeyWire { t: k.t, v: k.v, shape: (k.shape != implied).then_some(k.shape) })
            .collect();
        let n = if self.source.is_keys() { 3 } else { 4 };
        let mut st = ser.serialize_struct("ParamCurve", n)?;
        st.serialize_field("param", &self.param)?;
        st.serialize_field("ease", &ease)?;
        st.serialize_field("keys", &keys)?;
        if self.source.is_keys() {
            st.skip_field("source")?;
        } else {
            st.serialize_field("source", &self.source)?;
        }
        st.end()
    }
}

impl<'de> Deserialize<'de> for ParamCurve {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<ParamCurve, D::Error> {
        let w = CurveWire::deserialize(de)?;
        let implied = ParamCurve::implied_shape_of(&w.param, w.ease);
        Ok(ParamCurve {
            param: w.param,
            keys: w
                .keys
                .into_iter()
                .map(|k| AutoKey { t: k.t, v: k.v, shape: k.shape.unwrap_or(implied) })
                .collect(),
            source: w.source,
        })
    }
}

/// Source of the curve's values (R.59, item 466).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CurveSource {
    /// Keys placed by a person — the only one working today.
    #[default]
    Keys,
    /// Its own deterministic mini-formula of time and addressable
    /// properties (R.61). Such a curve has no keys.
    Formula { expr: String },
    /// The value is led by ANOTHER property (R.61).
    ///
    /// `path` — the address of the source (see `LinkPath`). The optional
    /// expression `expr` recalculates the read value; in it
    /// the leading property is available under the name `src`, and the material time —
    /// under the name `t` (in seconds).
    Link {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expr: Option<String>,
        /// Thresholds and snapshots for a NON-NUMERIC target (R.61): a flag switches
        /// by a threshold, choice/text/media/colour — by snapshots at the marks of the axis
        /// of the VALUE of the leading property. A rig's axis is a value, not a time,
        /// and they must not be confused.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        stops: Vec<LinkStop>,
    },
}

/// A mark on the axis of the VALUE of the leading property (R.61).
///
/// It differs from a key by its axis: for a key the axis is TIME, here it is the leader's value.
/// Between marks the previous snapshot is held: the rig switches the font and
/// the variant as a step, there is no “half” between variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkStop {
    /// The value of the leading property, from which the snapshot applies.
    pub at: f64,
    /// What to substitute. The type must match the target's type — otherwise validation.
    pub value: ParamValue,
}

impl CurveSource {
    pub fn is_keys(&self) -> bool {
        matches!(self, CurveSource::Keys)
    }

    /// The address of the leading property, if the source is derived.
    pub fn link_path(&self) -> Option<&str> {
        match self {
            CurveSource::Link { path, .. } => Some(path),
            _ => None,
        }
    }

    /// The text of the expression, if there is one (for a link — optional).
    pub fn expr(&self) -> Option<&str> {
        match self {
            CurveSource::Formula { expr } => Some(expr),
            CurveSource::Link { expr, .. } => expr.as_deref(),
            CurveSource::Keys => None,
        }
    }
}

/// Structural check of the clip's automation curves: keys are sorted
/// by material position, the position is non-negative, the values are finite.
/// A key OUTSIDE the piece used by the clip is legal — it is a key hidden by
/// a trim; it will return if the event is stretched again. The parameter
/// name is NOT checked strictly: an orphan curve (a renamed/foreign
/// parameter) silently does not act — schema edits do not break the file's loading.
/// The generator on the event and its curves (R.59, stage 2).
///
/// Strict, no legacy: values exist if and only if the media is a
/// generator, the kind matches the kind in the path, an orphan curve is REJECTED.
/// `fx_auto` tolerates an orphan (a slot could change its kind, and the curve is
/// a hidden piece of a person's work); here there is none: the generator kind of an event is exactly one, and it
/// changes together with the media, so an orphan has nowhere to come from, while a silently
/// non-acting curve would look like “the animation disappeared”.
pub fn check_clip_gen(c: &ClipEvent, media_kind_path: Option<&str>) -> Result<(), ProjectError> {
    let Some(g) = &c.r#gen else {
        return match media_kind_path {
            Some(_) => Err(ProjectError::GenMissing(c.id)),
            None => Ok(()),
        };
    };
    let Some(path) = media_kind_path else { return Err(ProjectError::GenOnFile(c.id)) };
    if crate::genmedia::gen_kind_path(g) != path {
        return Err(ProjectError::GenKindMismatch(c.id));
    }
    for cur in &c.gen_auto {
        check_curve_source(c.id, cur)?;
        let sorted = cur.keys.windows(2).all(|w| w[0].t <= w[1].t);
        let finite = cur.keys.iter().all(|k| k.t.0 >= 0 && k.v.is_finite());
        if !sorted || !finite {
            return Err(ProjectError::BadFxAuto(c.id));
        }
        if g.param_spec(&cur.param).is_none() {
            return Err(ProjectError::GenAutoOrphan(c.id, cur.param.clone()));
        }
    }
    Ok(())
}

/// The source of a curve MUST be RESOLVABLE (R.61).
///
/// A formula is to be parsed; it can have no keys: the curve is set by text
/// entirely, and keys beside it would be a second source of truth about the value.
/// A link — must have an address of the right form and, if present, a parseable expression;
/// the reachability of the address is checked by `links::check_links`, because it
/// depends on the reel, not on a single curve.
pub fn check_curve_source(id: ClipId, c: &ParamCurve) -> Result<(), ProjectError> {
    match &c.source {
        CurveSource::Keys => {}
        // a formula — a LAYER over keys (R.67): the keys beneath it live, `k` is
        // their value with the shapes; without keys `k` = 0 — the former “formula
        // as a source” is precisely a layer over emptiness
        CurveSource::Formula { expr } => {
            check_expr(id, expr, LAYER_VARS, "layer formula")?;
        }
        CurveSource::Link { path, expr, .. } => {
            if crate::links::LinkPath::parse(path).is_none() {
                return Err(ProjectError::CurveSourceBroken(
                    id,
                    format!("link address {path:?} is not of the form <clip>/<owner>/<param>"),
                ));
            }
            if let Some(e) = expr {
                check_expr(id, e, LAYER_VARS, "link formula")?;
            }
        }
    }
    // a SEGMENT formula — its own variables (R.67)
    for k in &c.keys {
        if let SegShape::Formula { expr } = k.shape {
            check_expr(id, &expr.text(), SEGMENT_VARS, "segment formula")?;
        }
    }
    Ok(())
}

/// Variables of the layer and of the source formula: the material time, the leading property,
/// the value of the keys with shapes.
pub const LAYER_VARS: &[&str] = &["t", "src", "k"];
/// Variables of the segment formula: the share of the path, the ends, the seconds from the left key.
pub const SEGMENT_VARS: &[&str] = &["s", "a", "b", "t"];

/// The formula text is parsed and reads only known names — otherwise
/// a LOUD rejection with a name (rule 16): a silent zero would show
/// a picture that the person would believe.
fn check_expr(id: ClipId, expr: &str, vars: &[&str], what: &str) -> Result<(), ProjectError> {
    let f = crate::formula::Formula::parse(expr)
        .map_err(|e| ProjectError::CurveSourceBroken(id, format!("{what}: {e}")))?;
    if let Some(bad) = f.reads.iter().find(|r| !vars.contains(&r.as_str())) {
        return Err(ProjectError::CurveSourceBroken(
            id,
            format!("{what}: no variable is named {bad:?} (known: {})", vars.join(", ")),
        ));
    }
    Ok(())
}

pub fn check_fx_auto(id: ClipId, auto: &[Vec<ParamCurve>]) -> Result<(), ProjectError> {
    for curves in auto {
        for c in curves {
            // A source that does NOT RESOLVE is a LOUD rejection (R.59,
            // item 466; R.61). A silent “draw from the keys” would show
            // something other than what is written in the file, and this is the worst kind of
            // discrepancy: the person sees the picture and believes it.
            check_curve_source(id, c)?;
            let sorted = c.keys.windows(2).all(|w| w[0].t <= w[1].t);
            let ok = c.keys.iter().all(|k| k.t.0 >= 0 && k.v.is_finite());
            if !sorted || !ok {
                return Err(ProjectError::BadFxAuto(id));
            }
            // SPEED accepts only a straight line (R.61). The closed form
            // of the inverse warp map stands on a piecewise-linear profile:
            // the integral of the third order would be of the fourth degree, and from
            // a zigzag it is not expressible at all. The rejection is LOUD — a quiet
            // dropping of the shape would show one thing in the editor and another in the
            // sound.
            if c.param == "rate" && c.keys.iter().any(|k| k.shape != SegShape::Linear) {
                return Err(ProjectError::RateShapeMustBeLinear(id));
            }
        }
    }
    Ok(())
}

/// The value of the curve at the material position `at`. Beyond the end keys —
/// their values (clamping of the edges, as cam_at). The domain is NOT clamped: a position beyond
/// the body of the clip (a transition handle, pre-roll) is legal and honestly continues
/// the curve with its end value.
pub fn curve_value(keys: &[AutoKey], at: Tick) -> f64 {
    let Some(first) = keys.first() else { return 0.0 };
    if at <= first.t {
        return first.v;
    }
    for w in keys.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if at <= b.t {
            let span = (b.t.0 - a.t.0).max(1) as f64;
            let s = (((at.0 - a.t.0) as f64) / span).clamp(0.0, 1.0);
            // the segment's shape is carried by the LEFT key — it decides (R.61);
            // the seconds from it — the variable `t` of the segment formula (R.67)
            return a.shape.at_t(s, a.v, b.v, (at.0 - a.t.0) as f64 / 90_000.0);
        }
    }
    keys.last().unwrap().v
}

/// Apply the slot's curves to an effect at the material position `at`.
/// Returns the number of curves that acted (orphans are not counted).
pub fn apply_fx_auto(fx: &mut FxInstance, curves: &[ParamCurve], at: Tick) -> usize {
    apply_curves(fx, curves, at)
}

/// The same for ANY owner of properties (R.59): the generator kind, transition,
/// camera. One resolution rule for a curve for all carriers — a second one must not be
/// introduced; if they diverged, the “effect” and the “generator” would show
/// different values at the same tick.
pub fn apply_curves<T: HasParams + ?Sized>(t: &mut T, curves: &[ParamCurve], at: Tick) -> usize {
    let mut applied = 0;
    for c in curves {
        if c.keys.is_empty() {
            continue;
        }
        if t.set_param(&c.param, curve_value(&c.keys, at)) {
            applied += 1;
        }
    }
    applied
}

/// The material position of a timeline moment inside a clip — the SINGLE rule
/// for render, automation and UI (the engine then further aligns it to the
/// frame grid; `frame_step` is needed for the reverse mirror). Beyond the body of the clip
/// the value continues linearly and is clamped by the lower bound of the material:
/// the transition handle “holds the frame”, as before.
pub fn clip_material_at(
    source_in: Tick,
    start: Tick,
    duration: Tick,
    reversed: bool,
    frame_step: i64,
    t: Tick,
) -> Tick {
    if reversed {
        // mirror: the last frame of the clip shows source_in, the first —
        // the end of the used piece of the source (minus one frame step)
        Tick((source_in.0 + duration.0 - frame_step - (t.0 - start.0)).max(source_in.0))
    } else {
        Tick((source_in.0 + (t.0 - start.0)).max(0))
    }
}

/// The inverse of `clip_material_at`: the timeline moment at which the event
/// plays the material position `mat`. The pair must live NEXT TO each other — they are correct
/// only together, and `clip_material_at` also clamps the edges.
///
/// R.37: previously the inverse was open in kt-engine and knew the speed warp
/// (`material_at`/`timeline_at_material`). Now the translation “material ↔
/// timeline” DOES NOT DEPEND on the effect chain: speed compresses the material, but
/// not the automation axis (the user's decision, item 1 of R.37).
pub fn clip_timeline_at(
    source_in: Tick,
    start: Tick,
    duration: Tick,
    reversed: bool,
    frame_step: i64,
    mat: Tick,
) -> Tick {
    if reversed {
        Tick(start.0 + (source_in.0 + duration.0 - frame_step - mat.0))
    } else {
        Tick(start.0 + (mat.0 - source_in.0))
    }
}

/// Keyframes of the OLD-style slowed&reverb (`SlowedReverb.automation`,
/// shares of material) → ordinary curves `rate`/`mix` in material ticks.
/// Returns true if the clip changed. Called by the format readers: since R.33
/// the automation of all effects is one, and slowed has no own domain
/// any more.
pub fn migrate_legacy_slowed(c: &mut ClipEvent) -> bool {
    let mut touched = false;
    for (i, f) in c.fx.iter_mut().enumerate() {
        let FxInstance::SlowedReverb { automation, .. } = f else { continue };
        if automation.is_empty() {
            continue;
        }
        let legacy = std::mem::take(automation);
        if c.fx_auto.len() <= i {
            c.fx_auto.resize(i + 1, Vec::new());
        }
        // the share of material for slowed was counted FROM source_in — the reverse mirror
        // is not needed for it (unlike the video curves of the former domain)
        let key = |u: f64, v: f64| {
            AutoKey::at(material_from_progress(u, c.source_in, c.duration, false), v)
        };
        for (param, pick) in [
            ("rate", Box::new(|k: &SlowedKeyframe| k.rate) as Box<dyn Fn(&SlowedKeyframe) -> f64>),
            ("mix", Box::new(|k: &SlowedKeyframe| k.mix as f64)),
        ] {
            if c.fx_auto[i].iter().any(|c| c.param == param) {
                continue; // a curve of the new domain already exists — it takes precedence
            }
            c.fx_auto[i].push(ParamCurve {
                param: param.into(),

                keys: AutoKey::linear(legacy.iter().map(|k| key(k.t, pick(k))).collect()),
                source: Default::default(),
            });
        }
        touched = true;
    }
    touched
}

/// Camera keyframes → common curves (R.34 K3). The domain changes from the share of the
/// event to ticks FROM ITS START, the shape — from “a row with all the fields” to
/// “a curve per parameter”; `ease` moves from the event to each curve.
/// The conversion is one-way (for independent curves the keys stand at different
/// times, they are not assembled back into rows) — therefore `keyframes`
/// becomes a read-only legacy input, like `SlowedReverb.automation`.
pub fn migrate_legacy_camera(c: &mut CameraEvent) -> bool {
    if c.keyframes.is_empty() || !c.auto.is_empty() {
        return false;
    }
    let keys = std::mem::take(&mut c.keyframes);
    let dur = c.duration.0.max(1) as f64;
    let ease = c.ease;
    /// (the parameter name, how to get it from the camera snapshot)
    type Pick = (&'static str, fn(&CameraParams) -> f64);
    let params: &[Pick] = &[
        ("pos_x", |p| p.pos_x),
        ("pos_y", |p| p.pos_y),
        ("pos_z", |p| p.pos_z),
        ("rot_x", |p| p.rot_x),
        ("rot_y", |p| p.rot_y),
        ("rot_z", |p| p.rot_z),
        ("fov_deg", |p| p.fov_deg),
    ];
    // the base parameters are taken from the FIRST key: the curve overrides them
    // entirely, but the non-automatable fields (target/ortho) must remain
    // those that were in the animation
    c.cam = keys[0].cam;
    for (name, pick) in params {
        // flat curves are not created: a camera has seven parameters, and seven
        // straight lines in the editor are junk
        let vals: Vec<f64> = keys.iter().map(|k| pick(&k.cam)).collect();
        if vals.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-12) {
            continue;
        }
        // the former boolean smoothing of the camera becomes ONE shape for
        // all segments: the values after the conversion must coincide bit for bit
        let shape = if ease { SegShape::DEFAULT } else { SegShape::Linear };
        c.auto.push(ParamCurve {
            param: (*name).into(),
            keys: keys
                .iter()
                .zip(&vals)
                .map(|(k, v)| {
                    AutoKey::shaped(Tick((k.t.clamp(0.0, 1.0) * dur).round() as i64), *v, shape)
                })
                .collect(),
            source: Default::default(),
        });
    }
    true
}

/// CamMotion → “Geometry” node + curves (R.34 K2, since R.62 — instead of Pan).
/// The node has the same three parameters (position and zoom), so “CamMotion with
/// keyframes” is “Geometry with curves”; the conversion removes the
/// duplicate animation mechanism rather than porting it. The pan of the former frame
/// (−1..1 within the free space) is converted into the position (fractions of the
/// canvas): `pos = −pan·(1 − 1/zoom)/2` at the zoom of the key.
///
/// Subtleties that are easy to lose: (1) empty keyframes of CamMotion
/// meant NOT static, but the v1 preset “zoom-in 1.0 → 1.35” — it is
/// materialized explicitly, otherwise old projects would lose the motion; (2)
/// `time_scale` sped up the reading of keyframes (`u_eff = u · scale`),
/// so the key's position is divided by it; (3) `ease` lived on the effect, and in
/// the new domain each curve has its own.
/// The pan of the former frame (−1..1 within the free space at zoom z) →
/// the position of the “Geometry” node in canvas fractions.
pub fn pan_to_pos(pan: f64, zoom: f64) -> f64 {
    let z = zoom.clamp(0.05, 32.0);
    -pan.clamp(-4.0, 4.0) * (1.0 - 1.0 / z) / 2.0
}

pub fn migrate_legacy_cammotion(c: &mut ClipEvent) -> bool {
    let mut touched = false;
    for i in 0..c.fx.len() {
        let FxInstance::CamMotion { mode, ease, time_scale, keyframes } = &c.fx[i] else {
            continue;
        };
        let (mode, ease, scale) = (*mode, *ease, time_scale.clamp(0.05, 8.0));
        let keys: Vec<CamKeyframe> = if keyframes.is_empty() {
            vec![
                CamKeyframe { t: 0.0, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 },
                CamKeyframe { t: 1.0, pan_x: 0.0, pan_y: 0.0, zoom: 1.35 },
            ]
        } else {
            keyframes.clone()
        };
        let first = keys[0];
        let _ = mode; // the mode of the former frame: all three gave one frame (F-MED-004)
        c.fx[i] = FxInstance::geometry_at(
            pan_to_pos(first.pan_x, first.zoom),
            pan_to_pos(first.pan_y, first.zoom),
            first.zoom,
        );
        if c.fx_auto.len() <= i {
            c.fx_auto.resize(i + 1, Vec::new());
        }
        for (param, pick) in [
            (
                "pos_x",
                Box::new(|k: &CamKeyframe| pan_to_pos(k.pan_x, k.zoom))
                    as Box<dyn Fn(&CamKeyframe) -> f64>,
            ),
            ("pos_y", Box::new(|k: &CamKeyframe| pan_to_pos(k.pan_y, k.zoom))),
            ("zoom", Box::new(|k: &CamKeyframe| k.zoom)),
        ] {
            if c.fx_auto[i].iter().any(|c| c.param == param) {
                continue; // a curve of the new domain already exists — it takes precedence
            }
            // the same rule as for the camera: the boolean smoothing of the legacy
            // becomes one shape for all segments
            let shape = if ease { SegShape::DEFAULT } else { SegShape::Linear };
            c.fx_auto[i].push(ParamCurve {
                param: param.into(),
                keys: keys
                    .iter()
                    .map(|k| {
                        // u_eff = u·scale ⇒ the key is reached at u = t/scale
                        AutoKey::shaped(
                            material_from_progress(k.t / scale, c.source_in, c.duration, false),
                            pick(k),
                            shape,
                        )
                    })
                    .collect(),
                source: Default::default(),
            });
        }
        touched = true;
    }
    touched
}

/// The key position from the OLD domain (a share 0..1 of the event length) into
/// material ticks. The domain changed in R.33; the format reader calls this
/// for files before v3, otherwise the animation of old projects would move away.
///
/// `mirror` = true for curves whose share was counted by the TIMELINE progress
/// (video `ParamCurve` before R.33): for a reversed clip it is mirrored relative to the
/// material. For slowed&reverb keyframes the share from the start was counted
/// BY THE MATERIAL, they do not need a mirror.
pub fn material_from_progress(u: f64, source_in: Tick, duration: Tick, mirror: bool) -> Tick {
    let u = u.clamp(0.0, 1.0);
    let u = if mirror { 1.0 - u } else { u };
    Tick(source_in.0 + (u * duration.0 as f64).round() as i64)
}

/// Typed FX (not opaque JSON — validation on load, test 12).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FxInstance {
    Gain {
        db: f32,
        /// Channel balance −1…+1 (R.33 C5). 0 — the former path byte-for-byte:
        /// the multipliers of both channels exactly 1.0.
        #[serde(default, skip_serializing_if = "is_zero")]
        pan: f32,
    },
    /// The blend strength of the LAYER (R.38 item 12): mixing of the chain's input with its
    /// result. Not a user effect — it is absent from the registry and cannot be
    /// picked in the picker; `MorphPatch::apply_to` appends it when the
    /// morpher's strength < 1 or fades are set.
    ///
    /// Why a CHAIN operation and not a clip field: “the layer's opacity works for free as a slider
    /// of the effect's strength — 60 % is a crossfade of the processed picture with the unprocessed one”
    /// (reconnaissance of R.36,
    /// `docs/features/morpher/analogues/premiere-adjustment-layer.md`).
    /// As a chain slot it gets automation for free (fades are its curve),
    /// it enters `fx_chain_hash` (otherwise 60 % and 100 % would collapse
    /// into one frame of the post-FX cache) and travels through copy-paste.
    LayerMix {
        amount: f64,
        /// The index of the slot FROM WHICH this layer's contribution starts. Mixing goes
        /// with the state BEFORE it, not with the input of the whole chain: otherwise
        /// the strength of the layer would also roll back the event's OWN effects,
        /// to which the layer has no relation.
        #[serde(default)]
        from: u32,
    },
    SlowedReverb {
        /// Base speed (0.25–32); automation overrides it.
        rate: f64,
        /// Wet-signal share 0..1.
        mix: f32,
        reverb_delay_ms: f32,
        /// High-frequency damping in the comb lines 0..1 (the old reverbDamp).
        #[serde(default = "default_damp")]
        damp: f32,
        /// Feedback of the comb lines 0..0.98 (the old reverbDecay).
        #[serde(default = "default_decay")]
        decay: f32,
        /// Wet-output multiplier (the old reverbGain).
        #[serde(default = "default_wet_gain")]
        wet_gain: f32,
        /// Pre-delay of the reverb input, ms (R.33 C5): the tail lags behind the
        /// sound. 0 — the former behaviour byte-for-byte; structural (changes the
        /// line length), therefore not subject to automation.
        #[serde(default, skip_serializing_if = "is_zero")]
        predelay_ms: f32,
        /// How to obtain an intermediate frame when the effect stands on VIDEO
        /// (R.37 item 2). For an audio instance the field is not visible — it means
        /// nothing to it. The default is not serialized: files are byte-stable.
        #[serde(default, skip_serializing_if = "is_default_frame_mode")]
        frames: FrameMode,
        /// Width of the stereo tail 0..1. 1 — the former stereo reverb.
        #[serde(default = "default_one_f32", skip_serializing_if = "is_one_f32")]
        width: f32,
        /// Keyframe automation: t — the position IN THE SOURCE MATERIAL
        /// (frames from source_in, normalized by the clip duration), NOT
        /// the output time. A keyframe is glued to the material: early edits
        /// do not shift the material of later points (item 1 of v1.3, test 31).
        /// Between points linear interpolation, beyond the edges — the end ones.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        automation: Vec<SlowedKeyframe>,
    },
    /// Resonant filter (R.47): LP/HP/BP, RBJ biquad.
    Filter {
        #[serde(default)]
        mode: FilterMode,
        #[serde(default = "default_cutoff_hz")]
        cutoff_hz: f32,
        #[serde(default = "default_resonance")]
        resonance: f32,
    },
    /// Three-band equaliser (R.47): shelf + peak + shelf.
    Eq {
        #[serde(default, skip_serializing_if = "is_zero")]
        low_gain_db: f32,
        #[serde(default = "default_low_freq")]
        low_freq_hz: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        mid_gain_db: f32,
        #[serde(default = "default_mid_freq")]
        mid_freq_hz: f32,
        #[serde(default = "default_one_f32", skip_serializing_if = "is_one_f32")]
        mid_q: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        high_gain_db: f32,
        #[serde(default = "default_high_freq")]
        high_freq_hz: f32,
    },
    /// Soft saturation (R.47): softclip with level compensation.
    Distortion {
        #[serde(default = "default_drive_db")]
        drive_db: f32,
        #[serde(default = "default_half_f32")]
        tone: f32,
        #[serde(default = "default_one_f32", skip_serializing_if = "is_one_f32")]
        mix: f32,
    },
    /// Lo-fi degradation (R.47): bit depth + sample hold.
    Bitcrusher {
        #[serde(default = "default_bits")]
        bits: f32,
        #[serde(default = "default_crush_rate")]
        rate_hz: f32,
        #[serde(default = "default_one_f32", skip_serializing_if = "is_one_f32")]
        mix: f32,
    },
    /// Stereo width m/s (R.47): 0 — mono, 1 — as recorded, 2 — wider.
    StereoWidth {
        #[serde(default = "default_one_f32")]
        width: f32,
    },
    /// Tremolo (R.47): volume LFO; the phase is the integral of the frequency curve over
    /// the position of the clip (determinism without warm-up).
    Tremolo {
        #[serde(default = "default_trem_rate")]
        rate_hz: f32,
        #[serde(default = "default_trem_depth")]
        depth: f32,
    },
    /// Autopan (R.47): balance LFO, the pan law — as for Gain.pan.
    AutoPan {
        #[serde(default = "default_one_f32")]
        rate_hz: f32,
        #[serde(default = "default_autopan_depth")]
        depth: f32,
    },
    /// Vibrato (R.47): a modulated delay, 100 % wet (with dry it would be
    /// a chorus) — the mix parameter is deliberately absent.
    Vibrato {
        #[serde(default = "default_vib_rate")]
        rate_hz: f32,
        #[serde(default = "default_vib_depth")]
        depth_ms: f32,
    },
    /// Chorus (R.47): 1-3 voices on modulated delays, stereo —
    /// a phase shift of the LFO by 90° in the right channel.
    Chorus {
        #[serde(default = "default_chorus_rate")]
        rate_hz: f32,
        #[serde(default = "default_chorus_depth")]
        depth_ms: f32,
        #[serde(default = "default_half_f32")]
        mix: f32,
        #[serde(default = "default_voices")]
        voices: u32,
    },
    /// Flanger (R.47): a short modulated delay with feedback
    /// (negative — phase inversion).
    Flanger {
        #[serde(default = "default_flanger_rate")]
        rate_hz: f32,
        #[serde(default = "default_flanger_depth")]
        depth_ms: f32,
        #[serde(default = "default_flanger_fb")]
        feedback: f32,
        #[serde(default = "default_half_f32")]
        mix: f32,
    },
    /// Phaser (R.47): a cascade of 1-pole allpass with a sweep 200..2000 Hz.
    Phaser {
        #[serde(default = "default_phaser_rate")]
        rate_hz: f32,
        #[serde(default = "default_phaser_depth")]
        depth: f32,
        #[serde(default = "default_phaser_fb")]
        feedback: f32,
        #[serde(default = "default_stages")]
        stages: u32,
        #[serde(default = "default_half_f32")]
        mix: f32,
    },
    /// Echo (R.47): a ring at maximum, the time — fractional reading with
    /// tape glide (not structural), a filter in the loop — the tail darkens.
    Delay {
        #[serde(default = "default_delay_time")]
        time_ms: f32,
        #[serde(default = "default_delay_fb")]
        feedback: f32,
        #[serde(default = "default_delay_mix")]
        mix: f32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        pingpong: bool,
    },
    /// Compressor (R.47): peak detector with stereo link.
    Compressor {
        #[serde(default = "default_comp_thr")]
        threshold_db: f32,
        #[serde(default = "default_comp_ratio")]
        ratio: f32,
        #[serde(default = "default_comp_att")]
        attack_ms: f32,
        #[serde(default = "default_comp_rel")]
        release_ms: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        makeup_db: f32,
        /// The SIDECHAIN KEY (R.65): the track along which the detector computes.
        /// `None` — an ordinary compressor, and then the field is not written at all:
        /// a file without a sidechain stays byte-for-byte the same.
        ///
        /// The address is a `TrackId`, NOT a track number: the number is the rank of the row, and
        /// inserting a track above would move the key onto someone else's signal (the same
        /// lesson as the FX-state key in R.62).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<TrackId>,
    },
    /// Gate (R.47): a fast detector + −6 dB hysteresis + hold.
    Gate {
        #[serde(default = "default_gate_thr")]
        threshold_db: f32,
        #[serde(default = "default_gate_att")]
        attack_ms: f32,
        #[serde(default = "default_gate_hold")]
        hold_ms: f32,
        #[serde(default = "default_gate_rel")]
        release_ms: f32,
    },
    // ---- video FX (port of bc_fx.cpp, item 25; parameters and defaults of v1) ----------
    /// Basic colour correction (R.47, F-CLR-007): brightness/contrast/
    /// saturation/hue. Point-wise — absorbed into the merged geometry pass.
    ColorGrade {
        #[serde(default, skip_serializing_if = "is_zero")]
        brightness: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        contrast: f32,
        #[serde(default = "default_one_f32", skip_serializing_if = "is_one_f32")]
        saturation: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        hue: f32,
    },
    /// Two paints by luma (R.47): sepia/tint/invert — points of this
    /// space. Point-wise.
    Duotone {
        #[serde(default = "default_duo_sr")]
        shadow_r: u8,
        #[serde(default = "default_duo_sg")]
        shadow_g: u8,
        #[serde(default = "default_duo_sb")]
        shadow_b: u8,
        #[serde(default = "default_duo_hr")]
        high_r: u8,
        #[serde(default = "default_duo_hg")]
        high_g: u8,
        #[serde(default = "default_duo_hb")]
        high_b: u8,
        #[serde(default = "default_one_f32")]
        amount: f32,
    },
    /// Posterize by channels (R.47). Point-wise (inside a geometry series
    /// supersampling softens the steps — accepted, THRESHOLD).
    Posterize {
        #[serde(default = "default_levels")]
        levels: u32,
    },
    /// Vignette (R.47): darkening towards the edges, the coordinate of its own stage —
    /// inside the chain it is zoomed together with the picture. Point-wise.
    Vignette {
        #[serde(default = "default_half_f32")]
        strength: f32,
        #[serde(default = "default_vig_radius")]
        radius: f32,
        #[serde(default = "default_vig_soft")]
        softness: f32,
    },
    /// Four-corner fit (R.47, F-FX-004): homography quad→quad,
    /// the main tool of “the screen in the frame”. Geometry — concatenated.
    CornerPin {
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        x0: f64,
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        y0: f64,
        #[serde(default = "default_one_f64", skip_serializing_if = "is_one_f64")]
        x1: f64,
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        y1: f64,
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        x2: f64,
        #[serde(default = "default_one_f64", skip_serializing_if = "is_one_f64")]
        y2: f64,
        #[serde(default = "default_one_f64", skip_serializing_if = "is_one_f64")]
        x3: f64,
        #[serde(default = "default_one_f64", skip_serializing_if = "is_one_f64")]
        y3: f64,
    },
    /// Unsharp masking by mip (R.47, F-FX-001): source +
    /// (source − blurred base)·amount. Barrier.
    Sharpen {
        #[serde(default = "default_one_f32")]
        amount: f32,
        #[serde(default = "default_two_u32")]
        radius: u32,
    },
    /// Mosaic (R.47, F-FX-001): an integer copy of the cell's centre.
    /// Barrier.
    Pixelate {
        #[serde(default = "default_twelve_u32")]
        size: u32,
    },
    /// Chromatic aberration (R.47, F-FX-002): radial scale of
    /// channels — R towards the centre, B from the centre, G in place. Barrier.
    ChromaticAberration {
        #[serde(default = "default_chroma_amount")]
        amount: f32,
    },
    /// Radial blur (R.47, F-FX-003): zoom streak or rotation
    /// around the centre, constant 16 taps on both backends. Barrier.
    RadialBlur {
        #[serde(default = "default_quarter_f32")]
        amount: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        center_x: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        center_y: f32,
        #[serde(default, skip_serializing_if = "is_false")]
        spin: bool,
    },
    /// Shadow under the layer (R.47, F-FX-003): a blurred alpha with an offset,
    /// the source OVER the shadow. Barrier; an input without alpha — passthrough.
    DropShadow {
        #[serde(default = "default_eight_i32")]
        offset_x: i32,
        #[serde(default = "default_eight_i32")]
        offset_y: i32,
        #[serde(default = "default_four_u32")]
        radius: u32,
        #[serde(default = "default_shadow_opacity")]
        opacity: f32,
        #[serde(default, skip_serializing_if = "is_zero_u8")]
        tint_r: u8,
        #[serde(default, skip_serializing_if = "is_zero_u8")]
        tint_g: u8,
        #[serde(default, skip_serializing_if = "is_zero_u8")]
        tint_b: u8,
    },
    /// Halftone raster (R.47, F-FX-001): dots by luma on a rotated
    /// grid; the pattern lives in logical coordinates. Barrier.
    Halftone {
        #[serde(default = "default_eight_u32")]
        size: u32,
        #[serde(default = "default_halftone_angle")]
        angle: f32,
    },
    /// Look-ahead limiter (R.48, F-AUD-002): the ceiling is guaranteed by the look-ahead
    /// window — the peak is seen in advance, so the attack is instant and without
    /// distortion. For the rest of the engine the delay is ZERO: the chain reads the
    /// source ahead, the output stays aligned.
    Limiter {
        #[serde(default = "default_limiter_threshold")]
        threshold_db: f32,
        #[serde(default = "default_limiter_ceiling")]
        ceiling_db: f32,
        /// The look-ahead window. A STRUCTURAL parameter: the length of the ring is part of
        /// the node's device, a change rebuilds the processor (R.48, item 337).
        #[serde(default = "default_limiter_lookahead")]
        lookahead_ms: f32,
        #[serde(default = "default_limiter_release")]
        release_ms: f32,
    },
    /// Pitch change without changing duration (R.48, F-AUD-013). The engine
    /// (WSOLA or phase vocoder) is a PROJECT SETTING: one choice for the
    /// whole edit. Neutral (zero semitones and cents, no curves) gives the
    /// previous sound, byte for byte.
    PitchShift {
        #[serde(default, skip_serializing_if = "is_zero")]
        semitones: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        cents: f32,
    },
    /// Lens distortion (R.47, F-FX-002): polynomial k1·r²+k2·r⁴ —
    /// barrel/pincushion. Geo (Warp prim), concatenated.
    LensDistort {
        #[serde(default = "default_lens_k1")]
        k1: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        k2: f32,
    },
    /// Swirl (R.47): rotation that fades toward the radius. Geo.
    Twirl {
        #[serde(default = "default_twirl_angle")]
        angle: f32,
        #[serde(default = "default_twirl_radius")]
        radius: f32,
    },
    /// Circular wave (R.47): r += amp·sin(k·r − φ), phase animatable.
    /// Geo.
    Ripple {
        #[serde(default = "default_ripple_amount")]
        amount: f32,
        #[serde(default = "default_ripple_freq")]
        freq: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        phase: f32,
    },
    /// Kaleidoscope (R.47): mirror repeat of a sector. Geo.
    Kaleidoscope {
        #[serde(default = "default_kaleido_segments")]
        segments: u32,
        #[serde(default, skip_serializing_if = "is_zero")]
        angle: f32,
    },
    /// Cylinder (R.47, F-FX-004): bend along X, edges remain edges. Geo,
    /// category 3D.
    Cylinder3d {
        #[serde(default = "default_half_f32")]
        curvature: f32,
    },
    /// Node “Geometry” (R.62): crop (input fractions), position (canvas fractions),
    /// zoom, rotation around the anchor; outside the frame — TRANSPARENT. The leading
    /// node of an event lives in `ClipEvent.geometry` and also carries a conform
    /// rule; this variant is the extra nodes from the catalog and the form in which
    /// the leading node reaches kt-fx.
    Geometry {
        /// Leading node of an event (R.62): materialized by the engine from
        /// `ClipEvent.geometry`, carries the conform rule via the kt-fx context.
        /// Not stored in the model chain (`fx`) — there it is always `false`.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        lead: bool,
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        crop_l: f64,
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        crop_t: f64,
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        crop_r: f64,
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        crop_b: f64,
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        pos_x: f64,
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        pos_y: f64,
        #[serde(default = "default_zoom", skip_serializing_if = "is_one")]
        zoom: f64,
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        rot: f64,
        #[serde(default = "default_half", skip_serializing_if = "is_half")]
        anchor_x: f64,
        #[serde(default = "default_half", skip_serializing_if = "is_half")]
        anchor_y: f64,
    },
    /// Pan/zoom by keyframes of clip progress (smoothstep when eased).
    CamMotion {
        #[serde(default)]
        mode: PanMode,
        #[serde(default = "default_true")]
        ease: bool,
        #[serde(default = "default_one")]
        time_scale: f64,
        #[serde(default)]
        keyframes: Vec<CamKeyframe>,
    },
    Mirror {
        #[serde(default = "default_true")]
        horizontal: bool,
        #[serde(default)]
        vertical: bool,
    },
    Blur {
        #[serde(default = "default_blur_radius")]
        radius: u32,
        /// Vertical radius (R.33 stage 5). `0` means “same as horizontal”: the default is not written
        /// to the file and gives the previous blur byte for byte. Separate axes give
        /// a smear along one axis.
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        radius_y: u32,
    },
    Greenscreen {
        #[serde(default)]
        key_r: u8,
        #[serde(default = "default_255")]
        key_g: u8,
        #[serde(default)]
        key_b: u8,
        #[serde(default = "default_tolerance")]
        tolerance: u32,
        #[serde(default = "default_softness")]
        softness: u32,
        /// Key-colour spill suppression (R.33, stage 5): the dominant channel of the key
        /// is pulled toward its neighbours. 0 — previous behaviour.
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        spill: f64,
    },
    Edges {
        #[serde(default = "default_edges_threshold")]
        threshold: u32,
        /// Line thickness = Sobel sampling step (R.33, stage 5). 1 — the previous
        /// 3×3 kernel, byte for byte.
        #[serde(default = "default_one_u32", skip_serializing_if = "is_one_u32_v")]
        thickness: u32,
        /// Line colour: the grey output is multiplied by it. 255,255,255 — the
        /// previous white.
        #[serde(default = "default_255", skip_serializing_if = "is_255")]
        color_r: u8,
        #[serde(default = "default_255", skip_serializing_if = "is_255")]
        color_g: u8,
        #[serde(default = "default_255", skip_serializing_if = "is_255")]
        color_b: u8,
    },
    Noise {
        #[serde(default = "default_noise_amount")]
        amount: f64,
        /// Grain size in pixels (R.33, stage 5). 1 — the previous pixel grain,
        /// byte for byte; larger values give coarse “film” grain.
        #[serde(default = "default_one_u32", skip_serializing_if = "is_one_u32_v")]
        size: u32,
        /// Coloured noise: each channel has its own offset. `false` — the previous
        /// monochrome one (a single shift for all three channels).
        #[serde(default, skip_serializing_if = "is_false")]
        color: bool,
    },
    Vhs {
        #[serde(default = "default_rgb_shift")]
        rgb_shift: u32,
        #[serde(default = "default_bleed")]
        bleed: f64,
        /// Scan-line step (R.33, stage 5): every N-th row is darkened. 4 —
        /// previous behaviour byte for byte, 0 — no scan lines at all.
        #[serde(default = "default_scan_step", skip_serializing_if = "is_four_u32")]
        scan_step: u32,
        /// Horizontal row jitter, fraction of the frame. 0 — the previous
        /// steady frame.
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        jitter: f64,
    },
    Glow {
        #[serde(default = "default_glow_strength")]
        strength: f64,
        /// Glow radius = mip divisor (R.33, stage 5). 4 — the previous one.
        #[serde(default = "default_glow_radius", skip_serializing_if = "is_four_u32")]
        radius: u32,
        /// Brightness threshold of the BLURRED frame: below it there is no glow. 0 —
        /// no threshold (previous behaviour byte for byte).
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        threshold: u32,
        /// Glow tint; 255,255,255 — the previous neutral one.
        #[serde(default = "default_255", skip_serializing_if = "is_255")]
        tint_r: u8,
        #[serde(default = "default_255", skip_serializing_if = "is_255")]
        tint_g: u8,
        #[serde(default = "default_255", skip_serializing_if = "is_255")]
        tint_b: u8,
    },
    EventGrid {
        #[serde(default = "default_one_u32")]
        cols: u32,
        #[serde(default = "default_one_u32")]
        rows: u32,
        #[serde(default = "default_true")]
        center: bool,
    },
    // ---- 3D effects (R.29: category Video3d; since R.44 both backends compute them) ----
    /// Perspective layer: the clip frame is a quad in the 3D scene, rendered by the
    /// active (or pinned) camera to the project size. All defaults are identity:
    /// without a camera and with zero parameters the frame matches the frame without
    /// the effect. The units are frozen by the format: position in world px (X right,
    /// Y down, Z into depth, origin at the centre of the project frame), rotations in
    /// degrees (ZXY order, as orientation in AE), anchor in layer fractions (0.5, 0.5
    /// is the centre).
    Pan3d {
        #[serde(default)]
        pos_x: f64,
        #[serde(default)]
        pos_y: f64,
        #[serde(default)]
        pos_z: f64,
        #[serde(default)]
        rot_x: f64,
        #[serde(default)]
        rot_y: f64,
        #[serde(default)]
        rot_z: f64,
        #[serde(default = "default_half")]
        anchor_x: f64,
        #[serde(default = "default_half")]
        anchor_y: f64,
        #[serde(default = "default_one")]
        scale: f64,
        /// None — the camera active at the time (default, as in AE);
        /// Some(id) — a pinned camera (there may be several cameras — R.29).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        camera: Option<CameraId>,
    },
}

/// Effect class (R.29): a property of the variant's CODE, not of the data —
/// not serialized to the format. Video3d is a CATEGORY (FX picker, “3D”), not a backend flag:
/// since R.44 Pan3d is computed on CPU too (inverse homography in the merged geo pass,
/// kt-fx::geom), and the GpuRequired export refusal is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxClass {
    Audio,
    Video2d,
    Video3d,
}

impl FxInstance {
    pub fn class(&self) -> FxClass {
        match self {
            FxInstance::Gain { .. }
            | FxInstance::SlowedReverb { .. }
            | FxInstance::Filter { .. }
            | FxInstance::Eq { .. }
            | FxInstance::Distortion { .. }
            | FxInstance::Bitcrusher { .. }
            | FxInstance::StereoWidth { .. }
            | FxInstance::Tremolo { .. }
            | FxInstance::AutoPan { .. }
            | FxInstance::Vibrato { .. }
            | FxInstance::Chorus { .. }
            | FxInstance::Flanger { .. }
            | FxInstance::Phaser { .. }
            | FxInstance::Delay { .. }
            | FxInstance::Compressor { .. }
            | FxInstance::Gate { .. }
            | FxInstance::Limiter { .. }
            | FxInstance::PitchShift { .. } => FxClass::Audio,
            FxInstance::Pan3d { .. } | FxInstance::Cylinder3d { .. } => FxClass::Video3d,
            _ => FxClass::Video2d,
        }
    }

    /// Audio effect? (video FX live in the same clip chain but are handled by the
    /// VideoRenderer).
    pub fn is_audio(&self) -> bool {
        self.class() == FxClass::Audio
    }

    /// Layer blend strength — not a filter but a rule for MIXING the chain input
    /// with its output. Like the warp, it applies to BOTH kinds: a layer at 30 %
    /// must equally attenuate both the picture and the sound (“video and audio
    /// at once” — the user's decision on item 12).
    pub fn is_layer_mix(&self) -> bool {
        matches!(self, FxInstance::LayerMix { .. })
    }

    /// Stable variant tag (matches the serde `kind`). An EXHAUSTIVE
    /// match is the only compile-time lock of the registry (R.47, item 326): a new
    /// variant will not build until it is named, and the `all()` test checks the registry
    /// against the set of tags.
    pub fn tag(&self) -> &'static str {
        match self {
            FxInstance::Gain { .. } => "gain",
            FxInstance::LayerMix { .. } => "layer_mix",
            FxInstance::SlowedReverb { .. } => "slowed_reverb",
            FxInstance::Filter { .. } => "filter",
            FxInstance::Eq { .. } => "eq",
            FxInstance::Distortion { .. } => "distortion",
            FxInstance::Bitcrusher { .. } => "bitcrusher",
            FxInstance::StereoWidth { .. } => "stereo_width",
            FxInstance::Tremolo { .. } => "tremolo",
            FxInstance::AutoPan { .. } => "auto_pan",
            FxInstance::Vibrato { .. } => "vibrato",
            FxInstance::Chorus { .. } => "chorus",
            FxInstance::Flanger { .. } => "flanger",
            FxInstance::Phaser { .. } => "phaser",
            FxInstance::Delay { .. } => "delay",
            FxInstance::Compressor { .. } => "compressor",
            FxInstance::Gate { .. } => "gate",
            FxInstance::Limiter { .. } => "limiter",
            FxInstance::PitchShift { .. } => "pitch_shift",
            FxInstance::ColorGrade { .. } => "color_grade",
            FxInstance::Duotone { .. } => "duotone",
            FxInstance::Posterize { .. } => "posterize",
            FxInstance::Vignette { .. } => "vignette",
            FxInstance::CornerPin { .. } => "corner_pin",
            FxInstance::Sharpen { .. } => "sharpen",
            FxInstance::Pixelate { .. } => "pixelate",
            FxInstance::ChromaticAberration { .. } => "chromatic_aberration",
            FxInstance::RadialBlur { .. } => "radial_blur",
            FxInstance::DropShadow { .. } => "drop_shadow",
            FxInstance::Halftone { .. } => "halftone",
            FxInstance::LensDistort { .. } => "lens_distort",
            FxInstance::Twirl { .. } => "twirl",
            FxInstance::Ripple { .. } => "ripple",
            FxInstance::Kaleidoscope { .. } => "kaleidoscope",
            FxInstance::Cylinder3d { .. } => "cylinder3d",
            FxInstance::Geometry { .. } => "geometry",
            FxInstance::CamMotion { .. } => "cam_motion",
            FxInstance::Mirror { .. } => "mirror",
            FxInstance::Blur { .. } => "blur",
            FxInstance::Greenscreen { .. } => "greenscreen",
            FxInstance::Edges { .. } => "edges",
            FxInstance::Noise { .. } => "noise",
            FxInstance::Vhs { .. } => "vhs",
            FxInstance::Glow { .. } => "glow",
            FxInstance::EventGrid { .. } => "event_grid",
            FxInstance::Pan3d { .. } => "pan3d",
        }
    }

    /// Registry of ALL effect kinds with default parameters (analogue of
    /// `TransitionKind::ALL`, R.47 item 326). It cannot be expressed as a constant —
    /// variants carry a Vec (keyframes, automation), so OnceLock is used.
    /// Parameterized tests (spec completeness, golden and conformance loops) are built
    /// on it: a kind not listed here is visible at once.
    pub fn all() -> &'static [FxInstance] {
        static ALL: std::sync::OnceLock<Vec<FxInstance>> = std::sync::OnceLock::new();
        ALL.get_or_init(|| {
            vec![
                FxInstance::gain(0.0),
                FxInstance::LayerMix { amount: 1.0, from: 0 },
                FxInstance::slowed_reverb(0.85, 0.35, 100.0),
                FxInstance::filter(),
                FxInstance::eq(),
                FxInstance::distortion(),
                FxInstance::bitcrusher(),
                FxInstance::stereo_width(),
                FxInstance::tremolo(),
                FxInstance::auto_pan(),
                FxInstance::vibrato(),
                FxInstance::chorus(),
                FxInstance::flanger(),
                FxInstance::phaser(),
                FxInstance::delay(),
                FxInstance::compressor(),
                FxInstance::gate(),
                FxInstance::limiter(),
                FxInstance::pitch_shift(),
                FxInstance::color_grade(),
                FxInstance::duotone(),
                FxInstance::posterize(),
                FxInstance::vignette(),
                FxInstance::corner_pin(),
                FxInstance::sharpen(),
                FxInstance::pixelate(),
                FxInstance::chromatic_aberration(),
                FxInstance::radial_blur(),
                FxInstance::drop_shadow(),
                FxInstance::halftone(),
                FxInstance::lens_distort(),
                FxInstance::twirl(),
                FxInstance::ripple(),
                FxInstance::kaleidoscope(),
                FxInstance::cylinder3d(),
                FxInstance::geometry(),
                FxInstance::cam_motion(),
                FxInstance::mirror(),
                FxInstance::blur(8),
                FxInstance::greenscreen([0, 255, 0], 70, 25),
                FxInstance::edges(48),
                FxInstance::noise(0.3),
                FxInstance::vhs(4, 0.4),
                FxInstance::glow(0.65),
                FxInstance::event_grid(),
                FxInstance::pan3d(),
            ]
        })
    }

    /// The effect WARPS time: source consumption stops being equal to the event
    /// length. Such a clip is not cut by the morpher layer at all (finding 2 of
    /// R.35) — a linearly computed `source_in` of the piece would point to the wrong place.
    pub fn warps_time(&self) -> bool {
        matches!(self, FxInstance::SlowedReverb { .. })
    }

    /// Variant parameters (R.33): ONE source of truth for automation, clamping and
    /// UI. Previously three hand-written tables (`param_specs`, `set_param`, `get_param`)
    /// had to be edited in sync; now all three are generated from one `fx_params!`
    /// declaration.
    pub fn param_specs(&self) -> &'static [ParamSpec] {
        fx_specs(self)
    }

    /// Only automatable parameters (`ParamKind::Smooth`). A curve on a structural
    /// parameter makes no sense: changing it rebuilds the DSP (the reverb recreates the
    /// comb lines), while automation must be smooth — so the UI does not offer such
    /// parameters.
    pub fn auto_params(&self) -> impl Iterator<Item = &'static ParamSpec> {
        self.param_specs().iter().filter(|s| s.kind == ParamKind::Smooth)
    }

    pub fn param_spec(&self, name: &str) -> Option<&'static ParamSpec> {
        self.param_specs().iter().find(|s| s.name == name)
    }

    /// Set a parameter by name. The value is clamped to the spec range; integer
    /// fields are rounded. false — the name does not fit the variant (an orphan
    /// curve).
    pub fn set_param(&mut self, name: &str, v: f64) -> bool {
        let Some(spec) = self.param_spec(name) else { return false };
        fx_set_param(self, name, v.clamp(spec.min, spec.max))
    }

    /// Current value of a parameter by name (for the automation UI).
    pub fn get_param(&self, name: &str) -> Option<f64> {
        fx_get_param(self, name)
    }

    /// Blend two states of ONE kind: numeric parameters linearly, everything else
    /// from the nearer end (R.54, item 433).
    ///
    /// Why this belongs to the model, not the UI: the force axis in the picker moves the cursor
    /// BETWEEN neighbouring variants, and “how much is set here” is shown by intermediate frames.
    /// Declaring a “main parameter” for each kind (like the `Option` skimming in FCP) is neither
    /// needed nor harmful for this: `VHS` and `Color Grade` have four such parameters each,
    /// so choosing one would be arbitrary; mixing two CURATED points
    /// gives a meaningful move
    /// across all the knobs at once.
    ///
    /// Non-numeric values (checkboxes, modes, camera, keyframes) cannot be interpolated,
    /// so the NEAREST end is the basis: up to the midpoint — `self`, beyond it — `other`.
    /// So it switches exactly at the midpoint,
    /// not “sometime”.
    ///
    /// Different kinds are not mixed: the nearer end is returned as is.
    /// `t` is clamped to 0..1; `t == 0` and `t == 1` give the ENDS BYTE FOR BYTE —
    /// the rule “a click on a cell gives exactly its variant” rests on this.
    pub fn lerp(&self, other: &FxInstance, t: f64) -> FxInstance {
        let t = t.clamp(0.0, 1.0);
        let near = if t < 0.5 { self } else { other };
        if std::mem::discriminant(self) != std::mem::discriminant(other) {
            return near.clone();
        }
        if t == 0.0 || t == 1.0 {
            return near.clone();
        }
        let mut out = near.clone();
        for s in self.param_specs() {
            let (Some(a), Some(b)) = (self.get_param(&s.name), other.get_param(&s.name)) else {
                continue;
            };
            out.set_param(&s.name, a + (b - a) * t);
        }
        out
    }
}

/// How the parameter behaves under automation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// Changes smoothly on the fly — automatable.
    Smooth,
    /// A change rebuilds the DSP state (the delay line geometry) — it is not
    /// automatable, can be changed by hand.
    Structural,
}

/// Unit of measure — for the label and the UI step (the label translation lives
/// in the kt-app catalogs: kt-model knows nothing about the interface language).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamUnit {
    None,
    Db,
    /// Fraction 0..1.
    Norm,
    /// Multiplier (speed, zoom).
    Ratio,
    Px,
    Deg,
    Ms,
    /// Frequency, hertz (R.47: filters/EQ/LFO).
    Hz,
    /// Semitone (R.48: pitch).
    St,
    /// Second (R.66): key time, camera duration — window values that have a unit
    /// but no effect passport.
    Sec,
    /// Colour component 0..255.
    Rgb8,
    /// Countable number (columns, rows).
    Count,
    /// Fraction 0..1, DISPLAYED as a percentage (R.53, item 415). It is still stored and
    /// serialized as a fraction — only the display changes: “font size 0.070” says nothing
    /// to the user, “7.0 %” says everything.
    /// The conversion lives in ONE place (`widgets::param_value`).
    Percent,
}

/// TYPE of a property (R.59, item 466). Until this round the only property was a
/// NUMBER, while everything else — flags, lists, colour (stored as a triple of numbers),
/// text, a media reference — was drawn by hand in its own window and took no part in
/// automation, copy-paste or presets.
///
/// Curves animate `Num` and `Color` (per channel); the rest changes in a
/// STEP — choices and text have no intermediate values.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "ParamTypeRepr", into = "ParamTypeRepr")]
pub enum ParamType {
    #[default]
    Num,
    Flag,
    /// Colour rgba8. The channels are the owner's ordinary numeric rows
    /// (`<stem>_r/_g/_b/_a`), so a colour curve is four ordinary
    /// curves, not a new kind of key (see `color_groups`).
    Color,
    /// Choice from variants. Stored by the variant KEY, not by number:
    /// a number would shift when a variant is inserted in the middle, while our keys
    /// already carry the enumerations in the file.
    Choice {
        variants: std::borrow::Cow<'static, [&'static str]>,
    },
    Text {
        multiline: bool,
    },
    Media,
}

impl ParamType {
    /// Whether a curve can be attached. Step types cannot have one: there is no half
    /// between “Left” and “Right”.
    pub fn curvable(&self) -> bool {
        matches!(self, ParamType::Num | ParamType::Color)
    }

    /// The list of `Choice` variants DOES NOT GO TO THE FILE (R.59, stage 3): it
    /// belongs to the TARGET and is read from it live — for the same reason
    /// a template value is not stored. A saved copy would silently drift from the target
    /// as soon as a menu item is added.
    ///
    /// Likewise, this is the same TYPE regardless of the set of variants of `Choice`.
    ///
    /// Publication remembers the target's type, and the variant list of the target may grow
    /// with a new build — if we compared with `==`, publication would die
    /// from adding an item to the menu.
    pub fn same_family(&self, other: &ParamType) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

/// Representation of `ParamType` in the file: only the FAMILY.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "snake_case")]
enum ParamTypeRepr {
    Num,
    Flag,
    Color,
    Choice,
    Text {
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        multiline: bool,
    },
    Media,
}

impl From<ParamTypeRepr> for ParamType {
    fn from(r: ParamTypeRepr) -> ParamType {
        match r {
            ParamTypeRepr::Num => ParamType::Num,
            ParamTypeRepr::Flag => ParamType::Flag,
            ParamTypeRepr::Color => ParamType::Color,
            ParamTypeRepr::Choice => {
                ParamType::Choice { variants: std::borrow::Cow::Borrowed(&[]) }
            }
            ParamTypeRepr::Text { multiline } => ParamType::Text { multiline },
            ParamTypeRepr::Media => ParamType::Media,
        }
    }
}

impl From<ParamType> for ParamTypeRepr {
    fn from(t: ParamType) -> ParamTypeRepr {
        match t {
            ParamType::Num => ParamTypeRepr::Num,
            ParamType::Flag => ParamTypeRepr::Flag,
            ParamType::Color => ParamTypeRepr::Color,
            ParamType::Choice { .. } => ParamTypeRepr::Choice,
            ParamType::Text { multiline } => ParamTypeRepr::Text { multiline },
            ParamType::Media => ParamTypeRepr::Media,
        }
    }
}

/// VALUE of a property (R.59, stage 3).
///
/// Until this round the only value was a number: a flag, a choice, a colour (stored
/// as a triple of numbers), text and a media reference were drawn by hand in their own
/// windows and were not properties. A public parameter of a composition must be able
/// to carry any of the six kinds — otherwise a “release title” or a “logo” could not be
/// expressed at all.
///
/// The serde tags are STABLE: the value goes into the file (an override on the instance),
/// and renaming a variant would break other people's projects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v", rename_all = "snake_case")]
pub enum ParamValue {
    Num(f64),
    Text(String),
    /// rgba8. The channels in the target are ordinary numeric rows `<stem>_r/_g/_b/_a`.
    Color([u8; 4]),
    Flag(bool),
    /// KEY of a variant, not a number: a number would shift when a variant is inserted
    /// in the middle.
    Choice(String),
    Media(MediaId),
}

impl ParamValue {
    pub fn ty(&self) -> ParamType {
        match self {
            ParamValue::Num(_) => ParamType::Num,
            ParamValue::Text(_) => ParamType::Text { multiline: false },
            ParamValue::Color(_) => ParamType::Color,
            ParamValue::Flag(_) => ParamType::Flag,
            ParamValue::Choice(_) => {
                ParamType::Choice { variants: std::borrow::Cow::Borrowed(&[]) }
            }
            ParamValue::Media(_) => ParamType::Media,
        }
    }

    pub fn as_num(&self) -> Option<f64> {
        match self {
            ParamValue::Num(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_media(&self) -> Option<MediaId> {
        match self {
            ParamValue::Media(m) => Some(*m),
            _ => None,
        }
    }
}

/// A colour property DERIVED from the owner's numeric rows (R.59).
///
/// A colour is not declared as a separate row and does not go into the format: all 69
/// components are already named `<stem>_{r,g,b}` with unit `Rgb8`, and the group
/// is derived from them. So hand-written colour lists in the interface are a second
/// source of the same fact, and they are removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorGroup {
    /// Name base: `color`, `a`, `tl`, `tint`, …
    pub stem: String,
    /// Whether the group has a fourth component `<stem>_a`.
    pub alpha: bool,
}

/// Derive colour groups from the passport. A row `<stem>_r` with unit `Rgb8`
/// and a `_g`/`_b` pair next to it is a colour; `_a` adds alpha.
pub fn derive_color_groups(specs: &[ParamSpec]) -> Vec<ColorGroup> {
    let has = |n: &str| specs.iter().any(|s| s.name == n && s.unit == ParamUnit::Rgb8);
    let mut out = Vec::new();
    for s in specs {
        if s.unit != ParamUnit::Rgb8 {
            continue;
        }
        let Some(stem) = s.name.strip_suffix("_r") else { continue };
        if !has(&format!("{stem}_g")) || !has(&format!("{stem}_b")) {
            continue;
        }
        out.push(ColorGroup { stem: stem.to_string(), alpha: has(&format!("{stem}_a")) });
    }
    out
}

/// Property name: for built-in kinds — a static string from the declaration;
/// for public composition parameters and for plugins — text invented by the
/// AUTHOR at runtime (R.59).
pub type ParamName = std::borrow::Cow<'static, str>;

/// Description of one effect parameter.
///
/// `Copy` was removed in R.59: the name became `Cow` — the passport must be built at
/// RUNTIME, otherwise neither a public composition parameter (the author invents its name
/// and renames it) nor a plugin (its passport is known only when loaded) could be expressed.
/// All places read the spec by reference.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamSpec {
    /// Field name in snake_case — also the key in the file and in `ParamCurve.param`.
    pub name: ParamName,
    /// Value type. For built-in rows — `Num` (declared by the macro).
    pub ty: ParamType,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    /// Step for the wheel/drag in the UI.
    pub step: f64,
    pub unit: ParamUnit,
    pub kind: ParamKind,
    /// Ready-made label. EMPTY for built-ins — their label comes from the translation
    /// catalog by name; NON-EMPTY for dynamic owners, whose name a person invented and
    /// which cannot be in the catalog.
    pub label: ParamName,
}

impl ParamSpec {
    /// Numeric spec — the shape that the macro generates and that describes all
    /// built-in rows.
    pub const fn num(
        name: &'static str,
        min: f64,
        max: f64,
        default: f64,
        step: f64,
        unit: ParamUnit,
        kind: ParamKind,
    ) -> ParamSpec {
        ParamSpec {
            name: std::borrow::Cow::Borrowed(name),
            ty: ParamType::Num,
            min,
            max,
            default,
            step,
            unit,
            kind,
            label: std::borrow::Cow::Borrowed(""),
        }
    }
}

/// Conversion of f64 to the field type of the variant.
macro_rules! fx_cast {
    (f64, $v:expr) => {
        $v
    };
    (f32, $v:expr) => {
        $v as f32
    };
    (u32, $v:expr) => {
        $v.round() as u32
    };
    (u8, $v:expr) => {
        $v.round().clamp(0.0, 255.0) as u8
    };
    (i32, $v:expr) => {
        $v.round() as i32
    };
}
pub(crate) use fx_cast;

/// The ONLY declaration of effect parameters: specs, reading and writing are generated from it,
/// so the three tables cannot drift apart.
/// The same parameter passport, but for a STRUCTURE (R.34 K3: the camera needs
/// specs to get into the shared field grid and the shared curve widget).
/// One declaration — specs, reading and writing with clamping.
macro_rules! params_for_struct {
    ($ty:ident { $( $field:ident : $t:ident, $lo:expr, $hi:expr, $def:expr, $step:expr, $unit:ident, $kind:ident );+ $(;)? }
     $( choice $cname:ident : $ckeys:expr, $cty:ty );* $(;)?) => {
        impl HasParams for $ty {
            fn param_specs(&self) -> &[ParamSpec] {
                const S: &[ParamSpec] = &[ $( ParamSpec::num(
                    stringify!($field), $lo, $hi, $def, $step,
                    ParamUnit::$unit, ParamKind::$kind,
                ) ),+ ];
                S
            }
            fn get_param(&self, name: &str) -> Option<f64> {
                $( if name == stringify!($field) { return Some(self.$field as f64); } )+
                None
            }
            fn set_param(&mut self, name: &str, v: f64) -> bool {
                $( if name == stringify!($field) {
                    self.$field = fx_cast!($t, v.clamp($lo, $hi));
                    return true;
                } )+
                false
            }
            fn get_param_idx(&self, idx: usize) -> Option<f64> {
                let mut i = 0usize;
                $( if idx == i { return Some(self.$field as f64); } i += 1; )+
                let _ = i;
                None
            }
            fn set_param_idx(&mut self, idx: usize, v: f64) -> bool {
                let mut i = 0usize;
                $( if idx == i {
                    self.$field = fx_cast!($t, v.clamp($lo, $hi));
                    return true;
                } i += 1; )+
                let _ = i;
                false
            }
            // Choice properties of a structure (R.63). Without them the branches are empty, and
            // behaviour is byte-for-byte the same as before: `extra_specs` returns an
            // empty vector, and the value takes the trait defaults.
            fn extra_specs(&self) -> Vec<ParamSpec> {
                vec![ $( choice_spec(stringify!($cname), $ckeys) ),* ]
            }
            fn get_value(&self, name: &str) -> Option<ParamValue> {
                $( if name == stringify!($cname) {
                    return Some(ParamValue::Choice(self.$cname.tag().into()));
                } )*
                default_get_value(self, name)
            }
            fn set_value(&mut self, name: &str, v: &ParamValue) -> bool {
                $( if name == stringify!($cname) {
                    let ParamValue::Choice(key) = v else { return false };
                    let Some(x) = <$cty>::from_tag(key) else { return false };
                    self.$cname = x;
                    return true;
                } )*
                default_set_value(self, name, v)
            }
        }
    };
}

macro_rules! params_for {
    ($enum:ident, $specs:ident, $set:ident, $get:ident, $set_idx:ident, $get_idx:ident, {$(
        $variant:ident { $( $field:ident : $ty:ident, $lo:expr, $hi:expr, $def:expr, $step:expr, $unit:ident, $kind:ident );+ $(;)? }
    ),* $(,)?}) => {
        fn $specs(fx: &$enum) -> &'static [ParamSpec] {
            match fx {
                $( $enum::$variant { .. } => {
                    const S: &[ParamSpec] = &[
                        $( ParamSpec::num(
                            stringify!($field), $lo, $hi, $def, $step,
                            ParamUnit::$unit, ParamKind::$kind,
                        ) ),+
                    ];
                    S
                }, )*
                #[allow(unreachable_patterns)]
                _ => &[],
            }
        }

        /// Write by ROW INDEX (R.59): the order of rows is the declaration order, which is also
        /// the order of `param_specs()`, so the index is stable by construction.
        /// It is needed by the compiled automation plan: it does not look up a parameter
        /// by comparing strings on every
        /// frame.
        fn $set_idx(fx: &mut $enum, idx: usize, v: f64) -> bool {
            match fx {
                $( $enum::$variant { $($field,)+ .. } => {
                    let mut i = 0usize;
                    $( if idx == i {
                        *$field = fx_cast!($ty, v.clamp($lo, $hi));
                        return true;
                    } i += 1; )+
                    let _ = i;
                    false
                } )*
                #[allow(unreachable_patterns)]
                _ => false,
            }
        }

        fn $get_idx(fx: &$enum, idx: usize) -> Option<f64> {
            match fx {
                $( $enum::$variant { $($field,)+ .. } => {
                    let mut i = 0usize;
                    $( if idx == i { return Some(*$field as f64); } i += 1; )+
                    let _ = i;
                    None
                } )*
                #[allow(unreachable_patterns)]
                _ => None,
            }
        }

        /// Write by NAME. THE CLAMP LIVES HERE (R.59): until this round it sat in the
        /// wrappers `FxInstance::set_param` and `TransitionKind::set_param`, and generators had no
        /// wrapper at all — a title’s font size accepted any number from the file or from a
        /// command. Now one limit applies to all owners and to both addressing paths,
        /// named and indexed,
        /// (t642).
        fn $set(fx: &mut $enum, name: &str, v: f64) -> bool {
            match fx {
                $( $enum::$variant { $($field,)+ .. } => {
                    $( if name == stringify!($field) {
                        *$field = fx_cast!($ty, v.clamp($lo, $hi));
                        return true;
                    } )+
                    false
                } )*
                #[allow(unreachable_patterns)]
                _ => false,
            }
        }

        fn $get(fx: &$enum, name: &str) -> Option<f64> {
            match fx {
                $( $enum::$variant { $($field,)+ .. } => {
                    $( if name == stringify!($field) {
                        return Some(*$field as f64);
                    } )+
                    None
                } )*
                #[allow(unreachable_patterns)]
                _ => None,
            }
        }
    };
}
pub(crate) use params_for;

/// The passport of the kind's FLAGS — from ONE declaration, next to `params_for!`
/// (R.53, item 414). It generates specs, reading and writing: the three tables cannot drift
/// apart, and completeness (whether a bool field was forgotten) is guarded by a test that
/// checks the flags against the kind’s serde representation.
macro_rules! flags_for {
    ($enum:ident, $specs:ident, $set:ident, $get:ident, {$(
        $variant:ident { $( $field:ident : $def:expr );+ $(;)? }
    ),* $(,)?}) => {
        fn $specs(fx: &$enum) -> &'static [FlagSpec] {
            match fx {
                $( $enum::$variant { .. } => &[
                    $( FlagSpec { name: stringify!($field), default: $def } ),+
                ], )*
                #[allow(unreachable_patterns)]
                _ => &[],
            }
        }

        fn $set(fx: &mut $enum, name: &str, v: bool) -> bool {
            match fx {
                $( $enum::$variant { $($field,)+ .. } => {
                    $( if name == stringify!($field) {
                        *$field = v;
                        return true;
                    } )+
                    false
                } )*
                #[allow(unreachable_patterns)]
                _ => false,
            }
        }

        fn $get(fx: &$enum, name: &str) -> Option<bool> {
            match fx {
                $( $enum::$variant { $($field,)+ .. } => {
                    $( if name == stringify!($field) {
                        return Some(*$field);
                    } )+
                    None
                } )*
                #[allow(unreachable_patterns)]
                _ => None,
            }
        }
    };
}
pub(crate) use flags_for;

params_for! { FxInstance, fx_specs, fx_set_param, fx_get_param, fx_set_idx, fx_get_idx, {
    // ---- audio (R.33: automated by the shared mechanism) ----------------
    Gain {
        db: f32, -60.0, 12.0, 0.0, 0.1, Db, Smooth;
        pan: f32, -1.0, 1.0, 0.0, 0.01, Norm, Smooth;
    },
    LayerMix {
        amount: f64, 0.0, 1.0, 1.0, 0.01, Norm, Smooth;
    },
    SlowedReverb {
        // speed range — RATE_CLAMP of the warp map (warp.rs, R.39:
        // previously it lived in kt-fx and was a promise across the crate boundary).
        // Signed and including zero (R.57, the owner’s decision): minus runs BACKWARD
        // through the material, zero — a freeze frame.
        rate: f64, -32.0, 32.0, 1.0, 0.01, Ratio, Smooth;
        mix: f32, 0.0, 1.0, 0.0, 0.01, Norm, Smooth;
        // changing the length of comb lines rebuilds the reverb — not automatable
        reverb_delay_ms: f32, 1.0, 500.0, 60.0, 1.0, Ms, Structural;
        damp: f32, 0.0, 0.99, 0.2, 0.01, Norm, Smooth;
        decay: f32, 0.0, 0.98, 0.84, 0.01, Norm, Smooth;
        wet_gain: f32, 0.0, 2.0, 1.0, 0.01, Ratio, Smooth;
        // length of the pre-delay line — the same geometry as reverb_delay_ms
        predelay_ms: f32, 0.0, 200.0, 0.0, 1.0, Ms, Structural;
        width: f32, 0.0, 1.0, 1.0, 0.01, Norm, Smooth;
    },
    Filter {
        cutoff_hz: f32, 20.0, 20000.0, 800.0, 10.0, Hz, Smooth;
        resonance: f32, 0.5, 10.0, 0.707, 0.01, None, Smooth;
    },
    Eq {
        low_gain_db: f32, -24.0, 24.0, 0.0, 0.1, Db, Smooth;
        low_freq_hz: f32, 20.0, 2000.0, 200.0, 10.0, Hz, Smooth;
        mid_gain_db: f32, -24.0, 24.0, 0.0, 0.1, Db, Smooth;
        mid_freq_hz: f32, 100.0, 10000.0, 1000.0, 10.0, Hz, Smooth;
        mid_q: f32, 0.3, 8.0, 1.0, 0.01, None, Smooth;
        high_gain_db: f32, -24.0, 24.0, 0.0, 0.1, Db, Smooth;
        high_freq_hz: f32, 1000.0, 20000.0, 4000.0, 10.0, Hz, Smooth;
    },
    Distortion {
        drive_db: f32, 0.0, 36.0, 12.0, 0.1, Db, Smooth;
        tone: f32, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        mix: f32, 0.0, 1.0, 1.0, 0.01, Norm, Smooth;
    },
    Bitcrusher {
        bits: f32, 1.0, 16.0, 8.0, 0.1, None, Smooth;
        rate_hz: f32, 500.0, 48000.0, 12000.0, 100.0, Hz, Smooth;
        mix: f32, 0.0, 1.0, 1.0, 0.01, Norm, Smooth;
    },
    StereoWidth {
        width: f32, 0.0, 2.0, 1.0, 0.01, Ratio, Smooth;
    },
    Tremolo {
        rate_hz: f32, 0.1, 20.0, 5.0, 0.01, Hz, Smooth;
        depth: f32, 0.0, 1.0, 0.6, 0.01, Norm, Smooth;
    },
    AutoPan {
        rate_hz: f32, 0.05, 10.0, 1.0, 0.01, Hz, Smooth;
        depth: f32, 0.0, 1.0, 0.8, 0.01, Norm, Smooth;
    },
    Vibrato {
        rate_hz: f32, 0.1, 12.0, 5.0, 0.01, Hz, Smooth;
        depth_ms: f32, 0.05, 5.0, 0.5, 0.05, Ms, Smooth;
    },
    Chorus {
        rate_hz: f32, 0.05, 8.0, 0.8, 0.01, Hz, Smooth;
        depth_ms: f32, 0.0, 12.0, 3.5, 0.1, Ms, Smooth;
        mix: f32, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        // number of voices rebuilds the lines — not subject to a curve
        voices: u32, 1.0, 3.0, 2.0, 1.0, Count, Structural;
    },
    Flanger {
        rate_hz: f32, 0.05, 4.0, 0.25, 0.01, Hz, Smooth;
        depth_ms: f32, 0.1, 8.0, 2.0, 0.1, Ms, Smooth;
        feedback: f32, -0.9, 0.9, 0.5, 0.01, Norm, Smooth;
        mix: f32, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
    },
    Phaser {
        rate_hz: f32, 0.05, 4.0, 0.4, 0.01, Hz, Smooth;
        depth: f32, 0.0, 1.0, 0.7, 0.01, Norm, Smooth;
        feedback: f32, 0.0, 0.9, 0.3, 0.01, Norm, Smooth;
        // number of cascades rebuilds the state — not subject to a curve
        stages: u32, 2.0, 8.0, 4.0, 1.0, Count, Structural;
        mix: f32, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
    },
    Delay {
        time_ms: f32, 1.0, 1500.0, 350.0, 1.0, Ms, Smooth;
        feedback: f32, 0.0, 0.9, 0.35, 0.01, Norm, Smooth;
        mix: f32, 0.0, 1.0, 0.35, 0.01, Norm, Smooth;
    },
    Compressor {
        threshold_db: f32, -60.0, 0.0, -18.0, 0.1, Db, Smooth;
        ratio: f32, 1.0, 20.0, 4.0, 0.1, Ratio, Smooth;
        attack_ms: f32, 0.1, 250.0, 10.0, 0.1, Ms, Smooth;
        release_ms: f32, 5.0, 2000.0, 150.0, 1.0, Ms, Smooth;
        makeup_db: f32, 0.0, 24.0, 0.0, 0.1, Db, Smooth;
    },
    Gate {
        threshold_db: f32, -80.0, 0.0, -40.0, 0.1, Db, Smooth;
        attack_ms: f32, 0.1, 50.0, 1.0, 0.1, Ms, Smooth;
        hold_ms: f32, 0.0, 500.0, 40.0, 1.0, Ms, Smooth;
        release_ms: f32, 5.0, 1000.0, 100.0, 1.0, Ms, Smooth;
    },
    // ---- video 2D --------------------------------------------------------
    ColorGrade {
        brightness: f32, -1.0, 1.0, 0.0, 0.01, Norm, Smooth;
        contrast: f32, -1.0, 1.0, 0.0, 0.01, Norm, Smooth;
        saturation: f32, 0.0, 2.0, 1.0, 0.01, Ratio, Smooth;
        hue: f32, -180.0, 180.0, 0.0, 1.0, Deg, Smooth;
    },
    Duotone {
        shadow_r: u8, 0.0, 255.0, 24.0, 1.0, Rgb8, Smooth;
        shadow_g: u8, 0.0, 255.0, 18.0, 1.0, Rgb8, Smooth;
        shadow_b: u8, 0.0, 255.0, 64.0, 1.0, Rgb8, Smooth;
        high_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        high_g: u8, 0.0, 255.0, 236.0, 1.0, Rgb8, Smooth;
        high_b: u8, 0.0, 255.0, 190.0, 1.0, Rgb8, Smooth;
        amount: f32, 0.0, 1.0, 1.0, 0.01, Norm, Smooth;
    },
    Posterize {
        levels: u32, 2.0, 32.0, 6.0, 1.0, Count, Smooth;
    },
    Vignette {
        strength: f32, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        radius: f32, 0.0, 1.5, 0.7, 0.01, Norm, Smooth;
        softness: f32, 0.05, 1.0, 0.45, 0.01, Norm, Smooth;
    },
    Sharpen {
        amount: f32, 0.0, 4.0, 1.0, 0.01, Ratio, Smooth;
        radius: u32, 1.0, 16.0, 2.0, 1.0, Px, Smooth;
    },
    Pixelate {
        size: u32, 2.0, 128.0, 12.0, 1.0, Px, Smooth;
    },
    ChromaticAberration {
        amount: f32, 0.0, 0.05, 0.012, 0.001, Norm, Smooth;
    },
    RadialBlur {
        amount: f32, 0.0, 1.0, 0.25, 0.01, Norm, Smooth;
        center_x: f32, -1.0, 1.0, 0.0, 0.01, Norm, Smooth;
        center_y: f32, -1.0, 1.0, 0.0, 0.01, Norm, Smooth;
    },
    DropShadow {
        offset_x: i32, -64.0, 64.0, 8.0, 1.0, Px, Smooth;
        offset_y: i32, -64.0, 64.0, 8.0, 1.0, Px, Smooth;
        radius: u32, 1.0, 32.0, 4.0, 1.0, Px, Smooth;
        opacity: f32, 0.0, 1.0, 0.6, 0.01, Norm, Smooth;
        tint_r: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        tint_g: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        tint_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
    },
    Halftone {
        size: u32, 2.0, 64.0, 8.0, 1.0, Px, Smooth;
        angle: f32, -90.0, 90.0, 45.0, 1.0, Deg, Smooth;
    },
    Limiter {
        threshold_db: f32, -60.0, 0.0, -6.0, 0.1, Db, Smooth;
        ceiling_db: f32, -24.0, 0.0, -0.3, 0.1, Db, Smooth;
        lookahead_ms: f32, 1.0, 20.0, 5.0, 0.5, Ms, Structural;
        release_ms: f32, 5.0, 1000.0, 80.0, 1.0, Ms, Smooth;
    },
    PitchShift {
        semitones: f32, -24.0, 24.0, 0.0, 1.0, St, Smooth;
        cents: f32, -100.0, 100.0, 0.0, 1.0, None, Smooth;
    },
    LensDistort {
        k1: f32, -1.0, 1.0, 0.35, 0.01, Norm, Smooth;
        k2: f32, -1.0, 1.0, 0.0, 0.01, Norm, Smooth;
    },
    Twirl {
        angle: f32, -720.0, 720.0, 120.0, 1.0, Deg, Smooth;
        radius: f32, 0.1, 1.5, 0.8, 0.01, Norm, Smooth;
    },
    Ripple {
        amount: f32, 0.0, 0.5, 0.08, 0.005, Norm, Smooth;
        freq: f32, 1.0, 40.0, 10.0, 0.5, Count, Smooth;
        phase: f32, 0.0, 3600.0, 0.0, 1.0, Deg, Smooth;
    },
    Kaleidoscope {
        segments: u32, 2.0, 32.0, 6.0, 1.0, Count, Smooth;
        angle: f32, -180.0, 180.0, 0.0, 1.0, Deg, Smooth;
    },
    Cylinder3d {
        curvature: f32, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
    },
    CornerPin {
        x0: f64, -1.0, 2.0, 0.0, 0.005, Norm, Smooth;
        y0: f64, -1.0, 2.0, 0.0, 0.005, Norm, Smooth;
        x1: f64, -1.0, 2.0, 1.0, 0.005, Norm, Smooth;
        y1: f64, -1.0, 2.0, 0.0, 0.005, Norm, Smooth;
        x2: f64, -1.0, 2.0, 0.0, 0.005, Norm, Smooth;
        y2: f64, -1.0, 2.0, 1.0, 0.005, Norm, Smooth;
        x3: f64, -1.0, 2.0, 1.0, 0.005, Norm, Smooth;
        y3: f64, -1.0, 2.0, 1.0, 0.005, Norm, Smooth;
    },
    Geometry {
        crop_l: f64, 0.0, 0.95, 0.0, 0.005, Norm, Smooth;
        crop_t: f64, 0.0, 0.95, 0.0, 0.005, Norm, Smooth;
        crop_r: f64, 0.0, 0.95, 0.0, 0.005, Norm, Smooth;
        crop_b: f64, 0.0, 0.95, 0.0, 0.005, Norm, Smooth;
        pos_x: f64, -2.0, 2.0, 0.0, 0.005, Norm, Smooth;
        pos_y: f64, -2.0, 2.0, 0.0, 0.005, Norm, Smooth;
        zoom: f64, 0.05, 32.0, 1.0, 0.01, Ratio, Smooth;
        rot: f64, -360.0, 360.0, 0.0, 0.5, Deg, Smooth;
        anchor_x: f64, 0.0, 1.0, 0.5, 0.005, Norm, Smooth;
        anchor_y: f64, 0.0, 1.0, 0.5, 0.005, Norm, Smooth;
    },
    CamMotion {
        time_scale: f64, 0.05, 8.0, 1.0, 0.01, Ratio, Smooth;
    },
    Blur {
        radius: u32, 1.0, 48.0, 8.0, 1.0, Px, Smooth;
        radius_y: u32, 0.0, 48.0, 0.0, 1.0, Px, Smooth;
    },
    Greenscreen {
        key_r: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        key_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        key_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        tolerance: u32, 1.0, 200.0, 70.0, 1.0, None, Smooth;
        softness: u32, 0.0, 100.0, 25.0, 1.0, None, Smooth;
        spill: f64, 0.0, 1.0, 0.0, 0.01, Norm, Smooth;
    },
    Edges {
        threshold: u32, 1.0, 255.0, 48.0, 1.0, None, Smooth;
        thickness: u32, 1.0, 8.0, 1.0, 1.0, Px, Smooth;
        color_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
    },
    Noise {
        amount: f64, 0.0, 1.0, 0.08, 0.01, Norm, Smooth;
        size: u32, 1.0, 64.0, 1.0, 1.0, Px, Smooth;
    },
    Vhs {
        rgb_shift: u32, 0.0, 12.0, 4.0, 1.0, Px, Smooth;
        bleed: f64, 0.0, 1.0, 0.4, 0.01, Norm, Smooth;
        scan_step: u32, 0.0, 32.0, 4.0, 1.0, Px, Smooth;
        jitter: f64, 0.0, 1.0, 0.0, 0.01, Norm, Smooth;
    },
    Glow {
        strength: f64, 0.0, 2.0, 0.65, 0.01, Ratio, Smooth;
        radius: u32, 1.0, 32.0, 4.0, 1.0, Px, Smooth;
        threshold: u32, 0.0, 255.0, 0.0, 1.0, None, Smooth;
        tint_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        tint_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        tint_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
    },
    EventGrid {
        cols: u32, 1.0, 512.0, 1.0, 1.0, Count, Smooth;
        rows: u32, 1.0, 512.0, 1.0, 1.0, Count, Smooth;
    },
    // ---- video 3D (R.29) -------------------------------------------------
    Pan3d {
        pos_x: f64, -100_000.0, 100_000.0, 0.0, 1.0, Px, Smooth;
        pos_y: f64, -100_000.0, 100_000.0, 0.0, 1.0, Px, Smooth;
        pos_z: f64, -100_000.0, 100_000.0, 0.0, 1.0, Px, Smooth;
        rot_x: f64, -1440.0, 1440.0, 0.0, 0.5, Deg, Smooth;
        rot_y: f64, -1440.0, 1440.0, 0.0, 0.5, Deg, Smooth;
        rot_z: f64, -1440.0, 1440.0, 0.0, 0.5, Deg, Smooth;
        anchor_x: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        anchor_y: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        scale: f64, 0.01, 32.0, 1.0, 0.01, Ratio, Smooth;
    },
}}

// Transition parameters — the same declaration as for effects: the field UI,
// clamping and (in future) automation come to them for free.
// Camera units are frozen by the format (R.29): world px and ZXY degrees.
params_for_struct! { CameraParams {
    pos_x: f64, -100_000.0, 100_000.0, 0.0, 1.0, Px, Smooth;
    pos_y: f64, -100_000.0, 100_000.0, 0.0, 1.0, Px, Smooth;
    pos_z: f64, -100_000.0, 100_000.0, 0.0, 1.0, Px, Smooth;
    rot_x: f64, -1440.0, 1440.0, 0.0, 0.5, Deg, Smooth;
    rot_y: f64, -1440.0, 1440.0, 0.0, 0.5, Deg, Smooth;
    rot_z: f64, -1440.0, 1440.0, 0.0, 0.5, Deg, Smooth;
    fov_deg: f64, 1.0, 179.0, 39.6, 0.5, Deg, Smooth;
}}

params_for! { TransitionKind, transition_specs, transition_set_param, transition_get_param, transition_set_idx, transition_get_idx, {
    Wipe {
        angle: f64, -180.0, 180.0, 0.0, 1.0, Deg, Smooth;
        softness: f64, 0.0, 1.0, 0.15, 0.01, Norm, Smooth;
    },
    Iris {
        cx: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        cy: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        softness: f64, 0.0, 1.0, 0.15, 0.01, Norm, Smooth;
    },
    Clock {
        cx: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        cy: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        softness: f64, 0.0, 1.0, 0.15, 0.01, Norm, Smooth;
    },
    Barn {
        softness: f64, 0.0, 1.0, 0.15, 0.01, Norm, Smooth;
    },
    Luma {
        softness: f64, 0.0, 1.0, 0.15, 0.01, Norm, Smooth;
    },
    Zoom {
        amount: f64, 0.0, 4.0, 0.4, 0.01, Ratio, Smooth;
        cx: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        cy: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
    },
    Spin {
        turns: f64, -4.0, 4.0, 1.0, 0.05, Ratio, Smooth;
        amount: f64, 0.0, 4.0, 0.4, 0.01, Ratio, Smooth;
        cx: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        cy: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
    },
    Whip {
        blur: f64, 0.0, 1.0, 0.6, 0.01, Norm, Smooth;
    },
    BlurDissolve {
        amount: f64, 0.0, 64.0, 24.0, 1.0, Px, Smooth;
    },
    Pixelate {
        amount: f64, 1.0, 256.0, 48.0, 1.0, Px, Smooth;
    },
    Glitch {
        amount: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
    },
    Flash {
        color_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_g: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        color_b: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        strength: f64, 0.0, 1.0, 1.0, 0.01, Norm, Smooth;
    },
    Dip {
        color_r: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        color_g: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
        color_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
    },
    Blinds {
        count: u32, 2.0, 64.0, 8.0, 1.0, Count, Smooth;
        angle: f64, -90.0, 90.0, 0.0, 1.0, Deg, Smooth;
        softness: f64, 0.0, 1.0, 0.1, 0.01, Norm, Smooth;
    },
    Checker {
        cols: u32, 1.0, 64.0, 8.0, 1.0, Count, Smooth;
        rows: u32, 1.0, 64.0, 6.0, 1.0, Count, Smooth;
        softness: f64, 0.0, 1.0, 0.1, 0.01, Norm, Smooth;
    },
    Blocks {
        cols: u32, 2.0, 64.0, 16.0, 1.0, Count, Smooth;
        rows: u32, 2.0, 64.0, 9.0, 1.0, Count, Smooth;
        softness: f64, 0.0, 1.0, 0.25, 0.01, Norm, Smooth;
    },
    Spiral {
        turns: f64, 0.25, 6.0, 2.0, 0.05, Ratio, Smooth;
        cx: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        cy: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        softness: f64, 0.0, 1.0, 0.1, 0.01, Norm, Smooth;
    },
    Swap {
        depth: f64, 0.0, 1.0, 0.3, 0.01, Norm, Smooth;
    },
    Shake {
        amount: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        steps: u32, 1.0, 30.0, 12.0, 1.0, Count, Smooth;
    },
    FilmBurn {
        scale: f64, 1.0, 24.0, 6.0, 0.5, Ratio, Smooth;
        softness: f64, 0.0, 1.0, 0.2, 0.01, Norm, Smooth;
        tint_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        tint_g: u8, 0.0, 255.0, 120.0, 1.0, Rgb8, Smooth;
        tint_b: u8, 0.0, 255.0, 0.0, 1.0, Rgb8, Smooth;
    },
    LightLeaks {
        strength: f64, 0.0, 1.0, 0.6, 0.01, Norm, Smooth;
        tint_r: u8, 0.0, 255.0, 255.0, 1.0, Rgb8, Smooth;
        tint_g: u8, 0.0, 255.0, 90.0, 1.0, Rgb8, Smooth;
        tint_b: u8, 0.0, 255.0, 40.0, 1.0, Rgb8, Smooth;
    },
    ZoomBlur {
        amount: f64, 0.0, 1.0, 0.6, 0.01, Norm, Smooth;
        cx: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        cy: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
    },
    Flip {
        persp: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        shade: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
    },
    Cube {
        persp: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        shade: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
    },
    Door {
        persp: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
        shade: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
    },
    PagePeel {
        curl: f64, 0.05, 0.45, 0.18, 0.01, Norm, Smooth;
        shadow: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
    },
    PageRoll {
        curl: f64, 0.05, 0.45, 0.18, 0.01, Norm, Smooth;
        shadow: f64, 0.0, 1.0, 0.5, 0.01, Norm, Smooth;
    },
}}

/// Crop-frame mode for pan/zoom (v1: fit/fill/stretch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanMode {
    #[default]
    Fit,
    Fill,
    Stretch,
}

/// Rule for fitting the source frame to the canvas (R.62, F-MED-004):
/// fit with TRANSPARENT margins / fill with cropping / stretch /
/// as is (1:1 pixel, centred; larger than the canvas — overflows the edge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConformRule {
    #[default]
    Fit,
    Fill,
    Stretch,
    AsIs,
}

/// Rule at the EVENT level: follow the project or its own. The level “project +
/// exception at the event” is the owner’s decision for R.62: changing the project’s default
/// rewrites everyone who follows it and leaves the explicit ones untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conform {
    #[default]
    Project,
    Fit,
    Fill,
    Stretch,
    AsIs,
}

impl Conform {
    /// Resolve the event rule to the project rule.
    pub fn resolve(self, project: ConformRule) -> ConformRule {
        match self {
            Conform::Project => project,
            Conform::Fit => ConformRule::Fit,
            Conform::Fill => ConformRule::Fill,
            Conform::Stretch => ConformRule::Stretch,
            Conform::AsIs => ConformRule::AsIs,
        }
    }

    /// KEY of a variant for the property passport (`Choice` stores the key, not
    /// the number). It matches what serde writes — one fact must not
    /// have a second spelling.
    pub fn tag(self) -> &'static str {
        match self {
            Conform::Project => "project",
            Conform::Fit => "fit",
            Conform::Fill => "fill",
            Conform::Stretch => "stretch",
            Conform::AsIs => "as_is",
        }
    }

    pub fn from_tag(key: &str) -> Option<Conform> {
        CONFORM_KEYS.iter().position(|k| *k == key).map(|i| CONFORM_ALL[i])
    }

    pub fn from_rule(r: ConformRule) -> Conform {
        match r {
            ConformRule::Fit => Conform::Fit,
            ConformRule::Fill => Conform::Fill,
            ConformRule::Stretch => Conform::Stretch,
            ConformRule::AsIs => Conform::AsIs,
        }
    }
}

/// LEADING node “Geometry” of a video event (R.62): conform rule +
/// crop (source fractions), position (canvas fractions, 0 — centred), zoom,
/// rotation (degrees) around the anchor (frame fractions). It sits FIRST in the chain
/// (`ClipEvent.geometry_pos`), is dragged, cannot be deleted — like
/// Pan/Crop in Vegas.
///
/// An event field, NOT an `fx` slot: this way slot indices and curve/link addresses
/// do not shift because the event gained a node; the engine
/// materializes it into the chain at position `geometry_pos` when building.
/// Additional nodes of the same kind — `FxInstance::Geometry` (without a rule).
/// Place reserved for a mask (a separate change): an adjacent field, order inside the node —
/// crop → mask → placement.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EventGeometry {
    #[serde(default, skip_serializing_if = "is_default_conform")]
    pub rule: Conform,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub crop_l: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub crop_t: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub crop_r: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub crop_b: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub pos_x: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub pos_y: f64,
    #[serde(default = "default_zoom", skip_serializing_if = "is_one")]
    pub zoom: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub rot: f64,
    #[serde(default = "default_half", skip_serializing_if = "is_half")]
    pub anchor_x: f64,
    #[serde(default = "default_half", skip_serializing_if = "is_half")]
    pub anchor_y: f64,
}

fn is_default_conform(c: &Conform) -> bool {
    *c == Conform::Project
}
fn is_half(v: &f64) -> bool {
    *v == 0.5
}

impl Default for EventGeometry {
    fn default() -> Self {
        EventGeometry {
            rule: Conform::Project,
            crop_l: 0.0,
            crop_t: 0.0,
            crop_r: 0.0,
            crop_b: 0.0,
            pos_x: 0.0,
            pos_y: 0.0,
            zoom: 1.0,
            rot: 0.0,
            anchor_x: 0.5,
            anchor_y: 0.5,
        }
    }
}

impl EventGeometry {
    pub fn is_default(&self) -> bool {
        *self == EventGeometry::default()
    }

    /// Parameters are identity (the rule does not count): the node moves nothing,
    /// it only conforms the frame by the rule.
    pub fn params_identity(&self) -> bool {
        self.node().geometry_is_identity()
    }

    /// The same node as a chain slot (without the rule) — the form that
    /// kt-fx sees. The leading node is materialized by the engine into the chain at
    /// position `geometry_pos`.
    pub fn node(&self) -> FxInstance {
        FxInstance::Geometry {
            lead: true,
            crop_l: self.crop_l,
            crop_t: self.crop_t,
            crop_r: self.crop_r,
            crop_b: self.crop_b,
            pos_x: self.pos_x,
            pos_y: self.pos_y,
            zoom: self.zoom,
            rot: self.rot,
            anchor_x: self.anchor_x,
            anchor_y: self.anchor_y,
        }
    }

    /// Back from a slot (the rule stays its own).
    pub fn with_node(mut self, n: &FxInstance) -> EventGeometry {
        if let FxInstance::Geometry {
            crop_l,
            crop_t,
            crop_r,
            crop_b,
            pos_x,
            pos_y,
            zoom,
            rot,
            anchor_x,
            anchor_y,
            ..
        } = n
        {
            self.crop_l = *crop_l;
            self.crop_t = *crop_t;
            self.crop_r = *crop_r;
            self.crop_b = *crop_b;
            self.pos_x = *pos_x;
            self.pos_y = *pos_y;
            self.zoom = *zoom;
            self.rot = *rot;
            self.anchor_x = *anchor_x;
            self.anchor_y = *anchor_y;
        }
        self
    }
}

/// Keys of the conform rule for the property passport (R.63): the rule is
/// a published property of the leading node, just like zoom.
pub const CONFORM_KEYS: &[&str] = &["project", "fit", "fill", "stretch", "as_is"];
const CONFORM_ALL: [Conform; 5] =
    [Conform::Project, Conform::Fit, Conform::Fill, Conform::Stretch, Conform::AsIs];

params_for_struct! { EventGeometry {
    crop_l: f64, 0.0, 0.95, 0.0, 0.005, Norm, Smooth;
    crop_t: f64, 0.0, 0.95, 0.0, 0.005, Norm, Smooth;
    crop_r: f64, 0.0, 0.95, 0.0, 0.005, Norm, Smooth;
    crop_b: f64, 0.0, 0.95, 0.0, 0.005, Norm, Smooth;
    pos_x: f64, -2.0, 2.0, 0.0, 0.005, Norm, Smooth;
    pos_y: f64, -2.0, 2.0, 0.0, 0.005, Norm, Smooth;
    zoom: f64, 0.05, 32.0, 1.0, 0.01, Ratio, Smooth;
    rot: f64, -360.0, 360.0, 0.0, 0.5, Deg, Smooth;
    anchor_x: f64, 0.0, 1.0, 0.5, 0.005, Norm, Smooth;
    anchor_y: f64, 0.0, 1.0, 0.5, 0.005, Norm, Smooth;
} choice rule: CONFORM_KEYS, Conform}

/// Audio filter type (R.47): low-pass / high-pass / band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterMode {
    #[default]
    Lowpass,
    Highpass,
    Bandpass,
}

/// How to get a frame that is absent in the source (R.37 item 2): a stretched
/// video asks for material between frames. `Nearest` — keep the nearest one
/// (determinism, zero extra decode); `Blend` — linearly mix two adjacent
/// ones. A third option will go here: fruc_vulkan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameMode {
    #[default]
    Nearest,
    Blend,
}

/// CamMotion keyframe by clip progress t∈[0,1].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CamKeyframe {
    pub t: f64,
    #[serde(default)]
    pub pan_x: f64,
    #[serde(default)]
    pub pan_y: f64,
    #[serde(default = "default_zoom")]
    pub zoom: f64,
}

/// Camera event on a camera track (R.29, pattern — the morph track): the event is
/// both a camera and its “time switch on” (like a
/// camera layer in AE). There may be several cameras — the one active at moment t
/// is chosen by `Project::active_camera_at` by a deterministic rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraEvent {
    pub id: CameraId,
    /// Index of a camera track (0..camera_lanes).
    pub lane: u32,
    pub start: Tick,
    pub duration: Tick,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Base parameters (keyframes override them).
    pub cam: CameraParams,
    /// LEGACY input (R.34 K3): animation by event progress t∈[0,1].
    /// Read from old files and converted to `auto`; new curves
    /// are not written here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keyframes: Vec<CameraKeyframe>,
    /// Camera parameter curves — the SAME mechanism as for effects
    /// (R.34 K3). The key position is a tick FROM THE START of the event: moving the camera
    /// carries its animation along, and changing the length opens/hides keys
    /// like clip trimming (“nothing grows back by itself”, item 20 of round 28).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto: Vec<ParamCurve>,
    #[serde(default = "default_true")]
    pub ease: bool,
}

/// Camera parameters. Default — “50 mm”: perspective with fov ≈ 39.6°, auto-
/// distance (pos_z = 0 is treated by the engine as −(H/2)/tan(fov/2)) — a layer
/// at z=0 with identity Pan3d renders 1:1, 2D projects do not change
/// by a single pixel. Units — world px and degrees (ZXY), as in Pan3d.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraParams {
    #[serde(default)]
    pub pos_x: f64,
    #[serde(default)]
    pub pos_y: f64,
    /// 0 = “auto” (default distance 50 mm).
    #[serde(default)]
    pub pos_z: f64,
    /// Point of interest (two-node camera, as in AE). None — one-node:
    /// looks along +Z.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<(f64, f64, f64)>,
    /// Extra rotation AFTER target (AE orientation), degrees, ZXY.
    #[serde(default)]
    pub rot_x: f64,
    #[serde(default)]
    pub rot_y: f64,
    #[serde(default)]
    pub rot_z: f64,
    /// Vertical FOV in degrees.
    #[serde(default = "default_fov")]
    pub fov_deg: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ortho: bool,
}

impl Default for CameraParams {
    fn default() -> CameraParams {
        CameraParams {
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            target: None,
            rot_x: 0.0,
            rot_y: 0.0,
            rot_z: 0.0,
            fov_deg: default_fov(),
            ortho: false,
        }
    }
}

/// Camera keyframe by event progress t∈[0,1].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraKeyframe {
    pub t: f64,
    pub cam: CameraParams,
}

impl CameraEvent {
    /// Camera parameters at timeline moment t (pattern: cam_at from kt-fx):
    /// event progress 0..1, linear interpolation of scalar fields between
    /// keyframes, smoothstep when eased, beyond the edges — the edge values. `target`
    /// is interpolated only when set at both points of the segment (otherwise
    /// the left value is kept); `ortho` — from the left point.
    pub fn params_at(&self, t: Tick) -> CameraParams {
        if !self.auto.is_empty() {
            // general mechanism (R.34 K3): a curve per parameter, position —
            // tick from the start of the event
            let local = Tick(t.0 - self.start.0);
            let mut cam = self.cam;
            for c in &self.auto {
                if c.keys.is_empty() {
                    continue;
                }
                cam.set_param(&c.param, curve_value(&c.keys, local));
            }
            return cam;
        }
        if self.keyframes.is_empty() {
            return self.cam;
        }
        let span = self.duration.0.max(1);
        let u = ((t.0 - self.start.0) as f64 / span as f64).clamp(0.0, 1.0);
        let keys = &self.keyframes;
        if u <= keys[0].t {
            return keys[0].cam;
        }
        for w in keys.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if u <= b.t {
                let seg = (b.t - a.t).max(1e-9);
                let mut s = ((u - a.t) / seg).clamp(0.0, 1.0);
                if self.ease {
                    s = s * s * (3.0 - 2.0 * s); // smoothstep01
                }
                let l = |x: f64, y: f64| x + (y - x) * s;
                return CameraParams {
                    pos_x: l(a.cam.pos_x, b.cam.pos_x),
                    pos_y: l(a.cam.pos_y, b.cam.pos_y),
                    pos_z: l(a.cam.pos_z, b.cam.pos_z),
                    target: match (a.cam.target, b.cam.target) {
                        (Some(ta), Some(tb)) => Some((l(ta.0, tb.0), l(ta.1, tb.1), l(ta.2, tb.2))),
                        _ => a.cam.target,
                    },
                    rot_x: l(a.cam.rot_x, b.cam.rot_x),
                    rot_y: l(a.cam.rot_y, b.cam.rot_y),
                    rot_z: l(a.cam.rot_z, b.cam.rot_z),
                    fov_deg: l(a.cam.fov_deg, b.cam.fov_deg),
                    ortho: a.cam.ortho,
                };
            }
        }
        keys.last().unwrap().cam
    }
}

/// Active camera from the list (R.29 rule — see Project::active_camera_at;
/// pub function, so that the engine applies THE SAME rule to its snapshot).
pub fn active_camera_in(cameras: &[CameraEvent], t: Tick) -> Option<&CameraEvent> {
    cameras
        .iter()
        .filter(|c| c.start.0 <= t.0 && t.0 < (c.start + c.duration).0)
        .max_by_key(|c| (std::cmp::Reverse(c.lane), c.start.0, c.id.0))
}

/// Default vertical FOV — a moderate perspective in the spirit of the 50-mm
/// AE camera; exact equivalence to millimetres is not promised (we do not drag
/// cinema arithmetic into the model).
fn default_fov() -> f64 {
    39.6
}

fn default_half() -> f64 {
    0.5
}

fn default_zoom() -> f64 {
    1.0
}
fn default_one() -> f64 {
    1.0
}
fn is_one(v: &f64) -> bool {
    *v == 1.0
}
fn default_one_u32() -> u32 {
    1
}
fn default_255() -> u8 {
    255
}
fn default_blur_radius() -> u32 {
    8
}
fn default_tolerance() -> u32 {
    70
}
fn default_softness() -> u32 {
    25
}
fn default_edges_threshold() -> u32 {
    48
}
fn default_noise_amount() -> f64 {
    0.08
}
fn default_rgb_shift() -> u32 {
    4
}
fn default_bleed() -> f64 {
    0.4
}
fn default_glow_strength() -> f64 {
    0.65
}

fn default_damp() -> f32 {
    0.2
}
fn default_decay() -> f32 {
    0.84
}
fn default_wet_gain() -> f32 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlowedKeyframe {
    pub t: f64,
    pub rate: f64,
    pub mix: f32,
}

impl FxInstance {
    /// Constructor with default reverb parameters (for tests/simple cases).
    /// Effect constructors with DEFAULTS for added fields (R.33, stage 5).
    /// Struct literals in calling code were replaced with them, so that
    /// the next parameter does not require editing fifty places.
    /// Default constructors for kinds without their own constructor (R.47,
    /// item 326): needed by the registry `all()`; literals in tests go through them.
    pub fn filter() -> FxInstance {
        FxInstance::Filter { mode: FilterMode::Lowpass, cutoff_hz: 800.0, resonance: 0.707 }
    }

    pub fn eq() -> FxInstance {
        FxInstance::Eq {
            low_gain_db: 0.0,
            low_freq_hz: 200.0,
            mid_gain_db: 0.0,
            mid_freq_hz: 1000.0,
            mid_q: 1.0,
            high_gain_db: 0.0,
            high_freq_hz: 4000.0,
        }
    }

    pub fn distortion() -> FxInstance {
        FxInstance::Distortion { drive_db: 12.0, tone: 0.5, mix: 1.0 }
    }

    pub fn bitcrusher() -> FxInstance {
        FxInstance::Bitcrusher { bits: 8.0, rate_hz: 12_000.0, mix: 1.0 }
    }

    pub fn stereo_width() -> FxInstance {
        FxInstance::StereoWidth { width: 1.0 }
    }

    pub fn tremolo() -> FxInstance {
        FxInstance::Tremolo { rate_hz: 5.0, depth: 0.6 }
    }

    pub fn auto_pan() -> FxInstance {
        FxInstance::AutoPan { rate_hz: 1.0, depth: 0.8 }
    }

    pub fn vibrato() -> FxInstance {
        FxInstance::Vibrato { rate_hz: 5.0, depth_ms: 0.5 }
    }

    pub fn chorus() -> FxInstance {
        FxInstance::Chorus { rate_hz: 0.8, depth_ms: 3.5, mix: 0.5, voices: 2 }
    }

    pub fn flanger() -> FxInstance {
        FxInstance::Flanger { rate_hz: 0.25, depth_ms: 2.0, feedback: 0.5, mix: 0.5 }
    }

    pub fn phaser() -> FxInstance {
        FxInstance::Phaser { rate_hz: 0.4, depth: 0.7, feedback: 0.3, stages: 4, mix: 0.5 }
    }

    pub fn delay() -> FxInstance {
        FxInstance::Delay { time_ms: 350.0, feedback: 0.35, mix: 0.35, pingpong: false }
    }

    pub fn compressor() -> FxInstance {
        FxInstance::Compressor {
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 150.0,
            makeup_db: 0.0,
            key: None,
        }
    }

    pub fn gate() -> FxInstance {
        FxInstance::Gate { threshold_db: -40.0, attack_ms: 1.0, hold_ms: 40.0, release_ms: 100.0 }
    }

    pub fn limiter() -> FxInstance {
        FxInstance::Limiter {
            threshold_db: -6.0,
            ceiling_db: -0.3,
            lookahead_ms: 5.0,
            release_ms: 80.0,
        }
    }

    pub fn pitch_shift() -> FxInstance {
        FxInstance::PitchShift { semitones: 0.0, cents: 0.0 }
    }

    pub fn color_grade() -> FxInstance {
        FxInstance::ColorGrade { brightness: 0.0, contrast: 0.0, saturation: 1.0, hue: 0.0 }
    }

    pub fn duotone() -> FxInstance {
        FxInstance::Duotone {
            shadow_r: 24,
            shadow_g: 18,
            shadow_b: 64,
            high_r: 255,
            high_g: 236,
            high_b: 190,
            amount: 1.0,
        }
    }

    pub fn posterize() -> FxInstance {
        FxInstance::Posterize { levels: 6 }
    }

    pub fn vignette() -> FxInstance {
        FxInstance::Vignette { strength: 0.5, radius: 0.7, softness: 0.45 }
    }

    pub fn corner_pin() -> FxInstance {
        FxInstance::CornerPin {
            x0: 0.0,
            y0: 0.0,
            x1: 1.0,
            y1: 0.0,
            x2: 0.0,
            y2: 1.0,
            x3: 1.0,
            y3: 1.0,
        }
    }

    pub fn sharpen() -> FxInstance {
        FxInstance::Sharpen { amount: 1.0, radius: 2 }
    }

    pub fn pixelate() -> FxInstance {
        FxInstance::Pixelate { size: 12 }
    }

    pub fn chromatic_aberration() -> FxInstance {
        FxInstance::ChromaticAberration { amount: 0.012 }
    }

    pub fn radial_blur() -> FxInstance {
        FxInstance::RadialBlur { amount: 0.25, center_x: 0.0, center_y: 0.0, spin: false }
    }

    pub fn drop_shadow() -> FxInstance {
        FxInstance::DropShadow {
            offset_x: 8,
            offset_y: 8,
            radius: 4,
            opacity: 0.6,
            tint_r: 0,
            tint_g: 0,
            tint_b: 0,
        }
    }

    pub fn halftone() -> FxInstance {
        FxInstance::Halftone { size: 8, angle: 45.0 }
    }

    pub fn lens_distort() -> FxInstance {
        FxInstance::LensDistort { k1: 0.35, k2: 0.0 }
    }

    pub fn twirl() -> FxInstance {
        FxInstance::Twirl { angle: 120.0, radius: 0.8 }
    }

    pub fn ripple() -> FxInstance {
        FxInstance::Ripple { amount: 0.08, freq: 10.0, phase: 0.0 }
    }

    pub fn kaleidoscope() -> FxInstance {
        FxInstance::Kaleidoscope { segments: 6, angle: 0.0 }
    }

    pub fn cylinder3d() -> FxInstance {
        FxInstance::Cylinder3d { curvature: 0.5 }
    }

    /// Additional “Geometry” node with zoom around the centre (replacing the
    /// former “Pan: stretch, zoom z”).
    pub fn geometry_zoom(zoom: f64) -> FxInstance {
        let mut g = EventGeometry { zoom, ..Default::default() }.node();
        if let FxInstance::Geometry { lead, .. } = &mut g {
            *lead = false;
        }
        g
    }

    /// Additional “Geometry” node with position (canvas fractions) and zoom.
    pub fn geometry_at(pos_x: f64, pos_y: f64, zoom: f64) -> FxInstance {
        let mut g = EventGeometry { pos_x, pos_y, zoom, ..Default::default() }.node();
        if let FxInstance::Geometry { lead, .. } = &mut g {
            *lead = false;
        }
        g
    }

    pub fn geometry() -> FxInstance {
        EventGeometry::default().node()
    }

    /// Additional (non-leading) “Geometry” node from the catalog.
    pub fn geometry_node() -> FxInstance {
        let mut g = EventGeometry::default().node();
        if let FxInstance::Geometry { lead, .. } = &mut g {
            *lead = false;
        }
        g
    }

    /// Whether this is a leading “Geometry” node (materialized by the engine).
    pub fn is_lead_geometry(&self) -> bool {
        matches!(self, FxInstance::Geometry { lead: true, .. })
    }

    /// “Geometry” node with identity parameters. The conform rule is not part of it:
    /// whether the LEADING node is identity is decided by kt-fx from the source kind
    /// and the canvas (`geometry_is_identity_in`).
    pub fn geometry_is_identity(&self) -> bool {
        match self {
            FxInstance::Geometry {
                crop_l,
                crop_t,
                crop_r,
                crop_b,
                pos_x,
                pos_y,
                zoom,
                rot,
                ..
            } => {
                *crop_l == 0.0
                    && *crop_t == 0.0
                    && *crop_r == 0.0
                    && *crop_b == 0.0
                    && *pos_x == 0.0
                    && *pos_y == 0.0
                    && *zoom == 1.0
                    && *rot == 0.0
            }
            _ => false,
        }
    }

    pub fn cam_motion() -> FxInstance {
        FxInstance::CamMotion { mode: PanMode::Fit, ease: true, time_scale: 1.0, keyframes: vec![] }
    }

    pub fn mirror() -> FxInstance {
        FxInstance::Mirror { horizontal: true, vertical: false }
    }

    pub fn event_grid() -> FxInstance {
        FxInstance::EventGrid { cols: 2, rows: 2, center: true }
    }

    pub fn pan3d() -> FxInstance {
        FxInstance::Pan3d {
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            rot_x: 0.0,
            rot_y: 0.0,
            rot_z: 0.0,
            anchor_x: 0.5,
            anchor_y: 0.5,
            scale: 1.0,
            camera: None,
        }
    }

    pub fn blur(radius: u32) -> FxInstance {
        FxInstance::Blur { radius, radius_y: 0 }
    }

    pub fn edges(threshold: u32) -> FxInstance {
        FxInstance::Edges { threshold, thickness: 1, color_r: 255, color_g: 255, color_b: 255 }
    }

    pub fn gain(db: f32) -> FxInstance {
        FxInstance::Gain { db, pan: 0.0 }
    }

    pub fn vhs(rgb_shift: u32, bleed: f64) -> FxInstance {
        FxInstance::Vhs { rgb_shift, bleed, scan_step: 4, jitter: 0.0 }
    }

    pub fn glow(strength: f64) -> FxInstance {
        FxInstance::Glow {
            strength,
            radius: 4,
            threshold: 0,
            tint_r: 255,
            tint_g: 255,
            tint_b: 255,
        }
    }

    pub fn greenscreen(key: [u8; 3], tolerance: u32, softness: u32) -> FxInstance {
        FxInstance::Greenscreen {
            key_r: key[0],
            key_g: key[1],
            key_b: key[2],
            tolerance,
            softness,
            spill: 0.0,
        }
    }

    pub fn noise(amount: f64) -> FxInstance {
        FxInstance::Noise { amount, size: 1, color: false }
    }

    pub fn slowed_reverb(rate: f64, mix: f32, reverb_delay_ms: f32) -> FxInstance {
        FxInstance::SlowedReverb {
            rate,
            mix,
            reverb_delay_ms,
            damp: default_damp(),
            decay: default_decay(),
            wet_gain: default_wet_gain(),
            predelay_ms: 0.0,
            frames: FrameMode::Nearest,
            width: 1.0,
            automation: Vec::new(),
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ProjectError {
    #[error("unsupported format_version {0} (supported: {FORMAT_VERSION})")]
    UnsupportedVersion(u32),
    #[error("invalid json: {0}")]
    Json(String),
    #[error("clip {0:?}: duration must be positive")]
    NonPositiveDuration(ClipId),
    #[error("clip {0:?}: negative source_in")]
    NegativeSourceIn(ClipId),
    #[error("clip {0:?}: unknown media {1:?}")]
    UnknownMedia(ClipId, MediaId),
    #[error("duplicate clip id {0:?}")]
    DuplicateClipId(ClipId),
    #[error("clip {0:?}: id is in the range reserved for morpher slices")]
    ReservedClipId(ClipId),
    #[error("marked area: name must not be empty")]
    AreaUnnamed,
    #[error("marked area {0:?}: duration must be positive")]
    AreaEmpty(String),
    #[error("duplicate name {0:?} (areas and named ranges share one namespace)")]
    DuplicateName(String),
    #[error("duplicate media id {0:?}")]
    DuplicateMediaId(MediaId),
    #[error("settings: fps must divide {tick_hz}", tick_hz = kt_time::TICK_HZ)]
    BadFps,
    #[error("project conform rule must be a rule, not “follow project”")]
    BadConform,
    #[error("clip {0:?}: geometry position beyond the chain")]
    BadGeometryPos(ClipId),
    #[error("reel {0:?}: raster must be even 16..8192, fps a divisor of 90000 in 1..120")]
    BadSequenceSettings(MediaId),
    #[error("region: end must be greater than start")]
    BadRegion,
    #[error("duplicate morpher id {0:?}")]
    DuplicateMorphId(MorphId),
    #[error("morpher {0:?}: unknown target clip {1:?}")]
    MorphUnknownTarget(MorphId, ClipId),
    #[error("morpher {0:?}: unknown media {1:?}")]
    MorphUnknownMedia(MorphId, MediaId),
    #[error("duplicate transition id {0:?}")]
    DuplicateTransitionId(TransitionId),
    #[error("transition {0:?}: unknown clip {1:?}")]
    TransitionUnknownClip(TransitionId, ClipId),
    #[error("transition {0:?}: clips are neither butt-joined nor staggered-overlapping")]
    TransitionNotAtJoint(TransitionId),
    #[error("transition {0:?}: a third clip {1:?} sits inside the overlap window")]
    TransitionOverlapBusy(TransitionId, ClipId),
    #[error("transition {0:?}: window exceeds clip bodies")]
    TransitionTooWide(TransitionId),
    #[error("transition {0:?}: window below minimal duration")]
    TransitionTooShort(TransitionId),
    #[error("two transitions on joint {0:?}->{1:?}")]
    DuplicateTransitionJoint(Option<ClipId>, Option<ClipId>),
    #[error("transition {0:?}: no clip on either side")]
    TransitionWithoutClips(TransitionId),
    #[error("transition windows overlap on a lane: {0:?}, {1:?}")]
    TransitionWindowsOverlap(TransitionId, TransitionId),
    #[error("duplicate foreign key {0}")]
    DuplicateForeignKey(u64),
    #[error("clips overlap on a lane: {0:?}, {1:?} (crossfade is an explicit transition)")]
    ClipOverlap(ClipId, ClipId),
    #[error("clip {0:?} overlaps a foreign placeholder (key {1})")]
    ForeignOverlap(ClipId, u64),
    #[error("duplicate camera id {0:?}")]
    DuplicateCameraId(CameraId),
    #[error("camera {0:?}: duration must be positive")]
    CameraBadDuration(CameraId),
    #[error("camera {0:?}: fov must be in (0, 180)")]
    CameraBadFov(CameraId),
    #[error("camera {0:?}: keyframes must be sorted within [0, 1]")]
    CameraBadKeyframes(CameraId),
    #[error("clip {0:?}: fx automation keys must be finite and sorted within [0, 1]")]
    BadFxAuto(ClipId),
    #[error("transition {0:?}: parameter curves must be finite, sorted and known")]
    BadTransitionAuto(TransitionId),
    // --- nested sequences (R.50) ---
    #[error("sequence {0:?} would contain itself: {1}")]
    SequenceCycle(MediaId, String),
    #[error("sequence {0:?} ({1}) nests deeper than {MAX_NEST_DEPTH} levels")]
    SequenceTooDeep(MediaId, String),
    #[error("sequence {0:?}: name must not be empty")]
    SequenceUnnamed(MediaId),
    #[error("sequence {0:?}: morph layers inside a reel cannot target areas or named ranges")]
    MorphRefInsideSequence(MorphId),
    #[error("media id {0:?} is out of range (top bit is reserved for nested sequences)")]
    MediaIdTooLarge(MediaId),
    // --- in-place reel expansion (R.55, stage 5) ---
    #[error("clip {0:?} crossed the expanded band boundary: an event cannot change reels")]
    BandEscape(ClipId),
    // --- object tracks (R.62, change B) ---
    #[error("reel {0:?}: {1:?} track {2} does not exist")]
    TrackMissing(Option<MediaId>, LaneKind, u32),
    #[error("track {0:?}: curves on a track are not supported yet (timeline-time domain)")]
    TrackCurvesUnsupported(TrackId),
    // --- source of curve values (R.59, item 466) ---
    #[error(
        "clip {0:?}: this curve is driven by a formula or a link, and nothing evaluates those yet"
    )]
    CurveSourceUnsupported(ClipId),
    #[error("clip {0:?}: the curve source cannot be resolved: {1}")]
    CurveSourceBroken(ClipId, String),
    // --- property link (R.61) ---
    #[error("{0}")]
    BadLink(#[from] crate::links::LinkError),
    // --- segment shape (R.61) ---
    #[error(
        "clip {0:?}: the speed curve takes the linear shape only — the closed-form warp map \
         stands on a piecewise-linear profile"
    )]
    RateShapeMustBeLinear(ClipId),
    // --- generator parameters on an event (R.59, stage 2) ---
    #[error("clip {0:?} sits on a generator but carries no generator values")]
    GenMissing(ClipId),
    #[error("clip {0:?} carries generator values but its media is a file")]
    GenOnFile(ClipId),
    #[error("clip {0:?}: generator values are of a different kind than the media says")]
    GenKindMismatch(ClipId),
    #[error("clip {0:?}: generator curve '{1}' names no parameter of this kind")]
    GenAutoOrphan(ClipId, String),
    // --- public composition parameters (R.59, stage 3) ---
    #[error("reel {0:?}: duplicate published parameter id {1:?}")]
    PublishedDuplicateId(Option<MediaId>, PubId),
    #[error("reel {0:?}: published parameter {1:?} has an empty name")]
    PublishedUnnamed(Option<MediaId>, PubId),
    #[error("reel {0:?}: published parameter {1:?} points at a property that no longer exists")]
    PublishedTargetMissing(Option<MediaId>, PubId),
    #[error("reel {0:?}: published parameter {1:?} was declared with a different type")]
    PublishedTypeMismatch(Option<MediaId>, PubId),
    #[error("clip {0:?} carries overrides but does not sit on a reel")]
    OverrideOnPlainClip(ClipId),
    #[error("clip {0:?}: override {1:?} names nothing the reel published")]
    OverrideDangling(ClipId, PubId),
    #[error("clip {0:?}: override {1:?} carries a value of another type")]
    OverrideTypeMismatch(ClipId, PubId),
    #[error("clip {0:?}: override {1:?} carries a curve this property cannot take")]
    OverrideCurveNotAllowed(ClipId, PubId),
    #[error("clip {0:?}: override {1:?} has curve keys that are not finite and sorted")]
    OverrideBadCurve(ClipId, PubId),
    #[error("clip {0:?}: duplicate override {1:?}")]
    OverrideDuplicate(ClipId, PubId),
}

/// Full check of a transition against the project: junction, fit, minimum,
/// uniqueness of the junction and non-overlap of windows on the track. `ignore` — the id of the
/// transition itself during SetTransition (do not compare with itself; Add passes None,
/// but the self-id is skipped too). Used by both validate() and the check_on
/// of commands (test 135).
pub fn check_transition_full(
    p: &dyn Reel,
    t: &TransitionEvent,
    ignore: Option<TransitionId>,
) -> Result<(), ProjectError> {
    let clip = |id: ClipId| p.clips().iter().find(|c| c.id == id);
    let find = |side: Option<ClipId>| -> Result<Option<&ClipEvent>, ProjectError> {
        match side {
            None => Ok(None),
            Some(id) => Ok(Some(clip(id).ok_or(ProjectError::TransitionUnknownClip(t.id, id))?)),
        }
    };
    let a = find(t.a)?;
    let b = find(t.b)?;
    // fade (R.25): one end is empty; the offset toward it by the body is not
    // limited, the junction is taken from the live clip. Since R.51 (item 399) a pair can
    // also be OVERLAPPED “staircase” — then the window is the overlap zone, and the
    // offsets are not used at all (dead weight until it goes back to a butt joint).
    let mut staggered = false;
    let (kind, lane, joint) = match (a, b) {
        (Some(a), Some(b)) => {
            let (a_end, b_end) = (a.start + a.duration, b.start + b.duration);
            if a.kind != b.kind || a.lane != b.lane {
                return Err(ProjectError::TransitionNotAtJoint(t.id));
            }
            if a_end == b.start {
                (a.kind, a.lane, a_end)
            } else if a.start < b.start && b.start < a_end && a_end < b_end {
                staggered = true;
                (a.kind, a.lane, a_end)
            } else {
                // full containment, equal starts, reverse order or a gap — such a pair has
                // neither direction nor a window
                return Err(ProjectError::TransitionNotAtJoint(t.id));
            }
        }
        (Some(a), None) => (a.kind, a.lane, a.start + a.duration),
        (None, Some(b)) => (b.kind, b.lane, b.start),
        (None, None) => return Err(ProjectError::TransitionWithoutClips(t.id)),
    };
    if staggered {
        // the window is set by the BODIES of the pair; a third clip in it breaks not the transition but
        // the surroundings (1/N zones remain without transitions — just as three without a
        // transition give 1/N, item 162).
        let (a, b) = (a.unwrap(), b.unwrap());
        let w = TickRange::new(b.start, a.start + a.duration);
        if w.end - w.start < MIN_TRANSITION_DURATION {
            return Err(ProjectError::TransitionTooShort(t.id));
        }
        for c in p.clips() {
            if c.id != a.id
                && c.id != b.id
                && c.kind == kind
                && c.lane == lane
                && c.start < w.end
                && w.start < c.start + c.duration
            {
                return Err(ProjectError::TransitionOverlapBusy(t.id, c.id));
            }
        }
    } else {
        if t.in_offset.0 < 0
            || t.out_offset.0 < 0
            || a.is_some_and(|a| t.in_offset > a.duration)
            || b.is_some_and(|b| t.out_offset > b.duration)
        {
            return Err(ProjectError::TransitionTooWide(t.id));
        }
        if t.in_offset + t.out_offset < MIN_TRANSITION_DURATION {
            return Err(ProjectError::TransitionTooShort(t.id));
        }
    }
    // curves of the kind's parameters — the same rules as for effect curves
    for c in &t.auto {
        let sorted = c.keys.windows(2).all(|w| w[0].t <= w[1].t);
        let ok = c.keys.iter().all(|k| k.t.0 >= 0 && k.v.is_finite());
        if !sorted || !ok || t.kind.param_spec(&c.param).is_none() {
            return Err(ProjectError::BadTransitionAuto(t.id));
        }
    }
    let w = if staggered {
        TickRange::new(b.unwrap().start, joint)
    } else {
        TickRange::new(joint - t.in_offset, joint + t.out_offset)
    };
    for other in p.transitions() {
        if other.id == t.id || Some(other.id) == ignore {
            continue;
        }
        if (other.a, other.b) == (t.a, t.b) {
            return Err(ProjectError::DuplicateTransitionJoint(t.a, t.b));
        }
        let Some(oc) = other.a.or(other.b).and_then(clip) else { continue };
        let Some(ow) = other.window(p) else { continue };
        if oc.kind == kind && oc.lane == lane && w.start < ow.end && ow.start < w.end {
            return Err(ProjectError::TransitionWindowsOverlap(t.id, other.id));
        }
    }
    Ok(())
}

impl Project {
    pub fn new(settings: ProjectSettings) -> Project {
        Project {
            format_version: FORMAT_VERSION,
            settings,
            published: Vec::new(),
            media: Vec::new(),
            clips: Vec::new(),
            markers: Vec::new(),
            region: None,
            loop_enabled: false,
            tracks: vec![
                Track::new(TrackId(1), LaneKind::Video),
                Track::new(TrackId(2), LaneKind::Audio),
            ],
            legacy_video_lanes: 0,
            legacy_audio_lanes: 0,
            legacy_morph_lanes: 0,
            legacy_lane_order: Vec::new(),
            morphers: Vec::new(),
            transitions: Vec::new(),
            foreign: Vec::new(),
            sequences: Vec::new(),
            ui: None,
            overlap_layers: true,
            trim_stretches_auto: false,
            timeline_end: None,
            legacy_camera_lanes: 0,
            cameras: Vec::new(),
            areas: Vec::new(),
        }
    }

    pub fn lanes(&self, kind: LaneKind) -> u32 {
        self.tracks.iter().filter(|t| t.kind == kind).count() as u32
    }

    pub fn track_at(&self, kind: LaneKind, lane: u32) -> Option<&Track> {
        track_index(&self.tracks, kind, lane).map(|i| &self.tracks[i])
    }

    pub fn track_at_mut(&mut self, kind: LaneKind, lane: u32) -> Option<&mut Track> {
        track_index(&self.tracks, kind, lane).map(move |i| &mut self.tracks[i])
    }

    /// ROOT track by id (working view: the contents of the current reel stand as the root).
    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    /// Track anywhere in the document (root or reel).
    pub fn find_track(&self, id: TrackId) -> Option<&Track> {
        self.track(id)
            .or_else(|| self.sequences.iter().find_map(|s| s.tracks.iter().find(|t| t.id == id)))
    }

    pub fn track_label(&self, kind: LaneKind, lane: u32) -> String {
        self.track_at(kind, lane)
            .map(|t| t.label(lane))
            .unwrap_or_else(|| default_lane_label(kind, lane))
    }

    /// Append kind tracks up to `n` in number (at the end of the rows).
    pub fn ensure_tracks(&mut self, kind: LaneKind, n: u32) {
        while self.lanes(kind) < n {
            let at = natural_insert_pos(&self.tracks, kind);
            self.tracks.insert(at, Track::new(TrackId(0), kind));
        }
        self.assign_track_ids();
    }

    /// First free track id in the WHOLE document.
    pub fn next_track_id(&self) -> TrackId {
        let m = self
            .tracks
            .iter()
            .chain(self.sequences.iter().flat_map(|s| s.tracks.iter()))
            .map(|t| t.id.0)
            .max()
            .unwrap_or(0);
        TrackId(m + 1)
    }

    /// Assign an id to all tracks without one (and resolve duplicates): the root,
    /// then reels, in row order — deterministic. Idempotent.
    pub fn assign_track_ids(&mut self) {
        let mut seen = std::collections::HashSet::new();
        let mut next = self.next_track_id().0;
        let mut fix = |t: &mut Track| {
            if t.id.0 == 0 || !seen.insert(t.id) {
                t.id = TrackId(next);
                seen.insert(t.id);
                next += 1;
            }
        };
        for t in &mut self.tracks {
            fix(t);
        }
        for s in &mut self.sequences {
            for t in &mut s.tracks {
                fix(t);
            }
        }
    }

    /// Insert a track of kind `kind` with rank `at` (the row goes right above the former
    /// `at`; if there is no `at`, at the end); content of the kind with
    /// number ≥ at shifts by +1. Returns the id of the new track.
    pub fn insert_track(&mut self, kind: LaneKind, at: u32) -> TrackId {
        let at = at.min(self.lanes(kind));
        for c in &mut self.clips {
            if lane_key(c).0 == kind && c.lane >= at {
                c.lane += 1;
            }
        }
        for m in &mut self.morphers {
            if kind == LaneKind::Morph && m.lane >= at {
                m.lane += 1;
            }
        }
        for c in &mut self.cameras {
            if kind == LaneKind::Camera && c.lane >= at {
                c.lane += 1;
            }
        }
        for f in &mut self.foreign {
            if lane_key_of(f.kind) == kind && f.lane >= at {
                f.lane += 1;
            }
        }
        for a in &mut self.areas {
            for e in a.lanes.iter_mut() {
                if e.0 == kind && e.1 >= at {
                    e.1 += 1;
                }
            }
        }
        let id = self.next_track_id();
        // the row goes above the former `at`; if rank `at` is absent — at the end of the block
        // of its own kind (natural order), not at the end of the list
        let pos = track_index(&self.tracks, kind, at)
            .unwrap_or_else(|| natural_insert_pos(&self.tracks, kind));
        self.tracks.insert(pos, Track::new(id, kind));
        id
    }

    /// Delete the track of kind `kind` with rank `at` together with its content;
    /// higher numbers shift by −1.
    pub fn remove_track(&mut self, kind: LaneKind, at: u32) {
        let Some(pos) = track_index(&self.tracks, kind, at) else { return };
        self.tracks.remove(pos);
        match kind {
            LaneKind::Video | LaneKind::Audio => {
                self.clips.retain(|c| !(lane_key(c).0 == kind && c.lane == at));
                self.foreign.retain(|f| !(lane_key_of(f.kind) == kind && f.lane == at));
            }
            LaneKind::Morph => self.morphers.retain(|m| m.lane != at),
            LaneKind::Camera => self.cameras.retain(|c| c.lane != at),
        }
        for c in &mut self.clips {
            if lane_key(c).0 == kind && c.lane > at {
                c.lane -= 1;
            }
        }
        for m in &mut self.morphers {
            if kind == LaneKind::Morph && m.lane > at {
                m.lane -= 1;
            }
        }
        for c in &mut self.cameras {
            if kind == LaneKind::Camera && c.lane > at {
                c.lane -= 1;
            }
        }
        for f in &mut self.foreign {
            if lane_key_of(f.kind) == kind && f.lane > at {
                f.lane -= 1;
            }
        }
        for a in &mut self.areas {
            a.lanes.retain(|&(k, l)| !(k == kind && l == at));
            for e in a.lanes.iter_mut() {
                if e.0 == kind && e.1 > at {
                    e.1 -= 1;
                }
            }
        }
    }

    fn legacy(&self) -> LegacyLanes {
        LegacyLanes {
            video_lanes: self.legacy_video_lanes,
            audio_lanes: self.legacy_audio_lanes,
            morph_lanes: self.legacy_morph_lanes,
            camera_lanes: self.legacy_camera_lanes,
            lane_order: self.legacy_lane_order.clone(),
        }
    }

    fn clear_legacy(&mut self) {
        self.legacy_video_lanes = 0;
        self.legacy_audio_lanes = 0;
        self.legacy_morph_lanes = 0;
        self.legacy_camera_lanes = 0;
        self.legacy_lane_order = Vec::new();
    }

    /// How many tracks of each kind the root content REQUIRES.
    fn needed(&self) -> LaneBases {
        let mut n = LaneBases::default();
        for c in &self.clips {
            let k = lane_key(c).0;
            *n.get_mut(k) = n.get(k).max(c.lane + 1);
        }
        for f in &self.foreign {
            let k = lane_key_of(f.kind);
            *n.get_mut(k) = n.get(k).max(f.lane + 1);
        }
        for m in &self.morphers {
            n.morph = n.morph.max(m.lane + 1);
        }
        for c in &self.cameras {
            n.camera = n.camera.max(c.lane + 1);
        }
        n
    }

    /// Active camera at moment t: the event whose [start, start+duration)
    /// covers t; priority — the smaller lane (like video z-order); if equal,
    /// the larger start (“the one that comes later is on top”, the Vegas rule
    /// R.28/item 177), then the larger id. The rule is deterministic — the render
    /// depends on it (test 188). None — there are no cameras; the engine takes
    /// the default “50 mm” (identity for Pan3d with defaults).
    pub fn active_camera_at(&self, t: Tick) -> Option<&CameraEvent> {
        active_camera_in(&self.cameras, t)
    }

    /// Order of rows from top to bottom, normalized to the actual track counts.
    /// The only way to know what is “lower”.
    pub fn rows(&self) -> Vec<(LaneKind, u32)> {
        rows_of_tracks(&self.tracks)
    }

    /// All occupied names of ONE space: areas and range markers.
    /// Empty marker names are legal and are not part of the space.
    pub fn taken_names(&self) -> Vec<String> {
        self.areas
            .iter()
            .map(|a| a.name.clone())
            .chain(self.markers.iter().filter(|m| !m.name.is_empty()).map(|m| m.name.clone()))
            .collect()
    }

    /// Area by name.
    pub fn area(&self, name: &str) -> Option<&MarkedArea> {
        self.areas.iter().find(|a| a.name == name)
    }

    /// Range marker by name (point markers do not define a window).
    pub fn named_range(&self, name: &str) -> Option<TickRange> {
        self.markers
            .iter()
            .find(|m| m.name == name && m.duration.0 > 0)
            .map(|m| TickRange::new(m.t, m.t + m.duration))
    }

    /// Row index in track order; None — there is no such track.
    pub fn row_of(&self, key: (LaneKind, u32)) -> Option<usize> {
        self.rows().iter().position(|&k| k == key)
    }

    /// Reorder rows — the ONLY door for permutation (R.56, item 449;
    /// R.62: rows are the vector of tracks).
    ///
    /// `order` names the rows by CURRENT numbers; object tracks are
    /// reordered into this order, and the content is renumbered so that the number inside the kind
    /// equals the rank. Duplicates and nonsense are discarded, missing ones are appended to the end.
    /// Returns the permutation of numbers.
    pub fn set_lane_order(&mut self, order: &[(LaneKind, u32)]) -> LanePerm {
        let n = LaneBases::of_root(self);
        let norm = normalize_lane_order(order, n.video, n.audio, n.morph, n.camera);
        let perm = LanePerm::from_rows(&norm, n);
        let old = std::mem::take(&mut self.tracks);
        self.tracks = norm
            .iter()
            .filter_map(|&(k, l)| track_index(&old, k, l).map(|i| old[i].clone()))
            .collect();
        if !perm.is_identity() {
            for c in &mut self.clips {
                c.lane = perm.map_clip(c.kind, c.lane);
            }
            for m in &mut self.morphers {
                m.lane = perm.map(LaneKind::Morph, m.lane);
            }
            for c in &mut self.cameras {
                c.lane = perm.map(LaneKind::Camera, c.lane);
            }
            for f in &mut self.foreign {
                f.lane = perm.map_clip(f.kind, f.lane);
            }
            // a region is addressed by SEMANTIC track keys — so it travels with the renumbering,
            // otherwise “the layer on these tracks” would move onto someone else's tracks
            for a in &mut self.areas {
                perm.remap_keys(&mut a.lanes);
            }
        }
        perm
    }

    /// Tracks from the legacy fields of the ROOT and all reels (counters, `lane_order`,
    /// `ui.lane_names/lane_heights`) — on READ, one-way (R.62);
    /// then append tracks up to the content. Returns the permutation of the
    /// ROOT: the format reader needs it to reorder its own
    /// side tables (foreign OTIO tracks in kt-otio).
    pub fn canonicalize_lanes(&mut self) -> LanePerm {
        for s in &mut self.sequences {
            s.canonicalize_lanes();
        }
        let need = self.needed();
        let perm = if self.tracks.is_empty() {
            let legacy = self.legacy();
            let (names, heights) = self
                .ui
                .as_ref()
                .map(|u| (u.lane_names.clone(), u.lane_heights.clone()))
                .unwrap_or_default();
            self.tracks = tracks_from_legacy(&legacy, need, &names, &heights);
            self.clear_legacy();
            self.set_lane_order(&legacy.lane_order)
        } else {
            self.clear_legacy();
            LanePerm::identity(LaneBases::of_root(self))
        };
        if let Some(u) = &mut self.ui {
            u.lane_names = Vec::new();
            u.lane_heights = Vec::new();
            u.lane_order = Vec::new();
        }
        self.ensure_tracks(LaneKind::Video, need.video.max(1));
        self.ensure_tracks(LaneKind::Audio, need.audio.max(1));
        self.ensure_tracks(LaneKind::Morph, need.morph);
        self.ensure_tracks(LaneKind::Camera, need.camera);
        self.assign_track_ids();
        perm
    }

    /// Clips after the morphers are applied are the ONLY exit point of clips to the
    /// engine, export and drawing (v1 effectiveClipForRender/renderClips):
    /// the engine knows nothing about morphers. The order of application is deterministic —
    /// by ascending MorphId (creation order; in v1 the order of the vector was
    /// implicit — defect D11 of the reconnaissance). Several morphers on one clip
    /// are applied cumulatively.
    pub fn effective_clips(&self) -> Vec<ClipEvent> {
        // fast path: GOLDEN relies on it
        if self.morphers.is_empty() && self.sequences.is_empty() {
            return self.clips.clone();
        }
        let out = self.morph_pass(self, &self.rows());
        if self.sequences.is_empty() {
            return out;
        }
        self.inline_pass(out)
    }

    /// Morpher layers of ONE reel. Moved out of `effective_clips` (R.50):
    /// one can put everything inside a nested reel, including layers, — so this pass too
    /// must be able to work on a reel, not only on the root.
    fn morph_pass(&self, reel: &dyn Reel, rows: &[(LaneKind, u32)]) -> Vec<ClipEvent> {
        // Derived sources are resolved HERE — at the only door through which the reel's content
        // exits to the engine (R.61). A reel without links and formulas passes BORROWED:
        // no graph is built at all,
        // the bytes stay as before.
        let step = 90_000 / self.settings.fps.max(1) as i64;
        let resolved = crate::links::resolve_derived(reel.clips(), step);
        let ms_all = reel.morphers();
        if ms_all.is_empty() {
            return resolved.into_owned();
        }
        // frame step is needed to convert the layer's keys into the target's material
        let frame_step = 90_000 / self.settings.fps.max(1) as i64;
        let mut ms: Vec<&MorpherEvent> = ms_all.iter().filter(|m| m.enabled).collect();
        // Composition order is BOTTOM TO TOP (the lowest layer first, like an
        // adjustment layer in AE); within a row, the former rule by id applies.
        ms.sort_by_key(|m| (std::cmp::Reverse(m.row_in(rows)), m.id));
        let mut out = Vec::with_capacity(resolved.len());
        for c in resolved.iter() {
            let wins: Vec<Vec<TickRange>> =
                ms.iter().map(|m| m.windows_on(c, self, reel, rows)).collect();
            let body = TickRange::new(c.start, c.start + c.duration);
            // CUT POINTS ARE NOT GIVEN BY ALL LAYERS (R.55, stage 2). Only those whose
            // contribution cannot be switched off by a strength curve cut: source substitution and
            // time warp in the patch. A layer of effects alone does not produce pieces —
            // it lies over the whole body, and the time is set by the GATES. The cut set
            // is still COMMON to all cutting layers: otherwise they would cut
            // each other’s pieces.
            let mut cuts: Vec<i64> = ms
                .iter()
                .zip(&wins)
                .filter(|(m, _)| m.cuts_target(c))
                .flat_map(|(_, ws)| ws.iter())
                .flat_map(|w| [w.start.0, w.end.0])
                .filter(|&x| x > body.start.0 && x < body.end.0)
                .collect();
            cuts.sort_unstable();
            cuts.dedup();
            let mut bounds = Vec::with_capacity(cuts.len() + 2);
            bounds.push(body.start.0);
            bounds.extend(cuts);
            bounds.push(body.end.0);
            for (idx, pair) in bounds.windows(2).enumerate() {
                let w = TickRange::new(Tick(pair[0]), Tick(pair[1]));
                // piece no. 0 keeps the id of the original — rule `Split`; if there is
                // no cutting, it is an identity copy of the clip
                let id = if idx == 0 { c.id } else { slice_id(c.id, idx as u32) };
                let mut piece = clip_slice(c, w, id);
                for (m, ws) in ms.iter().zip(&wins) {
                    if m.cuts_target(c) {
                        // a cutting layer lies over a piece WHOLE, and only if the piece is entirely
                        // inside its window (as before)
                        if ws.iter().any(|x| x.start <= w.start && w.end <= x.end) {
                            m.apply_to(&mut piece, frame_step, &[w]);
                        }
                        continue;
                    }
                    // GATES are computed PER PIECE, not from the source clip:
                    // the pieces were produced by OTHER layers, and a window computed from the start of
                    // the body would have shifted by the length of the cut-off part.
                    let gate: Vec<TickRange> =
                        ws.iter().map(|x| x.intersect(&w)).filter(|x| !x.is_empty()).collect();
                    if !gate.is_empty() {
                        m.apply_to(&mut piece, frame_step, &gate);
                    }
                }
                out.push(piece);
            }
        }
        out
    }

    /// Swap the contents of the root and the contents of the reel `seq`
    /// (R.50, B1). A repeated call with the same address restores the original.
    ///
    /// This is how ENTRY INSIDE is done. The alternative — carry the “current reel”
    /// through all sixty command handlers (`p.clips`, `p.transitions`,
    /// `p.video_lanes`, …): skip one — and the edit silently goes to the root
    /// while the person is looking inside. Here not a single command knows
    /// anything about nesting, and it should not: it edits the REEL, and which reel is
    /// at hand is a question of context.
    ///
    /// The exchange keeps the UNION of the contents unchanged: all events of the document
    /// are still visible, just in other containers. Therefore checks
    /// of the kind “is this file still used by someone” remain correct during
    /// editing inside the reel.
    pub fn swap_reel(&mut self, seq: MediaId) {
        let Some(i) = self.sequences.iter().position(|s| s.id == seq) else { return };
        let s = &mut self.sequences[i];
        std::mem::swap(&mut self.clips, &mut s.clips);
        std::mem::swap(&mut self.transitions, &mut s.transitions);
        std::mem::swap(&mut self.morphers, &mut s.morphers);
        std::mem::swap(&mut self.cameras, &mut s.cameras);
        std::mem::swap(&mut self.tracks, &mut s.tracks);
        std::mem::swap(&mut self.overlap_layers, &mut s.overlap_layers);
        // The public set is the CONTENT of the reel: its targets are its clips, and
        // they now move to the root's place. If we did not swap it too, the publication
        // would point at clips that are no longer in this reel, and `reconcile_published`
        // would wipe it at the first command.
        std::mem::swap(&mut self.published, &mut s.published);
    }

    /// A document in which the REEL `seq` stands as the ROOT (R.56, item 452).
    ///
    /// It is needed by the offline count for the reel: the preview of its content is computed by the same
    /// composition code as everything else (invariant 7), and this code
    /// needs a project with the reel in place of the root. The trick is the same as in
    /// `reel_document`, only the reel is named explicitly rather than taken from
    /// “where the person stands”: the preview service does not take orders from a person.
    ///
    /// The reel itself is removed from the list — otherwise it would refer to itself and
    /// `validate` would see a cycle. The other reels remain: their instances may
    /// sit inside.
    pub fn reel_as_root(&self, seq: MediaId) -> Option<Project> {
        self.sequences.iter().find(|s| s.id == seq)?;
        let mut p = self.clone();
        p.swap_reel(seq);
        p.sequences.retain(|s| s.id != seq);
        Some(p)
    }

    /// The clips of ONE reel after morpher layers, WITHOUT expanding nested items
    /// (R.50, wave B). `None` — the root.
    ///
    /// This is how the engine builds the TREE of renderers: a nested item reaches it
    /// as one event and gets its own plan node — only this way does “an effect on the RESULT”
    /// become expressible. The expansion
    /// (`effective_clips`) remains the reference oracle: with it, the finished subtree
    /// is checked byte for byte (the law E).
    pub fn effective_reel(&self, seq: Option<MediaId>) -> Vec<ClipEvent> {
        self.resolve_reel(seq, &[]).clips
    }

    /// The same, but UNDER THE SET OF OVERRIDES of the instance (R.59, stage 3).
    ///
    /// An empty set must take the path byte-for-byte equal to the previous one: neither a clone
    /// of the reel nor a clone of the transitions and cameras (`Cow::Borrowed`). Resting on this
    /// are GOLDEN_VIDEO/GOLDEN_AUDIO and “zero cost without use”
    /// (t673/t676).
    ///
    /// Overrides are placed **BEFORE** the morpher layers — see the module
    /// `published`: the author's address is the structure of the reel as the author sees it.
    pub fn resolve_reel(&self, seq: Option<MediaId>, overrides: &[Override]) -> ResolvedReel<'_> {
        let reel: &dyn Reel = match seq {
            None => self,
            Some(id) => match self.sequence(id) {
                Some(s) => s,
                None => {
                    return ResolvedReel {
                        clips: Vec::new(),
                        transitions: std::borrow::Cow::Borrowed(&[]),
                        cameras: std::borrow::Cow::Borrowed(&[]),
                    };
                }
            },
        };
        let rows = reel.rows();
        let live = !overrides.is_empty()
            && reel.published().iter().any(|p| overrides.iter().any(|o| o.pub_id == p.id));
        if !live {
            return ResolvedReel {
                clips: self.morph_pass(reel, &rows),
                transitions: std::borrow::Cow::Borrowed(reel.transitions()),
                cameras: std::borrow::Cow::Borrowed(reel.cameras()),
            };
        }
        let Some(id) = seq else {
            // The root has no public parameters (it belongs to no instance), and it can
            // be reached here only by a caller's mistake
            debug_assert!(false, "the root reel is nobody's instance and takes no overrides");
            return ResolvedReel {
                clips: self.morph_pass(reel, &rows),
                transitions: std::borrow::Cow::Borrowed(reel.transitions()),
                cameras: std::borrow::Cow::Borrowed(reel.cameras()),
            };
        };
        let mut s = self.sequence(id).expect("checked above").clone();
        let frame_step = 90_000 / self.settings.fps.max(1) as i64;
        let published = s.published.clone();
        crate::published::apply_overrides(
            &published,
            overrides,
            &mut s.clips,
            &mut s.transitions,
            &mut s.cameras,
            frame_step,
            &|m| self.default_gen_for(m),
        );
        let clips = self.morph_pass(&s, &rows);
        ResolvedReel {
            clips,
            transitions: std::borrow::Cow::Owned(s.transitions),
            cameras: std::borrow::Cow::Owned(s.cameras),
        }
    }

    /// The LIVE value of the publication target — that very “template value”
    /// which we do NOT STORE (R.59, stage 3).
    ///
    /// `seq` — the reel inside which the address lives. `None` — the root (it has no
    /// publications, but reading the address is legal: this is how the publication
    /// window asks the target before the handle is set up).
    pub fn addr_value(&self, seq: Option<MediaId>, addr: &ParamAddr) -> Option<ParamValue> {
        let reel: &dyn Reel = match seq {
            None => self,
            Some(id) => self.sequence(id)?,
        };
        let name = addr.param.as_str();
        match &addr.owner {
            ParamOwner::Transition { id } => {
                reel.transitions().iter().find(|t| t.id == *id)?.kind.get_value(name)
            }
            ParamOwner::Camera { id } => {
                reel.cameras().iter().find(|c| c.id == *id)?.cam.get_value(name)
            }
            owner => {
                let c = reel.clips().iter().find(|c| c.id == addr.clip)?;
                match owner {
                    ParamOwner::FxSlot { slot } => c.fx.get(*slot)?.get_value(name),
                    ParamOwner::Generator => c.r#gen.as_ref()?.get_value(name),
                    ParamOwner::ClipAttr => crate::published::clip_attr_value(c, name),
                    // template within a template: the value of an inner instance is
                    // its override, and if there is none — the template value of ITS reel,
                    // by the same code one level down
                    ParamOwner::Nested { pub_id } => self.instance_value(c, *pub_id),
                    // an audio event has no geometry node at all: the address
                    // “does not exist”, and validation will say so in words
                    ParamOwner::Geometry => {
                        (c.kind == ClipKind::Video).then(|| c.geometry.get_value(name))?
                    }
                    ParamOwner::Transition { .. } | ParamOwner::Camera { .. } => None,
                }
            }
        }
    }

    /// Type of the target live — validation catches “the target changed kind” by it.
    pub fn addr_type(&self, seq: Option<MediaId>, addr: &ParamAddr) -> Option<ParamType> {
        let reel: &dyn Reel = match seq {
            None => self,
            Some(id) => self.sequence(id)?,
        };
        let name = addr.param.as_str();
        match &addr.owner {
            ParamOwner::Transition { id } => {
                reel.transitions().iter().find(|t| t.id == *id)?.kind.value_type(name)
            }
            ParamOwner::Camera { id } => {
                reel.cameras().iter().find(|c| c.id == *id)?.cam.value_type(name)
            }
            owner => {
                let c = reel.clips().iter().find(|c| c.id == addr.clip)?;
                match owner {
                    ParamOwner::FxSlot { slot } => c.fx.get(*slot)?.value_type(name),
                    ParamOwner::Generator => c.r#gen.as_ref()?.value_type(name),
                    ParamOwner::ClipAttr => crate::published::clip_attr_type(name),
                    ParamOwner::Nested { pub_id } => {
                        let inner = self.sequence(c.media)?;
                        inner.published.iter().find(|p| p.id == *pub_id).map(|p| p.ty.clone())
                    }
                    ParamOwner::Geometry => {
                        (c.kind == ClipKind::Video).then(|| c.geometry.value_type(name))?
                    }
                    ParamOwner::Transition { .. } | ParamOwner::Camera { .. } => None,
                }
            }
        }
    }

    /// The PASSPORT of the property at the address — a mirror of `addr_type` (R.60).
    ///
    /// The limits, step and default of the public handle are taken FROM HERE, not from
    /// zero: “the min/max values are always filled in as 0 by default” —
    /// an empty narrowing was drawn as zero, although the target's passport knows its own limits.
    /// The `match` on the owner is exhaustive — a new owner will not
    /// build until a branch is added here.
    pub fn addr_spec(&self, seq: Option<MediaId>, addr: &ParamAddr) -> Option<ParamSpec> {
        let reel: &dyn Reel = match seq {
            None => self,
            Some(id) => self.sequence(id)?,
        };
        let name = addr.param.as_str();
        match &addr.owner {
            ParamOwner::Transition { id } => {
                reel.transitions().iter().find(|t| t.id == *id)?.kind.value_spec(name)
            }
            ParamOwner::Camera { id } => {
                reel.cameras().iter().find(|c| c.id == *id)?.cam.value_spec(name)
            }
            owner => {
                let c = reel.clips().iter().find(|c| c.id == addr.clip)?;
                match owner {
                    ParamOwner::FxSlot { slot } => c.fx.get(*slot)?.value_spec(name),
                    ParamOwner::Generator => c.r#gen.as_ref()?.value_spec(name),
                    ParamOwner::ClipAttr => crate::published::clip_attr_spec(name),
                    ParamOwner::Nested { pub_id } => {
                        // Outward goes the passport of the TARGET of the inner handle,
                        // NARROWED by its own limits: the consumer of the external
                        // reel may not get more than
                        // the author of the inner one allowed.
                        let inner = self.sequence(c.media)?;
                        let p = inner.published.iter().find(|p| p.id == *pub_id)?;
                        let mut s = self.addr_spec(Some(c.media), &p.target)?;
                        if let Some(lo) = p.range.min {
                            s.min = s.min.max(lo);
                        }
                        if let Some(hi) = p.range.max {
                            s.max = s.max.min(hi);
                        }
                        s.default = s.default.clamp(s.min, s.max);
                        Some(s)
                    }
                    ParamOwner::Geometry => {
                        (c.kind == ClipKind::Video).then(|| c.geometry.value_spec(name))?
                    }
                    ParamOwner::Transition { .. } | ParamOwner::Camera { .. } => None,
                }
            }
        }
    }

    /// The TEMPLATE value of the declared handle: read from the target live.
    pub fn published_value(&self, seq: MediaId, id: PubId) -> Option<ParamValue> {
        let p = self.sequence(seq)?.published.iter().find(|p| p.id == id)?;
        self.addr_value(Some(seq), &p.target)
    }

    /// The EFFECTIVE value of the handle at the instance: the override, and if there is
    /// none — the template value. Exactly this is shown by the composition parameters window
    /// and exactly this goes into the picture.
    pub fn instance_value(&self, inst: &ClipEvent, id: PubId) -> Option<ParamValue> {
        if let Some(o) = inst.overrides.iter().find(|o| o.pub_id == id) {
            return Some(o.value.clone());
        }
        self.published_value(inst.media, id)
    }

    /// Expansion of nested items (R.50, item 383).
    ///
    /// A nested item with an EMPTY chain is semantically IDENTICAL to its expanded
    /// content: the settings are inherited, and no intermediate raster arises.
    /// So the picture and sound come out here without a single edit of the plan,
    /// workers, cache or GPU — and the inline itself becomes the REFERENCE ORACLE
    /// for the real subtree (law E, tests t434-t436).
    ///
    /// What it does NOT do, and does not pretend to do: a chain, warp and
    /// reverse on the instance itself would apply to each piece separately,
    /// not to the assembled picture — that is a different result, so the commands simply do NOT
    /// allow them on a nested item (rule 16). The level and gain
    /// are multiplied: on non-overlapping content this is the same thing, and
    /// overlap is already the business of the real subtree.
    fn inline_pass(&self, clips: Vec<ClipEvent>) -> Vec<ClipEvent> {
        // Free piece numbers: the morpher pass may already have cut the instance itself,
        // and new pieces must take numbers ABOVE its numbers —
        // otherwise two different pieces would get one address.
        let mut next: std::collections::HashMap<ClipId, u32> = std::collections::HashMap::new();
        for c in &clips {
            if let Some(o) = slice_origin(c.id) {
                let idx = (c.id.0 & ((1 << SLICE_IDX_BITS) - 1)) as u32;
                let e = next.entry(o).or_insert(0);
                *e = (*e).max(idx + 1);
            }
        }
        let mut out = Vec::with_capacity(clips.len());
        for c in &clips {
            self.inline_one(c, 0, &mut next, &mut out);
        }
        out
    }

    fn inline_one(
        &self,
        inst: &ClipEvent,
        depth: usize,
        next: &mut std::collections::HashMap<ClipId, u32>,
        out: &mut Vec<ClipEvent>,
    ) {
        let Some(seq) = self.sequence(inst.media) else {
            out.push(inst.clone());
            return;
        };
        // REVERSE AND SPEED ARE NOT EXPANDED (R.55, stage 4). Expansion is
        // the reference oracle of law E, and what `kt-cli probe` prints, and it is
        // legal exactly where the nested item is IDENTICAL to its content.
        // Mirror and resampler are applied to the ASSEMBLED material: expanding them
        // by pieces we would compute something else — the same argument as for
        // law G for the level (t438). Such a nested item remains an event.
        if inst.reversed || crate::chain_warps_time(&inst.active_fx(), &inst.active_fx_auto()) {
            out.push(inst.clone());
            return;
        }
        if depth >= MAX_NEST_DEPTH {
            return; // validate would not let such a thing through; here — a safety net
        }
        // instance window in content coordinates
        let win = TickRange::new(inst.source_in, inst.source_in + inst.duration);
        // INSTANCE OVERRIDES (R.59, stage 3) apply here too.
        // Expansion is the reference oracle of law E and what `kt-cli
        // probe` prints: if it showed the TEMPLATE, the oracle would check the subtree against the wrong
        // thing, and probe would answer the complaint “here is another title” with someone else's
        // text.
        let inner = self.resolve_reel(Some(seq.id), &inst.overrides).clips;
        // Piece addresses are computed from the REAL instance clip: this way
        // `same_clip` leads the piece back to the event the person clicked,
        // and `slice_id` does not overflow onto its own tag.
        let base = slice_origin(inst.id).unwrap_or(inst.id);
        for child in &inner {
            let body = TickRange::new(child.start, child.start + child.duration);
            let cut = body.intersect(&win);
            if cut.is_empty() || cut.len() < crate::MIN_CLIP_DURATION {
                continue;
            }
            let n = next.entry(base).or_insert(0);
            let id = slice_id(base, *n);
            *n += 1;
            let mut piece = clip_slice(child, cut, id);
            piece.start = inst.start + (cut.start - win.start);
            // The content track is placed UNDER the instance track — exactly
            // as `expand_instance` places it: the two paths must give one
            // picture, otherwise “collapse, then expand” would stop being
            // an identity.
            piece.lane = inst.lane + child.lane;
            piece.opacity *= inst.opacity;
            piece.gain_db += inst.gain_db;
            piece.link_group = inst.link_group;
            self.inline_one(&piece, depth + 1, next, out);
        }
    }

    /// Moving generator parameters from the MEDIA PATH to the EVENT (R.59,
    /// stage 2) — the ONLY place of migration.
    ///
    /// Files of R.52..R.58 kept values as a string `gen:text:t=…`, that is, they shared
    /// them between all places of use. Here the path
    /// is shortened to the KIND, and the values are handed out to events — each its own
    /// copy, exactly the one that was. The former animation mode of a title
    /// (`am=`/`ams=`) turns into curves by the same code as the preset in the
    /// window (`text_anim_curves`): if they diverged, an old project would move
    /// differently from the same button pressed by hand.
    ///
    /// An event that already has values is not touched: this is a file of the
    /// new format, and its path is already short.
    pub fn migrate_gen_to_events(&mut self) {
        use crate::genmedia::{
            gen_kind_path, parse_gen_path_full, text_anim_curves, text_anim_values,
        };
        let mut per_media: std::collections::HashMap<MediaId, crate::GeneratorKind> =
            std::collections::HashMap::new();
        let mut legacy: std::collections::HashMap<MediaId, Vec<ParamCurve>> =
            std::collections::HashMap::new();
        for m in &mut self.media {
            let Some((mut kind, anim)) = parse_gen_path_full(&m.path) else { continue };
            if let Some(a) = anim {
                // “a block no wider than the frame”: only the layout knows the exact
                // span, and it is not here — and this is exactly the case where
                // approximating is more honest than dragging kt-text into the model.
                legacy.insert(m.id, text_anim_curves(a.anim, a.secs, 2.0));
                text_anim_values(&mut kind, a.anim);
            }
            m.path = gen_kind_path(&kind);
            per_media.insert(m.id, kind);
        }
        if per_media.is_empty() {
            return;
        }
        let fill = |clips: &mut Vec<ClipEvent>| {
            for c in clips {
                if c.r#gen.is_some() {
                    continue;
                }
                let Some(k) = per_media.get(&c.media) else { continue };
                c.r#gen = Some(k.clone());
                if c.gen_auto.is_empty()
                    && let Some(curves) = legacy.get(&c.media)
                {
                    c.gen_auto = curves.clone();
                }
            }
        };
        fill(&mut self.clips);
        for s in &mut self.sequences {
            fill(&mut s.clips);
        }
    }

    /// Zones of a clip inside which the morpher layer boundary cannot be placed
    /// (finding 1 of R.35): the transition windows and the overlaps of neighbours on the track. A cut
    /// there would break NOT the target but the surroundings — for video the composition order is
    /// `(Reverse(lane), start)`, and a piece with a larger start would pop out above its
    /// neighbour; for audio, `overlap_weight` computes the ramp from the geometry of the clips.
    /// Normalization after loading: the number of tracks is pulled up to the
    /// actual lane of the clips (files written before the fields appeared).
    pub fn normalize(&mut self) -> LanePerm {
        // IMPORTANT: stable sort and dedup by time — with two markers on
        // one tick with different names an unstable sort would give a non-deterministic
        // order and break the byte stability of the project (test 12).
        self.markers.sort_by_key(|m| m.t);
        self.markers.dedup_by_key(|m| m.t);
        // generator parameters moved to the event (R.59, stage 2):
        // fixed on READ, because files of R.52..R.58 kept them in the media
        // path.
        self.migrate_gen_to_events();
        // track number = row rank (R.56, item 449): fixed on READ, because
        // old files (and foreign imports) know only numbers. The permutation is handed
        // outward — the format reader needs it for its own side maps
        // (foreign tracks in kt-otio).
        self.canonicalize_lanes()
    }

    /// Full check of the invariants. Load errors are EXPLICIT — no silent discarding
    /// of broken clips (defect p of the old format).
    pub fn validate(&self) -> Result<(), ProjectError> {
        if self.format_version != FORMAT_VERSION {
            return Err(ProjectError::UnsupportedVersion(self.format_version));
        }
        if self.settings.fps == 0 || kt_time::TICK_HZ % self.settings.fps as i64 != 0 {
            return Err(ProjectError::BadFps);
        }
        for c in self.reels().flat_map(|(_, r)| r.clips().iter()) {
            if c.geometry_pos > c.fx.len() {
                return Err(ProjectError::BadGeometryPos(c.id));
            }
        }
        for s in &self.sequences {
            if !s.settings_ok() {
                return Err(ProjectError::BadSequenceSettings(s.id));
            }
        }
        let mut media_ids = std::collections::HashSet::new();
        for m in &self.media {
            // the high bit of the address is given to nested reels (R.50): the engine
            // puts `SourceId(NEST_TAG | seq)` into the cache key, and a file with such an
            // id would silently share the cache with a sequence.
            if m.id.0 >= NEST_TAG {
                return Err(ProjectError::MediaIdTooLarge(m.id));
            }
            if !media_ids.insert(m.id) {
                return Err(ProjectError::DuplicateMediaId(m.id));
            }
        }
        // Area: the name is an ADDRESS, so it must be non-empty and unique in ONE space
        // together with the range markers (otherwise “area Finale” and “marker Finale” in one picker
        // would be a trap).
        let mut names = std::collections::HashSet::new();
        for a in &self.areas {
            if a.name.trim().is_empty() {
                return Err(ProjectError::AreaUnnamed);
            }
            if a.duration.0 <= 0 {
                return Err(ProjectError::AreaEmpty(a.name.clone()));
            }
            if !names.insert(a.name.clone()) {
                return Err(ProjectError::DuplicateName(a.name.clone()));
            }
        }
        for m in &self.markers {
            if !m.name.is_empty() && m.duration.0 > 0 && !names.insert(m.name.clone()) {
                return Err(ProjectError::DuplicateName(m.name.clone()));
            }
        }
        if let Some(r) = &self.region
            && r.end <= r.start
        {
            return Err(ProjectError::BadRegion);
        }
        // Tree of reels. Fast path on the first line: a flat project —
        // the vast majority — pays one `is_empty` check, and a live drag
        // (`apply_coalesced` calls validate on every frame of the gesture)
        // does not get more expensive by a single beat.
        let sources = if self.sequences.is_empty() {
            media_ids.clone()
        } else {
            self.validate_tree(&media_ids)?;
            media_ids.iter().copied().chain(self.sequences.iter().map(|s| s.id)).collect()
        };
        // The content of each reel — by one code path. `ClipId` is unique across the WHOLE
        // document, not within a reel: `Retained.clips`/`.transitions`
        // (kt-otio) are global maps by id, and coinciding ids from different reels
        // would SILENTLY exchange material, start timecode and effects.
        let mut clip_ids = std::collections::HashSet::new();
        for (seq, reel) in self.reels() {
            // TRACK EXISTS (R.62): the event's number is its rank among the tracks of
            // its kind, and an event on a non-existent track has no row on the screen —
            // “it can be created, it cannot be controlled”.
            // Reading appends tracks (`normalize`); validation rejects.
            let n = LaneBases::of_tracks(reel.tracks());
            for c in reel.clips() {
                let k = lane_key(c).0;
                if c.lane >= n.get(k) {
                    return Err(ProjectError::TrackMissing(seq, k, c.lane));
                }
            }
            for m in reel.morphers() {
                if m.lane >= n.morph {
                    return Err(ProjectError::TrackMissing(seq, LaneKind::Morph, m.lane));
                }
            }
            for c in reel.cameras() {
                if c.lane >= n.camera {
                    return Err(ProjectError::TrackMissing(seq, LaneKind::Camera, c.lane));
                }
            }
            if n.video == 0 || n.audio == 0 {
                return Err(ProjectError::TrackMissing(
                    seq,
                    if n.video == 0 { LaneKind::Video } else { LaneKind::Audio },
                    0,
                ));
            }
            // curves on a track — the next change (the time domain of the
            // timeline): they cannot be accepted silently in the material domain
            for t in reel.tracks() {
                if t.fx_auto.iter().any(|a| !a.is_empty()) {
                    return Err(ProjectError::TrackCurvesUnsupported(t.id));
                }
            }
            self.validate_reel(reel, seq, &sources, &media_ids, &mut clip_ids)?;
            // Links of ONE reel (R.61): the address is resolvable, does not cross the reel boundary,
            // there are no cycles. The check is per reel, not per project —
            // precisely because the address space is closed by the reel.
            crate::links::check_links(reel).map_err(ProjectError::BadLink)?;
        }
        Ok(())
    }

    /// Reels as a DOCUMENT: addresses, names, cycles, depth.
    fn validate_tree(
        &self,
        media_ids: &std::collections::HashSet<MediaId>,
    ) -> Result<(), ProjectError> {
        let mut seq_ids = std::collections::HashSet::new();
        for s in &self.sequences {
            if s.id.0 >= NEST_TAG {
                return Err(ProjectError::MediaIdTooLarge(s.id));
            }
            if s.name.trim().is_empty() {
                return Err(ProjectError::SequenceUnnamed(s.id));
            }
            // addresses of files and reels — ONE space: `clip.media`
            // looks into both there and there.
            if !seq_ids.insert(s.id) || media_ids.contains(&s.id) {
                return Err(ProjectError::DuplicateMediaId(s.id));
            }
        }
        let idx: std::collections::HashMap<MediaId, usize> =
            self.sequences.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
        let kids: Vec<Vec<usize>> = self
            .sequences
            .iter()
            .map(|s| {
                let mut v: Vec<usize> =
                    s.clips.iter().filter_map(|c| idx.get(&c.media).copied()).collect();
                v.sort_unstable();
                v.dedup();
                v
            })
            .collect();
        // Iterative DFS FROM ALL vertices, not only from the root: a pair of reels
        // A ↔ B that is not used in the edit anywhere is still a cycle — a file with
        // it would be saved and would not open. Iterative — because
        // recursion over a hostile file is a stack overflow, not an error.
        const WHITE: u8 = 0;
        const GRAY: u8 = 1;
        const BLACK: u8 = 2;
        let n = self.sequences.len();
        let mut mark = vec![WHITE; n];
        let mut depth = vec![1usize; n];
        for start in 0..n {
            if mark[start] != WHITE {
                continue;
            }
            mark[start] = GRAY;
            let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
            while let Some(&(node, ci)) = stack.last() {
                if ci < kids[node].len() {
                    stack.last_mut().unwrap().1 += 1;
                    let k = kids[node][ci];
                    match mark[k] {
                        GRAY => {
                            // The error text names the NAMES, not the addresses:
                            // “Opening -> Titles -> Opening” the person
                            // unwinds themselves; “seq 7 -> seq 3” — no.
                            let from =
                                stack.iter().position(|&(v, _)| v == k).unwrap_or(stack.len() - 1);
                            let mut path: Vec<&str> = stack[from..]
                                .iter()
                                .map(|&(v, _)| self.sequences[v].name.as_str())
                                .collect();
                            path.push(self.sequences[k].name.as_str());
                            return Err(ProjectError::SequenceCycle(
                                self.sequences[k].id,
                                path.join(" -> "),
                            ));
                        }
                        WHITE => {
                            mark[k] = GRAY;
                            stack.push((k, 0));
                        }
                        _ => {}
                    }
                } else {
                    depth[node] = 1 + kids[node].iter().map(|&k| depth[k]).max().unwrap_or(0);
                    mark[node] = BLACK;
                    stack.pop();
                }
            }
        }
        // Depth is counted for EACH reel, not only for used ones:
        // a reel lying in the bin and not placed anywhere, at the first
        // placement would produce an invalid tree — one must complain where
        // it was assembled, not a week later on the drag.
        if let Some(i) = (0..n).find(|&i| depth[i] > MAX_NEST_DEPTH) {
            return Err(ProjectError::SequenceTooDeep(
                self.sequences[i].id,
                self.sequences[i].name.clone(),
            ));
        }
        Ok(())
    }

    /// Contents of ONE reel. `sources` — everything `clip.media` is allowed to look at
    /// (files and reels), `media_ids` — only files (a morpher layer
    /// takes its material from a file: a nested item under the layer is a separate decision, and
    /// it cannot be silently counted as a file).
    /// Handles of the reel declared outward (R.59, stage 3).
    ///
    /// The target type is checked LIVE, not against a stored copy: the copy is a second
    /// source of the same fact, and a “handle that does nothing” (the minus of the reconnaissance,
    /// F-TL-012) must be a load error,
    /// not a surprise for the consumer.
    fn validate_published(
        &self,
        reel: &dyn Reel,
        seq: Option<MediaId>,
    ) -> Result<(), ProjectError> {
        if reel.published().is_empty() {
            return Ok(());
        }
        let mut ids = std::collections::HashSet::new();
        for p in reel.published() {
            if !ids.insert(p.id) {
                return Err(ProjectError::PublishedDuplicateId(seq, p.id));
            }
            if p.name.trim().is_empty() {
                return Err(ProjectError::PublishedUnnamed(seq, p.id));
            }
            let Some(ty) = self.addr_type(seq, &p.target) else {
                return Err(ProjectError::PublishedTargetMissing(seq, p.id));
            };
            if !ty.same_family(&p.ty) {
                return Err(ProjectError::PublishedTypeMismatch(seq, p.id));
            }
        }
        Ok(())
    }

    /// Instance overrides: only on a nested item, and only on the PUBLISHED
    /// ones (the owner's decision).
    pub fn validate_overrides_for(&self, c: &ClipEvent) -> Result<(), ProjectError> {
        self.validate_overrides(c)
    }

    fn validate_overrides(&self, c: &ClipEvent) -> Result<(), ProjectError> {
        if c.overrides.is_empty() {
            return Ok(());
        }
        let Some(s) = self.sequence(c.media) else {
            return Err(ProjectError::OverrideOnPlainClip(c.id));
        };
        let mut seen = std::collections::HashSet::new();
        for o in &c.overrides {
            if !seen.insert(o.pub_id) {
                return Err(ProjectError::OverrideDuplicate(c.id, o.pub_id));
            }
            let Some(p) = s.published.iter().find(|p| p.id == o.pub_id) else {
                return Err(ProjectError::OverrideDangling(c.id, o.pub_id));
            };
            if !o.value.ty().same_family(&p.ty) {
                return Err(ProjectError::OverrideTypeMismatch(c.id, o.pub_id));
            }
            if o.curves.is_empty() {
                continue;
            }
            if !p.ty.curvable() || !crate::published::owner_takes_curves(&p.target.owner) {
                return Err(ProjectError::OverrideCurveNotAllowed(c.id, o.pub_id));
            }
            for cur in &o.curves {
                // sources and forms — the same as for a slot (R.67); a link is
                // only INSIDE the reel: law E does not let it out
                check_curve_source(c.id, cur)?;
                if let Some(path) = cur.source.link_path()
                    && let Some(lp) = crate::links::LinkPath::parse(path)
                    && let Some(target) = lp.clip
                    && !s.clips.iter().any(|x| x.id == target)
                {
                    return Err(ProjectError::CurveSourceBroken(
                        c.id,
                        format!(
                            "override link {path:?} points outside the reel: publish a \
                             composition parameter instead"
                        ),
                    ));
                }
                let sorted = cur.keys.windows(2).all(|w| w[0].t <= w[1].t);
                let finite = cur.keys.iter().all(|k| k.t.0 >= 0 && k.v.is_finite());
                if !sorted || !finite {
                    return Err(ProjectError::OverrideBadCurve(c.id, o.pub_id));
                }
            }
        }
        Ok(())
    }

    fn validate_reel(
        &self,
        reel: &dyn Reel,
        seq: Option<MediaId>,
        sources: &std::collections::HashSet<MediaId>,
        media_ids: &std::collections::HashSet<MediaId>,
        seen_clips: &mut std::collections::HashSet<ClipId>,
    ) -> Result<(), ProjectError> {
        let is_root = seq.is_none();
        self.validate_published(reel, seq)?;
        let mut clip_ids = std::collections::HashSet::new();
        for c in reel.clips() {
            if !seen_clips.insert(c.id) {
                return Err(ProjectError::DuplicateClipId(c.id));
            }
            clip_ids.insert(c.id);
            // the high range is given to the pieces of a morpher layer (R.35): a real
            // clip with such an id would collide with a synthetic one
            if c.id.0 >= MAX_REAL_CLIP_ID {
                return Err(ProjectError::ReservedClipId(c.id));
            }
            if c.duration.0 <= 0 {
                return Err(ProjectError::NonPositiveDuration(c.id));
            }
            if c.source_in.0 < 0 {
                return Err(ProjectError::NegativeSourceIn(c.id));
            }
            if !sources.contains(&c.media) {
                return Err(ProjectError::UnknownMedia(c.id, c.media));
            }
            check_fx_auto(c.id, &c.fx_auto)?;
            check_clip_gen(c, self.gen_media_path(c.media))?;
            self.validate_overrides(c)?;
            // CHAIN, REVERSE AND WARP on a nested item are legal (R.55, stage 4).
            // Until this round, reverse and stretch were rejected: the sub-mix
            // was read strictly in order, while both need to read differently.
            // Now the sub-mix has a producer with a pre-roll, and reverse with
            // warp act on the SAMPLES of the assembled mix — exactly the same way
            // as on the samples of a file.
        }
        let mut morph_ids = std::collections::HashSet::new();
        for m in reel.morphers() {
            if !morph_ids.insert(m.id) {
                return Err(ProjectError::DuplicateMorphId(m.id));
            }
            // the target is a clip of THIS reel: a reference across the boundary would return
            // `None` from `joint()`, and the layer would silently stop working
            for t in m.targets.clip_ids() {
                if !clip_ids.contains(t) {
                    return Err(ProjectError::MorphUnknownTarget(m.id, *t));
                }
            }
            for mid in m.media_refs() {
                if !media_ids.contains(&mid) {
                    return Err(ProjectError::MorphUnknownMedia(m.id, mid));
                }
            }
            // Areas and markers — navigation ACROSS THE PROJECT; they live in the root and
            // their time is the root's time. A layer inside a reel that referred to them
            // would cover the wrong place, and there would be no way to tell.
            if !is_root
                && matches!(m.targets, MorphTargets::Areas { .. } | MorphTargets::Marks { .. })
            {
                return Err(ProjectError::MorphRefInsideSequence(m.id));
            }
        }
        // Cameras (R.29): duplicate ids, duration, fov, keyframes sorted
        // in [0,1]. Overlaps of camera events on one track are LEGAL
        // (active_camera_at resolves them) — they are not
        // connected to ClipOverlap.
        let mut cam_ids = std::collections::HashSet::new();
        for c in reel.cameras() {
            if !cam_ids.insert(c.id) {
                return Err(ProjectError::DuplicateCameraId(c.id));
            }
            if c.duration.0 <= 0 {
                return Err(ProjectError::CameraBadDuration(c.id));
            }
            let fovs =
                std::iter::once(c.cam.fov_deg).chain(c.keyframes.iter().map(|k| k.cam.fov_deg));
            for fov in fovs {
                if !(fov > 0.0 && fov < 180.0) {
                    return Err(ProjectError::CameraBadFov(c.id));
                }
            }
            for w in c.keyframes.windows(2) {
                if w[1].t < w[0].t {
                    return Err(ProjectError::CameraBadKeyframes(c.id));
                }
            }
            // camera curves — the same rules as for effect curves
            for cur in &c.auto {
                let sorted = cur.keys.windows(2).all(|w| w[0].t <= w[1].t);
                let ok = cur.keys.iter().all(|k| k.t.0 >= 0 && k.v.is_finite());
                let known = c.cam.param_spec(&cur.param).is_some();
                if !sorted || !ok || !known {
                    return Err(ProjectError::CameraBadKeyframes(c.id));
                }
            }
            if c.keyframes.iter().any(|k| !(0.0..=1.0).contains(&k.t)) {
                return Err(ProjectError::CameraBadKeyframes(c.id));
            }
        }
        let mut tr_ids = std::collections::HashSet::new();
        for t in reel.transitions() {
            if !tr_ids.insert(t.id) {
                return Err(ProjectError::DuplicateTransitionId(t.id));
            }
            check_transition_full(reel, t, None)?;
        }
        let mut foreign_keys = std::collections::HashSet::new();
        for f in reel.foreign() {
            if !foreign_keys.insert(f.key) {
                return Err(ProjectError::DuplicateForeignKey(f.key));
            }
        }
        // Clips of one (kind, lane) do not overlap — invariant R.24, but
        // ONLY with overlap-stacking switched off (R.28): with it on, overlaps
        // are legal and sound as a crossfade, as in v1. Foreign placeholders
        // always take up space: their raw data is written back to OTIO, where a track
        // is a strict sequence, and there is nothing to overlay on them.
        enum SpanId {
            Clip(ClipId),
            Foreign(u64),
        }
        let mut spans: Vec<(ClipKind, u32, i64, i64, SpanId)> = reel
            .clips()
            .iter()
            .map(|c| (c.kind, c.lane, c.start.0, (c.start + c.duration).0, SpanId::Clip(c.id)))
            .chain(reel.foreign().iter().map(|f| {
                (f.kind, f.lane, f.start.0, (f.start + f.duration).0, SpanId::Foreign(f.key))
            }))
            .collect();
        spans.sort_by_key(|s| (s.0 as u8, s.1, s.2, s.3));
        for w in spans.windows(2) {
            let (ka, la, _, end_a, ida) = &w[0];
            let (kb, lb, start_b, _, idb) = &w[1];
            if ka == kb && la == lb && *end_a > *start_b {
                // overlap-stacking allows overlapping of CLIPS, but not of foreign
                // placeholders
                if reel.overlaps_allowed()
                    && matches!((ida, idb), (SpanId::Clip(_), SpanId::Clip(_)))
                {
                    continue;
                }
                return Err(match (ida, idb) {
                    (SpanId::Clip(a), SpanId::Clip(b)) => ProjectError::ClipOverlap(*a, *b),
                    (SpanId::Clip(c), SpanId::Foreign(f))
                    | (SpanId::Foreign(f), SpanId::Clip(c)) => ProjectError::ForeignOverlap(*c, *f),
                    (SpanId::Foreign(f), SpanId::Foreign(_)) => {
                        ProjectError::DuplicateForeignKey(*f)
                    }
                });
            }
        }
        Ok(())
    }

    /// What `clip.media` looks at — a file or a nested reel.
    pub fn source(&self, id: MediaId) -> Option<Source<'_>> {
        if let Some(m) = self.media.iter().find(|m| m.id == id) {
            return Some(Source::File(m));
        }
        self.sequences.iter().find(|s| s.id == id).map(Source::Seq)
    }

    pub fn sequence(&self, id: MediaId) -> Option<&Sequence> {
        self.sequences.iter().find(|s| s.id == id)
    }

    pub fn sequence_mut(&mut self, id: MediaId) -> Option<&mut Sequence> {
        self.sequences.iter_mut().find(|s| s.id == id)
    }

    pub fn is_nest(&self, c: &ClipEvent) -> bool {
        self.sequences.iter().any(|s| s.id == c.media)
    }

    /// A clip on a generator (`gen:` media, R.52): the source computes the frame
    /// itself. Strictly by the PARSEABLE path: an unparseable `gen:` from a future
    /// build is a stub for a missing file — the generator window is not
    /// offered for it.
    pub fn is_gen(&self, c: &ClipEvent) -> bool {
        self.is_gen_media(c.media)
    }

    pub fn is_gen_media(&self, id: MediaId) -> bool {
        self.gen_media_path(id).is_some()
    }

    /// Generator kind path by media address. `None` — this is not a generator
    /// (a file, a reel, a broken `gen:` from a future build). The single door:
    /// validation, the commands and source assembly all ask it.
    pub fn gen_media_path(&self, id: MediaId) -> Option<&str> {
        let m = self.media.iter().find(|m| m.id == id)?;
        crate::genmedia::parse_gen_path(&m.path).map(|_| m.path.as_str())
    }

    /// The DEFAULT generator kind for a media address: the commands that create an event
    /// on a generator (`AddClip`,
    /// `SetClipMedia`) fill it in with this. The values come from the path, so an old full
    /// path migrates by itself — and this is the ONLY place where such filling
    /// happens.
    pub fn default_gen_for(&self, id: MediaId) -> Option<crate::GeneratorKind> {
        let m = self.media.iter().find(|m| m.id == id)?;
        crate::genmedia::parse_gen_path(&m.path)
    }

    /// How many places in the TREE refer to this reel.
    ///
    /// It closes what the reconnaissance called the common trouble of Premiere, FCP and
    /// VEGAS: “on a timeline clip you cannot SEE how many more places will change
    /// together with it”. It is counted by recomputation; there is no separate bookkeeping.
    pub fn sequence_uses(&self, id: MediaId) -> usize {
        let count = |cs: &[ClipEvent]| cs.iter().filter(|c| c.media == id).count();
        count(&self.clips) + self.sequences.iter().map(|s| count(&s.clips)).sum::<usize>()
    }

    /// The largest occupied addresses ACROSS THE WHOLE DOCUMENT (R.50).
    ///
    /// A count “by the root” is a hole: events inside reels are invisible to it, and a
    /// fresh address issued from the root would COLLIDE with an address inside a reel.
    /// A batch is then rejected as a whole, and the gesture (insert, cut) simply does
    /// NOT happen, SILENTLY. The exchange of contents on entering inside does not change
    /// this union, so one can count on any kind of project.
    pub fn max_clip_id(&self) -> u64 {
        self.clips
            .iter()
            .chain(self.sequences.iter().flat_map(|s| s.clips.iter()))
            .map(|c| c.id.0)
            .max()
            .unwrap_or(0)
    }

    pub fn max_group_id(&self) -> u64 {
        self.clips
            .iter()
            .chain(self.sequences.iter().flat_map(|s| s.clips.iter()))
            .filter_map(|c| c.link_group.map(|g| g.0))
            .max()
            .unwrap_or(0)
    }

    pub fn max_transition_id(&self) -> u64 {
        self.transitions
            .iter()
            .chain(self.sequences.iter().flat_map(|s| s.transitions.iter()))
            .map(|t| t.id.0)
            .max()
            .unwrap_or(0)
    }

    pub fn max_morph_id(&self) -> u64 {
        self.morphers
            .iter()
            .chain(self.sequences.iter().flat_map(|s| s.morphers.iter()))
            .map(|m| m.id.0)
            .max()
            .unwrap_or(0)
    }

    /// The largest occupied camera address ACROSS THE WHOLE DOCUMENT (R.55, stage 3).
    /// A count “by the root” is the same hole as the other four had (item 394):
    /// cameras inside reels are invisible to it.
    pub fn max_camera_id(&self) -> u64 {
        self.cameras
            .iter()
            .chain(self.sequences.iter().flat_map(|s| s.cameras.iter()))
            .map(|c| c.id.0)
            .max()
            .unwrap_or(0)
    }

    /// All reels of the document: the root first, then nested ones.
    fn reels(&self) -> impl Iterator<Item = (Option<MediaId>, &dyn Reel)> {
        std::iter::once((None, self as &dyn Reel))
            .chain(self.sequences.iter().map(|s| (Some(s.id), s as &dyn Reel)))
    }

    /// Parse without validate() — for the legacy import (R.24): old .ktproj files
    /// legitimately contain overlaps, which ops::upgrade_legacy converts,
    /// and only AFTER it must the project pass validate.
    pub fn from_json_unvalidated(s: &str) -> Result<Project, ProjectError> {
        // format_version is checked before full parsing, so as to give a meaningful
        // error on files of future versions.
        #[derive(Deserialize)]
        struct VersionOnly {
            format_version: u32,
        }
        let v: VersionOnly =
            serde_json::from_str(s).map_err(|e| ProjectError::Json(e.to_string()))?;
        if v.format_version != FORMAT_VERSION {
            return Err(ProjectError::UnsupportedVersion(v.format_version));
        }
        let mut p: Project =
            serde_json::from_str(s).map_err(|e| ProjectError::Json(e.to_string()))?;
        p.normalize();
        Ok(p)
    }

    pub fn from_json(s: &str) -> Result<Project, ProjectError> {
        let p = Self::from_json_unvalidated(s)?;
        p.validate()?;
        Ok(p)
    }
}
