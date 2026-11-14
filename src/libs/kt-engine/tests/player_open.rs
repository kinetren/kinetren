//! scheduler-followups D1 (found-defects 270): a player made on the editor's UI thread builds its
//! first version on its own thread. t2331. A test binary of its own: it holds every build.

use kt_engine::Player;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;

const SEC: i64 = 48_000;
/// How long the builder thread holds every build: a large project's first build, on any machine.
const BUILD_HOLD_MS: u64 = 500;

fn clip(id: u64, start: i64, dur: i64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane: (id % 4) as u32,
        start: Tick::from_audio_frames(start),
        duration: Tick::from_audio_frames(dur),
        source_in: Tick::ZERO,
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn project(n: u64) -> Project {
    kt_plugins_builtin::install();
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:sine"));
    p.clips = (0..n).map(|i| clip(i + 1, i as i64 * SEC / 8, SEC / 4)).collect();
    p.ensure_tracks(LaneKind::Audio, 4);
    p.validate().expect("scene guard: the project is legal");
    p
}

/// t2331: a player made on a thread marked as the UI thread counts no build there; Play pressed
/// before its version lands renders nothing (time stands before the first frame), and the first
/// sound after it is the project's.
#[test]
fn t2331_the_first_version_is_built_off_the_caller() {
    let p = project(400);
    kt_engine::hold_builds_for_tests(BUILD_HOLD_MS);
    kt_time::block::mark(kt_time::block::role::UI);
    let before = kt_time::block::here();
    let (player, mut ring) = Player::new_deferred(p.clone());
    let made = kt_time::block::here();
    kt_time::block::mark(kt_time::block::role::NONE);
    let waited: Vec<String> = (0..kt_time::block::kind::COUNT)
        .filter(|&k| made[k] != before[k])
        .map(|k| format!("{} ×{}", kt_time::block::kind::name(k as u32), made[k] - before[k]))
        .collect();
    assert!(waited.is_empty(), "making the player waited on its caller: {waited:?}");
    assert_eq!(player.built_seq(), 0, "scene guard: the version was built before the test could look");
    player.add_media(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 330.0 })));
    player.play();
    let deadline = kt_time::test_deadline();
    // while the version is being built the ring stays empty: nothing is rendered from the empty one
    while player.built_seq() == 0 {
        assert_eq!(ring.slots(), 0, "a start rendered before the project's version landed");
        assert!(std::time::Instant::now() < deadline, "the first version never landed");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(player.built().project.clips.len(), p.clips.len(), "the published version is not the project's");
    // the first frames handed out are the project's sound (its sine), not the empty version's silence
    let mut first = Vec::new();
    while first.len() < 2048 {
        assert!(std::time::Instant::now() < deadline, "the start never rendered");
        while first.len() < 2048
            && let Ok(v) = ring.pop()
        {
            first.push(v);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(first.iter().any(|v| v.abs() > 0.01), "the first pass after the build was silence");
    kt_engine::hold_builds_for_tests(0);
}

/// t2339 (the class of t2331): an edit asked on a pause holds the next start, however late the
/// render thread notices it. A player of an empty project is built; with every build held, a version
/// with the events is asked on the pause and Play pressed at once: nothing is rendered from the
/// former (silent) version, and the first sound is the edit's. The hold was decided by whether the
/// player played when the render thread NOTICED the edit, so under load (that thread late to its
/// loop) the former version played.
#[test]
fn t2339_an_edit_asked_on_a_pause_holds_the_start() {
    let p = project(400);
    let mut empty = p.clone();
    empty.clips.clear();
    let (player, mut ring) = Player::new(
        empty,
        vec![(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 330.0 })))],
    );
    let built = player.wait_built().seq;
    kt_engine::hold_builds_for_tests(BUILD_HOLD_MS);
    player.set_project(p.clone());
    player.play();
    let deadline = kt_time::test_deadline();
    while player.built_seq() == built {
        assert_eq!(ring.slots(), 0, "a start rendered the former version while the edit was being built");
        assert!(std::time::Instant::now() < deadline, "the edit's version never landed");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let mut first = Vec::new();
    while first.len() < 2048 {
        assert!(std::time::Instant::now() < deadline, "the start never rendered");
        while first.len() < 2048
            && let Ok(v) = ring.pop()
        {
            first.push(v);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(first.iter().any(|v| v.abs() > 0.01), "the first pass after the edit was the former version's silence");
    kt_engine::hold_builds_for_tests(0);
}
