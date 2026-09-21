//! Тесты U1 (UI-паритет): маркеры/регион/дорожки, apply_batch/apply_coalesced,
//! операции буфера обмена и региона (docs/porting-notes.md, план UI-паритета).

use kt_model::ops::*;
use kt_model::*;
use kt_time::Tick;

fn base() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "a.mp4".into() });
    // A/V-пара 0..90000 + одиночный аудио 90000..180000
    p.clips.push(clip(10, Some(1), ClipKind::Video, 0, 0, 90_000, 0));
    p.clips.push(clip(11, Some(1), ClipKind::Audio, 0, 0, 90_000, 0));
    p.clips.push(clip(12, None, ClipKind::Audio, 0, 90_000, 90_000, 30_000));
    p.normalize();
    p.validate().unwrap();
    p
}

fn clip(
    id: u64,
    group: Option<u64>,
    kind: ClipKind,
    lane: u32,
    start: i64,
    dur: i64,
    src_in: i64,
) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        id: ClipId(id),
        link_group: group.map(GroupId),
        kind,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(src_in),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

// ---- round-trip новых полей -------------------------------------------------

#[test]
fn new_fields_round_trip_and_old_json_loads() {
    let mut p = base();
    p.markers = vec![Marker::at(Tick(1000)), Marker::at(Tick(5000))];
    p.region = Some(Region { start: Tick(100), end: Tick(50_000) });
    p.loop_enabled = true;
    p.video_lanes = 2;
    p.audio_lanes = 3;
    let json = serde_json::to_string(&p).unwrap();
    let p2 = Project::from_json(&json).unwrap();
    assert_eq!(p, p2);

    // «старый» JSON без новых полей: убираем их и грузим
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let obj = v.as_object_mut().unwrap();
    for k in ["markers", "region", "loop_enabled", "video_lanes", "audio_lanes"] {
        obj.remove(k);
    }
    let p3 = Project::from_json(&v.to_string()).unwrap();
    assert_eq!(p3.markers, vec![]);
    assert_eq!(p3.region, None);
    assert!(!p3.loop_enabled);
    assert_eq!((p3.video_lanes, p3.audio_lanes), (1, 1), "нормализовано из клипов");
}

// ---- apply_batch -------------------------------------------------------------

#[test]
fn apply_batch_is_atomic_and_single_undo() {
    let mut m = TimelineModel::new(base());
    let before = m.project().clone();
    // 3-я команда провалится (клип не существует)
    let err = m.apply_batch(vec![
        Command::AddMarker { at: Tick(1), duration: Tick::ZERO },
        Command::SetGain { id: ClipId(10), gain_db: -3.0 },
        Command::RemoveClip { id: ClipId(404) },
    ]);
    assert!(err.is_err());
    assert_eq!(m.project(), &before, "провал любой команды = ноль изменений");
    assert_eq!(m.undo_depth(), 0);

    // успешный батч = одна undo-запись на весь жест
    m.apply_batch(vec![
        Command::RemoveClip { id: ClipId(10) },
        Command::RemoveClip { id: ClipId(11) },
    ])
    .unwrap();
    assert_eq!(m.undo_depth(), 1);
    m.undo().unwrap();
    assert_eq!(m.project(), &before, "один undo возвращает оба клипа");
}

#[test]
fn apply_coalesced_merges_gesture_into_one_undo() {
    let mut m = TimelineModel::new(base());
    let before = m.project().clone();
    for i in 0..20 {
        m.apply_coalesced(Command::SetGain { id: ClipId(12), gain_db: i as f32 }, 77).unwrap();
    }
    assert_eq!(m.undo_depth(), 1, "20 правок одного жеста = 1 undo-запись");
    m.undo().unwrap();
    assert_eq!(m.project(), &before);
    // другой токен = новая запись
    m.apply_coalesced(Command::SetGain { id: ClipId(12), gain_db: 1.0 }, 78).unwrap();
    m.apply_coalesced(Command::SetGain { id: ClipId(12), gain_db: 2.0 }, 79).unwrap();
    assert_eq!(m.undo_depth(), 2);
}

// ---- маркеры / регион / дорожки ----------------------------------------------

