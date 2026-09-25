//! Типы формата проекта. Источник правды по схеме `.ktproj` v1.

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

/// Идентификатор камеры-события (Р.29). serde(transparent) — в файле
/// просто число, как MediaId.
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

/// Минимальная суммарная ширина окна перехода (1 мс, как crate::MIN_CLIP_DURATION).
pub const MIN_TRANSITION_DURATION: Tick = Tick(90);

/// Направление жеста перехода. Одно поле вместо четырёх видов: ядро у
/// «влево/вправо/вверх/вниз» одно, а пресеты в UI разные.
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
    /// Единичный вектор направления в координатах кадра (X вправо, Y вниз).
    pub fn vec(self) -> (f64, f64) {
        match self {
            Dir::Left => (-1.0, 0.0),
            Dir::Right => (1.0, 0.0),
            Dir::Up => (0.0, -1.0),
            Dir::Down => (0.0, 1.0),
        }
    }
}

/// Вид перехода (Р.33 п.1: «переходы хотелось бы разные сделать, накидай
/// там разных 10-20 современных, управление ими надо унифицировать как
/// fx-chain»). Устроен как `FxInstance`: тег + поля-параметры, спеки
/// параметров общие с эффектами — поэтому переход получает и единый UI
/// полей, и, когда дойдут руки, автоматизацию тем же механизмом.
///
/// `Crossfade` — умолчание и ровно прежний путь рендера: файлы без
/// параметров байт-стабильны, а GOLDEN не двигается.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransitionKind {
    /// Линейный dissolve (видео) + кроссфейд (аудио).
    #[default]
    Crossfade,
    /// Край едет по кадру: линейный вайп, `angle` даёт диагональ.
    Wipe {
        #[serde(default)]
        dir: Dir,
        #[serde(default)]
        angle: f64,
        #[serde(default = "default_soft")]
        softness: f64,
    },
    /// Раскрытие из центра: круг/ромб/прямоугольник. Центр (Р.51) —
    /// доли кадра, умолчание середина; при 0.5/0.5 картинка байт-та же.
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
    /// Стрелка часов вокруг центра.
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
    /// Шторки: две створки расходятся от центра.
    Barn {
        #[serde(default)]
        vertical: bool,
        #[serde(default = "default_soft")]
        softness: f64,
    },
    /// Вайп по ЯРКОСТИ уходящего кадра — «проявление» по картинке.
    Luma {
        #[serde(default)]
        invert: bool,
        #[serde(default = "default_soft")]
        softness: f64,
    },
    /// Входящий выталкивает уходящий за край.
    Push {
        #[serde(default)]
        dir: Dir,
    },
    /// Входящий наезжает поверх, уходящий стоит.
    Slide {
        #[serde(default)]
        dir: Dir,
    },
    /// Наплыв с приближением: уходящий разъезжается, входящий налетает.
    Zoom {
        #[serde(default = "default_zoom_amount")]
        amount: f64,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cx: f64,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cy: f64,
    },
    /// Поворот с наплывом.
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
    /// Резкий пролёт камерой со смазом.
    Whip {
        #[serde(default)]
        dir: Dir,
        #[serde(default = "default_whip_blur")]
        blur: f64,
    },
    /// Растворение через размытие обоих кадров.
    BlurDissolve {
        #[serde(default = "default_blur_amount")]
        amount: f64,
    },
    /// Растворение через укрупнение пикселей.
    Pixelate {
        #[serde(default = "default_pixelate")]
        amount: f64,
    },
    /// Цифровой сбой: сдвиги строк и разъезд каналов.
    Glitch {
        #[serde(default = "default_glitch")]
        amount: f64,
    },
    /// Засветка: кадр уходит в цвет и возвращается.
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
    /// Через цвет: уходящий гаснет в цвет, из него проявляется входящий.
    Dip {
        #[serde(default)]
        color_r: u8,
        #[serde(default)]
        color_g: u8,
        #[serde(default)]
        color_b: u8,
    },
    // ---- узоры (Р.51, п.400) -------------------------------------------
    /// Жалюзи: полосы открываются веером, угол даёт диагональ.
    Blinds {
        #[serde(default = "default_count8")]
        count: u32,
        #[serde(default)]
        angle: f64,
        #[serde(default = "default_soft_thin")]
        softness: f64,
    },
    /// Шахматы: клетки открываются двумя встречными волнами.
    Checker {
        #[serde(default = "default_count8")]
        cols: u32,
        #[serde(default = "default_count6")]
        rows: u32,
        #[serde(default = "default_soft_thin")]
        softness: f64,
    },
    /// Случайные блоки: клетки вспыхивают в псевдослучайном порядке.
    Blocks {
        #[serde(default = "default_count16")]
        cols: u32,
        #[serde(default = "default_count9")]
        rows: u32,
        #[serde(default = "default_blocks_soft")]
        softness: f64,
    },
    /// Спираль: архимедов фронт закручивается вокруг центра.
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
    // ---- сдвиг (симбиоз: движутся ОБА кадра) ----------------------------
    /// Разделение: половинки уходящего разъезжаются, входящий открывается.
    Split {
        #[serde(default)]
        vertical: bool,
    },
    /// Сжатие: уходящий сжимается к дальнему краю, входящий расправляется.
    Squeeze {
        #[serde(default)]
        dir: Dir,
    },
    /// Растяжение: входящий растягивается из центральной линии.
    Stretch {
        #[serde(default)]
        vertical: bool,
    },
    /// Обмен: кадры меняются местами по дуге, порядок сторон
    /// переключается на середине (2.5D без глубины).
    Swap {
        #[serde(default)]
        vertical: bool,
        #[serde(default = "default_swap_depth")]
        depth: f64,
    },
    // ---- стилизованные --------------------------------------------------
    /// Тряска: рывки кадра, смена стороны на середине.
    Shake {
        #[serde(default = "default_glitch")]
        amount: f64,
        #[serde(default = "default_shake_steps")]
        steps: u32,
    },
    /// Прожог плёнки: фронт по крупному шуму с горячей кромкой.
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
    /// Блики света: цветные пятна пролетают по кадру поверх растворения.
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
    /// Радиальный смаз: лучи от центра, смена стороны на середине.
    ZoomBlur {
        #[serde(default = "default_whip_blur")]
        amount: f64,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cx: f64,
        #[serde(default = "default_center", skip_serializing_if = "is_center")]
        cy: f64,
    },
    // ---- объёмные (Р.51, п.401): панели и цилиндры, окклюзия
    // аналитическая — глубинного буфера в композите нет и не надо -------
    /// Переворот: панель крутится вокруг своей середины, на обороте
    /// видна изнанка с входящим кадром.
    Flip {
        #[serde(default)]
        vertical: bool,
        #[serde(default = "default_persp")]
        persp: f64,
        #[serde(default = "default_shade")]
        shade: f64,
    },
    /// Куб: уходящий и входящий — две грани, куб поворачивается.
    Cube {
        #[serde(default)]
        dir: Dir,
        #[serde(default = "default_persp")]
        persp: f64,
        #[serde(default = "default_shade")]
        shade: f64,
    },
    /// Двери: две створки уходящего распахиваются вглубь, за ними входящий.
    Door {
        #[serde(default)]
        vertical: bool,
        #[serde(default = "default_persp")]
        persp: f64,
        #[serde(default = "default_shade")]
        shade: f64,
    },
    /// Перелистывание: лист заворачивается назад цилиндром, виден его
    /// изнаночный хвост; под листом открывается входящий.
    PagePeel {
        #[serde(default)]
        dir: Dir,
        #[serde(default = "default_curl")]
        curl: f64,
        #[serde(default = "default_shade")]
        shadow: f64,
    },
    /// Свиток: лист наматывается на рулон, виден верхний виток.
    PageRoll {
        #[serde(default)]
        dir: Dir,
        #[serde(default = "default_curl")]
        curl: f64,
        #[serde(default = "default_shade")]
        shadow: f64,
    },
}

/// Форма раскрытия у Iris.
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

/// Общий интерфейс «числовые параметры со спеками». Его реализуют и
/// эффекты, и виды переходов — поэтому UI полей у них ОДИН (требование
/// Р.33 п.1: «управление ими надо унифицировать как fx-chain»), и любой
/// новый носитель параметров получает готовую сетку полей даром.
/// Паспорт ФЛАГА — параметра, у которого нет диапазона (Р.53, п.414).
///
/// Числа описывает `ParamSpec`, а «да/нет» не число: его нельзя тянуть
/// ползунком и нельзя автоматизировать кривой. До этого пункта такие
/// поля рисовались рукописными строками в каждом окне — и ровно поэтому
/// забывались: тон 1 кГц у SMPTE и цветной режим шума существовали в
/// формате, но включить их было НЕЧЕМ (класс t252 «осиротевшая фича»).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagSpec {
    /// Имя поля snake_case — оно же ключ в файле.
    pub name: &'static str,
    pub default: bool,
}

/// Кому принадлежит параметр (Р.53, п.414). Одно и то же ИМЯ значит у
/// разных носителей разное: `size` — зерно у эффекта Noise и КЕГЛЬ у
/// титра, `amount` — сила у полудюжины эффектов. Глобальная карта «имя →
/// подпись» на этом и врала: кегль титра был подписан «Зерно:».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamScope {
    Fx,
    Generator,
}

pub trait HasParams {
    /// Паспорт свойств. Заимствуется у `self` (а не `&'static`), потому
    /// что у динамических владельцев — публичного параметра композиции и
    /// плагина — он строится в рантайме (Р.59, п.466).
    fn param_specs(&self) -> &[ParamSpec];
    fn get_param(&self, name: &str) -> Option<f64>;
    fn set_param(&mut self, name: &str, v: f64) -> bool;

    /// Чтение по ИНДЕКСУ ряда паспорта. Умолчание — через имя (медленно,
    /// зато верно для любого владельца); встроенные виды генерят прямой
    /// доступ макросом.
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

    /// Чьи это параметры — подпись ищется сперва в своей области
    /// (Р.53, п.414).
    fn params_scope(&self) -> ParamScope {
        ParamScope::Fx
    }

    /// Флаги вида (Р.53, п.414). Пусто — их нет; тогда окно не рисует
    /// ни одного чекбокса, и это законно.
    fn flag_specs(&self) -> &'static [FlagSpec] {
        &[]
    }
    fn get_flag(&self, _name: &str) -> Option<bool> {
        None
    }
    fn set_flag(&mut self, _name: &str, _v: bool) -> bool {
        false
    }

    /// Спека параметра по имени (кривая-сирота видна сразу).
    fn param_spec(&self, name: &str) -> Option<&ParamSpec> {
        self.param_specs().iter().find(|s| s.name == name)
    }

    /// Индекс ряда по имени — дверь из именного мира (файл, UI) в
    /// индексный (компилированный план автоматизации).
    fn param_idx(&self, name: &str) -> Option<usize> {
        self.param_specs().iter().position(|s| s.name == name)
    }

    /// Свойства, которых НЕТ среди числовых рядов и флагов: текст, выбор,
    /// ссылка на медиа. Пусто у подавляющего большинства владельцев —
    /// объявляет их только тот, у кого они есть (титр — свой текст и
    /// семью шрифта).
    fn extra_specs(&self) -> Vec<ParamSpec> {
        Vec::new()
    }

    /// Цветовые группы — ВЫВОДЯТСЯ из паспорта (Р.59, стадия 3), а не
    /// объявляются: ручной список был бы вторым источником того же факта.
    fn color_groups(&self) -> Vec<ColorGroup> {
        derive_color_groups(self.param_specs())
    }

    /// Тип свойства по имени. `None` — такого свойства у владельца нет,
    /// и это ровно то, чем валидация ловит публикацию на исчезнувшую цель.
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

    /// ПАСПОРТ свойства по имени — зеркало `value_type` (Р.60).
    ///
    /// Единственный источник пределов, шага и умолчания для публикации и
    /// для окна потребителя. Копии паспорта в публикации не будет по той
    /// же причине, по которой не хранится значение шаблона: она
    /// разошлась бы с целью при первой правке внутри ленты. Порядок
    /// веток обязан совпадать с `value_type` — разойдутся, и у цвета
    /// паспорт окажется от компоненты, а тип от группы.
    fn value_spec(&self, name: &str) -> Option<ParamSpec> {
        if self.color_groups().iter().any(|g| g.stem == name) {
            // пределы цвета берутся у компоненты `_r`: она и есть шкала
            // хранения (Rgb8), а не выдумка интерфейса
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

    /// ТИПИЗИРОВАННОЕ чтение. Умолчание разводит имя по трём существующим
    /// путям (число, флаг, компоненты цвета); владельцы с текстом и
    /// выбором переопределяют.
    fn get_value(&self, name: &str) -> Option<ParamValue> {
        default_get_value(self, name)
    }

    /// ТИПИЗИРОВАННАЯ запись. Возвращает false, если такого свойства нет
    /// или тип не тот: молчаливая подмена типа была бы порчей.
    fn set_value(&mut self, name: &str, v: &ParamValue) -> bool {
        default_set_value(self, name, v)
    }
}

/// Тело умолчания `HasParams::get_value` СВОБОДНОЙ функцией: переопределяя
/// метод, владелец обязан уметь позвать умолчание, а из перекрытого метода
/// трейта оно недоступно.
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

/// Тело умолчания `HasParams::set_value`.
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

/// Спека текстового свойства (паспорт строится в рантайме — Р.59).
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

/// Спека свойства-выбора; варианты — КЛЮЧИ, а не номера.
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
    /// Все виды с УМОЛЧАНИЯМИ параметров — единственный реестр: по нему
    /// строится меню выбора в UI и по нему же тесты проверяют полноту
    /// спек. Новый вид, не попавший сюда, тестом виден сразу.
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

    /// Тег вида, как он лежит в файле (для UI и сравнения).
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

    /// Симметричные виды: `kernel(b, a, 1−k)` для них тождественен
    /// `kernel(a, b, k)`, инверсия ничего не меняет — UI прячет галку
    /// (правило 16: неработающий контрол не показывается). Match
    /// исчерпывающий намеренно: новый вид обязан решить, симметричен ли.
    pub fn symmetric(&self) -> bool {
        match self {
            // у zoom обе стороны считаются той же формулой от своей роли,
            // и mix-тождество `mix(y, x, 1−t) == mix(x, y, t)` делает
            // инверсию точным тождеством (у spin — нет: знак поворота
            // при инверсии меняется, и это осмысленно)
            TransitionKind::Crossfade
            | TransitionKind::Zoom { .. }
            | TransitionKind::BlurDissolve { .. }
            | TransitionKind::Pixelate { .. }
            | TransitionKind::Glitch { .. }
            | TransitionKind::Flash { .. }
            | TransitionKind::Dip { .. }
            // сила лучей sin(πk) и смена стороны на середине симметричны
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

    /// Числовые параметры вида — та же спека, что у эффектов, поэтому UI
    /// рисует их тем же кодом.
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

/// Кривая аудио-кроссфейда (Р.33 п.1, решение пользователя: у
/// «вайпов/глитчей» звукового смысла нет, поэтому звук всегда
/// кроссфейдится, но кривую можно выбрать). Умолчание — equal-power:
/// ровно прежняя формула, GOLDEN_AUDIO не двигается.
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
    /// Вес НАРАСТАЮЩЕЙ стороны в позиции t ∈ [0,1]. Затухающая сторона
    /// считается тем же кодом от (1 − t) — иначе половинки разъезжаются.
    pub fn rise(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            AudioCurve::EqualPower => t.sqrt(),
            AudioCurve::Linear => t,
            // S-кривая: мягкий старт и мягкий конец, сумма мощностей ровнее
            AudioCurve::SCurve => (t * t * (3.0 - 2.0 * t)).sqrt(),
            // экспонента: долгий тихий вход, резкий подъём в конце
            AudioCurve::Exponential => t * t,
        }
    }
}

/// Переход на стыке двух СОСЕДНИХ клипов одной дорожки (Р.24, OTIO
/// Transition). Позиция НЕ хранится — выводится из клипов: стык == конец `a`
/// == начало `b`; идентичность по паре ClipId переживает групповые переносы
/// (стык уехал у обоих — переход жив). Окно: [стык − in_offset,
/// стык + out_offset); in_offset лежит в теле A (incoming играет левую
/// «ручку» источника), out_offset — в теле B (outgoing играет правую).
/// Материал ручек — чтение источника за пределами использованного куска;
/// за краями материала движок отдаёт тишину/кламп-кадр (существующая
/// политика, ничего не сериализуется).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionEvent {
    pub id: TransitionId,
    /// Уходящий клип (его конец — стык). `None` — фейд-ин: слева пустота
    /// (край дорожки или Gap).
    pub a: Option<ClipId>,
    /// Входящий клип (его начало — стык). `None` — фейд-аут: справа пустота.
    pub b: Option<ClipId>,
    pub in_offset: Tick,
    pub out_offset: Tick,
    #[serde(default)]
    pub kind: TransitionKind,
    /// Кривая аудио-кроссфейда (Р.33). Умолчание equal-power не
    /// сериализуется — файлы прежних версий байт-стабильны.
    #[serde(default, skip_serializing_if = "is_default_curve")]
    pub audio: AudioCurve,
    /// Кривые ПАРАМЕТРОВ вида (Р.34 К4) — тот же механизм, что у
    /// эффектов и камер. Позиция ключа — тик от НАЧАЛА окна перехода
    /// (`joint − in_offset`): правка края окна открывает/прячет ключи,
    /// как обрезка клипа, а не растягивает анимацию.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto: Vec<ParamCurve>,
    /// Инверсия перехода (Р.51, п.398): жест играется задом наперёд —
    /// исполнители меняют стороны местами и обращают прогресс
    /// (`kernel(b, a, 1−k)`), сам переход по-прежнему идёт из A в B.
    /// Частные флаги видов (`Clock.reverse` — направление стрелки,
    /// `Luma.invert` — порог яркости) — ДРУГИЕ оси, они остаются.
    /// Умолчание не сериализуется — файлы прежних версий байт-стабильны.
    #[serde(default, skip_serializing_if = "is_false")]
    pub invert: bool,
}

fn is_default_curve(c: &AudioCurve) -> bool {
    *c == AudioCurve::EqualPower
}

impl TransitionEvent {
    /// Вид с УЧЁТОМ кривых в позиции `local` (тик от начала окна).
    /// Единственная точка разрешения — её зовут оба исполнителя рендера.
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

    /// Стык: конец `a`, а для фейд-ина — начало `b` (Р.25). None — оба конца
    /// пропали; такое состояние живёт только внутри мутации до
    /// reconcile_transitions.
    pub fn joint(&self, p: &dyn Reel) -> Option<Tick> {
        if let Some(a) = self.a.and_then(|id| p.clips().iter().find(|c| c.id == id)) {
            return Some(a.start + a.duration);
        }
        let b = self.b.and_then(|id| p.clips().iter().find(|c| c.id == id))?;
        Some(b.start)
    }

    /// Фейд — переход с ОДНИМ живым концом: с той стороны, где `None`,
    /// пустота (край дорожки или Gap), и слой уходит в прозрачность/тишину.
    /// Эталонная библиотека трактует такой переход так же (dissolve с Gap).
    pub fn is_fade(&self) -> bool {
        self.a.is_none() || self.b.is_none()
    }

    /// Клип, к телу которого прилегает переход с обеих сторон окна.
    pub fn touches(&self, id: ClipId) -> bool {
        self.a == Some(id) || self.b == Some(id)
    }

    /// Пара НАЛОЖЕННОГО перехода (Р.51, п.399): оба конца живы, одна
    /// (kind, lane) и перекрытие «лесенкой» — a начался раньше и кончился
    /// внутри b. Полное вложение и равные старты парой не считаются:
    /// у такого расположения нет ни направления, ни осмысленного окна.
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

    /// Полуоткрытое окно перехода на таймлайне. У стыкового — офсеты от
    /// стыка; у наложенного (Р.51) окно — САМА зона перекрытия: режим не
    /// хранится, окно следует за тримом клипов, собственных ручек у него
    /// нет (урок Kdenlive из F-TL-010 — второй подсистемы окна не заводим).
    /// `in/out_offset` в режиме наложения не используются (лежат мёртвым
    /// грузом и оживают при возврате пары встык).
    pub fn window(&self, p: &dyn Reel) -> Option<TickRange> {
        if let Some((a, b)) = self.overlap_pair(p) {
            return Some(TickRange::new(b.start, a.start + a.duration));
        }
        let at = self.joint(p)?;
        Some(TickRange::new(at - self.in_offset, at + self.out_offset))
    }
}

/// ЛЕНТА МОНТАЖА: события и переходы между ними (Р.50, п.378).
///
/// Реализуют и корень проекта, и вложенная секвенция. Трейт существует
/// ради одной вещи: вся геометрия переходов (`joint`, `window`,
/// `check_transition_full`, `forbidden_zones`) обязана спрашивать ТУ
/// ленту, на которой переход стоит. Переход, чьи концы лежат в секвенции,
/// а сам он ищется среди корневых клипов, вернул бы `None` — и
/// `reconcile_transitions` убил бы его на первой же команде.
///
/// Взят трейт, а не структура-вид: тогда 29 существующих вызовов вида
/// `t.joint(project)` продолжают компилироваться дословно, а новый путь
/// просто передаёт `&Sequence`.
pub trait Reel {
    fn clips(&self) -> &[ClipEvent];
    fn transitions(&self) -> &[TransitionEvent];
    /// Строки сверху вниз. Единственный способ узнать, что «ниже», и —
    /// с Р.56 — то, чему обязан соответствовать НОМЕР дорожки.
    fn rows(&self) -> Vec<(LaneKind, u32)>;
    /// Дорожки-объекты (Р.62).
    fn tracks(&self) -> &[Track];
    /// Остальное содержимое ленты. Умолчания — пустота: реализующий
    /// обязан переопределить только то, что у него есть.
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
    /// Объявленные наружу ручки (Р.59, стадия 3). У корня пусто по
    /// смыслу: корень никому не экземпляр.
    fn published(&self) -> &[PublishedParam] {
        &[]
    }
}

