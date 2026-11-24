//! persistent-cache (kkt-26.11.22 D1, kkt-26.11.21 A10, B1): the final preview frame on disk, at the
//! quality the preview plays. t3445, t3451, t3452, t3453, t4106. The tests with an executor skip without a
//! wgpu adapter.

use kt_engine::render_cache::{Config, Made, Manifest, RawSegments};
use kt_engine::{Player, PresentOptions, VideoGpuExec};
use kt_media::cache_folder::{Folder, Tenant};
use kt_media::{MediaSource, Quality};
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
