//! The audio effects' cost and bytes (audio-fx-cost; capabilities `audio-fx`, `plugins`).
//!
//! t1980 — every audio kind's node chain costs no more than its bound in units of the reference
//! kernel (`kt_fx::audio::cost`): a RATIO measured in the same run, block by block alternately, each block the minimum of nine rounds,
//! never a wall-clock limit. The bounds are about twice what the test profile measured when they
//! were set; a kind past its bound has grown a per-sample cost (a library call per sample, an
//! allocation, a walk) and is a finding, not noise.
//!
//! t1981 — the outputs the optimisations of audio-fx-cost had to keep: every kind at its defaults
//! and at its heavy settings, and three VHS Tape output switches mid-stream, as digests printed on
//! the tree BEFORE them (`kt-cli fx-cost --digest --secs 2` at db6df6eb), on the CPU's path AND
//! on the baseline path (`kt_plugin_api::cpu`): the AVX2 copy of a kernel is the same bytes.

use kt_fx::audio::cost;
use kt_model::FxInstance;

/// (kind, the digest at its defaults, at its heavy settings — 0: none), printed at db6df6eb.
#[rustfmt::skip]
const DIGESTS: &[(&str, u64, u64)] = &[
    ("gain", 0x4ee9c946f69552fa, 0x0000000000000000),
    ("slowed_reverb", 0x86dd5ff75fc3435c, 0x708185736c7b3ba2),
    ("filter", 0xcc07af326450cf66, 0x0000000000000000),
    ("eq", 0x4ee9c946f69552fa, 0x91895a51b045f2fd),
    ("distortion", 0x55be375a8677dd5a, 0x0000000000000000),
    ("bitcrusher", 0x2e4ef7e94722c985, 0x0000000000000000),
    ("stereo_width", 0x4ee9c946f69552fa, 0x0000000000000000),
    ("tremolo", 0xc035c46e4e37188a, 0x0000000000000000),
    ("auto_pan", 0xdb6ae54e1f49a255, 0x0000000000000000),
    ("vibrato", 0x9c9ac92554dbab27, 0x0000000000000000),
    ("chorus", 0xaab302e0dd1e6a8b, 0xf6871274bd02e331),
    ("flanger", 0x886423b1d7b96bce, 0x73db52203425ddb8),
    ("phaser", 0x28a0ac3472dd93db, 0x9d092c3547b592e4),
    ("delay", 0x20e6f9d497a42094, 0xb0794a8889ff1a07),
    ("compressor", 0x85f9ca723cf3c1c9, 0xe446f6df4e588e14),
    ("gate", 0xb3ae677014ace8d9, 0x0000000000000000),
    ("limiter", 0x9d130a1b5aa9b35b, 0x1e00d6d36c84513d),
    ("pitch_shift", 0x4ee9c946f69552fa, 0x1090d8cc9acff169),
    ("vhs_tape_sound", 0x55fbde11f47961eb, 0x3ea02914091a2f8e),
    ("datamosh_sound", 0x10fcfdc90decab48, 0xea7f59dfad3b03f7),
    ("camera_isp_sound", 0x083b0b916e9562a5, 0x4eefa7bd0380369d),
];

/// (switch, its digest, 0), printed at db6df6eb.
#[rustfmt::skip]
const SWITCHES: &[(&str, u64, u64)] = &[
    ("vhs hifi->normal", 0x3db74b35af7304df, 0),
    ("vhs mix->hifi", 0x811d36f06ffbc5ad, 0),
    ("vhs hifi->mix stereo", 0x78d252394c15d3ae, 0),
];

fn audio_kinds() -> Vec<&'static FxInstance> {
    kt_plugins_builtin::install();
    FxInstance::all().iter().filter(|f| f.is_audio()).collect()
}

