//! A correct `round`/`roundf` for the Windows GNU target (change windows-build).
//!
//! Rust's `f64::round` lowers to the C `round`; on `x86_64-pc-windows-gnu` that comes from
//! mingw-w64's static runtime, which gets the boundary wrong: `round(0.49999999999999994)`
//! returns 1 (the classic `floor(x + 0.5)`: the sum rounds up to 1.0). Every `.round()` of the
//! CPU renderer — the byte reference of invariant 11 — went through it, so Windows diverged
//! from Linux on such values (t198 caught it on the real Windows runner, not only under wine).
//!
//! Defining the symbols here makes the linker resolve `round`/`roundf` to these before it
//! reaches the mingw library (Rust objects precede the system libraries). kt-time is linked
//! into every binary and test of the workspace. Other targets are untouched.
//!
//! Round half away from zero without ever forming `x + 0.5`: `x - trunc(x)` is exact for
//! every finite `x` (for |x| ≥ 2^52 it is 0), so the comparison with 0.5 is exact too.
//! NaN stays NaN (the comparison is false), ±inf stays itself, −0.0 stays −0.0.

#[unsafe(no_mangle)]
pub extern "C" fn round(x: f64) -> f64 {
    let t = x.trunc();
    if (x - t).abs() >= 0.5 { t + 1.0f64.copysign(x) } else { t }
}

#[unsafe(no_mangle)]
pub extern "C" fn roundf(x: f32) -> f32 {
    let t = x.trunc();
    if (x - t).abs() >= 0.5 { t + 1.0f32.copysign(x) } else { t }
}
