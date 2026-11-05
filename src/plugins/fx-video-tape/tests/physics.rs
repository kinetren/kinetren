//! The physical consequences of the VHS tape simulation (vhs-tape-simulation; capability
//! `video-fx`): each test asserts what the process predicts, not how the picture looks. They run on
//! the standard's grid directly (`sim::simulate` over an A×H picture): the host's resample is the
//! frame's business, not the tape's.

use fx_video_tape::deck::{lt, Deck, LT_W};
#[path = "model/sim.rs"]
mod sim;
use sim::Sim;
use fx_video_tape::standard::{Standard, Std, KN, KR};
use kt_plugin_api::math::FrozenMath;
use kt_plugin_api::Plugin;

trait LnSafe {
    fn ln_safe(self) -> f64;
}
impl LnSafe for f64 {
    /// The natural log through our frozen log10 (t1066 covers the crate's tests too).
    fn ln_safe(self) -> f64 {
        self.max(1e-12).flog10() * std::f64::consts::LN_10
    }
}

/// The passport values with `sets` applied by row name.
fn values(sets: &[(&str, f64)]) -> Vec<f64> {
    let p = fx_video_tape::VhsTape::new();
    let m = p.manifest();
    let mut v = fx_video_tape::defaults();
    for (name, x) in sets {
        let i = m.passport.iter().position(|s| s.name == *name).unwrap_or_else(|| panic!("no row {name}"));
        v[i] = *x;
    }
    v
}

/// Ideal heads on their tracks, almost no noise, no dropouts, no saturation, a TBC.
const CLEAN: &[(&str, f64)] = &[
    ("rec_a_spacing", 0.0),
    ("rec_b_spacing", 0.0),
    ("rec_a_gap", 0.1),
    ("rec_b_gap", 0.1),
    ("cn_db", 60.0),
    ("dropouts_per_s", 0.0),
    ("tape_saturation", 0.0),
    ("jitter_us", 0.0),
    ("tbc", 1.0),
];

fn with(base: &[(&str, f64)], more: &[(&str, f64)]) -> Vec<f64> {
    let mut s: Vec<(&str, f64)> = base.to_vec();
    s.extend_from_slice(more);
    values(&s)
}

/// An A×H picture of the standard from a function of (active column, row) → RGB.
fn picture(st: &Standard, f: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
    let mut v = vec![0u8; (st.a * st.h * 4) as usize];
    for y in 0..st.h {
        for x in 0..st.a {
            let c = f(x, y);
            let i = ((y * st.a + x) * 4) as usize;
            v[i..i + 3].copy_from_slice(&c);
            v[i + 3] = 255;
        }
    }
    v
}

fn run(vals: &[f64], pic: &[u8], t: i64) -> Sim {
    let d = Deck::new(vals);
    sim::simulate(&d, pic.to_vec(), t)
}

fn grey(l: f64) -> [u8; 3] {
    let b = (l * 255.0).round() as u8;
    [b, b, b]
}

/// The luma the picture put in at grid sample (n, y).
fn input_luma(s: &Sim, n: i32, y: usize) -> f32 {
    sim::luma_at(&s.rgb, &s.uni, n, y)
}

/// RMS of the output luma against the input over the active part of a row. The first 150 samples
/// are left out: the step from blanking into the picture is pre-emphasised into the white clip, and
/// the clip's error decays through the de-emphasis (τ₁ = 1.3 µs, ~120 samples) — VHS's own streak
/// after a hard edge, not noise.
fn row_err(s: &Sim, y: usize) -> f64 {
    let (m, a, w) = (s.uni.m as i32, s.uni.a as i32, s.uni.w as usize);
    let mut e = 0.0;
    let mut k = 0;
    for n in m + 150..m + a - 40 {
        let d = (s.luma[y * w + n as usize] - input_luma(s, n, y)) as f64;
        e += d * d;
        k += 1;
    }
    (e / k as f64).sqrt()
}

