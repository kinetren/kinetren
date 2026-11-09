//! test-hygiene: a start's first pass reaches the output device whole. t2216. A test binary of its
//! own: the seam it sets is process-wide.

use kt_engine::Player;
use kt_engine::preview_bench::{BenchOptions, play_range};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;

/// Test t2216 (found-defects 158): the callback primes on the first frames in the ring, and from then
/// on time runs. The start's pass was pushed burst by burst, so a render thread put off between two
/// bursts let the device eat the first ones and find the ring empty before it had ever been full:
/// t1935's two underruns at load 192, the device on time. Here every burst of a start's pass holds
/// the render thread 30 ms (a burst is 21 ms of sound): pushed as it went, the device outran it.
#[test]
fn t2216_a_starts_pass_reaches_the_device_whole() {
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
    let mut c = ClipEvent {
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
        start: Tick::ZERO,
        duration: Tick(90_000 * 20),
        source_in: Tick::ZERO,
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    c.fx = vec![];
    p.clips.push(c);
    p.ensure_tracks(LaneKind::Audio, 1);
    p.validate().expect("scene guard: the project is legal");
    let (player, ring) = Player::new(
        p,
        vec![(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 3600), AudioPattern::Sine { hz: 330.0 })) as _)],
    );
    kt_engine::hold_start_bursts_for_tests(30);
    let r = play_range(
        &player,
        ring,
        BenchOptions { from: Tick(90_000), to: Tick(90_000 * 3), ..Default::default() },
    );
    kt_engine::hold_start_bursts_for_tests(0);
    eprintln!(
        "callback underruns {}, the device at most {:.0} ms late, render underruns {}",
        r.callback_underruns, r.device_late_max_ms, r.underruns
    );
    assert_eq!(r.callback_underruns, 0, "the device found the ring empty during the start's pass");
}
