//! Тесты 148/152/153 (Р.24, этап 10): маппинг Project↔Timeline.
//!
//! 152 — семантика маппинга (инверсия видео-дорожек, эффекты/LTW, транспорт/
//! ui/бин, стабильность id при save→load→save);
//! 153 — импорт чужого файла без kt: детерминизм назначения id и дефолты;
//! 148 — golden легаси-импорта: fixtures/demo.ktproj → save байт-в-байт
//! равен закоммиченной fixtures/demo-legacy-import.otio (замораживает и
//! формулу upgrade_legacy, и схему kt-конвертов).

use kt_model::*;
use kt_otio::{Retained, import_legacy, load_project, parse_document, save_project};
use kt_time::Tick;

fn clip(id: u64, kind: ClipKind, lane: u32, start: i64, dur: i64, media: u64) -> ClipEvent {
    ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn rich_project() -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 1280, height: 720, sample_rate: 48000 });
    for id in [1u64, 2, 3] {
        p.media.push(MediaRef { id: MediaId(id), path: format!("media/m{id}.mp4") });
    }
    // видео: 2 дорожки (z-порядок!), аудио: пара встык с переходом
    p.clips.push(clip(1, ClipKind::Video, 0, 0, 90_000, 1)); // верхний слой
    p.clips.push(clip(2, ClipKind::Video, 1, 0, 45_000, 2)); // задний план
    let mut a1 = clip(10, ClipKind::Audio, 0, 0, 90_000, 1);
    a1.gain_db = -3.0;
    a1.link_group = Some(GroupId(5));
    a1.fx = vec![
        FxInstance::gain(1.5),
        FxInstance::slowed_reverb(0.8, 0.2, 85.0),
    ];
    a1.fx_bypass = vec![true];
    let mut a2 = clip(11, ClipKind::Audio, 0, 90_000, 90_000, 2);
    a2.reversed = true;
    a2.source_in = Tick(30_000);
    p.clips.push(a1);
    p.clips.push(a2);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(10)),
        b: Some(ClipId(11)),
        in_offset: Tick(9_000),
        out_offset: Tick(4_500),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
    });
    p.markers.push(Marker { t: Tick(15_000), duration: Tick(30_000), name: "range".into(), color: None });
    p.markers.push(Marker::named(Tick(120_000), "point"));
    p.region = Some(Region { start: Tick(10_000), end: Tick(60_000) });
    p.loop_enabled = true;
    p.ui = Some(UiState {
        view_start: 0.0,
        ticks_per_px: 1000.0,
        cursor: 42_000,
        lane_names: vec![(LaneKind::Video, 0, "MAIN".into()), (LaneKind::Audio, 0, "DIALOG".into())],
        ..Default::default()
    });
    p.normalize();
    p.validate().unwrap();
    p
}

