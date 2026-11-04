//! One kernel text for the CPU and the GPU — the PROTOTYPE of change `compute-backends`
//! (F-EXT-004). Nothing shipped calls it: the effects keep their own CPU kernel and WGSL until the
//! owner decides the migration.
//!
//! The idea: WGSL and Rust share a subset in which one text is BOTH a valid Rust function and a
//! valid WGSL function. `shared_kernel!` compiles the text as Rust, with the text's `f32` meaning
//! **f64** (`cpu`: the f64 byte reference, invariant 11) and once more with a real `f32`
//! (`cpu_f32`: what a twice-as-wide SIMD lane would compute), and keeps the same tokens as the
//! WGSL string `WGSL` the GPU compiles at run time. Rust never contracts `a * b + c` into an FMA nor
//! reassociates, so the same text compiled for SSE2, SSE4.1, AVX2 or AVX-512 gives the same bytes
//! (`run_pointwise` dispatches them).
//!
//! The subset (anything else fails to compile on one side or the other):
//! - items: `fn name(arg: T, …) -> T { let x = e; … return e; }`, T ∈ {`f32`, `vec2f`, `vec3f`,
//!   `vec4f`}; helper names carry the kernel's prefix (one fused WGSL module holds every plugin);
//! - expressions: `+ - * /` between vectors and scalars (both orders), unary `-`, comparisons and
//!   `&& || !` on scalars, the fields `.x .y .z .w`, constructors with ALL components
//!   (`vec3f(x, y, z)`, no splat, no `vec4f(v3, w)`), calls to the builtins below and to the text's
//!   own functions, float literals with a point (`2.0`, never `2`);
//! - no `var`, no `if`: a branch is `select(f, t, cond)` (both sides are evaluated, as on the GPU).
//!
//! Builtins, with WGSL's definitions: `floor`, `round` (ties to even), `abs`, `sqrt`, `min`, `max`,
//! `clamp(e, lo, hi) = min(max(e, lo), hi)`, `mix(a, b, t) = a * (1 - t) + b * t`, `select`. All are
//! exact or correctly rounded on both sides — no transcendental function: those stay in `math`.

/// Lanes and the four operators (vector∘vector, vector∘scalar, scalar∘vector) of one vector type.
macro_rules! vector {
        ($F:ty, $V:ident, $($c:ident),+) => {
            impl Lanes for $V {
                #[inline(always)]
                fn map(self, f: impl Fn($F) -> $F) -> $V {
                    $V { $($c: f(self.$c)),+ }
                }
                #[inline(always)]
                fn zip(self, o: $V, f: impl Fn($F, $F) -> $F) -> $V {
                    $V { $($c: f(self.$c, o.$c)),+ }
                }
            }
            vector!(@op $F, $V, Add, add, +, $($c),+);
            vector!(@op $F, $V, Sub, sub, -, $($c),+);
            vector!(@op $F, $V, Mul, mul, *, $($c),+);
            vector!(@op $F, $V, Div, div, /, $($c),+);
            impl Neg for $V {
                type Output = $V;
                #[inline(always)]
                fn neg(self) -> $V {
                    $V { $($c: -self.$c),+ }
                }
            }
        };
        (@op $F:ty, $V:ident, $T:ident, $f:ident, $o:tt, $($c:ident),+) => {
            impl $T for $V {
                type Output = $V;
                #[inline(always)]
                fn $f(self, r: $V) -> $V {
                    $V { $($c: self.$c $o r.$c),+ }
                }
            }
            impl $T<$F> for $V {
                type Output = $V;
                #[inline(always)]
                fn $f(self, r: $F) -> $V {
                    $V { $($c: self.$c $o r),+ }
                }
            }
            impl $T<$V> for $F {
                type Output = $V;
                #[inline(always)]
                fn $f(self, r: $V) -> $V {
                    $V { $($c: self $o r.$c),+ }
                }
            }
        };
    }

