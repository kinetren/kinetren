//! Аудио-узлы цепочки (Р.47): Freeverb (дословный переезд из lib.rs) и —
//! со следующего шага Ка1 — enum AudioNode со всеми эффектами.

use super::primitives::{
    ALLPASS_TUNINGS, Allpass, COMB_TUNINGS, Comb, STEREO_SPREAD, SmoothedParam, tail_frames_for,
};
use crate::CH;
use kt_time::{AUDIO_SAMPLE_RATE, Tick};

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
        if (delay_ms - self.delay_ms).abs() > 0.5 || (predelay_ms - self.predelay_ms).abs() > 0.5 {
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
    Filter(Box<FilterNode>),
    Eq(Box<EqNode>),
    Distortion(DistNode),
    Bitcrusher(CrushNode),
    StereoWidth(WidthNode),
    Tremolo(TremoloNode),
    AutoPan(AutoPanNode),
    Vibrato(Box<VibratoNode>),
    Chorus(Box<ChorusNode>),
    Flanger(Box<FlangerNode>),
    Phaser(Box<PhaserNode>),
    Delay(Box<DelayNode>),
    Compressor(CompNode),
    Gate(GateNode),
    Limiter(Box<LimiterNode>),
    Pitch(Box<PitchNode>),
}

impl AudioNode {
    /// Построить узел под слот. Не-аудио виды узла не получают (цепочка
    /// их не видит — они прозрачны).
    pub(crate) fn build(fx: &FxInstance) -> Option<AudioNode> {
        match fx {
            FxInstance::Gain { .. } => Some(AudioNode::Gain(GainNode::new())),
            FxInstance::LayerMix { .. } => Some(AudioNode::LayerMix),
            FxInstance::SlowedReverb { .. } => Some(AudioNode::Reverb(ReverbNode::new())),
            FxInstance::Filter { .. } => Some(AudioNode::Filter(Box::new(FilterNode::new()))),
            FxInstance::Eq { .. } => Some(AudioNode::Eq(Box::new(EqNode::new()))),
            FxInstance::Distortion { .. } => Some(AudioNode::Distortion(DistNode::new())),
            FxInstance::Bitcrusher { .. } => Some(AudioNode::Bitcrusher(CrushNode::new())),
            FxInstance::StereoWidth { .. } => Some(AudioNode::StereoWidth(WidthNode::new())),
            FxInstance::Tremolo { .. } => Some(AudioNode::Tremolo(TremoloNode::new())),
            FxInstance::AutoPan { .. } => Some(AudioNode::AutoPan(AutoPanNode::new())),
            FxInstance::Vibrato { .. } => Some(AudioNode::Vibrato(Box::new(VibratoNode::new()))),
            FxInstance::Chorus { .. } => Some(AudioNode::Chorus(Box::new(ChorusNode::new()))),
            FxInstance::Flanger { .. } => Some(AudioNode::Flanger(Box::new(FlangerNode::new()))),
            FxInstance::Phaser { .. } => Some(AudioNode::Phaser(Box::new(PhaserNode::new()))),
            FxInstance::Delay { .. } => Some(AudioNode::Delay(Box::new(DelayNode::new()))),
            FxInstance::Compressor { .. } => Some(AudioNode::Compressor(CompNode::new())),
            FxInstance::Gate { .. } => Some(AudioNode::Gate(GateNode::new())),
            FxInstance::Limiter { .. } => Some(AudioNode::Limiter(Box::new(LimiterNode::new()))),
            FxInstance::PitchShift { .. } => Some(AudioNode::Pitch(Box::new(PitchNode::new()))),
            _ => None,
        }
    }

    pub(crate) fn apply(&mut self, fx: &FxInstance, structural: bool) {
        match (self, fx) {
            (AudioNode::Gain(n), FxInstance::Gain { db, pan }) => n.apply(*db, *pan),
            (AudioNode::LayerMix, FxInstance::LayerMix { .. }) => {}
            (AudioNode::Reverb(n), FxInstance::SlowedReverb { .. }) => n.apply(fx, structural),
            (AudioNode::Filter(n), FxInstance::Filter { mode, cutoff_hz, resonance }) => {
                n.apply(*mode, *cutoff_hz, *resonance)
            }
            (
                AudioNode::Eq(n),
                FxInstance::Eq {
                    low_gain_db,
                    low_freq_hz,
                    mid_gain_db,
                    mid_freq_hz,
                    mid_q,
                    high_gain_db,
                    high_freq_hz,
                },
            ) => n.apply([
                *low_gain_db,
                *low_freq_hz,
                *mid_gain_db,
                *mid_freq_hz,
                *mid_q,
                *high_gain_db,
                *high_freq_hz,
            ]),
            (AudioNode::Distortion(n), FxInstance::Distortion { drive_db, tone, mix }) => {
                n.apply(*drive_db, *tone, *mix)
            }
            (AudioNode::Bitcrusher(n), FxInstance::Bitcrusher { bits, rate_hz, mix }) => {
                n.apply(*bits, *rate_hz, *mix)
            }
            (AudioNode::StereoWidth(n), FxInstance::StereoWidth { width }) => n.apply(*width),
            (AudioNode::Tremolo(n), FxInstance::Tremolo { rate_hz, depth }) => {
                n.apply(*rate_hz, *depth)
            }
            (AudioNode::AutoPan(n), FxInstance::AutoPan { rate_hz, depth }) => {
                n.apply(*rate_hz, *depth)
            }
            (AudioNode::Vibrato(n), FxInstance::Vibrato { rate_hz, depth_ms }) => {
                n.apply(*rate_hz, *depth_ms)
            }
            (AudioNode::Chorus(n), FxInstance::Chorus { rate_hz, depth_ms, mix, voices }) => {
                n.apply(*rate_hz, *depth_ms, *mix, *voices)
            }
            (AudioNode::Flanger(n), FxInstance::Flanger { rate_hz, depth_ms, feedback, mix }) => {
                n.apply(*rate_hz, *depth_ms, *feedback, *mix)
            }
            (
                AudioNode::Phaser(n),
                FxInstance::Phaser { rate_hz, depth, feedback, stages, mix },
            ) => n.apply(*rate_hz, *depth, *feedback, *stages, *mix),
            (AudioNode::Delay(n), FxInstance::Delay { time_ms, feedback, mix, pingpong }) => {
                n.apply(*time_ms, *feedback, *mix, *pingpong)
            }
            (
                AudioNode::Compressor(n),
                FxInstance::Compressor { threshold_db, ratio, attack_ms, release_ms, makeup_db, .. },
            ) => n.apply(*threshold_db, *ratio, *attack_ms, *release_ms, *makeup_db),
            (
                AudioNode::Gate(n),
                FxInstance::Gate { threshold_db, attack_ms, hold_ms, release_ms },
            ) => n.apply(*threshold_db, *attack_ms, *hold_ms, *release_ms),
            (
                AudioNode::Limiter(n),
                FxInstance::Limiter { threshold_db, ceiling_db, lookahead_ms, release_ms },
            ) => n.apply(*threshold_db, *ceiling_db, *lookahead_ms, *release_ms, structural),
            // движок питча — настройка ПРОЕКТА: он приезжает не из слота,
            // а из ChainOpts (см. ClipProcessor::new_with_opts)
            (AudioNode::Pitch(n), FxInstance::PitchShift { semitones, cents }) => {
                let engine = n.engine;
                n.apply(*semitones, *cents, engine);
            }
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
            AudioNode::Filter(n) => n.set_targets(auto, slot, s),
            AudioNode::Eq(n) => n.set_targets(auto, slot, s),
            AudioNode::Distortion(n) => n.set_targets(auto, slot, s),
            AudioNode::Bitcrusher(n) => n.set_targets(auto, slot, s),
            AudioNode::StereoWidth(n) => n.set_targets(auto, slot, s),
            AudioNode::Tremolo(n) => n.set_targets(auto, slot, s),
            AudioNode::AutoPan(n) => n.set_targets(auto, slot, s),
            AudioNode::Limiter(n) => n.set_targets(auto, slot, s),
            // питч читает кривые ПО ПОЗИЦИИ ЗЕРНА (bind), цели ему не нужны
            AudioNode::Pitch(_) => {}
            AudioNode::Vibrato(n) => n.set_targets(auto, slot, s),
            AudioNode::Chorus(n) => n.set_targets(auto, slot, s),
            AudioNode::Flanger(n) => n.set_targets(auto, slot, s),
            AudioNode::Phaser(n) => n.set_targets(auto, slot, s),
            AudioNode::Delay(n) => n.set_targets(auto, slot, s),
            AudioNode::Compressor(n) => n.set_targets(auto, slot, s),
            AudioNode::Gate(n) => n.set_targets(auto, slot, s),
        }
    }

