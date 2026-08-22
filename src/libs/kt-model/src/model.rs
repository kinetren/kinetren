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
    AddMedia { id: MediaId, path: String },
    AddClip(ClipEvent),
    RemoveClip { id: ClipId },
    /// Двигает клип (и всю его link-группу — тем же смещением).
    MoveClip { id: ClipId, new_start: Tick, new_lane: u32 },
    /// Триммит начало: start/duration/source_in двигаются согласованно.
    TrimStart { id: ClipId, new_start: Tick },
    TrimEnd { id: ClipId, new_end: Tick },
    /// Разрез в точке `at` (абсолютный тик). Правая половина получает новый id
    /// и новую link-группу (правые половины A/V-пары остаются связаны).
    Split { id: ClipId, at: Tick, new_id: ClipId, new_group: Option<GroupId> },
    SetGain { id: ClipId, gain_db: f32 },
    SetFx { id: ClipId, fx: Vec<FxInstance> },
    LinkClips { ids: Vec<ClipId>, group: GroupId },
    UnlinkClip { id: ClipId },
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
}

impl TimelineModel {
    pub fn new(project: Project) -> TimelineModel {
        TimelineModel { project, undo: Vec::new(), redo: Vec::new() }
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
        // 1) валидация без мутаций
        self.check(&cmd)?;
        // 2) снапшот — только теперь, когда команда точно применима
        self.undo.push(self.project.clone());
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
        // 3) мутация (не может провалиться после check)
        self.mutate(cmd);
        debug_assert!(self.project.validate().is_ok());
        Ok(())
    }

    pub fn undo(&mut self) -> Result<(), CommandError> {
        let prev = self.undo.pop().ok_or(CommandError::NothingToUndo)?;
        self.redo.push(std::mem::replace(&mut self.project, prev));
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), CommandError> {
        let next = self.redo.pop().ok_or(CommandError::NothingToRedo)?;
        self.undo.push(std::mem::replace(&mut self.project, next));
        Ok(())
    }

    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    fn check(&self, cmd: &Command) -> Result<(), CommandError> {
        let find = |id: ClipId| self.clip(id).ok_or(CommandError::ClipNotFound(id));
        match cmd {
            Command::AddMedia { id, .. } => {
                if self.project.media.iter().any(|m| m.id == *id) {
                    return Err(CommandError::IdInUse(id.0));
                }
            }
            Command::AddClip(c) => {
                if self.project.clips.iter().any(|x| x.id == c.id) {
                    return Err(CommandError::IdInUse(c.id.0));
                }
                if !self.project.media.iter().any(|m| m.id == c.media) {
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
                if self.project.clips.iter().any(|x| x.id == *new_id) {
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
        }
        Ok(())
    }

    fn mutate(&mut self, cmd: Command) {
        match cmd {
            Command::AddMedia { id, path } => {
                self.project.media.push(MediaRef { id, path });
            }
            Command::AddClip(c) => self.project.clips.push(c),
            Command::RemoveClip { id } => self.project.clips.retain(|c| c.id != id),
            Command::MoveClip { id, new_start, new_lane } => {
                let (group, delta) = {
                    let c = self.clip(id).unwrap();
                    (c.link_group, new_start - c.start)
                };
                for c in &mut self.project.clips {
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
                let c = self.clip_mut(id);
                let delta = new_start - c.start;
                c.start = new_start;
                c.duration = c.duration - delta;
                c.source_in = c.source_in + delta;
            }
            Command::TrimEnd { id, new_end } => {
                let c = self.clip_mut(id);
                c.duration = new_end - c.start;
            }
            Command::Split { id, at, new_id, new_group } => {
                let left = self.clip_mut(id);
                let left_len = at - left.start;
                let mut right = left.clone();
                left.duration = left_len;
                right.id = new_id;
                right.link_group = new_group;
                right.start = at;
                right.duration = right.duration - left_len;
                right.source_in = right.source_in + left_len;
                self.project.clips.push(right);
            }
            Command::SetGain { id, gain_db } => self.clip_mut(id).gain_db = gain_db,
            Command::SetFx { id, fx } => self.clip_mut(id).fx = fx,
            Command::LinkClips { ids, group } => {
                for c in &mut self.project.clips {
                    if ids.contains(&c.id) {
                        c.link_group = Some(group);
                    }
                }
            }
            Command::UnlinkClip { id } => self.clip_mut(id).link_group = None,
        }
    }

    fn clip_mut(&mut self, id: ClipId) -> &mut ClipEvent {
        self.project.clips.iter_mut().find(|c| c.id == id).unwrap()
    }
}
