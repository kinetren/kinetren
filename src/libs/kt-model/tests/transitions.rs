//! Тест 135 (п.135 porting-notes, Р.24): переходы в модели.
//!
//! Кроссфейд — явный TransitionEvent на стыке двух соседних клипов (OTIO),
//! а не перекрытие. Здесь: команды/undo, инварианты валидации, судьба
//! перехода при правках соседей (reconcile), ретаргет при Split.

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
    p.media.push(MediaRef { id: MediaId(1), path: "synth:Ramp".into() });
    for (id, start) in [(1u64, 0i64), (2, 48_000), (3, 96_000)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(id),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(start),
            duration: Tick(48_000),
            source_in: Tick(start.max(0)),
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
    p
}

fn tr(id: u64, a: u64, b: u64, in_off: i64, out_off: i64) -> TransitionEvent {
    TransitionEvent {
        id: TransitionId(id),
        a: Some(ClipId(a)),
        b: Some(ClipId(b)),
        in_offset: Tick(in_off),
        out_offset: Tick(out_off),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    }
}

#[test]
fn t135_commands_undo_and_window() {
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 4_500))).unwrap();
    assert_eq!(m.project().transitions.len(), 1);
    let w = m.project().transitions[0].window(m.project()).unwrap();
    assert_eq!((w.start, w.end), (Tick(39_000), Tick(52_500)), "window around the joint");

    m.apply(Command::SetTransition(tr(1, 1, 2, 1_000, 2_000))).unwrap();
    assert_eq!(m.project().transitions[0].in_offset, Tick(1_000));
    m.apply(Command::RemoveTransition { id: TransitionId(1) }).unwrap();
    assert!(m.project().transitions.is_empty());

    m.undo().unwrap(); // remove откатился
    assert_eq!(m.project().transitions[0].in_offset, Tick(1_000));
    m.undo().unwrap(); // set откатился
    assert_eq!(m.project().transitions[0].in_offset, Tick(9_000));
    m.undo().unwrap(); // add откатился
    assert!(m.project().transitions.is_empty());
}

#[test]
fn t135_validation_rejects_bad_transitions() {
    let mut m = TimelineModel::new(base_project());
    // не соседи (1 и 3 не встык)
    assert!(matches!(
        m.apply(Command::AddTransition(tr(1, 1, 3, 900, 900))),
        Err(CommandError::InvalidTransition(ProjectError::TransitionNotAtJoint(_)))
    ));
    // окно шире тела клипа
    assert!(matches!(
        m.apply(Command::AddTransition(tr(1, 1, 2, 48_001, 0))),
        Err(CommandError::InvalidTransition(ProjectError::TransitionTooWide(_)))
    ));
    // уже минимума
    assert!(matches!(
        m.apply(Command::AddTransition(tr(1, 1, 2, 44, 45))),
        Err(CommandError::InvalidTransition(ProjectError::TransitionTooShort(_)))
    ));
    // отрицательный офсет
    assert!(matches!(
        m.apply(Command::AddTransition(tr(1, 1, 2, -10, 900))),
        Err(CommandError::InvalidTransition(ProjectError::TransitionTooWide(_)))
    ));

    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    // второй переход на тот же стык
    assert!(matches!(
        m.apply(Command::AddTransition(tr(2, 1, 2, 900, 900))),
        Err(CommandError::InvalidTransition(ProjectError::DuplicateTransitionJoint(_, _)))
    ));
    // окна соседних переходов одной дорожки пересекаются:
    // окно (1|2) = [39000, 57000), окно (2|3) с in=9000 начинается на 87000 — ок,
    // а с in=48000/out=0 стартует на 48000 < 57000 — конфликт
    assert!(matches!(
        m.apply(Command::AddTransition(tr(2, 2, 3, 48_000, 0))),
        Err(CommandError::InvalidTransition(ProjectError::TransitionWindowsOverlap(_, _)))
    ));
    m.apply(Command::AddTransition(tr(2, 2, 3, 9_000, 9_000))).unwrap();

    // занятый id и несуществующий id
    assert!(matches!(
        m.apply(Command::AddTransition(tr(2, 1, 2, 900, 900))),
        Err(CommandError::IdInUse(2))
    ));
    assert!(matches!(
        m.apply(Command::SetTransition(tr(99, 1, 2, 900, 900))),
        Err(CommandError::TransitionNotFound(TransitionId(99)))
    ));
    assert!(matches!(
        m.apply(Command::RemoveTransition { id: TransitionId(99) }),
        Err(CommandError::TransitionNotFound(TransitionId(99)))
    ));

    // validate() проекта ловит то же самое напрямую (для конвертеров)
    let mut p = base_project();
    p.transitions.push(tr(7, 1, 3, 900, 900));
    assert!(matches!(p.validate(), Err(ProjectError::TransitionNotAtJoint(TransitionId(7)))));

    // Инвариант этапа 6 (реш. 8 Р.24) действует, когда переслаивание
    // ВЫКЛЮЧЕНО (Р.28): тогда перекрытие клипов одной дорожки незаконно — и в
    // validate(), и через жёсткую проверку apply_batch
    let mut p = base_project();
    p.overlap_layers = false;
    p.clips[1].start = Tick(40_000); // наезд на клип 1
    assert!(matches!(p.validate(), Err(ProjectError::ClipOverlap(ClipId(1), ClipId(2)))));
    let mut base = base_project();
    base.overlap_layers = false;
    let mut m = TimelineModel::new(base);
    assert!(matches!(
        m.apply(Command::MoveClip { id: ClipId(2), new_start: Tick(40_000), new_lane: 0 }),
        Err(CommandError::InvalidLayout(ProjectError::ClipOverlap(_, _)))
    ));
    // и с foreign-плейсхолдером
    let mut p = base_project();
    p.foreign.push(ForeignEvent {
        key: 1,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(46_000),
        duration: Tick(10_000),
    });
    assert!(matches!(p.validate(), Err(ProjectError::ForeignOverlap(_, 1))));
}

