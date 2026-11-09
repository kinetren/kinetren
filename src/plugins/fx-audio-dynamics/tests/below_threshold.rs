//! The compressor below its threshold (kernel-specialization; capabilities `audio-fx`, `plugins`).
//!
//! t2172 — under `below_threshold(thr)` the compressor skips its `log10` and `pow` and takes the
//! make-up gain: that is the general formula's answer to the bit. For every threshold of a grid
//! wider than the passport (−100…+20 dB in 0.1 dB, plus random `f32` thresholds), the 4096 floats
//! just under the level and 4096 spread over [1e-6, level) give no reduction (`over` is +0), and
//! the formula's gain equals the make-up gain's bits.

use fx_audio_dynamics::below_threshold;
use kt_plugin_api::math::FrozenMath;

/// The compressor's per-sample formula above the skip (`CompNode::process`), verbatim.
fn general(env: f32, thr: f32, ratio: f32, makeup: f32) -> f32 {
    let env_db = 20.0 * env.flog10();
    let over = (env_db - thr).max(0.0);
    let gr_db = over * (1.0 - 1.0 / ratio);
    10f32.fpowf(-gr_db / 20.0) * makeup
}

#[test]
fn t2172_below_the_level_the_gain_is_the_makeup_to_the_bit() {
    assert_eq!(10f32.fpowf(-0.0).to_bits(), 1f32.to_bits(), "10^-0 is exactly one");
    let mut thresholds: Vec<f32> = (-1000..=200).map(|i| i as f32 * 0.1).collect();
    let mut s: u32 = 0x2545_f491;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s
    };
    for _ in 0..2000 {
        thresholds.push(-100.0 + 120.0 * (next() >> 8) as f32 / (1u32 << 24) as f32);
    }
    let lo = 1e-6f32.to_bits();
    let mut checked = 0u64;
    for &thr in &thresholds {
        let b = below_threshold(thr);
        if b.is_nan() || b <= 1e-6 {
            continue;
        }
        let top = b.to_bits() - 1; // the largest float under the level (positive floats order as bits)
        let mut envs: Vec<f32> = (0..4096u32).filter_map(|k| top.checked_sub(k).filter(|v| *v >= lo)).map(f32::from_bits).collect();
        for _ in 0..4096 {
            envs.push(f32::from_bits(lo + next() % (top - lo + 1)));
        }
        for env in envs {
            assert!(env < b && env >= 1e-6);
            // no reduction: `over` is +0, so `gr_db` is +0 for every ratio ≥ 1 and the gain the make-up
            let over = (20.0 * env.flog10() - thr).max(0.0);
            assert_eq!(over.to_bits(), 0, "thr {thr} env {env:e}: over {over}");
            let (ratio, makeup) = [(1.0f32, 1.0f32), (4.0, 1.3), (20.0, 15.85)][checked as usize % 3];
            let g = general(env, thr, ratio, makeup);
            assert_eq!(g.to_bits(), makeup.to_bits(), "thr {thr} env {env:e} ratio {ratio} makeup {makeup}: {g}");
            checked += 1;
        }
    }
    assert!(checked > 20_000_000, "{checked}");
}