/// Зоны, куда не встаёт граница морфер-слоя: окна переходов этого клипа
/// и наезды соседей по дорожке. Резка там ломает не цель, а ОКРУЖЕНИЕ (Р.37).
///
/// Свободная функция, а не метод трейта: `Reel` обязан оставаться
/// объектно-безопасным (обход дерева лент ходит по `&dyn Reel`), а тело
/// зовёт `t.joint(reel)` — коэрцию `&Self -> &dyn Reel` внутри
/// провайд-метода сделать нечем.
pub fn forbidden_zones(reel: &dyn Reel, c: &ClipEvent) -> Vec<TickRange> {
    let body = TickRange::new(c.start, c.start + c.duration);
    let mut out = Vec::new();
    for t in reel.transitions() {
        if t.a != Some(c.id) && t.b != Some(c.id) {
            continue;
        }
        // у наложенного перехода (Р.51) окно — зона перекрытия
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

// ---- РАСКРЫТИЕ ЛЕНТЫ НА МЕСТЕ (Р.55, С5) -----------------------------------

/// Сколько дорожек каждого вида. Служит и «где начинается полоса», и
/// «сколько её».
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

/// ПОЛОСА РАСКРЫТИЯ: лента, разложенная на дорожки корня ПРЯМО ПОД своим
/// экземпляром (Р.55, С5).
///
/// Приём тот же, что у `swap_reel` (п.392), и по той же причине: контекст
/// правки делается ПЕРЕКЛАДЫВАНИЕМ СОДЕРЖИМОГО, а не параметром у
/// шестидесяти рук. Только здесь перекладывание идёт СО СМЕЩЕНИЕМ — по
/// времени и по номеру дорожки:
///
/// ```text
/// t_корня = t_ленты + dt,   dt = inst.start − inst.source_in
/// дорожка = своя + base[вид], base[вид] = сколько дорожек этого вида у корня
/// ```
///
/// Что из этого следует и ради чего всё затевалось: **отрисовка, хит-тест,
/// снап, магнит, рамка, выделение, `rows()`, `MovePlan`, `forbidden_zones`
/// и `reconcile_transitions` не меняются ни на строку**. События полосы
/// физически лежат в корневых векторах, поэтому `TrimStart{id}` приходит
/// на них обычным путём, и «пропустил одно место — правка молча уехала в
/// корень» стать не может: уехать физически нечем, а нарушение ловит
/// `unmerge_reel` и отвергает весь батч.
///
/// Полоса занимает ХВОСТ числового пространства дорожек, а видимое место
/// под экземпляром задаётся только через `lane_order`. Тогда корневые
/// номера все `< base`, номера полосы все `>= base`, и вставка/удаление
/// дорожки в корне не трогает полосу — по построению, а не по проверке.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Band {
    /// Через какой экземпляр раскрыто.
    pub inst: ClipId,
    /// Какая лента разложена.
    pub seq: MediaId,
    /// Сдвиг по времени: `t_корня = t_ленты + dt`.
    pub dt: Tick,
    /// С какого номера начинаются дорожки полосы.
    pub base: LaneBases,
    /// Сколько их.
    pub len: LaneBases,
}

impl Band {
    /// Полоса для экземпляра `inst`. `None` — это не вложение.
    ///
    /// `dt` инвариантен под тримом: `TrimStart`/`TrimEnd` двигают `start` и
    /// `source_in` на ОДНУ дельту (`clip_slice`), поэтому при перетаскивании
    /// края экземпляра содержимое полосы не едет по экрану — окно
    /// открывается над неподвижным материалом. Ровно этот жест владелец и
    /// просил: «тянуть край можно прямо здесь».
    pub fn of(p: &Project, inst: ClipId) -> Option<Band> {
        let c = p.clips.iter().find(|c| c.id == inst)?;
        let s = p.sequence(c.media)?;
        // Блок дорожек ленты встаёт СРАЗУ ПОД строкой экземпляра (Р.62,
        // change B: дорожки — объекты, номер = ранг вектора). Значит
        // первая дорожка полосы каждого вида получает ранг «сколько
        // строк этого вида стоит не ниже экземпляра».
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

    /// Окно экземпляра в координатах КОРНЯ: что из полосы видно, а что —
    /// хвост за обрезкой.
    pub fn window(&self, p: &Project) -> Option<TickRange> {
        let c = p.clips.iter().find(|c| c.id == self.inst)?;
        Some(TickRange::new(c.start, c.start + c.duration))
    }

    /// Принадлежит ли дорожка полосе.
    pub fn owns(&self, kind: LaneKind, lane: u32) -> bool {
        let b = self.base.get(kind);
        lane >= b && lane < b + self.len.get(kind)
    }

    /// Строки полосы в номерах КОРНЯ, сверху вниз по видам.
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
    /// Разложить ленту экземпляра на дорожки корня СО СМЕЩЕНИЕМ (Р.55, С5).
    ///
    /// Тело ленты ОПУСТОШАЕТСЯ: события переехали, а не скопировались —
    /// поэтому `max_clip_id` и родня продолжают видеть каждый адрес ровно
    /// один раз (п.394), и `unmerge_reel` возвращает всё на место.
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
        // корневое содержимое ниже блока полосы сдвигается на его длину:
        // номер = ранг, а блок встаёт между
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
        // СТРОКИ ПОЛОСЫ встают неразрывным блоком СРАЗУ ПОД строкой
        // экземпляра
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

    /// Собрать ленту обратно — С ДОКАЗАТЕЛЬСТВОМ, что никто не сменил полосу.
    ///
    /// Ошибка здесь — не мелочь, а сторож всей конструкции: она означает,
    /// что жест вынес событие через границу лент, и батч обязан быть
    /// отвергнут ЦЕЛИКОМ, а не «доправлен». Именно поэтому адресация
    /// правок не нужна: уехать физически нечем, а попытку ловит вот это
    /// место.
    pub fn unmerge_reel(&mut self, b: &Band) -> Result<(), ProjectError> {
        if !self.sequences.iter().any(|s| s.id == b.seq) {
            return Ok(()); // ленты уже нет (undo) — разбирать нечего
        }
        let mine_clip = |c: &ClipEvent| b.owns(lane_key(c).0, c.lane);
        let (mut inner, root): (Vec<ClipEvent>, Vec<ClipEvent>) =
            std::mem::take(&mut self.clips).into_iter().partition(mine_clip);
        self.clips = root;
        // ЭКЗЕМПЛЯР обязан остаться в корне: утащи его внутрь — лента
        // стала бы содержать саму себя
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
        // ПЕРЕХОД целиком либо в полосе, либо в корне: конец по одну
        // сторону границы — это переход через границу лент, которого в
        // модели не бывает (Р.50, t425)
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
        // корневое содержимое ниже блока возвращается на свои номера
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
        // ДОРОЖКИ полосы возвращаются ленте в порядке строк; у корня
        // остаётся его вектор без блока
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

/// Старший бит адреса медиа отдан вложенным лентам (Р.50).
///
/// Движку нужен ОДИН `SourceId` на ленту — ключ кэша, ключ мемоизации
/// пост-FX, адрес в перф-панели. Он берётся как `NEST_TAG | seq`, поэтому
/// адрес настоящего файла обязан лежать ниже: иначе файл и секвенция
/// молча поделили бы кэш. Проверяется в `validate`.
pub const NEST_TAG: u64 = 1 << 63;

/// Следующий бит адреса медиа отдан ГЕНЕРАТОРУ НА СОБЫТИИ (Р.59, стадия 2).
///
/// Значения генератора живут на клипе, значит и источник у каждого клипа
/// СВОЙ: адрес берётся как `GEN_TAG | id` события. Адрес считается от
/// СОБЫТИЯ, а не от значений — он входит в план, и хэш значений в нём
/// означал бы пересборку плана на каждое нажатие клавиши (ровно то, что
/// владелец назвал «ужасно» в Р.53). Форма заказа `(адрес, тик,
/// качество)` не меняется, поэтому «заказ == чтение», полоски
/// прогретости, префетч и экспорт-чанки работают без правок.
///
/// Тег обязан лежать НИЖЕ `NEST_TAG` и ВЫШЕ любого id клипа — включая
/// синтетические id кусков морфер-слоя (`slice_id`). Это и держит
/// `const _` ниже.
pub const GEN_TAG: u64 = 1 << 62;

/// Следующий бит адреса медиа отдан СУБ-КОМПОЗИТУ ДОРОЖКИ (Р.62, change
/// B). Дорожка с цепочкой, уровнем ≠ 1 или режимом наложения ≠ Normal
/// входит в план ОДНИМ слоем-вложением, и её суб-композиту нужен свой
/// `SourceId` — ключ мемоизации и ключ GPU-плана. Настоящей ленты у неё
/// нет, поэтому адрес синтетический: `NEST_TAG | TRACK_TAG | id дорожки`.
///
/// Бит лежит НИЖЕ `NEST_TAG` и выше любого адреса ленты: адреса лент —
/// это `MediaId` документа, они меньше `TRACK_TAG` по построению
/// (`validate` отвергает `MediaId >= NEST_TAG`, а id дорожек растут с 1).
pub const TRACK_TAG: u64 = 1 << 61;

/// Адрес суб-композита дорожки в пространстве медиа.
/// КЛЮЧИ САЙДЧЕЙНА цепочки (Р.65): у каких приборов назначен второй вход.
/// Один дом ответа «что такое ключ в цепочке» — его спрашивают и проверка
/// команды, и движок, и формат.
pub fn sidechain_keys(fx: &[FxInstance]) -> impl Iterator<Item = TrackId> + '_ {
    fx.iter().filter_map(|f| match f {
        FxInstance::Compressor { key, .. } => *key,
        _ => None,
    })
}

/// Замкнётся ли граф ключей, если у дорожки `target` встанет цепочка `fx`.
///
/// Цикл невозможно исполнить: кадр дорожки зависел бы сам от себя. Поэтому
/// он отвергается ПРИ ЗАПИСИ (движок страхуется своим обрывом рекурсии).
pub fn sidechain_cycle(tracks: &[Track], target: TrackId, fx: &[FxInstance]) -> bool {
    // обход от каждого ключа предлагаемой цепочки: дошли до target — цикл
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

/// Предел глубины вложенности (Р.50, п.378).
///
/// Число ПОСЧИТАНО, а не выбрано круглым: в момент композита уровня в
/// памяти видеокарты живы два аккумулятора этого уровня плюс по
/// аккумулятору и результату на каждого предка — около 25 МиБ на уровень
/// при 1080p, при бюджете временных текстур 128 МиБ (kt-gpu). Предел
/// стоит и здесь (жёстко, с именами лент в тексте ошибки), и в
/// исполнителе (аккуратный `TooLarge` с откатом на CPU).
pub const MAX_NEST_DEPTH: usize = 4;

/// Вложенная секвенция — лента, на которую ССЫЛАЮТСЯ (Р.50, п.378).
///
/// Своих настроек у неё НЕТ — ни fps, ни растра, ни частоты: вложение
/// наследует родителя (прямое решение владельца). Из этого следует
/// главное техническое свойство раунда: промежуточного растра не
/// возникает, поэтому **вложение с пустой цепочкой тождественно своему
/// развёрнутому содержимому**, и раскрытие в `effective_clips` служит
/// эталоном-оракулом для настоящего поддерева плана.
///
/// Адрес — `MediaId` в ОДНОМ пространстве с медиа: `ClipEvent.media`
/// указывает либо на файл (`Project.media`), либо на ленту
/// (`Project.sequences`). `MediaRef`-а у секвенции НЕТ намеренно — иначе
/// `build_player` поднял бы на неё `MediaWorker`, watchdog покрасил бы её
/// тинтом офлайна, а служба индекса полезла бы строить пики ffmpeg'ом.
/// Перечисленные пути перебирают `project.media`, где секвенций нет, и
/// остаются правильными ПО ПОСТРОЕНИЮ.
///
/// Имя — подпись, а НЕ адрес (в отличие от `MarkedArea`): переименование
/// не имеет шанса порвать ссылку, а дедупликация тел при записи в .otio
/// опирается на устойчивый ключ, переживающий переименование.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sequence {
    pub id: MediaId,
    /// Подпись в бине и `Stack.name` в файле. Непустая (иначе в бине
    /// карточка без подписи), но НЕ обязана быть уникальной.
    pub name: String,
    pub clips: Vec<ClipEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<TransitionEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub morphers: Vec<MorpherEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<CameraEvent>,
    /// Дорожки ленты — те же объекты, что у корня (Р.62).
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
    /// Объявленные наружу ручки ленты (Р.59, стадия 3). Пустой список не
    /// сериализуется — лента без публикаций ведёт себя и пишется как
    /// сегодня, байт в байт.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub published: Vec<PublishedParam>,
    /// СВОЙ растр ленты (Р.62, пересмотр Р.50 по просьбе владельца).
    /// `None` — проект. Экземпляр приводится в кадр родителя тем же
    /// правилом, что файл, с видом = растр ленты.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raster: Option<(u32, u32)>,
    /// СВОЙ fps ленты: квантование её непрерывного времени к своей сетке
    /// (кадр держится, без смешения). `None` — проект. На звук не влияет.
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
            // id назначает `Project::assign_track_ids`: у ленты нет доступа к
            // пространству id документа
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

    /// Дописать дорожки вида до `n` штук (в конец строк).
    pub fn ensure_tracks(&mut self, kind: LaneKind, n: u32) {
        while self.lanes(kind) < n {
            let at = natural_insert_pos(&self.tracks, kind);
            self.tracks.insert(at, Track::new(TrackId(0), kind));
        }
    }

    /// Легаси-счётчики и порядок ленты → вектор дорожек (односторонне).
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

    /// Сколько дорожек каждого вида ТРЕБУЕТ содержимое.
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

    /// Свои настройки законны? Растр — чётный в пределах 16…8192, fps —
    /// делитель 90 000 в пределах 1…120 (та же проверка, что у проекта).
    pub fn settings_ok(&self) -> bool {
        let raster_ok = self.raster.is_none_or(|(w, h)| {
            [w, h].iter().all(|v| (16..=8192).contains(v) && v.is_multiple_of(2))
        });
        let fps_ok =
            self.fps.is_none_or(|f| (1..=120).contains(&f) && kt_time::TICK_HZ % f as i64 == 0);
        raster_ok && fps_ok
    }

    /// Строки ленты сверху вниз — то же правило, что у корня.
    pub fn rows(&self) -> Vec<(LaneKind, u32)> {
        rows_of_tracks(&self.tracks)
    }

    /// Конец содержимого ленты.
    pub fn end(&self) -> Tick {
        self.clips.iter().map(|c| c.start + c.duration).max().unwrap_or(Tick::ZERO)
    }

    /// Дорожки ленты из легаси-полей (счётчики, `lane_order`) — на ЧТЕНИИ,
    /// односторонне; затем дописать до содержимого. Имена служебных строк
    /// ленты старый формат не хранил.
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

    /// Единственная дверь перестановки строк ленты — см. `Project::set_lane_order`.
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

/// На что смотрит `ClipEvent.media` — ЕДИНСТВЕННЫЙ ответ на вопрос
/// «файл это или лента» (Р.50, п.378). Заведён по образцу `lane_key`:
/// отображение, выписанное в шести местах, рано или поздно разъезжается.
pub enum Source<'a> {
    File(&'a MediaRef),
    Seq(&'a Sequence),
}

/// Чужая OTIO-конструкция, которую мы не рендерим (неизвестная схема,
/// вложенный Stack, выключенный клип) — серый плейсхолдер (Р.24 п.5).
/// Занимает место на дорожке (участвует в проверке перекрытий), удалять
/// можно (undo работает), двигать нельзя: сырьё пишется обратно нетронутым.
/// `key` — ключ сырого JSON в Retained (kt-otio).
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
    /// Маркеры (сортированы по времени, без дублей). У метки может быть
    /// имя (п.1 р.10); старые проекты с массивом чисел читаются как есть.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<Marker>,
    /// Выделенный регион таймлайна (вне undo-истории, как в старом редакторе).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
    /// Loop по региону (вне undo).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub loop_enabled: bool,
    /// ДОРОЖКИ — объекты сверху вниз (Р.62, change B): порядок вектора =
    /// порядок строк, номер события = ранг среди дорожек своего вида.
    /// Пустые дорожки полноценны. На загрузке дописывается вверх до
    /// max(lane содержимого)+1 и не меньше одной видео и одной аудио.
    /// Счётчики и `lane_order` прежних раундов — `legacy_*`, только на
    /// чтение.
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
    /// Morph-события. Применяются к клипам в `effective_clips()`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub morphers: Vec<MorpherEvent>,
    /// Переходы на стыках клипов (Р.24: кроссфейд — явный объект, не
    /// перекрытие).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<TransitionEvent>,
    /// Плейсхолдеры чужих OTIO-конструкций (появляются только из импорта).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub foreign: Vec<ForeignEvent>,
    /// Вложенные ленты (Р.50, п.378). Пусто — проект плоский, и ВСЕ
    /// быстрые пути (validate, effective_clips, писатель формата)
    /// остаются в точности прежними: на этом стоят GOLDEN и байтовая
    /// стабильность файлов без вложений.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sequences: Vec<Sequence>,
    /// Состояние вьюпорта редактора (п.13, порт v1: timelineViewStartMs/
    /// zoomLevel/cursorMs). Вне undo-истории (как region); старые файлы
    /// без секции читаются как None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiState>,
    /// Переслаивание клипов на одной дорожке (Р.28, решение пользователя:
    /// «опция проекта, которая позволит переслаивать несколько событий одной
    /// дорожки одновременно… включи её по умолчанию»). Включено — перекрытия
    /// законны и звучат как в v1 (equal-power на двоих, 1/N при 3+), наезд
    /// драгом НИЧЕГО не режет. Выключено — прежний инвариант Р.24: клипы
    /// одной (kind, lane) не перекрываются.
    #[serde(default = "default_true")]
    pub overlap_layers: bool,
    /// Трим РАСТЯГИВАЕТ анимацию вместе с событием (Р.40, решение
    /// пользователя: «Ну можно НЕ по умолчанию сделать такое поведение
    /// да»). ВЫКЛЮЧЕНО по умолчанию, и это главное: ключ стоит на своём
    /// кадре материала, куда бы ни ехал край события — ровно то, что
    /// пользователь и просил («вот как сделать так чтобы он НЕ съезжал
    /// никуда хоть я левый край двигаю влево хоть в право»).
    ///
    /// Включённая опция нужна для другой манеры работы — «анимация от
    /// начала события»: тогда доля ключа внутри события сохраняется, а
    /// кадр материала под ним меняется.
    #[serde(default, skip_serializing_if = "is_false")]
    pub trim_stretches_auto: bool,
    /// Монтажная длина таймлайна, когда она БОЛЬШЕ материала: чужие файлы
    /// задают её хвостовым Gap'ом после последнего клипа (решение
    /// пользователя Р.25 — «хранить длину, рендерить до конца», иначе
    /// сохранение молча укорачивало чужой монтаж). None — длина по клипам.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeline_end: Option<Tick>,
    #[serde(default, rename = "camera_lanes", skip_serializing)]
    pub legacy_camera_lanes: u32,
    /// Камеры-события. Активную в момент t выбирает `active_camera_at`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<CameraEvent>,
    /// Именованные области «время × дорожки» (Р.35 п.2): рамка выделения,
    /// сохранённая под именем. Морфер ссылается на них ПО ИМЕНИ.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub areas: Vec<MarkedArea>,
    /// Публичные параметры ЛЕНТЫ, содержимое которой сейчас стоит на
    /// месте корня (Р.59, стадия 3).
    ///
    /// Поле есть и здесь, и у `Sequence`, ровно по той же причине, по
    /// которой здесь лежат `morphers`, `cameras` и `lane_order`: вход
    /// внутрь ленты сделан ОБМЕНОМ содержимого (`swap_reel`), и команда
    /// правит ту ленту, в которой стоишь, не зная о вложениях вовсе.
    /// У настоящего корня набор всегда пуст: корень никому не экземпляр.
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

/// Морфер (Р.35): непрозрачный «патч» на фиолетовой morph-дорожке —
/// СЛОЙ, а не пометка. Действует там, где его блок накрывает цель
/// (`timed`), и по умолчанию — на всё, что НИЖЕ его строки.
///
/// До Р.35 это был «патч на замороженный список целей, целиком и вне
/// времени»: `start`/`duration` только позиционировали блок. Файлы того
/// времени читаются с `timed = false` и рендерятся в точности как
/// раньше (`migrate_legacy_morpher`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MorpherEvent {
    pub id: MorphId,
    /// Индекс morph-дорожки (0..morph_lanes).
    pub lane: u32,
    pub start: Tick,
    pub duration: Tick,
    /// Кого накрывает. Умолчание — «всё, что ниже строки морфера».
    #[serde(default, skip_serializing_if = "MorphTargets::is_default")]
    pub targets: MorphTargets,
    /// Что делает: подмена источника И цепочки эффектов — одно событие
    /// несёт оба (до Р.35 это были взаимоисключающие `MorphKind`).
    /// `alias = "kind"` — вход старых файлов.
    #[serde(
        rename = "patch",
        alias = "kind",
        default,
        skip_serializing_if = "MorphPatch::is_empty"
    )]
    pub patch: MorphPatch,
    /// Выключатель события целиком (Р.33). Выключенный морфер не
    /// применяется к целям, но живёт на дорожке и включается как был.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// Действие ограничено ОКНОМ блока (умолчание, Р.35 п.1: «если их
    /// проигрываем — то и морфируем»). `false` — прежняя семантика:
    /// патч на цели целиком, время блока ни на что не влияет.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub timed: bool,
    /// Сила наложения слоя 0..1 (Р.38 п.12: «у морфера нет возможности
    /// применить fade и opacity»). 1.0 — слой действует полностью, и это
    /// прежний путь БАЙТ-В-БАЙТ: слот смешения не появляется вовсе.
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub strength: f64,
    /// Нарастание и затухание силы у краёв блока. Ноль — жёсткие края.
    #[serde(default, skip_serializing_if = "tick_is_zero")]
    pub fade_in: Tick,
    #[serde(default, skip_serializing_if = "tick_is_zero")]
    pub fade_out: Tick,
}

fn is_true(v: &bool) -> bool {
    *v
}

/// Кого накрывает морфер (Р.35). Пространство и время разведены:
/// `Below`/`Clips` выбирают ПРОСТРАНСТВО (окно берётся из блока),
/// `Marks`/`Areas` — именованные диапазоны и области (Р.35 п.2).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(tag = "on", rename_all = "snake_case")]
pub enum MorphTargets {
    /// Всё, что НИЖЕ строки морфера в порядке дорожек (adjustment layer).
    #[default]
    Below,
    /// Замороженный при создании список — прежнее поведение.
    Clips { ids: Vec<ClipId> },
    /// Именованные метки-диапазоны: окна берутся из них.
    Marks { names: Vec<String> },
    /// Именованные области (время × дорожки).
    Areas { names: Vec<String> },
}

impl MorphTargets {
    pub fn is_default(&self) -> bool {
        matches!(self, MorphTargets::Below)
    }

    /// Замороженные id — пусто у всех режимов, кроме `Clips`.
    pub fn clip_ids(&self) -> &[ClipId] {
        match self {
            MorphTargets::Clips { ids } => ids,
            _ => &[],
        }
    }

    /// Замороженные id для правки (RemoveClip/Split держат их живыми).
    pub fn clip_ids_mut(&mut self) -> Option<&mut Vec<ClipId>> {
        match self {
            MorphTargets::Clips { ids } => Some(ids),
            _ => None,
        }
    }
}

/// Ручной `Deserialize`: файлы до Р.35 несут ГОЛЫЙ массив id
/// (`"targets": [1, 2]`) — он читается как `Clips`. Прецедент — `Marker`.
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

/// Что морфер делает с целью. Все поля — «ничего не делать» по
/// умолчанию, поэтому пустой патч не сериализуется вовсе.
///
/// Подмена источника и цепочки эффектов живут ВМЕСТЕ (Р.35 п.3: «в ui
/// морфера их объединить») — в интерфейсе это одна fx-полоса, где
/// подмена нарисована фиолетовым слотом первой.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct MorphPatch {
    /// `None` = источник не менять (только сдвиг точки входа).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<MediaId>,
    /// Сдвиг source_in (тики); применяется, если `absolute` не задан.
    #[serde(default, skip_serializing_if = "tick_is_zero")]
    pub source_in_delta: Tick,
    /// Абсолютный source_in — приоритетнее delta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absolute_source_in: Option<Tick>,
    /// Выключатель морф-слота: настройки подмены целы, но она не
    /// действует (галка у фиолетового слота).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub src_off: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub video_fx: Vec<FxInstance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio_fx: Vec<FxInstance>,
    /// Выключатели слотов, параллельны своим цепочкам.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub video_bypass: Vec<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio_bypass: Vec<bool>,
    /// Кривые параметров слоя (Р.38 п.13: «у морфера я не могу задавать
    /// автоматизацию эффектов»). Параллельны своим цепочкам, как и
    /// выключатели.
    ///
    /// **Домен — время БЛОКА морфера, 0..duration.** У морфера нет
    /// материала: он слой правок, а не событие с исходником. Тот же
    /// домен, что у камеры и у перехода, и то же правило, что Р.37 задал
    /// событиям: «временной промежуток, вырезанный этим элементом».
    /// В `apply_to` ключи пересчитываются в материал ЦЕЛИ.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub video_auto: Vec<Vec<ParamCurve>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio_auto: Vec<Vec<ParamCurve>>,
}

/// Ручной `Deserialize`: принимает три формы — плоскую (Р.35) и обе
/// старых (`{"Replace": …}` / `{"EffectBatch": …}`), потому что до Р.35
/// это был внешне-тегированный enum `MorphKind`.
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
    /// Пустой патч ничего не делает — такой не сериализуется.
    pub fn is_empty(&self) -> bool {
        self.media.is_none()
            && self.source_in_delta.0 == 0
            && self.absolute_source_in.is_none()
            && self.video_fx.is_empty()
            && self.audio_fx.is_empty()
            // кривые без слотов невозможны, но «пустой» патч НЕ
            // сериализуется — молча потерять их нельзя (Р.38)
            && self.video_auto.iter().all(|a| a.is_empty())
            && self.audio_auto.iter().all(|a| a.is_empty())
    }

    /// Меняет ли патч ЭТОТ клип. Ответ «нет» — причина не резать его
    /// вовсе: слой с одними видео-эффектами не должен трогать аудио под
    /// собой (и уж тем более оставлять на нём шов).
    pub fn affects(&self, c: &ClipEvent) -> bool {
        self.replaces_source() || !self.chain_for(c.kind).is_empty()
    }

    /// Тянет ли ВРЕМЯ цепочка патча для клипа этого вида.
    ///
    /// Спрашивается там, где решают, РЕЗАТЬ ли цель: варп нельзя «смешать
    /// обратно» слотом силы — он меняет не значение, а сколько материала
    /// съедено, и действует на весь клип, чем его ни гаси.
    pub fn warps_time_for(&self, kind: ClipKind) -> bool {
        self.chain_for(kind).iter().any(|f| f.warps_time())
    }

    /// Подменяет ли патч источник (с учётом выключателя морф-слота).
    pub fn replaces_source(&self) -> bool {
        !self.src_off
            && (self.media.is_some()
                || self.absolute_source_in.is_some()
                || self.source_in_delta.0 != 0)
    }

    /// ВКЛЮЧЁННЫЕ эффекты патча для клипа этого вида. Выключенный слот
    /// просто не дописывается: у клипа своя нумерация bypass, и
    /// смешивать их нельзя.
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
        // ВАРП ВРЕМЕНИ достаётся ОБОИМ видам (Р.37 п.2). Он живёт в
        // `audio_fx` — там его канонический дом, `merge_chains`/
        // `split_chains` в UI на этом стоят, — но он не пиксельный фильтр,
        // а правило выбора исходного кадра. Без этого слой замедлял бы
        // звук и не замедлял картинку: рассинхрон, которого никто не просил.
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
            // СИЛА СЛОЯ достаётся и звуку (Р.38 п.12, решение «видео и
            // аудио сразу»). Живёт она в `video_fx` — там её канонический
            // дом, — но приглушать обязана оба вида.
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

    /// Кривые в том же порядке, в каком `chain_for` отдаёт слоты:
    /// выключенные пропущены, у видео в хвосте — варп-слоты из аудио.
    /// Разойдись порядок — кривая уехала бы на чужой эффект.
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

    /// Все медиа, которые держит патч (гард `RemoveMedia`).
    pub fn media_refs(&self) -> impl Iterator<Item = MediaId> + '_ {
        self.media.into_iter()
    }

    /// Применить патч к копии клипа (v1 applyMorpherPatchToClipCopy).
    /// Верхнего клампа source_in по длительности источника нет намеренно:
    /// модель не знает длительностей медиа (никакого FFmpeg в kt-model);
    /// чтение за концом источника движок отдаёт тишиной/последним кадром.
    /// `anchor` — начало блока морфера на таймлайне, `frame_step` — шаг
    /// кадра проекта: по ним ключи слоя переводятся из времени БЛОКА в
    /// материал цели тем же отображением, что и всё остальное
    /// (`clip_material_at` учитывает реверс).
    pub fn apply_to(&self, c: &mut ClipEvent, anchor: Tick, frame_step: i64) {
        if self.replaces_source() {
            if let Some(m) = self.media {
                // Подмена источника меняет МЕДИА, а значения генератора —
                // свойство события на КОНКРЕТНОМ медиа (Р.59, стадия 2).
                // Патч их не несёт и придумать не может, поэтому цель
                // остаётся без них: адрес источника вырождается в `media`,
                // и кадр берётся у общего источника вида — того же, что
                // рисует бин. Придумай мы значения — слой рисовал бы
                // «чей-то чужой титр», а это молчаливая порча.
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
            // ЯКОРЬ И КЛЮЧИ ЕДУТ ОДНОЙ ДЕЛЬТОЙ (Р.43, п.300) — то же
            // правило, что у `clip_slice`.
            //
            // Ключ приклеен к материалу (t212), и это верно, пока материал
            // ТОТ ЖЕ: обрезка прячет и открывает ключи, разрез делит кривую.
            // Подмена источника меняет саму систему координат, и кривая,
            // оставшаяся в старой, уезжает из окна события целиком: она
            // зажимается последним ключом и выглядит как «эффект перестал
            // применяться». У пользователя `source_in` уезжал с 47.0 с на
            // 99.0 с при ключах на 47.0..59.6 с — pan3d становился
            // тождественным (rot_y = rot_z = 0).
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
        // Мастер-выключатель гасит цепочку СОБЫТИЯ, а не слой над ним
        // (Р.35): своя цепочка уезжает целиком, эффекты слоя действуют.
        // Иначе выключенная цепочка события молча убивала бы слой,
        // который к ней никакого отношения не имеет.
        if c.fx_off {
            c.fx.clear();
            c.fx_bypass.clear();
            c.fx_auto.clear();
            c.fx_off = false;
        }
        // fx_bypass короче цепочки = дописанные слоты включены
        // (active_fx трактует отсутствующий флаг как false)
        let auto = self.auto_for(c.kind);
        c.fx.extend(add);
        // кривые слоя (Р.38 п.13) — в домен МАТЕРИАЛА цели: у слоя своё
        // время (0..duration блока), у события — материал, и движок
        // сэмплирует именно материал
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
    /// Нужен ли слот смешения: сила меньше единицы или есть фейды.
    /// На умолчаниях (1.0 и нули) слота НЕТ — путь байт-в-байт прежний, и
    /// на этом стоят GOLDEN_VIDEO и GOLDEN_AUDIO.
    pub fn mixes(&self) -> bool {
        self.strength < 1.0 || self.fade_in.0 > 0 || self.fade_out.0 > 0
    }

    /// Нужен ли слот смешения С УЧЁТОМ ВОРОТ (Р.55, С2).
    ///
    /// `gate` — окна, в которых слой ДЕЙСТВУЕТ, в координатах таймлайна,
    /// уже пересечённые с телом цели. Если ворота покрывают тело целиком,
    /// ответ прежний (`mixes`) и путь остаётся байт-в-байт: на умолчаниях
    /// слота нет, и на этом стоят GOLDEN_VIDEO и GOLDEN_AUDIO.
    pub fn mixes_on(&self, gate: &[TickRange], body: TickRange) -> bool {
        self.mixes() || !covers_whole(gate, body)
    }

    /// Кривая силы в домене БЛОКА с ВОРОТАМИ (Р.55, С2).
    ///
    /// Ворота — это и есть «по времени» для слоя, который НЕ РЕЖЕТ цель:
    /// вне окна сила равна нулю, внутри — обычной силе с фейдами. Ступень
    /// строится двумя ключами на СОСЕДНИХ тиках: на одном тике
    /// `dedup_by_key` схлопнул бы их в один, и вместо ступени вышла бы
    /// рампа во всё тело.
    ///
    /// Ключ за краем тела не ставится: `clip_material_at` клампит края, и
    /// два ключа схлопнулись бы в один тик с РАЗНЫМИ значениями. Там, где
    /// ворота упираются в край тела, гасить нечего — слой действует от
    /// самого края.
    pub fn mix_curve_on(&self, gate: &[TickRange], body: TickRange) -> Option<ParamCurve> {
        if covers_whole(gate, body) {
            return self.mix_curve();
        }
        let d = self.duration.0.max(1);
        let fi = self.fade_in.0.clamp(0, d);
        let fo = self.fade_out.0.clamp(0, d - fi);
        let s = self.strength;
        // сила в точке ТАЙМЛАЙНА по фейдам блока (без ворот)
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
            // переломы фейдов, попавшие ВНУТРЬ окна
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

    /// Кривая силы в домене БЛОКА: 0 на самом краю, `strength` на плато.
    /// `None` — фейдов нет, хватает базового значения слота.
    pub fn mix_curve(&self) -> Option<ParamCurve> {
        if self.fade_in.0 <= 0 && self.fade_out.0 <= 0 {
            return None;
        }
        let d = self.duration.0.max(1);
        // фейды не налезают друг на друга: сумма клампится длиной блока
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

    /// Наложить слой на кусок цели: патч плюс слот СИЛЫ (Р.38 п.12).
    ///
    /// Сила — последний слот цепочки: она смешивает ВХОД цепочки с её
    /// результатом, поэтому обязана стоять после всего, что слой делает.
    pub fn apply_to(&self, c: &mut ClipEvent, frame_step: i64, gate: &[TickRange]) {
        let body = TickRange::new(c.start, c.start + c.duration);
        self.patch.apply_to(c, self.start, frame_step);
        if !self.mixes_on(gate, body) {
            return;
        }
        // с какого слота начинается вклад ИМЕННО этого слоя
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

    /// Все медиа, которые держит морфер.
    pub fn media_refs(&self) -> impl Iterator<Item = MediaId> + '_ {
        self.patch.media_refs()
    }

    /// Окно блока на таймлайне.
    pub fn window(&self) -> TickRange {
        TickRange::new(self.start, self.start + self.duration)
    }

    /// Цель ли этот клип. `Below` — умолчание: всё, что НИЖЕ строки
    /// морфера в порядке дорожек (adjustment layer); `Marks`/`Areas` —
    /// именованные диапазоны и области (Р.35 п.2), они же задают ОКНА.
    pub fn covers(&self, c: &ClipEvent, rows: &[(LaneKind, u32)]) -> bool {
        match &self.targets {
            MorphTargets::Clips { ids } => ids.contains(&c.id),
            MorphTargets::Below => {
                let mine = rows.iter().position(|&k| k == (LaneKind::Morph, self.lane));
                let his = rows.iter().position(|&k| k == lane_key(c));
                matches!((mine, his), (Some(a), Some(b)) if b > a)
            }
            // ссылку резолвит `ref_windows`: мёртвая ссылка = пустое
            // множество, а не ошибка (файл её не теряет)
            MorphTargets::Marks { .. } | MorphTargets::Areas { .. } => true,
        }
    }

    /// Окна именованных ссылок, ограниченные телом клипа. Пусто — режим
    /// не ссылочный либо ссылки никуда не ведут.
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

    /// Индекс строки морфера — ключ порядка наложения слоёв.
    pub fn row_in(&self, rows: &[(LaneKind, u32)]) -> usize {
        rows.iter().position(|&k| k == (LaneKind::Morph, self.lane)).unwrap_or(0)
    }

    /// Окна, в которых морфер РЕАЛЬНО меняет ЭТОТ клип. Пусто — клип не
    /// трогается и (важно!) не режется. Порядок проверок объясняет всю
    /// стоимость решения:
    ///
    /// 1. цель ли клип;
    /// 2. **меняет ли патч клип ЭТОГО вида** — слой с одними видео-FX не
    ///    должен оставлять шов на аудио под собой;
    /// 3. `timed = false` (файлы до v4) — патч на всё тело, как в v1;
    /// 4. клип, который резать НЕЛЬЗЯ (варп потребления источника —
    ///    находка 2 Р.35), патчится целиком;
    /// 5. запретные зоны (окна переходов и наезды соседей) выталкивают
    ///    границу наружу: слой «липнет» к краю кроссфейда, зато соседи по
    ///    дорожке не переслаиваются от резки (находка 1 Р.35);
    /// 6. огрызок короче `crate::MIN_CLIP_DURATION` схлопывается к краю тела.
    ///
    /// Запретные зоны считаются по ЛЕНТЕ `reel`, а не по корню (Р.50):
    /// слой может лежать внутри вложения. `p` остаётся ради настроек
    /// проекта — вложение их наследует, своих у него нет.
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
        // ВАРП ВРЕМЕНИ — вето на РЕЗКУ (Р.37): линейно посчитанный
        // `source_in` куска у растянутого клипа указывает не туда, поэтому
        // такой клип патчится ЦЕЛИКОМ. Но «целиком» — это про ГРАНИЦЫ, а
        // не про ДОСЯГАЕМОСТЬ: вето стояло ПЕРВЫМ и возвращало всё тело,
        // ни разу не спросив, дотягивается ли блок до клипа ПО ВРЕМЕНИ.
        //
        // Цена ошибки (Р.43, п.298): морф-блок длиной 0.6 с на 5020-й
        // секунде патчил КАЖДОЕ событие проекта, у которого есть
        // slowed&reverb, — в реальном проекте пользователя это подменило
        // источник у аудио на 148-й секунде («звук от другого события»),
        // а морфер со slowed&reverb в патче раздал варп ВСЕМ событиям
        // ниже своей строки (`chain_for` отдаёт варп обоим видам).
        // Поэтому сначала считаем ДОСЯГАЕМОСТЬ, и только потом вето.
        // РЕЖЕТ ли слой цель (Р.55, С2). Патч из одних эффектов не режет
        // вовсе: он ложится на ВСЁ ТЕЛО, а «по времени» задают ВОРОТА
        // силы. Тогда граница точна до тика, соседей и переходов она не
        // касается, а шва на материале не возникает совсем.
        let cuts = self.cuts_target(c);
        let warps = |fx: &[FxInstance]| fx.iter().any(|f| f.warps_time());
        // Вето по варпу имеет смысл ТОЛЬКО там, где режут: линейно
        // посчитанный `source_in` куска у растянутого клипа указывает не
        // туда. Ворота ничего не режут, поэтому вето к ним не относится —
        // и «по времени» начинает работать и на растянутых событиях.
        let no_cut = cuts && (warps(&c.active_fx()) || warps(&self.patch.chain_for(c.kind)));
        // ссылочные режимы: ВРЕМЯ задаёт сама ссылка, блок его не
        // ограничивает — иначе пользователь ткнул бы в дальний диапазон и
        // не понял, почему ничего не произошло. Галка «по времени» в этих
        // режимах гаснет (окно и так пришло из ссылки).
        if matches!(self.targets, MorphTargets::Marks { .. } | MorphTargets::Areas { .. }) {
            let refs = self.ref_windows(c, p);
            if refs.is_empty() {
                return Vec::new(); // ссылки сюда не дотянулись
            }
            if no_cut {
                return vec![body];
            }
            if !cuts {
                return refs; // ворота: окна ссылок как есть, до тика
            }
            return refs.into_iter().filter_map(|w| self.fit_window(w, c, p, reel, body)).collect();
        }
        if !self.timed {
            return vec![body]; // v1: время блока не действует
        }
        let w = self.window().intersect(&body);
        if w.len() < crate::MIN_CLIP_DURATION {
            return Vec::new(); // блок сюда не дотянулся
        }
        if no_cut {
            return vec![body];
        }
        if !cuts {
            return vec![w]; // ворота: окно блока как есть, до тика
        }
        match self.fit_window(w, c, p, reel, body) {
            Some(w) => vec![w],
            None => Vec::new(),
        }
    }

    /// Режет ли слой цель на куски, или ложится на всё тело с воротами
    /// силы (Р.55, С2).
    ///
    /// Режут ровно два вида патча, и оба — потому что слотом силы их не
    /// выключить:
    ///
    /// * **подмена источника** — «взять кадр из другого файла» не
    ///   выражается значением параметра вовсе;
    /// * **варп времени в патче** — он меняет не значение, а сколько
    ///   материала съедено, и действует на весь клип, чем его ни гаси.
    ///
    /// Всё остальное (а это почти всё) — цепочка эффектов, и её вклад
    /// гасится кривой силы до нуля. Так «по времени» перестаёт зависеть от
    /// того, есть ли у события переход: резать внутри окна кроссфейда
    /// нельзя, а ГАСИТЬ — можно где угодно.
    pub fn cuts_target(&self, c: &ClipEvent) -> bool {
        self.patch.replaces_source() || self.patch.warps_time_for(c.kind)
    }

    /// Посадить окно на сетку материала, вытолкнуть из запретных зон и
    /// схлопнуть огрызки к краям тела. `None` — окна не осталось.
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
        // Граница садится на сетку МАТЕРИАЛА клипа: кадр — у видео, 8
        // сэмплов (15 тиков) — у аудио. Иначе точка входа куска пришлась
        // бы на дробный кадр: у прямого клипа это стоило бы сэмпла на
        // шве, у reversed — кадра (кламп зеркального якоря). Отсчёт от
        // начала тела, а у reversed — от КОНЦА: там якорь материала.
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
        // ПОРЯДОК ВАЖЕН: сетка сначала, зоны после. Край зоны бывает не
        // на сетке (клип соседа начался между кадрами), и тогда выбор
        // один: либо кадр материала, либо неразрезанное окружение. Дороже
        // стоит окружение — кусок с бо́льшим `start` вылезает над соседом
        // и меняет веса кроссфейда, а сетка стоит одного кадра на шве.
        // ВЫТАЛКИВАЕМ ТОЛЬКО ВНУТРЬ (Р.55, С2). До этого граница уходила
        // к БЛИЖАЙШЕМУ краю зоны — в том числе НАРУЖУ, и у события с
        // фейдом слева и фейдом справа левая граница уезжала в начало
        // тела, правая в конец: «по времени» превращалось в «на всё
        // событие». Окно теперь только СОКРАЩАЕТСЯ; не осталось окна —
        // слой сюда не дотянулся, и это честный ответ, а не молчаливое
        // расширение.
        let zones = forbidden_zones(reel, c);
        let mut s = snap_out_of(grid(w.start), &zones, true);
        let mut e = snap_out_of(grid(w.end), &zones, false);
        s = Tick(s.0.clamp(body.start.0, body.end.0));
        e = Tick(e.0.clamp(body.start.0, body.end.0));
        // огрызок, который всё равно нельзя было бы получить командой
        // (Split запрещает куски короче минимума), липнет к краю тела
        if (s - body.start) < crate::MIN_CLIP_DURATION {
            s = body.start;
        }
        if (body.end - e) < crate::MIN_CLIP_DURATION {
            e = body.end;
        }
        (e - s >= crate::MIN_CLIP_DURATION).then_some(TickRange::new(s, e))
    }
}

/// Слить пересекающиеся и примыкающие диапазоны в непересекающийся набор
/// (ссылок у морфера может быть несколько, и они могут накладываться).
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

/// Вытолкнуть точку из запретных зон В ЗАДАННУЮ СТОРОНУ. Зоны могут
/// перекрываться и примыкать, поэтому проход повторяется до сходимости
/// (не более чем по числу зон — каждый шаг выводит точку из очередной).
///
/// `forward` — куда двигать: начало окна вперёд (к концу зоны), конец окна
/// назад (к её началу). Сторона задана, а не выбрана по близости, и это
/// принципиально: «к ближайшему краю» расширяет окно там, где граница
/// попала в зону чуть дальше её середины, и слой начинает действовать
/// ШИРЕ, чем человек просил (Р.55, С2).
fn snap_out_of(t: Tick, zones: &[TickRange], forward: bool) -> Tick {
    let mut t = t;
    for _ in 0..=zones.len() {
        let Some(z) = zones.iter().find(|z| z.start < t && t < z.end) else { return t };
        t = if forward { z.end } else { z.start };
    }
    t
}

/// Покрывают ли ворота тело целиком (тогда их как бы и нет).
fn covers_whole(gate: &[TickRange], body: TickRange) -> bool {
    gate.iter().any(|w| w.start <= body.start && body.end <= w.end)
}

/// Ключ строки события: вид дорожки и её номер. Отображение было
/// продублировано четырежды в `timeline.rs` и дважды в `convert.rs`.
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

/// Старший бит id — признак КУСКА клипа, порождённого морфер-слоем.
/// Настоящие клипы в этот диапазон не попадают (проверяет `validate`),
/// поэтому оригинал восстанавливается из самого id и побочные таблицы не
/// нужны.
///
/// **Бит понижен с 63-го до 61-го в Р.59 (стадия 2).** Адрес источника
/// генератора берётся как `GEN_TAG | id` СОБЫТИЯ, а событием бывает и
/// кусок слоя: останься тег наверху, `GEN_TAG | slice_id` залез бы в
/// диапазон `NEST_TAG`, и кусок титра молча поделил бы кэш с лентой.
/// Ниже — `const _`-сторож, который не даст развести эти три числа.
pub const SLICE_TAG: u64 = 1 << 61;
/// Столько младших бит id отдано номеру куска.
const SLICE_IDX_BITS: u32 = 16;
/// Потолок id настоящего клипа (всё выше зарезервировано под куски).
pub const MAX_REAL_CLIP_ID: u64 = 1 << (61 - SLICE_IDX_BITS);

/// Адресное пространство целиком: настоящий клип < кусок < генератор <
/// лента. Нарушение любого неравенства — это два разных объекта с одним
/// ключом кэша, и заметно это стало бы стейл-картинкой, а не падением.
const _: () = {
    // куски помещаются под своим тегом…
    assert!(MAX_REAL_CLIP_ID << SLICE_IDX_BITS <= SLICE_TAG);
    // …а САМЫЙ СТАРШИЙ кусок — строго ниже тега генератора
    assert!(SLICE_TAG + (MAX_REAL_CLIP_ID << SLICE_IDX_BITS) <= GEN_TAG);
    assert!(GEN_TAG < NEST_TAG);
};

/// Id куска — чистая функция от (оригинал, номер). Ни счётчиков, ни
/// хэшей: id не должен двигаться от правки соседнего клипа или переноса
/// блока морфера мышью, иначе аудио-FX сбрасывались бы на каждый чих
/// (инвариант №3).
pub fn slice_id(origin: ClipId, idx: u32) -> ClipId {
    ClipId(SLICE_TAG | (origin.0 << SLICE_IDX_BITS) | idx as u64)
}

/// Оригинал куска — ИЗ САМОГО id. `None` — это не кусок.
pub fn slice_origin(id: ClipId) -> Option<ClipId> {
    (id.0 & SLICE_TAG != 0).then_some(ClipId((id.0 & !SLICE_TAG) >> SLICE_IDX_BITS))
}

/// Тот же клип или его кусок. Всё, что адресует клип по id СРЕДИ
/// effective (концы переходов в движке), обязано спрашивать это.
pub fn same_clip(eff_id: ClipId, origin: ClipId) -> bool {
    eff_id == origin || slice_origin(eff_id) == Some(origin)
}

/// Кусок клипа на окне `w`. Правило source_in — ТО ЖЕ, что у
/// `Command::Split` (который с Р.35 сам зовёт эту функцию): у прямого
/// клипа точка входа едет с началом куска, у reversed — с его концом,
/// потому что материал у него зеркальный (п.16).
/// Пара якорей куска: ЛИНЕЙНЫЙ и МАТЕРИАЛЬНЫЙ (Р.39).
///
/// У `source_in` две работы сразу: он якорь ЧТЕНИЯ МАТЕРИАЛА и начало ОСИ
/// АВТОМАТИЗАЦИИ — время события ключа есть ровно `start + (k.t −
/// source_in)` (это и считает `clip_timeline_at`). При линейной карте обе
/// совпадают; под варпом расходятся, и якорь обязан ехать по интегралу
/// скорости, а ось — остаться линейной (решение Р.37: «БЕЗ учёта
/// замедления-ускорения»).
fn slice_anchors(c: &ClipEvent, w: TickRange) -> (Tick, Tick) {
    let linear = if c.reversed {
        c.source_in + ((c.start + c.duration) - w.end)
    } else {
        c.source_in + (w.start - c.start)
    };
    let Some(warp) = crate::clip_warp(c) else { return (linear, linear) };
    // ВАРП, ПОТОМ ЗЕРКАЛО (Р.37 п.247) — тем же выражением, каким кадр
    // выбирает рендер: у прямого клипа якорь на НАЧАЛЕ окна, у зеркального
    // на его КОНЦЕ, отсчитанном от фактически потреблённого `used`, а не
    // от длины события.
    let tl = if c.reversed { w.end - c.start } else { w.start - c.start };
    (linear, c.source_in + Tick(warp.material_off(c.duration, c.reversed, tl.0)))
}

/// Насколько влево можно утащить НАЧАЛО клипа: дальше кончается материал.
///
/// У растянутого клипа это НЕ `start − source_in`: расширение влево на δ
/// съедает `rate·δ` материала, поэтому у ускоренного клипа предел ближе,
/// а у замедленного дальше. Инверсия карты — `output_time_for_source`.
pub fn trim_min_start(c: &ClipEvent) -> Tick {
    if c.reversed {
        // у зеркального клипа левый трим source_in не трогает (п.16)
        return Tick(0);
    }
    let Some(warp) = crate::clip_warp(c) else {
        return Tick((c.start - c.source_in).0.max(0));
    };
    // сколько ВЫХОДНОГО времени соответствует материалу, лежащему левее
    // точки входа: s < 0, карта экстраполирует скоростью в нуле
    let span = c.duration.0.max(1) as f64;
    // Скорость в нуле ≤ 0 (Р.57): материал слева НЕ потребляется этим
    // событием вовсе — ни назад, ни на удержании. Экстраполяция делила бы
    // на ноль и давала `i64::MIN` (переполнение в debug).
    if crate::eff_rate(warp.base(), warp.prof(), 0.0) <= 0.0 {
        return Tick(0);
    }
    let s = -(c.source_in.0 as f64) / span;
    let dt = crate::output_time_for_source(warp.base(), warp.prof(), s) * span;
    Tick((c.start.0 + dt.round() as i64).max(0))
}

/// Где ВО ВРЕМЕНИ СОБЫТИЯ кончается материал (Р.63 п.2), если он вообще
/// кончается внутри него. `None` — длина неизвестна или материала хватает
/// на всё событие.
///
/// Одна функция на всех: полоса таймлайна рисует отсюда штриховку хвоста,
/// `kt-cli probe` печатает отсюда же момент, а предел трима — это она же
/// плюс «не меньше нынешнего конца». Разойдись они — картинка, диагностика
/// и жест говорили бы о разном крае.
pub fn material_end_on_timeline(c: &ClipEvent, media_len: Option<Tick>) -> Option<i64> {
    let len = media_len.filter(|l| l.0 > 0)?;
    let left = (len.0 - c.source_in.0).max(0);
    let used = crate::clip_warp(c).map(|w| w.used()).unwrap_or(c.duration.0);
    if used <= left {
        return None; // материала хватает на всё событие
    }
    let end = trim_max_end_in(c, Some(len)).0;
    (end < (c.start + c.duration).0).then_some(end)
}

/// Насколько вправо можно утащить КОНЕЦ клипа, ЗНАЯ длину материала
/// (Р.63 п.2). `None` — длина неизвестна (генератор, лента, ещё не
/// открытый файл), и предела нет, как было всегда.
///
/// Прямое событие упирается концом в конец материала ровно так же, как
/// началом — в его начало: край справа такой же край, как слева. Предел
/// считается ПО МАТЕРИАЛУ, поэтому замедленное событие упирается позже
/// (при rate 0.5 секунда материала — две секунды на таймлайне), а
/// ускоренное раньше — карта варпа спрашивается той же функцией, что у
/// левого края.
pub fn trim_max_end_in(c: &ClipEvent, media_len: Option<Tick>) -> Tick {
    let by_material = || -> Tick {
        let Some(len) = media_len.filter(|l| l.0 > 0) else {
            return Tick(i64::MAX / 4);
        };
        // сколько материала осталось справа от точки входа
        let left = (len.0 - c.source_in.0).max(0);
        let Some(warp) = crate::clip_warp(c) else {
            return Tick(c.start.0 + left);
        };
        let span = c.duration.0.max(1) as f64;
        if crate::eff_rate(warp.base(), warp.prof(), 0.0) <= 0.0 {
            return Tick(i64::MAX / 4);
        }
        // выходное время, за которое съедается `left` материала
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
    // у зеркального предел ставит материал СЛЕВА (он уезжает вниз), и он
    // жёстче правого — берём оба
    Tick(trim_max_end(c).0.min(by_material().0))
}

/// Насколько вправо можно утащить КОНЕЦ клипа. Симметрично: у зеркального
/// клипа расширение вправо тянет `source_in` вниз, и предел ставит материал.
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

/// Материальная точка входа куска — ЕДИНСТВЕННОЕ правило проекта.
pub fn slice_source_in(c: &ClipEvent, w: TickRange) -> Tick {
    slice_anchors(c, w).1
}

/// Хватает ли материала слева от окна. ОДНА функция на команду, её
/// `check_on` и предфильтры в `ops`: разойдись они — жест то падал бы, то
/// молча не доезжал.
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
    // Ключи едут на ТУ ЖЕ дельту: разность `(k.t − source_in)` обязана
    // остаться прежней, иначе за якорем уедет вся кривая. Проверка на
    // числах: клип 4 с, рампа 1.0→2.0, разрез на 2 с — без компенсации
    // первая точка профиля правого куска встаёт в −1.25 вместо −1.0, и
    // его скорость в начале читается 1.625 вместо 1.5: стык без скачка
    // МАТЕРИАЛА, но со скачком СКОРОСТИ. Меняется только материал.
    //
    // Р.33 обещал, что команды ключей не трогают вовсе. Обещание держалось
    // тем, что при линейной карте дельта РОВНО НОЛЬ — так и осталось:
    // на клипе без варпа этот блок не исполняется.
    let shift = material - linear;
    if shift.0 != 0 {
        for slot in &mut s.fx_auto {
            for cur in slot {
                for k in &mut cur.keys {
                    k.t = k.t + shift;
                }
            }
        }
        // кривые генератора живут в том же домене (тик материала) и
        // обязаны ехать той же дельтой — иначе у куска титра «печатная
        // машинка» начинается не там, где у соседнего куска
        for cur in &mut s.gen_auto {
            for k in &mut cur.keys {
                k.t = k.t + shift;
            }
        }
    }
    s
}

/// Растянуть кривые события под НОВЫЙ материальный пролёт (Р.40, опция
/// `Project.trim_stretches_auto`).
///
/// Зовётся ТОЛЬКО из тримов и ТОЛЬКО при включённой опции: разрез и резка
/// морфер-слоем делят одну анимацию на части, растягивать там нечего.
///
/// Доля ключа считается от ПОТРЕБЛЁННОГО материала (`ClipWarp::used`), а не
/// от длины события: у растянутого клипа это разные величины, и мерить
/// долю длиной события значило бы завести третье правило рядом с двумя
/// существующими.
/// Доля ключа считается ОТ СТАРОГО якоря и переносится на новый: смысл
/// опции — «анимация приколота к событию», значит ключ, стоявший на
/// половине прежнего события, встаёт на половину нового. Мерить смещение
/// от НОВОГО якоря было бы другой операцией: у левого трима смещение уже
/// уменьшено на величину самого трима, и ключи съезжали бы дважды.
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

/// Сколько МАТЕРИАЛА событие реально занимает: у растянутого — ПРОЛЁТ
/// позиции (Р.57), у обычного — своя длина.
///
/// Именно пролёт, а не «сколько съедено»: кривая со знаком может уйти
/// назад и вернуться, съев вдвое больше, чем заняла. На положительных
/// кривых пролёт равен прежнему `used`.
pub fn clip_material_span(c: &ClipEvent) -> i64 {
    crate::clip_warp(c).map(|w| w.span_len()).unwrap_or(c.duration.0).max(1)
}

/// Морфер из файла ДО Р.35: прежняя семантика — патч действует на цели
/// целиком, время блока ни на что не влияет. Зовут ридер OTIO и
/// `ops::upgrade_legacy`; тот же приём, что `migrate_legacy_cammotion`
/// (п.230) — односторонний апгрейд, а не «совместимость».
pub fn migrate_legacy_morpher(m: &mut MorpherEvent) -> bool {
    if !m.timed {
        return false;
    }
    m.timed = false;
    true
}

/// Метка на таймлайне: момент и (необязательное) имя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Marker {
    /// Начало метки. Идентичность метки — по нему (как раньше по `t`).
    pub t: Tick,
    /// Диапазонная метка (Р.24, OTIO marked_range): 0 = точечная.
    #[serde(default, skip_serializing_if = "tick_is_zero")]
    pub duration: Tick,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Цвет OTIO Marker.2 — round-trip чужих файлов; UI рисует своим стилем.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

fn tick_is_zero(t: &Tick) -> bool {
    t.0 == 0
}

/// Именованная область таймлайна: время × набор дорожек (Р.35 п.2,
/// дословно: «area-selection сделать сохраняемой как marked_region в
/// виде marked_area, и тоже на неё ссылаться»).
///
/// Имя — это АДРЕС: непустое и уникальное в одном пространстве имён с
/// метками-диапазонами («область „финал“ и метка „финал“» в одном пикере
/// — ловушка). Переименование правит ссылки морферов.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkedArea {
    pub name: String,
    pub start: Tick,
    pub duration: Tick,
    /// СЕМАНТИЧЕСКИЕ ключи дорожек, не номера строк: перестановка строк
    /// область не двигает. Пусто = все дорожки.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lanes: Vec<(LaneKind, u32)>,
    /// Цвет OTIO-совместимых меток; UI рисует своим стилем.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

impl MarkedArea {
    pub fn range(&self) -> TickRange {
        TickRange::new(self.start, self.start + self.duration)
    }

    /// Накрывает ли область эту дорожку (пустой список = все).
    pub fn covers_lane(&self, key: (LaneKind, u32)) -> bool {
        self.lanes.is_empty() || self.lanes.contains(&key)
    }
}

/// Уникальное имя в занятом пространстве: `wanted`, если свободно, иначе
/// «wanted 2», «wanted 3»… ОДНА функция на проверку команд и на вставку —
/// разойдись они, батч вставки падал бы на собственных суффиксах.
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
    /// Конец диапазона (== началу у точечной).
    pub fn end(&self) -> Tick {
        self.t + self.duration
    }
    pub fn is_range(&self) -> bool {
        self.duration.0 > 0
    }
}

impl<'de> Deserialize<'de> for Marker {
    /// Принимает старую форму (число тиков), объект {t, name} и диапазонную
    /// {t, duration, name, color}: старые проекты продолжают открываться.
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

/// Вьюпорт таймлайна, сохраняемый в проекте.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UiState {
    /// Левый край вью (тики).
    pub view_start: f64,
    /// Зум (тиков на пиксель).
    pub ticks_per_px: f64,
    /// Позиция курсора (тики).
    pub cursor: i64,
    /// ЛЕГАСИ-вход (до Р.35 порядок строк был чисто визуальным и жил
    /// здесь). Теперь он в модели (`Project.lane_order`) и ходит через
    /// undo — морфер-слой действует «на всё, что ниже», значит порядок
    /// влияет на картинку. Ридер переносит это поле в модель; писатель
    /// сюда больше не пишет.
    #[serde(default, skip_serializing)]
    pub lane_order: Vec<(LaneKind, u32)>,
    /// Медиа-бин: режим свободной раскладки (п.7 р.8). По умолчанию
    /// включён — пользователь сам группирует карточки.
    #[serde(default, skip_serializing_if = "is_false")]
    pub bin_free: bool,
    /// Позиции карточек в свободном режиме (медиа, x, y в точках от
    /// левого-верхнего угла бина). Переключение режимов раскладку НЕ
    /// трогает — она просто не используется в режиме сетки.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bin_layout: Vec<(MediaId, f32, f32)>,
    /// Ширина панели метров (п.6 р.13): её тянет собственная полоса,
    /// поэтому egui её не помнит — храним сами. 0 = по умолчанию.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub meters_w: f32,
    /// Индивидуальная высота строк в пикселях (Р.38 п.11: «разреши менять
    /// высоту каждого трека индивидуально»). УДОБСТВО, а не содержимое:
    /// высота не влияет на картинку — в отличие от ПОРЯДКА строк, который
    /// в Р.35 стал содержимым, потому что морфер действует «на всё, что
    /// ниже». Абсолютные пиксели: вертикальный зум переопределение не
    /// трогает, иначе «поставил дорожке высоту» не держалось бы.
    /// ЛЕГАСИ-вход (до Р.62 высота жила здесь): переносится на дорожку
    /// при чтении (`normalize`), писатель сюда больше не пишет.
    #[serde(default, skip_serializing)]
    pub lane_heights: Vec<(LaneKind, u32, f32)>,
    /// Группы карточек бина (п.13 р.11): рамка + имя + замок. Одиночный
    /// «замок» — это группа из одного члена без имени, отдельного поля
    /// нет намеренно (иначе появляется класс дефектов «карточка заперта,
    /// а группа нет»). Вне undo, как весь UiState.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bin_groups: Vec<BinGroup>,
    /// Имена дорожек (п.2 р.10). Пусто → подпись по формуле V1/A1/M1.
    /// Вне undo, как весь UiState; имя привязано к (вид, индекс) и едет
    /// вместе с содержимым при перестановке дорожек.
    /// ЛЕГАСИ-вход (до Р.62 имя жило здесь): переносится на дорожку при
    /// чтении (`normalize`), писатель сюда больше не пишет.
    #[serde(default, skip_serializing)]
    pub lane_names: Vec<(LaneKind, u32, String)>,
    /// Замок оси графика автоматизации (Р.40 п.15). ОДИН НА ВСЮ ПРОГРАММУ
    /// и помнится в проекте — решение пользователя дословно: «Один на всю
    /// программу, помнится в проекте». До этого он жил в `AutoView`
    /// каждого окна и умирал вместе с окном.
    ///
    /// Умолчание — ВКЛЮЧЁН (правило 4: полезный режим по умолчанию), а
    /// serde-умолчание у `bool` — `false`, поэтому поле хранит ОБРАТНОЕ
    /// («замок снят»): файлы без него читаются со включённым замком и
    /// остаются байт-стабильными.
    #[serde(default, skip_serializing_if = "is_false")]
    pub auto_axis_unlinked: bool,
    /// Набор экранных контролов над кадром (Р.64): 0 — выключены,
    /// 1 — трансформация (рамка, ручки масштаба, поворот, якорь),
    /// 2 — кроп (четыре ручки своей стороны).
    ///
    /// АКТИВЕН РОВНО ОДИН НАБОР — правило Resolve (`View > Viewer
    /// Overlay`): два набора над одним кадром дерутся за ту же мышь, и
    /// «где мои ручки» становится стабильным источником жалоб (разведка
    /// F-UI-001). Удобство, не содержимое: живёт вне undo, как весь
    /// `UiState`, и умолчание «выключены» держит файлы байт-стабильными.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub geom_controls: u32,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

/// Группа карточек медиа-бина (п.13 р.11).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BinGroup {
    /// Имя в разрыве верхней линии рамки; пустое — рамка без подписи.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub members: Vec<MediaId>,
    /// Замок: карточки группы не двигаются мышью.
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
}

/// Выбросить из групп исчезнувшие медиа и сами опустевшие группы
/// (медиа удаляют из проекта, а раскладка/группы жили отдельно и текли).
pub fn prune_bin_groups(groups: &mut Vec<BinGroup>, alive: &[MediaId]) {
    for g in groups.iter_mut() {
        g.members.retain(|m| alive.contains(m));
    }
    groups.retain(|g| !g.members.is_empty());
}

/// Вид дорожки в display-порядке (ClipKind не годится: нет Morph/Camera).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneKind {
    #[default]
    Video,
    Audio,
    Morph,
    /// Камерные дорожки (Р.29): живут только камеры-события.
    Camera,
}

