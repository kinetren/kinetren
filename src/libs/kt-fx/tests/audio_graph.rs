//! fx-graph-dag D4 (capability `audio-fx`): the event's processor runs a sound graph. A held output
//! feeds every slot that names it, a Merge sums in a fixed order, the side with less look-ahead is
//! delayed before a Merge, and a compressor's key socket takes an arm of its own chain.

use kt_fx::{CH, ChainGraph, ChainOpts, ClipProcessor, PullKind, sound_split};
use kt_model::fx_graph::{FxIn, MergeMode, SlotInputs};
use kt_model::{AutoProfiles, FxInstance};

const SR: usize = 48_000;

fn comp() -> FxInstance {
    FxInstance::plugin_with("compressor", &[-24.0, 8.0, 5.0, 60.0, 0.0, 0.0]).unwrap()
}

fn limiter() -> FxInstance {
    FxInstance::plugin_with("limiter", &[-20.0, -6.0, 5.0, 80.0]).unwrap()
}

fn sum() -> FxInstance {
    FxInstance::Merge { mode: MergeMode::Sum, mix: 1.0 }
}

fn wire(main: FxIn, second: Option<FxIn>) -> SlotInputs {
    SlotInputs { main, second, matte: None }
}

/// A tone with a click: a loud single frame at `click`.
fn signal(frames: usize, click: usize) -> Vec<f32> {
    let mut v = vec![0f32; frames * CH];
    for i in 0..frames {
        let s = (2.0 * std::f32::consts::PI * 220.0 * i as f32 / SR as f32).sin() * 0.4;
        v[i * CH] = s;
        v[i * CH + 1] = s * 0.5;
    }
    v[click * CH] = 0.95;
    v[click * CH + 1] = 0.95;
    v
}

/// The processor of `fx` wired by `fx_in`, read over the whole source in bursts.
fn render(fx: &[FxInstance], fx_in: &[SlotInputs], src: &[f32]) -> Vec<f32> {
    let split = sound_split(fx, fx_in);
    let frames = src.len() / CH;
    let auto = AutoProfiles::default();
    let mut p = ClipProcessor::new_graph(&split.main, &split.graph, &auto, 0, frames as i64, ChainOpts::default());
    let mut out = vec![0f32; src.len()];
    let mut pull = |_: PullKind, at: i64, dst: &mut [f32]| {
        for (k, v) in dst.iter_mut().enumerate() {
            let i = at as usize * CH + k;
            *v = if at >= 0 && i < src.len() { src[i] } else { 0.0 };
        }
        true
    };
    for (n, chunk) in out.chunks_mut(1024 * CH).enumerate() {
        p.read((n * 1024) as i64, chunk, &mut pull);
    }
    out
}

fn line(fx: &[FxInstance], src: &[f32]) -> Vec<f32> {
    render(fx, &[], src)
}

fn peak_frame(buf: &[f32]) -> usize {
    (0..buf.len() / CH).max_by(|a, b| buf[a * CH].abs().total_cmp(&buf[b * CH].abs())).unwrap()
}

/// Test t3213 (fx-graph-dag D4): parallel compression — the source summed with a compressed arm
/// equals the source plus the compressor run alone, sample for sample, whichever side is the
/// Background; two runs give the same bytes.
#[test]
fn t3213_parallel_compression_is_the_source_plus_the_arm() {
    kt_plugins_builtin::install();
    let src = signal(SR / 2, 9_000);
    let arm = line(&[comp()], &src);
    let want: Vec<f32> = src.iter().zip(&arm).map(|(s, a)| s + a).collect();
    // Background = the Source, Foreground = the arm
    let fx = [comp(), sum()];
    let bg_src = render(&fx, &[SlotInputs::default(), wire(FxIn::Source, Some(FxIn::Slot(0)))], &src);
    assert!(bg_src == want, "the sum is not the source plus the arm");
    // Background = the arm, Foreground = the Source
    let bg_arm = render(&fx, &[SlotInputs::default(), wire(FxIn::Prev, Some(FxIn::Source))], &src);
    assert!(bg_arm == want, "the other order of building gave other samples");
    assert!(bg_src == render(&fx, &[SlotInputs::default(), wire(FxIn::Source, Some(FxIn::Slot(0)))], &src), "two runs differ");
    // a Crossfade at 0 is the Background alone
    let xf = [comp(), FxInstance::Merge { mode: MergeMode::Crossfade, mix: 0.0 }];
    assert!(render(&xf, &[SlotInputs::default(), wire(FxIn::Source, Some(FxIn::Slot(0)))], &src) == src, "a crossfade at 0 is not its Background");
}

