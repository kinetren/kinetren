//! Тест 143 (п.143 porting-notes, Р.24): импорт легаси-перекрытий.
//!
//! Аудио-пара без FX → трим до середины наезда + переход тем же окном
//! (байт-эквивалентность звука закреплена t136 на той же формуле);
//! 3+/FX-перекрытия → развод по дорожкам БЕЗ компенсации громкости
//! (решение пользователя п.9 Р.24); видео-перекрытие → демоция позднего на
//! вставленную дорожку ниже (композит байт-идентичен старому тайбрейку).

use kt_model::ops::{UpgradeReport, upgrade_legacy};
use kt_model::*;
use kt_time::Tick;

fn clip(id: u64, kind: ClipKind, lane: u32, start: i64, dur: i64, src: i64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(src),
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

fn raw_project(clips: Vec<ClipEvent>) -> Project {
    // НЕ validate: легаси-перекрытия до апгрейда инвариант нарушают
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    p.clips = clips;
    // тесты t143 закрепляют конверсию наездов в переходы/дорожки — она живёт
    // в режиме БЕЗ переслаивания (Р.28); с переслаиванием импорт ничего не
    // трогает, это проверяет тест 164
    p.overlap_layers = false;
    p
}

#[test]
fn t143_audio_pair_becomes_transition() {
    // A[0,90000), B[60000,150000) — наезд [60000,90000), C = 75000
    let mut p = raw_project(vec![
        clip(1, ClipKind::Audio, 0, 0, 90_000, 0),
        clip(2, ClipKind::Audio, 0, 60_000, 90_000, 7_000),
    ]);
    let rep = upgrade_legacy(&mut p);
    p.validate().expect("после апгрейда проект валиден");
    assert_eq!(rep, UpgradeReport { transitions_created: 1, audio_moved: 0, video_moved: 0 });

    let a = p.clips.iter().find(|c| c.id == ClipId(1)).unwrap();
    let b = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert_eq!(a.start + a.duration, Tick(75_000), "A тримлен до C");
    assert_eq!(b.start, Tick(75_000), "B начинается с C");
    assert_eq!(b.source_in, Tick(7_000 + 15_000), "TrimStart сдвинул источник B");
    let t = &p.transitions[0];
    assert_eq!((t.a, t.b), (Some(ClipId(1)), Some(ClipId(2))));
    assert_eq!((t.in_offset, t.out_offset), (Tick(15_000), Tick(15_000)), "окно == наезд");
    assert_eq!(t.window(&p).unwrap(), kt_time::TickRange::new(Tick(60_000), Tick(90_000)));

    // идемпотентность
    assert!(upgrade_legacy(&mut p).is_empty());
    p.validate().unwrap();
}

#[test]
fn t143_reversed_incoming_keeps_source_in() {
    // у reversed TrimStart source_in НЕ трогает (п.16) — ручка зеркальная
    let mut b = clip(2, ClipKind::Audio, 0, 60_000, 90_000, 5_000);
    b.reversed = true;
    let mut p = raw_project(vec![clip(1, ClipKind::Audio, 0, 0, 90_000, 0), b]);
    upgrade_legacy(&mut p);
    p.validate().unwrap();
    let b = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert_eq!(b.start, Tick(75_000));
    assert_eq!(b.source_in, Tick(5_000), "reversed: источник не сдвинут");
}

#[test]
fn t143_three_overlap_and_fx_pairs_are_split_to_lanes() {
    // три клипа в одной зоне → пара невозможна, двое уезжают ниже
    let mut p = raw_project(vec![
        clip(1, ClipKind::Audio, 0, 0, 90_000, 0),
        clip(2, ClipKind::Audio, 0, 30_000, 90_000, 0),
        clip(3, ClipKind::Audio, 0, 50_000, 90_000, 0),
    ]);
    let rep = upgrade_legacy(&mut p);
    p.validate().expect("развод даёт валидный проект");
    assert_eq!(rep.transitions_created, 0, "тройное перекрытие — не пара");
    assert_eq!(rep.audio_moved, 2);
    // ранний остался на своей дорожке
    assert_eq!(p.clips.iter().find(|c| c.id == ClipId(1)).unwrap().lane, 0);
    // все дорожки различны в зоне конфликта, материал цел
    assert_eq!(p.clips.len(), 3);

    // FX-клип в паре → тоже развод (трим сдвинул бы автоматизацию)
    let mut fxc = clip(2, ClipKind::Audio, 0, 60_000, 90_000, 0);
    fxc.fx = vec![FxInstance::slowed_reverb(0.8, 0.2, 85.0)];
    let mut p = raw_project(vec![clip(1, ClipKind::Audio, 0, 0, 90_000, 0), fxc]);
    let rep = upgrade_legacy(&mut p);
    p.validate().unwrap();
    assert_eq!((rep.transitions_created, rep.audio_moved), (0, 1));
}

#[test]
fn t143_video_overlap_demotes_later_clip_below() {
    // видео: сверху был тот, у кого start меньше (старый тайбрейк) —
    // поздний уезжает на ВСТАВЛЕННУЮ дорожку ниже, существующая нижняя
    // дорожка сдвигается дальше вниз
    let mut p = raw_project(vec![
        clip(1, ClipKind::Video, 0, 0, 90_000, 0),
        clip(2, ClipKind::Video, 0, 60_000, 90_000, 0),
        clip(3, ClipKind::Video, 1, 0, 30_000, 0), // задний план — не мешает
    ]);
    let rep = upgrade_legacy(&mut p);
    p.validate().unwrap();
    assert_eq!(rep.video_moved, 1);
    assert_eq!(p.clips.iter().find(|c| c.id == ClipId(1)).unwrap().lane, 0, "ранний сверху");
    assert_eq!(p.clips.iter().find(|c| c.id == ClipId(2)).unwrap().lane, 1, "поздний ниже");
    assert_eq!(
        p.clips.iter().find(|c| c.id == ClipId(3)).unwrap().lane,
        2,
        "старый фон уехал вниз"
    );
    assert!(p.video_lanes >= 3);
}

/// Тест 164 (п.164 porting-notes, Р.28): импорт старого проекта НИЧЕГО не
/// меняет — «как было, с перекрытиями». Решение пользователя после того, как
/// его настоящий проект (1952 клипа, 76 наездов) разворачивало в ~40
/// видео-дорожек.
#[test]
fn t164_legacy_import_keeps_overlaps_and_lanes() {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    p.video_lanes = 2;
    p.audio_lanes = 2;
    // характерная для реального проекта смесь: A/V-пара с наездом, тройная
    // зона и микро-наезд в один тик
    let mut add = |id: u64, kind: ClipKind, lane: u32, start: i64, dur: i64, group: Option<u64>| {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            id: ClipId(id),
            link_group: group.map(GroupId),
            kind,
            lane,
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
    };
    add(1, ClipKind::Video, 0, 0, 90_000, Some(1));
    add(2, ClipKind::Audio, 0, 0, 90_000, Some(1));
    add(3, ClipKind::Video, 0, 60_000, 90_000, Some(2)); // наезд 30 000
    add(4, ClipKind::Audio, 0, 60_000, 90_000, Some(2));
    add(5, ClipKind::Audio, 0, 80_000, 90_000, None); // тройная зона
    add(6, ClipKind::Video, 1, 0, 45_000, None);
    add(7, ClipKind::Video, 1, 44_999, 45_000, None); // микро-наезд в 1 тик

    let before = p.clips.clone();
    let (lanes_v, lanes_a) = (p.video_lanes, p.audio_lanes);
    let report = kt_model::ops::upgrade_legacy(&mut p);

    assert_eq!(p.clips.len(), before.len(), "ни один клип не пропал");
    for c in &p.clips {
        let was = before.iter().find(|x| x.id == c.id).unwrap();
        assert_eq!(
            (c.start, c.duration, c.source_in, c.lane),
            (was.start, was.duration, was.source_in, was.lane),
            "клип {:?} не тронут импортом",
            c.id
        );
    }
    assert_eq!((p.video_lanes, p.audio_lanes), (lanes_v, lanes_a), "дорожки не размножились");
    assert!(p.transitions.is_empty(), "перекрытия остались перекрытиями, а не переходами");
    assert_eq!(report.transitions_created, 0);
    assert_eq!(report.audio_moved, 0);
    assert_eq!(report.video_moved, 0);
    p.validate().expect("проект с перекрытиями валиден");
}
