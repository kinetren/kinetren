//! Редактируемая модель: команды + undo.
//!
//! Undo — снапшоты всего `Project` (просто и надёжно; глубина ограничена).
//! Команда ВАЛИДИРУЕТСЯ ДО снапшота: невозможная команда возвращает Err и не
//! оставляет no-op записей в истории (дефект q старого TimelineEditor, где
//! push шёл до проверок).

use crate::project::*;
use kt_time::{Tick, TickRange};

pub const MAX_UNDO: usize = 80;

/// Минимальная длительность клипа. 1 мс, как в старом редакторе.
/// Выровнять СКОРОСТЬ варпа у link-партнёров (Р.37 п.2, решение
/// пользователя: «эффект для аудио/видео можно объединять сразу»).
///
/// Правило живёт в МОДЕЛИ, а не в окне FX, потому что `SetFx` прилетает
/// ещё из вставки цепочки из буфера, из bake морфера, из ридера формата и
/// из kt-cli. «Пара A/V потребляет источник с одной скоростью» — это
/// инвариант монтажа: разъедутся скорости, разъедутся картинка и звук.
/// Прецедент — `reconcile_transitions` после каждой геометрической
/// мутации. Внутри `mutate_on` зеркалирование попадает в ТОТ ЖЕ undo-шаг
/// по построению, даже под `apply_coalesced`.
///
/// Синхронно: наличие слота, `rate` и его кривая. Своё у каждого:
/// параметры реверба, режим кадров, bypass слота (галка на видео-слоте —
/// это и есть «отключить замедление картинки»).
fn sync_warp_to_link_group(p: &mut Project, id: ClipId) {
    let Some(src) = p.clips.iter().find(|c| c.id == id) else { return };
    let Some(group) = src.link_group else { return };
    // слот ищется ПО ВИДУ, а не по индексу: у аудио может быть
    // [Gain, Slowed], у видео [Blur, Slowed, Mirror]
    let warp_slot = |c: &ClipEvent| c.fx.iter().position(|f| f.warps_time());
    let mine = warp_slot(src);
    let (tmpl, rate_curve) = match mine {
        Some(i) => (
            Some(src.fx[i].clone()),
            src.fx_auto.get(i).and_then(|cs| cs.iter().find(|c| c.param == "rate").cloned()),
        ),
        None => (None, None),
    };
    let partners: Vec<ClipId> = p
        .clips
        .iter()
        .filter(|c| c.id != id && c.link_group == Some(group))
        .map(|c| c.id)
        .collect();
    for pid in partners {
        let Some(c) = p.clips.iter_mut().find(|c| c.id == pid) else { continue };
        match (warp_slot(c), &tmpl) {
            // у обоих есть — переносим ТОЛЬКО скорость и её кривую
            (Some(j), Some(t)) => {
                if let (FxInstance::SlowedReverb { rate, .. }, FxInstance::SlowedReverb { rate: r0, .. }) =
                    (&mut c.fx[j], t)
                {
                    *rate = *r0;
                }
                if c.fx_auto.len() <= j {
                    c.fx_auto.resize(j + 1, Vec::new());
                }
                c.fx_auto[j].retain(|cu| cu.param != "rate");
                if let Some(rc) = &rate_curve {
                    c.fx_auto[j].push(rc.clone());
                }
            }
            // появился у одного — заводим у партнёра (одна добавка кладёт
            // экземпляр на ОБА события пары)
            (None, Some(t)) => {
                c.fx.push(t.clone());
                let mut curves = Vec::new();
                if let Some(rc) = &rate_curve {
                    curves.push(rc.clone());
                }
                c.fx_auto.resize(c.fx.len(), Vec::new());
                // слот только что добавлен — last_mut есть по построению
                if let Some(slot) = c.fx_auto.last_mut() {
                    *slot = curves;
                }
            }
            // исчез у одного — убираем у партнёра вместе с его кривыми
            (Some(j), None) => {
                c.fx.remove(j);
                if j < c.fx_auto.len() {
                    c.fx_auto.remove(j);
                }
                if j < c.fx_bypass.len() {
                    c.fx_bypass.remove(j);
                }
            }
            (None, None) => {}
        }
    }
}