impl LaneKind {
    pub const ALL: [LaneKind; 4] =
        [LaneKind::Video, LaneKind::Audio, LaneKind::Morph, LaneKind::Camera];

    /// Есть ли у строки этого вида mute и solo: у служебных (морф,
    /// камера) их нет — глушить там нечего (правило UI 9).
    pub fn has_switches(self) -> bool {
        matches!(self, LaneKind::Video | LaneKind::Audio)
    }
}

/// Стабильный адрес ДОРОЖКИ (Р.62, change B). Номер дорожки — ранг строки
/// (Р.56) и меняется при вставке соседа; id — нет. На нём держится
/// FX-состояние цепочки дорожки (инвариант 3) и адресация свойств.
/// `0` — «ещё не назначен» (`Sequence::new`, содержимое сворачивания);
/// назначает `Project::assign_track_ids` — на чтении и после команды.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
pub struct TrackId(pub u64);

/// Режим наложения композита ВИДЕОДОРОЖКИ на слои ниже (Р.62, F-CMP-003).
/// `out = lerp(below, blend(below, above), α_above × уровень)`; Normal —
/// прежний alpha_over байт-в-байт.
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

/// Пределы уровня аудиодорожки в дБ (паспорт свойств дорожки, Р.62).
pub const TRACK_GAIN_MIN_DB: f64 = -60.0;
pub const TRACK_GAIN_MAX_DB: f64 = 24.0;

