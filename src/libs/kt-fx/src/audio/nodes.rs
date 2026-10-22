//! Audio nodes of the chain (R.47): Freeverb (a verbatim move from lib.rs) and —
//! from the next step of Ka1 — enum AudioNode with all the effects.

use super::primitives::{
    ALLPASS_TUNINGS, Allpass, COMB_TUNINGS, Comb, STEREO_SPREAD, SmoothedParam, tail_frames_for,
};
use crate::CH;
use kt_time::{AUDIO_SAMPLE_RATE, Tick};

/// Stereo freeverb. Changing delay re-creates the lines (this is a structural change of
/// the geometry — unavoidable), but wet/dry are smoothed and do NOT touch the buffers.
pub struct Freeverb {
    combs: [Vec<Comb>; CH],
    allpasses: [Vec<Allpass>; CH],
    wet: SmoothedParam,
    delay_ms: f32,
    damp: f32,
    feedback: f32,
    wet_gain: f32,
    /// Pre-delay of the reverb input (R.33 C5): a ring per channel. Empty —
    /// the previous path byte-for-byte.
    predelay: [Vec<f32>; CH],
    pre_pos: usize,
    predelay_ms: f32,
    /// Stereo tail width 0..1: 1 — the previous “real” stereo reverb,
    /// 0 — a common mono tail for both channels.
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
        // structural change of geometry: re-creation is allowed only for a
        // really different delay (threshold 0.5 ms, as in the old code, but wet/dry
        // does not touch this); pre-delay — the same geometry
        if (delay_ms - self.delay_ms).abs() > 0.5 || (predelay_ms - self.predelay_ms).abs() > 0.5 {
            let keep_wet = self.wet;
            *self =
                Freeverb::new(keep_wet.target, delay_ms, damp, decay, wet_gain, predelay_ms, width);
            self.wet = keep_wet;
        }
    }

    /// Only the wet-mix target (automation) — smoothed, buffers not touched.
    pub fn set_mix(&mut self, mix: f32) {
        self.wet.set_target(mix.clamp(0.0, 1.0));
    }

    /// Tail: decay time to −60 dB (t60) along the longest comb line.
    pub fn tail(&self) -> Tick {
        let max_len = self.combs[CH - 1].iter().map(|c| c.buf.len()).max().unwrap_or(1);
        tail_frames_for(max_len as f32, self.feedback)
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        // the previous path (without pre-delay, full width) remains
        // VERBATIM: GOLDEN_AUDIO relies on it
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
            // width: 1 — channels as they are, 0 — a common mono tail
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
            // predelay also holds material (up to 200 ms) — without clearing it
            // it leaked into the tail after a seek (t319, R.47 item 324)
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
// Node chain (R.47, item 325): processing goes IN SLOT ORDER
// ---------------------------------------------------------------------------

use kt_model::{AutoProfiles, FxInstance};

/// Node rules:
/// - `process(buf, pos)` — pos = clip-local frame of the buffer start
///   (LFO/grid determinism: the value is a function of the position, not of a counter);
/// - `apply(fx, structural)` sets the parameter BASES (and re-creates the buffers
///   only on structural or a real geometry change);
/// - `set_targets(auto, slot, s)` is called once per chunk ONLY for a non-empty
///   `auto` (produce gate, bytes of old chains) — the node sets the smoothing
///   targets along its curves;
/// - a node must survive `seam` (the state lives on) and ring out on zero input
///   in the tail (`tail()` > 0 ⇒ the tail is honest).
pub enum AudioNode {
    Gain(GainNode),
    /// Layer-strength marker: the dry-side snapshot and the blend are executed by
    /// `produce()` — the node needs an inter-node buffer, which it does not own.
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
    /// Build the node for a slot. Non-audio node kinds do not get one (the chain
    /// does not see them — they are transparent).
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
            // the pitch engine — a PROJECT setting: it does not come from the slot,
            // but from ChainOpts (see ClipProcessor::new_with_opts)
            (AudioNode::Pitch(n), FxInstance::PitchShift { semitones, cents }) => {
                let engine = n.engine;
                n.apply(*semitones, *cents, engine);
            }
            // the chain fingerprint matched — the kinds by slots match by
            // construction; one can get here only through a matching error
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
            // pitch reads curves BY GRAIN POSITION (bind), it does not need targets
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

    /// Bind the LFO nodes to their frequency and clip-length curves (a copy of the points —
    /// the model is not read in process). Called from apply_params/set_auto/
    /// set_clip_len of the processor.
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

    /// Chunk processing. `key` — the KEY (sidechain) signal of the same window and
    /// the same domain (f32 48 kHz/2ch), if the caller provided it; a node that
    /// has no second input must ignore it verbatim —
    /// otherwise the key branch would cost bytes where no key exists.
    ///
    /// The key has NO state of its own, deliberately: a setter living between
    /// calls would introduce a hidden dependence on the call order, while
    /// invariant 2 requires that only the render thread touch the state, and only
    /// within its own window.
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

    /// Lookahead of the node in frames (R.48, item 337): how much CONTENT
    /// ahead of the output position the node needs in order to give its output WITHOUT
    /// delay. A node with a non-zero lookahead must itself align
    /// the output with its own ring and live in BOTH formulas — here and in
    /// the static `chain_lookahead` (their agreement is guarded by t322g).
    ///
    /// The lookahead is NOT the tail: it enters the seek warm-up and the source window, but
    /// does NOT extend `ext_end` (otherwise the clip would sound
    /// after its own end).
    /// Pass the pitch engine (a PROJECT setting, not a slot’s) to the node:
    /// it receives it when the processor is created; a change of engine — a rebuild
    /// of the whole processor (guarded by the opts check in FxStateStore).
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

/// Slot volume/balance. The constant path — the verbatim multipliers of the old
/// code (GOLDEN_AUDIO relies on them); with curves — a SmoothedParam with targets at the
/// chunk midpoint, a tick on every frame (the same loop order: first
/// volume per frame, then balance per frame).
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
        // balance: linear law, at zero both multipliers are exactly 1.0 —
        // the previous path byte-for-byte
        let p = pan.clamp(-1.0, 1.0);
        self.static_pan_l = (1.0 - p).min(1.0);
        self.static_pan_r = (1.0 + p).min(1.0);
    }

    /// Whether the slot has curves — by the fact of the profiles (as gain_slots/pan_slots
    /// were previously collected from non-empty points).
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

/// The reverb half of the Slowed+Reverb slot (the warp half lives in a separate
/// processor field — the warp holds the monopoly on consuming the source).
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
        // mix is smoothed by a one-pole inside the reverb (the same call as before
        // R.47 — the bytes of a single-slot chain do not move)
        self.rv.set_mix(auto.value(slot, "mix", self.base_mix, s) as f32);
    }
}