pub const MIN_CLIP_DURATION: Tick = Tick(90);

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    AddMedia {
        id: MediaId,
        path: String,
    },
    AddClip(ClipEvent),
    RemoveClip {
        id: ClipId,
    },
    /// Двигает клип (и всю его link-группу — тем же смещением).
    MoveClip {
        id: ClipId,
        new_start: Tick,
        new_lane: u32,
    },
    /// Триммит начало: start/duration/source_in двигаются согласованно.
    TrimStart {
        id: ClipId,
        new_start: Tick,
    },
    TrimEnd {
        id: ClipId,
        new_end: Tick,
    },
    /// Заметка о РОДСТВЕ: `child` — событие, рождённое от `parent` в этом же
    /// пакете (Р.49, п.357). Состояние проекта не меняет вовсе.
    ///
    /// Зачем командой. Производные события — правая половина разреза, остатки
    /// кропа и punch-out, вставленная копия, куски bake — рождаются ВНУТРИ
    /// `ops::*`, и снаружи их родителя уже не вычислить: после перенумерации
    /// это была бы догадка по совпадению материала и окна. Тот, кто режет,
    /// родителя знает точно — пусть он его и назовёт. Носитель родства должен
    /// пережить undo вместе с самой правкой, а значит жить в пакете команд.
    NoteBirth {
        child: ClipId,
        parent: ClipId,
    },
    /// Разрез в точке `at` (абсолютный тик). Правая половина получает новый id
    /// и новую link-группу (правые половины A/V-пары остаются связаны).
    Split {
        id: ClipId,
        at: Tick,
        new_id: ClipId,
        new_group: Option<GroupId>,
    },
    /// Опция «трим растягивает анимацию вместе с событием» (Р.40).
    /// Выключена по умолчанию: ключ остаётся на своём кадре материала,
    /// куда бы ни ехал край события.
    SetTrimStretchesAuto {
        on: bool,
    },
    SetGain {
        id: ClipId,
        gain_db: f32,
    },
    /// Уровень (непрозрачность) видео-события 0..1 — Р.28, п.12.
    SetOpacity {
        id: ClipId,
        opacity: f32,
    },
    /// Мастер-выключатель FX-цепочки события (Р.33). Состояние слотов не
    /// трогает: включил обратно — цепочка та же.
    SetFxOff {
        id: ClipId,
        off: bool,
    },
    /// Реверс события (п.16). Undo-able.
    SetReversed {
        id: ClipId,
        reversed: bool,
    },
    /// Bypass-флаги FX-слотов (п.2 v1.3). Undo-able.
    SetFxBypass {
        id: ClipId,
        bypass: Vec<bool>,
    },
    SetFx {
        id: ClipId,
        fx: Vec<FxInstance>,
    },
    /// Автоматизация параметров видео-FX (Р.30): полная замена кривых
    /// клипа (параллельно fx, как fx_bypass). UI шлёт coalesced-жестом.
    SetFxAuto {
        id: ClipId,
        auto: Vec<Vec<ParamCurve>>,
    },
    LinkClips {
        ids: Vec<ClipId>,
        group: GroupId,
    },
    UnlinkClip {
        id: ClipId,
    },
    /// Переход на стыке двух соседних клипов одной дорожки (Р.24). Undo-able.
    AddTransition(TransitionEvent),
    RemoveTransition {
        id: TransitionId,
    },
    /// Полная замена перехода (драг краёв/тела — батчи и apply_coalesced).
    SetTransition(TransitionEvent),
    /// Метка; duration > 0 = диапазонная (Р.24, marked_range).
    AddMarker {
        at: Tick,
        duration: Tick,
    },
    RemoveMarker {
        at: Tick,
    },
    /// Задать/сменить имя метки (п.1 р.10). Отдельная команда: создание
    /// с именем = батч [AddMarker, SetMarkerName] (один undo-шаг), живой
    /// ввод — apply_coalesced по токену жеста.
    SetMarkerName {
        at: Tick,
        name: String,
    },
    MoveMarker {
        from: Tick,
        to: Tick,
    },
    /// Длительность метки (0 = схлопнуть в точечную). Края ленты — жестами.
    SetMarkerDuration {
        at: Tick,
        duration: Tick,
    },
    /// Вставить пустую дорожку типа `kind` в позицию `at`; lane клипов
    /// этого типа >= at сдвигаются на +1.
    InsertLane {
        kind: ClipKind,
        at: u32,
    },
    SetLanes {
        video: u32,
        audio: u32,
    },
    RemoveMedia {
        id: MediaId,
    },
    SetSettings {
        settings: ProjectSettings,
    },
    /// Morpher (порт v1). В отличие от v1 ВСЕ операции undo-able (дефект D6).
    AddMorpher(MorpherEvent),
    RemoveMorpher {
        id: MorphId,
    },
    /// Полная замена события (окно редактора; в UI — coalesced-жест).
    SetMorpher(MorpherEvent),
    /// Bake: деструктивно вписать патч в целевые клипы и удалить событие.
    /// Результат рендера не меняется: effective_clips уже применял то же.
    BakeMorpher {
        id: MorphId,
    },
    /// Порядок строк дорожек сверху вниз (Р.35). С переездом морфера в
    /// слой он влияет на КАРТИНКУ, поэтому живёт в модели и ходит через
    /// undo: перетащил дорожку — изменил монтаж. Полная замена; вектор
    /// нормализуется мутацией, проверять его не нужно.
    SetLaneOrder {
        order: Vec<(LaneKind, u32)>,
    },
    /// Именованные области (Р.35 п.2). Адресация ПО ИМЕНИ: индексы
    /// разъезжаются под undo/redo, а имя — это адрес.
    AddArea(MarkedArea),
    RemoveArea {
        name: String,
    },
    /// Переименование правит и ССЫЛКИ морферов — одним шагом истории.
    RenameArea {
        from: String,
        to: String,
    },
    SetAreaRange {
        name: String,
        start: Tick,
        duration: Tick,
    },
    SetAreaLanes {
        name: String,
        lanes: Vec<(LaneKind, u32)>,
    },
    /// Вставить morph-дорожку в позицию `at` (lane событий >= at сдвигается).
    InsertMorphLane {
        at: u32,
    },
    /// Удалить morph-дорожку вместе с её событиями.
    RemoveMorphLane {
        at: u32,
    },
    /// Камеры (Р.29, 3D-композитинг) — по образцу морферных команд.
    AddCamera(CameraEvent),
    /// Полная замена события (окно параметров; в UI — coalesced-жест).
    SetCamera(CameraEvent),
    RemoveCamera {
        id: CameraId,
    },
    /// Вставить камерную дорожку в позицию `at`.
    InsertCameraLane {
        at: u32,
    },
    /// Удалить камерную дорожку вместе с её событиями.
    RemoveCameraLane {
        at: u32,
    },
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CommandError {
    #[error("clip {0:?} not found")]
    ClipNotFound(ClipId),
    #[error("media {0:?} not found")]
    MediaNotFound(MediaId),
    #[error("id {0:?} already in use")]
    IdInUse(u64),
    #[error("result would violate minimal duration")]
    TooShort,
    #[error("marked area {0:?} not found")]
    AreaNotFound(String),
    #[error("marked area: name must not be empty")]
    AreaUnnamed,
    #[error("name {0:?} is already taken (areas and named ranges share one namespace)")]
    NameTaken(String),
    #[error("split point outside clip")]
    SplitOutside,
    #[error("invalid clip: {0}")]
    InvalidClip(#[from] ProjectError),
    #[error("transition {0:?} not found")]
    TransitionNotFound(TransitionId),
    #[error("layout invariant violated: {0}")]
    InvalidLayout(ProjectError),
    #[error("invalid transition: {0}")]
    InvalidTransition(ProjectError),
    #[error("marker at {0:?} not found")]
    MarkerNotFound(Tick),
    #[error("marker at {0:?} already exists")]
    MarkerExists(Tick),
    #[error("media {0:?} is used by clips")]
    MediaInUse(MediaId),
    #[error("lane count would orphan clips")]
    LanesTooFew,
    #[error("morpher {0:?} not found")]
    MorpherNotFound(MorphId),
    #[error("morpher needs at least one target clip")]
    MorpherNoTargets,
    #[error("morph lane {0} not found")]
    MorphLaneNotFound(u32),
    #[error("camera {0:?} not found")]
    CameraNotFound(CameraId),
    #[error("invalid camera: {0}")]
    InvalidCamera(ProjectError),
    #[error("camera lane {0} not found")]
    CameraLaneNotFound(u32),
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("nothing to redo")]
    NothingToRedo,
}

