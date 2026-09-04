//! Аудио-FX: slowed (rubato-варispeed) + reverb (freeverb) + gain.
//!
//! Архитектурные решения, закрывающие хрипы старого движка
//! (дефекты a–f, docs/porting-notes.md):
//! - весь звук — f32 48kHz/2ch; FX не видят sample-rate источника (a);
//! - состояние живёт в одном объекте на клип, владелец — аудио-render-поток
//!   через FxStateStore в kt-engine, без шаринга с prefetch (b);
//! - варispeed — rubato SincFixedOut с непрерывным потреблением источника:
//!   дробный курсор, ни один сэмпл не пропускается и не дублируется (a, c, d);
//! - параметры сглаживаются one-pole (~20 мс) и НИКОГДА не сбрасывают
//!   буферы; полный сброс — только reset() при разрыве непрерывности (e);
//! - tail() сообщает хвост реверба — граф продлевает окно клипа (f).

use kt_model::{FxInstance, SlowedKeyframe};
use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick};
use rubato::{
    Resampler, SincFixedOut, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

pub const CH: usize = AUDIO_CHANNELS;

/// Пре-ролл при разрыве непрерывности: max(tail, 100 мс) рендерится в мусор,
/// чтобы состояние (реверб, ресемплер) было прогрето детерминированно.
pub const MIN_PREROLL_FRAMES: i64 = (AUDIO_SAMPLE_RATE as i64) / 10;

/// Роль обращения к источнику. `Warmup` — пре-ролл после seek: эти кадры
/// рендерятся в мусор и на выход не идут, поэтому их промах кэша НЕ
/// underrun. Раньше он им был, и счётчик рос на каждом старте
/// воспроизведения и на каждом завороте лупа (п.166).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullKind {
    Output,
    Warmup,
}

pub mod simd;
pub mod video;
mod video_simd;

/// Явный доступ к ОБОИМ путям ядер для parity-тестов (Р.32, t198).
/// Тесты не трогают env (KT_SIMD читается раз на процесс) — они зовут
/// скаляр и AVX2 напрямую и сравнивают байты.
#[doc(hidden)]
pub mod simd_testing {
    use crate::video::Frame;

    /// Скалярный эталон билинейки.
    #[allow(clippy::too_many_arguments)]
    pub fn bilinear_scalar(
        src: &Frame,
        x0: f64,
        y0: f64,
        cw: f64,
        ch: f64,
        ow: u32,
        oh: u32,
        out: &mut [u8],
    ) {
        crate::video::bilinear_into_scalar(src, x0, y0, cw, ch, ow, oh, out);
    }

    /// AVX2-ядро билинейки. Паникует, если фичи нет — вызывающий тест
    /// обязан сам проверить наличие (skip/KT_REQUIRE_SIMD).
    #[cfg(target_arch = "x86_64")]
    #[allow(clippy::too_many_arguments)]
    pub fn bilinear_avx2(
        src: &Frame,
        x0: f64,
        y0: f64,
        cw: f64,
        ch: f64,
        ow: u32,
        oh: u32,
        out: &mut [u8],
    ) {
        assert!(std::is_x86_feature_detected!("avx2"), "нет AVX2");
        // SAFETY: фича проверена строкой выше, длины буферов — на
        // ответственности теста (он их и создаёт)
        unsafe { crate::video_simd::bilinear_into_avx2(src, x0, y0, cw, ch, ow, oh, out) };
    }

    /// AVX2-ядро noise (in-place по всему кадру, хвост столбцов не
    /// трогается — тест дорисовывает его скаляром).
    #[cfg(target_arch = "x86_64")]
    pub fn noise_avx2(rgba: &mut [u8], w: u32, h: u32, seed: u32, k: i64) {
        assert!(std::is_x86_feature_detected!("avx2"), "нет AVX2");
        // SAFETY: фича проверена; длины — на ответственности теста
        unsafe { crate::video_simd::noise_avx2(rgba, w, h, seed, k) };
    }

    /// AVX2-ядро аддитива glow.
    #[cfg(target_arch = "x86_64")]
    pub fn glow_add_avx2(dst: &mut [u8], up: &[u8], s: f64, pixels: usize) {
        assert!(std::is_x86_feature_detected!("avx2"), "нет AVX2");
        // SAFETY: фича проверена
        unsafe { crate::video_simd::glow_add_avx2(dst, up, s, pixels) };
    }

    /// Скалярные эталоны noise/glow (копии тел из video.rs).
    pub fn noise_scalar(rgba: &mut [u8], w: u32, seed: u32, k: i64) {
        for (p, px) in rgba.chunks_exact_mut(4).enumerate() {
            let (x, y) = (p as u32 % w, p as u32 / w);
            let h = crate::video::noise_hash(x ^ crate::video::noise_hash(y ^ seed));
            let n = ((h >> 16) & 0x7fff) as i64 - 16384;
            let off = (n * k / 32767) as i32;
            for c in px.iter_mut().take(3) {
                *c = (*c as i32 + off).clamp(0, 255) as u8;
            }
        }
    }

    pub fn glow_add_scalar(dst: &mut [u8], up: &[u8], s: f64) {
        for (o, u) in dst.chunks_exact_mut(4).zip(up.chunks_exact(4)) {
            for c in 0..3 {
                o[c] = (o[c] as f64 + u[c] as f64 * s).min(255.0) as u8;
            }
        }
    }