// ---------------------------------------------------------------------------
// Ka2 nodes (R.47, item 327): filters, EQ, distortion, bitcrusher, width
// ---------------------------------------------------------------------------

use super::primitives::{Biquad, BiquadKind, apply_width, softclip};
use kt_model::FilterMode;

/// Coefficient recalculation — in sub-blocks: a chunk of 1024 (21 ms) on frequency
/// sweeps is audible as zipper; 128 frames — not.
const TUNE_STEP: usize = 128;

/// Resonant cut LP/HP/BP: RBJ biquad per channel.
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
        // changing the kind does NOT re-create the state (2 f32 per channel): the transient
        // from changing the shape is allowed and shorter than any re-creation click
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
        // resonance ringing: t60 = 6.9·Q/(π·f0)
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

/// Three-band EQ: shelf + peak + shelf, each — a biquad per channel.
/// The Q of the EQ shelves and the mid-band Q floor — the SAME numbers for the
/// sound and for the frequency response graph (R.48, item 347).
pub(crate) const EQ_SHELF_Q: f32 = 0.707;
pub(crate) const EQ_MID_Q_FLOOR: f32 = 0.3;

/// Frequency response of the whole equalizer at frequency `at_hz`, dB. Values — in the order
/// `EqNode::NAMES`: low_gain, low_freq, mid_gain, mid_freq, mid_q,
/// high_gain, high_freq. The cascade = the sum of dB; the filters are tuned by the same
/// `tune` that sounds, so the curve in the UI equals what is heard.
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

