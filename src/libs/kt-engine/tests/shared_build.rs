//! Tests t2000–t2006 (edit-without-rebuild, capabilities `audio-render`, `transport`, `video-plan`,
//! `compositions`): one build per project version, shared by every consumer and built off the UI
//! thread; a region change builds nothing; an edit derives what it touched; an incremental build is
//! a fresh one, byte for byte; a build resolves each reel once; a frame is composed by its state's
//! renderer.
//!
//! The player tests count process-wide (`incremental::RENDERER_BUILDS`, `kt_time::work`): they hold
//! `SERIAL`, and no other test in this binary builds a player.

use kt_engine::incremental::{BuildMemo, RENDERER_BUILDS, with_memo};
use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore, Player, VideoRenderer};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::cost::{self, Cost};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick, TickRange};
use std::sync::Mutex;
use std::sync::atomic::Ordering;

static SERIAL: Mutex<()> = Mutex::new(());

const SEC: i64 = 90_000;

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: 25,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

fn ev(id: u64, kind: ClipKind, lane: u32, start: i64, dur: i64, media: u64) -> ClipEvent {
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
        kind,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: Vec::new(),
        opacity: 1.0,
        fx_off: false,
    }
}

fn slowed() -> FxInstance {
    kt_plugins_builtin::install();
    FxInstance::plugin_with("slowed_reverb", &[0.75, 0.2, 85.0, 0.2, 0.84, 1.0, 0.0, 0.0, 1.0]).unwrap()
}

fn reverb() -> FxInstance {
    kt_plugins_builtin::install();
    FxInstance::plugin_with("slowed_reverb", &[1.0, 0.3, 60.0, 0.4, 0.7, 1.0, 0.0, 0.0, 1.0]).unwrap()
}

/// A shaped speed curve swinging through zero (sampled, the address map built).
fn shaped_rate(dur: i64) -> ParamCurve {
    let mut keys = Vec::new();
    let (mut t, mut i) = (0, 0);
    while t <= dur {
        keys.push(AutoKey::shaped(Tick(t), if i % 2 == 0 { 1.5 } else { -0.8 }, SegShape::Smooth { tension: 1.0 }));
        t += dur / 2;
        i += 1;
    }
    ParamCurve { param: "rate".into(), keys, source: Default::default() }
}

fn crossfade(p: &mut Project, id: u64, a: u64, b: u64) {
    p.transitions.push(TransitionEvent {
        id: TransitionId(id),
        a: Some(ClipId(a)),
        b: Some(ClipId(b)),
        in_offset: Tick(SEC / 10),
        out_offset: Tick(SEC / 10),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
}

/// `n` A/V pairs butted on two lanes (every third with a reverb, every seventh slowed with a shaped
/// speed curve), crossfades at the joints of lane 0, a reel of four pairs placed twice, and a
/// morpher layer over a stretch of the edit.
fn scene(n: u64) -> Project {
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.media.push(MediaRef::new(MediaId(2), "synth:sine"));
    p.ensure_tracks(LaneKind::Video, 2);
    p.ensure_tracks(LaneKind::Audio, 2);
    let mut reel = Sequence::new(MediaId(100), "Reel");
    reel.ensure_tracks(LaneKind::Video, 1);
    reel.ensure_tracks(LaneKind::Audio, 1);
    for i in 0..4u64 {
        reel.clips.push(ev(100_000 + 2 * i, ClipKind::Video, 0, i as i64 * SEC, SEC, 1));
        let mut a = ev(100_001 + 2 * i, ClipKind::Audio, 0, i as i64 * SEC, SEC, 2);
        a.fx = vec![reverb()];
        reel.clips.push(a);
    }
    p.sequences.push(reel);
    for i in 0..n {
        let lane = (i % 2) as u32;
        let start = (i / 2) as i64 * SEC;
        let (v, a) = (10 + 2 * i, 11 + 2 * i);
        let mut cv = ev(v, ClipKind::Video, lane, start, SEC, 1);
        let mut ca = ev(a, ClipKind::Audio, lane, start, SEC, 1 + i % 2);
        cv.link_group = Some(GroupId(v));
        ca.link_group = Some(GroupId(v));
        if i % 3 == 0 {
            ca.fx = vec![reverb()];
        }
        if i % 7 == 0 {
            for c in [&mut cv, &mut ca] {
                c.fx = vec![slowed()];
                c.fx_auto = vec![vec![shaped_rate(SEC)]];
            }
        }
        p.clips.push(cv);
        p.clips.push(ca);
        if lane == 0 && i >= 2 {
            crossfade(&mut p, 2 * i, v - 4, v);
            crossfade(&mut p, 2 * i + 1, a - 4, a);
        }
    }
    let end = (n / 2 + 1) as i64 * SEC;
    p.clips.push(ev(9_000_001, ClipKind::Video, 0, end, 4 * SEC, 100));
    p.clips.push(ev(9_000_002, ClipKind::Audio, 0, end, 4 * SEC, 100));
    p.clips.push(ev(9_000_003, ClipKind::Video, 1, end + 4 * SEC, 4 * SEC, 100));
    p.clips.push(ev(9_000_004, ClipKind::Audio, 1, end + 4 * SEC, 4 * SEC, 100));
    p.ensure_tracks(LaneKind::Morph, 1);
    p.morphers.push(MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(SEC / 3),
        duration: Tick(3 * SEC),
        targets: MorphTargets::Clips { ids: vec![ClipId(11), ClipId(15)] },
        patch: MorphPatch { audio_fx: vec![reverb()], audio_auto: vec![vec![]], ..Default::default() },
        timed: true,
        strength: 1.0,
        fade_in: Tick(0),
        fade_out: Tick(0),
        enabled: true,
    });
    p.validate().expect("the scene is a valid project");
    p
}

fn fetcher() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(SEC * 600), AudioPattern::Ramp)));
    f.add_source(MediaId(2), Box::new(SyntheticSource::new(Tick(SEC * 600), AudioPattern::Sine { hz: 220.0 })));
    f
}