/// Модель с историей. Движок читает `project()` (снапшот-клон при rebuild).
#[derive(Debug, Clone)]
pub struct TimelineModel {
    project: Project,
    undo: Vec<Project>,
    redo: Vec<Project>,
    /// Токен последнего coalesced-жеста (см. apply_coalesced).
    last_gesture: Option<u64>,
}

/// Кто от кого родился в пакете команд (Р.49, п.357). Родство сообщает
/// МОДЕЛЬ, а не догадывается вызывающий: у `Split` оно прямо в аргументах, у
/// остальных производных — в заметке `NoteBirth`, которую оставляет тот, кто
/// резал. Догадка по совпадению материала и окна была бы именно догадкой:
/// после перенумерации отличить «половину разреза» от «соседнего события того
/// же файла» нечем.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Births {
    /// Пары «новое событие ← родитель», в порядке рождения.
    pub pairs: Vec<(ClipId, ClipId)>,
}

impl Births {
    fn of(cmds: &[Command]) -> Births {
        let mut pairs = Vec::new();
        for c in cmds {
            match c {
                Command::Split { id, new_id, .. } => pairs.push((*new_id, *id)),
                Command::NoteBirth { child, parent } => pairs.push((*child, *parent)),
                _ => {}
            }
        }
        Births { pairs }
    }

