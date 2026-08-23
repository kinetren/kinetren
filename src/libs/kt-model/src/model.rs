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
    /// Разрез в точке `at` (абсолютный тик). Правая половина получает новый id
    /// и новую link-группу (правые половины A/V-пары остаются связаны).
    Split {
        id: ClipId,
        at: Tick,
        new_id: ClipId,
        new_group: Option<GroupId>,
    },
    SetGain {
        id: ClipId,
        gain_db: f32,
    },
    /// Реверс события (п.16). Undo-able.
    SetReversed {
        id: ClipId,
        reversed: bool,
    },
    SetFx {
        id: ClipId,
        fx: Vec<FxInstance>,
    },
    LinkClips {
        ids: Vec<ClipId>,
        group: GroupId,
    },
    UnlinkClip {
        id: ClipId,
    },
    AddMarker {
        at: Tick,
    },
    RemoveMarker {
        at: Tick,
    },
    MoveMarker {
        from: Tick,
        to: Tick,
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
    #[error("split point outside clip")]
    SplitOutside,
    #[error("invalid clip: {0}")]
    InvalidClip(#[from] ProjectError),
    #[error("marker at {0:?} not found")]
    MarkerNotFound(Tick),
    #[error("marker at {0:?} already exists")]
    MarkerExists(Tick),
    #[error("media {0:?} is used by clips")]
    MediaInUse(MediaId),
    #[error("lane count would orphan clips")]
    LanesTooFew,
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
        self.apply_batch(vec![cmd])
    }

    /// Пакет команд = ОДИН undo-снапшот на жест (сплит A/V-пары, удаление
    /// группы, paste, punch-out). Атомарно: валидация всей серии на клоне,
    /// провал любой команды = ноль изменений и ноль записей в истории.
    pub fn apply_batch(&mut self, cmds: Vec<Command>) -> Result<(), CommandError> {
        if cmds.is_empty() {
            return Ok(());
        }
        let mut scratch = self.project.clone();
        for cmd in &cmds {
            Self::check_on(&scratch, cmd)?;
            Self::mutate_on(&mut scratch, cmd.clone());
        }
        debug_assert!(scratch.validate().is_ok());
        self.push_undo();
        self.project = scratch;
        self.last_gesture = None;
        Ok(())
    }

    /// Как apply, но серия вызовов с одним `gesture`-токеном сливается в
    /// одну undo-запись (live-редактирование параметров FX из окна).
    pub fn apply_coalesced(&mut self, cmd: Command, gesture: u64) -> Result<(), CommandError> {
        let mut scratch = self.project.clone();
        Self::check_on(&scratch, &cmd)?;
        Self::mutate_on(&mut scratch, cmd);
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
            | Command::SetReversed { id, .. }
            | Command::SetFx { id, .. }
            | Command::UnlinkClip { id }
            | Command::MoveClip { id, .. } => {
                find(*id)?;
            }
            Command::TrimStart { id, new_start } => {
                let c = find(*id)?;
                let end = c.start + c.duration;
                if end - *new_start < MIN_CLIP_DURATION {
                    return Err(CommandError::TooShort);
                }
                // source_in не может уйти в минус
                if (c.source_in + (*new_start - c.start)).0 < 0 {
                    return Err(CommandError::TooShort);
                }
            }
            Command::TrimEnd { id, new_end } => {
                let c = find(*id)?;
                if *new_end - c.start < MIN_CLIP_DURATION {
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
            Command::AddMarker { at } => {
                if p.markers.contains(at) {
                    return Err(CommandError::MarkerExists(*at));
                }
            }
            Command::RemoveMarker { at } => {
                if !p.markers.contains(at) {
                    return Err(CommandError::MarkerNotFound(*at));
                }
            }
            Command::MoveMarker { from, to } => {
                if !p.markers.contains(from) {
                    return Err(CommandError::MarkerNotFound(*from));
                }
                if p.markers.contains(to) && from != to {
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
            }
            Command::SetSettings { settings } => {
                if settings.fps == 0 || kt_time::TICK_HZ % settings.fps as i64 != 0 {
                    return Err(CommandError::InvalidClip(ProjectError::BadFps));
                }
            }
        }
        Ok(())
    }

    fn mutate_on(p: &mut Project, cmd: Command) {
        match cmd {
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
            Command::RemoveClip { id } => p.clips.retain(|c| c.id != id),
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
            Command::TrimStart { id, new_start } => {
                let c = Self::clip_mut_on(p, id);
                let delta = new_start - c.start;
                c.start = new_start;
                c.duration = c.duration - delta;
                c.source_in = c.source_in + delta;
            }
            Command::TrimEnd { id, new_end } => {
                let c = Self::clip_mut_on(p, id);
                c.duration = new_end - c.start;
            }
            Command::Split { id, at, new_id, new_group } => {
                let left = Self::clip_mut_on(p, id);
                let left_len = at - left.start;
                let mut right = left.clone();
                left.duration = left_len;
                right.id = new_id;
                right.link_group = new_group;
                right.start = at;
                right.duration = right.duration - left_len;
                right.source_in = right.source_in + left_len;
                p.clips.push(right);
            }
            Command::SetGain { id, gain_db } => Self::clip_mut_on(p, id).gain_db = gain_db,
            Command::SetReversed { id, reversed } => Self::clip_mut_on(p, id).reversed = reversed,
            Command::SetFx { id, fx } => Self::clip_mut_on(p, id).fx = fx,
            Command::LinkClips { ids, group } => {
                for c in &mut p.clips {
                    if ids.contains(&c.id) {
                        c.link_group = Some(group);
                    }
                }
            }
            Command::UnlinkClip { id } => Self::clip_mut_on(p, id).link_group = None,
            Command::AddMarker { at } => {
                let i = p.markers.partition_point(|&m| m < at);
                p.markers.insert(i, at);
            }
            Command::RemoveMarker { at } => p.markers.retain(|&m| m != at),
            Command::MoveMarker { from, to } => {
                p.markers.retain(|&m| m != from);
                let i = p.markers.partition_point(|&m| m < to);
                p.markers.insert(i, to);
            }
            Command::InsertLane { kind, at } => {
                match kind {
                    ClipKind::Video => p.video_lanes = (p.video_lanes.max(at) + 1).max(1),
                    ClipKind::Audio => p.audio_lanes = (p.audio_lanes.max(at) + 1).max(1),
                }
                for c in &mut p.clips {
                    if c.kind == kind && c.lane >= at {
                        c.lane += 1;
                    }
                }
            }
            Command::SetLanes { video, audio } => {
                p.video_lanes = video.max(1);
                p.audio_lanes = audio.max(1);
            }
            Command::RemoveMedia { id } => p.media.retain(|m| m.id != id),
            Command::SetSettings { settings } => p.settings = settings,
        }
    }

    fn clip_mut_on(p: &mut Project, id: ClipId) -> &mut ClipEvent {
        p.clips.iter_mut().find(|c| c.id == id).unwrap()
    }
}
