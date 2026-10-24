//! Test 18: adding media does NOT reset the cache
//! (regression of item 27: rebuild_player re-created the Player whole — the warmed
//! cache, transport and ring were thrown away on every file drop).

use kt_engine::Player;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;

fn base_project() -> Project {
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

fn synth() -> Box<SyntheticSource> {
    Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 220.0 }))
}

#[test]
fn t18_add_media_preserves_cache_coverage_and_transport() {
    let project = base_project();
    let (player, _consumer) = Player::new(project.clone(), vec![(MediaId(1), synth())]);

    // warm-up: the prefetch thread fills the cache around the playhead
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let cov = player.cache.coverage(
            kt_engine::source_id(MediaId(1)),
            kt_cache::BurstKind::Audio,
            0..1_000_000,
        );
        let covered: i64 = cov.iter().map(|r| r.end - r.start).sum();
        if covered >= 40 {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "the cache did not warm up in 10 s");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let pos = Tick(90_000); // 1 c
    player.seek(pos);
    let bytes_before = player.cache.audio_bytes();
    let cov_before = player.cache.coverage(
        kt_engine::source_id(MediaId(1)),
        kt_cache::BurstKind::Audio,
        0..1_000_000,
    );
    let gen_before = player.transport.generation();
    assert!(bytes_before > 0);

    // adding a second media — incremental
    player.add_media(MediaId(2), synth());
    let mut p2 = project.clone();
    p2.media.push(MediaRef::new(MediaId(2), "synth:2"));
    p2.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(2),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 1,
        start: Tick(0),
        duration: Tick(90_000 * 4),
        source_in: Tick(0),
        media: MediaId(2),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p2.normalize(); // tracks for the content (R.62)
    p2.validate().unwrap();
    player.set_project(p2);

    // the cache and transport survived the addition
    assert_eq!(player.transport.playhead(), pos, "the playhead must not be reset");
    assert_eq!(player.transport.generation(), gen_before, "the generation must not be bumped");
    let cov_after = player.cache.coverage(
        kt_engine::source_id(MediaId(1)),
        kt_cache::BurstKind::Audio,
        0..1_000_000,
    );
    // prefetch is alive and could still WARM more — we check that coverage did not shrink
    let covered = |cov: &[std::ops::Range<i64>], x: i64| cov.iter().any(|r| r.contains(&x));
    for r in &cov_before {
        for x in [r.start, (r.start + r.end) / 2, r.end - 1] {
            assert!(
                covered(&cov_after, x),
                "burst {x} was in the cache before add_media and vanished: before {cov_before:?}, after {cov_after:?}"
            );
        }
    }
    assert!(player.cache.audio_bytes() >= bytes_before, "the cache must not shrink");

    // the new worker is alive: prefetch fills media 2
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let cov = player.cache.coverage(
            kt_engine::source_id(MediaId(2)),
            kt_cache::BurstKind::Audio,
            0..1_000_000,
        );
        if !cov.is_empty() {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "new media does not decode — the worker is dead");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Test 20 (item 1): while paused, background threads do NOT burn resources.
/// Before: audio-render spun 500 times per second, prefetch cloned the whole
/// Project and rebuilt the renderers every 100 ms — the system lagged even
/// with an empty project.
#[test]
fn t20_paused_player_is_quiescent() {
    let project = base_project();
    let (player, _consumer) = Player::new(project.clone(), vec![(MediaId(1), synth())]);
    std::thread::sleep(std::time::Duration::from_millis(400)); // warm-up / stabilization

    use std::sync::atomic::Ordering::Relaxed;
    let r0 = player.loop_stats.render_passes.load(Relaxed);
    let rb0 = player.loop_stats.prefetch_rebuilds.load(Relaxed);
    std::thread::sleep(std::time::Duration::from_secs(1));
    let render_delta = player.loop_stats.render_passes.load(Relaxed) - r0;
    let rebuild_delta = player.loop_stats.prefetch_rebuilds.load(Relaxed) - rb0;

    assert!(
        render_delta <= 8,
        "when paused the render thread must park (safety timeout 250 ms), \
         and not spin: {render_delta} passes in 1 s (previously ~500)"
    );
    assert_eq!(rebuild_delta, 0, "prefetch must not rebuild renderers without model edits");

    // a model edit wakes and rebuilds exactly once
    player.set_project(project);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let rebuilds = player.loop_stats.prefetch_rebuilds.load(Relaxed) - rb0;
    assert!(rebuilds >= 1, "after set_project prefetch must rebuild the graphs");
}

/// Pause → resume: the render wakes up and the ring fills again
/// (guard against a deadlock on the condvar; t14 covers it).
#[test]
fn t20_pause_resume_still_streams() {
    let project = base_project();
    let (player, mut consumer) = Player::new(project, vec![(MediaId(1), synth())]);
    // wait for the cache warm-up, so that the render is not silent due to misses
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while player
        .cache
        .coverage(kt_engine::source_id(MediaId(1)), kt_cache::BurstKind::Audio, 0..40)
        .is_empty()
    {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let drain = |c: &mut rtrb::Consumer<f32>| {
        let mut n = 0usize;
        while c.pop().is_ok() {
            n += 1;
        }
        n
    };

    player.play();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(drain(&mut consumer) > 0, "after play the ring must fill");

    player.pause();
    std::thread::sleep(std::time::Duration::from_millis(200));
    drain(&mut consumer);

    player.play();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(drain(&mut consumer) > 0, "after resume the render must wake up (condvar)");
}

/// Test 211 (R.33 item 10): a source returned to disk swaps its
/// placeholder ON THE FLY. Frames of the old source must leave the cache —
/// otherwise after the file came back a badge would hang in the preview for a long time.
#[test]
fn t211_replace_media_swaps_source_and_drops_its_cache() {
    let project = base_project();
    let dc = |v: f32| -> Box<SyntheticSource> {
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Dc { value: v }))
    };
    let (player, _consumer) = Player::new(project, vec![(MediaId(1), dc(1.0))]);

    // warming the cache of the first source
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let cov = player.cache.coverage(
            kt_engine::source_id(MediaId(1)),
            kt_cache::BurstKind::Audio,
            0..1_000_000,
        );
        let covered: i64 = cov.iter().map(|r| r.end - r.start).sum();
        if covered >= 40 {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "the cache did not warm up in 10 s");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let old = player
        .cache
        .get_audio(kt_engine::source_id(MediaId(1)), 0)
        .expect("the burst of the source must lie in the cache");
    assert!((old.data[0] - 1.0).abs() < 1e-6, "the wrong signal was warmed");

    // source swap: the cache of THIS media is discarded
    player.replace_media(MediaId(1), dc(0.25));
    assert!(
        player.cache.get_audio(kt_engine::source_id(MediaId(1)), 0).is_none(),
        "frames of the previous source remained in the cache — the preview would show a placeholder"
    );

    // the new source fills the cache with its own values
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(b) = player.cache.get_audio(kt_engine::source_id(MediaId(1)), 0) {
            assert!((b.data[0] - 0.25).abs() < 1e-6, "the cache holds values of the old source");
            break;
        }
        assert!(std::time::Instant::now() < deadline, "the new source did not fill the cache");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Test 316 (R.46, item 321): a returned file brings ITS OWN native, and the decode
/// step is recomputed on the fly. While the placeholder is in place (native smaller
/// than Quality) a clip with zoom decodes as usual; once it was swapped for a source
/// with headroom, the clip's decode quality went up, and the strips/prefetch
/// must now ask for it.
#[test]
fn t316_replace_media_updates_the_decode_stage() {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 128,
        height: 72,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "checker"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![FxInstance::geometry_zoom(2.0)],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    let small: Box<dyn kt_media::MediaSource> =
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence));
    let (player, _consumer) = Player::new(p, vec![(MediaId(1), small)]);
    player.set_preview_quality(kt_media::Quality { width: 128, height: 72 });
    assert_eq!(
        player.video_decode_qualities(),
        vec![(ClipId(1), kt_media::Quality { width: 128, height: 72 })],
        "a 64x36 source has no headroom above Quality — there cannot be a step"
    );

    player.replace_media(
        MediaId(1),
        Box::new(kt_media::synthetic::CheckerSource::new(Tick(90_000 * 60), 512, 288)),
    );
    // the renderer rebuild follows the project version (bumped by replace_media)
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if player.video_decode_qualities()
            == vec![(ClipId(1), kt_media::Quality { width: 256, height: 144 })]
        {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "the step was not recomputed after the substitution");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Test 500 (R.53, item 411): reconfiguring a source does NOT re-create it.
