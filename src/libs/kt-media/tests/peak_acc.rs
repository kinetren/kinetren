//! The peaks pyramid at the CPU's level (kernel-specialization; capability `media-index`).
//!
//! t2175 — `PeakAcc::push` runs through `fast!` with the two channels' block as one fixed-size
//! fold: the pyramid of fixed material (past full scale, a NaN, negative zeros, a block cut by the
//! chunking) keeps the values it had before (a digest printed on the tree before the change), fed
//! whole and in odd chunks, on the CPU's path and on the baseline path.

use kt_media::index::PeakAcc;

fn material() -> Vec<f32> {
    let mut s: u32 = 0x9e37_79b9;
    let mut v = Vec::new();
    for i in 0..48_000 * 7 + 131 {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let n = (s >> 8) as f32 / (1u32 << 24) as f32 - 0.5;
        let l = n * 2.6 * ((i / 9000) % 3) as f32;
        v.extend([l, -0.0 + n * 0.1]);
    }
    v[1000] = f32::NAN;
    v[5001] = f32::INFINITY;
    v[7002] = f32::NEG_INFINITY;
    v
}

fn digest(data: &[f32], chunk: usize) -> u64 {
    let mut acc = PeakAcc::default();
    for c in data.chunks(chunk * 2) {
        acc.push(c);
    }
    let p = acc.finish();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for level in &p.levels {
        for ch in level {
            for &(lo, hi) in ch {
                for b in lo.to_le_bytes().into_iter().chain(hi.to_le_bytes()) {
                    h ^= b as u64;
                    h = h.wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
    }
    h
}

/// Printed at d5d5b6b9.
const WANT: u64 = 0x1749fc40ba01185e;

#[test]
fn t2175_the_pyramid_keeps_its_values_on_both_paths() {
    let data = material();
    let whole = digest(&data, data.len());
    if std::env::var("KT_PRINT_DIGESTS").is_ok() {
        println!("{whole:#018x}");
        return;
    }
    assert_eq!(whole, WANT);
    for chunk in [1usize, 777, 1024] {
        assert_eq!(digest(&data, chunk), WANT, "in chunks of {chunk}");
    }
    assert_eq!(kt_plugin_api::cpu::with_baseline(|| digest(&data, 1024)), WANT, "the baseline path");
}
