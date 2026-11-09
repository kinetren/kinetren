//! The codec's jobs move packed planes (stream-layout-per-build): t2273 (an encoder job reads back
//! the vectors, the residual at 16 bits a sample and the reconstruction at 8, and a decoder job reads
//! back its picture at 8 — found-defects 135: a 32-bit word a sample), t2274 (the source's colour
//! conversion on the device is the CPU reference to the byte, at sizes that pad), t2276 (a loop the
//! memo is sized for runs no job on its second lap), t2278 (an advance produces no picture).

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::Arc;

use fx_video_codec::codec::{self, Cfg, Planes};
use kt_plugin_api::compute::{ComputeHost, Init, Job, Shader};
use kt_plugin_api::generator::lowbias32;

static UP: AtomicU64 = AtomicU64::new(0);
static DOWN: AtomicU64 = AtomicU64::new(0);
static JOBS: AtomicU64 = AtomicU64::new(0);
static CONVERSIONS: AtomicU64 = AtomicU64::new(0);
/// The counters are the process's: one test at a time.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The test's device: the GPU's compute host, counting what each job moves.
struct Counting(Arc<dyn ComputeHost>);

impl ComputeHost for Counting {
    fn run(&self, job: &Job<'_>) -> Result<Vec<Vec<u8>>, String> {
        let up: usize = job.buffers.iter().map(|b| if let Init::Bytes(b) = b { b.len() } else { 0 }).sum();
        UP.fetch_add(up as u64, Relaxed);
        JOBS.fetch_add(1, Relaxed);
        if job.steps.iter().any(|s| s.name.ends_with(":convert")) {
            CONVERSIONS.fetch_add(1, Relaxed);
        }
        let got = self.0.run(job)?;
        DOWN.fetch_add(got.iter().map(|v| v.len() as u64).sum(), Relaxed);
        Ok(got)
    }
    fn prepare(&self, s: &Shader) -> Result<(), String> {
        self.0.prepare(s)
    }
}

fn open() -> Result<Arc<dyn ComputeHost>, String> {
    kt_gpu::compute::open().map(|h| Arc::new(Counting(h)) as Arc<dyn ComputeHost>)
}

fn no_device() -> bool {
    kt_plugin_api::compute::install(open);
    let none = kt_plugin_api::compute::host().is_none();
    if none {
        assert!(std::env::var("KT_REQUIRE_GPU").as_deref() != Ok("1"), "KT_REQUIRE_GPU=1, but no compute device");
        eprintln!("skipped: no compute device");
    }
    none
}

/// A noisy RGBA frame with smooth structure (the search finds vectors, the residual is not zero).
fn frame(w: u32, h: u32, seed: u32, dx: i32) -> Vec<u8> {
    let mut v = vec![255u8; (w * h * 4) as usize];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            for c in 0..3 {
                let s = ((x - dx) / 6 + (y / 5) * 7 + c * 31) as u32;
                let noise = lowbias32(seed ^ lowbias32(s ^ ((x as u32) << 12) ^ y as u32)) % 24;
                v[((y as u32 * w + x as u32) * 4 + c as u32) as usize] = ((lowbias32(s) % 200) + noise) as u8;
            }
        }
    }
    v
}

