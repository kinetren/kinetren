//! t1065 (ci-green-again, invariant 11): the one math of the frozen paths stays within one ulp of
//! glibc on a sweep of every function it offers.
//!
//! `kt_plugin_api::math` replaced the platform libm in the kernels so Linux and Windows give the
//! same bytes (the owner, 01.11: "own math, close to Linux, where Linux is the more accurate one").
//! "Close" is held here: on Linux with glibc, std's functions ARE glibc, and every input of the
//! sweep must land within one ulp of them. Elsewhere there is no glibc to ask, so the test says so.

use kt_plugin_api::math;

/// The distance in ulps of two finite floats of one sign class; equal NaNs and infinities are 0.
fn ulps32(a: f32, b: f32) -> u64 {
    if a.is_nan() && b.is_nan() || a == b {
        return 0;
    }
    let key = |x: f32| {
        let i = x.to_bits() as i32;
        if i < 0 { i64::from(i32::MIN) - i64::from(i) } else { i64::from(i) }
    };
    key(a).abs_diff(key(b))
}

fn ulps64(a: f64, b: f64) -> u64 {
    if a.is_nan() && b.is_nan() || a == b {
        return 0;
    }
    let key = |x: f64| {
        let i = x.to_bits() as i64;
        if i < 0 { i128::from(i64::MIN) - i128::from(i) } else { i128::from(i) }
    };
    key(a).abs_diff(key(b)) as u64
}

/// A deterministic sweep of `n` points over `[lo, hi]`, jittered so it does not sit on a grid.
fn sweep(lo: f64, hi: f64, n: usize) -> impl Iterator<Item = f64> {
    let mut s = 0x9e37_79b9_7f4a_7c15u64;
    (0..n).map(move |i| {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let j = (s >> 11) as f64 / (1u64 << 53) as f64;
        lo + (hi - lo) * ((i as f64 + j) / n as f64)
    })
}

struct Tally {
    name: &'static str,
    n: u64,
    differ: u64,
    worst: u64,
    at: String,
}

impl Tally {
    fn new(name: &'static str) -> Tally {
        Tally { name, n: 0, differ: 0, worst: 0, at: String::new() }
    }
    fn add(&mut self, d: u64, at: impl FnOnce() -> String) {
        self.n += 1;
        if d > 0 {
            self.differ += 1;
        }
        if d > self.worst {
            self.worst = d;
            self.at = at();
        }
    }
}

#[test]
fn t1065_the_frozen_math_is_within_one_ulp_of_glibc() {
    if !(cfg!(target_os = "linux") && cfg!(target_env = "gnu")) {
        eprintln!("t1065 not run: std's functions are glibc only on Linux; the Linux job runs it");
        return;
    }
    const N: usize = 200_000;
    let mut all = Vec::new();
    macro_rules! one32 {
        ($name:literal, $ours:expr, $std:expr, $lo:expr, $hi:expr) => {{
            let mut t = Tally::new($name);
            for x in sweep($lo, $hi, N) {
                let x = x as f32;
                let (a, b): (f32, f32) = ($ours(x), $std(x));
                t.add(ulps32(a, b), || format!("{x:e}: ours {a:e}, glibc {b:e}"));
            }
            all.push(t);
        }};
    }
    macro_rules! one64 {
        ($name:literal, $ours:expr, $std:expr, $lo:expr, $hi:expr) => {{
            let mut t = Tally::new($name);
            for x in sweep($lo, $hi, N) {
                let (a, b): (f64, f64) = ($ours(x), $std(x));
                t.add(ulps64(a, b), || format!("{x:e}: ours {a:e}, glibc {b:e}"));
            }
            all.push(t);
        }};
    }
    // the ranges the kernels use, and beyond: phases of LFOs and tones run to thousands of radians
    one32!("sinf", math::sinf, f32::sin, -2000.0, 2000.0);
    one32!("cosf", math::cosf, f32::cos, -2000.0, 2000.0);
    one32!("sincosf.0", |x| math::sincosf(x).0, |x: f32| x.sin_cos().0, -50.0, 50.0);
    one32!("sincosf.1", |x| math::sincosf(x).1, |x: f32| x.sin_cos().1, -50.0, 50.0);
    one32!("tanf", math::tanf, f32::tan, -1.57, 1.57);
    one32!("expf", math::expf, f32::exp, -87.0, 88.0);
    one32!("log10f", math::log10f, f32::log10, 1e-30, 1e30);
    one32!("asinf", math::asinf, f32::asin, -1.0, 1.0);
    one32!("powf(10, x)", |x| math::powf(10.0, x), |x: f32| 10f32.powf(x), -30.0, 30.0);
    one32!("powf(2, x)", |x| math::powf(2.0, x), |x: f32| 2f32.powf(x), -100.0, 100.0);
    one32!("powf(x, 1.7)", |x| math::powf(x, 1.7), |x: f32| x.powf(1.7), 0.0, 1000.0);
    one32!("atan2f(x, 1-x)", |x| math::atan2f(x, 1.0 - x), |x: f32| x.atan2(1.0 - x), -10.0, 10.0);
    one64!("sin", math::sin, f64::sin, -1e5, 1e5);
    one64!("cos", math::cos, f64::cos, -1e5, 1e5);
    one64!("sincos.0", |x| math::sincos(x).0, |x: f64| x.sin_cos().0, -50.0, 50.0);
    one64!("exp", math::exp, f64::exp, -700.0, 700.0);
    one64!("log10", math::log10, f64::log10, 1e-300, 1e300);
    one64!("pow(10, x)", |x| math::pow(10.0, x), |x: f64| 10f64.powf(x), -300.0, 300.0);
    one64!("pow(2, x)", |x| math::pow(2.0, x), |x: f64| 2f64.powf(x), -10.0, 10.0);
    one64!("atan2(x, 1-x)", |x| math::atan2(x, 1.0 - x), |x: f64| x.atan2(1.0 - x), -10.0, 10.0);
    let mut report = String::new();
    for t in &all {
        report.push_str(&format!(
            "{:>15}: {} points, {} differ ({:.4} %), worst {} ulp {}\n",
            t.name,
            t.n,
            t.differ,
            100.0 * t.differ as f64 / t.n as f64,
            t.worst,
            t.at
        ));
    }
    println!("{report}");
    let bad: Vec<_> = all.iter().filter(|t| t.worst > 1).map(|t| t.name).collect();
    assert!(bad.is_empty(), "more than one ulp from glibc: {bad:?}\n{report}");
}
