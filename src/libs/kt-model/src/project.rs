//! Типы формата проекта. Источник правды по схеме `.ktproj` v1.

use kt_time::Tick;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipKind {
    Video,
    Audio,
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
    /// Состояние вьюпорта редактора (п.13, порт v1: timelineViewStartMs/
    /// zoomLevel/cursorMs). Вне undo-истории (как region); старые файлы
    /// без секции читаются как None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiState>,
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
    pub t: Tick,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
}

impl Marker {
    pub fn at(t: Tick) -> Marker {
        Marker { t, name: String::new() }
    }
    pub fn named(t: Tick, name: impl Into<String>) -> Marker {
        Marker { t, name: name.into() }
    }
}

impl<'de> Deserialize<'de> for Marker {
    /// Принимает и старую форму (число тиков), и новую (объект): проекты,
    /// сохранённые до появления имён, продолжают открываться.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Marker, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Bare(i64),
            Full {
                t: i64,
                #[serde(default)]
                name: String,
            },
        }
        Ok(match Repr::deserialize(d)? {
            Repr::Bare(t) => Marker::at(Tick(t)),
            Repr::Full { t, name } => Marker { t: Tick(t), name },
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

/// Вид дорожки в display-порядке (ClipKind не годится: нет Morph).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneKind {
    Video,
    Audio,
    Morph,
}

/// Нормализация display-порядка дорожек к фактическим счётчикам (тест 49):
/// дубли и несуществующие (kind, lane) выбрасываются, недостающие
/// дописываются в конец в блочном порядке V..A..M. Пустой вход даёт
/// дефолтный блочный порядок.
pub fn normalize_lane_order(
    order: &[(LaneKind, u32)],
    video: u32,
    audio: u32,
    morph: u32,
) -> Vec<(LaneKind, u32)> {
    let valid = |&(k, l): &(LaneKind, u32)| match k {
        LaneKind::Video => l < video,
        LaneKind::Audio => l < audio,
        LaneKind::Morph => l < morph,
    };
    let mut out: Vec<(LaneKind, u32)> = Vec::with_capacity((video + audio + morph) as usize);
    for &e in order.iter().filter(|e| valid(e)) {
        if !out.contains(&e) {
            out.push(e);
        }
    }
    for (kind, n) in [(LaneKind::Video, video), (LaneKind::Audio, audio), (LaneKind::Morph, morph)]
    {
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
}

impl FxInstance {
    /// Аудио-эффект? (видео-FX живут в той же цепочке клипа, но
    /// обрабатываются VideoRenderer'ом)
    pub fn is_audio(&self) -> bool {
        matches!(self, FxInstance::Gain { .. } | FxInstance::SlowedReverb { .. })
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

fn default_zoom() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
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
            ui: None,
        }
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
        Ok(())
    }

    /// Детерминированная сериализация (стабильный порядок полей → тест 12
    /// «save→load→save байт-стабильно»).
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("project serialization is total");
        s.push('\n');
        s
    }

    pub fn from_json(s: &str) -> Result<Project, ProjectError> {
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
        p.validate()?;
        Ok(p)
    }
}
