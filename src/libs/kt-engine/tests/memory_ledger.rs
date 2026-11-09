//! The memory ledger through the engine (memory-ledger): a re-decoded frame finds its kept composite
//! (t2294, found-defects 119), the player tells the display's executor the loop (t2296), an export
//! claims the preview's growth and gives it back (t2299). The tests share the process's ledger: one
//! at a time.

use kt_cache::{BurstCache, CacheConfig};
use kt_engine::{FramePlan, PresentOptions, VideoGpuExec, VideoRenderer};
use kt_media::{MediaSource, Quality};
use kt_model::demo::{DemoSources, demo_project};
use kt_model::{MediaId, Project};
use kt_time::Tick;
use kt_time::mem::Tenant;
use std::sync::{Arc, Mutex};

static ONE: Mutex<()> = Mutex::new(());
const Q: Quality = Quality { width: 160, height: 90 };
const SHOW: PresentOptions = PresentOptions { show_incomplete: false, glow: false };

fn sources(p: &Project) -> std::collections::HashMap<MediaId, Box<dyn MediaSource>> {
    let mut s: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        p.media.iter().map(|m| (m.id, kt_media::factory::open_pseudo(&m.path).unwrap().unwrap())).collect();
    for (a, st) in kt_engine::gen_sources(p) {
        s.insert(a, kt_media::factory::open_gen(&st));
    }
    s
}

/// Decode the layers of `t` into `cache`.
fn decode(video: &VideoRenderer, cache: &BurstCache, srcs: &mut std::collections::HashMap<MediaId, Box<dyn MediaSource>>, t: Tick) {
    for (media, pts, dq) in video.layers_at(t, Q) {
        if let Some(s) = srcs.get_mut(&media) {
            let f = s.read_video_frame(pts, Some(dq)).unwrap();
            cache.insert_video(kt_engine::source_id(media), Some(dq), kt_engine::video_cache_idx(pts), f.into());
        }
    }
}

fn plan<'a>(v: &'a VideoRenderer, c: &'a BurstCache, t: Tick) -> FramePlan<'a> {
    v.frame_plan(c, t, Q)
}

/// t2294: a moment kept, its frame pushed out of a small cache and decoded again with the same bytes,
/// is presented from the kept composite.
#[test]
fn t2294_a_frame_decoded_again_finds_its_kept_composite() {
    let _g = ONE.lock().unwrap_or_else(|p| p.into_inner());
    kt_plugins_builtin::install();
    let Some(gpu) = kt_gpu::test_engine() else { return };
    let p = demo_project(DemoSources::Synth);
    let video = VideoRenderer::new(&p);
    let t = Tick(90_000);
    let frame = Q.width as usize * Q.height as usize * 4;
    // room for the moment's layers and little else
    let layers = video.layers_at(t, Q).len();
    let cache = BurstCache::new(CacheConfig { max_audio_bytes: 1, max_video_bytes: frame * (layers + 1) });
    let mut srcs = sources(&p);
    decode(&video, &cache, &mut srcs, t);
    assert!(gpu.present(&plan(&video, &cache, t), SHOW).unwrap(), "complete");
    assert!(!gpu.last_stages().replayed);
    let key = gpu.present_key(&plan(&video, &cache, t)).unwrap();
    // other moments push its frames out
    for k in 1..=20 {
        decode(&video, &cache, &mut srcs, Tick(t.0 + k * 90_000));
    }
    let (m, pts, dq) = video.layers_at(t, Q)[0];
    assert!(
        !cache.contains_video(kt_engine::source_id(m), Some(dq), kt_engine::video_cache_idx(pts)),
        "scene guard: the moment's frame was evicted"
    );
    // decoded again: the same bytes, the serial it had, the key it was kept under
    decode(&video, &cache, &mut srcs, t);
    let again = plan(&video, &cache, t);
    assert_eq!(gpu.present_key(&again), Some(key), "the plan key of the kept composite");
    assert!(gpu.present(&again, SHOW).unwrap());
    assert!(gpu.last_stages().replayed, "presented from the kept composite");
    assert_eq!(gpu.last_cost(), Default::default(), "nothing composed");
}

