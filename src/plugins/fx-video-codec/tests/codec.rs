//! The codec model of `datamosh` (datamosh-codec-path): t1235–t1239, t1243; t1572 (compute-kernels-render).
//!
//! The stream is synthetic: a value-noise texture (a lattice of hashed values every 8 px, bilinear)
//! translated by whole pixels, so motion estimation has an exact answer and every expectation is a
//! statement about the codec, not about the footage.

use std::collections::HashMap;
use std::sync::Arc;

use fx_video_codec::codec::{self, Cfg, Planes};
use fx_video_codec::mosh::{self, Rows};
use fx_video_codec::{rows, Datamosh};
use kt_plugin_api::generator::lowbias32;
use kt_plugin_api::stream::{mix, Rgba, Side, StreamInput, StreamKernel, StreamLayout, StreamMemo};
use kt_plugin_api::{ParamValues, Plugin};

const W: u32 = 128;
const H: u32 = 96;

/// Value noise: hashed lattice every 8 px, bilinear between; `dx`/`dy` translate it.
/// The encoder needs the host's compute device: skip without one, as the GPU tests do.
fn no_device() -> bool {
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    let none = kt_plugin_api::compute::host().is_none();
    if none {
        assert!(std::env::var("KT_REQUIRE_GPU").as_deref() != Ok("1"), "KT_REQUIRE_GPU=1, but no compute device");
        eprintln!("skipped: no compute device");
    }
    none
}

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

/// A stream for tests: main frames by position, an optional second input.
struct Mock {
    first: i64,
    cut: i64,
    end: i64,
    main: HashMap<i64, Vec<u8>>,
    second: Option<(i64, HashMap<i64, Vec<u8>>)>,
    /// The stream's identity in the addresses: two mocks never share a memo key.
    tag: u64,
    /// The rows per position (curves), over the static ones; `None` — no curves.
    curve: Option<Curve>,
    /// A region of interest: its identity and its coverage plane (W×H).
    region: Option<(u64, Vec<u8>)>,
}

type Curve = std::sync::Arc<dyn Fn(i64) -> Vec<f64> + Send + Sync>;

static TAGS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Mock {
    fn new(first: i64, cut: i64, end: i64, f: impl Fn(i64) -> Vec<u8>) -> Mock {
        Mock { first, cut, end, main: (first..end).map(|k| (k, f(k))).collect(), second: None, tag: TAGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed), curve: None, region: None }
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
        self.second.as_ref().filter(|(_, m)| m.contains_key(&k)).map(|(s, _)| *s)
    }
    fn ident(&self, side: Side, k: i64) -> u64 {
        let has = match side {
            Side::Main => self.main.contains_key(&k),
            Side::Second => self.second.as_ref().is_some_and(|(_, m)| m.contains_key(&k)),
        };
        if has { mix(mix(self.tag, side as u64), k as u64) } else { 0 }
    }
    fn rows_at(&self, k: i64) -> Option<Vec<f64>> {
        self.curve.as_ref().map(|c| c(k))
    }
    fn region_ident(&self, _k: i64) -> u64 {
        self.region.as_ref().map_or(0, |r| r.0)
    }
}

