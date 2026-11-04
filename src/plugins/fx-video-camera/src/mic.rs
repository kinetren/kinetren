//! The camera's microphone (sim-audio-pairs; note F-FX-014): the audio half of `camera_isp`. The
//! device the ISP simulates recorded the scene's sound through its own chain, and this films the
//! event's sound again through it, step by step:
//!
//! 1. the scene's pressure: the source's full scale is `mic_scene_spl` dB SPL (94 dB = 1 Pa);
//! 2. wind at the port: turbulence whose pressure grows with the cube of the wind speed, below a
//!    few hundred hertz, in gusts;
//! 3. the acoustic port: the hole, its channel and the front cavity are a Helmholtz resonator,
//!    `f = c/2π·√(A/(V·L_eff))` (`L_eff = L + 0.85·d`), a second-order lowpass with its peak;
//! 4. the capsule: the vent's high-pass, the self-noise (dBA), the acoustic overload point (a soft
//!    clip), the converter's full scale at the AOP;
//! 5. the device's gain control (AGC): an RMS detector with a fast attack and a slow release lifts a
//!    quiet scene to its target up to its maximum gain — the room tone breathes up between words;
//! 6. the noise suppressor: a 512-point STFT, the noise floor by minimum statistics, a spectral gain
//!    with a floor — strong settings leave "musical noise" and swallow word tails;
//! 7. the device's codec: AAC at its bitrate (`kt_plugin_api::audio_codec`, no prediction: AAC-LC),
//!    16-bit PCM at 16 kHz (a USB webcam), or G.711 µ-law at 8 kHz (a security camera) — the rate
//!    change through anti-alias and anti-image filters.
//!
//! The pair shares the DEVICE: the looks set the picture's sensor and this chain together, and
//! the sensor's seed seeds the capsule's noise. Everything random is a hash of the timeline sample.

use kt_plugin_api::audio_codec::{self as codec, Damage, Decoder, Encoder, N};
use kt_plugin_api::generator::lowbias32;
use kt_plugin_api::math::FrozenMath;
use kt_plugin_api::{AudioState, Placement};

const FS: f64 = 48_000.0;
/// The NS frame and hop.
const NS_N: usize = 512;
const NS_HOP: usize = 256;
/// Half the resampling filters' length (63 taps each, at 48 kHz).
const RS_M: usize = 31;
/// The look-ahead: the codec's window, the NS frame, two resampling filters.
pub const LA: i64 = (2 * N + NS_N + 2 * RS_M) as i64;

/// The device codecs.
pub const CODECS: &[&str] = &["aac", "pcm16k", "g711"];
pub const AAC_KBPS: &[&str] = &["64", "96", "128", "192"];
const AAC_KBPS_V: [f64; 4] = [64.0, 96.0, 128.0, 192.0];
pub const CHANNELS: &[&str] = &["mono", "stereo"];

/// The microphone rows by name, read once per `apply`.
#[derive(Clone, Copy, Debug)]
pub struct MicRows {
    pub stereo: bool,
    pub scene_spl: f64,
    pub wind: f64,
    pub port_d: f64,
    pub port_l: f64,
    pub front_v: f64,
    pub self_noise: f64,
    pub aop: f64,
    pub hpf: f64,
    pub agc: bool,
    pub agc_target: f64,
    pub agc_max: f64,
    pub ns: bool,
    pub ns_strength: f64,
    pub codec: usize,
    pub kbps: f64,
    pub seed: u32,
}

