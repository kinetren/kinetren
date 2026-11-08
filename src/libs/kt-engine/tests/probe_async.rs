//! Test t1844 (ui-never-waits, capability `transport`): the UI's own audio renderer of a new project
//! version is built on a thread of its own; the meters and the loop region answer from the one
//! before meanwhile. A build takes 412-447 ms on the owner's 1974 events, and these run on the UI
//! thread. The builder is held at its start by the test, so "does not wait" is a fact: a call that
//! waited for the build could never return.

use kt_engine::Player;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{Tick, TickRange};

fn project(gain_db: f32) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000 * 4),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = kt_time::test_deadline();
    while !cond() {
        assert!(std::time::Instant::now() < deadline, "{what}");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn t1844_a_new_version_of_the_ui_renderer_is_built_off_the_ui_thread() {
    let src = Box::new(SyntheticSource::new(Tick(90_000 * 8), AudioPattern::Sine { hz: 440.0 }));
    let (player, _consumer) = Player::new(project(0.0), vec![(MediaId(1), src)]);
    // the first is built where it is asked: there is nothing to answer with before it
    let _ = player.sample_levels_at(Tick(0));
    player.gate_probe_builds(true);
    player.set_project(project(-6.0));
    let version = player.project_version();
    // the meters and the loop region on the new version, with its build held at its start
    for i in 0..3 {
        player.set_loop_region(Some(TickRange::new(Tick(i * 9_000), Tick(90_000 + i * 9_000))));
        let _ = player.sample_levels_at(Tick(i * 1_000));
    }
    assert_ne!(player.probe_version(), Some(version), "the held build landed");
    player.gate_probe_builds(false);
    wait_until("the probe of the new version was never built", || player.probe_version() == Some(version));
}
