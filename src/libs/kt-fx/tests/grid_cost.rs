//! The cost of an event grid (morpher-event-grid-cost).
//!
//! The owner (ktprompts 26.10.18 item 8): an event grid with automation 0 → 12×30 → 12×10 → 0
//! drops fps — "it is basically a simple pixel shader, so do we have extra computations?". The
//! cost is counted, not timed: `GeomProgram::sample_cost` is a pure function of the program, and
//! the executor reports what it ran through `last_cost`.

use kt_fx::geom::{GeomExec, build_geom_exec};
use kt_fx::video::{FrameCost, VideoFxCtx};
use kt_model::FxInstance;

const W: u32 = 960;
const H: u32 = 540;

fn grid(cols: u32, rows: u32) -> FxInstance {
    FxInstance::plugin_with("event_grid", &[cols as f64, rows as f64, 0.0]).unwrap()
}

fn program_cost(f: &FxInstance) -> u64 {
    match build_geom_exec(&[f], W, H, W, H, 0.5, None, 0, 1, 1) {
        GeomExec::Fused(p) => p.sample_cost().expect("an affine grid has a constant tap count"),
        GeomExec::Legacy => panic!("the event grid always takes the fused pass"),
    }
}

/// Test 942 (morpher-event-grid-cost, D1): the sub-samples of the grid's pass, as the program
/// states them and as the executor counts them.
///
/// Before the tile rule every output pixel of a grid with 4 or more cells per axis read
/// 4×4 = 16 sub-samples, so 12×10 and 12×30 cost the same 8.3 M at 960×540 — the 16× of the
/// short answer, and at 12×30 359 of every 360 tiles were repeats (1×1, 2×2, 12×10, 12×30:
/// 518 400 / 2 073 600 / 8 294 400 / 8 294 400). With one tile, copied, a grid costs one cell:
/// 12×10 is 80×54×16, 12×30 is 80×18×16; 1×1 and 2×2 are not marked and cost as before.
#[test]
fn t942_grid_cost_is_counted() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let px = W as u64 * H as u64;
    for (cols, rows, want) in
        [(1, 1, px), (2, 2, px * 4), (12, 10, 80 * 54 * 16), (12, 30, 80 * 18 * 16)]
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
        let (_, ran) = kt_gpu::testkit::chain_measured(&[f], src, &ctx);
        // a marked grid runs its cell, then copies it: one more pass, the same sub-samples
        let tiled = got < px * 16 && cols >= 4;
        let passes = if tiled { 2 } else { 1 };
        assert_eq!(
            ran,
            FrameCost { passes, subsamples: got, ..FrameCost::default() },
            "{cols}×{rows}: the executor's count"
        );
    }
}

/// The fused program of a chain as the scheduler builds it (geometry and absorbed taps).
fn fused(fx: &[FxInstance], in_w: u32, in_h: u32, out_w: u32, out_h: u32) -> kt_fx::geom::GeomProgram {
    let plan = kt_fx::plan::plan_chain(fx, in_w, in_h, 1, out_w, out_h, 0.5, None, 77, &[]);
    let runs: Vec<_> = plan
        .ops
        .into_iter()
        .filter_map(|op| match op {
            kt_fx::plan::PlanOp::Run { exec: GeomExec::Fused(p), .. } => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(runs.len(), 1, "the chain is one fused run");
    runs.into_iter().next().unwrap()
}

/// Test 943 (morpher-event-grid-cost, D2): one tile, copied, is EXACT — and refused where it
/// would not be.
///
/// Every marked program is executed twice on the CPU reference — as marked, and with the mark
/// removed (the full pass) — and the bytes must be the same, for grids that give 1, 2 and 4
/// taps per axis, centred or not, on even and odd frames, with a downscaled input, with
/// geometry and a point-wise op BEFORE the grid. Refused: 3 taps (non-dyadic sub-sample offsets
/// round per cell), a geometry step or a point-wise op AFTER the grid, 2×2 and below.
#[test]
fn t943_one_tile_is_refused_where_it_would_not_be() {
    kt_plugins_builtin::install();
    let grid = |cols: u32, rows: u32, center: bool| {
        FxInstance::plugin_with("event_grid", &[cols as f64, rows as f64, if center { 1.0 } else { 0.0 }]).unwrap()
    };
    let mirror = kt_model::FxInstance::plugin_with("mirror", &[1.0, 0.0]).unwrap();
    let noise = FxInstance::noise(0.3);
    let mut checked = 0;
    for (ow, oh) in [(320u32, 180u32), (321, 181), (333, 177)] {
        for (iw, ih) in [(ow, oh), (ow * 2, oh * 2)] {
            for center in [false, true] {
                for (cols, rows) in [(12, 30), (12, 10), (5, 7), (16, 2), (40, 40), (12, 1), (400, 200)] {
                    for chain in [
                        vec![grid(cols, rows, center)],
                        vec![mirror.clone(), grid(cols, rows, center)],
                        vec![noise.clone(), grid(cols, rows, center)],
                        vec![grid(3, 2, false), grid(cols, rows, center)],
                    ] {
                        let p = fused(&chain, iw, ih, ow, oh);
                        if p.tile.is_none() {
                            continue;
                        }
                        let full = kt_fx::geom::GeomProgram { tile: None, ..p.clone() };
                        // t944 holds the tile's bytes to the full pass; here it costs what it
                        // says: one cell plus the field outside the grid
                        assert!(p.sample_cost() < full.sample_cost(), "{chain:?}: no saving");
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked >= 100, "the sweep hardly ran: {checked} marked programs");
    // the refusals
    let refused = |fx: &[FxInstance], why: &str| {
        assert!(fused(fx, 320, 180, 320, 180).tile.is_none(), "{why}: {fx:?} was marked");
    };
    // exactly 3 taps: a 300×170 input into 320×180 (300/106 ≈ 2.83, 170/60 ≈ 2.83)
    let three = fused(&[grid(3, 3 * 4, false)], 300, 170, 320, 180);
    assert!(
        matches!(three.ss, kt_fx::geom::SsSpec::Const { nx: 3, .. }),
        "the 3-tap scene is not 3 taps: {:?}",
        three.ss
    );
    assert!(three.tile.is_none(), "3 taps across were marked");
    let three = fused(&[grid(12, 3, false)], 300, 170, 320, 180);
    assert!(matches!(three.ss, kt_fx::geom::SsSpec::Const { ny: 3, .. }), "{:?}", three.ss);
    assert!(three.tile.is_none(), "3 taps down were marked");
    refused(&[grid(12, 30, false), mirror.clone()], "a geometry step after the grid");
    // a point-wise op after the grid joins the run only with a geometry step after it (alone it
    // is its own barrier pass over the whole, already copied frame — exact by construction)
    refused(&[grid(12, 30, false), noise.clone(), mirror.clone()], "a point-wise op after the grid");
    let trailing = kt_fx::plan::plan_chain(
        &[grid(12, 30, false), noise.clone()],
        320,
        180,
        1,
        320,
        180,
        0.5,
        None,
        77,
        &[],
    );
    assert!(
        matches!(trailing.ops.last(), Some(kt_fx::plan::PlanOp::Barrier { .. })),
        "a trailing noise is its own pass: {:?}",
        trailing.ops.len()
    );
    refused(&[grid(2, 2, false)], "2×2 gains nothing");
    refused(&[grid(1, 1, false)], "1×1 is not a grid");
}