#[test]
fn t135_reconcile_on_neighbor_edits() {
    // Move одного соседа рвёт стык — переход умирает (уводим на другую
    // дорожку: сдвиг по времени теперь честно ловил бы ClipOverlap)
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    m.apply(Command::MoveClip { id: ClipId(2), new_start: Tick(48_000), new_lane: 1 }).unwrap();
    assert!(m.project().transitions.is_empty(), "broken joint must kill the transition");
    m.undo().unwrap();
    assert_eq!(m.project().transitions.len(), 1, "undo brings it back");

    // Групповой перенос обоих соседей стык сохраняет — переход живёт
    // (клип 3 убираем: сдвиг вправо теперь честно ловил бы коллизию)
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::RemoveClip { id: ClipId(3) }).unwrap();
    m.apply(Command::LinkClips { ids: vec![ClipId(1), ClipId(2)], group: GroupId(10) }).unwrap();
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    m.apply(Command::MoveClip { id: ClipId(1), new_start: Tick(6_000), new_lane: 0 }).unwrap();
    assert_eq!(m.project().transitions.len(), 1, "group move keeps the joint");
    assert_eq!(m.project().transitions[0].joint(m.project()), Some(Tick(54_000)));

    // Трим укорачивает тело — офсет клампится; ниже минимума — умирает
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 2, 3, 9_000, 0))).unwrap();
    // конец клипа 3 не трогаем; тримим ВЕСЬ клип 2 справа нельзя (стык!),
    // поэтому клампим через укорочение клипа 2 слева: тело 2 стало 3000
    m.apply(Command::TrimStart { id: ClipId(2), new_start: Tick(93_000) }).unwrap();
    assert_eq!(m.project().transitions.len(), 1);
    assert_eq!(m.project().transitions[0].in_offset, Tick(3_000), "clamped by body");
    // тело не может стать меньше MIN_CLIP_DURATION, поэтому кламп упирается
    // в 90 и переход остаётся жив (ветка «окно < минимума» в reconcile —
    // защитная: через тримы недостижима, т.к. минимумы клипа и окна равны)
    m.apply(Command::TrimStart { id: ClipId(2), new_start: Tick(95_910) }).unwrap();
    assert_eq!(m.project().transitions.len(), 1);
    assert_eq!(m.project().transitions[0].in_offset, Tick(90), "clamped to minimal body");

    // Удаление соседа — переход умирает
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    m.apply(Command::RemoveClip { id: ClipId(2) }).unwrap();
    assert!(m.project().transitions.is_empty());
}