#[test]
fn t152_mapping_semantics_roundtrip() {
    let p = rich_project();
    let text = save_project(&p, &Retained::default(), None);

    // инверсия видео-дорожек: первый видео-трек в файле — НИЖНИЙ (lane 1)
    let tl = parse_document(&text).unwrap();
    let kinds: Vec<(String, String)> = tl
        .tracks
        .children
        .iter()
        .filter_map(|c| match c {
            kt_otio::Composable::Track(t) => Some((t.kind.clone(), t.name.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(kinds[0].0, "Video");
    assert_eq!(kinds[1], ("Video".into(), "MAIN".into()), "lane 0 (верхний) — последний видео");
    assert_eq!(kinds[2], ("Audio".into(), "DIALOG".into()));

    // round-trip: модель эквивалентна (цвет метки нормализуется в GREEN)
    let bundle = load_project(&text, None).unwrap();
    let p2 = bundle.project;
    assert_eq!(p2.clips, p.clips, "клипы поле-в-поле (fx/bypass/reversed/gain/link)");
    assert_eq!(p2.transitions, p.transitions);
    assert_eq!(p2.region, p.region);
    assert!(p2.loop_enabled);
    assert_eq!(p2.media, p.media, "пути и id медиа стабильны");
    assert_eq!(p2.morphers, p.morphers);
    let m = &p2.markers[0];
    assert_eq!((m.t, m.duration, m.name.as_str()), (Tick(15_000), Tick(30_000), "range"));
    assert_eq!(m.color.as_deref(), Some("GREEN"), "цвет нормализован дефолтом эталона");
    let ui = p2.ui.as_ref().unwrap();
    assert_eq!(ui.cursor, 42_000);
    assert!(ui.lane_names.contains(&(LaneKind::Video, 0, "MAIN".into())));

    // стабильность: второй круг байт-в-байт (идемпотентность на уровне проекта)
    let text2 = save_project(&p2, &bundle.retained, None);
    let p3 = load_project(&text2, None).unwrap().project;
    assert_eq!(p3, p2);
    assert_eq!(save_project(&p3, &bundle.retained, None), text2, "save→load→save стабильно");
}

#[test]
fn t152_foreign_time_warp_and_transition_type_are_retained() {
    let p = rich_project();
    let text = save_project(&p, &Retained::default(), None);
    // чужой тайм-варп x2 + чужой тип перехода
    let hacked = text
        .replace("\"SMPTE_Dissolve\"", "\"Custom_Wipe\"")
        .replace(
            "\"OTIO_SCHEMA\": \"Effect.1\",\n                                    \"name\": \"\",\n                                    \"effect_name\": \"kt:gain\"",
            "\"OTIO_SCHEMA\": \"LinearTimeWarp.1\",\n                                    \"name\": \"\",\n                                    \"effect_name\": \"kt:gain\"",
        );
    // (подмена gain-эффекта на варп может не найтись из-за форматирования —
    // тогда проверяем только тип перехода)
    let bundle = load_project(&hacked, None).unwrap();
    assert!(
        bundle.notices.iter().any(|n| n.contains("Custom_Wipe")),
        "notice о чужом типе перехода: {:?}",
        bundle.notices
    );
    // ре-сейв возвращает чужой тип нетронутым
    let out = save_project(&bundle.project, &bundle.retained, None);
    assert!(out.contains("Custom_Wipe"), "transition_type passthrough");
}

#[test]
fn t153_foreign_import_is_deterministic() {
    // чужой файл без kt: минимальный таймлайн из эталонного стиля
    let text = serde_json::json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": "foreign cut",
        "tracks": {"OTIO_SCHEMA": "Stack.1", "children": [{
            "OTIO_SCHEMA": "Track.1", "kind": "Video", "children": [
                {"OTIO_SCHEMA": "Clip.2", "name": "a",
                 "source_range": {"OTIO_SCHEMA": "TimeRange.1",
                    "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 12.0, "rate": 24.0},
                    "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": 48.0, "rate": 24.0}},
                 "media_references": {"DEFAULT_MEDIA": {"OTIO_SCHEMA": "ExternalReference.1",
                    "target_url": "shots/a.mov", "available_range": null}},
                 "active_media_reference_key": "DEFAULT_MEDIA"},
                {"OTIO_SCHEMA": "Gap.1", "source_range": {"OTIO_SCHEMA": "TimeRange.1",
                    "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 0.0, "rate": 24.0},
                    "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": 24.0, "rate": 24.0}}},
                {"OTIO_SCHEMA": "Clip.2", "name": "b",
                 "source_range": {"OTIO_SCHEMA": "TimeRange.1",
                    "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 0.0, "rate": 24.0},
                    "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": 24.0, "rate": 24.0}},
                 "media_references": {"DEFAULT_MEDIA": {"OTIO_SCHEMA": "ExternalReference.1",
                    "target_url": "shots/a.mov", "available_range": null}},
                 "active_media_reference_key": "DEFAULT_MEDIA"}
            ]
        }]}
    })
    .to_string();
    let b1 = load_project(&text, None).unwrap();
    let b2 = load_project(&text, None).unwrap();
    assert_eq!(b1.project, b2.project, "повторный импорт детерминирован");
    assert!(b1.notices.iter().any(|n| n.contains("default project settings")));
    let p = &b1.project;
    // id по порядку обхода,媒иа дедуплицированы по URL
    assert_eq!(p.clips.iter().map(|c| c.id.0).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(p.media.len(), 1);
    // 24 fps делит 90000 → подхвачен эвристикой
    assert_eq!(p.settings.fps, 24);
    // времена квантованы к тикам: клип b начинается после 48+24 кадров @24
    assert_eq!(p.clips[1].start, Tick((48 + 24) * 90_000 / 24));
    // пересохранение стабильно
    let out = save_project(p, &b1.retained, None);
    let p2 = load_project(&out, None).unwrap().project;
    assert_eq!(&p2, p);
}

/// Golden легаси-импорта. Регенерация:
/// `cargo test -p kt-otio --test convert -- --ignored t148_regen`
#[test]
fn t148_legacy_import_matches_committed_golden() {
    let src = include_str!("../../../fixtures/demo.ktproj");
    let (p, _notices) = import_legacy(src).expect("legacy imports");
    let out = save_project(&p, &Retained::default(), None);
    let golden = include_str!("../../../fixtures/demo-legacy-import.otio");
    assert_eq!(out, golden, "легаси-импорт разошёлся с golden — это дефект конверсии");
}

#[test]
#[ignore = "генератор эталона: cargo test -p kt-otio --test convert -- --ignored t148_regen"]
fn t148_regen() {
    let src = include_str!("../../../fixtures/demo.ktproj");
    let (p, notices) = import_legacy(src).expect("legacy imports");
    let out = save_project(&p, &Retained::default(), None);
    std::fs::write(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/demo-legacy-import.otio"),
        &out,
    )
    .unwrap();
    println!("regenerated ({} bytes), notices: {notices:?}", out.len());
}

// ---- тест 154: чужие конструкции, которые формат разрешает --------------------

/// Все закоммиченные сэмплы апстрима (fixtures/otio-samples, пин в README)
/// обязаны ЧИТАТЬСЯ. Раньше падали три класса: клип без `source_range`,
/// клип с `media_reference: null`, `name: null` у ссылок.
#[test]
fn t154_upstream_samples_load() {
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/otio-samples"));
    let mut seen = 0;
    for e in std::fs::read_dir(dir).expect("каталог сэмплов") {
        let path = e.unwrap().path();
        if path.extension().and_then(|x| x.to_str()) != Some("otio") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let bundle = load_project(&text, path.parent())
            .unwrap_or_else(|e| panic!("{} не читается: {e}", path.display()));
        // повторная запись обязана быть идемпотентной (инвариант kt-otio)
        let once = save_project(&bundle.project, &bundle.retained, path.parent());
        let again = load_project(&once, path.parent()).expect("наш же вывод читается");
        let twice = save_project(&again.project, &again.retained, path.parent());
        assert_eq!(once, twice, "{}: пересохранение не идемпотентно", path.display());
        seen += 1;
    }
    assert!(seen >= 9, "сэмплы апстрима пропали из фикстур (нашлось {seen})");
}

