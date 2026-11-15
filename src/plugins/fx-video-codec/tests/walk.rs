//! The history walk on the device (stream-history-on-device, capability `video-fx`): t2350 (the
//! batched walk is the per-position walk's bytes), t2351 (one wait a batch, only the kept pictures
//! back), t2352 (the vector edit on the device is the CPU's), t2353 (a re-walk is bounded by the
//! checkpoint stride), t2363 (a walk is its batches in the trace).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::Arc;

use fx_video_codec::codec::{self, Batch, Loss, Planes, Residual};
use fx_video_codec::mosh::{self, Rows};
use fx_video_codec::walk::{CHECKPOINT, MAX_BATCH};
use fx_video_codec::{rows, Datamosh};
use kt_plugin_api::compute::{Buf, ComputeHost, Held, Job, Shader};
use kt_plugin_api::generator::lowbias32;
use kt_plugin_api::stream::{mix, Rgba, Side, StreamInput, StreamKernel, StreamLayout, StreamMemo};
use kt_plugin_api::{ParamValues, Plugin};

const W: u32 = 64;
const H: u32 = 48;

static JOBS: AtomicU64 = AtomicU64::new(0);
static DOWN: AtomicU64 = AtomicU64::new(0);
/// Bytes uploaded, the clean encoder's residual dispatches and the source conversions.
static UP: AtomicU64 = AtomicU64::new(0);
static RESIDUALS: AtomicU64 = AtomicU64::new(0);
static CONVERTS: AtomicU64 = AtomicU64::new(0);

fn count(job: &Job<'_>) {
    JOBS.fetch_add(1, Relaxed);
    UP.fetch_add(job.buffers.iter().map(|b| if let kt_plugin_api::compute::Init::Bytes(v) = b { v.len() as u64 } else { 0 }).sum(), Relaxed);
    RESIDUALS.fetch_add(job.steps.iter().filter(|s| s.name.ends_with(":residual")).count() as u64, Relaxed);
    CONVERTS.fetch_add(job.steps.iter().filter(|s| s.name.ends_with(":convert")).count() as u64, Relaxed);
}
/// The counters are the process's: one test at a time.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Counting(Arc<dyn ComputeHost>);

