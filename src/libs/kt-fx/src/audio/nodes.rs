//! Audio nodes of the chain (R.47): Freeverb (a verbatim move from lib.rs) and —
//! from the next step of Ka1 — enum AudioNode with all the effects.

// Freeverb and its tail formula live in `kt_plugin_api::dsp` since plugin-api-stage1 (3b.2): the
// Slowed+Reverb plugin and the core share one text.
pub use kt_plugin_api::dsp::Freeverb;

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
#[derive(Clone)]
struct OwnedCurve {
    points: Vec<(f64, f64)>,
    shapes: Vec<kt_model::SegShape>,
}

impl kt_plugin_api::CurveRead for OwnedCurve {
    fn value(&self, base: f64, s: f64) -> f64 {
        kt_model::param_at(base, &self.points, s, &self.shapes)
    }
    fn clone_box(&self) -> Box<dyn kt_plugin_api::CurveRead> {
        Box::new(self.clone())
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
    let p = slot.plugin()?;
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
            // the chain fingerprint matched — the kinds by slots match by
            // construction; one can get here only through a matching error
            _ => debug_assert!(false, "node/slot kind mismatch"),
        }
    }

    /// Where the clip sits on the timeline (sim-audio-pairs).
    pub(crate) fn place(&mut self, at: kt_plugin_api::Placement) {
        if let AudioNode::Plugin(n) = self {
            n.place(at);
        }
    }

    pub(crate) fn set_targets(&mut self, auto: &AutoProfiles, slot: usize, s: f64) {
        match self {
            AudioNode::Plugin(n) => n.set_targets(&SlotView { auto, slot }, s),
            AudioNode::LayerMix => {}
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
            AudioNode::Plugin(n) => n.tail(),
            _ => kt_time::Tick::ZERO,
        }
    }

    pub(crate) fn reset(&mut self) {
        match self {
            AudioNode::LayerMix => {}
            AudioNode::Plugin(n) => n.reset(),
        }
    }

    /// A copy that continues exactly as this node (pitch-tail-choice D4); `None` — its state
    /// cannot be copied.
    pub(crate) fn fork(&self) -> Option<AudioNode> {
        match self {
            AudioNode::LayerMix => Some(AudioNode::LayerMix),
            AudioNode::Plugin(n) => n.fork().map(AudioNode::Plugin),
        }
    }
}

/// The echo row of an edge-reading slot is on (pitch-tail-choice D1): its kind declares
/// `Decl.echo_row` and the slot's value there is non-zero.
pub(crate) fn echo_rings(fx: &FxInstance) -> bool {
    match (fx, plugin_audio(fx)) {
        (FxInstance::Plugin(p), Some((m, _))) => {
            m.decl.reads_across_edges && m.decl.echo_row.is_some_and(|row| p.get(row).is_some_and(|v| v != 0.0))
        }
        _ => false,
    }
}

// the reverb half of Slowed+Reverb moved into src/plugins/fx-audio-modulation (3b.2, D11.5)

// filter, eq, distortion, bitcrusher, stereo_width moved into src/plugins/fx-audio-tone; tremolo,
// auto_pan, vibrato, chorus, flanger, phaser, pitch_shift, delay into src/plugins/fx-audio-modulation
// (plugin-api-stage1, stage 3)

// compressor moved into src/plugins/fx-audio-dynamics (plugin-api-stage1, stage 3b, D11.4)

// pitch_shift moved into src/plugins/fx-audio-modulation (plugin-api-stage1, stage 3)

// gain, gate, limiter moved into src/plugins/fx-audio-dynamics (plugin-api-stage1, stage 3)
