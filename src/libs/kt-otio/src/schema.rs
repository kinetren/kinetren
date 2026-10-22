//! Typed OTIO schemas + passthrough.
//!
//! Every object of the file carries an `OTIO_SCHEMA` field of the form `Name.Version`. Known schemas
//! are parsed into the types below; unknown fields of known schemas are collected by
//! `#[serde(flatten)] extra` (order is kept — preserve_order);
//! unknown schemas/versions of leaves remain `RawObject` and are written back
//! verbatim. A version of the structural spine (Timeline/Stack/Track) newer than
//! the supported one — a hard error: a track cannot be “half-understood”, the positions
//! of its children would become guesses.
//!
//! Canonical key order on write: `OTIO_SCHEMA` first, then the known fields in the order
//! of the struct declaration, then `extra` in the order of the source document.
//! The order is frozen by golden tests.

use indexmap::IndexMap;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::Error;

/// metadata of any object: arbitrary nested JSON, foreign namespaces
/// untouched. Ours — `"kt"` (see meta.rs).
pub type Metadata = Map<String, Value>;

/// Raw object of an unknown schema: the map verbatim, including `OTIO_SCHEMA`
/// at its source position.
// `transparent` (R.70): a raw node is exactly its object, with the tag inside; in
// the project file it travels as-is, so that export returns it byte-for-byte
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RawObject(pub Map<String, Value>);

impl RawObject {
    pub fn schema_tag(&self) -> Option<&str> {
        self.0.get("OTIO_SCHEMA").and_then(Value::as_str)
    }
}

// ---- time ------------------------------------------------------------------

/// Time. Serialized WITH the `OTIO_SCHEMA` TAG — the reference library
/// rejects nested time objects without a tag (“expected SerializableObject,
/// found AnyDictionary”, found by conformance test 149). On read the tag
/// is ignored (tolerant of its absence).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RationalTime {
    pub value: f64,
    pub rate: f64,
}

impl Serialize for RationalTime {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(3))?;
        m.serialize_entry("OTIO_SCHEMA", "RationalTime.1")?;
        m.serialize_entry("value", &self.value)?;
        m.serialize_entry("rate", &self.rate)?;
        m.end()
    }
}

impl<'de> Deserialize<'de> for RationalTime {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Repr {
            value: f64,
            rate: f64,
        }
        let r = Repr::deserialize(d)?;
        Ok(RationalTime { value: r.value, rate: r.rate })
    }
}

impl RationalTime {
    /// Our ticks: value is an integer (double is exact up to 2^53), rate = 90000.
    pub fn from_ticks(t: kt_time::Tick) -> Self {
        Self { value: t.0 as f64, rate: kt_time::TICK_HZ as f64 }
    }

    /// Foreign rates are quantized to the tick grid (a policy of the whole system).
    pub fn to_ticks(&self) -> kt_time::Tick {
        kt_time::Tick((self.value * kt_time::TICK_HZ as f64 / self.rate).round() as i64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeRange {
    pub start_time: RationalTime,
    pub duration: RationalTime,
}

impl Serialize for TimeRange {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(3))?;
        m.serialize_entry("OTIO_SCHEMA", "TimeRange.1")?;
        m.serialize_entry("duration", &self.duration)?;
        m.serialize_entry("start_time", &self.start_time)?;
        m.end()
    }
}

impl<'de> Deserialize<'de> for TimeRange {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Repr {
            start_time: RationalTime,
            duration: RationalTime,
        }
        let r = Repr::deserialize(d)?;
        Ok(TimeRange { start_time: r.start_time, duration: r.duration })
    }
}

// ---- leaf schemas ---------------------------------------------------------

fn default_true() -> bool {
    true
}