impl ComputeHost for Counting {
    fn run(&self, job: &Job<'_>) -> Result<Vec<Vec<u8>>, String> {
        count(job);
        let got = self.0.run(job)?;
        DOWN.fetch_add(got.iter().map(|v| v.len() as u64).sum(), Relaxed);
        Ok(got)
    }
    fn keeps(&self) -> bool {
        self.0.keeps()
    }
    fn run_keeping(&self, job: &Job<'_>, keep: &[Buf]) -> Result<(Vec<Vec<u8>>, Vec<Held>), String> {
        count(job);
        let got = self.0.run_keeping(job, keep)?;
        DOWN.fetch_add(got.0.iter().map(|v| v.len() as u64).sum(), Relaxed);
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

/// Value noise translated by whole pixels (the codec tests' texture, smaller).
fn tex(seed: u32, dx: i32, dy: i32) -> Vec<u8> {
    let lat = |x: i32, y: i32, c: u32| -> i32 {
        (lowbias32(seed.wrapping_mul(0x9e37) ^ lowbias32((x as u32).wrapping_mul(73) ^ lowbias32((y as u32).wrapping_mul(151) ^ c))) % 200) as i32 + 28
    };
    let mut out = vec![255u8; (W * H * 4) as usize];
    for y in 0..H as i32 {
        for x in 0..W as i32 {
            let (sx, sy) = (x - dx + 1000, y - dy + 1000);
            let (gx, gy, fx, fy) = (sx / 8, sy / 8, sx % 8, sy % 8);
            for c in 0..3u32 {
                let a = lat(gx, gy, c) * (8 - fx) + lat(gx + 1, gy, c) * fx;
                let b = lat(gx, gy + 1, c) * (8 - fx) + lat(gx + 1, gy + 1, c) * fx;
                out[((y as u32 * W + x as u32) * 4 + c) as usize] = ((a * (8 - fy) + b * fy) / 64) as u8;
            }
        }
    }
    out
}

type Curve = Arc<dyn Fn(i64) -> Vec<f64> + Send + Sync>;

/// An event of the second input: its start, end and frames.
type Event = (i64, i64, HashMap<i64, Vec<u8>>);

static TAGS: AtomicU64 = AtomicU64::new(1);

/// A stream: main frames, an optional second input of events (their starts), curves, a view.
struct Mock {
    first: i64,
    cut: i64,
    end: i64,
    main: HashMap<i64, Vec<u8>>,
    second: Vec<Event>,
    tag: u64,
    curve: Option<Curve>,
    view: Option<i64>,
    /// A region of interest's coverage, `W`×`H`.
    region: Option<Vec<u8>>,
}

impl Mock {
    fn new(first: i64, cut: i64, end: i64, f: impl Fn(i64) -> Vec<u8>) -> Mock {
        Mock { first, cut, end, main: (first..end).map(|k| (k, f(k))).collect(), second: vec![], tag: TAGS.fetch_add(1, Relaxed), curve: None, view: None, region: None }
    }
    fn second_at(&self, k: i64) -> Option<&Event> {
        self.second.iter().find(|(s, e, _)| *s <= k && k < *e)
    }
}

impl StreamLayout for Mock {
    fn size(&self) -> (u32, u32) {
        (W, H)
    }
    fn first(&self) -> i64 {
        self.first
    }
    fn cut(&self) -> i64 {
        self.cut
    }
    fn end(&self) -> i64 {
        self.end
    }
    fn second_start(&self, k: i64) -> Option<i64> {
        self.second_at(k).map(|s| s.0)
    }
    fn ident(&self, side: Side, k: i64) -> u64 {
        let has = match side {
            Side::Main => self.main.contains_key(&k),
            Side::Second => self.second_at(k).is_some(),
        };
        if has { mix(mix(self.tag, side as u64), k as u64) } else { 0 }
    }
    fn rows_at(&self, k: i64) -> Option<Vec<f64>> {
        self.curve.as_ref().map(|c| c(k))
    }
    fn shown_from(&self) -> Option<i64> {
        self.view
    }
    fn region_ident(&self, _k: i64) -> u64 {
        self.region.as_ref().map_or(0, |_| 0x2704)
    }
}

impl StreamInput for Mock {
    fn with_region(&self, _k: i64, f: &mut dyn FnMut(&[u8])) -> bool {
        match &self.region {
            Some(c) => {
                f(c);
                true
            }
            None => false,
        }
    }
    fn with_frame(&self, side: Side, k: i64, f: &mut dyn FnMut(Rgba<'_>)) -> bool {
        let fr = match side {
            Side::Main => self.main.get(&k),
            Side::Second => self.second_at(k).and_then(|s| s.2.get(&k)),
        };
        match fr {
            Some(v) => {
                f(Rgba { w: W, h: H, rgba: v });
                true
            }
            None => false,
        }
    }
}

fn params(sets: &[(usize, f64)]) -> Vec<f64> {
    let d = Datamosh::new();
    let mut p = ParamValues::defaults(&d.manifest().passport).0;
    for &(i, v) in sets {
        p[i] = v;
    }
    p
}

fn walk(p: &[f64], s: &Mock, n: i64, memo: &StreamMemo) -> Arc<Planes> {
    mosh::render(Rows::from(p), s, n, memo).expect("all frames present").0
}

fn reference(p: &[f64], s: &Mock, n: i64, memo: &StreamMemo) -> Arc<Planes> {
    mosh::render_reference(Rows::from(p), s, n, memo).expect("all frames present").0
}

/// The streams of t2350: each a name, its rows and its stream.
fn cases() -> Vec<(&'static str, Vec<f64>, Mock)> {
    let moving = |seed: u32| move |k: i64| tex(seed, 2 * k as i32, (k % 3) as i32);
    let mut v = vec![];
    // a cut mosh starting in the event before
    v.push(("cut mosh", params(&[(rows::MOSH, 1.0), (rows::GOP, 8.0), (rows::BEFORE, 6.0)]), Mock::new(-6, 0, 80, moving(1))));
    // the gates by curves, per-frame scale, rotation, swap, loss and quantiser
    let base = params(&[(rows::GOP, 10.0), (rows::BLOCK, 1.0)]);
    let mut g = Mock::new(0, 0, 90, moving(2));
    let b2 = base.clone();
    g.curve = Some(Arc::new(move |k| {
        let mut r = b2.clone();
        r[rows::MOSH] = if (5..60).contains(&k) { 1.0 } else { 0.0 };
        r[rows::RESET] = if (30..33).contains(&k) { 1.0 } else { 0.0 };
        r[rows::BLOOM] = if (12..50).contains(&k) { 1.0 } else { 0.0 };
        r[rows::FREEZE] = if (62..70).contains(&k) { 1.0 } else { 0.0 };
        r[rows::MV_SCALE] = 1.0 + 0.1 * (k % 7) as f64 - 0.3;
        r[rows::MV_ROTATE] = (k * 13 % 180) as f64 - 90.0;
        r[rows::LOSS] = if k % 5 == 0 { 0.3 } else { 0.0 };
        r[rows::QP] = 20.0 + (k % 9) as f64;
        r[rows::RESIDUAL] = 0.5 + 0.05 * (k % 10) as f64;
        r
    }));
    v.push(("gates and curves", base, g));
    v.push(("swap, loss 1", params(&[(rows::MOSH, 1.0), (rows::MV_SWAP, 1.0), (rows::LOSS, 1.0), (rows::SEED, 7.0)]), Mock::new(0, 0, 40, moving(3))));
    // a second input of two events
    let mut t = Mock::new(0, 0, 60, |_| tex(6, 0, 0));
    t.second = vec![(0, 25, (0..25).map(|k| (k, tex(7, 0, 3 * k as i32))).collect()), (25, 60, (25..60).map(|k| (k, tex(8, k as i32, 0))).collect())];
    v.push(("second input", params(&[(rows::VECTORS_FROM, 1.0), (rows::MOSH, 1.0), (rows::GOP, 12.0), (rows::MV_ROTATE, 30.0)]), t));
    v
}

/// Test t2350: the batched walk on the device gives the per-position walk's pictures and keys, played
/// in order, cold at the end, from a memo that lost its history, and with a view that keeps less.
#[test]
fn t2350_the_batched_walk_is_the_per_position_walk() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if no_device() {
        return;
    }
    for (name, p, mut s) in cases() {
        let (a, b) = (StreamMemo::new(256 << 20), StreamMemo::new(256 << 20));
        let r = Rows::from(&p);
        for n in s.first..s.end {
            let (wp, wk) = mosh::render(r, &s, n, &a).unwrap();
            let (rp, rk) = mosh::render_reference(r, &s, n, &b).unwrap();
            assert_eq!(wk, rk, "{name}: key at {n}");
            assert!(*wp == *rp, "{name}: picture at {n}");
        }
        let last = s.end - 1;
        let cold = reference(&p, &s, last, &StreamMemo::new(256 << 20));
        assert!(*walk(&p, &s, last, &StreamMemo::new(256 << 20)) == *cold, "{name}: cold");
        // the LRU lost the history: the walk goes on from its checkpoints
        a.drop_unpinned();
        for n in [last, last - 9, s.first + 3] {
            assert!(*walk(&p, &s, n, &a) == *reference(&p, &s, n, &StreamMemo::new(256 << 20)), "{name}: from checkpoints at {n}");
        }
        // a view keeps fewer pictures and the same bytes
        s.view = Some(last - 5);
        let v = StreamMemo::new(256 << 20);
        assert!(*walk(&p, &s, last, &v) == *cold, "{name}: with a view");
    }
}

/// Test t2351: a 64-position cut mosh walked cold with a view at 48 submits one job a batch and
/// brings back only the pictures at and after the view and the checkpoints.
#[test]
fn t2351_a_walk_waits_once_a_batch_and_reads_back_what_is_kept() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if no_device() {
        return;
    }
    let p = params(&[(rows::MOSH, 1.0)]);
    let mut s = Mock::new(0, 0, 64, |k| tex(11, 2 * k as i32, 0));
    s.view = Some(48);
    let memo = StreamMemo::new(256 << 20);
    let r = Rows::from(&p);
    let j0 = JOBS.load(Relaxed);
    let got = walk(&p, &s, 63, &memo);
    let jobs = JOBS.load(Relaxed) - j0;
    assert!(jobs <= 64usize.div_ceil(MAX_BATCH) as u64 + 1, "{jobs} jobs for 64 positions");
    let held = |k: i64| memo.contains(mosh::key(r, &s, k));
    for k in 48..64 {
        assert!(held(k), "the shown position {k} did not come back");
    }
    for k in [0, 16, 32] {
        assert!(held(k), "the checkpoint {k} did not come back");
    }
    let unkept: Vec<i64> = (1..48).filter(|k| k % CHECKPOINT != 0 && held(*k)).collect();
    // the batch's end is the one resume point before the view
    assert!(unkept.len() <= 1, "pictures before the view came back: {unkept:?}");
    assert!(*got == *reference(&p, &s, 63, &StreamMemo::new(256 << 20)));
    eprintln!("t2351: {jobs} jobs, {unkept:?} resume points");
}

/// Test t2352: the vector edit and the residual drop on the device are the CPU's, for every vector the
/// search gives, swap, scales and rotations, and loss of 0, 0.05, 0.5 and 1 under several seeds.
#[test]
fn t2352_the_vector_edit_on_the_device_is_the_cpus() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if no_device() {
        return;
    }
    let r = codec::TABLE_RANGE;
    let span = 2 * r + 1;
    // one block per vector: 129 × 129 blocks of 8
    let (w, h, bs) = ((span * 8) as usize, (span * 8) as usize, 8usize);
    let src: Vec<(i16, i16)> = (0..span * span).map(|i| ((i % span - r) as i16, (i / span - r) as i16)).collect();
    let src_bytes: Vec<u8> = src.iter().flat_map(|v| [(v.0 as i32).to_le_bytes(), (v.1 as i32).to_le_bytes()].concat()).collect();
    let cases = [
        (1.0, 0.0, false, 0.0, 0u32),
        (1.0, 0.0, true, 0.0, 0),
        (-16.0, 0.0, false, 0.0, 0),
        (0.37, 0.0, false, 0.0, 0),
        (2.45, 33.0, false, 0.05, 1),
        (1.0, -180.0, true, 0.5, 2),
        (0.9, 90.0, false, 1.0, 3),
        (16.0, 179.0, true, 0.05, 9999),
    ];
    for (scale, rotate, swap, loss, seed) in cases {
        let rows = Rows::from(&params(&[(rows::MV_SCALE, scale), (rows::MV_ROTATE, rotate), (rows::MV_SWAP, swap as u8 as f64), (rows::LOSS, loss), (rows::SEED, seed as f64), (rows::SEARCH, 16.0)]));
        let k = 77i64;
        let mut b = Batch::new(w, h);
        let src_buf = b.upload(src_bytes.clone());
        let table = (swap || scale != 1.0 || rotate != 0.0).then(|| b.upload(codec::vector_table(|v| mosh::transform(&rows, v))));
        let l = Loss::of(rows.loss, rows.seed, k, rows.cfg.search);
        if table.is_none() && l.is_none() {
            continue;
        }
        let (mv, lost) = b.edit(src_buf, bs, table, l);
        let (rmv, rlost) = (b.read(mv), lost.map(|x| b.read(x)));
        let got = b.run().expect("the device runs the edit").reads;
        let dev: Vec<(i32, i32)> = got[rmv].chunks_exact(8).map(|c| (i32::from_le_bytes(c[..4].try_into().unwrap()), i32::from_le_bytes(c[4..].try_into().unwrap()))).collect();
        for (i, v) in src.iter().enumerate() {
            let mut want = mosh::transform(&rows, *v);
            let mut gone = false;
            if rows.loss > 0.0 {
                let hh = lowbias32(rows.seed ^ lowbias32((k as u32) ^ lowbias32(i as u32 ^ 0x6d6f_7368)));
                if (hh as f64) < rows.loss * 4_294_967_296.0 {
                    let a = lowbias32(hh ^ 0x5bd1_e995);
                    let rr = rows.cfg.search;
                    let sp = (2 * rr + 1) as u32;
                    want = (((a % sp) as i32 - rr) as i16, (((a >> 16) % sp) as i32 - rr) as i16);
                    gone = true;
                }
            }
            assert_eq!(dev[i], (want.0 as i32, want.1 as i32), "vector {v:?} at scale {scale} rotate {rotate} swap {swap} loss {loss}");
            if let Some(rl) = rlost {
                assert_eq!(u32::from_le_bytes(got[rl][i * 4..i * 4 + 4].try_into().unwrap()) != 0, gone, "lost mark {i}");
            }
        }
    }
    // the residual drop: a small plane, blocks of 16 and 8
    for bs in [16usize, 8] {
        let (w, h) = (64usize, 48usize);
        let res = Residual {
            y: (0..w * h).map(|i| (lowbias32(i as u32) % 1025) as i16 - 512).collect(),
            cb: (0..w * h / 4).map(|i| (lowbias32(i as u32 ^ 5) % 1025) as i16 - 512).collect(),
            cr: (0..w * h / 4).map(|i| (lowbias32(i as u32 ^ 9) % 1025) as i16 - 512).collect(),
        };
        let blocks = (w / bs) * (h / bs);
        let mut b = Batch::new(w, h);
        let pkt = codec::Packet { intra: false, mv: vec![(0, 0); blocks], res: res.clone() };
        let (mv, rd) = b.upload_packet(&pkt);
        let l = Loss::of(0.4, 3, 5, 8);
        let (_, lost) = b.edit(mv.unwrap(), bs, None, l);
        let lost = lost.unwrap();
        let out = b.drop_lost(rd, lost, bs);
        let (ro, rl) = (b.read3(out), b.read(lost));
        let got = b.run().expect("the device runs the drop").reads;
        let mut want = res.clone();
        for i in 0..blocks {
            if u32::from_le_bytes(got[rl][i * 4..i * 4 + 4].try_into().unwrap()) != 0 {
                codec::drop_block_residual(&mut want, w, bs, i);
            }
        }
        let un = |v: &Vec<u8>| v.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>();
        assert_eq!(un(&got[ro[0]]), want.y, "luma drop at block {bs}");
        assert_eq!(un(&got[ro[1]]), want.cb);
        assert_eq!(un(&got[ro[2]]), want.cr);
    }
}

