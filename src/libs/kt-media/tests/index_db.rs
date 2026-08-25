//! Тесты 15–16 (docs/porting-notes.md): peak-пирамида и инвалидация индекс-БД.

use kt_media::index::{MediaEntryStatus, MediaIndexDb, PeakPyramid};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, PEAK_BLOCK_FRAMES};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};

fn synth(seconds: i64, pattern: AudioPattern) -> SyntheticSource {
    SyntheticSource::new(Tick(90_000 * seconds), pattern)
}

// ---- Тест 15: peak-пирамида -------------------------------------------------

#[test]
fn t15_pyramid_matches_brute_force() {
    let mut src = synth(2, AudioPattern::Sine { hz: 997.0 });
    let pyr = PeakPyramid::build(&mut src);

    // пересчёт «в лоб» всех сэмплов
    let frames = Tick(90_000 * 2).to_audio_frames();
    let bursts = frames / AUDIO_BURST_FRAMES as i64;
    let usable = (bursts * AUDIO_BURST_FRAMES as i64) as usize;
    let mut all = vec![0i16; usable * AUDIO_CHANNELS];
    let mut check = synth(2, AudioPattern::Sine { hz: 997.0 });
    for b in 0..bursts {
        let burst = check.read_audio_burst(b).unwrap();
        for (i, v) in burst.data.iter().enumerate() {
            all[b as usize * AUDIO_BURST_FRAMES * AUDIO_CHANNELS + i] =
                (v.clamp(-1.0, 1.0) * 32767.0) as i16;
        }
    }
    // каждый уровень: блок 256*2^level кадров
    for (level, channels) in pyr.levels.iter().enumerate() {
        let block = PEAK_BLOCK_FRAMES << level;
        for (ch, blocks) in channels.iter().enumerate() {
            for (bi, &(mn, mx)) in blocks.iter().enumerate() {
                let from = bi * block;
                let to = ((bi + 1) * block).min(usable);
                if from >= to {
                    continue;
                }
                let mut emn = i16::MAX;
                let mut emx = i16::MIN;
                for f in from..to {
                    let v = all[f * AUDIO_CHANNELS + ch];
                    emn = emn.min(v);
                    emx = emx.max(v);
                }
                assert_eq!((mn, mx), (emn, emx), "level {level} ch {ch} block {bi}");
            }
        }
    }
    // вершина пирамиды — один блок
    assert_eq!(pyr.levels.last().unwrap()[0].len(), 1);
}

#[test]
fn t15_reopen_reads_from_db_without_decode() {
    let dir = tempfile::tempdir().unwrap();
    let media_file = dir.path().join("tone.bin");
    std::fs::write(&media_file, b"fake media content v1").unwrap();
    let db_path = dir.path().join("p.ktindex.db");

    {
        let mut db = MediaIndexDb::open(&db_path).unwrap();
        let (id, status) = db.ensure_media(&media_file).unwrap();
        assert_eq!(status, MediaEntryStatus::Rebuilt);
        let mut src = synth(1, AudioPattern::Ramp);
        db.build_audio_peaks(id, &mut src).unwrap();
        assert!(src.audio_reads > 0);
        db.mark_complete(id).unwrap();
    }

    // повторное открытие: записи переиспользуются, декода нет
    let mut db = MediaIndexDb::open(&db_path).unwrap();
    let (id, status) = db.ensure_media(&media_file).unwrap();
    assert_eq!(status, MediaEntryStatus::Reused);
    assert!(db.is_complete(id).unwrap());
    let peaks = db.audio_peaks(id, 0, 0).unwrap().expect("peaks persisted");
    assert!(!peaks.is_empty());
    // ramp: min каждого блока = первый сэмпл блока (масштаб 1e-6 * 32767)
    let expect0 = (SyntheticSource::sample_at(AudioPattern::Ramp, 0, 0) * 32767.0) as i16;
    assert_eq!(peaks[0].0, expect0);
}

// ---- Тест 16: инвалидация ---------------------------------------------------

