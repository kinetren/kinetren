//! A smoother that stands still (kernel-specialization; capability `plugins`).
//!
//! t2170 — `SmoothedParam::still` is true exactly when a tick would leave the value's bits, and
//! then every later tick leaves them too (until the target moves); `advance(n)` gives the bits of
//! `n` ticks, specialized or inside `cpu::with_general`. Glides up, down, to a target it stops
//! short of, through −0 and from a reset, at several smoothing times.

use kt_plugin_api::cpu;
use kt_plugin_api::dsp::SmoothedParam;

fn glides() -> Vec<(f32, f32, f32)> {
    // (start, target, ms)
    let mut v = vec![(0.0, 1.0, 20.0), (1.0, 0.0, 20.0), (-0.0, -0.0, 20.0), (0.0, -0.0, 5.0), (-0.0, 0.0, 5.0)];
    for (a, b) in [(-18.0f32, -40.0f32), (800.0, 12_000.0), (0.707, 10.0), (1e-30, 3.0), (5.0, 5.000_001)] {
        for ms in [0.01f32, 1.0, 20.0, 150.0] {
            v.push((a, b, ms));
            v.push((b, a, ms));
        }
    }
    v
}

#[test]
fn t2170_still_is_a_fixed_point_and_advance_is_n_ticks() {
    let mut rested = 0;
    for (a, b, ms) in glides() {
        let mut p = SmoothedParam::with_ms(a, ms);
        p.set_target(b);
        let mut reference = p;
        let mut was_still = false;
        for i in 0..200_000 {
            let still = p.still();
            let before = p.value().to_bits();
            let after = p.tick().to_bits();
            assert_eq!(still, before == after, "{a}->{b} {ms} ms tick {i}: still() disagrees with the tick");
            assert!(!was_still || still, "{a}->{b} {ms} ms tick {i}: moved after standing still");
            was_still = still;
            reference.tick();
        }
        rested += was_still as usize;
        // advance(n) on a fresh glide, both paths, against n ticks
        for n in [1usize, 7, 128, 1000, 100_000] {
            let mut t = SmoothedParam::with_ms(a, ms);
            t.set_target(b);
            let mut g = t;
            let mut s = t;
            for _ in 0..n {
                t.tick();
            }
            assert_eq!(s.advance(n).to_bits(), t.value().to_bits(), "{a}->{b} {ms} ms advance({n})");
            assert_eq!(cpu::with_general(|| g.advance(n)).to_bits(), t.value().to_bits(), "general advance({n})");
            assert_eq!(s.tick().to_bits(), t.tick().to_bits(), "the tick after advance({n})");
        }
    }
    assert!(rested > 30, "most glides come to rest within 200 000 ticks: {rested}");
}