impl MicRows {
    pub fn from(v: &[f64]) -> MicRows {
        let g = |n: &str| v[crate::ix(n)];
        let pick = |n: &str, hi: usize| (g(n).round().max(0.0) as usize).min(hi);
        MicRows {
            stereo: pick("mic_channels", 1) == 1,
            scene_spl: g("mic_scene_spl"),
            wind: g("mic_wind").max(0.0),
            port_d: g("mic_port_d").max(0.05),
            port_l: g("mic_port_l").max(0.05),
            front_v: g("mic_front_v").max(0.1),
            self_noise: g("mic_self_noise"),
            aop: g("mic_aop"),
            hpf: g("mic_hpf").max(1.0),
            agc: g("mic_agc") >= 0.5,
            agc_target: g("mic_agc_target"),
            agc_max: g("mic_agc_max").max(0.0),
            ns: g("mic_ns") >= 0.5,
            ns_strength: g("mic_ns_strength").clamp(0.0, 1.0),
            codec: pick("mic_codec", 2),
            kbps: AAC_KBPS_V[pick("mic_kbps", 3)],
            seed: g("sensor_seed") as u32,
        }
    }

    /// The port's Helmholtz frequency (Hz): c/2π·√(A/(V·L_eff)), lengths in mm.
    pub fn helmholtz(&self) -> f64 {
        let a = std::f64::consts::PI * 0.25 * self.port_d * self.port_d;
        let l = self.port_l + 0.85 * self.port_d;
        343_000.0 / std::f64::consts::TAU * (a / (self.front_v * l)).sqrt()
    }

    /// The device's sample rate's decimation from 48 kHz (1, 3, 6).
    pub fn decimation(&self) -> usize {
        [1, 3, 6][self.codec]
    }
}

/// A biquad (transposed direct form II, f64).
#[derive(Clone, Copy, Default)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [f64; 2],
}

impl Biquad {
    /// RBJ lowpass at `f` with `q`.
    fn lowpass(f: f64, q: f64) -> Biquad {
        let w = std::f64::consts::TAU * (f / FS).min(0.49);
        let (s, c) = w.fsin_cos();
        let al = s / (2.0 * q);
        let a0 = 1.0 + al;
        Biquad { b: [(1.0 - c) / 2.0 / a0, (1.0 - c) / a0, (1.0 - c) / 2.0 / a0], a: [-2.0 * c / a0, (1.0 - al) / a0], z: [0.0; 2] }
    }
    /// A first-order high-pass at `f` (as a biquad).
    fn highpass1(f: f64) -> Biquad {
        let k = (std::f64::consts::PI * f / FS).ftan();
        let a0 = 1.0 + k;
        Biquad { b: [1.0 / a0, -1.0 / a0, 0.0], a: [(k - 1.0) / a0, 0.0], z: [0.0; 2] }
    }
    /// A first-order low-pass at `f`.
    fn lowpass1(f: f64) -> Biquad {
        let k = (std::f64::consts::PI * f / FS).ftan();
        let a0 = 1.0 + k;
        Biquad { b: [k / a0, k / a0, 0.0], a: [(k - 1.0) / a0, 0.0], z: [0.0; 2] }
    }
    #[inline]
    fn run(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// A windowed-sinc lowpass of 2·RS_M + 1 taps at `fc` Hz (Blackman window), unity DC gain.
fn sinc_lowpass(fc: f64) -> Vec<f64> {
    let pi = std::f64::consts::PI;
    let mut h: Vec<f64> = (0..=2 * RS_M)
        .map(|i| {
            let k = i as f64 - RS_M as f64;
            let x = 2.0 * fc / FS;
            let s = if k == 0.0 { x } else { (pi * x * k).fsin() / (pi * k) };
            let n = i as f64 / (2 * RS_M) as f64;
            let w = 0.42 - 0.5 * (2.0 * pi * n).fcos() + 0.08 * (4.0 * pi * n).fcos();
            s * w
        })
        .collect();
    let sum: f64 = h.iter().sum();
    h.iter_mut().for_each(|v| *v /= sum);
    h
}

/// G.711 µ-law: compress a sample in [−1, 1] to its 8-bit code and back (the segment law).
pub fn mulaw(x: f64) -> f64 {
    const MU: f64 = 255.0;
    let s = x.clamp(-1.0, 1.0);
    let c = (1.0 + MU * s.abs()).flog10() / (1.0 + MU).flog10();
    let q = (c * 127.0).round() / 127.0;
    s.signum() * ((1.0 + MU).fpowf(q) - 1.0) / MU
}

/// A delay line of fixed length.
#[derive(Clone)]
struct Delay {
    buf: Vec<f64>,
    i: usize,
}

impl Delay {
    fn new(n: usize) -> Delay {
        Delay { buf: vec![0.0; n.max(1)], i: 0 }
    }
    #[inline]
    fn run(&mut self, x: f64) -> f64 {
        let y = self.buf[self.i];
        self.buf[self.i] = x;
        self.i = (self.i + 1) % self.buf.len();
        y
    }
}

/// The noise suppressor of one channel: overlap-add STFT with a sqrt-Hann window.
#[derive(Clone)]
struct Ns {
    inbuf: Vec<f64>,
    outbuf: Vec<f64>,
    fill: usize,
    noise: Vec<f64>,
    smooth: Vec<f64>,
    min_now: Vec<f64>,
    min_prev: Vec<f64>,
    frames: u64,
    gain: Vec<f64>,
}

/// The suppressor's transform, built once.
fn ns_fft() -> &'static codec::Fft {
    static F: std::sync::OnceLock<codec::Fft> = std::sync::OnceLock::new();
    F.get_or_init(|| codec::Fft::new(NS_N))
}

const NS_BINS: usize = NS_N / 2 + 1;
/// Minimum statistics: the minimum is kept over two windows of this many frames (≈ 0.75 s each).
const NS_WIN: u64 = 140;

impl Ns {
    fn new() -> Ns {
        Ns {
            inbuf: vec![0.0; NS_N],
            outbuf: vec![0.0; NS_N],
            fill: NS_N - NS_HOP,
            noise: vec![0.0; NS_BINS],
            smooth: vec![0.0; NS_BINS],
            min_now: vec![f64::MAX; NS_BINS],
            min_prev: vec![f64::MAX; NS_BINS],
            frames: 0,
            gain: vec![1.0; NS_BINS],
        }
    }