fn is_normal_blend(b: &BlendMode) -> bool {
    *b == BlendMode::Normal
}

fn is_zero_track_id(id: &TrackId) -> bool {
    id.0 == 0
}

/// ДОРОЖКА — объект модели (Р.62, change B). Порядок вектора дорожек =
/// порядок строк сверху вниз; номер дорожки у события — её ранг среди
/// дорожек своего вида (инвариант Р.56 не менялся). Имя и высота живут
/// ЗДЕСЬ и ходят через undo; выключатели, уровень, цепочка и режим
/// наложения — содержимое проекта (решение владельца: mute и solo
/// действуют на экспорт).
///
/// Уровень — по виду: у аудио `gain_db` (дБ, 0 — как было), у видео
/// `opacity` (доля, 1 — как было); неродное поле остаётся умолчанием.
/// Служебные строки (морф, камера) несут только имя, высоту и замок.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Track {
    #[serde(skip_serializing_if = "is_zero_track_id")]
    pub id: TrackId,
    pub kind: LaneKind,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Высота строки в пикселях; 0 — расчётная.
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

/// Свойства дорожки одним куском — то, что правит окно «Свойства
/// дорожки» и команда `SetTrackProps` (цепочка — отдельной командой).
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

    /// Подпись строки: имя пользователя либо формула V1/A1/M1/C1 по рангу.
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

    /// Активные слоты цепочки (bypass отфильтрован), как у события.
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

    /// Нужна ли дорожке СТАДИЯ в аудио-миксе (сумма → цепочка → уровень →
    /// гашение): без неё события складываются в выход прежним порядком —
    /// байт-в-байт (GOLDEN_AUDIO). Соло решает вызывающий (множество по виду).
    pub fn needs_audio_stage(&self) -> bool {
        self.kind == LaneKind::Audio
            && (self.mute || self.gain_db != 0.0 || !self.active_fx().is_empty())
    }

    /// Нужна ли видеодорожке стадия (неявная лента в плане): цепочка,
    /// уровень ≠ 1 или режим ≠ Normal. Mute/solo видео — исключение из
    /// композита, стадии не требует.
    pub fn needs_video_stage(&self) -> bool {
        self.kind == LaneKind::Video
            && (self.opacity != 1.0
                || self.blend != BlendMode::Normal
                || !self.active_fx().is_empty())
    }

    /// НОВШЕСТВО Р.62 на дорожке: то, чего прежние сборки не знали вовсе и
    /// молча потеряли бы (выключатели, уровень, режим наложения, цепочка).
    /// Имя и высота сюда НЕ входят: имя и до Р.62 жило в `Track.name`
    /// файла, высота — в `ui`, и старый читатель терял бы разве что
    /// удобство. От этого предиката зависит версия файла.
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

    /// Ничего, кроме вида и id, не задано — в файл пишется только конверт.
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

/// Ключи режимов наложения для паспорта свойств (Р.62): `Choice` хранит
/// КЛЮЧ варианта, а не номер — номер поехал бы при вставке в середину.
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

/// Предел высоты строки в паспорте: 0 — «расчётная».
pub const TRACK_HEIGHT_MAX: f64 = 400.0;

const TRACK_FLAGS: &[FlagSpec] = &[
    FlagSpec { name: "mute", default: false },
    FlagSpec { name: "solo", default: false },
    FlagSpec { name: "lock", default: false },
];

const TRACK_HEIGHT_SPEC: ParamSpec =
    ParamSpec::num("height", 0.0, TRACK_HEIGHT_MAX, 0.0, 1.0, ParamUnit::Px, ParamKind::Structural);

/// ПАСПОРТ СВОЙСТВ ДОРОЖКИ (Р.62, change B). Уровень, непрозрачность,
/// высота, выключатели и режим наложения — типизированные свойства с
/// клампом: окно свойств и редактор читают пределы отсюда, а не из своих
/// литералов (прецедент Р.33 — Gain был −24…24 в одном окне и −48…24 в
/// другом). Кривых у дорожки НЕТ: её ключ — тик таймлайна, а это второй
/// домен автоматизации (следующий change), поэтому ряды объявлены
/// `ParamKind::Structural` — `auto_params()` их не отдаёт, и повесить
/// кривую нечем ни команде, ни интерфейсу.
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
        // уровень — ПО ВИДУ: у аудио дБ, у видео непрозрачность; чужого
        // ряда у дорожки нет, иначе в окне свойств висел бы мёртвый
        // контрол (правило UI 9)
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
        // у служебных строк из выключателей только замок: глушить и
        // солировать морф-слой нечего (правило UI 9)
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

/// ЧТО НЕ ПОПАДЁТ В ФАЙЛ (Р.62): имена заглушённых дорожек и активное
/// соло по видам. Mute и solo — содержимое проекта и действуют на
/// экспорт (решение владельца), значит программа ОБЯЗАНА сказать об этом
/// до старта — и окно рендера, и kt-cli одной формулировкой.
pub struct SilencedTracks {
    /// Имена заглушённых (mute) дорожек сверху вниз.
    pub muted: Vec<String>,
    /// Имена дорожек в СОЛО (пусто — соло не включено).
    pub solo: Vec<String>,
}

impl SilencedTracks {
    pub fn is_empty(&self) -> bool {
        self.muted.is_empty() && self.solo.is_empty()
    }
}

/// Собрать список по ленте (корню или вложенной).
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

/// Формула подписи дорожки без имени: V1/A1/M1/C1 по рангу.
pub fn default_lane_label(kind: LaneKind, lane: u32) -> String {
    let p = match kind {
        LaneKind::Video => "V",
        LaneKind::Audio => "A",
        LaneKind::Morph => "M",
        LaneKind::Camera => "C",
    };
    format!("{p}{}", lane + 1)
}

/// Строки по вектору дорожек: `(вид, ранг внутри вида)` — определение
/// номера дорожки (Р.56 через Р.62).
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

/// Куда встаёт НОВАЯ дорожка вида `kind` без явного места: после последней
/// своего вида, иначе после блока предшествующих видов (V..A..M..C) —
/// естественный блочный порядок прежних счётчиков, на котором стоят
/// сцены тестов и умолчание «морф-строки под аудио».
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

/// Индекс дорожки вида `kind` с рангом `lane` в векторе.
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

/// Дорожки старого файла: счётчики, `lane_order`, имена и высоты в `ui`.
/// Только ЧИТАЮТСЯ (миграция односторонняя, Р.62): писатель этих полей
/// больше не знает, а `normalize` превращает их в вектор дорожек.
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

/// Развернуть легаси-счётчики в вектор дорожек: натуральный порядок
/// V..A..M..C, затем перестановка по `lane_order` (содержимое
/// перенумеровывается ТЕМ ЖЕ отображением, что вернётся вызывающему).
/// Имена и высоты (`ui`) ключуются СТАРЫМИ номерами — вешаются до
/// перестановки, дальше едут вместе с объектом.
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

/// Нормализация display-порядка дорожек к фактическим счётчикам (тест 49):
/// дубли и несуществующие (kind, lane) выбрасываются, недостающие
/// дописываются в конец в блочном порядке V..A..M..C. Пустой вход даёт
/// дефолтный блочный порядок.
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

/// Строки идут по возрастанию номера ВНУТРИ КАЖДОГО ВИДА — инвариант
/// Р.56 (п.449). Порядок видов между собой произволен: `lane_order`
/// кодирует только чередование.
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

/// Перенумерация дорожек по видам: `map(вид, старый) = новый` (Р.56, п.449).
///
/// **Номер дорожки внутри вида = ЕЁ РАНГ среди строк этого вида.** До Р.56
/// номер и строка жили отдельно: движок складывал видео по НОМЕРУ
/// (`(Reverse(lane), start)` в `VideoRenderer::build_reel`), а экран
/// показывал по `lane_order`. Дорожка, добавленная из меню морф-строки,
/// получала самый большой номер — то есть НИЖНИЙ слой, — и рисовалась
/// третьей сверху: «положил видео под потолок, а его не видно» (претензия
/// владельца п.2 Р.56). Правило снимает расхождение в модели, не трогая
/// движок; п.50 обещал «z-order == экран» и был нарушен в двух местах UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanePerm {
    pub video: Vec<u32>,
    pub audio: Vec<u32>,
    pub morph: Vec<u32>,
    pub camera: Vec<u32>,
}

impl LanePerm {
    /// Ранг каждой дорожки среди строк своего вида. `rows` обязан быть
    /// НОРМАЛИЗОВАННЫМ (каждая дорожка ровно раз) — иначе ранги врут.
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

    /// Новый номер дорожки. Номер вне счётчика остаётся собой: перенумеровать
    /// нечего, а паниковать на чужом мусоре читатель формата не должен.
    pub fn map(&self, k: LaneKind, lane: u32) -> u32 {
        self.of(k).get(lane as usize).copied().unwrap_or(lane)
    }

    /// Переставить ключи `(вид, дорожка)` — области, строки порядка.
    pub fn remap_keys(&self, keys: &mut [(LaneKind, u32)]) {
        for e in keys.iter_mut() {
            e.1 = self.map(e.0, e.1);
        }
    }

    /// Переставить таблицы с нагрузкой: имена и высоты дорожек.
    pub fn remap_keyed<T>(&self, items: &mut [(LaneKind, u32, T)]) {
        for e in items.iter_mut() {
            e.1 = self.map(e.0, e.1);
        }
    }

    /// Ключ `(ClipKind, lane)` — так адресуются чужие OTIO-треки.
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

/// Синхронизация display-вектора при ВСТАВКЕ дорожки (lane >= at у этого
/// вида сдвинулись на +1): новая строка встаёт прямо над бывшей `at`
/// (теперь at+1), при её отсутствии — в конец (тест 49).
pub fn lane_order_insert(rows: &mut Vec<(LaneKind, u32)>, kind: LaneKind, at: u32) {
    for e in rows.iter_mut() {
        if e.0 == kind && e.1 >= at {
            e.1 += 1;
        }
    }
    let pos = rows.iter().position(|&(k, l)| k == kind && l == at + 1).unwrap_or(rows.len());
    rows.insert(pos, (kind, at));
}

/// Синхронизация display-вектора при УДАЛЕНИИ дорожки (тест 49).
pub fn lane_order_remove(rows: &mut Vec<(LaneKind, u32)>, kind: LaneKind, at: u32) {
    rows.retain(|&(k, l)| !(k == kind && l == at));
    for e in rows.iter_mut() {
        if e.0 == kind && e.1 > at {
            e.1 -= 1;
        }
    }
}

/// Подпись дорожки по формуле — см. `Project::track_label` для имени
/// пользователя (Р.62: имена живут на дорожке-объекте).
pub fn lane_label(kind: LaneKind, lane: u32) -> String {
    default_lane_label(kind, lane)
}

/// Полуоткрытый регион [start, end), start < end.
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

/// Движок смены высоты (Р.48, F-AUD-013). Выбор один на весь монтаж:
/// алгоритм — это ВЫБОР ПОД МАТЕРИАЛ, и слышен он на транзиентах.
/// `Wsola` — время-домен: удар остаётся ударом, плотная полифония даёт
/// лёгкий «шелест» на стыках зёрен. `Vocoder` — спектральный: подклад и
/// синты едут чисто, удар размазывается. Дефолт — Wsola: он дешевле и
/// безопаснее на речи, с которой чаще работают.
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
    /// Движок питча (Р.48): дефолт в файл не пишется — старые проекты
    /// байт-стабильны.
    #[serde(default, skip_serializing_if = "is_default_pitch_engine")]
    pub pitch_engine: PitchEngine,

    /// Форма отрезка у НОВЫХ ключей (Р.61).
    ///
    /// Живёт в ПРОЕКТЕ, а не в настройках программы: настройка машины
    /// дала бы «один файл — два поведения» (урок `Lock Envelopes to
    /// Events` у Vegas), и проект перестал бы описывать сам себя.
    /// Смена умолчания НЕ трогает уже поставленные ключи — для этого
    /// есть команда «применить форму ко всем отрезкам».
    /// Умолчание в файл не пишется: прежние проекты байт-стабильны.
    #[serde(default, skip_serializing_if = "SegShape::is_default")]
    pub default_shape: SegShape,
    /// Правило приведения кадра ПО УМОЛЧАНИЮ (Р.62): действует на все
    /// события с `Conform::Project`, задним числом. Умолчание «вписать»
    /// не пишется — прежние файлы байт-стабильны.
    #[serde(default, skip_serializing_if = "is_default_rule")]
    pub conform: ConformRule,
    /// Настройки выставлены ЯВНО (окно настроек применено). Пока `false`,
    /// первый клип в пустой проект задаёт растр и fps (правило 20: явное
    /// решение постановкой не трогается). `false` не пишется.
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
    /// Путь относительно файла проекта.
    pub path: String,
    /// Длительность материала, ЕСЛИ ОНА ИЗВЕСТНА (Р.63 п.2).
    ///
    /// Модель сама её не добывает и добывать не может (закон П: что лежит
    /// по адресу — свойство ИСТОЧНИКА). Её записывает тот, кто источник
    /// ОТКРЫЛ: kt-app при постановке и синке медиа, kt-cli при probe,
    /// читатель файла — из конверта. Отсюда `Option`: у генератора и
    /// ленты своей длительности нет, у не открытого ещё файла её пока
    /// никто не измерил, и в этих случаях предела у трима нет — как было
    /// всегда.
    ///
    /// Зачем модели это знать. До Р.63 `trim_max_end` у незеркального
    /// события отдавал `i64::MAX / 4`: правый край можно было утащить
    /// сколько угодно вправо, за конец материала, и никто об этом не
    /// говорил. Ровно так у владельца событие стало длиной 316.967 с при
    /// файле 226.268 с — последние 11.5 с материала «звучали» тишиной и
    /// считались underrun'ами (Р.63 п.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<Tick>,
}

impl Project {
    /// Длина материала этого медиа, если её кто-то измерил (Р.63).
    /// Единственная дверь: у ленты своей длительности нет (её содержимое
    /// кончается само), у генератора её не бывает вовсе.
    pub fn media_duration(&self, id: MediaId) -> Option<Tick> {
        self.media.iter().find(|m| m.id == id).and_then(|m| m.duration).filter(|d| d.0 > 0)
    }
}

impl MediaRef {
    /// Медиа без известной длительности — обычный случай на создании.
    pub fn new(id: MediaId, path: impl Into<String>) -> MediaRef {
        MediaRef { id, path: path.into(), duration: None }
    }

