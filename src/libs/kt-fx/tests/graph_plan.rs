//! fx-graph-dag D3 (capability `video-plan`): a chain graph is lowered by the one planner into a
//! straight program — `Hold`/`Load`/`Merge` at the graph's positions, every one of them a stop, and a
//! run between two stops planned as the line made of its slots.

use kt_fx::plan::{PlanOp, lower, plan_chain, plan_graph_in};
use kt_model::fx_graph::{FxIn, SlotInputs};
use kt_model::{BlendMode, FxInstance, MergeMode};

const W: u32 = 320;
const H: u32 = 180;

fn p(id: &str) -> FxInstance {
    FxInstance::plugin(id).unwrap()
}

fn merge() -> FxInstance {
    FxInstance::Merge { mode: MergeMode::Blend(BlendMode::Screen), mix: 1.0 }
}

fn main(m: FxIn) -> SlotInputs {
    SlotInputs { main: m, ..Default::default() }
}

fn names(ops: &[PlanOp]) -> Vec<String> {
    ops.iter()
        .map(|o| match o {
            PlanOp::Run { i, j, .. } => format!("geo{i}..{j}"),
            PlanOp::Barrier { idx, res_scale } => format!("bar{idx}@{res_scale}"),
            PlanOp::PointRun { i, j, res_scale } => format!("run{i}..{j}@{res_scale}"),
            PlanOp::Snapshot => "snap".into(),
            PlanOp::Mix { idx } => format!("mix{idx}"),
            PlanOp::Hold { reg, at } => format!("hold{reg}@{at}"),
            PlanOp::Load { reg, at } => format!("load{reg}@{at}"),
            PlanOp::Merge { idx, fg, .. } => format!("merge{idx}<{fg}"),
            PlanOp::Input { k, at } => format!("read{k}@{at}"),
        })
        .collect()
}

fn graph_ops(fx: &[FxInstance], fx_in: &[SlotInputs]) -> Vec<String> {
    let g = lower(fx, fx_in);
    names(&plan_graph_in(fx, g.as_ref(), &[], W, H, 1, W, H, 0.5, None, 90_000, &[], None).ops)
}

/// Test t3203 (fx-graph-dag D3): the lowering's program for a branch after a zoom, a blur on one
/// arm, a zoom and a glow on the other, merged; a line lowers to no graph at all; each run between
/// the stops is the plan of the line of its slots; a dangling slot runs nothing.
#[test]
fn t3203_a_graph_lowers_to_a_straight_program() {
    kt_plugins_builtin::install();
    let zoom = |z: f64| FxInstance::geometry_at(0.0, 0.0, z);
    // 0 zoom → 1 blur ; 2 zoom (from 0) → 3 glow ; 4 merge(bg 1, fg 3)
    let fx = vec![zoom(1.5), FxInstance::blur(6), zoom(2.0), FxInstance::glow(0.5), merge()];
    let fx_in = vec![
        SlotInputs::default(),
        SlotInputs::default(),
        main(FxIn::Slot(0)),
        SlotInputs::default(),
        SlotInputs { main: FxIn::Slot(1), second: Some(FxIn::Prev), matte: None },
    ];
    assert_eq!(
        graph_ops(&fx, &fx_in),
        ["geo0..1", "hold0@1", "bar1@1", "hold1@2", "load0@2", "geo2..3", "bar3@1", "hold0@4", "load1@4", "merge4<0"],
        "the lowered program"
    );
    // a line has no graph: the plan is plan_chain's, op for op
    assert!(lower(&fx, &[]).is_none());
    assert!(lower(&fx, &[SlotInputs::default(), main(FxIn::Slot(0))]).is_none(), "a canonical line lowered to a graph");
    // the runs between stops are the line plans of their slots: the arm [zoom 2, glow] alone
    let arm = vec![zoom(2.0), FxInstance::glow(0.5)];
    assert_eq!(names(&plan_chain(&arm, W, H, 1, W, H, 0.5, None, 90_000, &[]).ops), ["geo0..1", "bar1@1"]);
    // a dangling slot (read by nothing on the way to the Output) is skipped
    let dangling = vec![FxInstance::blur(6), p("edges"), FxInstance::glow(0.5)];
    let w = vec![SlotInputs::default(), main(FxIn::Source), main(FxIn::Slot(0))];
    let ops = graph_ops(&dangling, &w);
    assert!(!ops.iter().any(|o| o.starts_with("bar1")), "the dangling edges ran: {ops:?}");
    assert_eq!(ops, ["bar0@1", "bar2@1"], "a dangling slot left graph steps");
    // a Merge with no Foreground passes its Background (here the Source): the blur reaches nothing
    let lone = vec![FxInstance::blur(6), merge()];
    let ops = graph_ops(&lone, &[SlotInputs::default(), main(FxIn::Source)]);
    assert!(!ops.iter().any(|o| o.starts_with("bar") || o.starts_with("merge")), "a lone Merge ran something: {ops:?}");
}
