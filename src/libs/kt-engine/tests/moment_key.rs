//! durable-moment-key (kkt-26.11.22 D2, D7): a kept moment's key is what its picture is made of, never
//! what is decoded now. t4108, t4109, t4112, t4113, t4116 (t4110 kt-cache, t4111 kt-media, t4114
//! frames_cache_growth, t4115 render_cache). The tests with an executor skip without a wgpu adapter.

use kt_engine::{Player, PresentOptions, VideoGpuExec, VideoRenderer, source_id, video_cache_idx};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;
use std::sync::Arc;

const Q: Quality = Quality { width: 96, height: 54 };
const STEP: i64 = 3000;

fn project() -> Project {
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
        opacity: 0.8,
        fx_off: false,
        fx_in: Vec::new(),
    }
}

/// Decode what the moment reads into the cache.
fn decode(video: &VideoRenderer, cache: &kt_cache::BurstCache, srcs: &mut [(MediaId, Box<dyn MediaSource>)], t: Tick) {
    for (addr, pts, dq) in video.layers_at(t, Q) {
        let Some((_, s)) = srcs.iter_mut().find(|(m, _)| *m == addr) else { continue };
        let f = s.read_video_frame(pts, Some(dq)).unwrap();
        cache.insert_video(source_id(addr), Some(dq), video_cache_idx(pts), f.into());
    }
}

