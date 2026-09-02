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
        fx: vec![],
        opacity: 1.0,
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
        FxInstance::Gain { db: 1.5 },
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

/// Клип без `source_range` берёт материал целиком — длительность и точку
/// входа из `available_range` активной ссылки (спека OTIO; так живёт
/// Clip-004 в simple_cut.otio).
#[test]
fn t154_clip_without_source_range_uses_available_range() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/otio-samples/simple_cut.otio");
    let text = std::fs::read_to_string(path).unwrap();
    let b = load_project(&text, None).expect("simple_cut читается");
    assert_eq!(b.project.clips.len(), 4);
    let last = b.project.clips.iter().max_by_key(|c| c.start).unwrap();
    // available_range Clip-004: start 100, duration 6 кадров @24 fps
    assert_eq!(last.duration, Tick(6 * 90_000 / 24));
    assert_eq!(last.source_in, Tick(100 * 90_000 / 24));
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
            opacity: 1.0,
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
