//! Runtime selection of the SIMD level of the audio kernels (R.32; the video ones left for the GPU).
//!
//! User requirement: “we need to do it carefully somehow, what if there are no
//! super-modern instructions” — therefore the build stays baseline
//! x86-64 (SSE2), and the AVX2 kernels are enabled ONLY after a runtime check.
//! A CPU without AVX2 gets exactly the same scalar code as before.
//!
//! The MAIN invariant (invariant No. 11 of the root): the SIMD kernel must be BYTE-FOR-BYTE equal to
//! the scalar one on any CPU.
//!
//! `KT_SIMD=0` — forced scalar (debugging/comparison).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SimdLevel {
    Scalar,
    /// AVX2 (includes AVX and SSE4.1). There is deliberately no separate SSE level:
    /// the f64 kernels need 4 lanes (__m256d), and a 2-lane variant does not pay for
    /// a second implementation that would have to be kept bit-exact.
    Avx2,
}

/// The level is computed ONCE per process : the branch costs
/// per kernel call (frame/layer), not per pixel.
pub fn simd_level() -> SimdLevel {
    static LEVEL: std::sync::OnceLock<SimdLevel> = std::sync::OnceLock::new();
    *LEVEL.get_or_init(|| {
        if std::env::var("KT_SIMD").as_deref() == Ok("0") {
            return SimdLevel::Scalar;
        }
        #[cfg(target_arch = "x86_64")]
        if std::is_x86_feature_detected!("avx2") {
            return SimdLevel::Avx2;
        }
        SimdLevel::Scalar
    })
}

/// String for the perf panel/diagnostics.
pub fn simd_name() -> &'static str {
    match simd_level() {
        SimdLevel::Scalar => "scalar",
        SimdLevel::Avx2 => "avx2",
    }
}
