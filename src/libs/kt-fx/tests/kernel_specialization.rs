//! The loudness meter at the CPU's level (kernel-specialization; capability `audio-render`).
//!
//! t2174 — the meter runs through `fast!`, its true peak reads a doubled ring instead of rotating
//! its tail, and its interpolator table is fetched once per feed: LUFS, LRA and dBTP of fixed
//! material keep the bits they had before (printed on the tree before the change), fed whole and in
//! odd slices, on the CPU's path and on the baseline path.

use kt_fx::audio::loudness::{Loudness, LoudnessMeter};
use kt_plugin_api::math::FrozenMath;

/// Music-like material, a pause, a quiet stretch and a sine at a quarter of the rate with a phase of
/// 45° (its wave passes 3 dB above its samples: the true peak's interpolation has work).
fn material() -> Vec<f32> {
    let mut v = kt_fx::audio::cost::stimulus(48_000 * 12);
    v.extend(std::iter::repeat_n(0.0, 48_000 * 2 * 2));
    v.extend(kt_fx::audio::cost::stimulus(48_000 * 5).iter().map(|s| s * 0.003));
    for i in 0..48_000 * 3 {
        let x = (std::f32::consts::FRAC_PI_2 * i as f32 + std::f32::consts::FRAC_PI_4).fsin() * 0.7;
        v.extend([x, -x]);
    }
    v
}

fn measure(data: &[f32], slice: usize) -> Loudness {
    let mut m = LoudnessMeter::new();
    for c in data.chunks(slice * 2) {
        m.feed(c);
    }
    m.finish()
}

fn bits(l: &Loudness) -> (u64, u64, u64) {
    (l.lufs.map_or(0, f64::to_bits), l.lra.to_bits(), l.dbtp.to_bits())
}

/// (lufs, lra, dbtp) bits, printed at d5d5b6b9.
const WANT: (u64, u64, u64) = (0xc019c28d04e9520e, 0x4030fd1bff17d393, 0xc007cb4e3dfd9a28);

#[test]
fn t2174_the_meter_keeps_its_bits_on_both_paths() {
    let data = material();
    let whole = bits(&measure(&data, data.len() / 2));
    if std::env::var("KT_PRINT_DIGESTS").is_ok() {
        println!("({:#018x}, {:#018x}, {:#018x}) {:?}", whole.0, whole.1, whole.2, measure(&data, data.len() / 2));
        return;
    }
    assert_eq!(whole, WANT, "the meter moved its bits");
    for slice in [1usize, 777, 4096] {
        assert_eq!(bits(&measure(&data, slice)), WANT, "in slices of {slice}");
    }
    let base = kt_plugin_api::cpu::with_baseline(|| bits(&measure(&data, 1024)));
    assert_eq!(base, WANT, "the baseline path");
}
