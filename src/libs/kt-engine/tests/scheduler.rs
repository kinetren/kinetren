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

    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000 * 4),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    let renderer = AudioRenderer::new(&p);
    let mut workers = HashMap::new();
    let range = 0i64..(48_000 * 4);

    let n1 = {
        workers.insert(MediaId(1), worker);
        prefetch_audio_pass(
            &renderer,
            &cache,
            &workers,
            range.clone(),
            Priority::FarPrefetch,
            0,
            false,
        )
    };
    assert!(n1 > 0);
    wait_idle(&workers[&MediaId(1)]);

    let n2 = prefetch_audio_pass(
        &renderer,
        &cache,
        &workers,
        range.clone(),
        Priority::FarPrefetch,
        0,
        false,
    );
    assert_eq!(n2, 0, "по тёплому кэшу prefetch не должен сабмитить ничего");

    // coverage совпадает с wanted
    let cov = cache.coverage(source_id(MediaId(1)), BurstKind::Audio, 0..1000);
    let (_, want) = &renderer.wanted_audio_bursts(range)[0];
    assert_eq!(cov, vec![want.clone()]);

    workers.remove(&MediaId(1)).unwrap().shutdown();
}

/// Тест 287 (Р.43 п.297): сотни микрособытий не превращаются в сотни
/// одинаковых заказов префетча.
///
/// Пользователь строит стоп-кадр из микрособытий и подтвердил, что это
/// намеренный приём: в его проекте 885 аудио-событий длиной 90 тиков
/// (4.8 сэмпла) стоят встык на 0.88 с и читают ОДИН И ТОТ ЖЕ кадр
/// источника. Каждое клало в `wanted` свою пару, и на них делалось 885
/// запросов `coverage` под мьютексом кэша — при том, что бёрст там ровно
/// один.
#[test]
fn t287_stutter_of_micro_clips_asks_for_one_burst() {
    let mut p = Project::new(ProjectSettings {
        fps: 60,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    // 400 событий по 90 тиков встык, все с одной точки материала
    let step = kt_time::Tick(90);
    for i in 0..400u64 {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(i + 1),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: kt_time::Tick(step.0 * i as i64),
            duration: step,
            source_in: kt_time::Tick(4_500_000),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);
    let span = kt_time::Tick(step.0 * 400).to_audio_frames();
    let wanted = r.wanted_audio_bursts(0..span);
    assert_eq!(
        wanted.len(),
        1,
        "400 микрособытий на одном кадре источника — один заказ, а не 400: {wanted:?}"
    );
}

/// Тест 501 (Р.53, п.411): перенастройка источника — НЕ запрос.
///
/// (а) Она не подчиняется generation-отмене: seek посреди сессии окна
/// (драг пипетки цвета) не имеет права выбросить её из очереди — иначе
/// источник навсегда остался бы со старыми параметрами, а модель ушла бы
/// вперёд. Именно поэтому `Queue.retune` живёт отдельным полем.
///
/// (б) После перенастройки в кэше не остаётся НИ ОДНОГО кадра старых
/// параметров: чистит их сам воркер — тот же поток, который их вставляет.
#[test]
fn t501_retune_is_not_cancelled_and_not_raced() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let media = MediaId(1);
    let sid = source_id(media);
    let q = Quality { width: 32, height: 18 };
    let src = match kt_media::factory::open_pseudo("gen:solid") {
        Some(Ok(s)) => s,
        _ => unreachable!("gen: путь обязан открыться"),
    };
    let w = MediaWorker::spawn(media, src, cache.clone());

    // заказываем пачку кадров чёрного генератора и ждём прогрева
    let step = 3000i64;
    for i in 0..64 {
        w.submit(DecodeRequest {
            kind: RequestKind::video_on_grid(Tick(i * step), Some(q), step),
            prio: Priority::NearPrefetch,
            generation: 1,
        });
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !w.is_idle() {
        assert!(std::time::Instant::now() < deadline, "прогрев не закончился");
        std::thread::sleep(Duration::from_millis(10));
    }
    let warmed = (0..64).filter(|i| cache.get_video(sid, Some(q), i * step).is_some()).count();
    assert!(warmed > 8, "кэш не прогрелся: {warmed} кадров");

    // (а) перенастройка + НЕМЕДЛЕННЫЙ seek с огромным поколением
    w.retune("gen:solid:c=ff8800");
    w.cancel_generations_below(u64::MAX / 2);

    // ждём, пока перенастройка доедет: кадры новых параметров приходят
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        w.submit(DecodeRequest {
            kind: RequestKind::video_on_grid(Tick(0), Some(q), step),
            prio: Priority::Playback,
            generation: u64::MAX / 2,
        });
        if let Some(f) = cache.get_video(sid, Some(q), 0)
            && f.rgba[..3] == [255, 136, 0]
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "seek отменил перенастройку — источник остался со старыми параметрами"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    // (б) ни одного кадра СТАРЫХ параметров не осталось
    for i in 0..64 {
        if let Some(f) = cache.get_video(sid, Some(q), i * step) {
            assert_eq!(
                &f.rgba[..3],
                &[255, 136, 0],
                "кадр {i} остался со старыми параметрами: чистка разошлась с воркером"
            );
        }
    }
    w.shutdown();
}
