//! Р.62, change B: дорожка — объект модели со стабильным id.

use kt_model::HasParams;
use kt_model::LaneKind::{Audio as A, Morph as M, Video as V};
use kt_model::*;
use kt_time::Tick;

fn settings30() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

fn vclip(id: u64, lane: u32, start: i64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane,
        start: Tick(start),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn project() -> Project {
    let mut p = Project::new(settings30());
    p.media.push(MediaRef { id: MediaId(1), path: "synth:sine:440".into() });
    p.ensure_tracks(V, 2);
    p.clips.push(vclip(1, 0, 0));
    p.clips.push(vclip(2, 1, 100_000));
    p.validate().unwrap();
    p
}

/// Тест 729 (Р.62): id дорожки стабилен при вставке сверху, перестановке
/// и удалении соседа; имя и высота — содержимое дорожки и ходят через
/// undo; свойства едут с объектом, а не с номером.
#[test]
fn t729_track_id_is_stable_and_name_height_are_undoable() {
    let mut m = TimelineModel::new(project());
    let bottom = m.project().track_at(V, 1).unwrap().id;
    assert_ne!(bottom.0, 0, "id назначен");

    // имя и высота — ОДНА команда свойств, и она откатывается
    let mut props = m.project().track(bottom).unwrap().props();
    props.name = "Титры".into();
    props.height = 48.0;
    m.apply(Command::SetTrackProps { id: bottom, props }).unwrap();
    assert_eq!(m.project().track_label(V, 1), "Титры");
    assert_eq!(m.project().track(bottom).unwrap().height, 48.0);
    m.undo().unwrap();
    assert_eq!(m.project().track_label(V, 1), "V2", "имя не откатилось");
    assert_eq!(m.project().track(bottom).unwrap().height, 0.0, "высота не откатилась");
    m.redo().unwrap();

    // вставка дорожки СВЕРХУ: номер вырос, id тот же, свойства при нём
    m.apply(Command::InsertTrack { kind: V, at: 0 }).unwrap();
    let p = m.project();
    assert_eq!(p.lanes(V), 3);
    assert_eq!(p.track_at(V, 2).unwrap().id, bottom, "id уехал при вставке соседа");
    assert_eq!(p.track_label(V, 2), "Титры");
    assert_eq!(p.clips.iter().find(|c| c.id == ClipId(2)).unwrap().lane, 2);
    let inserted = p.track_at(V, 0).unwrap().id;
    assert!(inserted != bottom && inserted.0 != 0, "новая дорожка получила свой id");

    // перестановка: нижняя наверх — id и свойства едут с объектом
    m.apply(Command::SetLaneOrder { order: vec![(V, 2), (V, 0), (V, 1), (A, 0)] }).unwrap();
    let p = m.project();
    assert_eq!(p.track_at(V, 0).unwrap().id, bottom);
    assert_eq!(p.track_label(V, 0), "Титры");
    assert_eq!(p.clips.iter().find(|c| c.id == ClipId(2)).unwrap().lane, 0);

    // удаление соседа: id остаётся, содержимое соседа уходит. После
    // перестановки строки идут «Титры» (бывшая нижняя), пустая вставленная,
    // дорожка события 1 — значит содержимое уносит удаление РАНГА 2
    m.apply(Command::RemoveTrack { kind: V, at: 2 }).unwrap();
    let p = m.project();
    assert_eq!(p.lanes(V), 2);
    assert_eq!(p.track_at(V, 0).unwrap().id, bottom);
    assert!(p.clips.iter().all(|c| c.id != ClipId(1)), "события удалённой дорожки остались");
    m.undo().unwrap();
    assert_eq!(m.project().lanes(V), 3, "undo не вернул дорожку");
    assert!(m.project().clips.iter().any(|c| c.id == ClipId(1)), "undo не вернул события");

    // последнюю дорожку вида убрать нельзя; служебные — можно все
    assert!(matches!(
        m.apply(Command::RemoveTrack { kind: A, at: 0 }),
        Err(CommandError::LastTrack(A))
    ));
    m.apply(Command::InsertTrack { kind: M, at: 0 }).unwrap();
    m.apply(Command::RemoveTrack { kind: M, at: 0 }).unwrap();
    assert_eq!(m.project().lanes(M), 0);

    // id уникальны по документу, включая ленты
    let mut p = m.project().clone();
    p.sequences.push(Sequence::new(MediaId(100), "Кусок"));
    p.assign_track_ids();
    let ids: Vec<TrackId> =
        p.tracks.iter().chain(p.sequences[0].tracks.iter()).map(|t| t.id).collect();
    let uniq: std::collections::HashSet<TrackId> = ids.iter().copied().collect();
    assert_eq!(ids.len(), uniq.len());
}

/// Тест 730 (Р.62): паспорт свойств дорожки. Пределы и типы — оттуда, а
/// не из литералов интерфейса; уровень объявлен ПО ВИДУ (дБ у аудио,
/// непрозрачность у видео), у служебных строк из выключателей только
/// замок. Кривой на свойстве дорожки нет и быть не может: её ключ — тик
/// таймлайна, а это второй домен автоматизации (следующий change),
/// поэтому ряды структурные и `auto_params()` пуст.
#[test]
fn t730_track_property_passport_and_no_curves() {
    let p = project();
    let audio = p.track_at(A, 0).unwrap();
    let video = p.track_at(V, 0).unwrap();

    // уровень — по виду; чужого ряда у дорожки нет (мёртвых контролов не бывает)
    let spec = audio.param_spec("gain_db").expect("у аудиодорожки уровень в дБ");
    assert_eq!((spec.min, spec.max, spec.default), (TRACK_GAIN_MIN_DB, TRACK_GAIN_MAX_DB, 0.0));
    assert_eq!(spec.unit, ParamUnit::Db);
    assert!(audio.param_spec("opacity").is_none(), "у аудио непрозрачности нет");
    let spec = video.param_spec("opacity").expect("у видеодорожки непрозрачность");
    assert_eq!((spec.min, spec.max, spec.default), (0.0, 1.0, 1.0));
    assert!(video.param_spec("gain_db").is_none(), "у видео уровня в дБ нет");

    // кламп идёт ЧЕРЕЗ паспорт
    let mut t = audio.clone();
    assert!(t.set_param("gain_db", 999.0));
    assert_eq!(t.gain_db, TRACK_GAIN_MAX_DB, "уровень не зажат пределом паспорта");
    let mut t = video.clone();
    assert!(t.set_param("opacity", -5.0));
    assert_eq!(t.opacity, 0.0);

    // выключатели: у клипных строк три, у служебных только замок
    let names = |t: &Track| t.flag_specs().iter().map(|f| f.name).collect::<Vec<_>>();
    assert_eq!(names(audio), vec!["mute", "solo", "lock"]);
    let mut q = project();
    q.ensure_tracks(M, 1);
    let morph = q.track_at(M, 0).unwrap();
    assert_eq!(names(morph), vec!["lock"], "у морф-строки нет mute и solo");
    assert_eq!(morph.get_flag("mute"), None);

    // режим наложения — типизированный выбор У ВИДЕО, ключом варианта
    assert_eq!(video.get_value("blend"), Some(ParamValue::Choice("normal".into())));
    let mut t = video.clone();
    assert!(t.set_value("blend", &ParamValue::Choice("screen".into())));
    assert_eq!(t.blend, BlendMode::Screen);
    assert!(!t.set_value("blend", &ParamValue::Choice("нет такого".into())), "чужой ключ не молча");
    assert_eq!(audio.value_type("blend"), None, "у аудиодорожки режима наложения нет");

    // КРИВЫХ У ДОРОЖКИ НЕТ: ни одного анимируемого ряда
    for t in [audio, video] {
        assert!(
            t.param_specs().iter().all(|s| s.kind != ParamKind::Smooth),
            "домен времени дорожки — следующая работа: анимируемых рядов быть не должно"
        );
    }
    // и модель отвергает их громко, если они всё же появились в файле
    let mut bad = project();
    bad.track_at_mut(A, 0).unwrap().fx_auto = vec![vec![ParamCurve {
        param: "gain_db".into(),
        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(90_000), 1.0)]),
        source: Default::default(),
    }]];
    assert!(
        matches!(bad.validate(), Err(ProjectError::TrackCurvesUnsupported(_))),
        "кривая на дорожке обязана валить загрузку с причиной: {:?}",
        bad.validate()
    );
}