/// The amplitude of a sinusoid of `cyc` cycles per sample in a row of `v` (least squares).
fn tone(v: &[f32], from: usize, to: usize, cyc: f64) -> f64 {
    let (mut c, mut s) = (0.0, 0.0);
    let mean: f64 = v[from..to].iter().map(|x| *x as f64).sum::<f64>() / (to - from) as f64;
    for (j, x) in v[from..to].iter().enumerate() {
        let ph = std::f64::consts::TAU * cyc * (from + j) as f64;
        c += (*x as f64 - mean) * ph.fcos();
        s += (*x as f64 - mean) * ph.fsin();
    }
    2.0 * (c * c + s * s).sqrt() / (to - from) as f64
}

/// t1180: without impairments the FM round trip gives back the luma, and the carrier is the
/// standard's.
#[test]
fn t1180_the_fm_round_trip_gives_back_the_luma() {
    for std in [0.0, 1.0] {
        let vals = with(CLEAN, &[("standard", std)]);
        let st = Deck::new(&vals).st;
        // bars of eight levels, ~115 samples each
        let pic = picture(st, |x, _| grey(0.05 + 0.9 * ((x * 8 / st.a) as f64) / 7.0));
        let s = run(&vals, &pic, 0);
        let w = st.w as usize;
        let seg = st.a as i32 / 8;
        let mut worst = 0f64;
        for y in (0..st.h as usize).step_by(7) {
            for b in 0..8 {
                for n in st.m as i32 + b * seg + 30..st.m as i32 + (b + 1) * seg - 30 {
                    let d = (s.luma[y * w + n as usize] - input_luma(&s, n, y)).abs() as f64;
                    worst = worst.max(d);
                }
            }
        }
        assert!(worst < 0.015, "{std}: the round trip is off by {worst:.4} of white");
        // the written carrier: zero crossings of the magnetisation over a white line
        let s = run(&vals, &picture(st, |_, _| grey(0.95)), 0);
        let y = 100usize;
        let (a0, a1) = (st.m as usize + 20, (st.m + st.a) as usize - 20);
        let row = &s.m[y * w..(y + 1) * w];
        let cross = (a0..a1).filter(|&n| (row[n] >= 0.0) != (row[n + 1] >= 0.0)).count();
        let f = cross as f64 / 2.0 / ((a1 - a0) as f64 / st.fs);
        let want = st.f_sync + (sim::luma_at(&s.rgb, &s.uni, st.m as i32 + 5, y) as f64 - st.l_sync) / (1.0 - st.l_sync) * (st.f_white - st.f_sync);
        assert!((f / want - 1.0).abs() < 0.01, "{std}: the carrier is {f:.0} Hz, the standard says {want:.0}");
    }
}

/// t1181: the colour-under keeps the luma's bandwidth and cuts the chroma's.
#[test]
fn t1181_colour_under_cuts_the_chroma_not_the_luma() {
    let vals = with(CLEAN, &[]);
    let st = Deck::new(&vals).st;
    let w = st.w as usize;
    let resp = |chroma: bool, f: f64| -> f64 {
        let cyc = f / st.fs;
        let pic = picture(st, |x, _| {
            let s = (std::f64::consts::TAU * cyc * (x + st.m) as f64).fsin();
            if chroma {
                // U swings, Y stays 0.5
                let (yv, u) = (0.5, 0.12 * s);
                let b = yv + u / 0.492;
                let g = (yv - 0.299 * yv - 0.114 * b) / 0.587;
                [(yv * 255.0) as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8]
            } else {
                grey(0.5 + 0.3 * s)
            }
        });
        let s = run(&vals, &pic, 0);
        let y = 200;
        let v = if chroma { &s.u[y * w..(y + 1) * w] } else { &s.luma[y * w..(y + 1) * w] };
        tone(v, st.m as usize + 80, (st.m + st.a) as usize - 80, cyc)
    };
    let (l0, l1) = (resp(false, 0.2e6), resp(false, 1.0e6));
    let (c0, c1) = (resp(true, 0.2e6), resp(true, 1.0e6));
    let db = |a: f64, b: f64| 20.0 * (a / b).flog10();
    eprintln!("luma 1 MHz {:.1} dB, chroma 1 MHz {:.1} dB", db(l1, l0), db(c1, c0));
    assert!(db(l1, l0) > -3.0, "the luma at 1 MHz is {:.1} dB", db(l1, l0));
    assert!(db(c1, c0) < -12.0, "the chroma at 1 MHz is only {:.1} dB down", db(c1, c0));
}