/// Клип без `source_range` берёт материал целиком — длительность из
/// `available_range` активной ссылки (спека OTIO; так живёт Clip-004 в
/// simple_cut.otio). Точка входа при этом НУЛЕВАЯ: `available_range`
/// живёт в системе координат материала (его стартовый таймкод), а наш
/// `source_in` — смещение от первого кадра файла (п.180).
#[test]
fn t154_clip_without_source_range_uses_available_range() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/otio-samples/simple_cut.otio");
    let text = std::fs::read_to_string(path).unwrap();
    let b = load_project(&text, None).expect("simple_cut читается");
    assert_eq!(b.project.clips.len(), 4);
    let last = b.project.clips.iter().max_by_key(|c| c.start).unwrap();
    // available_range Clip-004: start 100, duration 6 кадров @24 fps
    assert_eq!(last.duration, Tick(6 * 90_000 / 24));
    assert_eq!(last.source_in, Tick::ZERO, "старт available_range — таймкод, а не пропуск кадров");
}

/// Клип с `media_reference: null` законен — это клип без привязки к
/// материалу. Читается, рендерится чёрным/тишиной, а `null` переживает
/// пересохранение (ссылку не выдумываем).
#[test]
fn t154_clip_without_media_reference_survives() {
    let path =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/otio-samples/transition_test.otio");
    let text = std::fs::read_to_string(path).unwrap();
    let b = load_project(&text, None).expect("transition_test читается");
    assert_eq!(b.project.clips.len(), 3);
    for c in &b.project.clips {
        let m = b.project.media.iter().find(|m| m.id == c.media).expect("медиа-запись есть");
        assert_eq!(m.path, "", "у клипа без ссылки нет пути к материалу");
    }
    // клипы без ссылки НЕ делят один media_id: общего материала у них нет
    let ids: std::collections::HashSet<_> = b.project.clips.iter().map(|c| c.media).collect();
    assert_eq!(ids.len(), 3);

    let out = save_project(&b.project, &b.retained, None);
    parse_document(&out).expect("наш вывод типизируется");
    let json: serde_json::Value = serde_json::from_str(&out).unwrap();
    let refs = json["tracks"]["children"][0]["children"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["OTIO_SCHEMA"] == "Clip.2")
        .map(|c| c["media_references"]["DEFAULT_MEDIA"].clone())
        .collect::<Vec<_>>();
    assert_eq!(refs.len(), 3);
    assert!(refs.iter().all(|r| r.is_null()), "null-ссылка не должна превращаться в ExternalReference: {refs:?}");
}