fn digests() -> Vec<String> {
    let input = cost::stimulus(2 * 48_000);
    let mut bad = Vec::new();
    for f in audio_kinds() {
        let Some((_, d, h)) = DIGESTS.iter().find(|(t, _, _)| *t == f.tag()) else {
            bad.push(format!("{}: no row (a new kind prints its row with kt-cli fx-cost --digest)", f.tag()));
            continue;
        };
        let got = cost::digest(&cost::chain_output(std::slice::from_ref(f), &input));
        if got != *d {
            bad.push(format!("{} default: 0x{got:016x}, want 0x{d:016x}", f.tag()));
        }
        let got_h = cost::heavy(f).map_or(0, |hv| cost::digest(&cost::chain_output(&[hv], &input)));
        if got_h != *h {
            bad.push(format!("{} heavy: 0x{got_h:016x}, want 0x{h:016x}", f.tag()));
        }
    }
    for (name, a, b) in cost::switches() {
        let want = SWITCHES.iter().find(|(n, _, _)| *n == name).map(|r| r.1);
        let got = cost::digest(&cost::switched_output(&a, &b, &input));
        if want != Some(got) {
            bad.push(format!("{name}: 0x{got:016x}, want {want:x?}"));
        }
    }
    bad
}

#[test]
fn t1981_the_optimised_kernels_keep_their_bytes_on_both_paths() {
    let fast = digests();
    assert!(fast.is_empty(), "the CPU's path moved bytes: {fast:#?}");
    let base = kt_plugin_api::cpu::with_baseline(digests);
    assert!(base.is_empty(), "the baseline path moved bytes: {base:#?}");
}

/// (kind, its bound at the defaults, at the heavy settings) in units of the reference kernel: twice
/// what the test profile measured at audio-fx-cost (release measures within a third of it), at
/// least one reference — a kind that costs nothing measures noise. For scale: before audio-fx-cost
/// VHS Tape's sound cost 55 at its defaults and 77 heavy (release), after it 22 and 31.
#[rustfmt::skip]
const BOUNDS: &[(&str, f64, f64)] = &[
    ("gain", 1.0, 0.0),
    ("slowed_reverb", 4.0, 4.0),
    ("filter", 1.0, 0.0),
    ("eq", 1.0, 2.0),
    ("distortion", 1.0, 0.0),
    ("bitcrusher", 1.0, 0.0),
    ("stereo_width", 1.0, 0.0),
    ("tremolo", 2.0, 0.0),
    ("auto_pan", 2.0, 0.0),
    ("vibrato", 6.0, 0.0),
    ("chorus", 11.0, 16.0),
    ("flanger", 7.0, 7.0),
    ("phaser", 11.0, 13.0),
    ("delay", 3.0, 3.0),
    ("compressor", 7.0, 8.0),
    ("gate", 1.0, 0.0),
    ("limiter", 3.0, 3.0),
    ("pitch_shift", 1.0, 28.0),
    ("vhs_tape_sound", 54.0, 71.0),
    ("datamosh_sound", 39.0, 39.0),
    ("camera_isp_sound", 62.0, 71.0),
];

/// Rounds per kind: each block's time is the minimum over them (`cost::chain_cost`).
const ROUNDS: usize = 9;

#[test]
fn t1980_every_audio_kind_costs_within_its_bound() {
    let input = cost::stimulus(48_000);
    let print = std::env::var("KT_PRINT_COST").is_ok();
    let mut bad = Vec::new();
    for f in audio_kinds() {
        let Some(&(_, bd, bh)) = BOUNDS.iter().find(|(t, _, _)| *t == f.tag()) else {
            bad.push(format!("{}: no bound (a new kind measures itself with KT_PRINT_COST=1)", f.tag()));
            continue;
        };
        let cases = [(f.clone(), bd, "default")].into_iter().chain(cost::heavy(f).map(|h| (h, bh, "heavy")));
        for (fx, b, what) in cases {
            let c = cost::chain_cost(&[fx], &input, ROUNDS);
            if print {
                println!("{:<18} {what:<8} {:>8.2} ref  ({:.3}% of real time)", f.tag(), c.ratio(), c.share * 100.0);
            } else if c.ratio() > b {
                bad.push(format!("{} {what}: {:.2} reference kernels, bound {b}", f.tag(), c.ratio()));
            }
        }
    }
    assert!(bad.is_empty(), "an audio kind costs past its bound: {bad:#?}");
}
