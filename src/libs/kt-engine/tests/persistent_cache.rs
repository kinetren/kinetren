//! persistent-cache (kkt-26.11.22 D1, kkt-26.11.21 A10, B1): the final preview frame on disk, at the
//! quality the preview plays. t3445, t3451, t3452, t3453, t4106. The tests with an executor skip without a
//! wgpu adapter.

use kt_engine::render_cache::{Config, Made, Manifest, RawSegments};
use kt_engine::{Player, PresentOptions, VideoGpuExec, copies};
use kt_media::cache_folder::{Folder, Tenant};
use kt_media::{MediaSource, Quality, copy};
use kt_model::*;
use kt_time::Tick;
use kt_time::trace::Ev;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const SMALL: Quality = Quality { width: 96, height: 54 };
const LARGE: Quality = Quality { width: 192, height: 108 };
const STEP: i64 = 3000;
const SHOW: PresentOptions = PresentOptions { glow: false };

fn project() -> Project {
    kt_plugins_builtin::install();
    Project::new(ProjectSettings {
        fps: 30,
        width: 192,
        height: 108,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    })
}

fn clip(id: u64, lane: u32, media: u64, at: std::ops::Range<i64>) -> ClipEvent {
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
        start: Tick(at.start * STEP),
        duration: Tick((at.end - at.start) * STEP),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 0.8,
        fx_off: false,
        fx_in: Vec::new(),
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

fn sources(p: &Project) -> Vec<(MediaId, Box<dyn MediaSource>)> {
    let files = p.media.iter().filter_map(|m| Some((m.id, kt_media::factory::open_pseudo(&m.path)?.ok()?)));
    files.chain(kt_engine::gen_sources(p).into_iter().map(|(a, st)| (a, kt_media::factory::open_gen(&st)))).collect()
}

fn gpu() -> Option<Arc<dyn VideoGpuExec>> {
    kt_plugins_builtin::install();
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    Some(kt_gpu::test_engine()? as Arc<dyn VideoGpuExec>)
}

/// A player with the tier at `root`, which prerenders what costs `heavy` ns and more.
fn player(p: &Project, root: &Path, q: Quality, heavy: u64) -> Option<Player> {
    let exec = gpu()?;
    let (player, _ring) = Player::new(p.clone(), sources(p));
    player.set_preview_quality(q);
    player.set_video_gpu(Some(exec));
    player.render_cache().set_heavy_for_tests(Some(heavy));
    player.set_render_cache(Some(Config {
        codec: Arc::new(RawSegments),
        root: root.to_path_buf(),
        made: Made::of(60),
        cap: None,
    }));
    Some(player)
}

fn wait(what: &str, mut ok: impl FnMut() -> bool) {
    let deadline = kt_time::test_deadline();
    while !ok() {
        assert!(std::time::Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn at(i: i64) -> Tick {
    Tick(i * STEP)
}

/// The moments on disk at the preview's quality.
fn on_disk(player: &Player, moments: i64) -> Vec<i64> {
    (0..moments).filter(|i| player.disk_picture(at(*i)).is_some()).collect()
}

/// Measure the moments heavy by hand and wait until the prerender wrote them.
fn prerender(player: &Player, moments: i64, q: Quality) {
    for i in 0..moments {
        player.note_preview_cost(at(i), 1_000_000_000, (q.width, q.height));
    }
    player.render_cache().kick();
    wait("the prerender never wrote the moments", || on_disk(player, moments).len() as i64 == moments);
}

/// The files under `dir`, relative, with `/`.
fn files(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            match e.path().is_dir() {
                true => walk(root, &e.path(), out),
                false => out.push(e.path().strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/")),
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out
}

/// A heavy title over a ramp: the title at `span`, an event with a chain at 20..30.
fn edited(span: std::ops::Range<i64>) -> Project {
    let mut p = project();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.media.push(MediaRef::new(MediaId(3), gen_kind_path(&title(""))));
    let mut titled = clip(3, 0, 3, span);
    titled.r#gen = Some(title("KEEP"));
    let mut heavy = clip(2, 1, 1, 20..30);
    heavy.fx = vec![FxInstance::blur(4)];
    p.clips = vec![clip(1, 2, 1, 0..30), heavy, titled];
    p.normalize();
    p.validate().unwrap();
    p
}

/// Test t3451 (persistent-cache D1, D2; the owner, kkt-26.11.22 D1): a project with a Datamosh event, a
/// track stage and a reel instance, prerendered until nothing is left to write: every entry of `results/`
/// is a frame segment whose manifest names whole moments, and there is no entry of a stage.
#[test]
fn t3451_only_whole_frames_lie_in_the_store() {
    let root = tempfile::tempdir().unwrap();
    let mut p = project();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.media.push(MediaRef::new(MediaId(2), "synth:silence"));
    let mut mosh = FxInstance::plugin("datamosh").expect("datamosh");
    for (name, v) in [("lines", 0.0), ("before", 0.0), ("mosh", 1.0), ("gop", 12.0), ("sharp", 0.0)] {
        assert!(mosh.set_param(name, v), "{name}");
    }
    let mut moshed = clip(2, 1, 2, 0..8);
    moshed.fx = vec![mosh, FxInstance::blur(2)];
    p.clips = vec![clip(1, 2, 1, 0..8), moshed];
    let mut reel = Sequence::new(MediaId(77), "reel".to_string());
    reel.clips = vec![clip(10, 0, 1, 0..8)];
    p.sequences.push(reel);
    let mut nested = clip(4, 0, 77, 0..8);
    nested.opacity = 0.5;
    p.clips.push(nested);
    p.normalize();
    // the track the Datamosh event is on (a new project may carry default tracks)
    let track = p.tracks.iter_mut().filter(|t| t.kind == LaneKind::Video).nth(1).expect("scene guard: the second video track");
    track.fx.push(FxInstance::blur(3));
    p.validate().unwrap();
    let Some(player) = player(&p, root.path(), SMALL, 1) else { return };
    prerender(&player, 8, SMALL);
    wait("the prerender never settled", || player.render_cache().settled());
    drop(player);
    let mut frames = 0;
    for f in files(&root.path().join("results")) {
        let parts: Vec<&str> = f.split('/').collect();
        assert!(matches!(parts.as_slice(), [v, _, "frames", _] if v.starts_with('v')), "an entry of a stage: {f}");
        if f.ends_with(".idx") {
            let m = Manifest::parse(&std::fs::read_to_string(root.path().join("results").join(&f)).unwrap()).expect("a frame segment");
            assert_eq!((m.w, m.h), (SMALL.width, SMALL.height), "a picture that is not the whole frame");
            frames += m.keys.len();
        }
    }
    assert_eq!(frames, 8, "the store holds the moments and nothing else");
    let others: Vec<String> = files(root.path()).into_iter().filter(|f| f.starts_with("media/") || f.starts_with("mix/") || f.contains("layers")).collect();
    assert!(others.is_empty(), "{others:?}");
}

/// Test t3452 (persistent-cache D2, D8): a title moved by one moment misses exactly the moments it leaves
/// and enters, whatever heavy event stands beside it; a raised quality reads no frame of the former, the
/// prerender writes the new one's at rest, and the former's stay.
/// - **PATH** play the heavy range → pause, the bar turns bright → drag the title one second right → the
///   bar is dim only where the title was and is → set the preview quality to 1/1 → the bar is dim, at rest
///   it turns bright again → back to 1/2 → bright at once
#[test]
fn t3452_an_edit_renders_again_only_the_moments_it_touches() {
    let root = tempfile::tempdir().unwrap();
    let Some(player) = player(&edited(10..12), root.path(), SMALL, 1) else { return };
    prerender(&player, 30, SMALL);
    // no prerender from here on until it is let
    player.render_cache().set_quiet_for_tests(Some(Duration::from_secs(3600)));
    player.render_cache().note_input();
    player.set_project(edited(11..13));
    player.wait_built();
    let held = on_disk(&player, 30);
    // the title covers 11 before and after, at another moment of its own
    let moved = if held.contains(&11) { vec![10, 12] } else { vec![10, 11, 12] };
    assert_eq!(held, (0..30).filter(|i| !moved.contains(i)).collect::<Vec<_>>(), "the moments the title left or entered, and no other");
    // a kept moment beside the edit is shown from the store, with no chain run
    let items = kt_time::trace::testing::capture(|| {
        player.seek(at(25));
        let r = player.present_preview(at(25), SHOW);
        assert!(matches!(r, Some((_, true))), "the kept moment is there at the first ask: {r:?}");
    });
    let recs = kt_time::trace::testing::recs(&items);
    let of = |ev: Ev| recs.iter().filter(|r| r.ev == ev as u16).count();
    let restored = recs.iter().any(|r| r.ev == Ev::RenderCache as u16 && r.aux == kt_time::trace::render_cache::RESTORE);
    assert!(restored && of(Ev::GpuExec) == 0, "read from the store with no chain run");

    // another quality: none of these is read
    player.render_cache().note_input();
    player.set_preview_quality(LARGE);
    assert!(on_disk(&player, 30).is_empty(), "a frame of another quality was read");
    player.render_cache().set_quiet_for_tests(Some(Duration::ZERO));
    player.render_cache().kick();
    wait("the prerender never wrote the raised quality", || on_disk(&player, 30).len() == 30);
    let (w, h) = player.disk_picture(at(25)).map(|p| (p.w, p.h)).unwrap();
    assert_eq!((w, h), (LARGE.width, LARGE.height));
    player.set_preview_quality(SMALL);
    assert!(on_disk(&player, 30).len() >= held.len(), "the former quality's frames went without a need");
}

/// Test t4106 (persistent-cache D8): a moment light at a small preview and heavy at a large one is written
/// at the large; one measured only small is judged by that time scaled by pixels, its own time replacing
/// the estimate once composed; one light in both is left alone.
#[test]
fn t4106_a_moment_is_judged_heavy_at_the_quality_it_is_written_at() {
    let root = tempfile::tempdir().unwrap();
    const HEAVY: u64 = 1_000;
    let mut p = project();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.clips = vec![clip(1, 0, 1, 0..3)];
    p.normalize();
    let Some(player) = player(&p, root.path(), SMALL, HEAVY) else { return };
    // 0: measured at both; 1: small only, 4 × 300 is heavy; 2: small only, 4 × 100 is light
    player.note_preview_cost(at(0), 10, (SMALL.width, SMALL.height));
    player.note_preview_cost(at(0), 5_000, (LARGE.width, LARGE.height));
    player.note_preview_cost(at(1), 300, (SMALL.width, SMALL.height));
    player.note_preview_cost(at(2), 100, (SMALL.width, SMALL.height));
    player.render_cache().kick();
    wait("the prerender never settled", || player.render_cache().settled());
    assert!(on_disk(&player, 3).is_empty(), "at the small size nothing is heavy: {:?}", on_disk(&player, 3));
    player.set_preview_quality(LARGE);
    player.render_cache().kick();
    wait("the moments heavy at the large size were not written", || on_disk(&player, 3) == [0, 1]);
    wait("the prerender never settled", || player.render_cache().settled());
    assert_eq!(on_disk(&player, 3), [0, 1], "a moment light at both sizes was written");
    let own = player.preview_cost(at(1), at(2)).unwrap();
    assert_eq!(own.size, (LARGE.width, LARGE.height), "measured at its own size once composed");
}

/// Test t3453 (persistent-cache D1; project-cache-folder D3): frames and copies go by one value order: of
/// the same bytes the one a read of which saves less goes first, and a frame read in the last minute is not
/// dropped for anything.
#[test]
fn t3453_a_cheap_copy_goes_before_an_expensive_frame() {
    let root = tempfile::tempdir().unwrap();
    let f = Folder::open(&root.path().join("kinetren-cache")).unwrap();
    let (frames, copy) = ("results/v3/dev/frames/a.mkv", "media/00112233445566778899aabbccddeeff/copy-960x540.mkv");
    let put = |rel: &str, cost_us: u64| {
        let p = f.root().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, vec![0u8; 4000]).unwrap();
        f.wrote(rel, 4000, cost_us);
        f.used(rel);
    };
    put(frames, 40_000);
    put(copy, 3_000);
    f.set_cap(f.sizes().values().map(|s| s.0).sum::<u64>() + 1000);
    // both were read just now: nothing goes for a newcomer
    assert!(!f.ask_room(4000) && f.root().join(frames).exists() && f.root().join(copy).exists());
    f.set_protect_for_tests(Some(Duration::ZERO));
    assert!(f.ask_room(4000));
    assert!(f.root().join(frames).exists() && !f.root().join(copy).exists(), "the cheap copy goes first");
    assert_eq!(f.counts(Tenant::Copy).2, 1);
    assert!(f.ask_room(8000) && !f.root().join(frames).exists(), "then the frame");
}

/// Test t3445 (persistent-cache D5, invariant 11): an export while the preview plays from its kept frames
/// is the export of a fresh start: the same frames, the same samples, and nothing read of the store.
#[test]
fn t3445_an_export_never_reads_the_store() {
    let root = tempfile::tempdir().unwrap();
    let p = edited(10..12);
    let Some(player) = player(&p, root.path(), LARGE, 1) else { return };
    let export = || {
        let (mut video, mut audio) = (Vec::new(), Vec::new());
        let backends = kt_engine::ExportBackends { video_gpu: gpu(), ..Default::default() };
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        kt_engine::export::render_raw_video_range_with(&p, sources(&p), LARGE, None, &backends, &cancel, |_| {}, |f| {
            video.extend_from_slice(f);
            Ok(())
        })
        .unwrap();
        kt_engine::render_raw_audio(&p, sources(&p), |s| {
            audio.extend_from_slice(s);
            Ok(())
        })
        .unwrap();
        (video, audio)
    };
    let fresh = export();
    prerender(&player, 30, LARGE);
    player.seek(at(0));
    player.play();
    let playing = export();
    assert!(fresh == playing, "the export moved with a preview that plays from the store");
    let folder = Folder::peek(root.path()).expect("the store's folder");
    // the preview's own reads end with it: what is counted now is the export's
    drop(player);
    let reads = || [Tenant::Frames, Tenant::Copy].map(|t| (folder.counts(t).0, folder.counts(t).1));
    let at_start = reads();
    let again = export();
    assert_eq!(reads(), at_start, "an export asked the store");
    assert!(fresh == again, "the export moved with the store");
}

// ---- source copies -------------------------------------------------------------------------------

/// A heavy "file" of the test: 384×216 of one colour, its sound a sine; a copy is its size's picture.
struct Fake {
    info: kt_media::MediaInfo,
    shade: u8,
    name: String,
    sound: kt_media::synthetic::SyntheticSource,
    reads: Arc<std::sync::atomic::AtomicU64>,
}

impl MediaSource for Fake {
    fn info(&self) -> &kt_media::MediaInfo {
        &self.info
    }
    fn identity(&self) -> Option<kt_media::SourceIdent> {
        Some(kt_media::SourceIdent::of(&self.name))
    }
    fn read_audio_burst(&mut self, idx: i64) -> Result<kt_media::AudioBurst, kt_media::MediaError> {
        self.sound.read_audio_burst(idx)
    }
    fn read_video_frame(&mut self, pts: Tick, q: Option<Quality>) -> Result<kt_media::VideoFrame, kt_media::MediaError> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let q = q.unwrap_or(Quality { width: self.info.width, height: self.info.height });
        Ok(kt_media::VideoFrame {
            pts,
            width: q.width,
            height: q.height,
            rgba: [self.shade, self.shade, self.shade, 255].repeat((q.width * q.height) as usize).into_boxed_slice(),
            has_alpha: false,
        })
    }
    fn keyframe_before(&self, p: Tick) -> Option<Tick> {
        Some(p)
    }
}

/// What the tests' copies are made by: `long_gop` is what the file says of itself.
struct FakeCopies {
    long_gop: bool,
    reads: Arc<std::sync::atomic::AtomicU64>,
    copied: Arc<std::sync::atomic::AtomicU64>,
    /// Frames a job is allowed to write before it waits for more.
    gate: Arc<std::sync::atomic::AtomicU64>,
    asked: Arc<std::sync::Mutex<Vec<(u32, u32)>>>,
}

const NATIVE: (u32, u32) = (3840, 2160);

impl FakeCopies {
    fn new(long_gop: bool) -> Arc<FakeCopies> {
        Arc::new(FakeCopies {
            long_gop,
            reads: Arc::default(),
            copied: Arc::default(),
            gate: Arc::new(std::sync::atomic::AtomicU64::new(u64::MAX)),
            asked: Arc::default(),
        })
    }

    fn open(&self, shade: u8, name: &str) -> Fake {
        Fake {
            info: kt_media::MediaInfo {
                duration: Tick(90_000),
                has_video: true,
                has_audio: true,
                fps: Some((30, 1)),
                width: NATIVE.0,
                height: NATIVE.1,
                sar: (1, 1),
                still: false,
            },
            shade,
            name: name.into(),
            sound: kt_media::synthetic::SyntheticSource::new(Tick(90_000), kt_media::synthetic::AudioPattern::Sine { hz: 440.0 }),
            reads: if shade == 200 { self.reads.clone() } else { self.copied.clone() },
        }
    }
}

impl copy::CopyMaker for FakeCopies {
    fn facts(&self, _: &Arc<Folder>, path: &Path) -> Option<copy::CopyFacts> {
        let id = kt_media::index::ContentId(xxhash_rust::xxh3::xxh3_128(path.to_string_lossy().as_bytes()).to_le_bytes());
        Some(copy::CopyFacts { id, size: NATIVE, long_gop: self.long_gop, frames: 30, copyable: true })
    }
    fn make(&self, _: &Path, dest: &Path, size: (u32, u32), go: &mut dyn FnMut(u64) -> copy::Go) -> Result<copy::Copied, copy::CopyError> {
        self.asked.lock().unwrap().push(size);
        let mut n = 0;
        while n < 30 {
            match go(n) {
                copy::Go::Stop => return Err(copy::CopyError::Stopped),
                copy::Go::On if n < self.gate.load(std::sync::atomic::Ordering::Acquire) => n += 1,
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
        std::fs::write(dest, b"copy").map_err(|e| copy::CopyError::Failed(e.to_string()))?;
        Ok(copy::Copied { frames: n, encoder: "fake".into(), decode_us: 20_000 })
    }
    fn original(&self, _: &Arc<Folder>, _: &Path) -> Result<Box<dyn MediaSource>, String> {
        Ok(Box::new(self.open(200, "original")))
    }
    fn copy(&self, path: &Path) -> Result<Box<dyn MediaSource>, String> {
        match path.is_file() {
            true => Ok(Box::new(self.open(190, &path.to_string_lossy()))),
            false => Err("none".into()),
        }
    }
    fn check_camera(&self, _: &Path, _: &Path) -> Result<(), copy::Mismatch> {
        Ok(())
    }
}

/// A 4K project of one 4K file, its sound under a slowed reverb, at `q`.
fn copied(root: &Path, maker: &Arc<FakeCopies>, q: Quality) -> Option<Player> {
    let exec = gpu()?;
    let mut p = project();
    (p.settings.width, p.settings.height) = NATIVE;
    p.media.push(MediaRef::new(MediaId(1), "/footage/a.mp4"));
    let mut sound = clip(2, 0, 1, 0..30);
    sound.kind = ClipKind::Audio;
    sound.fx = vec![FxInstance::slowed_reverb(0.8, 0.5, 60.0)];
    let mut picture = clip(1, 0, 1, 0..30);
    picture.opacity = 1.0;
    p.clips = vec![picture, sound];
    p.normalize();
    p.validate().unwrap();
    let (player, _ring) = Player::new(p, vec![(MediaId(1), Box::new(maker.open(200, "original")) as Box<dyn MediaSource>)]);
    player.set_preview_quality(q);
    player.set_video_gpu(Some(exec));
    player.copies().set_config(Some(copies::Config {
        maker: maker.clone(),
        root: root.to_path_buf(),
        resolve: Arc::new(|p| Some(p.into())),
    }));
    Some(player)
}

fn at_part(n: u32) -> Quality {
    Quality { width: NATIVE.0 / n, height: NATIVE.1 / n }
}

fn card(p: &Player) -> Option<copies::Card> {
    p.copies().card(MediaId(1))
}

fn copy_at(n: u32) -> Option<copies::Card> {
    Some(copies::Card::Copy(NATIVE.0 / n, NATIVE.1 / n))
}

/// Test t3449 (persistent-cache D3): a costly 4K file at preview 1/4 gets a 1/4 copy and plays it; at 1/2
/// the 1/4 one plays until the 1/2 is ready, then its file is gone; at 1/8 no job starts and the 1/2 is read;
/// at 1/1 the original; Off reads none; a cheap file never gets one.
#[test]
fn t3449_raising_the_quality_makes_a_better_copy_lowering_costs_nothing() {
    let root = tempfile::tempdir().unwrap();
    let maker = FakeCopies::new(true);
    let Some(player) = copied(root.path(), &maker, at_part(4)) else { return };
    let settled = || wait("the copies never settled", || player.copies().settled());
    let files = || files(root.path()).into_iter().filter(|f| f.contains("/copy-")).collect::<Vec<_>>();
    wait("the 1/4 copy was never made", || card(&player) == copy_at(4));
    settled();
    // the 1/2 job is held at its first frame
    maker.gate.store(0, std::sync::atomic::Ordering::Release);
    player.set_preview_quality(at_part(2));
    wait("the 1/2 job never started", || maker.asked.lock().unwrap().last() == Some(&(1920, 1080)));
    assert!(matches!(card(&player), Some(copies::Card::Making(..))));
    let q = at_part(2);
    let seen = maker.copied.load(std::sync::atomic::Ordering::Relaxed);
    player.request_video(MediaId(1), at(7), q);
    wait("the 1/4 copy does not play meanwhile", || maker.copied.load(std::sync::atomic::Ordering::Relaxed) > seen);
    maker.gate.store(u64::MAX, std::sync::atomic::Ordering::Release);
    wait("the 1/2 copy never came", || card(&player) == copy_at(2));
    settled();
    assert_eq!(files().len(), 1, "the smaller copy stayed: {:?}", files());
    let jobs = maker.asked.lock().unwrap().len();
    player.set_preview_quality(at_part(8));
    settled();
    assert_eq!((card(&player), maker.asked.lock().unwrap().len()), (copy_at(2), jobs), "a lower quality started a job");
    player.set_preview_quality(at_part(1));
    wait("the original does not play at 1/1", || card(&player).is_none());
    player.set_preview_quality(at_part(2));
    wait("the copy came back", || card(&player) == copy_at(2));
    player.copies().set_mode(copy::CopyMode::Off);
    wait("Off reads a copy", || card(&player).is_none());
    drop(player);

    let cheap = FakeCopies::new(false);
    let root = tempfile::tempdir().unwrap();
    let Some(player) = copied(root.path(), &cheap, at_part(4)) else { return };
    wait("the copies never settled", || player.copies().settled());
    assert!(cheap.asked.lock().unwrap().is_empty() && card(&player).is_none(), "a cheap file got a copy");
}

/// Test t3444 (persistent-cache D3): copies switched off and on during playback and on a pause: the
/// playhead goes on, nothing of the sound is decoded again, the pictures of the swapped source go and the
/// next are the copy's.
#[test]
fn t3444_a_switch_moves_the_picture_only() {
    let root = tempfile::tempdir().unwrap();
    let maker = FakeCopies::new(true);
    let Some(player) = copied(root.path(), &maker, at_part(4)) else { return };
    wait("the copy was never made", || card(&player) == copy_at(4));
    let sound = || player.cache.audio_bytes();
    let pictures = || player.cache.contains_video(kt_engine::source_id(MediaId(1)), Some(at_part(4)), kt_engine::video_cache_idx(at(5)));
    let fetch = || {
        wait("the picture never came", || {
            player.request_video(MediaId(1), at(5), at_part(4));
            pictures()
        })
    };
    fetch();
    wait("the sound was never read", || sound() > 0);
    for playing in [true, false] {
        player.seek(at(0));
        if playing {
            player.play();
        }
        for mode in [copy::CopyMode::Off, copy::CopyMode::AsPreview] {
            fetch();
            let (heard, head, generation) = (sound(), player.transport.playhead_frames(), player.transport.generation());
            player.copies().set_mode(mode);
            let want = if mode == copy::CopyMode::Off { None } else { copy_at(4) };
            wait("the switch never came", || card(&player) == want);
            assert!(sound() >= heard, "the decoded sound went with the picture");
            assert_eq!(player.transport.generation(), generation, "the switch sought");
            assert!(player.transport.playhead_frames() >= head);
            fetch();
            let f = player.cache.get_video(kt_engine::source_id(MediaId(1)), Some(at_part(4)), kt_engine::video_cache_idx(at(5)));
            assert_eq!(f.map(|f| f.rgba[0]), Some(if want.is_some() { 190 } else { 200 }), "the picture of the file it plays from");
        }
        player.pause();
    }
}

/// Test t3443 (persistent-cache D3): the plan over a copy is the plan over the original — the same layers'
/// geometry, masks and chain, the same decode sizes, the same renderer; only what the frame is named by
/// differs.
#[test]
fn t3443_the_same_plan_a_softer_picture() {
    let root = tempfile::tempdir().unwrap();
    let maker = FakeCopies::new(true);
    let Some(player) = copied(root.path(), &maker, at_part(4)) else { return };
    player.copies().set_mode(copy::CopyMode::Off);
    wait("the copies never settled", || player.copies().settled() && card(&player).is_none());
    let orders = |p: &Player| p.video_decode_qualities();
    let (a, built) = (orders(&player), player.built_seq());
    let ident = || player.cache.ident(kt_engine::source_id(MediaId(1)));
    let was = ident();
    player.copies().set_mode(copy::CopyMode::AsPreview);
    wait("the copy was never made", || card(&player) == copy_at(4));
    assert_eq!((orders(&player), player.built_seq()), (a, built), "the plan moved with the copy");
    assert_ne!(ident(), was, "a composite of the copy must not be found under the original's key");
}

/// Test t3446 (persistent-cache D3): a moment prerendered from a copy is not shown once copies are off, nor
/// after a restart; it composes from the original and keeps its own entry.
#[test]
fn t3446_the_disk_tier_keeps_copy_and_original_apart() {
    let root = tempfile::tempdir().unwrap();
    let maker = FakeCopies::new(true);
    let tier = |player: &Player| {
        player.render_cache().set_heavy_for_tests(Some(1));
        player.set_render_cache(Some(Config {
            codec: Arc::new(RawSegments),
            root: root.path().to_path_buf(),
            made: Made::of(60),
            cap: None,
        }));
    };
    let q = at_part(8);
    let Some(player) = copied(root.path(), &maker, q) else { return };
    wait("the copy was never made", || card(&player) == copy_at(8));
    tier(&player);
    prerender(&player, 4, q);
    let shade = |p: &Player| p.disk_picture(at(2)).map(|p| p.rgba[0]);
    assert_eq!(shade(&player), Some(190), "the copy's picture");
    drop(player);
    let Some(player) = copied(root.path(), &maker, q) else { return };
    player.copies().set_mode(copy::CopyMode::Off);
    tier(&player);
    wait("the copies never settled", || player.copies().settled());
    assert_eq!(shade(&player), None, "the copy's entry was found for the original");
    prerender(&player, 4, q);
    assert_eq!(shade(&player), Some(200), "the original's own entry");
    player.copies().set_mode(copy::CopyMode::AsPreview);
    wait("the copy came back", || card(&player) == copy_at(8));
    assert_eq!(shade(&player), Some(190), "and the copy's is there still");
}