/// The noise of a run: the mean row error of even and of odd rows.
fn field_err(s: &Sim) -> (f64, f64) {
    let h = s.uni.h as usize - 2 * SWITCH_ROWS;
    let (mut e, mut o) = (0.0, 0.0);
    for y in 0..h {
        if y & 1 == 0 { e += row_err(s, y) } else { o += row_err(s, y) }
    }
    (e / (h / 2) as f64, o / (h / 2) as f64)
}

fn flat() -> impl Fn(u32, u32) -> [u8; 3] {
    |x, _| grey(0.35 + 0.3 * ((x / 60) % 2) as f64)
}

/// t1182: a clogged head kills every other field.
#[test]
fn t1182_a_clogged_head_kills_every_other_field() {
    let clean = values(&[]);
    let st = Deck::new(&clean).st;
    let pic = picture(st, flat());
    let (ce, co) = field_err(&run(&clean, &pic, 0));
    let (e, o) = field_err(&run(&values(&[("rec_a_spacing", 3.0)]), &pic, 0));
    eprintln!("clean {ce:.4}/{co:.4}, clogged A {e:.4}/{o:.4}");
    assert!(e > 8.0 * o, "field A {e:.4} is not noise against field B {o:.4}");
    assert!(o < 1.5 * co.max(0.004), "field B {o:.4} was hurt (clean {co:.4})");
}

/// Field rows left out at the bottom: the head switches there (its noise is not the tape's).
const SWITCH_ROWS: usize = 12;

/// The error per field-A row, smoothed over 9 rows (the switching rows left out).
fn profile(s: &Sim) -> Vec<f64> {
    let h = s.uni.h as usize;
    let raw: Vec<f64> = (0..h / 2 - SWITCH_ROWS).map(|yf| row_err(s, yf * 2)).collect();
    (0..raw.len())
        .map(|i| {
            let (a, b) = (i.saturating_sub(4), (i + 5).min(raw.len()));
            raw[a..b].iter().sum::<f64>() / (b - a) as f64
        })
        .collect()
}

/// t1183: the tracking knob moves the noise bar by what the geometry predicts.
#[test]
fn t1183_the_tracking_knob_moves_the_noise_bar() {
    let skew = 60.0;
    let st = Standard::get(Std::Pal);
    let pic = picture(st, flat());
    let prof = |tracking: f64| -> Vec<f64> {
        let s = run(&values(&[("decks", 1.0), ("skew_um", skew), ("tracking_um", tracking), ("cn_db", 40.0)]), &pic, 0);
        profile(&s).iter().map(|v| v.ln_safe()).collect()
    };
    let (p1, p2) = (prof(-10.0), prof(10.0));
    // the noise per row is a function of the head's offset o(u) = tracking + skew·(u − ½), so a
    // change of tracking TRANSLATES the profile: find the shift that aligns them
    let n = p1.len() as i64;
    let mut best = (0i64, f64::MAX);
    for sft in -(n / 2)..=(n / 2) {
        let (mut e, mut k) = (0.0, 0);
        for i in 0..n {
            let j = i + sft;
            if (0..n).contains(&j) {
                e += (p1[i as usize] - p2[j as usize]).abs();
                k += 1;
            }
        }
        if k > n / 3 && e / (k as f64) < best.1 {
            best = (sft, e / k as f64);
        }
    }
    let moved = best.0 as f64 / n as f64;
    let (max, min) = (p1.iter().cloned().fold(f64::MIN, f64::max).exp(), p1.iter().cloned().fold(f64::MAX, f64::min).fexp());
    eprintln!("the bar moved {moved:.3} of a field, contrast {:.1}×", max / min);
    assert!(max > 4.0 * min, "no bar: the noise per row varies only {:.1}×", max / min);
    let want = -20.0 / skew;
    assert!((moved - want).abs() < 0.06, "the bar moved {moved:.3} of a field, the geometry says {want:.3}");
    // centred, no skew: no bar
    let s = run(&values(&[("cn_db", 40.0)]), &pic, 0);
    let p = profile(&s);
    let (mx, mn) = (p.iter().cloned().fold(0.0, f64::max), p.iter().cloned().fold(f64::MAX, f64::min));
    assert!(mx < 2.0 * mn, "a centred head shows a bar ({:.1}×)", mx / mn);
}