/// The seven smoothed EQ parameters in one block (low_g, low_f, mid_g,
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
            // all gains at zero — plain pass-through: the EQ is transparent byte-for-byte
            if v[0] == 0.0 && v[2] == 0.0 && v[5] == 0.0 {
                continue;
            }
            for ch in 0..CH {
                // the same numbers as the frequency response graph (R.48): the same constants
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

/// Soft saturation with level compensation and tone.
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
            // tone: one-pole LP 500..12000 Hz (exp-map)
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

/// Lo-fi: bit-depth quantization + sample hold on the POSITION GRID
/// (floor(pos·rate/sr) — determinism: two renders of one clip give the same
/// artifacts, precedent — the seed of video noise).
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
            // the hold grid is tied to the clip’s POSITION, not to a counter
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

/// Stereo m/s width. width == 1.0 — an EXPLICIT pass-through (m/s repacking is
/// not byte-identical).
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
// Ka3 nodes (R.47, item 328): the LFO family
// ---------------------------------------------------------------------------

use super::primitives::{DelayLine, lfo_phase};

/// The common LFO half of the modulation nodes: the phase is the integral of the frequency curve
/// over the clip position (`lfo_phase`), NOT a counter — seek/seam/wrap give the
/// same phase as a cold render (invariant 11). The node holds a COPY of the points of the
/// rate_hz curve (bind on apply_params/set_auto): the model is not
/// read in process (invariant 2).
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

    /// Phase in radians at frame `p` (clip-local).
    #[inline]
    fn phase(&self, p: i64) -> f64 {
        let tau = p as f64 / self.clip_len as f64;
        lfo_phase(self.base_hz, &self.points, tau, self.clip_len) * std::f64::consts::TAU
    }
}

/// Tremolo: g = 1 − depth·(0.5 + 0.5·sin φ).
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

/// Autopan: p = depth·sin φ, the pan law — as in Gain.
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

/// Vibrato: 100 % wet, delay = 1 ms + depth·(0.5+0.5·sin φ).
pub struct VibratoNode {
    lfo: LfoDrive,
    depth_ms: SmoothedParam,
    lines: [DelayLine; CH],
}

/// Maximum vibrato delay: 1 ms base + 5 ms depth + margin.
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

/// Chorus: base 15 ms, voice k — phase +k·2π/voices, right channel +90°.
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

/// Flanger: base 0.5 ms, feedback into the ring (sign = phase inversion).
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

