//! Аудио-узлы цепочки (Р.47): Freeverb (дословный переезд из lib.rs) и —
//! со следующего шага Ка1 — enum AudioNode со всеми эффектами.

use crate::{CH};
use kt_time::{AUDIO_SAMPLE_RATE, Tick};
use super::primitives::{Allpass, Comb, COMB_TUNINGS, ALLPASS_TUNINGS, STEREO_SPREAD, SmoothedParam, tail_frames_for};

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
            // predelay тоже держит материал (до 200 мс) — без зачистки он
            // протекал в хвост после seek (t319, Р.47 п.324)
            self.predelay[ch].fill(0.0);
        }
        self.pre_pos = 0;
        self.wet.reset_to_target();
    }
}

pub(crate) fn reverb_tail_for_delay(delay_ms: f32, decay: f32) -> Tick {
    let max_comb = COMB_TUNINGS[COMB_TUNINGS.len() - 1] as f32
        * (AUDIO_SAMPLE_RATE as f32 / 44_100.0)
        * (delay_ms / 100.0)
        + ((CH - 1) * STEREO_SPREAD) as f32;
    tail_frames_for(max_comb, decay)
}

// ---------------------------------------------------------------------------
// Узловая цепочка (Р.47, п.325): обработка идёт В ПОРЯДКЕ СЛОТОВ
// ---------------------------------------------------------------------------

use kt_model::{AutoProfiles, FxInstance};

/// Правила узла:
/// - `process(buf, pos)` — pos = клип-локальный кадр начала буфера
///   (детерминизм LFO/сеток: значение — функция позиции, не счётчика);
/// - `apply(fx, structural)` ставит БАЗЫ параметров (и пересоздаёт буферы
///   только при structural или реальной смене геометрии);
/// - `set_targets(auto, slot, s)` зовётся раз в чанк ТОЛЬКО при непустом
///   `auto` (гейт produce, байты старых цепочек) — узел ставит цели
///   сглаживания по своим кривым;
/// - узел обязан переживать `seam` (состояние живёт) и дозванивать на
///   нулевом входе в хвосте (`tail()` > 0 ⇒ хвост честный).
pub enum AudioNode {
    Gain(GainNode),
    /// Маркер силы слоя: снапшот сухой стороны и бленд исполняет
    /// `produce()` — узлу нужен межузловой буфер, которым он не владеет.
    LayerMix,
    Reverb(ReverbNode),
}

impl AudioNode {
    /// Построить узел под слот. Не-аудио виды узла не получают (цепочка
    /// их не видит — они прозрачны).
    pub(crate) fn build(fx: &FxInstance) -> Option<AudioNode> {
        match fx {
            FxInstance::Gain { .. } => Some(AudioNode::Gain(GainNode::new())),
            FxInstance::LayerMix { .. } => Some(AudioNode::LayerMix),
            FxInstance::SlowedReverb { .. } => Some(AudioNode::Reverb(ReverbNode::new())),
            _ => None,
        }
    }

    pub(crate) fn apply(&mut self, fx: &FxInstance, structural: bool) {
        match (self, fx) {
            (AudioNode::Gain(n), FxInstance::Gain { db, pan }) => n.apply(*db, *pan),
            (AudioNode::LayerMix, FxInstance::LayerMix { .. }) => {}
            (AudioNode::Reverb(n), FxInstance::SlowedReverb { .. }) => n.apply(fx, structural),
            // отпечаток цепочки совпал — виды по слотам совпадают по
            // построению; сюда можно попасть только ошибкой сопоставления
            _ => debug_assert!(false, "node/slot kind mismatch"),
        }
    }

    pub(crate) fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        match self {
            AudioNode::Gain(n) => n.set_targets(auto, slot, s),
            AudioNode::LayerMix => {}
            AudioNode::Reverb(n) => n.set_targets(auto, slot, s),
        }
    }

    pub(crate) fn process(&mut self, buf: &mut [f32], _pos: i64) {
        match self {
            AudioNode::Gain(n) => n.process(buf),
            AudioNode::LayerMix => {}
            AudioNode::Reverb(n) => n.rv.process(buf),
        }
    }

    pub(crate) fn tail(&self) -> kt_time::Tick {
        match self {
            AudioNode::Reverb(n) => n.rv.tail(),
            _ => kt_time::Tick::ZERO,
        }
    }

    pub(crate) fn reset(&mut self) {
        match self {
            AudioNode::Gain(n) => n.reset(),
            AudioNode::LayerMix => {}
            AudioNode::Reverb(n) => n.rv.reset(),
        }
    }
}

