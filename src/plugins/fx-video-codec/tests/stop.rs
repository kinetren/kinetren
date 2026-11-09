//! t2257 (gpu-arbiter; capability `plugins`): a stream history stops between positions when the host
//! says nobody wants the frame, keeps what it computed, and a later render resumes to the same bytes.

use std::cell::Cell;
use std::collections::HashMap;

use fx_video_codec::{rows, Datamosh};
use kt_plugin_api::generator::lowbias32;
use kt_plugin_api::stream::{mix, Rgba, Side, StreamInput, StreamKernel, StreamLayout, StreamMemo};
use kt_plugin_api::{ParamValues, Plugin};

const W: u32 = 64;
const H: u32 = 48;

fn no_device() -> bool {
    kt_plugin_api::compute::install(kt_gpu::compute::open);
    let none = kt_plugin_api::compute::host().is_none();
    if none {
        assert!(std::env::var("KT_REQUIRE_GPU").as_deref() != Ok("1"), "KT_REQUIRE_GPU=1, but no compute device");
        eprintln!("skipped: no compute device");
    }
    none
}

/// Hashed noise moved by `dx` pixels: motion the encoder finds.
fn tex(seed: u32, dx: i32) -> Vec<u8> {
    let mut out = vec![255u8; (W * H * 4) as usize];
    for y in 0..H as i32 {
        for x in 0..W as i32 {
            for c in 0..3u32 {
                let v = lowbias32(seed ^ lowbias32(((x - dx + 1000) / 4) as u32 ^ lowbias32(((y / 4) as u32) << 8 ^ c)));
                out[((y as u32 * W + x as u32) * 4 + c) as usize] = (v % 200) as u8 + 28;
            }
        }
    }
    out
}

/// A cut stream; `stop` turns true after `after` asks, and every source read after it is counted.
struct Stream {
    main: HashMap<i64, Vec<u8>>,
    tag: u64,
    after: Option<u32>,
    asks: Cell<u32>,
    stopped: Cell<bool>,
    reads_after: Cell<u32>,
}

impl Stream {
    fn new(tag: u64, after: Option<u32>) -> Stream {
        let main = (-12..40).map(|k| (k, if k < 0 { tex(1, 0) } else { tex(2, 2 * k as i32) })).collect();
        Stream { main, tag, after, asks: Cell::new(0), stopped: Cell::new(false), reads_after: Cell::new(0) }
    }
}

impl StreamLayout for Stream {
    fn size(&self) -> (u32, u32) {
        (W, H)
    }
    fn first(&self) -> i64 {
        -12
    }
    fn cut(&self) -> i64 {
        0
    }
    fn end(&self) -> i64 {
        40
    }
    fn second_start(&self, _k: i64) -> Option<i64> {
        None
    }
    fn ident(&self, side: Side, k: i64) -> u64 {
        if side == Side::Main && self.main.contains_key(&k) { mix(mix(self.tag, 1), k as u64) } else { 0 }
    }
}

impl StreamInput for Stream {
    fn with_frame(&self, side: Side, k: i64, f: &mut dyn FnMut(Rgba<'_>)) -> bool {
        if self.stopped.get() {
            self.reads_after.set(self.reads_after.get() + 1);
        }
        match self.main.get(&k).filter(|_| side == Side::Main) {
            Some(v) => {
                f(Rgba { w: W, h: H, rgba: v });
                true
            }
            None => false,
        }
    }
    fn stop(&self) -> bool {
        self.asks.set(self.asks.get() + 1);
        let stop = self.after.is_some_and(|a| self.asks.get() > a);
        self.stopped.set(self.stopped.get() || stop);
        stop
    }
}

/// Test t2257: a history of ~50 positions stopped after its 5th ask returns `None` and reads no source
/// frame after the stop; the memo holds what it computed, so the next render (no stop) needs fewer
/// positions and gives the bytes of a render in a fresh memo.
#[test]
fn t2257_a_history_stops_within_one_position_and_resumes_to_the_same_bytes() {
    if no_device() {
        return;
    }
    let d = Datamosh::new();
    let mut p = ParamValues::defaults(&d.manifest().passport).0;
    p[rows::BEFORE] = 12.0;
    p[rows::MOSH] = 1.0;
    let n = 39;
    let memo = StreamMemo::new(256 << 20);
    let s = Stream::new(7, Some(5));
    let all = d.needs(&p, &s, n, &memo).len();
    let mut out = Vec::new();
    assert!(d.render(&p, &s, n, &memo, &mut out).is_none(), "the stop was not obeyed");
    assert!(s.stopped.get());
    assert_eq!(s.reads_after.get(), 0, "a source frame was read after the stop");
    let left = d.needs(&p, &s, n, &memo).len();
    assert!(left < all, "nothing computed before the stop stayed in the memo ({left} of {all} still needed)");

    let resumed = Stream::new(7, None);
    let mut a = Vec::new();
    let ka = d.render(&p, &resumed, n, &memo, &mut a).expect("resumed").key;
    let fresh = Stream::new(7, None);
    let mut b = Vec::new();
    let kb = d.render(&p, &fresh, n, &StreamMemo::new(256 << 20), &mut b).expect("fresh").key;
    assert_eq!(ka, kb);
    assert!(a == b, "the resumed history's bytes differ from an uninterrupted one's");
}