    /// То же, но длительность уже измерена открывшим источник.
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
    /// z-order для видео (меньше = выше), микс-дорожка для аудио.
    pub lane: u32,
    pub start: Tick,
    pub duration: Tick,
    pub source_in: Tick,
    pub media: MediaId,
    #[serde(default)]
    pub gain_db: f32,
    /// Реверс источника (аудио и видео читаются задом наперёд; FX-цепочка
    /// применяется ПОСЛЕ реверса). serde-default — старые файлы стабильны.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reversed: bool,
    #[serde(default)]
    pub fx: Vec<FxInstance>,
    /// Bypass-флаги слотов (п.2 v1.3): true = слот ВЫКЛЮЧЕН. Параллелен
    /// fx (индексы); короче fx = хвост включён. Пустой не сериализуется.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fx_bypass: Vec<bool>,
    /// Непрозрачность видео 0..1 (Р.28, п.12 — «уровень» события как в
    /// Vegas: линия по верху клипа, тянется мышью). Для аудио роль уровня
    /// играет `gain_db`. Умолчание 1.0 не сериализуется — старые файлы
    /// байт-стабильны.
    #[serde(default = "one_f32", skip_serializing_if = "is_one_f32")]
    pub opacity: f32,
    /// Автоматизация параметров видео-FX (Р.30): кривые по слотам,
    /// параллельно `fx` (как fx_bypass; короче fx = хвост без кривых).
    /// Домен t — прогресс клипа 0..1 (как CamMotion и камеры). Пустой
    /// вектор не сериализуется — старые файлы байт-стабильны; в .otio
    /// кривые едут внутри kt-конверта эффекта и поднимают файл до v2
    /// (старый ридер потерял бы их молча — правило Р.29).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fx_auto: Vec<Vec<ParamCurve>>,
    /// Мастер-выключатель ВСЕЙ цепочки события (Р.33, требование
    /// пользователя: «не хватает мастер-галки отключить текущий
    /// fx-chain»). Отличается от bypass слотов тем, что не трогает их
    /// состояние: включил обратно — цепочка та же. Умолчание (включено)
    /// не сериализуется, файлы байт-стабильны.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fx_off: bool,
    /// Вид генератора СО ЗНАЧЕНИЯМИ (Р.59, стадия 2).
    ///
    /// `Some` тогда и только тогда, когда `media` — генератор; сам вид
    /// обязан совпадать с видом в пути медиа (`gen_kind_path`), это
    /// проверяет `validate`. До этого раунда значения лежали в пути
    /// медиа, то есть были общими на все места использования, и «второй
    /// выпуск с другим заголовком» записать было физически некуда.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#gen: Option<crate::GeneratorKind>,
    /// Кривые параметров генератора. Домен `t` — ТИК МАТЕРИАЛА, как у
    /// `fx_auto`: ключ приклеен к материалу, а не к длине события.
    /// Разрешает их ИСТОЧНИК по своему `pts`, а не план кадра.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gen_auto: Vec<ParamCurve>,
    /// Переопределения публичных параметров ЛЕНТЫ, на которую смотрит это
    /// событие (Р.59, стадия 3).
    ///
    /// Законны только у экземпляра вложения и только на ОПУБЛИКОВАННОЕ
    /// (решение владельца: «экземпляр переопределяет только
    /// опубликованное»). Это единственное место, куда можно положить
    /// отличие экземпляра от ленты: вложение у нас — ССЫЛКА, и правка
    /// ленты доезжает до всех мест сама.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<Override>,
    /// Ведущий узел «Геометрия» (Р.62). У аудио-события не участвует.
    /// Умолчание не сериализуется — прежние файлы байт-стабильны;
    /// событие без узла тождественно узлу «следовать проекту».
    #[serde(default, skip_serializing_if = "EventGeometry::is_default")]
    pub geometry: EventGeometry,
    /// Позиция ведущего узла в цепочке: перед слотом `fx[geometry_pos]`
    /// (0 — первым, `fx.len()` — последним). Эффекты ДО него работают в
    /// пространстве источника, ПОСЛЕ — на холсте.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub geometry_pos: usize,
    /// Кривые параметров ведущего узла (домен — тик материала, как у
    /// `fx_auto`). Движок кладёт их в слот узла при материализации.
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
    /// Активная FX-цепочка (без bypass-слотов) — единственный вид цепочки,
    /// который видят движок и рендереры.
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

    /// Кривые автоматизации, выровненные с active_fx(): bypass-слоты
    /// выпадают ВМЕСТЕ со своими кривыми (Р.30) — иначе кривая приклеилась
    /// бы к чужому слоту.
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

/// Кейфрейм автоматизации параметра.
///
/// `t` — позиция В МАТЕРИАЛЕ источника (тики в системе координат
/// `source_in`), а не прогресс клипа (Р.33, решение пользователя «ключи
/// приклеены к материалу, как в Premiere/AE»). Из этого следует всё
/// остальное поведение: обрезка прячет и открывает ключи, не искажая
/// анимацию; разрез отдаёт каждой половине её кусок кривой; реверс
/// зеркалит анимацию вместе с материалом; смена скорости у аудио тянет
/// ключи за собой. Ни одна геометрическая команда ключи не трогает —
/// приклеенность обеспечена ПРЕДСТАВЛЕНИЕМ, а не кодом команд.
///
/// До Р.33 здесь была доля 0..1 от длины события: обрезка растягивала
/// анимацию, а `Split` копировал кривую целиком в обе половины.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AutoKey {
    pub t: Tick,
    pub v: f64,
    /// Форма отрезка ПОСЛЕ этого ключа (Р.61). До Р.61 форма была одна на
    /// всю кривую — булево `ParamCurve::ease`, — и ни «этот участок
    /// ступенькой», ни зигзаг им не выражались.
    ///
    /// У последнего ключа поле не действует: справа отрезка нет.
    /// Умолчание (`Smooth { tension: 1.0 }`) считает РОВНО прежний
    /// `smoothstep01` и не сериализуется — байты прежних файлов и
    /// `GOLDEN_*` не двигаются.
    #[serde(default, skip_serializing_if = "SegShape::is_default")]
    pub shape: SegShape,
}

impl AutoKey {
    /// Ключ с формой по умолчанию. Конструктор, а не литерал: поле формы
    /// добавлено позже, и сто двадцать литералов по дереву переписывать
    /// на каждое новое поле — второй способ забыть про одно из них.
    pub const fn at(t: Tick, v: f64) -> AutoKey {
        AutoKey { t, v, shape: SegShape::DEFAULT }
    }

    /// Ключ с явной формой отрезка после него.
    pub const fn shaped(t: Tick, v: f64, shape: SegShape) -> AutoKey {
        AutoKey { t, v, shape }
    }

    /// Проставить всем ключам одну форму — «применить ко всем отрезкам».
    pub fn all_shaped(mut keys: Vec<AutoKey>, shape: SegShape) -> Vec<AutoKey> {
        for k in &mut keys {
            k.shape = shape;
        }
        keys
    }

    /// Кусочно-линейная кривая одной строкой (частый случай в тестах и в
    /// путях, где сглаживание запрещено, — например у скорости варпа).
    pub fn linear(keys: Vec<AutoKey>) -> Vec<AutoKey> {
        AutoKey::all_shaped(keys, SegShape::Linear)
    }
}

/// Вставить ключ, сохранив ФОРМУ ОТРЕЗКА, который он делит (Р.61).
///
/// Единственное правило вставки на всю программу. Ключ, поставленный
/// посреди зигзага, обязан оставить зигзаг с обеих сторон: иначе клик по
/// графику молча превращал бы кривую в сглаживание. У скорости варпа то
/// же правило спасает от отказа валидации — там все отрезки прямые, и
/// новый ключ прямой по наследству, а не по счастливой случайности.
///
/// `keys` остаётся отсортированным; ключ в ту же позицию заменяется.
pub fn insert_key(keys: &mut Vec<AutoKey>, at: Tick, v: f64, default: SegShape) {
    // форма делимого отрезка = форма ближайшего ключа СЛЕВА; слева нет —
    // форма первого; ключей нет вовсе — умолчание проекта
    let shape =
        keys.iter().rev().find(|k| k.t <= at).or_else(|| keys.first()).map_or(default, |k| k.shape);
    keys.retain(|k| k.t != at);
    keys.push(AutoKey::shaped(at, v, shape));
    keys.sort_by_key(|k| k.t.0);
}

/// Профиль зигзага: чем именно дрожит отрезок.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZigProfile {
    /// Треугольник — самый дешёвый и самый «ручной» на вид.
    #[default]
    Triangle,
    /// Прямоугольник: значение прыгает между двумя уровнями.
    Square,
    /// Синус — единственный гладкий из трёх.
    Sine,
}

/// Форма отрезка между двумя соседними ключами (Р.61).
///
/// Набор ЗАКРЫТ, и у каждого вида СВОИ параметры — не одно общее число
/// «жёсткости». Так решено потому, что зигзаг признан формой отрезка
/// (решение владельца, пересмотр вердикта F-AUT-009 п.3), а ему одного
/// числа мало: нужны и периоды, и амплитуда, и профиль.
///
/// Каждая форма — ЗАМКНУТАЯ функция `(s ∈ [0,1], a, b, параметры)` без
/// состояния: на этом стоят и мемоизация пост-FX, и детерминизм.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "s", rename_all = "snake_case")]
pub enum SegShape {
    /// Прямая: значение идёт от `a` к `b` линейно.
    Linear,
    /// Третий порядок. `tension = 1` — ровно прежний `smoothstep01`;
    /// 0 вырождается в прямую, больше единицы — резче у концов.
    Smooth { tension: f64 },
    /// Ступень: до следующего ключа держится значение левого.
    Hold,
    /// Мягкий старт, резкий приход.
    EaseIn { tension: f64 },
    /// Резкий старт, мягкий приход.
    EaseOut { tension: f64 },
    /// Дрожание вокруг линии `a → b`.
    ///
    /// **Амплитуда — в ЕДИНИЦАХ ПАРАМЕТРА, а не долей перепада `b − a`**:
    /// самый частый зигзаг есть дрожание вокруг ПОСТОЯННОГО значения, где
    /// перепад равен нулю, и доля схлопнула бы фичу ровно там, где она
    /// нужна. Кламп пределами цели делает вызывающий — паспорт свойства
    /// знает их, а форма нет.
    Zigzag { periods: f64, amplitude: f64, profile: ZigProfile },
}

impl Default for SegShape {
    fn default() -> SegShape {
        SegShape::DEFAULT
    }
}

impl SegShape {
    /// Умолчание — прежнее сглаживание кривой (`ease = true`).
    pub const DEFAULT: SegShape = SegShape::Smooth { tension: 1.0 };

    pub fn is_default(&self) -> bool {
        *self == SegShape::DEFAULT
    }

    /// Значение НА ОТРЕЗКЕ: `s` — доля пути от левого ключа к правому.
    ///
    /// Концы обязаны совпадать со значениями ключей у всех форм, кроме
    /// ступени: иначе кривая рвалась бы на каждом ключе.
    pub fn at(self, s: f64, a: f64, b: f64) -> f64 {
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
            // `tension = 1` обязано давать РОВНО `s*s*(3-2s)`: на этом
            // стоят GOLDEN_* и замороженный оракул t695b
            SegShape::Smooth { tension } => {
                let k = tension.clamp(0.0, 4.0);
                // концы шкалы натяжения считаются ТОЧНО, а не формулой
                // смешивания: `s + (w − s)·1` арифметически равно `w`, но
                // в плавающей точке — нет, и умолчание разошлось бы с
                // прежним сглаживанием в последнем бите, сдвинув GOLDEN_*
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
            SegShape::Zigzag { periods, amplitude, profile } => {
                let base = a + (b - a) * s;
                // концы обязаны совпасть с ключами: окно гасит дрожание
                // на краях, иначе кривая рвалась бы на каждом ключе.
                // Парабола, а не синус: `sin(pi)` в плавающей точке равен
                // 1.2e-16, и правый конец не сходился с ключом ТОЧНО.
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

/// Кривая одного параметра одного FX-слота. Форма КАЖДОГО отрезка живёт
/// на его левом ключе (`AutoKey::shape`, Р.61); за краями — крайние
/// значения. Имя параметра — snake_case поля варианта (как в файле);
/// кривая-сирота (имя не подходит варианту) молча не действует —
/// валидация ловит её при загрузке/командах.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamCurve {
    pub param: String,
    pub keys: Vec<AutoKey>,
    /// ОТКУДА берутся значения (Р.59, п.466). Сегодня — только ключи;
    /// формула и связь с другим свойством занимают место в схеме, но
    /// отвергаются валидацией: место под них заведено ЗАРАНЕЕ, чтобы
    /// будущий вычислитель не потребовал ломать формат, а файл с
    /// невычислимой кривой падал громко, а не рисовал не то.
    ///
    /// Поле не пишется, пока источник — ключи: байты прежних файлов не
    /// двигаются ни на один символ. Сериализация кривой своя (см. ниже),
    /// поэтому атрибутов serde на полях нет.
    pub source: CurveSource,
}

/// Провод кривой в файл (Р.61, D1b).
///
/// В МОДЕЛИ форма живёт на ключе; в ФАЙЛЕ она пишется ПО СОДЕРЖИМОМУ —
/// тем же правилом, что и версия конверта. Кривая, все ключи которой
/// несут одну выразимую форму, пишется ПРЕЖНИМ полем `ease` и ни одного
/// байта не двигает; форма, от неё отличная, едет на своём ключе и
/// поднимает файл до v10.
///
/// Без этого простое удаление `ease` сдвинуло бы байты КАЖДОГО файла с
/// кривыми — то есть сломало бы «файл без новшеств остаётся прежним»
/// (t433b по всем фикстурам и golden `demo-3d.otio`, t189b).
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
    /// Форма, которую выражает прежнее булево поле `ease`, С УЧЁТОМ ИМЕНИ
    /// параметра (Р.63): `true` — третий порядок с прежним сглаживанием,
    /// `false` — прямая, а у СКОРОСТИ варпа — прямая всегда.
    ///
    /// Про скорость это не поблажка валидатору, а верный перевод. До Р.61
    /// карта варпа поля `ease` не читала ВОВСЕ (его читал только `cam_at`
    /// камеры): кривая скорости всегда звучала кусочно-линейной, каким бы
    /// ни было булево. Развернув `ease: true` в «плавно», читатель Р.61
    /// придумывал форму, которой в звуке никогда не было, — и файл
    /// владельца (14 кривых `rate`) перестал открываться вовсе
    /// (`RateShapeMustBeLinear`, Р.63 п.1). Читатель знает имя параметра,
    /// значит знает и единственно верную форму.
    ///
    /// ЯВНАЯ форма на ключе (поле `shape`, файлы с v10) тут не трогается:
    /// её наш писатель у скорости не производит, а пришедшую снаружи
    /// валидация по-прежнему отвергает громко.
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
    /// Стартовая кривая-КОНСТАНТА для параметра (Р.61).
    ///
    /// Единственное место, где решается форма НОВОЙ кривой: умолчание
    /// проекта, но у скорости варпа ВСЕГДА прямая — на кусочно-линейном
    /// профиле стоит замкнутая карта растяжения, и сглаженная стартовая
    /// кривая была бы отказом валидации в ту же секунду.
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
        // `ease = false` ровно тогда, когда ВСЯ кривая прямая: так файл
        // прежнего вида пишется прежними байтами
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

/// Источник значений кривой (Р.59, п.466).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CurveSource {
    /// Ключи, расставленные человеком, — единственный работающий сегодня.
    #[default]
    Keys,
    /// Своя детерминированная мини-формула от времени и адресуемых
    /// свойств (Р.61). Ключей у такой кривой нет.
    Formula { expr: String },
    /// Значение ведёт ДРУГОЕ свойство (Р.61).
    ///
    /// `path` — адрес источника (см. `LinkPath`). Необязательное
    /// выражение `expr` пересчитывает прочитанное значение; в нём
    /// ведущее свойство доступно под именем `src`, а время материала —
    /// под именем `t` (в секундах).
    Link {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expr: Option<String>,
        /// Пороги и снимки для НЕЧИСЛОВОЙ цели (Р.61): флаг переключается
        /// порогом, выбор/текст/медиа/цвет — снимками на отметках оси
        /// ЗНАЧЕНИЯ ведущего свойства. Ось у риги — значение, а не время,
        /// и путать их нельзя.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        stops: Vec<LinkStop>,
    },
}

/// Отметка на оси ЗНАЧЕНИЯ ведущего свойства (Р.61).
///
/// От ключа отличается осью: у ключа она ВРЕМЯ, здесь — значение ведущего.
/// Между отметками держится предыдущий снимок: рига переключает шрифт и
/// вариант ступенькой, «половины» между вариантами не существует.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkStop {
    /// Значение ведущего свойства, начиная с которого действует снимок.
    pub at: f64,
    /// Что подставить. Тип обязан совпасть с типом цели — иначе валидация.
    pub value: ParamValue,
}

impl CurveSource {
    pub fn is_keys(&self) -> bool {
        matches!(self, CurveSource::Keys)
    }

    /// Адрес ведущего свойства, если источник производный.
    pub fn link_path(&self) -> Option<&str> {
        match self {
            CurveSource::Link { path, .. } => Some(path),
            _ => None,
        }
    }

    /// Текст выражения, если он есть (у связи — необязательный).
    pub fn expr(&self) -> Option<&str> {
        match self {
            CurveSource::Formula { expr } => Some(expr),
            CurveSource::Link { expr, .. } => expr.as_deref(),
            CurveSource::Keys => None,
        }
    }
}

/// Структурная проверка кривых автоматизации клипа: ключи отсортированы
/// по материальной позиции, позиция неотрицательна, значения конечны.
/// Ключ ВНЕ куска, используемого клипом, законен — это спрятанный
/// обрезкой ключ, он вернётся, если событие снова растянуть. Имя
/// параметра НЕ проверяется жёстко: кривая-сирота (переименованный/чужой
/// параметр) молча не действует — правки схемы не валят загрузку файла.
/// Генератор на событии и его кривые (Р.59, стадия 2).
///
/// Строго, без легаси: значения есть тогда и только тогда, когда медиа —
/// генератор, вид совпадает с видом в пути, кривая-сирота ОТВЕРГАЕТСЯ.
/// У `fx_auto` сирота законна (слот мог сменить вид, а кривая — спрятанная
/// работа человека), здесь — нет: вид генератора у события ровно один и
/// меняется он вместе с медиа, так что сироте взяться неоткуда, а молча
/// не действующая кривая выглядела бы как «анимация пропала».
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

/// Источник кривой обязан РАЗРЕШАТЬСЯ (Р.61).
///
/// Формула — разбираться; у неё ключей быть не может: кривая задана текстом
/// целиком, и ключи рядом были бы вторым источником правды о значении.
/// Связь — иметь адрес по форме и, если есть, разбираемое выражение;
/// достижимость адреса проверяет `links::check_links`, потому что она
/// зависит от ленты, а не от одной кривой.
pub fn check_curve_source(id: ClipId, c: &ParamCurve) -> Result<(), ProjectError> {
    match &c.source {
        CurveSource::Keys => {}
        CurveSource::Formula { expr } => {
            crate::formula::Formula::parse(expr)
                .map_err(|e| ProjectError::CurveSourceBroken(id, e.to_string()))?;
            if !c.keys.is_empty() {
                return Err(ProjectError::CurveSourceBroken(
                    id,
                    "a formula curve carries keys: the text defines it whole".into(),
                ));
            }
        }
        CurveSource::Link { path, expr, .. } => {
            if crate::links::LinkPath::parse(path).is_none() {
                return Err(ProjectError::CurveSourceBroken(
                    id,
                    format!("link address {path:?} is not of the form <clip>/<owner>/<param>"),
                ));
            }
            if let Some(e) = expr {
                crate::formula::Formula::parse(e)
                    .map_err(|er| ProjectError::CurveSourceBroken(id, er.to_string()))?;
            }
        }
    }
    Ok(())
}

pub fn check_fx_auto(id: ClipId, auto: &[Vec<ParamCurve>]) -> Result<(), ProjectError> {
    for curves in auto {
        for c in curves {
            // Источник, который НЕ РАЗРЕШАЕТСЯ, — ГРОМКИЙ отказ (Р.59,
            // п.466; Р.61). Молчаливое «нарисуем по ключам» показало бы
            // не то, что записано в файле, а это худший сорт
            // расхождения: человек видит картинку и верит ей.
            check_curve_source(id, c)?;
            let sorted = c.keys.windows(2).all(|w| w[0].t <= w[1].t);
            let ok = c.keys.iter().all(|k| k.t.0 >= 0 && k.v.is_finite());
            if !sorted || !ok {
                return Err(ProjectError::BadFxAuto(id));
            }
            // СКОРОСТЬ принимает только прямую (Р.61). Замкнутая форма
            // обратной карты варпа стоит на кусочно-линейном профиле:
            // интеграл от третьего порядка был бы четвёртой степени, а
            // от зигзага не выражается вовсе. Отказ ГРОМКИЙ — тихое
            // выбрасывание формы показало бы в редакторе одно, а в
            // звуке другое.
            if c.param == "rate" && c.keys.iter().any(|k| k.shape != SegShape::Linear) {
                return Err(ProjectError::RateShapeMustBeLinear(id));
            }
        }
    }
    Ok(())
}

/// Значение кривой в материальной позиции `at`. За крайними ключами —
/// их значения (кламп краёв, как cam_at). Домен НЕ клампится: позиция за
/// телом клипа (ручка перехода, пре-ролл) законна и честно продолжает
/// кривую её крайним значением.
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
            // форму отрезка несёт ЛЕВЫЙ ключ — он и решает (Р.61)
            return a.shape.at(s, a.v, b.v);
        }
    }
    keys.last().unwrap().v
}

/// Применить кривые слота к эффекту в материальной позиции `at`.
/// Возвращает число сработавших кривых (сироты не считаются).
pub fn apply_fx_auto(fx: &mut FxInstance, curves: &[ParamCurve], at: Tick) -> usize {
    apply_curves(fx, curves, at)
}

/// То же для ЛЮБОГО владельца свойств (Р.59): вид генератора, переход,
/// камера. Одно правило разрешения кривой на все носители — второго
/// заводить нельзя, разойдись они, «эффект» и «генератор» показывали бы
/// разные значения на одном и том же тике.
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

/// Материальная позиция момента таймлайна внутри клипа — ЕДИНОЕ правило
/// для рендера, автоматизации и UI (движок потом ещё выравнивает её по
/// сетке кадров, `frame_step` нужен зеркалу реверса). За телом клипа
/// значение продолжается линейно и клампится нижней границей материала:
/// ручка перехода «держит кадр», как и раньше.
pub fn clip_material_at(
    source_in: Tick,
    start: Tick,
    duration: Tick,
    reversed: bool,
    frame_step: i64,
    t: Tick,
) -> Tick {
    if reversed {
        // зеркало: последний кадр клипа показывает source_in, первый —
        // конец использованного куска источника (минус шаг кадра)
        Tick((source_in.0 + duration.0 - frame_step - (t.0 - start.0)).max(source_in.0))
    } else {
        Tick((source_in.0 + (t.0 - start.0)).max(0))
    }
}

/// Обратная к `clip_material_at`: момент таймлайна, в который событие
/// играет материальную позицию `mat`. Пара обязана жить РЯДОМ — они верны
/// только вместе, а `clip_material_at` ещё и клампит края.
///
/// Р.37: раньше обратная была открыта в kt-engine и умела варп скорости
/// (`material_at`/`timeline_at_material`). Теперь перевод «материал ↔
/// таймлайн» НЕ ЗАВИСИТ от цепочки эффектов: скорость сжимает материал, но
/// не ось автоматизации (решение пользователя, п.1 Р.37).
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

