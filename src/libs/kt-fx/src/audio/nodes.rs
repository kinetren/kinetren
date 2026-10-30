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
    /// Layer-strength marker: the dry-side snapshot and the blend are executed by
    /// `produce()` — the node needs an inter-node buffer, which it does not own.
    LayerMix,
    Reverb(ReverbNode),
    /// An audio plugin's state (plugin-api-stage1, D2a): one dynamic call per chunk.
    Plugin(Box<dyn kt_plugin_api::AudioState>),
}

/// The curves of one slot as a plugin state sees them (D2a): kt-model's profiles, shapes included.
pub(crate) struct SlotView<'a> {
    pub auto: &'a AutoProfiles,
    pub slot: usize,
}

impl kt_plugin_api::SlotCurves for SlotView<'_> {
    fn value(&self, param: &str, base: f64, s: f64) -> f64 {
        self.auto.value(self.slot, param, base, s)
    }
    fn points(&self, param: &str) -> &kt_model::ParamPoints {
        self.auto.points(self.slot, param)
    }
    fn owned(&self, param: &str) -> Box<dyn kt_plugin_api::CurveRead> {
        Box::new(OwnedCurve {
            points: self.auto.points(self.slot, param).to_vec(),
            shapes: self.auto.shapes_of(self.slot, param).to_vec(),
        })
    }
}

/// A copy of one curve a plugin state keeps for reads by position.
struct OwnedCurve {
    points: Vec<(f64, f64)>,
    shapes: Vec<kt_model::SegShape>,
}

impl kt_plugin_api::CurveRead for OwnedCurve {
    fn value(&self, base: f64, s: f64) -> f64 {
        kt_model::param_at(base, &self.points, s, &self.shapes)
    }
}

/// The project settings a plugin kernel reads (D2a): today the pitch engine.
pub(crate) fn host_opts(engine: kt_model::PitchEngine) -> kt_plugin_api::HostOpts {
    let e = match engine {
        kt_model::PitchEngine::Wsola => 0.0,
        kt_model::PitchEngine::Vocoder => 1.0,
    };
    kt_plugin_api::HostOpts { opts: vec![("pitch_engine", e)] }
}

/// The installed audio kernel of a plugin slot, if any.
pub(crate) fn plugin_audio(fx: &FxInstance) -> Option<(&'static kt_plugin_api::Manifest, &'static dyn kt_plugin_api::AudioKernel)> {
    let FxInstance::Plugin(slot) = fx else { return None };
    let p = kt_model::plugin_slot::registry().get(&slot.id)?;
    Some((p.manifest(), p.audio()?))
}

impl AudioNode {
    /// Build the node for a slot. Non-audio node kinds do not get one (the chain
    /// does not see them — they are transparent).
    pub(crate) fn build(fx: &FxInstance, engine: kt_model::PitchEngine) -> Option<AudioNode> {
        if let Some((_, k)) = plugin_audio(fx) {
            return Some(AudioNode::Plugin(k.build(&host_opts(engine))));
        }
        match fx {
            FxInstance::LayerMix { .. } => Some(AudioNode::LayerMix),
            FxInstance::SlowedReverb { .. } => Some(AudioNode::Reverb(ReverbNode::new())),
            _ => None,
        }
    }

    /// Whether `build` gives the slot a node (without building one).
    pub(crate) fn has_node(fx: &FxInstance) -> bool {
        match fx {
            FxInstance::Plugin(_) => plugin_audio(fx).is_some(),
            _ => AudioNode::build(fx, kt_model::PitchEngine::Wsola).is_some(),
        }
    }

    pub(crate) fn apply(&mut self, fx: &FxInstance, structural: bool) {
        match (self, fx) {
            (AudioNode::Plugin(n), FxInstance::Plugin(p)) => n.apply(&p.values, structural),
            (AudioNode::LayerMix, FxInstance::LayerMix { .. }) => {}
            (AudioNode::Reverb(n), FxInstance::SlowedReverb { .. }) => n.apply(fx, structural),
            // the chain fingerprint matched — the kinds by slots match by
            // construction; one can get here only through a matching error
            _ => debug_assert!(false, "node/slot kind mismatch"),
        }
    }

    pub(crate) fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        match self {
            AudioNode::Plugin(n) => n.set_targets(&SlotView { auto, slot }, s),
            AudioNode::LayerMix => {}
            AudioNode::Reverb(n) => n.set_targets(auto, slot, s),
        }
    }

    /// Bind a node to its slot's curves and the clip length (a plugin state copies what its
    /// `process` reads — the LFO's rate curve, pitch's readers: the model is not read in
    /// process). Called from apply_params/set_auto/set_clip_len of the processor.
    pub(crate) fn bind_lfo(&mut self, auto: &AutoProfiles, slot: usize, clip_len: i64) {
        if let AudioNode::Plugin(n) = self {
            n.bind(&SlotView { auto, slot }, clip_len);
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
            AudioNode::LayerMix => {}
            AudioNode::Reverb(n) => n.rv.process(buf),
            AudioNode::Plugin(n) => n.process(buf, _pos, key),
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
    pub(crate) fn lookahead(&self) -> i64 {
        match self {
            AudioNode::Plugin(n) => n.lookahead(),
            _ => 0,
        }
    }

    pub(crate) fn tail(&self) -> kt_time::Tick {
        match self {
            AudioNode::Reverb(n) => n.rv.tail(),
            AudioNode::Plugin(n) => n.tail(),
            _ => kt_time::Tick::ZERO,
        }
    }

    pub(crate) fn reset(&mut self) {
        match self {
            AudioNode::LayerMix => {}
            AudioNode::Reverb(n) => n.rv.reset(),
            AudioNode::Plugin(n) => n.reset(),
        }
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

// filter, eq, distortion, bitcrusher, stereo_width moved into src/plugins/fx-audio-tone; tremolo,
// auto_pan, vibrato, chorus, flanger, phaser, pitch_shift, delay into src/plugins/fx-audio-modulation
// (plugin-api-stage1, stage 3)

// compressor moved into src/plugins/fx-audio-dynamics (plugin-api-stage1, stage 3b, D11.4)

// pitch_shift moved into src/plugins/fx-audio-modulation (plugin-api-stage1, stage 3)

// gain, gate, limiter moved into src/plugins/fx-audio-dynamics (plugin-api-stage1, stage 3)
