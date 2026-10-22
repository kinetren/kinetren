//! Public parameters of a composition and overrides at the instance
//! (R.59, stage 3).
//!
//! The owner named the subject verbatim: “a template project of a news release…
//! obviously people will not go into the composition each time and change there the
//! text, logos”. The mechanism is exactly two fields and one rule:
//!
//! * a REEL has a list of handles declared by its author (`published`) —
//!   references to properties inside it, with a NAME that the author invented;
//! * an INSTANCE has a list of overrides (`overrides`) — values of
//!   these and only these handles.
//!
//! **The engine knows nothing about publication at all.** The override is resolved
//! in the same place where the morpher layers are applied — in the model, when clips are issued
//! (`Project::resolve_reel`). The engine gets ready clips, and it tells the REVISIONS of the
//! reel apart by a hash; exactly one thing changes — the revision becomes a
//! function of the PAIR (reel, set of instance overrides).
//!
//! Two decisions that are easy to lose:
//!
//! 1. **The template value is NOT STORED.** It is read from the target live
//!    (`Project::addr_value`). A stored copy is a second source of the same
//!    fact: the author edits the reel, the copy stays the same, and the “default
//!    value” silently lies.
//! 2. **Overrides go in BEFORE the morpher layers.** The author's address is the
//!    structure of the reel as the author sees it; the morpher appends slots AT THE END
//!    of the chain, and when the event chain is switched off it clears it entirely —
//!    had we applied them after, `FxSlot(i)` would point to the wrong place.

use kt_time::Tick;
use serde::{Deserialize, Serialize};

use crate::genmedia::GeneratorKind;
use crate::project::{
    CameraEvent, CameraId, ClipEvent, ClipId, HasParams, MediaId, ParamCurve, ParamKind, ParamSpec,
    ParamType, ParamUnit, ParamValue, TransitionEvent, TransitionId, clip_material_at,
};

/// Number of the public parameter within the reel. An address that survives
/// renaming: the author invents the name and edits it whenever they want.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PubId(pub u64);

/// Narrowing of the limits of a declared handle (reconnaissance F-TL-012, trick 1: the author
/// puts out a slider with limits so that the consumer physically cannot
/// drive a title off the frame). Empty — the target's limits as they are.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct PubRange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

impl PubRange {
    pub fn is_empty(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    /// Normalize the narrowing against the target's PASSPORT (R.60).
    ///
    /// A limit equal to the target's limit is the ABSENCE of narrowing, not a
    /// number: otherwise the very first publication would write a copy of the passport
    /// into the file, and it would diverge from the target when the effect's version changes.
    /// The tolerance is half a passport step: the field gives a number rounded
    /// to the step, and an exact equality is never obtained on it.
    pub fn narrowed(spec: &ParamSpec, min: f64, max: f64) -> PubRange {
        let eps = (spec.step.abs() * 0.5).max(f64::EPSILON);
        PubRange {
            min: (min > spec.min + eps).then_some(min),
            max: (max < spec.max - eps).then_some(max),
        }
    }

    /// Clamp the number to the declared limits. The target's limits do not go anywhere
    /// in the process — `set_param` itself applies them, and the order does not matter:
    /// both operations are a clamp.
    pub fn clamp(&self, v: f64) -> f64 {
        let mut v = v;
        if let Some(lo) = self.min {
            v = v.max(lo);
        }
        if let Some(hi) = self.max {
            v = v.min(hi);
        }
        v
    }
}

/// Whom the property inside the reel belongs to.
///
/// For `Transition` and `Camera` the field `ParamAddr::clip` is not used:
/// their own numbers carry the address. It is kept as one type deliberately —
/// a second “address without a clip” would split the publication code into two branches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "owner", rename_all = "snake_case")]
pub enum ParamOwner {
    /// Chain slot of the event's effects (index into `ClipEvent::fx`).
    FxSlot {
        slot: usize,
    },
    /// Values of the event's generator (R.59, stage 2) — the title text, the badge
    /// colour, the font size.
    Generator,
    /// A property of the event itself: `opacity`, `gain_db`, `media`, `reversed`.
    ClipAttr,
    Transition {
        id: TransitionId,
    },
    Camera {
        id: CameraId,
    },
    /// LEADING “Geometry” NODE of the event (R.62): the conform rule,
    /// crop, position, zoom, rotation, anchor.
    ///
    /// The node lives not in `fx` but in its own field `ClipEvent::geometry` — and
    /// until R.63 it alone had no publication address: “for what reason does the first geometry
    /// have no way to publish its parameters while the second has one? or do we have code duplicates?” (the owner).
    /// Not duplicates — a hole in addressing: its passport, `set_value` and curves are
    /// exactly the same as for a slot, and the branch here is one line long
    /// for exactly that reason.
    /// An audio event has no such node, and validation rejects the address.
    Geometry,
    /// A public parameter of a reel nested INSIDE this one (a template in a
    /// template). The outer reel patches `overrides` of the inner instance,
    /// and the rest is resolved by itself — recursion is free by construction.
    Nested {
        pub_id: PubId,
    },
}

/// Address of a property INSIDE a reel. Glued to `ClipId`, so it survives
/// trimming, moving and the LEFT half of a cut (piece no. 0 keeps the id — the same
/// rule as for morpher cutting).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamAddr {
    pub clip: ClipId,
    #[serde(flatten)]
    pub owner: ParamOwner,
    pub param: String,
}