/// Кейфреймы slowed&reverb СТАРОГО образца (`SlowedReverb.automation`,
/// доли материала) → обычные кривые `rate`/`mix` в тиках материала.
/// Возвращает true, если клип менялся. Зовётся ридерами формата: с Р.33
/// автоматизация у всех эффектов одна, и собственного домена у slowed
/// больше нет.
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
        // доля материала у slowed считалась ОТ source_in — зеркало реверса
        // ей не нужно (в отличие от видео-кривых прежнего домена)
        let key = |u: f64, v: f64| {
            AutoKey::at(material_from_progress(u, c.source_in, c.duration, false), v)
        };
        for (param, pick) in [
            ("rate", Box::new(|k: &SlowedKeyframe| k.rate) as Box<dyn Fn(&SlowedKeyframe) -> f64>),
            ("mix", Box::new(|k: &SlowedKeyframe| k.mix as f64)),
        ] {
            if c.fx_auto[i].iter().any(|c| c.param == param) {
                continue; // кривая нового домена уже есть — она главнее
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

/// Кейфреймы камеры → общие кривые (Р.34 К3). Домен меняется с доли
/// события на тики ОТ ЕГО НАЧАЛА, форма — с «строка со всеми полями» на
/// «кривая на параметр»; `ease` переезжает с события на каждую кривую.
/// Конверсия односторонняя (у независимых кривых ключи стоят в разное
/// время, обратно в строки они не собираются) — поэтому `keyframes`
/// становится read-only легаси-входом, как `SlowedReverb.automation`.
pub fn migrate_legacy_camera(c: &mut CameraEvent) -> bool {
    if c.keyframes.is_empty() || !c.auto.is_empty() {
        return false;
    }
    let keys = std::mem::take(&mut c.keyframes);
    let dur = c.duration.0.max(1) as f64;
    let ease = c.ease;
    /// (имя параметра, как достать его из снимка камеры)
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
    // базовые параметры берутся у ПЕРВОГО ключа: кривая перекрывает их
    // целиком, но неавтоматизируемые поля (target/ortho) должны остаться
    // теми, что были в анимации
    c.cam = keys[0].cam;
    for (name, pick) in params {
        // ровные кривые не заводим: у камеры семь параметров, и семь
        // прямых линий в редакторе — мусор
        let vals: Vec<f64> = keys.iter().map(|k| pick(&k.cam)).collect();
        if vals.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-12) {
            continue;
        }
        // прежнее булево сглаживание камеры становится ОДНОЙ формой на
        // все отрезки: значения после конверсии обязаны совпасть побитово
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

/// CamMotion → узел «Геометрия» + кривые (Р.34 К2, с Р.62 — вместо Pan).
/// У узла те же три параметра (положение и зум), то есть «CamMotion с
/// кейфреймами» — это «Геометрия с кривыми»; конверсия убирает
/// дублирующий механизм анимации, а не портирует его. Пан прежней рамки
/// (−1..1 в пределах свободного места) переводится в положение (доли
/// холста): `pos = −pan·(1 − 1/zoom)/2` при зуме ключа.
///
/// Тонкости, которые легко потерять: (1) пустые кейфреймы у CamMotion
/// означали НЕ статику, а v1-пресет «наезд 1.0 → 1.35» — он
/// материализуется явно, иначе старые проекты потеряли бы движение;
/// (2) `time_scale` разгонял чтение кейфреймов (`u_eff = u · scale`),
/// поэтому позиция ключа делится на него; (3) `ease` жил на эффекте, а в
/// новом домене он у каждой кривой свой.
/// Пан прежней рамки (−1..1 в пределах свободного места при зуме z) →
/// положение узла «Геометрия» в долях холста.
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
        let _ = mode; // режим прежней рамки: все три давали один кадр (F-MED-004)
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
                continue; // кривая нового домена уже есть — она главнее
            }
            // то же правило, что у камеры: булево сглаживание легаси
            // становится одной формой на все отрезки
            let shape = if ease { SegShape::DEFAULT } else { SegShape::Linear };
            c.fx_auto[i].push(ParamCurve {
                param: param.into(),
                keys: keys
                    .iter()
                    .map(|k| {
                        // u_eff = u·scale ⇒ ключ достигается при u = t/scale
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

/// Позиция ключа из СТАРОГО домена (доля 0..1 от длины события) в
/// материальные тики. Домен сменился в Р.33; ридер формата зовёт это
/// для файлов до v3, иначе анимация старых проектов уехала бы.
///
/// `mirror` = true для кривых, чья доля считалась по прогрессу ТАЙМЛАЙНА
/// (видео-`ParamCurve` до Р.33): у реверсированного клипа она зеркальна
/// материалу. У кейфреймов slowed&reverb доля с самого начала считалась
/// ПО МАТЕРИАЛУ, им зеркало не нужно.
pub fn material_from_progress(u: f64, source_in: Tick, duration: Tick, mirror: bool) -> Tick {
    let u = u.clamp(0.0, 1.0);
    let u = if mirror { 1.0 - u } else { u };
    Tick(source_in.0 + (u * duration.0 as f64).round() as i64)
}

/// Типизированные FX (не opaque JSON — валидация на загрузке, тест 12).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FxInstance {
    Gain {
        db: f32,
        /// Баланс каналов −1…+1 (Р.33 С5). 0 — прежний путь байт-в-байт:
        /// множители обоих каналов ровно 1.0.
        #[serde(default, skip_serializing_if = "is_zero")]
        pan: f32,
    },
    /// Сила наложения СЛОЯ (Р.38 п.12): смешение входа цепочки с её
    /// результатом. Не пользовательский эффект — его нет в реестре и не
    /// выбрать в пикере; его дописывает `MorphPatch::apply_to`, когда у
    /// морфера сила < 1 или заданы фейды.
    ///
    /// Почему операция ЦЕПОЧКИ, а не поле клипа: «непрозрачность слоя
    /// бесплатно работает как ползунок силы эффекта — 60 % это кроссфейд
    /// обработанной картинки с необработанной» (разведка Р.36,
    /// `docs/features/morpher/analogues/premiere-adjustment-layer.md`).
    /// Как слот цепочки она бесплатно получает автоматизацию (фейды — это
    /// её кривая), входит в `fx_chain_hash` (иначе 60 % и 100 %
    /// схлопнулись бы в один кадр пост-FX кэша) и едет через копипаст.
    LayerMix {
        amount: f64,
        /// Индекс слота, С КОТОРОГО начинается вклад этого слоя. Смешение
        /// идёт с состоянием ДО него, а не со входом всей цепочки: иначе
        /// сила слоя откатывала бы заодно СОБСТВЕННЫЕ эффекты события,
        /// к которым слой отношения не имеет.
        #[serde(default)]
        from: u32,
    },
    SlowedReverb {
        /// Базовая скорость (0.25–32); автоматизация её перекрывает.
        rate: f64,
        /// Доля wet-сигнала 0..1.
        mix: f32,
        reverb_delay_ms: f32,
        /// Демпфирование ВЧ в comb-линиях 0..1 (старый reverbDamp).
        #[serde(default = "default_damp")]
        damp: f32,
        /// Обратная связь comb-линий 0..0.98 (старый reverbDecay).
        #[serde(default = "default_decay")]
        decay: f32,
        /// Множитель wet-выхода (старый reverbGain).
        #[serde(default = "default_wet_gain")]
        wet_gain: f32,
        /// Пред-задержка входа реверба, мс (Р.33 С5): хвост отстаёт от
        /// звука. 0 — прежнее поведение байт-в-байт; структурная (меняет
        /// длину линии), поэтому автоматизации не подлежит.
        #[serde(default, skip_serializing_if = "is_zero")]
        predelay_ms: f32,
        /// Как добывать промежуточный кадр, когда эффект стоит на ВИДЕО
        /// (Р.37 п.2). У аудио-экземпляра поля не видно — оно ему не
        /// значит ничего. Умолчание не сериализуется: файлы байт-стабильны.
        #[serde(default, skip_serializing_if = "is_default_frame_mode")]
        frames: FrameMode,
        /// Ширина стерео-хвоста 0..1. 1 — прежний стереореверб.
        #[serde(default = "default_one_f32", skip_serializing_if = "is_one_f32")]
        width: f32,
        /// Keyframe-автоматизация: t — позиция В МАТЕРИАЛЕ ИСТОЧНИКА
        /// (кадры от source_in, нормированные длительностью клипа), НЕ
        /// выходное время. Кейфрейм приклеен к материалу: ранние правки
        /// не сдвигают материал поздних точек (п.1 v1.3, тест 31).
        /// Между точками линейная интерполяция, за краями — крайние.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        automation: Vec<SlowedKeyframe>,
    },
    /// Резонансный срез (Р.47): LP/HP/BP, RBJ-биквад.
    Filter {
        #[serde(default)]
        mode: FilterMode,
        #[serde(default = "default_cutoff_hz")]
        cutoff_hz: f32,
        #[serde(default = "default_resonance")]
        resonance: f32,
    },
    /// Трёхполосный эквалайзер (Р.47): полка + пик + полка.
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
    /// Мягкое насыщение (Р.47): softclip с компенсацией уровня.
    Distortion {
        #[serde(default = "default_drive_db")]
        drive_db: f32,
        #[serde(default = "default_half_f32")]
        tone: f32,
        #[serde(default = "default_one_f32", skip_serializing_if = "is_one_f32")]
        mix: f32,
    },
    /// Lo-fi деградация (Р.47): битовая глубина + удержание сэмпла.
    Bitcrusher {
        #[serde(default = "default_bits")]
        bits: f32,
        #[serde(default = "default_crush_rate")]
        rate_hz: f32,
        #[serde(default = "default_one_f32", skip_serializing_if = "is_one_f32")]
        mix: f32,
    },
    /// Ширина стерео m/s (Р.47): 0 — моно, 1 — как записано, 2 — шире.
    StereoWidth {
        #[serde(default = "default_one_f32")]
        width: f32,
    },
    /// Тремоло (Р.47): LFO громкости; фаза — интеграл кривой частоты по
    /// позиции клипа (детерминизм без прогрева).
    Tremolo {
        #[serde(default = "default_trem_rate")]
        rate_hz: f32,
        #[serde(default = "default_trem_depth")]
        depth: f32,
    },
    /// Автопан (Р.47): LFO баланса, закон панорамы — как у Gain.pan.
    AutoPan {
        #[serde(default = "default_one_f32")]
        rate_hz: f32,
        #[serde(default = "default_autopan_depth")]
        depth: f32,
    },
    /// Вибрато (Р.47): модулируемая задержка, 100 % wet (с сухим это был
    /// бы chorus) — параметра mix нет осознанно.
    Vibrato {
        #[serde(default = "default_vib_rate")]
        rate_hz: f32,
        #[serde(default = "default_vib_depth")]
        depth_ms: f32,
    },
    /// Хорус (Р.47): 1-3 голоса на модулируемых задержках, стерео —
    /// сдвиг фазы LFO на 90° в правом канале.
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
    /// Флэнжер (Р.47): короткая модулируемая задержка с обратной связью
    /// (отрицательная — инверсия фазы).
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
    /// Фейзер (Р.47): каскад 1-полюсных allpass со свипом 200..2000 Гц.
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
    /// Эхо (Р.47): кольцо на максимум, время — дробное чтение с
    /// тейп-глайдом (не структурное), фильтр в петле — хвост темнеет.
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
    /// Компрессор (Р.47): peak-детектор со стерео-линком.
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
        /// КЛЮЧ сайдчейна (Р.65): дорожка, по которой считает детектор.
        /// `None` — обычный компрессор, и тогда поле не пишется вовсе:
        /// файл без сайдчейна остаётся байт-в-байт прежним.
        ///
        /// Адрес — `TrackId`, а НЕ номер дорожки: номер это ранг строки, и
        /// вставка дорожки выше перевесила бы ключ на чужой сигнал (тот же
        /// урок, что у ключа FX-состояния в Р.62).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<TrackId>,
    },
    /// Гейт (Р.47): быстрый детектор + гистерезис −6 дБ + hold.
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
    // ---- видео-FX (порт bc_fx.cpp, п.25; параметры и дефолты v1) ----------
    /// Базовая цветокоррекция (Р.47, F-CLR-007): яркость/контраст/
    /// насыщенность/оттенок. Поточечный — поглощается в слитый гео-проход.
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
    /// Две краски по люме (Р.47): сепия/тонировка/инверсия — точки этого
    /// пространства. Поточечный.
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
    /// Постеризация по каналам (Р.47). Поточечный (внутри гео-серии
    /// суперсемплинг смягчает ступени — принято, THRESHOLD).
    Posterize {
        #[serde(default = "default_levels")]
        levels: u32,
    },
    /// Виньетка (Р.47): затемнение к краям, координата своей стадии —
    /// внутри цепочки зумится вместе с картинкой. Поточечный.
    Vignette {
        #[serde(default = "default_half_f32")]
        strength: f32,
        #[serde(default = "default_vig_radius")]
        radius: f32,
        #[serde(default = "default_vig_soft")]
        softness: f32,
    },
    /// Посадка за четыре угла (Р.47, F-FX-004): гомография quad→quad,
    /// главный инструмент «экрана в кадре». Гео — конкатенируется.
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
    /// Нерезкое маскирование по мипу (Р.47, F-FX-001): источник +
    /// (источник − размытая база)·amount. Барьер.
    Sharpen {
        #[serde(default = "default_one_f32")]
        amount: f32,
        #[serde(default = "default_two_u32")]
        radius: u32,
    },
    /// Мозаика (Р.47, F-FX-001): целочисленная копия центра ячейки.
    /// Барьер.
    Pixelate {
        #[serde(default = "default_twelve_u32")]
        size: u32,
    },
    /// Хроматическая аберрация (Р.47, F-FX-002): радиальный масштаб
    /// каналов — R к центру, B от центра, G на месте. Барьер.
    ChromaticAberration {
        #[serde(default = "default_chroma_amount")]
        amount: f32,
    },
    /// Радиальное размытие (Р.47, F-FX-003): зум-стрик или вращение
    /// вокруг центра, константные 16 тапов на обоих бэкендах. Барьер.
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
    /// Тень под слоем (Р.47, F-FX-003): размытая альфа со сдвигом,
    /// источник OVER тень. Барьер; вход без альфы — passthrough.
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
    /// Полутоновый растр (Р.47, F-FX-001): точки по люме на повёрнутой
    /// сетке; узор живёт в логических координатах. Барьер.
    Halftone {
        #[serde(default = "default_eight_u32")]
        size: u32,
        #[serde(default = "default_halftone_angle")]
        angle: f32,
    },
    /// Look-ahead лимитер (Р.48, F-AUD-002): потолок гарантирован окном
    /// упреждения — пик виден заранее, поэтому атака мгновенная и без
    /// искажений. Для остального движка задержка НУЛЕВАЯ: цепочка читает
    /// источник вперёд, выход остаётся выровненным.
    Limiter {
        #[serde(default = "default_limiter_threshold")]
        threshold_db: f32,
        #[serde(default = "default_limiter_ceiling")]
        ceiling_db: f32,
        /// Окно упреждения. СТРУКТУРНЫЙ параметр: длина кольца — часть
        /// устройства узла, смена пересобирает процессор (Р.48, п.337).
        #[serde(default = "default_limiter_lookahead")]
        lookahead_ms: f32,
        #[serde(default = "default_limiter_release")]
        release_ms: f32,
    },
    /// Смена высоты без смены длительности (Р.48, F-AUD-013). Движок
    /// (WSOLA или фазовый вокодер) — НАСТРОЙКА ПРОЕКТА: выбор один на
    /// весь монтаж. Нейтраль (ноль полутонов и центов, без кривых) —
    /// байт-в-байт прежний звук.
    PitchShift {
        #[serde(default, skip_serializing_if = "is_zero")]
        semitones: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        cents: f32,
    },
    /// Линзовая дисторсия (Р.47, F-FX-002): полином k1·r²+k2·r⁴ —
    /// бочка/подушка. Гео (Warp-прим), конкатенируется.
    LensDistort {
        #[serde(default = "default_lens_k1")]
        k1: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        k2: f32,
    },
    /// Водоворот (Р.47): поворот, затухающий к радиусу. Гео.
    Twirl {
        #[serde(default = "default_twirl_angle")]
        angle: f32,
        #[serde(default = "default_twirl_radius")]
        radius: f32,
    },
    /// Круговая волна (Р.47): r += amp·sin(k·r − φ), phase анимируема.
    /// Гео.
    Ripple {
        #[serde(default = "default_ripple_amount")]
        amount: f32,
        #[serde(default = "default_ripple_freq")]
        freq: f32,
        #[serde(default, skip_serializing_if = "is_zero")]
        phase: f32,
    },
    /// Калейдоскоп (Р.47): зеркальный повтор сектора. Гео.
    Kaleidoscope {
        #[serde(default = "default_kaleido_segments")]
        segments: u32,
        #[serde(default, skip_serializing_if = "is_zero")]
        angle: f32,
    },
    /// Цилиндр (Р.47, F-FX-004): изгиб по X, края остаются краями. Гео,
    /// категория 3D.
    Cylinder3d {
        #[serde(default = "default_half_f32")]
        curvature: f32,
    },
    /// Узел «Геометрия» (Р.62): кроп (доли входа), положение (доли холста),
    /// зум, поворот вокруг якоря; вне рамки — ПРОЗРАЧНО. Ведущий узел
    /// события живёт в `ClipEvent.geometry` и несёт ещё правило
    /// приведения; этот вариант — дополнительные узлы из каталога и форма,
    /// в которой ведущий доезжает до kt-fx.
    Geometry {
        /// Ведущий узел события (Р.62): материализован движком из
        /// `ClipEvent.geometry`, несёт правило приведения через контекст
        /// kt-fx. В цепочке модели (`fx`) не хранится — там всегда `false`.
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
    /// Пан/зум по кейфреймам прогресса клипа (smoothstep при ease).
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
        /// Вертикальный радиус (Р.33 С5). `0` — «как горизонтальный»:
        /// умолчание не пишется в файл и даёт прежнее размытие
        /// байт-в-байт. Раздельные оси дают «смаз» по одной оси.
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
        /// Подавление засветки ключевым цветом (Р.33 С5): доминирующий
        /// канал ключа притягивается к соседним. 0 — прежнее поведение.
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        spill: f64,
    },
    Edges {
        #[serde(default = "default_edges_threshold")]
        threshold: u32,
        /// Толщина линии = шаг выборки Собеля (Р.33 С5). 1 — прежнее
        /// ядро 3×3 байт-в-байт.
        #[serde(default = "default_one_u32", skip_serializing_if = "is_one_u32_v")]
        thickness: u32,
        /// Цвет линий: серый выход домножается на него. 255,255,255 —
        /// прежний белый.
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
        /// Размер зерна в пикселях (Р.33 С5). 1 — прежнее пиксельное
        /// зерно байт-в-байт; больше — крупная «плёночная» крупа.
        #[serde(default = "default_one_u32", skip_serializing_if = "is_one_u32_v")]
        size: u32,
        /// Цветной шум: у каждого канала своё смещение. `false` —
        /// прежний монохромный (один сдвиг на все три канала).
        #[serde(default, skip_serializing_if = "is_false")]
        color: bool,
    },
    Vhs {
        #[serde(default = "default_rgb_shift")]
        rgb_shift: u32,
        #[serde(default = "default_bleed")]
        bleed: f64,
        /// Шаг скан-линий (Р.33 С5): каждая N-я строка гасится. 4 —
        /// прежнее поведение байт-в-байт, 0 — скан-линий нет вовсе.
        #[serde(default = "default_scan_step", skip_serializing_if = "is_four_u32")]
        scan_step: u32,
        /// Дрожание строк по горизонтали, доля кадра. 0 — прежний
        /// ровный кадр.
        #[serde(default, skip_serializing_if = "is_zero_f64")]
        jitter: f64,
    },
    Glow {
        #[serde(default = "default_glow_strength")]
        strength: f64,
        /// Радиус свечения = делитель мипа (Р.33 С5). 4 — прежний.
        #[serde(default = "default_glow_radius", skip_serializing_if = "is_four_u32")]
        radius: u32,
        /// Порог яркости РАЗМЫТОГО кадра: ниже него свечения нет. 0 —
        /// порога нет (прежнее поведение байт-в-байт).
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        threshold: u32,
        /// Оттенок свечения; 255,255,255 — прежний нейтральный.
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
    // ---- 3D-эффекты (Р.29: категория Video3d; с Р.44 считаются на обоих бэкендах) ----
    /// Слой в перспективе: кадр клипа — квад в 3D-сцене, рендерится
    /// активной (или прибитой) камерой в размер проекта. Все дефолты —
    /// тождество: без камеры и с нулевыми параметрами кадр совпадает с
    /// кадром без эффекта. Единицы заморожены форматом: позиция в мировых
    /// px (X вправо, Y вниз, Z вглубь, начало — центр кадра проекта),
    /// повороты в градусах (порядок ZXY, как orientation в AE), якорь в
    /// долях слоя (0.5, 0.5 — центр).
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
        /// None — активная по времени камера (умолчание, как в AE);
        /// Some(id) — прибитая камера (камер может быть несколько — Р.29).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        camera: Option<CameraId>,
    },
}

/// Класс эффекта (Р.29): свойство КОДА варианта, не данных — в формат
/// не сериализуется. Video3d — КАТЕГОРИЯ (пикер FX, «3D»), а не признак
/// бэкенда: с Р.44 Pan3d считается и на CPU (обратная гомография в слитом
/// гео-проходе, kt-fx::geom), отказ экспорта GpuRequired снят.
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

    /// Аудио-эффект? (видео-FX живут в той же цепочке клипа, но
    /// обрабатываются VideoRenderer'ом)
    pub fn is_audio(&self) -> bool {
        self.class() == FxClass::Audio
    }

    /// Сила наложения слоя — не фильтр, а правило СМЕШЕНИЯ входа цепочки
    /// с её выходом. Как и варп, достаётся ОБОИМ видам: слой на 30 %
    /// обязан одинаково приглушать и картинку, и звук («видео и аудио
    /// сразу» — решение пользователя по п.12).
    pub fn is_layer_mix(&self) -> bool {
        matches!(self, FxInstance::LayerMix { .. })
    }

    /// Стабильный тег варианта (совпадает с serde-`kind`). ИСЧЕРПЫВАЮЩИЙ
    /// match — единственный компайл-замок реестра (Р.47, п.326): новый
    /// вариант не соберётся, пока не назван, а тест `all()` сверяет реестр
    /// с множеством тегов.
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

    /// Реестр ВСЕХ видов эффектов с дефолтными параметрами (аналог
    /// `TransitionKind::ALL`, Р.47 п.326). Константой не выразить —
    /// варианты несут Vec (keyframes, automation), — поэтому OnceLock.
    /// По нему строятся параметризованные тесты (полнота спек, golden- и
    /// конформанс-циклы): вид, не попавший сюда, виден сразу.
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

    /// Эффект ВАРПИТ время: потребление источника перестаёт быть равным
    /// длине события. Такой клип морфер-слой не режет вовсе (находка 2
    /// Р.35) — линейно посчитанный `source_in` куска указал бы не туда.
    pub fn warps_time(&self) -> bool {
        matches!(self, FxInstance::SlowedReverb { .. })
    }

    /// Параметры варианта (Р.33): ОДИН источник правды для автоматизации,
    /// клампа и UI. Раньше их описывали три ручные таблицы (`param_specs`,
    /// `set_param`, `get_param`), которые надо было править синхронно;
    /// теперь все три генерятся из одной декларации `fx_params!`.
    pub fn param_specs(&self) -> &'static [ParamSpec] {
        fx_specs(self)
    }

    /// Только автоматизируемые параметры (`ParamKind::Smooth`). Кривая на
    /// структурном параметре бессмысленна: его смена пересобирает DSP
    /// (реверб пересоздаёт comb-линии), а автоматизация обязана быть
    /// плавной — поэтому UI такие параметры не предлагает.
    pub fn auto_params(&self) -> impl Iterator<Item = &'static ParamSpec> {
        self.param_specs().iter().filter(|s| s.kind == ParamKind::Smooth)
    }

    pub fn param_spec(&self, name: &str) -> Option<&'static ParamSpec> {
        self.param_specs().iter().find(|s| s.name == name)
    }

    /// Установить параметр по имени. Значение клампится диапазоном спеки;
    /// целочисленные поля округляются. false — имя не подходит варианту
    /// (кривая-сирота).
    pub fn set_param(&mut self, name: &str, v: f64) -> bool {
        let Some(spec) = self.param_spec(name) else { return false };
        fx_set_param(self, name, v.clamp(spec.min, spec.max))
    }

    /// Текущее значение параметра по имени (для UI автоматизации).
    pub fn get_param(&self, name: &str) -> Option<f64> {
        fx_get_param(self, name)
    }

    /// Смешать два состояния ОДНОГО вида: числовые параметры линейно, всё
    /// остальное — от ближайшего конца (Р.54, п.433).
    ///
    /// Зачем это модели, а не UI: ось силы в пикере ведёт курсор МЕЖДУ
    /// соседними вариантами, и «сколько тут ставят» показывается
    /// промежуточными кадрами. Объявлять у каждого вида «главный
    /// параметр» (как `Option`-скимминг у FCP) для этого не нужно и
    /// вредно — у `VHS` и `Color Grade` таких параметров четыре, и любой
    /// выбор одного был бы спорным; смешивание двух КУРИРОВАННЫХ точек
    /// даёт осмысленный ход по всем ручкам сразу.
    ///
    /// Нечисловое (галки, режимы, камера, кейфреймы) интерполировать
    /// нельзя, поэтому основой берётся БЛИЖАЙШИЙ конец: до середины —
    /// `self`, дальше — `other`. Значит и переключается оно ровно в
    /// середине, а не «когда-нибудь».
    ///
    /// Разные виды не смешиваются: возвращается ближайший конец как есть.
    /// `t` зажимается в 0..1, `t == 0`/`t == 1` дают КОНЦЫ БАЙТ-В-БАЙТ —
    /// на этом стоит правило «клик по ячейке даёт именно её вариант».
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

/// Как параметр ведёт себя при автоматизации.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// Меняется плавно на ходу — автоматизируется.
    Smooth,
    /// Смена пересобирает состояние DSP (геометрия линий задержки) —
    /// автоматизации не подлежит, руками менять можно.
    Structural,
}

/// Единица измерения — для подписи и шага в UI (перевод подписи живёт в
/// каталогах kt-app: kt-model про язык интерфейса ничего не знает).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamUnit {
    None,
    Db,
    /// Доля 0..1.
    Norm,
    /// Множитель (скорость, зум).
    Ratio,
    Px,
    Deg,
    Ms,
    /// Частота, герцы (Р.47: фильтры/EQ/LFO).
    Hz,
    /// Полутон (Р.48: питч).
    St,
    /// Компонента цвета 0..255.
    Rgb8,
    /// Счётное число (колонки, строки).
    Count,
    /// Доля 0..1, ПОКАЗЫВАЕМАЯ процентами (Р.53, п.415). Хранится и
    /// сериализуется по-прежнему долей — меняется только показ: «кегль
    /// 0.070» пользователю ничего не говорит, «7.0 %» говорит всё.
    /// Пересчёт живёт в ОДНОМ месте (`widgets::param_value`).
    Percent,
}

/// ТИП свойства (Р.59, п.466). До этого раунда свойством было только
/// ЧИСЛО, а всё остальное — флаги, списки, цвет (лежит тройкой чисел),
/// текст, ссылка на медиа — рисовалось руками в своём окне и не
/// участвовало ни в автоматизации, ни в копипасте, ни в пресетах.
///
/// Кривыми анимируются `Num` и `Color` (поканально); остальное меняется
/// СТУПЕНЬКОЙ — промежуточных значений у выбора и у текста не бывает.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "ParamTypeRepr", into = "ParamTypeRepr")]
pub enum ParamType {
    #[default]
    Num,
    Flag,
    /// Цвет rgba8. Каналы — обычные числовые ряды владельца
    /// (`<stem>_r/_g/_b/_a`), поэтому кривая цвета это четыре обычные
    /// кривые, а не новый вид ключа (см. `color_groups`).
    Color,
    /// Выбор из вариантов. Хранится КЛЮЧОМ варианта, а не номером:
    /// номер поехал бы при добавлении варианта в середину, а ключи у нас
    /// уже несут перечисления в файле.
    Choice {
        variants: std::borrow::Cow<'static, [&'static str]>,
    },
    Text {
        multiline: bool,
    },
    Media,
}

impl ParamType {
    /// Можно ли вешать кривую. Ступенчатым типам нельзя: между «Слева» и
    /// «Справа» нет половины.
    pub fn curvable(&self) -> bool {
        matches!(self, ParamType::Num | ParamType::Color)
    }

