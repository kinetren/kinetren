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
                if let (
                    FxInstance::SlowedReverb { rate, .. },
                    FxInstance::SlowedReverb { rate: r0, .. },
                ) = (&mut c.fx[j], t)
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
    /// Смена пути медиа (Р.52): правка параметров генератора — это смена
    /// его `gen:`-пути, и все клипы этого медиа обновляются (семантика
    /// Vegas: два клипа на один генератор делят вид). Путь для модели
    /// непрозрачен, как в `AddMedia`, — команда годится и будущему
    /// relink. Пересоздание источника у плеера — забота call-site
    /// (`KtApp::sync_media_sources`), как и у `AddMedia`.
    SetMediaPath {
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
    // --- вложенные секвенции (Р.50) ---
    /// Завести пустую ленту. Содержимое приезжает отдельной командой:
    /// сворачивание — это пакет «завести, наполнить, убрать оригиналы,
    /// поставить экземпляр», и он обязан быть ОДНИМ шагом истории.
    AddSequence {
        id: MediaId,
        name: String,
    },
    /// Убрать ленту. Отказ, пока на неё смотрит хоть одно событие дерева:
    /// молча осиротить клипы дороже, чем сказать «занята».
    RemoveSequence {
        id: MediaId,
    },
    RenameSequence {
        id: MediaId,
        name: String,
    },
    /// Полная замена СОДЕРЖИМОГО ленты (Box — чтобы `Command` не разнесло
    /// по размеру ради одного варианта).
    SetSequenceContent {
        id: MediaId,
        content: Box<Sequence>,
    },
    /// Сменить источник события — файл или ленту. Несущая команда и
    /// сворачивания, и «сделать независимым».
    SetClipMedia {
        id: ClipId,
        media: MediaId,
    },
}

impl Command {
    /// Меняет ли команда то, что СЧИТАЮТ рендереры (Р.53, п.412).
    ///
    /// Закон П: путь медиа — свойство ИСТОЧНИКА, а не плана. Рендереры
    /// знают только `MediaId`, поэтому `SetMediaPath` не имеет права
    /// клонировать проект и пересобирать графы: его адресат — воркер
    /// этого медиа (`Player::retune_media`). До этого пункта каждое
    /// нажатие клавиши в поле титра останавливало аудио-render-поток.
    ///
    /// Match ИСЧЕРПЫВАЮЩИЙ и без `_ =>`: новая команда не соберётся,
    /// пока не ответили, трогает ли она план. Прецедент того же рода —
    /// `on_region_changed` без `set_project` (Р.43 п.296, сторож t286).
    /// Событие, которое команда правит, — если оно у неё одно и ЕСТЬ.
    ///
    /// Заведено ради одного вопроса: «в какой ЛЕНТЕ эта правка» (Р.55, С5).
    /// Перебирать варианты в вызывающем нельзя — это второе место, где
    /// знают устройство команд, и оно разошлось бы с первым.
    pub fn target_clip(&self) -> Option<ClipId> {
        match self {
            Command::AddClip(c) => Some(c.id),
            Command::RemoveClip { id }
            | Command::MoveClip { id, .. }
            | Command::TrimStart { id, .. }
            | Command::TrimEnd { id, .. }
            | Command::Split { id, .. }
            | Command::SetGain { id, .. }
            | Command::SetOpacity { id, .. }
            | Command::SetFxOff { id, .. }
            | Command::SetReversed { id, .. }
            | Command::SetFxBypass { id, .. }
            | Command::SetFx { id, .. }
            | Command::SetFxAuto { id, .. }
            | Command::UnlinkClip { id }
            | Command::SetClipMedia { id, .. } => Some(*id),
            _ => None,
        }
    }