#[test]
fn t135_split_retargets_outgoing() {
    // Split OUTGOING-соседа: стык (его конец) уезжает к правой половине
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    m.apply(Command::Split {
        id: ClipId(1),
        at: Tick(24_000),
        new_id: ClipId(50),
        new_group: None,
    })
    .unwrap();
    assert_eq!(m.project().transitions.len(), 1, "transition survives outgoing split");
    let t = &m.project().transitions[0];
    assert_eq!((t.a, t.b), (Some(ClipId(50)), Some(ClipId(2))));
    assert_eq!(t.joint(m.project()), Some(Tick(48_000)));

    // Split INCOMING-соседа: стык (его начало) остаётся за левой половиной
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    m.apply(Command::Split {
        id: ClipId(2),
        at: Tick(72_000),
        new_id: ClipId(51),
        new_group: None,
    })
    .unwrap();
    let t = &m.project().transitions[0];
    assert_eq!((t.a, t.b), (Some(ClipId(1)), Some(ClipId(2))), "incoming split keeps ids");
    // тело incoming укоротилось до 24000 — out_offset клампнулся
    assert_eq!(t.out_offset, Tick(9_000).min(Tick(24_000)));
}

// ---- тест 155: фейды у края дорожки (Р.25) -----------------------------------

fn fade(id: u64, a: Option<u64>, b: Option<u64>, in_off: i64, out_off: i64) -> TransitionEvent {
    TransitionEvent {
        id: TransitionId(id),
        a: a.map(ClipId),
        b: b.map(ClipId),
        in_offset: Tick(in_off),
        out_offset: Tick(out_off),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    }
}

/// Переход с ОДНИМ живым концом законен: слева/справа от него пустота
/// (край дорожки или Gap). Стык — граница живого клипа, окно считается
/// от неё, офсет в сторону пустоты телом не ограничен.
#[test]
fn t155_fade_is_a_transition_with_one_side() {
    let mut p = base_project();
    // фейд-ин первого клипа и фейд-аут последнего
    p.transitions.push(fade(1, None, Some(1), 9_000, 9_000));
    p.transitions.push(fade(2, Some(3), None, 9_000, 9_000));
    p.validate().expect("фейды у краёв дорожки законны");

    let fi = &p.transitions[0];
    assert!(fi.is_fade());
    assert_eq!(fi.joint(&p), Some(Tick(0)), "стык фейд-ина — начало его клипа");
    assert_eq!(fi.window(&p).unwrap().start, Tick(-9_000), "часть окна висит в пустоте");

    let fo = &p.transitions[1];
    assert_eq!(fo.joint(&p), Some(Tick(144_000)), "стык фейд-аута — конец его клипа");

    // офсет В СТОРОНУ КЛИПА по-прежнему ограничен телом
    let mut bad = p.clone();
    bad.transitions[0].out_offset = Tick(48_001);
    assert!(bad.validate().is_err(), "фейд не может быть шире тела своего клипа");
    // а в сторону пустоты — нет
    let mut wide = p.clone();
    wide.transitions[0].in_offset = Tick(500_000);
    wide.validate().expect("в пустоту фейд тянется свободно");

    // переход без клипов вообще — ошибка
    let mut orphan = base_project();
    orphan.transitions.push(fade(9, None, None, 9_000, 9_000));
    assert!(orphan.validate().is_err());
}