    pub fn parent_of(&self, child: ClipId) -> Option<ClipId> {
        self.pairs.iter().find(|(c, _)| *c == child).map(|(_, p)| *p)
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

impl TimelineModel {
    pub fn new(project: Project) -> TimelineModel {
        TimelineModel { project, undo: Vec::new(), redo: Vec::new(), last_gesture: None }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn clip(&self, id: ClipId) -> Option<&ClipEvent> {
        self.project.clips.iter().find(|c| c.id == id)
    }

    /// Полуоткрытый интервал клипа на таймлайне.
    pub fn clip_range(c: &ClipEvent) -> TickRange {
        TickRange::new(c.start, c.start + c.duration)
    }

    pub fn apply(&mut self, cmd: Command) -> Result<(), CommandError> {
        self.apply_batch(vec![cmd]).map(|_| ())
    }

    /// Пакет команд = ОДИН undo-снапшот на жест (сплит A/V-пары, удаление
    /// группы, paste, punch-out). Атомарно: валидация всей серии на клоне,
    /// провал любой команды = ноль изменений и ноль записей в истории.
    pub fn apply_batch(&mut self, cmds: Vec<Command>) -> Result<Births, CommandError> {
        if cmds.is_empty() {
            return Ok(Births::default());
        }
        let scratch = self.simulate(&cmds)?;
        let births = Births::of(&cmds);
        self.push_undo();
        self.project = scratch;
        self.last_gesture = None;
        Ok(births)
    }

    /// Проект, каким он СТАНЕТ после батча — без записи в undo (Р.26).
    /// Нужен call-site'ам, которым надо посчитать что-то по будущему
    /// состоянию: возврат «ручек» кроссфейда считает свободное место уже
    /// после того, как клип уехал (ops::restore_handles).
    pub fn preview(&self, cmds: &[Command]) -> Result<Project, CommandError> {
        self.simulate(cmds)
    }

    fn simulate(&self, cmds: &[Command]) -> Result<Project, CommandError> {
        let mut scratch = self.project.clone();
        for cmd in cmds {
            Self::check_on(&scratch, cmd)?;
            Self::mutate_on(&mut scratch, cmd.clone());
        }
        // reconcile — ОДИН раз на весь батч: жест атомарен, и переход не
        // должен умирать в переходном состоянии между командами (roll стыка
        // = TrimEnd+TrimStart: между ними стык формально разорван)
        Self::reconcile_transitions(&mut scratch);
        // Жёсткая валидация результата (Р.24, этап 6): перекрытия после
        // групповых мувов/тримов/вставок дешевле отловить одной проверкой,
        // чем дублировать их логику в check_on. Провал = ноль изменений.
        scratch.validate().map_err(CommandError::InvalidLayout)?;
        Ok(scratch)
    }

    /// Как apply, но серия вызовов с одним `gesture`-токеном сливается в
    /// одну undo-запись (live-редактирование параметров FX из окна).
    pub fn apply_coalesced(&mut self, cmd: Command, gesture: u64) -> Result<(), CommandError> {
        let mut scratch = self.project.clone();
        Self::check_on(&scratch, &cmd)?;
        Self::mutate_on(&mut scratch, cmd);
        Self::reconcile_transitions(&mut scratch);
        scratch.validate().map_err(CommandError::InvalidLayout)?;
        if self.last_gesture != Some(gesture) {
            self.push_undo();
        }
        self.project = scratch;
        self.last_gesture = Some(gesture);
        Ok(())
    }

    fn push_undo(&mut self) {
        self.undo.push(self.project.clone());
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Регион ВНЕ undo-истории (семантика старого редактора).
    pub fn set_region(&mut self, r: Option<Region>) {
        self.project.region = r;
    }

    pub fn set_loop_enabled(&mut self, on: bool) {
        self.project.loop_enabled = on;
    }

    /// Вьюпорт (вне undo, как регион) — сохраняется в файл проекта.
    pub fn set_ui_state(&mut self, ui: Option<crate::UiState>) {
        self.project.ui = ui;
    }

    pub fn undo(&mut self) -> Result<(), CommandError> {
        let mut prev = self.undo.pop().ok_or(CommandError::NothingToUndo)?;
        // регион/loop не откатываются — переносим текущие значения
        prev.region = self.project.region;
        prev.loop_enabled = self.project.loop_enabled;
        self.redo.push(std::mem::replace(&mut self.project, prev));
        self.last_gesture = None;
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), CommandError> {
        let mut next = self.redo.pop().ok_or(CommandError::NothingToRedo)?;
        next.region = self.project.region;
        next.loop_enabled = self.project.loop_enabled;
        self.undo.push(std::mem::replace(&mut self.project, next));
        self.last_gesture = None;
        Ok(())
    }

    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    fn check_on(p: &Project, cmd: &Command) -> Result<(), CommandError> {
        let find =
            |id: ClipId| p.clips.iter().find(|c| c.id == id).ok_or(CommandError::ClipNotFound(id));
        match cmd {
            // заметка о родстве ничего не требует от проекта: она про пакет
            Command::NoteBirth { .. } => {}
            Command::AddMedia { id, .. } => {
                if p.media.iter().any(|m| m.id == *id) {
                    return Err(CommandError::IdInUse(id.0));
                }
            }
            Command::AddClip(c) => {
                if p.clips.iter().any(|x| x.id == c.id) {
                    return Err(CommandError::IdInUse(c.id.0));
                }
                if !p.media.iter().any(|m| m.id == c.media) {
                    return Err(CommandError::MediaNotFound(c.media));
                }
                if c.duration < MIN_CLIP_DURATION {
                    return Err(CommandError::TooShort);
                }
                if c.source_in.0 < 0 {
                    return Err(CommandError::InvalidClip(ProjectError::NegativeSourceIn(c.id)));
                }
            }
            Command::RemoveClip { id }
            | Command::SetGain { id, .. }
            | Command::SetOpacity { id, .. }
            | Command::SetFxOff { id, .. }
            | Command::SetReversed { id, .. }
            | Command::SetFxBypass { id, .. }
            | Command::SetFx { id, .. }
            | Command::UnlinkClip { id }
            | Command::MoveClip { id, .. } => {
                find(*id)?;
            }
            Command::SetFxAuto { id, auto } => {
                find(*id)?;
                check_fx_auto(*id, auto).map_err(CommandError::InvalidClip)?;
            }
            Command::TrimStart { id, new_start } => {
                let c = find(*id)?;
                let end = c.start + c.duration;
                if end - *new_start < MIN_CLIP_DURATION {
                    return Err(CommandError::TooShort);
                }
                // source_in не может уйти в минус (у reversed левый трим
                // source_in не трогает — п.16). Считает ТА ЖЕ функция, что
                // и мутация: иначе валидация и правка разойдутся на
                // растянутом клипе (Р.39).
                if !slice_fits_source(c, TickRange::new(*new_start, end)) {
                    return Err(CommandError::TooShort);
                }
            }
            Command::TrimEnd { id, new_end } => {
                let c = find(*id)?;
                if *new_end - c.start < MIN_CLIP_DURATION {
                    return Err(CommandError::TooShort);
                }
                // у reversed расширение вправо тянет source_in вниз (п.16)
                if c.reversed && (c.source_in + (c.start + c.duration - *new_end)).0 < 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::Split { id, at, new_id, .. } => {
                let c = find(*id)?;
                if p.clips.iter().any(|x| x.id == *new_id) {
                    return Err(CommandError::IdInUse(new_id.0));
                }
                let r = Self::clip_range(c);
                if !(r.contains(*at)
                    && (*at - r.start) >= MIN_CLIP_DURATION
                    && (r.end - *at) >= MIN_CLIP_DURATION)
                {
                    return Err(CommandError::SplitOutside);
                }
            }
            Command::LinkClips { ids, .. } => {
                for id in ids {
                    find(*id)?;
                }
            }
            Command::AddTransition(t) => {
                if p.transitions.iter().any(|x| x.id == t.id) {
                    return Err(CommandError::IdInUse(t.id.0));
                }
                check_transition_full(p, t, None).map_err(CommandError::InvalidTransition)?;
            }
            Command::RemoveTransition { id } => {
                if !p.transitions.iter().any(|t| t.id == *id) {
                    return Err(CommandError::TransitionNotFound(*id));
                }
            }
            Command::SetTransition(t) => {
                if !p.transitions.iter().any(|x| x.id == t.id) {
                    return Err(CommandError::TransitionNotFound(t.id));
                }
                check_transition_full(p, t, Some(t.id)).map_err(CommandError::InvalidTransition)?;
            }
            Command::AddMarker { at, duration } => {
                if p.markers.iter().any(|m| m.t == *at) {
                    return Err(CommandError::MarkerExists(*at));
                }
                if duration.0 < 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::RemoveMarker { at } => {
                if !p.markers.iter().any(|m| m.t == *at) {
                    return Err(CommandError::MarkerNotFound(*at));
                }
            }
            Command::SetMarkerDuration { at, duration } => {
                if !p.markers.iter().any(|m| m.t == *at) {
                    return Err(CommandError::MarkerNotFound(*at));
                }
                if duration.0 < 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::SetMarkerName { at, name } => {
                // одно пространство имён с областями (Р.35 п.2)
                if !name.is_empty()
                    && p.markers.iter().any(|m| m.t == *at && m.duration.0 > 0)
                    && p.taken_names().iter().any(|t| t == name)
                    && !p.markers.iter().any(|m| m.t == *at && m.name == *name)
                {
                    return Err(CommandError::NameTaken(name.clone()));
                }
                if !p.markers.iter().any(|m| m.t == *at) {
                    return Err(CommandError::MarkerNotFound(*at));
                }
            }
            Command::MoveMarker { from, to } => {
                if !p.markers.iter().any(|m| m.t == *from) {
                    return Err(CommandError::MarkerNotFound(*from));
                }
                if p.markers.iter().any(|m| m.t == *to) && from != to {
                    return Err(CommandError::MarkerExists(*to));
                }
            }
            Command::InsertLane { .. } => {}
            Command::SetLanes { video, audio } => {
                let need_v =
                    p.clips.iter().filter(|c| c.kind == ClipKind::Video).map(|c| c.lane + 1).max();
                let need_a =
                    p.clips.iter().filter(|c| c.kind == ClipKind::Audio).map(|c| c.lane + 1).max();
                if *video < need_v.unwrap_or(1).max(1) || *audio < need_a.unwrap_or(1).max(1) {
                    return Err(CommandError::LanesTooFew);
                }
            }
            Command::RemoveMedia { id } => {
                if !p.media.iter().any(|m| m.id == *id) {
                    return Err(CommandError::MediaNotFound(*id));
                }
                if p.clips.iter().any(|c| c.media == *id) {
                    return Err(CommandError::MediaInUse(*id));
                }
                // morpher с подменой на это медиа тоже держит его
                if p.morphers.iter().any(|m| m.media_refs().any(|x| x == *id)) {
                    return Err(CommandError::MediaInUse(*id));
                }
            }
            Command::SetSettings { settings } => {
                if settings.fps == 0 || kt_time::TICK_HZ % settings.fps as i64 != 0 {
                    return Err(CommandError::InvalidClip(ProjectError::BadFps));
                }
            }
            Command::AddMorpher(m) => {
                if p.morphers.iter().any(|x| x.id == m.id) {
                    return Err(CommandError::IdInUse(m.id.0));
                }
                Self::check_morpher_on(p, m)?;
            }
            Command::SetMorpher(m) => {
                if !p.morphers.iter().any(|x| x.id == m.id) {
                    return Err(CommandError::MorpherNotFound(m.id));
                }
                Self::check_morpher_on(p, m)?;
            }
            Command::RemoveMorpher { id } | Command::BakeMorpher { id } => {
                if !p.morphers.iter().any(|x| x.id == *id) {
                    return Err(CommandError::MorpherNotFound(*id));
                }
            }
            Command::SetLaneOrder { .. }
            | Command::InsertMorphLane { .. }
            | Command::SetTrimStretchesAuto { .. } => {}
            Command::AddArea(a) => {
                if a.name.trim().is_empty() {
                    return Err(CommandError::AreaUnnamed);
                }
                if p.taken_names().contains(&a.name) {
                    return Err(CommandError::NameTaken(a.name.clone()));
                }
                if a.duration.0 <= 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::RemoveArea { name } | Command::SetAreaLanes { name, .. } => {
                if p.area(name).is_none() {
                    return Err(CommandError::AreaNotFound(name.clone()));
                }
            }
            Command::SetAreaRange { name, duration, .. } => {
                if p.area(name).is_none() {
                    return Err(CommandError::AreaNotFound(name.clone()));
                }
                if duration.0 <= 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::RenameArea { from, to } => {
                if p.area(from).is_none() {
                    return Err(CommandError::AreaNotFound(from.clone()));
                }
                if to.trim().is_empty() {
                    return Err(CommandError::AreaUnnamed);
                }
                if to != from && p.taken_names().contains(to) {
                    return Err(CommandError::NameTaken(to.clone()));
                }
            }
            Command::AddCamera(c) => {
                if p.cameras.iter().any(|x| x.id == c.id) {
                    return Err(CommandError::IdInUse(c.id.0));
                }
                Self::check_camera_on(c)?;
            }
            Command::SetCamera(c) => {
                if !p.cameras.iter().any(|x| x.id == c.id) {
                    return Err(CommandError::CameraNotFound(c.id));
                }
                Self::check_camera_on(c)?;
            }
            Command::RemoveCamera { id } => {
                if !p.cameras.iter().any(|x| x.id == *id) {
                    return Err(CommandError::CameraNotFound(*id));
                }
            }
            Command::InsertCameraLane { .. } => {}
            Command::RemoveCameraLane { at } => {
                if *at >= p.camera_lanes {
                    return Err(CommandError::CameraLaneNotFound(*at));
                }
            }
            Command::RemoveMorphLane { at } => {
                if *at >= p.morph_lanes {
                    return Err(CommandError::MorphLaneNotFound(*at));
                }
            }
        }
        Ok(())
    }

    fn check_morpher_on(p: &Project, m: &MorpherEvent) -> Result<(), CommandError> {
        // пустой список запрещён ТОЛЬКО в режиме замороженных целей: у
        // слоя («всё, что ниже») и у ссылок пустое множество законно —
        // просто сейчас под морфером ничего нет
        if let MorphTargets::Clips { ids } = &m.targets {
            if ids.is_empty() {
                return Err(CommandError::MorpherNoTargets);
            }
            for t in ids {
                if !p.clips.iter().any(|c| c.id == *t) {
                    return Err(CommandError::ClipNotFound(*t));
                }
            }
        }
        for mid in m.media_refs() {
            if !p.media.iter().any(|x| x.id == mid) {
                return Err(CommandError::MediaNotFound(mid));
            }
        }
        Ok(())
    }

    /// Проверка камеры-события теми же правилами, что validate() (Р.29):
    /// команда не имеет права провести в проект то, что он потом отвергнет.
    fn check_camera_on(c: &CameraEvent) -> Result<(), CommandError> {
        if c.duration.0 <= 0 {
            return Err(CommandError::InvalidCamera(ProjectError::CameraBadDuration(c.id)));
        }
        let fovs = std::iter::once(c.cam.fov_deg).chain(c.keyframes.iter().map(|k| k.cam.fov_deg));
        for fov in fovs {
            if !(fov > 0.0 && fov < 180.0) {
                return Err(CommandError::InvalidCamera(ProjectError::CameraBadFov(c.id)));
            }
        }
        let sorted = c.keyframes.windows(2).all(|w| w[0].t <= w[1].t);
        if !sorted || c.keyframes.iter().any(|k| !(0.0..=1.0).contains(&k.t)) {
            return Err(CommandError::InvalidCamera(ProjectError::CameraBadKeyframes(c.id)));
        }
        Ok(())
    }

    fn mutate_on(p: &mut Project, cmd: Command) {
        match cmd {
            Command::NoteBirth { .. } => {}
            Command::AddMedia { id, path } => {
                p.media.push(MediaRef { id, path });
            }
            Command::AddClip(c) => {
                match c.kind {
                    ClipKind::Video => p.video_lanes = p.video_lanes.max(c.lane + 1),
                    ClipKind::Audio => p.audio_lanes = p.audio_lanes.max(c.lane + 1),
                }
                p.clips.push(c)
            }
            Command::RemoveClip { id } => {
                p.clips.retain(|c| c.id != id);
                // D5 (v1): вычищаем id из замороженных целей morpher'ов
                for m in &mut p.morphers {
                    if let Some(ids) = m.targets.clip_ids_mut() {
                        ids.retain(|t| *t != id);
                    }
                }
            }
            Command::MoveClip { id, new_start, new_lane } => {
                let (group, delta, kind) = {
                    let c = p.clips.iter().find(|c| c.id == id).unwrap();
                    (c.link_group, new_start - c.start, c.kind)
                };
                match kind {
                    ClipKind::Video => p.video_lanes = p.video_lanes.max(new_lane + 1),
                    ClipKind::Audio => p.audio_lanes = p.audio_lanes.max(new_lane + 1),
                }
                for c in &mut p.clips {
                    if c.id == id {
                        c.start = new_start;
                        c.lane = new_lane;
                    } else if group.is_some() && c.link_group == group {
                        // партнёры по A/V-линку двигаются тем же смещением
                        c.start = c.start + delta;
                    }
                }
            }
            // Тримы применяются ко ВСЕЙ link-группе (v1; п.6 «магнит
            // распадается»: трим края видео оставлял аудио-партнёра прежней
            // длины). Партнёры клампятся к своим ограничениям.
            // Реверс (п.16): у reversed клипа source_in живёт «с другого
            // конца» — TrimStart его НЕ трогает, TrimEnd сдвигает.
            Command::TrimStart { id, new_start } => {
                // опция «растягивать анимацию вместе с событием» (Р.40):
                // ВЫКЛЮЧЕНА по умолчанию — ключ остаётся на своём кадре
                let stretch = p.trim_stretches_auto;
                let group = p.clips.iter().find(|c| c.id == id).and_then(|c| c.link_group);
                for c in &mut p.clips {
                    if c.id != id && (group.is_none() || c.link_group != group) {
                        continue;
                    }
                    let max_start = c.start + c.duration - MIN_CLIP_DURATION;
                    let ns = Tick(new_start.0.clamp(trim_min_start(c).0, max_start.0));
                    // геометрия и материал — ОДНИМ правилом (Р.39):
                    // у растянутого клипа `source_in` едет по интегралу
                    // скорости, а не на длину куска
                    let (span_old, anchor_old) = (crate::clip_material_span(c), c.source_in);
                    *c = clip_slice(c, TickRange::new(ns, c.start + c.duration), c.id);
                    if stretch {
                        let span_new = crate::clip_material_span(c);
                        crate::stretch_auto_span(c, anchor_old, span_old, span_new);
                    }
                }
            }
            Command::TrimEnd { id, new_end } => {
                let stretch = p.trim_stretches_auto;
                let group = p.clips.iter().find(|c| c.id == id).and_then(|c| c.link_group);
                for c in &mut p.clips {
                    if c.id != id && (group.is_none() || c.link_group != group) {
                        continue;
                    }
                    let min_end = c.start + MIN_CLIP_DURATION;
                    let ne = Tick(new_end.0.max(min_end.0).min(trim_max_end(c).0));
                    let (span_old, anchor_old) = (crate::clip_material_span(c), c.source_in);
                    *c = clip_slice(c, TickRange::new(c.start, ne), c.id);
                    if stretch {
                        let span_new = crate::clip_material_span(c);
                        crate::stretch_auto_span(c, anchor_old, span_old, span_new);
                    }
                }
            }
            Command::Split { id, at, new_id, new_group } => {
                // правило кусков ОДНО на весь проект — `clip_slice`
                // (реверс, п.16: левая половина играет ХВОСТ источника)
                let c = Self::clip_mut_on(p, id).clone();
                let body = TickRange::new(c.start, c.start + c.duration);
                *Self::clip_mut_on(p, id) =
                    clip_slice(&c, TickRange::new(body.start, at), id);
                let mut right = clip_slice(&c, TickRange::new(at, body.end), new_id);
                right.link_group = new_group;
                p.clips.push(right);
                // D5 (v1): правая половина наследует таргетинг morpher'ов
                for m in &mut p.morphers {
                    if let Some(ids) = m.targets.clip_ids_mut()
                        && ids.contains(&id)
                    {
                        ids.push(new_id);
                    }
                }
                // переход, где клип был OUTGOING (стык на прежнем КОНЦЕ),
                // уезжает к правой половине; incoming-стык (начало) остаётся
                // за левой — она сохранила id
                for t in &mut p.transitions {
                    if t.a == Some(id) {
                        t.a = Some(new_id);
                    }
                }
            }
            Command::SetTrimStretchesAuto { on } => p.trim_stretches_auto = on,
            Command::SetGain { id, gain_db } => Self::clip_mut_on(p, id).gain_db = gain_db,
            Command::SetOpacity { id, opacity } => {
                Self::clip_mut_on(p, id).opacity = opacity.clamp(0.0, 1.0)
            }
            Command::SetFxOff { id, off } => Self::clip_mut_on(p, id).fx_off = off,
            Command::SetReversed { id, reversed } => Self::clip_mut_on(p, id).reversed = reversed,
            Command::SetFxBypass { id, bypass } => {
                Self::clip_mut_on(p, id).fx_bypass = bypass;
                sync_warp_to_link_group(p, id);
            }
            Command::SetFx { id, fx } => {
                let c = Self::clip_mut_on(p, id);
                // цепочка сменилась — кривые лишних слотов отпадают (Р.30);
                // перестановку слотов редактор ведёт сам и шлёт SetFxAuto
                // тем же жестом
                c.fx_auto.truncate(fx.len());
                c.fx = fx;
                sync_warp_to_link_group(p, id);
            }
            Command::SetFxAuto { id, auto } => {
                Self::clip_mut_on(p, id).fx_auto = auto;
                sync_warp_to_link_group(p, id);
            }
            Command::LinkClips { ids, group } => {
                for c in &mut p.clips {
                    if ids.contains(&c.id) {
                        c.link_group = Some(group);
                    }
                }
            }
            Command::UnlinkClip { id } => Self::clip_mut_on(p, id).link_group = None,
            Command::AddTransition(t) => p.transitions.push(t),
            Command::RemoveTransition { id } => p.transitions.retain(|t| t.id != id),
            Command::SetTransition(t) => {
                if let Some(slot) = p.transitions.iter_mut().find(|x| x.id == t.id) {
                    *slot = t;
                }
            }
            Command::AddMarker { at, duration } => {
                let i = p.markers.partition_point(|m| m.t < at);
                p.markers.insert(i, Marker { duration, ..Marker::at(at) });
            }
            Command::RemoveMarker { at } => p.markers.retain(|m| m.t != at),
            Command::SetMarkerName { at, name } => {
                if let Some(m) = p.markers.iter_mut().find(|m| m.t == at) {
                    m.name = name;
                }
            }
            Command::MoveMarker { from, to } => {
                // имя, длительность и цвет переезжают ВМЕСТЕ с меткой
                let moved =
                    p.markers.iter().find(|m| m.t == from).cloned().unwrap_or(Marker::at(from));
                p.markers.retain(|m| m.t != from);
                let i = p.markers.partition_point(|m| m.t < to);
                p.markers.insert(i, Marker { t: to, ..moved });
            }
            Command::SetMarkerDuration { at, duration } => {
                if let Some(m) = p.markers.iter_mut().find(|m| m.t == at) {
                    m.duration = duration;
                }
            }
            Command::InsertLane { kind, at } => {
                let mut rows = p.rows();
                match kind {
                    ClipKind::Video => p.video_lanes = (p.video_lanes.max(at) + 1).max(1),
                    ClipKind::Audio => p.audio_lanes = (p.audio_lanes.max(at) + 1).max(1),
                }
                for c in &mut p.clips {
                    if c.kind == kind && c.lane >= at {
                        c.lane += 1;
                    }
                }
                let lk = match kind {
                    ClipKind::Video => LaneKind::Video,
                    ClipKind::Audio => LaneKind::Audio,
                };
                lane_order_insert(&mut rows, lk, at);
                p.set_lane_order(&rows);
            }
            Command::SetLanes { video, audio } => {
                // порядок строк подтягивается сам: normalize_lane_order
                // выбрасывает исчезнувшие и дописывает появившиеся
                p.video_lanes = video.max(1);
                p.audio_lanes = audio.max(1);
                let cur = p.lane_order.clone();
                p.set_lane_order(&cur);
            }
            Command::SetLaneOrder { order } => p.set_lane_order(&order),
            Command::RemoveMedia { id } => p.media.retain(|m| m.id != id),
            Command::SetSettings { settings } => p.settings = settings,
            Command::AddMorpher(m) => {
                p.morph_lanes = p.morph_lanes.max(m.lane + 1);
                p.morphers.push(m);
                let cur = p.lane_order.clone();
                p.set_lane_order(&cur); // новая дорожка попадает в порядок
            }
            Command::SetMorpher(m) => {
                p.morph_lanes = p.morph_lanes.max(m.lane + 1);
                if let Some(slot) = p.morphers.iter_mut().find(|x| x.id == m.id) {
                    *slot = m;
                }
            }
            Command::RemoveMorpher { id } => p.morphers.retain(|m| m.id != id),
            Command::BakeMorpher { id } => {
                let Some(pos) = p.morphers.iter().position(|m| m.id == id) else { return };
                // «bake == то, что рисовалось» — значит РЕЗКА тоже
                // материализуется. Считаем ровно этот морфер: остальные
                // остаются жить и режут по-прежнему.
                let baked = {
                    let mut q = p.clone();
                    q.morphers.retain(|m| m.id == id);
                    q.effective_clips()
                };
                p.morphers.remove(pos);
                // синтетические id кусков → настоящие; кусок №0 держит id
                // оригинала (правило `Split`), остальные получают max+1…
                let mut next = p.clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 1;
                // (оригинал, новый id, конец куска) — для перевешивания
                let mut born: Vec<(ClipId, ClipId, Tick)> = Vec::new();
                let mut clips = Vec::with_capacity(baked.len());
                for mut c in baked {
                    if let Some(origin) = slice_origin(c.id) {
                        c.id = ClipId(next);
                        next += 1;
                        born.push((origin, c.id, c.start + c.duration));
                    }
                    clips.push(c);
                }
                let ends: Vec<(ClipId, Tick)> =
                    p.clips.iter().map(|c| (c.id, c.start + c.duration)).collect();
                p.clips = clips;
                if born.is_empty() {
                    return;
                }
                // переход, где разрезанный клип был OUTGOING, уезжает к
                // куску, которому достался стык (прежний КОНЕЦ тела)
                for t in &mut p.transitions {
                    let Some(a) = t.a else { continue };
                    let Some(&(_, end)) = ends.iter().find(|(i, _)| *i == a) else { continue };
                    if let Some(&(_, new, _)) =
                        born.iter().find(|(o, _, e)| *o == a && *e == end)
                    {
                        t.a = Some(new);
                    }
                }
                // куски наследуют таргетинг ОСТАЛЬНЫХ морферов (D5 v1)
                for m in &mut p.morphers {
                    if let Some(ids) = m.targets.clip_ids_mut() {
                        for (origin, new, _) in &born {
                            if ids.contains(origin) {
                                ids.push(*new);
                            }
                        }
                    }
                }
            }
            Command::AddArea(a) => p.areas.push(a),
            Command::RemoveArea { name } => {
                p.areas.retain(|a| a.name != name);
                // ссылка на удалённую область остаётся в файле и
                // резолвится в ничто: молча выкидывать её нельзя —
                // пользователь вернёт область тем же именем (правило 20)
            }
            Command::RenameArea { from, to } => {
                if let Some(a) = p.areas.iter_mut().find(|a| a.name == from) {
                    a.name = to.clone();
                }
                for m in &mut p.morphers {
                    if let MorphTargets::Areas { names } = &mut m.targets {
                        for n in names.iter_mut().filter(|n| **n == from) {
                            *n = to.clone();
                        }
                    }
                }
            }
            Command::SetAreaRange { name, start, duration } => {
                if let Some(a) = p.areas.iter_mut().find(|a| a.name == name) {
                    a.start = Tick(start.0.max(0));
                    a.duration = duration;
                }
            }
            Command::SetAreaLanes { name, lanes } => {
                if let Some(a) = p.areas.iter_mut().find(|a| a.name == name) {
                    a.lanes = lanes;
                }
            }
            Command::InsertMorphLane { at } => {
                let mut rows = p.rows();
                p.morph_lanes = (p.morph_lanes.max(at) + 1).max(1);
                for m in &mut p.morphers {
                    if m.lane >= at {
                        m.lane += 1;
                    }
                }
                // сдвиг display-строк — ВНУТРИ команды, которая знает `at`
                // (снаружи его звали руками, и половину случаев забывали)
                lane_order_insert(&mut rows, LaneKind::Morph, at);
                p.set_lane_order(&rows);
            }
            Command::RemoveMorphLane { at } => {
                let mut rows = p.rows();
                p.morphers.retain(|m| m.lane != at);
                for m in &mut p.morphers {
                    if m.lane > at {
                        m.lane -= 1;
                    }
                }
                p.morph_lanes = p.morph_lanes.saturating_sub(1);
                lane_order_remove(&mut rows, LaneKind::Morph, at);
                p.set_lane_order(&rows);
            }
            Command::AddCamera(c) => p.cameras.push(c),
            Command::SetCamera(c) => {
                if let Some(x) = p.cameras.iter_mut().find(|x| x.id == c.id) {
                    *x = c;
                }
            }
            Command::RemoveCamera { id } => p.cameras.retain(|c| c.id != id),
            Command::InsertCameraLane { at } => {
                let mut rows = p.rows();
                p.camera_lanes = (p.camera_lanes.max(at) + 1).max(1);
                for c in &mut p.cameras {
                    if c.lane >= at {
                        c.lane += 1;
                    }
                }
                lane_order_insert(&mut rows, LaneKind::Camera, at);
                p.set_lane_order(&rows);
            }
            Command::RemoveCameraLane { at } => {
                let mut rows = p.rows();
                p.cameras.retain(|c| c.lane != at);
                for c in &mut p.cameras {
                    if c.lane > at {
                        c.lane -= 1;
                    }
                }
                p.camera_lanes = p.camera_lanes.saturating_sub(1);
                lane_order_remove(&mut rows, LaneKind::Camera, at);
                p.set_lane_order(&rows);
            }
        }
    }

    /// Судьба переходов после правок геометрии клипов (Р.24, п.135):
    /// клип пропал или стык разорван (конец `a` != начало `b`, дорожка/вид
    /// разошлись) — переход умирает; трим укоротил соседа — офсеты клампятся
    /// телами, схлопнувшееся ниже минимума окно умирает. Групповой перенос
    /// обоих соседей стык сохраняет — переход живёт (идентичность по ClipId,
    /// позиция не хранится).
    fn reconcile_transitions(p: &mut Project) {
        let Project { clips, transitions, .. } = p;
        transitions.retain_mut(|t| {
            let side = |id: Option<ClipId>| match id {
                // сторона-пустота у фейда (Р.25) — законна и остаётся пустой
                None => Some(None),
                // клип стороны пропал: переход умирает целиком (фейды
                // заводятся явно, а не остатками от удаления)
                Some(id) => clips.iter().find(|c| c.id == id).map(Some),
            };
            let (Some(a), Some(b)) = (side(t.a), side(t.b)) else { return false };
            if let (Some(a), Some(b)) = (a, b)
                && (a.kind != b.kind || a.lane != b.lane || a.start + a.duration != b.start)
            {
                return false;
            }
            if let Some(a) = a {
                t.in_offset = t.in_offset.min(a.duration);
            }
            if let Some(b) = b {
                t.out_offset = t.out_offset.min(b.duration);
            }
            t.in_offset + t.out_offset >= MIN_TRANSITION_DURATION
        });
    }

    fn clip_mut_on(p: &mut Project, id: ClipId) -> &mut ClipEvent {
        p.clips.iter_mut().find(|c| c.id == id).unwrap()
    }
}
