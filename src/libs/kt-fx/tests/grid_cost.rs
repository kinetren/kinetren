//! The cost of an event grid (morpher-event-grid-cost).
//!
//! The owner (ktprompts 26.10.18 item 8): an event grid with automation 0 → 12×30 → 12×10 → 0
//! drops fps — "it is basically a simple pixel shader, so do we have extra computations?". The
//! cost is counted, not timed: `GeomProgram::sample_cost` is a pure function of the program, and
//! the executor reports what it ran through `FxScratch::cost`.

use kt_fx::geom::{GeomExec, build_geom_exec};
use kt_fx::video::{FrameCost, FxScratch, VideoFxCtx, apply_video_chain_pooled};
use kt_model::FxInstance;

const W: u32 = 960;
const H: u32 = 540;

fn grid(cols: u32, rows: u32) -> FxInstance {
    FxInstance::EventGrid { cols, rows, center: false }
}

fn program_cost(f: &FxInstance) -> u64 {
    match build_geom_exec(&[f], W, H, W, H, 0.5, None, 0, 1, 1) {
        GeomExec::Fused(p) => p.sample_cost().expect("an affine grid has a constant tap count"),
        GeomExec::Legacy => panic!("the event grid always takes the fused pass"),
    }
}

/// Test 942 (morpher-event-grid-cost, D1): the sub-samples of the grid's pass, as the program
/// states them and as the CPU executor counts them.
///
/// Before the tile rule every output pixel of a grid with 4 or more cells per axis reads
/// 4×4 = 16 sub-samples, so 12×10 and 12×30 cost the same 8.3 M at 960×540 — the 16× of the
/// short answer, and at 12×30 359 of every 360 tiles are repeats.
#[test]
fn t942_grid_cost_is_counted() {
    let px = W as u64 * H as u64;
    for (cols, rows, want) in
        [(1, 1, px), (2, 2, px * 4), (12, 10, px * 16), (12, 30, px * 16)]
    {
        let f = grid(cols, rows);
        let got = program_cost(&f);
        assert_eq!(got, want, "{cols}×{rows}: sub-samples per frame");

        // the executor reports what it ran: one pass, the program's sub-samples
        let ctx = VideoFxCtx {
            out_w: W,
            out_h: H,
            u: 0.5,
            burst_index: 0,
            cam3d: None,
            in_scale: 1,
            lead: None,
        };
        let src = kt_fx::video::Frame { w: W, h: H, rgba: vec![128; (px * 4) as usize], has_alpha: false };
        let mut scratch = FxScratch::default();
        let _ = apply_video_chain_pooled(&[f], src, &ctx, &mut scratch);
        assert_eq!(
            scratch.cost,
            FrameCost { passes: 1, subsamples: got, ..FrameCost::default() },
            "{cols}×{rows}: the CPU executor's count"
        );
    }
}
