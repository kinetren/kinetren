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
const FEEDBACK: f32 = 0.84;
const DAMP: f32 = 0.2;

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
    fn process(&mut self, input: f32) -> f32 {
        let out = self.buf[self.pos];
        self.filter_state = out * (1.0 - DAMP) + self.filter_state * DAMP;
        self.buf[self.pos] = input + self.filter_state * FEEDBACK;
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
}

impl Freeverb {
    pub fn new(mix: f32, delay_ms: f32) -> Freeverb {
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
        Freeverb { combs, allpasses, wet: SmoothedParam::new(mix.clamp(0.0, 1.0)), delay_ms }
    }

    pub fn set_params(&mut self, mix: f32, delay_ms: f32) {
        self.wet.set_target(mix.clamp(0.0, 1.0));
        // структурная смена геометрии: пересоздание допускается только при
        // реально другом delay (порог 0.5 мс, как в старом коде, но wet/dry
        // это не трогает)
        if (delay_ms - self.delay_ms).abs() > 0.5 {
            let keep_wet = self.wet;
            *self = Freeverb::new(keep_wet.target, delay_ms);
            self.wet = keep_wet;
        }
    }

    /// Хвост: время затухания до −60 dB (t60) по максимальной comb-линии.
    pub fn tail(&self) -> Tick {
        let max_len = self.combs[CH - 1].iter().map(|c| c.buf.len()).max().unwrap_or(1);
        // энергия падает на 20·log10(FEEDBACK) dB за проход линии
        let db_per_pass = -20.0 * FEEDBACK.log10(); // ≈ 1.51 dB
        let passes = 60.0 / db_per_pass;
        let frames = (max_len as f32 * passes) as i64;
        Tick::from_audio_frames(frames)
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        for frame in samples.chunks_exact_mut(CH) {
            let wet_gain = self.wet.tick();
            for (ch, s) in frame.iter_mut().enumerate() {
                let dry = *s;
                let input = dry * 0.015;
                let mut wet = 0.0;
                for comb in &mut self.combs[ch] {
                    wet += comb.process(input);
                }
                for ap in &mut self.allpasses[ch] {
                    wet = ap.process(wet);
                }
                *s = dry * (1.0 - wet_gain) + wet * wet_gain;
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
}

// ---------------------------------------------------------------------------
// Процессор клипа: цепочка FX поверх (возможно варпнутого) источника
// ---------------------------------------------------------------------------

/// Скорость воспроизведения цепочки (1.0 = без варпа).
pub fn chain_rate(fx: &[FxInstance]) -> f64 {
    fx.iter()
        .find_map(|f| match *f {
            FxInstance::SlowedReverb { rate, .. } => Some(rate.clamp(0.25, 32.0)),
            _ => None,
        })
        .unwrap_or(1.0)
}

/// Длина хвоста цепочки БЕЗ создания state (для расширения окна клипа
/// в графе до того, как процессор построен).
pub fn chain_tail(fx: &[FxInstance]) -> Tick {
    fx.iter()
        .map(|f| match *f {
            FxInstance::SlowedReverb { mix, reverb_delay_ms, .. } if mix > 0.0 => {
                reverb_tail_for_delay(reverb_delay_ms)
            }
            _ => Tick::ZERO,
        })
        .max()
        .unwrap_or(Tick::ZERO)
}

fn reverb_tail_for_delay(delay_ms: f32) -> Tick {
    let max_comb = COMB_TUNINGS[COMB_TUNINGS.len() - 1] as f32
        * (AUDIO_SAMPLE_RATE as f32 / 44_100.0)
        * (delay_ms / 100.0)
        + ((CH - 1) * STEREO_SPREAD) as f32;
    let db_per_pass = -20.0 * FEEDBACK.log10();
    let frames = (max_comb.max(8.0) * (60.0 / db_per_pass)) as i64;
    Tick::from_audio_frames(frames)
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
    /// Курсор источника для пути без варпа (rate == 1).
    plain_cursor: i64,
}

impl ClipProcessor {
    pub fn new(fx: &[FxInstance], src_in_frames: i64) -> ClipProcessor {
        let mut p = ClipProcessor {
            fingerprint: chain_fingerprint(fx),
            next_local: i64::MIN,
            src_in: src_in_frames,
            warp: None,
            reverb: None,
            static_gain: 1.0,
            rate: 1.0,
            plain_cursor: src_in_frames,
        };
        p.apply_params(fx, true);
        p
    }

    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Донести параметры. `structural` = первый раз (создание объектов).
    pub fn apply_params(&mut self, fx: &[FxInstance], structural: bool) {
        let mut gain = 1.0f32;
        for f in fx {
            match *f {
                FxInstance::Gain { db } => gain *= 10f32.powf(db / 20.0),
                FxInstance::SlowedReverb { rate, mix, reverb_delay_ms } => {
                    self.rate = rate.clamp(0.25, 32.0);
                    if structural {
                        self.reverb = Some(Freeverb::new(mix, reverb_delay_ms));
                    } else if let Some(rv) = &mut self.reverb {
                        rv.set_params(mix, reverb_delay_ms);
                    }
                    if let Some(w) = &mut self.warp {
                        w.set_rate(rate);
                    }
                }
            }
        }
        self.static_gain = gain;
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
        self.produce(out, pull);
        self.next_local = local + (out.len() / CH) as i64;
        self.warp.as_ref().map(|w| w.underruns()).unwrap_or(0) - before
    }

    fn hard_seek(&mut self, local: i64, pull: &mut dyn FnMut(i64, &mut [f32]) -> bool) {
        if let Some(r) = &mut self.reverb {
            r.reset();
        }
        let preroll = self.tail_frames().max(MIN_PREROLL_FRAMES).min(local.max(0));
        let start = local - preroll;
        // позиция источника для local=start: дробный курсор начинается здесь
        let src_start = self.src_in + (start as f64 * self.rate) as i64;
        if (self.rate - 1.0).abs() > 1e-9 {
            let mut w = Warp::new(self.rate, src_start);
            w.set_rate(self.rate);
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
            self.produce(&mut scratch[..n * CH], pull);
            done += n as i64;
        }
    }

    fn produce(&mut self, out: &mut [f32], pull: &mut dyn FnMut(i64, &mut [f32]) -> bool) {
        match &mut self.warp {
            Some(w) => w.produce(out, pull),
            None => {
                let frames = out.len() / CH;
                if !pull(self.plain_cursor, out) {
                    out.fill(0.0);
                }
                self.plain_cursor += frames as i64;
            }
        }
        if let Some(r) = &mut self.reverb {
            r.process(out);
        }
        if self.static_gain != 1.0 {
            for s in out.iter_mut() {
                *s *= self.static_gain;
            }
        }
    }
}
