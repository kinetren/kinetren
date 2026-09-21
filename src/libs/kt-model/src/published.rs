//! Публичные параметры композиции и переопределения у экземпляра
//! (Р.59, стадия 3).
//!
//! Владелец назвал предмет дословно: «проект-шаблон новостного выпуска…
//! очевидно не будут же люди каждый раз лезть в композицию и менять там
//! текст логотипы». Механизм — ровно два поля и одно правило:
//!
//! * у ЛЕНТЫ есть объявленный автором список ручек (`published`) —
//!   ссылок на свойства внутри неё, с ИМЕНЕМ, которое придумал автор;
//! * у ЭКЗЕМПЛЯРА есть список переопределений (`overrides`) — значений
//!   этих и только этих ручек.
//!
//! **Движок про публикацию не знает вовсе.** Переопределение разрешается
//! там же, где применяются морфер-слои, — в модели, при выдаче клипов
//! (`Project::resolve_reel`). Движку достаются готовые клипы, а РЕДАКЦИИ
//! ленты он различает хэшем; меняется ровно одно — редакция становится
//! функцией ПАРЫ (лента, набор переопределений экземпляра).
//!
//! Два решения, которые легко потерять:
//!
//! 1. **Значение шаблона НЕ ХРАНИТСЯ.** Оно читается у цели живьём
//!    (`Project::addr_value`). Хранимая копия — второй источник того же
//!    факта: автор правит ленту, копия остаётся прежней, и «значение по
//!    умолчанию» молча врёт.
//! 2. **Переопределения ложатся ДО морфер-слоёв.** Адрес автора — это
//!    структура ленты, какой он её видит; морфер дописывает слоты В КОНЕЦ
//!    цепочки, а при выключенной цепочке события чистит её целиком —
//!    лягни мы после, `FxSlot(i)` указывал бы не туда.

use serde::{Deserialize, Serialize};

use crate::genmedia::GeneratorKind;
use crate::project::{
    CameraEvent, CameraId, ClipEvent, ClipId, HasParams, MediaId, ParamCurve, ParamType,
    ParamValue, TransitionEvent, TransitionId, clip_material_at,
};

/// Номер публичного параметра внутри ленты. Адрес, переживающий
/// переименование: имя придумывает автор и правит его когда хочет.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PubId(pub u64);

/// Сужение пределов объявленной ручки (разведка F-TL-012, фишка 1: автор
/// выносит наружу слайдер с пределами, чтобы потребитель физически не мог
/// увести титр за кадр). Пусто — пределы цели как есть.
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

    /// Зажать число объявленными пределами. Пределы цели при этом никуда
    /// не деваются — их применяет сам `set_param`, и порядок неважен:
    /// обе операции — кламп.
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

/// Кому принадлежит свойство внутри ленты.
///
/// У `Transition` и `Camera` поле `ParamAddr::clip` не используется:
/// адрес несут их собственные номера. Оставлено одним типом намеренно —
/// второй «адрес без клипа» развёл бы код публикации на две ветки.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "owner", rename_all = "snake_case")]
pub enum ParamOwner {
    /// Слот цепочки эффектов события (индекс в `ClipEvent::fx`).
    FxSlot {
        slot: usize,
    },
    /// Значения генератора события (Р.59, стадия 2) — текст титра, цвет
    /// плашки, кегль.
    Generator,
    /// Свойство самого события: `opacity`, `gain_db`, `media`, `reversed`.
    ClipAttr,
    Transition {
        id: TransitionId,
    },
    Camera {
        id: CameraId,
    },
    /// Публичный параметр ленты, вложенной ВНУТРЬ этой (шаблон в
    /// шаблоне). Внешняя лента патчит `overrides` внутреннего экземпляра,
    /// дальше он разбирается сам — рекурсия бесплатна по построению.
    Nested {
        pub_id: PubId,
    },
}

/// Адрес свойства ВНУТРИ ленты. Приклеен к `ClipId`, поэтому переживает
/// трим, перенос и ЛЕВУЮ половину разреза (кусок №0 держит id — то же
/// правило, что у морфер-резки).
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

/// Объявленная наружу ручка ленты.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublishedParam {
    pub id: PubId,
    /// Свободный текст АВТОРА (решение владельца Р.59). Это содержимое
    /// документа, как имя события, и в каталог переводов программы оно не
    /// попадёт никогда.
    pub name: String,
    pub target: ParamAddr,
    /// Тип, каким его видел автор при публикации. Расхождение с целью —
    /// явная ошибка валидации, а не молчаливое приведение.
    pub ty: ParamType,
    #[serde(default, skip_serializing_if = "PubRange::is_empty")]
    pub range: PubRange,
}

/// Переопределение ОДНОЙ объявленной ручки у ОДНОГО экземпляра.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Override {
    pub pub_id: PubId,
    pub value: ParamValue,
    /// Кривая живёт в МАТЕРИАЛЬНОМ времени ЭКЗЕМПЛЯРА — а это и есть
    /// время ленты. В материал цели её переводит тот же код, что кривые
    /// морфер-слоя (`clip_material_at`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub curves: Vec<ParamCurve>,
}

impl Override {
    pub fn value(pub_id: PubId, value: ParamValue) -> Override {
        Override { pub_id, value, curves: Vec::new() }
    }
}

