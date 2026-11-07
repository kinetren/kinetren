//! Test t1875 (input-without-work, capability `transport`): the UI's own audio renderer — the
//! meters on a pause and the loop's pinned head — is built once per project version. Each change
//! of the loop region built a whole one on the UI thread: 447 ms on the owner's 1974 events.
//!
//! Its own test binary: `kt_time::work` counts process-wide, and no other player may run beside it.

use kt_engine::Player;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::work::{self, Work};
use kt_time::{Tick, TickRange};

fn project() -> Project {
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
        gain_db: 0.0,
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

fn builds() -> u64 {
    work::totals()[Work::EngineBuild as usize]
}

/// The engine threads have built what the opening asks for: the count stands for a while.
fn settle() {
    let deadline = kt_time::test_deadline();
    let mut last = builds();
    let mut still = 0;
    while still < 5 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let now = builds();
        still = if now == last { still + 1 } else { 0 };
        last = now;
    }
}

#[test]
fn t1875_the_ui_renderer_is_built_once_per_project_version() {
    let src = Box::new(SyntheticSource::new(Tick(90_000 * 8), AudioPattern::Sine { hz: 440.0 }));
    let (player, _consumer) = Player::new(project(), vec![(MediaId(1), src)]);
    settle();
    let before = builds();
    for i in 0..5 {
        player.set_loop_region(Some(TickRange::new(Tick(i * 9_000), Tick(90_000 + i * 9_000))));
        let _ = player.sample_levels_at(Tick(i * 1_000));
    }
    assert_eq!(builds() - before, 1, "five region changes and five meter readings on one project version");
    // a new project is a new version: the next region change builds again, once
    let mut p = project();
    p.clips[0].gain_db = -3.0;
    player.set_project(p);
    settle();
    let before = builds();
    player.set_loop_region(Some(TickRange::new(Tick(0), Tick(90_000))));
    player.set_loop_region(Some(TickRange::new(Tick(9_000), Tick(90_000))));
    assert_eq!(builds() - before, 1);
}
