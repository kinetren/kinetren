//! t1457 (export-throughput D5), t1573 (compute-kernels-render D4), capability export: the GPU export
//! with its readback ring and on several devices.
//! `submit`/`collect` give the bytes `execute_into` gives, frame for frame, and the pipelined
//! export on the GPU writes the serial GPU export's frames (one device: byte for byte,
//! invariant 11). Skipped without an adapter (`KT_REQUIRE_GPU=1` makes it a failure).
//! t2960 (export-readback D1): the ring holds as many frames as its bytes allow, more than three
//! at 1080p, and every one of them gives `execute_into`'s bytes.

use kt_engine::export::{ExportConverter, ExportSink, render_pipelined, render_raw_video_range_with};
use kt_engine::{ExportBackends, VideoGpuExec, VideoRenderer};
use kt_media::{MediaSource, Quality};
use kt_model::{MediaId, Project};
use kt_time::Tick;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn sources(p: &Project) -> Vec<(MediaId, Box<dyn MediaSource>)> {
    p.media.iter().map(|m| (m.id, kt_media::factory::open_pseudo(&m.path).unwrap().unwrap())).collect()
}

struct CopyConv;
impl ExportConverter for CopyConv {
    type Frame = Vec<u8>;
    fn convert(&mut self, rgba: &[u8], out: &mut Vec<u8>) -> std::io::Result<()> {
        out.clear();
        out.extend_from_slice(rgba);
        Ok(())
    }
}
#[derive(Default)]
struct Hashes(Vec<u64>);
impl ExportSink for Hashes {
    type Frame = Vec<u8>;
    type Converter = CopyConv;
    fn converter(&self) -> std::io::Result<CopyConv> {
        Ok(CopyConv)
    }
    fn alloc_frame(&self) -> std::io::Result<Vec<u8>> {
        Ok(Vec::new())
    }
    fn write_video(&mut self, f: &Vec<u8>) -> std::io::Result<()> {
        self.0.push(fnv(f));
        Ok(())
    }
    fn write_audio(&mut self, _: &[f32]) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn t1457_the_readback_ring_gives_the_executed_bytes() {
    kt_plugins_builtin::install();
    let Some(gpu) = kt_gpu::test_engine() else { return };
    let p = kt_model::demo::demo_project(kt_model::demo::DemoSources::Synth);
    let q = Quality { width: 640, height: 360 };
    let range = (Tick(90_000 * 3), Tick(90_000 * 5));
    let never = Arc::new(AtomicBool::new(false));
    let backends = ExportBackends { video_gpu: Some(gpu.clone() as Arc<dyn VideoGpuExec>), ..Default::default() };

    // the serial GPU export (execute_into per frame)
    let mut serial = Vec::new();
    render_raw_video_range_with(&p, sources(&p), q, Some(range), &backends, &never, |_| {}, |f| {
        serial.push(fnv(f));
        Ok(())
    })
    .unwrap();
    // the pipelined one (submit ahead, collect in order) on a FRESH engine: no memo to lean on
    let fresh = kt_gpu::test_engine().unwrap();
    let backends2 = ExportBackends { video_gpu: Some(fresh.clone() as Arc<dyn VideoGpuExec>), ..Default::default() };
    let mut piped = Hashes::default();
    render_pipelined(&p, sources(&p), sources(&p), q, Some(range), &backends2, &never, None, &mut piped).unwrap();
    assert!(serial.len() > 40);
    assert_eq!(piped.0, serial, "the pipelined GPU export writes the serial GPU export's frames");

    // the ring directly: three in flight, collected in order, each equal to execute_into
    assert!(fresh.readback_depth(q.width, q.height) >= 3);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig { max_audio_bytes: 1, max_video_bytes: 256 << 20 });
    let video = VideoRenderer::new(&p);
    let mut src = sources(&p);
    let ts: Vec<Tick> = (0..6).map(|i| Tick(range.0.0 + i * 3600 * 7)).collect();
    for (media, pts, dq) in ts.iter().flat_map(|t| video.layers_at(*t, q)) {
        let s = &mut src.iter_mut().find(|(m, _)| *m == media).unwrap().1;
        let f = s.read_video_frame(pts, Some(dq)).unwrap();
        cache.insert_video(kt_engine::source_id(media), Some(dq), kt_engine::video::video_cache_idx(pts), f.into());
    }
    let mut want = Vec::new();
    for t in &ts {
        let mut out = Vec::new();
        gpu.execute_into(&video.frame_plan(&cache, *t, q), &mut out).unwrap();
        want.push(out);
    }
    let mut tickets = std::collections::VecDeque::new();
    let mut got = Vec::new();
    for t in &ts {
        tickets.push_back(fresh.submit(&video.frame_plan(&cache, *t, q)).unwrap());
        if tickets.len() == 3 {
            let mut out = Vec::new();
            fresh.collect(tickets.pop_front().unwrap(), &mut out).unwrap();
            got.push(out);
        }
    }
    while let Some(tk) = tickets.pop_front() {
        let mut out = Vec::new();
        fresh.collect(tk, &mut out).unwrap();
        got.push(out);
    }
    assert_eq!(got.len(), want.len());
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        assert!(g == w, "ring frame {i} differs from execute_into");
    }
}

