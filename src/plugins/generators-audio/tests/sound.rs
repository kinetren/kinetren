//! The sound kernels (generators-placement-and-audio, capability `generators`): t1157 tone level,
//! frequency and channels; t1158 noise colour, level, seed and stereo; t1159 the sweep's curve,
//! phase continuity and repeat; t1160 the beep as the gated tone, silence as zeros.
//!
//! The kernels are called directly, burst by burst, as the host does (`GeneratorSource`): the
//! values in passport order, the global sample number of each burst's first frame.

use kt_plugin_api::generator::Synth;

const BURST: usize = 1024;
const RATE: f64 = 48_000.0;

fn synth(id: &str) -> Synth {
    let all = generators_audio::plugins();
    let p = all.iter().find(|p| p.manifest().id == id).unwrap_or_else(|| panic!("no kind {id}"));
    p.generator().and_then(|g| g.synth()).expect("a sound kind synthesizes")
}

/// `frames` frames from sample `n0`, interleaved stereo, read burst by burst.
fn read(id: &str, v: &[f64], n0: i64, frames: usize) -> Vec<f32> {
    let s = synth(id);
    let mut out = Vec::with_capacity(frames * 2);
    let mut n = n0;
    while out.len() < frames * 2 {
        let mut b = [0f32; BURST * 2];
        s(v, n, &mut b);
        out.extend_from_slice(&b);
        n += BURST as i64;
    }
    out.truncate(frames * 2);
    out
}

fn left(x: &[f32]) -> Vec<f64> {
    x.chunks_exact(2).map(|c| c[0] as f64).collect()
}

fn crossings(x: &[f64]) -> usize {
    x.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count()
}

#[test]
fn t1157_tone_level_frequency_and_channels() {
    let x = read("tone", &[440.0, -6.0, 0.0], 0, 48_000);
    let l = left(&x);
    let peak = l.iter().fold(0f64, |m, v| m.max(v.abs()));
    let want = 10f64.powf(-6.0 / 20.0);
    assert!((peak - want).abs() < 1e-4, "the peak {peak} is not −6 dBFS ({want})");
    let z = crossings(&l);
    assert!(z.abs_diff(880) <= 1, "440 Hz crosses zero 880 times a second, got {z}");
    assert!(x.chunks_exact(2).all(|c| c[0] == c[1]), "both channels carry the tone");
    let lo = read("tone", &[440.0, -6.0, 1.0], 0, 4800);
    assert!(lo.chunks_exact(2).all(|c| c[1] == 0.0) && lo.iter().any(|v| *v != 0.0), "left only");
    let ro = read("tone", &[440.0, -6.0, 2.0], 0, 4800);
    assert!(ro.chunks_exact(2).all(|c| c[0] == 0.0) && ro.iter().any(|v| *v != 0.0), "right only");
}

/// A radix-2 FFT, in place (the test's own measuring tool, not a frozen path).
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let a = -std::f64::consts::TAU / len as f64;
        for s in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = ((a * k as f64).cos(), (a * k as f64).sin());
                let (ur, ui) = (re[s + k], im[s + k]);
                let (vr, vi) = (re[s + k + len / 2] * wr - im[s + k + len / 2] * wi, re[s + k + len / 2] * wi + im[s + k + len / 2] * wr);
                re[s + k] = ur + vr;
                im[s + k] = ui + vi;
                re[s + k + len / 2] = ur - vr;
                im[s + k + len / 2] = ui - vi;
            }
        }
        len <<= 1;
    }
}

/// The slope of the power DENSITY in dB per octave, octave bands centred 125 Hz … 8 kHz (Welch,
/// Hann, 8192-point segments, a least-squares line through the bands).
fn slope_db_per_octave(x: &[f64]) -> f64 {
    const N: usize = 8192;
    let mut psd = vec![0f64; N / 2];
    let hann: Vec<f64> = (0..N).map(|i| 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / N as f64).cos()).collect();
    for seg in x.chunks_exact(N) {
        let mut re: Vec<f64> = seg.iter().zip(&hann).map(|(a, w)| a * w).collect();
        let mut im = vec![0f64; N];
        fft(&mut re, &mut im);
        for k in 0..N / 2 {
            psd[k] += re[k] * re[k] + im[k] * im[k];
        }
    }
    let bin = RATE / N as f64;
    let pts: Vec<(f64, f64)> = (0..7)
        .map(|o| {
            let fc = 125.0 * 2f64.powi(o);
            let (a, b) = ((fc / 2f64.sqrt() / bin) as usize, (fc * 2f64.sqrt() / bin) as usize);
            let d = psd[a..b].iter().sum::<f64>() / (b - a) as f64;
            (o as f64, 10.0 * d.log10())
        })
        .collect();
    let n = pts.len() as f64;
    let (sx, sy) = pts.iter().fold((0.0, 0.0), |s, p| (s.0 + p.0, s.1 + p.1));
    let (sxx, sxy) = pts.iter().fold((0.0, 0.0), |s, p| (s.0 + p.0 * p.0, s.1 + p.0 * p.1));
    (n * sxy - sx * sy) / (n * sxx - sx * sx)
}