    pub fn touches_render_plan(&self) -> bool {
        match self {
            Command::SetMediaPath { .. } => false,
            Command::NoteBirth { .. }
            | Command::AddMedia { .. }
            | Command::AddClip(_)
            | Command::RemoveClip { .. }
            | Command::MoveClip { .. }
            | Command::TrimStart { .. }
            | Command::TrimEnd { .. }
            | Command::Split { .. }
            | Command::SetTrimStretchesAuto { .. }
            | Command::SetGain { .. }
            | Command::SetOpacity { .. }
            | Command::SetFxOff { .. }
            | Command::SetReversed { .. }
            | Command::SetFxBypass { .. }
            | Command::SetFx { .. }
            | Command::SetFxAuto { .. }
            | Command::LinkClips { .. }
            | Command::UnlinkClip { .. }
            | Command::AddTransition(_)
            | Command::RemoveTransition { .. }
            | Command::SetTransition(_)
            | Command::AddMarker { .. }
            | Command::RemoveMarker { .. }
            | Command::SetMarkerName { .. }
            | Command::MoveMarker { .. }
            | Command::SetMarkerDuration { .. }
            | Command::InsertLane { .. }
            | Command::SetLanes { .. }
            | Command::RemoveMedia { .. }
            | Command::SetSettings { .. }
            | Command::AddMorpher(_)
            | Command::RemoveMorpher { .. }
            | Command::SetMorpher(_)
            | Command::BakeMorpher { .. }
            | Command::SetLaneOrder { .. }
            | Command::AddArea(_)
            | Command::RemoveArea { .. }
            | Command::RenameArea { .. }
            | Command::SetAreaRange { .. }
            | Command::SetAreaLanes { .. }
            | Command::InsertMorphLane { .. }
            | Command::RemoveMorphLane { .. }
            | Command::AddCamera(_)
            | Command::SetCamera(_)
            | Command::RemoveCamera { .. }
            | Command::InsertCameraLane { .. }
            | Command::RemoveCameraLane { .. }
            | Command::AddSequence { .. }
            | Command::RemoveSequence { .. }
            | Command::RenameSequence { .. }
            | Command::SetSequenceContent { .. }
            | Command::SetClipMedia { .. } => true,
        }
    }
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
    #[error("sequence {0:?} not found")]
    SequenceNotFound(MediaId),
    #[error("sequence {0:?} is still used by {1} clip(s)")]
    SequenceInUse(MediaId, usize),
    #[error("row order and lane count cannot change while a reel is expanded in place")]
    BandsOpen,
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("nothing to redo")]
    NothingToRedo,
}

/// Модель с историей. Движок читает `project()` (снапшот-клон при rebuild).
#[derive(Debug, Clone)]
pub struct TimelineModel {
    /// КАНОН: настоящая раскладка без раскрытых полос. Он уходит в файл,
    /// в плеер и в экспорт, и только он существует по-настоящему.
    project: Project,
    /// ПРОИЗВОДНОЕ: канон плюс раскрытые на месте ленты (Р.55, С5).
    ///
    /// Его отдаёт `project()`, то есть с ним работает весь интерфейс —
    /// отрисовка, хит-тест, снап, выделение, окна. Ни одна из этих рук не
    /// знает, что часть событий на экране живёт в другой ленте: они
    /// физически лежат в корневых векторах, только со смещением.
    /// При пустом `bands` это побайтовая копия канона, то есть прежнее
    /// поведение без единой лишней ветки.
    bench: Project,
    /// Раскрытые полосы. Рабочее место, а не работа: в файл не пишется
    /// (правило t390-t391), но КАЖДЫЙ undo-снимок помнит свои — иначе
    /// Ctrl+Z внутри раскрытия выглядел бы как «нажал, и всё пропало».
    bands: Vec<Band>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// Токен последнего coalesced-жеста (см. apply_coalesced).
    last_gesture: Option<u64>,
    /// Лента, которую сейчас редактируют (Р.50, В1). `None` — корень.
    ///
    /// Живёт В МОДЕЛИ, а не в проекте: это не монтаж, а место, где стоит
    /// человек. В файл не пишется (вид внутри ленты — рабочее место, а не
    /// работа) и в undo-снимок не входит — но КАЖДЫЙ снимок помнит, в
    /// каком контексте правка делалась.
    editing: Option<MediaId>,
}

/// Снимок истории: проект и контекст, в котором правка делалась (Р.50, В1).
///
/// Без контекста Ctrl+Z внутри ленты выглядел бы как «нажал — ничего не
/// произошло»: отменилась бы правка в другом месте, которого не видно.
#[derive(Debug, Clone)]
struct Snapshot {
    project: Project,
    scope: Option<MediaId>,
    /// Какие полосы были раскрыты в момент правки (Р.55, С5).
    bands: Vec<Band>,
}

