//! Тест 21 (docs/porting-notes.md, пп.14/15): видео-путь без decode-штормов —
//! последовательный прогон декодирует каждый кадр один раз и никогда назад;
//! повторный прогон по тёплому кэшу — ноль декодов; смешанная аудио+видео
//! очередь не гоняет декодер зигзагом (раздельные курсоры).

use kt_cache::{BurstCache, CacheConfig};
use kt_engine::{DecodeRequest, MediaWorker, Priority, RequestKind, source_id};
use kt_media::Quality;
use kt_media::synthetic::{AudioPattern, SyntheticSource, SyntheticStats};
use kt_model::MediaId;
use kt_time::{TICK_HZ, Tick};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

const FRAME_STEP: i64 = TICK_HZ / 30; // 30 fps
const Q: Quality = Quality { width: 64, height: 36 };

fn wait_idle(w: &MediaWorker) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !w.is_idle() {
        assert!(std::time::Instant::now() < deadline, "воркер не докрутил очередь");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn setup() -> (Arc<BurstCache>, Arc<SyntheticStats>, MediaWorker) {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let stats = Arc::new(SyntheticStats::default());
    let src = Box::new(SyntheticSource::with_stats(
        Tick(90_000 * 60),
        AudioPattern::Sine { hz: 220.0 },
        stats.clone(),
    ));
    let worker = MediaWorker::spawn(MediaId(1), src, cache.clone());
    (cache, stats, worker)
}

#[test]
fn t21_sequential_grid_decodes_each_frame_once_and_never_backward() {
    let (cache, stats, worker) = setup();
    let n: i64 = 90; // 3 секунды сетки 30 fps
    for i in 0..n {
        worker.submit(DecodeRequest {
            kind: RequestKind::video(Tick(i * FRAME_STEP), Some(Q)),
            prio: Priority::Playback,
            generation: 0,
        });
    }
    wait_idle(&worker);

    assert_eq!(
        stats.video_backward_reads.load(Relaxed),
        0,
        "последовательная сетка не должна дёргать декодер назад"
    );
    let reads = stats.video_reads.load(Relaxed);
    assert!(reads <= n as u64, "каждый кадр декодируется не более одного раза: {reads} > {n}");
    for i in 0..n {
        assert!(
            cache.get_video(source_id(MediaId(1)), Some(Q), i * FRAME_STEP).is_some(),
            "кадр {i} не попал в кэш"
        );
    }

    // повторный прогон по тёплому кэшу — ноль новых декодов
    for i in 0..n {
        worker.submit(DecodeRequest {
            kind: RequestKind::video(Tick(i * FRAME_STEP), Some(Q)),
            prio: Priority::Playback,
            generation: 0,
        });
    }
    wait_idle(&worker);
    assert_eq!(stats.video_reads.load(Relaxed), reads, "тёплый кэш не должен декодировать");
    worker.shutdown();
}

#[test]
fn t21_mixed_audio_video_same_priority_no_zigzag() {
    let (_cache, stats, worker) = setup();
    // перемешанный сабмит: аудио и видео возрастают, но чередуются
    for i in 0..60i64 {
        worker.submit(DecodeRequest {
            kind: RequestKind::video(Tick(i * FRAME_STEP), Some(Q)),
            prio: Priority::Playback,
            generation: 0,
        });
        worker.submit(DecodeRequest {
            kind: RequestKind::Audio { idx: i },
            prio: Priority::Playback,
            generation: 0,
        });
    }
    wait_idle(&worker);
    assert_eq!(
        stats.video_backward_reads.load(Relaxed),
        0,
        "видео-курсор не должен зигзагом следовать за аудио"
    );
    assert_eq!(
        stats.audio_backward_reads.load(Relaxed),
        0,
        "аудио-курсор не должен зигзагом следовать за видео"
    );
    worker.shutdown();
}
