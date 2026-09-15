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
    /// Перескоки через край loop-региона (р.15 п.2): нужны видео-части,
    /// которой иначе неоткуда узнать о переходе на новый круг.
    wraps: AtomicU64,
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

    /// Сколько раз позиция ПЕРЕПРЫГИВАЛА через край loop-региона.
    /// Generation при wrap'е намеренно не бампается (это не seek, звук и
    /// FX-хвосты продолжаются), поэтому видео-часть узнаёт о перескоке
    /// только отсюда. Без этого счётчика анти-откатный ратчет ключа
    /// показа замораживал картинку на втором круге loop'а, пока звук
    /// продолжал играть (р.15 п.2).
    pub fn wraps(&self) -> u64 {
        self.wraps.load(Ordering::Acquire)
    }

    /// Продвинуть на потреблённые аудио-кадры. Только аудио-render-поток.
    pub fn advance_frames(&self, frames: i64) {
        self.playhead_frames.fetch_add(frames, Ordering::AcqRel);
    }

    /// Продвижение с wrap'ом по loop-региону [start, end) в кадрах.
    /// Generation НЕ бампается: wrap — не seek, FX-состояние и кольцо живут
    /// (иначе слышимый разрыв). `frames` может быть отрицательным
    /// (reverse-транспорт, J4 v1.4): wrap работает в обе стороны —
    /// rem_euclid корректен для отрицательных смещений (тест 43); вне
    /// региона позиция клампится в 0 (реверс не уводит playhead в минус).
    pub fn advance_frames_wrapped(&self, frames: i64, region: Option<(i64, i64)>) {
        match region {
            Some((s, e)) if e > s => {
                let prev =
                    self.playhead_frames.fetch_update(Ordering::AcqRel, Ordering::Acquire, |p| {
                        let np = p + frames;
                        Some(if p >= s && p <= e && (np >= e || np < s) {
                            s + (np - s).rem_euclid(e - s)
                        } else {
                            np.max(0)
                        })
                    });
                // счётчик бампаем ПОСЛЕ обмена и по возвращённому старому
                // значению: замыкание fetch_update может выполниться
                // несколько раз при гонке, внутри считать нельзя
                if let Ok(p) = prev {
                    let np = p + frames;
                    if p >= s && p <= e && (np >= e || np < s) {
                        self.wraps.fetch_add(1, Ordering::AcqRel);
                    }
                }
            }
            _ => {
                let _ =
                    self.playhead_frames.fetch_update(Ordering::AcqRel, Ordering::Acquire, |p| {
                        Some((p + frames).max(0))
                    });
            }
        }
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

#[cfg(test)]
mod reverse_wrap_tests {
    use super::*;

    /// Тест 43 (J4): loop-wrap работает в обе стороны; вне региона
    /// отрицательное продвижение клампится в 0.
    #[test]
    fn t43_wrap_backwards_and_clamp() {
        let t = Transport::new();
        // вперёд через границу (регресс не сломан)
        t.seek_frames(9_800);
        t.advance_frames_wrapped(400, Some((5_000, 10_000)));
        assert_eq!(t.playhead_frames(), 5_200);
        // назад через начало региона → к концу
        t.seek_frames(5_100);
        t.advance_frames_wrapped(-400, Some((5_000, 10_000)));
        assert_eq!(t.playhead_frames(), 9_700);
        // ровно в start: без движения не wrap'ится
        t.seek_frames(5_000);
        t.advance_frames_wrapped(0, Some((5_000, 10_000)));
        assert_eq!(t.playhead_frames(), 5_000);
        // назад ровно на span → та же позиция
        t.seek_frames(7_000);
        t.advance_frames_wrapped(-5_000, Some((5_000, 10_000)));
        assert_eq!(t.playhead_frames(), 7_000);
        // вне региона (левее): обычное движение, без wrap
        t.seek_frames(1_000);
        t.advance_frames_wrapped(-300, Some((5_000, 10_000)));
        assert_eq!(t.playhead_frames(), 700);
        // без региона: кламп в 0
        t.seek_frames(100);
        t.advance_frames_wrapped(-500, None);
        assert_eq!(t.playhead_frames(), 0);
    }
}
