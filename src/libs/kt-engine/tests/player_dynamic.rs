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
        reversed: false,
        fx_bypass: vec![],
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
        reversed: false,
        fx_bypass: vec![],
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
    // prefetch живой и мог ДОГРЕТЬ ещё — проверяем, что покрытие не сжалось
    let covered = |cov: &[std::ops::Range<i64>], x: i64| cov.iter().any(|r| r.contains(&x));
    for r in &cov_before {
        for x in [r.start, (r.start + r.end) / 2, r.end - 1] {
            assert!(
                covered(&cov_after, x),
                "burst {x} был в кэше до add_media и пропал: до {cov_before:?}, после {cov_after:?}"
            );
        }
    }
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

/// Тест 20 (п.1): в паузе фоновые потоки НЕ жгут ресурсы.
/// Раньше: audio-render крутился 500 раз/с, prefetch клонировал весь
/// Project и пересобирал рендереры каждые 100 мс — система лагала даже
/// с пустым проектом.
#[test]
fn t20_paused_player_is_quiescent() {
    let project = base_project();
    let (player, _consumer) = Player::new(project.clone(), vec![(MediaId(1), synth())]);
    std::thread::sleep(std::time::Duration::from_millis(400)); // прогрев/стабилизация

    use std::sync::atomic::Ordering::Relaxed;
    let r0 = player.loop_stats.render_passes.load(Relaxed);
    let rb0 = player.loop_stats.prefetch_rebuilds.load(Relaxed);
    std::thread::sleep(std::time::Duration::from_secs(1));
    let render_delta = player.loop_stats.render_passes.load(Relaxed) - r0;
    let rebuild_delta = player.loop_stats.prefetch_rebuilds.load(Relaxed) - rb0;

    assert!(
        render_delta <= 8,
        "в паузе render-поток должен парковаться (страховочный таймаут 250мс), \
         а не крутиться: {render_delta} проходов за 1 с (раньше ~500)"
    );
    assert_eq!(rebuild_delta, 0, "prefetch не должен пересобирать рендереры без правок модели");

    // правка модели будит и пересобирает ровно один раз
    player.set_project(project);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let rebuilds = player.loop_stats.prefetch_rebuilds.load(Relaxed) - rb0;
    assert!(rebuilds >= 1, "после set_project prefetch обязан пересобрать графы");
}

/// Пауза → продолжение: рендер просыпается и кольцо снова наполняется
/// (защита от дедлока на condvar; страхует t14).
#[test]
fn t20_pause_resume_still_streams() {
    let project = base_project();
    let (player, mut consumer) = Player::new(project, vec![(MediaId(1), synth())]);
    // дождаться прогрева кэша, чтобы рендер не молчал из-за промахов
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while player
        .cache
        .coverage(kt_engine::source_id(MediaId(1)), kt_cache::BurstKind::Audio, 0..40)
        .is_empty()
    {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let drain = |c: &mut rtrb::Consumer<f32>| {
        let mut n = 0usize;
        while c.pop().is_ok() {
            n += 1;
        }
        n
    };

    player.play();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(drain(&mut consumer) > 0, "после play кольцо должно наполняться");

    player.pause();
    std::thread::sleep(std::time::Duration::from_millis(200));
    drain(&mut consumer);

    player.play();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(drain(&mut consumer) > 0, "после resume рендер обязан проснуться (condvar)");
}
