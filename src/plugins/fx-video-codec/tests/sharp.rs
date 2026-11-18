//! The sharp datamosh (mosh-sharp, F-FX-015): the codec decides the break at `lines`, a carrier
//! executes its decisions at the stream's input size. t2970–t2973.
//!
//! The stream is synthetic, as in `codec.rs`: value noise translated by whole pixels, here at an input
//! several times the codec's size.

use std::collections::HashMap;
use std::sync::Mutex;

use fx_video_codec::codec::{self, Planes};
use fx_video_codec::mosh::{self, Rows};
use fx_video_codec::{rows, Datamosh};
use kt_plugin_api::generator::lowbias32;
use kt_plugin_api::stream::{mix, Rgba, Side, StreamInput, StreamLayout, StreamMemo};
use kt_plugin_api::Plugin;

/// One test at a time: they share the compute device, and a software one is slow at these sizes.
static SERIAL: Mutex<()> = Mutex::new(());

fn no_device() -> bool {
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    let none = kt_plugin_api::compute::host().is_none();
    if none {
        assert!(std::env::var("KT_REQUIRE_GPU").as_deref() != Ok("1"), "KT_REQUIRE_GPU=1, but no compute device");
        eprintln!("skipped: no compute device");
    }
    none
}

/// Value noise `w`×`h`: hashed lattice every 8 px, bilinear; `dx` translates it.
fn tex(seed: u32, w: u32, h: u32, dx: i32) -> Vec<u8> {
    let lat = |x: i32, y: i32, c: u32| -> i32 {
        (lowbias32(seed.wrapping_mul(0x9e37) ^ lowbias32((x as u32).wrapping_mul(73) ^ lowbias32((y as u32).wrapping_mul(151) ^ c))) % 200) as i32 + 28
    };
    let mut out = vec![255u8; (w * h * 4) as usize];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let (sx, sy) = (x - dx + 1000, y + 1000);
            let (gx, gy, fx, fy) = (sx / 8, sy / 8, sx % 8, sy % 8);
            for c in 0..3u32 {
                let a = lat(gx, gy, c) * (8 - fx) + lat(gx + 1, gy, c) * fx;
                let b = lat(gx, gy + 1, c) * (8 - fx) + lat(gx + 1, gy + 1, c) * fx;
                out[((y as u32 * w + x as u32) * 4 + c) as usize] = ((a * (8 - fy) + b * fy) / 64) as u8;
            }
        }
    }
    out
}

static TAGS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// A stream at input size `w`×`h`; rows per position by `curve`.
struct Mock {
    w: u32,
    h: u32,
    first: i64,
    cut: i64,
    end: i64,
    main: HashMap<i64, Vec<u8>>,
    tag: u64,
    curve: Option<Box<dyn Fn(i64) -> Vec<f64> + Send + Sync>>,
}

impl Mock {
    fn new(w: u32, h: u32, first: i64, cut: i64, end: i64, f: impl Fn(i64) -> Vec<u8>) -> Mock {
        let tag = TAGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Mock { w, h, first, cut, end, main: (first..end).map(|k| (k, f(k))).collect(), tag, curve: None }
    }
}

