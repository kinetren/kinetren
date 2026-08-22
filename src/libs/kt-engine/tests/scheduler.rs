//! Тест 9 (docs/porting-notes.md): MediaWorker-планировщик.
//! - playback вытесняет prefetch;
//! - generation-отмена дропает устаревшие запросы;
//! - ни один ключ не декодируется дважды;
//! - prefetch идемпотентен (второй проход по тёплому кэшу не сабмитит ничего).

use kt_cache::{BurstCache, BurstKind, CacheConfig};
use kt_engine::{
    AudioRenderer, DecodeRequest, MediaWorker, Priority, RequestKind, prefetch_audio_pass,
    source_id,
};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_model::*;
use kt_time::Tick;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Медленный источник: каждый read спит и ведёт журнал обслуженных idx.
struct SlowSource {
    inner: SyntheticSource,
    delay: Duration,
    served: Arc<ServeLog>,
}

#[derive(Default)]
struct ServeLog {
    order: std::sync::Mutex<Vec<i64>>,
    total: AtomicU64,
}

impl MediaSource for SlowSource {
    fn info(&self) -> &MediaInfo {
        self.inner.info()
    }
    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        std::thread::sleep(self.delay);
        self.served.order.lock().unwrap().push(idx);
        self.served.total.fetch_add(1, Ordering::SeqCst);
        self.inner.read_audio_burst(idx)
    }
    fn read_video_frame(&mut self, p: Tick, q: Option<Quality>) -> Result<VideoFrame, MediaError> {
        self.inner.read_video_frame(p, q)
    }
    fn keyframe_before(&self, p: Tick) -> Option<Tick> {
        self.inner.keyframe_before(p)
    }
}

fn slow_worker(media: u64, delay_ms: u64, cache: &Arc<BurstCache>) -> (MediaWorker, Arc<ServeLog>) {
    let served = Arc::new(ServeLog::default());
    let source = SlowSource {
        inner: SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Ramp),
        delay: Duration::from_millis(delay_ms),
        served: served.clone(),
    };
    (MediaWorker::spawn(MediaId(media), Box::new(source), cache.clone()), served)
}

fn wait_idle(w: &MediaWorker) {
    for _ in 0..2000 {
        if w.is_idle() {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("worker did not drain in time");
}

#[test]
fn t9_playback_preempts_prefetch() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let (worker, served) = slow_worker(1, 5, &cache);

    // Забиваем очередь дальним префетчем...
    for idx in 100..120 {
        worker.submit(DecodeRequest {
            kind: RequestKind::Audio { idx },
            prio: Priority::FarPrefetch,
            generation: 0,
        });
    }
    // ...и когда воркер уже занят, прилетает playback.
    std::thread::sleep(Duration::from_millis(2));
    worker.submit(DecodeRequest {
        kind: RequestKind::Audio { idx: 7 },
        prio: Priority::Playback,
        generation: 0,
    });
    wait_idle(&worker);
    let order = served.order.lock().unwrap().clone();
    let pos_playback = order.iter().position(|&i| i == 7).expect("playback served");
    assert!(
        pos_playback <= 2,
        "playback обязан вытеснить очередь prefetch, а обслужен {pos_playback}-м: {order:?}"
    );
    worker.shutdown();
}

#[test]
fn t9_generation_cancellation_drops_stale_work() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let (worker, served) = slow_worker(1, 5, &cache);

    for idx in 0..30 {
        worker.submit(DecodeRequest {
            kind: RequestKind::Audio { idx },
            prio: Priority::NearPrefetch,
            generation: 0,
        });
    }
    std::thread::sleep(Duration::from_millis(12)); // пара запросов успевает
    worker.cancel_generations_below(1); // seek: всё поколение 0 устарело
    for idx in 500..503 {
        worker.submit(DecodeRequest {
            kind: RequestKind::Audio { idx },
            prio: Priority::NearPrefetch,
            generation: 1,
        });
    }
    wait_idle(&worker);
    let order = served.order.lock().unwrap().clone();
    let old_served = order.iter().filter(|&&i| i < 30).count();
    assert!(
        old_served <= 4,
        "после отмены поколения хвост старых запросов не должен обслуживаться: {order:?}"
    );
    assert!(order.ends_with(&[500, 501, 502]), "новое поколение обслужено: {order:?}");
    worker.shutdown();
}

#[test]
fn t9_no_key_is_served_twice() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let (worker, served) = slow_worker(1, 1, &cache);

    // Дубли в очереди + повторный сабмит после прогрева.
    for _round in 0..3 {
        for idx in 0..10 {
            worker.submit(DecodeRequest {
                kind: RequestKind::Audio { idx },
                prio: Priority::NearPrefetch,
                generation: 0,
            });
        }
    }
    wait_idle(&worker);
    for idx in 0..10 {
        worker.submit(DecodeRequest {
            kind: RequestKind::Audio { idx },
            prio: Priority::Playback,
            generation: 0,
        });
    }
    wait_idle(&worker);
    assert_eq!(
        served.total.load(Ordering::SeqCst),
        10,
        "каждый ключ декодируется ровно один раз (dedup очереди + кэш-проверка)"
    );
    worker.shutdown();
}

#[test]
fn t9_equal_priority_prefers_forward_of_cursor() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let (worker, served) = slow_worker(1, 3, &cache);

    // Первый запрос ставит курсор на 50; дальше подкидываем позади и впереди.
    worker.submit(DecodeRequest {
        kind: RequestKind::Audio { idx: 50 },
        prio: Priority::NearPrefetch,
        generation: 0,
    });
    std::thread::sleep(Duration::from_millis(1));
    for idx in [10, 51, 11, 52, 12, 53] {
        worker.submit(DecodeRequest {
            kind: RequestKind::Audio { idx },
            prio: Priority::NearPrefetch,
            generation: 0,
        });
    }
    wait_idle(&worker);
    let order = served.order.lock().unwrap().clone();
    let fwd: Vec<i64> = order.iter().copied().filter(|&i| i > 50).collect();
    let back: Vec<i64> = order.iter().copied().filter(|&i| i <= 12).collect();
    let last_fwd = order.iter().rposition(|&i| i > 50).unwrap();
    let first_back = order.iter().position(|&i| i <= 12).unwrap();
    assert!(
        last_fwd < first_back,
        "вперёд по курсору раньше задних: {order:?} (fwd {fwd:?}, back {back:?})"
    );
    worker.shutdown();
}

#[test]
fn t9_prefetch_pass_is_idempotent() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let (worker, _served) = slow_worker(1, 0, &cache);

    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000 * 4),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        fx: vec![],
    });
    let renderer = AudioRenderer::new(&p);
    let mut workers = HashMap::new();
    let range = 0i64..(48_000 * 4);

    let n1 = {
        workers.insert(MediaId(1), worker);
        prefetch_audio_pass(&renderer, &cache, &workers, range.clone(), Priority::FarPrefetch, 0)
    };
    assert!(n1 > 0);
    wait_idle(&workers[&MediaId(1)]);

    let n2 =
        prefetch_audio_pass(&renderer, &cache, &workers, range.clone(), Priority::FarPrefetch, 0);
    assert_eq!(n2, 0, "по тёплому кэшу prefetch не должен сабмитить ничего");

    // coverage совпадает с wanted
    let cov = cache.coverage(source_id(MediaId(1)), BurstKind::Audio, 0..1000);
    let (_, want) = &renderer.wanted_audio_bursts(range)[0];
    assert_eq!(cov, vec![want.clone()]);

    workers.remove(&MediaId(1)).unwrap().shutdown();
}
