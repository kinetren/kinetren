//! Тест 18 (docs/porting-notes.md): добавление медиа НЕ сбрасывает кэш
//! (регрессия п.27: rebuild_player пересоздавал Player целиком — прогретый
//! кэш, транспорт и кольцо выбрасывались при каждом дропе файла).

use kt_engine::Player;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;

fn base_project() -> Project {
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
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
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
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
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

/// Тест 211 (Р.33 п.10): вернувшийся на диск исходник подменяет свою
/// заглушку НА ХОДУ. Кадры старого источника обязаны уйти из кэша —
/// иначе после возврата файла в превью ещё долго висела бы плашка.
#[test]
fn t211_replace_media_swaps_source_and_drops_its_cache() {
    let project = base_project();
    let dc = |v: f32| -> Box<SyntheticSource> {
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Dc { value: v }))
    };
    let (player, _consumer) = Player::new(project, vec![(MediaId(1), dc(1.0))]);

    // прогрев кэша первого источника
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
    let old = player
        .cache
        .get_audio(kt_engine::source_id(MediaId(1)), 0)
        .expect("burst источника обязан лежать в кэше");
    assert!((old.data[0] - 1.0).abs() < 1e-6, "прогрели не тот сигнал");

    // подмена источника: кэш ЭТОГО медиа выброшен
    player.replace_media(MediaId(1), dc(0.25));
    assert!(
        player.cache.get_audio(kt_engine::source_id(MediaId(1)), 0).is_none(),
        "кадры прежнего источника остались в кэше — превью показывало бы заглушку"
    );

    // новый источник наполняет кэш своими значениями
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(b) = player.cache.get_audio(kt_engine::source_id(MediaId(1)), 0) {
            assert!((b.data[0] - 0.25).abs() < 1e-6, "в кэше значения старого источника");
            break;
        }
        assert!(std::time::Instant::now() < deadline, "новый источник не наполнил кэш");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Тест 316 (Р.46, п.321): вернувшийся файл приносит СВОЙ натив, и ступень
/// декода пересчитывается на ходу. Пока на месте заглушка (натив мельче
/// Quality) клип с зумом декодируется как обычно; подменили источником с
/// запасом — качество декода клипа выросло, и полоски/префетч обязаны
/// спрашивать уже его.
#[test]
fn t316_replace_media_updates_the_decode_stage() {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 128,
        height: 72,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef { id: MediaId(1), path: "checker".into() });
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![FxInstance::geometry_zoom(2.0)],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    let small: Box<dyn kt_media::MediaSource> =
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence));
    let (player, _consumer) = Player::new(p, vec![(MediaId(1), small)]);
    player.set_preview_quality(kt_media::Quality { width: 128, height: 72 });
    assert_eq!(
        player.video_decode_qualities(),
        vec![(ClipId(1), kt_media::Quality { width: 128, height: 72 })],
        "у источника 64x36 запаса над Quality нет — ступени быть не может"
    );

    player.replace_media(
        MediaId(1),
        Box::new(kt_media::synthetic::CheckerSource::new(Tick(90_000 * 60), 512, 288)),
    );
    // пересборка рендерера идёт по версии проекта (её бампает replace_media)
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if player.video_decode_qualities()
            == vec![(ClipId(1), kt_media::Quality { width: 256, height: 144 })]
        {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "ступень не пересчиталась после подмены");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Тест 500 (Р.53, п.411): перенастройка источника НЕ пересоздаёт его.
