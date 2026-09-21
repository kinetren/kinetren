//! Конверт `metadata["kt"]` — наш неймспейс в OTIO (решение Р.24: коротко
//! "kt", не "kinotrain"). Версия одна на ФАЙЛ: `Timeline.metadata.kt.v`;
//! вложенные kt-конверты (клипы, эффекты, переходы) версии не несут.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{Error, Timeline};

pub const KT_NS: &str = "kt";
/// Максимальная ПОНИМАЕМАЯ версия конверта. Писатель ставит версию ПО
/// СОДЕРЖИМОМУ (Р.29): v=2 ⇔ файл несёт камеры/камерные дорожки (старый
/// ридер молча потерял бы их при перезаписи → штатный отказ KtMetaTooNew,
/// правило р.24 «наше чтобы никто не ломал»); иначе v=1 — файлы без 3D
/// байт-стабильны и открываются старыми сборками. Один Pan3d версию НЕ
/// бампает: неизвестный эффект деградирует в foreign-passthrough.
pub const KT_META_VERSION: u32 = 9;
pub const KT_META_V1: u32 = 1;
/// Камеры и камерные дорожки (Р.29).
pub const KT_META_V2: u32 = 2;
/// Кривые автоматизации в тиках МАТЕРИАЛА (Р.33): старая сборка прочла бы
/// их как доли 0..1 и уронила бы валидацию — отказ честнее тихой порчи.
pub const KT_META_V3: u32 = 3;
/// Морфер-СЛОЙ (Р.35): действует по времени блока и по умолчанию на всё,
/// что ниже его строки; порядок дорожек переехал в модель. Старая сборка
/// прочла бы слой как патч на пустой список целей и молча отрендерила бы
/// не то — отказ честнее тихой порчи.
pub const KT_META_V4: u32 = 4;
/// Автоматизация и сила морфер-слоя (Р.38, пп.12-13). Кривые слоя живут
/// в его патче, сила и фейды — на самом событии; старая сборка потеряла
/// бы и то и другое молча, а «слой на 30%» превратился бы в «слой
/// целиком» — испорченный монтаж, а не потерянная мелочь.
pub const KT_META_V5: u32 = 5;
/// Вложенные секвенции (Р.50). Тело ленты инлайнится в каждый экземпляр
/// `Stack`-ом, поэтому ЧУЖОЙ редактор увидит правильный монтаж и без
/// нашего конверта; а вот СТАРАЯ НАША сборка прочла бы вложение серым
/// плейсхолдером `ForeignEvent` (п.158) и при первой же перезаписи вернула
/// бы его в файл сырьём — то есть превратила бы живой монтаж в мёртвый
/// кусок JSON. Отказ честнее.
pub const KT_META_V6: u32 = 6;
/// Переходы Р.51: инверсия (`invert` в конверте перехода). Старая сборка
/// сыграла бы переход НЕинвертированным — направление жеста в кадре
/// противоположное, то есть молча испорченный монтаж; сам ключ она бы
/// даже сохранила (passthrough непрочитанного конверта), что делает
/// порчу ещё незаметнее. Отказ честнее.
pub const KT_META_V7: u32 = 7;
/// Событие ЛЕВЕЕ НУЛЯ внутри ленты (Р.58, п.462): `start` в конверте клипа
/// (`kt.start`) там, где позицией в последовательном треке его не выразить.
/// Старая сборка положила бы такое событие в ноль и сдвинула бы всё
/// содержимое ленты за ним — молча; отказ честнее.
pub const KT_META_V8: u32 = 8;
/// Параметры генератора НА СОБЫТИИ (Р.59, стадия 2). Старая сборка
/// прочла бы значения из пути медиа — а их там больше нет, — и показала
/// бы пустой титр вместо заголовка, причём во всех местах сразу. Отказ
/// честнее.
/// Той же версией (решение раунда: версия ОДНА, поднимается ПО
/// СОДЕРЖИМОМУ) — публичные параметры композиции и переопределения у
/// экземпляра (Р.59, стадия 3). Старый ридер сыграл бы все экземпляры
/// шаблонными значениями: десять выпусков новостей с одним заголовком,
/// и ни одного видимого признака. Отказ честнее.
pub const KT_META_V9: u32 = 9;

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
    /// Порядок строк дорожек (Р.35): переехал из `ui.lane_order` в
    /// модель, потому что морфер-слой сделал его частью КАРТИНКИ.
    /// Пусто = естественный блочный порядок.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lane_order: Vec<(kt_model::LaneKind, u32)>,
    /// Именованные области «время × дорожки» (Р.35 п.2). Своих сущностей
    /// в OTIO у них нет (Marker описывает только время), поэтому едут в
    /// нашей секции целиком.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub areas: Vec<kt_model::MarkedArea>,
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
    /// Камерные дорожки и камеры (Р.29, 3D-композитинг). Наличие любого
    /// из них поднимает версию файла до v2 (см. KT_META_VERSION).
    /// Камеры сортируются по CameraId (детерминизм, тест 144).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub camera_lanes: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<kt_model::CameraEvent>,
    /// Вложенные ленты (Р.50) — то, чего OTIO не выражает: число дорожек,
    /// порядок строк, морфер-слои и камеры ВНУТРИ ленты.
    ///
    /// Сам МОНТАЖ ленты сюда не едет: он инлайнится `Stack`-ом в каждый
    /// экземпляр, и чужой редактор читает его штатно. Здесь — сайдкар по
    /// образцу `KtLayers`: не сошёлся с файлом — notice и чтение как есть,
    /// а не порча. Сортируются по адресу (детерминизм, т.144).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sequences: Vec<KtSequence>,
    /// Наложенные переходы КОРНЕВОЙ ленты (Р.51, п.399); у вложенных —
    /// то же поле в их `KtSequence`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlap_transitions: Vec<KtOverlapTransition>,
    /// Публичные параметры ленты, стоящей на месте КОРНЯ (Р.59, стадия
    /// 3). В сохранённом документе это всегда пусто — корень никому не
    /// экземпляр, — но молча терять содержимое модели нельзя.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub published: Vec<kt_model::PublishedParam>,
    /// Форвард-милосердие к будущим версиям (Р.29): неизвестные kt-поля
    /// v3+ возятся passthrough'ом — следующие добавки не потребуют
    /// жёсткого отказа, если старый ридер ничего из них не теряет.
    #[serde(default, flatten)]
    pub extra: Map<String, Value>,
}