    /// One sample in, one sample out (NS_N samples late).
    fn run(&mut self, x: f64, strength: f64, on: bool) -> f64 {
        self.inbuf[self.fill] = x;
        self.fill += 1;
        let y = self.outbuf[self.fill - (NS_N - NS_HOP) - 1];
        if self.fill == NS_N {
            self.frame(strength, on);
            self.inbuf.copy_within(NS_HOP.., 0);
            self.fill = NS_N - NS_HOP;
        }
        y
    }

    fn frame(&mut self, strength: f64, on: bool) {
        let pi = std::f64::consts::PI;
        let win = |n: usize| (pi * (n as f64 + 0.5) / NS_N as f64).fsin();
        let mut re: Vec<f64> = (0..NS_N).map(|n| self.inbuf[n] * win(n)).collect();
        let mut im = vec![0.0; NS_N];
        let fft = ns_fft();
        fft.run(&mut re, &mut im, false);
        // the noise floor: the minimum of the smoothed power over the last two windows
        for k in 0..NS_BINS {
            let p = re[k] * re[k] + im[k] * im[k];
            self.smooth[k] = 0.8 * self.smooth[k] + 0.2 * p;
            self.min_now[k] = self.min_now[k].min(self.smooth[k]);
            self.noise[k] = self.min_now[k].min(self.min_prev[k]) * 1.5;
        }
        self.frames += 1;
        if self.frames.is_multiple_of(NS_WIN) {
            self.min_prev = std::mem::replace(&mut self.min_now, vec![f64::MAX; NS_BINS]);
        }
        let over = 1.0 + 3.0 * strength;
        let floor = 10f64.fpowf(-(6.0 + 24.0 * strength) / 20.0);
        for k in 0..NS_BINS {
            let p = re[k] * re[k] + im[k] * im[k] + 1e-20;
            let g = if on { (1.0 - over * self.noise[k] / p).max(0.0).sqrt().max(floor) } else { 1.0 };
            // a fast attack, a slower release of the gain: the suppressor's own time constants
            self.gain[k] = if g > self.gain[k] { 0.7 * g + 0.3 * self.gain[k] } else { 0.4 * g + 0.6 * self.gain[k] };
            let gk = self.gain[k];
            re[k] *= gk;
            im[k] *= gk;
            if k > 0 && k < NS_N / 2 {
                re[NS_N - k] = re[k];
                im[NS_N - k] = -im[k];
            }
        }
        fft.run(&mut re, &mut im, true);
        // overlap-add (sqrt-Hann analysis × synthesis = Hann, which sums to 1 at 50 % overlap)
        self.outbuf.copy_within(NS_HOP.., 0);
        for v in &mut self.outbuf[NS_N - NS_HOP..] {
            *v = 0.0;
        }
        for (n, r) in re.iter().enumerate() {
            self.outbuf[n] += r / NS_N as f64 * win(n);
        }
    }
}

/// One channel's chain state.
#[derive(Clone)]
struct Chan {
    port: Biquad,
    vent: Biquad,
    wind_lp: Biquad,
    agc_env: f64,
    agc_gain: f64,
    ns: Ns,
    aa: Vec<f64>,
    ai: Vec<f64>,
    aa_hist: Vec<f64>,
    ai_hist: Vec<f64>,
    delay_rs: Delay,
    enc: Encoder,
    dec: Decoder,
    block: Vec<f64>,
    fresh: Vec<f64>,
    out: std::collections::VecDeque<f64>,
    delay_codec: Delay,
}

/// The audio state of `camera_isp_sound`.
#[derive(Clone)]
pub struct MicSound {
    v: Vec<f64>,
    r: MicRows,
    at: Placement,
    ch: [Chan; 2],
    /// The timeline frame of the next input sample (a jump restarts the codec's frame grid).
    next_in: Option<i64>,
    frame_fill: usize,
}

impl MicSound {
    pub fn new() -> MicSound {
        let v = crate::defaults();
        let r = MicRows::from(&v);
        MicSound { ch: [Self::chan(&r), Self::chan(&r)], v, r, at: Placement::default(), next_in: None, frame_fill: 0 }
    }