/// The audio of `[from, from + n bursts)` through fresh FX states.
fn sound(r: &AudioRenderer, from: i64, bursts: usize) -> Vec<f32> {
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; bursts * AUDIO_BURST_FRAMES * AUDIO_CHANNELS];
    r.render_frames_with(&mut fetcher(), &mut store, from, &mut out);
    out
}

fn derives(f: impl FnOnce()) -> u64 {
    cost::meter(f).1[Cost::EventDerive as usize]
}

/// Test 2004: after each of a sequence of edits — a gain, a curve key, a trim, an edit inside the
/// reel, a transition removed, a track muted, an undo back to the start — the build that reuses the
/// last build's events equals a fresh build: every derived event and layer, and the sound.
#[test]
fn t2004_an_incremental_build_is_a_fresh_one() {
    let base = scene(60);
    type Edit = Box<dyn Fn(&mut Project)>;
    let mut edits: Vec<Edit> = vec![
        Box::new(|p| p.clips.iter_mut().find(|c| c.id == ClipId(13)).unwrap().gain_db = -4.5),
        Box::new(|p| {
            let c = p.clips.iter_mut().find(|c| c.id == ClipId(11)).unwrap();
            c.fx_auto[0][0].keys[1].v = 0.6;
        }),
        Box::new(|p| p.clips.iter_mut().find(|c| c.id == ClipId(41)).unwrap().duration = Tick(SEC / 2)),
        Box::new(|p| p.sequences[0].clips[1].gain_db = -9.0),
        Box::new(|p| {
            p.transitions.pop();
        }),
        Box::new(|p| {
            let t = p.tracks.iter_mut().find(|t| t.kind == LaneKind::Audio).unwrap();
            t.mute = true;
        }),
        Box::new(|p| p.clips.iter_mut().find(|c| c.id == ClipId(10)).unwrap().opacity = 0.5),
    ];
    edits.push(Box::new(|_| {}));
    let facts = kt_engine::SourceFacts::default();
    let mut memo = BuildMemo::default();
    let mut p = base.clone();
    with_memo(&mut memo, || (AudioRenderer::new(&p), VideoRenderer::new_with_facts(&p, &facts)));
    for (i, edit) in edits.iter().enumerate() {
        if i + 1 == edits.len() {
            p = base.clone(); // the undo
        } else {
            edit(&mut p);
        }
        p.validate().unwrap();
        let (ia, iv) = with_memo(&mut memo, || (AudioRenderer::new(&p), VideoRenderer::new_with_facts(&p, &facts)));
        let (fa, fv) = (AudioRenderer::new(&p), VideoRenderer::new_with_facts(&p, &facts));
        assert!(ia.state_text() == fa.state_text(), "edit {i}: the audio build differs from a fresh one");
        assert!(iv.state_text() == fv.state_text(), "edit {i}: the video build differs from a fresh one");
        for from in [0, 48_000 * 3, 48_000 * 31] {
            assert!(sound(&ia, from, 40) == sound(&fa, from, 40), "edit {i}: the sound at {from} differs");
            assert_eq!(ia.scene_hash(from..from + 48_000), fa.scene_hash(from..from + 48_000));
        }
    }
}