/// Evict all the cache holds by other frames.
fn crowd_out(cache: &kt_cache::BurstCache, bytes: usize) {
    for i in 0..(bytes / (256 * 256 * 4) + 4) as i64 {
        let f = kt_media::VideoFrame { pts: Tick(i), width: 256, height: 256, rgba: vec![0; 256 * 256 * 4].into_boxed_slice(), has_alpha: false };
        cache.insert_video(kt_cache::SourceId(999), Some(Quality { width: 256, height: 256 }), i, f.into());
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

/// The sources of a project as its player raises them.
fn sources(p: &Project) -> Vec<(MediaId, Box<dyn MediaSource>)> {
    let files = p.media.iter().filter_map(|m| Some((m.id, kt_media::factory::open_pseudo(&m.path)?.ok()?)));
    files.chain(kt_engine::gen_sources(p).into_iter().map(|(a, st)| (a, kt_media::factory::open_gen(&st)))).collect()
}

/// The renderer its player would build.
fn renderer(p: &Project, srcs: &[(MediaId, Box<dyn MediaSource>)], cache: &Arc<kt_cache::BurstCache>) -> VideoRenderer {
    let mut facts = kt_engine::video::SourceFacts::default();
    for (a, s) in srcs {
        facts.observe(*a, s.info());
    }
    let mut v = VideoRenderer::new_with_facts(p, &facts);
    v.set_stream_memo(Arc::new(kt_plugin_api::stream::StreamMemo::new(256 << 20)));
    v.set_source_idents(cache.ident_reader());
    v
}

fn adopt(cache: &kt_cache::BurstCache, srcs: &[(MediaId, Box<dyn MediaSource>)]) {
    for (m, src) in srcs {
        cache.adopt(source_id(*m), src.identity().unwrap());
    }
}

fn gpu() -> Option<Arc<dyn VideoGpuExec>> {
    kt_plugins_builtin::install();
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    Some(kt_gpu::test_engine()? as Arc<dyn VideoGpuExec>)
}

const SHOW: PresentOptions = PresentOptions { glow: false };

/// Test t4108 (durable-moment-key D2): one moment — a file, a generator, a reel and a Datamosh event — has
/// one key before its frames are decoded, while they are held, after others evicted them and after they
/// are decoded again, the probe plan's and the full plan's; the display's memo answers to each once the
/// moment was presented; another file under the same media id is another key, and two private caches with
/// no identity still name their frames by serials.
#[test]
fn t4108_one_key_before_during_and_after_the_decode() {
    let Some(exec) = gpu() else { return };
    let mut p = project();
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    p.media.push(MediaRef::new(MediaId(2), "synth:silence"));
    p.media.push(MediaRef::new(MediaId(3), gen_kind_path(&title(""))));
    let mut titled = clip(3, 2, 3);
    titled.r#gen = Some(title("KEY"));
    let mut mosh = FxInstance::plugin("datamosh").expect("datamosh");
    for (name, v) in [("lines", 0.0), ("before", 0.0), ("mosh", 1.0), ("gop", 12.0), ("sharp", 0.0)] {
        assert!(mosh.set_param(name, v), "{name}");
    }
    let mut moshed = clip(2, 1, 2);
    moshed.fx = vec![mosh];
    let mut inner = clip(10, 0, 1);
    inner.source_in = Tick(30_000);
    p.clips = vec![clip(1, 0, 1), moshed, titled];
    let mut reel = Sequence::new(MediaId(77), "reel".to_string());
    reel.clips = vec![inner];
    p.sequences.push(reel);
    let mut nested = clip(4, 3, 77);
    nested.opacity = 0.5;
    p.clips.push(nested);
    p.normalize();
    p.validate().unwrap();
    let mut srcs = sources(&p);
    let cache = Arc::new(kt_cache::BurstCache::new(kt_cache::CacheConfig { max_audio_bytes: 1, max_video_bytes: 8 << 20 }));
    adopt(&cache, &srcs);
    let video = renderer(&p, &srcs, &cache);
    // a short history: it fits the small cache
    let t = Tick(4 * STEP);
    let probe = || exec.present_key(&video.probe_plan(&cache, t, Q)).unwrap();
    let absent = probe();
    while !video.layers_at(t, Q).is_empty() && video.layers_at(t, Q).iter().any(|(m, pts, dq)| !cache.contains_video(source_id(*m), Some(*dq), video_cache_idx(*pts))) {
        decode(&video, &cache, &mut srcs, t);
        let _ = video.frame_plan(&cache, t, Q);
    }
    let decoded = probe();
    let full = video.frame_plan(&cache, t, Q);
    assert!(!full.starved(), "scene guard: the moment is whole");
    assert_eq!(exec.present_key(&full), Some(absent), "the full plan's");
    assert!(exec.present(&full, SHOW).unwrap());
    drop(full);
    assert!(exec.holds(probe()));
    crowd_out(&cache, 8 << 20);
    let (m, pts, dq) = video.layers_at(t, Q)[0];
    assert!(!cache.contains_video(source_id(m), Some(dq), video_cache_idx(pts)), "scene guard: evicted");
    let evicted = probe();
    assert!(exec.holds(evicted), "the kept composite is found without its sources");
    decode(&video, &cache, &mut srcs, t);
    assert_eq!((decoded, evicted, probe()), (absent, absent, absent), "the key names what is decoded now");
    assert!(video.probe_plan(&cache, t, Q).durable());
    // another file under the same media id
    cache.adopt(source_id(MediaId(1)), kt_cache::SourceIdent::of("another file"));
    assert_ne!(probe(), absent);

    // private caches know no identity: the same address, other bytes, another key
    let (a, b) = (kt_cache::BurstCache::new(Default::default()), kt_cache::BurstCache::new(Default::default()));
    let mut one = project();
    one.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    one.clips = vec![clip(1, 0, 1)];
    one.normalize();
    let mut srcs = sources(&one);
    let video = VideoRenderer::new(&one);
    for c in [&a, &b] {
        decode(&video, c, &mut srcs, t);
    }
    let key = |c: &kt_cache::BurstCache| exec.present_key(&video.probe_plan(c, t, Q)).unwrap();
    assert_ne!(key(&a), key(&b));
    assert!(!video.probe_plan(&a, t, Q).durable(), "and such a picture is never on disk");
}

/// Test t4109 (durable-moment-key D1, found-defects 1063): a title's text is edited while the retune
/// waits: the new renderer's moment composed from the OLD frame is kept under the old frame's name; once
/// the worker applied the retune the new moment is another, composed and shown; an undo finds the old one
/// kept, with nothing composed.
#[test]
fn t4109_an_old_picture_is_never_kept_under_new_values() {
    let Some(exec) = gpu() else { return };
    let scene = |text: &str| {
        let mut p = project();
        p.media.push(MediaRef::new(MediaId(1), gen_kind_path(&title(""))));
        let mut c = clip(1, 0, 1);
        c.r#gen = Some(title(text));
        c.opacity = 0.77;
        p.clips = vec![c];
        p.normalize();
        p
    };
    let cache = Arc::new(kt_cache::BurstCache::new(Default::default()));
    let (old, new) = (scene("OLD"), scene("NEW"));
    let (mut old_src, mut new_src) = (sources(&old), sources(&new));
    let t = Tick(4 * STEP);
    adopt(&cache, &old_src);
    let before = renderer(&old, &old_src, &cache);
    decode(&before, &cache, &mut old_src, t);
    assert!(exec.present(&before.frame_plan(&cache, t, Q), SHOW).unwrap());
    let old_key = exec.present_key(&before.probe_plan(&cache, t, Q)).unwrap();

    // the edit: the renderer is new, the worker has not applied the retune
    let after = renderer(&new, &new_src, &cache);
    let early = after.frame_plan(&cache, t, Q);
    assert!(!early.starved(), "scene guard: the old frame answers");
    let early_key = exec.present_key(&early).unwrap();
    assert!(exec.present(&early, SHOW).unwrap());
    drop(early);

    // the worker: the identity and the drop in one step, then the new frame
    adopt(&cache, &new_src);
    let key = exec.present_key(&after.probe_plan(&cache, t, Q)).unwrap();
    assert!(key != early_key && !exec.holds(key), "the old picture answers for the new values");
    decode(&after, &cache, &mut new_src, t);
    assert!(exec.present(&after.frame_plan(&cache, t, Q), SHOW).unwrap());
    assert!(!exec.last_stages().replayed, "the new text was not composed");

    // the undo, before its frame is decoded again
    adopt(&cache, &old_src);
    let undone = exec.present_key(&before.probe_plan(&cache, t, Q)).unwrap();
    assert!(undone == old_key && exec.holds(undone), "the old picture is kept under its own name");
    assert!(exec.present_kept(undone, SHOW));
}

/// A moving texture of one file among others.
struct Tex {
    info: kt_media::MediaInfo,
    seed: u32,
}

impl Tex {
    fn new(seed: u32) -> Tex {
        let info = kt_media::MediaInfo {
            duration: Tick(90_000 * 10),
            has_video: true,
            has_audio: false,
            fps: Some((30, 1)),
            width: 320,
            height: 180,
            sar: (1, 1),
            still: false,
        };
        Tex { info, seed }
    }
}

impl MediaSource for Tex {
    fn info(&self) -> &kt_media::MediaInfo {
        &self.info
    }
    fn identity(&self) -> Option<kt_media::SourceIdent> {
        Some(kt_media::SourceIdent::of(&format!("tex {}", self.seed)))
    }
    fn read_audio_burst(&mut self, idx: i64) -> Result<kt_media::AudioBurst, kt_media::MediaError> {
        Ok(kt_media::AudioBurst::silence(idx))
    }
    fn read_video_frame(&mut self, p: Tick, q: Option<Quality>) -> Result<kt_media::VideoFrame, kt_media::MediaError> {
        let q = q.unwrap_or(Quality { width: 320, height: 180 });
        let k = (p.0 / STEP) as u32;
        let mut rgba = vec![255u8; (q.width * q.height * 4) as usize];
        for y in 0..q.height {
            for x in 0..q.width {
                let v = ((x + 2 * k + self.seed) / 8 * (37 + self.seed) + y / 8 * 91) % 200 + 28;
                let i = ((y * q.width + x) * 4) as usize;
                (rgba[i], rgba[i + 1], rgba[i + 2]) = (v as u8, (v * 3 % 251) as u8, (255 - v) as u8);
            }
        }
        Ok(kt_media::VideoFrame { pts: p, width: q.width, height: q.height, rgba: rgba.into_boxed_slice(), has_alpha: false })
    }
    fn keyframe_before(&self, p: Tick) -> Option<Tick> {
        Some(p)
    }
}

/// Test t4116 (durable-moment-key D3, found-defects 1061): a Datamosh event whose media is relinked to
/// another file under the same media id starts a new history: the picture is a fresh renderer's over a
/// fresh memo; so through a slot in front of the stage and through its second input.
#[test]
fn t4116_a_relinked_file_starts_a_new_history() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) || gpu().is_none() || kt_plugin_api::compute::host().is_none() {
        eprintln!("skipped: no device");
        return;
    }
    kt_engine::video::install_stream_front(kt_gpu::stream_front_engine);
    let scene = |front: bool, second: bool| {
        let mut p = project();
        for m in 1..=3 {
            p.media.push(MediaRef::new(MediaId(m), "synth:ramp"));
        }
        let mut mosh = FxInstance::plugin("datamosh").expect("datamosh");
        for (name, v) in [("lines", 0.0), ("before", 6.0), ("mosh", 1.0), ("gop", 12.0), ("sharp", 0.0)] {
            assert!(mosh.set_param(name, v), "{name}");
        }
        if second {
            // the event below gives the motion
            assert!(mosh.set_param("vectors_from", 2.0));
        }
        let mut b = clip(2, 0, 2);
        b.start = Tick(30_000);
        b.duration = Tick(60_000);
        b.opacity = 1.0;
        b.fx = if front { vec![FxInstance::plugin("posterize").unwrap(), mosh] } else { vec![mosh] };
        let mut a = clip(1, 0, 1);
        a.duration = Tick(30_000);
        a.opacity = 1.0;
        p.clips = vec![a, b];
        if second {
            let mut c = clip(3, 1, 3);
            c.opacity = 1.0;
            p.clips.push(c);
        }
        p.normalize();
        p.validate().unwrap();
        p
    };
    let t = Tick(60_000);
    // compose the moment, its history ordered and decoded as it is asked
    let compose = |video: &VideoRenderer, cache: &Arc<kt_cache::BurstCache>, srcs: &mut [(MediaId, Box<dyn MediaSource>)]| -> Vec<u8> {
        let deadline = kt_time::test_deadline();
        loop {
            decode(video, cache, srcs, t);
            let (out, complete) = kt_gpu::testkit::compose(video, cache, t, Q);
            if complete {
                return out;
            }
            assert!(std::time::Instant::now() < deadline, "the mosh never came out complete");
        }
    };
    let raise = |seeds: [u32; 3]| -> Vec<(MediaId, Box<dyn MediaSource>)> {
        seeds.iter().enumerate().map(|(i, s)| (MediaId(i as u64 + 1), Box::new(Tex::new(*s)) as Box<dyn MediaSource>)).collect()
    };
    for (front, second) in [(false, false), (true, false), (false, true)] {
        let p = scene(front, second);
        let mut first = raise([0, 5, 9]);
        // the event's own file; with a second input, the second input's
        let mut relinked = if second { raise([0, 5, 23]) } else { raise([0, 23, 9]) };
        let cache = Arc::new(kt_cache::BurstCache::new(Default::default()));
        adopt(&cache, &first);
        let video = renderer(&p, &first, &cache);
        let was = compose(&video, &cache, &mut first);
        // the player's replace: the worker's adopt, the same renderer and memo
        adopt(&cache, &relinked);
        let got = compose(&video, &cache, &mut relinked);
        let clean = Arc::new(kt_cache::BurstCache::new(Default::default()));
        adopt(&clean, &relinked);
        let want = compose(&renderer(&p, &relinked, &clean), &clean, &mut relinked);
        assert_eq!(got, want, "the old file's history (front {front}, second {second})");
        if !second {
            assert_ne!(got, was, "scene guard: the other file is another picture (front {front})");
        }
    }
}