/// Both tests count bytes on one process-wide device: run them in one test.
#[test]
fn t2273_t2274_packed_jobs_and_the_conversion() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if no_device() {
        return;
    }
    // t2274: the conversion — a size whose padding replicates the last column and row
    for (sw, sh) in [(100u32, 60u32), (64, 48), (37, 21)] {
        let (w, h) = (codec::pad16(sw), codec::pad16(sh));
        let cfg = Cfg { bs: 8, search: 8, qp: 22.0 };
        let a = frame(sw, sh, 1, 0);
        let b = frame(sw, sh, 1, 3);
        let ia_cpu = codec::encode(&Planes::from_rgba(&a, sw, sh, w, h), None, &cfg).unwrap();
        let ia_gpu = codec::encode_rgba(&a, sw, sh, w, h, None, &cfg).unwrap();
        assert_eq!(ia_gpu.recon, ia_cpu.recon, "intra {sw}x{sh}");
        assert_eq!(ia_gpu.pkt, ia_cpu.pkt, "intra packet {sw}x{sh}");
        let pb_cpu = codec::encode(&Planes::from_rgba(&b, sw, sh, w, h), Some(&ia_cpu.recon), &cfg).unwrap();
        let pb_gpu = codec::encode_rgba(&b, sw, sh, w, h, Some(&ia_gpu.recon), &cfg).unwrap();
        assert_eq!(pb_gpu.recon, pb_cpu.recon, "inter {sw}x{sh}");
        assert_eq!(pb_gpu.pkt, pb_cpu.pkt, "inter packet {sw}x{sh}");
        assert!(pb_cpu.pkt.res.y.iter().any(|&r| r < 0) && pb_cpu.pkt.mv.iter().any(|&v| v != (0, 0)), "a residual with both signs and some motion");
    }
    // t2273: the bytes of one encoder job and one decoder job, 1280×720 (the owner's 720 lines)
    let (sw, sh) = (1280u32, 720u32);
    let (w, h) = (codec::pad16(sw), codec::pad16(sh));
    let cfg = Cfg { bs: 8, search: 16, qp: 24.0 };
    let a = codec::encode_rgba(&frame(sw, sh, 2, 0), sw, sh, w, h, None, &cfg).unwrap();
    let (u0, d0) = (UP.load(Relaxed), DOWN.load(Relaxed));
    let rgba = frame(sw, sh, 2, 2);
    let b = codec::encode_rgba(&rgba, sw, sh, w, h, Some(&a.recon), &cfg).unwrap();
    let (up, down) = (UP.load(Relaxed) - u0, DOWN.load(Relaxed) - d0);
    let luma = (w * h) as u64;
    let mv = b.pkt.mv.len() as u64 * 8;
    let planes = luma * 3 / 2;
    assert_eq!(down, mv + 2 * planes + planes, "encoder readback: vectors, 16-bit residual, 8-bit reconstruction");
    assert_eq!(up, rgba.len() as u64 + planes, "encoder upload: the frame's RGBA and the 8-bit reference");
    let (u0, d0) = (UP.load(Relaxed), DOWN.load(Relaxed));
    let _ = codec::decode(&a.recon, &b.pkt.mv, cfg.bs, &b.pkt.res, 200).unwrap();
    let (up, down) = (UP.load(Relaxed) - u0, DOWN.load(Relaxed) - d0);
    assert_eq!(down, planes, "decoder readback: the 8-bit picture");
    assert_eq!(up, planes + 2 * planes + mv, "decoder upload: reference, 16-bit residual, vectors");
    eprintln!("1280x720: encoder reads back {:.2} MB (was {:.2}), decoder {:.2} MB (was {:.2})", (mv + 3 * planes) as f64 / 1e6, (mv + 8 * planes) as f64 / 1e6, planes as f64 / 1e6, 4.0 * planes as f64 / 1e6);
}

/// A cut-mosh stream of noise frames moving right, for the loop of t2276.
struct Loop {
    frames: Vec<Vec<u8>>,
}

const LW: u32 = 64;
const LH: u32 = 48;

impl kt_plugin_api::stream::StreamLayout for Loop {
    fn size(&self) -> (u32, u32) {
        (LW, LH)
    }
    fn first(&self) -> i64 {
        0
    }
    fn cut(&self) -> i64 {
        0
    }
    fn end(&self) -> i64 {
        self.frames.len() as i64
    }
    fn second_start(&self, _k: i64) -> Option<i64> {
        None
    }
    fn ident(&self, _side: kt_plugin_api::stream::Side, k: i64) -> u64 {
        kt_plugin_api::stream::mix(0x2276, k as u64)
    }
}

