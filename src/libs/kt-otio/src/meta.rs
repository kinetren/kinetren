//! The `metadata["kt"]` envelope — our namespace in OTIO (decision of R.24: short
//! "kt", not "kinotrain"). One version per FILE: `Timeline.metadata.kt.v`;
//! nested kt envelopes (clips, effects, transitions) carry no version.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{Error, Timeline};

pub const KT_NS: &str = "kt";
/// The maximum UNDERSTOOD envelope version. The writer sets the version BY CONTENT (R.29): v=2 ⇔ the file carries cameras/camera tracks
/// (an old reader would silently lose them on rewrite → the regular refusal KtMetaTooNew,
/// round 24 rule “ours so nobody breaks it”); otherwise v=1 — files without 3D
/// are byte-stable and open in old builds.
/// One Pan3d does NOT bump the version: an unknown effect
/// degrades into foreign-passthrough.
pub const KT_META_VERSION: u32 = 13;
pub const KT_META_V1: u32 = 1;
/// Cameras and camera tracks (R.29).
pub const KT_META_V2: u32 = 2;
/// Automation curves in MATERIAL ticks (R.33): an old build would read them as fractions 0..1
/// and fail validation — a refusal is more honest than silent corruption.
pub const KT_META_V3: u32 = 3;
/// Morpher LAYER (R.35): acts by the block's time and by default on everything below its row;
/// track order moved into the model. An old build would read the layer as a patch onto an empty
/// target list and would silently render the wrong thing —
/// a refusal is more honest than silent corruption.
pub const KT_META_V4: u32 = 4;
/// Automation and morpher-layer strength (R.38, items 12-13). Layer curves live in its patch,
/// strength and fades sit on the event itself; an old build would silently lose both, and
/// “a layer at 30%” would become “a layer in full” — a spoiled edit,
/// not a minor loss.
pub const KT_META_V5: u32 = 5;
/// Nested sequences (R.50). The reel body is inlined into each instance as a `Stack`,
/// so a FOREIGN editor sees the correct edit even without our envelope;
/// but an OLD build of ours would read the nesting as a grey placeholder `ForeignEvent` (item 158)
/// and on the first rewrite would write it back to the file as raw data —
/// that is, turn a live edit into a dead chunk of JSON.
/// A refusal is more honest.
pub const KT_META_V6: u32 = 6;
/// Transitions R.51: invert (`invert` in the transition envelope). An old build would play the transition NOT inverted —
/// the gesture's direction on screen is the opposite, that is a silently spoiled edit;
/// it would even keep the key (passthrough of an unread envelope), which makes the damage
/// still less visible.
/// A refusal is more honest.
pub const KT_META_V7: u32 = 7;
/// An event LEFT OF ZERO inside a reel (R.58, item 462): `start` in the clip envelope (`kt.start`)
/// where it cannot be expressed by position on a sequential track.
/// An old build would put such an event at zero and shift all the reel's content after it —
/// silently; a refusal is more honest.
pub const KT_META_V8: u32 = 8;
/// Generator parameters ON THE EVENT (R.59, stage 2). An old build would read the values from the media path —
/// but they are no longer there — and would show an empty title instead of the heading,
/// in every place at once. A refusal
/// is more honest.
/// The same version (round decision: ONE version, raised BY CONTENT) — public composition parameters
/// and per-instance overrides (R.59, stage 3). An old reader would play all instances with the
/// template values: ten news releases with one heading,
/// and not one visible sign.
/// A refusal is more honest.
pub const KT_META_V9: u32 = 9;

/// Segment shape on a keyframe and working sources of the curve (R.61).
///
/// Raised ONLY when the file contains a shape that the earlier boolean field `ease` cannot express —
/// that is, when a curve has different shapes on different segments, or a shape
/// not from the pair “third order / straight”. An old build would read such a curve as uniform:
/// a zigzag would become a straight line, a step a smooth glide,
/// and not one sign of damage.
/// A curve with a uniform expressible shape does NOT bump the version and is written
/// with the same bytes as before (D1b).
pub const KT_META_V10: u32 = 10;
/// R.62: conform rule, the leading “Geometry” node, reel settings.
pub const KT_META_V11: u32 = 11;
/// R.65: SIDECHAIN KEY on the track compressor. An old build would silently lose the field —
/// and ducking would simply not sound; the version tells it so.
pub const KT_META_V12: u32 = 12;
/// R.67: SEGMENT FORMULA on the key and a LAYER formula over non-empty keys
/// (a formula that reads `k`). An old build would read a segment formula as a straight line,
/// and would erase the layer together with its keys (“formula without keys”) — silently.
/// Raised ONLY by content.
pub const KT_META_V13: u32 = 13;

