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

/// Минимальная суммарная ширина окна перехода (1 мс, как MIN_CLIP_DURATION).
pub const MIN_TRANSITION_DURATION: Tick = Tick(90);

/// Вид перехода. Пока один: аудио — equal-power кроссфейд (формула теста 2),
/// видео — линейный dissolve. В OTIO пишется как SMPTE_Dissolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    Crossfade,
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
    pub kind: TransitionKind,
}

impl TransitionEvent {
    /// Стык: конец `a`, а для фейд-ина — начало `b` (Р.25). None — оба конца
    /// пропали; такое состояние живёт только внутри мутации до
    /// reconcile_transitions.
    pub fn joint(&self, p: &Project) -> Option<Tick> {
        if let Some(a) = self.a.and_then(|id| p.clips.iter().find(|c| c.id == id)) {
            return Some(a.start + a.duration);
        }
        let b = self.b.and_then(|id| p.clips.iter().find(|c| c.id == id))?;
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

    /// Полуоткрытое окно перехода на таймлайне.
    pub fn window(&self, p: &Project) -> Option<TickRange> {
        let at = self.joint(p)?;
        Some(TickRange::new(at - self.in_offset, at + self.out_offset))
    }
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
}

fn default_true() -> bool {
    true
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

/// Morpher (порт v1): непрозрачный «патч» на фиолетовой morph-дорожке.
/// Действует на ЗАМОРОЖЕННЫЙ при создании список клипов (`targets`) —
/// целиком, независимо от времени: `start`/`duration` только позиционируют
/// блок на таймлайне (v1-семантика, осознанно сохранена: подмена источника
/// посреди клипа невыразима без его разреза).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MorpherEvent {
    pub id: MorphId,
    /// Индекс morph-дорожки (0..morph_lanes).
    pub lane: u32,
    pub start: Tick,
    pub duration: Tick,
    /// Замороженные цели. Команды модели поддерживают список актуальным:
    /// удаление клипа вычищает id, split добавляет правую половину
    /// (в v1 висячие id жили вечно — дефект D5 разведки).
    pub targets: Vec<ClipId>,
    pub kind: MorphKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MorphKind {
    /// Подмена источника и/или сдвиг точки входа у целевых клипов.
    Replace {
        /// None = источник не менять (только сдвиг точки входа).
        media: Option<MediaId>,
        /// Сдвиг source_in (тики); применяется, если absolute не задан.
        source_in_delta: Tick,
        /// Абсолютный source_in — приоритетнее delta (в v1 поле было
        /// недостижимо из UI — дефект D4 разведки).
        absolute_source_in: Option<Tick>,
    },
    /// Дописать FX в конец цепочек целевых клипов. В отличие от v1
    /// (дефект D1) действует live — bake лишь материализует то же самое.
    EffectBatch { video_fx: Vec<FxInstance>, audio_fx: Vec<FxInstance> },
}

impl MorpherEvent {
    /// Применить патч к копии клипа (v1 applyMorpherPatchToClipCopy).
    /// Верхнего клампа source_in по длительности источника нет намеренно:
    /// модель не знает длительностей медиа (никакого FFmpeg в kt-model);
    /// чтение за концом источника движок отдаёт тишиной/последним кадром.
    pub fn apply_to(&self, c: &mut ClipEvent) {
        match &self.kind {
            MorphKind::Replace { media, source_in_delta, absolute_source_in } => {
                if let Some(m) = media {
                    c.media = *m;
                }
                c.source_in = match absolute_source_in {
                    Some(abs) => Tick(abs.0.max(0)),
                    None => Tick((c.source_in + *source_in_delta).0.max(0)),
                };
            }
            MorphKind::EffectBatch { video_fx, audio_fx } => {
                let extra = match c.kind {
                    ClipKind::Video => video_fx,
                    ClipKind::Audio => audio_fx,
                };
                if !extra.is_empty() {
                    // fx_bypass короче цепочки = дописанные слоты включены
                    // (active_fx трактует отсутствующий флаг как false)
                    c.fx.extend(extra.iter().cloned());
                }
            }
        }
    }
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
    /// Порядок отображения дорожек сверху вниз (п.6 v1.4): произвольное
    /// перемешивание V/A/M. Пусто (старые файлы) = блочный V..A..M.
    /// Чисто визуальное свойство: семантика ClipEvent.lane (z-order видео,
    /// микс-шина аудио) НЕ зависит от порядка строк. Вне undo, как весь
    /// UiState; нормализация — normalize_lane_order (тест 49).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
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
pub fn lane_names_rotate(names: &mut [(LaneKind, u32, String)], kind: LaneKind, from: u32, to: u32) {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub sample_rate: u32,
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
}

fn one_f32() -> f32 {
    1.0
}

fn is_one_f32(v: &f32) -> bool {
    *v == 1.0
}

impl ClipEvent {
    /// Активная FX-цепочка (без bypass-слотов) — единственный вид цепочки,
    /// который видят движок и рендереры.
    pub fn active_fx(&self) -> Vec<FxInstance> {
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
        self.fx
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.fx_bypass.get(*i).copied().unwrap_or(false))
            .map(|(i, _)| self.fx_auto.get(i).cloned().unwrap_or_default())
            .collect()
    }
}

/// Кейфрейм автоматизации параметра (Р.30): t — прогресс клипа 0..1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AutoKey {
    pub t: f64,
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

/// Структурная проверка кривых автоматизации клипа (Р.30): t
/// отсортированы в [0,1], значения конечны. Имя параметра НЕ проверяется
/// жёстко: кривая-сирота (переименованный/чужой параметр) молча не
/// действует — будущие правки схемы не должны валить загрузку файла.
pub fn check_fx_auto(id: ClipId, auto: &[Vec<ParamCurve>]) -> Result<(), ProjectError> {
    for curves in auto {
        for c in curves {
            let sorted = c.keys.windows(2).all(|w| w[0].t <= w[1].t);
            let in_range =
                c.keys.iter().all(|k| (0.0..=1.0).contains(&k.t) && k.v.is_finite());
            if !sorted || !in_range {
                return Err(ProjectError::BadFxAuto(id));
            }
        }
    }
    Ok(())
}

/// Значение кривой в точке u (домен 0..1, кламп краёв — как cam_at).
pub fn curve_value(keys: &[AutoKey], ease: bool, u: f64) -> f64 {
    let Some(first) = keys.first() else { return 0.0 };
    let u = u.clamp(0.0, 1.0);
    if u <= first.t {
        return first.v;
    }
    for w in keys.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if u <= b.t {
            let span = (b.t - a.t).max(1e-9);
            let mut s = ((u - a.t) / span).clamp(0.0, 1.0);
            if ease {
                s = s * s * (3.0 - 2.0 * s); // smoothstep01
            }
            return a.v + (b.v - a.v) * s;
        }
    }
    keys.last().unwrap().v
}

/// Применить кривые слота к эффекту в момент u. Возвращает число
/// сработавших кривых (сироты не считаются).
pub fn apply_fx_auto(fx: &mut FxInstance, curves: &[ParamCurve], u: f64) -> usize {
    let mut applied = 0;
    for c in curves {
        if c.keys.is_empty() {
            continue;
        }
        if fx.set_param(&c.param, curve_value(&c.keys, c.ease, u)) {
            applied += 1;
        }
    }
    applied
}

/// Типизированные FX (не opaque JSON — валидация на загрузке, тест 12).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FxInstance {
    Gain {
        db: f32,
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
        /// Keyframe-автоматизация: t — позиция В МАТЕРИАЛЕ ИСТОЧНИКА
        /// (кадры от source_in, нормированные длительностью клипа), НЕ
        /// выходное время. Кейфрейм приклеен к материалу: ранние правки
        /// не сдвигают материал поздних точек (п.1 v1.3, тест 31).
        /// Между точками линейная интерполяция, за краями — крайние.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        automation: Vec<SlowedKeyframe>,
    },
    // ---- видео-FX (порт bc_fx.cpp, п.25; параметры и дефолты v1) ----------
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
    },
    Edges {
        #[serde(default = "default_edges_threshold")]
        threshold: u32,
    },
    Noise {
        #[serde(default = "default_noise_amount")]
        amount: f64,
    },
    Vhs {
        #[serde(default = "default_rgb_shift")]
        rgb_shift: u32,
        #[serde(default = "default_bleed")]
        bleed: f64,
    },
    Glow {
        #[serde(default = "default_glow_strength")]
        strength: f64,
    },
    EventGrid {
        #[serde(default = "default_one_u32")]
        cols: u32,
        #[serde(default = "default_one_u32")]
        rows: u32,
        #[serde(default = "default_true")]
        center: bool,
    },
    // ---- 3D-эффекты (Р.29: категория Video3d, считаются ТОЛЬКО на GPU) ----
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
/// не сериализуется. Video3d считается только на GPU; CPU-фолбэк рисует
/// клип без таких эффектов (+notice), экспорт без GPU — явный отказ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxClass {
    Audio,
    Video2d,
    Video3d,
}

