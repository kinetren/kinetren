//! t2317 (local-cost-queue, row 314; capability `plugins`): a frame of the shared audio codec
//! allocates nothing but its packet's payload, once — the transforms' scratch is on the stack, the
//! predictors update without copying the prediction — and the decoded samples are the bytes they
//! were before (a digest printed on the tree before the change).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use kt_plugin_api::audio_codec::{Damage, Decoder, Encoder, N};
use kt_plugin_api::math::FrozenMath;

struct Counting;

thread_local! {
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn allocs() -> u64 {
    ALLOCS.with(|c| c.get())
}

/// Music-like material: two tones, a sweep and a click every 0.3 s.
fn signal(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let t = i as f64 / 48_000.0;
            0.3 * (t * 440.0 * std::f64::consts::TAU).fsin()
                + 0.2 * (t * (200.0 + 900.0 * t) * std::f64::consts::TAU).fsin()
                + if i % 14_400 < 8 { 0.4 } else { 0.0 }
        })
        .collect()
}

/// (predict, digest of the decoded samples' bits) printed before local-cost-queue (87f6f38d).
const DIGESTS: [(bool, u64); 2] = [(false, 0xc7f4_0992_3281_ff0b), (true, 0x9dcd_1b63_e7f9_6c32)];

#[test]
fn t2317_a_codec_frame_allocates_only_its_payload() {
    let frames = 68;
    let x = signal((frames + 1) * N);
    let mut bad = Vec::new();
    for (predict, want) in DIGESTS {
        let mut enc = Encoder::new(48.0);
        let mut dec = Decoder::new(7);
        let mut out = vec![0f64; N];
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut worst = 0;
        for f in 0..frames {
            let before = allocs();
            let block = &x[f * N..f * N + 2 * N];
            let pkt = enc.encode(block, f % 16 == 0, 1e-4, N / 2, predict);
            dec.decode(&pkt, &pkt.payload.v, Damage { residual: 1.0, ..Default::default() }, N / 2, predict, f as u64, &mut out);
            let used = allocs() - before;
            if f >= 4 {
                worst = worst.max(used);
            }
            drop(pkt);
            for v in &out {
                h = (h ^ v.to_bits()).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        if std::env::var("KT_PRINT_DIGESTS").is_ok() {
            println!("({predict}, 0x{h:016x})");
        }
        if h != want {
            bad.push(format!("predict={predict}: 0x{h:016x}, want 0x{want:016x}"));
        }
        if worst > 1 {
            bad.push(format!("predict={predict}: a frame allocated {worst} times (the payload is one)"));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