/// Фейд живёт по тем же правилам, что кроссфейд: трим клампит окно, удаление
/// клипа уносит переход, копипаст переносит фейд вместе с его клипом.
#[test]
fn t155_fade_survives_edits_and_clipboard() {
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(fade(1, None, Some(1), 9_000, 9_000))).unwrap();

    // трим клипа укорачивает офсет в его сторону
    m.apply(Command::TrimEnd { id: ClipId(1), new_end: Tick(4_000) }).unwrap();
    let t = &m.project().transitions[0];
    assert_eq!(t.out_offset, Tick(4_000), "офсет в тело клампится тримом");
    assert_eq!(t.in_offset, Tick(9_000), "офсет в пустоту трим не трогает");

    // удаление клипа уносит и фейд
    m.apply(Command::RemoveClip { id: ClipId(1) }).unwrap();
    assert!(m.project().transitions.is_empty());

    // копипаст: фейд едет вместе со своим клипом
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(fade(1, None, Some(2), 9_000, 9_000))).unwrap();
    let block = kt_model::ops::copy_group(m.project(), ClipId(2));
    assert_eq!(block.transitions.len(), 1, "фейд в блоке");
    let (mut next_clip, mut next_group, mut next_tr, mut next_morph) =
        (100u64, 100u64, 100u64, 1u64);
    let mut next_cam = 1u64;
    let (cmds, _end, _) = kt_model::ops::paste_block(
        &block,
        Tick(300_000),
        kt_model::ops::PasteIds {
            clip: &mut next_clip,
            group: &mut next_group,
            transition: &mut next_tr,
            morph: &mut next_morph,
            camera: &mut next_cam,
        },
        &kt_model::ops::PasteNames::default(),
    );
    m.apply_batch(cmds).unwrap();
    let pasted = m.project().transitions.iter().find(|t| t.id == TransitionId(100)).unwrap();
    assert!(pasted.a.is_none() && pasted.b == Some(ClipId(100)), "вставленный фейд остался фейдом");
}

// ---- тест 159: возврат «ручек» при разрыве кроссфейда (Р.26) ------------------

/// Наезд режет клипы до стыка; растаскивание обязано вернуть срезанное —
/// как в Vegas, где перекрытие просто перестаёт быть перекрытием.
#[test]
fn t159_restore_handles_after_pulling_apart() {
    // готовим состояние «после наезда»: клипы встык + переход
    let mut m = TimelineModel::new(base_project());
    let before_trim = m.project().clone();
    // наезд: клип 2 съехал на 20_000 влево, стык — середина наезда
    m.apply_batch(vec![
        Command::MoveClip { id: ClipId(2), new_start: Tick(28_000), new_lane: 0 },
        Command::TrimEnd { id: ClipId(1), new_end: Tick(38_000) },
        Command::TrimStart { id: ClipId(2), new_start: Tick(38_000) },
        Command::AddTransition(tr(1, 1, 2, 10_000, 10_000)),
    ])
    .unwrap();

    // растаскиваем: клип 2 уезжает далеко вправо
    let cmds = vec![Command::MoveClip { id: ClipId(2), new_start: Tick(300_000), new_lane: 0 }];
    let after = m.preview(&cmds).unwrap();
    assert!(after.transitions.is_empty(), "разорванный стык убивает переход");
    let restore = kt_model::ops::restore_handles(m.project(), &after);
    assert_eq!(restore.len(), 2, "по команде на каждый конец: {restore:?}");

    let mut all = cmds;
    all.extend(restore);
    m.apply_batch(all).unwrap();

    let p = m.project();
    let a = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let b = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    let a0 = before_trim.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let b0 = before_trim.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert_eq!(a.duration, a0.duration, "уходящему вернули хвост");
    assert_eq!(b.duration, b0.duration, "входящему вернули голову");
    assert_eq!(b.source_in, b0.source_in, "точка входа тоже вернулась");
    assert_eq!(b.start, Tick(300_000) - Tick(10_000), "голова растёт влево от нового места");
}

/// Растащили НЕ полностью — вернётся столько, сколько влезло, и зазор
/// делится между обоими концами.
#[test]
fn t159_restore_is_clamped_by_free_space() {
    let mut m = TimelineModel::new(base_project());
    // наезд: клип 2 съехал на 20_000 влево, стык — середина наезда
    m.apply_batch(vec![
        Command::MoveClip { id: ClipId(2), new_start: Tick(28_000), new_lane: 0 },
        Command::TrimEnd { id: ClipId(1), new_end: Tick(38_000) },
        Command::TrimStart { id: ClipId(2), new_start: Tick(38_000) },
        Command::AddTransition(tr(1, 1, 2, 10_000, 10_000)),
    ])
    .unwrap();

    // зазор всего 6000 тиков против 20000 «хотелок»
    let cmds = vec![Command::MoveClip { id: ClipId(2), new_start: Tick(44_000), new_lane: 0 }];
    let after = m.preview(&cmds).unwrap();
    let mut all = cmds;
    all.extend(kt_model::ops::restore_handles(m.project(), &after));
    m.apply_batch(all).unwrap();

    let p = m.project();
    let a = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let b = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert_eq!((a.start + a.duration).0, 41_000, "уходящему досталась половина зазора");
    assert_eq!(b.start.0, 41_000, "входящему — вторая половина, клипы сомкнулись");
    p.validate().expect("перекрытия не появилось");
}