#[test]
fn markers_sorted_and_region_survives_undo() {
    let mut m = TimelineModel::new(base());
    m.apply(Command::AddMarker { at: Tick(5000), duration: Tick::ZERO }).unwrap();
    m.apply(Command::AddMarker { at: Tick(1000), duration: Tick::ZERO }).unwrap();
    assert_eq!(
        m.project().markers,
        vec![Marker::at(Tick(1000)), Marker::at(Tick(5000))],
        "сортировка при вставке"
    );
    m.apply(Command::MoveMarker { from: Tick(1000), to: Tick(9000) }).unwrap();
    assert_eq!(m.project().markers, vec![Marker::at(Tick(5000)), Marker::at(Tick(9000))]);

    // регион ставится ПОСЛЕ команд и обязан пережить undo
    m.set_region(Some(Region { start: Tick(0), end: Tick(10) }));
    m.set_loop_enabled(true);
    m.undo().unwrap();
    assert_eq!(m.project().region, Some(Region { start: Tick(0), end: Tick(10) }));
    assert!(m.project().loop_enabled);
    m.redo().unwrap();
    assert_eq!(m.project().region, Some(Region { start: Tick(0), end: Tick(10) }));
}

#[test]
fn insert_lane_shifts_only_same_kind() {
    let mut m = TimelineModel::new(base());
    m.apply(Command::InsertLane { kind: ClipKind::Audio, at: 0 }).unwrap();
    assert_eq!(m.project().audio_lanes, 2);
    assert_eq!(m.project().video_lanes, 1, "видео не тронуто");
    for c in &m.project().clips {
        match c.kind {
            ClipKind::Audio => assert_eq!(c.lane, 1, "аудио-клипы сдвинуты"),
            ClipKind::Video => assert_eq!(c.lane, 0),
        }
    }
    // SetLanes не может осиротить клипы
    assert!(m.apply(Command::SetLanes { video: 1, audio: 1 }).is_err());
}

#[test]
fn remove_media_refuses_while_in_use() {
    let mut m = TimelineModel::new(base());
    assert_eq!(
        m.apply(Command::RemoveMedia { id: MediaId(1) }),
        Err(CommandError::MediaInUse(MediaId(1)))
    );
    let ids: Vec<ClipId> = m.project().clips.iter().map(|c| c.id).collect();
    m.apply_batch(ids.into_iter().map(|id| Command::RemoveClip { id }).collect()).unwrap();
    m.apply(Command::RemoveMedia { id: MediaId(1) }).unwrap();
    assert!(m.project().media.is_empty());
}

// ---- буфер обмена / регион-операции -------------------------------------------

#[test]
fn paste_block_keeps_pairs_and_chains() {
    let m = TimelineModel::new(base());
    let block = copy_group(m.project(), ClipId(10));
    assert_eq!(block.clips.len(), 2, "линк-группа целиком");

    let mut next_clip = 100u64;
    let mut next_group = 50u64;
    let mut next_tr = 500u64;
    let mut next_morph = 1u64;
    let mut next_cam = 1u64;
    let (cmds, end1, _) = paste_block(
        &block,
        Tick(200_000),
        PasteIds {
            clip: &mut next_clip,
            group: &mut next_group,
            transition: &mut next_tr,
            morph: &mut next_morph,
            camera: &mut next_cam,
        },
        &PasteNames::default(),
    );
    let mut m2 = TimelineModel::new(m.project().clone());
    m2.apply_batch(cmds).unwrap();
    let pasted: Vec<&ClipEvent> = m2.project().clips.iter().filter(|c| c.id.0 >= 100).collect();
    assert_eq!(pasted.len(), 2);
    assert_eq!(pasted[0].link_group, pasted[1].link_group, "пара осталась парой");
    assert_eq!(pasted[0].link_group, Some(GroupId(50)), "НОВАЯ группа");
    assert_eq!(end1, Tick(290_000));

    // chain-paste: второй блок встык к концу первого
    let (cmds2, end2, _) = paste_block(
        &block,
        end1,
        PasteIds {
            clip: &mut next_clip,
            group: &mut next_group,
            transition: &mut next_tr,
            morph: &mut next_morph,
            camera: &mut next_cam,
        },
        &PasteNames::default(),
    );
    m2.apply_batch(cmds2).unwrap();
    assert_eq!(end2, Tick(380_000));
    assert!(m2.project().clips.iter().any(|c| c.start == end1), "встык без зазора");
}

