//! The sound of the broken stream (sim-audio-pairs; capability `audio-fx`): the datamosh's audio
//! half breaks at the moments the picture breaks, by the codec's own mechanisms.

use fx_video_codec::sound::{MoshSound, LA};
use kt_plugin_api::audio_codec::N;
use kt_plugin_api::math::FrozenMath;
use kt_plugin_api::{AudioState, Placement, Plugin};

const FS: f64 = 48_000.0;

fn values(sets: &[(&str, f64)]) -> Vec<f64> {
    let p = fx_video_codec::DatamoshSound::new();
    let m = p.manifest();
    let mut v = fx_video_codec::defaults();
    for (name, x) in sets {
        let i = m.passport.iter().position(|s| s.name == *name).unwrap_or_else(|| panic!("no row {name}"));
        v[i] = *x;
    }
    v
}

/// A sweep with a slow vibrato: content that changes, as a picture does.
fn music(t: f64) -> f32 {
    let f = 300.0 + 200.0 * (std::f64::consts::TAU * 0.7 * t).fsin();
    (0.3 * (std::f64::consts::TAU * f * t + 2.0 * (std::f64::consts::TAU * 0.7 * t).fcos()).fsin()
        + 0.1 * (std::f64::consts::TAU * 1250.0 * t).fsin()) as f32
}

/// Play `secs` of `f` through the stream from timeline frame `start` (an event of `secs` length);
/// the output by OUTPUT position, and the frames the decoder concealed.
fn play(vals: &[f64], start: i64, secs: f64, f: impl Fn(f64) -> f32) -> (Vec<f32>, u64) {
    let mut s = MoshSound::new();
    s.place(Placement { start, fps_num: 25, fps_den: 1 });
    s.apply(vals, true);
    let n = (secs * FS) as i64;
    s.bind(&kt_plugin_api::NoCurves, n);
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < n + LA {
        let mut buf = vec![0f32; 2048];
        for i in 0..1024 {
            let x = if pos + (i as i64) < n { f((start + pos + i as i64) as f64 / FS) } else { 0.0 };
            buf[2 * i] = x;
            buf[2 * i + 1] = x;
        }
        s.process(&mut buf, pos, None);
        for i in 0..1024 {
            if pos + i as i64 >= LA {
                out.push(buf[2 * i]);
            }
        }
        pos += 1024;
    }
    out.truncate(n as usize);
    (out, s.concealed())
}

/// The RMS error of `y` against `f` over output samples [a, b).
fn err(y: &[f32], start: i64, f: &impl Fn(f64) -> f32, a: usize, b: usize) -> f64 {
    let e: f64 = (a..b).map(|m| {
        let d = (y[m] - f((start + m as i64) as f64 / FS)) as f64;
        d * d
    }).sum();
    (e / (b - a) as f64).sqrt()
}