/// The transfer of a symmetric kernel at `cyc` cycles per sample.
fn transfer(k: &[f32], cyc: f64) -> f64 {
    k.iter().enumerate().map(|(i, v)| *v as f64 * (std::f64::consts::TAU * cyc * (i as f64 - KR as f64)).fcos()).sum::<f64>().abs()
}

/// t1184: azimuth rejects the neighbour's FM, lets its colour-under leak, and the comb cancels it.
#[test]
fn t1184_azimuth_rejects_fm_the_comb_cancels_the_chroma_leak() {
    let st = Standard::get(Std::Pal);
    // a head partly on the neighbour: offset by 20 µm, a 32 µm head on a 49 µm track
    let kernels = |tracking: f64| {
        let d = Deck::new(&with(CLEAN, &[("tracking_um", tracking)]));
        let c = d.words(0);
        let mut row = vec![0f32; LT_W];
        fx_video_tape::deck::line_row(&c, 100, &mut row);
        (row, fx_video_tape::deck::row_geo(&c, 100))
    };
    let vals = with(CLEAN, &[("tracking_um", 12.0)]);
    let d = Deck::new(&vals);
    let c = d.words(0);
    let mut row = vec![0f32; LT_W];
    fx_video_tape::deck::line_row(&c, 100, &mut row);
    let g = fx_video_tape::deck::row_geo(&c, 100);
    let kx = &row[fx_video_tape::deck::LT_SCALARS + KN..];
    let ko = &row[fx_video_tape::deck::LT_SCALARS..fx_video_tape::deck::LT_SCALARS + KN];
    let strip_um = g.c1w as f64 * st.pitch_sp * 1e6;
    // the box the tilted gap integrates over: the strip along w·tan 12°, plus the gap's own transit
    let box_um = strip_um * 0.212_556_56 + 0.1;
    for f in [4.3e6, 0.627e6] {
        let x = std::f64::consts::PI * box_um * 1e-6 * f / st.v_write;
        let sinc = (x.fsin() / x).abs();
        let got = transfer(kx, f / st.fs) / transfer(ko, f / st.fs);
        let (g_db, w_db) = (20.0 * got.flog10(), 20.0 * sinc.flog10());
        eprintln!("{:.2} MHz: neighbour/own {g_db:.1} dB, sinc law {w_db:.1} dB (strip {strip_um:.1} µm)", f / 1e6);
        assert!((g_db - w_db).abs() < 2.0, "at {f} Hz the azimuth loss is {g_db:.1} dB, the law says {w_db:.1}");
    }
    // the head wholly over the neighbour (a 32 µm strip): the FM is rejected far more than the
    // colour-under
    let (full, gf) = kernels(49.0);
    assert!((gf.c1w as f64 * st.pitch_sp * 1e6 - 32.0).abs() < 0.5, "the head is not wholly on the neighbour");
    let (fo, fx) = (&full[fx_video_tape::deck::LT_SCALARS..fx_video_tape::deck::LT_SCALARS + KN], &full[fx_video_tape::deck::LT_SCALARS + KN..]);
    let rej = |f: f64| 20.0 * (transfer(fx, f / st.fs) / transfer(fo, f / st.fs)).flog10();
    eprintln!("whole strip: FM {:.1} dB, colour-under {:.1} dB", rej(4.3e6), rej(0.627e6));
    assert!(rej(4.3e6) < rej(0.627e6) - 12.0, "the FM is not rejected 12 dB more than the colour-under");

    // a picture whose fields differ: field A grey, field B saturated colour
    let pic = picture(st, |_, y| if y & 1 == 0 { [128, 128, 128] } else { [220, 40, 40] });
    let leak = |comb: f64| -> (f64, f64) {
        let s = run(&with(CLEAN, &[("tracking_um", 20.0), ("comb", comb)]), &pic, 0);
        let mut out = vec![0u8; 320 * 240 * 4];
        sim::read_out(&s, 320, 240, &mut out);
        // field A rows of the output (even grid rows map to even output rows at 576→240? use the grid)
        let w = s.uni.w as usize;
        let cd = s.uni.comb_rows as usize;
        let (mut cl, mut ll) = (0.0, 0.0);
        let mut k = 0;
        for y in (8..s.uni.h as usize - 8).step_by(2) {
            for n in (s.uni.m + 60) as usize..(s.uni.m + s.uni.a - 60) as usize {
                let u = if cd > 0 { 0.5 * (s.u[y * w + n] + s.u[(y - cd) * w + n]) } else { s.u[y * w + n] };
                let v = if cd > 0 { 0.5 * (s.v[y * w + n] + s.v[(y - cd) * w + n]) } else { s.v[y * w + n] };
                cl += (u * u + v * v) as f64;
                let dl = s.luma[y * w + n] - 128.0 / 255.0;
                ll += (dl * dl) as f64;
                k += 1;
            }
        }
        ((cl / k as f64).sqrt(), (ll / k as f64).sqrt())
    };
    let (c_off, l_off) = leak(0.0);
    let (c_on, _) = leak(1.0);
    eprintln!("chroma leak {c_off:.4} → {c_on:.4} with the comb; luma leak {l_off:.4}");
    assert!(l_off < c_off, "the neighbour's luma leaks more ({l_off:.4}) than its chroma ({c_off:.4})");
    assert!(20.0 * (c_on / c_off).flog10() < -10.0, "the comb cut the chroma leak only {:.1} dB", 20.0 * (c_on / c_off).flog10());
}