#[test]
fn copy_region_crops_and_rebases() {
    let p = base();
    let r = Region { start: Tick(30_000), end: Tick(120_000) };
    let block = copy_region_block(&p, r);
    // клипы 10,11 (0..90000) кропаются справа; клип 12 (90000..180000) — слева
    assert_eq!(block.clips.len(), 3);
    let c10 = block.clips.iter().find(|c| c.id == ClipId(10)).unwrap();
    assert_eq!(c10.start, Tick(0), "rebase к нулю региона");
    assert_eq!(c10.duration, Tick(60_000));
    assert_eq!(c10.source_in, Tick(30_000), "сдвиг источника на срез слева");
    let c12 = block.clips.iter().find(|c| c.id == ClipId(12)).unwrap();
    assert_eq!(c12.start, Tick(60_000));
    assert_eq!(c12.source_in, Tick(30_000), "кроп слева не было — source_in исходный");
}

#[test]
fn punch_out_keeps_source_alignment_and_unlinks() {
    let mut m = TimelineModel::new(base());
    let r = Region { start: Tick(20_000), end: Tick(70_000) };
    let mut next = 100u64;
    let cmds = punch_out_region(m.project(), r, &mut next);
    m.apply_batch(cmds).unwrap();
    let p = m.project();
    // клипы 10/11/  вырезаны, 12 не пересекался — не тронут
    assert!(p.clips.iter().any(|c| c.id == ClipId(12)));
    assert!(!p.clips.iter().any(|c| c.id == ClipId(10)));
    // от каждого из пары — левый и правый остатки
    let remnants: Vec<&ClipEvent> = p.clips.iter().filter(|c| c.id.0 >= 100).collect();
    assert_eq!(remnants.len(), 4);
    for c in &remnants {
        assert_eq!(c.link_group, None, "остатки без линка (семантика старого)");
        if c.start == Tick(70_000) {
            assert_eq!(c.source_in, Tick(70_000), "правый остаток продолжает источник");
            assert_eq!(c.duration, Tick(20_000));
        } else {
            assert_eq!(c.start, Tick(0));
            assert_eq!(c.duration, Tick(20_000));
        }
    }
}

#[test]
fn delete_region_pulls_linked_partners() {
    let mut m = TimelineModel::new(base());
    // регион трогает только клип 12, но и клипы 10/11 (пара) вне региона
    // должны выжить; сдвинем 11 так, чтобы регион пересекал только его
    m.apply(Command::MoveClip { id: ClipId(12), new_start: Tick(500_000), new_lane: 0 }).unwrap();
    let r = Region { start: Tick(10_000), end: Tick(20_000) }; // пересекает 10 и 11
    let cmds = delete_region(m.project(), r);
    m.apply_batch(cmds).unwrap();
    assert_eq!(m.project().clips.len(), 1, "пара удалена целиком, 12 остался");
    assert_eq!(m.project().clips[0].id, ClipId(12));
}

#[test]
fn trim_group_to_cursor() {
    let mut m = TimelineModel::new(base());
    let cmds = trim_group_start_to(m.project(), ClipId(10), Tick(30_000));
    assert_eq!(cmds.len(), 2, "оба члена группы");
    m.apply_batch(cmds).unwrap();
    for id in [10u64, 11] {
        let c = m.clip(ClipId(id)).unwrap();
        assert_eq!(c.start, Tick(30_000));
        assert_eq!(c.source_in, Tick(30_000));
    }
    let cmds = trim_group_end_to(m.project(), ClipId(10), Tick(60_000));
    m.apply_batch(cmds).unwrap();
    assert_eq!(m.clip(ClipId(10)).unwrap().duration, Tick(30_000));
}

/// Тест 24 (модельная часть, п.16): reversed сериализуется, undo/redo
/// работает, старые проекты без поля читаются (default=false).
#[test]
fn t24_reversed_roundtrip_and_undo() {
    let p = base();
    let mut m = TimelineModel::new(p);
    let id = m.project().clips[0].id;
    m.apply(Command::SetReversed { id, reversed: true }).unwrap();
    assert!(m.project().clips[0].reversed);
    // сериализация round-trip
    let json = serde_json::to_string(m.project()).unwrap();
    let p2 = Project::from_json(&json).unwrap();
    assert!(p2.clips[0].reversed);
    // undo/redo
    m.undo().unwrap();
    assert!(!m.project().clips[0].reversed);
    m.redo().unwrap();
    assert!(m.project().clips[0].reversed);
    // проект БЕЗ поля reversed читается (обратная совместимость)
    let stripped = json.replace("\"reversed\": true,", "").replace("\"reversed\":true,", "");
    let p3 = Project::from_json(&stripped).unwrap();
    assert!(!p3.clips[0].reversed);
}