/// Удалили соседа — выживший клип получает свой кусок обратно (решение
/// пользователя Р.26 п.1).
#[test]
fn t159_restore_after_neighbour_removed() {
    let mut m = TimelineModel::new(base_project());
    // наезд: клип 2 съехал на 20_000 влево, стык — середина наезда
    m.apply_batch(vec![
        Command::MoveClip { id: ClipId(2), new_start: Tick(28_000), new_lane: 0 },
        Command::TrimEnd { id: ClipId(1), new_end: Tick(38_000) },
        Command::TrimStart { id: ClipId(2), new_start: Tick(38_000) },
        Command::AddTransition(tr(1, 1, 2, 10_000, 10_000)),
    ])
    .unwrap();

    let cmds = vec![Command::RemoveClip { id: ClipId(2) }];
    let after = m.preview(&cmds).unwrap();
    let restore = kt_model::ops::restore_handles(m.project(), &after);
    assert_eq!(restore.len(), 1, "восстанавливается только выживший конец");
    let mut all = cmds;
    all.extend(restore);
    m.apply_batch(all).unwrap();
    let a = m.project().clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    assert_eq!((a.start + a.duration), Tick(48_000), "хвост вернулся целиком");
}

/// Фейду возвращать нечего: он заводится явной командой, ничего не срезая.
#[test]
fn t159_fade_has_nothing_to_restore() {
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(fade(7, None, Some(2), 9_000, 9_000))).unwrap();
    let cmds = vec![Command::RemoveClip { id: ClipId(2) }];
    let after = m.preview(&cmds).unwrap();
    assert!(kt_model::ops::restore_handles(m.project(), &after).is_empty());
}

/// Возврат упирается в препятствия дорожки — включая чужие плейсхолдеры
/// (инвариант «без перекрытий» считает и их).
#[test]
fn t159_restore_stops_at_obstacles() {
    // состояние «после наезда» собираем сразу: клипы встык + переход,
    // а в четырёх тысячах тиков за стыком стоит чужая конструкция
    let mut p = base_project();
    p.clips.retain(|c| c.id != ClipId(3));
    let (a, b) = (Tick(0), Tick(38_000));
    for c in &mut p.clips {
        if c.id == ClipId(1) {
            (c.start, c.duration) = (a, b - a);
        } else {
            (c.start, c.duration) = (b, Tick(38_000));
        }
    }
    p.foreign.push(ForeignEvent {
        key: 1,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(80_000),
        duration: Tick(4_000),
    });
    p.transitions.push(tr(1, 1, 2, 10_000, 10_000));
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);

    // клип 2 уезжает за плейсхолдер; клип 1 хочет вернуть 10_000, но
    // упирается в него уже через 4_000 — впрочем, места до плейсхолдера
    // хватает: проверяем, что рост остановился ровно на препятствии
    let cmds = vec![Command::MoveClip { id: ClipId(2), new_start: Tick(90_000), new_lane: 0 }];
    let after = m.preview(&cmds).unwrap();
    let mut all = cmds;
    all.extend(kt_model::ops::restore_handles(m.project(), &after));
    m.apply_batch(all).unwrap();

    let p = m.project();
    p.validate().expect("итог валиден");
    let c2 = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert_eq!(c2.start, Tick(84_000), "голова выросла только до плейсхолдера");
    let c1 = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    assert_eq!(c1.start + c1.duration, Tick(48_000), "хвост вернулся целиком");
}

