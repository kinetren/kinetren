//! Тест 772 (Р.65): КЛЮЧ сайдчейна в модели — адрес, отказ, цикл.
//!
//! Решение владельца: прибор живёт только в цепочке ДОРОЖКИ. Значит модель
//! обязана называть отказ, а не принимать невозможное молча: ключ на
//! событии, ключ на видеодорожку, ключ на себя, ключ по кругу.

use kt_model::LaneKind::{Audio as A, Video as V};
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

fn aclip(id: u64, lane: u32) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane,
        start: Tick(0),
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

/// Сцена: две аудиодорожки (музыка — 0, речь — 1) и одна видео.
fn scene() -> (TimelineModel, TrackId, TrackId, TrackId) {
    let mut p = Project::new(settings30());
    p.media.push(MediaRef::new(MediaId(1), "synth:sine:440"));
    p.ensure_tracks(A, 2);
    p.ensure_tracks(V, 1);
    p.clips.push(aclip(1, 0));
    p.clips.push(aclip(2, 1));
    p.validate().unwrap();
    let music = track_index(&p.tracks, A, 0).map(|i| p.tracks[i].id).unwrap();
    let speech = track_index(&p.tracks, A, 1).map(|i| p.tracks[i].id).unwrap();
    let video = track_index(&p.tracks, V, 0).map(|i| p.tracks[i].id).unwrap();
    (TimelineModel::new(p), music, speech, video)
}

fn comp_keyed(key: Option<TrackId>) -> FxInstance {
    FxInstance::Compressor {
        threshold_db: -24.0,
        ratio: 8.0,
        attack_ms: 5.0,
        release_ms: 120.0,
        makeup_db: 0.0,
        key,
    }
}

fn set_track_fx(m: &mut TimelineModel, id: TrackId, fx: Vec<FxInstance>) -> Result<(), CommandError> {
    m.apply(Command::SetTrackFx { id, fx, bypass: vec![] })
}

/// Тест 772 (Р.65): ключ проверяется, отказ назван, модель не тронута.
#[test]
fn t772_sidechain_key_is_validated_and_refusals_are_named() {
    let (mut m, music, speech, video) = scene();

    // --- нормальный случай: музыка слушает речь ------------------------
    set_track_fx(&mut m, music, vec![comp_keyed(Some(speech))]).expect("ключ на речь законен");
    assert_eq!(
        m.project().track(music).map(|t| kt_model::sidechain_keys(&t.fx).count()),
        Some(1),
        "ключ не доехал до модели"
    );

    // --- отказы: сам себя, видео, несуществующая -----------------------
    let depth = m.undo_depth();
    for (key, what) in [
        (music, "ключ на самого себя"),
        (video, "ключ на ВИДЕО-дорожку"),
        (TrackId(9999), "ключ на несуществующую дорожку"),
    ] {
        let e = set_track_fx(&mut m, music, vec![comp_keyed(Some(key))]).unwrap_err();
        assert!(
            matches!(e, CommandError::SidechainKeyInvalid(_)),
            "{what}: ожидалась названная ошибка, получено {e:?}"
        );
    }
    assert_eq!(m.undo_depth(), depth, "отвергнутая команда не имеет права трогать историю");

    // --- ЦИКЛ: речь слушает музыку, которая слушает речь ---------------
    let e = set_track_fx(&mut m, speech, vec![comp_keyed(Some(music))]).unwrap_err();
    assert!(matches!(e, CommandError::SidechainCycle(_)), "цикл обязан быть отвергнут: {e:?}");
    // …и цикл длиннее двух шагов — тоже
    let (mut m3, a, b, _) = scene();
    let mut p = m3.project().clone();
    p.ensure_tracks(A, 3);
    let c = track_index(&p.tracks, A, 2).map(|i| p.tracks[i].id).unwrap();
    m3 = TimelineModel::new(p);
    set_track_fx(&mut m3, a, vec![comp_keyed(Some(b))]).expect("a слушает b");
    set_track_fx(&mut m3, b, vec![comp_keyed(Some(c))]).expect("b слушает c");
    let e = set_track_fx(&mut m3, c, vec![comp_keyed(Some(a))]).unwrap_err();
    assert!(matches!(e, CommandError::SidechainCycle(_)), "цикл из трёх звеньев: {e:?}");

    // --- ключ на СОБЫТИИ невозможен ------------------------------------
    let e = m
        .apply(Command::SetFx { id: ClipId(1), fx: vec![comp_keyed(Some(speech))] })
        .unwrap_err();
    assert!(
        matches!(e, CommandError::SidechainOnClip),
        "ключ на событии обязан быть отвергнут названной ошибкой: {e:?}"
    );

    // --- удаление дорожки-ключа НЕ правит чужие приборы (правило 20) ---
    let before = m.project().track(music).map(|t| t.fx.clone());
    let lane = m.project().tracks.iter().position(|t| t.id == speech).unwrap();
    let _ = lane;
    m.apply(Command::RemoveTrack { kind: A, at: 1 }).expect("дорожка речи удалена");
    assert_eq!(
        m.project().track(music).map(|t| t.fx.clone()),
        before,
        "удаление ключевой дорожки переписало чужой прибор — это правка за пользователя"
    );
    let _ = m.undo();
    assert!(
        m.project().track(speech).is_some(),
        "undo обязан вернуть дорожку-ключ, и ключ снова живой"
    );
}