impl FxInstance {
    pub fn class(&self) -> FxClass {
        match self {
            FxInstance::Gain { .. } | FxInstance::SlowedReverb { .. } => FxClass::Audio,
            FxInstance::Pan3d { .. } => FxClass::Video3d,
            _ => FxClass::Video2d,
        }
    }

    /// Аудио-эффект? (видео-FX живут в той же цепочке клипа, но
    /// обрабатываются VideoRenderer'ом)
    pub fn is_audio(&self) -> bool {
        self.class() == FxClass::Audio
    }

    /// Эффект существует только на GPU (Video3d).
    pub fn requires_gpu(&self) -> bool {
        self.class() == FxClass::Video3d
    }

    /// Автоматизируемые параметры варианта (Р.30): (имя поля как в файле,
    /// min, max). Пустой список — вариант не автоматизируется (bools,
    /// аудио-FX — их автоматизация живёт своей механикой).
    pub fn param_specs(&self) -> &'static [(&'static str, f64, f64)] {
        match self {
            FxInstance::Pan { .. } => {
                &[("pan_x", -4.0, 4.0), ("pan_y", -4.0, 4.0), ("zoom", 0.05, 32.0)]
            }
            FxInstance::CamMotion { .. } => &[("time_scale", 0.05, 8.0)],
            FxInstance::Blur { .. } => &[("radius", 1.0, 48.0)],
            FxInstance::Greenscreen { .. } => &[
                ("key_r", 0.0, 255.0),
                ("key_g", 0.0, 255.0),
                ("key_b", 0.0, 255.0),
                ("tolerance", 1.0, 200.0),
                ("softness", 0.0, 100.0),
            ],
            FxInstance::Edges { .. } => &[("threshold", 1.0, 255.0)],
            FxInstance::Noise { .. } => &[("amount", 0.0, 1.0)],
            FxInstance::Vhs { .. } => &[("rgb_shift", 0.0, 12.0), ("bleed", 0.0, 1.0)],
            FxInstance::Glow { .. } => &[("strength", 0.0, 2.0)],
            FxInstance::EventGrid { .. } => {
                &[("cols", 1.0, 512.0), ("rows", 1.0, 512.0)]
            }
            FxInstance::Pan3d { .. } => &[
                ("pos_x", -100_000.0, 100_000.0),
                ("pos_y", -100_000.0, 100_000.0),
                ("pos_z", -100_000.0, 100_000.0),
                ("rot_x", -1440.0, 1440.0),
                ("rot_y", -1440.0, 1440.0),
                ("rot_z", -1440.0, 1440.0),
                ("anchor_x", 0.0, 1.0),
                ("anchor_y", 0.0, 1.0),
                ("scale", 0.01, 32.0),
            ],
            _ => &[],
        }
    }

    /// Установить параметр по имени (Р.30, автоматизация). Значение
    /// клампится диапазоном param_specs; целочисленные поля округляются.
    /// false — имя не подходит варианту (кривая-сирота).
    pub fn set_param(&mut self, name: &str, v: f64) -> bool {
        let Some(&(_, lo, hi)) = self.param_specs().iter().find(|(n, _, _)| *n == name)
        else {
            return false;
        };
        let v = v.clamp(lo, hi);
        let vu32 = v.round() as u32;
        let vu8 = v.round().clamp(0.0, 255.0) as u8;
        match (self, name) {
            (FxInstance::Pan { pan_x, .. }, "pan_x") => *pan_x = v,
            (FxInstance::Pan { pan_y, .. }, "pan_y") => *pan_y = v,
            (FxInstance::Pan { zoom, .. }, "zoom") => *zoom = v,
            (FxInstance::CamMotion { time_scale, .. }, "time_scale") => *time_scale = v,
            (FxInstance::Blur { radius }, "radius") => *radius = vu32,
            (FxInstance::Greenscreen { key_r, .. }, "key_r") => *key_r = vu8,
            (FxInstance::Greenscreen { key_g, .. }, "key_g") => *key_g = vu8,
            (FxInstance::Greenscreen { key_b, .. }, "key_b") => *key_b = vu8,
            (FxInstance::Greenscreen { tolerance, .. }, "tolerance") => *tolerance = vu32,
            (FxInstance::Greenscreen { softness, .. }, "softness") => *softness = vu32,
            (FxInstance::Edges { threshold }, "threshold") => *threshold = vu32,
            (FxInstance::Noise { amount }, "amount") => *amount = v,
            (FxInstance::Vhs { rgb_shift, .. }, "rgb_shift") => *rgb_shift = vu32,
            (FxInstance::Vhs { bleed, .. }, "bleed") => *bleed = v,
            (FxInstance::Glow { strength }, "strength") => *strength = v,
            (FxInstance::EventGrid { cols, .. }, "cols") => *cols = vu32,
            (FxInstance::EventGrid { rows, .. }, "rows") => *rows = vu32,
            (FxInstance::Pan3d { pos_x, .. }, "pos_x") => *pos_x = v,
            (FxInstance::Pan3d { pos_y, .. }, "pos_y") => *pos_y = v,
            (FxInstance::Pan3d { pos_z, .. }, "pos_z") => *pos_z = v,
            (FxInstance::Pan3d { rot_x, .. }, "rot_x") => *rot_x = v,
            (FxInstance::Pan3d { rot_y, .. }, "rot_y") => *rot_y = v,
            (FxInstance::Pan3d { rot_z, .. }, "rot_z") => *rot_z = v,
            (FxInstance::Pan3d { anchor_x, .. }, "anchor_x") => *anchor_x = v,
            (FxInstance::Pan3d { anchor_y, .. }, "anchor_y") => *anchor_y = v,
            (FxInstance::Pan3d { scale, .. }, "scale") => *scale = v,
            _ => return false,
        }
        true
    }

    /// Текущее значение параметра по имени (для UI автоматизации).
    pub fn get_param(&self, name: &str) -> Option<f64> {
        let v = match (self, name) {
            (FxInstance::Pan { pan_x, .. }, "pan_x") => *pan_x,
            (FxInstance::Pan { pan_y, .. }, "pan_y") => *pan_y,
            (FxInstance::Pan { zoom, .. }, "zoom") => *zoom,
            (FxInstance::CamMotion { time_scale, .. }, "time_scale") => *time_scale,
            (FxInstance::Blur { radius }, "radius") => *radius as f64,
            (FxInstance::Greenscreen { key_r, .. }, "key_r") => *key_r as f64,
            (FxInstance::Greenscreen { key_g, .. }, "key_g") => *key_g as f64,
            (FxInstance::Greenscreen { key_b, .. }, "key_b") => *key_b as f64,
            (FxInstance::Greenscreen { tolerance, .. }, "tolerance") => *tolerance as f64,
            (FxInstance::Greenscreen { softness, .. }, "softness") => *softness as f64,
            (FxInstance::Edges { threshold }, "threshold") => *threshold as f64,
            (FxInstance::Noise { amount }, "amount") => *amount,
            (FxInstance::Vhs { rgb_shift, .. }, "rgb_shift") => *rgb_shift as f64,
            (FxInstance::Vhs { bleed, .. }, "bleed") => *bleed,
            (FxInstance::Glow { strength }, "strength") => *strength,
            (FxInstance::EventGrid { cols, .. }, "cols") => *cols as f64,
            (FxInstance::EventGrid { rows, .. }, "rows") => *rows as f64,
            (FxInstance::Pan3d { pos_x, .. }, "pos_x") => *pos_x,
            (FxInstance::Pan3d { pos_y, .. }, "pos_y") => *pos_y,
            (FxInstance::Pan3d { pos_z, .. }, "pos_z") => *pos_z,
            (FxInstance::Pan3d { rot_x, .. }, "rot_x") => *rot_x,
            (FxInstance::Pan3d { rot_y, .. }, "rot_y") => *rot_y,
            (FxInstance::Pan3d { rot_z, .. }, "rot_z") => *rot_z,
            (FxInstance::Pan3d { anchor_x, .. }, "anchor_x") => *anchor_x,
            (FxInstance::Pan3d { anchor_y, .. }, "anchor_y") => *anchor_y,
            (FxInstance::Pan3d { scale, .. }, "scale") => *scale,
            _ => return None,
        };
        Some(v)
    }
}