    /// Округление half-away-from-zero: 4 значения через AVX2-эмуляцию
    /// (микро-доказательство против `f64::round`).
    #[cfg(target_arch = "x86_64")]
    pub fn round_half_away_avx2(v: [f64; 4]) -> [f64; 4] {
        assert!(std::is_x86_feature_detected!("avx2"), "нет AVX2");
        // SAFETY: фича проверена
        unsafe {
            use std::arch::x86_64::*;
            let x = _mm256_loadu_pd(v.as_ptr());
            let r = crate::video_simd::round_half_away_nonneg(x);
            let mut out = [0f64; 4];
            _mm256_storeu_pd(out.as_mut_ptr(), r);
            out
        }
    }
}

/// Сглаженный параметр: one-pole к цели, ~20 мс. Не «телепортируется»
/// при set_target — только в reset().
#[derive(Debug, Clone, Copy)]
pub struct SmoothedParam {
    current: f32,
    target: f32,
    alpha: f32,
}

impl SmoothedParam {
    pub fn new(value: f32) -> SmoothedParam {
        // 1 - exp(-1 / (0.020 * 48000))
        SmoothedParam { current: value, target: value, alpha: 1.0 - (-1.0f32 / 960.0).exp() }
    }
    pub fn set_target(&mut self, t: f32) {
        self.target = t;
    }
    pub fn reset_to_target(&mut self) {
        self.current = self.target;
    }
    #[inline]
    pub fn tick(&mut self) -> f32 {
        self.current += (self.target - self.current) * self.alpha;
        self.current
    }
    pub fn value(&self) -> f32 {
        self.current
    }
}

// ---------------------------------------------------------------------------
// Freeverb
// ---------------------------------------------------------------------------

/// Классические тюнинги freeverb (44.1 kHz), масштабируются к 48 kHz
/// и параметру delay (базовая точка delay_ms = 100).
const COMB_TUNINGS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNINGS: [usize; 4] = [556, 441, 341, 225];
const STEREO_SPREAD: usize = 23;

struct Comb {
    buf: Vec<f32>,
    pos: usize,
    filter_state: f32,
}

impl Comb {
    fn new(len: usize) -> Comb {
        Comb { buf: vec![0.0; len.max(1)], pos: 0, filter_state: 0.0 }
    }
    #[inline]
    fn process(&mut self, input: f32, damp: f32, feedback: f32) -> f32 {
        let out = self.buf[self.pos];
        self.filter_state = out * (1.0 - damp) + self.filter_state * damp;
        self.buf[self.pos] = input + self.filter_state * feedback;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}

struct Allpass {
    buf: Vec<f32>,
    pos: usize,
}

impl Allpass {
    fn new(len: usize) -> Allpass {
        Allpass { buf: vec![0.0; len.max(1)], pos: 0 }
    }
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let bufout = self.buf[self.pos];
        let out = bufout - input;
        self.buf[self.pos] = input + bufout * 0.5;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
}

/// Стерео-freeverb. Смена delay пересоздаёт линии (это структурная смена
/// геометрии — неизбежно), но wet/dry сглажены и НЕ трогают буферы.
pub struct Freeverb {
    combs: [Vec<Comb>; CH],
    allpasses: [Vec<Allpass>; CH],
    wet: SmoothedParam,
    delay_ms: f32,
    damp: f32,
    feedback: f32,
    wet_gain: f32,
}

impl Freeverb {
    pub fn new(mix: f32, delay_ms: f32, damp: f32, decay: f32, wet_gain: f32) -> Freeverb {
        let scale = |base: usize, extra: usize| -> usize {
            let s = base as f32 * (AUDIO_SAMPLE_RATE as f32 / 44_100.0) * (delay_ms / 100.0);
            (s as usize + extra).max(8)
        };
        let combs = std::array::from_fn(|ch| {
            COMB_TUNINGS
                .iter()
                .map(|&t| Comb::new(scale(t, ch * STEREO_SPREAD)))
                .collect::<Vec<_>>()
        });
        let allpasses = std::array::from_fn(|ch| {
            ALLPASS_TUNINGS
                .iter()
                .map(|&t| Allpass::new(scale(t, ch * STEREO_SPREAD)))
                .collect::<Vec<_>>()
        });
        Freeverb {
            combs,
            allpasses,
            wet: SmoothedParam::new(mix.clamp(0.0, 1.0)),
            delay_ms,
            damp: damp.clamp(0.0, 0.99),
            feedback: decay.clamp(0.0, 0.98),
            wet_gain: wet_gain.max(0.0),
        }
    }

    pub fn set_params(&mut self, mix: f32, delay_ms: f32, damp: f32, decay: f32, wet_gain: f32) {
        self.wet.set_target(mix.clamp(0.0, 1.0));
        self.damp = damp.clamp(0.0, 0.99);
        self.feedback = decay.clamp(0.0, 0.98);
        self.wet_gain = wet_gain.max(0.0);
        // структурная смена геометрии: пересоздание допускается только при
        // реально другом delay (порог 0.5 мс, как в старом коде, но wet/dry
        // это не трогает)
        if (delay_ms - self.delay_ms).abs() > 0.5 {
            let keep_wet = self.wet;
            *self = Freeverb::new(keep_wet.target, delay_ms, damp, decay, wet_gain);
            self.wet = keep_wet;
        }
    }