#[test]
fn t1573_several_engines_write_the_frames_of_one() {
    kt_plugins_builtin::install();
    let Some(one) = kt_gpu::test_engine() else { return };
    let p = kt_model::demo::demo_project(kt_model::demo::DemoSources::Synth);
    let q = Quality { width: 640, height: 360 };
    let range = (Tick(90_000 * 3), Tick(90_000 * 5));
    let never = Arc::new(AtomicBool::new(false));
    let single = ExportBackends { video_gpu: Some(one as Arc<dyn VideoGpuExec>), ..Default::default() };
    let mut want = Hashes::default();
    render_pipelined(&p, sources(&p), sources(&p), q, Some(range), &single, &never, None, &mut want).unwrap();
    let more: Vec<Arc<dyn VideoGpuExec>> =
        (0..2).map(|_| kt_gpu::test_engine().unwrap() as Arc<dyn VideoGpuExec>).collect();
    let several =
        ExportBackends { video_gpu: Some(kt_gpu::test_engine().unwrap() as Arc<dyn VideoGpuExec>), video_gpu_more: more, ..Default::default() };
    let mut got = Hashes::default();
    render_pipelined(&p, sources(&p), sources(&p), q, Some(range), &several, &never, None, &mut got).unwrap();
    assert!(want.0.len() > 40);
    assert_eq!(got.0, want.0, "the file does not depend on how many devices composed it");
}

/// t2901 (render-honest D1): the count of devices that compose may change as the render goes —
/// scripted here 1, 3, 2, 3, 1 every four frames — and the frames are the frames of one device; the
/// export's line names the counts used.
#[test]
fn t2901_a_changing_count_writes_the_frames_of_one_device() {
    kt_plugins_builtin::install();
    let Some(one) = kt_gpu::test_engine() else { return };
    let p = kt_model::demo::demo_project(kt_model::demo::DemoSources::Synth);
    let q = Quality { width: 320, height: 180 };
    let range = (Tick(90_000 * 3), Tick(90_000 * 5));
    let never = Arc::new(AtomicBool::new(false));
    let single = ExportBackends { video_gpu: Some(one as Arc<dyn VideoGpuExec>), ..Default::default() };
    let mut want = Hashes::default();
    render_pipelined(&p, sources(&p), sources(&p), q, Some(range), &single, &never, None, &mut want).unwrap();
    let more: Vec<Arc<dyn VideoGpuExec>> =
        (0..2).map(|_| kt_gpu::test_engine().unwrap() as Arc<dyn VideoGpuExec>).collect();
    let changing = ExportBackends {
        video_gpu: Some(kt_gpu::test_engine().unwrap() as Arc<dyn VideoGpuExec>),
        video_gpu_more: more,
        devices: kt_engine::export::DeviceCount::Script(vec![1, 3, 2, 3, 1]),
    };
    let prof = kt_engine::export::ExportProfile::default();
    let mut got = Hashes::default();
    render_pipelined(&p, sources(&p), sources(&p), q, Some(range), &changing, &never, Some(&prof), &mut got).unwrap();
    assert!(want.0.len() > 40);
    assert_eq!(got.0, want.0, "the file does not depend on a count that changes");
    let line = prof.devices_text();
    for k in ["1 ×", "2 ×", "3 ×"] {
        assert!(line.contains(k), "the line names the counts used: {line}");
    }
}