/// t1185: time is the only state — one position, one picture; positions in any order.
#[test]
fn t1185_time_is_the_only_state() {
    let vals = values(&[("dropouts_per_s", 50.0)]);
    let st = Deck::new(&vals).st;
    let pic = picture(st, flat());
    let frame = |t: i64| {
        let s = run(&vals, &pic, t);
        let mut o = vec![0u8; 200 * 150 * 4];
        sim::read_out(&s, 200, 150, &mut o);
        o
    };
    let ts = [0i64, 3600, 7200, 90_000 * 60];
    let fwd: Vec<Vec<u8>> = ts.iter().map(|t| frame(*t)).collect();
    let rev: Vec<Vec<u8>> = ts.iter().rev().map(|t| frame(*t)).collect();
    for (i, f) in fwd.iter().enumerate() {
        assert!(*f == rev[ts.len() - 1 - i], "position {}: the order of calls moved a byte", ts[i]);
    }
    assert!(fwd[0] == frame(0), "one position gave two pictures");
    assert!(fwd[0] != fwd[1], "two positions gave the same noise");
}

/// t1188: still on two heads shows a bar, on four heads it does not.
#[test]
fn t1188_still_on_two_heads_has_a_bar_on_four_none() {
    let st = Standard::get(Std::Pal);
    let pic = picture(st, flat());
    let contrast = |heads: f64| {
        let s = run(&values(&[("transport", 1.0), ("heads", heads), ("cn_db", 40.0)]), &pic, 0);
        let p = profile(&s);
        (p.iter().cloned().fold(0.0, f64::max) / p.iter().cloned().fold(f64::MAX, f64::min), s)
    };
    let (c2, _) = contrast(0.0);
    let (c4, s4) = contrast(1.0);
    eprintln!("still: two heads {c2:.1}×, four heads {c4:.1}×");
    assert!(c2 > 4.0, "two heads in still show no bar ({c2:.1}×)");
    assert!(c4 < 2.0, "four heads in still show a bar ({c4:.1}×)");
    for y in (1..st.h as usize).step_by(2) {
        assert_eq!(s4.lt[y * LT_W + lt::CONTENT] as usize, y - 1, "four-head still: row {y} is not field A's line");
    }
}

