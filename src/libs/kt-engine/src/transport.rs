//! Transport: авторитетный playhead и состояние воспроизведения.
//!
//! Инвариант №6: мастер-часы — потребление аудио. `advance_frames` вызывает
//! ТОЛЬКО аудио-render-поток, ровно на число кадров, потреблённых
//! аудио-колбэком (M4). Wall-clock здесь запрещён (детерминизм, инвариант №10).
//! В M2 «колбэком» выступает офлайн-драйвер/тест — семантика та же.

use kt_time::Tick;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};

#[derive(Default)]
pub struct Transport {
    /// Позиция в аудио-кадрах 48 kHz (не в тиках: продвижение — целые кадры).
    playhead_frames: AtomicI64,
    playing: AtomicBool,
    /// Растёт при каждом seek: MediaWorker'ы дропают запросы устаревших
    /// поколений (инвариант №5), FX делают reset (инвариант №3).
    generation: AtomicU64,
}

impl Transport {
    pub fn new() -> Transport {
        Transport::default()
    }

    pub fn playhead_frames(&self) -> i64 {
        self.playhead_frames.load(Ordering::Acquire)
    }

    pub fn playhead(&self) -> Tick {
        Tick::from_audio_frames(self.playhead_frames())
    }

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Acquire)
    }

    pub fn set_playing(&self, on: bool) {
        self.playing.store(on, Ordering::Release);
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Продвинуть на потреблённые аудио-кадры. Только аудио-render-поток.
    pub fn advance_frames(&self, frames: i64) {
        self.playhead_frames.fetch_add(frames, Ordering::AcqRel);
    }

    /// Прыжок. Бампает generation.
    pub fn seek_frames(&self, frame: i64) {
        self.playhead_frames.store(frame, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    pub fn seek(&self, t: Tick) {
        self.seek_frames(t.to_audio_frames());
    }
}
