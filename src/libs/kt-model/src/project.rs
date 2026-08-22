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
    #[serde(default)]
    pub fx: Vec<FxInstance>,
}

/// Типизированные FX (не opaque JSON — валидация на загрузке, тест 12).
/// Наполняется в M6; заготовка нужна формату с v1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FxInstance {
    Gain { db: f32 },
    SlowedReverb { rate: f64, mix: f32, reverb_delay_ms: f32 },
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
}

impl Project {
    pub fn new(settings: ProjectSettings) -> Project {
        Project { format_version: FORMAT_VERSION, settings, media: Vec::new(), clips: Vec::new() }
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
        let p: Project = serde_json::from_str(s).map_err(|e| ProjectError::Json(e.to_string()))?;
        p.validate()?;
        Ok(p)
    }
}
