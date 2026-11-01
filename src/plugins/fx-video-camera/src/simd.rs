//! The AVX2 twin (design D8): the SAME row functions of `cpu.rs`, inlined into functions compiled
//! for AVX2. Rust never contracts `a*b+c` into FMA nor re-associates floats, so the vectorised loops
//! give the scalar bytes by construction; t1053 (every barrier) and t1216 hold it. The host decides
//! when they may run (`BarrierCtx::simd`, the ONE switch).

#![cfg(target_arch = "x86_64")]

use crate::cpu::{
    Ctx, Row, chroma_row, demosaic_row, lens_correct_row, optics_row, pixel_row, raw_correct_row,
    raw_front_row, raw_nr_row, readout_row, unprocess_row, view_row, yuv_row,
};

macro_rules! twin {
    ($safe:ident, $avx:ident, $row:ident) => {
        #[target_feature(enable = "avx2")]
        unsafe fn $avx(cx: &Ctx, y: u32, out: &mut [u32]) {
            $row(cx, y, out)
        }
        fn $safe(cx: &Ctx, y: u32, out: &mut [u32]) {
            // SAFETY: the host sets `BarrierCtx::simd` only when the CPU has AVX2 (`simd_level()`)
            unsafe { $avx(cx, y, out) }
        }
    };
}

twin!(t00, a00, unprocess_row);
twin!(t01, a01, optics_row);
twin!(t02, a02, pixel_row);
twin!(t03, a03, readout_row);
twin!(t04, a04, raw_front_row);
twin!(t05, a05, raw_nr_row);
twin!(t06, a06, raw_correct_row);
twin!(t07, a07, demosaic_row);
twin!(t08, a08, lens_correct_row);
twin!(t09, a09, yuv_row);
twin!(t10, a10, chroma_row);
twin!(t11, a11, view_row);

/// The twins in pass order (`crate::pass`).
pub(crate) const TWINS: [Row; 12] = [t00, t01, t02, t03, t04, t05, t06, t07, t08, t09, t10, t11];