/// Test t3214 (fx-graph-dag D4): a look-ahead limiter in an arm — the dry side is delayed by the
/// limiter's window before the Merge, so the click lands on the same frame on both sides and the
/// sum is the source plus the limiter run alone.
#[test]
fn t3214_a_look_ahead_arm_is_aligned_with_the_dry_side() {
    kt_plugins_builtin::install();
    let click = 12_000;
    let src = signal(SR / 2, click);
    let arm = line(&[limiter()], &src);
    assert_eq!(peak_frame(&arm), click, "scene: the limiter alone keeps the click in place");
    let fx = [limiter(), sum()];
    let w = [SlotInputs::default(), wire(FxIn::Source, Some(FxIn::Slot(0)))];
    let split = sound_split(&fx, &w);
    assert!(kt_fx::chain_lookahead_in(&split.main, &split.graph, &AutoProfiles::default()) > 0, "scene: the arm has a look-ahead");
    let got = render(&fx, &w, &src);
    assert_eq!(peak_frame(&got), click, "the click moved: the dry side is not aligned");
    let want: Vec<f32> = src.iter().zip(&arm).map(|(s, a)| s + a).collect();
    assert!(got == want, "the aligned sum is not the source plus the limiter alone");
    assert!(got == render(&fx, &w, &src), "two runs differ");
}

/// Test t3215 (fx-graph-dag D4): a compressor keyed by a high-passed copy of its own input ducks
/// under the quiet hi-hat, not under the loud bass; keyed by itself (the line) it ducks all along.
#[test]
fn t3215_a_compressor_keyed_by_its_own_filtered_copy() {
    kt_plugins_builtin::install();
    let frames = SR;
    let hats = [(10_000usize, 14_000usize), (30_000usize, 34_000usize)];
    let mut src = vec![0f32; frames * CH];
    let mut seed = 0x1234_5678u32;
    for i in 0..frames {
        let bass = (2.0 * std::f32::consts::PI * 60.0 * i as f32 / SR as f32).sin() * 0.25;
        let hat = if hats.iter().any(|(a, b)| (*a..*b).contains(&i)) {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5) * 0.5
        } else {
            0.0
        };
        src[i * CH] = bass + hat;
        src[i * CH + 1] = bass + hat;
    }
    let hp = FxInstance::plugin_with("filter", &[1.0, 3000.0, 0.707]).unwrap();
    let fx = [hp, comp()];
    let w = [SlotInputs::default(), wire(FxIn::Source, Some(FxIn::Slot(0)))];
    let keyed = render(&fx, &w, &src);
    let rms = |b: &[f32], a: usize, z: usize| {
        let s = &b[a * CH..z * CH];
        (s.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / s.len() as f64).sqrt()
    };
    // the bass alone between the hats: hardly touched; under a hat: pressed down
    let gain = |b: &[f32], a: usize, z: usize| rms(b, a, z) / rms(&src, a, z);
    let (under, between) = (gain(&keyed, 12_000, 14_000), gain(&keyed, 20_000, 28_000));
    assert!(between > 0.9, "the loud bass drove the key: {between}");
    assert!(under < between * 0.8, "the hi-hat did not duck the sound: {under} against {between}");
    // the same compressor keyed by its own input (no wiring) presses the bass all along
    let own = line(&[comp()], &src);
    assert!(gain(&own, 20_000, 28_000) < 0.9, "scene: the bass alone is over the threshold");
    assert!(keyed == render(&fx, &w, &src), "two runs differ");
    let _ = ChainGraph::default();
}