    /// Список вариантов `Choice` В ФАЙЛ НЕ ЕДЕТ (Р.59, стадия 3): он
    /// принадлежит ЦЕЛИ и читается у неё живьём — по той же причине, по
    /// которой не хранится значение шаблона. Сохранённая копия разошлась
    /// бы с целью молча, как только в меню добавится пункт.
    ///
    /// Тот же ЛИ это тип, безотносительно набора вариантов у `Choice`.
    ///
    /// Публикация запоминает тип цели, а список вариантов у цели может
    /// пополниться новой сборкой — сравнивай мы `==`, публикация умирала
    /// бы от добавления пункта в меню.
    pub fn same_family(&self, other: &ParamType) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

/// Представление `ParamType` в файле: только СЕМЕЙСТВО.
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

/// ЗНАЧЕНИЕ свойства (Р.59, стадия 3).
///
/// До этого раунда значением было только число: флаг, выбор, цвет (лежит
/// тройкой чисел), текст и ссылка на медиа рисовались руками в своих
/// окнах и свойствами не были. Публичный параметр композиции обязан
/// уметь нести любое из шести — иначе «заголовок выпуска» и «логотип»
/// выразить нечем.
///
/// Теги serde СТАБИЛЬНЫ: значение уезжает в файл (переопределение у
/// экземпляра), и переименование варианта сломало бы чужие проекты.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v", rename_all = "snake_case")]
pub enum ParamValue {
    Num(f64),
    Text(String),
    /// rgba8. Каналы в цели — обычные числовые ряды `<stem>_r/_g/_b/_a`.
    Color([u8; 4]),
    Flag(bool),
    /// КЛЮЧ варианта, а не номер: номер поехал бы при добавлении
    /// варианта в середину.
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

/// Цветовое свойство, ВЫВЕДЕННОЕ из числовых рядов владельца (Р.59).
///
/// Цвет не объявляется отдельным рядом и не поедет в формат: все 69
/// компонент уже названы `<stem>_{r,g,b}` с единицей `Rgb8`, и группа
/// выводится из них. Поэтому ручные списки цветов в интерфейсе — второй
/// источник того же факта, и они удаляются.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorGroup {
    /// Основа имени: `color`, `a`, `tl`, `tint`, …
    pub stem: String,
    /// Есть ли у группы четвёртая компонента `<stem>_a`.
    pub alpha: bool,
}

/// Вывести цветовые группы из паспорта. Ряд `<stem>_r` с единицей `Rgb8`
/// и парой `_g`/`_b` рядом — это цвет; `_a` добавляет альфу.
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

/// Имя свойства: у встроенных видов — статическая строка из объявления,
/// у публичных параметров композиции и у плагинов — текст, придуманный
/// АВТОРОМ в рантайме (Р.59).
pub type ParamName = std::borrow::Cow<'static, str>;

/// Описание одного параметра эффекта.
///
/// `Copy` снят в Р.59: имя стало `Cow` — паспорт обязан строиться в
/// РАНТАЙМЕ, иначе ни публичный параметр композиции (имя придумывает
/// автор и переименовывает его), ни плагин (паспорт известен только при
/// загрузке) выразить нечем. Все места читают спеку по ссылке.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamSpec {
    /// Имя поля snake_case — оно же ключ в файле и в `ParamCurve.param`.
    pub name: ParamName,
    /// Тип значения. У встроенных рядов — `Num` (их объявляет макрос).
    pub ty: ParamType,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    /// Шаг для колеса/драга в UI.
    pub step: f64,
    pub unit: ParamUnit,
    pub kind: ParamKind,
    /// Готовая подпись. ПУСТО у встроенных — их подпись берётся из
    /// каталога переводов по имени; НЕПУСТО у динамических владельцев,
    /// чьё имя придумал человек и в каталог попасть не может.
    pub label: ParamName,
}

impl ParamSpec {
    /// Числовая спека — форма, которую генерит макрос и которой описаны
    /// все встроенные ряды.
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

/// Приведение f64 к типу поля варианта.
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

/// ЕДИНСТВЕННАЯ декларация параметров эффектов: спеки, запись и чтение
/// генерятся из неё, поэтому рассинхрон трёх таблиц невозможен.
/// Тот же паспорт параметров, но для СТРУКТУРЫ (Р.34 К3: камере нужны
/// спеки, чтобы попасть в общую сетку полей и в общий виджет кривых).
/// Одна декларация — спеки, чтение и запись с клампом.
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
            // Свойства-ВЫБОРЫ структуры (Р.63). Без них ветки пусты, и
            // поведение побайтово равно прежнему: `extra_specs` отдаёт
            // пустой вектор, а значение идёт умолчаниями трейта.
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

        /// Запись по ИНДЕКСУ ряда (Р.59): порядок рядов — порядок
        /// объявления, он же порядок `param_specs()`, поэтому индекс
        /// стабилен по построению. Нужен компилированному плану
        /// автоматизации: он не ищет параметр сравнением строк на каждый
        /// кадр.
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

        /// Запись по имени. КЛАМП ЖИВЁТ ЗДЕСЬ (Р.59): до этого раунда
        /// он стоял в обёртках `FxInstance::set_param` и
        /// `TransitionKind::set_param`, а у генераторов обёртки не было
        /// вовсе — кегль титра принимал любое число из файла и из
        /// команды. Теперь предел один на всех владельцев и на оба пути
        /// адресации, именной и индексный (t642).
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

/// Паспорт ФЛАГОВ вида — из ОДНОЙ декларации, рядом с `params_for!`
/// (Р.53, п.414). Генерит спеки, чтение и запись: рассинхрона трёх
/// таблиц не бывает, а полноту (не забыт ли bool-поле) сторожит тест,
/// который сверяет флаги с serde-представлением вида.
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
    // ---- аудио (Р.33: автоматизируются общим механизмом) ----------------
    Gain {
        db: f32, -60.0, 12.0, 0.0, 0.1, Db, Smooth;
        pan: f32, -1.0, 1.0, 0.0, 0.01, Norm, Smooth;
    },
    LayerMix {
        amount: f64, 0.0, 1.0, 1.0, 0.01, Norm, Smooth;
    },
    SlowedReverb {
        // диапазон скорости — RATE_CLAMP карты варпа (warp.rs, Р.39:
        // раньше он жил в kt-fx и был обещанием через границу крейта).
        // Со знаком и с нулём (Р.57, решение владельца): минус идёт по
        // материалу НАЗАД, ноль — стоп-кадр.
        rate: f64, -32.0, 32.0, 1.0, 0.01, Ratio, Smooth;
        mix: f32, 0.0, 1.0, 0.0, 0.01, Norm, Smooth;
        // смена длины comb-линий пересоздаёт реверб — не автоматизируется
        reverb_delay_ms: f32, 1.0, 500.0, 60.0, 1.0, Ms, Structural;
        damp: f32, 0.0, 0.99, 0.2, 0.01, Norm, Smooth;
        decay: f32, 0.0, 0.98, 0.84, 0.01, Norm, Smooth;
        wet_gain: f32, 0.0, 2.0, 1.0, 0.01, Ratio, Smooth;
        // длина линии пред-задержки — та же геометрия, что reverb_delay_ms
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
        // число голосов пересоздаёт линии — кривой не подлежит
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
        // число каскадов пересоздаёт состояние — кривой не подлежит
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
    // ---- видео 2D --------------------------------------------------------
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
    // ---- видео 3D (Р.29) -------------------------------------------------
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

// Параметры переходов — та же декларация, что у эффектов: UI полей,
// кламп и (в будущем) автоматизация приходят к ним бесплатно.
// Единицы камеры заморожены форматом (Р.29): мировые px и градусы ZXY.
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

/// Режим кроп-рамки пан/зума (v1: fit/fill/stretch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanMode {
    #[default]
    Fit,
    Fill,
    Stretch,
}

/// Правило приведения кадра источника к холсту (Р.62, F-MED-004):
/// вписать с ПРОЗРАЧНЫМИ полями / заполнить с обрезкой / растянуть /
/// как есть (1:1 пиксель по центру, крупнее холста — за край).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConformRule {
    #[default]
    Fit,
    Fill,
    Stretch,
    AsIs,
}

/// Правило у СОБЫТИЯ: следовать проекту либо своё. Уровень «проект +
/// исключение у события» — решение владельца Р.62: смена умолчания проекта
/// переписывает всех, кто ему следует, и не трогает явные.
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
    /// Разрешить правило события правилом проекта.
    pub fn resolve(self, project: ConformRule) -> ConformRule {
        match self {
            Conform::Project => project,
            Conform::Fit => ConformRule::Fit,
            Conform::Fill => ConformRule::Fill,
            Conform::Stretch => ConformRule::Stretch,
            Conform::AsIs => ConformRule::AsIs,
        }
    }

    /// КЛЮЧ варианта для паспорта свойства (`Choice` хранит ключ, а не
    /// номер). Совпадает с тем, что пишет serde, — второго написания у
    /// одного факта быть не должно.
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

/// ВЕДУЩИЙ узел «Геометрия» видео-события (Р.62): правило приведения +
/// кроп (доли источника), положение (доли холста, 0 — по центру), зум,
/// поворот (градусы) вокруг якоря (доли рамки). Стоит в цепочке ПЕРВЫМ
/// (`ClipEvent.geometry_pos`), перетаскивается, не удаляется — как
/// Pan/Crop в Vegas.
///
/// Поле события, а НЕ слот `fx`: так индексы слотов и адреса кривых/связей
/// не сдвигаются от того, что у события появился узел; движок
/// материализует его в цепочку на позиции `geometry_pos` при сборке.
/// Дополнительные узлы того же вида — `FxInstance::Geometry` (без правила).
/// Место под маску (отдельный change): соседнее поле, порядок внутри узла —
/// кроп → маска → размещение.
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

    /// Параметры тождественны (правило не в счёт): узел ничего не
    /// двигает, только приводит кадр по правилу.
    pub fn params_identity(&self) -> bool {
        self.node().geometry_is_identity()
    }

    /// Тот же узел как слот цепочки (без правила) — форма, которую видит
    /// kt-fx. Ведущий узел материализуется движком в цепочку на позиции
    /// `geometry_pos`.
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

    /// Обратно из слота (правило остаётся своё).
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

/// Ключи правила приведения для паспорта свойства (Р.63): правило —
/// такое же публикуемое свойство ведущего узла, как зум.
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

/// Тип аудио-фильтра (Р.47): срез сверху/снизу/полоса.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterMode {
    #[default]
    Lowpass,
    Highpass,
    Bandpass,
}

/// Как добывать кадр, которого в источнике нет (Р.37 п.2): растянутое
/// видео просит материал между кадрами. `Nearest` — держим ближайший
/// (детерминизм, ноль лишнего декода); `Blend` — линейно смешиваем два
/// соседних. Третьим сюда встанет fruc_vulkan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameMode {
    #[default]
    Nearest,
    Blend,
}

/// Кейфрейм CamMotion по прогрессу клипа t∈[0,1].
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

/// Камера-событие на камерной дорожке (Р.29, образец — морф-дорожка):
/// событие само является и камерой, и её «включением по времени» (как
/// camera layer в AE). Камер может быть несколько — активную в момент t
/// выбирает `Project::active_camera_at` детерминированным правилом.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraEvent {
    pub id: CameraId,
    /// Индекс камерной дорожки (0..camera_lanes).
    pub lane: u32,
    pub start: Tick,
    pub duration: Tick,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Базовые параметры (кейфреймы их перекрывают).
    pub cam: CameraParams,
    /// ЛЕГАСИ-вход (Р.34 К3): анимация по прогрессу события t∈[0,1].
    /// Читается из старых файлов и переводится в `auto`; новые кривые
    /// сюда не пишутся.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keyframes: Vec<CameraKeyframe>,
    /// Кривые параметров камеры — ТОТ ЖЕ механизм, что у эффектов
    /// (Р.34 К3). Позиция ключа — тик ОТ НАЧАЛА события: перенос камеры
    /// везёт анимацию с собой, а правка длины открывает/прячет ключи,
    /// как обрезка клипа («ничего не отрастает само», п.20 р.28).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto: Vec<ParamCurve>,
    #[serde(default = "default_true")]
    pub ease: bool,
}

/// Параметры камеры. Дефолт — «50 мм»: перспектива с fov ≈ 39.6°, авто-
/// дистанция (pos_z = 0 трактуется движком как −(H/2)/tan(fov/2)) — слой
/// в z=0 с тождественным Pan3d рендерится 1:1, 2D-проекты не меняются
/// ни на пиксель. Единицы — мировые px и градусы (ZXY), как у Pan3d.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraParams {
    #[serde(default)]
    pub pos_x: f64,
    #[serde(default)]
    pub pos_y: f64,
    /// 0 = «авто» (дефолтная дистанция 50 мм).
    #[serde(default)]
    pub pos_z: f64,
    /// Точка интереса (two-node camera, как в AE). None — one-node:
    /// смотрит вдоль +Z.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<(f64, f64, f64)>,
    /// Доворот ПОСЛЕ target (AE orientation), градусы, ZXY.
    #[serde(default)]
    pub rot_x: f64,
    #[serde(default)]
    pub rot_y: f64,
    #[serde(default)]
    pub rot_z: f64,
    /// Вертикальный FOV в градусах.
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

/// Кейфрейм камеры по прогрессу события t∈[0,1].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraKeyframe {
    pub t: f64,
    pub cam: CameraParams,
}

impl CameraEvent {
    /// Параметры камеры в момент таймлайна t (образец cam_at из kt-fx):
    /// прогресс события 0..1, линейная интерполяция скалярных полей между
    /// кейфреймами, smoothstep при ease, за краями — крайние. `target`
    /// интерполируется, только когда задан у обеих точек сегмента (иначе
    /// держится значение левой); `ortho` — от левой точки.
    pub fn params_at(&self, t: Tick) -> CameraParams {
        if !self.auto.is_empty() {
            // общий механизм (Р.34 К3): кривая на параметр, позиция —
            // тик от начала события
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

/// Активная камера из списка (правило Р.29 — см. Project::active_camera_at;
/// pub-функция, чтобы движок применял ТО ЖЕ правило к своему снапшоту).
pub fn active_camera_in(cameras: &[CameraEvent], t: Tick) -> Option<&CameraEvent> {
    cameras
        .iter()
        .filter(|c| c.start.0 <= t.0 && t.0 < (c.start + c.duration).0)
        .max_by_key(|c| (std::cmp::Reverse(c.lane), c.start.0, c.id.0))
}

/// Вертикальный FOV умолчания — умеренная перспектива в духе 50-мм
/// камеры AE; точная эквивалентность миллиметрам не обещается (киношную
/// арифметику в модель не тащим).
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
    /// Конструктор с дефолтными параметрами реверба (для тестов/простых случаев).
    /// Конструкторы эффектов с УМОЛЧАНИЯМИ добавленных полей (Р.33 С5).
    /// Струкурные литералы в вызывающем коде заменены на них, чтобы
    /// следующий параметр не требовал править полсотни мест.
    /// Дефолтные конструкторы видов без своего конструктора (Р.47,
    /// п.326): нужны реестру `all()`; литералы в тестах идут через них.
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

    /// Дополнительный узел «Геометрия» с зумом вокруг центра (замена
    /// прежнего «Pan: растянуть, зум z»).
    pub fn geometry_zoom(zoom: f64) -> FxInstance {
        let mut g = EventGeometry { zoom, ..Default::default() }.node();
        if let FxInstance::Geometry { lead, .. } = &mut g {
            *lead = false;
        }
        g
    }

    /// Дополнительный узел «Геометрия» с положением (доли холста) и зумом.
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

    /// Дополнительный (не ведущий) узел «Геометрия» из каталога.
    pub fn geometry_node() -> FxInstance {
        let mut g = EventGeometry::default().node();
        if let FxInstance::Geometry { lead, .. } = &mut g {
            *lead = false;
        }
        g
    }

    /// Ведущий ли это узел «Геометрия» (материализованный движком).
    pub fn is_lead_geometry(&self) -> bool {
        matches!(self, FxInstance::Geometry { lead: true, .. })
    }

    /// Узел «Геометрия» с тождественными параметрами. Правило приведения
    /// сюда не входит: тождествен ли ВЕДУЩИЙ узел, решает kt-fx по виду
    /// источника и холсту (`geometry_is_identity_in`).
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
    #[error("project conform rule must be a rule, not «follow project»")]
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
    // --- вложенные секвенции (Р.50) ---
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
    // --- раскрытие ленты на месте (Р.55, С5) ---
    #[error("clip {0:?} crossed the expanded band boundary: an event cannot change reels")]
    BandEscape(ClipId),
    // --- дорожки-объекты (Р.62, change B) ---
    #[error("reel {0:?}: {1:?} track {2} does not exist")]
    TrackMissing(Option<MediaId>, LaneKind, u32),
    #[error("track {0:?}: curves on a track are not supported yet (timeline-time domain)")]
    TrackCurvesUnsupported(TrackId),
    // --- источник значений кривой (Р.59, п.466) ---
    #[error(
        "clip {0:?}: this curve is driven by a formula or a link, and nothing evaluates those yet"
    )]
    CurveSourceUnsupported(ClipId),
    #[error("clip {0:?}: the curve source cannot be resolved: {1}")]
    CurveSourceBroken(ClipId, String),
    // --- связь свойств (Р.61) ---
    #[error("{0}")]
    BadLink(#[from] crate::links::LinkError),
    // --- форма отрезка (Р.61) ---
    #[error(
        "clip {0:?}: the speed curve takes the linear shape only — the closed-form warp map \
         stands on a piecewise-linear profile"
    )]
    RateShapeMustBeLinear(ClipId),
    // --- параметры генератора на событии (Р.59, стадия 2) ---
    #[error("clip {0:?} sits on a generator but carries no generator values")]
    GenMissing(ClipId),
    #[error("clip {0:?} carries generator values but its media is a file")]
    GenOnFile(ClipId),
    #[error("clip {0:?}: generator values are of a different kind than the media says")]
    GenKindMismatch(ClipId),
    #[error("clip {0:?}: generator curve '{1}' names no parameter of this kind")]
    GenAutoOrphan(ClipId, String),
    // --- публичные параметры композиции (Р.59, стадия 3) ---
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

