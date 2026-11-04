//! The sound of the tape (sim-audio-pairs; capability `audio-fx`): what the deck's physics predicts
//! for the audio half of `vhs_tape`, and that a defect of the deck is heard where the picture shows
//! it — the same field of the same tape frame.

use fx_video_tape::deck::Deck;
use fx_video_tape::sim;
use fx_video_tape::sound::{self, tape_pos, TapeSound, LA};
use kt_plugin_api::math::FrozenMath;
use kt_plugin_api::{AudioState, Placement, Plugin};

/// The passport values with `sets` applied by row name.
fn values(sets: &[(&str, f64)]) -> Vec<f64> {
    let p = fx_video_tape::VhsTapeSound::new();
    let m = p.manifest();
    let mut v = fx_video_tape::defaults();
    for (name, x) in sets {
        let i = m.passport.iter().position(|s| s.name == *name).unwrap_or_else(|| panic!("no row {name}"));
        v[i] = *x;
    }
    v
}

const FS: f64 = 48_000.0;

/// Play `secs` of `f(t)` (the same on both channels) through the deck; the output by OUTPUT
/// position (the state's look-ahead undone), from timeline frame `start`.
fn play(vals: &[f64], start: i64, secs: f64, f: impl Fn(f64) -> f32) -> Vec<[f32; 2]> {
    let mut s = TapeSound::new();
    s.place(Placement { start, fps_num: 25, fps_den: 1 });
    s.apply(vals, true);
    let n = (secs * FS) as i64;
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < n + LA {
        let mut buf = vec![0f32; 2048];
        for i in 0..1024 {
            let x = f((start + pos + i as i64) as f64 / FS);
            buf[2 * i] = x;
            buf[2 * i + 1] = x;
        }
        s.process(&mut buf, pos, None);
        for i in 0..1024 {
            if pos + i as i64 >= LA {
                out.push([buf[2 * i], buf[2 * i + 1]]);
            }
        }
        pos += 1024;
    }
    out.truncate(n as usize);
    out
}

fn sine(hz: f64, amp: f32) -> impl Fn(f64) -> f32 {
    move |t| amp * (std::f64::consts::TAU * hz * t).fsin() as f32
}

/// The amplitude of `hz` in `v[from..to]` (least squares, channel `ch`).
fn tone(v: &[[f32; 2]], from: usize, to: usize, hz: f64, ch: usize) -> f64 {
    let (mut c, mut s) = (0.0, 0.0);
    for (j, x) in v[from..to].iter().enumerate() {
        let ph = std::f64::consts::TAU * hz * (from + j) as f64 / FS;
        c += x[ch] as f64 * ph.fcos();
        s += x[ch] as f64 * ph.fsin();
    }
    2.0 * (c * c + s * s).sqrt() / (to - from) as f64
}

/// The RMS error of output `y` against the input `f` while head A, and while head B, reads the
/// tape (the picture's switch: from line `h/2 − switch_lines` of a field the next head plays),
/// leaving out `guard` s after each switch (the compander's recovery), and the first `skip` s.
fn head_err(vals: &[f64], start: i64, y: &[[f32; 2]], f: &impl Fn(f64) -> f32, skip: f64, guard: f64) -> [f64; 2] {
    let d = Deck::new(vals);
    let hf = (d.st.h / 2) as f64;
    let sp = vals[fx_video_tape::deck::row::SWITCH_POINT];
    let t_sw = (hf - sp) * d.st.t_line;
    let field_t = 0.5 / d.st.frame_rate;
    let mut e = [0f64; 2];
    let mut k = [0usize; 2];
    for (m, o) in y.iter().enumerate().skip((skip * FS) as usize) {
        let t = (start + m as i64) as f64 / FS;
        let p = tape_pos(&d, t);
        let tau = p.line as f64 * d.st.t_line + p.in_line;
        let (head, since) = if tau >= t_sw { (1 - p.field, tau - t_sw) } else { (p.field, tau + field_t - t_sw) };
        if since < guard {
            continue;
        }
        let r = (o[0] - f(t)) as f64;
        e[head as usize] += r * r;
        k[head as usize] += 1;
    }
    [(e[0] / k[0] as f64).sqrt(), (e[1] / k[1] as f64).sqrt()]
}

