//! play-start-and-prefetch: a start's pass waits for the consumer's drain of a seek's flush, and Play's
//! first sound is measured. t2641, t2645.

use kt_engine::Player;
use kt_engine::preview_bench::{BenchOptions, play_range};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;
use std::sync::atomic::Ordering;
use std::time::Duration;

fn player() -> (Player, rtrb::Consumer<f32>) {
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
    p.clips.push(ClipEvent {
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
        duration: Tick(90_000 * 20),
        source_in: Tick::ZERO,
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
        fx_in: Vec::new(),
    });
    p.ensure_tracks(LaneKind::Audio, 1);
    p.validate().expect("scene guard: the project is legal");
    Player::new(
        p,
        vec![(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 3600), AudioPattern::Sine { hz: 330.0 })) as _)],
    )
}

fn wait(what: &str, mut ok: impl FnMut() -> bool) {
    let deadline = kt_time::test_deadline();
    while !ok() {
        assert!(std::time::Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Test t2641 (D3): with a consumer that drains flushes itself, a start writes nothing into the ring
/// while the seek's flush is pending — a pass written then is what the drain drops, and the start
/// lost its first ring — and writes its pass once the consumer has drained it.
#[test]
fn t2641_a_start_waits_for_the_drain_of_its_flush() {
    let (player, mut ring) = player();
    player.shared.drains_flush.store(true, Ordering::Release);
    // what the opening rendered is old: take it, until the paused render thread stands
    let mut quiet = 0;
    wait("the opening's pass settled", || {
        let mut n = 0;
        while ring.pop().is_ok() {
            n += 1;
        }
        quiet = if n == 0 { quiet + 1 } else { 0 };
        std::thread::sleep(Duration::from_millis(20));
        quiet >= 10
    });
    player.seek(Tick(90_000 * 5));
    player.play();
    // the render thread notices the start and holds; nothing drains the flush yet (a hold is waited
    // out by its consequence's absence: the pass it would write comes within a pass, ~8 ms)
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(ring.slots(), 0, "the start's pass went into the ring before its flush was drained");
    // the consumer drains (the callback, or the output thread while the device is closed)
    while ring.pop().is_ok() {}
    player.shared.flush.store(false, Ordering::Release);
    wait("the start's pass came after the drain", || ring.slots() > 0);
    player.pause();
}

/// Test t2645 (D4, D5): a device that opens 150 ms after Play puts Play's first sound at least that
/// late; the bench reports it as the render thread measured it, and the trace has it (`play` aux
/// `first_sound`).
#[test]
fn t2645_the_first_sound_waits_for_the_device_and_is_reported() {
    let (player, ring) = player();
    let open = Duration::from_millis(150);
    let mut r = None;
    let items = kt_time::trace::testing::capture(|| {
        r = Some(play_range(
            &player,
            ring,
            BenchOptions { from: Tick(90_000), to: Tick(90_000 * 2), device_open: open, ..Default::default() },
        ));
    });
    let r = r.unwrap();
    eprintln!("play → first sound {:.1} ms with a device that opens in {open:?}", r.first_sound_ms);
    assert!(r.first_sound_ms >= open.as_secs_f64() * 1e3, "the first sound came before the device opened: {:.1} ms", r.first_sound_ms);
    let recs = kt_time::trace::testing::recs(&items);
    let traced: Vec<u64> = recs
        .iter()
        .filter(|x| x.ev == kt_time::trace::Ev::Play as u16 && x.aux == kt_time::trace::play::FIRST_SOUND)
        .map(|x| x.b)
        .collect();
    assert_eq!(traced.len(), 1, "one Play, one first sound in the trace: {traced:?}");
    assert!(traced[0] >= open.as_micros() as u64, "the trace's first sound {} µs is before the open", traced[0]);
}
