//! Типы формата проекта. Источник правды по схеме `.ktproj` v1.

use kt_time::{Tick, TickRange};
use serde::{Deserialize, Serialize};

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
    /// Раскрытие из центра: круг/ромб/прямоугольник.
    Iris {
        #[serde(default)]
        shape: IrisShape,
        #[serde(default = "default_soft")]
        softness: f64,
    },
    /// Стрелка часов вокруг центра.
    Clock {
        #[serde(default)]
        reverse: bool,
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
    },
    /// Поворот с наплывом.
    Spin {
        #[serde(default = "default_one")]
        turns: f64,
        #[serde(default = "default_zoom_amount")]
        amount: f64,
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
pub trait HasParams {
    fn param_specs(&self) -> &'static [ParamSpec];
    fn get_param(&self, name: &str) -> Option<f64>;
    fn set_param(&mut self, name: &str, v: f64) -> bool;

    /// Спека параметра по имени (кривая-сирота видна сразу).
    fn param_spec(&self, name: &str) -> Option<&'static ParamSpec> {
        self.param_specs().iter().find(|s| s.name == name)
    }
}

impl HasParams for FxInstance {
    fn param_specs(&self) -> &'static [ParamSpec] {
        FxInstance::param_specs(self)
    }
    fn get_param(&self, name: &str) -> Option<f64> {
        FxInstance::get_param(self, name)
    }
    fn set_param(&mut self, name: &str, v: f64) -> bool {
        FxInstance::set_param(self, name, v)
    }
}

impl HasParams for TransitionKind {
    fn param_specs(&self) -> &'static [ParamSpec] {
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
        TransitionKind::Iris { shape: IrisShape::Circle, softness: 0.15 },
        TransitionKind::Clock { reverse: false, softness: 0.15 },
        TransitionKind::Barn { vertical: false, softness: 0.15 },
        TransitionKind::Luma { invert: false, softness: 0.15 },
        TransitionKind::Push { dir: Dir::Left },
        TransitionKind::Slide { dir: Dir::Left },
        TransitionKind::Zoom { amount: 0.4 },
        TransitionKind::Spin { turns: 1.0, amount: 0.4 },
        TransitionKind::Whip { dir: Dir::Left, blur: 0.6 },
        TransitionKind::BlurDissolve { amount: 24.0 },
        TransitionKind::Pixelate { amount: 48.0 },
        TransitionKind::Glitch { amount: 0.5 },
        TransitionKind::Flash { color_r: 255, color_g: 255, color_b: 255, strength: 1.0 },
        TransitionKind::Dip { color_r: 0, color_g: 0, color_b: 0 },
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
            | TransitionKind::Dip { .. } => true,
            TransitionKind::Wipe { .. }
            | TransitionKind::Iris { .. }
            | TransitionKind::Clock { .. }
            | TransitionKind::Barn { .. }
            | TransitionKind::Luma { .. }
            | TransitionKind::Push { .. }
            | TransitionKind::Slide { .. }
            | TransitionKind::Spin { .. }
            | TransitionKind::Whip { .. } => false,
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
                k.set_param(&c.param, curve_value(&c.keys, c.ease, local));
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

/// Старший бит адреса медиа отдан вложенным лентам (Р.50).
///
/// Движку нужен ОДИН `SourceId` на ленту — ключ кэша, ключ мемоизации
/// пост-FX, адрес в перф-панели. Он берётся как `NEST_TAG | seq`, поэтому
/// адрес настоящего файла обязан лежать ниже: иначе файл и секвенция
/// молча поделили бы кэш. Проверяется в `validate`.
pub const NEST_TAG: u64 = 1 << 63;

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
    #[serde(default = "default_lanes")]
    pub video_lanes: u32,
    #[serde(default = "default_lanes")]
    pub audio_lanes: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub morph_lanes: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub camera_lanes: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lane_order: Vec<(LaneKind, u32)>,
    #[serde(default = "default_true")]
    pub overlap_layers: bool,
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
            video_lanes: default_lanes(),
            audio_lanes: default_lanes(),
            morph_lanes: 0,
            camera_lanes: 0,
            lane_order: Vec::new(),
            overlap_layers: true,
        }
    }

    /// Строки ленты сверху вниз — то же правило, что у корня.
    pub fn rows(&self) -> Vec<(LaneKind, u32)> {
        normalize_lane_order(
            &self.lane_order,
            self.video_lanes,
            self.audio_lanes,
            self.morph_lanes,
            self.camera_lanes,
        )
    }

    /// Конец содержимого ленты.
    pub fn end(&self) -> Tick {
        self.clips.iter().map(|c| c.start + c.duration).max().unwrap_or(Tick::ZERO)
    }
}