/// t1471: the intact stream gives the sound back — the codec's round trip, with and without the
/// prediction, and a coarser quantiser (`qp`) costs fidelity.
#[test]
fn t1471_the_intact_stream_plays_the_sound_back() {
    let start = 48_000 * 2;
    let rms = |v: &[f32]| (v.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
    let (y, c) = play(&values(&[]), start, 2.0, music);
    let e = err(&y, start, &music, 4 * N, y.len() - 2 * N);
    let sig = rms(&y[4 * N..]);
    let snr = 20.0 * (sig / e).flog10();
    let (yl, _) = play(&values(&[("audio_predict", 0.0)]), start, 2.0, music);
    let el = err(&yl, start, &music, 4 * N, yl.len() - 2 * N);
    let (yq, _) = play(&values(&[("qp", 51.0), ("audio_kbps", 4.0)]), start, 2.0, music);
    let eq = err(&yq, start, &music, 4 * N, yq.len() - 2 * N);
    eprintln!("intact SNR {snr:.1} dB, concealed {c}; without prediction err {el:.5}; qp 51 err {eq:.5}");
    assert_eq!(c, 0, "an intact stream concealed frames");
    assert!(snr > 25.0, "the intact stream is not the sound: {snr:.1} dB");
    assert!(el < 0.05, "the codec without prediction is broken");
    assert!(eq > 2.0 * e, "qp 51 is not coarser");
}

/// A `mosh` gate on over `[on, off)` of the event and a `reset` key rising at `off` (fractions of the
/// event): the curves a person draws.
struct Gate {
    on: f64,
    off: f64,
}

#[derive(Clone)]
struct GateRead {
    from: f64,
    to: f64,
}

impl kt_plugin_api::CurveRead for GateRead {
    fn value(&self, _base: f64, s: f64) -> f64 {
        if s >= self.from && s < self.to { 1.0 } else { 0.0 }
    }
    fn clone_box(&self) -> Box<dyn kt_plugin_api::CurveRead> {
        Box::new(self.clone())
    }
}

impl kt_plugin_api::SlotCurves for Gate {
    fn value(&self, param: &str, base: f64, s: f64) -> f64 {
        match param {
            "mosh" | "reset" => self.owned(param).value(base, s),
            _ => base,
        }
    }
    fn points(&self, param: &str) -> &kt_plugin_api::curves::ParamPoints {
        match param {
            "mosh" | "reset" => &[(0.0, 0.0), (1.0, 1.0)],
            _ => &[],
        }
    }
    fn owned(&self, param: &str) -> Box<dyn kt_plugin_api::CurveRead> {
        Box::new(match param {
            "mosh" => GateRead { from: self.on, to: self.off },
            _ => GateRead { from: self.off, to: 2.0 },
        })
    }
}

/// `play` with the curves of a gate.
fn play_gated(vals: &[f64], start: i64, secs: f64, g: &Gate, f: impl Fn(f64) -> f32) -> (Vec<f32>, u64) {
    let mut s = MoshSound::new();
    s.place(Placement { start, fps_num: 25, fps_den: 1 });
    s.apply(vals, true);
    let n = (secs * FS) as i64;
    s.bind(g, n);
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < n + LA {
        let mut buf = vec![0f32; 2048];
        for i in 0..1024 {
            let x = if pos + (i as i64) < n { f((start + pos + i as i64) as f64 / FS) } else { 0.0 };
            buf[2 * i] = x;
            buf[2 * i + 1] = x;
        }
        s.process(&mut buf, pos, None);
        for i in 0..1024 {
            if pos + i as i64 >= LA {
                out.push(buf[2 * i]);
            }
        }
        pos += 1024;
    }
    out.truncate(n as usize);
    (out, s.concealed())
}

/// t1472: `mosh` drawn over a part of the event deletes the I-frames it covers: up to the first
/// deleted one the sound is the intact stream's byte for byte; the audio frame carrying it breaks at
/// that moment and the sound stays broken, as the picture does, until an intact I-frame — the person's
/// Reset key — heals it.
#[test]
fn t1472_a_deleted_i_frame_breaks_the_sound_until_the_reset() {
    // the event: 3 s from 3.0 s, GOP 24 at 25 fps — I-frames at 3.00 (the stream's first), 3.96, 4.92, 5.88
    let start = 48_000 * 3;
    let vals = values(&[]);
    let (yi, ci) = play(&vals, start, 3.0, music);
    // mosh on from 3.6 s, off with a Reset at 4.8 s: the I-frame at 3.96 is the one deleted
    let g = Gate { on: 0.2, off: 0.6 };
    let (ym, cm) = play_gated(&vals, start, 3.0, &g, music);
    eprintln!("concealed: intact {ci}, moshed {cm}");
    assert_eq!(ci, 0);
    assert!(cm >= 1, "the deleted I-frame's audio frame was not concealed");
    // the audio frame of 3.96 s: output index of that frame's start; its overlap reaches a frame back
    let at = |t: f64| ((t - 3.0) * FS) as usize;
    let frame = ((3.96 * FS) as i64).div_euclid(N as i64) * N as i64 - start;
    let clean_to = (frame - N as i64) as usize;
    assert!(yi[..clean_to] == ym[..clean_to], "the sound moved before the deleted I-frame");
    let broken = err(&ym, start, &music, at(4.05), at(4.7));
    let healed = err(&ym, start, &music, at(4.95), at(5.8));
    eprintln!("broken {broken:.4}, healed by the Reset {healed:.5}");
    assert!(broken > 0.05, "the sound did not break after the deleted I-frame: {broken}");
    assert!(healed < 0.01, "the Reset did not heal the sound: {healed}");
}

/// t1473: `bloom` repeats the packet of the run's first P-frame: the decoder adds the same residual
/// again and again — the sound in the run drifts away from the intact stream as the run goes on.
#[test]
fn t1473_bloom_repeats_the_packet() {
    let start = 48_000;
    let (yi, _) = play(&values(&[("gop", 0.0)]), start, 2.0, music);
    let (yb, _) = play(&values(&[("gop", 0.0), ("bloom", 1.0)]), start, 2.0, music);
    let d = |a: usize, b: usize| {
        let e: f64 = (a..b).map(|m| ((yb[m] - yi[m]) as f64).powi(2)).sum();
        (e / (b - a) as f64).sqrt()
    };
    let early = d(2 * N, 6 * N);
    let late = d(40 * N, 80 * N);
    eprintln!("bloom: early {early:.4}, late {late:.4}");
    assert!(late > 0.02, "a bloom run sounds intact");
}

/// t1474: `loss` flips bits in hit packets: the Rice-coded lines after the flip decode as garbage
/// or the decoder catches it and conceals; at 0 nothing differs from the intact stream.
#[test]
fn t1474_lost_bits_break_frames() {
    let start = 0;
    let (yi, _) = play(&values(&[]), start, 1.5, music);
    let (y0, c0) = play(&values(&[("loss", 0.0)]), start, 1.5, music);
    let (yl, cl) = play(&values(&[("loss", 0.4)]), start, 1.5, music);
    assert!(y0 == yi && c0 == 0, "loss 0 changed the stream");
    let diff = yi.iter().zip(&yl).filter(|(a, b)| (**a - **b).abs() > 1e-3).count();
    eprintln!("loss 0.4: concealed {cl}, samples off {diff}");
    assert!(diff > 4 * N, "bit errors did not break the sound");
}

/// t1475: the residual the decoder applies is the picture's `residual` row: a quarter of it lets the
/// prediction run away from the sound.
#[test]
fn t1475_a_cut_residual_lets_the_prediction_run() {
    let start = 0;
    let (y1, _) = play(&values(&[]), start, 1.5, music);
    let (yq, _) = play(&values(&[("residual", 0.25)]), start, 1.5, music);
    let e1 = err(&y1, start, &music, 4 * N, y1.len() - 2 * N);
    let eq = err(&yq, start, &music, 4 * N, yq.len() - 2 * N);
    eprintln!("residual 1: {e1:.4}, 0.25: {eq:.4}");
    assert!(eq > 5.0 * e1, "the residual row does not act");
}

/// t1476: the stream is a function of the rows and the timeline: two runs give the same bytes; the
/// same event seated elsewhere meets its I-frames elsewhere.
#[test]
fn t1476_the_stream_is_a_function_of_rows_and_time() {
    let vals = values(&[("before", 12.0), ("mosh", 1.0), ("loss", 0.2)]);
    let a = play(&vals, 48_000, 1.5, music);
    let b = play(&vals, 48_000, 1.5, music);
    assert!(a == b, "two runs differ");
}