/// The display's executor with what it was told of the loop.
struct Told {
    exec: Arc<dyn VideoGpuExec>,
    loops: Mutex<Vec<Option<usize>>>,
}

impl VideoGpuExec for Told {
    fn execute_into(&self, plan: &FramePlan<'_>, out: &mut Vec<u8>) -> Result<bool, kt_engine::GpuExecError> {
        self.exec.execute_into(plan, out)
    }
    fn info(&self) -> &kt_engine::GpuBackendInfo {
        self.exec.info()
    }
    fn can_present(&self) -> bool {
        self.exec.can_present()
    }
    fn present(&self, plan: &FramePlan<'_>, o: PresentOptions) -> Result<bool, kt_engine::GpuExecError> {
        self.exec.present(plan, o)
    }
    fn present_key(&self, plan: &FramePlan<'_>) -> Option<u64> {
        self.exec.present_key(plan)
    }
    fn holds(&self, key: u64) -> bool {
        self.exec.holds(key)
    }
    fn present_kept(&self, key: u64, o: PresentOptions) -> bool {
        self.exec.present_kept(key, o)
    }
    fn keep_loop(&self, moments: Option<usize>) {
        self.loops.lock().unwrap().push(moments);
        self.exec.keep_loop(moments);
    }
}

fn wait_told(told: &Told, want: Option<usize>) {
    let deadline = kt_time::test_deadline();
    while told.loops.lock().unwrap().last() != Some(&want) {
        assert!(std::time::Instant::now() < deadline, "the executor was never told {want:?}: {:?}", told.loops.lock().unwrap());
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// t2296: the player tells the display's executor the loop region's moments when it changes, the
/// executor's want is those composites at the size it presents plus an eighth, and no region is no want.
#[test]
fn t2296_the_player_asks_for_the_loops_composites() {
    let _g = ONE.lock().unwrap_or_else(|p| p.into_inner());
    kt_plugins_builtin::install();
    let Some(gpu) = kt_gpu::test_engine() else { return };
    kt_time::mem::set_probe(|| Some(64 << 30));
    let p = demo_project(DemoSources::Synth);
    let fps = p.settings.fps as i64;
    let srcs: Vec<(MediaId, Box<dyn MediaSource>)> =
        p.media.iter().map(|m| (m.id, kt_media::factory::open_pseudo(&m.path).unwrap().unwrap())).collect();
    let (player, _ring) = kt_engine::Player::new(p, srcs);
    player.set_preview_quality(Q);
    let told = Arc::new(Told { exec: gpu.clone(), loops: Mutex::new(Vec::new()) });
    player.set_video_gpu(Some(told.clone()));
    wait_told(&told, None);
    // 400 s: past what the 512 MB base holds at this size
    let secs = 400;
    player.set_loop_region(Some(kt_time::TickRange { start: Tick(0), end: Tick(secs * 90_000) }));
    let n = (secs * fps) as usize + 1;
    wait_told(&told, Some(n));
    // the size it presents: a composite of the preview
    let t = Tick(90_000);
    let deadline = kt_time::test_deadline();
    while !player.present_preview(t, SHOW).is_some_and(|(_, c)| c) {
        assert!(std::time::Instant::now() < deadline, "the frame never came");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    player.finish_present();
    let one = Q.width as u64 * Q.height as u64 * 4;
    let want = (n as u64 * one * 9 / 8).saturating_sub(512 << 20);
    assert!(want > 0, "scene guard: the loop is past the base");
    assert_eq!(kt_time::mem::ledger().wanted(Tenant::Kept), want, "the region's composites and an eighth, past the base");
    player.set_loop_region(None);
    wait_told(&told, None);
    assert_eq!(kt_time::mem::ledger().wanted(Tenant::Kept), 0, "no region, no want");
    kt_time::mem::set_probe(|| None);
}

/// t2299: an export takes the preview's growth while it runs — the stream memo and the present memo
/// drop to their bases — and gives it back after; its frames are the frames of an export with none.
#[test]
fn t2299_the_export_takes_the_room_and_gives_it_back() {
    let _g = ONE.lock().unwrap_or_else(|p| p.into_inner());
    kt_plugins_builtin::install();
    let Some(gpu) = kt_gpu::test_engine() else { return };
    let Some(preview) = kt_gpu::test_engine() else { return };
    let p = demo_project(DemoSources::Synth);
    let range = (Tick(90_000), Tick(90_000 * 2));
    let backends = kt_engine::ExportBackends { video_gpu: Some(gpu.clone() as Arc<dyn VideoGpuExec>), ..Default::default() };
    let never = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let export = |during: &dyn Fn()| {
        let mut hashes = Vec::new();
        let srcs: Vec<(MediaId, Box<dyn MediaSource>)> =
            p.media.iter().map(|m| (m.id, kt_media::factory::open_pseudo(&m.path).unwrap().unwrap())).collect();
        kt_engine::export::render_raw_video_range_with(&p, srcs, Q, Some(range), &backends, &never, |_| during(), |f| {
            hashes.push(f.iter().fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3)));
            Ok(())
        })
        .unwrap();
        hashes
    };
    let plain = export(&|| {});
    // the preview holds grants: its present memo past a small base, the stream memo sized for a loop
    kt_time::mem::set_probe(|| Some(64 << 30));
    let one = Q.width as u64 * Q.height as u64 * 4;
    preview.set_present_base_for_tests(4 * one);
    preview.keep_loop(Some(12));
    let video = VideoRenderer::new(&p);
    let cache = BurstCache::new(CacheConfig { max_audio_bytes: 1, max_video_bytes: 64 << 20 });
    let mut srcs = sources(&p);
    for k in 0..12 {
        let t = Tick(90_000 + k * 3_000);
        decode(&video, &cache, &mut srcs, t);
        assert!(preview.present(&plan(&video, &cache, t), SHOW).unwrap());
    }
    assert_eq!(preview.present_memo_stats().0, 12, "scene guard: kept past the base");
    let stream_base = kt_engine::video::STREAM_MEMO_BASE;
    let sized = kt_engine::video::size_stream_memo(1 << 20, Some(400));
    assert!(sized > stream_base, "scene guard: the stream memo is granted");
    let stream = kt_engine::video::stream_memo();
    let seen = Mutex::new(Vec::new());
    let granted = export(&|| {
        let s = kt_time::mem::snapshot();
        seen.lock().unwrap().push((s.grant(Tenant::Kept), s.grant(Tenant::History), stream.bytes_cap(), preview.present_memo_stats()));
    });
    let seen = seen.into_inner().unwrap();
    assert!(!seen.is_empty());
    for (kept, hist, cap, (len, bytes, _)) in &seen {
        assert_eq!((*kept, *hist), (0, 0), "no grant while the export runs");
        assert_eq!(*cap, stream_base, "the stream memo at its base");
        assert!(*bytes <= 4 * one && *len <= 4, "the kept composites dropped to the base: {len}");
    }
    assert_eq!(granted, plain, "the frames do not depend on the grants");
    assert!(kt_time::mem::snapshot().grant(Tenant::Kept) > 0, "the grant is back after the export");
    assert!(kt_time::mem::snapshot().grant(Tenant::History) > 0);
    // the memos grow again at their next keep or sizing
    assert!(kt_engine::video::size_stream_memo(1 << 20, Some(400)) > stream_base);
    preview.keep_loop(None);
    kt_engine::video::size_stream_memo(1 << 20, None);
    kt_time::mem::set_probe(|| None);
}