// ---- typed metadata.kt envelopes (otio-types approach) -----------------
// Structurally small things get OWN types (model evolution does not silently move the file schema);
// heavy and stable ones (FxInstance, MorpherEvent, BinGroup, LaneKind) reuse the serde-derive
// of the live kt-model types — drift is caught by the byte-golden
// fixtures (test 148).

/// The Timeline.metadata.kt envelope — everything that has no standard place in OTIO.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KtTimelineMeta {
    pub v: u32,
    pub settings: Option<KtSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<KtTransport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<kt_model::UiState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bin_media: Vec<KtBinMedia>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bin_groups: Vec<kt_model::BinGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bin_layout: Vec<(kt_model::MediaId, f32, f32)>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bin_free: bool,
    /// TRACK COUNTERS: an empty line cannot be expressed by content, and files
    /// of all earlier rounds keep it here. From R.62 this is DERIVED from the vector of tracks —
    /// still written (byte stability, t433b),
    /// read as the basis if `tracks` is absent.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub morph_lanes: u32,
    /// LEGACY (before R.62): row order in a separate field. Now the order
    /// is the order of `tracks`; the field is only read.
    #[serde(default, skip_serializing)]
    pub lane_order: Vec<(kt_model::LaneKind, u32)>,
    /// TRACK OBJECTS top to bottom (R.62, change B) — row order,
    /// switches, level, blend mode, height and the chain of service rows.
    /// Written only when there is something to say: for a project with natural order and empty properties
    /// the file is byte-for-byte as before (t433b).
    /// The name, `enabled` and the chain of VIDEO and AUDIO tracks live on the file’s own `Track`s —
    /// a foreign editor must see them; they are
    /// not duplicated here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<KtTrack>,
    /// Named regions “time × tracks” (R.35 item 2). They have no entities
    /// of their own in OTIO (Marker describes only time), so they travel
    /// in our section as a whole.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub areas: Vec<kt_model::MarkedArea>,
    /// The edit length of the timeline in ticks, when it is LONGER than the material
    /// (the trailing Gap of a foreign file; the user’s decision, R.25). Empty
    /// tracks that were not in the file we do not write — their count travels here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lanes: Option<(u32, u32)>,
    /// Layer overlap (R.28): how many consecutive Tracks each of our tracks occupies.
    /// In OTIO an overlap inside one Track cannot be expressed (clip position = order of children),
    /// so layers are written as adjacent tracks — and by this map we
    /// reassemble the track. It is written only when there is more than one layer;
    /// files without overlaps are byte-for-byte as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<KtLayers>,
    /// Serialized sorted by MorphId (deterministic application).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub morphers: Vec<kt_model::MorpherEvent>,
    /// Camera tracks and cameras (R.29, 3D compositing). The presence of any of them
    /// raises the file version to v2 (see KT_META_VERSION).
    /// Cameras are sorted by CameraId (determinism, test 144).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub camera_lanes: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<kt_model::CameraEvent>,
    /// Nested reels (R.50) — what OTIO does not express: the number of tracks,
    /// row order, morpher layers and cameras INSIDE the reel.
    ///
    /// The reel's EDIT itself does not travel here: it is inlined as a `Stack` into each
    /// instance, and a foreign editor reads it normally. Here is a sidecar, modelled on
    /// `KtLayers`: if it does not match the file — a notice and reading as-is, not corruption.
    /// Sorted by address (determinism, test 144).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sequences: Vec<KtSequence>,
    /// Overlapping transitions of the ROOT reel (R.51, item 399); for nested ones —
    /// the same field in their `KtSequence`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlap_transitions: Vec<KtOverlapTransition>,
    /// Public parameters of the reel standing in the ROOT's place (R.59, stage
    /// 3). In the saved document this is always empty — the root is no one's
    /// instance — but the model's content must not be silently lost.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub published: Vec<kt_model::PublishedParam>,
    /// Forward mercy toward future versions (R.29): unknown kt fields
    /// of v3+ are carried through passthrough — the next additions will not require
    /// a hard refusal, if the old reader loses none of them.
    #[serde(default, flatten)]
    pub extra: Map<String, Value>,
}

/// TRACK in the envelope (R.62). For video and audio it carries only what is
/// not in the OTIO track (id, level, blend mode, solo, lock, height);
/// for service rows (morph, camera) — everything, including the name and the chain:
/// they have no OTIO track at all.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KtTrack {
    pub kind: kt_model::LaneKind,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub id: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub height: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mute: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub solo: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lock: bool,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub gain_db: f64,
    #[serde(default = "one_f64", skip_serializing_if = "is_one_f64")]
    pub opacity: f64,
    #[serde(default, skip_serializing_if = "is_normal_blend")]
    pub blend: kt_model::BlendMode,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fx_off: bool,
}

fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

fn one_f64() -> f64 {
    1.0
}

fn is_one_f64(v: &f64) -> bool {
    *v == 1.0
}

fn is_normal_blend(b: &kt_model::BlendMode) -> bool {
    *b == kt_model::BlendMode::Normal
}

/// Sidecar of a nested reel (R.50): everything that has no place in OTIO.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KtSequence {
    pub id: u64,
    pub name: String,
    #[serde(default = "one_u32")]
    pub video_lanes: u32,
    #[serde(default = "one_u32")]
    pub audio_lanes: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub morph_lanes: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub camera_lanes: u32,
    #[serde(default, skip_serializing)]
    pub lane_order: Vec<(kt_model::LaneKind, u32)>,
    /// Tracks of the reel (R.62) — as for the root.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<KtTrack>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub morphers: Vec<kt_model::MorpherEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<kt_model::CameraEvent>,
    /// Layer overlap inside a reel. Enabled by default, as for the root.
    #[serde(default = "yes_bool", skip_serializing_if = "is_true")]
    pub overlap_layers: bool,
    /// Overlapping transitions of the reel (R.51, item 399).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlap_transitions: Vec<KtOverlapTransition>,
    /// Public parameters of a REEL (R.59, stage 3): handles declared by the author
    /// with references inward into it. Empty — a reel without publications, and
    /// the file is byte-for-byte as before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub published: Vec<kt_model::PublishedParam>,
    /// Layer map of a REEL (R.28 one floor down): overlapping events of its one
    /// track lie as adjacent tracks inside its `Stack`. Without the map they
    /// would scatter across different tracks — the same defect as the root had.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<KtLayers>,
}

/// Overlapping transition (R.51, item 399): its pair lies in DIFFERENT adjacent
/// `Track`s (layer map), so it cannot be expressed as a child of a sequential track —
/// it travels as a list in the envelope of its OWN reel. No offsets: the window is
/// the overlap zone itself. The id is not stored: identity is the pair; the number is
/// assigned on read (as for junction positions — by the order of children).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KtOverlapTransition {
    pub a: u64,
    pub b: u64,
    #[serde(default, skip_serializing_if = "is_crossfade_kind")]
    pub kind: kt_model::TransitionKind,
    #[serde(default, skip_serializing_if = "is_equal_power")]
    pub audio: kt_model::AudioCurve,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto: Vec<kt_model::ParamCurve>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub invert: bool,
}

fn is_crossfade_kind(k: &kt_model::TransitionKind) -> bool {
    *k == kt_model::TransitionKind::Crossfade
}

fn is_equal_power(c: &kt_model::AudioCurve) -> bool {
    *c == kt_model::AudioCurve::EqualPower
}

fn one_u32() -> u32 {
    1
}

fn yes_bool() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

/// Layer map of tracks (R.28) + the project's layering policy.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KtLayers {
    /// Layering is off for this project (the answer “no” on the banner).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub off: bool,
    /// video[lane] = the number of Tracks of this video track.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub video: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio: Vec<u32>,
}

fn is_zero_usize(v: &usize) -> bool {
    *v == 0
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct KtSettings {
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub sample_rate: u32,
    /// Pitch engine (R.48). The default is not written — files before R.48
    /// are byte-stable, and an old build will read the file, simply not knowing
    /// about the choice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch_engine: Option<kt_model::PitchEngine>,
    /// Segment shape of NEW keys (R.61). Lives in the PROJECT, not in the
    /// program settings: otherwise one file would behave differently on two machines.
    /// The default is not written — earlier files stay byte-stable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_shape: Option<kt_model::SegShape>,
    /// Default frame conform rule (R.62). “Fit” is not
    /// written — earlier files stay byte-stable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conform: Option<kt_model::ConformRule>,
    /// Settings set explicitly (R.62): until then — the first clip sets
    /// the raster and fps. `false` is not written.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub settings_explicit: bool,
}

/// Region/loop — transport convenience outside OTIO semantics (ticks).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct KtTransport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<(i64, i64)>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub loop_enabled: bool,
}

/// Media not used on the timeline (bin): a path in the same format
/// as target_url.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KtBinMedia {
    pub id: u64,
    pub path: String,
}

