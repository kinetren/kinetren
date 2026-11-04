//! The camera's microphone (sim-audio-pairs; capability `audio-fx`): what the device's chain predicts.

use fx_video_camera::mic::{mulaw, MicRows, MicSound, LA};
use kt_plugin_api::math::FrozenMath;
use kt_plugin_api::{AudioState, Placement};

const FS: f64 = 48_000.0;

/// The passport values with `sets` applied by row name, after turning off what the test does not
/// ask for (a plain capsule into AAC: no AGC, no NS).
fn values(sets: &[(&str, f64)]) -> Vec<f64> {
    let mut v = fx_video_camera::defaults();
    for (name, x) in [("mic_agc", 0.0), ("mic_ns", 0.0)].iter().chain(sets) {
        v[fx_video_camera::ix(name)] = *x;
    }
    v
}

/// Play `secs` of `f`: the output's left channel by OUTPUT position.
fn play(vals: &[f64], secs: f64, f: impl Fn(f64) -> f32) -> Vec<f32> {
    let mut s = MicSound::new();
    s.place(Placement { start: 0, fps_num: 25, fps_den: 1 });
    s.apply(vals, true);
    let n = (secs * FS) as i64;
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < n + LA {
        let mut buf = vec![0f32; 2048];
        for i in 0..1024 {
            let x = f((pos + i as i64) as f64 / FS);
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
    out
}

fn sine(hz: f64, amp: f32) -> impl Fn(f64) -> f32 {
    move |t| amp * (std::f64::consts::TAU * hz * t).fsin() as f32
}

/// The amplitude of `hz` in `v[from..to]` (least squares).
fn tone(v: &[f32], from: usize, to: usize, hz: f64) -> f64 {
    let (mut c, mut s) = (0.0, 0.0);
    for (j, x) in v[from..to].iter().enumerate() {
        let ph = std::f64::consts::TAU * hz * (from + j) as f64 / FS;
        c += *x as f64 * ph.fcos();
        s += *x as f64 * ph.fsin();
    }
    2.0 * (c * c + s * s).sqrt() / (to - from) as f64
}

fn rms(v: &[f32]) -> f64 {
    (v.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>() / v.len() as f64).sqrt()
}

/// t1477: the acoustic port is a Helmholtz resonator: a long channel into a large cavity peaks where
/// c/2π·√(A/(V·L_eff)) says, and rolls off above it.
#[test]
fn t1477_the_port_is_a_helmholtz_resonator() {
    let vals = values(&[("mic_port_d", 0.6), ("mic_port_l", 3.0), ("mic_front_v", 20.0), ("mic_kbps", 3.0)]);
    let f0 = MicRows::from(&vals).helmholtz();
    assert!((3000.0..4000.0).contains(&f0), "the port's frequency: {f0}");
    let lvl = |hz: f64| tone(&play(&vals, 0.5, sine(hz, 0.02)), 4800, 19_000, hz);
    let (low, peak, high) = (lvl(800.0), lvl(f0), lvl(f0 * 2.5));
    eprintln!("f0 {f0:.0} Hz: 800 Hz {low:.5}, peak {peak:.5}, 2.5 f0 {high:.5}");
    assert!(peak > 2.0 * low, "no resonance at the port's frequency");
    assert!(high < 0.5 * low, "no roll-off above the port");
}

/// t1478: the AGC lifts a quiet scene to its target, and stops at its maximum gain on a quieter one;
/// off, the level stays.
#[test]
fn t1478_the_agc_lifts_a_quiet_scene() {
    let on = values(&[("mic_agc", 1.0), ("mic_agc_max", 40.0)]);
    let off = values(&[]);
    let level = |vals: &[f64], amp: f32| {
        let y = play(vals, 4.0, sine(700.0, amp));
        rms(&y[(3.0 * FS) as usize..])
    };
    let (quiet_on, quiet_off) = (level(&on, 0.15), level(&off, 0.15));
    let (tiny_on, tiny_off) = (level(&on, 0.003), level(&off, 0.003));
    eprintln!("quiet {quiet_off:.5} → {quiet_on:.4}; tiny {tiny_off:.6} → {tiny_on:.5}");
    // the detector attacks fast and releases slowly: it holds the PEAK of a sine at the target
    let target = 10f64.fpowf(-12.0 / 20.0);
    let peak = quiet_on * std::f64::consts::SQRT_2;
    assert!(peak > 0.7 * target && peak < 1.3 * target, "the AGC's peak {peak} is not its target {target}");
    let gain = tiny_on / tiny_off;
    assert!(gain > 70.0 && gain < 120.0, "the AGC did not stop at its 40 dB: ×{gain:.1}");
}

/// t1479: the noise suppressor learns the steady floor and takes it down between bursts of signal.
#[test]
fn t1479_the_noise_suppressor_takes_the_floor_down() {
    // a steady noise floor with a tone burst every other half second
    let noisy = |t: f64| {
        let n = (t * FS) as u64;
        let h = (n.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) as f64 / 16_777_216.0 - 0.5;
        let burst = (t * 2.0).floor() as i64 % 2 == 0;
        let tone = if burst { 0.2 * (std::f64::consts::TAU * 1000.0 * t).fsin() } else { 0.0 };
        (0.02 * h + tone) as f32
    };
    let on = values(&[("mic_ns", 1.0), ("mic_ns_strength", 0.7)]);
    let off = values(&[]);
    let yn = play(&on, 4.0, noisy);
    let yo = play(&off, 4.0, noisy);
    // the last gap: 3.5–4.0 s
    let (a, b) = ((3.6 * FS) as usize, (3.95 * FS) as usize);
    let (fn_, fo) = (rms(&yn[a..b]), rms(&yo[a..b]));
    let (ta, tb) = ((3.1 * FS) as usize, (3.4 * FS) as usize);
    let (tn, to) = (tone(&yn, ta, tb, 1000.0), tone(&yo, ta, tb, 1000.0));
    eprintln!("floor {fo:.5} → {fn_:.5}, tone {to:.5} → {tn:.5}");
    assert!(fn_ < 0.5 * fo, "the noise suppressor did not take the floor down");
    assert!(tn > 0.5 * to, "the noise suppressor swallowed the tone");
}

/// t1480: G.711 at 8 kHz keeps nothing above 4 kHz; 16 kHz PCM nothing above 8 kHz; AAC keeps both.
#[test]
fn t1480_the_device_rate_limits_the_band() {
    let level = |vals: &[f64], hz: f64| tone(&play(vals, 0.5, sine(hz, 0.05)), 4800, 19_000, hz);
    let flat = [("mic_port_d", 3.0), ("mic_port_l", 0.2), ("mic_front_v", 1.0), ("mic_aop", 100.0)];
    let with = |c: f64| {
        let mut s = flat.to_vec();
        s.push(("mic_codec", c));
        values(&s)
    };
    let (aac, usb, g711) = (with(0.0), with(1.0), with(2.0));
    let (a6, a2) = (level(&aac, 6000.0), level(&aac, 2000.0));
    let (u6, u10) = (level(&usb, 6000.0), level(&usb, 10_000.0));
    let (g2, g6) = (level(&g711, 2000.0), level(&g711, 6000.0));
    eprintln!("AAC 2k {a2:.5} 6k {a6:.5}; USB 6k {u6:.5} 10k {u10:.5}; G.711 2k {g2:.5} 6k {g6:.5}");
    assert!(a6 > 0.5 * a2, "AAC lost 6 kHz");
    assert!(u10 < 0.02 * u6, "16 kHz PCM kept 10 kHz");
    assert!(g6 < 0.02 * g2, "G.711 kept 6 kHz");
    assert!(g2 > 0.3 * a2, "G.711 lost its band");
    // µ-law spends its 8 bits on the quiet end: a small sample keeps its value within 1 %
    assert!((mulaw(0.01) - 0.01).abs() < 0.0015 && (mulaw(0.5) - 0.5).abs() < 0.03);
}

/// t1481: wind is low-frequency turbulence growing fast with its speed.
#[test]
fn t1481_wind_is_low_turbulence_at_the_port() {
    let silence = |_t: f64| 0f32;
    let calm = play(&values(&[]), 1.0, silence);
    let breeze = play(&values(&[("mic_wind", 2.0)]), 1.0, silence);
    let gale = play(&values(&[("mic_wind", 8.0)]), 1.0, silence);
    let (c, b, g) = (rms(&calm[9600..]), rms(&breeze[9600..]), rms(&gale[9600..]));
    eprintln!("calm {c:.6}, 2 m/s {b:.6}, 8 m/s {g:.6}");
    assert!(b > 3.0 * c, "the breeze is not heard");
    assert!(g > 20.0 * b, "wind does not grow with its cube");
}

/// t1482: the chain is a function of the rows and the timeline: two runs give the same bytes.
#[test]
fn t1482_the_chain_is_deterministic() {
    let vals = values(&[("mic_agc", 1.0), ("mic_ns", 1.0), ("mic_wind", 1.0)]);
    let f = sine(440.0, 0.1);
    assert!(play(&vals, 1.0, &f) == play(&vals, 1.0, &f), "two runs differ");
}

/// t1483: the looks are devices: each sets its microphone with its sensor.
#[test]
fn t1483_the_looks_set_the_microphone() {
    use kt_plugin_api::Plugin;
    let p = fx_video_camera::CameraIsp::new();
    let look = |key: &str| {
        let v = p.manifest().ui.variants.iter().find(|u| u.name_key == key).unwrap_or_else(|| panic!("no look {key}"));
        let mut vals = fx_video_camera::defaults();
        for (n, x) in v.sets {
            vals[fx_video_camera::ix(n)] = *x;
        }
        MicRows::from(&vals)
    };
    let phone = look("fxreg.var.phone_night");
    assert!(phone.stereo && phone.agc && phone.ns && phone.codec == 0, "the phone: {phone:?}");
    let web = look("fxreg.var.cheap_webcam");
    assert!(!web.stereo && web.agc && !web.ns && web.codec == 1, "the webcam: {web:?}");
    assert!(web.helmholtz() < 5000.0, "a hollow webcam's port: {}", web.helmholtz());
    let sec = look("fxreg.var.security_ir");
    assert!(!sec.stereo && sec.codec == 2 && sec.wind > 0.0, "the security camera: {sec:?}");
    // a look that names no device leaves the plain camera microphone
    let wrong = look("fxreg.var.wrong_cfa_phase");
    assert!(wrong.codec == 0 && wrong.wind == 0.0);
}
