//! MediaWorker: поток, ЭКСКЛЮЗИВНО владеющий одним MediaSource (инвариант №5).
//!
//! Клиенты (playback / prefetch / тумбнейлы) не трогают декодер — они кладут
//! запросы в приоритетную очередь; результат воркер вставляет в BurstCache.
//! Это устраняет главный источник тормозов старого движка: prefetch и playback
//! дрались за один декодер через mutex, гоняя его seek'ами (дефект h).
//!
//! Правила очереди:
//! - меньший Priority строго первее (Playback > NearPrefetch > ...);
//! - при равном приоритете — запросы ВПЕРЁД по курсору декодера, ближайший
//!   первым (анти ping-pong: не скакать назад, пока есть работа вперёд);
//!   назад — только когда вперёд пусто;
//! - каждый ключ обслуживается не более одного раза за постановку (dedup);
//! - уже лежащее в кэше не декодируется повторно;
//! - seek транспорта бампает generation: запросы старых поколений дропаются.

use crate::Priority;
use kt_cache::{BurstCache, SourceId};
use kt_media::{MediaSource, Quality};
use kt_model::MediaId;
use kt_time::Tick;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

/// Счётчики реальных обращений к декодерам (для перф-панели и тестов 21).
#[derive(Default)]
pub struct DecodeStats {
    pub audio_decodes: AtomicU64,
    pub video_decodes: AtomicU64,
    /// Спанов видео-декода (батчей) и кадров, декодированных спанами.
    pub video_span_batches: AtomicU64,
    pub video_span_frames: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RequestKind {
    /// Аудио-burst источника.
    Audio { idx: i64 },
    /// Видеокадр по pts источника (в тиках), выровненный вызывающим.
    /// `step` — шаг сетки кадров (тики): ширина покрытия в coverage (п.20).
    Video { pts: Tick, quality: Option<(u32, u32)>, step: i64 },
}

impl RequestKind {
    pub fn video(pts: Tick, quality: Option<Quality>) -> RequestKind {
        RequestKind::Video { pts, quality: quality.map(|q| (q.width, q.height)), step: 1 }
    }

