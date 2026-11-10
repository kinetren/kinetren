//! The warp's sinc interpolator (change ci-green-again, invariant 11): rubato's arithmetic with
//! a table and a dot product that give the same bytes on every platform and every CPU.
//!
//! rubato builds its windowed-sinc table (128 × 128 points for the warp) with `f32::sin`/`cos`,
//! that is the PLATFORM's libm, and picks its dot product by the CPU at run time — AVX+FMA, SSE
//! without FMA, or scalar, three different roundings. Slowed+Reverb printed on Linux failed on
//! windows-latest in all four cases of t973c. This is rubato's `make_sincs` (BlackmanHarris2,
//! periodic, normalised by the sum over the oversampling) evaluated with `kt_plugin_api::math`,
//! and its AVX+FMA product as the ONE rounding: eight lanes of fused multiply-adds, then
//! `(l0+l4 + l1+l5) + (l2+l6 + l3+l7)`. A fused multiply-add is exact by IEEE 754, so the AVX2
//! path (when the CPU has it and `KT_SIMD` allows) and the portable path are byte-for-byte equal.

use kt_plugin_api::math::{FrozenMath, fmaf};
use rubato::sinc_interpolator::SincInterpolator;
use std::f32::consts::PI;

pub(crate) struct FrozenSinc {
    /// `sincs[sub][tap]`, `tap < len`, as rubato's table (shared: `sincs_of`)
    sincs: std::sync::Arc<Vec<Vec<f32>>>,
    len: usize,
    nbr_sincs: usize,
    avx: bool,
}

impl FrozenSinc {
    /// As rubato's `make_interpolator` for the warp: `sinc_len` rounded up to eight, the cutoff
    /// scaled by the ratio when it slows down (`ratio` < 1 is fewer outputs than inputs).
    pub(crate) fn new(sinc_len: usize, ratio: f64, f_cutoff: f32, oversampling: usize) -> FrozenSinc {
        let len = 8 * sinc_len.div_ceil(8);
        let f_cutoff = if ratio >= 1.0 { f_cutoff } else { f_cutoff * ratio as f32 };
        FrozenSinc {
            sincs: sincs_of(len, oversampling, f_cutoff),
            len,
            nbr_sincs: oversampling,
            avx: avx_fma(),
        }
    }

    /// The portable product — the reference the AVX path equals.
    fn dot(wave: &[f32], sinc: &[f32]) -> f32 {
        let mut acc = [0f32; 8];
        for (w, s) in wave.chunks_exact(8).zip(sinc.chunks_exact(8)) {
            for l in 0..8 {
                acc[l] = fmaf(w[l], s[l], acc[l]);
            }
        }
        let a: [f32; 4] = std::array::from_fn(|i| acc[4 + i] + acc[i]);
        (a[0] + a[1]) + (a[2] + a[3])
    }
}

fn avx_fma() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        crate::simd::simd_level() == crate::simd::SimdLevel::Avx2 && std::is_x86_feature_detected!("fma")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx", enable = "fma")]
unsafe fn dot_avx(wave: &[f32], sinc: &[f32]) -> f32 {
    use std::arch::x86_64::*;
    let mut acc = _mm256_setzero_ps();
    for (w, s) in wave.chunks_exact(8).zip(sinc.chunks_exact(8)) {
        // SAFETY: each chunk is eight f32, the unaligned loads read exactly those
        let (w, s) = unsafe { (_mm256_loadu_ps(w.as_ptr()), _mm256_loadu_ps(s.as_ptr())) };
        acc = _mm256_fmadd_ps(w, s, acc);
    }
    let low = _mm_add_ps(_mm256_extractf128_ps(acc, 1), _mm256_castps256_ps128(acc));
    let pair = _mm_hadd_ps(low, low);
    _mm_cvtss_f32(_mm_hadd_ps(pair, pair))
}

impl SincInterpolator<f32> for FrozenSinc {
    fn get_sinc_interpolated(&self, wave: &[f32], index: usize, subindex: usize) -> f32 {
        let wave = &wave[index..index + self.len];
        let sinc = &self.sincs[subindex];
        #[cfg(target_arch = "x86_64")]
        if self.avx {
            // SAFETY: `avx` is set only when the CPU has AVX and FMA
            return unsafe { dot_avx(wave, sinc) };
        }
        FrozenSinc::dot(wave, sinc)
    }

