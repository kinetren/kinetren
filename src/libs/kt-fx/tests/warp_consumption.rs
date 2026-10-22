//! Diagnostics/regression: Warp MUST consume the source at ~rate·output
//! after each set_rate (we allow for the rubato ramp with a tolerance).

use kt_fx::{CH, Warp};

#[test]
fn warp_consumption_tracks_rate_changes() {
    let mut w = Warp::new(1.0, 0, (1.0, 1.0));
    let mut out = vec![0f32; 1024 * CH];
    let rates = [1.0f64, 0.35, 0.6, 1.0];
    for &r in &rates {
        w.set_rate(r);
        // warming up the ramp: one chunk per transition
        let mut src = |_k: kt_fx::PullKind, _f: i64, buf: &mut [f32]| {
            buf.fill(0.0);
            true
        };
        w.produce(kt_fx::PullKind::Output, &mut out, &mut src);
        let before = w.src_cursor();
        for _ in 0..46 {
            let mut src = |_k: kt_fx::PullKind, _f: i64, buf: &mut [f32]| {
                buf.fill(0.0);
                true
            };
            w.produce(kt_fx::PullKind::Output, &mut out, &mut src);
        }
        let consumed = w.src_cursor() - before;
        let expect = (46.0 * 1024.0 * r) as i64;
        assert!(
            (consumed - expect).abs() < expect / 10 + 2048,
            "rate {r}: consumed {consumed}, expected ~{expect}"
        );
    }
}