impl StreamLayout for Mock {
    fn size(&self) -> (u32, u32) {
        (self.w, self.h)
    }
    /// The host's rule: `lines` (144 in these tests) at the input's aspect.
    fn coded(&self) -> (u32, u32) {
        kt_plugin_api::stream::coded_size(144, (self.w, self.h))
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
    fn second_start(&self, _k: i64) -> Option<i64> {
        None
    }
    fn ident(&self, _side: Side, k: i64) -> u64 {
        if self.main.contains_key(&k) { mix(mix(self.tag, ((self.w as u64) << 32) | self.h as u64), k as u64) } else { 0 }
    }
    fn rows_at(&self, k: i64) -> Option<Vec<f64>> {
        self.curve.as_ref().map(|c| c(k))
    }
}

impl StreamInput for Mock {
    fn with_frame(&self, side: Side, k: i64, f: &mut dyn FnMut(Rgba<'_>)) -> bool {
        match (side, self.main.get(&k)) {
            (Side::Main, Some(v)) => {
                f(Rgba { w: self.w, h: self.h, rgba: v });
                true
            }
            _ => false,
        }
    }
}

fn params(sets: &[(usize, f64)]) -> Vec<f64> {
    let mut p = fx_video_codec::defaults();
    for &(i, v) in sets {
        p[i] = v;
    }
    p
}

/// The kernel's RGBA picture at `n` and its size.
fn render(p: &[f64], s: &Mock, n: i64, memo: &StreamMemo) -> (Vec<u8>, u32, u32) {
    let k = Datamosh::new();
    let mut out = Vec::new();
    let o = k.stream().unwrap().render(p, s, n, memo, &mut out).expect("rendered");
    (out, o.w, o.h)
}

/// The luma of an RGBA frame through the codec's planes and back (what the kernel's conversion keeps).
fn luma(rgba: &[u8]) -> Vec<f64> {
    rgba.chunks_exact(4).map(|c| (77 * c[0] as i32 + 150 * c[1] as i32 + 29 * c[2] as i32 + 128) as f64 / 256.0).collect()
}

fn mad(a: &[u8], b: &[u8]) -> f64 {
    let (la, lb) = (luma(a), luma(b));
    la.iter().zip(&lb).map(|(x, y)| (x - y).abs()).sum::<f64>() / la.len() as f64
}

/// The mean horizontal luma step: fine detail.
fn detail(rgba: &[u8], w: u32) -> f64 {
    let l = luma(rgba);
    let mut sum = 0.0;
    let mut n = 0;
    for row in l.chunks_exact(w as usize) {
        for x in 1..row.len() {
            sum += (row[x] - row[x - 1]).abs();
            n += 1;
        }
    }
    sum / n as f64
}

/// The codec's own picture of a frame: RGBA through its planes and back.
fn through_planes(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let p = Planes::from_rgba(rgba, w, h, codec::pad16(w), codec::pad16(h));
    let mut out = Vec::new();
    p.to_rgba(w, h, &mut out);
    out
}

/// 144 lines: index 0 of `lines`.
const L144: f64 = 0.0;

/// t2971: an intact sharp stream is the source at the input's size — every block either carries the
/// source's own difference or was found static by the codec.
#[test]
fn t2971_an_intact_sharp_stream_is_the_source() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if no_device() {
        return;
    }
    // 4× the codec's 192×144; 4 px a frame is one codec pel
    let (w, h) = (768, 576);
    let s = Mock::new(w, h, 0, 0, 8, |k| tex(1, w, h, 4 * k as i32));
    let p = params(&[(rows::LINES_ROW, L144), (rows::SHARP, 1.0)]);
    let memo = StreamMemo::new(256 << 20);
    for n in 0..8 {
        let (pic, ow, oh) = render(&p, &s, n, &memo);
        assert_eq!((ow, oh), (w, h), "the carrier's picture is at the input's size");
        let src = through_planes(&s.main[&n], w, h);
        assert!(pic == src, "frame {n}: the intact carrier is the source (mean error {})", mad(&pic, &src));
    }
}

/// The cut stream: shot A (still) in the event before, shot B (moving 4 px a frame) from the cut.
fn cut_stream(w: u32, h: u32) -> Mock {
    Mock::new(w, h, -12, 0, 12, move |k| if k < 0 { tex(1, w, h, 0) } else { tex(2, w, h, 4 * k as i32) })
}

/// t2972: the sharp break is the soft one in detail — after a dropped keyframe the carrier keeps A's
/// texture dragged by B's motion, shrunk it is the soft frame's picture, and it carries fine detail.
#[test]
fn t2972_the_sharp_break_is_the_soft_break_in_detail() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if no_device() {
        return;
    }
    let (w, h) = (768, 576);
    let s = cut_stream(w, h);
    let base = [(rows::LINES_ROW, L144), (rows::BEFORE, 12.0), (rows::MOSH, 1.0), (rows::QP, 12.0)];
    let soft = params(&base);
    let mut sharp_sets = base.to_vec();
    sharp_sets.push((rows::SHARP, 1.0));
    let sharp = params(&sharp_sets);
    let memo = StreamMemo::new(512 << 20);
    for n in [3i64, 6, 9] {
        let (sp, sw, sh) = render(&soft, &s, n, &memo);
        assert_eq!((sw, sh), (192, 144), "the soft picture is the codec's");
        let (cp, cw, ch) = render(&sharp, &s, n, &memo);
        assert_eq!((cw, ch), (w, h));
        let shrunk = codec::resample_rgba(&cp, w, h, sw, sh);
        let b_small = codec::resample_rgba(&s.main[&n], w, h, sw, sh);
        let (to_soft, to_b) = (mad(&shrunk, &sp), mad(&shrunk, &b_small));
        assert!(to_soft * 2.0 < to_b, "frame {n}: the sharp break shrunk is the soft break ({to_soft}), not shot B ({to_b})");
        let up = codec::resample_rgba(&sp, sw, sh, w, h);
        // the texture's lattice is 8 px at the input, 2 px at the codec: the carrier keeps the input's
        // detail (within a tenth of the source's own), the soft frame enlarged loses a third of it
        let (d_sharp, d_soft, d_src) = (detail(&cp, w), detail(&up, w), detail(&s.main[&n], w));
        assert!(d_sharp > 1.3 * d_soft && d_sharp > 0.9 * d_src, "frame {n}: the carrier has the detail ({d_sharp} against {d_soft} enlarged, the source {d_src})");
    }
}