/// Сайдкар вложенной ленты (Р.50): всё, чему в OTIO места нет.
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lane_order: Vec<(kt_model::LaneKind, u32)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub morphers: Vec<kt_model::MorpherEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<kt_model::CameraEvent>,
    /// Переслаивание внутри ленты. По умолчанию включено, как у корня.
    #[serde(default = "yes_bool", skip_serializing_if = "is_true")]
    pub overlap_layers: bool,
    /// Наложенные переходы ленты (Р.51, п.399).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlap_transitions: Vec<KtOverlapTransition>,
    /// Публичные параметры ЛЕНТЫ (Р.59, стадия 3): объявленные автором
    /// ручки со ссылками внутрь неё. Пусто — лента без публикаций, и
    /// файл байт-в-байт прежний.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub published: Vec<kt_model::PublishedParam>,
    /// Карта слоёв ЛЕНТЫ (Р.28 этажом ниже): перекрытые события одной её
    /// дорожки лежат соседними треками внутри её `Stack`. Без карты они
    /// разъехались бы по разным дорожкам — тот же дефект, что был у корня.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<KtLayers>,
}

/// Наложенный переход (Р.51, п.399): его пара лежит в РАЗНЫХ соседних
/// `Track` (карта слоёв), поэтому ребёнком последовательного трека он
/// невыразим — едет списком в конверте СВОЕЙ ленты. Офсетов нет: окно —
/// сама зона перекрытия. Id не хранится: идентичность — пара, номер
/// назначается на чтении (как у стыковых позиция — порядком детей).
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
    /// Движок питча (Р.48). Дефолт не пишется — файлы до Р.48
    /// байт-стабильны, а старая сборка прочтёт файл, просто не зная
    /// про выбор.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch_engine: Option<kt_model::PitchEngine>,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Мастер-выключатель FX-цепочки события (Р.33). Умолчание
    /// (включено) не пишется — файлы без него байт-стабильны.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fx_off: bool,
    /// Истинный `start` в тиках — ТОЛЬКО когда его нельзя выразить
    /// позицией в последовательном треке (Р.58, п.462): событие левее нуля
    /// внутри ленты и всё, что после него легло бы на курсор трека.
    /// Обычный монтаж поля не несёт — файлы без него байт-стабильны.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    /// Значения генератора ЭТОГО события (Р.59, стадия 2).
    ///
    /// В `GeneratorReference.parameters` они тоже уезжают — но там это
    /// СНИМОК для чужого читателя, который про наши кривые ничего не
    /// знает. Свой ридер читает отсюда: здесь значения лежат рядом со
    /// своими кривыми и не зависят от того, что чужой редактор сделал с
    /// первоклассной схемой.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#gen: Option<kt_model::GeneratorKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gen_auto: Vec<kt_model::ParamCurve>,
    /// Переопределения публичных параметров ЛЕНТЫ у ЭТОГО экземпляра
    /// (Р.59, стадия 3). Пусто — экземпляр играет шаблон.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<kt_model::Override>,
    /// Форвард-милосердие (Р.29): неизвестные kt-поля будущих версий.
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

/// То же, но с РАЗЛИЧЕНИЕМ «секции нет» и «секция битая». Первое —
/// нормальный чужой файл, второе — ошибка: `.ok()` там выбрасывал ВЕСЬ
/// конверт молча, и проект открывался как чужой — без настроек, дорожек,
/// морферов и раскладки. Ровно тот класс дефектов, что запрещён
/// инвариантом «ошибки загрузки ЯВНЫЕ» (дефект p старого формата).
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

/// Адрес ленты у вложения (Р.50): `metadata.kt.seq`.
///
/// Именно по нему экземпляры схлопываются в ОДНУ ленту при чтении: тело
/// инлайнится в каждый экземпляр, потому что объектной идентичности в
/// JSON OTIO нет. Нет ключа — вложение ЧУЖОЕ, и читается как новая лента.
pub fn nest_seq_id(metadata: &Map<String, Value>) -> Option<u64> {
    metadata.get(KT_NS)?.as_object()?.get("seq")?.as_u64()
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