/// t1462: a clogged head A loses the picture's field A and the Hi-Fi sound while head A reads — the
/// same field, from the same switch point — while head B's field stays clean in both.
#[test]
fn t1462_a_clogged_head_is_seen_and_heard_in_the_same_field() {
    let clean = values(&[]);
    let clog = values(&[("rec_a_spacing", 3.0)]);
    // the picture (the tape's own grid, one frame)
    let st = Deck::new(&clean).st;
    let mut pic = vec![0u8; (st.a * st.h * 4) as usize];
    for (i, p) in pic.chunks_exact_mut(4).enumerate() {
        let x = (i as u32 % st.a) / 60 % 2;
        let l = (90 + 77 * x) as u8;
        p.copy_from_slice(&[l, l, l, 255]);
    }
    let field_rms = |vals: &[f64]| {
        let s = sim::simulate(&Deck::new(vals), pic.clone(), 0);
        let (m, a, w) = (s.uni.m as usize, s.uni.a as usize, s.uni.w as usize);
        let mut e = [0f64; 2];
        for y in 0..(st.h as usize - 24) {
            for n in (m + 150)..(m + a - 40) {
                let d = (s.luma[y * w + n] - sim::luma_at(&s.rgb, &s.uni, n as i32, y)) as f64;
                e[y % 2] += d * d;
            }
        }
        e
    };
    let pc = field_rms(&clean);
    let pk = field_rms(&clog);
    // the sound: a 1 kHz tone at −12 dBFS
    let f = sine(1000.0, 0.25);
    let start = 48_000 * 3;
    let ac = head_err(&clean, start, &play(&clean, start, 1.0, &f), &f, 0.3, 0.003);
    let ak = head_err(&clog, start, &play(&clog, start, 1.0, &f), &f, 0.3, 0.003);
    eprintln!("picture clean {pc:?} clogged {pk:?}; sound clean {ac:?} clogged {ak:?}");
    // the picture loses field A (even rows), keeps field B
    assert!(pk[0] > 8.0 * pk[1], "the picture's field A is not lost: {pk:?}");
    // the sound is lost while head A reads — field A, from the switch before it — and plays while
    // head B reads: the defect beats at the head's fields, as the picture's
    assert!(ak[0] > 8.0 * ak[1].max(1e-3), "the sound is not lost while head A reads: {ak:?}");
    assert!(ak[1] < 3.0 * ac[1].max(0.01), "head B's sound was hurt: {ak:?} against {ac:?}");
    // and a clean deck plays the tone back (the Hi-Fi round trip)
    assert!(ac.iter().all(|e| *e < 0.05), "the clean Hi-Fi round trip is off: {ac:?}");
}

/// t1463: an oxide defect of the picture's dropout field cuts the Hi-Fi carrier at its line: with
/// the dropout compensator off, the sound clicks at exactly the lines the picture loses.
#[test]
fn t1463_a_dropout_is_a_click_at_its_line() {
    let vals = values(&[("dropouts_per_s", 40.0), ("dropout_us", 40.0), ("doc", 0.0)]);
    let d = Deck::new(&vals);
    let f = sine(700.0, 0.25);
    let start = 0;
    let y = play(&vals, start, 1.0, &f);
    // the lines with a dropout, from the picture's own line table
    let mut hits = 0;
    let mut quiet = 0;
    let mut drops = 0;
    let mut m = (0.25 * FS) as usize;
    while m < y.len() - 64 {
        let t = m as f64 / FS;
        let p = tape_pos(&d, t);
        let hf = d.st.h / 2;
        let line_has = |pp: sound::TapePos| {
            if pp.line >= hf {
                return false;
            }
            let c = sound::frame_words(&d, pp.frame);
            sound::line_read(&d, &c, 2 * pp.line + pp.field).drop_len > 0.0
        };
        let err = (y[m][0] - f(t)).abs();
        if line_has(p) {
            drops += 1;
            if err > 0.02 {
                hits += 1;
            }
        } else if err < 0.02 {
            quiet += 1;
        }
        m += 1;
    }
    let total = y.len() - (0.25 * FS) as usize - 64 - drops;
    eprintln!("samples on a dropout line {drops}, of them clicked {hits}; quiet elsewhere {quiet}/{total}");
    assert!(drops > 20, "too few dropouts to judge: {drops}");
    assert!(hits * 2 > drops, "the sound does not click on the picture's dropout lines: {hits}/{drops}");
    assert!(quiet as f64 > 0.95 * total as f64, "the sound is broken off the dropout lines");
}

