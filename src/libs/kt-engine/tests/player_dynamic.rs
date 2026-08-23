//! Тест 18 (docs/porting-notes.md): добавление медиа НЕ сбрасывает кэш
//! (регрессия п.27: rebuild_player пересоздавал Player целиком — прогретый
//! кэш, транспорт и кольцо выбрасывались при каждом дропе файла).

use kt_engine::Player;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;

fn base_project() -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
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
    p.validate().unwrap();
    p
}

fn synth() -> Box<SyntheticSource> {
    Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 220.0 }))
}

#[test]
fn t18_add_media_preserves_cache_coverage_and_transport() {
    let project = base_project();
    let (player, _consumer) = Player::new(project.clone(), vec![(MediaId(1), synth())]);

    // прогрев: prefetch-поток наполняет кэш вокруг playhead
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let cov = player.cache.coverage(
            kt_engine::source_id(MediaId(1)),
            kt_cache::BurstKind::Audio,
            0..1_000_000,
        );
        let covered: i64 = cov.iter().map(|r| r.end - r.start).sum();
        if covered >= 40 {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "кэш не прогрелся за 10 с");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let pos = Tick(90_000); // 1 c
    player.seek(pos);
    let bytes_before = player.cache.audio_bytes();
    let cov_before = player.cache.coverage(
        kt_engine::source_id(MediaId(1)),
        kt_cache::BurstKind::Audio,
        0..1_000_000,
    );
    let gen_before = player.transport.generation();
    assert!(bytes_before > 0);

    // добавление второго медиа — инкрементально
    player.add_media(MediaId(2), synth());
    let mut p2 = project.clone();
    p2.media.push(MediaRef { id: MediaId(2), path: "synth:2".into() });
    p2.clips.push(ClipEvent {
        id: ClipId(2),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 1,
        start: Tick(0),
        duration: Tick(90_000 * 4),
        source_in: Tick(0),
        media: MediaId(2),
        gain_db: 0.0,
        fx: vec![],
    });
    p2.validate().unwrap();
    player.set_project(p2);

    // кэш и транспорт пережили добавление
    assert_eq!(player.transport.playhead(), pos, "playhead не должен сбрасываться");
    assert_eq!(player.transport.generation(), gen_before, "generation не должен бампаться");
    let cov_after = player.cache.coverage(
        kt_engine::source_id(MediaId(1)),
        kt_cache::BurstKind::Audio,
        0..1_000_000,
    );
    assert_eq!(cov_before, cov_after, "coverage медиа 1 не должен пострадать");
    assert!(player.cache.audio_bytes() >= bytes_before, "кэш не должен сжаться");

    // новый воркер живой: prefetch наполняет медиа 2
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let cov = player.cache.coverage(
            kt_engine::source_id(MediaId(2)),
            kt_cache::BurstKind::Audio,
            0..1_000_000,
        );
        if !cov.is_empty() {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "новое медиа не декодируется — воркер мёртв");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