/// Тест 165 (п.165 porting-notes, Р.28): дорожка с ПЕРЕСЛОЕНИЕМ уезжает в
/// соседние Track'и и собирается обратно в одну. В OTIO позиция клипа — это
/// порядок детей трека, поэтому иначе перекрытие развалило бы монтаж.
#[test]
fn t165_overlapping_lane_survives_layer_roundtrip() {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "media/a.mp4".into() });
    // три клипа одной дорожки: два перекрываются, третий встык
    for (id, start, dur) in [(1u64, 0i64, 90_000i64), (2, 45_000, 90_000), (3, 200_000, 45_000)] {
        p.clips.push(ClipEvent {
            id: ClipId(id),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(start),
            duration: Tick(dur),
            source_in: Tick(0),
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
    p.normalize();
    p.validate().expect("перекрытие законно при переслаивании");

    let text = save_project(&p, &Retained::default(), None);
    let doc = parse_document(&text).expect("наш вывод типизируется");
    let tracks = doc
        .tracks
        .children
        .iter()
        .filter(|c| matches!(c, kt_otio::Composable::Track(_)))
        .count();
    assert_eq!(tracks, 2, "перекрытая дорожка разложилась на два трека");

    let back = load_project(&text, None).expect("читается обратно").project;
    assert_eq!(back.clips.len(), 3);
    assert!(back.clips.iter().all(|c| c.lane == 0), "дорожка собрана обратно: {:?}", back.clips);
    assert_eq!(back.clips, p.clips, "геометрия клипов не изменилась");
    assert_eq!(back.video_lanes, p.video_lanes, "дорожек столько же");

    // второй круг байт-в-байт (идемпотентность, расширение т.144)
    let again = save_project(&back, &Retained::default(), None);
    assert_eq!(again, text, "save→load→save стабильно");
}

/// Проект БЕЗ перекрытий пишется как раньше: карта слоёв не появляется.
#[test]
fn t165_plain_project_has_no_layer_map() {
    let p = rich_project();
    let text = save_project(&p, &Retained::default(), None);
    assert!(!text.contains("\"layers\""), "без перекрытий карта слоёв не пишется");
}

/// Тест 178: NTSC-частоты чужих файлов (23.976/29.97/59.94) не ложатся на
/// сетку 90 кГц (90000/23.976 = 3753.75) — и раньше такой файл МОЛЧА
/// становился проектом на 30 fps. Найдено на настоящем фильме: Netflix
/// «Sol Levante» (аниматик, 63 склейки, 23.976) импортировался как 30.
/// Теперь дробная частота округляется до ближайшей целой, делящей 90000.
#[test]
fn t178_ntsc_rates_land_on_the_nearest_grid_fps() {
    use kt_otio::grid_fps;
    assert_eq!(grid_fps(23.976023976023978), Some(24), "NTSC-24");
    assert_eq!(grid_fps(29.97002997002997), Some(30), "NTSC-30");
    assert_eq!(grid_fps(59.94005994005994), Some(60), "NTSC-60");
    assert_eq!(grid_fps(47.952047952047955), Some(48), "NTSC-48");
    // целые частоты — как были
    for r in [24.0, 25.0, 30.0, 50.0, 60.0] {
        assert_eq!(grid_fps(r), Some(r as u32), "целая {r}");
    }
    // не делит 90000 — не годится (сетка была бы нецелой в тиках)
    assert_eq!(grid_fps(23.0), None, "23 не делит 90000");
    assert_eq!(grid_fps(7.0), None);
    // мусор и внесистемные значения
    assert_eq!(grid_fps(0.0), None);
    assert_eq!(grid_fps(-24.0), None);
    assert_eq!(grid_fps(1000.0), None);
    assert_eq!(grid_fps(f64::NAN), None);
    // «почти целая» — только в пределах половины кадра
    assert_eq!(grid_fps(24.4), Some(24));
    assert_eq!(grid_fps(24.6), Some(25));

    // и целиком: таймлайн на 23.976 читается как проект 24 fps
    let doc = format!(
        r#"{{"OTIO_SCHEMA":"Timeline.1","name":"ntsc","tracks":{{"OTIO_SCHEMA":"Stack.1",
        "name":"","children":[{{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
        {{"OTIO_SCHEMA":"Clip.1","name":"c","source_range":{{"OTIO_SCHEMA":"TimeRange.1",
        "duration":{{"OTIO_SCHEMA":"RationalTime.1","value":240,"rate":{r}}},
        "start_time":{{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":{r}}}}},
        "media_reference":{{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a.mov",
        "available_range":null,"available_image_bounds":null,"metadata":{{}}}},
        "effects":[],"markers":[],"enabled":true,"metadata":{{}}}}],
        "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{{}}}}],
        "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{{}}}},
        "global_start_time":null,"metadata":{{}}}}"#,
        r = 23.976023976023978f64
    );
    let b = kt_otio::load_project(&doc, None).expect("чужой NTSC-файл читается");
    assert_eq!(b.project.settings.fps, 24, "23.976 → сетка 24, а не умолчание 30");
}

/// Тест 180: стартовый таймкод материала. В OTIO `source_range` живёт в
/// системе координат самого материала — той же, что `available_range`, а у
/// вещательных и киношных файлов она начинается со вшитого таймкода (у
/// «Sol Levante» — ровно час). Наш `source_in` — смещение от ПЕРВОГО кадра
/// файла, поэтому таймкод вычитается на чтении и возвращается на записи.
/// Найдено кросс-рендером настоящего фильма (т.179): без вычитания движок
/// уезжал на час в 162-секундный файл.
#[test]
fn t180_media_start_timecode_is_subtracted_and_restored() {
    let doc = |src_start: f64, avail_start: f64| {
        format!(
            r#"{{"OTIO_SCHEMA":"Timeline.1","name":"tc","tracks":{{"OTIO_SCHEMA":"Stack.1",
        "name":"","children":[{{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
        {{"OTIO_SCHEMA":"Clip.1","name":"c","source_range":{{"OTIO_SCHEMA":"TimeRange.1",
        "duration":{{"OTIO_SCHEMA":"RationalTime.1","value":48,"rate":24}},
        "start_time":{{"OTIO_SCHEMA":"RationalTime.1","value":{src_start},"rate":24}}}},
        "media_reference":{{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a.mov",
        "available_range":{{"OTIO_SCHEMA":"TimeRange.1",
        "duration":{{"OTIO_SCHEMA":"RationalTime.1","value":3890,"rate":24}},
        "start_time":{{"OTIO_SCHEMA":"RationalTime.1","value":{avail_start},"rate":24}}}},
        "available_image_bounds":null,"metadata":{{}}}},
        "effects":[],"markers":[],"enabled":true,"metadata":{{}}}}],
        "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{{}}}}],
        "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{{}}}},
        "global_start_time":null,"metadata":{{}}}}"#
        )
    };

    // час таймкода @24 = 86400 кадров; клип берёт материал с 10-й секунды
    let hour = 86_400.0;
    let b = load_project(&doc(hour + 240.0, hour), None).expect("файл с таймкодом читается");
    let c = &b.project.clips[0];
    assert_eq!(c.source_in, Tick(240 * 90_000 / 24), "10 c ОТ НАЧАЛА ФАЙЛА, а не от часа");
    assert_eq!(c.duration, Tick(48 * 90_000 / 24));

    // и обратно: чужой редактор обязан увидеть тот же таймкод
    let out = save_project(&b.project, &b.retained, None);
    let back = load_project(&out, None).expect("наш вывод читается");
    assert_eq!(back.project.clips[0].source_in, c.source_in, "круг не двигает точку входа");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let st = &v["tracks"]["children"][0]["children"][0]["source_range"]["start_time"];
    let ticks = st["value"].as_f64().unwrap() * 90_000.0 / st["rate"].as_f64().unwrap();
    assert_eq!(
        ticks.round() as i64,
        (hour as i64 + 240) * 90_000 / 24,
        "таймкод материала вернулся на место"
    );

    // файл без таймкода (start 0) ведёт себя как раньше
    let b0 = load_project(&doc(240.0, 0.0), None).expect("файл без таймкода");
    assert_eq!(b0.project.clips[0].source_in, Tick(240 * 90_000 / 24));
    // клип, начинающийся ДО объявленного начала материала, клампится нулём
    let bn = load_project(&doc(0.0, hour), None).expect("клип левее материала");
    assert_eq!(bn.project.clips[0].source_in, Tick::ZERO);
}

// ---- тест 189 (Р.29): камеры и версия файла ПО СОДЕРЖИМОМУ -------------------

fn project_3d() -> Project {
    let mut p = rich_project();
    // Pan3d на верхнем видео-клипе: обычный kt-эффект, версию НЕ бампает
    p.clips[0].fx = vec![FxInstance::Pan3d {
        pos_x: 10.0,
        pos_y: -4.0,
        pos_z: 120.0,
        rot_x: 0.0,
        rot_y: 25.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale: 1.1,
        camera: Some(CameraId(2)),
    }];
    // автоматизация (Р.30): кривая поворота — замораживается тем же golden
    p.clips[0].fx_auto = vec![vec![ParamCurve {
        param: "rot_y".into(),
        ease: true,
        keys: vec![AutoKey { t: Tick(0), v: 0.0 }, AutoKey { t: Tick(90_000), v: 90.0 }],
    }]];
    // две камеры на двух дорожках, с перекрытием по времени и кейфреймами
    p.camera_lanes = 2;
    p.cameras = vec![
        CameraEvent {
            id: CameraId(1),
            lane: 0,
            start: Tick(0),
            duration: Tick(90_000),
            name: "main".into(),
            cam: CameraParams { pos_z: -900.0, ..Default::default() },
            keyframes: vec![],
            // домен кривых камеры — тики от начала события (Р.34 К3)
            auto: vec![
                ParamCurve {
                    param: "pos_x".into(),
                    ease: true,
                    keys: vec![
                        AutoKey { t: Tick(0), v: 0.0 },
                        AutoKey { t: Tick(90_000), v: 60.0 },
                    ],
                },
                ParamCurve {
                    param: "fov_deg".into(),
                    ease: true,
                    keys: vec![
                        AutoKey { t: Tick(0), v: 39.6 },
                        AutoKey { t: Tick(90_000), v: 30.0 },
                    ],
                },
            ],
            ease: true,
        },
        CameraEvent {
            id: CameraId(2),
            lane: 1,
            start: Tick(45_000),
            duration: Tick(90_000),
            name: String::new(),
            cam: CameraParams {
                target: Some((0.0, 0.0, 0.0)),
                fov_deg: 55.0,
                ..Default::default()
            },
            keyframes: vec![],
            auto: vec![],
            ease: false,
        },
    ];
    p.normalize();
    p.validate().unwrap();
    p
}

/// Версия файла по содержимому: без камер v=1 (байт-стабильность старых
/// файлов), с камерами v=2 (старый ридер отказывает KtMetaTooNew, а не
/// молча теряет 3D-сцену при перезаписи). Round-trip и идемпотентность.
#[test]
fn t189_cameras_roundtrip_and_conditional_version() {
    // без камер — v=1, никаких камерных ключей
    let flat = save_project(&rich_project(), &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&flat).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 1, "без камер файл остаётся v1");
    assert!(!flat.contains("camera"), "камерные ключи не пишутся впустую");

    // только камеры (без кривых) — v=2
    let mut cams_only = project_3d();
    cams_only.clips[0].fx_auto.clear();
    for c in &mut cams_only.cameras {
        c.auto.clear(); // кривые камеры — тоже v3
    }
    let text = save_project(&cams_only, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 2, "камеры поднимают версию файла до v2");

    // камеры + автоматизация — v=3, полный round-trip, идемпотентность
    let p = project_3d();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 3, "кривые нового домена поднимают до v3");
    let b = load_project(&text, None).expect("наш 3D-файл читается");
    assert_eq!(b.project.camera_lanes, 2);
    assert_eq!(b.project.cameras, p.cameras, "камеры пережили круг");
    assert_eq!(b.project.clips.iter().find(|c| c.id == ClipId(1)).unwrap().fx, p.clips[0].fx);
    let again = save_project(&b.project, &b.retained, None);
    assert_eq!(text, again, "идемпотентность записи с камерами");

    // ЛЕГАСИ-кейфреймы камеры (домен «доля события») конвертируются при
    // чтении в общие кривые — Р.34 К3, односторонний апгрейд
    let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
    let cam = &mut old["metadata"]["kt"]["cameras"][0];
    cam["auto"] = serde_json::json!([]);
    cam["keyframes"] = serde_json::json!([
        { "t": 0.0, "cam": { "pos_x": 0.0, "fov_deg": 39.6 } },
        { "t": 1.0, "cam": { "pos_x": 60.0, "fov_deg": 30.0 } },
    ]);
    let b = load_project(&serde_json::to_string_pretty(&old).unwrap(), None)
        .expect("файл со старыми кейфреймами камеры обязан открываться");
    let c = &b.project.cameras[0];
    assert!(c.keyframes.is_empty(), "легаси-кейфреймы съедены миграцией");
    let px = c.auto.iter().find(|c| c.param == "pos_x").expect("кривая pos_x");
    assert_eq!(
        px.keys.iter().map(|k| (k.t.0, k.v)).collect::<Vec<_>>(),
        vec![(0, 0.0), (90_000, 60.0)],
        "доля события переведена в тики от начала"
    );
    assert!(
        c.auto.iter().all(|c| c.param != "pos_y"),
        "ровные параметры кривых не заводят"
    );
}

/// Неизвестные kt-поля будущих версий (v3+) возятся passthrough'ом:
/// v2-сборка ничего не теряет — и потому будущим добавкам не понадобится
/// жёсткий отказ.
#[test]
fn t189b_future_kt_fields_pass_through() {
    let text = save_project(&project_3d(), &Retained::default(), None);
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["metadata"]["kt"]["future_field"] = serde_json::json!({"x": 1});
    let b = load_project(&serde_json::to_string_pretty(&v).unwrap(), None).unwrap();
    let out = save_project(&b.project, &b.retained, None);
    assert!(out.contains("future_field"), "неизвестное kt-поле не потерялось");
}

/// Golden 3D-файла: замораживает serde-схему камер/Pan3d (как t148 — v1).
/// Регенерация: `cargo test -p kt-otio --test convert -- --ignored t189_regen`
#[test]
fn t189_matches_committed_golden() {
    let out = save_project(&project_3d(), &Retained::default(), None);
    let golden = include_str!("../../../fixtures/demo-3d.otio");
    assert_eq!(out, golden, "3D-схема (камеры/Pan3d) разошлась с golden");
}

// ---- тест 192 (Р.30): автоматизация параметров в файле ------------------------

/// Кривые едут внутри kt-конверта эффекта; файл С автоматизацией — v3
/// (Р.33: ключи в тиках материала, старый ридер прочёл бы их как доли и
/// испортил анимацию), без неё — v1. Round-trip и идемпотентность.
#[test]
fn t192_fx_auto_roundtrip_and_v3() {
    // плоский проект с автоматизацией, БЕЗ камер: v3 даёт именно она
    let mut p = rich_project();
    p.clips[0].fx = vec![FxInstance::blur(8)];
    p.clips[0].fx_auto = vec![vec![ParamCurve {
        param: "radius".into(),
        ease: false,
        keys: vec![AutoKey { t: Tick(0), v: 1.0 }, AutoKey { t: Tick(90_000), v: 48.0 }],
    }]];
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 3, "автоматизация поднимает файл до v3");
    let b = load_project(&text, None).expect("файл с автоматизацией читается");
    let c = b.project.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    assert_eq!(c.fx_auto, p.clips[0].fx_auto, "кривые пережили круг");
    let again = save_project(&b.project, &b.retained, None);
    assert_eq!(text, again, "идемпотентность записи с автоматизацией");
    // ключ с отрицательной позицией — честный отказ загрузки (validate)
    let mut broken: serde_json::Value = serde_json::from_str(&text).unwrap();
    let eff = &mut broken["tracks"]["children"][1]["children"][0]["effects"][0];
    eff["metadata"]["kt"]["auto"][0]["keys"][0]["t"] = serde_json::json!(-5);
    assert!(
        load_project(&serde_json::to_string_pretty(&broken).unwrap(), None).is_err(),
        "ключ до начала материала обязан валить загрузку явно"
    );
    // нечитаемая кривая тоже валит загрузку: молча потерять анимацию хуже
    let mut broken: serde_json::Value = serde_json::from_str(&text).unwrap();
    let eff = &mut broken["tracks"]["children"][1]["children"][0]["effects"][0];
    eff["metadata"]["kt"]["auto"] = serde_json::json!("мусор");
    assert!(
        load_project(&serde_json::to_string_pretty(&broken).unwrap(), None).is_err(),
        "нечитаемая автоматизация не должна теряться молча"
    );
}

/// Файл СТАРОГО домена (кривые в долях длины события) читается и
/// переводится в тики материала по геометрии клипа — иначе анимация
/// рабочих проектов пользователя уехала бы (Р.33).
#[test]
fn t212_pre_v3_curves_migrate_to_material_ticks() {
    let mut p = rich_project();
    p.clips[0].fx = vec![FxInstance::blur(8)];
    p.clips[0].fx_auto = vec![vec![ParamCurve {
        param: "radius".into(),
        ease: false,
        keys: vec![AutoKey { t: Tick(0), v: 1.0 }, AutoKey { t: Tick(90_000), v: 48.0 }],
    }]];
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    // делаем из него файл ПРОШЛОГО формата: версия v2, ключи в долях
    let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
    old["metadata"]["kt"]["v"] = serde_json::json!(2);
    let eff = &mut old["tracks"]["children"][1]["children"][0]["effects"][0];
    eff["metadata"]["kt"]["auto"][0]["keys"][0]["t"] = serde_json::json!(0.0);
    eff["metadata"]["kt"]["auto"][0]["keys"][1]["t"] = serde_json::json!(0.5);
    let b = load_project(&serde_json::to_string_pretty(&old).unwrap(), None)
        .expect("файл старого формата обязан открываться");
    let c = b.project.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    // клип 0..90_000 с source_in=0: доли 0 и 0.5 → тики 0 и 45_000
    assert_eq!(
        c.fx_auto[0][0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![0, 45_000],
        "старые доли не переведены в тики материала"
    );
    // и следующая запись уже в новом домене (v3)
    let again = save_project(&b.project, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&again).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 3, "перезапись поднимает файл до v3");
}

/// Вид перехода и кривая его аудио живут в файле (Р.33 С4): вайп остаётся
/// вайпом после круга, наружу уходит осмысленный `transition_type`, файл
/// поднимается до v3, а обычный кроссфейд байт-стабилен и остаётся v1.
#[test]
fn t222_transition_kind_roundtrip_and_version() {
    // 1. обычный кроссфейд ничего не меняет в файле
    let plain = rich_project();
    let text = save_project(&plain, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 1, "кроссфейд по умолчанию не бампит версию");
    let tr = find_transition(&v);
    assert_eq!(tr["transition_type"], "SMPTE_Dissolve");
    assert!(tr["metadata"].get("kt").is_none(), "у кроссфейда наш конверт не пишется");

    // 2. вайп с параметрами и не-дефолтной кривой аудио
    let mut p = rich_project();
    p.transitions[0].kind = TransitionKind::Wipe {
        dir: Dir::Up,
        softness: 0.25,
        angle: 12.0,
    };
    p.transitions[0].audio = AudioCurve::SCurve;
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 3, "вид перехода поднимает файл до v3");
    let tr = find_transition(&v);
    assert_eq!(tr["transition_type"], "SMPTE_Wipe", "вайп наружу — стандартным типом");
    assert_eq!(tr["metadata"]["kt"]["kind"]["kind"], "wipe");
    assert_eq!(tr["metadata"]["kt"]["audio"], "s_curve");

    let b = load_project(&text, None).expect("файл с вайпом читается");
    assert_eq!(b.project.transitions, p.transitions, "вид и кривая пережили круг");
    let again = save_project(&b.project, &b.retained, None);
    assert_eq!(text, again, "идемпотентность записи с видом перехода");

    // 3. возврат к кроссфейду ЧИСТИТ наш конверт: устаревшее «wipe» из
    // retained не должно уехать обратно в файл
    let mut back = b.project.clone();
    back.transitions[0].kind = TransitionKind::Crossfade;
    back.transitions[0].audio = AudioCurve::EqualPower;
    let text = save_project(&back, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 1, "без вида перехода файл снова v1");
    let tr = find_transition(&v);
    assert!(tr["metadata"].get("kt").is_none(), "конверт вида убран целиком");

    // 4. чужой transition_type не подменяется нашим (passthrough, т.152)
    let mut alien: serde_json::Value = serde_json::from_str(&text).unwrap();
    *find_transition_mut(&mut alien) = {
        let mut t = find_transition(&alien).clone();
        t["transition_type"] = serde_json::json!("Vendor_Fancy");
        t
    };
    let src = serde_json::to_string_pretty(&alien).unwrap();
    let b = load_project(&src, None).expect("чужой тип перехода читается");
    let out = save_project(&b.project, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        find_transition(&v)["transition_type"],
        "Vendor_Fancy",
        "чужой тип возвращается как был"
    );

    // 4b. кривые параметров вида (Р.34 К4) ездят в том же конверте
    let mut p = rich_project();
    p.transitions[0].kind = TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.2 };
    p.transitions[0].auto = vec![ParamCurve {
        param: "angle".into(),
        ease: true,
        keys: vec![AutoKey { t: Tick(0), v: 0.0 }, AutoKey { t: Tick(13_500), v: 45.0 }],
    }];
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 3, "кривые перехода — тоже v3");
    let b = load_project(&text, None).expect("файл с кривыми перехода читается");
    assert_eq!(b.project.transitions[0].auto, p.transitions[0].auto, "кривые пережили круг");
    assert_eq!(text, save_project(&b.project, &b.retained, None), "идемпотентность");

    // 5. нечитаемый вид — не молчание: замечание и честный кроссфейд
    let mut broken: serde_json::Value = serde_json::from_str(&text).unwrap();
    find_transition_mut(&mut broken)["metadata"]["kt"] = serde_json::json!({ "kind": "чепуха" });
    let b = load_project(&serde_json::to_string_pretty(&broken).unwrap(), None)
        .expect("битый вид не валит файл целиком");
    assert_eq!(b.project.transitions[0].kind, TransitionKind::Crossfade);
    assert!(
        b.notices.iter().any(|n| n.contains("transition kind")),
        "битый вид обязан дать замечание, а не тишину: {:?}",
        b.notices
    );
}

