//! Модель времени: целочисленные тики 1/90000 с.
//!
//! 90 kHz делится нацело на все поддерживаемые fps (24/25/30/60) и на шаг
//! аудио-burst (1024 кадра @ 48 kHz = 1920 тиков). Все времена в движке и в
//! формате проекта выражаются в тиках; миллисекунды — только на границе UI.

use serde::{Deserialize, Serialize};

/// Частота тиков, Гц.
pub const TICK_HZ: i64 = 90_000;

/// Частота дискретизации аудио внутри движка. Всё аудио после декода — 48 kHz.
pub const AUDIO_SAMPLE_RATE: u32 = 48_000;

/// Каналов аудио внутри движка (interleaved stereo).
pub const AUDIO_CHANNELS: usize = 2;

/// Кадров (пар сэмплов) в одном аудио-burst.
pub const AUDIO_BURST_FRAMES: usize = 1024;

/// Шаг аудио-burst в тиках: 90000 * 1024 / 48000 = 1920 (точно).
pub const AUDIO_BURST_TICKS: i64 = TICK_HZ * AUDIO_BURST_FRAMES as i64 / AUDIO_SAMPLE_RATE as i64;

/// Момент времени / длительность в тиках 1/90000 с.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Tick(pub i64);

const fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

impl Tick {
    pub const ZERO: Tick = Tick(0);

    pub fn from_ms(ms: i64) -> Tick {
        Tick(ms * TICK_HZ / 1000)
    }

    /// В миллисекунды с округлением к нулю (для отображения в UI).
    pub fn as_ms(self) -> i64 {
        self.0 * 1000 / TICK_HZ
    }

    pub fn from_secs_f64(s: f64) -> Tick {
        Tick((s * TICK_HZ as f64).round() as i64)
    }

    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / TICK_HZ as f64
    }

    /// Тики → номер аудио-сэмпла (кадра). Точно: 48000/90000 = 8/15.
    /// Осторожно: точен только на тиках, кратных 15/8 (границы burst'ов кратны).
    pub fn to_audio_frames(self) -> i64 {
        self.0 * AUDIO_SAMPLE_RATE as i64 / TICK_HZ
    }

    /// Номер аудио-сэмпла (кадра) → тики. Точно: 90000/48000 = 15/8 требует кратности 8,
    /// поэтому считаем через умножение до деления.
    /// Выровнять тик ВВЕРХ к аудио-сетке (кратно 15 тикам): round-trip
    /// «тик → аудиокадр → тик» усекает вниз и точен только на кратных 15.
    /// Из-за этого навигация «строго правее» застревала на одной точке —
    /// курсор после seek оказывался на тик ЛЕВЕЕ цели (п.7 р.10).
    pub fn align_audio_up(self) -> Tick {
        const STEP: i64 = TICK_HZ / gcd(TICK_HZ, AUDIO_SAMPLE_RATE as i64); // 15
        Tick(self.0.div_euclid(STEP) * STEP + if self.0.rem_euclid(STEP) > 0 { STEP } else { 0 })
    }

    pub fn from_audio_frames(frames: i64) -> Tick {
        Tick(frames * TICK_HZ / AUDIO_SAMPLE_RATE as i64)
    }

    pub fn saturating_sub(self, rhs: Tick) -> Tick {
        Tick(self.0.saturating_sub(rhs.0))
    }
}