impl Band {
    /// Пересчитать полосу после пакета команд (Р.55, С5).
    ///
    /// **Единственное место конструкции, где правильность держится на
    /// человеке, а не на построении.** Поэтому `match` ИСЧЕРПЫВАЮЩИЙ, без
    /// `_ =>`: новая команда не соберётся, пока строку не заполнили — тот
    /// же приём, что у `Command::touches_render_plain` и таблицы t253.
    ///
    /// Нумерацию дорожек меняют ровно шесть команд. Полоса лежит в ХВОСТЕ
    /// числового пространства, поэтому правило простое: дорожка вставлена
    /// или убрана НИЖЕ начала полосы — едет вся полоса (`base`); внутри
    /// полосы — меняется её размер (`len`).
    ///
    /// Ошибка здесь не падает ни одним ассертом сама: полоса «съезжает», и
    /// `unmerge_reel` начинает отвергать батчи — либо, при симметричной
    /// ошибке, молча кладёт событие не в ту ленту. Сторож — t568.
    pub fn after_batch(&mut self, cmds: &[Command]) {
        for cmd in cmds {
            match cmd {
                Command::InsertLane { kind, at } => {
                    let lk = match kind {
                        ClipKind::Video => LaneKind::Video,
                        ClipKind::Audio => LaneKind::Audio,
                    };
                    self.lane_inserted(lk, *at);
                }
                Command::InsertMorphLane { at } => self.lane_inserted(LaneKind::Morph, *at),
                Command::InsertCameraLane { at } => self.lane_inserted(LaneKind::Camera, *at),
                Command::RemoveMorphLane { at } => self.lane_removed(LaneKind::Morph, *at),
                Command::RemoveCameraLane { at } => self.lane_removed(LaneKind::Camera, *at),
                // `SetLanes` ПОЛОСУ НЕ ДВИГАЕТ, и это решение, а не
                // недосмотр. Команда задаёт число дорожек целиком, а клипы
                // при этом не перенумеровываются — значит граница полосы
                // (`base`) обязана остаться там, где события и лежат.
                //
                // Единственный, кто её шлёт, — удаление дорожки, и оно
                // сопровождается `MoveClip` для всех, кто выше. Поэтому
                // удалять дорожку при раскрытой полосе приложение не даёт:
                // сначала сворачивает раскрытие и говорит об этом.
                //
                // `SetLaneOrder` с Р.56 перенумеровывает содержимое (номер
                // дорожки = ранг строки, п.449) — на бенче это переставило
                // бы события полосы из ХВОСТА числового пространства, и
                // `unmerge_reel` разобрал бы их не по тем лентам. Обе
                // команды отвергает `simulate` при открытой полосе
                // (`CommandError::BandsOpen`), сюда они не доходят.
                Command::SetLanes { .. } | Command::SetLaneOrder { .. } => {
                    debug_assert!(false, "SetLanes/SetLaneOrder are refused while a band is open")
                }
                // Всё остальное номеров дорожек не двигает. Перечислено
                // поимённо НАРОЧНО: `_ =>` спрятал бы следующую команду,
                // которая начнёт их двигать.
                Command::NoteBirth { .. }
                | Command::AddMedia { .. }
                | Command::SetMediaPath { .. }
                | Command::AddClip(_)
                | Command::RemoveClip { .. }
                | Command::MoveClip { .. }
                | Command::TrimStart { .. }
                | Command::TrimEnd { .. }
                | Command::Split { .. }
                | Command::SetTrimStretchesAuto { .. }
                | Command::SetGain { .. }
                | Command::SetOpacity { .. }
                | Command::SetFxOff { .. }
                | Command::SetReversed { .. }
                | Command::SetFxBypass { .. }
                | Command::SetFx { .. }
                | Command::SetFxAuto { .. }
                | Command::LinkClips { .. }
                | Command::UnlinkClip { .. }
                | Command::AddTransition(_)
                | Command::RemoveTransition { .. }
                | Command::SetTransition(_)
                | Command::AddMarker { .. }
                | Command::RemoveMarker { .. }
                | Command::SetMarkerName { .. }
                | Command::MoveMarker { .. }
                | Command::SetMarkerDuration { .. }
                | Command::RemoveMedia { .. }
                | Command::SetSettings { .. }
                | Command::AddMorpher(_)
                | Command::RemoveMorpher { .. }
                | Command::SetMorpher(_)
                | Command::BakeMorpher { .. }
                | Command::AddArea(_)
                | Command::RemoveArea { .. }
                | Command::RenameArea { .. }
                | Command::SetAreaRange { .. }
                | Command::SetAreaLanes { .. }
                | Command::AddCamera(_)
                | Command::SetCamera(_)
                | Command::RemoveCamera { .. }
                | Command::AddSequence { .. }
                | Command::RemoveSequence { .. }
                | Command::RenameSequence { .. }
                | Command::SetSequenceContent { .. }
                | Command::SetClipMedia { .. } => {}
            }
        }
    }

