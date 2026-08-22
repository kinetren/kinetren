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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RequestKind {
    /// Аудио-burst источника.
    Audio { idx: i64 },
    /// Видеокадр по pts источника (в тиках), выровненный вызывающим.
    Video { pts: Tick, quality: Option<(u32, u32)> },
}

impl RequestKind {
    pub fn video(pts: Tick, quality: Option<Quality>) -> RequestKind {
        RequestKind::Video { pts, quality: quality.map(|q| (q.width, q.height)) }
    }

    /// Позиция для сортировки «вперёд по курсору».
    fn position(&self) -> i64 {
        match *self {
            RequestKind::Audio { idx } => idx * kt_time::AUDIO_BURST_TICKS,
            RequestKind::Video { pts, .. } => pts.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DecodeRequest {
    pub kind: RequestKind,
    pub prio: Priority,
    pub generation: u64,
}

#[derive(Default)]
struct Queue {
    // (prio, position, kind) → generation; BTreeMap даёт и порядок, и dedup
    entries: BTreeMap<(Priority, i64, RequestKind), u64>,
    closed: bool,
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
        mut source: Box<dyn MediaSource>,
        cache: Arc<BurstCache>,
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
            .spawn(move || worker_loop(&sh, source_id, source.as_mut(), &cache))
            .expect("spawn media worker");
        MediaWorker { shared, join: Some(join), media }
    }

    pub fn submit(&self, req: DecodeRequest) {
        let mut q = self.shared.queue.lock().unwrap();
        if req.generation < self.shared.min_generation.load(Ordering::Acquire) {
            return;
        }
        let key = (req.prio, req.kind.position(), req.kind);
        // dedup: тот же ключ обновляет generation, но не дублирует работу
        q.entries.insert(key, req.generation);
        self.shared.idle.store(false, Ordering::Release);
        drop(q);
        self.shared.cond.notify_one();
    }

    /// Отменить все запросы поколений < `min_generation` (seek транспорта).
    pub fn cancel_generations_below(&self, min_generation: u64) {
        self.shared.min_generation.store(min_generation, Ordering::Release);
        let mut q = self.shared.queue.lock().unwrap();
        q.entries.retain(|_, g| *g >= min_generation);
        drop(q);
        self.shared.cond.notify_one();
    }

    /// true, когда очередь пуста и воркер не занят декодом.
    pub fn is_idle(&self) -> bool {
        self.shared.idle.load(Ordering::Acquire)
            && self.shared.queue.lock().unwrap().entries.is_empty()
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
) {
    // Курсор «где был декодер»: позиция последнего обслуженного запроса.
    let mut cursor: i64 = 0;
    loop {
        let req = {
            let mut q = shared.queue.lock().unwrap();
            loop {
                if q.closed {
                    return;
                }
                if let Some(key) = pick_next(&q.entries, cursor) {
                    q.entries.remove(&key);
                    break key;
                }
                shared.idle.store(true, Ordering::Release);
                q = shared.cond.wait(q).unwrap();
            }
        };
        shared.idle.store(false, Ordering::Release);
        let (_prio, pos, kind) = req;
        cursor = pos;
        match kind {
            RequestKind::Audio { idx } => {
                if cache.get_audio(source_id, idx).is_none()
                    && let Ok(burst) = source.read_audio_burst(idx)
                {
                    cache.insert_audio(source_id, Arc::new(burst));
                }
            }
            RequestKind::Video { pts, quality } => {
                let q = quality.map(|(width, height)| Quality { width, height });
                let frame_idx = pts.0; // ключ кадра = pts в тиках
                if cache.get_video(source_id, q, frame_idx).is_none()
                    && let Ok(frame) = source.read_video_frame(pts, q)
                {
                    cache.insert_video(source_id, q, frame_idx, Arc::new(frame));
                }
            }
        }
    }
}

/// Выбор следующего запроса: минимальный приоритет; внутри него — ближайший
/// вперёд от курсора; если вперёд пусто — ближайший назад.
fn pick_next(
    entries: &BTreeMap<(Priority, i64, RequestKind), u64>,
    cursor: i64,
) -> Option<(Priority, i64, RequestKind)> {
    let (&(top_prio, _, _), _) = entries.iter().next()?;
    // вперёд от курсора в этом приоритете
    if let Some((&key, _)) =
        entries.range((top_prio, cursor, RequestKind::Audio { idx: i64::MIN })..).next()
        && key.0 == top_prio
    {
        return Some(key);
    }
    // иначе — последний (ближайший к курсору) из «задних»
    entries
        .range(..(top_prio, cursor, RequestKind::Audio { idx: i64::MIN }))
        .next_back()
        .map(|(&k, _)| k)
        .filter(|k| k.0 == top_prio)
        .or_else(|| entries.iter().next().map(|(&k, _)| k))
}
