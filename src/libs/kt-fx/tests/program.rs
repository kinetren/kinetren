//! loudness-norms: the program stage's kernel — the ceiling between samples.

use kt_fx::audio::loudness::{LoudnessMeter, measure};
use kt_fx::audio::program::{LOOKAHEAD, Params, Stage};
use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};

const SR: usize = AUDIO_SAMPLE_RATE as usize;

fn mono(n: usize, f: impl Fn(usize) -> f32) -> Vec<f32> {
    (0..n).flat_map(|i| [f(i), f(i)]).collect()
}

/// The stage over `x`, primed, in pieces of `cut` frames: as long as `x`.
fn run(x: &[f32], p: Params, cut: usize) -> Vec<f32> {
    let la = LOOKAHEAD * AUDIO_CHANNELS;
    let mut padded = x.to_vec();
    padded.resize(x.len() + la, 0.0);
    let mut s = Stage::new(p);
    s.prime(&padded[..la]);
    let mut out = padded[la..].to_vec();
    for c in out.chunks_mut(cut * AUDIO_CHANNELS) {
        s.process(c);
    }
    out
}

fn noise(n: usize, amp: f32) -> Vec<f32> {
    let mut h = 0x9E37_79B9u32;
    (0..n * AUDIO_CHANNELS)
        .map(|_| {
            h ^= h << 13;
            h ^= h >> 17;
            h ^= h << 5;
            (h as f32 / u32::MAX as f32 * 2.0 - 1.0) * amp
        })
        .collect()
}

/// t4201: a sine at a quarter of the rate at 45° (its peak stands between the samples), a full-scale
/// square, isolated clicks, a burst out of silence and noise, at +12 dB: the meter reads at most −1.0
/// dBTP each; a signal under the inner ceiling passes to the bit; the slicing changes nothing; the
/// baseline path gives the same bytes; a gain change glides.
#[test]
fn t4201_the_ceiling_holds_between_samples() {
    let p = Params { gain_db: 12.0, ceiling_dbtp: -1.0 };
    let pi = std::f32::consts::PI;
    let signals: Vec<(&str, Vec<f32>)> = vec![
        ("quarter sine", mono(SR, |i| 0.9 * (pi / 2.0 * i as f32 + pi / 4.0).sin())),
        ("square", mono(SR, |i| if (i / 50) % 2 == 0 { 1.0 } else { -1.0 })),
        ("clicks", mono(SR, |i| if i % 997 == 500 { 1.0 } else { 0.0 })),
        ("burst", mono(SR, |i| if (20_000..21_000).contains(&i) { (0.9 * i as f32).sin() } else { 0.0 })),
        ("noise", noise(SR, 1.0)),
        ("quiet noise with a loud second", {
            let mut v = noise(2 * SR, 0.02);
            for (a, b) in v[SR..SR * 2].iter_mut().zip(noise(SR, 0.9)) {
                *a = b;
            }
            v
        }),
    ];
    for (what, x) in &signals {
        let y = run(x, p, 1024);
        assert_eq!(y.len(), x.len());
        let m = measure(&y);
        assert!(m.dbtp <= -1.0, "{what}: {:.3} dBTP over the ceiling", m.dbtp);
        assert!(m.dbtp > -2.0, "{what}: {:.3} dBTP — held far under the ceiling", m.dbtp);
        for cut in [1usize, 7919] {
            let z = run(x, p, cut);
            assert!(y.iter().zip(&z).all(|(a, b)| a.to_bits() == b.to_bits()), "{what}: pieces of {cut} change the output");
        }
        let base = kt_plugin_api::cpu::with_baseline(|| run(x, p, 1024));
        assert!(y.iter().zip(&base).all(|(a, b)| a.to_bits() == b.to_bits()), "{what}: the baseline path differs");
    }
    // under the inner ceiling the stage is the delay alone
    let quiet = noise(SR, 0.4);
    let y = run(&quiet, Params { gain_db: 0.0, ceiling_dbtp: -1.0 }, 1024);
    assert!(quiet.iter().zip(&y).all(|(a, b)| a.to_bits() == b.to_bits()), "a quiet signal does not pass to the bit");
    // after a held peak the release ends on exactly the signal
    let mut tail = noise(2 * SR, 0.3);
    tail[2000] = 4.0;
    let y = run(&tail, Params { gain_db: 0.0, ceiling_dbtp: -1.0 }, 1024);
    assert!(y[2000].abs() < 0.9 && tail[SR * 2..].iter().zip(&y[SR * 2..]).all(|(a, b)| a.to_bits() == b.to_bits()));
    // a constant with +6 dB switched on in the middle: no step, the end doubled
    let x = mono(SR / 2, |_| 0.1);
    let mut s = Stage::new(Params { gain_db: 0.0, ceiling_dbtp: -1.0 });
    let mut out = x.clone();
    let half = out.len() / 2;
    s.process(&mut out[..half]);
    s.set(Params { gain_db: 6.0, ceiling_dbtp: -1.0 });
    s.process(&mut out[half..]);
    let step = out.chunks_exact(2).skip(LOOKAHEAD + 1).map(|f| f[0]).collect::<Vec<_>>();
    let worst = step.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
    assert!(worst < 0.001, "the gain jumps by {worst}");
    assert!((step.last().unwrap() - 0.1995).abs() < 0.001, "the gain never arrived: {}", step.last().unwrap());
    // silence stays silence, and a limited measure is what the meter reads
    assert!(run(&mono(4000, |_| 0.0), p, 64).iter().all(|v| *v == 0.0));
    let mut meter = LoudnessMeter::new();
    meter.feed(&run(&signals[0].1, p, 333));
    assert!(meter.finish().lufs.is_some());
}
