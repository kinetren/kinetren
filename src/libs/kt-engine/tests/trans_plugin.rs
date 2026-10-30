//! Test 1019 (plugin-api-stage1, stage 5; capability `plugins`): the transition kinds keep their
//! pixels when they move into plugins.
//!
//! Before any kind moves, every kind's CPU kernel is printed as a digest — its packed numbers
//! (`TransMix::of`), the volumetric panels (`trans_ext`) and the bytes of `mix_into` at five
//! phases, at the defaults and with every number row at 70 % of its range, over a two-sided pair
//! and a solo fade (an empty side) — and frozen here. The move must leave every row unchanged;
//! the GPU side is t224's (CPU against GPU per mode) and t459's (the closed form on ramps).

use kt_engine::transitions::{TransMix, mix_into, trans_ext};
use kt_media::VideoFrame;
use kt_model::TransitionKind;
use kt_time::Tick;

const W: usize = 48;
const H: usize = 27;

/// A frame with structure along both axes (a direction's Y term must be visible, t224's lesson)
/// and, for `alpha`, a transparent band.
fn frame(seed: u32, alpha: bool) -> VideoFrame {
    let mut rgba = vec![0u8; W * H * 4];
    for y in 0..H {
        for x in 0..W {
            let i = (y * W + x) * 4;
            rgba[i] = ((x * 255) / (W - 1)) as u8 ^ (seed as u8);
            rgba[i + 1] = ((y * 255) / (H - 1)) as u8;
            rgba[i + 2] = (((x + y) * 7 + seed as usize * 31) % 256) as u8;
            rgba[i + 3] = if alpha && x < W / 4 { 0 } else { 255 };
        }
    }
    VideoFrame { pts: Tick(0), width: W as u32, height: H as u32, rgba: rgba.into(), has_alpha: alpha }
}

/// The kind with every NUMBER row at 70 % of its range (the choice/flag rows keep their value).
fn tuned(kind: &TransitionKind) -> TransitionKind {
    let mut k = kind.clone();
    let specs: Vec<_> = k.param_specs().to_vec();
    for s in specs.iter().filter(|s| s.ty == kt_model::ParamType::Num) {
        k.set_param(&s.name, s.min + 0.7 * (s.max - s.min));
    }
    k
}

fn digest(kind: &TransitionKind) -> u64 {
    let (a, b) = (frame(3, false), frame(141, true));
    let mut text = String::new();
    for (case, kk) in [("default", kind.clone()), ("tuned", tuned(kind))] {
        let m = TransMix::of(&kk);
        text.push_str(&format!("{case} mode={} p={:?}\n", m.mode, m.p.map(f32::to_bits)));
        for k in [0u32, 1024, 2048, 3072, 4096] {
            let ext = trans_ext(&m, k);
            text.push_str(&format!("  ext({k})={:?}\n", ext.map(f32::to_bits)));
            if m.is_crossfade() {
                continue; // the crossfade is the integer dissolve, GOLDEN_VIDEO's
            }
            for (side, fa, fb) in [("pair", Some(&a), Some(&b)), ("fade-in", None, Some(&b)), ("fade-out", Some(&a), None)] {
                let mut dst = Vec::new();
                mix_into(&mut dst, fa, fb, k, m, W, H);
                text.push_str(&format!("  {side}({k})={:016x}\n", xxhash_rust::xxh3::xxh3_64(&dst)));
            }
        }
    }
    xxhash_rust::xxh3::xxh3_64(text.as_bytes())
}

/// (tag, digest) — printed with `KT_PRINT_GOLDEN=1` on the enum, before any kind moved.
#[rustfmt::skip]
const GOLDEN: &[(&str, u64)] = &[
    ("crossfade", 0xeeefe83d2093b283),
    ("wipe", 0x3d026e4405beb616),
    ("iris", 0x40a959542b330000),
    ("clock", 0xbd22c81895ea7fbf),
    ("barn", 0xdafbef069b063fff),
    ("luma", 0x1f3477e5093f4680),
    ("push", 0x4921cd3c2af97bb0),
    ("slide", 0xbeeeaf30e2ca4a01),
    ("zoom", 0xde124c050c828246),
    ("spin", 0x55735c8b3b9c61a4),
    ("whip", 0x2af2f4b5ddc499dc),
    ("blur_dissolve", 0x664cc0736e334b57),
    ("pixelate", 0x9919f3453b94a346),
    ("glitch", 0x31e63bc907d5ffdc),
    ("flash", 0x99b1c9ab43eeceb9),
    ("dip", 0xbd35619b432991fb),
    ("blinds", 0xb801a47c52e08c06),
    ("checker", 0x4653abcec363d122),
    ("blocks", 0xa17a6f2a2b842aec),
    ("spiral", 0x84bb0ebab214d29c),
    ("split", 0xa053cdc953b1fb8f),
    ("squeeze", 0x1bd63391f777e042),
    ("stretch", 0x152d64ea46c43d1a),
    ("swap", 0x7d515ef9a1f2f131),
    ("shake", 0x13fca52d21edb0ab),
    ("film_burn", 0x1e7335a4cce44d57),
    ("light_leaks", 0x2fabe0f3d01a1cf5),
    ("zoom_blur", 0x201d4a5fed6ef3bf),
    ("flip", 0xe1301eeb13c527fd),
    ("cube", 0xe56f39017c6d3e23),
    ("door", 0x4becfb91500d9003),
    ("page_peel", 0xb2d51f946b6ff8ca),
    ("page_roll", 0x09cc92edae0a72ad),
];

#[test]
fn t1019_a_moved_transition_kind_keeps_its_pixels() {
    let kinds = TransitionKind::ALL;
    if std::env::var("KT_PRINT_GOLDEN").is_ok() {
        for k in kinds {
            println!("    (\"{}\", 0x{:016x}),", k.tag(), digest(k));
        }
        return;
    }
    let mut problems = Vec::new();
    for k in kinds {
        match GOLDEN.iter().find(|(t, _)| *t == k.tag()) {
            None => problems.push(format!("{}: no golden row (completeness lock)", k.tag())),
            Some((_, want)) if digest(k) != *want => problems.push(format!("{}: the pixels moved", k.tag())),
            _ => {}
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}