    /// Только цель wet-mix (автоматизация) — сглаженно, буферы не трогаются.
    pub fn set_mix(&mut self, mix: f32) {
        self.wet.set_target(mix.clamp(0.0, 1.0));
    }

    /// Хвост: время затухания до −60 dB (t60) по максимальной comb-линии.
    pub fn tail(&self) -> Tick {
        let max_len = self.combs[CH - 1].iter().map(|c| c.buf.len()).max().unwrap_or(1);
        tail_frames_for(max_len as f32, self.feedback)
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        for frame in samples.chunks_exact_mut(CH) {
            let wet_gain = self.wet.tick();
            for (ch, s) in frame.iter_mut().enumerate() {
                let dry = *s;
                let input = dry * 0.015;
                let mut wet = 0.0;
                for comb in &mut self.combs[ch] {
                    wet += comb.process(input, self.damp, self.feedback);
                }
                for ap in &mut self.allpasses[ch] {
                    wet = ap.process(wet);
                }
                *s = dry * (1.0 - wet_gain) + wet * self.wet_gain * wet_gain;
            }
        }
    }

    pub fn reset(&mut self) {
        for ch in 0..CH {
            for c in &mut self.combs[ch] {
                c.buf.fill(0.0);
                c.filter_state = 0.0;
            }
            for a in &mut self.allpasses[ch] {
                a.buf.fill(0.0);
            }
        }
        self.wet.reset_to_target();
    }
}

// ---------------------------------------------------------------------------
// Warp (варispeed через rubato)
// ---------------------------------------------------------------------------

/// Читает источник ПОСЛЕДОВАТЕЛЬНО и выдаёт клип-локальный поток с
/// изменённой скоростью (rate < 1 — slowed, питч падает: это желаемая
/// эстетика slowed&reverb). Ни пропусков, ни дублей: source-позиция —
/// внутренний счётчик rubato, не пересчитывается извне.
pub struct Warp {
    rs: SincFixedOut<f32>,
    rate: f64,
    /// Следующий кадр источника, который будет запрошен у pull.
    src_cursor: i64,
    /// Готовый выход (interleaved), ещё не отданный.
    ready: std::collections::VecDeque<f32>,
    underruns: u64,
}

const WARP_CHUNK_OUT: usize = 1024;

impl Warp {
    pub fn new(rate: f64, src_start: i64) -> Warp {
        let rate = rate.clamp(0.25, 32.0);
        let params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        // ratio = f_out / f_in = 1 / rate (rate 0.5 → выхода вдвое больше)
        let rs = SincFixedOut::<f32>::new(1.0 / rate, 32.0, params, WARP_CHUNK_OUT, CH)
            .expect("rubato init");
        Warp {
            rs,
            rate,
            src_cursor: src_start,
            ready: std::collections::VecDeque::new(),
            underruns: 0,
        }
    }