/// Полная проверка перехода против проекта: стык, умещаемость, минимум,
/// уникальность стыка и непересечение окон на дорожке. `ignore` — id самого
/// перехода при SetTransition (не сравнивать с собой; Add передаёт None,
/// но self-id тоже пропускается). Используется и validate(), и check_on
/// команд (тест 135).
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
    // фейд (Р.25): один конец — пустота; офсет в её сторону телом не
    // ограничен, стык берётся у живого клипа. С Р.51 (п.399) пара может
    // быть и НАЛОЖЕННОЙ «лесенкой» — тогда окно = зона перекрытия, а
    // офсеты не используются вовсе (мёртвый груз до возврата встык).
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
                // полное вложение, равные старты, обратный порядок или
                // разрыв — у такой пары нет ни направления, ни окна
                return Err(ProjectError::TransitionNotAtJoint(t.id));
            }
        }
        (Some(a), None) => (a.kind, a.lane, a.start + a.duration),
        (None, Some(b)) => (b.kind, b.lane, b.start),
        (None, None) => return Err(ProjectError::TransitionWithoutClips(t.id)),
    };
    if staggered {
        // окно задают ТЕЛА пары; третий клип в нём ломает не переход, а
        // окружение (1/N-зоны остаются без переходов — как и трое без
        // перехода дают 1/N, п.162)
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
    // кривые параметров вида — те же правила, что у кривых эффектов
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

    /// Дорожка КОРНЯ по id (рабочий вид: содержимое текущей ленты стоит корнем).
    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    /// Дорожка в любом месте документа (корень или лента).
    pub fn find_track(&self, id: TrackId) -> Option<&Track> {
        self.track(id)
            .or_else(|| self.sequences.iter().find_map(|s| s.tracks.iter().find(|t| t.id == id)))
    }

    pub fn track_label(&self, kind: LaneKind, lane: u32) -> String {
        self.track_at(kind, lane)
            .map(|t| t.label(lane))
            .unwrap_or_else(|| default_lane_label(kind, lane))
    }

    /// Дописать дорожки вида до `n` штук (в конец строк).
    pub fn ensure_tracks(&mut self, kind: LaneKind, n: u32) {
        while self.lanes(kind) < n {
            let at = natural_insert_pos(&self.tracks, kind);
            self.tracks.insert(at, Track::new(TrackId(0), kind));
        }
        self.assign_track_ids();
    }

    /// Первый свободный id дорожки во ВСЁМ документе.
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

    /// Назначить id всем дорожкам без него (и разрешить дубли): корень,
    /// затем ленты, по порядку строк — детерминированно. Идемпотентно.
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

    /// Вставить дорожку вида `kind` с рангом `at` (строка встаёт прямо над
    /// бывшей `at`, при её отсутствии — в конец); содержимое вида с
    /// номером ≥ at сдвигается на +1. Возвращает id новой дорожки.
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
        // строка встаёт над бывшей `at`; ранга `at` нет — в конец блока
        // своего вида (естественный порядок), а не в конец списка
        let pos = track_index(&self.tracks, kind, at)
            .unwrap_or_else(|| natural_insert_pos(&self.tracks, kind));
        self.tracks.insert(pos, Track::new(id, kind));
        id
    }

    /// Удалить дорожку вида `kind` с рангом `at` вместе с её содержимым;
    /// номера выше съезжают на −1.
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

    /// Сколько дорожек каждого вида ТРЕБУЕТ содержимое корня.
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

    /// Активная камера в момент t: событие, чьё [start, start+duration)
    /// накрывает t; приоритет — меньшая lane (как z-order видео), при
    /// равной — больший start («сверху тот, кто позже», правило Vegas
    /// Р.28/п.177), затем больший id. Правило детерминировано — рендер
    /// от него зависит (тест 188). None — камер нет, движок берёт
    /// дефолтную «50 мм» (тождество для Pan3d с дефолтами).
    pub fn active_camera_at(&self, t: Tick) -> Option<&CameraEvent> {
        active_camera_in(&self.cameras, t)
    }

    /// Порядок строк сверху вниз, нормализованный к фактическим
    /// счётчикам дорожек. Единственный способ узнать, что «ниже».
    pub fn rows(&self) -> Vec<(LaneKind, u32)> {
        rows_of_tracks(&self.tracks)
    }

    /// Все занятые имена ОДНОГО пространства: области и метки-диапазоны.
    /// Пустые имена меток законны и в пространство не входят.
    pub fn taken_names(&self) -> Vec<String> {
        self.areas
            .iter()
            .map(|a| a.name.clone())
            .chain(self.markers.iter().filter(|m| !m.name.is_empty()).map(|m| m.name.clone()))
            .collect()
    }

    /// Область по имени.
    pub fn area(&self, name: &str) -> Option<&MarkedArea> {
        self.areas.iter().find(|a| a.name == name)
    }

    /// Метка-диапазон по имени (точечные метки окна не задают).
    pub fn named_range(&self, name: &str) -> Option<TickRange> {
        self.markers
            .iter()
            .find(|m| m.name == name && m.duration.0 > 0)
            .map(|m| TickRange::new(m.t, m.t + m.duration))
    }

    /// Индекс строки в порядке дорожек; None — такой дорожки нет.
    pub fn row_of(&self, key: (LaneKind, u32)) -> Option<usize> {
        self.rows().iter().position(|&k| k == key)
    }

    /// Переставить строки — ЕДИНСТВЕННАЯ дверь перестановки (Р.56, п.449;
    /// Р.62: строки — вектор дорожек).
    ///
    /// `order` называет строки ТЕКУЩИМИ номерами; дорожки-объекты
    /// переставляются в этот порядок, содержимое перенумеровано так, что
    /// номер внутри вида равен рангу. Дубли и небылицы выбрасываются,
    /// недостающие дописываются в конец. Возвращает перестановку номеров.
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
            // область адресуется СЕМАНТИЧЕСКИМИ ключами дорожек — значит едет
            // с перенумерацией, иначе «слой на этих дорожках» уехал бы на чужие
            for a in &mut self.areas {
                perm.remap_keys(&mut a.lanes);
            }
        }
        perm
    }

    /// Дорожки из легаси-полей КОРНЯ и всех лент (счётчики, `lane_order`,
    /// `ui.lane_names/lane_heights`) — на ЧТЕНИИ, односторонне (Р.62);
    /// затем дописать дорожки до содержимого. Возвращает перестановку
    /// КОРНЯ: читателю формата она нужна, чтобы переставить свои
    /// побочные таблицы (чужие OTIO-треки в kt-otio).
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

    /// Клипы после применения morpher'ов — ЕДИНСТВЕННАЯ точка выдачи клипов
    /// движку, экспорту и отрисовке (v1 effectiveClipForRender/renderClips):
    /// движок про morpher не знает. Порядок применения детерминирован —
    /// по возрастанию MorphId (порядок создания; в v1 порядок вектора был
    /// негласным — дефект D11 разведки). Несколько morpher'ов на клип
    /// применяются кумулятивно.
    pub fn effective_clips(&self) -> Vec<ClipEvent> {
        // быстрый путь: GOLDEN держится на нём
        if self.morphers.is_empty() && self.sequences.is_empty() {
            return self.clips.clone();
        }
        let out = self.morph_pass(self, &self.rows());
        if self.sequences.is_empty() {
            return out;
        }
        self.inline_pass(out)
    }

    /// Морфер-слои ОДНОЙ ленты. Вынесено из `effective_clips` (Р.50):
    /// внутрь вложения можно класть всё, включая слои, — значит и этот
    /// проход обязан уметь работать по ленте, а не только по корню.
    fn morph_pass(&self, reel: &dyn Reel, rows: &[(LaneKind, u32)]) -> Vec<ClipEvent> {
        // Производные источники разрешаются ЗДЕСЬ — в единственной двери,
        // через которую содержимое ленты выходит к движку (Р.61). Лента без
        // связей и формул проходит ЗАИМСТВОВАННОЙ: графа не строится вовсе,
        // байты прежние.
        let step = 90_000 / self.settings.fps.max(1) as i64;
        let resolved = crate::links::resolve_derived(reel.clips(), step);
        let ms_all = reel.morphers();
        if ms_all.is_empty() {
            return resolved.into_owned();
        }
        // шаг кадра нужен для перевода ключей слоя в материал цели
        let frame_step = 90_000 / self.settings.fps.max(1) as i64;
        let mut ms: Vec<&MorpherEvent> = ms_all.iter().filter(|m| m.enabled).collect();
        // Порядок наложения — СНИЗУ ВВЕРХ (нижний слой первым, как
        // adjustment layer в AE); внутри строки прежнее правило по id.
        ms.sort_by_key(|m| (std::cmp::Reverse(m.row_in(rows)), m.id));
        let mut out = Vec::with_capacity(resolved.len());
        for c in resolved.iter() {
            let wins: Vec<Vec<TickRange>> =
                ms.iter().map(|m| m.windows_on(c, self, reel, rows)).collect();
            let body = TickRange::new(c.start, c.start + c.duration);
            // ТОЧКИ РЕЗА ДАЮТ НЕ ВСЕ СЛОИ (Р.55, С2). Режут только те, чей
            // вклад нельзя выключить кривой силы: подмена источника и варп
            // времени в патче. Слой из одних эффектов кусков не порождает —
            // он ложится на всё тело, а время задают ВОРОТА. Набор реза
            // по-прежнему ОБЩИЙ на всех режущих: иначе они резали бы куски
            // друг друга.
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
                // кусок №0 держит id оригинала — правило `Split`; при
                // отсутствии резки это тождественная копия клипа
                let id = if idx == 0 { c.id } else { slice_id(c.id, idx as u32) };
                let mut piece = clip_slice(c, w, id);
                for (m, ws) in ms.iter().zip(&wins) {
                    if m.cuts_target(c) {
                        // режущий слой ложится на кусок ЦЕЛИКОМ и только
                        // если кусок целиком внутри его окна (как было)
                        if ws.iter().any(|x| x.start <= w.start && w.end <= x.end) {
                            m.apply_to(&mut piece, frame_step, &[w]);
                        }
                        continue;
                    }
                    // ВОРОТА считаются ПО КУСКУ, а не по исходному клипу:
                    // куски породили ДРУГИЕ слои, и окно, посчитанное от
                    // начала тела, уехало бы на длину отрезанного.
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

    /// Поменять местами содержимое корня и содержимое ленты `seq`
    /// (Р.50, В1). Повторный вызов с тем же адресом возвращает как было.
    ///
    /// Этим и сделан ВХОД ВНУТРЬ. Альтернатива — провести «текущую ленту»
    /// через все шестьдесят рук команд (`p.clips`, `p.transitions`,
    /// `p.video_lanes`, …): пропусти одну — и правка молча уедет в корень,
    /// пока человек смотрит внутрь. Здесь же ни одна команда о вложениях
    /// не знает вовсе, и знать не должна: она правит ЛЕНТУ, а какая лента
    /// под руками — вопрос контекста.
    ///
    /// Обмен держит СОЮЗ содержимого неизменным: все события документа
    /// по-прежнему видны, просто в других контейнерах. Поэтому проверки
    /// вида «этот файл ещё кем-то используется» остаются верными во время
    /// правки внутри ленты.
    pub fn swap_reel(&mut self, seq: MediaId) {
        let Some(i) = self.sequences.iter().position(|s| s.id == seq) else { return };
        let s = &mut self.sequences[i];
        std::mem::swap(&mut self.clips, &mut s.clips);
        std::mem::swap(&mut self.transitions, &mut s.transitions);
        std::mem::swap(&mut self.morphers, &mut s.morphers);
        std::mem::swap(&mut self.cameras, &mut s.cameras);
        std::mem::swap(&mut self.tracks, &mut s.tracks);
        std::mem::swap(&mut self.overlap_layers, &mut s.overlap_layers);
        // Публичный набор — СОДЕРЖИМОЕ ленты: его цели это её клипы, а
        // они сейчас переезжают на место корня. Не поменяй мы и его —
        // публикация указывала бы на клипы, которых в этой ленте больше
        // нет, и `reconcile_published` снёс бы её на первой же команде.
        std::mem::swap(&mut self.published, &mut s.published);
    }

    /// Документ, в котором ЛЕНТА `seq` стоит КОРНЕМ (Р.56, п.452).
    ///
    /// Нужен офлайн-счёту по ленте: превью её содержимого считается тем же
    /// кодом композиции, что и всё остальное (инвариант 7), а этому коду
    /// нужен проект с лентой на месте корня. Приём тот же, что у
    /// `reel_document`, только лента называется явно, а не берётся из
    /// «где стоит человек»: службе превью человек не указ.
    ///
    /// Сама лента из списка убирается — иначе она ссылалась бы на себя и
    /// `validate` увидел бы цикл. Остальные ленты остаются: внутри могут
    /// стоять их экземпляры.
    pub fn reel_as_root(&self, seq: MediaId) -> Option<Project> {
        self.sequences.iter().find(|s| s.id == seq)?;
        let mut p = self.clone();
        p.swap_reel(seq);
        p.sequences.retain(|s| s.id != seq);
        Some(p)
    }

    /// Клипы ОДНОЙ ленты после морфер-слоёв, БЕЗ раскрытия вложений
    /// (Р.50, волна Б). `None` — корень.
    ///
    /// Этим движок строит ДЕРЕВО рендереров: вложение доходит до него
    /// одним событием и получает собственный узел плана — только так
    /// «эффект на РЕЗУЛЬТАТ» и становится выразимым. Раскрытие
    /// (`effective_clips`) остаётся эталоном-оракулом: с ним готовое
    /// поддерево сверяется байт-в-байт (закон Э).
    pub fn effective_reel(&self, seq: Option<MediaId>) -> Vec<ClipEvent> {
        self.resolve_reel(seq, &[]).clips
    }

    /// То же, но под НАБОРОМ ПЕРЕОПРЕДЕЛЕНИЙ экземпляра (Р.59, стадия 3).
    ///
    /// Пустой набор обязан идти путём, побайтово равным прежнему: ни
    /// клона ленты, ни клона переходов и камер (`Cow::Borrowed`). На этом
    /// стоят GOLDEN_VIDEO/GOLDEN_AUDIO и «ноль цены без использования»
    /// (t673/t676).
    ///
    /// Переопределения ложатся **ДО** морфер-слоёв — см. модуль
    /// `published`: адрес автора это структура ленты, какой он её видит.
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
            // у корня публичных параметров не бывает (он никому не
            // экземпляр), и попасть сюда можно только ошибкой вызывающего
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

    /// ЖИВОЕ значение цели публикации — то самое «значение шаблона»,
    /// которое мы НЕ ХРАНИМ (Р.59, стадия 3).
    ///
    /// `seq` — лента, внутри которой живёт адрес. `None` — корень (у него
    /// публикаций не бывает, но чтение адреса законно: так окно
    /// публикации спрашивает цель до того, как ручка заведена).
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
                    // шаблон в шаблоне: значение внутреннего экземпляра —
                    // его переопределение, а нет его — значение шаблона
                    // ЕГО ленты, тем же кодом этажом ниже
                    ParamOwner::Nested { pub_id } => self.instance_value(c, *pub_id),
                    // у аудио-события узла геометрии нет вовсе: адрес
                    // «не существует», и валидация скажет это словом
                    ParamOwner::Geometry => {
                        (c.kind == ClipKind::Video).then(|| c.geometry.get_value(name))?
                    }
                    ParamOwner::Transition { .. } | ParamOwner::Camera { .. } => None,
                }
            }
        }
    }

    /// Тип цели вживую — им валидация ловит «цель сменила вид».
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

    /// ПАСПОРТ свойства по адресу — зеркало `addr_type` (Р.60).
    ///
    /// Пределы, шаг и умолчание публичной ручки берутся ОТСЮДА, а не из
    /// нуля: «значения min max всегда в 0 проставляются по умолчанию» —
    /// пустое сужение рисовалось нулём, хотя паспорт цели свои пределы
    /// знает. `match` по владельцу исчерпывающий — новый владелец не
    /// соберётся, пока сюда не дописана ветка.
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
                        // Наружу едет паспорт ЦЕЛИ внутренней ручки,
                        // СУЖЕННЫЙ её же пределами: потребитель внешней
                        // ленты не вправе получить больше, чем разрешил
                        // автор внутренней.
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

    /// Значение ШАБЛОНА объявленной ручки: читается у цели живьём.
    pub fn published_value(&self, seq: MediaId, id: PubId) -> Option<ParamValue> {
        let p = self.sequence(seq)?.published.iter().find(|p| p.id == id)?;
        self.addr_value(Some(seq), &p.target)
    }

    /// ДЕЙСТВУЮЩЕЕ значение ручки у экземпляра: переопределение, а нет
    /// его — значение шаблона. Ровно это показывает окно параметров
    /// композиции и ровно это едет в картинку.
    pub fn instance_value(&self, inst: &ClipEvent, id: PubId) -> Option<ParamValue> {
        if let Some(o) = inst.overrides.iter().find(|o| o.pub_id == id) {
            return Some(o.value.clone());
        }
        self.published_value(inst.media, id)
    }

    /// Раскрытие вложений (Р.50, п.383).
    ///
    /// Вложение с ПУСТОЙ цепочкой семантически ТОЖДЕСТВЕННО своему
    /// развёрнутому содержимому: настройки наследуются, промежуточного
    /// растра не возникает. Поэтому картинка и звук получаются здесь, без
    /// единой правки плана, воркеров, кэша и GPU — а сам инлайн становится
    /// ЭТАЛОНОМ-ОРАКУЛОМ для настоящего поддерева (закон Э, тесты t434-t436).
    ///
    /// Чего он НЕ делает и не притворяется, что делает: цепочка, варп и
    /// реверс на самом экземпляре применялись бы к каждому куску отдельно,
    /// а не к собранной картинке, — это другой результат, поэтому команды
    /// их на вложение просто НЕ ПУСКАЮТ (правило 16). Уровень и усиление
    /// домножаются: на неперекрывающемся содержимом это то же самое, а
    /// перекрытие — уже дело настоящего поддерева.
    fn inline_pass(&self, clips: Vec<ClipEvent>) -> Vec<ClipEvent> {
        // Свободные номера кусков: морфер-проход уже мог порезать сам
        // экземпляр, и новые куски обязаны встать ВЫШЕ его номеров —
        // иначе два разных куска получили бы один адрес.
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
        // РЕВЕРС И СКОРОСТЬ НЕ РАСКРЫВАЮТСЯ (Р.55, С4). Раскрытие —
        // эталон-оракул закона Э и то, что печатает `kt-cli probe`, и оно
        // законно ровно там, где вложение ТОЖДЕСТВЕННО своему содержимому.
        // Зеркало и ресемплер применяются к СОБРАННОМУ материалу: развернув
        // их по кускам, мы посчитали бы другое — это тот же аргумент, что у
        // закона Г для уровня (t438). Такое вложение остаётся событием.
        if inst.reversed || crate::chain_warps_time(&inst.active_fx(), &inst.active_fx_auto()) {
            out.push(inst.clone());
            return;
        }
        if depth >= MAX_NEST_DEPTH {
            return; // validate такого не пропустит; здесь — страховка
        }
        // окно экземпляра в координатах содержимого
        let win = TickRange::new(inst.source_in, inst.source_in + inst.duration);
        // ПЕРЕОПРЕДЕЛЕНИЯ ЭКЗЕМПЛЯРА (Р.59, стадия 3) применяются и здесь.
        // Раскрытие — эталон-оракул закона Э и то, что печатает `kt-cli
        // probe`: покажи оно ШАБЛОН, оракул сверял бы поддерево не с тем,
        // а probe отвечал бы на жалобу «тут другой заголовок» чужим
        // текстом.
        let inner = self.resolve_reel(Some(seq.id), &inst.overrides).clips;
        // Адреса кусков считаются от НАСТОЯЩЕГО клипа-экземпляра: так
        // `same_clip` доводит кусок обратно до события, по которому человек
        // кликнул, а `slice_id` не переполняется поверх собственного тега.
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
            // Дорожка содержимого ложится ПОД дорожку экземпляра — ровно
            // так же, как её кладёт `expand_instance`: два пути обязаны
            // давать одну картинку, иначе «свернул — развернул» перестаёт
            // быть тождеством.
            piece.lane = inst.lane + child.lane;
            piece.opacity *= inst.opacity;
            piece.gain_db += inst.gain_db;
            piece.link_group = inst.link_group;
            self.inline_one(&piece, depth + 1, next, out);
        }
    }

    /// Перенос параметров генератора из ПУТИ МЕДИА на СОБЫТИЕ (Р.59,
    /// стадия 2) — ЕДИНСТВЕННОЕ место миграции.
    ///
    /// Файлы Р.52..Р.58 держали значения строкой `gen:text:t=…`, то есть
    /// делили их между всеми местами использования. Здесь путь
    /// укорачивается до ВИДА, а значения раздаются событиям — каждому своя
    /// копия, ровно та, что была. Прежний режим анимации титра
    /// (`am=`/`ams=`) превращается в кривые тем же кодом, что и пресет в
    /// окне (`text_anim_curves`): разойдись они, старый проект поехал бы
    /// не так, как та же кнопка, нажатая руками.
    ///
    /// Событие, у которого значения уже есть, не трогается: это файл
    /// нового формата, и путь у него уже короткий.
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
                // «блок не шире кадра»: точный пролёт знает только
                // раскладка, а её здесь нет — и это ровно тот случай,
                // когда приблизиться честнее, чем тянуть в модель kt-text.
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

    /// Зоны клипа, внутри которых граница морфер-слоя ставиться не может
    /// (находка 1 Р.35): окна переходов и наезды соседей по дорожке. Резка
    /// там сломала бы НЕ цель, а окружение — у видео порядок композиции
    /// `(Reverse(lane), start)`, и кусок с бо́льшим start вылез бы над
    /// соседом; у аудио `overlap_weight` считает рампу из геометрии клипов.
    /// Нормализация после загрузки: число дорожек подтягивается вверх до
    /// фактических lane клипов (файлы, писанные до появления полей).
    pub fn normalize(&mut self) -> LanePerm {
        // ВАЖНО: stable-сортировка и дедуп по времени — при двух метках на
        // один тик с разными именами unstable дал бы недетерминированный
        // порядок и сломал байт-стабильность проекта (тест 12)
        self.markers.sort_by_key(|m| m.t);
        self.markers.dedup_by_key(|m| m.t);
        // параметры генератора переехали на событие (Р.59, стадия 2):
        // чинится на ЧТЕНИИ, потому что файлы Р.52..Р.58 держали их в
        // пути медиа
        self.migrate_gen_to_events();
        // номер дорожки = ранг строки (Р.56, п.449): чинится на ЧТЕНИИ,
        // потому что старые файлы (и чужие импорты) знают только номера.
        // Перестановка отдаётся наружу — читателю формата она нужна для
        // своих побочных карт (чужие треки в kt-otio).
        self.canonicalize_lanes()
    }

    /// Полная проверка инвариантов. Ошибки загрузки ЯВНЫЕ — никаких
    /// молчаливых выбрасываний битых клипов (дефект p старого формата).
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
            // старший бит адреса отдан вложенным лентам (Р.50): движок
            // кладёт `SourceId(NEST_TAG | seq)` в ключ кэша, и файл с таким
            // id молча делил бы кэш с секвенцией
            if m.id.0 >= NEST_TAG {
                return Err(ProjectError::MediaIdTooLarge(m.id));
            }
            if !media_ids.insert(m.id) {
                return Err(ProjectError::DuplicateMediaId(m.id));
            }
        }
        // область: имя — это АДРЕС, поэтому непустое и уникальное в
        // ОДНОМ пространстве с метками-диапазонами (иначе «область финал»
        // и «метка финал» в одном пикере — ловушка)
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
        // Дерево лент. Быстрый путь первой строкой: плоский проект —
        // подавляющее большинство — платит одну проверку `is_empty`, и
        // живой драг (`apply_coalesced` зовёт validate на каждый кадр
        // жеста) не становится дороже ни на такт.
        let sources = if self.sequences.is_empty() {
            media_ids.clone()
        } else {
            self.validate_tree(&media_ids)?;
            media_ids.iter().copied().chain(self.sequences.iter().map(|s| s.id)).collect()
        };
        // Содержимое каждой ленты — одним кодом. `ClipId` уникален во ВСЁМ
        // документе, а не внутри ленты: `Retained.clips`/`.transitions`
        // (kt-otio) — глобальные карты по id, и совпавшие id из разных лент
        // МОЛЧА обменялись бы материалом, стартовым таймкодом и эффектами.
        let mut clip_ids = std::collections::HashSet::new();
        for (seq, reel) in self.reels() {
            // ДОРОЖКА СУЩЕСТВУЕТ (Р.62): номер события — ранг среди дорожек
            // своего вида, и событие на несуществующей дорожке не имеет
            // строки на экране — «создать можно, управлять нельзя».
            // Чтение дописывает дорожки (`normalize`); валидация отвергает.
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
            // кривые на дорожке — следующий change (домен времени
            // таймлайна): принимать их молча в домене материала нельзя
            for t in reel.tracks() {
                if t.fx_auto.iter().any(|a| !a.is_empty()) {
                    return Err(ProjectError::TrackCurvesUnsupported(t.id));
                }
            }
            self.validate_reel(reel, seq, &sources, &media_ids, &mut clip_ids)?;
            // Связи ОДНОЙ ленты (Р.61): адрес разрешим, границу ленты не
            // пересекает, циклов нет. Проверка на ленту, а не на проект,
            // ровно потому, что адресное пространство замкнуто лентой.
            crate::links::check_links(reel).map_err(ProjectError::BadLink)?;
        }
        Ok(())
    }

    /// Ленты как ДОКУМЕНТ: адреса, имена, циклы, глубина.
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
            // адреса файлов и лент — ОДНО пространство: `clip.media`
            // смотрит и туда, и туда
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
        // Итеративный DFS СО ВСЕХ вершин, а не только от корня: пара лент
        // A ↔ B, не использованная в монтаже нигде, всё равно цикл — файл с
        // ней сохранился бы и не открылся. Итеративный — потому что
        // рекурсия по вражескому файлу это переполнение стека, а не ошибка.
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
                            // Текст ошибки называет ИМЕНА, а не адреса:
                            // «Заставка -> Титры -> Заставка» человек
                            // разматывает сам, «seq 7 -> seq 3» — нет.
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
        // Глубина считается у КАЖДОЙ ленты, а не только у использованных:
        // лента, лежащая в бине и не поставленная никуда, при первой же
        // постановке дала бы недопустимое дерево — ругаться надо там, где
        // её собрали, а не через неделю на драге.
        if let Some(i) = (0..n).find(|&i| depth[i] > MAX_NEST_DEPTH) {
            return Err(ProjectError::SequenceTooDeep(
                self.sequences[i].id,
                self.sequences[i].name.clone(),
            ));
        }
        Ok(())
    }

    /// Содержимое ОДНОЙ ленты. `sources` — всё, на что имеет право смотреть
    /// `clip.media` (файлы и ленты), `media_ids` — только файлы (морфер-слой
    /// берёт материал из файла: вложение под слоем — отдельное решение, и
    /// молча считать его файлом нельзя).
    /// Объявленные наружу ручки ленты (Р.59, стадия 3).
    ///
    /// Тип цели сверяется ЖИВЬЁМ, а не с хранимой копией: копия — второй
    /// источник того же факта, и «ручка, которая ни на что не действует»
    /// (минус разведки F-TL-012) обязана быть ошибкой загрузки, а не
    /// сюрпризом потребителя.
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

    /// Переопределения экземпляра: только на вложении и только на
    /// ОПУБЛИКОВАННОЕ (решение владельца).
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
                if !cur.source.is_keys() {
                    return Err(ProjectError::CurveSourceUnsupported(c.id));
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
            // старший диапазон отдан кускам морфер-слоя (Р.35): настоящий
            // клип с таким id столкнулся бы с синтетическим
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
            // ЦЕПОЧКА, РЕВЕРС И ВАРП на вложении законны (Р.55, С4).
            // До этого раунда реверс и растяжение отвергались: суб-микс
            // читался строго по порядку, а обоим нужно читать иначе.
            // Теперь у суб-микса есть продюсер с пре-роллом, и реверс с
            // варпом действуют на СЭМПЛЫ собранного микса — ровно так же,
            // как на сэмплы файла.
        }
        let mut morph_ids = std::collections::HashSet::new();
        for m in reel.morphers() {
            if !morph_ids.insert(m.id) {
                return Err(ProjectError::DuplicateMorphId(m.id));
            }
            // цель — клип ЭТОЙ ленты: ссылка через границу вернула бы
            // `None` из `joint()`, и слой молча перестал бы действовать
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
            // Области и метки — навигация ПО ПРОЕКТУ, они живут в корне и
            // их время — корневое. Слой внутри ленты, сославшийся на них,
            // накрыл бы не то место, и понять это было бы нечем.
            if !is_root
                && matches!(m.targets, MorphTargets::Areas { .. } | MorphTargets::Marks { .. })
            {
                return Err(ProjectError::MorphRefInsideSequence(m.id));
            }
        }
        // Камеры (Р.29): дубли id, длительность, fov, отсортированные
        // кейфреймы в [0,1]. Перекрытия камер-событий на одной дорожке
        // ЗАКОННЫ (разруливает active_camera_at) — к ClipOverlap не
        // подключаются.
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
            // кривые камеры — те же правила, что у кривых эффектов
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
        // Клипы одной (kind, lane) не перекрываются — инвариант Р.24, но
        // ТОЛЬКО при выключенном переслаивании (Р.28): с включённым перекрытия
        // законны и звучат кроссфейдом, как в v1. Foreign-плейсхолдеры
        // занимают место всегда: их сырьё пишется обратно в OTIO, где дорожка
        // — строгая последовательность, и наложить на них нечего.
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
                // переслаивание разрешает наложение КЛИПОВ, но не чужих
                // плейсхолдеров
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

    /// На что смотрит `clip.media` — файл или вложенная лента.
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

    /// Клип на генераторе (`gen:`-медиа, Р.52): источник считает кадр
    /// сам. Строго по РАЗБИРАЕМОМУ пути: непарсящийся `gen:` от будущей
    /// сборки — заглушка отсутствующего файла, окно генератора ему не
    /// предлагается.
    pub fn is_gen(&self, c: &ClipEvent) -> bool {
        self.is_gen_media(c.media)
    }

    pub fn is_gen_media(&self, id: MediaId) -> bool {
        self.gen_media_path(id).is_some()
    }

    /// Путь-ВИД генератора по адресу медиа. `None` — это не генератор
    /// (файл, лента, битый `gen:` из будущей сборки). Единственная дверь:
    /// и валидация, и команды, и сборка источников спрашивают её.
    pub fn gen_media_path(&self, id: MediaId) -> Option<&str> {
        let m = self.media.iter().find(|m| m.id == id)?;
        crate::genmedia::parse_gen_path(&m.path).map(|_| m.path.as_str())
    }

    /// Вид генератора по УМОЛЧАНИЮ для медиа-адреса: тем его заполняют
    /// команды, создающие событие на генераторе (`AddClip`,
    /// `SetClipMedia`). Значения приходят из пути, поэтому старый полный
    /// путь мигрирует сам собой — и это ЕДИНСТВЕННОЕ место, где такое
    /// заполнение происходит.
    pub fn default_gen_for(&self, id: MediaId) -> Option<crate::GeneratorKind> {
        let m = self.media.iter().find(|m| m.id == id)?;
        crate::genmedia::parse_gen_path(&m.path)
    }

    /// Сколько мест в ДЕРЕВЕ ссылается на эту ленту.
    ///
    /// Закрывает то, что разведка назвала общей бедой Premiere, FCP и
    /// VEGAS: «по клипу на таймлайне НЕ ВИДНО, сколько ещё мест изменится
    /// вместе с ним». Считается пересчётом, своей бухгалтерии нет.
    pub fn sequence_uses(&self, id: MediaId) -> usize {
        let count = |cs: &[ClipEvent]| cs.iter().filter(|c| c.media == id).count();
        count(&self.clips) + self.sequences.iter().map(|s| count(&s.clips)).sum::<usize>()
    }

    /// Наибольшие занятые адреса ВО ВСЁМ ДОКУМЕНТЕ (Р.50).
    ///
    /// Счёт «по корню» — дыра: события внутри лент для него невидимы, и
    /// свежий адрес, выданный по корню, СТОЛКНУЛСЯ БЫ с адресом внутри
    /// ленты. Батч тогда отвергается целиком, и жест (вставка, разрез)
    /// просто МОЛЧА не происходит. Обмен содержимого при входе внутрь
    /// союза не меняет, поэтому считать можно по любому виду проекта.
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

    /// Наибольший занятый адрес камеры ВО ВСЁМ ДОКУМЕНТЕ (Р.55, С3).
    /// Счёт «по корню» — та же дыра, что была у остальных четырёх (п.394):
    /// камеры внутри лент для него невидимы.
    pub fn max_camera_id(&self) -> u64 {
        self.cameras
            .iter()
            .chain(self.sequences.iter().flat_map(|s| s.cameras.iter()))
            .map(|c| c.id.0)
            .max()
            .unwrap_or(0)
    }

    /// Все ленты документа: корень первым, затем вложенные.
    fn reels(&self) -> impl Iterator<Item = (Option<MediaId>, &dyn Reel)> {
        std::iter::once((None, self as &dyn Reel))
            .chain(self.sequences.iter().map(|s| (Some(s.id), s as &dyn Reel)))
    }

    /// Парс без validate() — для легаси-импорта (Р.24): старые .ktproj
    /// законно содержат перекрытия, их конвертирует ops::upgrade_legacy,
    /// и только ПОСЛЕ него проект обязан проходить validate.
    pub fn from_json_unvalidated(s: &str) -> Result<Project, ProjectError> {
        // format_version проверяем до полного парсинга, чтобы отдавать
        // осмысленную ошибку на файлах будущих версий.
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
