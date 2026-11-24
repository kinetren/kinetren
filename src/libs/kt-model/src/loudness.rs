//! The project's loudness target and its program stage (loudness-norms, the owner's B6: "на уровне
//! проекта нормализация а не в пресетах рендера"): the document's, never a reel's nor a render
//! preset's. The stage is absent until the person asks for it.

use crate::project::{ClipEvent, ClipId, ClipKind, LaneKind, MediaId, Project};
use crate::published::Override;
use serde::{Deserialize, Serialize};

/// The delivery the project is mixed for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoudnessStandard {
    #[default]
    EbuR128,
    AtscA85,
    Streaming,
    Podcast,
    Custom,
}

impl LoudnessStandard {
    pub const ALL: [LoudnessStandard; 5] = [
        LoudnessStandard::EbuR128,
        LoudnessStandard::AtscA85,
        LoudnessStandard::Streaming,
        LoudnessStandard::Podcast,
        LoudnessStandard::Custom,
    ];

    /// The standard's integrated loudness (LUFS) and true-peak ceiling (dBTP); `None` — Custom.
    pub fn numbers(self) -> Option<(f64, f64)> {
        Some(match self {
            LoudnessStandard::EbuR128 => (-23.0, -1.0),
            LoudnessStandard::AtscA85 => (-24.0, -2.0),
            LoudnessStandard::Streaming => (-14.0, -1.0),
            LoudnessStandard::Podcast => (-16.0, -1.0),
            LoudnessStandard::Custom => return None,
        })
    }
}

/// What the meters, the render report and every match read.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LoudnessTarget {
    pub standard: LoudnessStandard,
    pub target_lufs: f64,
    pub ceiling_dbtp: f64,
}

/// The ranges a target takes.
pub const TARGET_LUFS: std::ops::RangeInclusive<f64> = -40.0..=-5.0;
pub const CEILING_DBTP: std::ops::RangeInclusive<f64> = -9.0..=0.0;
/// The range of the program stage's gain.
pub const PROGRAM_GAIN_DB: std::ops::RangeInclusive<f64> = -24.0..=24.0;

impl Default for LoudnessTarget {
    fn default() -> Self {
        LoudnessTarget::of(LoudnessStandard::EbuR128)
    }
}

impl LoudnessTarget {
    /// A standard's target; Custom starts at the default's numbers.
    pub fn of(standard: LoudnessStandard) -> LoudnessTarget {
        let (target_lufs, ceiling_dbtp) = standard.numbers().unwrap_or((-23.0, -1.0));
        LoudnessTarget { standard, target_lufs, ceiling_dbtp }
    }

    pub fn is_default(&self) -> bool {
        *self == LoudnessTarget::default()
    }

    /// In range, and a named standard carries its own numbers.
    pub fn valid(&self) -> bool {
        TARGET_LUFS.contains(&self.target_lufs)
            && CEILING_DBTP.contains(&self.ceiling_dbtp)
            && self.standard.numbers().is_none_or(|n| n == (self.target_lufs, self.ceiling_dbtp))
    }
}

/// The stage on the master: a gain and a true-peak limiter at the project's ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ProgramStage {
    pub gain_db: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured: Option<ProgramMeasured>,
}

/// What "Match the program" found.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ProgramMeasured {
    /// The mix before the stage.
    pub lufs: f64,
    pub dbtp: f64,
    /// The loudness after the stage: short of the target when the limiter holds it back.
    pub reached: f64,
    /// What it was matched to and the gain found: another target, ceiling or gain — changed since
    /// measured.
    pub target: f64,
    pub ceiling: f64,
    pub gain_db: f64,
    /// What was heard then (`kt_engine::program::sound`): another value — changed since measured.
    pub sound: u64,
}

impl ProgramStage {
    pub fn valid(&self) -> bool {
        PROGRAM_GAIN_DB.contains(&self.gain_db)
            && self.measured.is_none_or(|m| m.lufs.is_finite() && m.dbtp.is_finite() && m.reached.is_finite())
    }
}

/// What is heard, as bytes (loudness-norms D4): the effective audio events, the audio tracks' own
/// fields, the transitions between audio events and, by the same rule, every reel they play. A moved
/// title does not touch it, a reopened file keeps it, a louder event changes it.
pub fn sound_bytes(p: &Project) -> Vec<u8> {
    let mut out = Vec::new();
    put(&mut out, &(p.settings.fps, &p.settings.pitch_engine));
    reel_sound(p, None, &[], 0, &mut out);
    out
}

fn put(out: &mut Vec<u8>, v: &impl Serialize) {
    let _ = serde_json::to_writer(&mut *out, v);
    out.push(0);
}

fn reel_sound(p: &Project, seq: Option<MediaId>, overrides: &[Override], depth: usize, out: &mut Vec<u8>) {
    let resolved = p.resolve_reel(seq, overrides);
    let heard: Vec<&ClipEvent> = resolved.clips.iter().filter(|c| c.kind == ClipKind::Audio).collect();
    let reel = seq.and_then(|id| p.sequence(id));
    let tracks = reel.map_or(&p.tracks, |s| &s.tracks);
    put(out, &reel.map_or(p.overlap_layers, |s| s.overlap_layers));
    for t in tracks.iter().filter(|t| t.kind == LaneKind::Audio) {
        put(out, t);
    }
    let mut ids = std::collections::HashSet::new();
    for c in &heard {
        ids.insert(c.id);
        put(out, c);
        if let Some(m) = p.media.iter().find(|m| m.id == c.media) {
            // the path alone: a file's length is read anew when it is opened, and no event depends on it
            put(out, &m.path);
        }
        if depth < crate::MAX_NEST_DEPTH && p.sequence(c.media).is_some() {
            reel_sound(p, Some(c.media), &c.overrides, depth + 1, out);
        }
    }
    let mine = |id: Option<ClipId>| id.is_some_and(|id| ids.contains(&id));
    for t in resolved.transitions.iter().filter(|t| mine(t.a) || mine(t.b)) {
        put(out, t);
    }
}