/// The output noise of a run over a flat picture (dB of white).
fn noise_db(vals: &[f64]) -> f64 {
    let st = Deck::new(vals).st;
    let pic = picture(st, |_, _| grey(0.5));
    let s = run(vals, &pic, 0);
    let h = st.h as usize;
    let e: f64 = (8..h - 2 * SWITCH_ROWS).map(|y| row_err(&s, y).powi(2)).sum::<f64>() / (h - 8 - 2 * SWITCH_ROWS) as f64;
    10.0 * e.flog10()
}

/// t1189: LP is noisier, not softer.
#[test]
fn t1189_lp_is_noisier_not_softer() {
    let (sp, lp) = (noise_db(&values(&[])), noise_db(&values(&[("speed", 1.0)])));
    eprintln!("noise SP {sp:.1} dB, LP {lp:.1} dB");
    assert!(lp > sp + 2.0, "LP is only {:.1} dB noisier", lp - sp);
    let st = Standard::get(Std::Pal);
    let cyc = 1.0e6 / st.fs;
    let pic = picture(st, |x, _| grey(0.5 + 0.3 * (std::f64::consts::TAU * cyc * (x + st.m) as f64).fsin()));
    let amp = |speed: f64| {
        let s = run(&with(CLEAN, &[("speed", speed)]), &pic, 0);
        let w = st.w as usize;
        tone(&s.luma[200 * w..201 * w], st.m as usize + 80, (st.m + st.a) as usize - 80, cyc)
    };
    let d = 20.0 * (amp(1.0) / amp(0.0)).flog10();
    assert!(d.abs() < 1.0, "LP changed the 1 MHz response by {d:.2} dB");
}

/// t1190: a defect of the recorder survives a clean player; a chipped head flickers.
#[test]
fn t1190_the_recorder_s_defect_survives_and_a_chipped_head_flickers() {
    let st = Standard::get(Std::Pal);
    let pic = picture(st, flat());
    let (e, o) = field_err(&run(&values(&[("decks", 1.0), ("rec_a_spacing", 3.0)]), &pic, 0));
    eprintln!("two decks, record head A clogged: {e:.4}/{o:.4}");
    assert!(e > 8.0 * o, "the record defect did not survive playback on a clean drum");
    let ndb = |vals: &[f64]| {
        let s = run(vals, &picture(st, |_, _| grey(0.5)), 0);
        let h = st.h as usize;
        let f = |p: usize| (8..h - 2 * SWITCH_ROWS).filter(|y| y & 1 == p).map(|y| row_err(&s, y).powi(2)).sum::<f64>();
        10.0 * (f(1) / f(0)).flog10()
    };
    let chip = ndb(&values(&[("rec_b_output", -6.0), ("rec_b_gap", 0.6)]));
    eprintln!("chipped head B: odd rows {chip:.1} dB noisier");
    assert!(chip > 6.0, "the chipped head's field is only {chip:.1} dB noisier");
}

/// The edge position (sub-sample) of a step in a luma row.
fn edge(s: &Sim, y: usize) -> f64 {
    let w = s.uni.w as usize;
    let r = &s.luma[y * w..(y + 1) * w];
    let mid = (s.uni.m + s.uni.a / 2) as usize;
    for n in mid - 200..mid + 200 {
        if r[n] < 0.5 && r[n + 1] >= 0.5 {
            return n as f64 + (0.5 - r[n] as f64) / (r[n + 1] - r[n]) as f64;
        }
    }
    f64::NAN
}