    /// Видео-запрос по сетке проекта: coverage закроется полосой [pts,
    /// pts+step) — полоска кэша честная (п.20).
    pub fn video_on_grid(pts: Tick, quality: Option<Quality>, frame_step: i64) -> RequestKind {
        RequestKind::Video {
            pts,
            quality: quality.map(|q| (q.width, q.height)),
            step: frame_step.max(1),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DecodeRequest {
    pub kind: RequestKind,
    pub prio: Priority,
    pub generation: u64,
}

/// (prio, pts-тики, quality) — ключ видео-очереди.
type VideoQueueKey = (Priority, i64, Option<(u32, u32)>);

#[derive(Default)]
struct Queue {
    // Раздельные карты по типу: у аудио и видео СВОИ курсоры декодера —
    // общий курсор заставлял видео-выбор зигзагом следовать за аудио
    // (п.14/15: постоянные backward seek → полный reopen на каждый кадр).
    // BTreeMap даёт и порядок, и dedup.
    audio: BTreeMap<(Priority, i64), u64>, // pos = burst idx
    video: BTreeMap<VideoQueueKey, (u64, i64)>, // pos = pts тики; (gen, step)
    closed: bool,
}

impl Queue {
    fn is_empty(&self) -> bool {
        self.audio.is_empty() && self.video.is_empty()
    }
}

struct Shared {
    queue: Mutex<Queue>,
    cond: Condvar,
    min_generation: AtomicU64,
    idle: AtomicBool,
}

/// Хэндл воркера. Drop НЕ останавливает поток — вызвать `shutdown()`.
pub struct MediaWorker {
    shared: Arc<Shared>,
    join: Option<std::thread::JoinHandle<()>>,
    pub media: MediaId,
}

impl MediaWorker {
    pub fn spawn(
        media: MediaId,
        source: Box<dyn MediaSource>,
        cache: Arc<BurstCache>,
    ) -> MediaWorker {
        Self::spawn_with_stats(media, source, cache, Arc::new(DecodeStats::default()))
    }

    pub fn spawn_with_stats(
        media: MediaId,
        mut source: Box<dyn MediaSource>,
        cache: Arc<BurstCache>,
        stats: Arc<DecodeStats>,
    ) -> MediaWorker {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            cond: Condvar::new(),
            min_generation: AtomicU64::new(0),
            idle: AtomicBool::new(true),
        });
        let sh = shared.clone();
        let source_id = crate::source_id(media);
        let join = std::thread::Builder::new()
            .name(format!("media-worker-{}", media.0))
            .spawn(move || worker_loop(&sh, source_id, source.as_mut(), &cache, &stats))
            .expect("spawn media worker");
        MediaWorker { shared, join: Some(join), media }
    }

    pub fn submit(&self, req: DecodeRequest) {
        let mut q = self.shared.queue.lock().unwrap();
        if req.generation < self.shared.min_generation.load(Ordering::Acquire) {
            return;
        }
        // dedup: тот же ключ обновляет generation, но не дублирует работу
        match req.kind {
            RequestKind::Audio { idx } => {
                q.audio.insert((req.prio, idx), req.generation);
            }
            RequestKind::Video { pts, quality, step } => {
                q.video.insert((req.prio, pts.0, quality), (req.generation, step));
            }
        }
        self.shared.idle.store(false, Ordering::Release);
        drop(q);
        self.shared.cond.notify_one();
    }

    /// Отменить все запросы поколений < `min_generation` (seek транспорта).
    pub fn cancel_generations_below(&self, min_generation: u64) {
        self.shared.min_generation.store(min_generation, Ordering::Release);
        let mut q = self.shared.queue.lock().unwrap();
        q.audio.retain(|_, g| *g >= min_generation);
        q.video.retain(|_, (g, _)| *g >= min_generation);
        drop(q);
        self.shared.cond.notify_one();
    }

    /// true, когда очередь пуста и воркер не занят декодом.
    pub fn is_idle(&self) -> bool {
        self.shared.idle.load(Ordering::Acquire) && self.shared.queue.lock().unwrap().is_empty()
    }

    pub fn shutdown(mut self) {
        {
            let mut q = self.shared.queue.lock().unwrap();
            q.closed = true;
        }
        self.shared.cond.notify_all();
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn worker_loop(
    shared: &Shared,
    source_id: SourceId,
    source: &mut dyn MediaSource,
    cache: &BurstCache,
    stats: &DecodeStats,
) {
    /// Насколько далеко вперёд от первого ключа расширяем спан.
    const SPAN_MAX_AHEAD_TICKS: i64 = 2 * kt_time::TICK_HZ;

    // Курсоры «где был декодер» — СВОИ у аудио и видео (см. Queue).
    let mut cursor_audio: i64 = 0;
    let mut cursor_video: i64 = 0;
    // Negative-кэш (п.19г): ключи, по которым декодер ничего не отдал
    // (за EOF) — не декодируем повторно, иначе prefetch каждые 100 мс
    // гонял декодер в вечные seek'и.
    let mut failed_video: std::collections::HashSet<(i64, Option<(u32, u32)>)> =
        std::collections::HashSet::new();
    let mut failed_audio: std::collections::HashSet<i64> = std::collections::HashSet::new();
    loop {
        enum Job {
            Audio {
                idx: i64,
            },
            /// Первый ключ + добранный вперёд хвост (любых приоритетов).
            VideoSpan {
                keys: Vec<(Tick, i64)>, // (pts, step покрытия)
                quality: Option<(u32, u32)>,
            },
        }
        let job = {
            let mut q = shared.queue.lock().unwrap();
            loop {
                if q.closed {
                    return;
                }
                match pick_next(&q, cursor_audio, cursor_video) {
                    Some(Pick::Audio(key)) => {
                        q.audio.remove(&key);
                        break Job::Audio { idx: key.1 };
                    }
                    Some(Pick::Video(key)) => {
                        let (_prio, first_pts, quality) = key;
                        let (_g, first_step) = q.video.remove(&key).unwrap_or((0, 1));
                        // Добор спана ПОВЕРХ приоритетов (п.19а): Playback-ключ
                        // становится НАЧАЛОМ спана, а NearPrefetch-хвост едет
                        // тем же декод-проходом. Раньше добор брал только свой
                        // приоритет: пока жив хоть один Playback-ключ (кадр
                        // неполный), спаны вырождались в «один кадр за раз с
                        // seek'ами» — кэш переставал расти.
                        let mut keys = vec![(Tick(first_pts), first_step)];
                        let horizon = first_pts + SPAN_MAX_AHEAD_TICKS;
                        let budget = span_key_budget(quality, cache.max_video_bytes());
                        let mut cand: Vec<(VideoQueueKey, i64)> = Vec::new();
                        for p in [
                            crate::Priority::Playback,
                            crate::Priority::NearPrefetch,
                            crate::Priority::FarPrefetch,
                            crate::Priority::Thumbnail,
                        ] {
                            cand.extend(
                                q.video
                                    .range((p, first_pts + 1, None)..)
                                    .take_while(|(k, _)| k.0 == p && k.1 <= horizon)
                                    .filter(|(k, _)| k.2 == quality)
                                    .map(|(&k, &(_, step))| (k, step)),
                            );
                        }
                        cand.sort_by_key(|(k, _)| k.1);
                        cand.dedup_by_key(|(k, _)| k.1);
                        for (k, step) in cand {
                            if keys.len() < budget
                                && !cache.contains_video(source_id, quality.map(into_q), k.1)
                                && !failed_video.contains(&(k.1, quality))
                            {
                                keys.push((Tick(k.1), step));
                            }
                            q.video.remove(&k);
                        }
                        break Job::VideoSpan { keys, quality };
                    }
                    None => {
                        shared.idle.store(true, Ordering::Release);
                        q = shared.cond.wait(q).unwrap();
                    }
                }
            }
        };
        shared.idle.store(false, Ordering::Release);
        match job {
            Job::Audio { idx } => {
                cursor_audio = idx * kt_time::AUDIO_BURST_TICKS;
                if !cache.contains_audio(source_id, idx) && !failed_audio.contains(&idx) {
                    stats.audio_decodes.fetch_add(1, Ordering::Relaxed);
                    match source.read_audio_burst(idx) {
                        Ok(burst) => cache.insert_audio(source_id, Arc::new(burst)),
                        Err(kt_media::MediaError::OutOfRange) => {
                            failed_audio.insert(idx);
                        }
                        Err(_) => {}
                    }
                }
            }
            Job::VideoSpan { keys, quality } => {
                let q = quality.map(into_q);
                cursor_video = keys.last().map(|(k, _)| k.0).unwrap_or(cursor_video);
                let missing: Vec<(Tick, i64)> = keys
                    .into_iter()
                    .filter(|(k, _)| {
                        !cache.contains_video(source_id, q, k.0)
                            && !failed_video.contains(&(k.0, quality))
                    })
                    .collect();
                if missing.is_empty() {
                    continue;
                }
                stats.video_decodes.fetch_add(missing.len() as u64, Ordering::Relaxed);
                if missing.len() > 1 {
                    stats.video_span_batches.fetch_add(1, Ordering::Relaxed);
                    stats.video_span_frames.fetch_add(missing.len() as u64, Ordering::Relaxed);
                }
                let steps: std::collections::HashMap<i64, i64> =
                    missing.iter().map(|(k, s)| (k.0, *s)).collect();
                let pts_only: Vec<Tick> = missing.iter().map(|(k, _)| *k).collect();
                let mut emitted: std::collections::HashSet<i64> = std::collections::HashSet::new();
                let res = source.read_video_span(&pts_only, q, &mut |k, frame| {
                    emitted.insert(k.0);
                    let span = steps.get(&k.0).copied().unwrap_or(1).max(1);
                    cache.insert_video_span(source_id, q, k.0, span, Arc::new(frame));
                });
                if res.is_ok() {
                    // ключи, не закрытые декодером (EOF) — в negative-кэш
                    for k in pts_only {
                        if !emitted.contains(&k.0) {
                            failed_video.insert((k.0, quality));
                        }
                    }
                }
            }
        }
    }
}

fn into_q((width, height): (u32, u32)) -> Quality {
    Quality { width, height }
}

/// Спан не должен уметь выселить сам себя: не больше половины видео-бюджета
/// за один проход (п.19в). Для превью 640×360 это прежние 64 ключа.
fn span_key_budget(quality: Option<(u32, u32)>, max_video_bytes: usize) -> usize {
    const SPAN_MAX_KEYS: usize = 64;
    match quality {
        Some((w, h)) => {
            let frame_bytes = (w as usize * h as usize * 4).max(1);
            ((max_video_bytes / 2) / frame_bytes).clamp(1, SPAN_MAX_KEYS)
        }
        None => SPAN_MAX_KEYS,
    }
}

enum Pick {
    Audio((Priority, i64)),
    Video(VideoQueueKey),
}

/// Выбор следующего запроса: минимальный приоритет по ОБЕИМ картам; внутри
/// него у каждого вида — кандидат вперёд от СВОЕГО курсора (ближайший);
/// если вперёд есть у обоих — меньшая forward-дистанция (ничья → аудио);
/// вперёд пусто у обоих — ближайший назад аналогично.
fn pick_next(q: &Queue, cursor_audio: i64, cursor_video: i64) -> Option<Pick> {
    let top_a = q.audio.keys().next().map(|k| k.0);
    let top_v = q.video.keys().next().map(|k| k.0);
    let top = match (top_a, top_v) {
        (Some(a), Some(v)) => a.min(v),
        (Some(a), None) => a,
        (None, Some(v)) => v,
        (None, None) => return None,
    };
    // burst idx → тики для сравнения дистанций в одном домене
    let a_cursor_idx = cursor_audio / kt_time::AUDIO_BURST_TICKS;
    let a_fwd = q.audio.range((top, a_cursor_idx)..).next().map(|(&k, _)| k).filter(|k| k.0 == top);
    let v_fwd =
        q.video.range((top, cursor_video, None)..).next().map(|(&k, _)| k).filter(|k| k.0 == top);
    match (a_fwd, v_fwd) {
        (Some(a), Some(v)) => {
            let da = a.1 * kt_time::AUDIO_BURST_TICKS - cursor_audio;
            let dv = v.1 - cursor_video;
            if da <= dv { Some(Pick::Audio(a)) } else { Some(Pick::Video(v)) }
        }
        (Some(a), None) => Some(Pick::Audio(a)),
        (None, Some(v)) => Some(Pick::Video(v)),
        (None, None) => {
            // назад: ближайший к своему курсору
            let a_back = q
                .audio
                .range(..(top, a_cursor_idx))
                .next_back()
                .map(|(&k, _)| k)
                .filter(|k| k.0 == top);
            let v_back = q
                .video
                .range(..(top, cursor_video, None))
                .next_back()
                .map(|(&k, _)| k)
                .filter(|k| k.0 == top);
            match (a_back, v_back) {
                (Some(a), Some(v)) => {
                    let da = cursor_audio - a.1 * kt_time::AUDIO_BURST_TICKS;
                    let dv = cursor_video - v.1;
                    if da <= dv { Some(Pick::Audio(a)) } else { Some(Pick::Video(v)) }
                }
                (Some(a), None) => Some(Pick::Audio(a)),
                (None, Some(v)) => Some(Pick::Video(v)),
                (None, None) => {
                    // остались только записи другого приоритета/края — любой
                    q.audio
                        .keys()
                        .next()
                        .map(|&k| Pick::Audio(k))
                        .or_else(|| q.video.keys().next().map(|&k| Pick::Video(k)))
                }
            }
        }
    }
}