/// Test 2003: N edits of one event of a large project derive that event only, in the audio and in
/// the video build; the first build derives every event; an unchanged rebuild derives none. And the
/// same through the player: each version's build (on its own thread) reports one derived event.
#[test]
fn t2003_an_edit_derives_what_it_touched() {
    let mut p = scene(1000);
    let facts = kt_engine::SourceFacts::default();
    let mut memo = BuildMemo::default();
    let first = derives(|| {
        with_memo(&mut memo, || drop((AudioRenderer::new(&p), VideoRenderer::new_with_facts(&p, &facts))));
    });
    assert!(first >= 2000, "the first build derives every event: {first}");
    let again = derives(|| {
        with_memo(&mut memo, || drop((AudioRenderer::new(&p), VideoRenderer::new_with_facts(&p, &facts))));
    });
    assert_eq!(again, 0, "an unchanged project derives nothing");
    let target = p.clips.iter().position(|c| c.id == ClipId(1013)).unwrap();
    for step in 0..5 {
        p.clips[target].gain_db = -0.5 * (step + 1) as f32;
        let n = derives(|| {
            with_memo(&mut memo, || drop((AudioRenderer::new(&p), VideoRenderer::new_with_facts(&p, &facts))));
        });
        assert_eq!(n, 1, "a slider step on one event derives that event: {n}");
    }
    let video = p.clips.iter().position(|c| c.id == ClipId(1012)).unwrap();
    p.clips[video].opacity = 0.25;
    let n = derives(|| {
        with_memo(&mut memo, || drop((AudioRenderer::new(&p), VideoRenderer::new_with_facts(&p, &facts))));
    });
    assert_eq!(n, 1, "an opacity step derives that layer: {n}");

    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (player, _ring) = Player::new(p.clone(), Vec::new());
    for step in 0..5 {
        p.clips[target].gain_db = -3.0 - step as f32;
        player.set_project(p.clone());
        let built = player.wait_built();
        assert_eq!(built.version, player.project_version());
        assert_eq!(built.cost[Cost::EventDerive as usize], 1, "the player's build of a slider step");
    }
}

/// The engine threads are done: the renderer count stands for a while.
fn settle() {
    let deadline = kt_time::test_deadline();
    let (mut last, mut still) = (RENDERER_BUILDS.load(Ordering::Relaxed), 0);
    while still < 5 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(40));
        let now = RENDERER_BUILDS.load(Ordering::Relaxed);
        still = if now == last { still + 1 } else { 0 };
        last = now;
    }
}

fn player_with(p: Project) -> (Player, rtrb::Consumer<f32>) {
    let src = |pat| -> Box<dyn kt_media::MediaSource> { Box::new(SyntheticSource::new(Tick(SEC * 600), pat)) };
    Player::new(p, vec![(MediaId(1), src(AudioPattern::Ramp)), (MediaId(2), src(AudioPattern::Sine { hz: 220.0 }))])
}

/// Test 2000: one audio and one video renderer per project version, process-wide — the render
/// thread, prefetch, the probe (meters, the loop's head) and the preview all take the same build.
#[test]
fn t2000_one_build_per_version_shared() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut p = scene(200);
    let (player, _ring) = player_with(p.clone());
    player.play();
    settle();
    let b0 = RENDERER_BUILDS.load(Ordering::Relaxed);
    let took0 = player.loop_stats.prefetch_rebuilds.load(Ordering::Relaxed);
    for step in 0..5 {
        p.clips[3].gain_db = -1.0 - step as f32;
        player.set_project(p.clone());
        let built = player.wait_built();
        let _ = player.sample_levels_at(Tick(SEC));
        player.set_loop_region(Some(TickRange::new(Tick(0), Tick(2 * SEC))));
        let deadline = kt_time::test_deadline();
        while player.probe_version() != Some(built.version) {
            assert!(std::time::Instant::now() < deadline, "the probe never took the build");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let deadline = kt_time::test_deadline();
        while player.loop_stats.prefetch_rebuilds.load(Ordering::Relaxed) < took0 + step + 1 {
            assert!(std::time::Instant::now() < deadline, "prefetch never took the build");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    settle();
    assert_eq!(RENDERER_BUILDS.load(Ordering::Relaxed) - b0, 10, "five versions: five audio and five video renderers");
}

/// Test 2001: the UI thread's part of an edit builds nothing and waits for nothing — handing a project
/// over, the meters, a region change, the grid and the coverage questions, a preview ask — even
/// while the builder is held.
#[test]
fn t2001_no_build_on_the_ui_thread() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut p = scene(200);
    let (player, _ring) = player_with(p.clone());
    settle();
    player.gate_builds(true);
    let (_, ui) = cost::meter(|| {
        for step in 0..10 {
            p.clips[5].gain_db = -0.5 * step as f32;
            player.set_project(std::sync::Arc::new(p.clone()));
            let _ = player.sample_levels_at(Tick(step * 1000));
            player.set_loop_region(Some(TickRange::new(Tick(step * 9_000), Tick(SEC + step * 9_000))));
            let _ = player.video_grid();
            let _ = player.video_decode_qualities();
            let _ = player.nest_coverage();
            let _ = player.ask_preview(Tick(step * 3600), None);
        }
    });
    assert_eq!(ui[Cost::RendererBuild as usize], 0, "a renderer was built on the UI thread");
    assert_eq!(ui[Cost::EventDerive as usize], 0, "an event was derived on the UI thread");
    assert_eq!(ui[Cost::ReelPass as usize], 0, "a reel was resolved on the UI thread");
    assert_ne!(player.built().version, player.project_version(), "the held build landed");
    player.gate_builds(false);
    assert_eq!(player.wait_built().version, player.project_version());
}

/// Test 2002: a region change is not an edit — twenty of them build nothing and move no version.
#[test]
fn t2002_a_region_change_builds_nothing() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (player, _ring) = player_with(scene(200));
    settle();
    let (b0, v0) = (RENDERER_BUILDS.load(Ordering::Relaxed), player.project_version());
    for i in 0..20 {
        player.set_loop_region(Some(TickRange::new(Tick(i * 9_000), Tick(2 * SEC + i * 9_000))));
    }
    player.set_loop_region(None);
    settle();
    assert_eq!(RENDERER_BUILDS.load(Ordering::Relaxed), b0);
    assert_eq!(player.project_version(), v0);
}

