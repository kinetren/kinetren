//! t1457 (export-throughput D5), t1573 (compute-kernels-render D4), capability export: the GPU export
//! with its readback ring and on several devices.
//! `submit`/`collect` give the bytes `execute_into` gives, frame for frame, and the pipelined
//! export on the GPU writes the serial GPU export's frames (one device: byte for byte,
//! invariant 11). Skipped without an adapter (`KT_REQUIRE_GPU=1` makes it a failure).

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
    assert!(fresh.readback_depth() >= 2);
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
        if tickets.len() == fresh.readback_depth() {
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
        ExportBackends { video_gpu: Some(kt_gpu::test_engine().unwrap() as Arc<dyn VideoGpuExec>), video_gpu_more: more };
    let mut got = Hashes::default();
    render_pipelined(&p, sources(&p), sources(&p), q, Some(range), &several, &never, None, &mut got).unwrap();
    assert!(want.0.len() > 40);
    assert_eq!(got.0, want.0, "the file does not depend on how many devices composed it");
}