/// Режим кроп-рамки пан/зума (v1: fit/fill/stretch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanMode {
    #[default]
    Fit,
    Fill,
    Stretch,
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
    /// Анимация по прогрессу события t∈[0,1] (образец CamMotion:
    /// линейная интерполяция, smoothstep при ease, за краями — крайние).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keyframes: Vec<CameraKeyframe>,
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
                        (Some(ta), Some(tb)) => {
                            Some((l(ta.0, tb.0), l(ta.1, tb.1), l(ta.2, tb.2)))
                        }
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
    pub fn slowed_reverb(rate: f64, mix: f32, reverb_delay_ms: f32) -> FxInstance {
        FxInstance::SlowedReverb {
            rate,
            mix,
            reverb_delay_ms,
            damp: default_damp(),
            decay: default_decay(),
            wet_gain: default_wet_gain(),
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
    #[error("transition {0:?}: clips are not butt-joined neighbors")]
    TransitionNotAtJoint(TransitionId),
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
}

/// Полная проверка перехода против проекта: стык, умещаемость, минимум,
/// уникальность стыка и непересечение окон на дорожке. `ignore` — id самого
/// перехода при SetTransition (не сравнивать с собой; Add передаёт None,
/// но self-id тоже пропускается). Используется и validate(), и check_on
/// команд (тест 135).
pub fn check_transition_full(
    p: &Project,
    t: &TransitionEvent,
    ignore: Option<TransitionId>,
) -> Result<(), ProjectError> {
    let clip = |id: ClipId| p.clips.iter().find(|c| c.id == id);
    let find = |side: Option<ClipId>| -> Result<Option<&ClipEvent>, ProjectError> {
        match side {
            None => Ok(None),
            Some(id) => Ok(Some(clip(id).ok_or(ProjectError::TransitionUnknownClip(t.id, id))?)),
        }
    };
    let a = find(t.a)?;
    let b = find(t.b)?;
    // фейд (Р.25): один конец — пустота; офсет в её сторону телом не
    // ограничен, стык берётся у живого клипа
    let (kind, lane, joint) = match (a, b) {
        (Some(a), Some(b)) => {
            if a.kind != b.kind || a.lane != b.lane || a.start + a.duration != b.start {
                return Err(ProjectError::TransitionNotAtJoint(t.id));
            }
            (a.kind, a.lane, a.start + a.duration)
        }
        (Some(a), None) => (a.kind, a.lane, a.start + a.duration),
        (None, Some(b)) => (b.kind, b.lane, b.start),
        (None, None) => return Err(ProjectError::TransitionWithoutClips(t.id)),
    };
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
    let w = TickRange::new(joint - t.in_offset, joint + t.out_offset);
    for other in &p.transitions {
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
            morphers: Vec::new(),
            transitions: Vec::new(),
            foreign: Vec::new(),
            ui: None,
            overlap_layers: true,
            timeline_end: None,
            camera_lanes: 0,
            cameras: Vec::new(),
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

    /// Клипы после применения morpher'ов — ЕДИНСТВЕННАЯ точка выдачи клипов
    /// движку, экспорту и отрисовке (v1 effectiveClipForRender/renderClips):
    /// движок про morpher не знает. Порядок применения детерминирован —
    /// по возрастанию MorphId (порядок создания; в v1 порядок вектора был
    /// негласным — дефект D11 разведки). Несколько morpher'ов на клип
    /// применяются кумулятивно.
    pub fn effective_clips(&self) -> Vec<ClipEvent> {
        let mut out = self.clips.clone();
        if self.morphers.is_empty() {
            return out;
        }
        let mut ms: Vec<&MorpherEvent> = self.morphers.iter().collect();
        ms.sort_by_key(|m| m.id);
        for m in ms {
            for c in out.iter_mut() {
                if m.targets.contains(&c.id) {
                    m.apply_to(c);
                }
            }
        }
        out
    }

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
            if !media_ids.insert(m.id) {
                return Err(ProjectError::DuplicateMediaId(m.id));
            }
        }
        let mut clip_ids = std::collections::HashSet::new();
        for c in &self.clips {
            if !clip_ids.insert(c.id) {
                return Err(ProjectError::DuplicateClipId(c.id));
            }
            if c.duration.0 <= 0 {
                return Err(ProjectError::NonPositiveDuration(c.id));
            }
            if c.source_in.0 < 0 {
                return Err(ProjectError::NegativeSourceIn(c.id));
            }
            if !media_ids.contains(&c.media) {
                return Err(ProjectError::UnknownMedia(c.id, c.media));
            }
            check_fx_auto(c.id, &c.fx_auto)?;
        }
        if let Some(r) = &self.region
            && r.end <= r.start
        {
            return Err(ProjectError::BadRegion);
        }
        let mut morph_ids = std::collections::HashSet::new();
        for m in &self.morphers {
            if !morph_ids.insert(m.id) {
                return Err(ProjectError::DuplicateMorphId(m.id));
            }
            for t in &m.targets {
                if !clip_ids.contains(t) {
                    return Err(ProjectError::MorphUnknownTarget(m.id, *t));
                }
            }
            if let MorphKind::Replace { media: Some(mid), .. } = &m.kind
                && !media_ids.contains(mid)
            {
                return Err(ProjectError::MorphUnknownMedia(m.id, *mid));
            }
        }
        // Камеры (Р.29): дубли id, длительность, fov, отсортированные
        // кейфреймы в [0,1]. Перекрытия камер-событий на одной дорожке
        // ЗАКОННЫ (разруливает active_camera_at) — к ClipOverlap не
        // подключаются.
        let mut cam_ids = std::collections::HashSet::new();
        for c in &self.cameras {
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
            if c.keyframes.iter().any(|k| !(0.0..=1.0).contains(&k.t)) {
                return Err(ProjectError::CameraBadKeyframes(c.id));
            }
        }
        let mut tr_ids = std::collections::HashSet::new();
        for t in &self.transitions {
            if !tr_ids.insert(t.id) {
                return Err(ProjectError::DuplicateTransitionId(t.id));
            }
            check_transition_full(self, t, None)?;
        }
        let mut foreign_keys = std::collections::HashSet::new();
        for f in &self.foreign {
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
        let mut spans: Vec<(ClipKind, u32, i64, i64, SpanId)> = self
            .clips
            .iter()
            .map(|c| (c.kind, c.lane, c.start.0, (c.start + c.duration).0, SpanId::Clip(c.id)))
            .chain(self.foreign.iter().map(|f| {
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
                if self.overlap_layers
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
