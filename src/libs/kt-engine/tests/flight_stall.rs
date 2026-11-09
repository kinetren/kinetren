//! R.72, M5: "what does 'playback froze: 5' mean? if it is important it must be in notices".
//! One freeze of the playhead is one episode: the engine pushed a record on every loop pass,
//! so the counter overstated it and the 512-entry ring drowned in it.

use kt_engine::flight::{Ev, FlightRecorder, Snap, StallWatch};

/// Runs the watcher as the render thread does, a pass every 50 ms.
fn run(w: &mut StallWatch, f: &FlightRecorder, from: u64, to: u64, rate: f32, head: i64) -> u64 {
    let mut t = from;
    while t < to {
        if let Some(idle) = w.tick(t, true, rate, head) {
            f.push_at(t, Ev::Stall, idle as i64, head, Snap::default());
        }
        t += 50;
    }
    t
}

fn stalls(f: &FlightRecorder) -> usize {
    f.records().iter().filter(|r| r.kind == Ev::Stall).count()
}

/// Test 1625: one freeze — one record; a rate of 0 stands by request and is no stall.
#[test]
fn t1625_one_freeze_is_one_record() {
    let f = FlightRecorder::new();
    let mut w = StallWatch::new(0);
    let t = run(&mut w, &f, 0, 3_000, 1.0, 48_000);
    assert_eq!(stalls(&f), 1, "a freeze of 3 s must be one record, not one per pass");
    assert_eq!(f.stalls(), (1, 48_000), "the interface reads the episode and its place");

    // the head moves: the episode is over, a new freeze is a new one
    let t = run(&mut w, &f, t, t + 500, 1.0, 96_000);
    assert_eq!(stalls(&f), 1, "half a second of standing is not a stall");
    let t = run(&mut w, &f, t, t + 2_000, 1.0, 96_000);
    assert_eq!(stalls(&f), 2, "the second freeze is the second episode");

    // rate 0: a freeze frame, the shuttle centre
    let t = run(&mut w, &f, t, t + 3_000, 0.0, 144_000);
    assert_eq!(stalls(&f), 2, "rate 0 is not a stall");

    // leaving rate 0 starts the wait over: the standing at rate 0 is not counted
    run(&mut w, &f, t, t + 900, 1.0, 144_000);
    assert_eq!(stalls(&f), 2, "the time at rate 0 must not count toward a stall");

    // pause: nothing either
    let mut w = StallWatch::new(0);
    assert_eq!(w.tick(5_000, false, 1.0, 0), None);
    assert_eq!(w.tick(9_000, false, 1.0, 0), None);

    // `clear` empties the ring, not the count the bell compares with
    f.clear();
    assert_eq!(f.stalls().0, 2);
}

/// Test 2303 (orch stage 9): a play after a rest is no freeze. The render thread parks on a pause
/// with no clock (one-scheduler), so its last look before the play is seconds old; the watch
/// counted the rest as standing and every play after a second of pause said "playback froze" (the
/// live `diagnostics` read the extra row in the Diagnostics window).
#[test]
fn t2303_a_play_after_a_parked_pause_is_no_freeze() {
    let f = FlightRecorder::new();
    let mut w = StallWatch::new(0);
    // the last look before the park: paused at the start
    assert_eq!(w.tick(1_000, false, 1.0, 0), None);
    // 3.5 s later the play wakes the thread: the head has not moved yet, by right
    let t = run(&mut w, &f, 4_562, 5_400, 1.0, 0);
    assert_eq!(stalls(&f), 0, "the rest before a play is not a freeze");
    // a real freeze after the play is still one
    run(&mut w, &f, t, t + 1_500, 1.0, 0);
    assert_eq!(stalls(&f), 1, "a head that stands a second into the play is a freeze");
    // the watch as built, then a play: its first look starts the wait
    let f = FlightRecorder::new();
    let mut w = StallWatch::new(0);
    run(&mut w, &f, 10_000, 10_900, 1.0, 0);
    assert_eq!(stalls(&f), 0, "the first playing look starts the wait, not the thread's start");
}
