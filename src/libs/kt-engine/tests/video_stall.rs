//! Тесты 27-28 (docs/porting-notes.md, пп.19-20): видео-кэш не «встаёт»
//! под потоком Playback-запросов, EOF-ключи не декодируются повторно,
//! coverage со span'ом даёт честные полосы.

use kt_cache::{BurstCache, BurstKind, CacheConfig};
use kt_engine::{DecodeRequest, MediaWorker, Priority, RequestKind, source_id};
use kt_media::Quality;
use kt_media::synthetic::{AudioPattern, SyntheticSource, SyntheticStats};
use kt_model::MediaId;
use kt_time::{TICK_HZ, Tick};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

const STEP: i64 = TICK_HZ / 30;
const Q: Quality = Quality { width: 64, height: 36 };

/// Тест 27 (п.19): «медленный 4К» + постоянный поток Playback-запросов
/// текущего кадра (имитация неполного превью). Раньше Playback-одиночки
/// вырождали спаны в «один кадр за раз» и покрытие переставало расти.
#[test]
fn t27_span_absorbs_prefetch_tail_under_playback_stream() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let stats = Arc::new(SyntheticStats::default());
    let src = SyntheticSource::with_stats(Tick(90_000 * 60), AudioPattern::Silence, stats.clone())
        .with_video_delay(std::time::Duration::from_millis(8)); // декод дольше «кадра» эмуляции
    let worker = MediaWorker::spawn(MediaId(1), Box::new(src), cache.clone());

    let total: i64 = 90; // 3 c сетки 30 fps
    let t0 = std::time::Instant::now();
    let covered = |cache: &BurstCache| -> i64 {
        cache
            .coverage(source_id(MediaId(1)), BurstKind::video(Some(Q)), 0..total * STEP)
            .iter()
            .map(|r| (r.end - r.start) / STEP)
            .sum()
    };

    let mut mid_covered = None;
    // эмулятор: playhead ползёт по «реальному времени» (25 мс/кадр), Playback
    // на текущий кадр (если не в кэше), NearPrefetch-проход раз в ~100 мс
    let mut last_prefetch = std::time::Instant::now() - std::time::Duration::from_secs(1);
    loop {
        let el = t0.elapsed();
        let cur = ((el.as_millis() as i64) / 25).min(total - 1);
        if !cache.contains_video(source_id(MediaId(1)), Some(Q), cur * STEP) {
            worker.submit(DecodeRequest {
                kind: RequestKind::video_on_grid(Tick(cur * STEP), Some(Q), STEP),
                prio: Priority::Playback,
                generation: 0,
            });
        }
        if last_prefetch.elapsed().as_millis() >= 100 {
            last_prefetch = std::time::Instant::now();
            for i in cur..(cur + 60).min(total) {
                if !cache.contains_video(source_id(MediaId(1)), Some(Q), i * STEP) {
                    worker.submit(DecodeRequest {
                        kind: RequestKind::video_on_grid(Tick(i * STEP), Some(Q), STEP),
                        prio: Priority::NearPrefetch,
                        generation: 0,
                    });
                }
            }
        }
        let c = covered(&cache);
        if c >= total / 2 && mid_covered.is_none() {
            mid_covered = Some(el); // момент половины покрытия
        }
        if c >= total {
            break;
        }
        assert!(
            el < std::time::Duration::from_secs(15),
            "покрытие встало: {}/{total} кадров (стагнация п.19)",
            covered(&cache)
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    // прогресс непрерывный: половина покрыта задолго до конца
    let mid_at = mid_covered.expect("половина покрытия должна была случиться");
    assert!(mid_at < t0.elapsed(), "половина покрытия обязана прийти раньше полного ({mid_at:?})");
    // каждый кадр декодирован ровно один раз
    assert_eq!(stats.video_reads.load(Relaxed), total as u64, "без повторных декодов");
    worker.shutdown();
}

/// Тест 27 (п.19г): ключи за EOF не передекодируются вечно.
#[test]
fn t27_failed_keys_are_not_redecoded() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let stats = Arc::new(SyntheticStats::default());
    let src = SyntheticSource::with_stats(Tick(90_000), AudioPattern::Silence, stats.clone())
        .with_video_eof();
    let worker = MediaWorker::spawn(MediaId(1), Box::new(src), cache.clone());

    let beyond = Tick(90_000 * 10); // далеко за EOF
    for _ in 0..5 {
        worker.submit(DecodeRequest {
            kind: RequestKind::video_on_grid(beyond, Some(Q), STEP),
            prio: Priority::NearPrefetch,
            generation: 0,
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !worker.is_idle() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    assert!(
        stats.video_reads.load(Relaxed) <= 1,
        "EOF-ключ декодируется максимум один раз, было {}",
        stats.video_reads.load(Relaxed)
    );
    worker.shutdown();
}

/// Тест 28 (п.20): воркер вставляет coverage полосой по шагу сетки —
/// весь прогнанный диапазон = ОДИН непрерывный run.
#[test]
fn t28_worker_inserts_grid_span_coverage() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let src = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    let worker = MediaWorker::spawn(MediaId(1), Box::new(src), cache.clone());
    let n: i64 = 30;
    for i in 0..n {
        worker.submit(DecodeRequest {
            kind: RequestKind::video_on_grid(Tick(i * STEP), Some(Q), STEP),
            prio: Priority::NearPrefetch,
            generation: 0,
        });
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !worker.is_idle() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let cov = cache.coverage(source_id(MediaId(1)), BurstKind::video(Some(Q)), 0..n * STEP);
    assert_eq!(cov, vec![0..n * STEP], "полоса должна быть непрерывной: {cov:?}");
    worker.shutdown();
}

/// Тест 758 (Р.63 п.13): после ПРЫЖКА звук не ждёт видео.
///
/// Жалоба владельца: «я просто прыгнул на участок с одним длинным
/// событием… но это не повод так долго подгружать видео с такими
/// underruns». По флайт-логу каждый прыжок в файл 870 с давал секунды
/// полной тишины, при том что офлайн-цена рендера того же места — 2 %
/// реального времени: звук стоял в очереди ЗА кадрами того же воркера.
///
/// Утверждается СЛЕДСТВИЕ: на прыжке первые аудио-бёрсты обслужены раньше
/// видео, то есть звук успевает к дедлайну, а видео догоняет само.
#[test]
fn t758_audio_is_served_before_video_after_a_seek() {
    let cache = Arc::new(BurstCache::new(CacheConfig::default()));
    let stats = Arc::new(SyntheticStats::default());
    // «медленный 1080p»: кадр дороже бёрста аудио — ровно как H.264 у
    // владельца против дешёвого декода Opus
    let src = SyntheticSource::with_stats(Tick(90_000 * 900), AudioPattern::Ramp, stats.clone())
        .with_video_delay(std::time::Duration::from_millis(12));
    let worker = MediaWorker::spawn(MediaId(1), Box::new(src), cache.clone());

    // ПРЫЖОК в хвост: префетч видео уже заказал окно вокруг новой точки и
    // воркер УЖЕ НАЧАЛ его декодировать — именно в эту дыру и попадает
    // звук воспроизведения (спан не смотрел в очередь до самого конца)
    let jump_tick = 800 * TICK_HZ;
    let first_burst = jump_tick / kt_time::AUDIO_BURST_TICKS;
    for i in 0..64i64 {
        worker.submit(DecodeRequest {
            kind: RequestKind::video_on_grid(Tick(jump_tick + i * STEP), Some(Q), STEP),
            prio: Priority::NearPrefetch,
            generation: 0,
        });
    }
    // даём воркеру войти в спан (один кадр = 12 мс)
    std::thread::sleep(std::time::Duration::from_millis(30));
    for i in 0..8i64 {
        worker.submit(DecodeRequest {
            kind: RequestKind::Audio { idx: first_burst + i },
            prio: Priority::Playback,
            generation: 0,
        });
    }

    // Звук обязан появиться БЫСТРО: восемь бёрстов — это 170 мс материала,
    // и ждать их дольше, чем они звучат, значит гарантированно молчать.
    // Мерка — кольцо вывода (около 85 мс): всё, что дольше, слышно тишиной.
    // Спан здесь 64 кадра по 12 мс = 768 мс одним куском на прежнем коде.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(250);
    let mut got = 0;
    while std::time::Instant::now() < deadline {
        got = (0..8)
            .filter(|i| cache.get_audio(source_id(MediaId(1)), first_burst + i).is_some())
            .count();
        if got == 8 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let vid = stats.video_reads.load(Relaxed);
    assert_eq!(
        got, 8,
        "звук после прыжка не успел: прочитано {got} из 8 бёрстов, \
         а видео-кадров за то же время — {vid} (звук ждёт видео)"
    );
    // и видео при этом не брошено — оно догоняет следом
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(900);
    while std::time::Instant::now() < deadline && stats.video_reads.load(Relaxed) < 8 {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        stats.video_reads.load(Relaxed) >= 8,
        "видео перестало догонять после того, как звук пропустили вперёд: кадров {}, звук {}",
        stats.video_reads.load(Relaxed),
        stats.audio_reads.load(Relaxed)
    );
    worker.shutdown();
}