/// Test t2353: after the LRU lost a 300-position walk, a frame near its end needs at most a checkpoint
/// stride of source positions, computes them in one job, and is the cold walk's picture.
#[test]
fn t2353_a_re_walk_is_bounded_by_the_checkpoint_stride() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if no_device() {
        return;
    }
    let p = params(&[(rows::MOSH, 1.0), (rows::GOP, 30.0)]);
    let s = Mock::new(0, 0, 300, |k| tex(13, k as i32, (k / 7) as i32));
    let memo = StreamMemo::new(256 << 20);
    let k = Datamosh::new();
    let mut out = Vec::new();
    k.render(&p, &s, 299, &memo, &mut out).unwrap();
    memo.drop_unpinned();
    let needs = k.needs(&p, &s, 297, &memo);
    assert!(!needs.is_empty() && needs.len() as i64 <= CHECKPOINT, "a re-walk needs {} positions: {needs:?}", needs.len());
    let j0 = JOBS.load(Relaxed);
    let again = walk(&p, &s, 297, &memo);
    assert_eq!(JOBS.load(Relaxed) - j0, 1, "the re-walk is one batch");
    assert!(*again == *reference(&p, &s, 297, &StreamMemo::new(256 << 20)));
}

/// Test t2363: a cold 40-position walk is `stream_walk` records whose positions sum to 40.
#[test]
fn t2363_a_walk_is_its_batches_in_the_trace() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if no_device() {
        return;
    }
    let p = params(&[(rows::MOSH, 1.0)]);
    let s = Mock::new(0, 0, 40, |k| tex(17, 2 * k as i32, 0));
    let memo = StreamMemo::new(256 << 20);
    let items = kt_time::trace::testing::capture(|| {
        walk(&p, &s, 39, &memo);
    });
    let recs: Vec<_> = kt_time::trace::testing::recs(&items).into_iter().filter(|r| r.kind() == Some(kt_time::trace::Ev::StreamWalk)).collect();
    assert_eq!(recs.len(), 40usize.div_ceil(MAX_BATCH), "one record a submission");
    assert_eq!(recs.iter().map(|r| r.b).sum::<u64>(), 40);
    assert_eq!(recs[0].a, 0);
}