/// The prelude a kernel text sees, for one float type `$F` (the text's `f32` is `$F`).
macro_rules! prelude_for {
    ($m:ident, $F:ty, $alias:item) => {
        #[allow(non_camel_case_types)]
        pub mod $m {
            use std::ops::{Add, Div, Mul, Neg, Sub};

            $alias

            #[derive(Clone, Copy, Debug, PartialEq)]
            pub struct vec2f {
                pub x: $F,
                pub y: $F,
            }
            #[derive(Clone, Copy, Debug, PartialEq)]
            pub struct vec3f {
                pub x: $F,
                pub y: $F,
                pub z: $F,
            }
            #[derive(Clone, Copy, Debug, PartialEq)]
            pub struct vec4f {
                pub x: $F,
                pub y: $F,
                pub z: $F,
                pub w: $F,
            }
            #[inline(always)]
            pub fn vec2f(x: $F, y: $F) -> vec2f {
                vec2f { x, y }
            }
            #[inline(always)]
            pub fn vec3f(x: $F, y: $F, z: $F) -> vec3f {
                vec3f { x, y, z }
            }
            #[inline(always)]
            pub fn vec4f(x: $F, y: $F, z: $F, w: $F) -> vec4f {
                vec4f { x, y, z, w }
            }

            /// A value the builtins apply to lane by lane.
            pub trait Lanes: Copy {
                fn map(self, f: impl Fn($F) -> $F) -> Self;
                fn zip(self, o: Self, f: impl Fn($F, $F) -> $F) -> Self;
            }
            impl Lanes for $F {
                #[inline(always)]
                fn map(self, f: impl Fn($F) -> $F) -> $F {
                    f(self)
                }
                #[inline(always)]
                fn zip(self, o: $F, f: impl Fn($F, $F) -> $F) -> $F {
                    f(self, o)
                }
            }

            vector!($F, vec2f, x, y);
            vector!($F, vec3f, x, y, z);
            vector!($F, vec4f, x, y, z, w);

            #[inline(always)]
            pub fn floor<T: Lanes>(e: T) -> T {
                e.map(<$F>::floor)
            }
            /// WGSL's `round`: ties to even.
            #[inline(always)]
            pub fn round<T: Lanes>(e: T) -> T {
                e.map(<$F>::round_ties_even)
            }
            #[inline(always)]
            pub fn abs<T: Lanes>(e: T) -> T {
                e.map(<$F>::abs)
            }
            #[inline(always)]
            pub fn sqrt<T: Lanes>(e: T) -> T {
                e.map(<$F>::sqrt)
            }
            #[inline(always)]
            pub fn min<T: Lanes>(a: T, b: T) -> T {
                a.zip(b, |a, b| if b < a { b } else { a })
            }
            #[inline(always)]
            pub fn max<T: Lanes>(a: T, b: T) -> T {
                a.zip(b, |a, b| if b > a { b } else { a })
            }
            #[inline(always)]
            pub fn clamp<T: Lanes>(e: T, lo: T, hi: T) -> T {
                min(max(e, lo), hi)
            }
            #[inline(always)]
            pub fn mix<T: Lanes + Mul<$F, Output = T> + Add<Output = T>>(a: T, b: T, t: $F) -> T {
                a * (1.0 - t) + b * t
            }
            /// WGSL's `select(f, t, cond)`: `t` when `cond`, both already evaluated.
            #[inline(always)]
            pub fn select<T>(f: T, t: T, cond: bool) -> T {
                if cond { t } else { f }
            }
        }
    };
}

prelude_for!(f64x, f64, pub type f32 = f64;);
prelude_for!(f32x, f32, pub type F = f32;);