/// Test 2005: a build resolves each reel instance once (found-defects row 155: the audio build, the
/// video build, the decode step and the generator sources each resolved every reel, ~5 times).
#[test]
fn t2005_a_build_resolves_each_reel_once() {
    let p = scene(100);
    let facts = kt_engine::SourceFacts::default();
    let passes = || {
        drop(kt_engine::gen_sources(&p));
        drop(AudioRenderer::new(&p));
        drop(VideoRenderer::new_with_facts(&p, &facts));
    };
    // apart, each pass resolves the root and the reel on its own (the reel per placement where it
    // builds the reel's sub-renderer)
    let apart = cost::meter(passes).1[Cost::ReelResolve as usize];
    assert!(apart >= 6, "scene guard: three passes resolve the two reels each: {apart}");
    let mut memo = BuildMemo::default();
    let (_, walked) = cost::meter(|| with_memo(&mut memo, || kt_engine::video::revisions_scope(passes)));
    assert_eq!(walked[Cost::ReelResolve as usize], 2, "the root and the reel, once each per build");
}

/// Records the moment of every plan it is handed.
struct Recorder {
    info: kt_engine::GpuBackendInfo,
    seen: Mutex<Vec<Tick>>,
}

impl kt_engine::VideoGpuExec for Recorder {
    fn execute_into(&self, p: &kt_engine::FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, kt_engine::GpuExecError> {
        self.seen.lock().unwrap().push(p.t);
        out.clear();
        out.extend_from_slice(&[0, 0, 0, 255]);
        Ok(true)
    }

    fn info(&self) -> &kt_engine::GpuBackendInfo {
        &self.info
    }
}

/// Test 2006: a frame is composed by the renderer of the state it was asked for. The build of a
/// new frame rate is held; the ask waits for it on the compose thread (the UI does not), and the
/// plan it composes is on the new grid.
#[test]
fn t2006_a_frame_is_composed_by_its_states_renderer() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut p = scene(20);
    let (player, _ring) = player_with(p.clone());
    let rec = std::sync::Arc::new(Recorder {
        info: kt_engine::GpuBackendInfo { name: "rec".into(), backend: "Test".into(), software: true },
        seen: Mutex::new(Vec::new()),
    });
    player.set_video_gpu(Some(rec.clone()));
    settle();
    player.gate_builds(true);
    p.settings.fps = 30;
    player.set_project(p.clone());
    let t = std::time::Instant::now();
    player.ask_preview(Tick(5_000), None);
    assert!(t.elapsed() < std::time::Duration::from_secs(1), "the ask waited");
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(rec.seen.lock().unwrap().is_empty(), "a frame was composed by the old state's renderer");
    player.gate_builds(false);
    let deadline = kt_time::test_deadline();
    let got = loop {
        if let Some(c) = player.take_composed(std::time::Duration::from_millis(50)) {
            break c;
        }
        assert!(std::time::Instant::now() < deadline, "the frame never came");
    };
    assert_eq!(got.t, Tick(5_000));
    assert_eq!(rec.seen.lock().unwrap().as_slice(), &[Tick(3_000)], "composed on the 30 fps grid");
}
