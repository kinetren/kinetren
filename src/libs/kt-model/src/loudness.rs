//! The project's loudness target and its program stage (loudness-norms, the owner's B6: "на уровне
//! проекта нормализация а не в пресетах рендера"): the document's, never a reel's nor a render
//! preset's. The stage is absent until the person asks for it.

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
    /// What was heard then (`kt_engine::program::sound`): another value — changed since measured.
    pub sound: u64,
}

impl ProgramStage {
    pub fn valid(&self) -> bool {
        PROGRAM_GAIN_DB.contains(&self.gain_db)
            && self.measured.is_none_or(|m| m.lufs.is_finite() && m.dbtp.is_finite() && m.reached.is_finite())
    }
}
