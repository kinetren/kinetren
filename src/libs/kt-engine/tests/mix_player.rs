//! cached-range-plays: the player through the final-mix store. t1935. A test binary of its own:
//! it reads the process-wide work counters.

use kt_engine::Player;
use kt_engine::preview_bench::{BenchOptions, play_range};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;

const SEC: i64 = 48_000;

fn clip(id: u64, start: i64, dur: i64, fx: Vec<FxInstance>) -> ClipEvent {
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
        fx,
        opacity: 1.0,
        fx_off: false,
    }
}

/// A large edit: many short events with a reverb, as the owner's 1974.
fn big_project(n: u64) -> Project {
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
    let fx = vec![
        FxInstance::plugin_with("slowed_reverb", &[0.8, 0.4, 60.0, 0.4, 0.7, 1.0, 0.0, 0.0, 1.0]).unwrap(),
    ];
    p.clips = (0..n).map(|i| clip(i + 1, i as i64 * SEC / 8, SEC / 4, fx.clone())).collect();
    // a composition placed many times far from the played range, as the owner's reels: each
    // instance's revision is a hash of the reel, so a rebuild of the renderer is long
    let mut reel = Sequence::new(MediaId(100), "Reel");
    reel.clips = (0..3000u64).map(|i| clip(100_000 + i, i as i64 * SEC / 8, SEC / 4, fx.clone())).collect();
    reel.ensure_tracks(LaneKind::Audio, 4);
    p.sequences.push(reel);
    for k in 0..400u64 {
        let mut inst = clip(200_000 + k, (n as i64 + 100 + k as i64) * SEC, SEC / 2, vec![]);
        inst.media = MediaId(100);
        p.clips.push(inst);
    }
    p.ensure_tracks(LaneKind::Audio, 4);
    p.validate().expect("scene guard: the project is legal");
    p
}

fn player(p: &Project) -> (Player, rtrb::Consumer<f32>) {
    Player::new(
        p.clone(),
        vec![(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 3600), AudioPattern::Sine { hz: 330.0 })) as _)],
    )
}

/// t1935: a covered loop plays from the kept mix — after the first laps no burst runs the effects —
/// and edits during the play do not stop the sound: the renderer is rebuilt off the render thread.
#[test]
fn t1935_covered_loop_replays_and_edits_do_not_stop_the_sound() {
    let p = big_project(2000);
    let t0 = std::time::Instant::now();
    let _ = kt_engine::AudioRenderer::new(&p);
    let build = t0.elapsed();
    let (player, ring) = player(&p);
    let from = Tick::from_audio_frames(10 * SEC);
    let to = Tick::from_audio_frames(10 * SEC + SEC / 2);
    let live0 = kt_time::work::count(kt_time::work::Work::AudioRender);
    let kept0 = kt_time::work::count(kt_time::work::Work::MixReplay);
    std::thread::scope(|s| {
        let edits = s.spawn(|| {
            // edits of an event far from the region, while the loop plays
            std::thread::sleep(std::time::Duration::from_millis(1500));
            for k in 0..4 {
                let mut q = p.clone();
                q.clips[1500].gain_db = -1.0 - k as f32;
                player.set_project(q);
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
        });
        let r = play_range(
            &player,
            ring,
            BenchOptions {
                from,
                to,
                loop_for: Some(std::time::Duration::from_secs(4)),
                settle: std::time::Duration::from_millis(2000),
                ..Default::default()
            },
        );
        edits.join().unwrap();
        let live = kt_time::work::count(kt_time::work::Work::AudioRender) - live0;
        let kept = kt_time::work::count(kt_time::work::Work::MixReplay) - kept0;
        eprintln!("build {build:?}, live {live}, kept {kept}, callback underruns {}, render underruns {}", r.callback_underruns, r.underruns);
        // the first lap is linear play, the second the lap; every later one comes from the store
        let lap = (to.to_audio_frames() - from.to_audio_frames()) / kt_time::AUDIO_BURST_FRAMES as i64 + 1;
        assert!(live as i64 <= 2 * lap + 2, "only two laps render live: {live} live, {kept} kept");
        assert!(kept > 2 * live, "the loop plays from the kept mix: {live} live, {kept} kept");
        assert!(build > std::time::Duration::from_millis(400), "scene guard: a rebuild must outlast the ring, took {build:?}");
        assert_eq!(r.callback_underruns, 0, "the sound stopped (a rebuild took {build:?})");
        assert_eq!(r.underruns, 0);
        // the bars learn what the store holds
        let region = from.0..to.0;
        let deadline = kt_time::test_deadline();
        while !player.final_audio_runs().iter().any(|r| r.start <= region.start && r.end >= region.end) {
            assert!(std::time::Instant::now() < deadline, "the kept loop never reached the bars");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });
}