/// П.13: UiState (вьюпорт) сохраняется в проект и читается; старые файлы
/// без секции ui загружаются как None.
#[test]
fn ui_state_round_trip_and_old_json() {
    let mut p = base();
    p.ui = Some(UiState {
        view_start: 12_345.0,
        ticks_per_px: 450.0,
        cursor: 90_000,
        lane_order: vec![],
        ..Default::default()
    });
    let json = serde_json::to_string(&p).unwrap();
    let p2 = Project::from_json(&json).unwrap();
    assert_eq!(p2.ui, p.ui);
    // старый JSON без ui
    let stripped: String = json
        .lines()
        .filter(|l| {
            !l.contains("\"ui\"")
                && !l.contains("view_start")
                && !l.contains("ticks_per_px")
                && !l.contains("\"cursor\"")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let _ = stripped; // структура json вложенная — надёжнее собрать без ui:
    let mut p3 = base();
    p3.ui = None;
    let plain = serde_json::to_string(&p3).unwrap();
    assert!(!plain.contains("\"ui\""), "None не сериализуется");
    assert_eq!(Project::from_json(&plain).unwrap().ui, None);
}

/// Тест 29 (п.6): трим применяется ко ВСЕЙ link-группе (v1) — края пары
/// больше не разъезжаются.
#[test]
fn t29_trim_applies_to_link_group() {
    let p = base(); // пара 10(V)+11(A) 0..90000
    let mut m = TimelineModel::new(p);
    m.apply(Command::TrimStart { id: ClipId(10), new_start: Tick(30_000) }).unwrap();
    let v = m.project().clips.iter().find(|c| c.id == ClipId(10)).unwrap().clone();
    let a = m.project().clips.iter().find(|c| c.id == ClipId(11)).unwrap().clone();
    assert_eq!(v.start, a.start, "партнёр обязан получить тот же срез");
    assert_eq!(v.duration, a.duration);

    m.apply(Command::TrimEnd { id: ClipId(11), new_end: Tick(80_000) }).unwrap();
    let v = m.project().clips.iter().find(|c| c.id == ClipId(10)).unwrap().clone();
    let a = m.project().clips.iter().find(|c| c.id == ClipId(11)).unwrap().clone();
    assert_eq!(v.start + v.duration, a.start + a.duration, "правые края пары должны совпасть");
}

/// Тест 30 (п.6): магнетизация разъехавшейся пары сходится и идемпотентна.
#[test]
fn t30_magnetize_desynced_pair_converges() {
    let mut p = base();
    // разъехавшаяся, но слинкованная пара; сосед 12 убран — сдвинутый
    // клип 11 теперь честно ловил бы ClipOverlap (инвариант Р.24)
    p.clips.retain(|c| c.id != ClipId(12));
    p.clips.iter_mut().find(|c| c.id == ClipId(11)).unwrap().start = Tick(15_000);
    p.normalize();
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);

    let mut ng = 100u64;
    let cmds = kt_model::ops::magnetize_commands(m.project(), ClipId(10), Tick(90_000), &mut ng)
        .expect("партнёр есть");
    assert!(!cmds.is_empty());
    m.apply_batch(cmds).unwrap();
    let v = m.project().clips.iter().find(|c| c.id == ClipId(10)).unwrap().clone();
    let a = m.project().clips.iter().find(|c| c.id == ClipId(11)).unwrap().clone();
    assert_eq!(v.start, Tick(0), "выделенный клип НЕ должен сдвигаться");
    assert_eq!(a.start, v.start, "партнёр подтянут");
    assert_eq!(a.start + a.duration, v.start + v.duration);
    assert_eq!(a.link_group, v.link_group);

    // повторно — no-op
    let again =
        kt_model::ops::magnetize_commands(m.project(), ClipId(10), Tick(90_000), &mut ng).unwrap();
    assert!(again.is_empty(), "уже намагничены = no-op");
}

/// Тест 30 (пп.7-8): реордер/удаление дорожки через батчи undo-able.
#[test]
fn t30_lane_delete_via_batch_is_single_undo() {
    let mut p = base();
    // вторая аудио-дорожка с клипом
    p.audio_lanes = 2;
    p.clips.push(clip(20, None, ClipKind::Audio, 1, 200_000, 90_000, 0));
    p.normalize();
    p.validate().unwrap();
    let mut m = TimelineModel::new(p);
    let before = m.project().clone();

    // удаление дорожки A1 (lane 0): её клипы удаляются, верхние съезжают
    let cmds = vec![
        Command::RemoveClip { id: ClipId(11) },
        Command::MoveClip { id: ClipId(20), new_start: Tick(200_000), new_lane: 0 },
        Command::SetLanes { video: 1, audio: 1 },
    ];
    m.apply_batch(cmds).unwrap();
    assert_eq!(m.project().audio_lanes, 1);
    assert!(m.project().clips.iter().all(|c| c.kind != ClipKind::Audio || c.lane == 0));

    m.undo().unwrap();
    assert_eq!(m.project(), &before, "один undo возвращает всё");
}

/// Тест 89 (п.1 р.10): метки с именами — старый формат (массив чисел)
/// читается, имя переживает round-trip, порядок детерминирован даже при
/// совпадающих тиках, MoveMarker переносит имя.
#[test]
fn t89_named_markers() {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    // старый проект: markers — просто числа
    p.markers = vec![Marker::at(Tick(5000)), Marker::at(Tick(1000))];
    let old_json = r#"{"format_version":1,"settings":{"fps":30,"width":640,"height":360,"sample_rate":48000},"media":[],"clips":[],"markers":[1000,5000]}"#;
    let loaded = Project::from_json(old_json).expect("старый формат должен читаться");
    assert_eq!(loaded.markers, vec![Marker::at(Tick(1000)), Marker::at(Tick(5000))]);
    assert!(loaded.markers.iter().all(|m| m.name.is_empty()));

    // новый формат: имя переживает round-trip и не пишется, когда пусто
    p.markers = vec![Marker::named(Tick(1000), "старт"), Marker::at(Tick(5000))];
    p.validate().unwrap();
    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains("старт"), "имя должно сериализоваться");
    let back = Project::from_json(&json).unwrap();
    assert_eq!(back.markers, p.markers);

    // дубли по времени схлопываются детерминированно (stable-сортировка)
    let mut p2 = p.clone();
    p2.markers = vec![Marker::named(Tick(2000), "первая"), Marker::named(Tick(2000), "вторая")];
    p2.normalize();
    assert_eq!(p2.markers, vec![Marker::named(Tick(2000), "первая")]);

    // MoveMarker переносит имя (раньше терялось)
    let mut m = TimelineModel::new(p.clone());
    m.apply(Command::MoveMarker { from: Tick(1000), to: Tick(3000) }).unwrap();
    let moved = m.project().markers.iter().find(|k| k.t == Tick(3000)).unwrap();
    assert_eq!(moved.name, "старт", "имя должно переехать вместе с меткой");

    // SetMarkerName на несуществующей метке — ошибка
    assert!(m.apply(Command::SetMarkerName { at: Tick(999_999), name: "x".into() }).is_err());
}

