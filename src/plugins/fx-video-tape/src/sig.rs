//! The per-sample arithmetic shared by the CPU model's stages and their WGSL mirrors (design D5): our own
//! f32 sine/cosine in CYCLES and atan2 in cycles, the counter hash, uniform and Gaussian grain, and
//! the u16 fixed-point packing of two values into one texel. Each function is a fixed sequence of
//! f32 operations, written again verbatim in `wgsl/common.wgsl`; edit both in the same commit.
//! No platform libm (t1066): WGSL's own `cos`/`atan2` are implementation-defined in accuracy, so the
//! GPU would depend on the vendor.

use kt_plugin_api::barrier::noise_hash;

const S1: f32 = 6.283_185_5;
const S3: f32 = -41.341_7;
const S5: f32 = 81.605_25;
const S7: f32 = -76.705_86;
const S9: f32 = 42.058_69;
const S11: f32 = -15.094_643;

/// sin(2π·x), x in cycles. The cycle is folded into [−¼, ¼] and a degree-11 odd polynomial
/// evaluates it (error ≈ 6e-8 against the true sine).
#[inline]
pub fn sin_cyc(x: f32) -> f32 {
    let mut r = x - (x + 0.5).floor();
    if r > 0.25 {
        r = 0.5 - r;
    }
    if r < -0.25 {
        r = -0.5 - r;
    }
    let t2 = r * r;
    r * (S1 + t2 * (S3 + t2 * (S5 + t2 * (S7 + t2 * (S9 + t2 * S11)))))
}

/// cos(2π·x), x in cycles.
#[inline]
pub fn cos_cyc(x: f32) -> f32 {
    sin_cyc(x + 0.25)
}

const A1: f32 = 0.999_977_26;
const A3: f32 = -0.332_623_47;
const A5: f32 = 0.193_543_46;
const A7: f32 = -0.116_432_87;
const A9: f32 = 0.052_653_32;
const A11: f32 = -0.011_721_2;
const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;
const PI: f32 = std::f32::consts::PI;
const INV_TAU: f32 = 1.0 / std::f32::consts::TAU;

/// atan2(y, x) in RADIANS (error ≈ 1.7e-6), 0 at the origin.
#[inline]
pub fn atan2_rad(y: f32, x: f32) -> f32 {
    let ax = x.abs();
    let ay = y.abs();
    let mx = ax.max(ay);
    if mx == 0.0 {
        return 0.0;
    }
    let a = ax.min(ay) / mx;
    let s = a * a;
    let mut r = a * (A1 + s * (A3 + s * (A5 + s * (A7 + s * (A9 + s * A11)))));
    if ay > ax {
        r = HALF_PI - r;
    }
    if x < 0.0 {
        r = PI - r;
    }
    if y < 0.0 {
        r = -r;
    }
    r
}

/// atan2(y, x) in CYCLES, in (−½, ½].
#[inline]
pub fn atan2_cyc(y: f32, x: f32) -> f32 {
    atan2_rad(y, x) * INV_TAU
}

/// The counter hash (lowbias32, the same as noise and vhs).
#[inline]
pub fn hash(x: u32) -> u32 {
    noise_hash(x)
}

/// A hash of three counters.
#[inline]
pub fn hash3(a: u32, b: u32, c: u32) -> u32 {
    hash(a ^ hash(b ^ hash(c)))
}

/// A uniform number in [0, 1) from a hash (24 bits, exact in f32).
#[inline]
pub fn unit(h: u32) -> f32 {
    (h >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// A Gaussian-like grain of unit variance: the sum of four uniforms (Irwin–Hall), centred and scaled.
#[inline]
pub fn gauss(h: u32) -> f32 {
    let h1 = hash(h);
    let h2 = hash(h1);
    let h3 = hash(h2);
    ((unit(h) + unit(h1)) + (unit(h2) + unit(h3)) - 2.0) * 1.732_050_8
}

/// The u16 fixed point of a texel half: `v` in [−off, 65535/scale − off).
#[inline]
pub fn q16(v: f32, off: f32, scale: f32) -> f32 {
    let q = ((v + off) * scale).round_ties_even().clamp(0.0, 65535.0);
    q / scale - off
}

/// The level of the demodulated luma and the RF envelope: offset and scale of their u16 halves.
pub const LEVEL_OFF: f32 = 2.0;
pub const LEVEL_SCALE: f32 = 8192.0;
pub const ENV_OFF: f32 = 0.0;
pub const ENV_SCALE: f32 = 16384.0;
/// The chroma U/V halves.
pub const UV_OFF: f32 = 2.0;
pub const UV_SCALE: f32 = 16384.0;

#[cfg(test)]
mod tests {
    use super::*;
    use kt_plugin_api::math::FrozenMath;

    #[test]
    fn our_functions_follow_the_true_ones() {
        let mut worst = (0f64, 0f64);
        for i in -4000..4000 {
            let x = i as f32 / 997.0;
            let t = (x as f64 * std::f64::consts::TAU).fsin();
            worst.0 = worst.0.max((sin_cyc(x) as f64 - t).abs());
            let (y, xx) = ((i as f32 * 0.37).fsin(), (i as f32 * 0.11).fcos());
            let a = (y as f64).fatan2(xx as f64);
            worst.1 = worst.1.max((atan2_rad(y, xx) as f64 - a).abs());
        }
        assert!(worst.0 < 4e-7 && worst.1 < 4e-6, "sin {:e}, atan2 {:e}", worst.0, worst.1);
    }
}
