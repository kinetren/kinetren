//! The codec model of `datamosh` (datamosh-codec-path): t1235–t1239, t1243.
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
}

static TAGS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Mock {
    fn new(first: i64, cut: i64, end: i64, f: impl Fn(i64) -> Vec<u8>) -> Mock {
        Mock { first, cut, end, main: (first..end).map(|k| (k, f(k))).collect(), second: None, tag: TAGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) }
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
    let s = cut_stream();
    let moshed = params(&[(rows::BEFORE, 12.0), (rows::DROP_COUNT, 1.0), (rows::QP, 12.0)]);
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
    let s = Mock::new(0, 0, 30, |k| tex(3, 2 * k as i32, (k / 3) as i32));
    let (b, len) = (6i64, 5i64);
    let p = params(&[(rows::BLOOM_AT, b as f64), (rows::BLOOM_LEN, len as f64)]);
    let cfg = Rows::from(&p).cfg;
    // the clean encoder, by hand
    let mut cl: Vec<codec::Clean> = vec![];
    for k in 0..=(b + len + 1) {
        let src = planes(&s.main[&k]);
        let c = codec::encode(&src, cl.last().map(|c| &c.recon), &cfg);
        cl.push(c);
    }
    assert!(!cl[b as usize].pkt.intra && cl[b as usize].pkt.mv.iter().any(|v| *v != (0, 0)));
    let memo = StreamMemo::new(64 << 20);
    for j in (b + 1)..=(b + len) {
        let prev = dec(&p, &s, j - 1, &memo);
        let pk = &cl[b as usize].pkt;
        let want = codec::reconstruct(&codec::predict(&prev, &pk.mv, cfg.bs), &pk.res, 256);
        assert_eq!(*dec(&p, &s, j, &memo), want, "frame {j} is packet {b} applied again");
    }
    // after the bloom the stream continues with its own packet
    let j = b + len + 1;
    let pk = &cl[j as usize].pkt;
    let want = codec::reconstruct(&codec::predict(&dec(&p, &s, j - 1, &memo), &pk.mv, cfg.bs), &pk.res, 256);
    assert_eq!(*dec(&p, &s, j, &memo), want);
    let plain = dec(&params(&[]), &s, b + len, &memo);
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
    let s = cut_stream();
    for p in [
        params(&[(rows::BEFORE, 12.0), (rows::DROP_COUNT, 2.0), (rows::GOP, 8.0)]),
        params(&[(rows::BEFORE, 12.0), (rows::DROP_COUNT, 1.0), (rows::BLOOM_AT, 4.0), (rows::BLOOM_LEN, 6.0)]),
        params(&[(rows::FREEZE_AT, 3.0), (rows::FREEZE_LEN, 12.0), (rows::LOSS, 0.2), (rows::SEED, 7.0)]),
    ] {
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
    let p = params(&[(rows::BEFORE, 12.0), (rows::DROP_COUNT, 1.0), (rows::MV_ROTATE, 30.0), (rows::LOSS, 0.1)]);
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
    let memo = StreamMemo::new(256 << 20);
    // freeze: every P-frame in [5, 15) predicts from the picture at 4
    let s = Mock::new(0, 0, 30, |k| tex(4, 2 * k as i32, 0));
    let p = params(&[(rows::FREEZE_AT, 5.0), (rows::FREEZE_LEN, 10.0)]);
    let cfg = Rows::from(&p).cfg;
    let mut cl: Vec<codec::Clean> = vec![];
    for k in 0..=12 {
        let c = codec::encode(&planes(&s.main[&k]), cl.last().map(|c| &c.recon), &cfg);
        cl.push(c);
    }
    let rf = dec(&p, &s, 4, &memo);
    for j in [5i64, 9, 12] {
        let pk = &cl[j as usize].pkt;
        let want = codec::reconstruct(&codec::predict(&rf, &pk.mv, cfg.bs), &pk.res, 256);
        assert_eq!(*dec(&p, &s, j, &memo), want, "frozen frame {j}");
    }
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