/// Поток декодера жив (счётчик спавнов не растёт), рендереры не
/// пересобираются (версия проекта не растёт), а картинка при этом новая
/// — кадры старых параметров выброшены. До этого пункта каждое нажатие
/// клавиши в поле титра гасило и поднимало поток и дважды пересобирало
/// рендереры (претензия владельца Р.53).
#[test]
fn t500_retune_keeps_worker_and_renderer() {
    let mut project = base_project();
    // клип-генератор поверх аудио-клипа базового проекта
    project.media.push(MediaRef { id: MediaId(2), path: "gen:solid".into() });
    project.clips.push(ClipEvent {
        // Р.59, стадия 2: значения — на СОБЫТИИ, источник стоит по его адресу
        r#gen: Some(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(2),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000 * 4),
        source_in: Tick(0),
        media: MediaId(2),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    project.validate().unwrap();

    // источник генератора поднимает САМ плеер (`sync_generators`): его
    // адрес производен от события, и вызывающему его взять неоткуда
    let (player, _consumer) = Player::new(
        project,
        vec![(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Dc { value: 1.0 })),
        )],
    );
    let q = kt_media::Quality { width: 64, height: 36 };
    player.set_preview_quality(q);
    let sid = kt_engine::source_id(kt_engine::gen_source_addr(ClipId(2)));
    let idx = kt_engine::video::video_cache_idx(Tick(0));

    // прогреваем кадр чёрного генератора
    player.cache.insert_video(
        sid,
        Some(q),
        idx,
        kt_media::VideoFrame {
            pts: Tick(0),
            width: q.width,
            height: q.height,
            rgba: vec![0u8; (q.width * q.height * 4) as usize].into_boxed_slice(),
            has_alpha: false,
        }
        .into(),
    );
    // серийник прогретой записи: по нему видно, что кадр старых
    // параметров именно ВЫБРОСИЛИ, а не пережил перенастройку
    let (_, warm_serial) =
        player.cache.get_video_stamped(sid, Some(q), idx).expect("прогретый кадр");
    let spawns = player.worker_spawns();
    let pver = player.project_version();
    let cver = player.content_version();

    let orange = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    assert!(
        kt_media::factory::can_retune_gen(
            &kt_model::GenState::new(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
            &kt_model::GenState::new(orange.clone())
        ),
        "цвет меняется без смены паспорта"
    );
    assert!(player.update_gen(ClipId(2), orange, Vec::new()));

    assert_eq!(player.worker_spawns(), spawns, "поток декодера обязан остаться живым");
    assert_eq!(player.project_version(), pver, "рендереры пересобирать незачем: план тот же");
    assert!(player.content_version() > cver, "содержимое источника изменилось — префетч будит");

    // кадр старых параметров выброшен, новый приходит с НОВЫМ цветом.
    // Дедлайн щедрый: утверждение — «перенастройка доезжает», а не «она
    // быстрая», и под параллельным прогоном всего workspace поток
    // воркера ждёт своей очереди на CPU (тот же урок, что у t399/t476).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let got = loop {
        // «Пусто видели» ловить нельзя: воркер успевает и выбросить, и
        // заполнить между двумя опросами, и под нагрузкой тест краснел на
        // исправном коде. Утверждение то же, но детерминированное —
        // ЗАПИСЬ ДРУГАЯ (серийник монотонен и не переиспользуется, п.409),
        // значит кадр старых параметров выброшен, а не пережил правку.
        if let Some((f, serial)) = player.cache.get_video_stamped(sid, Some(q), idx)
            && serial != warm_serial
        {
            break f;
        }
        assert!(std::time::Instant::now() < deadline, "кадр новых параметров не приехал");
        // превью-композит сам заказывает недостающие кадры у воркера —
        // это и есть продуктовый путь заказа
        let mut buf = Vec::new();
        let _ = player.compose_preview_into(Tick(0), &mut buf);
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(&got.rgba[..3], &[255, 136, 0], "новая запись несёт СТАРЫЙ цвет");
    // и поток за всё это время так и не пересоздавался
    assert_eq!(player.worker_spawns(), spawns);
    assert_eq!(player.project_version(), pver);
}

/// Тест 661 (Р.59, стадия 2): правка ЗНАЧЕНИЯ генератора не пересобирает
/// рендереры и не растит версию проекта.
///
/// Это и есть смысл всей стадии: адрес источника считается от СОБЫТИЯ, а
/// не от значений, поэтому план от них не зависит — а `set_project` это
/// клон всего проекта плюс остановка аудио-render-потока. До Р.53 такую
/// цену платило каждое нажатие клавиши в поле титра; здесь то же правило
/// закрепляется за новым носителем значений.
#[test]
fn t661_a_value_edit_does_not_rebuild_the_renderer() {
    use kt_model::{Command, GeneratorKind, ParamCurve};

    // закон П на уровне КОМАНД: исчерпывающий match в kt-model не даст
    // забыть про новую команду, а здесь зафиксирован её ответ
    let set = Command::SetClipGen {
        id: ClipId(2),
        kind: Some(GeneratorKind::Solid { color_r: 1, color_g: 1, color_b: 1 }),
    };
    let auto = Command::SetClipGenAuto { id: ClipId(2), auto: Vec::<ParamCurve>::new() };
    assert!(!set.touches_render_plan(), "значения генератора плана не касаются");
    assert!(!auto.touches_render_plan(), "кривые генератора разрешает ИСТОЧНИК, не план");
    // а вот «применить ко всем» правит ДОКУМЕНТ — состав источников мог
    // измениться, и звать её как правку значения нельзя
    assert!(
        Command::ApplyGenToAll {
            media: MediaId(2),
            kind: GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 },
            auto: vec![],
        }
        .touches_render_plan()
    );

    // и то же самое у плеера: поток жив, версия проекта стоит
    let mut project = base_project();
    project.media.push(MediaRef { id: MediaId(2), path: "gen:solid".into() });
    project.clips.push(ClipEvent {
        r#gen: Some(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(2),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000 * 4),
        source_in: Tick(0),
        media: MediaId(2),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    project.validate().unwrap();
    let (player, _consumer) = Player::new(
        project,
        vec![(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Dc { value: 1.0 })),
        )],
    );
    let spawns = player.worker_spawns();
    let pver = player.project_version();
    let cver = player.content_version();
    assert!(player.update_gen(
        ClipId(2),
        GeneratorKind::Solid { color_r: 9, color_g: 9, color_b: 9 },
        kt_model::text_anim_curves(kt_model::TextAnim::Typewriter, 1.0, 2.0),
    ));
    assert_eq!(player.worker_spawns(), spawns, "поток декодера пересоздавать незачем");
    assert_eq!(player.project_version(), pver, "рендереры пересобирать незачем: план тот же");
    assert!(player.content_version() > cver, "префетч обязан проснуться");
    // хранимый проект остаётся правдивым: его читают экспорт и диагностика
    let p = player.project();
    let c = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert!(matches!(c.r#gen, Some(GeneratorKind::Solid { color_r: 9, .. })));
    assert!(!c.gen_auto.is_empty(), "кривые обязаны доехать до хранимого проекта");
}