    pub fn set_rate(&mut self, rate: f64) {
        let rate = rate.clamp(0.25, 32.0);
        if (rate - self.rate).abs() > 1e-9 {
            self.rate = rate;
            // ramp=true: rubato плавно ведёт ratio к цели внутри чанка
            let _ = self.rs.set_resample_ratio(1.0 / rate, true);
        }
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Выдать `out.len()/CH` кадров, дотягивая источник через `pull`.
    /// `pull(frame, buf)` заполняет interleaved-кадры источника, начиная с
    /// `frame`; false = не удалось (underrun, будет тишина).
    pub fn produce(
        &mut self,
        kind: PullKind,
        out: &mut [f32],
        pull: &mut dyn FnMut(PullKind, i64, &mut [f32]) -> bool,
    ) {
        let need_frames = out.len() / CH;
        while self.ready.len() < need_frames * CH {
            let in_frames = self.rs.input_frames_next();
            let mut input = vec![0f32; in_frames * CH];
            if !pull(kind, self.src_cursor, &mut input) {
                self.underruns += 1;
                input.fill(0.0);
            }
            self.src_cursor += in_frames as i64;
            // interleaved → planar
            let planar: Vec<Vec<f32>> =
                (0..CH).map(|ch| (0..in_frames).map(|f| input[f * CH + ch]).collect()).collect();
            let out_planar = self.rs.process(&planar, None).expect("rubato process");
            let produced = out_planar[0].len();
            for f in 0..produced {
                for chd in out_planar.iter().take(CH) {
                    self.ready.push_back(chd[f]);
                }
            }
        }
        for s in out.iter_mut() {
            *s = self.ready.pop_front().unwrap();
        }
    }

    /// Переставить источник БЕЗ пересоздания ресемплера — шов заворота
    /// (см. `ClipProcessor::seam`): внутреннее состояние sinc'а живёт
    /// дальше, выбрасывается только произведённый, но не отданный выход
    /// (он посчитан от прежней позиции источника).
    pub fn seek_source(&mut self, src: i64) {
        self.src_cursor = src;
        self.ready.clear();
    }

    pub fn underruns(&self) -> u64 {
        self.underruns
    }

    /// Текущий курсор источника (диагностика/тесты).
    pub fn src_cursor(&self) -> i64 {
        self.src_cursor
    }
}

// ---------------------------------------------------------------------------
// Процессор клипа: цепочка FX поверх (возможно варпнутого) источника
// ---------------------------------------------------------------------------

fn tail_frames_for(max_comb_len: f32, feedback: f32) -> Tick {
    let fb = feedback.clamp(0.01, 0.98);
    let db_per_pass = -20.0 * fb.log10();
    Tick::from_audio_frames((max_comb_len.max(8.0) * (60.0 / db_per_pass)) as i64)
}

/// Значение rate в прогрессе t (линейная интерполяция, края зажаты).
pub fn rate_at(base: f64, automation: &[SlowedKeyframe], t: f64) -> f64 {
    interp(automation, t, base, |k| k.rate)
}

/// Значение mix в прогрессе t.
pub fn mix_at(base: f32, automation: &[SlowedKeyframe], t: f64) -> f32 {
    interp(automation, t, base as f64, |k| k.mix as f64) as f32
}

fn interp(kfs: &[SlowedKeyframe], t: f64, base: f64, get: impl Fn(&SlowedKeyframe) -> f64) -> f64 {
    if kfs.is_empty() {
        return base;
    }
    if t <= kfs[0].t {
        return get(&kfs[0]);
    }
    for w in kfs.windows(2) {
        if t <= w[1].t {
            let span = (w[1].t - w[0].t).max(1e-12);
            let a = (t - w[0].t) / span;
            return get(&w[0]) * (1.0 - a) + get(&w[1]) * a;
        }
    }
    get(&kfs[kfs.len() - 1])
}

// ---------------------------------------------------------------------------
// SOURCE-домен автоматизации (п.1, решение пользователя): kf.t — позиция
// В МАТЕРИАЛЕ (кадры источника от source_in, нормированные clip_len).
// Кейфрейм приклеен к материалу: добавление РАННЕГО кейфрейма меняет
// момент выходного времени, но не материал, на котором срабатывают поздние.
//
// Карта: rate(s) кусочно-линеен по s; T(s) = ∫₀ˢ ds′/rate(s′) — выходной
// прогресс (доли clip_len), при котором варп доиграл до source-доли s.
// S(t) = T⁻¹(t). Кусочно-аналитические формы (ln/expm1) — без итераций,
// точность ~1e-15, детерминизм libm (golden закреплён на linux x86_64).
// Rate внутри карты зажат тем же клампом, что и Warp — иначе карта
// разошлась бы с фактическим потреблением.
// ---------------------------------------------------------------------------

const RATE_CLAMP: (f64, f64) = (0.25, 32.0);

fn eff_rate(base: f64, automation: &[SlowedKeyframe], s: f64) -> f64 {
    rate_at(base, automation, s).clamp(RATE_CLAMP.0, RATE_CLAMP.1)
}

/// Δτ на сегменте [s0,s1] с линейным rate r0→r1: ∫ds/rate.
fn seg_dt(ds: f64, r0: f64, r1: f64) -> f64 {
    if (r1 - r0).abs() < 1e-9 { ds * 2.0 / (r0 + r1) } else { ds * (r1.ln() - r0.ln()) / (r1 - r0) }
}

/// Δs, потребляемая за Δτ на сегменте, начинающемся с rate r0 и наклоном
/// b = dr/ds: s(τ) = r0·(e^{bτ} − 1)/b.
fn seg_ds(dt: f64, r0: f64, b: f64) -> f64 {
    if (b * dt).abs() < 1e-12 { r0 * dt } else { r0 * (b * dt).exp_m1() / b }
}

/// Сегменты (s0, s1, r0, r1) профиля по source-домену; последний — хвост
/// с константным rate до бесконечности (s1 = f64::INFINITY).
fn segments(base: f64, kfs: &[SlowedKeyframe]) -> Vec<(f64, f64, f64, f64)> {
    let cl = |r: f64| r.clamp(RATE_CLAMP.0, RATE_CLAMP.1);
    if kfs.is_empty() {
        return vec![(0.0, f64::INFINITY, cl(base), cl(base))];
    }
    let mut out = Vec::with_capacity(kfs.len() + 1);
    let first = cl(kfs[0].rate);
    if kfs[0].t > 0.0 {
        out.push((0.0, kfs[0].t, first, first));
    }
    for w in kfs.windows(2) {
        if w[1].t > w[0].t {
            out.push((w[0].t, w[1].t, cl(w[0].rate), cl(w[1].rate)));
        }
    }
    let last = cl(kfs[kfs.len() - 1].rate);
    out.push((kfs[kfs.len() - 1].t, f64::INFINITY, last, last));
    out
}

/// T(s): выходной прогресс (доли clip_len), при котором потреблена
/// source-доля s. Строго возрастает. s < 0 — левая «ручка» перехода
/// (п.138): линейная экстраполяция скоростью первого сегмента.
pub fn output_time_for_source(base: f64, automation: &[SlowedKeyframe], s: f64) -> f64 {
    if s < 0.0 {
        return s / segments(base, automation)[0].2;
    }
    let mut acc = 0.0;
    for (s0, s1, r0, r1) in segments(base, automation) {
        if s <= s0 {
            break;
        }
        let hi = s.min(s1);
        let frac = if s1.is_finite() && s1 > s0 { (hi - s0) / (s1 - s0) } else { 0.0 };
        let r_hi = if s1.is_finite() { r0 + (r1 - r0) * frac } else { r0 };
        acc += seg_dt(hi - s0, r0, r_hi);
        if hi >= s {
            break;
        }
    }
    acc
}

/// S(t) = T⁻¹: source-доля, потреблённая к выходному прогрессу t
/// (t может быть > 1 — хвост за клипом; rate за последним kf константен;
/// t < 0 — левая «ручка» перехода, п.138: линейно скоростью первого
/// сегмента, «за краями — крайние»). Материал ручек НЕ сдвигает
/// нормировку автоматизации — знаменатель остаётся clip_len.
pub fn source_progress(base: f64, automation: &[SlowedKeyframe], t: f64) -> f64 {
    if t < 0.0 {
        return t * segments(base, automation)[0].2;
    }
    let mut tau_acc = 0.0;
    let mut s_acc = 0.0;
    for (s0, s1, r0, r1) in segments(base, automation) {
        let seg_span = s1 - s0;
        let seg_tau = if s1.is_finite() { seg_dt(seg_span, r0, r1) } else { f64::INFINITY };
        if tau_acc + seg_tau >= t {
            let rest = t - tau_acc;
            let b = if s1.is_finite() && seg_span > 0.0 { (r1 - r0) / seg_span } else { 0.0 };
            return s_acc + seg_ds(rest, r0, b);
        }
        tau_acc += seg_tau;
        s_acc += seg_span;
    }
    s_acc
}

/// Средняя скорость = потреблённая доля источника за весь клип: S(1).
pub fn avg_rate(base: f64, automation: &[SlowedKeyframe]) -> f64 {
    source_progress(base, automation, 1.0)
}

/// Скорость воспроизведения цепочки (1.0 = без варпа).
pub fn chain_rate(fx: &[FxInstance]) -> f64 {
    fx.iter()
        .find_map(|f| match f {
            FxInstance::SlowedReverb { rate, .. } => Some(rate.clamp(0.25, 32.0)),
            _ => None,
        })
        .unwrap_or(1.0)
}

/// Средняя скорость цепочки с учётом автоматизации: определяет, сколько
/// источника съедает клип (для source-окна и длительности).
pub fn chain_avg_rate(fx: &[FxInstance]) -> f64 {
    fx.iter()
        .find_map(|f| match f {
            FxInstance::SlowedReverb { rate, automation, .. } => Some(avg_rate(*rate, automation)),
            _ => None,
        })
        .unwrap_or(1.0)
}

/// Длина хвоста цепочки БЕЗ создания state (для расширения окна клипа
/// в графе до того, как процессор построен).
/// Запас источника на прайминг rubato (sinc_len 128 × oversampling) и
/// чанк-квантование варпа: реальный курсор источника может обгонять
/// rate_integral на групповую задержку ресемплера (~3к кадров).
pub const WARP_SOURCE_MARGIN_FRAMES: i64 = 8192;

/// Окно источника (кадры ОТНОСИТЕЛЬНО source_in), которое ClipProcessor
/// реально потребит, выдавая клип-локальные кадры `local`, включая pre-roll
/// перед local.start и прайминг варпа. Для цепочки без варпа — линейно и
/// без запаса. ЕДИНЫЙ источник правды для prefetch (wanted) и рендера
/// (window_end): расхождение этих двух расчётов = вечный промах кэша и
/// тишина (регрессия п.13, тест 17).
pub fn chain_source_window(
    fx: &[FxInstance],
    clip_len_frames: i64,
    local: std::ops::Range<i64>,
) -> std::ops::Range<i64> {
    chain_source_window_ext(fx, clip_len_frames, 0..clip_len_frames.max(1), local)
}

/// То же, но для клипа с «ручками» перехода (п.138): material — допустимый
/// клип-локальный диапазон [−head_ext, clip_len + tail_ext); нормировка
/// автоматизации остаётся по clip_len. Для material = 0..len поведение
/// байт-идентично chain_source_window (тест 17 не двигается).
pub fn chain_source_window_ext(
    fx: &[FxInstance],
    clip_len_frames: i64,
    material: std::ops::Range<i64>,
    local: std::ops::Range<i64>,
) -> std::ops::Range<i64> {
    let len = clip_len_frames.max(1);
    let preroll = chain_tail(fx).to_audio_frames().max(MIN_PREROLL_FRAMES);
    let l0 = (local.start - preroll).max(material.start).min(material.end);
    let l1 = local.end.clamp(material.start, material.end);
    let has_warp = fx.iter().any(|f| matches!(f, FxInstance::SlowedReverb { .. }));
    if !has_warp {
        return l0..l1.max(l0);
    }
    let (rate, automation) = fx
        .iter()
        .find_map(|f| match f {
            FxInstance::SlowedReverb { rate, automation, .. } => {
                Some((*rate, automation.as_slice()))
            }
            _ => None,
        })
        .unwrap_or((1.0, &[]));
    // окно источника не уходит ниже нуля только когда материал начинается
    // с тела клипа (старое поведение); левая ручка легально отрицательна
    let s_floor = if material.start >= 0 { 0 } else { i64::MIN };
    let t0 = l0 as f64 / len as f64;
    let t1 = l1 as f64 / len as f64;
    let s0 = ((len as f64 * source_progress(rate, automation, t0)).floor() as i64)
        .saturating_sub(WARP_SOURCE_MARGIN_FRAMES)
        .max(s_floor);
    let s1 = (len as f64 * source_progress(rate, automation, t1)).ceil() as i64
        + WARP_SOURCE_MARGIN_FRAMES;
    s0..s1.max(s0)
}

pub fn chain_tail(fx: &[FxInstance]) -> Tick {
    fx.iter()
        .map(|f| match f {
            FxInstance::SlowedReverb { mix, reverb_delay_ms, decay, automation, .. }
                if *mix > 0.0 || automation.iter().any(|k| k.mix > 0.0) =>
            {
                reverb_tail_for_delay(*reverb_delay_ms, *decay)
            }
            _ => Tick::ZERO,
        })
        .max()
        .unwrap_or(Tick::ZERO)
}

fn reverb_tail_for_delay(delay_ms: f32, decay: f32) -> Tick {
    let max_comb = COMB_TUNINGS[COMB_TUNINGS.len() - 1] as f32
        * (AUDIO_SAMPLE_RATE as f32 / 44_100.0)
        * (delay_ms / 100.0)
        + ((CH - 1) * STEREO_SPREAD) as f32;
    tail_frames_for(max_comb, decay)
}

/// Отпечаток СТРУКТУРЫ цепочки (виды FX по порядку). Совпадает → state
/// переиспользуется, параметры доезжают через set_params (сглаженно).
pub fn chain_fingerprint(fx: &[FxInstance]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for f in fx {
        std::mem::discriminant(f).hash(&mut h);
    }
    h.finish()
}

/// Стейтфул-процессор аудио-клипа. Владелец — аудио-render-поток (kt-engine,
/// FxStateStore). Читается строго последовательно; разрыв → reset + pre-roll.
pub struct ClipProcessor {
    fingerprint: u64,
    /// Клип-локальный кадр, который выйдет следующим (маркер непрерывности).
    next_local: i64,
    src_in: i64,
    warp: Option<Warp>,
    reverb: Option<Freeverb>,
    static_gain: f32,
    rate: f64,
    base_mix: f32,
    automation: Vec<SlowedKeyframe>,
    /// Длина клипа в кадрах — знаменатель прогресса автоматизации.
    clip_len: i64,
    /// «Ручки» перехода (п.138): материал клип-локально расширен до
    /// [−head_ext, clip_len + tail_ext). Нормировку автоматизации НЕ
    /// двигают — знаменатель остаётся clip_len.
    head_ext: i64,
    tail_ext: i64,
    /// Курсор источника для пути без варпа (rate == 1, без автоматизации).
    plain_cursor: i64,
}

impl ClipProcessor {
    pub fn new(fx: &[FxInstance], src_in_frames: i64, clip_len_frames: i64) -> ClipProcessor {
        let mut p = ClipProcessor {
            fingerprint: chain_fingerprint(fx),
            next_local: i64::MIN,
            src_in: src_in_frames,
            warp: None,
            reverb: None,
            static_gain: 1.0,
            rate: 1.0,
            base_mix: 0.0,
            automation: Vec::new(),
            clip_len: clip_len_frames.max(1),
            head_ext: 0,
            tail_ext: 0,
            plain_cursor: src_in_frames,
        };
        p.apply_params(fx, true);
        p
    }

