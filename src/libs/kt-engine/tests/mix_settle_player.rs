//! kept-mix-settle: the short loop's start as the output device hears it (kkt-26.11.10 J1, found-defects
//! 390). A test binary of its own: it drives a player.

use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore, Player};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const CH: usize = AUDIO_CHANNELS;
const BURST: i64 = AUDIO_BURST_FRAMES as i64;
const SEC: i64 = 48_000;
/// The device's callback (PipeWire's quantum).
const QUANTUM: usize = 1024;

fn source() -> Box<dyn kt_media::MediaSource> {
    Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 330.0 }))
}

fn project() -> Project {
    kt_plugins_builtin::install();
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:sine"));
    p.clips = vec![ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick::ZERO,
        duration: Tick::from_audio_frames(6 * SEC),
        source_in: Tick::ZERO,
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![FxInstance::plugin_with("slowed_reverb", &[1.0, 0.5, 60.0, 0.4, 0.7, 1.0, 0.0, 0.0, 1.0]).unwrap()],
        opacity: 1.0,
        fx_off: false,
    }];
    p.ensure_tracks(LaneKind::Audio, 1);
    p
}

/// An output device with no clock of its own: while `listen` is set it hands out whole quanta as
/// the ring holds them and keeps what it played; it drops the ring on a flush.
fn device(
    player: &Player,
    mut ring: rtrb::Consumer<f32>,
    listen: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) -> std::thread::JoinHandle<Vec<f32>> {
    let shared = player.shared.clone();
    let transport = player.transport.clone();
    std::thread::spawn(move || {
        let mut heard = Vec::new();
        while !stop.load(Ordering::Acquire) {
            if shared.flush.swap(false, Ordering::AcqRel) {
                while ring.pop().is_ok() {}
            }
            if !listen.load(Ordering::Acquire) || !transport.is_playing() || ring.slots() < QUANTUM * CH {
                std::thread::sleep(std::time::Duration::from_millis(1));
                continue;
            }
            for _ in 0..QUANTUM * CH {
                heard.push(ring.pop().unwrap());
            }
            shared.consumed_frames.fetch_add(QUANTUM as i64, Ordering::AcqRel);
        }
        heard
    })
}

fn wait(what: &str, mut ok: impl FnMut() -> bool) {
    let deadline = kt_time::test_deadline();
    while !ok() {
        assert!(std::time::Instant::now() < deadline, "{what}");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// t2374: a 230 ms loop whose first part the store holds from an earlier play (his rendereg
/// region, J1). The device hears every lap after the first as the continuous play from the
/// loop's start gives its second lap — the kept lap rings the lap before it, not a reset — byte
/// for byte, with no underrun.
#[test]
fn t2374_a_short_loop_start_is_heard_as_the_continuous_play() {
    let p = project();
    let (player, ring) = Player::new(p.clone(), vec![(MediaId(1), source())]);
    let from = SEC + 696;
    let len = 230 * SEC / 1000;
    let (listen, stop) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
    let dev = device(&player, ring, listen.clone(), stop.clone());

    // the earlier play: from 200 ms before the region; its first pass (a ring, 341 ms) keeps the
    // region's first ~140 ms as linear play, and the device takes nothing yet
    player.pause();
    player.seek(Tick::from_audio_frames(from - SEC / 5));
    wait("the earlier seek reached the device", || !player.shared.flush.load(Ordering::Acquire));
    let held = |player: &Player| {
        let a = Tick::from_audio_frames(from).0;
        let b = Tick::from_audio_frames(from + 4 * BURST).0;
        player.final_audio_runs().iter().any(|r| r.start <= a && r.end >= b)
    };
    // the device does not take it: the clock stands, and the start's pass is all that renders
    player.play();
    wait("the earlier play was kept", || held(&player));
    player.pause();
    let kept_end = player.final_audio_runs().iter().map(|r| r.end).max().unwrap();
    assert!(kept_end < Tick::from_audio_frames(from + len).0, "scene guard: only part of the first lap is kept");

    // the loop
    player.set_loop_region(Some(kt_time::TickRange { start: Tick::from_audio_frames(from), end: Tick::from_audio_frames(from + len) }));
    player.seek(Tick::from_audio_frames(from));
    wait("the loop's seek reached the device", || !player.shared.flush.load(Ordering::Acquire));
    let under0 = player.shared.callback_underruns.load(Ordering::Relaxed);
    listen.store(true, Ordering::Release);
    player.play();
    let laps = 8;
    wait("the device heard the laps", || player.transport.wraps() >= laps as u64);
    player.pause();
    stop.store(true, Ordering::Release);
    let heard = dev.join().unwrap();
    assert_eq!(player.shared.callback_underruns.load(Ordering::Relaxed), under0);
    assert_eq!(player.underruns.load(Ordering::Relaxed), 0);

    // the continuous play from a seek to the loop's start, in the player's chunks
    let r = AudioRenderer::new(&p);
    let mut fx = FxStateStore::new();
    r.sync_fx(&mut fx);
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), source());
    let n = (2 * len / BURST + 2) as usize;
    let mut cont = vec![0f32; n * AUDIO_BURST_FRAMES * CH];
    for (i, o) in cont.chunks_exact_mut(AUDIO_BURST_FRAMES * CH).enumerate() {
        r.render_looped_with(&mut f, &mut fx, from..from + len, from + i as i64 * BURST, o);
    }
    let lap = |s: &[f32], k: i64| s[(k * len) as usize * CH..((k + 1) * len) as usize * CH].to_vec();
    assert!(heard.len() as i64 >= (laps as i64 - 1) * len * CH as i64, "heard {} frames", heard.len() / CH);
    assert!(lap(&heard, 1) == lap(&cont, 1), "the second lap is the continuous play's");
    // a lap from the store: the chunk that straddles into the third lap renders live, the rest
    // replays the second
    let head = |k: i64| {
        let a = (k * len) as usize * CH;
        heard[a..a + 480 * CH].iter().map(|v| v * v).sum::<f32>()
    };
    for k in 3..laps as i64 - 1 {
        assert!(lap(&heard, k) == lap(&heard, 3), "lap {k} is the kept lap");
        assert!(head(k) >= 0.5 * head(1), "lap {k} starts hollow: {} against {}", head(k), head(1));
    }
}