#[test]
fn t16_content_change_rebuilds_only_that_media() {
    let dir = tempfile::tempdir().unwrap();
    let file_a = dir.path().join("a.bin");
    let file_b = dir.path().join("b.bin");
    std::fs::write(&file_a, b"content A v1").unwrap();
    std::fs::write(&file_b, b"content B v1").unwrap();
    let db_path = dir.path().join("p.ktindex.db");

    let mut db = MediaIndexDb::open(&db_path).unwrap();
    let (ida, _) = db.ensure_media(&file_a).unwrap();
    let (idb, _) = db.ensure_media(&file_b).unwrap();
    let mut src = synth(1, AudioPattern::Ramp);
    db.build_audio_peaks(ida, &mut src).unwrap();
    db.build_audio_peaks(idb, &mut src).unwrap();
    db.mark_complete(ida).unwrap();
    db.mark_complete(idb).unwrap();

    // подмена содержимого A при том же пути
    std::fs::write(&file_a, b"content A v2 -- different").unwrap();
    let (ida2, status_a) = db.ensure_media(&file_a).unwrap();
    assert_eq!(status_a, MediaEntryStatus::Rebuilt);
    assert_eq!(ida2, ida, "media_id стабилен, перестраиваются только секции");
    assert!(db.audio_peaks(ida, 0, 0).unwrap().is_none(), "секции A сброшены");
    assert!(!db.is_complete(ida).unwrap());
    // B не тронут
    assert!(db.audio_peaks(idb, 0, 0).unwrap().is_some(), "B не должен пострадать");
    assert!(db.is_complete(idb).unwrap());
}

#[test]
fn t16_mtime_touch_without_content_change_reuses() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.bin");
    std::fs::write(&file, b"stable content").unwrap();
    let db_path = dir.path().join("p.ktindex.db");

    let mut db = MediaIndexDb::open(&db_path).unwrap();
    let (id, _) = db.ensure_media(&file).unwrap();
    let mut src = synth(1, AudioPattern::Ramp);
    db.build_audio_peaks(id, &mut src).unwrap();
    db.mark_complete(id).unwrap();

    // touch: тот же контент, новый mtime
    std::thread::sleep(std::time::Duration::from_millis(20));
    let content = std::fs::read(&file).unwrap();
    std::fs::write(&file, &content).unwrap();

    let (id2, status) = db.ensure_media(&file).unwrap();
    assert_eq!(id2, id);
    assert_eq!(status, MediaEntryStatus::Reused, "хэш совпал — записи переиспользуются");
    assert!(db.audio_peaks(id, 0, 0).unwrap().is_some());
}

#[test]
fn t16_corrupted_db_is_recreated_without_panic() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("p.ktindex.db");
    std::fs::write(&db_path, b"this is definitely not sqlite").unwrap();

    let mut db = MediaIndexDb::open(&db_path).expect("битая БД пересоздаётся, не падает");
    let file = dir.path().join("a.bin");
    std::fs::write(&file, b"x").unwrap();
    let (_, status) = db.ensure_media(&file).unwrap();
    assert_eq!(status, MediaEntryStatus::Rebuilt);
}

// ---- Тест 70 (п.14 р.7): ближайший тумбнейл в допуске -----------------------

#[test]
fn t70_thumbnail_near() {
    let dir = tempfile::tempdir().unwrap();
    let media_file = dir.path().join("v.bin");
    std::fs::write(&media_file, b"fake video").unwrap();
    let db_path = dir.path().join("p.ktindex.db");
    let mut db = MediaIndexDb::open(&db_path).unwrap();
    let (id, _) = db.ensure_media(&media_file).unwrap();
    db.put_thumbnail(id, 90_000, b"jpeg-a").unwrap();
    db.put_thumbnail(id, 180_000, b"jpeg-b").unwrap();

    // точное совпадение приоритетно
    assert_eq!(
        db.thumbnail_near(id, 90_000, 45_000).unwrap(),
        Some((90_000, b"jpeg-a".to_vec()))
    );
    // ближайший в допуске (100_000 ближе к 90_000, чем к 180_000)
    assert_eq!(
        db.thumbnail_near(id, 100_000, 45_000).unwrap(),
        Some((90_000, b"jpeg-a".to_vec()))
    );
    assert_eq!(
        db.thumbnail_near(id, 150_000, 45_000).unwrap(),
        Some((180_000, b"jpeg-b".to_vec()))
    );
    // вне допуска — None (скраб не должен врать позицией)
    assert_eq!(db.thumbnail_near(id, 400_000, 45_000).unwrap(), None);
    // границы BETWEEN включительны
    assert_eq!(
        db.thumbnail_near(id, 135_000, 45_000).unwrap().map(|(t, _)| t),
        Some(90_000)
    );
    // чужое медиа не видно
    assert_eq!(db.thumbnail_near(id + 1, 90_000, 45_000).unwrap(), None);
}