/// Возврат не имеет права уронить сам жест: у reversed-клипа TrimEnd
/// ограничен source_in, а у обычного TrimStart — нулём материала. Без
/// клампа check_on вернёт TooShort и откатит ВЕСЬ батч (перенос вместе с
/// подарком) — поэтому кламп делает сам restore_handles.
#[test]
fn t159_restore_never_breaks_the_gesture() {
    for reversed in [false, true] {
        let mut p = base_project();
        p.clips.retain(|c| c.id != ClipId(3));
        for c in &mut p.clips {
            c.reversed = reversed;
            // материала за краем почти нет: вернуть можно лишь 2000 тиков
            c.source_in = Tick(2_000);
        }
        // состояние «после наезда»: встык + переход
        p.clips[0].duration = Tick(38_000);
        p.clips[1].start = Tick(38_000);
        p.clips[1].duration = Tick(38_000);
        p.transitions.push(tr(1, 1, 2, 10_000, 10_000));
        p.validate().unwrap();
        let mut m = TimelineModel::new(p);

        let cmds = vec![Command::MoveClip { id: ClipId(2), new_start: Tick(300_000), new_lane: 0 }];
        let after = m.preview(&cmds).unwrap();
        let mut all = cmds;
        all.extend(kt_model::ops::restore_handles(m.project(), &after));
        m.apply_batch(all).unwrap_or_else(|e| panic!("reversed={reversed}: жест провалился: {e}"));
        let p = m.project();
        p.validate().unwrap();
        assert_eq!(
            p.clips.iter().find(|c| c.id == ClipId(2)).unwrap().start,
            Tick(300_000 - if reversed { 10_000 } else { 2_000 }),
            "reversed={reversed}: вернулось столько, сколько есть материала"
        );
    }
}

/// Р.33 С4: у каждого вида перехода — полная спека параметров (та же, что
/// у эффектов), значения ставятся/читаются/клампятся, теги уникальны, а
/// умолчания конструктора совпадают с умолчаниями спеки. Без этого UI
/// перехода пришлось бы писать руками — ровно тот разнобой, на который
/// жаловался пользователь.
#[test]
fn t223_transition_kind_specs() {
    let all = TransitionKind::ALL;
    assert_eq!(
        all.len(),
        33,
        "33 вида: 16 прежних + 17 Р.51 (якорь «16» снят решением владельца, docs/user-feedback.md Р.51)"
    );
    let mut tags: Vec<&str> = all.iter().map(|k| k.tag()).collect();
    tags.sort_unstable();
    let uniq = tags.len();
    tags.dedup();
    assert_eq!(tags.len(), uniq, "теги видов уникальны: {tags:?}");

    let mut with_params = 0;
    for k in TransitionKind::ALL {
        let mut k = k.clone();
        for spec in k.param_specs() {
            with_params += 1;
            let (name, lo, hi) = (spec.name.clone(), spec.min, spec.max);
            assert!(
                (lo..=hi).contains(&spec.default),
                "{}.{name}: дефолт {} вне [{lo}, {hi}]",
                k.tag(),
                spec.default
            );
            assert!(spec.step > 0.0, "{}.{name}: шаг обязан быть положительным", k.tag());
            // умолчание вида == умолчание спеки: иначе «сбросить» в UI
            // даёт не то, что даёт создание перехода
            let fresh = TransitionKind::ALL.iter().find(|x| x.tag() == k.tag()).unwrap().clone();
            let got = fresh.get_param(&name).unwrap();
            assert!(
                (got - spec.default).abs() <= 1e-9,
                "{}.{name}: конструктор даёт {got}, спека — {}",
                k.tag(),
                spec.default
            );
            let mid = (lo + hi) / 2.0;
            assert!(k.set_param(&name, mid), "{}.{name} не установился", k.tag());
            let got = k.get_param(&name).unwrap();
            assert!((got - mid).abs() <= 0.5, "{}.{name}: {got} vs {mid}", k.tag());
            assert!(k.set_param(&name, hi + 1e6));
            assert!(k.get_param(&name).unwrap() <= hi + 1e-9, "{}.{name} не клампится", k.tag());
            assert!(k.set_param(&name, lo - 1e6));
            assert!(k.get_param(&name).unwrap() >= lo - 1e-9);
        }
        assert!(!k.set_param("no_such_param", 1.0), "{}: чужое имя — отказ", k.tag());
    }
    assert!(with_params >= 20, "параметров у видов: {with_params}");
    assert!(
        TransitionKind::Crossfade.param_specs().is_empty(),
        "у кроссфейда числовых параметров нет — он остаётся прежним"
    );

    // кривые аудио: монотонны, края точные, сумма мощностей не проваливается
    for c in
        [AudioCurve::EqualPower, AudioCurve::Linear, AudioCurve::SCurve, AudioCurve::Exponential]
    {
        assert!((c.rise(0.0)).abs() < 1e-12, "{c:?}: старт с нуля");
        assert!((c.rise(1.0) - 1.0).abs() < 1e-12, "{c:?}: конец в единице");
        let mut prev = -1.0;
        for i in 0..=100 {
            let v = c.rise(i as f64 / 100.0);
            assert!(v >= prev - 1e-12, "{c:?}: не монотонна в {i}");
            prev = v;
        }
    }
    // характер кривых по сумме МОЩНОСТЕЙ в середине стыка (обе половины
    // считаются одной формулой, поэтому вклад каждой — rise(0.5)^2)
    let mid = |c: AudioCurve| 2.0 * c.rise(0.5) * c.rise(0.5);
    assert!((mid(AudioCurve::EqualPower) - 1.0).abs() < 1e-12, "equal-power держит мощность");
    assert!((mid(AudioCurve::SCurve) - 1.0).abs() < 1e-12, "S-кривая тоже держит мощность");
    assert!((mid(AudioCurve::Linear) - 0.5).abs() < 1e-12, "линейная даёт классический −3 dB");
    assert!(mid(AudioCurve::Exponential) < 0.5, "экспонента — намеренно глубокий провал");
    assert_eq!(AudioCurve::default(), AudioCurve::EqualPower, "умолчание — прежняя формула");
}