impl std::ops::Add for Tick {
    type Output = Tick;
    fn add(self, rhs: Tick) -> Tick {
        Tick(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Tick {
    type Output = Tick;
    fn sub(self, rhs: Tick) -> Tick {
        Tick(self.0 - rhs.0)
    }
}

impl std::ops::Mul<i64> for Tick {
    type Output = Tick;
    fn mul(self, rhs: i64) -> Tick {
        Tick(self.0 * rhs)
    }
}

/// Полуоткрытый диапазон тиков `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TickRange {
    pub start: Tick,
    pub end: Tick,
}

impl TickRange {
    pub fn new(start: Tick, end: Tick) -> TickRange {
        TickRange { start, end }
    }

    pub fn len(&self) -> Tick {
        Tick((self.end.0 - self.start.0).max(0))
    }

    pub fn is_empty(&self) -> bool {
        self.end.0 <= self.start.0
    }

    pub fn contains(&self, t: Tick) -> bool {
        self.start <= t && t < self.end
    }

    pub fn intersect(&self, other: &TickRange) -> TickRange {
        TickRange {
            start: Tick(self.start.0.max(other.start.0)),
            end: Tick(self.end.0.min(other.end.0)),
        }
    }

    pub fn overlaps(&self, other: &TickRange) -> bool {
        !self.intersect(other).is_empty()
    }
}

/// Сетка видеокадров проекта.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FpsGrid {
    pub fps: u32,
}

impl FpsGrid {
    pub fn new(fps: u32) -> FpsGrid {
        assert!(fps > 0 && TICK_HZ % fps as i64 == 0, "fps must divide 90000");
        FpsGrid { fps }
    }

    /// Шаг кадра в тиках.
    pub fn frame_step(&self) -> i64 {
        TICK_HZ / self.fps as i64
    }

    /// Номер кадра, содержащего тик `t` (floor).
    pub fn frame_index(&self, t: Tick) -> i64 {
        t.0.div_euclid(self.frame_step())
    }

    /// Тик начала кадра с номером `idx`.
    pub fn frame_start(&self, idx: i64) -> Tick {
        Tick(idx * self.frame_step())
    }

    /// Выравнивание тика вниз на границу кадра.
    pub fn align_floor(&self, t: Tick) -> Tick {
        self.frame_start(self.frame_index(t))
    }
}

/// Номер аудио-burst'а, содержащего тик `t` (floor; burst покрывает [n*1920, (n+1)*1920)).
pub fn audio_burst_index(t: Tick) -> i64 {
    t.0.div_euclid(AUDIO_BURST_TICKS)
}

/// Тик начала аудио-burst'а с номером `idx`.
pub fn audio_burst_start(idx: i64) -> Tick {
    Tick(idx * AUDIO_BURST_TICKS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_burst_step_is_exact() {
        assert_eq!(AUDIO_BURST_TICKS, 1920);
        // Граница burst'а всегда попадает на целый сэмпл и обратно.
        for idx in [0i64, 1, 7, 1000, 123_456] {
            let t = audio_burst_start(idx);
            let frames = t.to_audio_frames();
            assert_eq!(frames, idx * AUDIO_BURST_FRAMES as i64);
            assert_eq!(Tick::from_audio_frames(frames), t);
        }
    }

    #[test]
    fn fps_grids_divide_tick_hz() {
        for fps in [24u32, 25, 30, 60] {
            let g = FpsGrid::new(fps);
            assert_eq!(g.frame_step() * fps as i64, TICK_HZ);
            // floor-выравнивание внутри кадра стабильно
            let t = Tick(g.frame_step() * 5 + g.frame_step() / 2);
            assert_eq!(g.frame_index(t), 5);
            assert_eq!(g.align_floor(t), Tick(g.frame_step() * 5));
        }
    }

    #[test]
    fn range_semantics_half_open() {
        let r = TickRange::new(Tick(100), Tick(200));
        assert!(r.contains(Tick(100)));
        assert!(!r.contains(Tick(200)));
        assert!(r.overlaps(&TickRange::new(Tick(199), Tick(300))));
        assert!(!r.overlaps(&TickRange::new(Tick(200), Tick(300))));
        assert!(TickRange::new(Tick(5), Tick(5)).is_empty());
    }

    #[test]
    fn negative_ticks_floor_correctly() {
        assert_eq!(audio_burst_index(Tick(-1)), -1);
        assert_eq!(audio_burst_index(Tick(0)), 0);
        let g = FpsGrid::new(30);
        assert_eq!(g.frame_index(Tick(-1)), -1);
    }
}