    /// Обновить длину клипа (trim) без сброса состояния.
    pub fn set_clip_len(&mut self, clip_len_frames: i64) {
        self.clip_len = clip_len_frames.max(1);
    }

    /// «Ручки» перехода (п.138): разрешить чтение [−head_ext,
    /// clip_len + tail_ext). Без сброса состояния (структура цепочки та же);
    /// dry-гейт сдвигается на tail_ext, нормировка автоматизации — нет.
    pub fn set_material_ext(&mut self, head_ext: i64, tail_ext: i64) {
        self.head_ext = head_ext.max(0);
        self.tail_ext = tail_ext.max(0);
    }

    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Донести параметры. `structural` = первый раз (создание объектов).
    pub fn apply_params(&mut self, fx: &[FxInstance], structural: bool) {
        let mut gain = 1.0f32;
        for f in fx {
            match f {
                FxInstance::Gain { db } => gain *= 10f32.powf(db / 20.0),
                FxInstance::SlowedReverb {
                    rate,
                    mix,
                    reverb_delay_ms,
                    damp,
                    decay,
                    wet_gain,
                    automation,
                } => {
                    self.rate = rate.clamp(0.25, 32.0);
                    self.base_mix = *mix;
                    self.automation = automation.clone();
                    if structural {
                        self.reverb =
                            Some(Freeverb::new(*mix, *reverb_delay_ms, *damp, *decay, *wet_gain));
                    } else if let Some(rv) = &mut self.reverb {
                        rv.set_params(*mix, *reverb_delay_ms, *damp, *decay, *wet_gain);
                    }
                    if let Some(w) = &mut self.warp
                        && self.automation.is_empty()
                    {
                        // с автоматизацией produce() выставит по позиции сам
                        w.set_rate(self.rate);
                    }
                }
                // видео-FX аудио-процессором игнорируются
                _ => {}
            }
        }
        self.static_gain = gain;
    }

