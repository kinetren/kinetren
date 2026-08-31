//! Тест 135 (п.135 porting-notes, Р.24): переходы в модели.
//!
//! Кроссфейд — явный TransitionEvent на стыке двух соседних клипов (OTIO),
//! а не перекрытие. Здесь: команды/undo, инварианты валидации, судьба
//! перехода при правках соседей (reconcile), ретаргет при Split.

use kt_model::*;
use kt_time::Tick;

fn base_project() -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:Ramp".into() });
    for (id, start) in [(1u64, 0i64), (2, 48_000), (3, 96_000)] {
        p.clips.push(ClipEvent {
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
        });
    }
    p.validate().unwrap();
    p
}

fn tr(id: u64, a: u64, b: u64, in_off: i64, out_off: i64) -> TransitionEvent {
    TransitionEvent {
        id: TransitionId(id),
        a: ClipId(a),
        b: ClipId(b),
        in_offset: Tick(in_off),
        out_offset: Tick(out_off),
        kind: TransitionKind::Crossfade,
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

    // Инвариант этапа 6 (реш. 8 Р.24, замена смыслу снесённого t2-1/N):
    // перекрытие клипов одной дорожки БОЛЬШЕ НЕ ЗАКОННО — и в validate(),
    // и через жёсткую проверку apply_batch
    let mut p = base_project();
    p.clips[1].start = Tick(40_000); // наезд на клип 1
    assert!(matches!(p.validate(), Err(ProjectError::ClipOverlap(ClipId(1), ClipId(2)))));
    let mut m = TimelineModel::new(base_project());
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
    m.apply(Command::Split { id: ClipId(1), at: Tick(24_000), new_id: ClipId(50), new_group: None })
        .unwrap();
    assert_eq!(m.project().transitions.len(), 1, "transition survives outgoing split");
    let t = &m.project().transitions[0];
    assert_eq!((t.a, t.b), (ClipId(50), ClipId(2)));
    assert_eq!(t.joint(m.project()), Some(Tick(48_000)));

    // Split INCOMING-соседа: стык (его начало) остаётся за левой половиной
    let mut m = TimelineModel::new(base_project());
    m.apply(Command::AddTransition(tr(1, 1, 2, 9_000, 9_000))).unwrap();
    m.apply(Command::Split { id: ClipId(2), at: Tick(72_000), new_id: ClipId(51), new_group: None })
        .unwrap();
    let t = &m.project().transitions[0];
    assert_eq!((t.a, t.b), (ClipId(1), ClipId(2)), "incoming split keeps ids");
    // тело incoming укоротилось до 24000 — out_offset клампнулся
    assert_eq!(t.out_offset, Tick(9_000).min(Tick(24_000)));
}