/// The padded planes' bytes of one picture of the mock (8 bits a sample, chroma 4:2:0).
fn planes_bytes() -> u64 {
    let (w, h) = (W.div_ceil(16) * 16, H.div_ceil(16) * 16);
    (w * h * 3 / 2) as u64
}

/// Test t2700 (found-defects 401, 621) and t2706 (found-defects 135): played frame by frame from a
/// memo that holds two pictures, a cut mosh uploads nothing but its source frames, encodes each
/// position once, and reads back the frame shown and the checkpoints as 8-bit planes — the walk's
/// state stays on the device between batches and walks; the pictures are the per-position walk's.
#[test]
fn t2700_t2706_a_walk_goes_on_from_the_device() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if no_device() {
        return;
    }
    let p = params(&[(rows::MOSH, 1.0), (rows::GOP, 8.0)]);
    let s = Mock::new(0, 0, 80, |k| tex(21, 2 * k as i32, (k % 3) as i32));
    let memo = StreamMemo::new(2 * planes_bytes() as usize);
    let r = Rows::from(&p);
    let kept0 = kt_gpu::compute::kept_buffers();
    walk(&p, &s, 0, &memo);
    let (mut up, mut down, mut residuals) = (0, 0, 0);
    let reference_memo = StreamMemo::new(256 << 20);
    for n in 1..80 {
        let (u0, d0, r0) = (UP.load(Relaxed), DOWN.load(Relaxed), RESIDUALS.load(Relaxed));
        let got = walk(&p, &s, n, &memo);
        up += UP.load(Relaxed) - u0;
        down += DOWN.load(Relaxed) - d0;
        residuals += RESIDUALS.load(Relaxed) - r0;
        assert!(*got == *mosh::render_reference(r, &s, n, &reference_memo).unwrap().0, "picture at {n}");
    }
    let frames = 79u64;
    let rgba = (W * H * 4) as u64;
    assert!(up <= frames * rgba, "uploaded {up} bytes over {frames} frames: more than their source frames ({rgba} each)");
    assert_eq!(residuals, 3 * frames, "each position encoded once (three planes)");
    // the frame shown each time, and at each checkpoint its picture and the clean reconstruction
    let checkpoints = (1..80).filter(|k| k % CHECKPOINT == 0).count() as u64;
    assert!(down <= (frames + 2 * checkpoints) * planes_bytes(), "read back {down} bytes over {frames} frames");
    assert!(kt_gpu::compute::kept_buffers() > kept0, "nothing stayed on the device");
    assert!(memo.live_bytes() > 0, "the live tier holds the walk's state");
    eprintln!("t2700: up {up}, down {down}, residual dispatches {residuals}");
}