/// A player over the demo project whose decoded frames of a loop do not fit the base of its cache.
fn loop_player(opacity: f32) -> Option<(Player, Project, Vec<Tick>)> {
    let exec = gpu()?;
    let mut p = kt_model::demo::demo_project(kt_model::demo::DemoSources::Synth);
    for c in &mut p.clips {
        c.opacity = opacity;
    }
    let srcs = sources(&p);
    let config = kt_cache::CacheConfig { max_audio_bytes: 16 << 20, max_video_bytes: BASE };
    let (player, _ring) = Player::with_cache(p.clone(), srcs, config);
    player.set_preview_quality(Quality { width: 320, height: 180 });
    player.set_video_gpu(Some(exec));
    let grid = player.video_grid();
    let start = grid.align_floor(Tick(4 * 90_000));
    let moments: Vec<Tick> = (0..40).map(|i| Tick(start.0 + i * grid.frame_step())).collect();
    let end = Tick(moments[39].0 + grid.frame_step());
    player.set_loop_region(Some(kt_time::TickRange { start, end }));
    Some((player, p, moments))
}

const BASE: usize = 2 << 20;

fn wait(what: &str, mut ok: impl FnMut() -> bool) {
    let deadline = kt_time::test_deadline();
    while !ok() {
        assert!(std::time::Instant::now() < deadline, "{what}");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Show each moment of the loop, whole, as a lap does.
fn lap(player: &Player, moments: &[Tick]) {
    for t in moments {
        player.seek(*t);
        wait("a moment never came out complete", || matches!(player.present_preview(*t, SHOW), Some((_, true))));
    }
}

fn decodes() -> u64 {
    kt_time::work::count(kt_time::work::Work::Decoded)
}

fn kept(player: &Player, moments: &[Tick]) -> usize {
    let (runs, _) = player.final_video_runs();
    moments.iter().filter(|t| runs.iter().any(|r| r.contains(&t.0))).count()
}

/// Show another range until what is left of the loop's frames is gone, and come back to the loop.
fn evict(player: &Player, moments: &[Tick]) {
    let grid = player.video_grid();
    let region = grid.align_floor(Tick(9 * 90_000)).0;
    player.set_loop_region(None);
    for i in 0..24 {
        let t = Tick(region + i * grid.frame_step());
        player.seek(t);
        wait("the other range never came", || matches!(player.present_preview(t, SHOW), Some((_, true))));
    }
    let end = Tick(moments[39].0 + grid.frame_step());
    player.set_loop_region(Some(kt_time::TickRange { start: moments[0], end }));
    // the window from the last place is done
    let mut last = decodes();
    wait("the decoders never rested", || {
        std::thread::sleep(std::time::Duration::from_millis(250));
        std::mem::replace(&mut last, decodes()) == decodes()
    });
}

/// The tests here read the process-wide work counters: one at a time.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Test t4112 (durable-moment-key D4, found-defects 965): a loop whose frames do not fit the cache's base
/// plays from its kept composites after its sources are evicted: once it is kept whole the cap is the base,
/// and after a lap of another range evicted the rest every moment is a replay, nothing is decoded and no
/// key moves.
/// - **PATH** put the loop region over four seconds of a dense edit → press Space → the first laps compose,
///   the bars turn bright → let it play on → every lap plays whole, and the memory of decoded frames in
///   Diagnostics has come down
#[test]
fn t4112_a_loop_plays_from_its_kept_composites_without_its_sources() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let Some((player, p, moments)) = loop_player(0.52) else { return };
    let frames = kt_engine::loop_frame_bytes(&VideoRenderer::new(&p), Quality { width: 320, height: 180 }, (moments[0].0, moments[39].0), usize::MAX);
    assert!(frames > BASE, "scene guard: the loop fits the base ({frames})");
    lap(&player, &moments);
    wait("the loop was never kept whole", || kept(&player, &moments) == moments.len());
    wait("the cap never came down to the base", || {
        player.seek(moments[0]);
        player.cache.max_video_bytes() == BASE
    });
    evict(&player, &moments);
    let (d0, r0) = (decodes(), kt_time::work::count(kt_time::work::Work::CompositeReplay));
    for _ in 0..2 {
        for t in &moments {
            player.seek(*t);
            assert!(matches!(player.present_preview(*t, SHOW), Some((_, true))), "{t:?} was not shown at once");
        }
    }
    assert_eq!(decodes(), d0, "a kept moment decoded its sources");
    let replays = kt_time::work::count(kt_time::work::Work::CompositeReplay) - r0;
    assert_eq!(replays, 2 * moments.len() as u64, "every moment is a replay");
}

/// Test t4113 (durable-moment-key D4): the loop plays from its kept composites, its sources evicted. An edit
/// outside it asks no source frame of it, and its moments are confirmed; an edit of one of its events asks
/// the changed moments' sources, and the moment on screen is complete again.
#[test]
fn t4113_an_edit_elsewhere_asks_no_decode_an_edit_here_asks_its_own() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let Some((player, p, moments)) = loop_player(0.57) else { return };
    lap(&player, &moments);
    wait("the loop was never kept whole", || kept(&player, &moments) == moments.len());
    wait("the cap never came down to the base", || {
        player.seek(moments[0]);
        player.cache.max_video_bytes() == BASE
    });
    evict(&player, &moments);
    assert!(moments.iter().all(|t| !player.missing_layers(*t).is_empty()), "scene guard: the loop's sources are evicted");
    let d0 = decodes();
    // an event wholly outside the loop
    let (a, b) = (moments[0], moments[39]);
    let mut q = p.clone();
    let far = q.clips.iter_mut().find(|c| c.kind == ClipKind::Video && (c.start > b || c.start + c.duration <= a)).expect("scene guard: an event outside the loop");
    far.opacity = 0.11;
    player.set_project(q.clone());
    wait("the moments were never confirmed", || kept(&player, &moments) == moments.len() && !player.final_video_runs().1);
    assert_eq!(decodes(), d0, "an edit elsewhere decoded the loop's sources");

    // an event of the loop
    let here = q.clips.iter_mut().find(|c| c.kind == ClipKind::Video && c.start <= a && c.start + c.duration > a).expect("scene guard: an event in the loop");
    here.opacity = 0.23;
    player.set_project(q);
    wait("the edited moment never came", || matches!(player.present_preview(a, SHOW), Some((_, true))));
    assert!(decodes() > d0, "the changed moment asks its sources");
}
