//! Тест 49 (docs/porting-notes.md, п.6 v1.4): display-порядок дорожек.
//! С Р.62 (change B) строки — ВЕКТОР ДОРОЖЕК-ОБЪЕКТОВ: `normalize_lane_order`
//! осталась чистой функцией нормализации заказа перестановки (дубли/мусор
//! вычищаются, недостающие дописываются), `lane_order_insert/remove` —
//! shift-aware синхронизация заказанного порядка. Старые файлы со
//! счётчиками и `lane_order` читаются в дорожки односторонне.

use kt_model::LaneKind::{Audio as A, Camera as C, Morph as M, Video as V};
use kt_model::*;

#[test]
fn t49_normalize_lane_order() {
    // пусто → блочный дефолт
    assert_eq!(normalize_lane_order(&[], 2, 2, 1, 0), vec![(V, 0), (V, 1), (A, 0), (A, 1), (M, 0)]);
    // произвольный валидный порядок сохраняется
    let mixed = vec![(V, 0), (A, 0), (V, 1), (M, 0), (A, 1)];
    assert_eq!(normalize_lane_order(&mixed, 2, 2, 1, 0), mixed);
    // дубли и несуществующие вычищаются, недостающие дописываются в хвост
    let dirty = vec![(V, 0), (V, 0), (A, 7), (M, 0), (V, 1)];
    assert_eq!(normalize_lane_order(&dirty, 2, 1, 1, 0), vec![(V, 0), (M, 0), (V, 1), (A, 0)]);
    // сжатие числа дорожек выбрасывает хвостовые
    assert_eq!(normalize_lane_order(&mixed, 1, 1, 0, 0), vec![(V, 0), (A, 0)]);
}

#[test]
fn t49_insert_remove_shift_aware() {
    // вставка V-дорожки в позицию 1: (V,1) старая → (V,2), новая (V,1)
    // встаёт прямо над ней, чужие виды не тронуты
    let mut rows = vec![(V, 0), (A, 0), (V, 1), (M, 0)];
    lane_order_insert(&mut rows, V, 1);
    assert_eq!(rows, vec![(V, 0), (A, 0), (V, 1), (V, 2), (M, 0)]);
    // вставка в конец вида (позиции at нет) — в хвост вектора
    lane_order_insert(&mut rows, A, 1);
    assert_eq!(rows, vec![(V, 0), (A, 0), (V, 1), (V, 2), (M, 0), (A, 1)]);
    // удаление сдвигает большие lane вниз, строка исчезает
    lane_order_remove(&mut rows, V, 1);
    assert_eq!(rows, vec![(V, 0), (A, 0), (V, 1), (M, 0), (A, 1)]);
    // нормализация после операций — идемпотентна (вектор уже корректен)
    assert_eq!(normalize_lane_order(&rows, 2, 2, 1, 0), rows);
    // и ТО ЖЕ делает `Project::insert_track`: строка над бывшей `at`; ранга
    // `at` нет — в конец БЛОКА своего вида (естественный порядок V..A..M..C,
    // а не хвост списка: в приложении место всё равно задаёт `SetLaneOrder`)
    let mut p = Project::new(settings30());
    p.ensure_tracks(M, 1);
    p.set_lane_order(&[(V, 0), (A, 0), (M, 0)]);
    p.insert_track(V, 0);
    assert_eq!(p.rows(), vec![(V, 0), (V, 1), (A, 0), (M, 0)]);
    p.insert_track(A, 1);
    assert_eq!(p.rows(), vec![(V, 0), (V, 1), (A, 0), (A, 1), (M, 0)]);
    p.remove_track(V, 0);
    assert_eq!(p.rows(), vec![(V, 0), (A, 0), (A, 1), (M, 0)]);
}

