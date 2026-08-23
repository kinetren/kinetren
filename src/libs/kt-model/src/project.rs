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
    /// Маркеры (сортированы по времени, без дублей).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<Tick>,
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
}

fn default_lanes() -> u32 {
    1
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
        /// Keyframe-автоматизация по прогрессу клипа t∈[0,1]; между точками
        /// линейная интерполяция, за краями — крайние значения.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        automation: Vec<SlowedKeyframe>,
    },
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
        }
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
        self.markers.sort_unstable();
        self.markers.dedup();
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