    fn has_warp(&self) -> bool {
        (self.rate - 1.0).abs() > 1e-9
            || self.automation.iter().any(|k| (k.rate - 1.0).abs() > 1e-9)
    }

    pub fn tail(&self) -> Tick {
        self.reverb.as_ref().map(|r| r.tail()).unwrap_or(Tick::ZERO)
    }

    pub fn tail_frames(&self) -> i64 {
        self.tail().to_audio_frames()
    }

    /// Полный сброс состояния и permission на новый seek.
    pub fn reset(&mut self) {
        self.warp = None;
        if let Some(r) = &mut self.reverb {
            r.reset();
        }
        self.next_local = i64::MIN;
    }

    /// Прочитать кадры [local, local + out/CH) клип-локального потока.
    /// `pull(src_frame, buf)` — источник (false = underrun).
    /// Возвращает счётчик underrun'ов warp-пути.
    pub fn read(
        &mut self,
        local: i64,
        out: &mut [f32],
        pull: &mut dyn FnMut(PullKind, i64, &mut [f32]) -> bool,
    ) -> u64 {
        if self.next_local != local {
            if self.next_local == i64::MIN {
                // Холодный старт или seek (`reset`): reset + pre-roll,
                // чтобы реверб и ресемплер были прогреты детерминированно.
                self.hard_seek(local, pull);
            } else {
                // ШОВ: позиция прыгнула, а ВЫХОД непрерывен (заворот
                // loop-региона, сдвиг клипа мышью при игре). Реверб
                // дозванивает через шов — как в v1; пре-ролла нет, греть
                // нечего. Это же снимает пересоздание sinc-ресемплера на
                // каждом круге (при регионе короче бёрста их было
                // несколько за один callback — «шумит компьютер», п.166).
                self.seam(local);
            }
        }
        let before = self.warp.as_ref().map(|w| w.underruns()).unwrap_or(0);
        self.produce(PullKind::Output, local, out, pull);
        self.next_local = local + (out.len() / CH) as i64;
        self.warp.as_ref().map(|w| w.underruns()).unwrap_or(0) - before
    }

