//! Конверт `metadata["kt"]` — наш неймспейс в OTIO (решение Р.24: коротко
//! "kt", не "kinotrain"). Версия одна на ФАЙЛ: `Timeline.metadata.kt.v`;
//! вложенные kt-конверты (клипы, эффекты, переходы) версии не несут.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{Error, Timeline};

pub const KT_NS: &str = "kt";
pub const KT_META_VERSION: u32 = 1;

// ---- типизированные конверты metadata.kt (приём otio-types) -----------------
// Структурно-мелкое — СВОИ типы (эволюция модели не двигает схему файла
// молча); тяжёлое и стабильное (FxInstance, MorpherEvent, BinGroup, LaneKind)
// переиспользует serde-derive живых типов kt-model — дрейф ловит байт-golden
// фикстуры (тест 148).

/// Конверт Timeline.metadata.kt — всё, чему нет стандартного места в OTIO.
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
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub morph_lanes: u32,
    /// Монтажная длина таймлайна в тиках, когда она БОЛЬШЕ материала
    /// (хвостовой Gap чужого файла; решение пользователя Р.25). Пустые
    /// дорожки, которых не было в файле, мы не пишем — их число едет здесь.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lanes: Option<(u32, u32)>,
    /// Переслаивание (Р.28): сколько подряд идущих Track'ов занимает каждая
    /// наша дорожка. В OTIO перекрытие внутри одного Track невыразимо
    /// (позиция клипа = порядок детей), поэтому слои пишутся соседними
    /// треками — а по этой карте мы собираем дорожку обратно. Пишется,
    /// только когда слой не один: файлы без перекрытий байт-в-байт прежние.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<KtLayers>,
    /// Сериализуются отсортированными по MorphId (детерминизм применения).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub morphers: Vec<kt_model::MorpherEvent>,
}

/// Карта слоёв дорожек (Р.28) + политика переслаивания проекта.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KtLayers {
    /// Переслаивание выключено для этого проекта (ответ «нет» на плашке).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub off: bool,
    /// video[lane] = число Track'ов этой видео-дорожки.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub video: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio: Vec<u32>,
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
}

/// Регион/loop — транспорт-удобство вне OTIO-семантики (тики).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct KtTransport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<(i64, i64)>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub loop_enabled: bool,
}

/// Медиа, не задействованное на таймлайне (бин): путь в том же формате,
/// что target_url.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KtBinMedia {
    pub id: u64,
    pub path: String,
}

/// Конверт клипа: наши идентификаторы и громкость.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct KtClipMeta {
    pub id: u64,
    pub media: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_group: Option<u64>,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub gain: f32,
    /// Непрозрачность видео (Р.28, п.12). 1.0 не пишется — файлы без неё
    /// байт-стабильны.
    #[serde(default = "one_f32", skip_serializing_if = "is_one_f32")]
    pub opacity: f32,
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

pub(crate) fn kt_set<T: Serialize>(metadata: &mut Map<String, Value>, v: &T) {
    let val = serde_json::to_value(v).expect("kt envelope is always serializable");
    match metadata.get_mut(KT_NS) {
        Some(slot) => *slot = val,
        None => {
            metadata.insert(KT_NS.into(), val);
        }
    }
}

/// Версия kt-конверта файла (None — чужой файл без наших данных).
pub fn kt_version(t: &Timeline) -> Option<u32> {
    t.metadata.get(KT_NS)?.get("v")?.as_u64().map(|v| v as u32)
}

/// Файл из более нового Kinotrain — честный отказ вместо тихой порчи.
pub fn check_kt_version(t: &Timeline) -> Result<(), Error> {
    match kt_version(t) {
        Some(v) if v > KT_META_VERSION => Err(Error::KtMetaTooNew(v)),
        _ => Ok(()),
    }
}

/// Наш конверт в metadata: чужие неймспейсы не трогаются, `kt` при первом
/// появлении дописывается В КОНЕЦ (порядок чужих ключей — passthrough).
pub fn kt_entry_mut(metadata: &mut Map<String, Value>) -> &mut Map<String, Value> {
    if !metadata.contains_key(KT_NS) {
        metadata.insert(KT_NS.into(), Value::Object(Map::new()));
    }
    match metadata.get_mut(KT_NS) {
        Some(Value::Object(m)) => m,
        Some(other) => {
            // чужое не-объектное "kt" — перезаписываем своим (коллизия
            // неймспейса, наш приоритет в наших файлах)
            *other = Value::Object(Map::new());
            match other {
                Value::Object(m) => m,
                _ => unreachable!(),
            }
        }
        None => unreachable!("inserted above"),
    }
}