impl ParamAddr {
    pub fn new(clip: ClipId, owner: ParamOwner, param: impl Into<String>) -> ParamAddr {
        ParamAddr { clip, owner, param: param.into() }
    }
}

/// Handle of the reel declared outward.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublishedParam {
    pub id: PubId,
    /// Free text of the AUTHOR (the owner’s decision, R.59). This is document content,
    /// like an event name, and it will not
    /// get into the program's translation catalog, ever.
    pub name: String,
    pub target: ParamAddr,
    /// The type as the author saw it at publication. A mismatch with the target is
    /// an explicit validation error, not a silent coercion.
    pub ty: ParamType,
    #[serde(default, skip_serializing_if = "PubRange::is_empty")]
    pub range: PubRange,
}

/// Override of ONE declared handle on ONE instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Override {
    pub pub_id: PubId,
    pub value: ParamValue,
    /// A curve lives in the MATERIAL time of the INSTANCE — and that is the time of the reel.
    /// The same code that translates the curves of the morpher layer
    /// (`clip_material_at`) translates it into the target's material.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub curves: Vec<ParamCurve>,
}

impl Override {
    pub fn value(pub_id: PubId, value: ParamValue) -> Override {
        Override { pub_id, value, curves: Vec::new() }
    }
}

/// The contents of the reel, RESOLVED for a concrete set of overrides.
///
/// `Cow` here is not decoration: the empty set must take the path byte-for-byte
/// equal to the previous one — and the previous path cloned nothing (t673/t676).
#[derive(Debug, Clone)]
pub struct ResolvedReel<'a> {
    pub clips: Vec<ClipEvent>,
    pub transitions: std::borrow::Cow<'a, [TransitionEvent]>,
    pub cameras: std::borrow::Cow<'a, [CameraEvent]>,
}

/// Can the handle of this carrier carry a CURVE.
///
/// There is one rule and it is about the DOMAIN: an override curve arrives in the
/// material time of the instance and is translated into the target's material
/// by `clip_material_at`. A transition's domain is the tick from the start of the window;
/// a camera's is the event progress: there is no translation there, and an “almost correct”
/// curve would be worse than a refusal. A property of the event (`opacity`,
/// `gain_db`) has no carrier curve at all.
pub fn owner_takes_curves(owner: &ParamOwner) -> bool {
    matches!(
        owner,
        ParamOwner::FxSlot { .. }
            | ParamOwner::Generator
            | ParamOwner::Geometry
            | ParamOwner::Nested { .. }
    )
}

/// Translate the curve keys from the time of the REEL into the target's material.
pub fn retime_curve(mut c: ParamCurve, target: &ClipEvent, frame_step: i64) -> ParamCurve {
    for k in &mut c.keys {
        k.t = clip_material_at(
            target.source_in,
            target.start,
            target.duration,
            target.reversed,
            frame_step,
            k.t,
        );
    }
    c.keys.sort_by_key(|k| k.t.0);
    c
}

/// Put the override curves into the slot, replacing those with the same name.
fn put_curves(slot: &mut Vec<ParamCurve>, curves: Vec<ParamCurve>) {
    for c in curves {
        slot.retain(|x| x.param != c.param);
        slot.push(c);
    }
}