impl kt_plugin_api::stream::StreamInput for Loop {
    fn with_frame(&self, _side: kt_plugin_api::stream::Side, k: i64, f: &mut dyn FnMut(kt_plugin_api::stream::Rgba<'_>)) -> bool {
        let Some(fr) = self.frames.get(k as usize) else { return false };
        f(kt_plugin_api::stream::Rgba { w: LW, h: LH, rgba: fr });
        true
    }
}

/// Two laps over `[10, 40)`: the jobs of the second lap and its pictures.
fn laps(memo: &kt_plugin_api::stream::StreamMemo, s: &Loop) -> (u64, Vec<Arc<Planes>>, Vec<Arc<Planes>>) {
    let mut p = fx_video_codec::defaults();
    p[fx_video_codec::rows::MOSH] = 1.0;
    p[fx_video_codec::rows::GOP] = 8.0;
    let r = fx_video_codec::mosh::Rows::from(&p);
    let lap = || (10..40).map(|n| fx_video_codec::mosh::render(r, s, n, memo).expect("frames present").0).collect::<Vec<_>>();
    let first = lap();
    let j0 = JOBS.load(Relaxed);
    let second = lap();
    (JOBS.load(Relaxed) - j0, first, second)
}

/// t2276: a memo sized by the loop rule keeps the loop: its second lap runs no encoder or decoder
/// job and shows the same pictures; half of it recomputes.
#[test]
fn t2276_a_loop_the_memo_holds_computes_nothing_twice() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if no_device() {
        return;
    }
    let s = Loop { frames: (0..40).map(|k| frame(LW, LH, 9, 2 * k)).collect() };
    // the player's rule (`stream_memo_bytes`, kt-engine): half a stage frame a position, every
    // position of the loop and an eighth, over a working set (here eight frames, not 256 MB)
    let frame = (LW * LH * 4) as usize;
    let n = 30usize;
    let sized = frame / 2 * (n + n / 8 + 2) + 8 * frame;
    let (jobs, a, b) = laps(&kt_plugin_api::stream::StreamMemo::new(sized), &s);
    assert_eq!(jobs, 0, "the second lap computed again");
    assert_eq!(a, b);
    let (jobs_half, c, _) = laps(&kt_plugin_api::stream::StreamMemo::new(sized / 2), &s);
    assert!(jobs_half > 10, "half the memo keeps the loop too ({jobs_half} jobs): the test is not about the bound");
    assert_eq!(a, c, "the bytes do not depend on the memo");
}

/// t2278: the prefetch's advance fills the memo with the history and produces no picture; the frame
/// then needs nothing, and its render (no job) gives the bytes of a cold render.
#[test]
fn t2278_an_advance_converts_no_picture() {
    use kt_plugin_api::stream::{StreamKernel, StreamMemo};
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if no_device() {
        return;
    }
    let s = Loop { frames: (0..40).map(|k| frame(LW, LH, 5, 3 * k)).collect() };
    let mut p = fx_video_codec::defaults();
    p[fx_video_codec::rows::MOSH] = 1.0;
    let k = fx_video_codec::Datamosh::new();
    let memo = StreamMemo::new(64 << 20);
    let r0 = CONVERSIONS.load(Relaxed);
    assert!(k.advance(&p, &s, 25, &memo));
    assert!(CONVERSIONS.load(Relaxed) > r0, "the advance's encoder jobs convert their sources");
    assert!(k.needs(&p, &s, 25, &memo).is_empty(), "the advance left the frame's history behind");
    let (mut warm, mut cold) = (Vec::new(), Vec::new());
    let j0 = JOBS.load(Relaxed);
    k.render(&p, &s, 25, &memo, &mut warm).unwrap();
    assert_eq!(JOBS.load(Relaxed) - j0, 0, "a held frame costs no job");
    k.render(&p, &s, 25, &StreamMemo::new(64 << 20), &mut cold).unwrap();
    assert_eq!(warm, cold);
}