#[test]
fn t49_lane_order_serde_roundtrip() {
    // с Р.35 порядок строк живёт В МОДЕЛИ (морфер-слой действует «на всё,
    // что ниже», значит порядок влияет на картинку); с Р.62 — это порядок
    // вектора дорожек, и отдельного поля нет ни в модели, ни в файле
    let mut p = Project::new(settings30());
    p.ensure_tracks(M, 1);
    p.set_lane_order(&[(M, 0), (V, 0), (A, 0)]);
    assert_eq!(p.rows(), vec![(M, 0), (V, 0), (A, 0)]);
    let json = serde_json::to_string(&p).unwrap();
    assert!(!json.contains("lane_order"), "отдельного поля порядка больше нет");
    let loaded = Project::from_json(&json).unwrap();
    assert_eq!(loaded, p, "round-trip порядка строк");
    assert_eq!(loaded.rows(), vec![(M, 0), (V, 0), (A, 0)], "строки в том же порядке");

    // естественный порядок читается так же
    let mut nat = p.clone();
    nat.set_lane_order(&[(V, 0), (A, 0), (M, 0)]);
    let json2 = serde_json::to_string(&nat).unwrap();
    assert_eq!(Project::from_json(&json2).unwrap(), nat);
    assert_eq!(nat.rows(), vec![(V, 0), (A, 0), (M, 0)], "и всё равно даёт порядок");

    // строка дорожки: row_of по порядку, а не по номеру дорожки
    assert_eq!(p.row_of((M, 0)), Some(0));
    assert_eq!(p.row_of((A, 0)), Some(2));
    assert_eq!(p.row_of((C, 0)), None, "камерных дорожек нет");
}

/// Тест 241 (Р.35 С1): порядок строк — состояние МОДЕЛИ и ходит через
/// undo; вставка и удаление дорожки двигают его ВНУТРИ своей команды.
#[test]
fn t241_lane_order_is_model_state() {
    use kt_model::{Command, TimelineModel};
    let mut m = TimelineModel::new(Project::new(settings30()));
    // вставка morph-дорожки САМА кладёт строку в порядок
    m.apply(Command::InsertTrack { kind: M, at: 0 }).unwrap();
    assert_eq!(m.project().rows(), vec![(V, 0), (A, 0), (M, 0)]);

    // перестановка — команда, и она откатывается
    let want = vec![(M, 0), (V, 0), (A, 0)];
    m.apply(Command::SetLaneOrder { order: want.clone() }).unwrap();
    assert_eq!(m.project().rows(), want, "порядок применился");
    m.undo().unwrap();
    assert_eq!(m.project().rows(), vec![(V, 0), (A, 0), (M, 0)], "undo вернул порядок");
    m.redo().unwrap();
    assert_eq!(m.project().rows(), want, "redo вернул перестановку");

    // удаление дорожки уносит её строку без ручной синхронизации
    m.apply(Command::RemoveTrack { kind: M, at: 0 }).unwrap();
    assert_eq!(m.project().rows(), vec![(V, 0), (A, 0)]);

    // мусор в команде нормализуется, а не проходит в модель
    m.apply(Command::SetLaneOrder { order: vec![(V, 0), (V, 0), (M, 7), (A, 0)] }).unwrap();
    assert_eq!(m.project().rows(), vec![(V, 0), (A, 0)], "дубли и небылицы выброшены");
}

/// Р.29: камерные дорожки — четвёртый вид в display-порядке (хвост блока
/// V..A..M..C), insert/remove работают той же машинерией.
#[test]
fn t188_camera_lanes_in_display_order() {
    assert_eq!(normalize_lane_order(&[], 1, 1, 1, 2), vec![(V, 0), (A, 0), (M, 0), (C, 0), (C, 1)]);
    // произвольный валидный порядок с камерами сохраняется
    let mixed = vec![(C, 0), (V, 0), (A, 0)];
    assert_eq!(normalize_lane_order(&mixed, 1, 1, 0, 1), mixed);
    // несуществующая камера вычищается
    assert_eq!(normalize_lane_order(&mixed, 1, 1, 0, 0), vec![(V, 0), (A, 0)]);
    let mut rows = vec![(V, 0), (C, 0)];
    lane_order_insert(&mut rows, C, 0);
    assert_eq!(rows, vec![(V, 0), (C, 0), (C, 1)]);
    lane_order_remove(&mut rows, C, 0);
    assert_eq!(rows, vec![(V, 0), (C, 0)]);
    let mut p = Project::new(settings30());
    p.ensure_tracks(C, 2);
    assert_eq!(p.rows(), vec![(V, 0), (A, 0), (C, 0), (C, 1)]);
}

// ---- Р.56, п.449: НОМЕР ДОРОЖКИ = РАНГ СТРОКИ --------------------------------

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