    /// Привязать LFO-узлы к их кривым частоты и длине клипа (копия точек —
    /// в process модель не читается). Зовётся из apply_params/set_auto/
    /// set_clip_len процессора.
    pub(crate) fn bind_lfo(&mut self, auto: &AutoProfiles, slot: usize, clip_len: i64) {
        match self {
            AudioNode::Tremolo(n) => n.lfo.bind(auto, slot, clip_len),
            AudioNode::AutoPan(n) => n.lfo.bind(auto, slot, clip_len),
            AudioNode::Vibrato(n) => n.lfo.bind(auto, slot, clip_len),
            AudioNode::Chorus(n) => n.lfo.bind(auto, slot, clip_len),
            AudioNode::Flanger(n) => n.lfo.bind(auto, slot, clip_len),
            AudioNode::Phaser(n) => n.lfo.bind(auto, slot, clip_len),
            AudioNode::Pitch(n) => n.bind(auto, slot, clip_len),
            _ => {}
        }
    }

    /// Обработка чанка. `key` — КЛЮЧЕВОЙ (сайдчейн) сигнал того же окна и
    /// того же домена (f32 48 kHz/2ch), если вызывающий его дал; узел, у
    /// которого второго входа нет, обязан игнорировать его дословно —
    /// иначе ветка ключа стоила бы байтов там, где ключа не бывает.
    ///
    /// Своего состояния у ключа НЕТ намеренно: сеттер, живущий между
    /// вызовами, завёл бы скрытую зависимость от порядка вызовов, а
    /// инвариант 2 требует, чтобы состояние трогал только render-поток и
    /// только на своём окне.
    pub(crate) fn process(&mut self, buf: &mut [f32], _pos: i64, key: Option<&[f32]>) {
        match self {
            AudioNode::Gain(n) => n.process(buf),
            AudioNode::LayerMix => {}
            AudioNode::Reverb(n) => n.rv.process(buf),
            AudioNode::Filter(n) => n.process(buf),
            AudioNode::Eq(n) => n.process(buf),
            AudioNode::Distortion(n) => n.process(buf),
            AudioNode::Bitcrusher(n) => n.process(buf, _pos),
            AudioNode::StereoWidth(n) => n.process(buf),
            AudioNode::Tremolo(n) => n.process(buf, _pos),
            AudioNode::AutoPan(n) => n.process(buf, _pos),
            AudioNode::Vibrato(n) => n.process(buf, _pos),
            AudioNode::Chorus(n) => n.process(buf, _pos),
            AudioNode::Flanger(n) => n.process(buf, _pos),
            AudioNode::Phaser(n) => n.process(buf, _pos),
            AudioNode::Delay(n) => n.process(buf),
            AudioNode::Compressor(n) => n.process(buf, key),
            AudioNode::Gate(n) => n.process(buf),
            AudioNode::Limiter(n) => n.process(buf),
            AudioNode::Pitch(n) => n.process(buf, _pos),
        }
    }

    /// Упреждение узла в кадрах (Р.48, п.337): сколько СОДЕРЖИМОГО
    /// впереди выходной позиции узлу нужно, чтобы отдать выход БЕЗ
    /// задержки. Узел с ненулевым упреждением обязан сам выравнивать
    /// выход своим кольцом и жить в ОБЕИХ формулах — здесь и в
    /// статической `chain_lookahead` (их согласие сторожит t322ж).
    ///
    /// Упреждение — НЕ хвост: оно входит в прогрев seek и в окно
    /// источника, но НЕ продлевает `ext_end` (иначе клип звучал бы
    /// после собственного конца).
    /// Донести движок питча (настройка ПРОЕКТА, а не слота): узел
    /// получает его при создании процессора; смена движка — пересборка
    /// процессора целиком (сторожит сверка opts в FxStateStore).
    pub(crate) fn set_engine(&mut self, engine: kt_model::PitchEngine) {
        if let AudioNode::Pitch(n) = self {
            n.engine = engine;
            n.pv = None;
            n.refresh_la();
            if engine == kt_model::PitchEngine::Vocoder && n.la > 0 {
                n.pv = Some(Box::new(PvState::new()));
            }
        }
    }

    pub(crate) fn lookahead(&self) -> i64 {
        match self {
            AudioNode::Limiter(n) => n.la,
            AudioNode::Pitch(n) => n.la,
            _ => 0,
        }
    }

    pub(crate) fn tail(&self) -> kt_time::Tick {
        match self {
            AudioNode::Reverb(n) => n.rv.tail(),
            AudioNode::Filter(n) => n.tail(),
            AudioNode::Vibrato(n) => n.tail(),
            AudioNode::Chorus(n) => n.tail(),
            AudioNode::Flanger(n) => n.tail(),
            AudioNode::Delay(n) => n.tail(true),
            _ => kt_time::Tick::ZERO,
        }
    }

