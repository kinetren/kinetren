//! The plugin interface of Kinetren (change plugin-api-stage1): the parameter PASSPORT every
//! effect, transition and generator describes itself with, and — from stage 2 on — the manifest,
//! the kernel traits and the registry.
//!
//! This crate is LOW on purpose (design D1): it depends on kt-time and serde only, so a plugin
//! crate does not depend on kt-model and an edit of one effect does not cascade through the core.
//! kt-model re-exports everything here under its former names.

use serde::{Deserialize, Serialize};

/// The address of a media item in the project (moved here with `ParamValue::Media`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MediaId(pub u64);

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
#[macro_export]
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

/// The ONLY declaration of effect parameters: specs, reading and writing are generated from it,
/// so the three tables cannot drift apart.
/// The same parameter passport, but for a STRUCTURE (R.34 K3: the camera needs
/// specs to get into the shared field grid and the shared curve widget).
/// One declaration — specs, reading and writing with clamping.
#[macro_export]
macro_rules! params_for_struct {
    ($ty:ident { $( $field:ident : $t:ident, $lo:expr, $hi:expr, $def:expr, $step:expr, $unit:ident, $kind:ident );+ $(;)? }
     $( choice $cname:ident : $ckeys:expr, $cty:ty );* $(;)?) => {
        impl $crate::HasParams for $ty {
            fn param_specs(&self) -> &[$crate::ParamSpec] {
                const S: &[$crate::ParamSpec] = &[ $( $crate::ParamSpec::num(
                    stringify!($field), $lo, $hi, $def, $step,
                    $crate::ParamUnit::$unit, $crate::ParamKind::$kind,
                ) ),+ ];
                S
            }
            fn get_param(&self, name: &str) -> Option<f64> {
                $( if name == stringify!($field) { return Some(self.$field as f64); } )+
                None
            }
            fn set_param(&mut self, name: &str, v: f64) -> bool {
                $( if name == stringify!($field) {
                    self.$field = $crate::fx_cast!($t, v.clamp($lo, $hi));
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
                    self.$field = $crate::fx_cast!($t, v.clamp($lo, $hi));
                    return true;
                } i += 1; )+
                let _ = i;
                false
            }
            // Choice properties of a structure (R.63). Without them the branches are empty, and
            // behaviour is byte-for-byte the same as before: `extra_specs` returns an
            // empty vector, and the value takes the trait defaults.
            fn extra_specs(&self) -> Vec<$crate::ParamSpec> {
                vec![ $( $crate::choice_spec(stringify!($cname), $ckeys) ),* ]
            }
            fn get_value(&self, name: &str) -> Option<$crate::ParamValue> {
                $( if name == stringify!($cname) {
                    return Some($crate::ParamValue::Choice(self.$cname.tag().into()));
                } )*
                $crate::default_get_value(self, name)
            }
            fn set_value(&mut self, name: &str, v: &ParamValue) -> bool {
                $( if name == stringify!($cname) {
                    let $crate::ParamValue::Choice(key) = v else { return false };
                    let Some(x) = <$cty>::from_tag(key) else { return false };
                    self.$cname = x;
                    return true;
                } )*
                $crate::default_set_value(self, name, v)
            }
        }
    };
}

