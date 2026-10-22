//! Tests 769-771, 778 (R.65): a LIVE SIDECHAIN on a track.
//!
//! Ducking has stopped being a snapshot of a curve: the music is quieted by a compressor
//! whose detector listens to the speech track. Here the engine is checked — the order of
//! stages, the key window, a dead and a muted key, the cost of the branch.

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

fn rms(v: &[f32], burst: usize) -> f32 {
    let sl = &v[burst * D as usize * CH..(burst + 1) * D as usize * CH];
    (sl.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / sl.len() as f64).sqrt() as f32
}

/// MUSIC SEPARATELY: the mix is linear, so the speech's contribution is subtracted by the same
/// render without the music event. This way “did the ducking shift in time” is measured on the music itself,
/// not on the sum, where the speech is louder.
fn music_only(p: &Project, bursts: i64) -> Vec<f32> {
    let full = stream(p, &mut sources(), bursts);
    let mut sp = p.clone();
    sp.clips.retain(|c| c.lane != 0);
    sp.validate().unwrap();
    let speech = stream(&sp, &mut sources(), bursts);
    full.iter().zip(speech.iter()).map(|(a, b)| a - b).collect()
}

fn ducker(key: Option<TrackId>) -> FxInstance {
    FxInstance::Compressor {
        threshold_db: -30.0,
        ratio: 12.0,
        attack_ms: 3.0,
        release_ms: 10.0,
        makeup_db: 0.0,
        key,
    }
}

/// Scene: the music (track 0) sounds continuously, the speech (track 1) in two
/// “lines” with a pause; on the music a compressor with the key on the speech.
fn scene(key_on_music: bool) -> (Project, TrackId, TrackId) {
    let clips = vec![
        clip(1, 0, 0, 12 * D, 1),  // music: a flat tone the whole run
        clip(2, 1, D, 2 * D, 2),   // speech: line 1
        clip(3, 1, 7 * D, 2 * D, 2), // speech: line 2
    ];
    let mut p = project(clips);
    p.ensure_tracks(A, 2);
    let music = p.track_at(A, 0).unwrap().id;
    let speech = p.track_at(A, 1).unwrap().id;
    if key_on_music {
        p.track_at_mut(A, 0).unwrap().fx = vec![ducker(Some(speech))];
    }
    p.validate().unwrap();
    (p, music, speech)
}

fn sources() -> DirectFetcher {
    direct(&[
        (1, AudioPattern::Dc { value: 0.30 }), // the music is quieter than the threshold on its own
        (2, AudioPattern::Dc { value: 0.90 }), // the speech is loud
    ])
}

/// Test 769 (R.65): the music dips under the speech and returns between.
///
/// The MIX is measured against the same mix WITHOUT the key: the speech is the same in both,
/// so the whole difference is the music. Measuring “the music separately” would be a
/// check of the internals, not of what is heard.
#[test]
fn t769_music_ducks_under_speech_live() {
    let (p, _, _) = scene(true);
    let out = stream(&p, &mut sources(), 12);
    let (p_plain, _, _) = scene(false);
    let plain = stream(&p_plain, &mut sources(), 12);

    // UNDER THE SPEECH: the same speech, but the mix is quieter — the music dipped
    for b in [1usize, 2, 7, 8] {
        assert!(
            rms(&out, b) < rms(&plain, b) * 0.9,
            "burst {b}: the music did not dip under the speech ({} against {})",
            rms(&out, b),
            rms(&plain, b)
        );
    }
    // BETWEEN THE SPEECH: it returned to its own level
    for b in [5usize, 6, 11] {
        assert!(
            (rms(&out, b) - rms(&plain, b)).abs() < rms(&plain, b) * 0.05,
            "burst {b}: between the lines the level must return ({} against {})",
            rms(&out, b),
            rms(&plain, b)
        );
    }
    // and before the first line there is no ducking yet
    assert!((rms(&out, 0) - rms(&plain, 0)).abs() < 1e-6, "ducking started before the speech");
}

