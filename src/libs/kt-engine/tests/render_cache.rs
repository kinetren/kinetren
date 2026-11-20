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
    // input every 100 ms, twenty times (the stimulus, not a wait): nothing works, nothing is written.
    // The quiet is ten seconds while it comes (found-defects 828): this thread paces the stimulus with
    // its own sleeps, and a loaded machine let one of them pass the 1.5-s quiet — the prerender then
    // rightly worked "while input came"
    rc.set_quiet_for_tests(Some(Duration::from_secs(10)));
    for i in 0..20 {
        p.note_input();
        rc.kick();
        assert!(!rc.busy(), "the prerender worked while input came");
        // one sleep stretched past the real quiet, as a loaded machine stretched it
        std::thread::sleep(Duration::from_millis(if i == 10 { 2_000 } else { 100 }));
    }
    assert_eq!(rc.counts().0, 0, "written while input came");
    // the quiet: it writes (the real quiet again)
    rc.set_quiet_for_tests(None);
    rc.kick();
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
    let since = kt_time::trace::now_ns();
    // the counter of moments written moves when a segment is COMMITTED, and the segment open when the
    // vote came is committed a moment later (found-defects 828: 251 against 250 under load): what may
    // not happen under the vote is a moment's work STARTING
    let items = kt_time::trace::testing::capture(|| {
        p.pause();
        for _ in 0..20 {
            assert!(!rc.busy(), "the prerender worked under an export");
            std::thread::sleep(Duration::from_millis(25));
        }
    });
    let started: Vec<u64> = kt_time::trace::testing::recs(&items)
        .iter()
        .filter(|r| r.ev == kt_time::trace::Ev::Job as u16 && r.aux == kt_time::trace::job::PRERENDER && r.t > since)
        .map(|r| r.a)
        .collect();
    assert!(started.is_empty(), "moments composed under an export: {started:?}");
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

/// Test t3601 (found-defects 776): with the playhead in the middle of a heavy run the prerender
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

/// A segment codec whose encoder takes NV12 (BT.709, limited), as a VAAPI one does: its writer
/// records the planes it is given and counts the RGBA frames; its reader turns the planes back into
/// RGBA by a plain inverse (nearest chroma) — the test's "codec tolerance".
struct Nv12Segments {
    planes: std::sync::Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    rgba: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}
struct Nv12Writer {
    file: std::fs::File,
    planes: std::sync::Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    rgba: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}
const NV12: kt_media::segment::SegmentPlanes = kt_media::segment::SegmentPlanes { code: 1, bt709: true, full: false };
impl kt_media::segment::SegmentWriter for Nv12Writer {
    fn push(&mut self, _rgba: &[u8]) -> Result<(), String> {
        self.rgba.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Err("this test encoder takes NV12".into())
    }
    fn finish(self: Box<Self>) -> Result<u64, String> {
        self.file.metadata().map(|m| m.len()).map_err(|e| e.to_string())
    }
    fn encoder(&self) -> &str {
        "test-nv12"
    }
    fn planes(&self) -> Option<kt_media::segment::SegmentPlanes> {
        Some(NV12)
    }
    fn push_planes(&mut self, planes: &[u8]) -> Result<(), String> {
        use std::io::Write;
        self.planes.lock().unwrap().push(planes.to_vec());
        self.file.write_all(planes).map_err(|e| e.to_string())
    }
}
struct Nv12Reader(std::fs::File);
impl kt_media::segment::SegmentReader for Nv12Reader {
    fn frame(&mut self, index: u32, w: u32, h: u32, out: &mut Vec<u8>) -> Result<(), String> {
        use std::io::{Read, Seek, SeekFrom};
        let (wu, hu) = (w as usize, h as usize);
        let n = wu * hu + wu.div_ceil(2) * 2 * hu.div_ceil(2);
        let mut p = vec![0u8; n];
        self.0.seek(SeekFrom::Start(index as u64 * n as u64)).map_err(|e| e.to_string())?;
        self.0.read_exact(&mut p).map_err(|e| e.to_string())?;
        out.clear();
        let (kr, kb) = (0.2126f64, 0.0722f64);
        for y in 0..hu {
            for x in 0..wu {
                let yy = (p[y * wu + x] as f64 - 16.0) / 219.0;
                let c = wu * hu + (y / 2) * wu.div_ceil(2) * 2 + (x / 2) * 2;
                let cb = (p[c] as f64 - 128.0) / 224.0;
                let cr = (p[c + 1] as f64 - 128.0) / 224.0;
                let r = yy + 2.0 * (1.0 - kr) * cr;
                let b = yy + 2.0 * (1.0 - kb) * cb;
                let g = (yy - kr * r - kb * b) / (1.0 - kr - kb);
                for v in [r, g, b] {
                    out.push((v * 255.0).round().clamp(0.0, 255.0) as u8);
                }
                out.push(255);
            }
        }
        Ok(())
    }
}
impl kt_media::segment::SegmentCodec for Nv12Segments {
    fn codec(&self) -> &str {
        "test-nv12"
    }
    fn writer(&self, path: &std::path::Path, _w: u32, _h: u32, _fps: u32) -> Result<Box<dyn kt_media::segment::SegmentWriter>, String> {
        Ok(Box::new(Nv12Writer {
            file: std::fs::File::create(path).map_err(|e| e.to_string())?,
            planes: self.planes.clone(),
            rgba: self.rgba.clone(),
        }))
    }
    fn reader(&self, path: &std::path::Path) -> Result<Box<dyn kt_media::segment::SegmentReader>, String> {
        Ok(Box::new(Nv12Reader(std::fs::File::open(path).map_err(|e| e.to_string())?)))
    }
}