/// Тест 738 (Р.62): ЗАМОК — один гейт на весь реестр команд. Каждая
/// команда либо называет строки, которые правит (`target_rows`), либо
/// объявлена «содержимого строк не трогает»; сам match исчерпывающий,
/// поэтому новая команда не соберётся, пока её не отнесли к половине.
/// Здесь проверяется СЛЕДСТВИЕ: жест по запертой дорожке отвергается с
/// причиной, а свойства самой дорожки правятся.
#[test]
fn t738_locked_track_refuses_edits_but_stays_editable_itself() {
    let mut m = TimelineModel::new(project());
    let locked = m.project().track_at(V, 0).unwrap().id;
    let mut props = m.project().track(locked).unwrap().props();
    props.lock = true;
    m.apply(Command::SetTrackProps { id: locked, props }).unwrap();

    // жесты по СОБЫТИЯМ запертой дорожки — отказ с причиной
    let moved = Command::MoveClip { id: ClipId(1), new_start: Tick(5_000), new_lane: 0 };
    for cmd in [
        moved.clone(),
        Command::RemoveClip { id: ClipId(1) },
        Command::TrimStart { id: ClipId(1), new_start: Tick(1_000) },
        Command::Split { id: ClipId(1), at: Tick(45_000), new_id: ClipId(99), new_group: None },
        Command::SetOpacity { id: ClipId(1), opacity: 0.5 },
        Command::SetFx { id: ClipId(1), fx: vec![] },
    ] {
        assert!(
            matches!(m.apply(cmd.clone()), Err(CommandError::TrackLocked(t)) if t == locked),
            "жест по запертой дорожке обязан быть отказом: {cmd:?}"
        );
    }
    // модель не тронута
    assert_eq!(m.project().clips.iter().find(|c| c.id == ClipId(1)).unwrap().start, Tick(0));

    // ПЕРЕЕЗД НА запертую дорожку — тоже отказ (замок про строку, а не
    // про событие): событие с соседней дорожки внутрь не заезжает
    assert!(matches!(
        m.apply(Command::MoveClip { id: ClipId(2), new_start: Tick(0), new_lane: 0 }),
        Err(CommandError::TrackLocked(_))
    ));

    // удаление запертой дорожки уносит её события — тоже отказ
    assert!(matches!(
        m.apply(Command::RemoveTrack { kind: V, at: 0 }),
        Err(CommandError::TrackLocked(_))
    ));

    // а СВОЙСТВА самой дорожки правятся — иначе её нечем отпереть
    let mut props = m.project().track(locked).unwrap().props();
    props.mute = true;
    m.apply(Command::SetTrackProps { id: locked, props }).expect("mute запертой дорожки");
    assert!(m.project().track(locked).unwrap().mute);
    let mut props = m.project().track(locked).unwrap().props();
    props.lock = false;
    m.apply(Command::SetTrackProps { id: locked, props }).expect("замок снимается");
    m.apply(moved).expect("после снятия замка жест проходит");

    // соседняя НЕзапертая дорожка всё это время работает
    let free = m.project().track_at(V, 1).map(|t| t.id);
    assert!(free.is_some());
    m.apply(Command::MoveClip { id: ClipId(2), new_start: Tick(7_000), new_lane: 1 })
        .expect("соседняя дорожка не заперта");
}
