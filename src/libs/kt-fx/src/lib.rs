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

use kt_model::FxInstance;
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
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
        // SAFETY: фича проверена строкой выше, длины буферов — на
        // ответственности теста (он их и создаёт)
        unsafe { crate::video_simd::bilinear_into_avx2(src, x0, y0, cw, ch, ow, oh, out) };
    }

    /// AVX2-ядро noise (in-place по всему кадру, хвост столбцов не
    /// трогается — тест дорисовывает его скаляром).
    #[cfg(target_arch = "x86_64")]
    pub fn noise_avx2(rgba: &mut [u8], w: u32, h: u32, seed: u32, k: i64) {
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
        // SAFETY: фича проверена; длины — на ответственности теста
        unsafe { crate::video_simd::noise_avx2(rgba, w, h, seed, k) };
    }

    /// AVX2-ядро аддитива glow.
    #[cfg(target_arch = "x86_64")]
    pub fn glow_add_avx2(dst: &mut [u8], up: &[u8], s: f64, pixels: usize) {
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
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

    /// Скалярный эталон edges целиком (копия тела из video.rs) — t198
    /// сверяет с ним результат ПУБЛИЧНОЙ цепочки, идущей SIMD-путём.
    pub fn edges_scalar(rgba: &[u8], w: usize, h: usize, threshold: u32) -> Vec<u8> {
        let th = threshold.clamp(1, 255) as i32;
        let luma: Vec<i32> = rgba
            .chunks_exact(4)
            .map(|p| (77 * p[0] as i32 + 150 * p[1] as i32 + 29 * p[2] as i32) >> 8)
            .collect();
        let mut out = vec![0u8; rgba.len()];
        for p in out.chunks_exact_mut(4) {
            p[3] = 255;
        }
        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                let l = |dx: i64, dy: i64| {
                    luma[(y as i64 + dy) as usize * w + (x as i64 + dx) as usize]
                };
                let gx = -l(-1, -1) - 2 * l(-1, 0) - l(-1, 1) + l(1, -1) + 2 * l(1, 0) + l(1, 1);
                let gy = -l(-1, -1) - 2 * l(0, -1) - l(1, -1) + l(-1, 1) + 2 * l(0, 1) + l(1, 1);
                let mag = (((gx * gx + gy * gy) as f64).sqrt() as i32).min(255);
                let v = if mag < th { 0 } else { mag as u8 };
                let i = (y * w + x) * 4;
                out[i] = v;
                out[i + 1] = v;
                out[i + 2] = v;
            }
        }
        out
    }

    /// Скалярный эталон vhs целиком (копия тела из video.rs).
    pub fn vhs_scalar(
        rgba: &mut [u8],
        w: usize,
        h: usize,
        rgb_shift: u32,
        bleed: f64,
        burst_index: i64,
    ) {
        let shift = rgb_shift.clamp(0, 12) as i64;
        let bleed = bleed.clamp(0.0, 1.0);
        let line_off = ((burst_index / 3000) % 7) as usize;
        let mut row = vec![0u8; w * 4];
        for y in 0..h {
            let scan = if (y + line_off).is_multiple_of(4) { 1i64 } else { 0 };
            let r0 = y * w * 4;
            row.copy_from_slice(&rgba[r0..r0 + w * 4]);
            for x in 0..w {
                let i = r0 + x * 4;
                let xr = (x as i64 + shift + scan).clamp(0, w as i64 - 1) as usize;
                let xb = (x as i64 - shift - scan).clamp(0, w as i64 - 1) as usize;
                let r = row[xr * 4] as f64;
                let b = row[xb * 4 + 2] as f64;
                rgba[i] = (row[x * 4] as f64 * (1.0 - bleed) + r * bleed).round() as u8;
                rgba[i + 2] = (row[x * 4 + 2] as f64 * (1.0 - bleed) + b * bleed).round() as u8;
                if scan == 1 {
                    rgba[i + 1] = row[x * 4 + 1].saturating_sub(18);
                }
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
        assert!(std::is_x86_feature_detected!("avx2"), "AVX2 is required");
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
    /// Пред-задержка входа реверба (Р.33 С5): кольцо на канал. Пустое —
    /// прежний путь байт-в-байт.
    predelay: [Vec<f32>; CH],
    pre_pos: usize,
    predelay_ms: f32,
    /// Ширина стерео-хвоста 0..1: 1 — прежний «настоящий» стереореверб,
    /// 0 — общий моно-хвост на оба канала.
    width: f32,
}

impl Freeverb {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mix: f32,
        delay_ms: f32,
        damp: f32,
        decay: f32,
        wet_gain: f32,
        predelay_ms: f32,
        width: f32,
    ) -> Freeverb {
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
        let pre = (predelay_ms.clamp(0.0, 200.0) / 1000.0 * AUDIO_SAMPLE_RATE as f32) as usize;
        Freeverb {
            combs,
            allpasses,
            wet: SmoothedParam::new(mix.clamp(0.0, 1.0)),
            delay_ms,
            damp: damp.clamp(0.0, 0.99),
            feedback: decay.clamp(0.0, 0.98),
            wet_gain: wet_gain.max(0.0),
            predelay: std::array::from_fn(|_| vec![0.0; pre]),
            pre_pos: 0,
            predelay_ms: predelay_ms.clamp(0.0, 200.0),
            width: width.clamp(0.0, 1.0),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_params(
        &mut self,
        mix: f32,
        delay_ms: f32,
        damp: f32,
        decay: f32,
        wet_gain: f32,
        predelay_ms: f32,
        width: f32,
    ) {
        self.wet.set_target(mix.clamp(0.0, 1.0));
        self.damp = damp.clamp(0.0, 0.99);
        self.feedback = decay.clamp(0.0, 0.98);
        self.wet_gain = wet_gain.max(0.0);
        self.width = width.clamp(0.0, 1.0);
        // структурная смена геометрии: пересоздание допускается только при
        // реально другом delay (порог 0.5 мс, как в старом коде, но wet/dry
        // это не трогает); пред-задержка — такая же геометрия
        if (delay_ms - self.delay_ms).abs() > 0.5
            || (predelay_ms - self.predelay_ms).abs() > 0.5
        {
            let keep_wet = self.wet;
            *self =
                Freeverb::new(keep_wet.target, delay_ms, damp, decay, wet_gain, predelay_ms, width);
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
        // прежний путь (без пред-задержки, полная ширина) остаётся
        // ДОСЛОВНЫМ: на нём стоит GOLDEN_AUDIO
        let plain = self.predelay[0].is_empty() && self.width >= 1.0;
        for frame in samples.chunks_exact_mut(CH) {
            let wet_gain = self.wet.tick();
            if plain {
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
                continue;
            }
            let mut wets = [0.0f32; CH];
            for ch in 0..CH {
                let dry = frame[ch];
                let mut input = dry * 0.015;
                if !self.predelay[ch].is_empty() {
                    let pos = self.pre_pos % self.predelay[ch].len();
                    input = std::mem::replace(&mut self.predelay[ch][pos], input);
                }
                let mut wet = 0.0;
                for comb in &mut self.combs[ch] {
                    wet += comb.process(input, self.damp, self.feedback);
                }
                for ap in &mut self.allpasses[ch] {
                    wet = ap.process(wet);
                }
                wets[ch] = wet;
            }
            self.pre_pos = self.pre_pos.wrapping_add(1);
            // ширина: 1 — каналы как есть, 0 — общий моно-хвост
            let (a, b) = (0.5 + 0.5 * self.width, 0.5 - 0.5 * self.width);
            for ch in 0..CH {
                let other = wets[(ch + 1) % CH];
                let wet = wets[ch] * a + other * b;
                let dry = frame[ch];
                frame[ch] = dry * (1.0 - wet_gain) + wet * self.wet_gain * wet_gain;
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

/// Карта растяжения времени ПЕРЕЕХАЛА В kt-model (Р.39): её обязан звать
/// `clip_slice` — правило куска клипа, — а kt-model зависеть от kt-fx не
/// может. Здесь только реэкспорт: у формулы по-прежнему ОДИН дом, а движок
/// и тесты продолжают звать `kt_fx::…`, и ни один вызов не тронут.
pub use kt_model::{
    AutoProfiles, ParamPoints, RATE_CLAMP, avg_rate, chain_avg_rate, chain_rate, eff_rate,
    output_time_for_source, param_at, rate_at, source_progress, warp_material_ticks,
};


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
    auto: &AutoProfiles,
    clip_len_frames: i64,
    local: std::ops::Range<i64>,
) -> std::ops::Range<i64> {
    chain_source_window_ext(fx, auto, clip_len_frames, 0..clip_len_frames.max(1), local)
}

/// То же, но для клипа с «ручками» перехода (п.138): material — допустимый
/// клип-локальный диапазон [−head_ext, clip_len + tail_ext); нормировка
/// автоматизации остаётся по clip_len. Для material = 0..len поведение
/// байт-идентично chain_source_window (тест 17 не двигается).
pub fn chain_source_window_ext(
    fx: &[FxInstance],
    auto: &AutoProfiles,
    clip_len_frames: i64,
    material: std::ops::Range<i64>,
    local: std::ops::Range<i64>,
) -> std::ops::Range<i64> {
    let len = clip_len_frames.max(1);
    let preroll = chain_tail(fx, auto).to_audio_frames().max(MIN_PREROLL_FRAMES);
    let l0 = (local.start - preroll).max(material.start).min(material.end);
    let l1 = local.end.clamp(material.start, material.end);
    let has_warp = fx.iter().any(|f| matches!(f, FxInstance::SlowedReverb { .. }));
    if !has_warp {
        return l0..l1.max(l0);
    }
    let rate = fx
        .iter()
        .find_map(|f| match f {
            FxInstance::SlowedReverb { rate, .. } => Some(*rate),
            _ => None,
        })
        .unwrap_or(1.0);
    let automation = auto.rate_points(fx);
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

/// Сколько кадров источника событие потребляет ЦЕЛИКОМ: S(1)·len, без
/// запаса ресемплера и без пре-ролла.
///
/// Это **ОСЬ ЗЕРКАЛА** у reversed-клипа (Р.43, п.292) — ровно та же
/// величина, что `ClipWarp::used` у видео. До Р.43 аудио зеркалило от
/// `chain_source_window_ext(..).end`, а туда входит
/// `WARP_SOURCE_MARGIN_FRAMES` = 8192 кадра (170.7 мс): зеркальный
/// замедленный клип играл материал, сдвинутый на эти 170 мс, и звучал
/// «звуком от другого события» (претензия Р.43 п.3), а картинка на том же
/// событии оставалась на месте — A/V расходились.
pub fn chain_source_used(fx: &[FxInstance], auto: &AutoProfiles, clip_len_frames: i64) -> i64 {
    let len = clip_len_frames.max(1);
    let Some(rate) = fx.iter().find_map(|f| match f {
        FxInstance::SlowedReverb { rate, .. } => Some(*rate),
        _ => None,
    }) else {
        return len;
    };
    (len as f64 * source_progress(rate, auto.rate_points(fx), 1.0)).ceil() as i64
}

pub fn chain_tail(fx: &[FxInstance], auto: &AutoProfiles) -> Tick {
    let wet = auto.mix_points(fx);
    fx.iter()
        .map(|f| match f {
            FxInstance::SlowedReverb { mix, reverb_delay_ms, decay, .. }
                if *mix > 0.0 || wet.iter().any(|(_, v)| *v > 0.0) =>
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
    /// Множитель Gain-слотов БЕЗ кривой (константный путь — байт-в-байт
    /// как до Р.33, на нём стоит GOLDEN_AUDIO).
    static_gain: f32,
    /// СИЛА СЛОЯ (Р.38 п.12): слот смешения, индекс начала вклада слоя и
    /// базовое значение. `None` — слоя с силой нет, и весь путь ниже
    /// байт-в-байт прежний (на нём стоит GOLDEN_AUDIO).
    mix_slot: Option<usize>,
    mix_from: usize,
    base_layer_mix: f64,
    /// Реверб принадлежит СОБЫТИЮ, а не слою: тогда он звучит и в сухой
    /// стороне смешения. Движок держит ровно один реверб на цепочку,
    /// поэтому выбор сводится к одному флагу.
    reverb_is_layer: bool,
    /// Множитель Gain-слотов СЛОЯ без кривой: он идёт только в мокрую
    /// сторону. Без слоя равен 1.0 — цикл не выполняется вовсе.
    layer_gain: f32,
    /// Баланс каналов (Р.33 С5): множители левого и правого. Оба 1.0 —
    /// прежний путь, цикл не выполняется вовсе.
    pan_l: f32,
    pan_r: f32,
    /// Слоты Gain с КРИВОЙ баланса и сглаживание их множителей.
    pan_automated: bool,
    pan_slots: Vec<(usize, f64)>,
    auto_pan_l: SmoothedParam,
    auto_pan_r: SmoothedParam,
    /// Сглаженная громкость автоматизированных Gain-слотов: цель ставится
    /// раз в чанк, к ней едем one-pole — шаг чанка не щёлкает.
    auto_gain: SmoothedParam,
    /// Есть хоть один Gain-слот с кривой?
    gain_automated: bool,
    /// Автоматизированные Gain-слоты: (индекс, базовое значение db).
    gain_slots: Vec<(usize, f64)>,
    rate: f64,
    base_mix: f32,
    /// Кривые клипа в SOURCE-домене. Владеет ими аудио-render-поток
    /// (инвариант №2): ни локов, ни обращений к модели на кадре.
    auto: AutoProfiles,
    /// Индекс слота с варпом — по нему берутся профили rate/mix.
    warp_slot: usize,
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
    pub fn new(
        fx: &[FxInstance],
        auto: &AutoProfiles,
        src_in_frames: i64,
        clip_len_frames: i64,
    ) -> ClipProcessor {
        let mut p = ClipProcessor {
            fingerprint: chain_fingerprint(fx),
            next_local: i64::MIN,
            src_in: src_in_frames,
            warp: None,
            reverb: None,
            static_gain: 1.0,
            mix_slot: None,
            mix_from: usize::MAX,
            base_layer_mix: 1.0,
            reverb_is_layer: false,
            layer_gain: 1.0,
            pan_l: 1.0,
            pan_r: 1.0,
            pan_automated: false,
            pan_slots: Vec::new(),
            auto_pan_l: SmoothedParam::new(1.0),
            auto_pan_r: SmoothedParam::new(1.0),
            auto_gain: SmoothedParam::new(1.0),
            gain_automated: false,
            gain_slots: Vec::new(),
            rate: 1.0,
            base_mix: 0.0,
            auto: auto.clone(),
            warp_slot: fx
                .iter()
                .position(|f| matches!(f, FxInstance::SlowedReverb { .. }))
                .unwrap_or(0),
            clip_len: clip_len_frames.max(1),
            head_ext: 0,
            tail_ext: 0,
            plain_cursor: src_in_frames,
        };
        p.apply_params(fx, true);
        p
    }

    /// Профиль скорости этого клипа (пустой — варпа/кривой нет).
    fn rate_points(&self) -> &ParamPoints {
        self.auto.points(self.warp_slot, "rate")
    }

    /// Обновить кривые БЕЗ сброса состояния: правка кейфрейма не должна
    /// глушить хвост реверба (инвариант №3).
    pub fn set_auto(&mut self, auto: &AutoProfiles) {
        if &self.auto != auto {
            self.auto = auto.clone();
        }
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
        let (mut pan_l, mut pan_r) = (1.0f32, 1.0f32);
        self.gain_automated = false;
        self.pan_automated = false;
        // слот СИЛЫ слоя (Р.38 п.12) ищется до всего остального: от него
        // зависит, что попадёт в сухую сторону смешения
        self.mix_slot = fx.iter().position(|f| f.is_layer_mix());
        (self.mix_from, self.base_layer_mix) = match self.mix_slot {
            Some(i) => match &fx[i] {
                FxInstance::LayerMix { amount, from } => ((*from as usize).min(i), *amount),
                _ => (usize::MAX, 1.0),
            },
            None => (usize::MAX, 1.0),
        };
        self.reverb_is_layer = fx
            .iter()
            .position(|f| matches!(f, FxInstance::SlowedReverb { .. }))
            .map(|i| i >= self.mix_from)
            .unwrap_or(true);
        let mut layer_gain = 1.0f32;
        for (slot, f) in fx.iter().enumerate() {
            match f {
                // громкость с кривой считается по материалу в produce();
                // без кривой — прежним одним множителем на весь буфер
                FxInstance::Gain { db, pan } => {
                    if self.auto.points(slot, "db").is_empty() {
                        // громкость СЛОЯ идёт только в мокрую сторону:
                        // сила 0 обязана вернуть событие как было
                        if slot >= self.mix_from {
                            layer_gain *= 10f32.powf(db / 20.0);
                        } else {
                            gain *= 10f32.powf(db / 20.0);
                        }
                    } else {
                        self.gain_automated = true;
                    }
                    // баланс: линейный закон, на нуле оба множителя ровно
                    // 1.0 — прежний путь байт-в-байт
                    if self.auto.points(slot, "pan").is_empty() {
                        let p = pan.clamp(-1.0, 1.0);
                        pan_l *= (1.0 - p).min(1.0);
                        pan_r *= (1.0 + p).min(1.0);
                    } else {
                        self.pan_automated = true;
                    }
                }
                FxInstance::SlowedReverb {
                    rate,
                    mix,
                    reverb_delay_ms,
                    damp,
                    decay,
                    wet_gain,
                    predelay_ms,
                    width,
                    ..
                } => {
                    self.rate = rate.clamp(0.25, 32.0);
                    self.base_mix = *mix;
                    if structural {
                        self.reverb =
                            Some(Freeverb::new(
                            *mix,
                            *reverb_delay_ms,
                            *damp,
                            *decay,
                            *wet_gain,
                            *predelay_ms,
                            *width,
                        ));
                    } else if let Some(rv) = &mut self.reverb {
                        rv.set_params(
                            *mix,
                            *reverb_delay_ms,
                            *damp,
                            *decay,
                            *wet_gain,
                            *predelay_ms,
                            *width,
                        );
                    }
                    let has_rate_curve = !self.rate_points().is_empty();
                    if let Some(w) = &mut self.warp
                        && !has_rate_curve
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
        self.layer_gain = layer_gain;
        self.pan_l = pan_l;
        self.pan_r = pan_r;
        self.gain_slots = fx
            .iter()
            .enumerate()
            .filter_map(|(i, f)| match f {
                FxInstance::Gain { db, .. } if !self.auto.points(i, "db").is_empty() => {
                    Some((i, *db as f64))
                }
                _ => None,
            })
            .collect();
        self.pan_slots = fx
            .iter()
            .enumerate()
            .filter_map(|(i, f)| match f {
                FxInstance::Gain { pan, .. } if !self.auto.points(i, "pan").is_empty() => {
                    Some((i, *pan as f64))
                }
                _ => None,
            })
            .collect();
    }

    /// Баланс автоматизированных Gain-слотов в позиции материала s.
    /// Возвращает пару множителей (левый, правый) — тот же линейный
    /// закон, что у константного пути.
    fn pan_at(&self, s: f64) -> (f32, f32) {
        let (mut l, mut r) = (1.0f32, 1.0f32);
        for (slot, base) in &self.pan_slots {
            let p = self.auto.value(*slot, "pan", *base, s).clamp(-1.0, 1.0) as f32;
            l *= (1.0 - p).min(1.0);
            r *= (1.0 + p).min(1.0);
        }
        (l, r)
    }

    /// Множитель автоматизированных Gain-слотов в позиции материала s.
    fn gain_at(&self, s: f64) -> f32 {
        let mut g = 1.0f32;
        for (slot, base_db) in &self.gain_slots {
            let db = self.auto.value(*slot, "db", *base_db, s);
            g *= 10f32.powf(db as f32 / 20.0);
        }
        g
    }

    fn has_warp(&self) -> bool {
        (self.rate - 1.0).abs() > 1e-9
            || self.rate_points().iter().any(|(_, r)| (r - 1.0).abs() > 1e-9)
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

    /// Материальный якорь события (кадры от нуля файла).
    pub fn src_in(&self) -> i64 {
        self.src_in
    }

    /// Переставить материальный якорь, СОХРАНИВ состояние FX (Р.43, п.293).
    ///
    /// `chain_fingerprint` знает только ВИДЫ слотов, поэтому трим, разрез,
    /// punch-out и переанкоринг морф-куска меняли `source_in` при том же
    /// `ClipId` и той же структуре цепочки — и процессор продолжал читать
    /// от СТАРОГО якоря. `reset()` не помогал: `hard_seek`/`seam` считают
    /// позицию как `self.src_in + source_offset(local)`. Симптом —
    /// «звук от другого события откуда-то взялся».
    ///
    /// Это ШОВ, а не seek: выход непрерывен, реверб обязан дозвонить
    /// (инвариант №3).
    pub fn set_src_in(&mut self, src_in: i64) {
        if self.src_in == src_in {
            return;
        }
        self.src_in = src_in;
        if self.next_local != i64::MIN {
            let local = self.next_local;
            self.seam(local);
        }
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
        (self.clip_len as f64 * source_progress(self.rate, self.rate_points(), t)).round() as i64
    }

    /// Разрыв ПОЗИЦИИ при непрерывном ВЫХОДЕ: источник переставляется,
    /// состояние FX живёт. Ни reset'а, ни пре-ролла (см. `read`).
    fn seam(&mut self, local: i64) {
        let src_start = self.src_in + self.source_offset(local);
        if self.has_warp() {
            let t = local as f64 / self.clip_len as f64;
            let r = rate_at(self.rate, self.rate_points(), t);
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
            let mut w = Warp::new(rate_at(self.rate, self.rate_points(), t), src_start);
            w.set_rate(rate_at(self.rate, self.rate_points(), t));
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
            if !self.auto.is_empty() {
                // ДОМЕН СОБЫТИЯ (Р.37): цель чанка — по ВЫХОДНОМУ времени,
                // а не по позиции в материале. Ранний кейфрейм скорости
                // больше не сдвигает МОМЕНТ срабатывания поздних ключей —
                // ни своих, ни чужих (mix/gain/pan индексировались тем же
                // `s`). Правило середины чанка при этом стало ТОЧНЫМ:
                // подынтегральная функция линейна, а середина трапеции —
                // её точное среднее.
                let tau = (chunk_local as f64 + n as f64 * 0.5) / self.clip_len as f64;
                let s = tau;
                let target_rate = eff_rate(self.rate, self.rate_points(), s);
                if let Some(w) = &mut self.warp {
                    w.set_rate(target_rate);
                }
                if let Some(r) = &mut self.reverb {
                    // mix сглаживается one-pole внутри реверба; остальные
                    // параметры реверба — тем же чанком
                    let slot = self.warp_slot;
                    r.set_mix(self.auto.value(slot, "mix", self.base_mix as f64, s) as f32);
                }
                // громкость: кривая на любом Gain-слоте цепочки
                if self.gain_automated {
                    self.auto_gain.set_target(self.gain_at(s));
                }
                if self.pan_automated {
                    let (l, r) = self.pan_at(s);
                    self.auto_pan_l.set_target(l);
                    self.auto_pan_r.set_target(r);
                }
            }
            // цель силы слоя на середину чанка — тем же правилом, что и
            // остальные параметры (Р.38 п.12: фейды это её кривая)
            let mix_k = self.mix_slot.map(|slot| {
                let tau = (chunk_local as f64 + n as f64 * 0.5) / self.clip_len as f64;
                self.auto.value(slot, "amount", self.base_layer_mix, tau).clamp(0.0, 1.0) as f32
            });
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
            // СУХАЯ сторона смешения (Р.38 п.12): состояние ДО вклада
            // слоя. Реверб у движка ровно один на цепочку, поэтому вопрос
            // «чей он» решается одним флагом: реверб СОБЫТИЯ звучит и в
            // сухой стороне, реверб СЛОЯ — только в мокрой.
            // реверб СОБЫТИЯ звучит и в сухой стороне (снимок после
            // него), реверб СЛОЯ — только в мокрой (снимок до)
            let mut dry: Option<Vec<f32>> = match mix_k {
                Some(_) if self.reverb_is_layer => Some(sub.to_vec()),
                Some(_) => Some(Vec::new()),
                None => None,
            };
            if let Some(r) = &mut self.reverb {
                r.process(sub);
            }
            if let Some(d) = dry.as_mut()
                && d.is_empty()
            {
                *d = sub.to_vec();
            }
            // автоматизированная громкость — ПОСЛЕ реверба, как и
            // константная: цепочка кончается уровнем события
            if self.gain_automated {
                for f in 0..n {
                    let g = self.auto_gain.tick();
                    for ch in 0..CH {
                        sub[f * CH + ch] *= g;
                    }
                }
            }
            if self.pan_automated {
                for f in sub.chunks_exact_mut(CH) {
                    let (l, r) = (self.auto_pan_l.tick(), self.auto_pan_r.tick());
                    f[0] *= l;
                    if CH > 1 {
                        f[1] *= r;
                    }
                }
            }
            // громкость СЛОЯ — только на мокрую сторону, потом смешение
            if let Some(k) = mix_k {
                if self.layer_gain != 1.0 {
                    for v in sub.iter_mut() {
                        *v *= self.layer_gain;
                    }
                }
                if let Some(d) = &dry {
                    let ik = 1.0 - k;
                    for (v, dv) in sub.iter_mut().zip(d.iter()) {
                        *v = dv * ik + *v * k;
                    }
                }
            }
            done += n;
        }
        if self.static_gain != 1.0 {
            for s in out.iter_mut() {
                *s *= self.static_gain;
            }
        }
        // баланс — последним: на нуле множители 1.0 и цикл не идёт вовсе
        if self.pan_l != 1.0 || self.pan_r != 1.0 {
            for f in out.chunks_exact_mut(CH) {
                f[0] *= self.pan_l;
                if CH > 1 {
                    f[1] *= self.pan_r;
                }
            }
        }
    }
}

