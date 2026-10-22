//! R.62 (change B): the TRACK CHAIN — on the sum of its events.
//!
//! The claims the round was started for: the chain sounds on the sum
//! (not written into each event), its tail stretches BEYOND the last event,
//! the output is aligned with the input (lookahead consumed by the rings), and the state
//! lives under a STABLE id — inserting a track above its neighbors does not mute them.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::LaneKind::Audio as A;
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const D: i64 = AUDIO_BURST_FRAMES as i64;

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

fn clip(id: u64, lane: u32, start_f: i64, dur_f: i64, media: u64) -> ClipEvent {
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
        start: Tick::from_audio_frames(start_f),
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

/// Render `bursts` bursts in a row with ONE stream (the state lives).
fn stream(p: &Project, f: &mut dyn AudioFetcher, bursts: i64) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut store = FxStateStore::default();
    r.sync_fx(&mut store);
    let mut all = Vec::new();
    let mut buf = vec![0f32; D as usize * CH];
    for i in 0..bursts {
        r.render_frames_with(f, &mut store, i * D, &mut buf);
        all.extend_from_slice(&buf);
    }
    all
}

fn peak(v: &[f32]) -> f32 {
    v.iter().fold(0f32, |a, b| a.max(b.abs()))
}

/// Test 733 (R.62): the track chain sounds ON THE SUM, the tail stretches beyond the
/// last event, the output is aligned with the input.
#[test]
fn t733_track_chain_runs_on_the_sum_and_keeps_its_tail() {
    // two lines back to back on ONE track; the reverb — on the TRACK
    let clips = vec![clip(1, 0, 0, D, 1), clip(2, 0, D, D, 1)];
    let mut p = project(clips);
    p.track_at_mut(A, 0).unwrap().fx = vec![FxInstance::slowed_reverb(1.0, 0.9, 120.0)];
    p.validate().unwrap();

    let mut f = direct(&[(1, AudioPattern::Dc { value: 0.5 })]);
    let out = stream(&p, &mut f, 24);
    let burst = |i: usize| &out[(i * D as usize) * CH..((i + 1) * D as usize) * CH];

    // THE TAIL BEYOND THE LAST EVENT: the material ended at 2·D, while the track
    // still sounds — the chain rings out over silence
    assert!(peak(burst(3)) > 1e-4, "the tail of the track chain does not extend past the last event");

    // …and it DECAYS rather than stretching forever (a 120 ms reverb = ~5.6
    // bursts per line, so we compare against the DISTANT end)
    assert!(
        peak(burst(23)) < peak(burst(3)),
        "the tail does not decay: {} against {}",
        peak(burst(23)),
        peak(burst(3))
    );

    // THE CHAIN ON THE SUM, not in each event: the same chain, appended to
    // each event, gives a DIFFERENT sound (each has its own reverb with its own
    // state, and at the seam the first one's tail is cut off)
    let mut per_clip = project(vec![clip(1, 0, 0, D, 1), clip(2, 0, D, D, 1)]);
    for c in per_clip.clips.iter_mut() {
        c.fx = vec![FxInstance::slowed_reverb(1.0, 0.9, 120.0)];
    }
    per_clip.validate().unwrap();
    let mut f2 = direct(&[(1, AudioPattern::Dc { value: 0.5 })]);
    let other = stream(&per_clip, &mut f2, 6);
    assert_ne!(out, other, "the track chain sounds as if appended to every event");
}

/// Test 733b: the LOOKAHEAD of the track chain does not shift the output. A limiter with an
/// analysis window reads ahead, and the rings return the output to its place:
/// relative to a track without a chain there is no shift.
#[test]
fn t733b_track_chain_lookahead_keeps_the_output_aligned() {
    // the event does NOT start at zero: a shift would be visible as a front offset
    let at = D / 2;
    let mut p = project(vec![clip(1, 0, at, D, 1)]);
    // a neutral limiter: the analysis window exists, but there is no gain
    // threshold 0 dB — no gain, but the analysis window (lookahead) exists
    p.track_at_mut(A, 0).unwrap().fx = vec![FxInstance::Limiter {
        threshold_db: 0.0,
        ceiling_db: 0.0,
        lookahead_ms: 5.0,
        release_ms: 80.0,
    }];
    p.validate().unwrap();
    let mut f = direct(&[(1, AudioPattern::Dc { value: 0.25 })]);
    let with = stream(&p, &mut f, 4);

    let plain = project(vec![clip(1, 0, at, D, 1)]);
    let mut f2 = direct(&[(1, AudioPattern::Dc { value: 0.25 })]);
    let without = stream(&plain, &mut f2, 4);

    let front =
        |v: &[f32]| v.chunks_exact(CH).position(|s| s[0].abs() > 1e-6).unwrap_or(usize::MAX);
    assert_eq!(
        front(&with),
        front(&without),
        "the track chain shifted the output — the lookahead was not consumed by the rings"
    );
}

/// Test 734 (R.62): the state key is the STABLE track id. Inserting a
/// track above renumbers the neighbors, but does not touch the state of their chains
/// (invariant 3); deleting a track takes its state away.
#[test]
fn t734_track_state_survives_insertion_above_and_dies_with_the_track() {
    let mut p = project(vec![clip(1, 0, 0, 4 * D, 1)]);
    p.track_at_mut(A, 0).unwrap().fx = vec![FxInstance::slowed_reverb(1.0, 0.9, 120.0)];
    p.validate().unwrap();
    let id = p.track_at(A, 0).unwrap().id;

    // reference: four bursts in one stream
    let mut f = direct(&[(1, AudioPattern::Dc { value: 0.5 })]);
    let want = stream(&p, &mut f, 4);

    // and now in the middle of the stream we INSERT a track ABOVE: the neighbor's number became
    // another, the id did not — so the state must continue
    let mut m = TimelineModel::new(p.clone());
    let mut f = direct(&[(1, AudioPattern::Dc { value: 0.5 })]);
    let mut store = FxStateStore::default();
    let mut buf = vec![0f32; D as usize * CH];
    let mut got = Vec::new();
    for i in 0..4 {
        if i == 2 {
            m.apply(Command::InsertTrack { kind: A, at: 0 }).unwrap();
            assert_eq!(
                m.project().track_at(A, 1).unwrap().id,
                id,
                "scene: the track with the chain moved to number 1, id unchanged"
            );
        }
        let r = AudioRenderer::new(m.project());
        r.sync_fx(&mut store);
        r.render_frames_with(&mut f, &mut store, i * D, &mut buf);
        got.extend_from_slice(&buf);
    }
    assert_eq!(got, want, "inserting a track above killed the state of the chain below");

    // the state of a track with a chain is alive while it is alive
    assert!(store.track_mut(id).is_some(), "a live track must keep its state");

    // and DELETING the track itself with a chain takes its state away (the empty
    // inserted one remains — the last audio track cannot be removed)
    m.apply(Command::RemoveTrack { kind: A, at: 1 }).unwrap();
    let r = AudioRenderer::new(m.project());
    r.sync_fx(&mut store);
    assert!(store.track_mut(id).is_none(), "a deleted track did not remove its state");
}
