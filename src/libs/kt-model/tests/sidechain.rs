//! Test 772 (R.65): the sidechain KEY in the model — address, rejection, cycle.
//!
//! The owner’s decision: the device lives only in the TRACK’s chain. So the model
//! must name a rejection rather than silently accept the impossible: a key on
//! an event, a key on a video track, a key on itself, a key in a loop.

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

/// Scene: two audio tracks (music — 0, speech — 1) and one video.
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

/// Test 772 (R.65): the key is checked, the rejection is named, the model is untouched.
#[test]
fn t772_sidechain_key_is_validated_and_refusals_are_named() {
    let (mut m, music, speech, video) = scene();

    // --- the normal case: music listens to speech ------------------------
    set_track_fx(&mut m, music, vec![comp_keyed(Some(speech))]).expect("a keyframe on speech is legal");
    assert_eq!(
        m.project().track(music).map(|t| kt_model::sidechain_keys(&t.fx).count()),
        Some(1),
        "the keyframe did not reach the model"
    );

    // --- rejections: itself, video, non-existent -----------------------
    let depth = m.undo_depth();
    for (key, what) in [
        (music, "a keyframe on itself"),
        (video, "a keyframe on a VIDEO track"),
        (TrackId(9999), "a keyframe on a nonexistent track"),
    ] {
        let e = set_track_fx(&mut m, music, vec![comp_keyed(Some(key))]).unwrap_err();
        assert!(
            matches!(e, CommandError::SidechainKeyInvalid(_)),
            "{what}: expected a named error, got {e:?}"
        );
    }
    assert_eq!(m.undo_depth(), depth, "a rejected command has no right to touch history");

    // --- CYCLE: speech listens to music, which listens to speech ---------------
    let e = set_track_fx(&mut m, speech, vec![comp_keyed(Some(music))]).unwrap_err();
    assert!(matches!(e, CommandError::SidechainCycle(_)), "the cycle must be rejected: {e:?}");
    // …and a cycle longer than two steps — too
    let (mut m3, a, b, _) = scene();
    let mut p = m3.project().clone();
    p.ensure_tracks(A, 3);
    let c = track_index(&p.tracks, A, 2).map(|i| p.tracks[i].id).unwrap();
    m3 = TimelineModel::new(p);
    set_track_fx(&mut m3, a, vec![comp_keyed(Some(b))]).expect("a listens to b");
    set_track_fx(&mut m3, b, vec![comp_keyed(Some(c))]).expect("b listens to c");
    let e = set_track_fx(&mut m3, c, vec![comp_keyed(Some(a))]).unwrap_err();
    assert!(matches!(e, CommandError::SidechainCycle(_)), "a cycle of three links: {e:?}");

    // --- a key on an EVENT is impossible ------------------------------------
    let e = m
        .apply(Command::SetFx { id: ClipId(1), fx: vec![comp_keyed(Some(speech))] })
        .unwrap_err();
    assert!(
        matches!(e, CommandError::SidechainOnClip),
        "a keyframe on an event must be rejected with a named error: {e:?}"
    );

    // --- deleting a key track does NOT edit other devices’ (rule 20) ---
    let before = m.project().track(music).map(|t| t.fx.clone());
    let lane = m.project().tracks.iter().position(|t| t.id == speech).unwrap();
    let _ = lane;
    m.apply(Command::RemoveTrack { kind: A, at: 1 }).expect("speech track deleted");
    assert_eq!(
        m.project().track(music).map(|t| t.fx.clone()),
        before,
        "deleting the keyframe track rewrote a foreign instrument — that is an edit made on behalf of the user"
    );
    let _ = m.undo();
    assert!(
        m.project().track(speech).is_some(),
        "undo must restore the keyframe track, and the keyframe is live again"
    );
}