/// Тест 90 (п.2 р.10): имена дорожек — round-trip, вставка/удаление
/// сдвигают индексы, перестановка переносит имя вместе с содержимым.
#[test]
fn t90_lane_names() {
    use kt_model::{lane_label, lane_names_insert, lane_names_remove, lane_names_swap};
    let mut names = vec![
        (LaneKind::Video, 0, "Камера".to_string()),
        (LaneKind::Video, 1, "Титры".to_string()),
        (LaneKind::Audio, 0, "Голос".to_string()),
    ];
    // подпись: имя или формула
    assert_eq!(lane_label(&names, LaneKind::Video, 0), "Камера");
    assert_eq!(lane_label(&names, LaneKind::Video, 5), "V6");
    assert_eq!(lane_label(&[], LaneKind::Audio, 2), "A3");

    // вставка дорожки сдвигает те, что ниже
    lane_names_insert(&mut names, LaneKind::Video, 0);
    assert_eq!(lane_label(&names, LaneKind::Video, 1), "Камера");
    assert_eq!(lane_label(&names, LaneKind::Video, 2), "Титры");
    assert_eq!(lane_label(&names, LaneKind::Audio, 0), "Голос", "чужой вид не тронут");

    // удаление убирает своё имя и подтягивает остальные
    lane_names_remove(&mut names, LaneKind::Video, 1);
    assert_eq!(lane_label(&names, LaneKind::Video, 1), "Титры");

    // перестановка переносит имя вместе с содержимым
    lane_names_swap(&mut names, LaneKind::Video, 1, 3);
    assert_eq!(lane_label(&names, LaneKind::Video, 3), "Титры");
    assert_eq!(lane_label(&names, LaneKind::Video, 1), "V2");

    // ДАЛЬНЯЯ перестановка (п.5 р.16): имена едут по той же формуле
    // сдвига, что и клипы в reorder_lane, — иначе подпись приклеится к
    // чужой дорожке
    let mut rot = vec![
        (LaneKind::Audio, 0, "Голос".to_string()),
        (LaneKind::Audio, 1, "Музыка".to_string()),
        (LaneKind::Audio, 2, "Шумы".to_string()),
    ];
    kt_model::lane_names_rotate(&mut rot, LaneKind::Audio, 0, 2);
    assert_eq!(lane_label(&rot, LaneKind::Audio, 2), "Голос");
    assert_eq!(lane_label(&rot, LaneKind::Audio, 0), "Музыка");
    assert_eq!(lane_label(&rot, LaneKind::Audio, 1), "Шумы");
    // и обратно — состояние восстанавливается
    kt_model::lane_names_rotate(&mut rot, LaneKind::Audio, 2, 0);
    assert_eq!(lane_label(&rot, LaneKind::Audio, 0), "Голос");
    assert_eq!(lane_label(&rot, LaneKind::Audio, 1), "Музыка");
    assert_eq!(lane_label(&rot, LaneKind::Audio, 2), "Шумы");

    // round-trip через проект: пустой вектор не сериализуется
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.ui = Some(UiState { lane_names: names.clone(), ..Default::default() });
    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains("Титры"));
    assert_eq!(Project::from_json(&json).unwrap().ui.unwrap().lane_names, names);
    let empty = UiState::default();
    assert!(!serde_json::to_string(&empty).unwrap().contains("lane_names"));
}

