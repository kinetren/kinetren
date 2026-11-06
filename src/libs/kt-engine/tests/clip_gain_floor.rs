//! R.72, USABILITY 6: "let the gain go down to mute". The floor of an event's gain is silence,
//! not −60 dB, and every gain above it keeps its factor bit for bit.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, db_to_linear};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick, TickRange};

fn project(clips: Vec<ClipEvent>) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.clips = clips;
    p.normalize();
    p.validate().unwrap();
    p
}

fn clip(id: u64, start: i64, frames: i64, source_in: i64, gain_db: f32) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick::from_audio_frames(start),
        duration: Tick::from_audio_frames(frames),
        source_in: Tick::from_audio_frames(source_in),
        media: MediaId(1),
        gain_db,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn render(p: &Project, frames: i64) -> Vec<f32> {
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Dc { value: 1.0 })),
    );
    let f: &mut dyn AudioFetcher = &mut f;
    let r = AudioRenderer::new(p);
    let mut out = vec![0f32; frames as usize * AUDIO_CHANNELS];
    r.render_frames(f, 0, &mut out);
    out
}

/// Test 1631: the floor gives zeros; −6 dB and 0 dB are the old factor exactly.
#[test]
fn t1631_gain_floor_is_exact_silence() {
    kt_plugins_builtin::install();
    let d = AUDIO_BURST_FRAMES as i64;
    let at = |db: f32| render(&project(vec![clip(1, 0, d, 0, db)]), d);

    assert!(at(EVENT_GAIN_FLOOR_DB).iter().all(|s| *s == 0.0), "the floor must be exact silence");
    assert!(at(EVENT_GAIN_FLOOR_DB - 10.0).iter().all(|s| *s == 0.0), "below the floor too");
    let quiet = at(EVENT_GAIN_FLOOR_DB + 0.1);
    assert_eq!(quiet[0], db_to_linear(EVENT_GAIN_FLOOR_DB + 0.1), "above the floor it is the law");
    assert_eq!(at(0.0)[0], 1.0);
    assert_eq!(at(-6.0)[0], db_to_linear(-6.0), "−6 dB changed its factor");
    assert_eq!(at(6.0)[0], db_to_linear(6.0), "+6 dB changed its factor");

    // a muted instance stays muted whatever the piece inside it adds
    assert_eq!(add_event_gain(EVENT_GAIN_FLOOR_DB, 6.0), EVENT_GAIN_FLOOR_DB);
    assert_eq!(add_event_gain(6.0, EVENT_GAIN_FLOOR_DB), EVENT_GAIN_FLOOR_DB);
    assert_eq!(add_event_gain(-3.0, 1.5), -1.5);
}

/// Test 1638: the sound of a frame hold is silence (S1), and the sound after it goes on.
#[test]
fn t1638_frame_hold_is_silent_and_goes_on() {
    kt_plugins_builtin::install();
    let d = AUDIO_BURST_FRAMES as i64 * 40;
    let c = clip(1, 0, d * 3, 0, 0.0);
    let cut = |w: (i64, i64)| {
        TickRange::new(Tick::from_audio_frames(w.0), Tick::from_audio_frames(w.1))
    };
    let held = frame_hold_piece(&c, cut((d, d * 2)), ClipId(2));
    assert_eq!(held.fx.iter().filter_map(|f| f.warp_rate()).collect::<Vec<_>>(), vec![0.0]);
    let mut rest = clip_slice(&c, cut((d, d * 2)), ClipId(3));
    rest.start = Tick::from_audio_frames(d * 2);
    let left = clip_slice(&c, cut((0, d)), ClipId(1));
    let out = render(&project(vec![left, held, rest]), d * 3);
    let frame = |f: i64| out[f as usize * AUDIO_CHANNELS];
    assert_eq!(frame(d / 2), 1.0, "before the hold the sound is untouched");
    assert_eq!(frame(d + d / 2), 0.0, "the hold must be silent");
    assert_eq!(frame(d * 2 + d / 2), 1.0, "after the hold the sound goes on");
}
