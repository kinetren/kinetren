//! t2127 (pipeline-prewarm D5; capability `video-plan`): the player hands the executor the effects
//! the project reaches — when the executor arrives, and when an edit brings an effect in — and asks
//! nothing for an edit that leaves the set as it was, so a project at rest starts no compile. The
//! walk sees the chains of tracks and of compositions.

use kt_engine::{FramePlan, GpuBackendInfo, GpuExecError, Player, VideoGpuExec};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;
use std::sync::{Arc, Mutex};

struct Recorder {
    info: GpuBackendInfo,
    asked: Mutex<Vec<Vec<String>>>,
    runs: Mutex<Vec<Vec<Vec<String>>>>,
}

impl VideoGpuExec for Recorder {
    fn execute_into(&self, _p: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, GpuExecError> {
        out.clear();
        Ok(true)
    }
    fn info(&self) -> &GpuBackendInfo {
        &self.info
    }
    fn prewarm(&self, ids: &[String]) {
        self.asked.lock().unwrap().push(ids.to_vec());
    }
    fn prewarm_runs(&self, runs: &[Vec<String>]) {
        self.runs.lock().unwrap().push(runs.to_vec());
    }
}

fn project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 25,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: true,
    });
    p.media.push(MediaRef::new(MediaId(1), "src"));
    let mut t = Track::new(TrackId(1), LaneKind::Video);
    t.fx.push(FxInstance::plugin("glow").unwrap());
    p.tracks.push(t);
    p
}

#[test]
fn t2127_the_player_prewarms_what_the_project_reaches_when_it_changes() {
    kt_plugins_builtin::install();
    let src = SyntheticSource::new(Tick(600_000), AudioPattern::Silence);
    let mut p = project();
    let (player, _ring) = Player::new(p.clone(), vec![(MediaId(1), Box::new(src))]);
    let exec = Arc::new(Recorder { info: GpuBackendInfo { name: "rec".into(), backend: "Test".into(), software: true }, asked: Mutex::default(), runs: Mutex::default() });
    player.set_video_gpu(Some(exec.clone()));
    let asked = || exec.asked.lock().unwrap().clone();
    assert_eq!(asked(), [vec!["glow".to_string()]], "the executor is told the project's effects when it arrives");

    // an edit that leaves the set as it was asks nothing
    p.tracks[0].opacity = 0.5;
    player.set_project(p.clone());
    assert_eq!(asked().len(), 1, "an edit without a new effect asked the executor");

    // an effect inside a composition comes in
    let mut seq = Sequence::new(MediaId(50), "reel");
    let mut inner = Track::new(TrackId(2), LaneKind::Video);
    inner.fx.push(FxInstance::plugin("vhs_tape").unwrap());
    seq.tracks.push(inner);
    p.sequences.push(seq);
    player.set_project(p.clone());
    let last = asked().last().cloned().unwrap();
    assert!(asked().len() == 2 && last.contains(&"vhs_tape".to_string()) && last.contains(&"glow".to_string()), "{:?}", asked());
}

/// t2196 (fused-pointwise-runs; capability `video-plan`): the player hands the executor the point-wise
/// run shapes the project reaches with its effect ids — a track's three-stage run, a clip's two-stage
/// run inside a strength slot — and asks again only when the set changed.
#[test]
fn t2196_the_player_prewarms_the_run_shapes() {
    kt_plugins_builtin::install();
    let src = SyntheticSource::new(Tick(600_000), AudioPattern::Silence);
    let mut p = project();
    p.tracks[0].fx = vec![FxInstance::plugin("color_grade").unwrap(), FxInstance::plugin("posterize").unwrap(), FxInstance::plugin("vignette").unwrap()];
    let (player, _ring) = Player::new(p.clone(), vec![(MediaId(1), Box::new(src))]);
    let exec = Arc::new(Recorder {
        info: GpuBackendInfo { name: "rec".into(), backend: "Test".into(), software: true },
        asked: Mutex::default(),
        runs: Mutex::default(),
    });
    player.set_video_gpu(Some(exec.clone()));
    let runs = || exec.runs.lock().unwrap().clone();
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<String>>();
    assert_eq!(runs(), [vec![s(&["color_grade", "posterize", "vignette"])]]);
    // a second run, inside a strength slot of a composition's track
    let mut seq = Sequence::new(MediaId(50), "reel");
    let mut inner = Track::new(TrackId(2), LaneKind::Video);
    inner.fx = vec![
        FxInstance::plugin("duotone").unwrap(),
        FxInstance::plugin("posterize").unwrap(),
        FxInstance::plugin("vignette").unwrap(),
        FxInstance::LayerMix { amount: 0.5, from: 1 },
    ];
    seq.tracks.push(inner);
    p.sequences.push(seq);
    player.set_project(p.clone());
    assert_eq!(runs().len(), 2);
    let last = runs().last().cloned().unwrap();
    assert!(last.contains(&s(&["posterize", "vignette"])) && last.contains(&s(&["color_grade", "posterize", "vignette"])), "{last:?}");
    assert_eq!(last.len(), 2, "duotone alone before the strength stop is no run: {last:?}");
    // an edit that keeps both sets asks nothing
    p.tracks[0].opacity = 0.4;
    player.set_project(p.clone());
    assert_eq!(runs().len(), 2, "an edit without a new run asked the executor");
    let shapes = kt_engine::prewarm::run_shapes(&p);
    assert_eq!(shapes.len(), 2);
}
