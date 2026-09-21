//! Тест 35 (docs/porting-notes.md): morpher-система в модели.
//! Порт v1 с исправлением дефектов разведки: D1 (FX batch live),
//! D5 (чистка/наследование targets), D6 (undo на всё), D11 (порядок по id).

use kt_model::*;
use kt_time::Tick;

fn base_project() -> TimelineModel {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:sine:440".into() });
    p.media.push(MediaRef { id: MediaId(2), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        id: ClipId(10),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(9_000),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::gain(-3.0)],
        fx_bypass: Vec::new(),
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        id: ClipId(11),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    TimelineModel::new(p)
}

fn replace_morpher(id: u64, targets: Vec<ClipId>) -> MorpherEvent {
    MorpherEvent {
        id: MorphId(id),
        lane: 0,
        start: Tick(0),
        duration: Tick(90 * 1500),
        targets: MorphTargets::Clips { ids: targets },
        patch: MorphPatch {
            media: Some(MediaId(2)),
            source_in_delta: Tick(4_500),
            absolute_source_in: None,
            ..Default::default()
        },
        timed: false,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    }
}

/// Replace: подмена источника + сдвиг точки входа только у целей;
/// сами клипы проекта не тронуты (flatten — копия).
#[test]
fn t35_replace_flatten() {
    let mut m = base_project();
    m.apply(Command::AddMorpher(replace_morpher(1, vec![ClipId(10)]))).unwrap();

    let eff = m.project().effective_clips();
    let c10 = eff.iter().find(|c| c.id == ClipId(10)).unwrap();
    assert_eq!(c10.media, MediaId(2), "источник подменён");
    assert_eq!(c10.source_in, Tick(13_500), "source_in сдвинут на delta");
    let c11 = eff.iter().find(|c| c.id == ClipId(11)).unwrap();
    assert_eq!(c11.media, MediaId(1), "не-цель не тронута");
    // исходные клипы проекта не изменены
    assert_eq!(m.project().clips[0].media, MediaId(1));

    // absolute приоритетнее delta (D4: поле полноценно)
    let mut abs = replace_morpher(1, vec![ClipId(10)]);
    abs.patch.absolute_source_in = Some(Tick(1_000));
    m.apply(Command::SetMorpher(abs)).unwrap();
    let eff = m.project().effective_clips();
    assert_eq!(eff.iter().find(|c| c.id == ClipId(10)).unwrap().source_in, Tick(1_000));

    // отрицательный итог клампится в 0
    let mut neg = replace_morpher(1, vec![ClipId(10)]);
    neg.patch.source_in_delta = Tick(-90_000);
    m.apply(Command::SetMorpher(neg)).unwrap();
    let eff = m.project().effective_clips();
    assert_eq!(eff.iter().find(|c| c.id == ClipId(10)).unwrap().source_in, Tick(0));
}

/// FX batch действует LIVE (исправление D1): effective дописывает цепочку
/// нужного типа (video-FX — только видео-клипам, audio — только аудио).
#[test]
fn t35_effect_batch_live_and_typed() {
    let mut m = base_project();
    m.apply(Command::AddMorpher(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(0),
        duration: Tick(90 * 1500),
        targets: MorphTargets::Clips { ids: vec![ClipId(10), ClipId(11)] },
        patch: MorphPatch {
            video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
            audio_fx: vec![FxInstance::gain(6.0)],
            ..Default::default()
        },
        timed: false,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    }))
    .unwrap();

    let eff = m.project().effective_clips();
    let a = eff.iter().find(|c| c.id == ClipId(10)).unwrap();
    assert_eq!(a.fx.len(), 2, "аудио-клип: исходный Gain + дописанный");
    assert!(matches!(a.fx[1], FxInstance::Gain { db, .. } if db == 6.0));
    let v = eff.iter().find(|c| c.id == ClipId(11)).unwrap();
    assert_eq!(v.fx.len(), 1);
    assert!(matches!(v.fx[0], FxInstance::Mirror { .. }));
    // дописанные слоты активны (fx_bypass короче цепочки)
    assert_eq!(a.active_fx().len(), 2);
}