    pub(crate) fn reset(&mut self) {
        match self {
            AudioNode::Gain(n) => n.reset(),
            AudioNode::LayerMix => {}
            AudioNode::Reverb(n) => n.rv.reset(),
            AudioNode::Filter(n) => n.reset(),
            AudioNode::Eq(n) => n.reset(),
            AudioNode::Distortion(n) => n.reset(),
            AudioNode::Bitcrusher(n) => n.reset(),
            AudioNode::StereoWidth(n) => n.reset(),
            AudioNode::Tremolo(n) => n.reset(),
            AudioNode::AutoPan(n) => n.reset(),
            AudioNode::Vibrato(n) => n.reset(),
            AudioNode::Chorus(n) => n.reset(),
            AudioNode::Flanger(n) => n.reset(),
            AudioNode::Phaser(n) => n.reset(),
            AudioNode::Delay(n) => n.reset(),
            AudioNode::Compressor(n) => n.reset(),
            AudioNode::Gate(n) => n.reset(),
            AudioNode::Limiter(n) => n.reset(),
            AudioNode::Pitch(n) => n.reset(),
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
            mix,
            reverb_delay_ms,
            damp,
            decay,
            wet_gain,
            predelay_ms,
            width,
            ..
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

// ---------------------------------------------------------------------------
// Узлы Ка2 (Р.47, п.327): фильтры, EQ, дисторшн, биткрашер, ширина
// ---------------------------------------------------------------------------

use super::primitives::{Biquad, BiquadKind, apply_width, softclip};
use kt_model::FilterMode;

/// Пересчёт коэффициентов — суб-блоками: чанк 1024 (21 мс) на свипах
/// частоты слышен как zipper, 128 кадров — нет.
const TUNE_STEP: usize = 128;

/// Резонансный срез LP/HP/BP: RBJ-биквад на канал.
pub struct FilterNode {
    mode: FilterMode,
    cutoff: SmoothedParam,
    q: SmoothedParam,
    bq: [Biquad; CH],
}

impl FilterNode {
    fn new() -> FilterNode {
        FilterNode {
            mode: FilterMode::Lowpass,
            cutoff: SmoothedParam::with_ms(800.0, 20.0),
            q: SmoothedParam::with_ms(0.707, 20.0),
            bq: Default::default(),
        }
    }

    fn kind(&self) -> BiquadKind {
        match self.mode {
            FilterMode::Lowpass => BiquadKind::Lowpass,
            FilterMode::Highpass => BiquadKind::Highpass,
            FilterMode::Bandpass => BiquadKind::Bandpass,
        }
    }

    fn apply(&mut self, mode: FilterMode, cutoff_hz: f32, resonance: f32) {
        // смена вида НЕ пересоздаёт состояние (2 f32 на канал): транзиент
        // от смены формы допустим и короче любого клика пересоздания
        self.mode = mode;
        self.cutoff.set_target(cutoff_hz.clamp(20.0, 20_000.0));
        self.q.set_target(resonance.clamp(0.5, 10.0));
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        let f = auto.value(slot, "cutoff_hz", self.cutoff.target as f64, s) as f32;
        let q = auto.value(slot, "resonance", self.q.target as f64, s) as f32;
        self.cutoff.set_target(f.clamp(20.0, 20_000.0));
        self.q.set_target(q.clamp(0.5, 10.0));
    }

    fn process(&mut self, buf: &mut [f32]) {
        let kind = self.kind();
        for block in buf.chunks_mut(TUNE_STEP * CH) {
            let mut f = 0.0;
            let mut q = 0.0;
            for _ in 0..block.len() / CH {
                f = self.cutoff.tick();
                q = self.q.tick();
            }
            for b in &mut self.bq {
                b.tune(&kind, f, q, 0.0);
            }
            for fr in block.chunks_exact_mut(CH) {
                for (ch, s) in fr.iter_mut().enumerate() {
                    *s = self.bq[ch].process(*s);
                }
            }
        }
    }

    fn tail(&self) -> Tick {
        // звон резонанса: t60 = 6.9·Q/(π·f0)
        let f0 = self.cutoff.value().max(20.0);
        let q = self.q.value().max(0.5);
        let secs = 6.9 * q / (std::f32::consts::PI * f0);
        Tick::from_audio_frames((secs * AUDIO_SAMPLE_RATE as f32) as i64)
    }

    fn reset(&mut self) {
        for b in &mut self.bq {
            b.reset();
        }
        self.cutoff.reset_to_target();
        self.q.reset_to_target();
    }
}

/// Трёхполосный EQ: полка + пик + полка, каждый — биквад на канал.
/// Добротность полок EQ и пол добротности середины — ОДНИ числа для
/// звука и для графика АЧХ (Р.48, п.347).
pub(crate) const EQ_SHELF_Q: f32 = 0.707;
pub(crate) const EQ_MID_Q_FLOOR: f32 = 0.3;

/// АЧХ всего эквалайзера на частоте `at_hz`, дБ. Значения — в порядке
/// `EqNode::NAMES`: low_gain, low_freq, mid_gain, mid_freq, mid_q,
/// high_gain, high_freq. Каскад = сумма дБ; фильтры настраиваются тем же
/// `tune`, что звучит, поэтому кривая в UI равна тому, что слышно.
pub fn eq_mag_db(v: &[f32; 7], at_hz: f32) -> f64 {
    let mut b = Biquad::default();
    let mut db = 0.0;
    b.tune(&BiquadKind::LowShelf, v[1], EQ_SHELF_Q, v[0]);
    db += b.mag_db(at_hz);
    b.tune(&BiquadKind::Peak, v[3], v[4].max(EQ_MID_Q_FLOOR), v[2]);
    db += b.mag_db(at_hz);
    b.tune(&BiquadKind::HighShelf, v[6], EQ_SHELF_Q, v[5]);
    db += b.mag_db(at_hz);
    db
}

pub struct EqNode {
    params: SmoothedParam7,
    bands: [[Biquad; 3]; CH],
}

/// Семь сглаженных параметров EQ одним блоком (low_g, low_f, mid_g,
/// mid_f, mid_q, high_g, high_f).
struct SmoothedParam7([SmoothedParam; 7]);

impl EqNode {
    const DEFAULTS: [f32; 7] = [0.0, 200.0, 0.0, 1000.0, 1.0, 0.0, 4000.0];
    const NAMES: [&'static str; 7] = [
        "low_gain_db",
        "low_freq_hz",
        "mid_gain_db",
        "mid_freq_hz",
        "mid_q",
        "high_gain_db",
        "high_freq_hz",
    ];

    fn new() -> EqNode {
        EqNode {
            params: SmoothedParam7(std::array::from_fn(|i| {
                SmoothedParam::with_ms(Self::DEFAULTS[i], 20.0)
            })),
            bands: Default::default(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply(&mut self, vals: [f32; 7]) {
        for (p, v) in self.params.0.iter_mut().zip(vals) {
            p.set_target(v);
        }
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        for (i, p) in self.params.0.iter_mut().enumerate() {
            let v = auto.value(slot, Self::NAMES[i], p.target as f64, s) as f32;
            p.set_target(v);
        }
    }

    fn process(&mut self, buf: &mut [f32]) {
        for block in buf.chunks_mut(TUNE_STEP * CH) {
            let mut v = [0f32; 7];
            for _ in 0..block.len() / CH {
                for (dst, p) in v.iter_mut().zip(self.params.0.iter_mut()) {
                    *dst = p.tick();
                }
            }
            // все гейны в нуле — plain-пропуск: EQ прозрачен байт-в-байт
            if v[0] == 0.0 && v[2] == 0.0 && v[5] == 0.0 {
                continue;
            }
            for ch in 0..CH {
                // те же числа, что у графика АЧХ (Р.48): одни константы
                self.bands[ch][0].tune(&BiquadKind::LowShelf, v[1], EQ_SHELF_Q, v[0]);
                self.bands[ch][1].tune(&BiquadKind::Peak, v[3], v[4].max(EQ_MID_Q_FLOOR), v[2]);
                self.bands[ch][2].tune(&BiquadKind::HighShelf, v[6], EQ_SHELF_Q, v[5]);
            }
            for fr in block.chunks_exact_mut(CH) {
                for (ch, s) in fr.iter_mut().enumerate() {
                    let mut x = *s;
                    for b in &mut self.bands[ch] {
                        x = b.process(x);
                    }
                    *s = x;
                }
            }
        }
    }

    fn reset(&mut self) {
        for chb in &mut self.bands {
            for b in chb {
                b.reset();
            }
        }
        for p in &mut self.params.0 {
            p.reset_to_target();
        }
    }
}

/// Мягкое насыщение с компенсацией уровня и тоном.
pub struct DistNode {
    drive_db: SmoothedParam,
    tone: SmoothedParam,
    mix: SmoothedParam,
    lp: [f32; CH],
}

impl DistNode {
    fn new() -> DistNode {
        DistNode {
            drive_db: SmoothedParam::with_ms(12.0, 20.0),
            tone: SmoothedParam::with_ms(0.5, 20.0),
            mix: SmoothedParam::with_ms(1.0, 20.0),
            lp: [0.0; CH],
        }
    }

    fn apply(&mut self, drive_db: f32, tone: f32, mix: f32) {
        self.drive_db.set_target(drive_db);
        self.tone.set_target(tone.clamp(0.0, 1.0));
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        self.drive_db
            .set_target(auto.value(slot, "drive_db", self.drive_db.target as f64, s) as f32);
        self.tone.set_target(auto.value(slot, "tone", self.tone.target as f64, s) as f32);
        self.mix.set_target(auto.value(slot, "mix", self.mix.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32]) {
        for fr in buf.chunks_exact_mut(CH) {
            let g = 10f32.powf(self.drive_db.tick() / 20.0);
            let comp = 1.0 / g.sqrt();
            let tone = self.tone.tick().clamp(0.0, 1.0);
            // тон: one-pole LP 500..12000 Гц (exp-map)
            let f = 500.0 * (12_000.0f32 / 500.0).powf(tone);
            let alpha = 1.0 - (-2.0 * std::f32::consts::PI * f / AUDIO_SAMPLE_RATE as f32).exp();
            let k = self.mix.tick().clamp(0.0, 1.0);
            for (ch, s) in fr.iter_mut().enumerate() {
                let dry = *s;
                let mut y = softclip(dry * g) * comp;
                self.lp[ch] += (y - self.lp[ch]) * alpha;
                y = self.lp[ch];
                *s = dry * (1.0 - k) + y * k;
            }
        }
    }

    fn reset(&mut self) {
        self.lp = [0.0; CH];
        self.drive_db.reset_to_target();
        self.tone.reset_to_target();
        self.mix.reset_to_target();
    }
}

/// Lo-fi: квант битовой глубины + удержание сэмпла по СЕТКЕ ПОЗИЦИИ
/// (floor(pos·rate/sr) — детерминизм: два рендера одного клипа дают одни
/// артефакты, прецедент seed шума видео).
pub struct CrushNode {
    bits: SmoothedParam,
    rate_hz: SmoothedParam,
    mix: SmoothedParam,
    held: [f32; CH],
    held_cell: i64,
}

impl CrushNode {
    fn new() -> CrushNode {
        CrushNode {
            bits: SmoothedParam::with_ms(8.0, 20.0),
            rate_hz: SmoothedParam::with_ms(12_000.0, 20.0),
            mix: SmoothedParam::with_ms(1.0, 20.0),
            held: [0.0; CH],
            held_cell: i64::MIN,
        }
    }

    fn apply(&mut self, bits: f32, rate_hz: f32, mix: f32) {
        self.bits.set_target(bits.clamp(1.0, 16.0));
        self.rate_hz.set_target(rate_hz.clamp(500.0, 48_000.0));
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        self.bits.set_target(auto.value(slot, "bits", self.bits.target as f64, s) as f32);
        self.rate_hz.set_target(auto.value(slot, "rate_hz", self.rate_hz.target as f64, s) as f32);
        self.mix.set_target(auto.value(slot, "mix", self.mix.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], pos: i64) {
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let bits = self.bits.tick().clamp(1.0, 16.0);
            let rate = self.rate_hz.tick().clamp(500.0, 48_000.0) as f64;
            let k = self.mix.tick().clamp(0.0, 1.0);
            let q = (2f32).powf(bits - 1.0);
            let p = pos + i as i64;
            // сетка удержания привязана к ПОЗИЦИИ клипа, не к счётчику
            let cell = ((p as f64) * rate / AUDIO_SAMPLE_RATE as f64).floor() as i64;
            if cell != self.held_cell {
                self.held_cell = cell;
                for (ch, s) in fr.iter().enumerate() {
                    self.held[ch] = (s * q).round() / q;
                }
            }
            for (ch, s) in fr.iter_mut().enumerate() {
                *s = *s * (1.0 - k) + self.held[ch] * k;
            }
        }
    }

    fn reset(&mut self) {
        self.held = [0.0; CH];
        self.held_cell = i64::MIN;
        self.bits.reset_to_target();
        self.rate_hz.reset_to_target();
        self.mix.reset_to_target();
    }
}

/// Ширина стерео m/s. width == 1.0 — ЯВНЫЙ пропуск (m/s-переупаковка не
/// байт-тождественна).
pub struct WidthNode {
    width: SmoothedParam,
}

impl WidthNode {
    fn new() -> WidthNode {
        WidthNode { width: SmoothedParam::with_ms(1.0, 20.0) }
    }

    fn apply(&mut self, width: f32) {
        self.width.set_target(width.clamp(0.0, 2.0));
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        self.width.set_target(auto.value(slot, "width", self.width.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32]) {
        if self.width.value() == 1.0 && self.width.target == 1.0 {
            return;
        }
        for fr in buf.chunks_exact_mut(CH) {
            let w = self.width.tick().clamp(0.0, 2.0);
            apply_width(fr, w);
        }
    }

    fn reset(&mut self) {
        self.width.reset_to_target();
    }
}

// ---------------------------------------------------------------------------
// Узлы Ка3 (Р.47, п.328): LFO-семейство
// ---------------------------------------------------------------------------

use super::primitives::{DelayLine, lfo_phase};

/// Общая LFO-половина модуляционных узлов: фаза — интеграл кривой частоты
/// по позиции клипа (`lfo_phase`), НЕ счётчик — seek/шов/заворот дают ту
/// же фазу, что холодный рендер (инвариант №11). Узел держит КОПИЮ точек
/// кривой rate_hz (bind при apply_params/set_auto): в process модель не
/// читается (инвариант №2).
struct LfoDrive {
    base_hz: f64,
    points: Vec<(f64, f64)>,
    clip_len: i64,
}

impl LfoDrive {
    fn new(base_hz: f64) -> LfoDrive {
        LfoDrive { base_hz, points: Vec::new(), clip_len: 1 }
    }

    fn bind(&mut self, auto: &AutoProfiles, slot: usize, clip_len: i64) {
        self.points = auto.points(slot, "rate_hz").to_vec();
        self.clip_len = clip_len.max(1);
    }

    /// Фаза в радианах на кадре `p` (клип-локально).
    #[inline]
    fn phase(&self, p: i64) -> f64 {
        let tau = p as f64 / self.clip_len as f64;
        lfo_phase(self.base_hz, &self.points, tau, self.clip_len) * std::f64::consts::TAU
    }
}

/// Тремоло: g = 1 − depth·(0.5 + 0.5·sin φ).
pub struct TremoloNode {
    lfo: LfoDrive,
    depth: SmoothedParam,
}

impl TremoloNode {
    fn new() -> TremoloNode {
        TremoloNode { lfo: LfoDrive::new(5.0), depth: SmoothedParam::with_ms(0.6, 20.0) }
    }

    fn apply(&mut self, rate_hz: f32, depth: f32) {
        self.lfo.base_hz = rate_hz as f64;
        self.depth.set_target(depth.clamp(0.0, 1.0));
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        self.depth.set_target(auto.value(slot, "depth", self.depth.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], pos: i64) {
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let d = self.depth.tick().clamp(0.0, 1.0);
            let g = 1.0 - d * (0.5 + 0.5 * self.lfo.phase(pos + i as i64).sin() as f32);
            for s in fr.iter_mut() {
                *s *= g;
            }
        }
    }

    fn reset(&mut self) {
        self.depth.reset_to_target();
    }
}

/// Автопан: p = depth·sin φ, закон панорамы — как у Gain.
pub struct AutoPanNode {
    lfo: LfoDrive,
    depth: SmoothedParam,
}

impl AutoPanNode {
    fn new() -> AutoPanNode {
        AutoPanNode { lfo: LfoDrive::new(1.0), depth: SmoothedParam::with_ms(0.8, 20.0) }
    }

    fn apply(&mut self, rate_hz: f32, depth: f32) {
        self.lfo.base_hz = rate_hz as f64;
        self.depth.set_target(depth.clamp(0.0, 1.0));
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        self.depth.set_target(auto.value(slot, "depth", self.depth.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], pos: i64) {
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let d = self.depth.tick().clamp(0.0, 1.0);
            let p = d * self.lfo.phase(pos + i as i64).sin() as f32;
            fr[0] *= (1.0 - p).min(1.0);
            if CH > 1 {
                fr[1] *= (1.0 + p).min(1.0);
            }
        }
    }

    fn reset(&mut self) {
        self.depth.reset_to_target();
    }
}

/// Вибрато: 100 % wet, delay = 1 мс + depth·(0.5+0.5·sin φ).
pub struct VibratoNode {
    lfo: LfoDrive,
    depth_ms: SmoothedParam,
    lines: [DelayLine; CH],
}

/// Максимум задержки вибрато: 1 мс базы + 5 мс глубины + запас.
const VIB_MAX_FRAMES: usize = (48_000.0 * 0.008) as usize;

impl VibratoNode {
    fn new() -> VibratoNode {
        VibratoNode {
            lfo: LfoDrive::new(5.0),
            depth_ms: SmoothedParam::with_ms(0.5, 20.0),
            lines: std::array::from_fn(|_| DelayLine::new(VIB_MAX_FRAMES)),
        }
    }

    fn apply(&mut self, rate_hz: f32, depth_ms: f32) {
        self.lfo.base_hz = rate_hz as f64;
        self.depth_ms.set_target(depth_ms.clamp(0.05, 5.0));
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        self.depth_ms
            .set_target(auto.value(slot, "depth_ms", self.depth_ms.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], pos: i64) {
        let sr = AUDIO_SAMPLE_RATE as f32;
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let d = self.depth_ms.tick().clamp(0.05, 5.0);
            let ph = self.lfo.phase(pos + i as i64).sin() as f32;
            let delay = (1.0 + d * (0.5 + 0.5 * ph)) * sr / 1000.0;
            for (ch, s) in fr.iter_mut().enumerate() {
                self.lines[ch].write(*s);
                *s = self.lines[ch].read_frac(delay);
            }
        }
    }

    fn tail(&self) -> Tick {
        Tick::from_audio_frames(VIB_MAX_FRAMES as i64)
    }

    fn reset(&mut self) {
        for l in &mut self.lines {
            l.reset();
        }
        self.depth_ms.reset_to_target();
    }
}

/// Хорус: base 15 мс, голос k — фаза +k·2π/voices, правый канал +90°.
pub struct ChorusNode {
    lfo: LfoDrive,
    depth_ms: SmoothedParam,
    mix: SmoothedParam,
    voices: u32,
    lines: [DelayLine; CH],
}

const CHORUS_BASE_MS: f32 = 15.0;
const CHORUS_MAX_FRAMES: usize = (48_000.0 * 0.030) as usize;

impl ChorusNode {
    fn new() -> ChorusNode {
        ChorusNode {
            lfo: LfoDrive::new(0.8),
            depth_ms: SmoothedParam::with_ms(3.5, 20.0),
            mix: SmoothedParam::with_ms(0.5, 20.0),
            voices: 2,
            lines: std::array::from_fn(|_| DelayLine::new(CHORUS_MAX_FRAMES)),
        }
    }

    fn apply(&mut self, rate_hz: f32, depth_ms: f32, mix: f32, voices: u32) {
        self.lfo.base_hz = rate_hz as f64;
        self.depth_ms.set_target(depth_ms.clamp(0.0, 12.0));
        self.mix.set_target(mix.clamp(0.0, 1.0));
        self.voices = voices.clamp(1, 3);
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        self.depth_ms
            .set_target(auto.value(slot, "depth_ms", self.depth_ms.target as f64, s) as f32);
        self.mix.set_target(auto.value(slot, "mix", self.mix.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], pos: i64) {
        let sr = AUDIO_SAMPLE_RATE as f32;
        let nv = self.voices.max(1);
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let d = self.depth_ms.tick().clamp(0.0, 12.0);
            let k = self.mix.tick().clamp(0.0, 1.0);
            let phi = self.lfo.phase(pos + i as i64);
            for (ch, s) in fr.iter_mut().enumerate() {
                let dry = *s;
                self.lines[ch].write(dry);
                let mut wet = 0.0f32;
                for v in 0..nv {
                    let ph = phi
                        + v as f64 * std::f64::consts::TAU / nv as f64
                        + if ch == 1 { std::f64::consts::FRAC_PI_2 } else { 0.0 };
                    let delay = (CHORUS_BASE_MS + d * (0.5 + 0.5 * ph.sin() as f32)) * sr / 1000.0;
                    wet += self.lines[ch].read_frac(delay);
                }
                wet /= nv as f32;
                *s = dry * (1.0 - k) + wet * k;
            }
        }
    }

    fn tail(&self) -> Tick {
        Tick::from_audio_frames(CHORUS_MAX_FRAMES as i64)
    }

    fn reset(&mut self) {
        for l in &mut self.lines {
            l.reset();
        }
        self.depth_ms.reset_to_target();
        self.mix.reset_to_target();
    }
}

/// Флэнжер: base 0.5 мс, обратная связь в кольцо (знак = инверсия фазы).
pub struct FlangerNode {
    lfo: LfoDrive,
    depth_ms: SmoothedParam,
    feedback: SmoothedParam,
    mix: SmoothedParam,
    lines: [DelayLine; CH],
}

const FLANGER_BASE_MS: f32 = 0.5;
const FLANGER_MAX_FRAMES: usize = (48_000.0 * 0.010) as usize;

impl FlangerNode {
    fn new() -> FlangerNode {
        FlangerNode {
            lfo: LfoDrive::new(0.25),
            depth_ms: SmoothedParam::with_ms(2.0, 20.0),
            feedback: SmoothedParam::with_ms(0.5, 20.0),
            mix: SmoothedParam::with_ms(0.5, 20.0),
            lines: std::array::from_fn(|_| DelayLine::new(FLANGER_MAX_FRAMES)),
        }
    }

    fn apply(&mut self, rate_hz: f32, depth_ms: f32, feedback: f32, mix: f32) {
        self.lfo.base_hz = rate_hz as f64;
        self.depth_ms.set_target(depth_ms.clamp(0.1, 8.0));
        self.feedback.set_target(feedback.clamp(-0.9, 0.9));
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        self.depth_ms
            .set_target(auto.value(slot, "depth_ms", self.depth_ms.target as f64, s) as f32);
        self.feedback
            .set_target(auto.value(slot, "feedback", self.feedback.target as f64, s) as f32);
        self.mix.set_target(auto.value(slot, "mix", self.mix.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], pos: i64) {
        let sr = AUDIO_SAMPLE_RATE as f32;
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let d = self.depth_ms.tick().clamp(0.1, 8.0);
            let fb = self.feedback.tick().clamp(-0.9, 0.9);
            let k = self.mix.tick().clamp(0.0, 1.0);
            let ph = self.lfo.phase(pos + i as i64).sin() as f32;
            let delay = (FLANGER_BASE_MS + d * (0.5 + 0.5 * ph)) * sr / 1000.0;
            for (ch, s) in fr.iter_mut().enumerate() {
                let dry = *s;
                let wet = self.lines[ch].read_frac(delay);
                self.lines[ch].write(dry + wet * fb);
                *s = dry * (1.0 - k) + wet * k;
            }
        }
    }

    fn tail(&self) -> Tick {
        tail_frames_for(FLANGER_MAX_FRAMES as f32, 0.9)
    }

    fn reset(&mut self) {
        for l in &mut self.lines {
            l.reset();
        }
        self.depth_ms.reset_to_target();
        self.feedback.reset_to_target();
        self.mix.reset_to_target();
    }
}

/// Фейзер: каскад 1-полюсных allpass, свип центра 200..2000 Гц (exp).
pub struct PhaserNode {
    lfo: LfoDrive,
    depth: SmoothedParam,
    feedback: SmoothedParam,
    mix: SmoothedParam,
    stages: u32,
    z: [[f32; 8]; CH],
    fb_state: [f32; CH],
}

impl PhaserNode {
    fn new() -> PhaserNode {
        PhaserNode {
            lfo: LfoDrive::new(0.4),
            depth: SmoothedParam::with_ms(0.7, 20.0),
            feedback: SmoothedParam::with_ms(0.3, 20.0),
            mix: SmoothedParam::with_ms(0.5, 20.0),
            stages: 4,
            z: [[0.0; 8]; CH],
            fb_state: [0.0; CH],
        }
    }

    fn apply(&mut self, rate_hz: f32, depth: f32, feedback: f32, stages: u32, mix: f32) {
        self.lfo.base_hz = rate_hz as f64;
        self.depth.set_target(depth.clamp(0.0, 1.0));
        self.feedback.set_target(feedback.clamp(0.0, 0.9));
        self.mix.set_target(mix.clamp(0.0, 1.0));
        self.stages = stages.clamp(2, 8);
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        self.depth.set_target(auto.value(slot, "depth", self.depth.target as f64, s) as f32);
        self.feedback
            .set_target(auto.value(slot, "feedback", self.feedback.target as f64, s) as f32);
        self.mix.set_target(auto.value(slot, "mix", self.mix.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32], pos: i64) {
        let sr = AUDIO_SAMPLE_RATE as f32;
        let n = self.stages.clamp(2, 8) as usize;
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let d = self.depth.tick().clamp(0.0, 1.0);
            let fb = self.feedback.tick().clamp(0.0, 0.9);
            let k = self.mix.tick().clamp(0.0, 1.0);
            let ph = 0.5 + 0.5 * self.lfo.phase(pos + i as i64).sin() as f32;
            // свип центра: 200..2000 Гц exp, глубина сжимает диапазон
            let f = 200.0 * (10.0f32).powf(ph * d);
            let t = (std::f32::consts::PI * f / sr).tan();
            let ap = (t - 1.0) / (t + 1.0);
            for (ch, s) in fr.iter_mut().enumerate() {
                let dry = *s;
                let mut x = dry + self.fb_state[ch] * fb;
                for st in 0..n {
                    // 1-полюсный allpass: y = a·x + z; z = x − a·y
                    let y = ap * x + self.z[ch][st];
                    self.z[ch][st] = x - ap * y;
                    x = y;
                }
                self.fb_state[ch] = x;
                *s = dry * (1.0 - k) + x * k;
            }
        }
    }

    fn reset(&mut self) {
        self.z = [[0.0; 8]; CH];
        self.fb_state = [0.0; CH];
        self.depth.reset_to_target();
        self.feedback.reset_to_target();
        self.mix.reset_to_target();
    }
}

// ---------------------------------------------------------------------------
// Узлы Ка4 (Р.47, п.329): эхо и динамика
// ---------------------------------------------------------------------------

use super::primitives::EnvFollower;

const DELAY_MAX_MS: f32 = 1500.0;
const DELAY_MAX_FRAMES: usize = (48_000.0 * (DELAY_MAX_MS / 1000.0)) as usize + 8;

/// Эхо: кольцо аллоцировано на максимум (смена времени — дробное чтение,
/// НЕ структурная); время сглаживается 100 мс — тейп-глайд без щелчков;
/// фильтр в петле — one-pole LP (хвост темнеет с каждым повтором,
/// наследие фишки F-AUD-009); pingpong — перекрёстная обратная связь.
pub struct DelayNode {
    time_ms: SmoothedParam,
    feedback: SmoothedParam,
    mix: SmoothedParam,
    pingpong: bool,
    lines: [DelayLine; CH],
    loop_lp: [f32; CH],
}

impl DelayNode {
    fn new() -> DelayNode {
        DelayNode {
            time_ms: SmoothedParam::with_ms(350.0, 100.0),
            feedback: SmoothedParam::with_ms(0.35, 20.0),
            mix: SmoothedParam::with_ms(0.35, 20.0),
            pingpong: false,
            lines: std::array::from_fn(|_| DelayLine::new(DELAY_MAX_FRAMES)),
            loop_lp: [0.0; CH],
        }
    }

    fn apply(&mut self, time_ms: f32, feedback: f32, mix: f32, pingpong: bool) {
        self.time_ms.set_target(time_ms.clamp(1.0, DELAY_MAX_MS));
        self.feedback.set_target(feedback.clamp(0.0, 0.9));
        self.mix.set_target(mix.clamp(0.0, 1.0));
        self.pingpong = pingpong;
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        self.time_ms.set_target(auto.value(slot, "time_ms", self.time_ms.target as f64, s) as f32);
        self.feedback
            .set_target(auto.value(slot, "feedback", self.feedback.target as f64, s) as f32);
        self.mix.set_target(auto.value(slot, "mix", self.mix.target as f64, s) as f32);
    }

    fn process(&mut self, buf: &mut [f32]) {
        let sr = AUDIO_SAMPLE_RATE as f32;
        // фильтр петли ~4 кГц: повторы темнеют
        let alpha = 1.0 - (-2.0 * std::f32::consts::PI * 4000.0 / sr).exp();
        for fr in buf.chunks_exact_mut(CH) {
            let d = self.time_ms.tick().clamp(1.0, DELAY_MAX_MS) * sr / 1000.0;
            let fb = self.feedback.tick().clamp(0.0, 0.9);
            let k = self.mix.tick().clamp(0.0, 1.0);
            let mut wet = [0f32; CH];
            for (w, line) in wet.iter_mut().zip(self.lines.iter()) {
                *w = line.read_frac(d);
            }
            for ch in 0..CH {
                let dry = fr[ch];
                // ping-pong: обратная связь уходит в ПРОТИВОПОЛОЖНЫЙ канал
                let fb_src = if self.pingpong { wet[(ch + 1) % CH] } else { wet[ch] };
                self.loop_lp[ch] += (fb_src - self.loop_lp[ch]) * alpha;
                self.lines[ch].write(dry + self.loop_lp[ch] * fb);
                fr[ch] = dry * (1.0 - k) + wet[ch] * k;
            }
        }
    }

    fn tail(&self, mix_on: bool) -> Tick {
        if !mix_on {
            return Tick::ZERO;
        }
        let frames = self.time_ms.target.clamp(1.0, DELAY_MAX_MS) / 1000.0 * 48_000.0;
        tail_frames_for(frames, self.feedback.target.clamp(0.01, 0.9))
    }

    fn reset(&mut self) {
        for l in &mut self.lines {
            l.reset();
        }
        self.loop_lp = [0.0; CH];
        self.time_ms.reset_to_target();
        self.feedback.reset_to_target();
        self.mix.reset_to_target();
    }
}

/// Компрессор: peak-огибающая, стерео-линк max(|l|,|r|), gr в дБ.
pub struct CompNode {
    threshold_db: SmoothedParam,
    ratio: SmoothedParam,
    makeup_db: SmoothedParam,
    env: EnvFollower,
}

impl CompNode {
    fn new() -> CompNode {
        CompNode {
            threshold_db: SmoothedParam::with_ms(-18.0, 20.0),
            ratio: SmoothedParam::with_ms(4.0, 20.0),
            makeup_db: SmoothedParam::with_ms(0.0, 20.0),
            env: EnvFollower::new(10.0, 150.0),
        }
    }

    fn apply(&mut self, thr: f32, ratio: f32, att: f32, rel: f32, makeup: f32) {
        self.threshold_db.set_target(thr.clamp(-60.0, 0.0));
        self.ratio.set_target(ratio.clamp(1.0, 20.0));
        self.makeup_db.set_target(makeup.clamp(0.0, 24.0));
        self.env.set_times(att.clamp(0.1, 250.0), rel.clamp(5.0, 2000.0));
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        self.threshold_db.set_target(auto.value(
            slot,
            "threshold_db",
            self.threshold_db.target as f64,
            s,
        ) as f32);
        self.ratio.set_target(auto.value(slot, "ratio", self.ratio.target as f64, s) as f32);
        self.makeup_db
            .set_target(auto.value(slot, "makeup_db", self.makeup_db.target as f64, s) as f32);
        let att = auto.value(slot, "attack_ms", 10.0, s) as f32;
        let rel = auto.value(slot, "release_ms", 150.0, s) as f32;
        self.env.set_times(att.clamp(0.1, 250.0), rel.clamp(5.0, 2000.0));
    }

    /// `key` — сайдчейн: детектор слушает ЕГО, а сжимается всё равно свой
    /// вход. Закон сжатия, пределы и сглаживание те же — ключ меняет
    /// только источник огибающей (Р.65). Ключ короче буфера (последний
    /// чанк окна) даёт для остатка пик 0 — это тише порога, то есть
    /// «ключ молчит», а не паника.
    fn process(&mut self, buf: &mut [f32], key: Option<&[f32]>) {
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let thr = self.threshold_db.tick();
            let ratio = self.ratio.tick().max(1.0);
            let makeup = 10f32.powf(self.makeup_db.tick() / 20.0);
            let peak = match key {
                None => fr.iter().fold(0f32, |m, s| m.max(s.abs())),
                Some(k) => k
                    .get(i * CH..i * CH + CH)
                    .map_or(0.0, |kf| kf.iter().fold(0f32, |m, s| m.max(s.abs()))),
            };
            let env = self.env.tick(peak).max(1e-6);
            let env_db = 20.0 * env.log10();
            let over = (env_db - thr).max(0.0);
            let gr_db = over * (1.0 - 1.0 / ratio);
            let g = 10f32.powf(-gr_db / 20.0) * makeup;
            for s in fr.iter_mut() {
                *s *= g;
            }
        }
    }

    fn reset(&mut self) {
        self.env.reset();
        self.threshold_db.reset_to_target();
        self.ratio.reset_to_target();
        self.makeup_db.reset_to_target();
    }
}

/// Длина окна упреждения лимитера в кадрах (Р.48, п.338). ОДИН дом
/// формулы: её зовут и узел (геометрия кольца), и статическая
/// `chain_lookahead` — разойтись они не могут по построению.
pub(crate) fn limiter_la_frames(lookahead_ms: f32) -> i64 {
    (lookahead_ms.clamp(1.0, 20.0) as f64 / 1000.0 * 48_000.0).ceil() as i64
}

/// Look-ahead лимитер (Р.48, п.338). Потолок гарантирован АЛГЕБРАИЧЕСКИ:
/// окно скользящего максимума покрывает материал [t, t+La], включая сам
/// выходной кадр, поэтому редукция успевает до пика и атака мгновенна —
/// платы искажениями нет. Выход выровнен со входом цепочки: кольцо
/// задерживает сигнал ровно на La, которое процессор вычитал из позиции.
pub struct LimiterNode {
    threshold_db: SmoothedParam,
    ceiling_db: SmoothedParam,
    release_ms: f32,
    /// Геометрия окна: меняется ТОЛЬКО при structural — расхождение с
    /// цепочкой ловит FxStateStore и пересобирает процессор целиком.
    la: i64,
    /// Кольцо выравнивания (la кадров × CH).
    ring: Vec<f32>,
    pos: usize,
    /// Моноочередь скользящего максимума |x|: значения убывают, спереди
    /// максимум окна. O(1) на кадр амортизированно.
    win: std::collections::VecDeque<(i64, f32)>,
    /// Сколько кадров подано в окно (возраст элементов).
    fed: i64,
    gain: f32,
}

impl LimiterNode {
    fn new() -> LimiterNode {
        LimiterNode {
            threshold_db: SmoothedParam::with_ms(-6.0, 20.0),
            ceiling_db: SmoothedParam::with_ms(-0.3, 20.0),
            release_ms: 80.0,
            la: 0,
            ring: Vec::new(),
            pos: 0,
            win: std::collections::VecDeque::new(),
            fed: 0,
            gain: 1.0,
        }
    }

    fn apply(&mut self, thr: f32, ceil: f32, lookahead_ms: f32, release: f32, structural: bool) {
        self.threshold_db.set_target(thr.clamp(-60.0, 0.0));
        self.ceiling_db.set_target(ceil.clamp(-24.0, 0.0));
        self.release_ms = release.clamp(5.0, 1000.0);
        let la = limiter_la_frames(lookahead_ms);
        if structural || self.ring.is_empty() {
            self.la = la;
            self.ring = vec![0.0; la.max(1) as usize * CH];
            self.pos = 0;
            self.win.clear();
            self.fed = 0;
        }
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        self.threshold_db.set_target(auto.value(
            slot,
            "threshold_db",
            self.threshold_db.target as f64,
            s,
        ) as f32);
        self.ceiling_db
            .set_target(auto.value(slot, "ceiling_db", self.ceiling_db.target as f64, s) as f32);
        self.release_ms =
            (auto.value(slot, "release_ms", self.release_ms as f64, s) as f32).clamp(5.0, 1000.0);
    }

    fn process(&mut self, buf: &mut [f32]) {
        if self.la <= 0 {
            return;
        }
        let rel_coef = 1.0 - (-1.0 / (self.release_ms / 1000.0 * 48_000.0)).exp();
        for fr in buf.chunks_exact_mut(CH) {
            let thr = 10f32.powf(self.threshold_db.tick() / 20.0);
            let ceil = 10f32.powf(self.ceiling_db.tick() / 20.0);
            // в окно кладётся ВХОДЯЩИЙ кадр (позиция t + La)
            let peak = fr.iter().fold(0f32, |m, v| m.max(v.abs()));
            while let Some(&(_, v)) = self.win.back() {
                if v <= peak {
                    self.win.pop_back();
                } else {
                    break;
                }
            }
            self.win.push_back((self.fed, peak));
            let cutoff = self.fed - self.la;
            while let Some(&(age, _)) = self.win.front() {
                if age < cutoff {
                    self.win.pop_front();
                } else {
                    break;
                }
            }
            self.fed += 1;
            // выход — кадр, задержанный на La (позиция t)
            for v in fr.iter_mut() {
                std::mem::swap(&mut self.ring[self.pos], v);
                self.pos += 1;
            }
            if self.pos >= self.ring.len() {
                self.pos = 0;
            }
            // максимум по [t, t+La] — включая сам выходной кадр, поэтому
            // out = wmax * g ≤ ceiling строго
            let wmax = self.win.front().map(|&(_, v)| v).unwrap_or(0.0);
            let target = (thr / wmax.max(thr)) * (ceil / thr);
            if target < self.gain {
                self.gain = target;
            } else {
                self.gain += (target - self.gain) * rel_coef;
            }
            for v in fr.iter_mut() {
                *v *= self.gain;
            }
        }
    }

    fn reset(&mut self) {
        self.ring.fill(0.0);
        self.pos = 0;
        self.win.clear();
        self.fed = 0;
        self.gain = 1.0;
        self.threshold_db.reset_to_target();
        self.ceiling_db.reset_to_target();
    }
}

/// Упреждение питча (Р.48, п.339): грейн повышения на две октавы читает
/// вход вчетверо быстрее выхода, и к концу грейна ему нужен материал на
/// `LG·4` кадров вперёд плюс окно WSOLA-поиска. Константа ОДНА на оба
/// движка: вокодеру хватает меньшего, лишнее для него — просто история.
/// Меньше `MIN_PREROLL_FRAMES` (4800) — прогрев seek'а всегда заполняет
/// кольцо настоящим материалом.
pub(crate) const PITCH_LOOKAHEAD_FRAMES: i64 = 4608;

/// Шаг синтез-грида WSOLA и хоп вокодера (абсолютный, от нуля клипа —
/// поэтому нарезка буферов на результат не влияет).
const PITCH_HOP: i64 = 512;
/// Длина зерна WSOLA (окно Ханна, перекрытие 50 %: сумма окон ≡ 1).
const PITCH_GRAIN: usize = 1024;
/// Полуширина WSOLA-поиска и его шаг (кандидаты перебираются по
/// возрастанию |off| — тай-брейк детерминирован).
const PITCH_SEARCH: i64 = 256;
const PITCH_SEARCH_STEP: i64 = 8;
/// Размер кадра фазового вокодера.
const PV_N: usize = 2048;
/// Кольца: вход обязан покрывать [выход − поиск, вход + чанк].
const PITCH_RING: usize = 8192;
/// Аккумулятор OLA: обязан вмещать чанк процессора (1024) ПЛЮС полное
/// окно синтеза (2048 у вокодера) — иначе зерно, синтезированное
/// вперёд, затирало бы ещё не отданные кадры, и результат зависел бы от
/// нарезки буферов (ровно это поймал t338 на кольце 2048).
const PITCH_ACC: usize = 4096;

/// Нейтраль питча: и база, и кривые на нуле. Нейтральный узел не трогает
/// буфер вовсе (байт-в-байт прежний звук) и не требует упреждения.
pub(crate) fn pitch_is_neutral(
    semitones: f32,
    cents: f32,
    st_points: &[(f64, f64)],
    ct_points: &[(f64, f64)],
) -> bool {
    semitones == 0.0
        && cents == 0.0
        && st_points.iter().all(|(_, v)| *v == 0.0)
        && ct_points.iter().all(|(_, v)| *v == 0.0)
}

/// Состояние фазового вокодера: планы FFT и фазовые аккумуляторы.
struct PvState {
    fwd: std::sync::Arc<dyn realfft::RealToComplex<f32>>,
    inv: std::sync::Arc<dyn realfft::ComplexToReal<f32>>,
    /// Фазы по КАНАЛАМ: стерео-стороны не должны интерферировать.
    prev_phase: [Vec<f32>; CH],
    synth_phase: [Vec<f32>; CH],
    window: Vec<f32>,
    time: Vec<f32>,
    spec: Vec<realfft::num_complex::Complex<f32>>,
    out_spec: Vec<realfft::num_complex::Complex<f32>>,
    mag: Vec<f32>,
    freq: Vec<f32>,
}

impl PvState {
    fn new() -> PvState {
        let mut planner = realfft::RealFftPlanner::<f32>::new();
        let fwd = planner.plan_fft_forward(PV_N);
        let inv = planner.plan_fft_inverse(PV_N);
        let bins = PV_N / 2 + 1;
        PvState {
            window: (0..PV_N)
                .map(|i| {
                    let x = std::f32::consts::TAU * i as f32 / PV_N as f32;
                    0.5 - 0.5 * x.cos()
                })
                .collect(),
            prev_phase: std::array::from_fn(|_| vec![0.0; bins]),
            synth_phase: std::array::from_fn(|_| vec![0.0; bins]),
            time: vec![0.0; PV_N],
            spec: fwd.make_output_vec(),
            out_spec: inv.make_input_vec(),
            mag: vec![0.0; bins],
            freq: vec![0.0; bins],
            fwd,
            inv,
        }
    }

    fn reset(&mut self) {
        for p in &mut self.prev_phase {
            p.fill(0.0);
        }
        for p in &mut self.synth_phase {
            p.fill(0.0);
        }
    }
}

/// Смена высоты без смены длительности (Р.48, п.339). Узел УПРЕЖДАЮЩИЙ:
/// читает вход с опережением `PITCH_LOOKAHEAD_FRAMES` и отдаёт выход
/// выровненным — задержки для движка нет (п.337).
///
/// Оба движка синтезируют на АБСОЛЮТНОМ гриде позиции клипа: зерно
/// номер g всегда покрывает выходные кадры [g·HOP, g·HOP + GRAIN), а
/// коэффициент высоты берётся по позиции ЗЕРНА — поэтому результат не
/// зависит ни от нарезки буферов, ни от числа вызовов (класс t325а).
pub struct PitchNode {
    engine: kt_model::PitchEngine,
    semitones: f32,
    cents: f32,
    st_points: Vec<(f64, f64)>,
    ct_points: Vec<(f64, f64)>,
    /// Формы отрезков кривых (Р.61): узел, читающий значение ПО ПОЗИЦИИ,
    /// обязан видеть тот же ряд форм, что и остальная программа.
    st_shapes: Vec<kt_model::SegShape>,
    ct_shapes: Vec<kt_model::SegShape>,
    clip_len: i64,
    /// 0 — нейтраль (узел прозрачен).
    la: i64,
    /// Входное кольцо по абсолютным позициям клипа.
    ring: Vec<f32>,
    /// Позиция следующего кадра записи; `i64::MIN` — потока ещё не было.
    written: i64,
    /// OLA-аккумулятор выхода (тоже по абсолютным позициям).
    acc: Vec<f32>,
    /// Номер следующего зерна к синтезу.
    next_grain: i64,
    pv: Option<Box<PvState>>,
}

impl PitchNode {
    fn new() -> PitchNode {
        PitchNode {
            engine: kt_model::PitchEngine::default(),
            semitones: 0.0,
            cents: 0.0,
            st_points: Vec::new(),
            ct_points: Vec::new(),
            st_shapes: Vec::new(),
            ct_shapes: Vec::new(),
            clip_len: 1,
            la: 0,
            ring: vec![0.0; PITCH_RING * CH],
            written: i64::MIN,
            acc: vec![0.0; PITCH_ACC * CH],
            next_grain: 0,
            pv: None,
        }
    }

    fn apply(&mut self, semitones: f32, cents: f32, engine: kt_model::PitchEngine) {
        self.semitones = semitones.clamp(-24.0, 24.0);
        self.cents = cents.clamp(-100.0, 100.0);
        self.engine = engine;
        self.refresh_la();
    }

    /// Кривые питча (тот же паттерн, что у LFO-узлов): значение берётся
    /// по позиции ЗЕРНА, а не тиками сглаживания — иначе результат зависел
    /// бы от числа вызовов.
    fn bind(&mut self, auto: &AutoProfiles, slot: usize, clip_len: i64) {
        self.st_points = auto.points(slot, "semitones").to_vec();
        self.ct_points = auto.points(slot, "cents").to_vec();
        self.st_shapes = auto.shapes_of(slot, "semitones").to_vec();
        self.ct_shapes = auto.shapes_of(slot, "cents").to_vec();
        self.clip_len = clip_len.max(1);
        self.refresh_la();
    }

    fn refresh_la(&mut self) {
        let neutral =
            pitch_is_neutral(self.semitones, self.cents, &self.st_points, &self.ct_points);
        let la = if neutral { 0 } else { PITCH_LOOKAHEAD_FRAMES };
        if la != self.la {
            self.la = la;
            self.reset();
        }
        if la > 0 && self.engine == kt_model::PitchEngine::Vocoder && self.pv.is_none() {
            self.pv = Some(Box::new(PvState::new()));
        }
    }

    /// Коэффициент высоты на позиции `p` (клип-локальный кадр).
    fn ratio_at(&self, p: i64) -> f64 {
        let tau = p as f64 / self.clip_len as f64;
        let st = kt_model::param_at(self.semitones as f64, &self.st_points, tau, &self.st_shapes);
        let ct = kt_model::param_at(self.cents as f64, &self.ct_points, tau, &self.ct_shapes);
        (2f64).powf((st + ct / 100.0) / 12.0).clamp(0.25, 4.0)
    }

    #[inline]
    fn ring_at(&self, frame: i64, ch: usize) -> f32 {
        let i = frame.rem_euclid(PITCH_RING as i64) as usize;
        self.ring[i * CH + ch]
    }

    /// Линейная интерполяция входа на дробной позиции.
    #[inline]
    fn ring_frac(&self, pos: f64, ch: usize) -> f32 {
        let i = pos.floor();
        let f = (pos - i) as f32;
        let a = self.ring_at(i as i64, ch);
        let b = self.ring_at(i as i64 + 1, ch);
        a * (1.0 - f) + b * f
    }

    #[inline]
    fn acc_at(&mut self, frame: i64, ch: usize) -> &mut f32 {
        let i = frame.rem_euclid(PITCH_ACC as i64) as usize;
        &mut self.acc[i * CH + ch]
    }

    /// Разрыв потока (шов, seek): кольца и грид переинициализируются от
    /// новой позиции — узел переживает шов, как реверб, просто без хвоста.
    fn restart(&mut self, in_pos: i64) {
        self.ring.fill(0.0);
        self.acc.fill(0.0);
        self.written = in_pos;
        self.next_grain = (in_pos - self.la).div_euclid(PITCH_HOP);
        if let Some(pv) = &mut self.pv {
            pv.reset();
        }
    }

    fn process(&mut self, buf: &mut [f32], in_pos: i64) {
        if self.la == 0 {
            return; // нейтраль: буфер не трогаем вовсе
        }
        let n = (buf.len() / CH) as i64;
        if self.written != in_pos {
            self.restart(in_pos);
        }
        // 1) вход в кольцо
        for (i, fr) in buf.chunks_exact(CH).enumerate() {
            let idx = (in_pos + i as i64).rem_euclid(PITCH_RING as i64) as usize;
            for (ch, v) in fr.iter().enumerate() {
                self.ring[idx * CH + ch] = *v;
            }
        }
        self.written = in_pos + n;
        // 2) синтез зёрен, покрывающих нужный выход
        let out_pos = in_pos - self.la;
        let out_end = out_pos + n;
        while self.next_grain * PITCH_HOP < out_end {
            let g = self.next_grain;
            match self.engine {
                kt_model::PitchEngine::Wsola => self.grain_wsola(g),
                kt_model::PitchEngine::Vocoder => self.grain_vocoder(g),
            }
            self.next_grain += 1;
        }
        // 3) выход из аккумулятора (прочитанное обнуляется — кольцо
        //    переиспользуется следующими зёрнами)
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let t = out_pos + i as i64;
            for (ch, v) in fr.iter_mut().enumerate() {
                let a = self.acc_at(t, ch);
                *v = *a;
                *a = 0.0;
            }
        }
    }

    /// Зерно WSOLA: читает вход со скоростью `r` от позиции зерна и
    /// подмешивает в аккумулятор с окном Ханна. Сдвиг чтения выбирается
    /// кросс-корреляцией с уже синтезированным перекрытием — это и
    /// сохраняет форму удара (в отличие от вокодера).
    fn grain_wsola(&mut self, g: i64) {
        let start = g * PITCH_HOP;
        let r = self.ratio_at(start);
        let off = self.best_offset(start, r);
        let base = (start + off) as f64;
        for k in 0..PITCH_GRAIN {
            let w = hann(k, PITCH_GRAIN);
            let src = base + k as f64 * r;
            for ch in 0..CH {
                let v = self.ring_frac(src, ch) * w;
                *self.acc_at(start + k as i64, ch) += v;
            }
        }
    }

    /// WSOLA-поиск: кандидаты перебираются по возрастанию |off|, побеждает
    /// строго больший счёт — значит при равенстве остаётся меньший сдвиг
    /// (детерминизм без зависимости от порядка обхода).
    fn best_offset(&self, start: i64, r: f64) -> i64 {
        let mut cands: Vec<i64> = vec![0];
        let mut d = PITCH_SEARCH_STEP;
        while d <= PITCH_SEARCH {
            cands.push(-d);
            cands.push(d);
            d += PITCH_SEARCH_STEP;
        }
        let overlap = PITCH_HOP as usize;
        let mut best = 0i64;
        let mut best_score = f64::NEG_INFINITY;
        for off in cands {
            let base = (start + off) as f64;
            let (mut num, mut den) = (0f64, 0f64);
            for k in (0..overlap).step_by(2) {
                let src = base + k as f64 * r;
                let cand = (self.ring_frac(src, 0) + self.ring_frac(src, 1)) as f64;
                let i = (start + k as i64).rem_euclid(PITCH_ACC as i64) as usize;
                let prev = (self.acc[i * CH] + self.acc[i * CH + 1]) as f64;
                num += cand * prev;
                den += cand * cand;
            }
            let score = num / (den.sqrt() + 1e-9);
            if score > best_score {
                best_score = score;
                best = off;
            }
        }
        best
    }

    /// Зерно фазового вокодера: спектр кадра переносится в бины `k·r` с
    /// пересчётом фазы по мгновенной частоте. Гармоники едут чисто,
    /// транзиент размазывается — цена спектрального метода.
    fn grain_vocoder(&mut self, g: i64) {
        let start = g * PITCH_HOP;
        let r = self.ratio_at(start) as f32;
        let bins = PV_N / 2 + 1;
        let hop = PITCH_HOP as f32;
        let expected = std::f32::consts::TAU * hop / PV_N as f32;
        for ch in 0..CH {
            {
                let pv = self.pv.as_deref_mut().expect("vocoder state");
                for i in 0..PV_N {
                    pv.time[i] = 0.0;
                }
            }
            // анализ: окно из кольца
            for i in 0..PV_N {
                let v = self.ring_at(start + i as i64, ch);
                let pv = self.pv.as_deref_mut().expect("vocoder state");
                pv.time[i] = v * pv.window[i];
            }
            let pv = self.pv.as_deref_mut().expect("vocoder state");
            pv.fwd.process(&mut pv.time, &mut pv.spec).expect("fft");
            // Анализ: магнитуда и МГНОВЕННАЯ частота каждого бина
            // (принципальная девиация фазы против ожидаемой за хоп).
            for k in 0..bins {
                let c = pv.spec[k];
                let m = (c.re * c.re + c.im * c.im).sqrt();
                let phase = c.im.atan2(c.re);
                let mut dp = phase - pv.prev_phase[ch][k] - k as f32 * expected;
                dp -= std::f32::consts::TAU * (dp / std::f32::consts::TAU).round();
                pv.prev_phase[ch][k] = phase;
                pv.mag[k] = m;
                pv.freq[k] = k as f32 * expected + dp;
            }
            // Синтез ОБРАТНЫМ отображением (как в геометрии видео):
            // целевой бин j спрашивает источник в позиции j/r и получает
            // единственную частоту — при прямом переносе k → round(k·r)
            // несколько источников делили бы один бин, и высота уезжала
            // (замер: 612 Гц вместо 659 на +7 полутонах).
            for j in 0..bins {
                let src = j as f32 / r;
                let k0 = src.floor() as usize;
                if k0 + 1 >= bins {
                    pv.out_spec[j] = realfft::num_complex::Complex::new(0.0, 0.0);
                    continue;
                }
                let f = src - k0 as f32;
                let m = pv.mag[k0] * (1.0 - f) + pv.mag[k0 + 1] * f;
                let w = (pv.freq[k0] * (1.0 - f) + pv.freq[k0 + 1] * f) * r;
                pv.synth_phase[ch][j] += w;
                let (sn, cs) = pv.synth_phase[ch][j].sin_cos();
                pv.out_spec[j] = realfft::num_complex::Complex::new(m * cs, m * sn);
            }
            // DC и Найквист у вещественного сигнала вещественны —
            // realfft это проверяет и иначе отказывается считать
            pv.out_spec[0].im = 0.0;
            let last = bins - 1;
            pv.out_spec[last].im = 0.0;
            pv.inv.process(&mut pv.out_spec, &mut pv.time).expect("ifft");
            let norm = 1.0 / PV_N as f32;
            for i in 0..PV_N {
                pv.time[i] *= pv.window[i] * norm;
            }
            for i in 0..PV_N {
                let v = self.pv.as_deref().expect("vocoder state").time[i];
                *self.acc_at(start + i as i64, ch) += v;
            }
        }
    }

    fn reset(&mut self) {
        self.ring.fill(0.0);
        self.acc.fill(0.0);
        self.written = i64::MIN;
        self.next_grain = 0;
        if let Some(pv) = &mut self.pv {
            pv.reset();
        }
    }
}

/// Окно Ханна с перекрытием 50 %: сумма двух соседних окон ≡ 1.
#[inline]
fn hann(i: usize, n: usize) -> f32 {
    let x = std::f32::consts::TAU * i as f32 / n as f32;
    0.5 - 0.5 * x.cos()
}

/// Гейт: быстрый детектор (~0.5 мс) + гистерезис −6 дБ + hold-счётчик
/// (кадры — переживает шов) + рампы attack/release.
pub struct GateNode {
    threshold_db: SmoothedParam,
    attack_ms: f32,
    release_ms: f32,
    hold_frames: i64,
    env: EnvFollower,
    open: bool,
    hold_left: i64,
    gain: f32,
}

impl GateNode {
    fn new() -> GateNode {
        GateNode {
            threshold_db: SmoothedParam::with_ms(-40.0, 20.0),
            attack_ms: 1.0,
            release_ms: 100.0,
            hold_frames: (0.040 * 48_000.0) as i64,
            env: EnvFollower::new(0.5, 0.5),
            open: false,
            hold_left: 0,
            gain: 0.0,
        }
    }

    fn apply(&mut self, thr: f32, att: f32, hold: f32, rel: f32) {
        self.threshold_db.set_target(thr.clamp(-80.0, 0.0));
        self.attack_ms = att.clamp(0.1, 50.0);
        self.release_ms = rel.clamp(5.0, 1000.0);
        self.hold_frames = ((hold.clamp(0.0, 500.0) / 1000.0) * 48_000.0) as i64;
    }

    fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        self.threshold_db.set_target(auto.value(
            slot,
            "threshold_db",
            self.threshold_db.target as f64,
            s,
        ) as f32);
        self.attack_ms =
            (auto.value(slot, "attack_ms", self.attack_ms as f64, s) as f32).clamp(0.1, 50.0);
        self.release_ms =
            (auto.value(slot, "release_ms", self.release_ms as f64, s) as f32).clamp(5.0, 1000.0);
        let hold = auto.value(slot, "hold_ms", (self.hold_frames as f64) / 48.0, s) as f32;
        self.hold_frames = ((hold.clamp(0.0, 500.0) / 1000.0) * 48_000.0) as i64;
    }

    fn process(&mut self, buf: &mut [f32]) {
        let sr = AUDIO_SAMPLE_RATE as f32;
        let att_step = 1.0 / (self.attack_ms / 1000.0 * sr).max(1.0);
        let rel_step = 1.0 / (self.release_ms / 1000.0 * sr).max(1.0);
        for fr in buf.chunks_exact_mut(CH) {
            let thr_db = self.threshold_db.tick();
            let open_thr = 10f32.powf(thr_db / 20.0);
            // гистерезис: закрытие на 6 дБ ниже открытия
            let close_thr = 10f32.powf((thr_db - 6.0) / 20.0);
            let peak = fr.iter().fold(0f32, |m, s| m.max(s.abs()));
            let env = self.env.tick(peak);
            if env >= open_thr {
                self.open = true;
                self.hold_left = self.hold_frames;
            } else if env < close_thr {
                if self.hold_left > 0 {
                    self.hold_left -= 1;
                } else {
                    self.open = false;
                }
            }
            self.gain = if self.open {
                (self.gain + att_step).min(1.0)
            } else {
                (self.gain - rel_step).max(0.0)
            };
            for s in fr.iter_mut() {
                *s *= self.gain;
            }
        }
    }

    fn reset(&mut self) {
        self.env.reset();
        self.open = false;
        self.hold_left = 0;
        self.gain = 0.0;
        self.threshold_db.reset_to_target();
    }
}