/// t2970: the batched walk carries the per-position reference's bytes — sharp with a bloom, a vector
/// edit and macroblock loss; soft with the input above the codec's size (the resample on the device).
#[test]
fn t2970_the_batched_walk_carries_the_reference_bytes() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if no_device() {
        return;
    }
    let (w, h) = (384, 288);
    let base = vec![(rows::LINES_ROW, L144), (rows::BEFORE, 12.0), (rows::MOSH, 1.0), (rows::MV_SCALE, 1.5), (rows::LOSS, 0.2), (rows::QP, 20.0)];
    let mut sharp_sets = base.clone();
    sharp_sets.push((rows::SHARP, 1.0));
    for (name, p) in [("sharp", params(&sharp_sets)), ("soft", params(&base))] {
        let mut s = cut_stream(w, h);
        let pc = p.clone();
        // a bloom over frames 4..=7
        s.curve = Some(Box::new(move |k| {
            let mut v = pc.clone();
            v[rows::BLOOM] = if (4..=7).contains(&k) { 1.0 } else { 0.0 };
            v
        }));
        let r = Rows::from(&p);
        let (a, b) = (StreamMemo::new(256 << 20), StreamMemo::new(256 << 20));
        for n in s.first..s.end {
            let (wp, wk) = mosh::render(r, &s, n, &a).expect("walk");
            let (rp, rk) = mosh::render_reference(r, &s, n, &b).expect("reference");
            assert_eq!(wk, rk, "{name}: key at {n}");
            assert!(*wp == *rp, "{name}: picture at {n}");
        }
        // cold, from nothing
        let last = s.end - 1;
        let cold = mosh::render_reference(r, &s, last, &StreamMemo::new(256 << 20)).unwrap().0;
        assert!(*mosh::render(r, &s, last, &StreamMemo::new(256 << 20)).unwrap().0 == *cold, "{name}: cold");
    }
}

/// t2973: the resample is the identity at the codec's size, keeps a flat colour, and a shrink of a
/// one-pixel checker gives its mean grey.
#[test]
fn t2973_the_resample_identity_flat_and_mean() {
    let (w, h) = (64u32, 48u32);
    let f = tex(5, w, h, 0);
    assert_eq!(codec::resample_rgba(&f, w, h, w, h), f, "the identity");
    let flat: Vec<u8> = (0..w * h).flat_map(|_| [90u8, 140, 200, 255]).collect();
    for (dw, dh) in [(16, 12), (128, 96), (100, 75)] {
        let o = codec::resample_rgba(&flat, w, h, dw, dh);
        assert!(o.chunks_exact(4).all(|c| c == [90, 140, 200, 255]), "flat at {dw}×{dh}");
    }
    let checker: Vec<u8> = (0..w * h).flat_map(|i| if (i % w + i / w) % 2 == 0 { [0u8, 0, 0, 255] } else { [255, 255, 255, 255] }).collect();
    let o = codec::resample_rgba(&checker, w, h, w / 4, h / 4);
    assert!(o.chunks_exact(4).all(|c| c[..3] == [128, 128, 128]), "the checker's mean");
}