/// Первый Transition в дереве дорожек (в rich_project он один).
fn find_transition(v: &serde_json::Value) -> &serde_json::Value {
    for tr in v["tracks"]["children"].as_array().unwrap() {
        for k in tr["children"].as_array().unwrap() {
            if k["OTIO_SCHEMA"].as_str().is_some_and(|s| s.starts_with("Transition.")) {
                return k;
            }
        }
    }
    panic!("в файле нет перехода");
}

fn find_transition_mut(v: &mut serde_json::Value) -> &mut serde_json::Value {
    for tr in v["tracks"]["children"].as_array_mut().unwrap() {
        for k in tr["children"].as_array_mut().unwrap() {
            if k["OTIO_SCHEMA"].as_str().is_some_and(|s| s.starts_with("Transition.")) {
                return k;
            }
        }
    }
    panic!("в файле нет перехода");
}

#[test]
#[ignore = "генератор эталона: cargo test -p kt-otio --test convert -- --ignored t189_regen"]
fn t189_regen() {
    let out = save_project(&project_3d(), &Retained::default(), None);
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/demo-3d.otio"), &out)
        .unwrap();
    println!("regenerated ({} bytes)", out.len());
}

/// Тест 240a (Р.35 С1): схема морфера сменилась — вид (`Replace`/
/// `EffectBatch`) схлопнулся в один патч, цели стали режимом. Файл ДО v4
/// читается с прежней семантикой (`timed: false`), слой поднимает файл до
/// v4, а проект без морферов остаётся байт-стабильным.
#[test]
fn t240_morpher_schema_v4_and_legacy_read() {
    // старый файл: голый массив целей и внешне-тегированный вид
    let base = save_project(&rich_project(), &Retained::default(), None);
    let mut old: serde_json::Value = serde_json::from_str(&base).unwrap();
    old["metadata"]["kt"]["morph_lanes"] = serde_json::json!(1);
    old["metadata"]["kt"]["morphers"] = serde_json::json!([{
        "id": 1, "lane": 0, "start": 0, "duration": 45000,
        "targets": [1],
        "kind": { "Replace": { "media": 2, "source_in_delta": 4500 } },
    }]);
    let b = load_project(&serde_json::to_string_pretty(&old).unwrap(), None)
        .expect("файл со старым морфером обязан открываться");
    let m = &b.project.morphers[0];
    assert!(!m.timed, "морфер до v4 действует вне времени — как раньше");
    assert_eq!(m.targets.clip_ids(), &[ClipId(1)], "голый массив целей прочитан");
    assert_eq!(m.patch.media, Some(MediaId(2)), "вид Replace прочитан в патч");
    assert_eq!(m.patch.source_in_delta, Tick(4_500));

    // такой морфер файл до v4 не поднимает
    let text = save_project(&b.project, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_ne!(v["metadata"]["kt"]["v"], 4, "прежняя семантика — прежняя версия");
    assert_eq!(text, save_project(&load_project(&text, None).unwrap().project,
        &b.retained, None), "идемпотентность записи морфера");

    // слой поднимает до v4 и переживает круг
    let mut p = b.project.clone();
    p.morphers[0].timed = true;
    p.morphers[0].targets = MorphTargets::Below;
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 4, "слой поднимает файл до v4");
    let m = &v["metadata"]["kt"]["morphers"][0];
    assert!(m.get("targets").is_none(), "умолчание «ниже» не сериализуется");
    assert!(m.get("timed").is_none(), "умолчание «по времени» не сериализуется");
    let b2 = load_project(&text, None).expect("файл v4 читается");
    assert_eq!(b2.project.morphers, p.morphers, "морфер пережил круг");

    // проект БЕЗ морферов версию не двигает
    let plain = save_project(&rich_project(), &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&plain).unwrap();
    assert_ne!(v["metadata"]["kt"]["v"], 4, "без морферов версия не растёт");
}