    fn chan(r: &MicRows) -> Chan {
        let m = r.decimation();
        let fc = 0.45 * FS / m as f64;
        let kbps = r.kbps / if r.stereo { 2.0 } else { 1.0 };
        Chan {
            port: Biquad::lowpass(r.helmholtz(), 3.0),
            vent: Biquad::highpass1(r.hpf),
            wind_lp: Biquad::lowpass1(150.0),
            agc_env: 0.0,
            agc_gain: 1.0,
            ns: Ns::new(),
            aa: sinc_lowpass(fc),
            ai: sinc_lowpass(fc),
            aa_hist: vec![0.0; 2 * RS_M + 1],
            ai_hist: vec![0.0; 2 * RS_M + 1],
            delay_rs: Delay::new(if m == 1 { 2 * RS_M } else { 1 }),
            enc: Encoder::new(kbps),
            dec: Decoder::new(0xC0DE),
            block: vec![0.0; 2 * N],
            fresh: Vec::with_capacity(N),
            out: std::collections::VecDeque::new(),
            delay_codec: Delay::new(if r.codec == 0 { 1 } else { 2 * N }),
        }
    }

    fn rebuild(&mut self) {
        self.r = MicRows::from(&self.v);
        self.ch = [Self::chan(&self.r), Self::chan(&self.r)];
        self.next_in = None;
        self.frame_fill = 0;
    }