#[test]
fn t1158_noise_colour_level_seed_and_stereo() {
    for (color, want, tol) in [(0.0, 0.0, 1.0), (1.0, -3.0, 1.0), (2.0, -6.0, 1.5)] {
        let x = read("audio_noise", &[color, -20.0, 1.0, 0.0], 0, 8 * 48_000);
        let l = left(&x);
        let rms = (l.iter().map(|v| v * v).sum::<f64>() / l.len() as f64).sqrt();
        let dbfs = 20.0 * (rms * 2f64.sqrt()).log10();
        assert!((dbfs + 20.0).abs() < 0.3, "colour {color}: RMS {dbfs:.2} dBFS AES17, want −20");
        let s = slope_db_per_octave(&l);
        assert!((s - want).abs() <= tol, "colour {color}: the density falls {s:.2} dB/octave, want {want}±{tol}");
        assert!(x.chunks_exact(2).all(|c| c[0] == c[1]), "stereo off: L = R");
    }
    let a = read("audio_noise", &[1.0, -20.0, 1.0, 0.0], 0, 4096);
    let b = read("audio_noise", &[1.0, -20.0, 2.0, 0.0], 0, 4096);
    assert_ne!(a, b, "another seed gives other bytes");
    let st = read("audio_noise", &[0.0, -20.0, 1.0, 1.0], 0, 96_000);
    let (l, r): (Vec<f64>, Vec<f64>) = st.chunks_exact(2).map(|c| (c[0] as f64, c[1] as f64)).unzip();
    let dot = l.iter().zip(&r).map(|(a, b)| a * b).sum::<f64>();
    let corr = dot / (l.iter().map(|v| v * v).sum::<f64>() * r.iter().map(|v| v * v).sum::<f64>()).sqrt();
    assert!(corr.abs() < 0.05, "stereo on: the channels are decorrelated ({corr:.3})");
}

#[test]
fn t1159_sweep_follows_its_curve_without_phase_jumps() {
    let (f0, f1, span) = (100.0f64, 6400.0f64, 6.0f64);
    let amp = 10f64.powf(-20.0 / 20.0);
    for (curve, name) in [(0.0, "log"), (1.0, "linear")] {
        let freq = |t: f64| if curve < 0.5 { f0 * (f1 / f0).powf(t / span) } else { f0 + (f1 - f0) * t / span };
        let cycles = |t: f64| {
            if curve < 0.5 {
                let k = f1 / f0;
                f0 * span / k.ln() * (k.powf(t / span) - 1.0)
            } else {
                f0 * t + (f1 - f0) * t * t / (2.0 * span)
            }
        };
        let x = read("sweep", &[f0, f1, span, curve, -20.0, 0.0], 0, (span * RATE) as usize + 4800);
        let l = left(&x);
        // the zero crossings in each 100 ms window are twice the cycles the curve integrates
        let w = 4800;
        for i in 0..(span * 10.0) as usize - 1 {
            let (a, b) = (i * w, (i + 1) * w);
            let z = crossings(&l[a..=b]) as f64;
            let want = 2.0 * (cycles(b as f64 / RATE) - cycles(a as f64 / RATE));
            assert!((z - want).abs() <= 2.0, "{name}: window {i}: {z} crossings, the curve says {want:.1}");
        }
        // no step inside one sweep is larger than the sine's own slope at that frequency
        for n in 0..(span * RATE) as usize - 1 {
            let limit = std::f64::consts::TAU * freq((n + 1) as f64 / RATE) / RATE * amp * 1.02 + 1e-6;
            assert!((l[n + 1] - l[n]).abs() <= limit, "{name}: a phase jump at sample {n}");
        }
        // it repeats after its duration
        let len = (span * RATE) as usize;
        assert_eq!(&l[len..len + 4800], &l[..4800], "{name}: the sweep repeats");
    }
}

#[test]
fn t1160_beep_is_the_gated_tone_and_silence_is_zeros() {
    let beep = read("beep", &[1000.0, -20.0, 1.0, 40.0, 0.0], 0, 3 * 48_000);
    let tone = read("tone", &[1000.0, -20.0, 0.0], 0, 3 * 48_000);
    for (n, (b, t)) in beep.chunks_exact(2).zip(tone.chunks_exact(2)).enumerate() {
        if n % 48_000 < 1920 {
            assert_eq!(b, t, "sample {n}: the pip is the tone");
        } else {
            assert_eq!(b, [0.0, 0.0], "sample {n}: silent between pips");
        }
    }
    // from any place: a burst does not depend on what was read before it
    let late = read("beep", &[1000.0, -20.0, 1.0, 40.0, 0.0], 47_104, 2048);
    assert_eq!(late, beep[47_104 * 2..(47_104 + 2048) * 2]);
    let s = read("silence", &[], 12_345, 4096);
    assert!(s.iter().all(|v| *v == 0.0), "silence is zeros");
}
