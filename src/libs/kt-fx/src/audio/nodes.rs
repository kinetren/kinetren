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
    Filter(Box<FilterNode>),
    Delay(Box<DelayNode>),
    Compressor(CompNode),
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
            FxInstance::Filter { .. } => Some(AudioNode::Filter(Box::new(FilterNode::new()))),
            FxInstance::Delay { .. } => Some(AudioNode::Delay(Box::new(DelayNode::new()))),
            FxInstance::Compressor { .. } => Some(AudioNode::Compressor(CompNode::new())),
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
            (AudioNode::Filter(n), FxInstance::Filter { mode, cutoff_hz, resonance }) => {
                n.apply(*mode, *cutoff_hz, *resonance)
            }
            (AudioNode::Delay(n), FxInstance::Delay { time_ms, feedback, mix, pingpong }) => {
                n.apply(*time_ms, *feedback, *mix, *pingpong)
            }
            (
                AudioNode::Compressor(n),
                FxInstance::Compressor { threshold_db, ratio, attack_ms, release_ms, makeup_db, .. },
            ) => n.apply(*threshold_db, *ratio, *attack_ms, *release_ms, *makeup_db),
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
            AudioNode::Filter(n) => n.set_targets(auto, slot, s),
            AudioNode::Delay(n) => n.set_targets(auto, slot, s),
            AudioNode::Compressor(n) => n.set_targets(auto, slot, s),
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
            AudioNode::Filter(n) => n.process(buf),
            AudioNode::Delay(n) => n.process(buf),
            AudioNode::Compressor(n) => n.process(buf, key),
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
            AudioNode::Filter(n) => n.tail(),
            AudioNode::Delay(n) => n.tail(true),
            AudioNode::Plugin(n) => n.tail(),
            _ => kt_time::Tick::ZERO,
        }
    }

    pub(crate) fn reset(&mut self) {
        match self {
            AudioNode::LayerMix => {}
            AudioNode::Reverb(n) => n.rv.reset(),
            AudioNode::Filter(n) => n.reset(),
            AudioNode::Delay(n) => n.reset(),
            AudioNode::Compressor(n) => n.reset(),
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

// ---------------------------------------------------------------------------
// Ka2 nodes (R.47, item 327): the filter (EQ, distortion, bitcrusher and width are plugins —
// src/plugins/fx-audio-tone; coefficients are re-tuned every `TUNE_STEP` frames)
// ---------------------------------------------------------------------------

use super::primitives::{Biquad, BiquadKind, TUNE_STEP};
use kt_model::FilterMode;

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

// eq, distortion, bitcrusher, stereo_width moved into src/plugins/fx-audio-tone (plugin-api-stage1, stage 3)

// tremolo, auto_pan, vibrato, chorus, flanger, phaser moved into src/plugins/fx-audio-modulation
// (plugin-api-stage1, stage 3)

use super::primitives::DelayLine;

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

// pitch_shift moved into src/plugins/fx-audio-modulation (plugin-api-stage1, stage 3)

// gain, gate, limiter moved into src/plugins/fx-audio-dynamics (plugin-api-stage1, stage 3)