impl StreamInput for Mock {
    fn with_frame(&self, side: Side, k: i64, f: &mut dyn FnMut(Rgba<'_>)) -> bool {
        let m = match side {
            Side::Main => Some(&self.main),
            Side::Second => self.second.as_ref().map(|(_, m)| m),
        };
        match m.and_then(|m| m.get(&k)) {
            Some(v) => {
                f(Rgba { w: W, h: H, rgba: v });
                true
            }
            None => false,
        }
    }
    fn with_region(&self, _k: i64, f: &mut dyn FnMut(&[u8])) -> bool {
        match &self.region {
            Some((_, c)) => {
                f(c);
                true
            }
            None => false,
        }
    }
}

/// Rows per position: the static `base` with `gate` set to 1 where `on(k)`.
fn curve(base: Vec<f64>, gate: usize, on: impl Fn(i64) -> bool + Send + Sync + 'static) -> Curve {
    std::sync::Arc::new(move |k| {
        let mut v = base.clone();
        v[gate] = if on(k) { 1.0 } else { 0.0 };
        v
    })
}

fn params(sets: &[(usize, f64)]) -> Vec<f64> {
    let d = Datamosh::new();
    let mut p = ParamValues::defaults(&d.manifest().passport).0;
    for &(i, v) in sets {
        p[i] = v;
    }
    p
}

fn planes(rgba: &[u8]) -> Planes {
    Planes::from_rgba(rgba, W, H, W as usize, H as usize)
}

fn dec(p: &[f64], s: &Mock, n: i64, memo: &StreamMemo) -> Arc<Planes> {
    mosh::render(Rows::from(p), s, n, memo).expect("all frames present").0
}

/// Mean and max absolute luma error inside the window [x0, x1) (whole rows).
fn err(a: &Planes, b: &Planes, x0: usize, x1: usize) -> (f64, i32) {
    let (mut sum, mut max, mut n) = (0i64, 0i32, 0i64);
    for y in 0..H as usize {
        for x in x0..x1 {
            let d = (a.y[y * a.w + x] as i32 - b.y[y * b.w + x] as i32).abs();
            sum += d as i64;
            max = max.max(d);
            n += 1;
        }
    }
    (sum as f64 / n as f64, max)
}

/// t1235: with no breakage the codec reproduces its input within the quantiser, and the closed
/// loop keeps the error from growing along the GOP.
#[test]
fn t1235_intact_stream_round_trips_within_the_quantiser() {
    if no_device() {
        return;
    }
    let s = Mock::new(0, 0, 24, |k| tex(5, k as i32, 0));
    let mut means = vec![];
    for qp in [10.0, 30.0] {
        let p = params(&[(rows::QP, qp)]);
        let q = Cfg { bs: 16, search: 16, qp }.qstep();
        let memo = StreamMemo::new(64 << 20);
        let mut per = vec![];
        for k in 0..24 {
            let d = dec(&p, &s, k, &memo);
            let src = planes(&s.main[&k]);
            let (m, mx) = err(&d, &src, 0, W as usize);
            assert!(m <= 0.5 * q + 1.0, "qp {qp} frame {k}: mean {m} above the quantiser's bound");
            assert!((mx as f64) <= 4.0 * q + 4.0, "qp {qp} frame {k}: max {mx}");
            // the chroma survives too (the planes are compared after the same 4:2:0)
            let ce: f64 = d.cb.iter().zip(&src.cb).map(|(a, b)| (*a as i32 - *b as i32).abs() as f64).sum::<f64>()
                / d.cb.len() as f64;
            assert!(ce <= 0.5 * q + 1.0, "qp {qp} frame {k}: chroma mean {ce}");
            per.push(m);
        }
        let head: f64 = per[1..5].iter().sum::<f64>() / 4.0;
        let tail: f64 = per[19..23].iter().sum::<f64>() / 4.0;
        assert!(tail <= 2.0 * head + 0.5, "qp {qp}: the error drifts along the GOP ({head} → {tail})");
        means.push(per.iter().sum::<f64>() / per.len() as f64);
    }
    assert!(means[1] > means[0], "a coarser quantiser loses more: {means:?}");
}

/// The classic cut: 12 frames of a still texture A, then a texture B moving right 2 px/frame.
fn cut_stream() -> Mock {
    Mock::new(-12, 0, 40, |k| if k < 0 { tex(1, 0, 0) } else { tex(2, 2 * k as i32, 0) })
}

/// t1236: dropping the I-frame at the cut keeps A's texture, moved by B's vectors.
#[test]
fn t1236_dropped_i_frame_moves_the_previous_shot_with_the_new_motion() {
    if no_device() {
        return;
    }
    let s = cut_stream();
    let moshed = params(&[(rows::BEFORE, 12.0), (rows::MOSH, 1.0), (rows::QP, 12.0)]);
    let intact = params(&[(rows::BEFORE, 12.0), (rows::QP, 12.0)]);
    let memo = StreamMemo::new(64 << 20);
    for n in [3i64, 6, 9] {
        let a_moved = planes(&tex(1, 2 * n as i32, 0));
        let b = planes(&s.main[&n]);
        // B's texture enters from the left edge: compare to the right of what entered
        let x0 = (2 * n as usize + 16).min(W as usize - 16);
        let m = dec(&moshed, &s, n, &memo);
        let (to_a, _) = err(&m, &a_moved, x0, W as usize);
        let (to_b, _) = err(&m, &b, x0, W as usize);
        assert!(to_a < 4.0 && to_b > 3.0 * to_a + 10.0, "frame {n}: moshed is A moved ({to_a}) not B ({to_b})");
        let i = dec(&intact, &s, n, &memo);
        let (i_b, _) = err(&i, &b, 0, W as usize);
        assert!(i_b < 4.0, "frame {n}: the intact stream shows B (err {i_b})");
    }
    // at the cut itself the dropped I-frame shows the reference again
    let at_cut = dec(&moshed, &s, 0, &memo);
    let before = dec(&moshed, &s, -1, &memo);
    assert_eq!(*at_cut, *before);
}

/// t1237: a bloom applies packet b's vector field again on every frame of (b, b+N].
#[test]
fn t1237_bloom_repeats_one_vector_field() {
    if no_device() {
        return;
    }
    let mut s = Mock::new(0, 0, 30, |k| tex(3, 2 * k as i32, (k / 3) as i32));
    let (b, len) = (6i64, 5i64);
    // the bloom gate is on over [b, b+len]: frame b decodes its own packet, then repeats it
    let p = params(&[]);
    s.curve = Some(curve(p.clone(), rows::BLOOM, move |k| (b..=b + len).contains(&k)));
    let cfg = Rows::from(&p).cfg;
    // the clean encoder, by hand
    let mut cl: Vec<codec::Clean> = vec![];
    for k in 0..=(b + len + 1) {
        let src = planes(&s.main[&k]);
        let c = codec::encode(&src, cl.last().map(|c| &c.recon), &cfg).unwrap();
        cl.push(c);
    }
    assert!(!cl[b as usize].pkt.intra && cl[b as usize].pkt.mv.iter().any(|v| *v != (0, 0)));
    let memo = StreamMemo::new(64 << 20);
    for j in (b + 1)..=(b + len) {
        let prev = dec(&p, &s, j - 1, &memo);
        let pk = &cl[b as usize].pkt;
        let want = codec::decode(&prev, &pk.mv, cfg.bs, &pk.res, 256).unwrap();
        assert_eq!(*dec(&p, &s, j, &memo), want, "frame {j} is packet {b} applied again");
    }
    // after the bloom the stream continues with its own packet
    let j = b + len + 1;
    let pk = &cl[j as usize].pkt;
    let want = codec::decode(&dec(&p, &s, j - 1, &memo), &pk.mv, cfg.bs, &pk.res, 256).unwrap();
    assert_eq!(*dec(&p, &s, j, &memo), want);
    let plain = dec(&params(&[]), &Mock::new(0, 0, 30, |k| tex(3, 2 * k as i32, (k / 3) as i32)), b + len, &memo);
    assert_ne!(*plain, *dec(&p, &s, b + len, &memo), "the bloom changes the picture");
}

fn rgba(p: &[f64], s: &Mock, n: i64, memo: &StreamMemo) -> Vec<u8> {
    let mut out = vec![];
    Datamosh::new().render(p, s, n, memo, &mut out).expect("rendered");
    out
}

/// t1238: frame N is the same bytes however it is reached; the needs answer what the memo lacks.
#[test]
fn t1238_seek_equals_play() {
    if no_device() {
        return;
    }
    let cases: Vec<(Vec<f64>, Option<Curve>)> = vec![
        (params(&[(rows::BEFORE, 12.0), (rows::MOSH, 1.0), (rows::GOP, 8.0)]), None),
        {
            let p = params(&[(rows::BEFORE, 12.0), (rows::MOSH, 1.0)]);
            let c = curve(p.clone(), rows::BLOOM, |k| (4..10).contains(&k));
            (p, Some(c))
        },
        {
            let p = params(&[(rows::LOSS, 0.2), (rows::SEED, 7.0)]);
            let c = curve(p.clone(), rows::FREEZE, |k| (3..15).contains(&k));
            (p, Some(c))
        },
        {
            // the person's beat: mosh on, a reset key at 9, mosh off after 16
            let p = params(&[(rows::BEFORE, 12.0), (rows::GOP, 6.0)]);
            let base = p.clone();
            let c: Curve = std::sync::Arc::new(move |k| {
                let mut v = base.clone();
                v[rows::MOSH] = if k < 16 { 1.0 } else { 0.0 };
                v[rows::RESET] = if k >= 9 { 1.0 } else { 0.0 };
                v
            });
            (p, Some(c))
        },
    ];
    for (p, c) in cases {
        let mut s = cut_stream();
        s.curve = c;
        let n = 21;
        let played = StreamMemo::new(256 << 20);
        let mut last = vec![];
        for k in s.first..=n {
            last = rgba(&p, &s, k, &played);
        }
        let cold = StreamMemo::new(256 << 20);
        assert_eq!(rgba(&p, &s, n, &cold), last, "seek to {n} with an empty memo");
        let tiny = StreamMemo::new(1);
        assert_eq!(rgba(&p, &s, n, &tiny), last, "a memo that holds nothing");
        let lay: &dyn StreamLayout = &s;
        let r = Rows::from(&p);
        assert!(mosh::needs(r, lay, n, &played).is_empty(), "a played frame needs nothing");
        assert_eq!(mosh::needs(r, lay, n + 1, &played), vec![(Side::Main, n + 1)], "the next frame needs its own source");
        let fresh = StreamMemo::new(256 << 20);
        let cold_needs = mosh::needs(r, lay, n, &fresh);
        assert!(cold_needs.len() > 1 && cold_needs.contains(&(Side::Main, n)));
    }
}

/// t1239: two threads with their own memos give the same bytes, pinned (invariant 11: Windows and
/// Linux must agree — the frozen DCT basis and integer planes make it so).
#[test]
fn t1239_determinism() {
    if no_device() {
        return;
    }
    let p = params(&[(rows::BEFORE, 12.0), (rows::MOSH, 1.0), (rows::MV_ROTATE, 30.0), (rows::LOSS, 0.1)]);
    let run = move || {
        let s = cut_stream();
        let memo = StreamMemo::new(256 << 20);
        (0..16).map(|k| rgba(&p, &s, k, &memo)).collect::<Vec<_>>()
    };
    let a = std::thread::spawn(run.clone());
    let b = std::thread::spawn(run);
    let (a, b) = (a.join().unwrap(), b.join().unwrap());
    assert_eq!(a, b);
    let h = a.iter().flatten().fold(0xcbf2_9ce4_8422_2325u64, |h, &v| (h ^ v as u64).wrapping_mul(0x100_0000_01b3));
    assert_eq!(h, PINNED, "the bytes moved: {h:#x}");
}

const PINNED: u64 = 0x9b02_633b_8b23_0698;

/// t1243: the other breakages change exactly the stream they name.
#[test]
fn t1243_freeze_transplant_vector_ops_and_loss() {
    if no_device() {
        return;
    }
    let memo = StreamMemo::new(256 << 20);
    // freeze: every P-frame in [5, 15) predicts from the picture at 4
    let s = Mock::new(0, 0, 30, |k| tex(4, 2 * k as i32, 0));
    let mut sf = Mock::new(0, 0, 30, |k| tex(4, 2 * k as i32, 0));
    let p = params(&[]);
    // the freeze gate on over [4, 15): frame 4 decodes, 5 … 14 predict from it
    sf.curve = Some(curve(p.clone(), rows::FREEZE, |k| (4..15).contains(&k)));
    let cfg = Rows::from(&p).cfg;
    let mut cl: Vec<codec::Clean> = vec![];
    for k in 0..=12 {
        let c = codec::encode(&planes(&sf.main[&k]), cl.last().map(|c| &c.recon), &cfg).unwrap();
        cl.push(c);
    }
    let rf = dec(&p, &sf, 4, &memo);
    for j in [5i64, 9, 12] {
        let pk = &cl[j as usize].pkt;
        let want = codec::decode(&rf, &pk.mv, cfg.bs, &pk.res, 256).unwrap();
        assert_eq!(*dec(&p, &sf, j, &memo), want, "frozen frame {j}");
    }
    // after the gate the reference updates again
    assert_ne!(*dec(&p, &sf, 16, &memo), *dec(&p, &s, 16, &memo), "the frozen run left its trace");
    // scale 0 with no residual holds the picture
    let hold = params(&[(rows::MV_SCALE, 0.0), (rows::RESIDUAL, 0.0)]);
    assert_eq!(*dec(&hold, &s, 7, &memo), *dec(&hold, &s, 6, &memo));
    // swap turns a horizontal motion into a vertical one
    let sw = params(&[(rows::MV_SWAP, 1.0), (rows::RESIDUAL, 0.0)]);
    let (a, b) = (dec(&sw, &s, 3, &memo), dec(&sw, &s, 4, &memo));
    let w = a.w;
    let down: f64 = (2..H as usize - 16).flat_map(|y| (16..W as usize - 16).map(move |x| (x, y)))
        .map(|(x, y)| (b.y[y * w + x] as i32 - a.y[(y - 2) * w + x] as i32).abs() as f64).sum();
    let right: f64 = (2..H as usize - 16).flat_map(|y| (16..W as usize - 16).map(move |x| (x, y)))
        .map(|(x, y)| (b.y[y * w + x] as i32 - a.y[y * w + x - 2] as i32).abs() as f64).sum();
    assert!(down < right, "swapped vectors move the picture down ({down} vs {right})");
    // transplant: an own still texture moved by the second input's motion (down 3 px/frame)
    let mut t = Mock::new(0, 0, 20, |_| tex(6, 0, 0));
    t.second = Some((0, (0..20).map(|k| (k, tex(7, 0, 3 * k as i32))).collect()));
    let tp = params(&[(rows::VECTORS_FROM, 1.0), (rows::RESIDUAL, 0.0)]);
    let n = 5;
    let got = dec(&tp, &t, n, &memo);
    let want = planes(&tex(6, 0, 3 * n as i32));
    let w = got.w;
    let inner: f64 = (3 * n as usize + 16..H as usize).flat_map(|y| (0..W as usize).map(move |x| (x, y)))
        .map(|(x, y)| (got.y[y * w + x] as i32 - want.y[y * w + x] as i32).abs() as f64).sum::<f64>()
        / ((H as usize - 3 * n as usize - 16) * W as usize) as f64;
    assert!(inner < 4.0, "the own texture moves with the second input's vectors (err {inner})");
    // loss: 0 is the clean stream, 1 is not, a seed is a seed
    let clean = dec(&params(&[]), &s, 8, &memo);
    assert_eq!(*clean, cl[8].recon);
    let l1 = dec(&params(&[(rows::LOSS, 1.0), (rows::SEED, 1.0)]), &s, 8, &memo);
    let l1b = dec(&params(&[(rows::LOSS, 1.0), (rows::SEED, 1.0)]), &s, 8, &StreamMemo::new(64 << 20));
    let l2 = dec(&params(&[(rows::LOSS, 1.0), (rows::SEED, 2.0)]), &s, 8, &memo);
    assert_ne!(*l1, *clean);
    assert_eq!(*l1, *l1b);
    assert_ne!(*l1, *l2);
}


/// t1244 (the owner, 2026-11-01: "aim the effect at regions of interest — faces, whole people"):
/// the REGION input. Inside the region the broken stream shows, outside it the intact one, byte
/// for byte; no region is the whole frame.
#[test]
fn t1244_a_region_of_interest_confines_the_mosh() {
    if no_device() {
        return;
    }
    let memo = StreamMemo::new(256 << 20);
    let moshed = params(&[(rows::BEFORE, 12.0), (rows::MOSH, 1.0), (rows::QP, 12.0)]);
    let intact = params(&[(rows::BEFORE, 12.0), (rows::QP, 12.0)]);
    let mut s = cut_stream();
    let half = W as usize / 2;
    let cov: Vec<u8> = (0..(W * H) as usize).map(|i| if i % W as usize >= half { 0 } else { 255 }).collect();
    s.region = Some((7, cov));
    let plain = cut_stream();
    let n = 6;
    let got = dec(&moshed, &s, n, &memo);
    let whole = dec(&moshed, &plain, n, &memo);
    let clean = dec(&intact, &plain, n, &memo);
    let w = got.w;
    for y in 0..H as usize {
        for x in 0..W as usize {
            let (g, want) = (got.y[y * w + x], if x < half { whole.y[y * w + x] } else { clean.y[y * w + x] });
            assert_eq!(g, want, "({x},{y}): inside the region the mosh, outside the intact stream");
        }
    }
    assert_ne!(*whole, *clean, "the scene moshes at all");
    // the region's identity is in the key: the same frame without it is another picture
    let mut out_a = vec![];
    let mut out_b = vec![];
    let a = Datamosh::new().render(&moshed, &s, n, &memo, &mut out_a).unwrap();
    let b = Datamosh::new().render(&moshed, &plain, n, &memo, &mut out_b).unwrap();
    assert_ne!(a.key, b.key);
}

/// t1245 (the owner, 2026-11-01: "reset/inactivity marks by hand on the beat"): the gates are
/// curves. A `reset` key rising to on forces an intact I-frame at that frame — the mosh ends on the
/// beat; `mosh` off keeps the next keyframe intact (inactivity).
#[test]
fn t1245_reset_and_inactivity_follow_the_curves() {
    if no_device() {
        return;
    }
    let memo = StreamMemo::new(256 << 20);
    let base = params(&[(rows::BEFORE, 12.0), (rows::QP, 12.0), (rows::GOP, 0.0)]);
    let b = |n: i64| planes(&tex(2, 2 * n as i32, 0));
    let a_moved = |n: i64| planes(&tex(1, 2 * n as i32, 0));
    let err_to = |p: &Planes, q: &Planes| err(p, q, (2 * 12 + 16).min(W as usize - 16), W as usize).0;
    // mosh on everywhere, a reset key at 10
    let mut s = cut_stream();
    let bb = base.clone();
    s.curve = Some(std::sync::Arc::new(move |k| {
        let mut v = bb.clone();
        v[rows::MOSH] = 1.0;
        v[rows::RESET] = if k >= 10 { 1.0 } else { 0.0 };
        v
    }));
    let before_beat = dec(&base, &s, 9, &memo);
    assert!(err_to(&before_beat, &a_moved(9)) < 4.0, "before the beat: the mosh");
    let at_beat = dec(&base, &s, 10, &memo);
    assert!(err(&at_beat, &b(10), 0, W as usize).0 < 4.0, "on the beat: an intact keyframe of B");
    let after = dec(&base, &s, 14, &memo);
    assert!(err(&after, &b(14), 0, W as usize).0 < 4.0, "after it the stream is B again");
    // mosh off from frame 5 on, GOP 12: the keyframe at 12 is intact, the mosh before it holds
    let mut t = cut_stream();
    let bb = params(&[(rows::BEFORE, 12.0), (rows::QP, 12.0), (rows::GOP, 12.0)]);
    let bc = bb.clone();
    t.curve = Some(std::sync::Arc::new(move |k| {
        let mut v = bc.clone();
        v[rows::MOSH] = if k < 5 { 1.0 } else { 0.0 };
        v
    }));
    assert!(err_to(&dec(&bb, &t, 9, &memo), &a_moved(9)) < 4.0, "the cut's keyframe was dropped while on");
    assert!(err(&dec(&bb, &t, 12, &memo), &b(12), 0, W as usize).0 < 4.0, "off: the next keyframe is intact");
    // the curve values are in the keys: the same frame under another curve is not reused
    assert_ne!(*dec(&bb, &t, 9, &memo), *dec(&bb, &cut_stream(), 9, &memo));
}

/// t1504 (compute-kernels): the encoder on the compute device keeps its quality in a closed loop,
/// and the moving picture's blocks carry the true motion.
#[test]
fn t1504_the_compute_encoder_keeps_its_quality() {
    if no_device() {
        return;
    }
    let cfg = Cfg { bs: 16, search: 16, qp: 24.0 };
    let mut prev: Option<codec::Clean> = None;
    for k in 0..8 {
        let src = planes(&tex(9, 2 * k, k));
        let c = codec::encode(&src, prev.as_ref().map(|c| &c.recon), &cfg).unwrap();
        let mse = src.y.iter().zip(&c.recon.y).map(|(a, b)| (*a as f64 - *b as f64).powi(2)).sum::<f64>() / src.y.len() as f64;
        let psnr = 10.0 * (255.0 * 255.0 / mse.max(1e-9)).log10();
        assert!(psnr > 40.0, "frame {k}: {psnr:.2} dB");
        if k > 0 {
            // the picture moves right and down, so a block reads the reference up and to the left
            let true_motion = c.pkt.mv.iter().filter(|v| **v == (-2, -1)).count();
            assert!(true_motion * 2 > c.pkt.mv.len(), "frame {k}: {true_motion} of {} blocks", c.pkt.mv.len());
        }
        prev = Some(c);
    }
}

/// The cost (datamosh-codec-path, task 5.1) — a measurement, not a check: run on a RELEASE build
/// with `cargo test --release -p fx-video-codec --test codec cost -- --ignored --nocapture`.
#[test]
#[ignore]
fn cost_per_frame_at_360_lines() {
    if no_device() {
        return;
    }
    let (w, h) = (640u32, 360u32);
    let frames: Vec<Vec<u8>> = (0..61)
        .map(|k| {
            let t = tex(9, 2 * k, k / 2);
            // the 128×96 texture tiled to 640×360
            let mut out = vec![255u8; (w * h * 4) as usize];
            for y in 0..h {
                for x in 0..w {
                    let s = (((y % H) * W + (x % W)) * 4) as usize;
                    let d = ((y * w + x) * 4) as usize;
                    out[d..d + 3].copy_from_slice(&t[s..s + 3]);
                }
            }
            out
        })
        .collect();
    let cfg = Cfg { bs: 16, search: 16, qp: 24.0 };
    let (pw, ph) = (codec::pad16(w), codec::pad16(h));
    let planes: Vec<Planes> = frames.iter().map(|f| Planes::from_rgba(f, w, h, pw, ph)).collect();
    let t0 = std::time::Instant::now();
    let mut prev: Option<codec::Clean> = None;
    for p in &planes {
        prev = Some(codec::encode(p, prev.as_ref().map(|c| &c.recon), &cfg).unwrap());
    }
    let enc = t0.elapsed().as_secs_f64() * 1000.0 / planes.len() as f64;
    let c = prev.unwrap();
    let t1 = std::time::Instant::now();
    for _ in 0..planes.len() {
        let _ = codec::decode(&c.recon, &c.pkt.mv, 16, &c.pkt.res, 200).unwrap();
    }
    let dec = t1.elapsed().as_secs_f64() * 1000.0 / planes.len() as f64;
    eprintln!("640x368: encode {enc:.2} ms/frame, broken decode {dec:.2} ms/frame; a cold seek through a 300-frame chain ≈ {:.0} ms", 300.0 * (enc + dec));
}

#[test]
fn t1572_the_decoder_copies_by_the_vector_clamps_and_scales() {
    if no_device() {
        return;
    }
    let (w, h, bs) = (32usize, 16usize, 16usize);
    let mut rf = Planes::grey(w, h);
    for y in 0..h {
        for x in 0..w {
            rf.y[y * w + x] = (x * 4 + y) as u8;
        }
    }
    for y in 0..h / 2 {
        for x in 0..w / 2 {
            rf.cb[y * w / 2 + x] = (x * 8 + y) as u8;
        }
    }
    let zero = vec![0i16; w * h];
    let chroma = vec![0i16; w * h / 4];
    let mut res = codec::Residual { y: zero, cb: chroma.clone(), cr: chroma };
    res.y[0] = 100;
    res.y[1] = -100;
    res.y[2] = 300;
    res.y[3] = 3;
    // block 0 reads 3 right and 2 up (off the frame); block 1 reads 5 left and 3 down
    let mv = [(3i16, -2i16), (-5, 3)];
    let got = codec::decode(&rf, &mv, bs, &res, 128).unwrap();
    let luma = |x: i32, y: i32| rf.y[y.clamp(0, 15) as usize * w + x.clamp(0, 31) as usize] as i32;
    let want = |x: i32, y: i32, r: i32| (luma(x, y) + ((r * 128) >> 8)).clamp(0, 255) as u8;
    assert_eq!(got.y[0], want(3, -2, 100), "the residual at half gain over the copy");
    assert_eq!(got.y[1], want(4, -2, -100), "a negative residual floors");
    assert_eq!(got.y[2], want(5, -2, 300));
    assert_eq!(got.y[3], want(6, -2, 3));
    assert_eq!(got.y[5 * w + 7], want(10, 3, 0), "the block's own pixel");
    assert_eq!(got.y[15 * w + 31], want(26, 18, 0), "off the bottom: clamped");
    // the chroma block is 8 wide and takes the vector halved, floored: (3,-2) → (1,-1), (-5,3) → (-3,1)
    let cb = |x: i32, y: i32| rf.cb[y.clamp(0, 7) as usize * w / 2 + x.clamp(0, 15) as usize];
    assert_eq!(got.cb[3 * w / 2 + 2], cb(3, 2));
    assert_eq!(got.cb[3 * w / 2 + 8 + 2], cb(8 + 2 - 3, 3 + 1));
}

/// t1651 (gpu-render-hardening, D4): the codec's compute shaders compile on D3D12 through FXC, as
/// the engine's do (t1650). Elsewhere: no D3D12, a skip.
#[test]
fn t1651_the_compute_shaders_compile_on_d3d12() {
    let Some(ctx) = kt_gpu::GpuContext::new_d3d12() else {
        eprintln!("skipped: no D3D12 adapter");
        return;
    };
    for (text, _, _) in codec::shaders() {
        kt_gpu::compute::compile(&ctx, text).unwrap_or_else(|e| panic!("{e}"));
    }
}