impl Reel for Project {
    fn clips(&self) -> &[ClipEvent] {
        &self.clips
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
}

impl Reel for Sequence {
    fn clips(&self) -> &[ClipEvent] {
        &self.clips
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
    /// Явное число дорожек — пустые дорожки полноценны (кнопки add track,
    /// вставка дорожки дропом в зазор). На загрузке нормализуется вверх
    /// до max(lane клипов)+1.
    #[serde(default = "default_lanes")]
    pub video_lanes: u32,
    #[serde(default = "default_lanes")]
    pub audio_lanes: u32,
    /// Morph-дорожки (порт v1): отдельный тип дорожек, клипы на них не
    /// живут — только morph-события. 0 = дорожек нет (старые файлы).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub morph_lanes: u32,
    /// Порядок строк дорожек сверху вниз (Р.35). ПЕРЕЕХАЛ СЮДА ИЗ
    /// `UiState`: морфер-слой действует «на всё, что ниже», значит
    /// порядок влияет на картинку — а всё, что влияет на картинку,
    /// обязано ходить через undo и жить в модели.
    ///
    /// Инвариант: либо ПУСТО (естественный блочный порядок V..A..M..C),
    /// либо ПОЛНЫЙ нормализованный вектор. Иначе проект после первого же
    /// открытия обрастал бы полем и терял байт-стабильность.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lane_order: Vec<(LaneKind, u32)>,
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
    /// Камерные дорожки (Р.29, 3D-композитинг): отдельный тип дорожек,
    /// как морф-дорожки. 0 = дорожек нет (старые файлы байт-стабильны).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub camera_lanes: u32,
    /// Камеры-события. Активную в момент t выбирает `active_camera_at`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<CameraEvent>,
    /// Именованные области «время × дорожки» (Р.35 п.2): рамка выделения,
    /// сохранённая под именем. Морфер ссылается на них ПО ИМЕНИ.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub areas: Vec<MarkedArea>,
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
            keys.push(AutoKey { t: Tick(0), v: 0.0 });
        }
        keys.push(AutoKey { t: Tick(fi), v: s });
        keys.push(AutoKey { t: Tick(d - fo), v: s });
        if fo > 0 {
            keys.push(AutoKey { t: Tick(d), v: 0.0 });
        }
        keys.dedup_by_key(|k| k.t.0);
        Some(ParamCurve { param: "amount".into(), ease: false, keys })
    }

