//! Диагностика/регрессия: Warp обязан потреблять источник ~rate·выход
//! после каждого set_rate (rubato ramp учитываем допуском).

use kt_fx::{CH, Warp};

#[test]
fn warp_consumption_tracks_rate_changes() {
    let mut w = Warp::new(1.0, 0);
    let mut out = vec![0f32; 1024 * CH];
    let rates = [1.0f64, 0.35, 0.6, 1.0];
    for &r in &rates {
        w.set_rate(r);
        // прогрев ramp'а: один чанк на переход
        let mut src = |_f: i64, buf: &mut [f32]| {
            buf.fill(0.0);
            true
        };
        w.produce(&mut out, &mut src);
        let before = w.src_cursor();
        for _ in 0..46 {
            let mut src = |_f: i64, buf: &mut [f32]| {
                buf.fill(0.0);
                true
            };
            w.produce(&mut out, &mut src);
        }
        let consumed = w.src_cursor() - before;
        let expect = (46.0 * 1024.0 * r) as i64;
        assert!(
            (consumed - expect).abs() < expect / 10 + 2048,
            "rate {r}: съедено {consumed}, ожидалось ~{expect}"
        );
    }
}