/// A string field which in a foreign file may hold `null` (as, for example, the upstream
/// multitrack.otio writes the names). The reference normalizes such a null
/// into an empty string on the first save — we do the same.
fn string_or_null<'de, D: serde::Deserializer<'de>>(de: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(de)?.unwrap_or_default())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    #[serde(default, deserialize_with = "string_or_null")]
    pub name: String,
    /// The default of the reference library.
    #[serde(default = "Marker::default_color")]
    pub color: String,
    pub marked_range: TimeRange,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Marker {
    pub fn default_color() -> String {
        "GREEN".into()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    #[serde(default, deserialize_with = "string_or_null")]
    pub name: String,
    pub effect_name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinearTimeWarp {
    #[serde(default, deserialize_with = "string_or_null")]
    pub name: String,
    #[serde(default)]
    pub effect_name: String,
    pub time_scalar: f64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalReference {
    #[serde(default, deserialize_with = "string_or_null")]
    pub name: String,
    pub target_url: String,
    pub available_range: Option<TimeRange>,
    /// Box2d — not modelled, carried as raw data.
    #[serde(default)]
    pub available_image_bounds: Option<Value>,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissingReference {
    #[serde(default, deserialize_with = "string_or_null")]
    pub name: String,
    pub available_range: Option<TimeRange>,
    #[serde(default)]
    pub available_image_bounds: Option<Value>,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Generator (R.52): a source that renders the picture itself. Our generators
/// are written with this schema first-class (`generator_kind = "kt.<tag>"`,
/// `parameters` — the fields of `kt_model::GeneratorKind`); FOREIGN generator_kind
/// values remain `Unknown` passthrough (see the guard in `decode_media_ref`) — their
/// byte idempotency matters more than typing (t154).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratorReference {
    #[serde(default, deserialize_with = "string_or_null")]
    pub name: String,
    pub generator_kind: String,
    pub available_range: Option<TimeRange>,
    #[serde(default)]
    pub available_image_bounds: Option<Value>,
    #[serde(default)]
    pub parameters: Map<String, Value>,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    #[serde(default, deserialize_with = "string_or_null")]
    pub name: String,
    pub transition_type: String,
    pub in_offset: RationalTime,
    pub out_offset: RationalTime,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

// ---- items ------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    #[serde(default, deserialize_with = "string_or_null")]
    pub name: String,
    pub source_range: Option<TimeRange>,
    /// References may be absent altogether, and the reference itself may be `null`: a clip
    /// not bound to material is legal (`media_reference: null` in Clip.1 —
    /// for example tests/sample_data/transition_test.otio upstream). We carry
    /// `null` as is, so that re-saving does not invent a reference.
    #[serde(default)]
    pub media_references: IndexMap<String, Option<MediaRefNode>>,
    #[serde(default = "default_media_key")]
    pub active_media_reference_key: String,
    #[serde(default)]
    pub effects: Vec<EffectNode>,
    #[serde(default)]
    pub markers: Vec<MarkerNode>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn default_media_key() -> String {
    crate::DEFAULT_MEDIA_KEY.into()
}

impl Clip {
    pub fn active_media(&self) -> Option<&MediaRefNode> {
        self.media_references.get(&self.active_media_reference_key)?.as_ref()
    }

    /// available_range of the active reference — the duration source for a clip without
    /// `source_range` (OTIO spec: such a clip uses the material in full).
    /// For a reference of an unknown schema the field is read from raw data: the range lies
    /// under the same name there, and the reference's schema does not affect its meaning.
    pub fn active_available_range(&self) -> Option<TimeRange> {
        match self.active_media()? {
            MediaRefNode::External(er) => er.available_range,
            MediaRefNode::Missing(mr) => mr.available_range,
            MediaRefNode::Generator(gr) => gr.available_range,
            MediaRefNode::Unknown(raw) => {
                raw.0.get("available_range").and_then(|v| serde_json::from_value(v.clone()).ok())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gap {
    #[serde(default, deserialize_with = "string_or_null")]
    pub name: String,
    pub source_range: Option<TimeRange>,
    #[serde(default)]
    pub effects: Vec<EffectNode>,
    #[serde(default)]
    pub markers: Vec<MarkerNode>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    #[serde(default, deserialize_with = "string_or_null")]
    pub name: String,
    #[serde(default)]
    pub children: Vec<Composable>,
    pub source_range: Option<TimeRange>,
    #[serde(default)]
    pub effects: Vec<EffectNode>,
    #[serde(default)]
    pub markers: Vec<MarkerNode>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    #[serde(default, deserialize_with = "string_or_null")]
    pub name: String,
    /// “Video” | “Audio” (foreign values are not rejected — passthrough).
    #[serde(default, deserialize_with = "string_or_null")]
    pub kind: String,
    #[serde(default)]
    pub children: Vec<Composable>,
    pub source_range: Option<TimeRange>,
    #[serde(default)]
    pub effects: Vec<EffectNode>,
    #[serde(default)]
    pub markers: Vec<MarkerNode>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    #[serde(default, deserialize_with = "string_or_null")]
    pub name: String,
    pub global_start_time: Option<RationalTime>,
    #[serde(default)]
    pub metadata: Metadata,
    /// A nested Stack must carry its own tag (the reference rejects a bare dictionary);
    /// on read the tag is removed, so that it does not settle into `extra` and get duplicated.
    #[serde(serialize_with = "tracks_ser", deserialize_with = "tracks_de")]
    pub tracks: Stack,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn tracks_ser<S: serde::Serializer>(stack: &Stack, ser: S) -> Result<S::Ok, S::Error> {
    encode("Stack.1", stack).serialize(ser)
}

fn tracks_de<'de, D: serde::Deserializer<'de>>(de: D) -> Result<Stack, D::Error> {
    let mut map = Map::<String, Value>::deserialize(de)?;
    map.shift_remove("OTIO_SCHEMA");
    serde_json::from_value(Value::Object(map)).map_err(serde::de::Error::custom)
}

// ---- polymorphic nodes -------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Composable {
    Clip(Clip),
    Gap(Gap),
    Transition(Transition),
    Stack(Stack),
    Track(Track),
    Unknown(RawObject),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MarkerNode {
    Marker(Marker),
    Unknown(RawObject),
}

#[derive(Debug, Clone, PartialEq)]
pub enum EffectNode {
    Effect(Effect),
    LinearTimeWarp(LinearTimeWarp),
    Unknown(RawObject),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MediaRefNode {
    External(ExternalReference),
    Missing(MissingReference),
    Generator(GeneratorReference),
    Unknown(RawObject),
}

// ---- dispatch --------------------------------------------------------

pub(crate) fn split_schema(tag: &str) -> Result<(&str, u32), Error> {
    let (name, ver) = tag.rsplit_once('.').ok_or_else(|| Error::BadSchemaTag(tag.into()))?;
    let ver: u32 = ver.parse().map_err(|_| Error::BadSchemaTag(tag.into()))?;
    Ok((name, ver))
}

fn take_tag(map: &Map<String, Value>) -> Result<String, Error> {
    map.get("OTIO_SCHEMA").and_then(Value::as_str).map(str::to_owned).ok_or(Error::MissingSchemaTag)
}

/// Typed parse without a tag (the tag has already been removed by the caller).
fn typed<T: for<'de> Deserialize<'de>>(mut map: Map<String, Value>) -> Result<T, Error> {
    map.shift_remove("OTIO_SCHEMA");
    Ok(serde_json::from_value(Value::Object(map))?)
}

/// Clip.1 → Clip.2: the only written upgrade (as in the reference).
fn upgrade_clip_1(mut map: Map<String, Value>) -> Map<String, Value> {
    if let Some(mr) = map.shift_remove("media_reference") {
        let mut refs = Map::new();
        refs.insert(crate::DEFAULT_MEDIA_KEY.into(), mr);
        map.insert("media_references".into(), Value::Object(refs));
        map.insert(
            "active_media_reference_key".into(),
            Value::String(crate::DEFAULT_MEDIA_KEY.into()),
        );
    }
    map
}

fn as_object(v: Value) -> Result<Map<String, Value>, Error> {
    match v {
        Value::Object(m) => Ok(m),
        _ => Err(Error::NotAnObject),
    }
}

pub(crate) fn decode_composable(v: Value) -> Result<Composable, Error> {
    let map = as_object(v)?;
    let tag = take_tag(&map)?;
    let (name, ver) = split_schema(&tag)?;
    match (name, ver) {
        ("Clip", 1) => Ok(Composable::Clip(typed(upgrade_clip_1(map))?)),
        ("Clip", 2) => Ok(Composable::Clip(typed(map)?)),
        ("Gap", 1) => Ok(Composable::Gap(typed(map)?)),
        ("Transition", 1) => Ok(Composable::Transition(typed(map)?)),
        // Structural spine: versions newer than the supported one — a hard refusal
        ("Stack", 1) => Ok(Composable::Stack(typed(map)?)),
        ("Track", 1) => Ok(Composable::Track(typed(map)?)),
        ("Stack" | "Track", _) => Err(Error::UnsupportedSchema(tag)),
        _ => Ok(Composable::Unknown(RawObject(map))),
    }
}

fn decode_marker(v: Value) -> Result<MarkerNode, Error> {
    let map = as_object(v)?;
    let tag = take_tag(&map)?;
    match split_schema(&tag)? {
        ("Marker", 2) => Ok(MarkerNode::Marker(typed(map)?)),
        _ => Ok(MarkerNode::Unknown(RawObject(map))),
    }
}

fn decode_effect(v: Value) -> Result<EffectNode, Error> {
    let map = as_object(v)?;
    let tag = take_tag(&map)?;
    match split_schema(&tag)? {
        ("Effect", 1) => Ok(EffectNode::Effect(typed(map)?)),
        ("LinearTimeWarp", 1) => Ok(EffectNode::LinearTimeWarp(typed(map)?)),
        _ => Ok(EffectNode::Unknown(RawObject(map))),
    }
}

fn decode_media_ref(v: Value) -> Result<MediaRefNode, Error> {
    let map = as_object(v)?;
    let tag = take_tag(&map)?;
    match split_schema(&tag)? {
        ("ExternalReference", 1) => Ok(MediaRefNode::External(typed(map)?)),
        ("MissingReference", 1) => Ok(MediaRefNode::Missing(typed(map)?)),
        // Only OUR generators are typed: foreign ones (SMPTEBars, Premiere titles)
        // remain passthrough — their key order and unknown fields are obliged to survive
        // re-saving byte-for-byte (t154)
        ("GeneratorReference", 1)
            if map
                .get("generator_kind")
                .and_then(Value::as_str)
                .is_some_and(|k| k.starts_with("kt.") || crate::generator_alias(k).is_some()) =>
        {
            Ok(MediaRefNode::Generator(typed(map)?))
        }
        _ => Ok(MediaRefNode::Unknown(RawObject(map))),
    }
}

// ---- writing: the tag as the first key ----------------------------------------------

fn tagged(tag: &str, inner: Value) -> Value {
    let Value::Object(fields) = inner else {
        unreachable!("schema structs serialize to objects");
    };
    let mut out = Map::with_capacity(fields.len() + 1);
    out.insert("OTIO_SCHEMA".into(), Value::String(tag.into()));
    out.extend(fields);
    Value::Object(out)
}

fn encode<T: Serialize>(tag: &str, x: &T) -> Value {
    tagged(tag, serde_json::to_value(x).expect("schema struct is always serializable"))
}

pub(crate) fn encode_composable(c: &Composable) -> Value {
    match c {
        Composable::Clip(x) => encode("Clip.2", x),
        Composable::Gap(x) => encode("Gap.1", x),
        Composable::Transition(x) => encode("Transition.1", x),
        Composable::Stack(x) => encode("Stack.1", x),
        Composable::Track(x) => encode("Track.1", x),
        Composable::Unknown(raw) => Value::Object(raw.0.clone()),
    }
}

// ---- serde bridges for polymorphic nodes --------------------------------------
// Deserialize goes through Value → decode_* (a dispatcher by OTIO_SCHEMA with a
// passthrough fallback), Serialize — through encode_* (the tag as the first key).

macro_rules! node_serde {
    ($ty:ty, $decode:path, $encode:expr) => {
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let v = Value::deserialize(d)?;
                $decode(v).map_err(D::Error::custom)
            }
        }
        impl Serialize for $ty {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let enc: fn(&$ty) -> Value = $encode;
                enc(self).serialize(s)
            }
        }
    };
}

node_serde!(Composable, decode_composable, |c| encode_composable(c));
node_serde!(MarkerNode, decode_marker, |m| match m {
    MarkerNode::Marker(x) => encode("Marker.2", x),
    MarkerNode::Unknown(raw) => Value::Object(raw.0.clone()),
});
node_serde!(EffectNode, decode_effect, |e| match e {
    EffectNode::Effect(x) => encode("Effect.1", x),
    EffectNode::LinearTimeWarp(x) => encode("LinearTimeWarp.1", x),
    EffectNode::Unknown(raw) => Value::Object(raw.0.clone()),
});
node_serde!(MediaRefNode, decode_media_ref, |m| match m {
    MediaRefNode::External(x) => encode("ExternalReference.1", x),
    MediaRefNode::Missing(x) => encode("MissingReference.1", x),
    MediaRefNode::Generator(x) => encode("GeneratorReference.1", x),
    MediaRefNode::Unknown(raw) => Value::Object(raw.0.clone()),
});

/// Preflight of the spine on the RAW Value: the serde bridge inside `typed()` turns our
/// errors into `Json(custom)`, so the refusal “spine newer than supported”
/// is checked before the typed parse. The recursion does NOT go inside
/// unknown schemas — their content is passthrough and is not interpreted.
fn preflight_backbone(v: &Value) -> Result<(), Error> {
    let Some(map) = v.as_object() else { return Ok(()) };
    let Some(tag) = map.get("OTIO_SCHEMA").and_then(Value::as_str) else { return Ok(()) };
    let Ok((name, ver)) = split_schema(tag) else { return Ok(()) };
    if matches!(name, "Timeline" | "Stack" | "Track") {
        if ver > 1 {
            return Err(Error::UnsupportedSchema(tag.into()));
        }
        if let Some(tracks) = map.get("tracks") {
            preflight_backbone(tracks)?;
        }
        if let Some(Value::Array(children)) = map.get("children") {
            for c in children {
                preflight_backbone(c)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn decode_timeline(v: Value) -> Result<Timeline, Error> {
    preflight_backbone(&v)?;
    let map = as_object(v)?;
    let tag = match take_tag(&map) {
        Ok(t) => t,
        Err(Error::MissingSchemaTag) => return Err(Error::NotOtio),
        Err(e) => return Err(e),
    };
    match split_schema(&tag)? {
        ("Timeline", 1) => typed(map),
        ("Timeline", _) => Err(Error::UnsupportedSchema(tag)),
        _ => Err(Error::UnsupportedRoot(tag)),
    }
}

pub(crate) fn encode_timeline(t: &Timeline) -> Value {
    encode("Timeline.1", t)
}