/// Test t3317 (export-host-copy D4): the prerender hands an encoder that takes planes the planes made
/// on the composing device — no RGBA reaches it, every moment's planes are the exact conversion of the
/// moment's composite, each moment reads back within the codec's tolerance, and the durable keys are
/// those an RGBA codec's tier writes.
#[test]
fn t3317_a_segment_is_written_from_planes() {
    let root = tempfile::tempdir().unwrap();
    let Some((p, moments)) = player(root.path(), 0.64) else { return };
    let planes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let rgba = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    p.set_render_cache(Some(Config {
        codec: std::sync::Arc::new(Nv12Segments { planes: planes.clone(), rgba: rgba.clone() }),
        root: root.path().join("nv12"),
        build: "test build".into(),
        base: None,
        cap: 1 << 30,
    }));
    prerender(&p, &moments);
    assert_eq!(rgba.load(std::sync::atomic::Ordering::Relaxed), 0, "no RGBA frame reached the encoder");
    let back: Vec<Vec<u8>> = moments.iter().map(|t| p.disk_picture(*t).expect("on disk").rgba.to_vec()).collect();
    // the moments' composites, the tier off, converted by the reference: each is among the planes written
    p.set_render_cache(None);
    let layout = kt_engine::yuv::YuvLayout { pix: kt_engine::yuv::YuvPix::Nv12, matrix: kt_engine::yuv::YuvMatrix::Bt709, full: false };
    let written = planes.lock().unwrap().clone();
    for t in &moments {
        let mut buf = Vec::new();
        let deadline = kt_time::test_deadline();
        while !p.compose_preview_into(*t, &mut buf).1 {
            assert!(Instant::now() < deadline, "the moment never composed complete");
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut want = Vec::new();
        kt_engine::yuv::convert(&layout, &buf, 160, 90, &mut want);
        assert!(written.contains(&want), "moment {}: its planes are the exact conversion of its composite", t.0);
        let i = moments.iter().position(|m| m == t).unwrap();
        // RGB: the encoder keeps no alpha
        let rgb = |v: &[u8]| v.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect::<Vec<u8>>();
        let (a, b) = (rgb(&back[i]), rgb(&buf));
        let mean = a.iter().zip(&b).map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as f64).sum::<f64>() / a.len() as f64;
        assert!(mean < 3.0, "moment {}: read back {mean:.2} from its composite", t.0);
    }
}

/// Test t3600 (found-defects 826): a run of which ONE moment is measured — its costs arrive one at a time,
/// as when the playhead steps — is prerendered from the moments before it, in one segment: the
/// prerender takes the unmeasured moments up to `RUN_REACH` on each side, so the run does not grow a
/// moment a pass, a segment each.
#[test]
fn t3600_a_run_known_by_one_moment_is_one_segment() {
    let root = tempfile::tempdir().unwrap();
    let Some((p, moments)) = player(root.path(), 0.66) else { return };
    p.set_render_cache(None);
    p.seek(moments[3]);
    p.note_preview_cost(moments[3], 1_000_000_000, (160, 90));
    p.set_render_cache(Some(Config {
        codec: std::sync::Arc::new(kt_engine::render_cache::RawSegments),
        root: root.path().to_path_buf(),
        build: "test build".into(),
        base: None,
        cap: 1 << 30,
    }));
    let items = kt_time::trace::testing::capture(|| {
        p.render_cache().kick();
        let deadline = kt_time::test_deadline();
        while !moments.iter().all(|t| p.disk_picture(*t).is_some()) {
            assert!(Instant::now() < deadline, "the prerender never wrote the run: {:?}", p.render_cache().counts());
            std::thread::sleep(Duration::from_millis(20));
        }
    });
    let stores: Vec<(u64, u64)> = kt_time::trace::testing::recs(&items)
        .iter()
        .filter(|r| r.ev == kt_time::trace::Ev::RenderCache as u16 && r.aux == kt_time::trace::render_cache::STORE)
        .map(|r| (r.c, r.a))
        .collect();
    let step = p.video_grid().frame_step();
    let holding: Vec<&(u64, u64)> = stores
        .iter()
        .filter(|(first, n)| moments.iter().any(|t| (*first as i64) <= t.0 && t.0 < *first as i64 + *n as i64 * step))
        .collect();
    assert_eq!(holding.len(), 1, "the run is one segment; written (first tick, moments): {stores:?}");
    assert!((holding[0].0 as i64) < moments[0].0, "it starts before the run's first moment: {stores:?}");
}