/// Test 770 (R.65): the order of stages by key, alignment, determinism.
#[test]
fn t770_stage_order_alignment_and_determinism() {
    // a key BELOW the target by rank (speech — track 1, music — 0): the order
    // must be determined by dependency, not by the row's rank
    let (mut p, _, _speech) = scene(true);
    // the key track has ITS OWN chain — its output is heard by both the mix and the detector
    p.track_at_mut(A, 1).unwrap().fx = vec![FxInstance::gain(-2.0)];
    p.validate().unwrap();
    let a = stream(&p, &mut sources(), 12);

    // the same render with DIFFERENT chunk boundaries — byte-for-byte
    let r = AudioRenderer::new(&p);
    let mut store = FxStateStore::default();
    r.sync_fx(&mut store);
    let mut f = sources();
    let mut b = Vec::new();
    let mut half = vec![0f32; (D as usize / 2) * CH];
    for i in 0..24 {
        r.render_frames_with(&mut f, &mut store, i * (D / 2), &mut half);
        b.extend_from_slice(&half);
    }
    assert_eq!(a, b, "the render depends on chunk boundaries — this is not determinism");

    // LOOKAHEAD at the target: a limiter in the music chain shifts its input window,
    // and the key must arrive at THE SAME window — the ducking does not “drift” in time.
    // We measure the MUSIC SEPARATELY: on the sum the speech is louder and masks the dip.
    let (mut p2, _, sp2) = scene(false);
    p2.track_at_mut(A, 0).unwrap().fx = vec![FxInstance::limiter(), ducker(Some(sp2))];
    p2.validate().unwrap();
    let with_key = music_only(&p2, 12);
    let (mut p3, _, _) = scene(false);
    // the reference — THE SAME chain without the instrument: a compressor with a silent key
    // must be transparent, and with a speech key — quieten
    p3.track_at_mut(A, 0).unwrap().fx = vec![FxInstance::limiter()];
    p3.validate().unwrap();
    let no_key = music_only(&p3, 12);
    for b in [2usize, 8] {
        assert!(
            rms(&with_key, b) < rms(&no_key, b) * 0.5,
            "burst {b}: with lookahead in the target chain the ducking drifted from the speech ({} against {})",
            rms(&with_key, b),
            rms(&no_key, b)
        );
    }
    // …and in the pause the key is silent, and with it sounds exactly the same as without it
    assert!(
        (rms(&with_key, 6) - rms(&no_key, 6)).abs() < rms(&no_key, 6) * 0.05,
        "in a pause the key must be transparent: {} against {}",
        rms(&with_key, 6),
        rms(&no_key, 6)
    );
}

/// Test 771 (R.65): a dead and a muted key.
#[test]
fn t771_dead_and_muted_key() {
    // --- DEAD KEY: there is no track with such an id -------------------------
    let (mut p, _, _) = scene(false);
    p.track_at_mut(A, 0).unwrap().fx = vec![ducker(Some(TrackId(9999)))];
    p.validate().unwrap();
    let dead = stream(&p, &mut sources(), 12);
    assert!(dead.iter().all(|s| s.is_finite()), "the dead key gave non-numbers");

    // …and the instrument works as an ORDINARY compressor: its own input is above the threshold,
    // so the music is compressed in the PAUSE as well (evenly, without ducking)
    let (mut p_self, _, _) = scene(false);
    p_self.track_at_mut(A, 0).unwrap().fx = vec![ducker(None)];
    p_self.validate().unwrap();
    let selfkey = stream(&p_self, &mut sources(), 12);
    for b in [5usize, 6, 11] {
        assert!(
            (rms(&dead, b) - rms(&selfkey, b)).abs() < 1e-6,
            "burst {b}: with a dead key the sound must match the ordinary compressor ({} against {})",
            rms(&dead, b),
            rms(&selfkey, b)
        );
    }

    // --- MUTED KEY: no ducking -------------------------------
    let (mut p2, _, _) = scene(true);
    p2.track_at_mut(A, 1).unwrap().mute = true;
    p2.validate().unwrap();
    let muted = stream(&p2, &mut sources(), 12);
    let lvl = rms(&muted, 0);
    for b in 1..12usize {
        assert!(
            (rms(&muted, b) - lvl).abs() < lvl * 0.05,
            "burst {b}: a muted key must not duck the music ({} against {lvl})",
            rms(&muted, b)
        );
    }
}

/// Test 778 (R.65): the cost of the key branch is measured, not declared.
#[test]
fn t778_key_branch_cost_is_measured() {
    let (p_key, _, _) = scene(true);
    let (p_plain, _, _) = scene(false);
    let run = |p: &Project| {
        let t = std::time::Instant::now();
        let _ = stream(p, &mut sources(), 12);
        t.elapsed().as_secs_f64()
    };
    // warm-up (the first run pulls the decode and allocations)
    let _ = run(&p_key);
    let _ = run(&p_plain);
    let with = (0..3).map(|_| run(&p_key)).fold(f64::MAX, f64::min);
    let without = (0..3).map(|_| run(&p_plain)).fold(f64::MAX, f64::min);
    // the key branch is a second run of the speech track: the ceiling is generous —
    // the claim here is about the ORDER of cost, not about microseconds
    assert!(
        with < without * 4.0 + 0.05,
        "the key branch is too costly: {with:.4} s against {without:.4} s without the key"
    );
}