/// Громкость/баланс слота. Константный путь — дословные множители старого
/// кода (на них стоит GOLDEN_AUDIO); с кривыми — SmoothedParam с целями на
/// середину чанка, tick на каждый кадр (тот же порядок циклов: сперва
/// громкость по кадрам, затем баланс по кадрам).
pub struct GainNode {
    static_gain: f32,
    static_pan_l: f32,
    static_pan_r: f32,
    gain_auto: bool,
    pan_auto: bool,
    base_db: f64,
    base_pan: f64,
    auto_gain: SmoothedParam,
    auto_pan_l: SmoothedParam,
    auto_pan_r: SmoothedParam,
}

impl GainNode {
    fn new() -> GainNode {
        GainNode {
            static_gain: 1.0,
            static_pan_l: 1.0,
            static_pan_r: 1.0,
            gain_auto: false,
            pan_auto: false,
            base_db: 0.0,
            base_pan: 0.0,
            auto_gain: SmoothedParam::new(1.0),
            auto_pan_l: SmoothedParam::new(1.0),
            auto_pan_r: SmoothedParam::new(1.0),
        }
    }

    fn apply(&mut self, db: f32, pan: f32) {
        self.base_db = db as f64;
        self.base_pan = pan as f64;
        self.static_gain = 10f32.powf(db / 20.0);
        // баланс: линейный закон, на нуле оба множителя ровно 1.0 —
        // прежний путь байт-в-байт
        let p = pan.clamp(-1.0, 1.0);
        self.static_pan_l = (1.0 - p).min(1.0);
        self.static_pan_r = (1.0 + p).min(1.0);
    }

    /// Есть ли кривые у слота — по факту профилей (как раньше собирались
    /// gain_slots/pan_slots по непустым точкам).
    pub(crate) fn bind_curves(&mut self, auto: &AutoProfiles, slot: usize) {
        self.gain_auto = !auto.points(slot, "db").is_empty();
        self.pan_auto = !auto.points(slot, "pan").is_empty();
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        if self.gain_auto {
            let db = auto.value(slot, "db", self.base_db, s);
            self.auto_gain.set_target(10f32.powf(db as f32 / 20.0));
        }
        if self.pan_auto {
            let p = auto.value(slot, "pan", self.base_pan, s).clamp(-1.0, 1.0) as f32;
            self.auto_pan_l.set_target((1.0 - p).min(1.0));
            self.auto_pan_r.set_target((1.0 + p).min(1.0));
        }
    }

    fn process(&mut self, buf: &mut [f32]) {
        if self.gain_auto {
            for f in buf.chunks_exact_mut(CH) {
                let g = self.auto_gain.tick();
                for s in f.iter_mut() {
                    *s *= g;
                }
            }
        } else if self.static_gain != 1.0 {
            for s in buf.iter_mut() {
                *s *= self.static_gain;
            }
        }
        if self.pan_auto {
            for f in buf.chunks_exact_mut(CH) {
                let (l, r) = (self.auto_pan_l.tick(), self.auto_pan_r.tick());
                f[0] *= l;
                if CH > 1 {
                    f[1] *= r;
                }
            }
        } else if self.static_pan_l != 1.0 || self.static_pan_r != 1.0 {
            for f in buf.chunks_exact_mut(CH) {
                f[0] *= self.static_pan_l;
                if CH > 1 {
                    f[1] *= self.static_pan_r;
                }
            }
        }
    }

    fn reset(&mut self) {
        self.auto_gain.reset_to_target();
        self.auto_pan_l.reset_to_target();
        self.auto_pan_r.reset_to_target();
    }
}

/// Реверб-половина слота Slowed+Reverb (варп-половина живёт отдельным
/// полем процессора — у варпа монополия на потребление источника).
pub struct ReverbNode {
    pub(crate) rv: Freeverb,
    base_mix: f64,
}

impl ReverbNode {
    fn new() -> ReverbNode {
        ReverbNode { rv: Freeverb::new(0.0, 100.0, 0.2, 0.7, 1.0, 0.0, 1.0), base_mix: 0.0 }
    }

    fn apply(&mut self, fx: &FxInstance, structural: bool) {
        let FxInstance::SlowedReverb {
            mix, reverb_delay_ms, damp, decay, wet_gain, predelay_ms, width, ..
        } = fx
        else {
            return;
        };
        self.base_mix = *mix as f64;
        if structural {
            self.rv = Freeverb::new(
                *mix,
                *reverb_delay_ms,
                *damp,
                *decay,
                *wet_gain,
                *predelay_ms,
                *width,
            );
        } else {
            self.rv.set_params(
                *mix,
                *reverb_delay_ms,
                *damp,
                *decay,
                *wet_gain,
                *predelay_ms,
                *width,
            );
        }
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        // mix сглаживается one-pole внутри реверба (тот же вызов, что до
        // Р.47 — байты одно-слотовой цепочки не двигаются)
        self.rv.set_mix(auto.value(slot, "mix", self.base_mix, s) as f32);
    }
}