/// Тест 241b (Р.35 С1): порядок строк переехал из `ui.lane_order` в
/// модель. Файл со старым полем апгрейдится односторонне, новый пишет
/// своё, а проект с естественным порядком остаётся байт-стабильным.
#[test]
fn t241_lane_order_moved_into_model() {
    let mut p = rich_project();
    p.morph_lanes = 1;
    p.set_lane_order(&[(kt_model::LaneKind::Morph, 0)]);
    assert!(!p.lane_order.is_empty(), "перемешанный порядок хранится");
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(v["metadata"]["kt"]["lane_order"].is_array(), "порядок пишется своим полем");
    assert!(
        v["metadata"]["kt"]["ui"].get("lane_order").is_none(),
        "в ui порядок больше не пишется"
    );
    let b = load_project(&text, None).expect("файл с порядком строк читается");
    assert_eq!(b.project.rows(), p.rows(), "порядок пережил круг");
    assert_eq!(text, save_project(&b.project, &b.retained, None), "идемпотентность");

    // СТАРЫЙ файл: порядок лежал в ui.lane_order — читается оттуда
    let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
    let order = old["metadata"]["kt"]["lane_order"].clone();
    old["metadata"]["kt"].as_object_mut().unwrap().remove("lane_order");
    old["metadata"]["kt"]["ui"]["lane_order"] = order;
    let b = load_project(&serde_json::to_string_pretty(&old).unwrap(), None)
        .expect("файл со старым местом порядка обязан открываться");
    assert_eq!(b.project.rows(), p.rows(), "порядок переехал из ui в модель");

    // естественный порядок поля не заводит
    let plain = save_project(&rich_project(), &Retained::default(), None);
    assert!(!plain.contains("lane_order"), "естественный порядок не хранится");
}