    fn lane_inserted(&mut self, kind: LaneKind, at: u32) {
        let base = self.base.get(kind);
        if at <= base {
            *self.base.get_mut(kind) += 1;
        } else if at <= base + self.len.get(kind) {
            // ХВОСТ полосы принадлежит ПОЛОСЕ (Р.56): за ней ничего не
            // лежит по построению, а вставка ровно в `base + len` —
            // это «ленте нужна ещё одна дорожка» (вставка в раскрытии).
            // До Р.56 такая дорожка не доставалась никому, и событие на
            // ней молча уезжало в корень.
            *self.len.get_mut(kind) += 1;
        }
    }

    fn lane_removed(&mut self, kind: LaneKind, lane: u32) {
        let base = self.base.get(kind);
        if lane < base {
            *self.base.get_mut(kind) = base.saturating_sub(1);
        } else if lane < base + self.len.get(kind) {
            *self.len.get_mut(kind) = self.len.get(kind).saturating_sub(1);
        }
    }
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
        TimelineModel {
            bench: project.clone(),
            project,
            bands: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            last_gesture: None,
            editing: None,
        }
    }

    /// Пересобрать рабочий вид из канона. Зовётся ПОСЛЕ каждой правки
    /// канона — включая правки полей вне истории (регион, loop, ui).
    fn rebuild_bench(&mut self) {
        self.bench = self.project.clone();
        for b in &self.bands {
            self.bench.merge_reel(b);
        }
    }

    /// Раскрытые полосы.
    pub fn bands(&self) -> &[Band] {
        &self.bands
    }

    /// Раскрыть ленту экземпляра НА МЕСТЕ (Р.55, С5). `false` — это не
    /// вложение, либо оно уже раскрыто.
    pub fn expand_band(&mut self, inst: ClipId) -> bool {
        if self.bands.iter().any(|b| b.inst == inst) {
            return false;
        }
        // полоса считается от КАНОНА: `base` обязан быть числом корневых
        // дорожек, а не «сколько их сейчас на экране вместе с чужой полосой»
        let Some(b) = Band::of(&self.project, inst) else { return false };
        self.bands.push(b);
        self.rebuild_bench();
        true
    }

    /// Свернуть раскрытие обратно.
    pub fn collapse_band(&mut self, inst: ClipId) -> bool {
        let Some(i) = self.bands.iter().position(|b| b.inst == inst) else { return false };
        self.bands.remove(i);
        self.rebuild_bench();
        true
    }

    pub fn collapse_all_bands(&mut self) {
        if self.bands.is_empty() {
            return;
        }
        self.bands.clear();
        self.rebuild_bench();
    }

    /// Чья это строка: корневая или раскрытой полосы.
    pub fn band_of_row(&self, kind: LaneKind, lane: u32) -> Option<&Band> {
        self.bands.iter().find(|b| b.owns(kind, lane))
    }

    /// Лента, которую сейчас редактируют. `None` — корень.
    pub fn editing(&self) -> Option<MediaId> {
        self.editing
    }

    /// Войти в ленту (или выйти в корень). Несуществующий адрес —
    /// молчаливый выход в корень: лента могла исчезнуть под undo.
    pub fn set_editing(&mut self, seq: Option<MediaId>) {
        let want = seq.filter(|id| {
            // адрес ищем в ДОКУМЕНТЕ: стоя внутри X, сам X в
            // `project.sequences` держит содержимое корня, но id у него
            // прежний — этого и достаточно
            self.project.sequence(*id).is_some()
        });
        if want == self.editing {
            return;
        }
        // Полный вход и раскрытие на месте — ДВА способа смотреть внутрь,
        // и одновременно они бессмысленны: полоса считалась от корня, а
        // корнем станет другая лента.
        self.bands.clear();
        if let Some(cur) = self.editing {
            self.project.swap_reel(cur);
        }
        if let Some(next) = want {
            self.project.swap_reel(next);
        }
        self.editing = want;
        self.last_gesture = None;
        self.rebuild_bench();
    }

    /// Документ, в котором ТЕКУЩАЯ лента стоит КОРНЕМ (Р.50, В1).
    ///
    /// Нужен офлайн-путям, считающим по времени того, что под руками:
    /// громкость, дакинг, превью материала. Рабочий вид им отдавать
    /// нельзя — в нём лента, внутри которой стоим, держит содержимое
    /// корня и выглядит вложенной в саму себя.
    pub fn reel_document(&self) -> Project {
        let mut p = self.project.clone();
        if let Some(cur) = self.editing {
            p.sequences.retain(|s| s.id != cur);
        }
        p
    }

    /// Проект КАК ЕГО ВИДИТ ЧЕЛОВЕК: содержимое ленты, в которой он сейчас
    /// стоит, лежит на месте корня (Р.50, В1), а раскрытые на месте ленты
    /// разложены на дорожки со смещением (Р.55, С5).
    ///
    /// Весь интерфейс — отрисовка, хит-тест, планирование жестов, снап,
    /// выделение, окна FX и морфера — работает с этим видом и о вложениях
    /// не знает вовсе. Тому, кому нужен ДОКУМЕНТ (движок, сохранение,
    /// экспорт), нужен `document()`; сторож — сканирующий тест t456b.
    ///
    /// Имя НЕ переименовывается в `bench` намеренно: `project()` зовут
    /// 182 раза, и все эти вызовы правильны — им нужен именно рабочий вид.
    /// Переименование ничего не прояснило бы, а различие «канон против
    /// вида» живёт в доке полей и в t456b.
    #[allow(clippy::misnamed_getters)]
    pub fn project(&self) -> &Project {
        &self.bench
    }

    /// ДОКУМЕНТ в настоящей раскладке: корень корнем, ленты лентами.
    ///
    /// Именно он уходит в плеер, в файл и в экспорт. Отдать туда рабочий
    /// вид значило бы записать на диск проект, у которого содержимое
    /// корня и ленты поменялись местами — потеря работы, которую человек
    /// заметил бы только при следующем открытии.
    pub fn document(&self) -> Project {
        let mut p = self.project.clone();
        if let Some(seq) = self.editing {
            p.swap_reel(seq);
        }
        p
    }

    /// Событие ПО ТОМУ, ЧТО ВИДИТ ЧЕЛОВЕК (Р.55, С5).
    ///
    /// Ищет в рабочем виде, а не в каноне: интерфейс спрашивает про то, по
    /// чему щёлкнул, а событие раскрытой полосы в каноне лежит внутри
    /// ленты. Искал бы здесь канон — каждый жест по полосе отвечал бы
    /// «такого события нет», причём молча.
    pub fn clip(&self, id: ClipId) -> Option<&ClipEvent> {
        self.bench.clips.iter().find(|c| c.id == id)
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
        // ПОЛОСЫ могли пережить правку с другими номерами дорожек
        // (`InsertLane` в корне двигает всю полосу вниз): пересчёт — там
        // же, где применяется результат, и только один раз.
        for b in &mut self.bands {
            b.after_batch(&cmds);
        }
        self.bands.retain(|b| self.project.clips.iter().any(|c| c.id == b.inst));
        self.rebuild_bench();
        self.last_gesture = None;
        Ok(births)
    }

    /// Проект, каким он СТАНЕТ после батча — без записи в undo (Р.26).
    /// Нужен call-site'ам, которым надо посчитать что-то по будущему
    /// состоянию: возврат «ручек» кроссфейда считает свободное место уже
    /// после того, как клип уехал (ops::restore_handles).
    pub fn preview(&self, cmds: &[Command]) -> Result<Project, CommandError> {
        // РАБОЧИЙ ВИД будущего, а не канон: вызывающий сравнивает его с
        // `project()` (возврат «ручек» кроссфейда, Р.26). Сравни он канон
        // с бенчем — и материал начал бы срезаться у чужих событий тихо и
        // с undo.
        let canon = self.simulate(cmds)?;
        let mut bench = canon;
        for b in &self.bands {
            bench.merge_reel(b);
        }
        Ok(bench)
    }

    fn simulate(&self, cmds: &[Command]) -> Result<Project, CommandError> {
        // ПОРЯДОК И ЧИСЛО СТРОК при открытой полосе не меняются (Р.56,
        // п.449). С Р.56 `SetLaneOrder` перенумеровывает содержимое, а
        // полоса раскрытия намеренно живёт в ХВОСТЕ числового
        // пространства: перенумеруй мы бенч — `unmerge_reel` разложил бы
        // события не по тем лентам, причём МОЛЧА. Приложение сворачивает
        // раскрытие само и говорит об этом.
        if !self.bands.is_empty()
            && cmds
                .iter()
                .any(|c| matches!(c, Command::SetLaneOrder { .. } | Command::SetLanes { .. }))
        {
            return Err(CommandError::BandsOpen);
        }
        // Скретч уже в РАБОЧЕМ виде: содержимое текущей ленты стоит на
        // месте корня (Р.50, В1), и команда правит его, не зная о
        // вложениях вовсе.
        let mut scratch = self.bench.clone();
        let run = |scratch: &mut Project| -> Result<(), CommandError> {
            for cmd in cmds {
                Self::check_on(scratch, cmd)?;
                Self::mutate_on(scratch, cmd.clone());
            }
            Ok(())
        };
        let res = run(&mut scratch);
        // reconcile — ОДИН раз на весь батч: жест атомарен, и переход не
        // должен умирать в переходном состоянии между командами (roll стыка
        // = TrimEnd+TrimStart: между ними стык формально разорван)
        Self::reconcile_transitions(&mut scratch);
        res?;
        // РАСКРЫТЫЕ ПОЛОСЫ собираются обратно в свои ленты (Р.55, С5) —
        // в обратном порядке, потому что каждая следующая раскрывалась
        // поверх уже раскрытых. Ошибка здесь значит, что жест вынес
        // событие через границу лент: батч отвергается ЦЕЛИКОМ, а не
        // «доправляется».
        let mut bands = self.bands.clone();
        for b in &mut bands {
            b.after_batch(cmds);
        }
        for b in bands.iter().rev() {
            scratch.unmerge_reel(b).map_err(CommandError::InvalidLayout)?;
        }
        // Валидация — ПО ДОКУМЕНТУ: в рабочем виде экземпляр ленты, в
        // которой стоим, лежит внутри неё самой и выглядит циклом.
        if let Some(seq) = self.editing {
            scratch.swap_reel(seq);
        }
        // Жёсткая валидация результата (Р.24, этап 6): перекрытия после
        // групповых мувов/тримов/вставок дешевле отловить одной проверкой,
        // чем дублировать их логику в check_on. Провал = ноль изменений.
        let ok = scratch.validate().map_err(CommandError::InvalidLayout);
        if let Some(seq) = self.editing {
            scratch.swap_reel(seq);
        }
        ok?;
        Ok(scratch)
    }

    /// Как apply, но серия вызовов с одним `gesture`-токеном сливается в
    /// одну undo-запись (live-редактирование параметров FX из окна).
    pub fn apply_coalesced(&mut self, cmd: Command, gesture: u64) -> Result<(), CommandError> {
        let scratch = self.simulate(std::slice::from_ref(&cmd))?;
        if self.last_gesture != Some(gesture) {
            self.push_undo();
        }
        self.project = scratch;
        for b in &mut self.bands {
            b.after_batch(std::slice::from_ref(&cmd));
        }
        self.bands.retain(|b| self.project.clips.iter().any(|c| c.id == b.inst));
        self.rebuild_bench();
        self.last_gesture = Some(gesture);
        Ok(())
    }

    fn push_undo(&mut self) {
        self.undo.push(Snapshot {
            project: self.project.clone(),
            scope: self.editing,
            bands: self.bands.clone(),
        });
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Регион ВНЕ undo-истории (семантика старого редактора).
    pub fn set_region(&mut self, r: Option<Region>) {
        self.project.region = r;
        self.bench.region = r;
    }

    pub fn set_loop_enabled(&mut self, on: bool) {
        self.project.loop_enabled = on;
        self.bench.loop_enabled = on;
    }

    /// Вьюпорт (вне undo, как регион) — сохраняется в файл проекта.
    pub fn set_ui_state(&mut self, ui: Option<crate::UiState>) {
        self.project.ui = ui.clone();
        self.bench.ui = ui;
    }

    pub fn undo(&mut self) -> Result<(), CommandError> {
        let mut prev = self.undo.pop().ok_or(CommandError::NothingToUndo)?;
        // регион/loop не откатываются — переносим текущие значения
        prev.project.region = self.project.region;
        prev.project.loop_enabled = self.project.loop_enabled;
        // КОНТЕКСТ едет вместе с ПРАВКОЙ, а не с тем, где человек стоял в
        // момент отмены (Р.50, В1): отменяется то, что он сделал ТАМ —
        // значит и показать надо то место. И redo-запись помнит ТОТ ЖЕ
        // контекст, иначе обратный ход уводил бы не туда.
        let here = Snapshot {
            project: self.project.clone(),
            scope: prev.scope,
            bands: self.bands.clone(),
        };
        self.project = prev.project;
        // Снимок уже несёт раскладку СВОЕГО контекста — обмен тут был бы
        // вторым, и содержимое уехало бы не туда.
        self.editing = prev.scope.filter(|id| self.project.sequence(*id).is_some());
        // ПОЛОСЫ едут вместе с контекстом по той же причине, что и он:
        // отменяется правка, сделанная В РАСКРЫТИИ, — значит и показать
        // надо раскрытие, иначе Ctrl+Z выглядит как «нажал, и всё пропало».
        self.bands = prev.bands;
        self.bands.retain(|b| self.project.clips.iter().any(|c| c.id == b.inst));
        self.rebuild_bench();
        self.redo.push(here);
        self.last_gesture = None;
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), CommandError> {
        let mut next = self.redo.pop().ok_or(CommandError::NothingToRedo)?;
        next.project.region = self.project.region;
        next.project.loop_enabled = self.project.loop_enabled;
        let here = Snapshot {
            project: self.project.clone(),
            scope: next.scope,
            bands: self.bands.clone(),
        };
        self.project = next.project;
        self.editing = next.scope.filter(|id| self.project.sequence(*id).is_some());
        self.bands = next.bands;
        self.bands.retain(|b| self.project.clips.iter().any(|c| c.id == b.inst));
        self.rebuild_bench();
        self.undo.push(here);
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
            Command::SetMediaPath { id, .. } => {
                if !p.media.iter().any(|m| m.id == *id) {
                    return Err(CommandError::MediaNotFound(*id));
                }
            }
            Command::AddClip(c) => {
                if p.clips.iter().any(|x| x.id == c.id) {
                    return Err(CommandError::IdInUse(c.id.0));
                }
                // от КЛИПА к источнику — только через `Project::source()`:
                // он смотрит и в файлы, и во вложенные ленты (Р.50)
                if p.source(c.media).is_none() {
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
                // ПО ВСЕМУ ДЕРЕВУ, а не только по корню (Р.50): файл, стоящий
                // лишь внутри ленты, удалился бы молча, и события повисли бы
                // на несуществующем адресе — всплыло бы это через неделю
                if p.sequence_uses(*id) > 0 {
                    return Err(CommandError::MediaInUse(*id));
                }
                // morpher с подменой на это медиа тоже держит его
                if p.morphers
                    .iter()
                    .chain(p.sequences.iter().flat_map(|s| s.morphers.iter()))
                    .any(|m| m.media_refs().any(|x| x == *id))
                {
                    return Err(CommandError::MediaInUse(*id));
                }
            }
            Command::AddSequence { id, name } => {
                if p.source(*id).is_some() {
                    return Err(CommandError::IdInUse(id.0));
                }
                if name.trim().is_empty() {
                    return Err(CommandError::InvalidClip(ProjectError::SequenceUnnamed(*id)));
                }
            }
            Command::RemoveSequence { id } => {
                if p.sequence(*id).is_none() {
                    return Err(CommandError::SequenceNotFound(*id));
                }
                let used = p.sequence_uses(*id);
                if used > 0 {
                    return Err(CommandError::SequenceInUse(*id, used));
                }
            }
            Command::RenameSequence { id, name } => {
                if p.sequence(*id).is_none() {
                    return Err(CommandError::SequenceNotFound(*id));
                }
                if name.trim().is_empty() {
                    return Err(CommandError::InvalidClip(ProjectError::SequenceUnnamed(*id)));
                }
            }
            Command::SetSequenceContent { id, content } => {
                if p.sequence(*id).is_none() {
                    return Err(CommandError::SequenceNotFound(*id));
                }
                if content.id != *id {
                    return Err(CommandError::SequenceNotFound(content.id));
                }
                // дорожек в ленте не меньше, чем занято её событиями:
                // «SetLanes не осиротит клипы» обязано выполняться и здесь
                let need = |k: ClipKind| {
                    content.clips.iter().filter(|c| c.kind == k).map(|c| c.lane + 1).max()
                };
                if content.video_lanes < need(ClipKind::Video).unwrap_or(1).max(1)
                    || content.audio_lanes < need(ClipKind::Audio).unwrap_or(1).max(1)
                {
                    return Err(CommandError::LanesTooFew);
                }
            }
            Command::SetClipMedia { id, media } => {
                if !p.clips.iter().any(|c| c.id == *id) {
                    return Err(CommandError::ClipNotFound(*id));
                }
                if p.source(*media).is_none() {
                    return Err(CommandError::MediaNotFound(*media));
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
            Command::SetMediaPath { id, path } => {
                if let Some(m) = p.media.iter_mut().find(|m| m.id == id) {
                    m.path = path;
                }
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
                *Self::clip_mut_on(p, id) = clip_slice(&c, TickRange::new(body.start, at), id);
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
            Command::AddSequence { id, name } => p.sequences.push(Sequence::new(id, name)),
            Command::RemoveSequence { id } => p.sequences.retain(|s| s.id != id),
            Command::RenameSequence { id, name } => {
                if let Some(s) = p.sequence_mut(id) {
                    s.name = name;
                }
            }
            Command::SetSequenceContent { id, content } => {
                if let Some(slot) = p.sequences.iter_mut().find(|s| s.id == id) {
                    *slot = *content;
                }
            }
            Command::SetClipMedia { id, media } => Self::clip_mut_on(p, id).media = media,
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
                    // Вложения при bake НЕ раскрываются (Р.50): морфер
                    // материализует свою резку по клипам ЭТОЙ ленты, а
                    // экземпляр вложения — такой же клип. Раскрытие
                    // подменило бы его содержимым и потеряло бы само
                    // событие.
                    q.sequences.clear();
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
                    if let Some(&(_, new, _)) = born.iter().find(|(o, _, e)| *o == a && *e == end) {
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
            Command::AddCamera(c) => {
                // строка обязана СУЩЕСТВОВАТЬ, иначе камера легла бы на
                // невидимую дорожку: создать можно, управлять нельзя.
                // То же правило, что у `MoveClip` для видео и аудио.
                p.camera_lanes = p.camera_lanes.max(c.lane + 1);
                p.cameras.push(c);
            }
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
    /// Переходы сверяются с содержимым КАЖДОЙ ленты (Р.50): переход живёт
    /// на той ленте, где лежат его концы, и сверять его с чужими клипами —
    /// значит убить его на первой же команде.
    fn reconcile_transitions(p: &mut Project) {
        let Project { clips, transitions, sequences, .. } = p;
        reconcile_reel(clips, transitions);
        for s in sequences {
            reconcile_reel(&s.clips, &mut s.transitions);
        }
    }

    fn clip_mut_on(p: &mut Project, id: ClipId) -> &mut ClipEvent {
        p.clips.iter_mut().find(|c| c.id == id).unwrap()
    }
}

fn reconcile_reel(clips: &[ClipEvent], transitions: &mut Vec<TransitionEvent>) {
    transitions.retain_mut(|t| {
        let side = |id: Option<ClipId>| match id {
            // сторона-пустота у фейда (Р.25) — законна и остаётся пустой
            None => Some(None),
            // клип стороны пропал: переход умирает целиком (фейды
            // заводятся явно, а не остатками от удаления)
            Some(id) => clips.iter().find(|c| c.id == id).map(Some),
        };
        let (Some(mut a), Some(mut b)) = (side(t.a), side(t.b)) else { return false };
        if let (Some(ca), Some(cb)) = (a, b) {
            // канонизация ролей (Р.51, п.399): Split и мувы могли поменять
            // порядок пары — «a» всегда ранний. Направленный вид при этом
            // зеркалится вместе с ролями; смерть перехода была бы хуже
            // (правило 20)
            if cb.start < ca.start {
                std::mem::swap(&mut t.a, &mut t.b);
                std::mem::swap(&mut a, &mut b);
            }
            let (ca, cb) = (a.unwrap(), b.unwrap());
            let (a_end, b_end) = (ca.start + ca.duration, cb.start + cb.duration);
            if ca.kind != cb.kind || ca.lane != cb.lane {
                return false;
            }
            if a_end != cb.start {
                // не встык: жив только как НАЛОЖЕННЫЙ переход (Р.51) —
                // чистая «лесенка» с окном не короче минимума и без
                // третьего клипа в нём (третий вполз мувом → переход
                // умирает, батч не валится — прецедент разорванного стыка)
                let stag = ca.start < cb.start && cb.start < a_end && a_end < b_end;
                if !stag || a_end - cb.start < MIN_TRANSITION_DURATION {
                    return false;
                }
                let busy = clips.iter().any(|c| {
                    c.id != ca.id
                        && c.id != cb.id
                        && c.kind == ca.kind
                        && c.lane == ca.lane
                        && c.start < a_end
                        && cb.start < c.start + c.duration
                });
                // офсеты у наложенного не клампятся — они мёртвый груз и
                // оживают при возврате пары встык
                return !busy;
            }
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