    fn len(&self) -> usize {
        self.len
    }

    fn nbr_sincs(&self) -> usize {
        self.nbr_sincs
    }
}

/// The table of (length, oversampling, cutoff), built once and shared (local-cost-queue): a warp is
/// built at every seek, fresh entry and lap entry of a warped event, and the table took 16 384 × (three
/// `fcos`, one `fsin`) each time. The same function of the same arguments: the same bytes. Bounded:
/// past `KEEP` tables (a speed curve that slows varies the cutoff) the memo starts over.
fn sincs_of(npoints: usize, factor: usize, f_cutoff: f32) -> std::sync::Arc<Vec<Vec<f32>>> {
    type Memo = std::collections::HashMap<(usize, usize, u32), std::sync::Arc<Vec<Vec<f32>>>>;
    const KEEP: usize = 32;
    static MEMO: std::sync::Mutex<Option<Memo>> = std::sync::Mutex::new(None);
    let key = (npoints, factor, f_cutoff.to_bits());
    if let Some(t) = MEMO.lock().unwrap().get_or_insert_with(Memo::new).get(&key) {
        return t.clone();
    }
    // built outside the lock: two threads may build the same table, both the same bytes
    let t = std::sync::Arc::new(make_sincs(npoints, factor, f_cutoff));
    let mut g = MEMO.lock().unwrap();
    let m = g.get_or_insert_with(Memo::new);
    if m.len() >= KEEP {
        m.clear();
    }
    m.insert(key, t.clone());
    t
}

/// rubato's `make_sincs::<f32>(npoints, factor, f_cutoff, BlackmanHarris2)`, its operations in its
/// order, on our math.
fn make_sincs(npoints: usize, factor: usize, f_cutoff: f32) -> Vec<Vec<f32>> {
    let total = npoints * factor;
    let np = total as f32;
    let (pi2, pi4, pi6) = (2.0 * PI, 4.0 * PI, 6.0 * PI);
    let (a, b, c, d) = (0.35875f32, 0.48829f32, 0.14128f32, 0.01168f32);
    let mut y = Vec::with_capacity(total);
    let mut sum = 0f32;
    for x in 0..total {
        let xf = x as f32;
        let w = a - b * (pi2 * xf / np).fcos() + c * (pi4 * xf / np).fcos() - d * (pi6 * xf / np).fcos();
        let w = w * w;
        let v = (xf - (total / 2) as f32) * f_cutoff / factor as f32;
        let sinc = if v == 0.0 { 1.0 } else { (v * PI).fsin() / (v * PI) };
        let val = w * sinc;
        sum += val;
        y.push(val);
    }
    sum /= factor as f32;
    let mut sincs = vec![vec![0f32; npoints]; factor];
    for p in 0..npoints {
        for n in 0..factor {
            sincs[factor - n - 1][p] = y[factor * p + n] / sum;
        }
    }
    sincs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t1067 (ci-green-again): the AVX2 product equals the portable one bit for bit — a fused
    /// multiply-add is exact by IEEE 754, so the CPU picks the speed, never the bytes.
    #[test]
    fn t1067_the_warp_sinc_product_is_one_rounding() {
        let s = FrozenSinc::new(128, 0.7, 0.95, 128);
        let wave: Vec<f32> = (0..1024).map(|i| ((i * 7919) % 2003) as f32 / 1001.0 - 1.0).collect();
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("avx") && std::is_x86_feature_detected!("fma") {
            for sub in [0, 17, 127] {
                for idx in [0, 3, 500] {
                    let p = FrozenSinc::dot(&wave[idx..idx + s.len], &s.sincs[sub]);
                    let v = unsafe { dot_avx(&wave[idx..idx + s.len], &s.sincs[sub]) };
                    assert_eq!(p.to_bits(), v.to_bits(), "sub {sub} idx {idx}");
                }
            }
        }
    }
}