/// Тест 141 (п.141 porting-notes, Р.24): метки-диапазоны (marked_range).
#[test]
fn t141_range_markers() {
    let mut m = TimelineModel::new(base());
    // диапазонная метка + точечная
    m.apply(Command::AddMarker { at: Tick(1_000), duration: Tick(20_000) }).unwrap();
    m.apply(Command::AddMarker { at: Tick(50_000), duration: Tick::ZERO }).unwrap();
    let mk = &m.project().markers[0];
    assert!(mk.is_range());
    assert_eq!(mk.end(), Tick(21_000));
    assert!(!m.project().markers[1].is_range());

    // длительность: команда, отрицательная — ошибка, чужая — не найдена
    m.apply(Command::SetMarkerDuration { at: Tick(50_000), duration: Tick(5_000) }).unwrap();
    assert_eq!(m.project().markers[1].duration, Tick(5_000));
    assert!(matches!(
        m.apply(Command::SetMarkerDuration { at: Tick(50_000), duration: Tick(-1) }),
        Err(CommandError::TooShort)
    ));
    assert!(matches!(
        m.apply(Command::SetMarkerDuration { at: Tick(777), duration: Tick(1) }),
        Err(CommandError::MarkerNotFound(_))
    ));

    // перенос везёт длительность и цвет
    let mut p = m.project().clone();
    p.markers[0].color = Some("RED".into());
    let mut m = TimelineModel::new(p);
    m.apply(Command::MoveMarker { from: Tick(1_000), to: Tick(9_000) }).unwrap();
    let moved = m.project().markers.iter().find(|k| k.t == Tick(9_000)).unwrap();
    assert_eq!(moved.duration, Tick(20_000), "duration переезжает");
    assert_eq!(moved.color.as_deref(), Some("RED"), "цвет переезжает");

    // интерим-serde: диапазон/цвет в round-trip, легаси-формы читаются
    let json = serde_json::to_string(m.project()).unwrap();
    let p2 = Project::from_json(&json).unwrap();
    assert_eq!(p2.markers, m.project().markers);
    let legacy = r#"{"format_version":1,
        "settings":{"fps":30,"width":64,"height":36,"sample_rate":48000},
        "media":[],"clips":[],
        "markers":[700, {"t":1000,"name":"x"}, {"t":2000,"duration":300,"name":"r"}]}"#;
    let p3 = Project::from_json(legacy).unwrap();
    assert_eq!(p3.markers.len(), 3);
    assert_eq!(p3.markers[0], Marker::at(Tick(700)));
    assert_eq!(p3.markers[1], Marker::named(Tick(1000), "x"));
    assert!(p3.markers[2].is_range() && p3.markers[2].duration == Tick(300));
}
