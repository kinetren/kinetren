//! Круг «загрузил → ОТРЕДАКТИРОВАЛ → сохранил → загрузил» (Р.49).
//!
//! Таких тестов у kt-otio не было ни одного: круги проверяли загрузку и
//! запись НЕТРОНУТОГО проекта. Между тем именно правка рождает новые
//! события — правую половину разреза, куски bake, вставку, клипы из бина, —
//! и у них нет конверта, привезённого из файла. Всё, что жило в конверте
//! (таймкод материала, чужие ссылки), для них исчезало молча.

use kt_model::*;
use kt_otio::{load_project, save_project};
use kt_time::Tick;

/// Файл с часом вшитого таймкода: `available_range` начинается не в нуле.
fn doc_with_timecode(src_start: f64, avail_start: f64) -> String {
    format!(
        r#"{{"OTIO_SCHEMA":"Timeline.1","name":"tc","tracks":{{"OTIO_SCHEMA":"Stack.1",
    "name":"","children":[{{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
    {{"OTIO_SCHEMA":"Clip.1","name":"c","source_range":{{"OTIO_SCHEMA":"TimeRange.1",
    "duration":{{"OTIO_SCHEMA":"RationalTime.1","value":480,"rate":24}},
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
}

/// Тест 377 (Р.49 А8): событие, РОЖДЁННОЕ правкой, пишется в тех же
/// координатах материала, что и его сосед из файла.
///
/// Час вшитого таймкода (у Sol Levante — ровно час) — норма вещательных и
/// киношных файлов. `available_range` лежал в конверте КЛИПА, а у
/// новорождённого (правая половина разреза, кусок bake, вставка, событие из
/// бина) конверта нет. Следствие наблюдаемое: две половины ОДНОГО разреза
/// уезжали в файл в РАЗНЫХ системах координат — левая от часа, правая от
/// нуля. Наш круг этого не замечал (мы вычитаем ровно то, что записали), а
/// чужой редактор видел правую половину на 10-й секунде вместо 1:00:10.
///
/// Лечение — конверт МЕДИА: `available_range` описывает ФАЙЛ, а не событие.
#[test]
fn t377_born_clip_keeps_media_start_timecode() {
    let hour = 86_400.0; // час @24 fps
    let b = load_project(&doc_with_timecode(hour + 240.0, hour), None).expect("файл читается");
    let mut m = TimelineModel::new(b.project);
    let src = m.project().clips[0].clone();
    assert_eq!(src.source_in, Tick(240 * 90_000 / 24), "10 c от начала файла");

    // РЕЖЕМ пополам: правая половина — новорождённая, конверта у неё нет
    let mid = src.start + Tick(src.duration.0 / 2);
    let next = m.project().clips.iter().map(|c| c.id.0).max().unwrap() + 1;
    m.apply(Command::Split { id: src.id, at: mid, new_id: ClipId(next), new_group: None })
        .expect("разрез");
    assert_eq!(m.project().clips.len(), 2, "после разреза два события");

    let right = m.project().clips.iter().max_by_key(|c| c.start.0).unwrap().clone();
    assert!(right.source_in > src.source_in, "правая половина входит в материал позже");

    // круг: сохранили и прочитали обратно
    let out = save_project(m.project(), &b.retained, None);
    let back = load_project(&out, None).expect("свой вывод читается");
    let mut got: Vec<_> = back.project.clips.clone();
    got.sort_by_key(|c| c.start.0);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].source_in, src.source_in, "левая половина сдвинулась");
    assert_eq!(
        got[1].source_in, right.source_in,
        "правая половина уехала на стартовый таймкод материала: \
         на часовом таймкоде это час пустоты вместо картинки"
    );

    // главное следствие: в ФАЙЛЕ обе половины лежат в координатах материала
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let tick_of = |n: &serde_json::Value| -> i64 {
        (n["value"].as_f64().unwrap() * 90_000.0 / n["rate"].as_f64().unwrap()).round() as i64
    };
    let hour_ticks = (hour as i64) * 90_000 / 24;
    let kids0 = &v["tracks"]["children"][0]["children"];
    for (i, want) in [src.source_in, right.source_in].iter().enumerate() {
        assert_eq!(
            tick_of(&kids0[i]["source_range"]["start_time"]),
            hour_ticks + want.0,
            "клип {i} записан НЕ в координатах материала: чужой редактор увидит его \
             на час раньше, чем он есть"
        );
    }
    let kids = &v["tracks"]["children"][0]["children"];
    for i in 0..2 {
        // писатель нормализует клип в схему Clip.2 — карта ссылок
        let refs = &kids[i]["media_references"];
        let key = kids[i]["active_media_reference_key"].as_str().expect("нет активного ключа");
        let ar = &refs[key]["available_range"];
        assert!(!ar.is_null(), "клип {i}: available_range потерян");
        assert_eq!(ar["start_time"]["value"].as_f64().unwrap(), hour);
    }
}

/// Тест 378 (Р.49 А8): конверт медиа не перетирает СВОЙ конверт клипа.
/// Чужой редактор мог оставить у одного события особые ссылки (несколько
/// вариантов материала, свой `active_key`); медиа-уровень — запасной путь
/// для новорождённых, а не общий знаменатель для всех.
#[test]
fn t378_media_refs_do_not_override_clip_own_refs() {
    let doc = r#"{"OTIO_SCHEMA":"Timeline.1","name":"t","tracks":{"OTIO_SCHEMA":"Stack.1",
    "name":"","children":[{"OTIO_SCHEMA":"Track.1","name":"V","kind":"Video","children":[
    {"OTIO_SCHEMA":"Clip.1","name":"c1","source_range":{"OTIO_SCHEMA":"TimeRange.1",
    "duration":{"OTIO_SCHEMA":"RationalTime.1","value":48,"rate":24},
    "start_time":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24}},
    "media_references":{"DEFAULT_MEDIA":{"OTIO_SCHEMA":"ExternalReference.1","name":"",
    "target_url":"a.mov","available_range":null,"available_image_bounds":null,"metadata":{}},
    "proxy":{"OTIO_SCHEMA":"ExternalReference.1","name":"","target_url":"a_proxy.mov",
    "available_range":null,"available_image_bounds":null,"metadata":{}}},
    "active_media_reference_key":"proxy",
    "effects":[],"markers":[],"enabled":true,"metadata":{}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{}}],
    "source_range":null,"effects":[],"markers":[],"enabled":true,"metadata":{}},
    "global_start_time":null,"metadata":{}}"#;
    let b = load_project(doc, None).expect("файл с двумя ссылками читается");
    let out = save_project(&b.project, &b.retained, None);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let c = &v["tracks"]["children"][0]["children"][0];
    assert_eq!(c["active_media_reference_key"], "proxy", "подменён активный ключ клипа");
    assert!(
        c["media_references"]["proxy"].is_object() && c["media_references"]["DEFAULT_MEDIA"].is_object(),
        "потеряна одна из ссылок клипа: {}",
        c["media_references"]
    );
}
