//! t2191 (fused-pointwise-runs; capability `video-plan`): the plan names point-wise runs and ends them
//! where the class says. Two or more consecutive fusable stages at a barrier position are one
//! `PointRun` (audio, stream and identity Geometry slots between them are transparent); a lone stage
//! stays a `Barrier`; an active geometry step, a barrier, noise and greenscreen (a barrier kernel of
//! their own), a strength slot and a stop end a run; a run takes the K its first stage had as a
//! barrier; stages between two geometry steps stay absorbed in the series.

use kt_fx::plan::{PlanOp, plan_chain, run_shapes};
use kt_model::FxInstance;

const W: u32 = 320;
const H: u32 = 180;

fn p(id: &str) -> FxInstance {
    FxInstance::plugin(id).unwrap()
}

fn grade() -> FxInstance {
    FxInstance::plugin_with("color_grade", &[0.1, 0.2, 1.3, 15.0]).unwrap()
}

fn ops(fx: &[FxInstance], stops: &[usize]) -> Vec<String> {
    plan_chain(fx, W, H, 1, W, H, 0.5, None, 90_000, stops)
        .ops
        .iter()
        .map(|o| match o {
            PlanOp::Run { i, j, .. } => format!("geo{i}..{j}"),
            PlanOp::Barrier { idx, res_scale } => format!("bar{idx}@{res_scale}"),
            PlanOp::PointRun { i, j, res_scale } => format!("run{i}..{j}@{res_scale}"),
            PlanOp::Snapshot => "snap".into(),
            PlanOp::Mix { idx } => format!("mix{idx}"),
        })
        .collect()
}

#[test]
fn t2191_the_plan_names_point_runs_and_ends_them_by_class() {
    kt_plugins_builtin::install();
    // a run of four, a gain (audio) and an identity Geometry node inside it are transparent
    let fx = vec![grade(), p("posterize"), p("gain"), FxInstance::geometry_at(0.0, 0.0, 1.0), p("duotone"), p("vignette")];
    assert_eq!(ops(&fx, &[]), ["run0..6@1"]);
    assert_eq!(run_shapes(&fx), [vec!["color_grade", "posterize", "duotone", "vignette"]]);
    // a lone stage stays a barrier op; trailing transparent slots are not part of a run
    assert_eq!(ops(&[p("posterize"), p("gain")], &[]), ["bar0@1"]);
    assert_eq!(ops(&[p("posterize"), p("vignette"), p("gain")], &[]), ["run0..2@1"]);
    assert!(run_shapes(&[p("posterize")]).is_empty());
    // a barrier ends it, noise and greenscreen (a barrier kernel of their own) end it
    let fx = vec![p("posterize"), p("vignette"), FxInstance::blur(4), grade(), FxInstance::noise(0.3), p("duotone"), p("posterize")];
    assert_eq!(ops(&fx, &[]), ["run0..2@1", "bar2@1", "bar3@1", "bar4@1", "run5..7@1"]);
    let fx = vec![p("posterize"), FxInstance::greenscreen([0, 255, 0], 70, 25), p("vignette")];
    assert_eq!(ops(&fx, &[]), ["bar0@1", "bar1@1", "bar2@1"]);
    // an active geometry step ends it; stages between two steps are absorbed into the series
    let fx = vec![p("posterize"), p("vignette"), FxInstance::geometry_at(0.1, 0.0, 1.2), grade(), p("duotone")];
    assert_eq!(ops(&fx, &[]), ["run0..2@1", "geo2..3", "run3..5@1"]);
    let fx = vec![FxInstance::geometry_at(0.1, 0.0, 1.2), grade(), p("duotone"), FxInstance::geometry_at(0.0, 0.1, 1.1)];
    assert_eq!(ops(&fx, &[]), ["geo0..4"]);
    assert!(run_shapes(&fx).is_empty(), "absorbed stages are no run to prewarm");
    // a stop (strength snapshot, mask point) ends it
    let fx = vec![grade(), p("posterize"), p("duotone"), p("vignette")];
    assert_eq!(ops(&fx, &[2]), ["run0..2@1", "snap", "run2..4@1"]);
    assert_eq!(ops(&fx, &[1]), ["bar0@1", "snap", "run1..4@1"]);
    // a strength slot: its snapshot point cuts, its mix ends the run
    let fx = vec![grade(), p("posterize"), p("duotone"), FxInstance::LayerMix { amount: 0.5, from: 1 }, p("vignette"), p("posterize")];
    assert_eq!(run_shapes(&fx), [vec!["posterize", "duotone"], vec!["vignette", "posterize"]]);
    // K×: a run between a compressing series and a zooming tail (behind a barrier, so the series
    // does not absorb it) takes the K a lone stage takes there
    let squeeze = FxInstance::geometry_at(0.0, 0.0, 0.25);
    let zoom = FxInstance::geometry_at(0.0, 0.0, 4.0);
    let lone = vec![squeeze.clone(), FxInstance::blur(2), p("posterize"), zoom.clone()];
    let k = match plan_chain(&lone, W, H, 1, W, H, 0.5, None, 0, &[]).ops.as_slice() {
        [PlanOp::Run { .. }, PlanOp::Barrier { .. }, PlanOp::Barrier { res_scale, .. }, PlanOp::Run { .. }] if *res_scale > 1 => *res_scale,
        other => panic!("a lone tap after a squeeze, before a zoom, at K>1: {} ops", other.len()),
    };
    let run = vec![squeeze, FxInstance::blur(2), p("posterize"), p("vignette"), grade(), zoom];
    let got = ops(&run, &[]);
    assert_eq!(got[2], format!("run2..5@{k}"), "{got:?}");
}