/// t1191: the TBC removes the line jitter and the switching skew, not the switching noise.
#[test]
fn t1191_the_tbc_removes_jitter_and_skew() {
    let st = Standard::get(Std::Pal);
    let pic = picture(st, |x, _| if x < st.a / 2 { grey(0.2) } else { grey(0.8) });
    let shifts = |tbc: f64, sp: f64| {
        let s = run(&values(&[("jitter_us", 0.5), ("switch_skew_us", 2.0), ("tbc", tbc), ("switch_lines", sp)]), &pic, 0);
        // the read-out position of the edge per grid row = the luma's edge + the row's time-base shift
        let rows: Vec<f64> = (0..st.h as usize).map(|y| edge(&s, y) + s.lt[y * LT_W + lt::TBE] as f64).collect();
        (rows, s)
    };
    let (no, _) = shifts(0.0, 6.5);
    let (yes, sy) = shifts(1.0, 6.5);
    let spread = |v: &[f64], from: usize, to: usize| {
        let m = v[from..to].iter().sum::<f64>() / (to - from) as f64;
        (v[from..to].iter().map(|x| (x - m).powi(2)).sum::<f64>() / (to - from) as f64).sqrt()
    };
    let h = st.h as usize;
    eprintln!("edge spread: no TBC {:.2}, TBC {:.2} samples", spread(&no, 20, h - 40), spread(&yes, 20, h - 40));
    assert!(spread(&no, 20, h - 40) > 2.0, "no line jitter without the TBC");
    assert!(spread(&yes, 20, h - 40) < 0.3, "the TBC left jitter");
    // the rows after the switching point: displaced by the skew without the TBC, in place with it
    let skew = 2.0 * st.per_us();
    let tail = |v: &[f64], sp: f64| -> f64 {
        let hf = h / 2;
        let first = (hf as f64 - sp).ceil() as usize;
        let body = v[2 * (hf - 40)..2 * (hf - 20)].iter().sum::<f64>() / 40.0;
        let sw: Vec<f64> = (first..hf).map(|yf| v[2 * yf]).collect();
        sw.iter().sum::<f64>() / sw.len() as f64 - body
    };
    assert!((tail(&no, 6.5) - skew).abs() < 0.4 * skew, "the switched rows moved {:.1}, the skew is {skew:.1}", tail(&no, 6.5));
    assert!(tail(&yes, 6.5).abs() < 0.5, "the TBC left the switching skew");
    // the switching noise stays with the TBC, and follows the switching point
    let burst_row = |s: &Sim| (0..h).filter(|y| s.lt[y * LT_W + lt::BURST] >= 0.0).map(|y| y >> 1).next();
    let (_, s9) = shifts(1.0, 9.5);
    assert_eq!(burst_row(&sy), Some(h / 2 - 7));
    assert_eq!(burst_row(&s9), Some(h / 2 - 10));
}

/// t1192: the dropout compensator hides dropouts with the line before.
#[test]
fn t1192_the_dropout_compensator_hides_dropouts() {
    let st = Standard::get(Std::Pal);
    let pic = picture(st, |_, _| grey(0.5));
    let err = |doc: f64| {
        let s = run(&values(&[("dropouts_per_s", 400.0), ("dropout_us", 12.0), ("doc", doc)]), &pic, 0);
        let h = st.h as usize;
        let rows: Vec<usize> = (2..h).filter(|y| s.lt[y * LT_W + lt::DROP_START] >= 0.0).collect();
        assert!(rows.len() >= 10, "the worn tape has only {} dropouts", rows.len());
        rows.iter().map(|y| row_err(&s, *y)).sum::<f64>() / rows.len() as f64
    };
    let (off, on) = (err(0.0), err(1.0));
    eprintln!("dropout rows: {off:.4} without DOC, {on:.4} with");
    assert!(on * 2.0 < off, "the compensator helped only {:.1}×", off / on);
}

