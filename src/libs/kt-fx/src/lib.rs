//! Аудио- и видео-эффекты. Реализация — этап M6 (rubato-warp + freeverb и др.),
//! трейты фиксируются заранее: на них опирается render graph (M2).
//!
//! Ключевые решения (см. docs/architecture.md и корневой CLAUDE.md):
//! - process() видит ТОЛЬКО f32 48kHz/2ch (инвариант №1);
//! - set_params() сглаживает (one-pole ~20 мс) и НИКОГДА не сбрасывает буферы —
//!   сброс только в reset() при seek (дефект e старого кода);
//! - tail() сообщает длину хвоста — граф продлевает аудио-окно клипа (дефект f);
//! - состоянием владеет исключительно аудио-render-поток через FxStateStore
//!   в kt-engine (дефект b) — поэтому трейт не требует Sync.

use kt_time::Tick;

/// Контекст обработки одного burst'а.
#[derive(Debug, Clone, Copy)]
pub struct FxCtx {
    /// Нормализованный прогресс burst'а внутри клипа [0..1] — для автоматизации.
    pub clip_progress: f64,
}

pub trait AudioFx: Send {
    /// Применить (сглаженно) новые параметры. Не сбрасывает состояние.
    fn set_params(&mut self, params: &kt_model::FxInstance);
    /// Обработать burst in-place. Вход и выход — 48k/2ch f32 interleaved.
    fn process(&mut self, samples: &mut [f32], ctx: &FxCtx);
    /// Длина хвоста (реверб и т.п.); граф продлевает окно клипа на эту величину.
    fn tail(&self) -> Tick;
    /// Полный сброс состояния. Вызывается ТОЛЬКО при seek/разрыве непрерывности.
    fn reset(&mut self);
}