/// t1464: the linear track loses its top at LP: the wavelength halves, the gap, spacing and
/// thickness losses grow — a 6 kHz tone falls 6 dB more than a 500 Hz one.
#[test]
fn t1464_lp_dulls_the_linear_track() {
    let sp = values(&[("audio_out", 1.0), ("cn_db", 60.0), ("jitter_us", 0.0), ("dropouts_per_s", 0.0)]);
    let lp = values(&[("audio_out", 1.0), ("cn_db", 60.0), ("jitter_us", 0.0), ("dropouts_per_s", 0.0), ("speed", 1.0)]);
    let level = |vals: &[f64], hz: f64| {
        let y = play(vals, 0, 0.5, sine(hz, 0.1));
        20.0 * (tone(&y, 4800, y.len(), hz, 0) / 0.1).flog10()
    };
    let (s6, l6) = (level(&sp, 6000.0), level(&lp, 6000.0));
    let (s5, l5) = (level(&sp, 500.0), level(&lp, 500.0));
    eprintln!("6 kHz SP {s6:.2} LP {l6:.2} dB; 500 Hz SP {s5:.2} LP {l5:.2} dB");
    // the coating's thickness loss reaches down to the low band at LP's wavelengths (a few dB);
    // the top falls far more
    assert!(s5.abs() < 1.5 && (l5 - s5).abs() < 3.0, "the low band moved");
    assert!((s6 - l6) - (s5 - l5) > 6.0, "LP's top did not fall more than its low band");
}

/// t1465: the linear track's wow and flutter is the picture's capstan: the delay of a tone swings
/// at 5.3 Hz in the phase of the picture's flutter term, as wide as the jitter row says.
#[test]
fn t1465_the_capstan_wobbles_the_linear_track() {
    let base = [("audio_out", 1.0), ("cn_db", 60.0), ("dropouts_per_s", 0.0)];
    let mut sets = base.to_vec();
    sets.push(("jitter_us", 0.3));
    let vals = values(&sets);
    let hz = 1000.0;
    let y = play(&vals, 0, 2.0, sine(hz, 0.1));
    // the delay per 10 ms window from the tone's phase
    let win = 480;
    let mut delays = Vec::new();
    let mut m = 4800;
    while m + win < y.len() {
        let (mut c, mut s) = (0.0, 0.0);
        for (j, o) in y.iter().enumerate().skip(m).take(win) {
            let ph = std::f64::consts::TAU * hz * j as f64 / FS;
            c += o[0] as f64 * ph.fcos();
            s += o[0] as f64 * ph.fsin();
        }
        // y ≈ a·sin(ω(t − τ)): phase −ωτ
        let phase = (-c).fatan2(s);
        let t = (m + win / 2) as f64 / FS;
        delays.push((t, phase / (std::f64::consts::TAU * hz) * FS));
        m += win;
    }
    let mean = delays.iter().map(|d| d.1).sum::<f64>() / delays.len() as f64;
    let (mut num, mut den_a, mut den_b) = (0.0, 0.0, 0.0);
    for (t, d) in &delays {
        let want = (std::f64::consts::TAU * (5.3 * t + 0.37)).fsin();
        num += (d - mean) * want;
        den_a += (d - mean) * (d - mean);
        den_b += want * want;
    }
    let corr = num / (den_a * den_b).sqrt();
    let amp = num / den_b;
    eprintln!("corr {corr:.3}, amplitude {amp:.2} samples (want ≈ {:.2})", 0.3 * 43.2);
    assert!(corr > 0.9, "the delay does not follow the capstan's flutter: {corr:.3}");
    assert!((amp - 0.3 * 43.2).abs() < 0.25 * 0.3 * 43.2, "the flutter's width is off: {amp:.2}");
}

/// t1466: the sound is a function of the rows and the timeline: two runs give the same bytes, and a
/// run seated elsewhere on the timeline reads other fields (other noise).
#[test]
fn t1466_the_sound_is_a_function_of_rows_and_time() {
    let vals = values(&[("rec_b_spacing", 0.6), ("dropouts_per_s", 20.0)]);
    let f = sine(440.0, 0.3);
    let a = play(&vals, 1000, 0.3, &f);
    let b = play(&vals, 1000, 0.3, &f);
    assert!(a == b, "two runs differ");
    let c = play(&vals, 1000 + 4801, 0.3, &f);
    assert!(a != c, "a run seated elsewhere gave the same noise");
}