/// The block prefix the GPU uses equals the CPU's running sum (wrapping u32).
#[test]
fn the_block_prefix_is_the_running_sum() {
    let vals = values(&[]);
    let st = Deck::new(&vals).st;
    let s = run(&vals, &picture(st, flat()), 0);
    let w = st.w as usize;
    let blk: Vec<u32> = (0..w).map(|n| sim::s2(&s.inc, w, 3, n as i32)).collect();
    for n in [0usize, 31, 32, 500, w - 1] {
        let mut p = sim::start_phase(&s.uni, 3);
        let mut j = n as i64;
        while j >= 0 {
            p = p.wrapping_add(blk[j as usize]);
            j -= 32;
        }
        assert_eq!(p, s.phase[3 * w + n], "sample {n}");
    }
}


/// The cost of one 1080p frame on this machine (run on a release build once: design D10).
#[test]
#[ignore]
fn cost_per_1080p_frame() {
    for (name, sets) in [("default", vec![]), ("worn", vec![("decks", 1.0), ("dropouts_per_s", 60.0)]), ("ntsc", vec![("standard", 1.0)])] {
        let vals = values(&sets);
        let st = Deck::new(&vals).st;
        let pic = picture(st, flat());
        let mut out = vec![0u8; 1920 * 1080 * 4];
        let t0 = std::time::Instant::now();
        let n = 10;
        for i in 0..n {
            let s = run(&vals, &pic, i * 3600);
            sim::read_out(&s, 1920, 1080, &mut out);
        }
        eprintln!("{name}: {:.1} ms per 1080p frame ({} threads)", t0.elapsed().as_secs_f64() * 1000.0 / n as f64, std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1));
    }
}

/// Pictures of the presets on our test card, for the report (not a check): set
/// `KT_TAPE_GALLERY=<dir>`; writes `<preset>.ppm` at the grid's resolution.
#[test]
#[ignore]
fn gallery_of_the_presets() {
    let Ok(dir) = std::env::var("KT_TAPE_GALLERY") else { return };
    let st = Standard::get(Std::Pal);
    let card = picture(st, |x, y| {
        let (fx, fy) = (x as f64 / st.a as f64, y as f64 / st.h as f64);
        let bars = [[191, 191, 191], [191, 191, 0], [0, 191, 191], [0, 191, 0], [191, 0, 191], [191, 0, 0], [0, 0, 191], [16, 16, 16]];
        if fy < 0.45 {
            let c = bars[(fx * 8.0) as usize];
            [c[0], c[1], c[2]]
        } else if fy < 0.6 {
            // a frequency burst: 0.5, 1, 2, 3, 4 MHz
            let f = [0.5e6, 1.0e6, 2.0e6, 3.0e6, 4.0e6][((fx * 5.0) as usize).min(4)];
            let v = 0.5 + 0.4 * (std::f64::consts::TAU * f / st.fs * (x + st.m) as f64).fsin();
            grey(v)
        } else if fy < 0.75 {
            grey(fx)
        } else {
            // a white box on black, a black box on white: the edges' streaks
            let w = (fx > 0.1 && fx < 0.4) != (fx > 0.55 && fx < 0.9);
            if fx < 0.5 { if w { [235, 235, 235] } else { [16, 16, 16] } } else if w { [16, 16, 16] } else { [235, 235, 235] }
        }
    });
    let p = fx_video_tape::VhsTape::new();
    let mut sets: Vec<(String, Vec<(&str, f64)>)> = vec![("defaults".into(), vec![])];
    for v in p.manifest().ui.variants {
        sets.push((v.name_key.trim_start_matches("fxreg.var.").to_string(), v.sets.to_vec()));
    }
    sets.push(("tracking".into(), vec![("decks", 1.0), ("skew_um", 50.0), ("tracking_um", 22.0)]));
    let write = |name: &str, rgba: &[u8], w: u32, h: u32| {
        let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
        for px in rgba.chunks_exact(4) {
            out.extend_from_slice(&px[..3]);
        }
        std::fs::write(format!("{dir}/{name}.ppm"), out).unwrap();
    };
    write("input", &card, st.a, st.h);
    for (name, s) in sets {
        let r = run(&values(&s), &card, 90_000);
        let mut o = vec![0u8; (st.a * st.h * 4) as usize];
        sim::read_out(&r, st.a, st.h, &mut o);
        write(&name, &o, st.a, st.h);
    }
}