fn vclip(id: u64, lane: u32) -> ClipEvent {
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
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
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

fn three_video_lanes() -> Project {
    let mut p = Project::new(settings30());
    p.media.push(MediaRef { id: MediaId(1), path: "synth:sine:440".into() });
    p.ensure_tracks(V, 3);
    // по событию на каждой дорожке, на РАЗНЫХ временах: перекрытие тут ни
    // при чём, а различить дорожки после перенумерации надо по id
    for (i, id) in [(0u32, 1u64), (1, 2), (2, 3)] {
        let mut c = vclip(id, i);
        c.start = kt_time::Tick(100_000 * i as i64);
        p.clips.push(c);
    }
    p
}

/// Тест 588 (Р.56, п.449). Перестановка строк ПЕРЕНУМЕРОВЫВАЕТ содержимое:
/// номер дорожки внутри вида — это её ранг среди строк этого вида.
///
/// До Р.56 движок складывал видео по НОМЕРУ, а экран показывал по
/// `lane_order` — дорожка могла стоять под потолком и быть нижним слоем
/// композита («закинул туда видео, и его не видно»). Утверждение здесь —
/// СЛЕДСТВИЕ: порядок выдачи клипов движку (`effective_clips`,
/// отсортированный как в `VideoRenderer::build_reel`) совпадает с
/// порядком строк. С Р.62 имя и высота — на дорожке-объекте и едут с ним.
#[test]
fn t588_lane_number_is_row_rank() {
    let mut p = three_video_lanes();
    // адресуемое дорожками: область, чужой трек, имя и высота строки
    p.areas.push(MarkedArea {
        name: "main".into(),
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        lanes: vec![(V, 2)],
        color: None,
    });
    p.foreign.push(ForeignEvent {
        key: 7,
        kind: ClipKind::Video,
        lane: 2,
        start: kt_time::Tick(500_000),
        duration: kt_time::Tick(1_000),
    });
    {
        let t = p.track_at_mut(V, 2).unwrap();
        t.name = "Потолок".into();
        t.height = 42.0;
    }
    let ceiling = p.track_at(V, 2).unwrap().id;
    p.validate().unwrap();

    let mut m = TimelineModel::new(p);
    // человек тащит нижнюю видео-строку на самый верх
    m.apply(Command::SetLaneOrder { order: vec![(V, 2), (V, 0), (V, 1), (A, 0)] }).unwrap();
    let p = m.project();

    // номера стали рангами
    assert_eq!(p.rows(), vec![(V, 0), (V, 1), (V, 2), (A, 0)]);
    let lane_of = |id: u64| p.clips.iter().find(|c| c.id == ClipId(id)).unwrap().lane;
    assert_eq!(lane_of(3), 0, "бывшая нижняя строка стала ВЕРХНЕЙ дорожкой");
    assert_eq!(lane_of(1), 1);
    assert_eq!(lane_of(2), 2);

    // побочные таблицы едут тем же отображением, объект — как объект
    assert_eq!(p.areas[0].lanes, vec![(V, 0)], "область осталась на СВОЕЙ дорожке");
    assert_eq!(p.foreign[0].lane, 0, "чужой трек уехал бы на соседний");
    let t = p.track_at(V, 0).unwrap();
    assert_eq!(t.id, ceiling, "id дорожки стабилен при перестановке");
    assert_eq!(t.name, "Потолок");
    assert_eq!(t.height, 42.0);

    // СЛЕДСТВИЕ: порядок композиции == порядок строк. Движок сортирует
    // `(Reverse(lane), start)` и рисует задний план первым — значит
    // последним в этом векторе идёт ВЕРХНИЙ слой, он же верхняя строка.
    let mut sorted = p.effective_clips();
    sorted.sort_by_key(|c| (std::cmp::Reverse(c.lane), c.start.0));
    let order: Vec<ClipId> = sorted.iter().map(|c| c.id).collect();
    assert_eq!(*order.last().unwrap(), ClipId(3), "верхняя строка обязана быть верхним слоем");

    // undo/redo возвращают номера целиком (история — снимки)
    m.undo().unwrap();
    let p = m.project();
    assert_eq!(p.clips.iter().find(|c| c.id == ClipId(3)).unwrap().lane, 2, "undo не вернул номер");
    assert_eq!(p.areas[0].lanes, vec![(V, 2)]);
    assert_eq!(p.track_at(V, 2).unwrap().name, "Потолок", "имя уехало от своей дорожки под undo");
    m.redo().unwrap();
    assert_eq!(m.project().clips.iter().find(|c| c.id == ClipId(3)).unwrap().lane, 0);
}

/// Тест 589 (Р.56, п.449; Р.62). Инвариант сторожит `validate`: событие на
/// несуществующей дорожке (номер за рангом последней) — ошибка и в корне,
/// и в ленте; чтение (`normalize`) дописывает дорожки.
#[test]
fn t589_validate_guards_canon_but_not_the_bench() {
    let mut p = three_video_lanes();
    // дорожку убрали, событие осталось — так выглядит чужой или битый файл
    let gone = p.track_at(V, 2).unwrap().id;
    p.tracks.retain(|t| t.id != gone);
    assert!(
        matches!(p.validate(), Err(ProjectError::TrackMissing(None, V, 2))),
        "событие без дорожки в КОРНЕ обязано быть ошибкой: {:?}",
        p.validate()
    );

    // то же внутри ленты
    let mut p2 = three_video_lanes();
    let mut s = Sequence::new(MediaId(9), "Reel");
    s.clips.push(vclip(10, 0));
    let mut c = vclip(11, 1);
    c.start = kt_time::Tick(200_000);
    s.clips.push(c);
    p2.sequences.push(s);
    assert!(
        matches!(p2.validate(), Err(ProjectError::TrackMissing(Some(MediaId(9)), V, 1))),
        "событие без дорожки в ЛЕНТЕ обязано быть ошибкой"
    );
    // и чинится чтением
    p2.normalize();
    p2.validate().expect("после normalize документ канонический");
    assert_eq!(p2.sequence(MediaId(9)).unwrap().lanes(V), 2);
}

/// Тест 590 (Р.56, п.449; Р.62). Старый файл знает только счётчики,
/// `lane_order` и имена в `ui`: чтение превращает их в дорожки — и в
/// корне, и в каждой ленте — односторонне, а содержимое перенумеровано
/// под ранги строк.
#[test]
fn t590_load_canonicalizes_root_and_reels() {
    let p = three_video_lanes();
    let mut v = serde_json::to_value(&p).unwrap();
    // конверт ДО Р.62: без `tracks`, со счётчиками, порядком и именами в ui
    v.as_object_mut().unwrap().remove("tracks");
    v["video_lanes"] = serde_json::json!(3);
    v["audio_lanes"] = serde_json::json!(1);
    v["lane_order"] = serde_json::json!([["video", 2], ["audio", 0], ["video", 0], ["video", 1]]);
    v["ui"] = serde_json::json!({
        "view_start": 0.0, "ticks_per_px": 100.0, "cursor": 0,
        "lane_names": [["video", 2, "Потолок"]],
        "lane_heights": [["video", 2, 42.0]]
    });
    v["sequences"] = serde_json::json!([{
        "id": 9, "name": "Reel", "video_lanes": 2, "audio_lanes": 1,
        "lane_order": [["video", 1], ["video", 0], ["audio", 0]],
        "clips": [
            serde_json::to_value(vclip(10, 0)).unwrap(),
            serde_json::to_value({ let mut c = vclip(11, 1); c.start = kt_time::Tick(200_000); c }).unwrap()
        ]
    }]);
    v["clips"].as_array_mut().unwrap().push(
        serde_json::to_value({
            let mut c = vclip(12, 0);
            c.media = MediaId(9);
            c.start = kt_time::Tick(900_000);
            c
        })
        .unwrap(),
    );

    let loaded = Project::from_json(&v.to_string()).expect("старый конверт читается");
    assert_eq!(loaded.rows(), vec![(V, 0), (A, 0), (V, 1), (V, 2)], "порядок строк из lane_order");
    assert_eq!(
        loaded.clips.iter().find(|c| c.id == ClipId(3)).unwrap().lane,
        0,
        "в корне верхняя строка не стала дорожкой 0"
    );
    let top = loaded.track_at(V, 0).unwrap();
    assert_eq!(top.name, "Потолок", "имя из ui.lane_names не доехало до дорожки");
    assert_eq!(top.height, 42.0, "высота из ui.lane_heights не доехала до дорожки");
    assert!(loaded.ui.as_ref().is_none_or(|u| u.lane_names.is_empty()), "легаси-имена не очищены");
    let s = &loaded.sequences[0];
    assert_eq!(s.rows(), vec![(V, 0), (V, 1), (A, 0)]);
    assert_eq!(
        s.clips.iter().find(|c| c.id == ClipId(11)).unwrap().lane,
        0,
        "лента не канонизирована"
    );
    assert!(loaded.tracks.iter().all(|t| t.id.0 != 0), "id не назначены");
    assert!(s.tracks.iter().all(|t| t.id.0 != 0), "id ленты не назначены");
    let all: Vec<TrackId> = loaded.tracks.iter().chain(s.tracks.iter()).map(|t| t.id).collect();
    let uniq: std::collections::HashSet<TrackId> = all.iter().copied().collect();
    assert_eq!(all.len(), uniq.len(), "id дорожек не уникальны по документу");
}