/// One kernel text, three forms: `cpu` (the text's `f32` is f64 — the byte reference), `cpu_f32`
/// (a real f32) and `WGSL` (the same tokens, for the GPU). See the module doc for the subset.
#[macro_export]
macro_rules! shared_kernel {
    ($vis:vis mod $name:ident {
        $( fn $f:ident ( $($arg:ident : $at:ident),* $(,)? ) -> $rt:ident { $($body:tt)* } )*
    }) => {
        $vis mod $name {
            /// The text with `f32` = f64: the CPU byte reference.
            #[allow(unused_variables, unused_parens, clippy::needless_return, clippy::all)]
            pub mod cpu {
                use $crate::shade::f64x::*;
                $( #[inline(always)] pub fn $f($($arg: $at),*) -> $rt { $($body)* } )*
            }
            /// The text with a real f32: what an 8-lane f32 SIMD would compute (not the reference).
            #[allow(unused_variables, unused_parens, clippy::needless_return, clippy::all)]
            pub mod cpu_f32 {
                use $crate::shade::f32x::*;
                $( #[inline(always)] pub fn $f($($arg: $at),*) -> $rt { $($body)* } )*
            }
            /// The same tokens as WGSL, for the GPU to compile at run time.
            pub const WGSL: &str = stringify!($( fn $f ( $($arg : $at),* ) -> $rt { $($body)* } )*);
        }
    };
}

/// The SIMD levels `run_pointwise` can compile ONE kernel loop for. Every level gives the same
/// bytes: Rust emits neither FMA contraction nor reassociation, and the builtins are exact.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Isa {
    /// The baseline build (x86-64: SSE2; `floor`/`round` are libm calls there).
    Base,
    Sse41,
    Avx2,
    Avx512,
}

impl Isa {
    /// The widest level this CPU runs.
    pub fn best() -> Isa {
        #[cfg(target_arch = "x86_64")]
        {
            if std::is_x86_feature_detected!("avx512f") {
                return Isa::Avx512;
            }
            if std::is_x86_feature_detected!("avx2") {
                return Isa::Avx2;
            }
            if std::is_x86_feature_detected!("sse4.1") {
                return Isa::Sse41;
            }
        }
        Isa::Base
    }
    /// Whether this CPU runs the level.
    pub fn available(self) -> bool {
        match self {
            Isa::Base => true,
            #[cfg(target_arch = "x86_64")]
            Isa::Sse41 => std::is_x86_feature_detected!("sse4.1"),
            #[cfg(target_arch = "x86_64")]
            Isa::Avx2 => std::is_x86_feature_detected!("avx2"),
            #[cfg(target_arch = "x86_64")]
            Isa::Avx512 => std::is_x86_feature_detected!("avx512f"),
            #[cfg(not(target_arch = "x86_64"))]
            _ => false,
        }
    }
}

/// The kernel of `run_pointwise`: a straight RGBA pixel as f64 and its position → the result.
pub trait PixelKernel: Sync {
    fn px(&self, v: f64x::vec4f, x: f64, y: f64) -> f64x::vec4f;
}

impl<K: Fn(f64x::vec4f, f64, f64) -> f64x::vec4f + Sync> PixelKernel for K {
    #[inline(always)]
    fn px(&self, v: f64x::vec4f, x: f64, y: f64) -> f64x::vec4f {
        self(v, x, y)
    }
}

/// The rows `y0..` of one band: the bytes of `frame_with_at` (each channel rounded half away from
/// zero, clamped to 0..255).
#[inline(always)]
fn band<K: PixelKernel>(rows: &mut [u8], w: usize, y0: usize, k: &K) {
    for (r, row) in rows.chunks_exact_mut(w * 4).enumerate() {
        let y = (y0 + r) as f64;
        for (x, px) in row.chunks_exact_mut(4).enumerate() {
            let v = f64x::vec4f(px[0] as f64, px[1] as f64, px[2] as f64, px[3] as f64);
            let o = k.px(v, x as f64, y);
            px[0] = o.x.round().clamp(0.0, 255.0) as u8;
            px[1] = o.y.round().clamp(0.0, 255.0) as u8;
            px[2] = o.z.round().clamp(0.0, 255.0) as u8;
            px[3] = o.w.round().clamp(0.0, 255.0) as u8;
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn band_sse41<K: PixelKernel>(rows: &mut [u8], w: usize, y0: usize, k: &K) {
    band(rows, w, y0, k)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn band_avx2<K: PixelKernel>(rows: &mut [u8], w: usize, y0: usize, k: &K) {
    band(rows, w, y0, k)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn band_avx512<K: PixelKernel>(rows: &mut [u8], w: usize, y0: usize, k: &K) {
    band(rows, w, y0, k)
}

fn band_at<K: PixelKernel>(isa: Isa, rows: &mut [u8], w: usize, y0: usize, k: &K) {
    match isa {
        // SAFETY: `run_pointwise` only passes a level `Isa::available` confirmed on this CPU
        #[cfg(target_arch = "x86_64")]
        Isa::Sse41 => unsafe { band_sse41(rows, w, y0, k) },
        #[cfg(target_arch = "x86_64")]
        Isa::Avx2 => unsafe { band_avx2(rows, w, y0, k) },
        #[cfg(target_arch = "x86_64")]
        Isa::Avx512 => unsafe { band_avx512(rows, w, y0, k) },
        _ => band(rows, w, y0, k),
    }
}

/// Runs a point-wise kernel over a straight RGBA8 frame of width `w`: `threads` bands of whole rows
/// (scoped threads; 1 = this thread), each compiled for `isa` (a level the CPU lacks falls back to
/// `Base`). The bytes depend on neither: a pixel is a pure function of itself and its position.
pub fn run_pointwise<K: PixelKernel>(rgba: &mut [u8], w: u32, threads: usize, isa: Isa, k: &K) {
    let w = w.max(1) as usize;
    let isa = if isa.available() { isa } else { Isa::Base };
    let h = rgba.len() / (w * 4);
    let threads = threads.clamp(1, h.max(1));
    if threads == 1 {
        band_at(isa, rgba, w, 0, k);
        return;
    }
    let per = h.div_ceil(threads);
    std::thread::scope(|s| {
        for (i, rows) in rgba.chunks_mut(per * w * 4).enumerate() {
            s.spawn(move || band_at(isa, rows, w, i * per, k));
        }
    });
}
