//! Тесты U1 (UI-паритет): маркеры/регион/дорожки, apply_batch/apply_coalesced,
//! операции буфера обмена и региона (docs/porting-notes.md, план UI-паритета).

use kt_model::ops::*;
use kt_model::*;
use kt_time::Tick;

fn base() -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 1280, height: 720, sample_rate: 48000 });
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
        fx: vec![],
    }
}

// ---- round-trip новых полей -------------------------------------------------

#[test]
fn new_fields_round_trip_and_old_json_loads() {
    let mut p = base();
    p.markers = vec![Tick(1000), Tick(5000)];
    p.region = Some(Region { start: Tick(100), end: Tick(50_000) });
    p.loop_enabled = true;
    p.video_lanes = 2;
    p.audio_lanes = 3;
    let json = p.to_json();
    let p2 = Project::from_json(&json).unwrap();
    assert_eq!(p, p2);
    assert_eq!(p2.to_json(), json, "байт-стабильность с новыми полями");

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
        Command::AddMarker { at: Tick(1) },
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
    m.apply(Command::AddMarker { at: Tick(5000) }).unwrap();
    m.apply(Command::AddMarker { at: Tick(1000) }).unwrap();
    assert_eq!(m.project().markers, vec![Tick(1000), Tick(5000)], "сортировка при вставке");
    m.apply(Command::MoveMarker { from: Tick(1000), to: Tick(9000) }).unwrap();
    assert_eq!(m.project().markers, vec![Tick(5000), Tick(9000)]);

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
    assert_eq!(block.len(), 2, "линк-группа целиком");

    let mut next_clip = 100u64;
    let mut next_group = 50u64;
    let (cmds, end1) = paste_block(&block, Tick(200_000), &mut next_clip, &mut next_group);
    let mut m2 = TimelineModel::new(m.project().clone());
    m2.apply_batch(cmds).unwrap();
    let pasted: Vec<&ClipEvent> = m2.project().clips.iter().filter(|c| c.id.0 >= 100).collect();
    assert_eq!(pasted.len(), 2);
    assert_eq!(pasted[0].link_group, pasted[1].link_group, "пара осталась парой");
    assert_eq!(pasted[0].link_group, Some(GroupId(50)), "НОВАЯ группа");
    assert_eq!(end1, Tick(290_000));

    // chain-paste: второй блок встык к концу первого
    let (cmds2, end2) = paste_block(&block, end1, &mut next_clip, &mut next_group);
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
    assert_eq!(block.len(), 3);
    let c10 = block.iter().find(|c| c.id == ClipId(10)).unwrap();
    assert_eq!(c10.start, Tick(0), "rebase к нулю региона");
    assert_eq!(c10.duration, Tick(60_000));
    assert_eq!(c10.source_in, Tick(30_000), "сдвиг источника на срез слева");
    let c12 = block.iter().find(|c| c.id == ClipId(12)).unwrap();
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
    let json = m.project().to_json();
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