    /// Смещение источника (кадры от src_in) для клип-локальной позиции:
    /// S(t) — инверсия карты source-домена (см. output_time_for_source).
    fn source_offset(&self, local: i64) -> i64 {
        let t = local as f64 / self.clip_len as f64;
        (self.clip_len as f64 * source_progress(self.rate, &self.automation, t)).round() as i64
    }

    /// Разрыв ПОЗИЦИИ при непрерывном ВЫХОДЕ: источник переставляется,
    /// состояние FX живёт. Ни reset'а, ни пре-ролла (см. `read`).
    fn seam(&mut self, local: i64) {
        let src_start = self.src_in + self.source_offset(local);
        if self.has_warp() {
            let t = local as f64 / self.clip_len as f64;
            let r = rate_at(self.rate, &self.automation, t);
            match &mut self.warp {
                Some(w) => {
                    w.set_rate(r);
                    w.seek_source(src_start);
                }
                None => self.warp = Some(Warp::new(r, src_start)),
            }
        } else {
            self.warp = None;
        }
        self.plain_cursor = src_start;
    }

    fn hard_seek(&mut self, local: i64, pull: &mut dyn FnMut(PullKind, i64, &mut [f32]) -> bool) {
        if let Some(r) = &mut self.reverb {
            r.reset();
        }
        // pre-roll может заходить в левую «ручку», но не раньше начала
        // материала (−head_ext)
        let preroll = self.tail_frames().max(MIN_PREROLL_FRAMES).min((local + self.head_ext).max(0));
        let start = local - preroll;
        // позиция источника для local=start (интеграл автоматизации)
        let src_start = self.src_in + self.source_offset(start);
        if self.has_warp() {
            let t = start as f64 / self.clip_len as f64;
            let mut w = Warp::new(rate_at(self.rate, &self.automation, t), src_start);
            w.set_rate(rate_at(self.rate, &self.automation, t));
            self.warp = Some(w);
        } else {
            self.warp = None;
        }
        self.plain_cursor = src_start;
        // прогрев: рендерим в мусор
        let mut scratch = vec![0f32; 1024 * CH];
        let mut done = 0i64;
        while done < preroll {
            let n = (preroll - done).min(1024) as usize;
            self.produce(PullKind::Warmup, start + done, &mut scratch[..n * CH], pull);
            done += n as i64;
        }
    }

    /// Максимальный шаг оценки автоматизации: буфер режется на чанки этого
    /// размера НЕЗАВИСИМО от размера запроса — офлайн-рендер, отдающий клип
    /// одним куском, обязан следовать профилю так же, как playback
    /// по-burst'ово. (Регрессия: оценка один раз на весь буфер давала
    /// константное замедление по значению в середине клипа и оборванный
    /// конец источника; поймано на слух, закрыто тестами
    /// t4_automation_changes_rate_over_time / _consumes_source_by_integral.)
    const AUTOMATION_STEP_FRAMES: usize = 1024;

