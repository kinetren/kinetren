//! Р.62, change B: дорожка — объект модели со стабильным id.

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
