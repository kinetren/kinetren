//! Тест 12 (docs/porting-notes.md): round-trip команд, undo, байт-стабильная
//! сериализация, явные ошибки загрузки.

use kt_model::*;
use kt_time::Tick;

fn sample_project() -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 1920, height: 1080, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "clips/a.mp4".into() });
    p.clips.push(ClipEvent {
        id: ClipId(10),
        link_group: Some(GroupId(1)),
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(900_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        fx: vec![FxInstance::slowed_reverb(0.82, 0.3, 140.0)],
    });
    p.clips.push(ClipEvent {
        id: ClipId(11),
        link_group: Some(GroupId(1)),
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick(900_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: -3.0,
        fx: vec![],
    });
    p
}

#[test]
fn save_load_save_is_byte_stable() {
    let p = sample_project();
    let json1 = p.to_json();
    let p2 = Project::from_json(&json1).unwrap();
    assert_eq!(p, p2);
    let json2 = p2.to_json();
    assert_eq!(json1, json2, "save→load→save должен быть байт-в-байт стабилен");
}

#[test]
fn load_rejects_explicitly() {
    // future version
    let mut p = sample_project();
    p.format_version = 999;
    let s = serde_json::to_string(&p).unwrap();
    assert_eq!(Project::from_json(&s), Err(ProjectError::UnsupportedVersion(999)));

    // unknown media — ошибка, а не молчаливое выбрасывание клипа (дефект p)
    let mut p = sample_project();
    p.clips[0].media = MediaId(777);
    let s = serde_json::to_string(&p).unwrap();
    assert_eq!(Project::from_json(&s), Err(ProjectError::UnknownMedia(ClipId(10), MediaId(777))));

    // non-positive duration
    let mut p = sample_project();
    p.clips[1].duration = Tick(0);
    let s = serde_json::to_string(&p).unwrap();
    assert_eq!(Project::from_json(&s), Err(ProjectError::NonPositiveDuration(ClipId(11))));
}

#[test]
fn unknown_fields_are_tolerated() {
    // forward-compat: лишние поля не валят загрузку
    let mut v: serde_json::Value = serde_json::from_str(&sample_project().to_json()).unwrap();
    v["future_field"] = serde_json::json!({"x": 1});
    v["clips"][0]["future_flag"] = serde_json::json!(true);
    let p = Project::from_json(&v.to_string()).unwrap();
    assert_eq!(p, sample_project());
}

#[test]
fn every_command_round_trips_via_undo() {
    let base = sample_project();
    let commands = vec![
        Command::AddMedia { id: MediaId(2), path: "b.wav".into() },
        Command::RemoveClip { id: ClipId(11) },
        Command::MoveClip { id: ClipId(10), new_start: Tick(1000), new_lane: 1 },
        Command::TrimStart { id: ClipId(10), new_start: Tick(9000) },
        Command::TrimEnd { id: ClipId(10), new_end: Tick(450_000) },
        Command::Split {
            id: ClipId(10),
            at: Tick(450_000),
            new_id: ClipId(20),
            new_group: Some(GroupId(2)),
        },
        Command::SetGain { id: ClipId(10), gain_db: 6.0 },
        Command::SetFx { id: ClipId(10), fx: vec![FxInstance::Gain { db: -1.5 }] },
        Command::LinkClips { ids: vec![ClipId(10), ClipId(11)], group: GroupId(9) },
        Command::UnlinkClip { id: ClipId(10) },
    ];
    for cmd in commands {
        let mut m = TimelineModel::new(base.clone());
        m.apply(cmd.clone()).unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
        assert_ne!(m.project(), &base, "{cmd:?} должна менять модель");
        m.undo().unwrap();
        assert_eq!(m.project(), &base, "undo после {cmd:?} должен вернуть исходное");
        m.redo().unwrap();
        m.undo().unwrap();
        assert_eq!(m.project(), &base);
    }
}

#[test]
fn invalid_command_leaves_no_undo_entry() {
    // Регрессия на дефект q: push undo до валидации создавал no-op записи.
    let mut m = TimelineModel::new(sample_project());
    assert!(m.apply(Command::RemoveClip { id: ClipId(404) }).is_err());
    assert!(
        m.apply(Command::Split {
            id: ClipId(10),
            at: Tick(0), // граница — резать нечего
            new_id: ClipId(21),
            new_group: None,
        })
        .is_err()
    );
    assert_eq!(m.undo_depth(), 0, "проваленные команды не должны попадать в историю");
    assert_eq!(m.undo(), Err(CommandError::NothingToUndo));
}

#[test]
fn linked_group_moves_together() {
    let mut m = TimelineModel::new(sample_project());
    m.apply(Command::MoveClip { id: ClipId(10), new_start: Tick(5000), new_lane: 0 }).unwrap();
    let audio = m.clip(ClipId(11)).unwrap();
    assert_eq!(audio.start, Tick(5000), "A/V-партнёр двигается тем же смещением");
}

#[test]
fn split_semantics() {
    let mut m = TimelineModel::new(sample_project());
    m.apply(Command::TrimStart { id: ClipId(10), new_start: Tick(9000) }).unwrap();
    let before = m.clip(ClipId(10)).unwrap().clone();
    m.apply(Command::Split {
        id: ClipId(10),
        at: Tick(300_000),
        new_id: ClipId(20),
        new_group: Some(GroupId(7)),
    })
    .unwrap();
    let left = m.clip(ClipId(10)).unwrap();
    let right = m.clip(ClipId(20)).unwrap();
    assert_eq!(left.start, before.start);
    assert_eq!(left.start + left.duration, Tick(300_000));
    assert_eq!(right.start, Tick(300_000));
    assert_eq!(right.start + right.duration, before.start + before.duration);
    // source_in правой половины продолжает источник без разрыва
    assert_eq!(right.source_in, before.source_in + (Tick(300_000) - before.start));
    assert_eq!(right.link_group, Some(GroupId(7)));
}

#[test]
fn trim_start_keeps_source_alignment() {
    let mut m = TimelineModel::new(sample_project());
    m.apply(Command::TrimStart { id: ClipId(10), new_start: Tick(90_000) }).unwrap();
    let c = m.clip(ClipId(10)).unwrap();
    assert_eq!(c.start, Tick(90_000));
    assert_eq!(c.duration, Tick(810_000));
    assert_eq!(c.source_in, Tick(90_000), "материал под курсором не должен сдвигаться");
}

#[test]
fn undo_depth_is_capped() {
    let mut m = TimelineModel::new(sample_project());
    for i in 0..(MAX_UNDO + 25) {
        m.apply(Command::SetGain { id: ClipId(10), gain_db: i as f32 }).unwrap();
    }
    assert_eq!(m.undo_depth(), MAX_UNDO);
    let mut n = 0;
    while m.undo().is_ok() {
        n += 1;
    }
    assert_eq!(n, MAX_UNDO);
}
