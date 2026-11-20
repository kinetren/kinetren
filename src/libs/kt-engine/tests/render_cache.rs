//! render-cache-disk (kkt-26.11.17 S5, review R3): slow moments prerendered to disk at rest and played
//! from it. t2981, t2982, t2984, t2985 (t2980 and t2983 are unit tests of `render_cache.rs`). The
//! player tests skip without a wgpu adapter.

use kt_engine::render_cache::{Config, FileIds, disk_key};
use kt_engine::{Player, PresentOptions, VideoRenderer};
use kt_media::{MediaSource, Quality};
use kt_model::demo::{DemoSources, demo_project};
use kt_model::*;
use kt_time::{Tick, TickRange};
use std::time::{Duration, Instant};

const Q: Quality = Quality { width: 96, height: 54 };

fn base() -> Project {
    Project::new(ProjectSettings {
        fps: 30,
        width: 96,
        height: 54,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    })
}

fn clip(id: u64, lane: u32, media: u64) -> ClipEvent {
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
        kind: ClipKind::Video,
        lane,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn title(text: &str) -> GeneratorKind {
    let mut k = GeneratorKind::default_text();
    if let GeneratorKind::Text { text: t, size, .. } = &mut k {
        *t = text.into();
        *size = 0.4;
    }
    k
}

/// A title over a file event (the file is `file`, never opened: its frame stays undecoded).
fn scene(file: &std::path::Path, text: &str, gain: f64) -> Project {
    let mut p = base();
    p.media.push(MediaRef::new(MediaId(1), gen_kind_path(&title(""))));
    p.media.push(MediaRef::new(MediaId(2), file.to_string_lossy().into_owned()));
    let mut a = clip(1, 1, 1);
    a.r#gen = Some(title(text));
    let mut b = clip(2, 0, 2);
    b.fx = vec![FxInstance::geometry_at(0.01, 0.0, gain)];
    p.clips = vec![a, b];
    p.normalize();
    p.validate().expect("a title over a file is legal");
    p
}

/// The renderer the player would build: the generators observed as procedural sources.
fn renderer(p: &Project) -> VideoRenderer {
    let mut facts = kt_engine::video::SourceFacts::default();
    for (addr, st) in kt_engine::gen_sources(p) {
        let src = kt_media::factory::open_gen(&st);
        facts.observe(addr, src.info());
    }
    VideoRenderer::new_with_facts(p, &facts)
}

fn key(p: &Project, cache: &kt_cache::BurstCache, t: Tick, q: Quality) -> u64 {
    let video = renderer(p);
    let files = FileIds::read(&p.media, None);
    let (plan, leaves) = video.probe_durable(cache, t, q);
    disk_key(&plan, &leaves, &files)
}

/// Test t2981 (render-cache-disk D1): the durable key is the same from two renderers over two frames
/// caches with their own serials, decoded or not; an effect parameter, the file's modification time, a
/// generator's value and the quality each give another key; another build is another folder.
#[test]
fn t2981_the_durable_key_survives_a_process_and_follows_the_picture() {
    kt_plugins_builtin::install();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("clip.mp4");
    std::fs::write(&file, b"not a video: the key reads its identity, never its frames").unwrap();
    let p = scene(&file, "AAAA", 1.1);
    let t = Tick(30_000);
    let fresh = || kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let (c1, c2) = (fresh(), fresh());
    // the second cache's serials move: frames of another source inserted first
    let video = renderer(&p);
    for i in 0..5 {
        let f = kt_media::VideoFrame { pts: Tick(i), width: 4, height: 4, rgba: vec![0; 64].into_boxed_slice(), has_alpha: false };
        c2.insert_video(kt_engine::source_id(MediaId(77)), Some(Q), i, f.into());
    }
    let k = key(&p, &c1, t, Q);
    assert_eq!(key(&p, &c2, t, Q), k, "another renderer over another cache: the same key");
    // decode the title's frame into the second cache: decoded or not, the key stays
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        kt_engine::gen_sources(&p).into_iter().map(|(a, st)| (a, kt_media::factory::open_gen(&st))).collect();
    let mut decoded = 0;
    for (addr, pts, dq) in video.layers_at(t, Q) {
        if let Some(s) = srcs.get_mut(&addr) {
            let f = s.read_video_frame(pts, Some(dq)).unwrap();
            c2.insert_video(kt_engine::source_id(addr), Some(dq), kt_engine::video::video_cache_idx(pts), f.into());
            decoded += 1;
        }
    }
    assert!(decoded > 0, "scene guard: the title's frame was decoded");
    assert_eq!(key(&p, &c2, t, Q), k, "a decoded frame does not move the key");
    // what the picture is made of
    assert_ne!(key(&scene(&file, "AAAA", 1.2), &c1, t, Q), k, "an effect parameter");
    assert_ne!(key(&scene(&file, "AAAB", 1.1), &c1, t, Q), k, "a generator's value");
    assert_ne!(key(&p, &c1, t, Quality { width: 192, height: 108 }), k, "the quality");
    let f = std::fs::File::options().write(true).open(&file).unwrap();
    f.set_modified(std::time::SystemTime::now() - Duration::from_secs(3600)).unwrap();
    drop(f);
    assert_ne!(key(&p, &c1, t, Q), k, "the file's modification time");
    // another build is another folder
    let root = tempfile::tempdir().unwrap();
    let raw = || std::sync::Arc::new(kt_engine::render_cache::RawSegments) as std::sync::Arc<dyn kt_engine::render_cache::SegmentCodec>;
    let a = kt_engine::render_cache::DiskStore::open(root.path(), "build 1", "dev", 1 << 20, raw()).unwrap();
    let a_dir = a.dir().to_path_buf();
    let b = kt_engine::render_cache::DiskStore::open(root.path(), "build 2", "dev", 1 << 20, raw()).unwrap();
    assert_ne!(a_dir, b.dir(), "another build's folder");
    assert!(!a_dir.exists(), "the older build's folder is removed when the new one opens");
}

const MOMENTS: i64 = 6;

/// A player over the demo project with the tier at `root`, its loop of `MOMENTS` moments.
fn player(root: &std::path::Path, opacity: f32) -> Option<(Player, Vec<Tick>)> {
    kt_plugins_builtin::install();
    let gpu = kt_gpu::test_engine()?;
    let mut p = demo_project(DemoSources::Synth);
    for c in &mut p.clips {
        c.opacity = opacity; // a picture no other test presents on the shared engine
    }
    let sources: Vec<(MediaId, Box<dyn MediaSource>)> =
        p.media.iter().map(|m| (m.id, kt_media::factory::open_pseudo(&m.path).unwrap().unwrap())).collect();
    let (player, _ring) = Player::new(p, sources);
    player.set_preview_quality(Quality { width: 160, height: 90 });
    player.set_video_gpu(Some(gpu));
    player.render_cache().set_heavy_for_tests(Some(1));
    player.set_render_cache(Some(Config {
        codec: std::sync::Arc::new(kt_engine::render_cache::RawSegments),
        root: root.to_path_buf(),
        build: "test build".into(),
        base: None,
        cap: 1 << 30,
    }));
    let grid = player.video_grid();
    let step = grid.frame_step();
    let start = grid.align_floor(Tick(4 * 90_000));
    let moments: Vec<Tick> = (0..MOMENTS).map(|i| Tick(start.0 + i * step)).collect();
    Some((player, moments))
}

/// Measure the moments heavy by hand and wait until the prerender wrote them all.
fn prerender(player: &Player, moments: &[Tick]) {
    for t in moments {
        player.note_preview_cost(*t, 1_000_000_000, (160, 90));
    }
    player.render_cache().kick();
    let deadline = kt_time::test_deadline();
    while !moments.iter().all(|t| player.disk_picture(*t).is_some()) {
        assert!(Instant::now() < deadline, "the prerender never wrote the moments: {:?}", player.render_cache().counts());
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The moments the bars draw as playing without work.
fn bright(player: &Player, moments: &[Tick]) -> usize {
    let (runs, _) = player.final_video_runs();
    moments.iter().filter(|t| runs.iter().any(|r| r.start <= t.0 && t.0 < r.end)).count()
}

/// Test t2982 (render-cache-disk D3, D4): a moment measured heavy is written at rest; a new player over
/// the same store and project (a restart) shows it from disk on its first ask while playing, with its
/// frames not decoded; the picture on disk is the composite's bytes.
#[test]
fn t2982_a_prerendered_moment_plays_from_disk_after_a_restart() {
    let root = tempfile::tempdir().unwrap();
    let Some((a, moments)) = player(root.path(), 0.61) else { return };
    prerender(&a, &moments);
    // the disk's picture is the composite's bytes: the bytes path composes on demand with the tier off
    let kept = a.disk_picture(moments[2]).unwrap();
    a.set_render_cache(None);
    let mut buf = Vec::new();
    let deadline = kt_time::test_deadline();
    while !a.compose_preview_into(moments[2], &mut buf).1 {
        assert!(Instant::now() < deadline, "the moment never composed complete");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(kept.rgba, buf, "the disk holds the composite's bytes");
    drop(a);
    // the restart
    let Some((b, _)) = player(root.path(), 0.61) else { return };
    let items = kt_time::trace::testing::capture(|| {
        b.seek(moments[0]);
        b.play();
        let r = b.present_preview(moments[3], PresentOptions::default());
        assert!(matches!(r, Some((_, true))), "playing, the first ask is the moment from disk: {r:?}");
    });
    b.pause();
    let recs = kt_time::trace::testing::recs(&items);
    let restores = recs
        .iter()
        .filter(|r| r.ev == kt_time::trace::Ev::RenderCache as u16 && r.aux == kt_time::trace::render_cache::RESTORE)
        .count();
    assert!(restores >= 1, "the moment came from disk");
    assert!(b.render_cache().counts().1 >= 1);
}

/// Test t2984 (render-cache-disk D4): the prerender composes nothing while input keeps coming, writes
/// after the quiet, stops for Play, and runs nothing under an export's vote.
#[test]
fn t2984_the_prerender_yields_to_the_person() {
    let root = tempfile::tempdir().unwrap();
    let Some((p, moments)) = player(root.path(), 0.67) else { return };
    let rc = p.render_cache();
    for t in &moments {
        p.note_preview_cost(*t, 1_000_000_000, (160, 90));
    }
    // input every 100 ms, twenty times (the stimulus, not a wait): nothing works, nothing is written
    for _ in 0..20 {
        p.note_input();
        rc.kick();
        assert!(!rc.busy(), "the prerender worked while input came");
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(rc.counts().0, 0, "written while input came");
    // the quiet: it writes
    let deadline = kt_time::test_deadline();
    while rc.counts().0 == 0 {
        assert!(Instant::now() < deadline, "nothing was written after the quiet");
        std::thread::sleep(Duration::from_millis(10));
    }
    // Play: nothing more while playing
    p.play();
    while rc.busy() {
        assert!(Instant::now() < deadline, "the moment in work was not abandoned for Play");
        std::thread::sleep(Duration::from_millis(2));
    }
    let n = rc.counts().0;
    for _ in 0..20 {
        assert!(!rc.busy(), "the prerender worked while playing");
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(rc.counts().0 <= n + 1, "written while playing");
    // an export's vote: paused, nothing runs
    let vote = kt_time::sched::Vote::new(kt_time::sched::why::EXPORT);
    vote.set(true);
    p.pause();
    let n = rc.counts().0;
    for _ in 0..20 {
        assert!(!rc.busy(), "the prerender worked under an export");
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(rc.counts().0, n, "written under an export");
    drop(vote);
    rc.kick();
    while !moments.iter().all(|t| p.disk_picture(*t).is_some()) {
        assert!(Instant::now() < deadline, "the prerender did not finish after the export");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Test t2989 (found-defects 813): on a machine whose codec opens no encoder, the prerender says so and
/// composes nothing: no moment is drawn at rest for a tier that can never write it.
#[test]
fn t2989_no_encoder_composes_nothing() {
    struct NoEncoder;
    impl kt_engine::render_cache::SegmentCodec for NoEncoder {
        fn codec(&self) -> &str {
            "h264"
        }
        fn writer(&self, _: &std::path::Path, _: u32, _: u32, _: u32) -> Result<Box<dyn kt_engine::render_cache::SegmentWriter>, String> {
            Err("no hardware encoder opens".into())
        }
        fn reader(&self, _: &std::path::Path) -> Result<Box<dyn kt_engine::render_cache::SegmentReader>, String> {
            Err("none".into())
        }
    }
    let root = tempfile::tempdir().unwrap();
    let Some((p, moments)) = player(root.path(), 0.73) else { return };
    p.set_render_cache(Some(Config {
        codec: std::sync::Arc::new(NoEncoder),
        root: root.path().to_path_buf(),
        build: "test build".into(),
        base: None,
        cap: 1 << 30,
    }));
    let rc = p.render_cache();
    let items = kt_time::trace::testing::capture(|| {
        for t in &moments {
            p.note_preview_cost(*t, 1_000_000_000, (160, 90));
        }
        rc.kick();
        let deadline = kt_time::test_deadline();
        while !matches!(rc.status(), Some(Err(_))) {
            assert!(Instant::now() < deadline, "the tier never said it has no encoder");
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    let recs = kt_time::trace::testing::recs(&items);
    let composed = recs.iter().filter(|r| r.ev == kt_time::trace::Ev::Job as u16 && r.aux == kt_time::trace::job::PRERENDER).count();
    assert_eq!(composed, 0, "the prerender composed moments it could never write");
    assert_eq!(rc.counts().0, 0);
}

/// Test t2985 (render-cache-disk D3): a new player with the loop region over prerendered moments draws
/// them bright after its scan at rest; an edit of the event takes them off the bars.
#[test]
fn t2985_the_bars_draw_what_the_disk_holds_for_this_state() {
    let root = tempfile::tempdir().unwrap();
    let Some((a, moments)) = player(root.path(), 0.71) else { return };
    prerender(&a, &moments);
    drop(a);
    let Some((b, _)) = player(root.path(), 0.71) else { return };
    let step = b.video_grid().frame_step();
    b.set_loop_region(Some(TickRange::new(moments[0], Tick(moments[0].0 + MOMENTS * step))));
    b.seek(moments[0]);
    b.render_cache().kick();
    let deadline = kt_time::test_deadline();
    while bright(&b, &moments) < moments.len() {
        assert!(Instant::now() < deadline, "the bars never drew the disk's moments: {}", bright(&b, &moments));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(b.render_cache().counts().1, 0, "the scan restored nothing: no device work");
    // an edit of the events: the disk's moments are of another state
    let mut p = (*b.project()).clone();
    for c in &mut p.clips {
        c.opacity = 0.72;
    }
    b.set_project(p);
    let deadline = kt_time::test_deadline();
    while bright(&b, &moments) > 0 {
        assert!(Instant::now() < deadline, "the bars still drew the old state's moments");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Test t3231 (found-defects 776): with the playhead in the middle of a heavy run the prerender
/// writes the whole run into ONE segment, from its start: taken by distance to the head, both sides
/// alternating, the part behind the head went into a segment per moment (54 moments in 14–16 files).
#[test]
fn t3201_a_heavy_run_is_one_segment_from_its_start() {
    let root = tempfile::tempdir().unwrap();
    let Some((p, moments)) = player(root.path(), 0.63) else { return };
    // the run is measured whole before the tier looks (a played loop measures it so): with the tier
    // on, the seek's own compose made the middle moment the only one known for a pass
    p.set_render_cache(None);
    p.seek(moments[moments.len() / 2]);
    for t in &moments {
        p.note_preview_cost(*t, 1_000_000_000, (160, 90));
    }
    p.set_render_cache(Some(Config {
        codec: std::sync::Arc::new(kt_engine::render_cache::RawSegments),
        root: root.path().to_path_buf(),
        build: "test build".into(),
        base: None,
        cap: 1 << 30,
    }));
    let items = kt_time::trace::testing::capture(|| prerender(&p, &moments));
    let stores: Vec<(u64, u64)> = kt_time::trace::testing::recs(&items)
        .iter()
        .filter(|r| r.ev == kt_time::trace::Ev::RenderCache as u16 && r.aux == kt_time::trace::render_cache::STORE)
        .map(|r| (r.c, r.a))
        .collect();
    // the test tier counts every measured moment heavy, so the prerender walks on past the run; what
    // matters is that the run went into one segment, started before its first moment
    let step = p.video_grid().frame_step();
    let holding: Vec<&(u64, u64)> = stores
        .iter()
        .filter(|(first, n)| moments.iter().any(|t| (*first as i64) <= t.0 && t.0 < *first as i64 + *n as i64 * step))
        .collect();
    assert_eq!(
        holding.len(),
        1,
        "the run of {} moments {:?} is in one segment; written (first tick, moments): {stores:?}",
        moments.len(),
        moments.iter().map(|t| t.0).collect::<Vec<_>>()
    );
    let (first, n) = *holding[0];
    assert!(
        (first as i64) <= moments[0].0 && moments[moments.len() - 1].0 < first as i64 + n as i64 * step,
        "the segment holds the whole run from its start: {stores:?}"
    );
}