/// Test t2701: the shown frame's conversion row by row is the per-pixel conversion, at every crop.
#[test]
fn t2701_the_conversion_back_is_the_reference() {
    for (w, h) in [(64usize, 48usize), (16, 16), (1920 / 4, 1088 / 4)] {
        let mut x = 0x2701u32;
        let mut next = || {
            x = lowbias32(x);
            (x >> 24) as u8
        };
        let p = Planes { w, h, y: (0..w * h).map(|_| next()).collect(), cb: (0..w * h / 4).map(|_| next()).collect(), cr: (0..w * h / 4).map(|_| next()).collect() };
        for (ow, oh) in [(w as u32, h as u32), (w as u32 - 3, h as u32 - 5), (w as u32 + 7, h as u32), (1, 1)] {
            let (mut a, mut b) = (Vec::new(), Vec::new());
            p.to_rgba(ow, oh, &mut a);
            p.to_rgba_reference(ow, oh, &mut b);
            assert!(a == b, "{w}x{h} cropped to {ow}x{oh}");
        }
    }
}

/// Test t2703 (found-defects 402): a refresh's I-frame computed by a walk stays beside it: after the
/// LRU lost the history, a re-walk to the refresh encodes nothing.
#[test]
fn t2703_a_refresh_is_kept_beside_the_walk() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if no_device() {
        return;
    }
    // GOP 0: one I-frame at the cut, then a chain whose 600th node is the intra refresh
    let p = params(&[(rows::MOSH, 1.0), (rows::GOP, 0.0)]);
    let n = mosh::MAX_CHAIN as i64;
    let s = Mock::new(0, 0, n + 10, |k| tex(23, k as i32, (k / 5) as i32));
    let memo = StreamMemo::new(256 << 20);
    let want = walk(&p, &s, n + 9, &memo);
    let at = walk(&p, &s, n, &StreamMemo::new(256 << 20));
    memo.drop_unpinned();
    let c0 = CONVERTS.load(Relaxed);
    let again = walk(&p, &s, n, &memo);
    let converts = CONVERTS.load(Relaxed) - c0;
    // from the checkpoint before the refresh: the clean encoder's sources up to it, not the refresh's
    // the refresh is self-contained: its chain is its own picture, kept on the device
    assert_eq!(converts, 0, "the refresh was encoded again");
    assert!(*again == *at);
    assert!(*walk(&p, &s, n + 9, &memo) == *want);
}

/// Test t2704 (found-defects 405): a region of interest at a held position (a dropped I-frame) is one
/// job: the clean encoder's picture it blends with is encoded in the walk's batch.
#[test]
fn t2704_a_region_at_a_held_position_is_one_job() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if no_device() {
        return;
    }
    let p = params(&[(rows::MOSH, 1.0), (rows::GOP, 8.0)]);
    let mut s = Mock::new(0, 0, 40, |k| tex(24, 3 * k as i32, 0));
    s.region = Some((0..W * H).map(|i| if (i % W) < W / 2 { 255 } else { 0 }).collect());
    let memo = StreamMemo::new(256 << 20);
    let reference_memo = StreamMemo::new(256 << 20);
    let r = Rows::from(&p);
    for n in 0..16 {
        walk(&p, &s, n, &memo);
    }
    // 16 is a dropped I-frame: the decoder holds 15
    let j0 = JOBS.load(Relaxed);
    let got = walk(&p, &s, 16, &memo);
    assert_eq!(JOBS.load(Relaxed) - j0, 1, "the region's clean picture took a job of its own");
    assert!(*got == *mosh::render_reference(r, &s, 16, &reference_memo).unwrap().0);
}
