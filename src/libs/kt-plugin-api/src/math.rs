//! The ONE math of the frozen paths (change ci-green-again, invariant 11): every transcendental
//! function an audio or pixel kernel, a generator, a curve or the engine calls on a path whose
//! bytes are frozen comes from here, never from `f32::sin`/`f64::powf` and their kin.
//!
//! Why: `f32::sin` and friends call the PLATFORM's libm — glibc on Linux, the mingw/msvcrt one
//! on Windows — and the two round differently in the last bit on some inputs. The references
//! printed on Linux failed on windows-latest (t973c, t1019, t1048 on 94e36e9e). The owner's
//! decision (01.11): "own math, close to Linux, where Linux is the more accurate one".
//!
//! The implementation is the pure-Rust `libm` crate (a port of musl's FreeBSD routines), pinned by
//! Cargo.lock and compiled into our binary: the same bytes on every platform and every CPU. Its
//! arch path is used only for `sqrt` and `fma`, which IEEE 754 defines exactly.
//!
//! "Close to Linux": glibc's float functions are nearly correctly rounded, libm's float ones are
//! not (a tenth of `expf`/`powf` results were a ulp off glibc's on t1065's sweep). So every `f32`
//! function here is the `f64` one rounded once to `f32` — correctly rounded except when the true
//! value lies within an `f64` ulp of an `f32` midpoint (about 2⁻²⁸ of inputs), i.e. glibc's result
//! wherever glibc is correctly rounded. The `f64` functions are libm's, within one ulp of glibc.
//! t1065 holds both on a sweep; t1066 refuses a direct std call in the kernels.
//!
//! What is NOT here: `sqrt`, `abs`, `floor`/`ceil`/`round`/`trunc`, `rem_euclid`, `powi`,
//! `to_radians`/`to_degrees`, `min`/`max`/`clamp` — exact (or plain arithmetic) on every
//! platform, so the std methods stay.

/// `sin` of an `f32`.
#[inline]
pub fn sinf(x: f32) -> f32 {
    libm::sin(x as f64) as f32
}

/// `cos` of an `f32`.
#[inline]
pub fn cosf(x: f32) -> f32 {
    libm::cos(x as f64) as f32
}

/// `(sin, cos)` of an `f32`, as `f32::sin_cos`.
#[inline]
pub fn sincosf(x: f32) -> (f32, f32) {
    let (s, c) = libm::sincos(x as f64);
    (s as f32, c as f32)
}

/// `tan` of an `f32`.
#[inline]
pub fn tanf(x: f32) -> f32 {
    libm::tan(x as f64) as f32
}

/// `e^x` of an `f32`.
#[inline]
pub fn expf(x: f32) -> f32 {
    libm::exp(x as f64) as f32
}

/// `x^y` of `f32`s.
#[inline]
pub fn powf(x: f32, y: f32) -> f32 {
    libm::pow(x as f64, y as f64) as f32
}

/// `log10` of an `f32`.
#[inline]
pub fn log10f(x: f32) -> f32 {
    libm::log10(x as f64) as f32
}

/// `atan2(y, x)` of `f32`s, as `y.atan2(x)`.
#[inline]
pub fn atan2f(y: f32, x: f32) -> f32 {
    libm::atan2(y as f64, x as f64) as f32
}

/// `asin` of an `f32`.
#[inline]
pub fn asinf(x: f32) -> f32 {
    libm::asin(x as f64) as f32
}

/// `x * y + z` with one rounding, as `f32::mul_add` — exact by IEEE 754 on every platform here,
/// where `f32::mul_add` without the `fma` target feature calls the platform's `fmaf`.
#[inline]
pub fn fmaf(x: f32, y: f32, z: f32) -> f32 {
    libm::fmaf(x, y, z)
}

/// `sin` of an `f64`.
#[inline]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// `cos` of an `f64`.
#[inline]
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// `(sin, cos)` of an `f64`, as `f64::sin_cos`.
#[inline]
pub fn sincos(x: f64) -> (f64, f64) {
    libm::sincos(x)
}

/// `e^x` of an `f64`.
#[inline]
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}

/// `x^y` of `f64`s.
#[inline]
pub fn pow(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

/// `log10` of an `f64`.
#[inline]
pub fn log10(x: f64) -> f64 {
    libm::log10(x)
}

/// `atan2(y, x)` of `f64`s, as `y.atan2(x)`.
#[inline]
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

/// The same functions as methods, named after std's with an `f` ("frozen") in front, so a kernel
/// reads as before: `x.fsin()` for `x.sin()`, `10f32.fpowf(db / 20.0)` for `10f32.powf(…)`.
/// `use kt_plugin_api::math::FrozenMath;` brings them in for `f32` and `f64` alike.
pub trait FrozenMath: Sized {
    fn fsin(self) -> Self;
    fn fcos(self) -> Self;
    fn fsin_cos(self) -> (Self, Self);
    fn ftan(self) -> Self;
    fn fexp(self) -> Self;
    fn fpowf(self, y: Self) -> Self;
    fn flog10(self) -> Self;
    /// `self.fatan2(x)` is `atan2(self, x)`, as `f32::atan2`.
    fn fatan2(self, x: Self) -> Self;
    fn fasin(self) -> Self;
    /// `self * a + b` with one rounding, as `f32::mul_add`.
    fn fmul_add(self, a: Self, b: Self) -> Self;
}

impl FrozenMath for f32 {
    fn fsin(self) -> f32 {
        sinf(self)
    }
    fn fcos(self) -> f32 {
        cosf(self)
    }
    fn fsin_cos(self) -> (f32, f32) {
        sincosf(self)
    }
    fn ftan(self) -> f32 {
        tanf(self)
    }
    fn fexp(self) -> f32 {
        expf(self)
    }
    fn fpowf(self, y: f32) -> f32 {
        powf(self, y)
    }
    fn flog10(self) -> f32 {
        log10f(self)
    }
    fn fatan2(self, x: f32) -> f32 {
        atan2f(self, x)
    }
    fn fasin(self) -> f32 {
        asinf(self)
    }
    fn fmul_add(self, a: f32, b: f32) -> f32 {
        fmaf(self, a, b)
    }
}

impl FrozenMath for f64 {
    fn fsin(self) -> f64 {
        sin(self)
    }
    fn fcos(self) -> f64 {
        cos(self)
    }
    fn fsin_cos(self) -> (f64, f64) {
        sincos(self)
    }
    fn ftan(self) -> f64 {
        libm::tan(self)
    }
    fn fexp(self) -> f64 {
        exp(self)
    }
    fn fpowf(self, y: f64) -> f64 {
        pow(self, y)
    }
    fn flog10(self) -> f64 {
        log10(self)
    }
    fn fatan2(self, x: f64) -> f64 {
        atan2(self, x)
    }
    fn fasin(self) -> f64 {
        libm::asin(self)
    }
    fn fmul_add(self, a: f64, b: f64) -> f64 {
        libm::fma(self, a, b)
    }
}
