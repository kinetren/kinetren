//! Типизированные схемы OTIO + passthrough.
//!
//! Каждый объект файла несёт `"OTIO_SCHEMA": "Name.Version"`. Известные схемы
//! парсятся в типы ниже; неизвестные поля известных схем собирает
//! `#[serde(flatten)] extra` (порядок сохраняется — preserve_order);
//! неизвестные схемы/версии листьев остаются `RawObject` и пишутся обратно
//! verbatim. Версия структурного хребта (Timeline/Stack/Track) новее
//! поддерживаемой — жёсткая ошибка: «понимать наполовину» дорожку нельзя,
//! позиции её детей стали бы догадкой.
//!
//! Канонический порядок ключей на записи: `OTIO_SCHEMA` первым, затем
//! известные поля в порядке объявления структуры, затем `extra` в порядке
//! исходного документа. Порядок заморожен golden-тестами.

use indexmap::IndexMap;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::Error;

/// metadata любого объекта: произвольный вложенный JSON, чужие неймспейсы
/// нетронуты. Наш — `"kt"` (см. meta.rs).
pub type Metadata = Map<String, Value>;

/// Сырой объект неизвестной схемы: карта verbatim, включая `OTIO_SCHEMA`
/// на исходной позиции.
#[derive(Debug, Clone, PartialEq)]
pub struct RawObject(pub Map<String, Value>);

impl RawObject {
    pub fn schema_tag(&self) -> Option<&str> {
        self.0.get("OTIO_SCHEMA").and_then(Value::as_str)
    }
}

// ---- время ------------------------------------------------------------------

/// Время. Сериализуется С ТЕГОМ `OTIO_SCHEMA` — эталонная библиотека
/// отвергает вложенные время-объекты без тега («expected SerializableObject,
/// found AnyDictionary», найдено конформанс-тестом 149). Чтение тег
/// игнорирует (толерантно к его отсутствию).
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
    /// Наши тики: value целый (double точен до 2^53), rate = 90000.
    pub fn from_ticks(t: kt_time::Tick) -> Self {
        Self { value: t.0 as f64, rate: kt_time::TICK_HZ as f64 }
    }

    /// Чужие rate квантуются к сетке тиков (политика всей системы).
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

// ---- листовые схемы ---------------------------------------------------------

fn default_true() -> bool {
    true
}

/// Строковое поле, у которого в чужом файле может стоять `null` (так пишет
/// имена, например, апстримовский multitrack.otio). Эталон нормализует такой
/// null в пустую строку на первом же сохранении — делаем то же самое.
fn string_or_null<'de, D: serde::Deserializer<'de>>(de: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(de)?.unwrap_or_default())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    #[serde(default, deserialize_with = "string_or_null")]
    pub name: String,
    /// Дефолт эталонной библиотеки.
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
    /// Box2d — не моделируем, возим сырьём.
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
    /// Ссылок может не быть вовсе, а сама ссылка — быть `null`: клип без
    /// привязки к материалу законен (`media_reference: null` в Clip.1 —
    /// например tests/sample_data/transition_test.otio апстрима). Возим
    /// `null` как есть, чтобы пересохранение не выдумывало ссылку.
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

    /// available_range активной ссылки — источник длительности для клипа без
    /// `source_range` (спека OTIO: такой клип использует материал целиком).
    /// У ссылки неизвестной схемы поле читается из сырья: диапазон там лежит
    /// под тем же именем, а схема ссылки на его смысл не влияет.
    pub fn active_available_range(&self) -> Option<TimeRange> {
        match self.active_media()? {
            MediaRefNode::External(er) => er.available_range,
            MediaRefNode::Missing(mr) => mr.available_range,
            MediaRefNode::Unknown(raw) => raw
                .0
                .get("available_range")
                .and_then(|v| serde_json::from_value(v.clone()).ok()),
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
    /// "Video" | "Audio" (чужие значения не отвергаем — passthrough).
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
    /// Вложенный Stack обязан нести свой тег (эталон отвергает голый словарь);
    /// на чтении тег снимается, чтобы не оседать в `extra` и не дублироваться.
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

// ---- полиморфные узлы -------------------------------------------------------

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
    Unknown(RawObject),
}

// ---- диспетчеризация --------------------------------------------------------

pub(crate) fn split_schema(tag: &str) -> Result<(&str, u32), Error> {
    let (name, ver) = tag.rsplit_once('.').ok_or_else(|| Error::BadSchemaTag(tag.into()))?;
    let ver: u32 = ver.parse().map_err(|_| Error::BadSchemaTag(tag.into()))?;
    Ok((name, ver))
}

fn take_tag(map: &Map<String, Value>) -> Result<String, Error> {
    map.get("OTIO_SCHEMA")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(Error::MissingSchemaTag)
}

/// Типизированный парс без тега (тег уже снят вызывающим).
fn typed<T: for<'de> Deserialize<'de>>(mut map: Map<String, Value>) -> Result<T, Error> {
    map.shift_remove("OTIO_SCHEMA");
    Ok(serde_json::from_value(Value::Object(map))?)
}

/// Clip.1 → Clip.2: единственный писаный upgrade (как в эталоне).
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
        // хребет: версии новее поддерживаемой — жёсткий отказ
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
        _ => Ok(MediaRefNode::Unknown(RawObject(map))),
    }
}

// ---- запись: тег первым ключом ----------------------------------------------

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

// ---- serde-мосты для полиморфных узлов --------------------------------------
// Deserialize идёт через Value → decode_* (диспетчер по OTIO_SCHEMA с
// passthrough-фолбэком), Serialize — через encode_* (тег первым ключом).

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
    MediaRefNode::Unknown(raw) => Value::Object(raw.0.clone()),
});

/// Префлайт хребта ПО СЫРОМУ Value: serde-мост внутри `typed()` превращает
/// наши ошибки в `Json(custom)`, поэтому отказ «хребет новее поддерживаемого»
/// проверяется до типизированного парса. Рекурсия НЕ заходит внутрь
/// неизвестных схем — их содержимое passthrough и не интерпретируется.
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