    /// The wind's pressure (Pa) at timeline frame `n`: turbulence with a gust envelope.
    fn wind(&self, n: i64, c: usize) -> f64 {
        if self.r.wind <= 0.0 {
            return 0.0;
        }
        let h = lowbias32(self.r.seed ^ (n as u32).wrapping_mul(0x9E37_79B9) ^ (c as u32 * 0x632B_E5AB) ^ 0x51D);
        let white = ((h >> 8) as f64 / 16_777_216.0 - 0.5) * 3.464;
        // gusts: a smooth random level on a 0.4 s grid
        let cell = n.div_euclid(19_200);
        let fr = n.rem_euclid(19_200) as f64 / 19_200.0;
        let lv = |k: i64| 0.4 + 1.2 * (lowbias32(self.r.seed ^ k as u32 ^ 0x6057) >> 8) as f64 / 16_777_216.0;
        let gust = lv(cell) + (lv(cell + 1) - lv(cell)) * fr * fr * (3.0 - 2.0 * fr);
        0.02 * self.r.wind * self.r.wind * self.r.wind * gust * white
    }
}

impl Default for MicSound {
    fn default() -> MicSound {
        MicSound::new()
    }
}

impl AudioState for MicSound {
    fn apply(&mut self, k: &[f64], structural: bool) {
        let mut v = crate::defaults();
        for (o, x) in v.iter_mut().zip(k) {
            *o = *x;
        }
        if v != self.v || structural {
            self.v = v;
            self.rebuild();
        }
    }

    /// The microphone's numbers follow their curves chunk by chunk (a gust, an AGC ridden by hand);
    /// the filters take new coefficients and keep their state.
    fn set_targets(&mut self, c: &dyn kt_plugin_api::SlotCurves, s: f64) {
        let mut v = self.v.clone();
        for row in ["mic_scene_spl", "mic_wind", "mic_port_d", "mic_port_l", "mic_front_v", "mic_hpf"] {
            let i = crate::ix(row);
            v[i] = c.value(row, v[i], s);
        }
        for row in ["mic_self_noise", "mic_aop", "mic_agc_target", "mic_agc_max", "mic_ns_strength"] {
            let i = crate::ix(row);
            v[i] = c.value(row, v[i], s);
        }
        let r = MicRows::from(&v);
        let refilter = r.helmholtz() != self.r.helmholtz() || r.hpf != self.r.hpf;
        self.r = r;
        if refilter {
            for ch in &mut self.ch {
                let (pz, vz) = (ch.port.z, ch.vent.z);
                ch.port = Biquad::lowpass(r.helmholtz(), 3.0);
                ch.vent = Biquad::highpass1(r.hpf);
                ch.port.z = pz;
                ch.vent.z = vz;
            }
        }
    }

    fn place(&mut self, at: Placement) {
        self.at = at;
    }

    fn lookahead(&self) -> i64 {
        LA
    }

    fn fork(&self) -> Option<Box<dyn AudioState>> {
        Some(Box::new(self.clone()))
    }

    fn reset(&mut self) {
        self.rebuild();
    }

