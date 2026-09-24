//! Тесты 416-417 (Р.49 Б9): команда синхронизации по звуку.

use kt_model::*;
use kt_time::Tick;

fn scene_at(start_a: Tick, source_in_b: Tick, start_b: Tick, reversed_b: bool) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.media.push(MediaRef::new(MediaId(2), "synth:sine:440"));
    let clip = |id: u64, media: u64, lane: u32, start: Tick, src: Tick, rev: bool| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane,
        start,
        duration: Tick(900_000),
        source_in: src,
        media: MediaId(media),
        gain_db: 0.0,
        reversed: rev,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    p.clips.push(clip(1, 1, 0, start_a, Tick(90_000), false));
    p.clips.push(clip(2, 2, 1, start_b, source_in_b, reversed_b));
    p.ensure_tracks(LaneKind::Audio, 2);
    p.normalize();
    p
}

fn scene(source_in_b: Tick, start_b: Tick, reversed_b: bool) -> Project {
    scene_at(Tick(180_000), source_in_b, start_b, reversed_b)
}

/// Тест 416 (Р.49 Б9): сдвиг применяется так, что материалы совмещаются, и
/// учитывает подрезанное начало каждого события.
#[test]
fn t416_sync_aligns_the_material_not_the_clip_edges() {
    // B входит в свой материал с другой точки и стоит не там
    let p = scene(Tick(45_000), Tick(600_000), false);
    // «материал B отстаёт от материала A на 0.2 с»
    let off = Tick(18_000);
    let ops::SyncPlan::Move(cmds) = ops::sync_by_audio_commands(&p, ClipId(1), ClipId(2), off)
    else {
        panic!("ожидался перенос");
    };
    assert_eq!(cmds.len(), 1, "ожидалась одна команда переноса: {cmds:?}");
    let mut m = TimelineModel::new(p.clone());
    m.apply_batch(cmds).expect("перенос");
    let b = m.project().clips.iter().find(|c| c.id == ClipId(2)).unwrap().clone();

    // проверяем СЛЕДСТВИЕ: момент реального мира, стоящий в начале A,
    // теперь стоит там же и у B
    let step = 90_000 / 30;
    let mat_a =
        clip_material_at(Tick(90_000), Tick(180_000), Tick(900_000), false, step, Tick(180_000));
    let mat_b = Tick(mat_a.0 + off.0);
    let at_b = clip_timeline_at(b.source_in, b.start, b.duration, b.reversed, step, mat_b);
    assert_eq!(at_b.0, 180_000, "материалы не совместились: момент B встал в {}", at_b.0);

    // повтор ничего не двигает: уже совмещено
    let again = ops::sync_by_audio_commands(m.project(), ClipId(1), ClipId(2), off);
    assert_eq!(
        again,
        ops::SyncPlan::AlreadyAligned,
        "повторная синхронизация двигает уже совмещённое: {again:?}"
    );
}

/// Тест 417 (Р.49 Б9): синхронизация НЕ тримит и НЕ линкует.
///
/// Она отвечает на вопрос «где», а не «как долго» и не «вместе ли»: магнит
/// подрезает соседа — это его свойство, а link-группа — отдельное решение
/// монтажёра. И реверс она обязана понимать: у перевёрнутого события
/// материал идёт назад, и своя арифметика по `source_in` тут уехала бы.
#[test]
fn t417_sync_only_moves_and_understands_reverse() {
    let p = scene(Tick(45_000), Tick(600_000), false);
    let before = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap().clone();
    let ops::SyncPlan::Move(cmds) =
        ops::sync_by_audio_commands(&p, ClipId(1), ClipId(2), Tick(18_000))
    else {
        panic!("ожидался перенос");
    };
    for c in &cmds {
        assert!(
            matches!(c, Command::MoveClip { .. }),
            "синхронизация выдала команду не переноса: {c:?}"
        );
    }
    let mut m = TimelineModel::new(p);
    m.apply_batch(cmds).unwrap();
    let after = m.project().clips.iter().find(|c| c.id == ClipId(2)).unwrap().clone();
    assert_eq!(after.duration, before.duration, "синхронизация подрезала событие");
    assert_eq!(after.source_in, before.source_in, "синхронизация сдвинула точку входа");
    assert_eq!(after.link_group, before.link_group, "синхронизация связала события");
    assert_eq!(after.lane, before.lane, "синхронизация переставила дорожку");

    // РЕВЕРС: то же следствие на перевёрнутом событии. Событие стоит
    // ПОЗЖЕ: у перевёрнутого начало материала показывается в конце, и
    // совмещение тянет его назад почти на всю длину.
    // у перевёрнутого события начало материала показывается в КОНЦЕ, и
    // совмещение тянет его назад почти на всю длину — значит цель обязана
    // стоять не в самом начале проекта, иначе места не хватит (t417b)
    let p = scene_at(Tick(900_000), Tick(45_000), Tick(2_500_000), true);
    let off = Tick(18_000);
    let ops::SyncPlan::Move(cmds) = ops::sync_by_audio_commands(&p, ClipId(1), ClipId(2), off)
    else {
        panic!("ожидался перенос на реверсе");
    };
    let mut m = TimelineModel::new(p);
    m.apply_batch(cmds).unwrap();
    let b = m.project().clips.iter().find(|c| c.id == ClipId(2)).unwrap().clone();
    let step = 90_000 / 30;
    let mat_a =
        clip_material_at(Tick(90_000), Tick(900_000), Tick(900_000), false, step, Tick(900_000));
    let at_b =
        clip_timeline_at(b.source_in, b.start, b.duration, b.reversed, step, Tick(mat_a.0 + off.0));
    assert_eq!(at_b.0, 900_000, "на реверсе материалы не совместились: {}", at_b.0);
}

/// Совмещение, уводящее событие за начало проекта, — ОТКАЗ, а не кламп.
/// Прижатое к нулю событие выглядит как успешно синхронизированное, а
/// материалы при этом остаются вразнобой.
#[test]
fn t417b_sync_refuses_instead_of_clamping_to_zero() {
    let p = scene(Tick(45_000), Tick(600_000), true);
    let r = ops::sync_by_audio_commands(&p, ClipId(1), ClipId(2), Tick(18_000));
    assert_eq!(
        r,
        ops::SyncPlan::OutOfRange,
        "совмещение за начало проекта прижато к нулю и выдано за успех: {r:?}"
    );
}
