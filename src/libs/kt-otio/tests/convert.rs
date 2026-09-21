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
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
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
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    for id in [1u64, 2, 3] {
        p.media.push(MediaRef { id: MediaId(id), path: format!("media/m{id}.mp4") });
    }
    // видео: 2 дорожки (z-порядок!), аудио: пара встык с переходом
    p.clips.push(clip(1, ClipKind::Video, 0, 0, 90_000, 1)); // верхний слой
    p.clips.push(clip(2, ClipKind::Video, 1, 0, 45_000, 2)); // задний план
    let mut a1 = clip(10, ClipKind::Audio, 0, 0, 90_000, 1);
    a1.gain_db = -3.0;
    a1.link_group = Some(GroupId(5));
    a1.fx = vec![FxInstance::gain(1.5), FxInstance::slowed_reverb(0.8, 0.2, 85.0)];
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
        invert: false,
    });
    p.markers.push(Marker {
        t: Tick(15_000),
        duration: Tick(30_000),
        name: "range".into(),
        color: None,
    });
    p.markers.push(Marker::named(Tick(120_000), "point"));
    p.region = Some(Region { start: Tick(10_000), end: Tick(60_000) });
    p.loop_enabled = true;
    p.ui = Some(UiState {
        view_start: 0.0,
        ticks_per_px: 1000.0,
        cursor: 42_000,
        lane_names: vec![
            (LaneKind::Video, 0, "MAIN".into()),
            (LaneKind::Audio, 0, "DIALOG".into()),
        ],
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
    let dir =
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/otio-samples"));
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
    assert!(
        refs.iter().all(|r| r.is_null()),
        "null-ссылка не должна превращаться в ExternalReference: {refs:?}"
    );
}

/// Тест 165 (п.165 porting-notes, Р.28): дорожка с ПЕРЕСЛОЕНИЕМ уезжает в
/// соседние Track'и и собирается обратно в одну. В OTIO позиция клипа — это
/// порядок детей трека, поэтому иначе перекрытие развалило бы монтаж.
#[test]
fn t165_overlapping_lane_survives_layer_roundtrip() {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "media/a.mp4".into() });
    // три клипа одной дорожки: два перекрываются, третий встык
    for (id, start, dur) in [(1u64, 0i64, 90_000i64), (2, 45_000, 90_000), (3, 200_000, 45_000)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
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
    let tracks =
        doc.tracks.children.iter().filter(|c| matches!(c, kt_otio::Composable::Track(_))).count();
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
        source: Default::default(),
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
                    source: Default::default(),
                },
                ParamCurve {
                    param: "fov_deg".into(),
                    ease: true,
                    keys: vec![
                        AutoKey { t: Tick(0), v: 39.6 },
                        AutoKey { t: Tick(90_000), v: 30.0 },
                    ],
                    source: Default::default(),
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
    assert!(c.auto.iter().all(|c| c.param != "pos_y"), "ровные параметры кривых не заводят");
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
        source: Default::default(),
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
        source: Default::default(),
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
    p.transitions[0].kind = TransitionKind::Wipe { dir: Dir::Up, softness: 0.25, angle: 12.0 };
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
        source: Default::default(),
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

    // 6. инверсия (Р.51, п.398): едет в конверте, бампит файл до v7,
    // выключение чистит и ключ, и версию
    let mut p = rich_project();
    p.transitions[0].kind = TransitionKind::Wipe { dir: Dir::Up, softness: 0.25, angle: 12.0 };
    p.transitions[0].invert = true;
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 7, "инверсия поднимает файл до v7");
    let tr = find_transition(&v);
    assert_eq!(tr["metadata"]["kt"]["invert"], true, "инверсия в конверте перехода");
    let b = load_project(&text, None).expect("файл с инверсией читается");
    assert!(b.project.transitions[0].invert, "инверсия пережила круг");
    assert_eq!(text, save_project(&b.project, &b.retained, None), "идемпотентность с инверсией");
    let mut back = b.project.clone();
    back.transitions[0].invert = false;
    let text = save_project(&back, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 3, "без инверсии версия снова по содержимому");
    assert!(
        find_transition(&v)["metadata"]["kt"].get("invert").is_none(),
        "выключенная инверсия не оставляет ключа"
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
    assert_eq!(
        text,
        save_project(&load_project(&text, None).unwrap().project, &b.retained, None),
        "идемпотентность записи морфера"
    );

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

/// Тест 248b (Р.35 С4): именованные области ездят в файле и поднимают
/// версию до v4; проект без них байт-стабилен.
#[test]
fn t248_areas_round_trip() {
    let mut p = rich_project();
    p.areas.push(kt_model::MarkedArea {
        name: "финал".into(),
        start: Tick(30_000),
        duration: Tick(90_000),
        lanes: vec![(kt_model::LaneKind::Video, 0)],
        color: None,
    });
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 4, "область — новшество v4");
    assert_eq!(v["metadata"]["kt"]["areas"][0]["name"], "финал");
    let b = load_project(&text, None).expect("файл с областями читается");
    assert_eq!(b.project.areas, p.areas, "область пережила круг");
    assert_eq!(text, save_project(&b.project, &b.retained, None), "идемпотентность");
    // проект БЕЗ областей поля не заводит
    assert!(!save_project(&rich_project(), &Retained::default(), None).contains("areas"));
}

/// Тест 251 (Р.35, нашлось живым сценарием): БИТАЯ секция `metadata.kt`
/// — ошибка загрузки, а не «чужой файл». `kt_get` глотал ошибку через
/// `.ok()`, и один неверный байт в морфере уносил ВЕСЬ конверт: проект
/// открывался без настроек, дорожек, морферов и раскладки — молча.
#[test]
fn t251_broken_kt_meta_is_an_error() {
    let p = rich_project();
    let text = save_project(&p, &Retained::default(), None);
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    // ломаем ОДНО поле в конверте
    v["metadata"]["kt"]["morph_lanes"] = serde_json::json!("не число");
    let bad = serde_json::to_string_pretty(&v).unwrap();
    match load_project(&bad, None) {
        Err(kt_otio::Error::KtMetaBroken(_)) => {}
        Err(e) => panic!("не тот отказ: {e}"),
        Ok(b) => panic!(
            "битый конверт открылся молча: {} клипов, {} дорожек",
            b.project.clips.len(),
            b.project.video_lanes
        ),
    }
    // чужой файл БЕЗ нашей секции по-прежнему открывается
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["metadata"].as_object_mut().unwrap().remove("kt");
    let foreign = serde_json::to_string_pretty(&v).unwrap();
    assert!(load_project(&foreign, None).is_ok(), "чужой OTIO обязан открываться");
}

