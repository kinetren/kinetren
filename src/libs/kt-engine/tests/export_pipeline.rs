//! The pipelined export (export-throughput, capability export): the stages of one file run
//! concurrently and the sink receives exactly the serial export's frames and samples.
//! - t1451: the demo project (layers, transitions, FX, automation, audio FX) at a size where
//!   the chunks are a few frames — the pipelined stream equals the serial passes frame for frame
//!   and sample for sample, and the call order into the sink is the same on every run;
//! - t1452: a cancel in the middle and a sink error in the middle stop every stage promptly.

use kt_engine::export::{
    ExportConverter, ExportSink, render_pipelined, render_raw_audio_range, };
use kt_media::{MediaSource, Quality};
use kt_model::{MediaId, Project};
use kt_time::Tick;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn project() -> Project {
    kt_plugins_builtin::install();
    kt_model::demo::demo_project(kt_model::demo::DemoSources::Synth)
}

fn sources(p: &Project) -> Vec<(MediaId, Box<dyn MediaSource>)> {
    p.media
        .iter()
        .map(|m| (m.id, kt_media::factory::open_pseudo(&m.path).expect("a pseudo path").expect("opens")))
        .collect()
}

/// 1280×720: with the demo's layers a chunk is a handful of frames, so 3 s are many chunks and
/// the look-ahead, the round robin and the writer's interleave all run.
const Q: Quality = Quality { width: 1280, height: 720 };

/// Three seconds over a transition with FX on both sides (the demo's first joint).
fn range() -> (Tick, Tick) {
    (Tick(90_000 * 3 + 1234), Tick(90_000 * 6))
}

/// Records what reaches the sink: a hash per video frame, the audio samples, the call order.
#[derive(Default)]
struct Recorder {
    frames: Vec<u64>,
    audio: Vec<u32>,
    calls: Vec<(char, usize)>,
    /// Cancel the export after this many frames are written.
    cancel_after: Option<(usize, Arc<AtomicBool>)>,
    /// Fail the write of this frame.
    fail_at: Option<usize>,
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

impl ExportSink for Recorder {
    type Frame = Vec<u8>;
    type Converter = CopyConv;
    fn converter(&self) -> std::io::Result<CopyConv> {
        Ok(CopyConv)
    }
    fn alloc_frame(&self) -> std::io::Result<Vec<u8>> {
        Ok(Vec::new())
    }
    fn write_video(&mut self, f: &Vec<u8>) -> std::io::Result<()> {
        if self.fail_at == Some(self.frames.len()) {
            return Err(std::io::Error::other("the disk is full"));
        }
        self.frames.push(fnv(f));
        self.calls.push(('v', f.len()));
        if let Some((n, c)) = &self.cancel_after
            && self.frames.len() == *n
        {
            c.store(true, Ordering::Release);
        }
        Ok(())
    }
    fn write_audio(&mut self, s: &[f32]) -> std::io::Result<()> {
        self.audio.extend(s.iter().map(|v| v.to_bits()));
        self.calls.push(('a', s.len()));
        Ok(())
    }
}

fn pipelined(p: &Project, rec: &mut Recorder, cancel: &Arc<AtomicBool>) -> std::io::Result<kt_engine::ExportStats> {
    render_pipelined(p, sources(p), sources(p), Q, Some(range()), &kt_gpu::testkit::backends(), cancel, None, rec)
}

/// t1451: pipelined == serial, frame for frame and sample for sample; the interleave is fixed.
#[test]
fn t1451_the_pipelined_export_writes_the_serial_streams() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let p = project();
    let never = Arc::new(AtomicBool::new(false));
    let mut serial_frames = Vec::new();
    let vs = kt_gpu::testkit::render_range(&p, sources(&p), Q, Some(range()), &never, |_| {}, |f| {
        serial_frames.push(fnv(f));
        Ok(())
    })
    .unwrap();
    let mut serial_audio = Vec::new();
    let as_ = render_raw_audio_range(&p, sources(&p), Some(range()), &never, |_| {}, |s| {
        serial_audio.extend(s.iter().map(|v| v.to_bits()));
        Ok(())
    })
    .unwrap();
    assert!(serial_frames.len() > 60, "the range renders frames: {}", serial_frames.len());
    assert!(serial_frames.windows(2).any(|w| w[0] != w[1]), "the scene moves");

    let mut a = Recorder::default();
    let st = pipelined(&p, &mut a, &never).unwrap();
    assert_eq!(a.frames.len(), serial_frames.len(), "the frame count");
    for (i, (x, y)) in a.frames.iter().zip(&serial_frames).enumerate() {
        assert_eq!(x, y, "video frame {i} differs from the serial export");
    }
    assert_eq!(a.audio, serial_audio, "the audio stream differs from the serial pass");
    assert_eq!(st.video_frames, vs.video_frames);
    assert_eq!(st.missing_video_layers, vs.missing_video_layers);
    assert_eq!(st.audio_frames, as_.audio_frames);
    assert_eq!(format!("{:?}", st.loudness), format!("{:?}", as_.loudness));

    // the sink hears the same calls in the same order on every run (the file does not depend on
    // which stage was faster)
    let mut b = Recorder::default();
    pipelined(&p, &mut b, &never).unwrap();
    assert_eq!(a.calls, b.calls, "the interleave of video and audio writes");
    assert!(a.calls.iter().skip(1).any(|c| c.0 == 'a') && a.calls.last().map(|c| c.0) == Some('a'));
    let first_audio = a.calls.iter().position(|c| c.0 == 'a').unwrap();
    assert!(first_audio <= 2, "the audio is interleaved from the first frame, not after the video");
}

/// t1452: a cancel mid-export and a sink error mid-export end it promptly, every stage stopped
/// (the call returns: all its threads are joined), the error is the cause.
#[test]
fn t1452_a_cancel_or_an_error_mid_pipeline_stops_every_stage() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let p = project();
    let cancel = Arc::new(AtomicBool::new(false));
    let mut rec = Recorder { cancel_after: Some((10, cancel.clone())), ..Default::default() };
    let t0 = std::time::Instant::now();
    let err = pipelined(&p, &mut rec, &cancel).expect_err("a cancelled export is an error");
    assert_eq!(err.to_string(), "cancelled");
    assert!(rec.frames.len() <= 11, "no frame after the cancel is written: {}", rec.frames.len());
    assert!(t0.elapsed() < std::time::Duration::from_secs(60), "the export stopped promptly");

    let never = Arc::new(AtomicBool::new(false));
    let mut rec = Recorder { fail_at: Some(5), ..Default::default() };
    let err = pipelined(&p, &mut rec, &never).expect_err("the sink's error ends the export");
    assert_eq!(err.to_string(), "the disk is full", "the cause, not a stage's \"the writer stopped\"");
    assert_eq!(rec.frames.len(), 5);
}
