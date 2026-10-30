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
        // the mode number is a function of the registry (D12), not of the pixels: it moves with a kind
        text.push_str(&format!("{case} p={:?}\n", m.p.map(f32::to_bits)));
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

/// (tag, digest) — printed with `KT_PRINT_GOLDEN=1` on the enum before any kind moved (906c5ea5);
/// reprinted without the mode number at 5.1a, after the wipe's row was shown equal with its former
/// mode substituted (the mode is the registry's, D12).
#[rustfmt::skip]
const GOLDEN: &[(&str, u64)] = &[
    ("crossfade", 0xc2d4bf8577cc473c),
    ("wipe", 0xdc677b6a51e9b9ba),
    ("iris", 0x968787d5a410be5a),
    ("clock", 0x6109eccf64fa7b76),
    ("barn", 0x5a5ce2073cddbacf),
    ("luma", 0x3d8aea4607a6f17d),
    ("push", 0x71ec014fb5440286),
    ("slide", 0x50533221a7ec6fb4),
    ("zoom", 0x91107519d3183f84),
    ("spin", 0x38cc8e6b8738581a),
    ("whip", 0xc94b93ff3293ec5b),
    ("blur_dissolve", 0xde24cb6ce3de98b2),
    ("pixelate", 0xf16169e989fdb0c8),
    ("glitch", 0x1bc6dffbb3e00686),
    ("flash", 0x6922377e91b11fe3),
    ("dip", 0xd4b114b9df506489),
    ("blinds", 0x1b9cdea11a194485),
    ("checker", 0x7a7acadeb36b5b56),
    ("blocks", 0x8036c43535b775cd),
    ("spiral", 0xbd70e5ab4fff0bea),
    ("split", 0xc2d12451b77d5e3c),
    ("squeeze", 0x2e8e8f4cec5a1c01),
    ("stretch", 0xf002461b87f21a7b),
    ("swap", 0xb1d4afb501041ace),
    ("shake", 0x0f29c203e9dbf3d0),
    ("film_burn", 0x337824a6302a7b16),
    ("light_leaks", 0x2bbaad47bb904aea),
    ("zoom_blur", 0x88e16edf42517dad),
    ("flip", 0x30b4dca677b86f3d),
    ("cube", 0xbc3484eb64cd3649),
    ("door", 0x49af5280f1e789f0),
    ("page_peel", 0x1162c56dbf954b33),
    ("page_roll", 0xd5cd13875bc69291),
];

#[test]
fn t1019_a_moved_transition_kind_keeps_its_pixels() {
    kt_plugins_builtin::install();
    let kinds = TransitionKind::all();
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