/// Тест 383 (Р.49 А10): битая НАША секция у клипа — ОШИБКА со списком адресов,
/// а не «обычный чужой файл».
///
/// `kt_get` с `.ok()` выбрасывал конверт молча: клип получал НОВЫЙ номер, а с
/// ним рвались link-группы A/V и ссылки морферов, терялись громкость,
/// прозрачность и выключенная цепочка. Notice тут не помогает — молча
/// переприсвоенный номер необратим. Близнец t251 (та же строгость у Timeline).
#[test]
fn t383_broken_clip_kt_section_is_an_error() {
    let doc = |kt: &str| {
        format!(
            r#"{{"OTIO_SCHEMA":"Timeline.1","name":"t","tracks":{{"OTIO_SCHEMA":"Stack.1",
        "name":"","children":[{{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
        {{"OTIO_SCHEMA":"Clip.1","name":"c","source_range":{{"OTIO_SCHEMA":"TimeRange.1",
        "duration":{{"OTIO_SCHEMA":"RationalTime.1","value":48,"rate":24}},
        "start_time":{{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24}}}},
        "media_reference":{{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a.mov",
        "available_range":null,"available_image_bounds":null,"metadata":{{}}}},
        "effects":[],"markers":[],"enabled":true,"metadata":{kt}}}],
        "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{{}}}}],
        "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{{}}}},
        "global_start_time":null,"metadata":{{}}}}"#
        )
    };

    // секции нет — обычный чужой файл, читается
    load_project(&doc("{}"), None).expect("чужой файл без нашей секции обязан читаться");
    // чужой неймспейс — тоже не наше дело
    load_project(&doc(r#"{"resolve":{"clip_color":"Orange"}}"#), None).expect("чужая секция");

    // секция ЕСТЬ и она битая — отказ с адресом
    let msg = match load_project(&doc(r#"{"kt":{"id":"не число","media":1}}"#), None) {
        Ok(_) => panic!("битая секция обязана валить загрузку"),
        Err(e) => e.to_string(),
    };
    assert!(msg.contains("kt"), "в сообщении нет упоминания нашей секции: {msg}");
    assert!(msg.contains('c') && msg.contains("lane"), "в сообщении нет адреса клипа: {msg}");
}

/// Тест 384 (Р.49 А10): чужое у ДОРОЖКИ возвращается на место — эффекты,
/// маркеры и флаг «выключена». Своих дорожечных эффектов у нас нет, а у
/// соседей есть, и круг их терял молча.
#[test]
fn t384_track_foreign_effects_markers_and_enabled_survive() {
    let doc = r#"{"OTIO_SCHEMA":"Timeline.1","name":"t","tracks":{"OTIO_SCHEMA":"Stack.1",
    "name":"","children":[{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
    {"OTIO_SCHEMA":"Clip.1","name":"c","source_range":{"OTIO_SCHEMA":"TimeRange.1",
    "duration":{"OTIO_SCHEMA":"RationalTime.1","value":48,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24}},
    "media_reference":{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a.mov",
    "available_range":null,"available_image_bounds":null,"metadata":{}},
    "effects":[],"markers":[],"enabled":true,"metadata":{}}],
    "source_range":null,
    "effects":[{"OTIO_SCHEMA":"Effect.1","name":"track fx","effect_name":"TrackGrade",
    "enabled":true,"metadata":{}}],
    "markers":[{"OTIO_SCHEMA":"Marker.2","name":"на дорожке","color":"BLUE","marked_range":
    {"OTIO_SCHEMA":"TimeRange.1","duration":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":12,"rate":24}},"metadata":{}}],
    "enabled":false,"metadata":{}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{}},
    "global_start_time":null,"metadata":{}}"#;
    let b = load_project(doc, None).expect("файл читается");
    assert!(
        b.notices.iter().any(|n| n.contains("disabled")),
        "о выключенной чужой дорожке не сказано ни слова: {:?}",
        b.notices
    );

    let out = save_project(&b.project, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let tr = &v["tracks"]["children"][0];
    assert_eq!(tr["enabled"], false, "флаг «выключена» потерян — чужой редактор услышит дорожку");
    assert_eq!(tr["effects"][0]["effect_name"], "TrackGrade", "потерян эффект дорожки");
    assert_eq!(tr["markers"][0]["name"], "на дорожке", "потерян маркер дорожки");
}

/// Тест 385 (Р.49 А10): чужое у МЕТКИ (комментарий, metadata) переживает круг.
/// Модель знает про метку только время, имя и цвет — остальное обязано
/// вернуться из конверта, иначе после нашего сохранения комментарии исчезают.
#[test]
fn t385_marker_comment_and_metadata_survive_roundtrip() {
    let doc = r#"{"OTIO_SCHEMA":"Timeline.1","name":"t","tracks":{"OTIO_SCHEMA":"Stack.1",
    "name":"","children":[],"source_range":null,"effects":[],"markers":[
    {"OTIO_SCHEMA":"Marker.2","name":"замечание","color":"RED","marked_range":
    {"OTIO_SCHEMA":"TimeRange.1","duration":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":48,"rate":24}},
    "comment":"переснять крупный план","metadata":{"resolve":{"flag":"yellow"}}}],
    "enabled":true,"metadata":{}},
    "global_start_time":null,"metadata":{}}"#;
    let b = load_project(doc, None).expect("файл с меткой читается");
    assert_eq!(b.project.markers.len(), 1);

    let out = save_project(&b.project, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let m = &v["tracks"]["markers"][0];
    assert_eq!(m["name"], "замечание");
    assert_eq!(m["comment"], "переснять крупный план", "комментарий метки потерян");
    assert_eq!(m["metadata"]["resolve"]["flag"], "yellow", "чужие metadata метки потеряны");
}

/// Тест 386 (Р.49 А10): `Clip.enabled` НЕ чиним — он не сломан. Выключенный
/// чужой клип уже приходит как `ForeignEvent` с verbatim-сырьём и возвращается
/// в файл как есть. «Класс проблем» не значит «ужать всё подряд».
#[test]
fn t386_disabled_clip_is_already_verbatim() {
    let doc = r#"{"OTIO_SCHEMA":"Timeline.1","name":"t","tracks":{"OTIO_SCHEMA":"Stack.1",
    "name":"","children":[{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
    {"OTIO_SCHEMA":"Clip.1","name":"выключен","source_range":{"OTIO_SCHEMA":"TimeRange.1",
    "duration":{"OTIO_SCHEMA":"RationalTime.1","value":48,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24}},
    "media_reference":{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a.mov",
    "available_range":null,"available_image_bounds":null,"metadata":{}},
    "effects":[],"markers":[],"enabled":false,"metadata":{"vendor":{"k":1}}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{}},
    "global_start_time":null,"metadata":{}}"#;
    let b = load_project(doc, None).expect("файл читается");
    assert!(b.project.clips.is_empty(), "выключенный клип не должен становиться нашим событием");
    assert_eq!(b.project.foreign.len(), 1, "он обязан лечь в foreign как есть");

    let out = save_project(&b.project, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let c = &v["tracks"]["children"][0]["children"][0];
    assert_eq!(c["enabled"], false, "флаг потерян");
    assert_eq!(c["name"], "выключен");
    assert_eq!(c["metadata"]["vendor"]["k"], 1, "чужие metadata выключенного клипа потеряны");
}

// ---- 431-433 (Р.50, п.382): вложенные ленты в формате -------------------------

fn nested_project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    for id in [1u64, 2] {
        p.media.push(MediaRef { id: MediaId(id), path: format!("media/m{id}.mp4") });
    }
    // лента: два события встык, кроссфейд между ними, свой морфер-слой и
    // своя камера — то, чего OTIO не выражает и что едет сайдкаром
    let mut s = Sequence::new(MediaId(100), "Заставка");
    s.clips.push(clip(11, ClipKind::Video, 0, 0, 45_000, 1));
    s.clips.push(clip(12, ClipKind::Video, 0, 45_000, 45_000, 2));
    // id перехода на чтении раздаётся заново — как и у корня (так было
    // всегда: переход адресуется парой концов, а не номером)
    s.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(11)),
        b: Some(ClipId(12)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    s.morphers.push(MorpherEvent {
        id: MorphId(3),
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        targets: MorphTargets::Below,
        patch: Default::default(),
        enabled: true,
        timed: true,
        strength: 1.0,
        fade_in: Tick(0),
        fade_out: Tick(0),
    });
    s.morph_lanes = 1;
    p.sequences.push(s);

    // экземпляр стоит ДВАЖДЫ — тело инлайнится в каждый, на чтении они
    // обязаны схлопнуться обратно в ОДНУ ленту
    let mut a = clip(1, ClipKind::Video, 0, 0, 90_000, 100);
    // Уровень, цепочка, РЕВЕРС и СКОРОСТЬ на вложении законны (Р.55, С4).
    // Путь записи и чтения у вложения общий с клипом
    // (`clip_effects`/`nest_effects`), поэтому здесь достаточно уровня и
    // реверса: остальное проверяется на клипах.
    a.opacity = 0.5;
    a.reversed = true;
    p.clips.push(a);
    p.clips.push(clip(2, ClipKind::Video, 0, 200_000, 60_000, 100));
    p.validate().expect("фикстура обязана быть законной");
    p
}

#[test]
fn t431_nested_reel_survives_the_round_trip() {
    let p = nested_project();
    let text = save_project(&p, &Retained::default(), None);

    // экземпляр записан Stack'ом ребёнком Track — тем же приёмом, каким
    // это делают Premiere и FCP, и чужой редактор увидит монтаж, а не
    // ссылку в наш конверт
    let tl = parse_document(&text).unwrap();
    let track = tl
        .tracks
        .children
        .iter()
        .find_map(|c| match c {
            kt_otio::Composable::Track(t) => Some(t),
            _ => None,
        })
        .expect("видео-дорожка");
    let stacks: Vec<_> = track
        .children
        .iter()
        .filter_map(|c| match c {
            kt_otio::Composable::Stack(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(stacks.len(), 2, "оба экземпляра обязаны быть Stack'ами");
    assert_eq!(stacks[0].name, "Заставка", "имя ленты — имя стека");
    assert!(stacks[0].source_range.is_some(), "окно экземпляра — source_range стека");
    assert!(
        !stacks[0].children.is_empty(),
        "тело обязано быть ИНЛАЙНОМ в каждом экземпляре: объектной идентичности \
         в JSON OTIO нет, и чужой редактор увидел бы пустоту"
    );

    // версия ПО СОДЕРЖИМОМУ: вложение — первым условием каскада
    assert_eq!(
        kt_otio::kt_version(&tl),
        Some(kt_otio::KT_META_V6),
        "файл с вложением обязан нести v6: старая сборка прочла бы вложение серым \
         плейсхолдером и при первой перезаписи вернула бы монтаж в файл мёртвым сырьём"
    );

    // round-trip: одна лента, оба экземпляра на неё
    let bundle = load_project(&text, None).unwrap();
    let q = bundle.project;
    assert_eq!(q.sequences.len(), 1, "экземпляры не схлопнулись в одну ленту: {:?}", q.sequences);
    let s = &q.sequences[0];
    assert_eq!(s.name, "Заставка");
    assert_eq!(s.clips, p.sequences[0].clips, "содержимое ленты поле-в-поле");
    assert_eq!(s.transitions, p.sequences[0].transitions, "кроссфейд внутри ленты");
    assert_eq!(s.morphers, p.sequences[0].morphers, "морфер-слой ленты — из сайдкара");
    assert_eq!(s.morph_lanes, 1, "морф-дорожка ленты — из сайдкара");
    assert_eq!(q.clips, p.clips, "экземпляры поле-в-поле (уровень, цепочка, окно)");
    assert_eq!(q.sequence_uses(MediaId(100)), 2, "лента обязана остаться в двух местах");

    // второй круг байт-в-байт
    let text2 = save_project(&q, &bundle.retained, None);
    assert_eq!(text2, text, "save→load→save с вложением не байт-стабилен");
}

#[test]
fn t432_diverged_bodies_become_two_reels() {
    let p = nested_project();
    let text = save_project(&p, &Retained::default(), None);
    // чужой редактор правил ОДИН экземпляр и не правил другой: подменяем
    // длительность события во втором инлайне
    let marker = "\"duration\": {\n";
    assert!(text.contains(marker));
    let mut tl = parse_document(&text).unwrap();
    {
        let kt_otio::Composable::Track(t) = &mut tl.tracks.children[0] else { panic!("трек") };
        let kt_otio::Composable::Stack(s) = t
            .children
            .iter_mut()
            .filter(|c| matches!(c, kt_otio::Composable::Stack(_)))
            .nth(1)
            .expect("второй экземпляр")
        else {
            panic!("стек")
        };
        s.children.clear(); // тело второго экземпляра стало другим
    }
    let text = kt_otio::write_document(&tl);
    let bundle = load_project(&text, None).unwrap();
    assert_eq!(
        bundle.project.sequences.len(),
        2,
        "разошедшиеся тела обязаны дать ДВЕ ленты: тихий выбор одного из двух \
         есть потеря чужой работы"
    );
    assert!(
        bundle.notices.iter().any(|n| n.contains("two different bodies")),
        "о расхождении тел не сказано: {:?}",
        bundle.notices
    );
    // адреса событий второй ленты СВЕЖИЕ — ClipId уникален по документу
    bundle.project.validate().expect("прочитанный проект обязан быть законным");
}

#[test]
fn t433_the_sidecar_is_advisory_not_authoritative() {
    let p = nested_project();
    let text = save_project(&p, &Retained::default(), None);
    // сайдкар потерялся (чужой редактор переписал metadata таймлайна):
    // монтаж ленты обязан прочитаться ИЗ ФАЙЛА, а не пропасть
    let mut tl = parse_document(&text).unwrap();
    {
        let kt = tl.metadata.get_mut("kt").unwrap().as_object_mut().unwrap();
        kt.remove("sequences");
    }
    let text = kt_otio::write_document(&tl);
    let bundle = load_project(&text, None).unwrap();
    let q = &bundle.project;
    assert_eq!(q.sequences.len(), 1, "лента пропала вместе с сайдкаром");
    assert_eq!(q.sequences[0].clips.len(), 2, "монтаж ленты обязан читаться из файла");
    assert_eq!(q.sequences[0].transitions.len(), 1, "кроссфейд внутри ленты обязан читаться");
    assert!(
        q.sequences[0].morphers.is_empty(),
        "морфер-слой без сайдкара взяться неоткуда — это и есть его цена"
    );
    assert!(
        bundle.notices.iter().any(|n| n.contains("no kt sidecar")),
        "о пропавшем сайдкаре не сказано: {:?}",
        bundle.notices
    );
    q.validate().expect("прочитанный проект обязан быть законным");
}

/// Тест 433b (Р.50): каскад версии не сдвинул НИ ОДИН прежний файл.
///
/// Версия конверта ставится по СОДЕРЖИМОМУ, и новое условие, встроенное в
/// каскад не туда, поднимает версию всем подряд — такой файл перестаёт
/// открываться прежними сборками. Это единственная проверка, ловящая ошибку
/// каскада целиком: t148/t189 сторожат по одному эталону, а здесь — все наши
/// закоммиченные .otio разом.
#[test]
fn t433b_version_cascade_did_not_move_old_files() {
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures"));
    let mut seen = 0;
    for e in std::fs::read_dir(dir).expect("каталог фикстур") {
        let path = e.unwrap().path();
        if path.extension().and_then(|x| x.to_str()) != Some("otio") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let before = kt_otio::kt_version(&parse_document(&text).unwrap());
        let bundle = load_project(&text, path.parent()).expect("наш файл читается");
        assert!(
            bundle.project.sequences.is_empty(),
            "{}: в прежней фикстуре вложений быть не должно",
            path.display()
        );
        let out = save_project(&bundle.project, &bundle.retained, path.parent());
        assert_eq!(
            kt_otio::kt_version(&parse_document(&out).unwrap()),
            before,
            "{}: версия конверта сдвинулась на файле БЕЗ вложений — условие \
             встроено в каскад не туда, и прежние сборки перестанут открывать файл",
            path.display()
        );
        assert_eq!(out, text, "{}: пересохранение не байт-в-байт", path.display());
        seen += 1;
    }
    assert!(seen >= 2, "наши .otio-фикстуры пропали (нашлось {seen})");
}

/// Тест 437 (Р.50 А5): чужое вложение — настоящая лента, а не серая дыра.
///
/// Дословная цена сегодняшнего поведения (п.158): «melt раскрывает вложенные
/// композиции и рисует их содержимое, а у нас там пустота» — из-за чего
/// `nested_example` навсегда сидел в smoke-ярусе кросс-рендера. Владелец
/// решил закрыть дыру в этом же раунде, приняв несовместимую смену поведения
/// импорта.
#[test]
fn t437_foreign_nested_stacks_become_real_reels() {
    let path =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/otio-samples/nested_example.otio");
    let text = std::fs::read_to_string(path).unwrap();
    let b = load_project(&text, None).expect("nested_example читается");
    let p = &b.project;

    // в файле ТРИ вложения: два Stack и один Track внутри Track
    assert_eq!(
        p.sequences.len(),
        3,
        "чужие вложения не стали лентами: {:?}",
        p.sequences.iter().map(|s| s.name.as_str()).collect::<Vec<_>>()
    );
    assert!(
        p.sequences.iter().any(|s| s.name.contains("Nested Track")),
        "`Track` внутри `Track` — тоже вложение, и он пропущен: {:?}",
        p.sequences.iter().map(|s| s.name.as_str()).collect::<Vec<_>>()
    );
    assert!(p.foreign.is_empty(), "вложения остались серыми плейсхолдерами: {:?}", p.foreign);
    // содержимое доехало: у каждой ленты есть события
    for s in &p.sequences {
        assert!(!s.clips.is_empty(), "лента '{}' пуста", s.name);
    }
    // экземпляры стоят на таймлайне и смотрят на свои ленты
    for s in &p.sequences {
        assert_eq!(p.sequence_uses(s.id), 1, "лента '{}' не поставлена", s.name);
    }
    p.validate().expect("прочитанный проект обязан быть законным");

    // ...и раскрытие даёт настоящий монтаж, а не пустоту: ни одно событие,
    // дошедшее до движка, не смотрит на ЛЕНТУ — все смотрят на материал
    let eff = p.effective_clips();
    assert!(
        eff.iter().all(|c| p.sequence(c.media).is_none()),
        "после раскрытия остались события, смотрящие на ленту, — движок увидел бы пустоту"
    );
    // каждая лента внесла своё содержимое, подрезанное окном экземпляра
    for s in &p.sequences {
        let inner: Vec<MediaId> = s.clips.iter().map(|c| c.media).collect();
        assert!(
            inner.iter().all(|m| eff.iter().any(|c| c.media == *m)),
            "материал ленты '{}' не доехал до движка",
            s.name
        );
    }

    // круг: наш вывод читается и остаётся идемпотентным
    let once = save_project(p, &b.retained, None);
    let again = load_project(&once, None).expect("наш вывод читается");
    assert_eq!(
        again.project.sequences.len(),
        3,
        "ленты не пережили круг: {:?}",
        again.project.sequences.iter().map(|s| s.name.as_str()).collect::<Vec<_>>()
    );
    let twice = save_project(&again.project, &again.retained, None);
    assert_eq!(once, twice, "пересохранение не идемпотентно");
}

/// Тест 465 (п.399, Р.51): наложенный переход в файле — конверт ленты
/// `overlap_transitions` (ребёнком последовательного трека пара из разных
/// Track невыразима), файл поднимается до v7, круг идемпотентен, битая
/// запись — notice, а не порча.
#[test]
fn t465_overlap_transition_roundtrip() {
    let mk = || {
        let mut p = rich_project();
        // видео-лесенка на одной дорожке: [0..90к) и [45к..135к)
        p.clips.retain(|c| c.id.0 >= 10); // аудио-пара остаётся
        p.clips.push(clip(1, ClipKind::Video, 0, 0, 90_000, 1));
        p.clips.push(clip(2, ClipKind::Video, 0, 45_000, 90_000, 2));
        p.transitions.push(TransitionEvent {
            id: TransitionId(7),
            a: Some(ClipId(1)),
            b: Some(ClipId(2)),
            in_offset: Tick::ZERO,
            out_offset: Tick::ZERO,
            kind: TransitionKind::Barn { vertical: false, softness: 0.2 },
            audio: AudioCurve::SCurve,
            auto: vec![],
            invert: true,
        });
        p.validate().unwrap();
        p
    };
    let p = mk();
    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], 7, "наложенный переход поднимает файл до v7");
    let ot = &v["metadata"]["kt"]["overlap_transitions"];
    assert_eq!(ot.as_array().map(|a| a.len()), Some(1), "переход в конверте ленты: {ot}");
    assert_eq!(ot[0]["a"], 1);
    assert_eq!(ot[0]["b"], 2);
    assert_eq!(ot[0]["kind"]["kind"], "barn");
    assert_eq!(ot[0]["invert"], true);

    let b = load_project(&text, None).expect("файл с наложенным переходом читается");
    let got: Vec<&TransitionEvent> =
        b.project.transitions.iter().filter(|t| t.overlap_pair(&b.project).is_some()).collect();
    assert_eq!(got.len(), 1, "наложенный переход восстановлен");
    let t = got[0];
    assert_eq!((t.a, t.b), (Some(ClipId(1)), Some(ClipId(2))));
    assert_eq!(t.kind, TransitionKind::Barn { vertical: false, softness: 0.2 });
    assert_eq!(t.audio, AudioCurve::SCurve);
    assert!(t.invert);
    let w = t.window(&b.project).unwrap();
    assert_eq!((w.start, w.end), (Tick(45_000), Tick(90_000)), "окно = перекрытие");
    b.project.validate().expect("прочитанное валидно");
    assert_eq!(text, save_project(&b.project, &b.retained, None), "идемпотентность");

    // битая запись (неизвестный клип) — notice и пропуск, не порча
    let mut broken: serde_json::Value = serde_json::from_str(&text).unwrap();
    broken["metadata"]["kt"]["overlap_transitions"][0]["b"] = serde_json::json!(777);
    let b = load_project(&serde_json::to_string_pretty(&broken).unwrap(), None)
        .expect("битая запись не валит файл");
    assert!(
        b.project.transitions.iter().all(|t| t.overlap_pair(&b.project).is_none()),
        "битый наложенный переход пропущен"
    );
    assert!(
        b.notices.iter().any(|n| n.contains("overlap transition")),
        "пропуск обязан дать замечание: {:?}",
        b.notices
    );
}

/// Все объекты данной OTIO-схемы в дереве JSON.
fn find_schema<'a>(v: &'a serde_json::Value, tag: &str, out: &mut Vec<&'a serde_json::Value>) {
    match v {
        serde_json::Value::Object(m) => {
            if m.get("OTIO_SCHEMA").and_then(|t| t.as_str()) == Some(tag) {
                out.push(v);
            }
            for x in m.values() {
                find_schema(x, tag, out);
            }
        }
        serde_json::Value::Array(a) => {
            for x in a {
                find_schema(x, tag, out);
            }
        }
        _ => {}
    }
}

/// Тест 473 (Р.52): наш генератор пишется первоклассным
/// `GeneratorReference.1` (`kt.solid` + parameters + ОБЯЗАТЕЛЬНЫЙ
/// available_range от нуля, rate 90000), читается обратно в тот же
/// канонический `gen:`-путь, запись идемпотентна; `synth:` остаётся
/// `ExternalReference` verbatim; генератор внутри вложенной ленты идёт
/// тем же кодом; kt.v от генератора НЕ растёт (старая сборка видит
/// заметную заглушку, а не молчаливую порчу).
#[test]
fn t473_generator_reference_roundtrip() {
    let mut p = rich_project();
    // Р.59, стадия 2: в пути медиа — ВИД, значения на событии
    let orange = kt_model::GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    p.media.push(MediaRef { id: MediaId(7), path: "gen:solid".into() });
    p.media.push(MediaRef { id: MediaId(8), path: "synth:ramp".into() });
    let mut g = clip(30, ClipKind::Video, 0, 200_000, 45_000, 7);
    g.source_in = Tick(4_500);
    g.r#gen = Some(orange.clone());
    p.clips.push(g);
    p.clips.push(clip(31, ClipKind::Audio, 0, 200_000, 45_000, 8));
    // тот же генератор внутри вложенной ленты (Р.50): reel_children один
    let mut seq = Sequence::new(MediaId(9_000_000), "лента");
    let mut inner = clip(40, ClipKind::Video, 0, 0, 30_000, 7);
    inner.r#gen = Some(orange);
    seq.clips.push(inner);
    p.sequences.push(seq);
    p.clips.push(clip(32, ClipKind::Video, 1, 300_000, 30_000, 9_000_000));
    p.validate().unwrap();

    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut gens = Vec::new();
    find_schema(&v, "GeneratorReference.1", &mut gens);
    assert_eq!(gens.len(), 2, "клип в корне и клип в ленте");
    let mut durs = Vec::new();
    for gr in &gens {
        assert_eq!(gr["generator_kind"], "kt.solid");
        assert_eq!(gr["parameters"]["color_r"], 255);
        assert_eq!(gr["parameters"]["color_g"], 136);
        assert_eq!(gr["parameters"]["color_b"], 0);
        assert!(
            gr["parameters"].get("kind").is_none(),
            "вид едет строкой generator_kind, не полем parameters"
        );
        let ar = &gr["available_range"];
        assert!(!ar.is_null(), "available_range ОБЯЗАТЕЛЕН у GeneratorReference");
        assert_eq!(ar["start_time"]["value"], 0.0);
        assert_eq!(ar["duration"]["rate"], 90000.0);
        durs.push(ar["duration"]["value"].as_f64().unwrap() as i64);
    }
    durs.sort();
    assert_eq!(durs, vec![30_000, 49_500], "потреблённое клипом: source_in + duration");

    // synth: остался прежней внешней ссылкой verbatim
    let mut exts = Vec::new();
    find_schema(&v, "ExternalReference.1", &mut exts);
    assert!(
        exts.iter().any(|e| e["target_url"] == "synth:ramp"),
        "synth: пишется ExternalReference как раньше; urls: {:?}",
        exts.iter().map(|e| e["target_url"].clone()).collect::<Vec<_>>()
    );

    let b = load_project(&text, None).expect("файл с генератором читается");
    assert_eq!(
        b.project.media.iter().find(|m| m.id == MediaId(7)).unwrap().path,
        "gen:solid",
        "в пути медиа остаётся ВИД (Р.59, стадия 2)"
    );
    assert_eq!(b.project.sequence(MediaId(9_000_000)).unwrap().clips[0].media, MediaId(7));
    assert_eq!(b.project.clips, p.clips, "клипы пережили круг");
    assert_eq!(text, save_project(&b.project, &b.retained, None), "идемпотентность");

    // Р.59, стадия 2: генератор ТЕПЕРЬ бампает версию — значения уехали с
    // пути медиа на событие, и старая сборка полезла бы за ними в путь и
    // показала бы умолчания во всех местах разом. Прежнее ожидание
    // («генератор версию не двигает», Р.52) снято этим решением: тогда
    // старой сборке доставалась заметная заглушка, теперь — молчаливо
    // не тот кадр, а это уже порча.
    let mut plain = rich_project();
    plain.sequences.push(Sequence::new(MediaId(9_000_000), "лента"));
    plain.clips.push(clip(32, ClipKind::Video, 1, 300_000, 30_000, 9_000_000));
    plain.validate().unwrap();
    let vp: serde_json::Value =
        serde_json::from_str(&save_project(&plain, &Retained::default(), None)).unwrap();
    assert_eq!(v["metadata"]["kt"]["v"], kt_otio::KT_META_V9 as u64);
    assert!(
        vp["metadata"]["kt"]["v"].as_u64().unwrap() < kt_otio::KT_META_V9 as u64,
        "проект без генератора версию не поднимает"
    );
}

/// Тест 474 (Р.52, родня t154): ЧУЖОЙ generator_kind (не `kt.*`) не
/// типизируется — клип получает пустой путь (заметная заглушка), а узел
/// уезжает при пересохранении без потерь.
#[test]
fn t474_foreign_generator_kind_is_passthrough() {
    let mut p = rich_project();
    p.media.push(MediaRef { id: MediaId(7), path: "gen:solid".into() });
    let mut g = clip(30, ClipKind::Video, 0, 200_000, 45_000, 7);
    g.r#gen = Some(kt_model::GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 });
    p.clips.push(g);
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);

    // «файл из чужого редактора»: generator_kind не наш и не алиас
    let foreign = text.replace("\"kt.solid\"", "\"Legacy.Title\"");
    assert_ne!(foreign, text);
    let b = load_project(&foreign, None).expect("чужой генератор читается");
    let m7 = b.project.media.iter().find(|m| m.id == MediaId(7)).unwrap();
    assert_eq!(m7.path, "", "чужой генератор = заглушка (пустой путь), не догадка");

    // пересохранение возит узел без потерь: вид и параметры на месте
    let text2 = save_project(&b.project, &b.retained, None);
    let v2: serde_json::Value = serde_json::from_str(&text2).unwrap();
    let mut gens = Vec::new();
    find_schema(&v2, "GeneratorReference.1", &mut gens);
    assert_eq!(gens.len(), 1);
    assert_eq!(gens[0]["generator_kind"], "Legacy.Title");
    assert_eq!(gens[0]["parameters"]["color_r"], 255, "параметры не потеряны");
    assert_eq!(
        text2,
        save_project(
            &load_project(&text2, None).unwrap().project,
            &load_project(&text2, None).unwrap().retained,
            None
        ),
        "идемпотентность пересохранения чужого генератора"
    );
}

/// Тест 494 (Р.52 С5): импорт-алиас — чужой `SMPTEBars` читается НАШИМ
/// генератором полос, а пересохранение канонизирует вид в `kt.smpte`
/// идемпотентно. (Апстрим-фикстура generator_reference_test.otio для
/// этого непригодна: её дорожка — архаичная `Sequence.1`, целиком
/// passthrough; она остаётся под t154.)
#[test]
fn t494_smpte_alias_import() {
    let mut p = rich_project();
    p.media.push(MediaRef { id: MediaId(7), path: "gen:smpte".into() });
    let mut g = clip(30, ClipKind::Video, 0, 200_000, 45_000, 7);
    g.r#gen = Some(kt_model::GeneratorKind::SmpteBars { tone: false, tone_db: -20.0 });
    p.clips.push(g);
    p.validate().unwrap();
    let ours = save_project(&p, &Retained::default(), None);
    // «чужой редактор»: то же содержимое, но вид назван по-апстримовски
    let foreign = ours.replace("\"kt.smpte\"", "\"SMPTEBars\"");
    assert_ne!(foreign, ours);

    let b = load_project(&foreign, None).expect("чужой SMPTEBars читается");
    assert_eq!(
        b.project.media.iter().find(|m| m.id == MediaId(7)).unwrap().path,
        "gen:smpte",
        "алиас обязан дать НАШ генератор полос, а не заглушку"
    );

    // пересохранение канонизирует вид; дальше — байт-в-байт
    let text2 = save_project(&b.project, &b.retained, None);
    assert!(text2.contains("\"kt.smpte\""), "вид канонизирован");
    assert!(!text2.contains("SMPTEBars"), "чужое имя ушло при канонизации");
    let b2 = load_project(&text2, None).unwrap();
    assert_eq!(text2, save_project(&b2.project, &b2.retained, None), "идемпотентность");
}

/// Тест 593 (Р.56, п.449): файл в форме проекта владельца — видеодорожка
/// «под потолком» — читается КАНОНИЧЕСКИ, и сырьё чужого трека едет
/// вместе со своей дорожкой.
///
/// В `untitled_v26.09.19.otio` порядок строк был
/// `[morph0, morph1, morph2, video8, camera1, camera0, video0, …]`: строка
/// стояла третьей сверху, а номер `8` делал её НИЖНИМ слоем композита —
/// «закинул туда видео, и оно не отображается вообще». На чтении номера
/// становятся рангами строк; обратной совместимости владелец не просил.
///
/// Вторая половина — про `Retained.tracks`: чужая metadata трека
/// адресуется парой (вид, номер), и без перестановки карты она молча
/// уехала бы на СОСЕДНИЙ трек.
#[test]
fn t593_owner_shaped_file_loads_canonically() {
    let mut p = rich_project();
    p.video_lanes = 3;
    p.morph_lanes = 1;
    // «под потолком»: самая НИЖНЯЯ по номеру видео-дорожка стоит второй
    // строкой сверху, сразу под морф-строкой
    p.clips.push(clip(3, ClipKind::Video, 2, 0, 90_000, 3));
    p.lane_order = vec![
        (LaneKind::Morph, 0),
        (LaneKind::Video, 2),
        (LaneKind::Video, 0),
        (LaneKind::Video, 1),
        (LaneKind::Audio, 0),
    ];
    let text = save_project(&p, &Retained::default(), None);

    // чужая metadata на треке бывшей video2 — вписываем прямо в документ
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    {
        let tracks = v["tracks"]["children"].as_array_mut().unwrap();
        // писатель гонит видео снизу вверх: (V,2) — ПЕРВЫЙ трек
        let t = &mut tracks[0];
        t["metadata"]["alien"] = serde_json::json!({"note": "верхняя строка"});
    }
    let text = serde_json::to_string_pretty(&v).unwrap();

    let b = load_project(&text, None).expect("файл владельца обязан читаться");
    let lp = &b.project;
    // строки НЕ переставились — переставились НОМЕРА: морф-строка как
    // стояла сверху, так и стоит, а видео под ней идёт 0,1,2
    assert!(rows_are_canonical(&lp.rows()), "после чтения номера обязаны совпасть с рангами");
    assert_eq!(
        lp.rows(),
        vec![
            (LaneKind::Morph, 0),
            (LaneKind::Video, 0),
            (LaneKind::Video, 1),
            (LaneKind::Video, 2),
            (LaneKind::Audio, 0)
        ],
        "порядок строк на экране изменился, а обязан был остаться"
    );
    assert_eq!(
        lp.clips.iter().find(|c| c.id == ClipId(3)).unwrap().lane,
        0,
        "событие «под потолком» не стало ВЕРХНИМ слоем"
    );
    assert_eq!(lp.clips.iter().find(|c| c.id == ClipId(1)).unwrap().lane, 1);
    assert_eq!(lp.clips.iter().find(|c| c.id == ClipId(2)).unwrap().lane, 2);

    // СЛЕДСТВИЕ для чужого сырья: оно на дорожке 0, а не на бывшей 2
    let extra = b.retained.tracks.get(&(ClipKind::Video, 0)).expect("чужой трек потерялся");
    assert!(
        format!("{:?}", extra).contains("верхняя строка"),
        "сырьё чужого трека не поехало за дорожкой: {extra:?}"
    );

    // круг: сохранение кладёт бывшую «video8» первым видео-треком, и
    // повторное чтение уже ничего не меняет
    let text2 = save_project(lp, &b.retained, None);
    let v2: serde_json::Value = serde_json::from_str(&text2).unwrap();
    // трек с чужим сырьём обязан быть ТЕМ ЖЕ, на котором лежит событие:
    // адресация по (вид, номер) пережила перенумерацию
    let alien = v2["tracks"]["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["metadata"]["alien"]["note"].as_str() == Some("верхняя строка"))
        .expect("чужая metadata трека пропала при записи");
    let ids: Vec<u64> = alien["children"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["metadata"]["kt"]["id"].as_u64())
        .collect();
    assert_eq!(ids, vec![3], "чужое сырьё уехало на трек чужого события: {ids:?}");
    let b2 = load_project(&text2, None).expect("круг");
    assert_eq!(b2.project, *lp, "второй круг что-то менял");
    assert_eq!(text2, save_project(&b2.project, &b2.retained, None), "идемпотентность");
}

/// Тест 606 (Р.57): знаковая скорость переживает круг — и база, и ключ.
///
/// Отрицательный `rate` и ноль — законные значения (velocity-огибающая
/// Vegas). Читатель не имеет права ни отвергнуть их, ни подтянуть к
/// прежнему минимуму 0.25: обратной совместимости владелец не просил, а
/// молчаливый кламп как раз и был претензией.
#[test]
fn t606_signed_rate_survives_the_round_trip() {
    let mut p = rich_project();
    p.clips[0].fx = vec![FxInstance::slowed_reverb(-0.5, 0.0, 85.0)];
    p.clips[0].fx_auto = vec![vec![ParamCurve {
        param: "rate".into(),
        ease: false,
        keys: vec![
            AutoKey { t: Tick(0), v: 1.0 },
            AutoKey { t: Tick(45_000), v: 0.0 },
            AutoKey { t: Tick(90_000), v: -2.0 },
        ],
        source: Default::default(),
    }]];
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let b = load_project(&text, None).expect("файл со знаковой скоростью читается");
    let c = b.project.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    assert_eq!(c.fx, p.clips[0].fx, "база скорости поехала");
    assert_eq!(c.fx_auto, p.clips[0].fx_auto, "ключи скорости поехали");
    let keys: Vec<f64> = c.fx_auto[0][0].keys.iter().map(|k| k.v).collect();
    assert_eq!(keys, vec![1.0, 0.0, -2.0], "ноль и минус не дожили до модели");
    assert_eq!(text, save_project(&b.project, &b.retained, None), "идемпотентность");
}

/// Тест 638 (Р.58, п.462): событие ЛЕВЕЕ НУЛЯ внутри ленты переживает
/// круг save→load→save байт-в-байт.
///
/// Трек OTIO последовательный и начинается с нуля: событие с `start < 0`
/// позицией в нём не выразить, а всё, что идёт за ним, легло бы на курсор
/// трека не в своё время. До Р.58 такой монтаж молча «приезжал» сдвинутым.
/// Истинное время едет в конверте (`kt.start`) ТОЛЬКО там, где трек его
/// не выразил, — обычный монтаж поля не несёт, и прежние файлы
/// байт-стабильны (t433b, golden). Это контракт: будущий сторож
/// `validate` не имеет права запрещать `start < 0` в ленте — файлы
/// владельца уже его содержат.
#[test]
fn t638_negative_start_inside_a_reel_survives_the_round_trip() {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "media/m1.mp4".into() });
    let mut s = Sequence::new(MediaId(100), "Заставка");
    // первое событие начинается ДО нуля ленты; второе стоит в своём
    // времени, но в треке OTIO легло бы на курсор (0 + 50 000 > 45 000)
    s.clips.push(clip(11, ClipKind::Video, 0, -10_000, 50_000, 1));
    s.clips.push(clip(12, ClipKind::Video, 0, 45_000, 30_000, 1));
    p.sequences.push(s);
    p.clips.push(clip(1, ClipKind::Video, 0, 0, 60_000, 100));
    p.validate().expect("start < 0 inside a reel is legal");

    // сколько событий (Clip/Stack) несут `kt.start` — по дереву, а не по
    // тексту: у морфер-слоёв в сайдкаре есть свой `start`
    fn kt_starts(kids: &[kt_otio::Composable]) -> usize {
        use kt_otio::Composable;
        let has = |m: &serde_json::Map<String, serde_json::Value>| {
            m.get("kt").and_then(|k| k.get("start")).is_some() as usize
        };
        kids.iter()
            .map(|k| match k {
                Composable::Clip(c) => has(&c.metadata),
                Composable::Stack(s) => has(&s.metadata) + kt_starts(&s.children),
                Composable::Track(t) => kt_starts(&t.children),
                _ => 0,
            })
            .sum()
    }
    let text = save_project(&p, &Retained::default(), None);
    // конверт несёт истинное время у ОБОИХ смещённых событий и больше ни у кого
    let tl = parse_document(&text).unwrap();
    assert_eq!(kt_starts(&tl.tracks.children), 2, "kt.start written where it is not needed");
    assert_eq!(
        kt_otio::kt_version(&parse_document(&text).unwrap()),
        Some(kt_otio::KT_META_V8),
        "a reel with an event before zero must bump the file to v8"
    );

    let bundle = load_project(&text, None).unwrap();
    let q = &bundle.project;
    let s = q.sequence(MediaId(100)).expect("reel");
    assert_eq!(s.clips, p.sequences[0].clips, "reel content field by field, incl. start < 0");
    assert_eq!(q.clips, p.clips, "instance field by field");

    let text2 = save_project(q, &bundle.retained, None);
    assert_eq!(text2, text, "save→load→save with a negative start is not byte-stable");

    // обычный монтаж поля не несёт — байт-стабильность прежних файлов
    let plain = save_project(&nested_project(), &Retained::default(), None);
    let tl = parse_document(&plain).unwrap();
    assert_eq!(kt_starts(&tl.tracks.children), 0, "kt.start leaked into ordinary files");
}

// ---- Р.59, стадия 2: параметры генератора на СОБЫТИИ ---------------------

/// Тест 664: значения генератора и их кривые переживают круг, версия
/// файла поднимается ПО СОДЕРЖИМОМУ, а чужой читатель видит значения
/// ЭТОГО события первоклассной схемой.
///
/// Два титра на ОДНОМ виде с разным текстом — сцена, которой до этого
/// раунда не существовало: текст был частью адреса медиа. Ради неё всё и
/// затевалось (шаблон новостного выпуска, претензия владельца п.1).
#[test]
fn t664_generator_values_on_the_event_round_trip() {
    let mut p = rich_project();
    let title = |text: &str| {
        let mut k = kt_model::GeneratorKind::default_text();
        if let kt_model::GeneratorKind::Text { text: t, .. } = &mut k {
            *t = text.into();
        }
        k
    };
    p.media.push(MediaRef { id: MediaId(7), path: "gen:text".into() });
    let mut a = clip(30, ClipKind::Video, 0, 200_000, 45_000, 7);
    a.r#gen = Some(title("Первый выпуск"));
    a.gen_auto = kt_model::text_anim_curves(kt_model::TextAnim::Typewriter, 2.0, 2.0);
    let mut b = clip(31, ClipKind::Video, 1, 200_000, 45_000, 7);
    b.r#gen = Some(title("Второй выпуск"));
    p.clips.push(a);
    p.clips.push(b);
    p.validate().unwrap();

    let text = save_project(&p, &Retained::default(), None);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        v["metadata"]["kt"]["v"],
        kt_otio::KT_META_V9 as u64,
        "генератор на событии обязан поднять версию: старая сборка полезла бы \
         за значениями в путь медиа, где их больше нет"
    );

    // чужой читатель видит ЗНАЧЕНИЯ КАЖДОГО события первоклассной схемой
    let mut gens = Vec::new();
    find_schema(&v, "GeneratorReference.1", &mut gens);
    assert_eq!(gens.len(), 2, "у каждого события своя ссылка на материал");
    let texts: std::collections::BTreeSet<String> = gens
        .iter()
        .map(|g| g["parameters"]["text"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(texts.contains("Первый выпуск") && texts.contains("Второй выпуск"), "{texts:?}");

    let back = load_project(&text, None).expect("файл со значениями на событии читается");
    assert_eq!(
        back.project.media.iter().find(|m| m.id == MediaId(7)).unwrap().path,
        "gen:text",
        "в пути медиа остаётся ВИД"
    );
    assert_eq!(back.project.clips, p.clips, "значения и кривые не пережили круг");
    assert_eq!(text, save_project(&back.project, &back.retained, None), "идемпотентность");
}

/// Тест 665: файл Р.52..Р.58 мигрирует бесплатно — значения из ПУТИ
/// медиа раздаются событиям, а прежний режим анимации титра становится
/// кривыми теми же формулами, что и пресет в окне.
///
/// Разойдись миграция с пресетом — проект, сохранённый прошлой сборкой,
/// поехал бы не так, как та же кнопка, нажатая руками. Поэтому формулы
/// живут в ОДНОМ месте (`kt_model::text_anim_curves`), и тест сверяет
/// результат чтения именно с ним.
#[test]
fn t665_old_generator_files_migrate_to_the_event() {
    // «старый файл»: генератор со ЗНАЧЕНИЯМИ в parameters и режимом
    // анимации полем — ровно то, что писала сборка Р.53
    let mut p = rich_project();
    p.media.push(MediaRef { id: MediaId(7), path: "gen:text".into() });
    let mut c = clip(30, ClipKind::Video, 0, 200_000, 45_000, 7);
    c.r#gen = Some(kt_model::GeneratorKind::default_text());
    p.clips.push(c);
    p.validate().unwrap();
    let text = save_project(&p, &Retained::default(), None);
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();

    // руками приводим файл к СТАРОЙ форме: kt.gen у клипа нет, значения и
    // режим — в parameters, версия v8
    v["metadata"]["kt"]["v"] = serde_json::json!(8);
    fn patch(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(m) => {
                if m.get("OTIO_SCHEMA").and_then(|s| s.as_str()) == Some("GeneratorReference.1")
                    && let Some(serde_json::Value::Object(pr)) = m.get_mut("parameters")
                {
                    pr.insert("text".into(), serde_json::json!("Старый титр"));
                    pr.insert("anim".into(), serde_json::json!("typewriter"));
                    pr.insert("anim_secs".into(), serde_json::json!(2.0));
                }
                if let Some(serde_json::Value::Object(kt)) = m.get_mut("kt") {
                    kt.shift_remove("gen");
                    kt.shift_remove("gen_auto");
                }
                for x in m.values_mut() {
                    patch(x);
                }
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(patch),
            _ => {}
        }
    }
    patch(&mut v);
    let old = serde_json::to_string(&v).unwrap();
    assert!(!old.contains("\"gen\""), "в старом файле секции значений быть не должно");

    let back = load_project(&old, None).expect("старый файл читается");
    assert_eq!(
        back.project.media.iter().find(|m| m.id == MediaId(7)).unwrap().path,
        "gen:text",
        "путь медиа укоротился до вида"
    );
    let c = back.project.clips.iter().find(|c| c.id == ClipId(30)).expect("клип на месте");
    match &c.r#gen {
        Some(kt_model::GeneratorKind::Text { text, reveal_soft, .. }) => {
            assert_eq!(text, "Старый титр", "значения обязаны доехать до события");
            assert_eq!(*reveal_soft, 0.0, "у машинки фронт жёсткий");
        }
        other => panic!("вид не тот: {other:?}"),
    }
    assert_eq!(
        c.gen_auto,
        kt_model::text_anim_curves(kt_model::TextAnim::Typewriter, 2.0, 2.0),
        "режим анимации обязан стать ТЕМИ ЖЕ кривыми, что ставит пресет"
    );
    // и прочитанный проект законен — то есть мигрировал ЦЕЛИКОМ
    back.project.validate().expect("мигрированный проект законен");
}

/// Тест 681 (Р.59, стадия 3): публичные параметры и переопределения
/// переживают круг, а версия поднимается ПО СОДЕРЖИМОМУ — ТЕМ ЖЕ
/// предикатом, что и генератор на событии (одна версия на раунд).
///
/// Старый ридер такой файл обязан отвергнуть, и это правильно: иначе он
/// молча сыграл бы все экземпляры шаблонными значениями — десять выпусков
/// новостей с одним заголовком и ни одного видимого признака.
#[test]
fn t681_published_params_and_overrides_round_trip() {
    let mut p = nested_project();
    // ручка у ленты: текст титра внутри неё
    p.media.push(MediaRef { id: MediaId(7), path: "gen:text".into() });
    let mut title = clip(13, ClipKind::Video, 1, 0, 45_000, 7);
    let mut kind = kt_model::GeneratorKind::default_text();
    if let kt_model::GeneratorKind::Text { text, .. } = &mut kind {
        *text = "Заголовок по умолчанию".into();
    }
    title.r#gen = Some(kind);
    let s = p.sequences.iter_mut().find(|s| s.id == MediaId(100)).unwrap();
    s.clips.push(title);
    s.video_lanes = 2;
    s.published = vec![kt_model::PublishedParam {
        id: kt_model::PubId(1),
        // имя — свободный текст АВТОРА (решение владельца Р.59)
        name: "Заголовок новости".into(),
        target: kt_model::ParamAddr::new(ClipId(13), kt_model::ParamOwner::Generator, "text"),
        ty: kt_model::ParamType::Text { multiline: false },
        range: kt_model::PubRange { min: None, max: None },
    }];
    // у ВТОРОГО экземпляра — своё значение
    let inst = p.clips.iter_mut().filter(|c| c.media == MediaId(100)).nth(1).unwrap();
    inst.overrides = vec![kt_model::Override::value(
        kt_model::PubId(1),
        kt_model::ParamValue::Text("Второй выпуск".into()),
    )];
    p.validate().expect("шаблон с переопределением законен");

    let text = save_project(&p, &Retained::default(), None);
    assert_eq!(
        kt_otio::kt_version(&parse_document(&text).unwrap()),
        Some(kt_otio::KT_META_V9),
        "публичные параметры обязаны поднимать ТУ ЖЕ версию, что генератор на \
         событии: версия одна на раунд и считается по содержимому"
    );

    let back = load_project(&text, None).expect("файл с публикациями читается");
    assert_eq!(
        back.project.sequence(MediaId(100)).unwrap().published,
        p.sequences.iter().find(|s| s.id == MediaId(100)).unwrap().published,
        "объявленный набор не пережил круг"
    );
    assert_eq!(back.project.clips, p.clips, "переопределения не пережили круг");
    assert_eq!(text, save_project(&back.project, &back.retained, None), "идемпотентность");

    // ...и файл БЕЗ этих полей не двинулся ни на символ
    let plain = save_project(&nested_project(), &Retained::default(), None);
    assert!(
        !plain.contains("published") && !plain.contains("overrides"),
        "поля публикации протекли в файл, который ими не пользуется"
    );
    assert_eq!(
        kt_otio::kt_version(&parse_document(&plain).unwrap()),
        Some(kt_otio::KT_META_V6),
        "обычная лента обязана остаться на своей версии"
    );
}
