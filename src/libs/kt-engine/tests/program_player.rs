//! loudness-norms (audio-render): the program stage in the player. A test binary of its own: it reads the
//! process-wide work counters.

use kt_engine::export::render_raw_audio_range;
use kt_engine::{Player, program};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::work::{Work, count};
use kt_time::{AUDIO_CHANNELS, Tick};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const CH: usize = AUDIO_CHANNELS;
const SEC: i64 = 48_000;
/// The device's callback (PipeWire's quantum).
const QUANTUM: usize = 1024;

fn source() -> Box<dyn kt_media::MediaSource> {
    Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 330.0 }))
}

fn project(program: Option<ProgramStage>) -> Project {
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
        fx_in: Vec::new(),
    }];
    p.ensure_tracks(LaneKind::Audio, 1);
    p.program = program;
    p
}

/// An output device with no clock of its own: while `listen` is set it hands out whole quanta as
/// the ring holds them and keeps what it played; it drops the ring on a flush.
fn device(
    player: &Player,
    mut ring: rtrb::Consumer<f32>,
    listen: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    paced: Option<i64>,
) -> std::thread::JoinHandle<Vec<f32>> {
    let shared = player.shared.clone();
    let transport = player.transport.clone();
    let cache = player.cache.clone();
    // it has no clock of its own: it waits for the decode of what the render thread reads next
    // (by what it heard, not by the playhead: that moves once a pass, however long the pass)
    let decoded = move |head: i64| {
        let Some(from) = paced else { return true };
        let at = (from + head) / 1024;
        let have = cache.coverage(kt_cache::SourceId(1), kt_cache::BurstKind::Audio, at..at + 64);
        have.iter().map(|r| r.end - r.start).sum::<i64>() == 64
    };
    std::thread::spawn(move || {
        let mut heard = Vec::new();
        while !stop.load(Ordering::Acquire) {
            if shared.flush.swap(false, Ordering::AcqRel) {
                while ring.pop().is_ok() {}
            }
            if !listen.load(Ordering::Acquire) || !transport.is_playing() || ring.slots() < QUANTUM * CH || !decoded((heard.len() / CH) as i64) {
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

/// Plays from `from` (a loop — `to`) until the device heard `n` frames.
fn hear(player: &Player, ring: rtrb::Consumer<f32>, from: i64, n: usize) -> Vec<f32> {
    let (listen, stop) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
    let dev = device(player, ring, listen.clone(), stop.clone(), Some(from));
    player.pause();
    player.seek(Tick::from_audio_frames(from));
    wait("the seek reached the device", || !player.shared.flush.load(Ordering::Acquire));
    listen.store(true, Ordering::Release);
    player.play();
    wait("the device heard the range", || player.transport.playhead_frames() >= from + n as i64);
    player.pause();
    stop.store(true, Ordering::Release);
    let heard = dev.join().unwrap();
    heard[..n * CH].to_vec()
}

/// t3919b: the forward play and the export from the same start are byte-equal through a stage whose
/// limiter acts; a stage under its ceiling gives the play without one, sample for sample.
#[test]
fn t3919b_the_play_and_the_export_are_one_sound() {
    let from = SEC + 696;
    let n = 2 * SEC as usize;
    let range = Some((Tick::from_audio_frames(from), Tick::from_audio_frames(from + n as i64)));
    for gain in [12.0, 0.0] {
        let p = project(Some(ProgramStage { gain_db: gain, measured: None }));
        let (player, ring) = Player::new(p.clone(), vec![(MediaId(1), source())]);
        let heard = hear(&player, ring, from, n);
        assert_eq!(player.underruns.load(Ordering::Relaxed), 0);
        let mut export = Vec::new();
        let off = Arc::new(AtomicBool::new(false));
        let s = render_raw_audio_range(&p, vec![(MediaId(1), source())], range, program::params(&p), &off, |_| {}, |c| {
            export.extend_from_slice(c);
            Ok(())
        })
        .unwrap();
        assert!((s.program.unwrap().deepest_db > 1.0) == (gain > 0.0), "scene guard: {:?}", s.program);
        assert!((player.shared.program_cut.load(Ordering::Relaxed) > 100) == (gain > 0.0), "the stage's light");
        let export = &export[..n * CH];
        let bad = heard.iter().zip(export).position(|(a, b)| a.to_bits() != b.to_bits());
        assert_eq!(bad.map(|i| i / CH), None, "gain {gain}: the play leaves the export at that frame");
        if gain == 0.0 {
            let (player, ring) = Player::new(project(None), vec![(MediaId(1), source())]);
            assert!(heard == hear(&player, ring, from, n), "a stage under its ceiling changed the play");
        }
    }
}

/// t4202: a covered loop plays from the kept mix through a stage; its gain is changed by 3 dB, taken
/// away and given back while it plays: no burst is rendered again, the level follows with no step
/// above the material's own, and the loop goes on.
#[test]
fn t4202_a_gain_change_keeps_the_kept_mix() {
    let p = project(Some(ProgramStage { gain_db: -9.0, measured: None }));
    let (player, ring) = Player::new(p.clone(), vec![(MediaId(1), source())]);
    let (listen, stop) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
    let dev = device(&player, ring, listen.clone(), stop.clone(), None);
    // a whole number of the tone's periods, so the wrap is no step of the material
    let (from, len) = (SEC, SEC);
    player.pause();
    player.set_loop_region(Some(kt_time::TickRange {
        start: Tick::from_audio_frames(from),
        end: Tick::from_audio_frames(from + len),
    }));
    player.seek(Tick::from_audio_frames(from));
    wait("the seek reached the device", || !player.shared.flush.load(Ordering::Acquire));
    listen.store(true, Ordering::Release);
    player.play();
    // the device has no clock and runs as fast as it is fed: the laps are waited for, not the time
    let laps = |n: u64| {
        let to = player.transport.wraps() + n;
        wait("the loop went on", || player.transport.wraps() >= to);
    };
    laps(4);
    let live = count(Work::AudioRender);
    for program in [
        Some(ProgramStage { gain_db: -6.0, measured: None }),
        None,
        Some(ProgramStage { gain_db: -6.0, measured: None }),
    ] {
        player.set_project(project(program));
        player.wait_built();
        laps(4);
    }
    assert_eq!(count(Work::AudioRender), live, "a burst was rendered again after the stage changed");
    player.pause();
    stop.store(true, Ordering::Release);
    let heard = dev.join().unwrap();
    assert_eq!(player.underruns.load(Ordering::Relaxed), 0);
    let left: Vec<f32> = heard.chunks_exact(CH).map(|f| f[0]).collect();
    let top = |a: usize| left[a..a + 4800].iter().fold(0f32, |m, v| m.max(v.abs()));
    let (first, last) = (top(2 * len as usize), top(left.len() - 9600));
    assert!((last / first - 1.4125).abs() < 0.03, "3 dB louder: {first} → {last}");
    let loudest = (0..left.len() / 4800 - 1).map(|k| top(k * 4800)).fold(0f32, f32::max);
    assert!(loudest > first * 2.5, "with the stage away the mix is heard as it is: {loudest}");
    // the tone's own step at the loudest, and a little for the ring of the reverb at the wrap
    let own = left.windows(2).skip(SEC as usize / 2).map(|w| (w[1] - w[0]).abs() / loudest).fold(0f32, f32::max);
    let tone = 2.0 * std::f32::consts::PI * 330.0 / SEC as f32;
    assert!(own < tone * 1.5, "a step of {own} against the tone's {tone}");
}