#[macro_export]
macro_rules! params_for {
    ($enum:ident, $specs:ident, $set:ident, $get:ident, $set_idx:ident, $get_idx:ident, {$(
        $variant:ident { $( $field:ident : $ty:ident, $lo:expr, $hi:expr, $def:expr, $step:expr, $unit:ident, $kind:ident );+ $(;)? }
    ),* $(,)?}) => {
        fn $specs(fx: &$enum) -> &'static [$crate::ParamSpec] {
            match fx {
                $( $enum::$variant { .. } => {
                    const S: &[$crate::ParamSpec] = &[
                        $( $crate::ParamSpec::num(
                            stringify!($field), $lo, $hi, $def, $step,
                            $crate::ParamUnit::$unit, $crate::ParamKind::$kind,
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
                        *$field = $crate::fx_cast!($ty, v.clamp($lo, $hi));
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
                        *$field = $crate::fx_cast!($ty, v.clamp($lo, $hi));
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

/// The passport of the kind's FLAGS — from ONE declaration, next to `params_for!`
/// (R.53, item 414). It generates specs, reading and writing: the three tables cannot drift
/// apart, and completeness (whether a bool field was forgotten) is guarded by a test that
/// checks the flags against the kind’s serde representation.
#[macro_export]
macro_rules! flags_for {
    ($enum:ident, $specs:ident, $set:ident, $get:ident, {$(
        $variant:ident { $( $field:ident : $def:expr );+ $(;)? }
    ),* $(,)?}) => {
        fn $specs(fx: &$enum) -> &'static [$crate::FlagSpec] {
            match fx {
                $( $enum::$variant { .. } => &[
                    $( $crate::FlagSpec { name: stringify!($field), default: $def } ),+
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

// ---- the manifest, the kernels and the registry (plugin-api-stage1, D2/D4) ----------------------

/// Pre-roll at a continuity break: max(tail, 100 ms) is rendered into garbage, so the state
/// (reverb, resampler) is warmed up deterministically. ONE home (kt-fx re-exports it): the
/// registry refuses an audio look-ahead that can reach it (D4), the processor pre-rolls by it.
pub const MIN_PREROLL_FRAMES: i64 = (kt_time::AUDIO_SAMPLE_RATE as i64) / 10;

/// A plugin's id: for a built-in it is the tag the kind had in the file (D3), lower snake_case.
pub type PluginId = std::borrow::Cow<'static, str>;

/// What a plugin is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Media {
    VideoFx,
    AudioFx,
    Transition,
    Generator,
}

/// The planner class of a video effect (kt-fx `plan_chain`): it decides where the code runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoClass {
    /// A coordinate map expressed in the host's closed primitive set — fused into one pass.
    Geometry,
    /// A function of one pixel — a tap of the fused pass, or its own pass.
    PointWise,
    /// Needs the neighbourhood — a pass of its own between the series.
    Barrier,
}

/// Whether the kernel may be called from several threads at once (the preview, the export and
/// the thumbnail service call video kernels concurrently).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Threads {
    Concurrent,
    Single,
}

/// The GPU side a plugin offers (D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gpu {
    /// No GPU kernel (audio, and every plugin before the GPU side is assembled).
    None,
    /// A WGSL text: a point-wise plugin's `tap_<id>` function, or a barrier's fragment pass.
    Wgsl(&'static str),
    /// A geometry plugin's map, expressed in the host's closed primitive set (no WGSL).
    Primitives,
}

/// A count of frames as a function of the parameter values (a tail, a look-ahead).
pub type FramesFn = fn(&ParamValues) -> i64;

/// What the host's invariants depend on — DATA, checked once at registration (D2, D4).
#[derive(Debug, Clone)]
pub struct Decl {
    /// State kept between frames / blocks.
    pub state: bool,
    pub tail: Option<FramesFn>,
    pub lookahead: Option<FramesFn>,
    pub threads: Threads,
    /// The planner class; required for a video effect.
    pub video_class: Option<VideoClass>,
    pub gpu: Gpu,
    /// A built-in (the assembler's closed list): it must keep the GPU default.
    pub builtin: bool,
}

impl Default for Decl {
    fn default() -> Decl {
        Decl {
            state: false,
            tail: None,
            lookahead: None,
            threads: Threads::Concurrent,
            video_class: None,
            gpu: Gpu::None,
            builtin: true,
        }
    }
}

/// How the plugin shows in the interface: the display name (an industry term, not translated —
/// t48) and catalog key PATHS kt-app resolves (D7).
#[derive(Debug, Clone, Default)]
pub struct UiDecl {
    pub name: &'static str,
    pub category_key: &'static str,
    pub keywords_key: &'static str,
}

/// A plugin's description of itself.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub id: PluginId,
    pub media: Media,
    pub passport: Vec<ParamSpec>,
    pub decl: Decl,
    pub ui: UiDecl,
}

/// Numeric parameter values in passport order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParamValues(pub Vec<f64>);

impl ParamValues {
    /// The passport's defaults.
    pub fn defaults(passport: &[ParamSpec]) -> ParamValues {
        ParamValues(passport.iter().map(|s| s.default).collect())
    }
    /// Every row at the top of its range (the registry's worst case for a look-ahead).
    pub fn top(passport: &[ParamSpec]) -> ParamValues {
        ParamValues(passport.iter().map(|s| s.max).collect())
    }
    pub fn get(&self, idx: usize) -> f64 {
        self.0.get(idx).copied().unwrap_or(0.0)
    }
}

/// A video effect's CPU kernel — the byte reference (invariant 7).
pub trait VideoKernel: Send + Sync {
    /// A point-wise effect's function of one pixel, straight RGBA in 0..255 as f64 (the same
    /// arithmetic the fused pass's tap does).
    fn tap(&self, _p: &ParamValues, _rgba: &mut [f64; 4]) {}
}

/// A plugin: its manifest and its kernels.
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> &Manifest;
    fn video(&self) -> Option<&dyn VideoKernel> {
        None
    }
}

/// The tail of an audio plugin in frames — ONE home of the formula: the registry, the static
/// `chain_tail` and the processor read it through this function (D2).
pub fn tail_frames(m: &Manifest, p: &ParamValues) -> i64 {
    m.decl.tail.map_or(0, |f| f(p))
}

/// The look-ahead of an audio plugin in frames — the same rule as `tail_frames`.
pub fn lookahead_frames(m: &Manifest, p: &ParamValues) -> i64 {
    m.decl.lookahead.map_or(0, |f| f(p))
}

/// Why a plugin was refused (D4). Each names the plugin and the declaration that failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub id: String,
    pub reason: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "plugin `{}` refused: {}", self.id, self.reason)
    }
}

impl std::error::Error for Refusal {}

/// The registered plugins, in registration order (the assembler's closed order for built-ins).
#[derive(Default)]
pub struct Registry {
    plugins: Vec<std::sync::Arc<dyn Plugin>>,
}

impl Registry {
    pub fn new() -> Registry {
        Registry::default()
    }

    /// Register, or refuse BEFORE any frame or sample is produced (D4).
    pub fn register(&mut self, p: std::sync::Arc<dyn Plugin>) -> Result<(), Refusal> {
        let m = p.manifest();
        let refuse = |reason: &str| Err(Refusal { id: m.id.to_string(), reason: reason.into() });
        let id_ok = !m.id.is_empty()
            && m.id.starts_with(|c: char| c.is_ascii_lowercase())
            && m.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !id_ok {
            return refuse("the id is not lower snake_case");
        }
        if self.get(&m.id).is_some() {
            return refuse("an id registered twice");
        }
        if m.decl.threads == Threads::Single {
            return refuse("a kernel that must not be called concurrently");
        }
        match m.media {
            Media::VideoFx => {
                // post-FX memoization is built on frames without history (invariants 4, 11)
                if m.decl.state {
                    return refuse("a video kernel declares state between frames");
                }
                if m.decl.tail.is_some() {
                    return refuse("a video kernel declares a tail");
                }
                if m.decl.lookahead.is_some() {
                    return refuse("a video kernel declares a look-ahead");
                }
                match (m.decl.video_class, m.decl.gpu) {
                    (None, _) => return refuse("a video effect without a planner class"),
                    (Some(VideoClass::Geometry), Gpu::Wgsl(_)) => {
                        return refuse("a geometry plugin offers WGSL instead of the host's primitives");
                    }
                    (Some(VideoClass::PointWise | VideoClass::Barrier), Gpu::None) if m.decl.builtin => {
                        return refuse("a built-in point-wise or barrier plugin without a GPU kernel");
                    }
                    _ => {}
                }
            }
            Media::AudioFx => {
                let top = lookahead_frames(m, &ParamValues::top(&m.passport));
                let dflt = lookahead_frames(m, &ParamValues::defaults(&m.passport));
                if top.max(dflt) >= MIN_PREROLL_FRAMES {
                    return refuse(&format!(
                        "an audio look-ahead of {} frames reaches the pre-roll ({MIN_PREROLL_FRAMES})",
                        top.max(dflt)
                    ));
                }
            }
            Media::Transition | Media::Generator => {}
        }
        self.plugins.push(p);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&std::sync::Arc<dyn Plugin>> {
        self.plugins.iter().find(|p| p.manifest().id == id)
    }

    /// The ids in registration order.
    pub fn ids(&self) -> Vec<PluginId> {
        self.plugins.iter().map(|p| p.manifest().id.clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

static INSTALLED: std::sync::OnceLock<std::sync::Arc<Registry>> = std::sync::OnceLock::new();

/// Install the process's registry ONCE (D6): the format reader cannot take an argument through
/// serde, so kt-app and kt-cli install the assembled built-in set here at start, and tests through
/// the same function. A second install with a different id list panics in debug; the same list is
/// accepted (several tests install it).
pub fn install(reg: std::sync::Arc<Registry>) {
    let ids = reg.ids();
    let set = INSTALLED.get_or_init(|| reg);
    debug_assert_eq!(set.ids(), ids, "a second registry with a different plugin list was installed");
}

/// The installed registry, if any.
pub fn installed() -> Option<&'static std::sync::Arc<Registry>> {
    INSTALLED.get()
}