/// Apply the overrides to the contents of the reel — BEFORE the morpher layers.
///
/// `default_gen` answers “which kind of generator is this media address?”:
/// the source substitution must reconcile the generator values with the new
/// media by the same rule as `fit_gen_to_media`, otherwise a logo file
/// would stay with someone else's title on the event.
pub fn apply_overrides(
    published: &[PublishedParam],
    overrides: &[Override],
    clips: &mut [ClipEvent],
    transitions: &mut [TransitionEvent],
    cameras: &mut [CameraEvent],
    frame_step: i64,
    default_gen: &dyn Fn(MediaId) -> Option<GeneratorKind>,
) {
    // Order is BY THE AUTHOR'S LIST, not by the order of the overrides:
    // the set at the instance comes from a file and from the clipboard, and its
    // order cannot be a rendering rule.
    for p in published {
        let Some(ov) = overrides.iter().find(|o| o.pub_id == p.id) else { continue };
        // Curves with a formula, layer or link are resolved in the REEL'S DOMAIN
        // (R.67): a formula typed on the instance's axis sounds on it too.
        // Zero cost without sources — the set is cloned only then.
        if ov.curves.iter().any(ParamCurve::needs_resolve) {
            let snapshot = clips.to_vec();
            let end = clips.iter().map(|c| c.start.0 + c.duration.0).max().unwrap_or(0);
            let me = crate::links::LinkCtx {
                source_in: Tick::ZERO,
                start: Tick::ZERO,
                duration: Tick(end.max(frame_step)),
                reversed: false,
                frame_step,
            };
            let mut resolved = ov.clone();
            for c in &mut resolved.curves {
                crate::links::resolve_curve_in(&snapshot, c, &me, p.target.clip, frame_step);
            }
            apply_one(p, &resolved, clips, transitions, cameras, frame_step, default_gen);
        } else {
            apply_one(p, ov, clips, transitions, cameras, frame_step, default_gen);
        }
    }
}

fn apply_one(
    p: &PublishedParam,
    ov: &Override,
    clips: &mut [ClipEvent],
    transitions: &mut [TransitionEvent],
    cameras: &mut [CameraEvent],
    frame_step: i64,
    default_gen: &dyn Fn(MediaId) -> Option<GeneratorKind>,
) {
    let value = match (&ov.value, &p.ty) {
        // the narrowing of limits is part of the DECLARATION, not decoration of the window:
        // a corrupted file has no right to drive a title off the frame
        (ParamValue::Num(v), ParamType::Num) => ParamValue::Num(p.range.clamp(*v)),
        (v, _) => v.clone(),
    };
    let name = p.target.param.as_str();
    match &p.target.owner {
        ParamOwner::Transition { id } => {
            if let Some(t) = transitions.iter_mut().find(|t| t.id == *id) {
                t.kind.set_value(name, &value);
            }
            return;
        }
        ParamOwner::Camera { id } => {
            if let Some(c) = cameras.iter_mut().find(|c| c.id == *id) {
                c.cam.set_value(name, &value);
            }
            return;
        }
        _ => {}
    }
    let Some(c) = clips.iter_mut().find(|c| c.id == p.target.clip) else { return };
    match &p.target.owner {
        ParamOwner::FxSlot { slot } => {
            let Some(fx) = c.fx.get_mut(*slot) else { return };
            fx.set_value(name, &value);
            if !ov.curves.is_empty() {
                c.fx_auto.resize(c.fx.len().max(*slot + 1), Vec::new());
                let retimed: Vec<ParamCurve> =
                    ov.curves.iter().cloned().map(|x| retime_curve(x, c, frame_step)).collect();
                put_curves(&mut c.fx_auto[*slot], retimed);
            }
        }
        ParamOwner::Generator => {
            let Some(g) = c.r#gen.as_mut() else { return };
            g.set_value(name, &value);
            if !ov.curves.is_empty() {
                let retimed: Vec<ParamCurve> =
                    ov.curves.iter().cloned().map(|x| retime_curve(x, c, frame_step)).collect();
                put_curves(&mut c.gen_auto, retimed);
            }
        }
        ParamOwner::Geometry => {
            // the same passport and the same `set_value` as for a slot; curves go into
            // their own home `geometry_auto` (the same domain, material tick)
            c.geometry.set_value(name, &value);
            if !ov.curves.is_empty() {
                let retimed: Vec<ParamCurve> =
                    ov.curves.iter().cloned().map(|x| retime_curve(x, c, frame_step)).collect();
                put_curves(&mut c.geometry_auto, retimed);
            }
        }
        ParamOwner::ClipAttr => set_clip_attr(c, name, &value, default_gen),
        ParamOwner::Nested { pub_id } => {
            // We patch the SET of the inner instance — it will sort itself out further,
            // by its own code. The curve remains in the material
            // of the inner instance: its material is the time of its reel, and
            // we stand exactly in it.
            let inner = Override {
                pub_id: *pub_id,
                value,
                curves: ov.curves.iter().cloned().map(|x| retime_curve(x, c, frame_step)).collect(),
            };
            c.overrides.retain(|o| o.pub_id != *pub_id);
            c.overrides.push(inner);
        }
        ParamOwner::Transition { .. } | ParamOwner::Camera { .. } => unreachable!(),
    }
}