    /// Наложить слой на кусок цели: патч плюс слот СИЛЫ (Р.38 п.12).
    ///
    /// Сила — последний слот цепочки: она смешивает ВХОД цепочки с её
    /// результатом, поэтому обязана стоять после всего, что слой делает.
    pub fn apply_to(&self, c: &mut ClipEvent, frame_step: i64) {
        self.patch.apply_to(c, self.start, frame_step);
        if !self.mixes() {
            return;
        }
        // с какого слота начинается вклад ИМЕННО этого слоя
        let from = c.fx.len().saturating_sub(self.patch.chain_for(c.kind).len()) as u32;
        let mut mix = FxInstance::LayerMix { amount: self.strength, from };
        mix.set_param("amount", self.strength);
        let curve = self.mix_curve().map(|mut cur| {
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
        let warps = |fx: &[FxInstance]| fx.iter().any(|f| f.warps_time());
        let no_cut = warps(&c.active_fx()) || warps(&self.patch.chain_for(c.kind));
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
        match self.fit_window(w, c, p, reel, body) {
            Some(w) => vec![w],
            None => Vec::new(),
        }
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
        let zones = forbidden_zones(reel, c);
        let mut s = snap_out_of(grid(w.start), &zones);
        let mut e = snap_out_of(grid(w.end), &zones);
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

/// Вытолкнуть точку из запретных зон к ближайшему их краю. Зоны могут
/// перекрываться и примыкать, поэтому проход повторяется до сходимости
/// (не более чем по числу зон — каждый шаг выводит точку из очередной).
fn snap_out_of(t: Tick, zones: &[TickRange]) -> Tick {
    let mut t = t;
    for _ in 0..=zones.len() {
        let Some(z) = zones.iter().find(|z| z.start < t && t < z.end) else { return t };
        t = if (t - z.start) <= (z.end - t) { z.start } else { z.end };
    }
    t
}

/// Ключ строки события: вид дорожки и её номер. Отображение было
/// продублировано четырежды в `timeline.rs` и дважды в `convert.rs`.
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
pub const SLICE_TAG: u64 = 1 << 63;
/// Столько младших бит id отдано номеру куска.
const SLICE_IDX_BITS: u32 = 16;
/// Потолок id настоящего клипа (всё выше зарезервировано под куски).
pub const MAX_REAL_CLIP_ID: u64 = 1 << (63 - SLICE_IDX_BITS);

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
    let s = -(c.source_in.0 as f64) / span;
    let dt = crate::output_time_for_source(warp.base(), warp.prof(), s) * span;
    Tick((c.start.0 + dt.round() as i64).max(0))
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

/// Сколько МАТЕРИАЛА событие реально проходит: у растянутого — интеграл
/// скорости, у обычного — своя длина.
pub fn clip_material_span(c: &ClipEvent) -> i64 {
    crate::clip_warp(c).map(|w| w.used()).unwrap_or(c.duration.0).max(1)
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

fn default_lanes() -> u32 {
    1
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneKind {
    Video,
    Audio,
    Morph,
    /// Камерные дорожки (Р.29): живут только камеры-события.
    Camera,
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

/// Синхронизация имён дорожек при ВСТАВКЕ дорожки (п.2 р.10).
pub fn lane_names_insert(names: &mut [(LaneKind, u32, String)], kind: LaneKind, at: u32) {
    for e in names.iter_mut() {
        if e.0 == kind && e.1 >= at {
            e.1 += 1;
        }
    }
}

/// Синхронизация имён при УДАЛЕНИИ дорожки: имя удалённой уходит,
/// остальные сдвигаются.
pub fn lane_names_remove(names: &mut Vec<(LaneKind, u32, String)>, kind: LaneKind, at: u32) {
    names.retain(|(k, l, _)| !(*k == kind && *l == at));
    for e in names.iter_mut() {
        if e.0 == kind && e.1 > at {
            e.1 -= 1;
        }
    }
}

/// Имя дорожки едет ВМЕСТЕ с содержимым при перестановке (иначе
/// пользователь перетащит «Голос» вниз, а подпись останется наверху).
pub fn lane_names_swap(names: &mut [(LaneKind, u32, String)], kind: LaneKind, a: u32, b: u32) {
    for e in names.iter_mut() {
        if e.0 == kind {
            if e.1 == a {
                e.1 = b;
            } else if e.1 == b {
                e.1 = a;
            }
        }
    }
}

/// Имена при ДАЛЬНЕЙ перестановке (перетащили дорожку через несколько
/// строк): содержимое едет по формуле сдвига (см. `reorder_lane` в
/// kt-app) — имена обязаны ехать по ней же, иначе подпись приклеится к
/// чужой дорожке (п.5 р.16).
pub fn lane_names_rotate(
    names: &mut [(LaneKind, u32, String)],
    kind: LaneKind,
    from: u32,
    to: u32,
) {
    if from == to {
        return;
    }
    for e in names.iter_mut() {
        if e.0 != kind {
            continue;
        }
        e.1 = if e.1 == from {
            to
        } else if from < to && e.1 > from && e.1 <= to {
            e.1 - 1
        } else if from > to && e.1 >= to && e.1 < from {
            e.1 + 1
        } else {
            e.1
        };
    }
}

/// Подпись дорожки: пользовательское имя либо формула V1/A1/M1.
pub fn lane_label(names: &[(LaneKind, u32, String)], kind: LaneKind, lane: u32) -> String {
    if let Some((_, _, n)) = names.iter().find(|(k, l, _)| *k == kind && *l == lane)
        && !n.is_empty()
    {
        return n.clone();
    }
    let p = match kind {
        LaneKind::Video => "V",
        LaneKind::Audio => "A",
        LaneKind::Morph => "M",
        LaneKind::Camera => "C",
    };
    format!("{p}{}", lane + 1)
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
}

fn is_default_pitch_engine(e: &PitchEngine) -> bool {
    *e == PitchEngine::default()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaRef {
    pub id: MediaId,
    /// Путь относительно файла проекта.
    pub path: String,
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
}

/// Кривая одного параметра одного FX-слота. Между точками — линейная
/// интерполяция (smoothstep при ease, как CamMotion), за краями — крайние
/// значения. Имя параметра — snake_case поля варианта (как в файле);
/// кривая-сирота (имя не подходит варианту) молча не действует —
/// валидация ловит её при загрузке/командах.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamCurve {
    pub param: String,
    #[serde(default = "default_true")]
    pub ease: bool,
    pub keys: Vec<AutoKey>,
}

/// Структурная проверка кривых автоматизации клипа: ключи отсортированы
/// по материальной позиции, позиция неотрицательна, значения конечны.
/// Ключ ВНЕ куска, используемого клипом, законен — это спрятанный
/// обрезкой ключ, он вернётся, если событие снова растянуть. Имя
/// параметра НЕ проверяется жёстко: кривая-сирота (переименованный/чужой
/// параметр) молча не действует — правки схемы не валят загрузку файла.
pub fn check_fx_auto(id: ClipId, auto: &[Vec<ParamCurve>]) -> Result<(), ProjectError> {
    for curves in auto {
        for c in curves {
            let sorted = c.keys.windows(2).all(|w| w[0].t <= w[1].t);
            let ok = c.keys.iter().all(|k| k.t.0 >= 0 && k.v.is_finite());
            if !sorted || !ok {
                return Err(ProjectError::BadFxAuto(id));
            }
        }
    }
    Ok(())
}

/// Значение кривой в материальной позиции `at`. За крайними ключами —
/// их значения (кламп краёв, как cam_at). Домен НЕ клампится: позиция за
/// телом клипа (ручка перехода, пре-ролл) законна и честно продолжает
/// кривую её крайним значением.
pub fn curve_value(keys: &[AutoKey], ease: bool, at: Tick) -> f64 {
    let Some(first) = keys.first() else { return 0.0 };
    if at <= first.t {
        return first.v;
    }
    for w in keys.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if at <= b.t {
            let span = (b.t.0 - a.t.0).max(1) as f64;
            let mut s = (((at.0 - a.t.0) as f64) / span).clamp(0.0, 1.0);
            if ease {
                s = s * s * (3.0 - 2.0 * s); // smoothstep01
            }
            return a.v + (b.v - a.v) * s;
        }
    }
    keys.last().unwrap().v
}

/// Применить кривые слота к эффекту в материальной позиции `at`.
/// Возвращает число сработавших кривых (сироты не считаются).
pub fn apply_fx_auto(fx: &mut FxInstance, curves: &[ParamCurve], at: Tick) -> usize {
    let mut applied = 0;
    for c in curves {
        if c.keys.is_empty() {
            continue;
        }
        if fx.set_param(&c.param, curve_value(&c.keys, c.ease, at)) {
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
        let key = |u: f64, v: f64| AutoKey {
            t: material_from_progress(u, c.source_in, c.duration, false),
            v,
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
                ease: false,
                keys: legacy.iter().map(|k| key(k.t, pick(k))).collect(),
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
        c.auto.push(ParamCurve {
            param: (*name).into(),
            ease,
            keys: keys
                .iter()
                .zip(&vals)
                .map(|(k, v)| AutoKey {
                    t: Tick((k.t.clamp(0.0, 1.0) * dur).round() as i64),
                    v: *v,
                })
                .collect(),
        });
    }
    true
}

/// CamMotion → Pan + кривые (Р.34 К2). У Pan те же три параметра уже
/// есть в спеках, то есть «CamMotion с кейфреймами» — это в точности
/// «Pan с кривыми»; конверсия убирает дублирующий механизм анимации, а
/// не портирует его.
///
/// Тонкости, которые легко потерять: (1) пустые кейфреймы у CamMotion
/// означали НЕ статику, а v1-пресет «наезд 1.0 → 1.35» — он
/// материализуется явно, иначе старые проекты потеряли бы движение;
/// (2) `time_scale` разгонял чтение кейфреймов (`u_eff = u · scale`),
/// поэтому позиция ключа делится на него; (3) `ease` жил на эффекте, а в
/// новом домене он у каждой кривой свой.
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
        c.fx[i] =
            FxInstance::Pan { mode, pan_x: first.pan_x, pan_y: first.pan_y, zoom: first.zoom };
        if c.fx_auto.len() <= i {
            c.fx_auto.resize(i + 1, Vec::new());
        }
        for (param, pick) in [
            ("pan_x", Box::new(|k: &CamKeyframe| k.pan_x) as Box<dyn Fn(&CamKeyframe) -> f64>),
            ("pan_y", Box::new(|k: &CamKeyframe| k.pan_y)),
            ("zoom", Box::new(|k: &CamKeyframe| k.zoom)),
        ] {
            if c.fx_auto[i].iter().any(|c| c.param == param) {
                continue; // кривая нового домена уже есть — она главнее
            }
            c.fx_auto[i].push(ParamCurve {
                param: param.into(),
                ease,
                keys: keys
                    .iter()
                    .map(|k| AutoKey {
                        // u_eff = u·scale ⇒ ключ достигается при u = t/scale
                        t: material_from_progress(k.t / scale, c.source_in, c.duration, false),
                        v: pick(k),
                    })
                    .collect(),
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
    /// Кроп-рамка + пан/зум в размер проекта.
    Pan {
        #[serde(default)]
        mode: PanMode,
        #[serde(default)]
        pan_x: f64,
        #[serde(default)]
        pan_y: f64,
        #[serde(default = "default_zoom")]
        zoom: f64,
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
            FxInstance::Pan { .. } => "pan",
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
                FxInstance::pan(),
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
}

/// Описание одного параметра эффекта.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSpec {
    /// Имя поля snake_case — оно же ключ в файле и в `ParamCurve.param`.
    pub name: &'static str,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    /// Шаг для колеса/драга в UI.
    pub step: f64,
    pub unit: ParamUnit,
    pub kind: ParamKind,
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

/// ЕДИНСТВЕННАЯ декларация параметров эффектов: спеки, запись и чтение
/// генерятся из неё, поэтому рассинхрон трёх таблиц невозможен.
/// Тот же паспорт параметров, но для СТРУКТУРЫ (Р.34 К3: камере нужны
/// спеки, чтобы попасть в общую сетку полей и в общий виджет кривых).
/// Одна декларация — спеки, чтение и запись с клампом.
macro_rules! params_for_struct {
    ($ty:ident { $( $field:ident : $t:ident, $lo:expr, $hi:expr, $def:expr, $step:expr, $unit:ident, $kind:ident );+ $(;)? }) => {
        impl HasParams for $ty {
            fn param_specs(&self) -> &'static [ParamSpec] {
                &[ $( ParamSpec {
                    name: stringify!($field),
                    min: $lo, max: $hi, default: $def, step: $step,
                    unit: ParamUnit::$unit, kind: ParamKind::$kind,
                } ),+ ]
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
        }
    };
}

macro_rules! params_for {
    ($enum:ident, $specs:ident, $set:ident, $get:ident, {$(
        $variant:ident { $( $field:ident : $ty:ident, $lo:expr, $hi:expr, $def:expr, $step:expr, $unit:ident, $kind:ident );+ $(;)? }
    ),* $(,)?}) => {
        fn $specs(fx: &$enum) -> &'static [ParamSpec] {
            match fx {
                $( $enum::$variant { .. } => &[
                    $( ParamSpec {
                        name: stringify!($field),
                        min: $lo, max: $hi, default: $def, step: $step,
                        unit: ParamUnit::$unit, kind: ParamKind::$kind,
                    } ),+
                ], )*
                #[allow(unreachable_patterns)]
                _ => &[],
            }
        }

        fn $set(fx: &mut $enum, name: &str, v: f64) -> bool {
            match fx {
                $( $enum::$variant { $($field,)+ .. } => {
                    $( if name == stringify!($field) {
                        *$field = fx_cast!($ty, v);
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

params_for! { FxInstance, fx_specs, fx_set_param, fx_get_param, {
    // ---- аудио (Р.33: автоматизируются общим механизмом) ----------------
    Gain {
        db: f32, -60.0, 12.0, 0.0, 0.1, Db, Smooth;
        pan: f32, -1.0, 1.0, 0.0, 0.01, Norm, Smooth;
    },
    LayerMix {
        amount: f64, 0.0, 1.0, 1.0, 0.01, Norm, Smooth;
    },
    SlowedReverb {
        // потолок скорости — RATE_CLAMP карты варпа (warp.rs, Р.39:
        // раньше он жил в kt-fx и был обещанием через границу крейта)
        rate: f64, 0.25, 32.0, 1.0, 0.01, Ratio, Smooth;
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
    Pan {
        pan_x: f64, -4.0, 4.0, 0.0, 0.01, Norm, Smooth;
        pan_y: f64, -4.0, 4.0, 0.0, 0.01, Norm, Smooth;
        zoom: f64, 0.05, 32.0, 1.0, 0.01, Ratio, Smooth;
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

params_for! { TransitionKind, transition_specs, transition_set_param, transition_get_param, {
    Wipe {
        angle: f64, -180.0, 180.0, 0.0, 1.0, Deg, Smooth;
        softness: f64, 0.0, 1.0, 0.15, 0.01, Norm, Smooth;
    },
    Iris {
        softness: f64, 0.0, 1.0, 0.15, 0.01, Norm, Smooth;
    },
    Clock {
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
    },
    Spin {
        turns: f64, -4.0, 4.0, 1.0, 0.05, Ratio, Smooth;
        amount: f64, 0.0, 4.0, 0.4, 0.01, Ratio, Smooth;
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
                cam.set_param(&c.param, curve_value(&c.keys, c.ease, local));
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

    pub fn pan() -> FxInstance {
        FxInstance::Pan { mode: PanMode::Fit, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 }
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
    #[error(
        "clip {0:?} is a nested reel: reverse and time warp on the instance are not \
         rendered yet"
    )]
    NestInstanceNotChainable(ClipId),
    #[error("sequence {0:?}: morph layers inside a reel cannot target areas or named ranges")]
    MorphRefInsideSequence(MorphId),
    #[error("media id {0:?} is out of range (top bit is reserved for nested sequences)")]
    MediaIdTooLarge(MediaId),
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
            media: Vec::new(),
            clips: Vec::new(),
            markers: Vec::new(),
            region: None,
            loop_enabled: false,
            video_lanes: 1,
            audio_lanes: 1,
            morph_lanes: 0,
            lane_order: Vec::new(),
            morphers: Vec::new(),
            transitions: Vec::new(),
            foreign: Vec::new(),
            sequences: Vec::new(),
            ui: None,
            overlap_layers: true,
            trim_stretches_auto: false,
            timeline_end: None,
            camera_lanes: 0,
            cameras: Vec::new(),
            areas: Vec::new(),
        }
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
        normalize_lane_order(
            &self.lane_order,
            self.video_lanes,
            self.audio_lanes,
            self.morph_lanes,
            self.camera_lanes,
        )
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

    /// Записать порядок строк, схлопнув естественный в пусто (инвариант
    /// поля `lane_order`).
    pub fn set_lane_order(&mut self, order: &[(LaneKind, u32)]) {
        let norm = normalize_lane_order(
            order,
            self.video_lanes,
            self.audio_lanes,
            self.morph_lanes,
            self.camera_lanes,
        );
        let natural = normalize_lane_order(
            &[],
            self.video_lanes,
            self.audio_lanes,
            self.morph_lanes,
            self.camera_lanes,
        );
        self.lane_order = if norm == natural { Vec::new() } else { norm };
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
        let ms_all = reel.morphers();
        if ms_all.is_empty() {
            return reel.clips().to_vec();
        }
        // шаг кадра нужен для перевода ключей слоя в материал цели
        let frame_step = 90_000 / self.settings.fps.max(1) as i64;
        let mut ms: Vec<&MorpherEvent> = ms_all.iter().filter(|m| m.enabled).collect();
        // Порядок наложения — СНИЗУ ВВЕРХ (нижний слой первым, как
        // adjustment layer в AE); внутри строки прежнее правило по id.
        ms.sort_by_key(|m| (std::cmp::Reverse(m.row_in(rows)), m.id));
        let mut out = Vec::with_capacity(reel.clips().len());
        for c in reel.clips() {
            let wins: Vec<Vec<TickRange>> =
                ms.iter().map(|m| m.windows_on(c, self, reel, rows)).collect();
            let body = TickRange::new(c.start, c.start + c.duration);
            // точки реза — ВНУТРЕННИЕ края окон; общий набор на все
            // морферы, чтобы они не резали куски друг друга
            let mut cuts: Vec<i64> = wins
                .iter()
                .flatten()
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
                    if ws.iter().any(|x| x.start <= w.start && w.end <= x.end) {
                        m.apply_to(&mut piece, frame_step);
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
        std::mem::swap(&mut self.video_lanes, &mut s.video_lanes);
        std::mem::swap(&mut self.audio_lanes, &mut s.audio_lanes);
        std::mem::swap(&mut self.morph_lanes, &mut s.morph_lanes);
        std::mem::swap(&mut self.camera_lanes, &mut s.camera_lanes);
        std::mem::swap(&mut self.lane_order, &mut s.lane_order);
        std::mem::swap(&mut self.overlap_layers, &mut s.overlap_layers);
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
        match seq {
            None => self.morph_pass(self, &self.rows()),
            Some(id) => match self.sequence(id) {
                Some(s) => self.morph_pass(s, &s.rows()),
                None => Vec::new(),
            },
        }
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
        if depth >= MAX_NEST_DEPTH {
            return; // validate такого не пропустит; здесь — страховка
        }
        // окно экземпляра в координатах содержимого
        let win = TickRange::new(inst.source_in, inst.source_in + inst.duration);
        let inner = self.morph_pass(seq, &seq.rows());
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

    /// Зоны клипа, внутри которых граница морфер-слоя ставиться не может
    /// (находка 1 Р.35): окна переходов и наезды соседей по дорожке. Резка
    /// там сломала бы НЕ цель, а окружение — у видео порядок композиции
    /// `(Reverse(lane), start)`, и кусок с бо́льшим start вылез бы над
    /// соседом; у аудио `overlap_weight` считает рампу из геометрии клипов.
    /// Нормализация после загрузки: число дорожек подтягивается вверх до
    /// фактических lane клипов (файлы, писанные до появления полей).
    pub fn normalize(&mut self) {
        for c in &self.clips {
            match c.kind {
                ClipKind::Video => self.video_lanes = self.video_lanes.max(c.lane + 1),
                ClipKind::Audio => self.audio_lanes = self.audio_lanes.max(c.lane + 1),
            }
        }
        self.video_lanes = self.video_lanes.max(1);
        self.audio_lanes = self.audio_lanes.max(1);
        for m in &self.morphers {
            self.morph_lanes = self.morph_lanes.max(m.lane + 1);
        }
        for c in &self.cameras {
            self.camera_lanes = self.camera_lanes.max(c.lane + 1);
        }
        // ВАЖНО: stable-сортировка и дедуп по времени — при двух метках на
        // один тик с разными именами unstable дал бы недетерминированный
        // порядок и сломал байт-стабильность проекта (тест 12)
        self.markers.sort_by_key(|m| m.t);
        self.markers.dedup_by_key(|m| m.t);
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
            self.validate_reel(reel, seq.is_none(), &sources, &media_ids, &mut clip_ids)?;
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
    fn validate_reel(
        &self,
        reel: &dyn Reel,
        is_root: bool,
        sources: &std::collections::HashSet<MediaId>,
        media_ids: &std::collections::HashSet<MediaId>,
        seen_clips: &mut std::collections::HashSet<ClipId>,
    ) -> Result<(), ProjectError> {
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
            // ЦЕПОЧКА на вложении с волны Б законна и применяется к
            // СОБРАННОЙ картинке — ради этого вложение и заводилось.
            // А вот реверс и варп времени экземпляра требуют смешивания
            // ДВУХ суб-композитов и ресемплера над суб-миксом: их пока нет,
            // и такого состояния просто НЕ БЫВАЕТ — отказывают команда,
            // файл и bake разом (правило 16).
            if !self.sequences.is_empty()
                && self.sequences.iter().any(|s| s.id == c.media)
                && (c.reversed || crate::chain_warps_time(&c.active_fx(), &c.active_fx_auto()))
            {
                return Err(ProjectError::NestInstanceNotChainable(c.id));
            }
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
