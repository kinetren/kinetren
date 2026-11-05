//! Test 1019 (plugin-api-stage1, stage 5; capability `plugins`): the transition kinds keep their
//! pixels when they move into plugins.
//!
//! Every kind's digest — its packed numbers (`TransMix::of`) and the volumetric panels
//! (`trans_ext`) at five phases, at the defaults and with every number row at 70 % of its range —
//! is frozen here. The pixels are t224's (the stored CPU frames per mode) and t459's (the closed
//! form on ramps).

use kt_engine::transitions::{TransMix, trans_ext};
use kt_model::TransitionKind;

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
    let mut text = String::new();
    for (case, kk) in [("default", kind.clone()), ("tuned", tuned(kind))] {
        let m = TransMix::of(&kk);
        // the mode number is a function of the registry (D12), not of the pixels: it moves with a kind
        text.push_str(&format!("{case} p={:?}\n", m.p.map(f32::to_bits)));
        for k in [0u32, 1024, 2048, 3072, 4096] {
            let ext = trans_ext(&m, k);
            text.push_str(&format!("  ext({k})={:?}\n", ext.map(f32::to_bits)));
        }
    }
    xxhash_rust::xxh3::xxh3_64(text.as_bytes())
}

/// (tag, digest) — printed with `KT_PRINT_GOLDEN=1`; reprinted without the pixels when the CPU
/// composite went (compute-kernels-render).
#[rustfmt::skip]
const GOLDEN: &[(&str, u64)] = &[
    ("crossfade", 0xc2d4bf8577cc473c),
    ("wipe", 0x5174933fec4f07d4),
    ("iris", 0xb8c419533649a3de),
    ("clock", 0xb8c419533649a3de),
    ("barn", 0xc21834c25a1ed4ba),
    ("luma", 0xc21834c25a1ed4ba),
    ("push", 0x21d846b7896ffdac),
    ("slide", 0x21d846b7896ffdac),
    ("zoom", 0x6fed622adaaf5139),
    ("spin", 0xb5cf4ff94243b84d),
    ("whip", 0xdf7b883e25d8dfc5),
    ("blur_dissolve", 0x03e40dae38b8e7b6),
    ("pixelate", 0x6068f0f5fe1cc1c5),
    ("glitch", 0x6da3190202c3f57e),
    ("flash", 0xd545111520031057),
    ("dip", 0x3a435d5aa6f5c0fd),
    ("blinds", 0x799cceb756f67eb2),
    ("checker", 0x2cbc7d9f31d14e29),
    ("blocks", 0x305ed0e2d9c1b329),
    ("spiral", 0x34a435e14fd9f43d),
    ("split", 0xc2d4bf8577cc473c),
    ("squeeze", 0x21d846b7896ffdac),
    ("stretch", 0xc2d4bf8577cc473c),
    ("swap", 0x5f4111d50d80cd4d),
    ("shake", 0x8a231e2a480c9367),
    ("film_burn", 0x2bc6f9bb20f574ab),
    ("light_leaks", 0x1019f99e0a78952e),
    ("zoom_blur", 0xf026587c68ab036a),
    ("flip", 0xa00ba0bbea2a73da),
    ("cube", 0x8c2b70f87ab2503d),
    ("door", 0xd17df8510cd788fb),
    ("page_peel", 0x14ce1e54694ad8bd),
    ("page_roll", 0x14ce1e54694ad8bd),
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
            Some((_, want)) if digest(k) != *want => problems.push(format!("{}: the numbers moved", k.tag())),
            _ => {}
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}
