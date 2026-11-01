//! The AVX2 twin (design D8): the SAME row functions of `cpu.rs`, inlined into functions compiled
//! for AVX2. Rust never contracts `a*b+c` into FMA nor re-associates floats, so the vectorised loops
//! give the scalar bytes by construction; t1053 (every barrier) and t1216 hold it. The host decides
//! when they may run (`BarrierCtx::simd`, the ONE switch).

#![cfg(target_arch = "x86_64")]

use crate::cpu::{Ctx, p0_row, p1_row, p2_row, p3_row, p4_row, p5_row};

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

twin!(p0, p0_avx2, p0_row);
twin!(p1, p1_avx2, p1_row);
twin!(p2, p2_avx2, p2_row);
twin!(p3, p3_avx2, p3_row);
twin!(p4, p4_avx2, p4_row);
twin!(p5, p5_avx2, p5_row);

/// The twins in pass order.
pub(crate) const TWINS: [fn(&Ctx, u32, &mut [u32]); 6] = [p0, p1, p2, p3, p4, p5];