/// t2298 (memory-ledger D7): an export's device keeps only the uploads its frames in flight read: a
/// frame read again at once is not uploaded again, nothing older stays, and the frames are the bytes
/// of a preview device with its full upload cache.
#[test]
fn t2298_an_export_device_keeps_the_uploads_of_the_frames_in_flight() {
    use kt_cache::{BurstCache, CacheConfig};
    kt_plugins_builtin::install();
    let Some(preview) = kt_gpu::test_engine() else { return };
    let export = kt_gpu::GpuFxEngine::new_for_export(kt_gpu::GpuContext::new_headless().unwrap()).expect("engine");
    let p = kt_model::demo::demo_project(kt_model::demo::DemoSources::Synth);
    let q = Quality { width: 320, height: 180 };
    let ts: Vec<Tick> = (0..24).map(|i| Tick(90_000 * 3 + i * 3_000)).collect();
    let video = VideoRenderer::new(&p);
    let cache = BurstCache::new(CacheConfig { max_audio_bytes: 1, max_video_bytes: 512 << 20 });
    let mut srcs: std::collections::HashMap<_, _> = sources(&p).into_iter().collect();
    for (media, pts, dq) in ts.iter().flat_map(|t| video.layers_at(*t, q)) {
        if let Some(s) = srcs.get_mut(&media) {
            let f = s.read_video_frame(pts, Some(dq)).unwrap();
            cache.insert_video(kt_engine::source_id(media), Some(dq), kt_engine::video_cache_idx(pts), f.into());
        }
    }
    // an export's device keeps the uploads of its last three frames (export-readback D1: the ring
    // may be deeper, a frame deeper in it holds its textures by reference)
    let ring = 3;
    let most = ts.iter().map(|t| video.layers_at(*t, q).len()).max().unwrap();
    let (mut a, mut b) = (Vec::new(), Vec::new());
    let mut uploaded_total = 0usize;
    for t in &ts {
        let plan = video.frame_plan(&cache, *t, q);
        export.execute_once(&plan, &mut a).unwrap();
        preview.execute_once(&plan, &mut b).unwrap();
        assert_eq!(fnv(&a), fnv(&b), "the bytes at {t:?}");
        uploaded_total += video.layers_at(*t, q).len();
        assert!(
            export.uploads_held() <= (ring + 1) * most,
            "at {t:?}: {} uploads held, the frames in flight read at most {}",
            export.uploads_held(),
            (ring + 1) * most
        );
        // the same frame again at once: nothing is uploaded
        export.execute_once(&plan, &mut a).unwrap();
        assert_eq!(export.last_stages().upload_bytes, 0, "read again at once, not uploaded at {t:?}");
    }
    assert!(preview.uploads_held() > (ring + 1) * most, "scene guard: the preview's cache holds more ({} of {uploaded_total})", preview.uploads_held());
}

/// t2960 (export-readback D1): the ring's bound is its bytes over the frame — more than three 1080p
/// frames, never fewer than three, no deeper for a larger frame — and that many 1080p frames go in
/// before the first comes out: the first reports done without being collected, and each collected
/// frame is `execute_into`'s, byte for byte. The ring of three refused the fourth submission.
#[test]
fn t2960_the_ring_holds_its_bytes_worth_of_frames() {
    kt_plugins_builtin::install();
    let Some(gpu) = kt_gpu::test_engine() else { return };
    let (hd, uhd, eight) = (gpu.readback_depth(1920, 1080), gpu.readback_depth(3840, 2160), gpu.readback_depth(7680, 4320));
    assert!(hd > 3, "a 1080p ring holds {hd}");
    assert!(uhd <= hd && eight <= uhd, "a larger frame gets no deeper ring: {hd}, {uhd}, {eight}");
    assert!(eight >= 3 && uhd >= 3, "never fewer than three: {uhd}, {eight}");

    let p = kt_model::demo::demo_project(kt_model::demo::DemoSources::Synth);
    let q = Quality { width: 1920, height: 1080 };
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig { max_audio_bytes: 1, max_video_bytes: 1 << 30 });
    let video = VideoRenderer::new(&p);
    let mut src = sources(&p);
    let ts: Vec<Tick> = (0..hd as i64).map(|i| Tick(90_000 * 3 + i * 3000)).collect();
    for (media, pts, dq) in ts.iter().flat_map(|t| video.layers_at(*t, q)) {
        let s = &mut src.iter_mut().find(|(m, _)| *m == media).unwrap().1;
        let f = s.read_video_frame(pts, Some(dq)).unwrap();
        cache.insert_video(kt_engine::source_id(media), Some(dq), kt_engine::video::video_cache_idx(pts), f.into());
    }
    let want: Vec<Vec<u8>> = ts
        .iter()
        .map(|t| {
            let mut out = Vec::new();
            gpu.execute_into(&video.frame_plan(&cache, *t, q), &mut out).unwrap();
            out
        })
        .collect();
    let ring = kt_gpu::test_engine().unwrap();
    let tickets: Vec<u64> = ts.iter().map(|t| ring.submit(&video.frame_plan(&cache, *t, q)).expect("the ring takes its depth")).collect();
    let deadline = kt_time::test_deadline();
    while !ring.done(tickets[0]) {
        assert!(std::time::Instant::now() < deadline, "the first frame never reported done");
        std::thread::yield_now();
    }
    for (i, tk) in tickets.into_iter().enumerate() {
        let mut out = Vec::new();
        ring.collect(tk, &mut out).unwrap();
        assert!(out == want[i], "ring frame {i} of {hd} differs from execute_into");
    }
}