/// Тест 462 (п.399, Р.51): переход на НАЛОЖЕНИИ — пара «лесенкой», окно =
/// зона перекрытия, режим не хранится. Валидация, судьба при правках
/// (reconcile), возврат ручек и копипаст.
#[test]
fn t462_overlap_transition_model() {
    // клипы 1 и 2 внахлёст «лесенкой»: [0..60к) и [30к..90к)
    let overlap_project = || {
        let mut p = base_project();
        p.clips.retain(|c| c.id.0 <= 2);
        p.clips[0].duration = Tick(60_000);
        p.clips[1].start = Tick(30_000);
        p.clips[1].duration = Tick(60_000);
        p.validate().unwrap();
        p
    };
    let ov = |id: u64, a: u64, b: u64| TransitionEvent {
        id: TransitionId(id),
        a: Some(ClipId(a)),
        b: Some(ClipId(b)),
        in_offset: Tick::ZERO,
        out_offset: Tick::ZERO,
        kind: TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.1 },
        audio: AudioCurve::Linear,
        auto: vec![],
        invert: false,
    };

    // 1. валидация: лесенка законна, окно — сама зона перекрытия
    let mut p = overlap_project();
    p.transitions.push(ov(1, 1, 2));
    p.validate().expect("наложенный переход законен");
    let w = p.transitions[0].window(&p).unwrap();
    assert_eq!((w.start, w.end), (Tick(30_000), Tick(60_000)), "окно = зона перекрытия");
    assert!(p.transitions[0].overlap_pair(&p).is_some());

    // 2. обратный порядок ролей — отказ (канонизацию делает reconcile)
    let mut p = overlap_project();
    p.transitions.push(ov(1, 2, 1));
    assert!(p.validate().is_err(), "перевёрнутая пара — не лесенка");

    // 3. полное вложение — отказ
    let mut p = overlap_project();
    p.clips[1].start = Tick(10_000);
    p.clips[1].duration = Tick(20_000); // клип 2 целиком внутри клипа 1
    p.transitions.push(ov(1, 1, 2));
    assert!(p.validate().is_err(), "вложение — не лесенка");

    // 4. третий клип в окне — отказ с именем виновника
    let mut p = overlap_project();
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(3),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(40_000),
        duration: Tick(10_000),
        source_in: Tick::ZERO,
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.transitions.push(ov(1, 1, 2));
    match p.validate() {
        Err(ProjectError::TransitionOverlapBusy(_, c)) => assert_eq!(c, ClipId(3)),
        other => panic!("ожидался TransitionOverlapBusy, получено {other:?}"),
    }

    // 5. reconcile: стыковой переход выживает НАЛОЖЕННЫМ при наезде пары
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    m.project().transitions[0].window(m.project()).unwrap();
    // наезд: клип 2 съехал влево на 18к — теперь лесенка
    m.apply(Command::MoveClip { id: ClipId(2), new_start: Tick(30_000), new_lane: 0 }).unwrap();
    let p = m.project();
    assert_eq!(p.transitions.len(), 1, "переход пережил наезд наложением");
    let t = &p.transitions[0];
    assert!(t.overlap_pair(p).is_some(), "теперь это наложенный переход");
    assert_eq!(
        t.kind,
        TransitionKind::Crossfade,
        "вид не тронут (см. t460 про сохранение настроек)"
    );
    let w = t.window(p).unwrap();
    assert_eq!((w.start, w.end), (Tick(30_000), Tick(48_000)), "окно = новое перекрытие");
    // офсеты — мёртвый груз, не тронуты
    assert_eq!((t.in_offset, t.out_offset), (Tick(9_000), Tick(9_000)));

    // ...и возвращение встык оживляет прежние офсеты
    m.apply(Command::MoveClip { id: ClipId(2), new_start: Tick(48_000), new_lane: 0 }).unwrap();
    let p = m.project();
    assert_eq!(p.transitions.len(), 1);
    assert!(p.transitions[0].overlap_pair(p).is_none(), "пара снова встык");
    let w = p.transitions[0].window(p).unwrap();
    assert_eq!((w.start, w.end), (Tick(39_000), Tick(57_000)), "окно снова от офсетов");

    // 6. reconcile: разрыв (растащили далеко) убивает наложенный переход
    let mut m = TimelineModel::new(overlap_project());
    m.apply(Command::AddTransition(ov(1, 1, 2))).unwrap();
    m.apply(Command::MoveClip { id: ClipId(2), new_start: Tick(90_000), new_lane: 0 }).unwrap();
    assert!(m.project().transitions.is_empty(), "перекрытие исчезло — переход умер");

    // 7. reconcile: канонизация ролей — клип b уехал ЛЕВЕЕ a, роли
    // зеркалятся, переход жив
    let mut m = TimelineModel::new(overlap_project());
    m.apply(Command::AddTransition(ov(1, 1, 2))).unwrap();
    // клип 2 (был поздним) уезжает в 0, клип 1 начинается там же... нет:
    // сдвинем клип 1 вправо, чтобы прежний b стал ранним
    m.apply(Command::MoveClip { id: ClipId(1), new_start: Tick(45_000), new_lane: 0 }).unwrap();
    let p = m.project();
    assert_eq!(p.transitions.len(), 1, "переход жив после смены порядка");
    let t = &p.transitions[0];
    assert_eq!((t.a, t.b), (Some(ClipId(2)), Some(ClipId(1))), "роли канонизированы: a ранний");
    assert!(t.overlap_pair(p).is_some());

    // 8. restore_handles: разрыв наложенного НЕ дарит тримы (ничего не
    // срезалось при наезде)
    let before = {
        let mut p = overlap_project();
        p.transitions.push(ov(1, 1, 2));
        p.validate().unwrap();
        p
    };
    let after = {
        let mut p = before.clone();
        p.transitions.clear();
        p.clips[1].start = Tick(90_000); // растащили
        p.validate().unwrap();
        p
    };
    assert!(
        ops::restore_handles(&before, &after).is_empty(),
        "у наложенного перехода нет срезанных ручек"
    );

    // 9. копипаст: наложенный переход выживает в блоке целиком
    let p = {
        let mut p = overlap_project();
        p.transitions.push(ov(1, 1, 2));
        p.validate().unwrap();
        p
    };
    let block = ops::copy_clips(&p, &[ClipId(1), ClipId(2)]);
    assert_eq!(block.transitions.len(), 1, "наложенный переход скопирован");
    let block1 = ops::copy_clips(&p, &[ClipId(1)]);
    assert!(block1.transitions.is_empty(), "без пары переход не едет");
}