    fn produce(
        &mut self,
        kind: PullKind,
        local: i64,
        out: &mut [f32],
        pull: &mut dyn FnMut(PullKind, i64, &mut [f32]) -> bool,
    ) {
        let total = out.len() / CH;
        let mut done = 0usize;
        while done < total {
            let n = (total - done).min(Self::AUTOMATION_STEP_FRAMES);
            let chunk_local = local + done as i64;
            // автоматизация: цели на середину чанка; rubato/one-pole сглаживают
            if !self.automation.is_empty() {
                // source-домен (п.1): цель чанка — по ПОЗИЦИИ В МАТЕРИАЛЕ,
                // а не по выходному времени; ранние кейфреймы не сдвигают
                // материал поздних
                let tau = (chunk_local as f64 + n as f64 * 0.5) / self.clip_len as f64;
                let s = source_progress(self.rate, &self.automation, tau);
                if let Some(w) = &mut self.warp {
                    w.set_rate(eff_rate(self.rate, &self.automation, s));
                }
                if let Some(r) = &mut self.reverb {
                    r.set_mix(mix_at(self.base_mix, &self.automation, s));
                }
            }
            let sub = &mut out[done * CH..(done + n) * CH];
            match &mut self.warp {
                Some(w) => w.produce(kind, sub, pull),
                None => {
                    if !pull(kind, self.plain_cursor, sub) {
                        sub.fill(0.0);
                    }
                    self.plain_cursor += n as i64;
                }
            }
            // Гейт dry-выхода в OUTPUT-домене: за концом МАТЕРИАЛА (тело +
            // правая «ручка» перехода, п.138) сухой сигнал обязан быть нулём
            // (в хвосте дозванивает только реверб). Благодаря этому окно
            // ИСТОЧНИКА можно расширять с запасом на прайминг rubato, не
            // боясь утечки dry в хвост (дефект «тихий хвост»: sinc-ресемплер
            // видел нули за жёстко обрезанным окном и рано глушил конец).
            let gate_end = self.clip_len + self.tail_ext;
            if chunk_local + (n as i64) > gate_end {
                let cut = (gate_end - chunk_local).clamp(0, n as i64) as usize;
                sub[cut * CH..].fill(0.0);
            }
            if let Some(r) = &mut self.reverb {
                r.process(sub);
            }
            done += n;
        }
        if self.static_gain != 1.0 {
            for s in out.iter_mut() {
                *s *= self.static_gain;
            }
        }
    }
}

#[cfg(test)]
mod automation_tests {
    use super::*;

    fn kf(t: f64, rate: f64) -> SlowedKeyframe {
        SlowedKeyframe { t, rate, mix: 0.1 }
    }

    #[test]
    fn rate_interp_clamps_ends_and_is_linear() {
        let a = vec![kf(0.2, 1.0), kf(0.4, 0.5)];
        assert_eq!(rate_at(9.0, &a, 0.0), 1.0, "до первой точки — её значение (base игнорируется)");
        assert_eq!(rate_at(9.0, &a, 0.3), 0.75);
        assert_eq!(rate_at(9.0, &a, 1.0), 0.5);
    }

    /// Source-карта: round-trip T∘S = id, монотонность, сверка с
    /// независимым численным интегратором (профиль song1).
    #[test]
    fn source_map_roundtrip_and_matches_numeric() {
        let a = vec![
            kf(0.21616990640748734, 1.0),
            kf(0.2935781137508999, 0.35),
            kf(0.3822750179985601, 0.62),
            kf(0.6397019809160477, 0.6),
            kf(0.7557084786003034, 1.0),
        ];
        assert_eq!(source_progress(1.0, &a, 0.0), 0.0);
        let mut prev = 0.0;
        for i in 0..=100 {
            let t = i as f64 / 100.0;
            let s = source_progress(1.0, &a, t);
            assert!(s >= prev, "S(t) обязана быть неубывающей");
            prev = s;
            // round-trip
            let back = output_time_for_source(1.0, &a, s);
            assert!((back - t).abs() < 1e-9, "T(S({t})) = {back}");
        }
        // независимый численный интегратор: τ(s) трапециями по 1/rate
        let numeric_t_of_s = |s_target: f64| -> f64 {
            let n = 200_000;
            let mut acc = 0.0;
            let mut prev_inv = 1.0 / rate_at(1.0, &a, 0.0).clamp(0.25, 32.0);
            for i in 1..=n {
                let s = s_target * i as f64 / n as f64;
                let inv = 1.0 / rate_at(1.0, &a, s).clamp(0.25, 32.0);
                acc += (s_target / n as f64) * (prev_inv + inv) / 2.0;
                prev_inv = inv;
            }
            acc
        };
        for s in [0.1f64, 0.3, 0.5, 0.7] {
            let analytic = output_time_for_source(1.0, &a, s);
            let numeric = numeric_t_of_s(s);
            assert!(
                (analytic - numeric).abs() < 1e-6,
                "T({s}): аналитика {analytic} vs численно {numeric}"
            );
        }
    }

    #[test]
    fn no_automation_map_is_linear() {
        assert_eq!(source_progress(0.5, &[], 0.5), 0.25);
        assert_eq!(avg_rate(0.82, &[]), 0.82);
        assert!((output_time_for_source(0.5, &[], 0.25) - 0.5).abs() < 1e-12);
    }

    /// Якорь к материалу (чистая функция): ранний бамп меняет ВРЕМЯ
    /// достижения материала, но не rate НА материале.
    #[test]
    fn early_keyframe_shifts_time_not_material() {
        let a = vec![kf(0.4, 1.0), kf(0.5, 0.5), kf(0.7, 0.5), kf(0.75, 1.0)];
        let mut b = vec![kf(0.1, 1.0), kf(0.18, 2.0), kf(0.26, 2.0), kf(0.34, 1.0)];
        b.extend(a.iter().cloned());
        for s in [0.45f64, 0.55, 0.65, 0.72] {
            assert_eq!(
                rate_at(1.0, &a, s),
                rate_at(1.0, &b, s),
                "rate на материале s={s} не должен зависеть от раннего бампа"
            );
        }
        // а выходное время достижения s=0.5 при бампе меньше
        let ta = output_time_for_source(1.0, &a, 0.5);
        let tb = output_time_for_source(1.0, &b, 0.5);
        assert!(tb < ta, "ускорение раньше = материал достигается раньше: {tb} vs {ta}");
    }
}