/// Can this set of overrides change the STATE OF THE SOURCE
/// inside a reel (R.59, stage 3).
///
/// Why this is needed and why it is not a trifle. Since R.53 the address of a generator
/// source is `GEN_TAG | event id`, and until this stage it unambiguously
/// determined the VALUES: one event, one set of values. An override
/// breaks the premise — the same event INSIDE a reel plays a different text in
/// different instances. So for such instances the source address must also differ,
/// otherwise a “second issue with another title” would silently show the first:
/// the order and the read would meet at one address (invariant 4 holds),
/// but the frame would be a FOREIGN one.
///
/// The answer “no” must be exact, not optimistic: the salt is zero
/// exactly when no handle of the set is able to reach the source — and then the addresses remain
/// byte-for-byte the same as before. This includes `Generator` (values directly),
/// `ClipAttr media` (an event may become a generator of another kind), and `Nested`
/// (a template in a template — the set travels
/// inside).
pub fn overrides_touch_sources(published: &[PublishedParam], ov: &[Override]) -> bool {
    ov.iter().any(|o| {
        published.iter().any(|p| {
            p.id == o.pub_id
                && match &p.target.owner {
                    ParamOwner::Generator | ParamOwner::Nested { .. } => true,
                    ParamOwner::ClipAttr => p.target.param == "media",
                    _ => false,
                }
        })
    })
}

/// Names of the event's own properties — the ONLY list of them.
pub const CLIP_ATTRS: &[(&str, ParamType)] = &[
    ("opacity", ParamType::Num),
    ("gain_db", ParamType::Num),
    ("media", ParamType::Media),
    ("reversed", ParamType::Flag),
];

pub fn clip_attr_type(name: &str) -> Option<ParamType> {
    CLIP_ATTRS.iter().find(|(n, _)| *n == name).map(|(_, t)| t.clone())
}

/// Passport of the event attribute. The limits are THE SAME that
/// `set_clip_attr` clamps them with: a second set of numbers would diverge from the first (R.60).
pub fn clip_attr_spec(name: &str) -> Option<ParamSpec> {
    let ty = clip_attr_type(name)?;
    // ±6.0206 dB — gain factor 2.0, the same scale as the level rail on the
    // timeline (`RAIL_DB_SPAN` is twice as wide because it is two-sided)
    let (min, max, default, step, unit) = match name {
        "opacity" => (0.0, 1.0, 1.0, 0.01, ParamUnit::Norm),
        "gain_db" => (-6.0206, 6.0206, 0.0, 0.1, ParamUnit::Db),
        _ => (0.0, 1.0, 0.0, 1.0, ParamUnit::None),
    };
    Some(ParamSpec {
        name: std::borrow::Cow::Owned(name.to_string()),
        ty,
        min,
        max,
        default,
        step,
        unit,
        kind: ParamKind::Smooth,
        label: std::borrow::Cow::Borrowed(""),
    })
}

pub fn clip_attr_value(c: &ClipEvent, name: &str) -> Option<ParamValue> {
    match name {
        "opacity" => Some(ParamValue::Num(c.opacity as f64)),
        "gain_db" => Some(ParamValue::Num(c.gain_db as f64)),
        "media" => Some(ParamValue::Media(c.media)),
        "reversed" => Some(ParamValue::Flag(c.reversed)),
        _ => None,
    }
}

fn set_clip_attr(
    c: &mut ClipEvent,
    name: &str,
    v: &ParamValue,
    default_gen: &dyn Fn(MediaId) -> Option<GeneratorKind>,
) {
    match (name, v) {
        ("opacity", ParamValue::Num(x)) => c.opacity = x.clamp(0.0, 1.0) as f32,
        ("gain_db", ParamValue::Num(x)) => c.gain_db = *x as f32,
        ("reversed", ParamValue::Flag(b)) => c.reversed = *b,
        ("media", ParamValue::Media(m)) => {
            if c.media == *m {
                return;
            }
            c.media = *m;
            // By the same rule as `fit_gen_to_media`: once it has become a file,
            // it cannot have generator values; once it has become a generator of another
            // kind — the values are taken from the path.
            match default_gen(*m) {
                Some(k) if c.r#gen.as_ref().is_some_and(|g| g.tag() == k.tag()) => {}
                Some(k) => {
                    c.r#gen = Some(k);
                    c.gen_auto.clear();
                }
                None => {
                    c.r#gen = None;
                    c.gen_auto.clear();
                }
            }
        }
        _ => {}
    }
}