    fn process(&mut self, buf: &mut [f32], pos: i64, _key: Option<&[f32]>) {
        let frames = buf.len() / 2;
        let n0 = self.at.start + pos;
        if self.next_in != Some(n0) {
            self.rebuild();
        }
        let r = self.r;
        let pa_fs = 10f64.fpowf((r.scene_spl - 94.0) / 20.0);
        let aop = 20e-6 * 10f64.fpowf(r.aop / 20.0);
        let self_noise = 20e-6 * 10f64.fpowf(r.self_noise / 20.0);
        let target = 10f64.fpowf(r.agc_target / 20.0);
        let max_gain = 10f64.fpowf(r.agc_max / 20.0);
        let (att, rel) = (1.0 - (-1.0 / (0.010 * FS)).fexp(), 1.0 - (-1.0 / (0.6 * FS)).fexp());
        let (g_up, g_down) = (1.0 - (-1.0 / (1.5 * FS)).fexp(), 1.0 - (-1.0 / (0.02 * FS)).fexp());
        let m = r.decimation();
        let nch = if r.stereo { 2 } else { 1 };
        for i in 0..frames {
            let n = n0 + i as i64;
            let input = if r.stereo {
                [buf[2 * i] as f64, buf[2 * i + 1] as f64]
            } else {
                let x = 0.5 * (buf[2 * i] as f64 + buf[2 * i + 1] as f64);
                [x, x]
            };
            let mut outs = [0f64; 2];
            for c in 0..nch {
                let wind = self.wind(n, c);
                let ch = &mut self.ch[c];
                // 1–3: the scene's pressure and the wind at the port, through the port
                let wind = ch.wind_lp.run(wind);
                let p = ch.port.run(input[c] * pa_fs + wind);
                // 4: the capsule: the vent, its noise, its overload; full scale at the AOP
                let h = lowbias32(r.seed ^ (n as u32).wrapping_mul(0x85EB_CA6B) ^ (c as u32) << 29);
                let hiss = ((h >> 8) as f64 / 16_777_216.0 - 0.5) * 3.464 * self_noise;
                let e = ch.vent.run(p) + hiss;
                let d = aop * (e / aop).clamp(-20.0, 20.0).ftanh_frozen() / aop;
                // 5: the AGC
                let lvl = d * d;
                ch.agc_env += if lvl > ch.agc_env { att } else { rel } * (lvl - ch.agc_env);
                let want = if r.agc { (target / ch.agc_env.sqrt().max(1e-9)).clamp(1.0, max_gain) } else { 1.0 };
                ch.agc_gain += if want < ch.agc_gain { g_down } else { g_up } * (want - ch.agc_gain);
                let a = d * ch.agc_gain;
                // 6: the noise suppressor
                let s = ch.ns.run(a, r.ns_strength, r.ns);
                // 7: the device's rate and coding
                let rs = if m == 1 {
                    ch.delay_rs.run(s)
                } else {
                    ch.aa_hist.copy_within(1.., 0);
                    ch.aa_hist[2 * RS_M] = s;
                    let lo: f64 = ch.aa.iter().zip(&ch.aa_hist).map(|(h, x)| h * x).sum();
                    // decimate: the device keeps every m-th sample; the player's interpolator
                    // sees zeros between them
                    let kept = if n.rem_euclid(m as i64) == 0 {
                        let q = if r.codec == 2 { mulaw(lo) } else { (lo * 32_767.0).round() / 32_767.0 };
                        q * m as f64
                    } else {
                        0.0
                    };
                    ch.ai_hist.copy_within(1.., 0);
                    ch.ai_hist[2 * RS_M] = kept;
                    ch.ai.iter().zip(&ch.ai_hist).map(|(h, x)| h * x).sum()
                };
                outs[c] = if r.codec == 0 {
                    // AAC: the codec's frames, on the grid of the stream's start
                    ch.fresh.push(rs);
                    ch.out.pop_front().unwrap_or(0.0)
                } else {
                    ch.delay_codec.run(rs)
                };
            }
            if r.codec == 0 {
                self.frame_fill += 1;
                if self.frame_fill == N {
                    self.frame_fill = 0;
                    let bw = (r.kbps.min(128.0) / 128.0 * 16_000.0 / (24_000.0 / N as f64)) as usize;
                    for c in 0..nch {
                        let ch = &mut self.ch[c];
                        ch.block.copy_within(N.., 0);
                        ch.block[N..].copy_from_slice(&ch.fresh);
                        ch.fresh.clear();
                        // the finest step: −96 dB of a line's range, the rate control does the rest
                        let pkt = ch.enc.encode(&ch.block, false, 1e-3, bw, false);
                        let bits = pkt.payload.v.clone();
                        let mut out = vec![0f64; N];
                        ch.dec.decode(&pkt, &bits, Damage { residual: 1.0, ..Default::default() }, bw, false, 0, &mut out);
                        ch.out.extend(out);
                    }
                }
            }
            if !r.stereo {
                outs[1] = outs[0];
            }
            buf[2 * i] = outs[0] as f32;
            buf[2 * i + 1] = outs[1] as f32;
        }
        self.next_in = Some(n0 + frames as i64);
    }
}

/// tanh by the frozen exp (the overload's soft clip).
trait FrozenTanh {
    fn ftanh_frozen(self) -> f64;
}

impl FrozenTanh for f64 {
    fn ftanh_frozen(self) -> f64 {
        let e = (2.0 * self.clamp(-20.0, 20.0)).fexp();
        (e - 1.0) / (e + 1.0)
    }
}