/// Содержимое ленты, РАЗРЕШЁННОЕ под конкретный набор переопределений.
///
/// `Cow` здесь не украшение: пустой набор обязан идти путём, побайтово
/// равным прежнему, — а прежний путь ничего не клонировал (t673/t676).
#[derive(Debug, Clone)]
pub struct ResolvedReel<'a> {
    pub clips: Vec<ClipEvent>,
    pub transitions: std::borrow::Cow<'a, [TransitionEvent]>,
    pub cameras: std::borrow::Cow<'a, [CameraEvent]>,
}

/// Может ли ручка этого носителя нести КРИВУЮ.
///
/// Правило одно и оно про ДОМЕН: кривая переопределения приезжает в
/// материальном времени экземпляра и переводится в материал цели
/// `clip_material_at`. У перехода домен — тик от начала окна, у камеры —
/// прогресс события: перевода туда не существует, и «почти правильная»
/// кривая там была бы хуже отказа. У свойства события (`opacity`,
/// `gain_db`) носителя кривой нет вовсе.
pub fn owner_takes_curves(owner: &ParamOwner) -> bool {
    matches!(owner, ParamOwner::FxSlot { .. } | ParamOwner::Generator | ParamOwner::Nested { .. })
}

/// Перевести ключи кривой из времени ЛЕНТЫ в материал цели.
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

/// Положить кривые переопределения в слот, заменив одноимённые.
fn put_curves(slot: &mut Vec<ParamCurve>, curves: Vec<ParamCurve>) {
    for c in curves {
        slot.retain(|x| x.param != c.param);
        slot.push(c);
    }
}

/// Применить переопределения к содержимому ленты — ДО морфер-слоёв.
///
/// `default_gen` отвечает на «этот адрес медиа — генератор какого вида?»:
/// подмена источника обязана согласовать значения генератора с новым
/// медиа тем же правилом, что `fit_gen_to_media`, иначе логотип-файл
/// остался бы с чужим титром на событии.
pub fn apply_overrides(
    published: &[PublishedParam],
    overrides: &[Override],
    clips: &mut [ClipEvent],
    transitions: &mut [TransitionEvent],
    cameras: &mut [CameraEvent],
    frame_step: i64,
    default_gen: &dyn Fn(MediaId) -> Option<GeneratorKind>,
) {
    // Порядок — ПО СПИСКУ АВТОРА, а не по порядку переопределений:
    // набор у экземпляра приходит из файла и из буфера обмена, и его
    // порядок правилом рендера быть не может.
    for p in published {
        let Some(ov) = overrides.iter().find(|o| o.pub_id == p.id) else { continue };
        apply_one(p, ov, clips, transitions, cameras, frame_step, default_gen);
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
        // сужение пределов — часть ОБЪЯВЛЕНИЯ, а не украшение окна:
        // испорченный файл не имеет права увести титр за кадр
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
        ParamOwner::ClipAttr => set_clip_attr(c, name, &value, default_gen),
        ParamOwner::Nested { pub_id } => {
            // Патчим НАБОР внутреннего экземпляра — дальше он разберётся
            // сам, своим же кодом. Кривая при этом остаётся в материале
            // внутреннего экземпляра: его материал — время его ленты, а
            // мы стоим ровно в нём.
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

/// Может ли этот набор переопределений изменить СОСТОЯНИЕ ИСТОЧНИКА
/// внутри ленты (Р.59, стадия 3).
///
/// Зачем это нужно и почему это не мелочь. С Р.53 адрес источника
/// генератора — `GEN_TAG | id СОБЫТИЯ`, и до этой стадии он однозначно
/// определял ЗНАЧЕНИЯ: событие одно, значения одни. Переопределение
/// ломает посылку — одно и то же событие ВНУТРИ ленты играет в разных
/// экземплярах разным текстом. Значит у таких экземпляров и адрес
/// источника обязан быть разным, иначе «второй выпуск с другим
/// заголовком» молча покажет первый: заказ и чтение сойдутся на одном
/// адресе (инвариант 4 соблюдён), но кадр будет ЧУЖОЙ.
///
/// Ответ «нет» обязан быть точным, а не оптимистичным: соль равна нулю
/// ровно тогда, когда ни одна ручка набора не способна дотянуться до
/// источника, — и тогда адреса остаются побайтово прежними. Сюда входят
/// `Generator` (значения прямо), `ClipAttr media` (событие может стать
/// генератором другого вида) и `Nested` (шаблон в шаблоне — набор едет
/// внутрь).
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

/// Имена свойств самого события — ЕДИНСТВЕННЫЙ их список.
pub const CLIP_ATTRS: &[(&str, ParamType)] = &[
    ("opacity", ParamType::Num),
    ("gain_db", ParamType::Num),
    ("media", ParamType::Media),
    ("reversed", ParamType::Flag),
];

pub fn clip_attr_type(name: &str) -> Option<ParamType> {
    CLIP_ATTRS.iter().find(|(n, _)| *n == name).map(|(_, t)| t.clone())
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
            // Тем же правилом, что `fit_gen_to_media`: стал файлом —
            // значений генератора быть не может; стал генератором другого
            // вида — значения берутся из пути.
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