/// Clip envelope: our identifiers and volume.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KtClipMeta {
    pub id: u64,
    pub media: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_group: Option<u64>,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub gain: f32,
    /// Video opacity (R.28, item 12). 1.0 is not written — files without it
    /// are byte-stable.
    #[serde(default = "one_f32", skip_serializing_if = "is_one_f32")]
    pub opacity: f32,
    /// Master switch of the event's FX chain (R.33). The default
    /// (enabled) is not written — files without it are byte-stable.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fx_off: bool,
    /// The true `start` in ticks — ONLY when it cannot be expressed by
    /// position on a sequential track (R.58, item 462): an event left of zero
    /// inside a reel, and everything that would otherwise land on the track's cursor after it.
    /// A regular edit does not carry the field — files without it are byte-stable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    /// Generator values of THIS event (R.59, stage 2).
    ///
    /// They also travel in `GeneratorReference.parameters` — but there this is
    /// a SNAPSHOT for a foreign reader that knows nothing about our curves.
    /// Our own reader reads from here: the values lie next to their own curves
    /// and do not depend on what a foreign editor did
    /// with the first-class schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#gen: Option<kt_model::GeneratorKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gen_auto: Vec<kt_model::ParamCurve>,
    /// Overrides of the public parameters of the REEL for THIS instance
    /// (R.59, stage 3). Empty — the instance plays the template.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<kt_model::Override>,
    /// The leading “Geometry” node (R.62). The default (follow the project,
    /// identical parameters) is not written — files stay byte-stable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<kt_model::EventGeometry>,
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub geometry_pos: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub geometry_auto: Vec<kt_model::ParamCurve>,
    /// Forward mercy (R.29): unknown kt fields of future versions.
    #[serde(default, flatten)]
    pub extra: Map<String, Value>,
}

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

fn one_f32() -> f32 {
    1.0
}

fn is_one_f32(v: &f32) -> bool {
    *v == 1.0
}

pub(crate) fn kt_get<T: for<'de> Deserialize<'de>>(metadata: &Map<String, Value>) -> Option<T> {
    serde_json::from_value(metadata.get(KT_NS)?.clone()).ok()
}

/// The same, but DISTINGUISHING “section absent” from “section corrupt”. The first is
/// a normal foreign file, the second is an error: `.ok()` there silently discarded the WHOLE
/// envelope, and the project opened as a foreign one — without settings, tracks,
/// morphers and layout. Exactly the class of defect forbidden by the invariant
/// “load errors are EXPLICIT” (defect p of the old format).
pub(crate) fn kt_get_checked<T: for<'de> Deserialize<'de>>(
    metadata: &Map<String, Value>,
) -> Result<Option<T>, serde_json::Error> {
    match metadata.get(KT_NS) {
        None => Ok(None),
        Some(v) => serde_json::from_value(v.clone()).map(Some),
    }
}

pub(crate) fn kt_set<T: Serialize>(metadata: &mut Map<String, Value>, v: &T) {
    let val = serde_json::to_value(v).expect("kt envelope is always serializable");
    match metadata.get_mut(KT_NS) {
        Some(slot) => *slot = val,
        None => {
            metadata.insert(KT_NS.into(), val);
        }
    }
}

/// Version of the file's kt envelope (None — a foreign file without our data).
pub fn kt_version(t: &Timeline) -> Option<u32> {
    t.metadata.get(KT_NS)?.get("v")?.as_u64().map(|v| v as u32)
}

/// A file from a newer Kinetren — an honest refusal instead of silent corruption.
pub fn check_kt_version(t: &Timeline) -> Result<(), Error> {
    match kt_version(t) {
        Some(v) if v > KT_META_VERSION => Err(Error::KtMetaTooNew(v)),
        _ => Ok(()),
    }
}

/// Reel address of an attachment (R.50): `metadata.kt.seq`.
///
/// It is by this that instances collapse into ONE reel on read: the body is inlined
/// into each instance, because there is no object identity in OTIO's JSON. No key —
/// the attachment is FOREIGN and is read as a new reel.
pub fn nest_seq_id(metadata: &Map<String, Value>) -> Option<u64> {
    metadata.get(KT_NS)?.as_object()?.get("seq")?.as_u64()
}

/// Our envelope in metadata: foreign namespaces are not touched; `kt` is
/// appended AT THE END on first appearance (the order of foreign keys — passthrough).
pub fn kt_entry_mut(metadata: &mut Map<String, Value>) -> &mut Map<String, Value> {
    if !metadata.contains_key(KT_NS) {
        metadata.insert(KT_NS.into(), Value::Object(Map::new()));
    }
    match metadata.get_mut(KT_NS) {
        Some(Value::Object(m)) => m,
        Some(other) => {
            // a foreign non-object “kt” — we overwrite it with ours (a namespace
            // collision; our priority in our files)
            *other = Value::Object(Map::new());
            match other {
                Value::Object(m) => m,
                _ => unreachable!(),
            }
        }
        None => unreachable!("inserted above"),
    }
}