/// The decoder thread is alive (the spawn counter does not grow), the renderers are not
/// rebuilt (the project version does not grow), while the picture is new
/// — frames of the old parameters are discarded. Before this item every keypress
/// in a title field killed and raised the thread and rebuilt the renderers twice
/// (the owner's claim, R.53).
#[test]
fn t500_retune_keeps_worker_and_renderer() {
    let mut project = base_project();
    // a generator clip over an audio clip of the base project
    project.media.push(MediaRef::new(MediaId(2), "gen:solid"));
    project.clips.push(ClipEvent {
        // R.59, stage 2: values are on the EVENT, the source sits at its address
        r#gen: Some(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(2),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000 * 4),
        source_in: Tick(0),
        media: MediaId(2),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    project.validate().unwrap();

    // the generator source is raised by the player ITSELF (`sync_generators`): its
    // address is derived from the event, and the caller has nowhere to take it from
    let (player, _consumer) = Player::new(
        project,
        vec![(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Dc { value: 1.0 })),
        )],
    );
    let q = kt_media::Quality { width: 64, height: 36 };
    player.set_preview_quality(q);
    let sid = kt_engine::source_id(kt_engine::gen_source_addr(ClipId(2)));
    let idx = kt_engine::video::video_cache_idx(Tick(0));

    // we warm up a frame of the black generator
    player.cache.insert_video(
        sid,
        Some(q),
        idx,
        kt_media::VideoFrame {
            pts: Tick(0),
            width: q.width,
            height: q.height,
            rgba: vec![0u8; (q.width * q.height * 4) as usize].into_boxed_slice(),
            has_alpha: false,
        }
        .into(),
    );
    // serial of the warmed record: it shows that the frame of the old
    // parameters was really DISCARDED, not surviving the reconfiguration
    let (_, warm_serial) =
        player.cache.get_video_stamped(sid, Some(q), idx).expect("warm frame");
    let spawns = player.worker_spawns();
    let pver = player.project_version();
    let cver = player.content_version();

    let orange = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    assert!(
        kt_media::factory::can_retune_gen(
            &kt_model::GenState::new(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
            &kt_model::GenState::new(orange.clone())
        ),
        "the colour changes without a change of passport"
    );
    assert!(player.update_gen(ClipId(2), orange, Vec::new()));

    assert_eq!(player.worker_spawns(), spawns, "the decoder stream must stay alive");
    assert_eq!(player.project_version(), pver, "there is no need to rebuild the renderers: the plan is the same");
    assert!(player.content_version() > cver, "the source content changed — prefetch wakes up");

    // the frame of the old parameters is discarded, the new one arrives with the NEW color.
    // The deadline is generous: the claim is “the reconfiguration arrives”, not “it is
    // fast”, and under a parallel run of the whole workspace the worker thread waits its turn
    // for the CPU (the same lesson as t399/t476).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let got = loop {
        // “Seen empty” cannot be caught: the worker manages both to discard and
        // to fill between two polls, and under load the test turned red on correct
        // code. The assertion is the same, but deterministic —
        // the RECORD IS DIFFERENT (the serial is monotonic and is not reused, item 409),
        // so the frame of the old parameters was discarded, not surviving the edit.
        if let Some((f, serial)) = player.cache.get_video_stamped(sid, Some(q), idx)
            && serial != warm_serial
        {
            break f;
        }
        assert!(std::time::Instant::now() < deadline, "the frame of the new parameters did not arrive");
        // the preview composite itself requests the missing frames from the worker —
        // this is the product path of requesting
        let mut buf = Vec::new();
        let _ = player.compose_preview_into(Tick(0), &mut buf);
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(&got.rgba[..3], &[255, 136, 0], "the new entry carries the OLD colour");
    // and throughout all this the thread was never re-created
    assert_eq!(player.worker_spawns(), spawns);
    assert_eq!(player.project_version(), pver);
}

/// Test 661 (R.59, stage 2): editing the VALUE of a generator does not rebuild
/// the renderers and does not grow the project version.
///
/// This is the whole point of the stage: the source address is computed from the EVENT, not
/// from the values, so the plan does not depend on them — while `set_project` is a clone
/// of the whole project plus a stop of the audio-render thread. Until R.53 every keypress
/// in a title field paid this price; here the same rule is pinned to the new carrier
/// of the values.
#[test]
fn t661_a_value_edit_does_not_rebuild_the_renderer() {
    use kt_model::{Command, GeneratorKind, ParamCurve};

    // the law P at the level of COMMANDS: the exhaustive match in kt-model will not let
    // a new command be forgotten, while its answer is pinned here
    let set = Command::SetClipGen {
        id: ClipId(2),
        kind: Some(GeneratorKind::Solid { color_r: 1, color_g: 1, color_b: 1 }),
    };
    let auto = Command::SetClipGenAuto { id: ClipId(2), auto: Vec::<ParamCurve>::new() };
    assert!(!set.touches_render_plan(), "the generator’s values of the plan do not touch it");
    assert!(!auto.touches_render_plan(), "the generator’s curves are resolved by the SOURCE, not by the plan");
    // but “apply to all” edits the DOCUMENT — the set of sources could have
    // changed, so it cannot be called like a value edit
    assert!(
        Command::ApplyGenToAll {
            media: MediaId(2),
            kind: GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 },
            auto: vec![],
        }
        .touches_render_plan()
    );

    // and the same goes for the player: the thread is alive, the project version holds
    let mut project = base_project();
    project.media.push(MediaRef::new(MediaId(2), "gen:solid"));
    project.clips.push(ClipEvent {
        r#gen: Some(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(2),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000 * 4),
        source_in: Tick(0),
        media: MediaId(2),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    project.validate().unwrap();
    let (player, _consumer) = Player::new(
        project,
        vec![(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Dc { value: 1.0 })),
        )],
    );
    let spawns = player.worker_spawns();
    let pver = player.project_version();
    let cver = player.content_version();
    assert!(player.update_gen(
        ClipId(2),
        GeneratorKind::Solid { color_r: 9, color_g: 9, color_b: 9 },
        kt_model::text_anim_curves(kt_model::TextAnim::Typewriter, 1.0, 2.0),
    ));
    assert_eq!(player.worker_spawns(), spawns, "there is no need to recreate the decoder stream");
    assert_eq!(player.project_version(), pver, "there is no need to rebuild the renderers: the plan is the same");
    assert!(player.content_version() > cver, "prefetch must wake up");
    // the stored project stays truthful: it is read by export and diagnostics
    let p = player.project();
    let c = p.clips.iter().find(|c| c.id == ClipId(2)).unwrap();
    assert!(matches!(c.r#gen, Some(GeneratorKind::Solid { color_r: 9, .. })));
    assert!(!c.gen_auto.is_empty(), "the curves must reach the stored project");
}