/// Phaser: a cascade of 1-pole allpass filters, center sweep 200..2000 Hz (exp).
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
            // center sweep: 200..2000 Hz exp, depth compresses the range
            let f = 200.0 * (10.0f32).powf(ph * d);
            let t = (std::f32::consts::PI * f / sr).tan();
            let ap = (t - 1.0) / (t + 1.0);
            for (ch, s) in fr.iter_mut().enumerate() {
                let dry = *s;
                let mut x = dry + self.fb_state[ch] * fb;
                for st in 0..n {
                    // 1-pole allpass: y = a·x + z; z = x − a·y
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
// Ka4 nodes (R.47, item 329): echo and dynamics
// ---------------------------------------------------------------------------

use super::primitives::EnvFollower;

const DELAY_MAX_MS: f32 = 1500.0;
const DELAY_MAX_FRAMES: usize = (48_000.0 * (DELAY_MAX_MS / 1000.0)) as usize + 8;

/// Echo: the ring is allocated to the maximum (a time change is a fractional read,
/// NOT structural); time is smoothed over 100 ms — tape glide without clicks;
/// the filter in the loop — one-pole LP (the tail darkens with each repeat,
/// a legacy of the F-AUD-009 feature); pingpong — cross feedback.
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
        // loop filter ~4 kHz: the repeats darken
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
                // ping-pong: the feedback goes to the OPPOSITE channel
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

/// Compressor: peak envelope, stereo link max(|l|,|r|), gr in dB.
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

    /// `key` — sidechain: the detector listens to IT, while compression still acts on its own
    /// input. The compression law, limits and smoothing are the same — the key changes
    /// only the envelope source (R.65). A key shorter than the buffer (the last
    /// chunk of the window) gives a peak of 0 for the remainder — that is quieter than the threshold, i.e.
    /// “the key is silent”, not a panic.
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

/// The length of the limiter’s lookahead window in frames (R.48, item 338). ONE home of the
/// formula: both the node (the ring geometry) and the static
/// `chain_lookahead` call it — they cannot diverge by construction.
pub(crate) fn limiter_la_frames(lookahead_ms: f32) -> i64 {
    (lookahead_ms.clamp(1.0, 20.0) as f64 / 1000.0 * 48_000.0).ceil() as i64
}

/// Look-ahead limiter (R.48, item 338). The ceiling is guaranteed ALGEBRAICALLY:
/// the sliding-maximum window covers the material [t, t+La], including the output
/// frame itself, so the reduction gets ahead of the peak and the attack is instant —
/// there is no distortion artifact. The output is aligned with the chain input: the ring
/// delays the signal by exactly La, which the processor subtracted from the position.
pub struct LimiterNode {
    threshold_db: SmoothedParam,
    ceiling_db: SmoothedParam,
    release_ms: f32,
    /// Window geometry: changes ONLY on structural — a divergence from the
    /// chain is caught by FxStateStore, which rebuilds the processor entirely.
    la: i64,
    /// Alignment ring (la frames × CH).
    ring: Vec<f32>,
    pos: usize,
    /// Monotone queue of the sliding maximum of |x|: values decrease, at the front
    /// the window maximum. O(1) per frame, amortized.
    win: std::collections::VecDeque<(i64, f32)>,
    /// How many frames have been fed into the window (the age of the elements).
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
            // the INCOMING frame goes into the window (position t + La)
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
            // the output — the frame delayed by La (position t)
            for v in fr.iter_mut() {
                std::mem::swap(&mut self.ring[self.pos], v);
                self.pos += 1;
            }
            if self.pos >= self.ring.len() {
                self.pos = 0;
            }
            // the maximum over [t, t+La] — including the output frame itself, therefore
            // out = wmax * g ≤ ceiling strictly
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

/// Pitch lookahead (R.48, item 339): a grain of a two-octave upshift reads the
/// input four times faster than the output, and by the end of the grain it needs material
/// `LG·4` frames ahead plus the WSOLA search window. The constant is ONE for both
/// engines: the vocoder needs less, the excess is simply history for it.
/// Less than `MIN_PREROLL_FRAMES` (4800) — the seek warm-up always fills
/// the ring with real material.
pub(crate) const PITCH_LOOKAHEAD_FRAMES: i64 = 4608;

/// The WSOLA synthesis grid step and the vocoder hop (absolute, from the clip zero —
/// therefore slicing the buffers does not affect the result).
const PITCH_HOP: i64 = 512;
/// WSOLA grain length (Hann window, 50 % overlap: the sum of windows ≡ 1).
const PITCH_GRAIN: usize = 1024;
/// Half-width of the WSOLA search and its step (candidates are enumerated in
/// ascending |off| — the tie-break is deterministic).
const PITCH_SEARCH: i64 = 256;
const PITCH_SEARCH_STEP: i64 = 8;
/// Frame size of the phase vocoder.
const PV_N: usize = 2048;
/// Rings: the input must cover [output − search, input + chunk].
const PITCH_RING: usize = 8192;
/// OLA accumulator: must hold the processor chunk (1024) PLUS the full
/// synthesis window (2048 for the vocoder) — otherwise a grain synthesized
/// ahead would overwrite frames not yet handed out, and the result would depend on the
/// slicing of the buffers (exactly what t338 caught on the 2048 ring).
const PITCH_ACC: usize = 4096;

/// Pitch neutral: both the base and the curves at zero. A neutral node does not touch
/// the buffer at all (byte-for-byte the previous sound) and requires no lookahead.
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

/// Phase vocoder state: FFT plans and phase accumulators.
struct PvState {
    fwd: std::sync::Arc<dyn realfft::RealToComplex<f32>>,
    inv: std::sync::Arc<dyn realfft::ComplexToReal<f32>>,
    /// Phases per CHANNEL: the stereo sides must not interfere.
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

/// Pitch change without a change of duration (R.48, item 339). A LOOKAHEAD node:
/// it reads the input with a lookahead of `PITCH_LOOKAHEAD_FRAMES` and gives its output
/// aligned — there is no delay for the engine (item 337).
///
/// Both engines synthesize on the ABSOLUTE grid of the clip position: grain
/// number g always covers the output frames [g·HOP, g·HOP + GRAIN), and the
/// pitch coefficient is taken by the GRAIN position — therefore the result depends
/// neither on the slicing of the buffers nor on the number of calls (class t325a).
pub struct PitchNode {
    engine: kt_model::PitchEngine,
    semitones: f32,
    cents: f32,
    st_points: Vec<(f64, f64)>,
    ct_points: Vec<(f64, f64)>,
    /// Segment shapes of the curves (R.61): a node reading the value BY POSITION
    /// must see the same row of shapes as the rest of the program.
    st_shapes: Vec<kt_model::SegShape>,
    ct_shapes: Vec<kt_model::SegShape>,
    clip_len: i64,
    /// 0 — neutral (the node is transparent).
    la: i64,
    /// Input ring by absolute clip positions.
    ring: Vec<f32>,
    /// Position of the next recording frame; `i64::MIN` — the stream has not yet existed.
    written: i64,
    /// The output OLA accumulator (also by absolute positions).
    acc: Vec<f32>,
    /// Number of the next grain to synthesize.
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

    /// Pitch curves (the same pattern as the LFO nodes): the value is taken
    /// by the GRAIN position, not by the smoothing ticks — otherwise the result would depend
    /// on the number of calls.
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

    /// Pitch coefficient at position `p` (clip-local frame).
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

    /// Linear interpolation of the input at a fractional position.
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

    /// Stream break (seam, seek): the rings and the grid are re-initialized from the
    /// new position — the node survives the seam like reverb, just without a tail.
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
            return; // neutral: do not touch the buffer at all
        }
        let n = (buf.len() / CH) as i64;
        if self.written != in_pos {
            self.restart(in_pos);
        }
        // 1) input into the ring
        for (i, fr) in buf.chunks_exact(CH).enumerate() {
            let idx = (in_pos + i as i64).rem_euclid(PITCH_RING as i64) as usize;
            for (ch, v) in fr.iter().enumerate() {
                self.ring[idx * CH + ch] = *v;
            }
        }
        self.written = in_pos + n;
        // 2) synthesis of the grains covering the needed output
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
        // 3) output from the accumulator (what was read is zeroed — the ring
        //    is reused by the next grains)
        for (i, fr) in buf.chunks_exact_mut(CH).enumerate() {
            let t = out_pos + i as i64;
            for (ch, v) in fr.iter_mut().enumerate() {
                let a = self.acc_at(t, ch);
                *v = *a;
                *a = 0.0;
            }
        }
    }

    /// WSOLA grain: reads the input at speed `r` from the grain position and
    /// mixes into the accumulator with a Hann window. The read offset is chosen
    /// by cross-correlation with the already synthesized overlap — this is what
    /// preserves the shape of the strike (unlike the vocoder).
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

    /// WSOLA search: candidates are enumerated in ascending |off|, the strictly
    /// greater score wins — so on a tie the smaller shift remains
    /// (determinism without dependence on the traversal order).
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

    /// Phase vocoder grain: the frame spectrum is carried into the bins `k·r` with a
    /// recomputation of the phase from the instantaneous frequency. Harmonics travel cleanly;
    /// the transient is smeared — the price of the spectral method.
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
            // analysis: the window from the ring
            for i in 0..PV_N {
                let v = self.ring_at(start + i as i64, ch);
                let pv = self.pv.as_deref_mut().expect("vocoder state");
                pv.time[i] = v * pv.window[i];
            }
            let pv = self.pv.as_deref_mut().expect("vocoder state");
            pv.fwd.process(&mut pv.time, &mut pv.spec).expect("fft");
            // Analysis: the magnitude and the INSTANTANEOUS frequency of each bin
            // (the principal deviation of the phase against the expected one per hop).
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
            // Synthesis by the INVERSE mapping (as in video geometry):
            // the target bin j asks the source at position j/r and gets the
            // single frequency — with a direct carry k → round(k·r) several sources would
            // share one bin, and the pitch drifted
            // (measured: 612 Hz instead of 659 at +7 semitones).
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
            // DC and Nyquist of a real signal are real — realfft checks this and otherwise
            // refuses to compute
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

/// Hann window with 50 % overlap: the sum of two adjacent windows ≡ 1.
#[inline]
fn hann(i: usize, n: usize) -> f32 {
    let x = std::f32::consts::TAU * i as f32 / n as f32;
    0.5 - 0.5 * x.cos()
}

/// Gate: a fast detector (~0.5 ms) + hysteresis −6 dB + hold counter
/// (frames — survives the seam) + attack/release ramps.
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
            // hysteresis: closing at 6 dB below opening
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