/// Bake == flatten: рендер-вид до bake и содержимое клипов после bake
/// совпадают; morpher исчезает; undo возвращает всё (D6).
#[test]
fn t35_bake_equals_flatten_and_undo() {
    let mut m = base_project();
    m.apply(Command::AddMorpher(replace_morpher(1, vec![ClipId(10)]))).unwrap();
    let before = m.project().effective_clips();

    m.apply(Command::BakeMorpher { id: MorphId(1) }).unwrap();
    assert!(m.project().morphers.is_empty(), "bake удаляет событие");
    assert_eq!(m.project().clips, before, "bake вписывает ровно то, что рендерилось");
    assert_eq!(m.project().effective_clips(), before, "рендер не изменился");

    m.undo().unwrap();
    assert_eq!(m.project().morphers.len(), 1, "undo возвращает morpher");
    assert_eq!(m.project().clips[0].media, MediaId(1), "и исходные клипы");
}

/// Порядок применения детерминирован по id (D11): два Replace на один клип —
/// побеждает поздний (кумулятивно), независимо от порядка в векторе.
#[test]
fn t35_deterministic_order_by_id() {
    let mut m = base_project();
    let mut late = replace_morpher(7, vec![ClipId(10)]);
    late.patch.media = None;
    late.patch.absolute_source_in = Some(Tick(777));
    // добавляем ПОЗДНИЙ id первым — порядок вектора обратный
    m.apply(Command::AddMorpher(late)).unwrap();
    m.apply(Command::AddMorpher(replace_morpher(3, vec![ClipId(10)]))).unwrap();

    let eff = m.project().effective_clips();
    let c = eff.iter().find(|c| c.id == ClipId(10)).unwrap();
    assert_eq!(c.media, MediaId(2), "morpher id=3 подменил источник");
    assert_eq!(c.source_in, Tick(777), "morpher id=7 применён ПОСЛЕ id=3");
}

/// D5: удаление клипа вычищает его из targets; split добавляет правую
/// половину; события с опустевшими целями остаются (как v1), но безвредны.
#[test]
fn t35_targets_follow_clip_edits() {
    let mut m = base_project();
    m.apply(Command::AddMorpher(replace_morpher(1, vec![ClipId(10), ClipId(11)]))).unwrap();

    m.apply(Command::Split {
        id: ClipId(10),
        at: Tick(45_000),
        new_id: ClipId(12),
        new_group: None,
    })
    .unwrap();
    assert!(
        m.project().morphers[0].targets.clip_ids().contains(&ClipId(12)),
        "правая половина унаследовала таргетинг"
    );
    let eff = m.project().effective_clips();
    assert_eq!(eff.iter().find(|c| c.id == ClipId(12)).unwrap().media, MediaId(2));

    m.apply(Command::RemoveClip { id: ClipId(10) }).unwrap();
    assert!(!m.project().morphers[0].targets.clip_ids().contains(&ClipId(10)), "удалённый вычищен");
    assert!(m.project().validate().is_ok(), "модель валидна после правок");
}

/// Дорожки: вставка сдвигает lane, удаление убирает события дорожки;
/// round-trip save→load сохраняет morpher'ы байт-в-байт.
#[test]
fn t35_lanes_and_roundtrip() {
    let mut m = base_project();
    let mut ev = replace_morpher(1, vec![ClipId(10)]);
    ev.lane = 0;
    m.apply(Command::AddMorpher(ev)).unwrap();
    assert_eq!(m.project().morph_lanes, 1);

    m.apply(Command::InsertMorphLane { at: 0 }).unwrap();
    assert_eq!(m.project().morphers[0].lane, 1, "вставка сдвинула событие");

    let json = serde_json::to_string(m.project()).unwrap();
    let loaded = Project::from_json(&json).unwrap();
    assert_eq!(&loaded, m.project(), "round-trip");

    m.apply(Command::RemoveMorphLane { at: 1 }).unwrap();
    assert!(m.project().morphers.is_empty(), "события дорожки удалены");
    assert_eq!(m.project().morph_lanes, 1);

    // валидация: morpher не может ссылаться на отсутствующий клип
    let mut bad = m.project().clone();
    bad.morphers.push(replace_morpher(9, vec![ClipId(999)]));
    assert!(matches!(bad.validate(), Err(ProjectError::MorphUnknownTarget(..))));
}
