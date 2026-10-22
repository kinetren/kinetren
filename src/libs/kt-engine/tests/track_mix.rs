//! R.62 (change B): the TRACK stage in audio — level, mute and solo.
//!
//! The main claim is negative: for a track WITHOUT properties the path is unchanged
//! byte-for-byte (t2/t3 and GOLDEN_AUDIO rest on it), because the track multiplier folds with the
//! event gain into one number, and the f32 addition order
//! does not change by a single tick.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, db_to_linear};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::LaneKind::Audio as A;
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;

fn settings() -> ProjectSettings {
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

fn audio_clip(id: u64, lane: u32, dur_f: i64, media: u64) -> ClipEvent {
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
        start: Tick::ZERO,
        duration: Tick::from_audio_frames(dur_f),
        source_in: Tick::ZERO,
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn project(clips: Vec<ClipEvent>) -> Project {
    let mut p = Project::new(settings());
    let mut ids: Vec<MediaId> = clips.iter().map(|c| c.media).collect();
    ids.sort();
    ids.dedup();
    for id in ids {
        p.media.push(MediaRef::new(id, format!("synth:{}", id.0)));
    }
    p.clips = clips;
    p.normalize();
    p.validate().unwrap();
    p
}

fn direct(sources: &[(u64, AudioPattern)]) -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    for &(id, pat) in sources {
        f.add_source(MediaId(id), Box::new(SyntheticSource::new(Tick(90_000 * 60), pat)));
    }
    f
}

fn render(p: &Project, f: &mut dyn AudioFetcher, frames: i64) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut out = vec![0f32; frames as usize * CH];
    let s = r.render_frames(f, 0, &mut out);
    assert_eq!(s.underruns, 0, "underrun with a fully available source");
    out
}

/// Test 732 (R.62, change B): track level, mute and solo.
#[test]
fn t732_track_level_mute_and_solo() {
    let d = AUDIO_BURST_FRAMES as i64;
    let two = || project(vec![audio_clip(1, 0, d, 1), audio_clip(2, 1, d, 2)]);
    let src =
        || direct(&[(1, AudioPattern::Dc { value: 0.25 }), (2, AudioPattern::Dc { value: 0.5 })]);

    // WITHOUT PROPERTIES — the old sum without weights (t2 claims the same)
    let base = render(&two(), &mut src(), d);
    assert_eq!(base[0], 0.75);

    // the track LEVEL is one more multiplier after the weights and the event gain
    let mut p = two();
    p.track_at_mut(A, 0).unwrap().gain_db = -6.0;
    let out = render(&p, &mut src(), d);
    assert_eq!(out[0], 0.25 * db_to_linear(-6.0) + 0.5, "the track level was applied inexactly");

    // …and it MULTIPLIES with the event gain: −6 dB on the track and +6 dB on the
    // event return the original within f32 rounding
    let mut p = two();
    p.track_at_mut(A, 0).unwrap().gain_db = -6.0;
    p.clips[0].gain_db = 6.0;
    let out = render(&p, &mut src(), d);
    assert!((out[0] - 0.75).abs() < 1e-6, "the track’s −6 dB and the event’s +6 dB did not add up: {}", out[0]);

    // MUTE — silence of the TRACK, the neighboring one sounds as before
    let mut p = two();
    p.track_at_mut(A, 0).unwrap().mute = true;
    let out = render(&p, &mut src(), d);
    assert_eq!(out[0], 0.5, "a muted track must be silent");
    let mut p = two();
    p.track_at_mut(A, 1).unwrap().mute = true;
    assert_eq!(render(&p, &mut src(), d)[0], 0.25);

    // SOLO — the set of ITS OWN kind: only the solo tracks sound
    let three =
        || project(vec![audio_clip(1, 0, d, 1), audio_clip(2, 1, d, 2), audio_clip(3, 2, d, 3)]);
    let src3 = || {
        direct(&[
            (1, AudioPattern::Dc { value: 0.25 }),
            (2, AudioPattern::Dc { value: 0.5 }),
            (3, AudioPattern::Dc { value: 0.125 }),
        ])
    };
    assert_eq!(render(&three(), &mut src3(), d)[0], 0.875, "scene: all three sound");
    let mut p = three();
    p.track_at_mut(A, 0).unwrap().solo = true;
    p.track_at_mut(A, 2).unwrap().solo = true;
    let out = render(&p, &mut src3(), d);
    assert_eq!(out[0], 0.375, "in solo exactly two tracks must sound");

    // CANCELLING SOLO restores the PREVIOUS mutes, not “all enabled”
    let mut p = three();
    p.track_at_mut(A, 1).unwrap().mute = true;
    p.track_at_mut(A, 0).unwrap().solo = true;
    assert_eq!(render(&p, &mut src3(), d)[0], 0.25, "in solo only the solo track sounds");
    p.track_at_mut(A, 0).unwrap().solo = false;
    assert_eq!(render(&p, &mut src3(), d)[0], 0.375, "unsoloing brought back the muted track");
}

/// Test 732b: muting is a MULTIPLIER of 0, not the removal of the event from the graph.
/// The event of a muted track is still rendered (its FX state
/// lives on), so un-muting in the middle of a reverb tail continues the tail rather than
/// starting it from a cold start (invariant 3).
#[test]
fn t732b_mute_keeps_the_fx_state_alive() {
    let d = AUDIO_BURST_FRAMES as i64;
    let mut clip = audio_clip(1, 0, 8 * d, 1);
    clip.fx = vec![FxInstance::slowed_reverb(1.0, 0.9, 120.0)];
    let p0 = project(vec![clip]);
    let src = || direct(&[(1, AudioPattern::Dc { value: 0.5 })]);

    // reference: NEVER muted — the tail runs continuously
    let r = AudioRenderer::new(&p0);
    let mut f = src();
    let mut store = kt_engine::FxStateStore::default();
    let mut chunk = vec![0f32; d as usize * CH];
    let mut plain_tail = Vec::new();
    for i in 0..4 {
        r.render_frames_with(&mut f, &mut store, i * d, &mut chunk);
        if i == 3 {
            plain_tail = chunk.clone();
        }
    }

    // and now: two bursts with mute, then mute is lifted. The chain state must be the SAME —
    // so after lifting it sounds exactly the same as without muting
    // (the reverb does not start over).
    let mut muted = p0.clone();
    muted.track_at_mut(A, 0).unwrap().mute = true;
    let rm = AudioRenderer::new(&muted);
    let ro = AudioRenderer::new(&p0);
    let mut f = src();
    let mut store = kt_engine::FxStateStore::default();
    let mut out = vec![0f32; d as usize * CH];
    for i in 0..4 {
        let r = if i < 2 { &rm } else { &ro };
        r.render_frames_with(&mut f, &mut store, i * d, &mut out);
        if i < 2 {
            assert!(out.iter().all(|s| *s == 0.0), "a muted track must be silent");
        }
    }
    assert_eq!(out, plain_tail, "after unmuting the tail started over — the state died");
}
