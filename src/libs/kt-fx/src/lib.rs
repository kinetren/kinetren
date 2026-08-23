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
    pub fn produce(&mut self, out: &mut [f32], pull: &mut dyn FnMut(i64, &mut [f32]) -> bool) {
        let need_frames = out.len() / CH;
        while self.ready.len() < need_frames * CH {
            let in_frames = self.rs.input_frames_next();
            let mut input = vec![0f32; in_frames * CH];
            if !pull(self.src_cursor, &mut input) {
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

/// ∫₀ᵗ rate dτ (кусочно-линейно; края зажаты): маппинг «прогресс клипа →
/// потреблённая доля источника».
pub fn rate_integral(base: f64, automation: &[SlowedKeyframe], t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if automation.is_empty() {
        return base * t;
    }
    let mut acc = 0.0;
    let mut prev_t = 0.0f64;
    let mut prev_v = rate_at(base, automation, 0.0);
    for k in automation {
        if k.t >= t {
            break;
        }
        if k.t > prev_t {
            acc += (k.t - prev_t) * (prev_v + k.rate) / 2.0;
            prev_t = k.t;
            prev_v = k.rate;
        }
    }
    let v = rate_at(base, automation, t);
    acc + (t - prev_t) * (prev_v + v) / 2.0
}

/// Средняя скорость по клипу: ∫₀¹ rate. Определяет расход источника.
pub fn avg_rate(base: f64, automation: &[SlowedKeyframe]) -> f64 {
    rate_integral(base, automation, 1.0)
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
    let len = clip_len_frames.max(1);
    let preroll = chain_tail(fx).to_audio_frames().max(MIN_PREROLL_FRAMES);
    let l0 = (local.start - preroll).max(0).min(len);
    let l1 = local.end.clamp(0, len);
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
    let t0 = l0 as f64 / len as f64;
    let t1 = l1 as f64 / len as f64;
    let s0 = ((len as f64 * rate_integral(rate, automation, t0)).floor() as i64)
        .saturating_sub(WARP_SOURCE_MARGIN_FRAMES)
        .max(0);
    let s1 = (len as f64 * rate_integral(rate, automation, t1)).ceil() as i64
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
            plain_cursor: src_in_frames,
        };
        p.apply_params(fx, true);
        p
    }

    /// Обновить длину клипа (trim) без сброса состояния.
    pub fn set_clip_len(&mut self, clip_len_frames: i64) {
        self.clip_len = clip_len_frames.max(1);
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
                    if let Some(w) = &mut self.warp {
                        w.set_rate(rate_at(self.rate, &self.automation, 0.5_f64.min(1.0)));
                    }
                }
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
        pull: &mut dyn FnMut(i64, &mut [f32]) -> bool,
    ) -> u64 {
        if self.next_local != local {
            // Разрыв непрерывности (seek/первое чтение): reset + pre-roll,
            // чтобы реверб и ресемплер были прогреты детерминированно.
            self.hard_seek(local, pull);
        }
        let before = self.warp.as_ref().map(|w| w.underruns()).unwrap_or(0);
        self.produce(local, out, pull);
        self.next_local = local + (out.len() / CH) as i64;
        self.warp.as_ref().map(|w| w.underruns()).unwrap_or(0) - before
    }

    /// Смещение источника (кадры от src_in) для клип-локальной позиции:
    /// интеграл rate по прогрессу — с автоматизацией НЕ линейно.
    fn source_offset(&self, local: i64) -> i64 {
        let t = local as f64 / self.clip_len as f64;
        (self.clip_len as f64 * rate_integral(self.rate, &self.automation, t)).round() as i64
    }

    fn hard_seek(&mut self, local: i64, pull: &mut dyn FnMut(i64, &mut [f32]) -> bool) {
        if let Some(r) = &mut self.reverb {
            r.reset();
        }
        let preroll = self.tail_frames().max(MIN_PREROLL_FRAMES).min(local.max(0));
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
            self.produce(start + done, &mut scratch[..n * CH], pull);
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
        local: i64,
        out: &mut [f32],
        pull: &mut dyn FnMut(i64, &mut [f32]) -> bool,
    ) {
        let total = out.len() / CH;
        let mut done = 0usize;
        while done < total {
            let n = (total - done).min(Self::AUTOMATION_STEP_FRAMES);
            let chunk_local = local + done as i64;
            // автоматизация: цели на середину чанка; rubato/one-pole сглаживают
            if !self.automation.is_empty() {
                let t = (chunk_local as f64 + n as f64 * 0.5) / self.clip_len as f64;
                if let Some(w) = &mut self.warp {
                    w.set_rate(rate_at(self.rate, &self.automation, t));
                }
                if let Some(r) = &mut self.reverb {
                    r.set_mix(mix_at(self.base_mix, &self.automation, t));
                }
            }
            let sub = &mut out[done * CH..(done + n) * CH];
            match &mut self.warp {
                Some(w) => w.produce(sub, pull),
                None => {
                    if !pull(self.plain_cursor, sub) {
                        sub.fill(0.0);
                    }
                    self.plain_cursor += n as i64;
                }
            }
            // Гейт dry-выхода в OUTPUT-домене: за концом клипа сухой сигнал
            // обязан быть нулём (в хвосте дозванивает только реверб).
            // Благодаря этому окно ИСТОЧНИКА можно расширять с запасом на
            // прайминг rubato, не боясь утечки dry в хвост (дефект «тихий
            // хвост»: sinc-ресемплер видел нули за жёстко обрезанным окном
            // и рано глушил конец клипа).
            if chunk_local + (n as i64) > self.clip_len {
                let cut = (self.clip_len - chunk_local).clamp(0, n as i64) as usize;
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

    #[test]
    fn rate_integral_matches_trapezoid_and_is_monotone() {
        // кейфреймы из старого проекта song1 — контрольная сумма посчитана
        // независимо (трапеции)
        let a = vec![
            kf(0.21616990640748734, 1.0),
            kf(0.2935781137508999, 0.35),
            kf(0.3822750179985601, 0.62),
            kf(0.6397019809160477, 0.6),
            kf(0.7557084786003034, 1.0),
        ];
        let avg = avg_rate(1.0, &a);
        assert!((avg - 0.805566).abs() < 1e-5, "avg_rate = {avg}");
        // монотонность и согласованность с rate_at
        let mut prev = 0.0;
        for i in 0..=100 {
            let t = i as f64 / 100.0;
            let v = rate_integral(1.0, &a, t);
            assert!(v >= prev, "интеграл обязан быть неубывающим");
            prev = v;
        }
        assert_eq!(rate_integral(1.0, &a, 0.0), 0.0);
        assert!((rate_integral(1.0, &a, 1.0) - avg).abs() < 1e-12);
    }

    #[test]
    fn no_automation_integral_is_linear() {
        assert_eq!(rate_integral(0.5, &[], 0.5), 0.25);
        assert_eq!(avg_rate(0.82, &[]), 0.82);
    }
}
