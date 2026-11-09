//! The CPU level an audio kernel's per-sample loop is compiled for (audio-fx-cost).
//!
//! The shipped build is baseline x86-64 (SSE2: a CPU without AVX must run it, R.32). SSE2 has no
//! instruction for `floor`/`ceil`/`round`/`trunc`, so every one of them — and `fract`, which is a
//! `trunc` — on a per-sample path is a CALL into the C library through the PLT: an LFO's phase, a
//! hash-to-unit, a delay line's fractional read, a tape's line position. `run_fast` runs the same
//! closure compiled a second time with AVX2 enabled where the CPU has it: those calls become one
//! instruction and the plain loops may use wider registers.
//!
//! The bytes do not move (invariant 11): Rust never contracts `a * b + c` into an FMA without
//! being asked and never reorders a float sum, the rounding functions are exact by IEEE 754 on
//! both paths, and the AVX2 feature set here does not include FMA. t1981 holds every audio kind
//! byte-equal on both paths; `KT_SIMD=0` forces the baseline path (as the kt-fx SIMD kernels).

/// Whether the AVX2 path is taken: checked once per process.
pub fn fast() -> bool {
    static LEVEL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *LEVEL.get_or_init(|| {
        if std::env::var("KT_SIMD").as_deref() == Ok("0") {
            return false;
        }
        #[cfg(target_arch = "x86_64")]
        {
            std::is_x86_feature_detected!("avx2")
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    })
}

/// Run `f` compiled for AVX2 where `fast()` allows it, else as built. Called through `fast!`, which
/// marks the closure `#[inline(always)]`: a closure with two callers is not inlined by itself, and
/// a body left out of line keeps the baseline code. The body is the kernel's `#[inline(always)]`
/// per-block method; what is not inlined into it keeps the baseline code too.
#[inline(always)]
pub fn run_fast<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(target_arch = "x86_64")]
    if !BASELINE.get() {
        #[target_feature(enable = "avx2")]
        #[inline]
        fn v3<R>(f: impl FnOnce() -> R) -> R {
            f()
        }
        if fast() {
            // SAFETY: `fast()` saw the CPU report AVX2
            return unsafe { v3(f) };
        }
    }
    f()
}

thread_local! {
    static BASELINE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static GENERAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether a kernel may take its "stands still" variant (kernel-specialization): a re-tune skipped
/// while its inputs are equal, a constant-parameter block, the compressor below its threshold, the
/// mixdown's constant-weight runs. Each variant is the general path's bytes by construction; this
/// switch is how the tests prove it (t2171–t2173). Read once per block, never per sample.
#[inline]
pub fn specialized() -> bool {
    !GENERAL.get()
}

/// Run `f` with every kernel and the mixdown on this thread on their GENERAL path: no "stands
/// still" variant (the reference of t2171–t2173).
pub fn with_general<R>(f: impl FnOnce() -> R) -> R {
    let was = GENERAL.replace(true);
    let r = f();
    GENERAL.set(was);
    r
}

/// Run `f` with every kernel it calls on this thread on the baseline path, whatever the CPU (the
/// reference of t1981).
pub fn with_baseline<R>(f: impl FnOnce() -> R) -> R {
    let was = BASELINE.replace(true);
    let r = f();
    BASELINE.set(was);
    r
}

/// `kt_plugin_api::fast!(self.run(buf, pos))` — the per-block body at the CPU's level (`run_fast`).
#[macro_export]
macro_rules! fast {
    ($e:expr) => {
        $crate::cpu::run_fast(
            #[inline(always)]
            || $e,
        )
    };
}
