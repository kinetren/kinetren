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
    let exec = Arc::new(Recorder { info: GpuBackendInfo { name: "rec".into(), backend: "Test".into(), software: true }, asked: Mutex::default() });
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
