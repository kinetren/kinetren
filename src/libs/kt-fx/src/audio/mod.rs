//! Audio processor of a clip: an FX chain over the (possibly warped)
//! source. The public surface of the crate is re-exported from lib.rs —
//! the paths of callers (`kt_fx::ClipProcessor` etc.) have not changed.

pub mod loudness;
mod nodes;
pub mod primitives;
mod warp;

pub use nodes::{AudioNode, Freeverb, eq_mag_db};
pub use primitives::SmoothedParam;
pub use warp::Warp;

use crate::{CH, MIN_PREROLL_FRAMES, PullKind};
use kt_model::FxInstance;
use kt_time::Tick;
use nodes::reverb_tail_for_delay;

use kt_model::{AutoProfiles, ParamPoints, eff_rate, rate_at, source_progress};

/// Length of the chain tail WITHOUT creating state (to extend the clip window
/// in the graph before the processor is built).
/// The source margin for rubato priming (sinc_len 128 × oversampling) and
/// chunk quantization of the warp: the real source cursor may run ahead of
/// rate_integral by the resampler’s group delay (~3k frames).
pub const WARP_SOURCE_MARGIN_FRAMES: i64 = 8192;

/// The source window (frames RELATIVE to source_in) that ClipProcessor will
/// actually consume, producing clip-local frames `local`, including the pre-roll
/// before local.start and the warp priming. For a chain without a warp — linear and
/// without margin. A SINGLE source of truth for prefetch (wanted) and render
/// (window_end): a divergence of these two calculations = a permanent cache miss and
/// silence (regression of item 13, test 17).
pub fn chain_source_window(
    fx: &[FxInstance],
    auto: &AutoProfiles,
    clip_len_frames: i64,
    local: std::ops::Range<i64>,
) -> std::ops::Range<i64> {
    chain_source_window_ext(fx, auto, clip_len_frames, 0..clip_len_frames.max(1), local)
}

/// The same, but for a clip with transition “handles” (item 138): material — the permitted
/// clip-local range [−head_ext, clip_len + tail_ext); the normalization of
/// automation stays along clip_len. For material = 0..len the behavior is
/// byte-identical to chain_source_window (test 17 does not move).
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
    // FORWARD — for the chain’s lookahead (R.48, item 337): exactly how much of the source
    // the chain actually reads while giving the same output. The fix lives in
    // ONE function, so render and prefetch expand the same way
    // (if they diverge — a permanent cache miss, item 13/t17).
    let l1 = (local.end + chain_lookahead(fx, auto)).clamp(material.start, material.end);
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
    // the source window does not go below zero only when the material starts
    // with the body of the clip (old behavior); the left handle is legally negative
    let s_floor = if material.start >= 0 { 0 } else { i64::MIN };
    let t0 = l0 as f64 / len as f64;
    let t1 = l1 as f64 / len as f64;
    // The SIGNED curve (R.57) goes beyond its own ends: a window over two
    // points would miss the excursion, and prefetch would warm the wrong thing (class
    // item 13/t17). For a monotone `span_over` returns EXACTLY the ends, so the
    // branch below is byte-for-byte the same as before.
    let (p0, p1) = if kt_model::rate_signed(rate, automation) {
        kt_model::span_over(rate, automation, t0, t1)
    } else {
        (source_progress(rate, automation, t0), source_progress(rate, automation, t1))
    };
    let s0 =
        ((len as f64 * p0).floor() as i64).saturating_sub(WARP_SOURCE_MARGIN_FRAMES).max(s_floor);
    let s1 = (len as f64 * p1).ceil() as i64 + WARP_SOURCE_MARGIN_FRAMES;
    s0..s1.max(s0)
}

/// The CONSUMPTION window: how much material the chain will eat on this piece
/// of output (R.57). Monotone by construction — it bounds the resampler cursor
/// (`SrcWindow.end`), whereas the ADDRESS window above sets what to warm in the
/// cache.
pub fn chain_consumption_window_ext(
    fx: &[FxInstance],
    auto: &AutoProfiles,
    clip_len_frames: i64,
    material: std::ops::Range<i64>,
    local: std::ops::Range<i64>,
) -> std::ops::Range<i64> {
    let len = clip_len_frames.max(1);
    let l0 = (local.start - chain_lookahead(fx, auto)).clamp(material.start, material.end);
    let l1 = (local.end + chain_lookahead(fx, auto)).clamp(material.start, material.end);
    let Some(rate) = fx.iter().find_map(|f| match f {
        FxInstance::SlowedReverb { rate, .. } => Some(*rate),
        _ => None,
    }) else {
        return l0..l1.max(l0);
    };
    let automation = auto.rate_points(fx);
    let s_floor = if material.start >= 0 { 0 } else { i64::MIN };
    let a0 = ((len as f64 * kt_model::consumed_abs(rate, automation, l0 as f64 / len as f64))
        .floor() as i64)
        .saturating_sub(WARP_SOURCE_MARGIN_FRAMES)
        .max(s_floor);
    let a1 = (len as f64 * kt_model::consumed_abs(rate, automation, l1 as f64 / len as f64)).ceil()
        as i64
        + WARP_SOURCE_MARGIN_FRAMES;
    a0..a1.max(a0)
}

/// How many source frames the event consumes ENTIRELY: S(1)·len, without
/// the resampler margin and without pre-roll.
///
/// This is the **MIRROR AXIS** of a reversed clip (R.43, item 292) — exactly the same
/// quantity as `ClipWarp::used` for video. Before R.43, audio mirrored from
/// `chain_source_window_ext(..).end`, and it includes
/// `WARP_SOURCE_MARGIN_FRAMES` = 8192 frames (170.7 ms): the mirrored
/// slowed clip played material shifted by these 170 ms, and sounded
/// like “sound from another event” (R.43 item 3 complaint), while the picture on the same
/// event stayed in place — A/V diverged.
pub fn chain_source_used(fx: &[FxInstance], auto: &AutoProfiles, clip_len_frames: i64) -> i64 {
    let len = clip_len_frames.max(1);
    let Some(rate) = fx.iter().find_map(|f| match f {
        FxInstance::SlowedReverb { rate, .. } => Some(*rate),
        _ => None,
    }) else {
        return len;
    };
    // MIRROR AXIS — the maximum of the position’s SPAN (R.57): for a signed curve
    // “how much is eaten” is larger than what was traversed, and the mirror would have drifted away from it.
    // For a monotone one it is exactly S(1) — the previous number.
    (len as f64 * kt_model::span_over(rate, auto.rate_points(fx), 0.0, 1.0).1).ceil() as i64
}

/// Lookahead of the chain in frames (R.48, item 337): by how much the chain’s INPUT
/// runs ahead of its OUTPUT. An effect with an analysis window (limiter, pitch) reads
/// the source FORWARD and gives its output aligned — for the rest of the engine the
/// delay is zero (playhead, fades and A/V sync do not move).
///
/// The value enters the warm-up of `hard_seek` and the source window
/// (`chain_source_window_ext` — one function feeds both render and prefetch),
/// but does NOT enter `chain_tail`: “how much to warm” and “how much to sound”
/// are kept apart (otherwise the clip would sound after its own end).
///
/// A formula of this kind must match `AudioNode::lookahead` of the same kind —
/// the agreement is guarded by t322g.
pub fn chain_lookahead(fx: &[FxInstance], auto: &AutoProfiles) -> i64 {
    fx.iter()
        .enumerate()
        .map(|(slot, f)| match f {
            FxInstance::Limiter { lookahead_ms, .. } => nodes::limiter_la_frames(*lookahead_ms),
            // for pitch, the neutral depends on the CURVES: a “zero” base is not
            // neutral while a live curve is present
            FxInstance::PitchShift { semitones, cents } => {
                let neutral = nodes::pitch_is_neutral(
                    *semitones,
                    *cents,
                    auto.points(slot, "semitones"),
                    auto.points(slot, "cents"),
                );
                if neutral { 0 } else { nodes::PITCH_LOOKAHEAD_FRAMES }
            }
            _ => 0i64,
        })
        .sum()
}

/// PROJECT settings that affect the execution of the chain (R.48): the pitch
/// engine is one for the whole edit. Not a parameter value but the device of the node —
/// a change rebuilds the processor (checked in `FxStateStore`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChainOpts {
    pub pitch_engine: kt_model::PitchEngine,
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
            // ringing of a resonant cut: t60 = 6.9*Q/(pi*f0) — the same
            // formula as FilterNode::tail (guarded by agreement t322f)
            // modulations with memory: the maximum of their delays (the same constants
            // as in the nodes — agreement is guarded by t322f)
            // echo: t60 of the decaying repeats (gate by mix — including the curve,
            // as for reverb); the formula is the same as in the node (t322f)
            FxInstance::Delay { time_ms, feedback, mix, .. }
                if *mix > 0.0 || wet.iter().any(|(_, v)| *v > 0.0) =>
            {
                crate::audio::primitives::tail_frames_for(
                    time_ms.clamp(1.0, 1500.0) / 1000.0 * 48_000.0,
                    feedback.clamp(0.01, 0.9),
                )
            }
            FxInstance::Vibrato { .. } => Tick::from_audio_frames((48_000.0 * 0.008) as i64),
            FxInstance::Chorus { .. } => Tick::from_audio_frames((48_000.0 * 0.030) as i64),
            FxInstance::Flanger { .. } => {
                crate::audio::primitives::tail_frames_for(48_000.0 * 0.010, 0.9)
            }
            FxInstance::Filter { cutoff_hz, resonance, .. } => {
                let secs = 6.9 * resonance.clamp(0.5, 10.0)
                    / (std::f32::consts::PI * cutoff_hz.clamp(20.0, 20_000.0));
                Tick::from_audio_frames((secs * 48_000.0) as i64)
            }
            _ => Tick::ZERO,
        })
        .max()
        .unwrap_or(Tick::ZERO)
}

/// Fingerprint of the chain’s STRUCTURE (FX kinds in order). Matches → state
/// is reused, parameters arrive via set_params (smoothed).
pub fn chain_fingerprint(fx: &[FxInstance]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for f in fx {
        std::mem::discriminant(f).hash(&mut h);
    }
    h.finish()
}

/// Config of the layer strength (R.38 item 12) in the node chain.
struct MixCfg {
    /// Slot of the LayerMix marker node (the amount curve goes by it).
    slot: usize,
    /// Slot of the start of a layer’s contribution: the dry-side snapshot is taken before the
    /// first node with a slot >= from.
    from: usize,
    base_amount: f64,
}

/// Stateful processor of the audio clip. Owner — the audio-render thread (kt-engine,
/// TRACK CHAIN (R.62, change B) — the same nodes as the event, but WITHOUT the
/// clip’s shape: no source, no length, no warp, no curves.
///
/// Why a separate type rather than a `ClipProcessor` with an empty source:
/// half of `ClipProcessor` is the material EXTRACTOR (warp, cursor,
/// “handles”, tail gate, the automation domain by event length), and the track
/// has none of this — its input is already assembled by the summer. What they share is
/// exactly one thing: the VECTOR OF NODES, and here it is the same one (`AudioNode`), so
/// that the cores are not duplicated by a single line.
///
/// The track has no curves by the decision of R.62 (its key is the timeline tick, which is
/// the second automation domain), so `set_targets` is not called at all:
/// the parameters are static and arrive via smoothing inside the nodes.
pub struct TrackProcessor {
    fingerprint: u64,
    nodes: Vec<(usize, AudioNode)>,
    opts: ChainOpts,
    /// Lookahead of the chain: the input runs ahead of the output by this many frames (R.48).
    la: i64,
}

impl TrackProcessor {
    pub fn new(fx: &[FxInstance], opts: ChainOpts) -> TrackProcessor {
        let mut p =
            TrackProcessor { fingerprint: chain_fingerprint(fx), nodes: Vec::new(), opts, la: 0 };
        p.apply_params(fx, true);
        p
    }

    /// Fingerprint of the chain’s STRUCTURE (kinds of slots) — the same as the event’s:
    /// if it matches, the processor is reused, and the values arrive
    /// smoothed (invariant 3).
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    pub fn lookahead_frames(&self) -> i64 {
        self.la
    }

    /// BY HOW MUCH the signal reaching slot `slot` is delayed: the sum of the lookahead
    /// windows of the nodes BEFORE it. A node with an analysis window (limiter, pitch)
    /// gives its output via its ring, so everything behind it sees
    /// samples older than the chain input by exactly that many frames.
    ///
    /// Needed by sidechain (R.65): the key must arrive at the position of those
    /// samples that the node is processing NOW, otherwise ducking
    /// fires before the speech (caught by t770 on the chain “limiter +
    /// compressor with a key”).
    pub fn slot_delay(&self, slot: usize) -> i64 {
        self.nodes.iter().take_while(|(s, _)| *s < slot).map(|(_, n)| n.lookahead()).sum()
    }

    pub fn tail(&self) -> Tick {
        self.nodes.iter().map(|(_, n)| n.tail()).max().unwrap_or(Tick::ZERO)
    }

    pub fn tail_frames(&self) -> i64 {
        self.tail().to_audio_frames()
    }

    pub fn apply_params(&mut self, fx: &[FxInstance], structural: bool) {
        if structural {
            self.nodes = fx
                .iter()
                .enumerate()
                .filter_map(|(slot, f)| AudioNode::build(f).map(|n| (slot, n)))
                .collect();
            self.fingerprint = chain_fingerprint(fx);
        }
        for (slot, node) in &mut self.nodes {
            if structural {
                node.set_engine(self.opts.pitch_engine);
            }
            node.apply(&fx[*slot], structural);
        }
        // the track has neither a layer strength nor a dry side: there is nothing to mix with
        self.la = self.nodes.iter().map(|(_, n)| n.lookahead()).sum();
    }

    pub fn reset(&mut self) {
        for (_, n) in &mut self.nodes {
            n.reset();
        }
    }

    /// Process the buffer standing at position `pos` (TIMELINE frames).
    ///
    /// The position is needed by nodes with an absolute grid (grain, OLA hop): computing it
    /// from the call counter is impossible — slicing the buffers would change the sound
    /// (R.48, t338). Lookahead nodes “eat” their window with their own rings, so the output
    /// is aligned with the input: the caller must feed an input that runs ahead of the output
    /// by exactly `lookahead_frames()`.
    pub fn process(&mut self, pos: i64, buf: &mut [f32]) {
        self.process_keyed(pos, buf, &|_| None);
    }

    /// Processing with KEY signals (sidechain, R.65): `key` is asked for
    /// BY SLOT — a chain may hold two compressors with different keys,
    /// and there is no shared “chain key”. Who obtained the key and
    /// over which window is the caller’s business (the engine): kt-fx knows nothing about
    /// tracks and must not.
    pub fn process_keyed<'k>(
        &mut self,
        pos: i64,
        buf: &mut [f32],
        key: &dyn Fn(usize) -> Option<&'k [f32]>,
    ) {
        for (slot, node) in &mut self.nodes {
            node.process(buf, pos, key(*slot));
        }
    }
}

/// FxStateStore). Read strictly sequentially; a gap → reset + pre-roll.
pub struct ClipProcessor {
    fingerprint: u64,
    /// Clip-local frame that will come out next (continuity marker).
    next_local: i64,
    src_in: i64,
    warp: Option<Warp>,
    /// Chain nodes IN SLOT ORDER (R.47, item 325): processing goes in
    /// order, as in video and in DAWs. The slot is stored alongside — by it the node
    /// takes its curves from AutoProfiles.
    nodes: Vec<(usize, AudioNode)>,
    /// Layer strength: the marker node stands IN the chain, and the snapshot of the dry side
    /// is taken before the FIRST node with a slot >= from (an honest positional
    /// semantics — the former reverb_is_layer flag became its consequence).
    mix: Option<MixCfg>,
    /// SIGNED speed (R.57): minus — motion backward through the material.
    /// Only the MODULE goes into the resampler; the sign lives in the read address.
    rate: f64,
    /// (min, max) of the speed module over the whole profile: the resampler needs the
    /// spread of ratios BEFORE the first chunk, otherwise the change would be rejected.
    reach: (f64, f64),
    /// Clip curves in the SOURCE domain. Owned by the audio-render thread
    /// (invariant 2): no locks, no model access per frame.
    auto: AutoProfiles,
    /// Slot index with a warp — by it the rate/mix profiles are taken.
    warp_slot: usize,
    /// Clip length in frames — the denominator of the automation progress.
    clip_len: i64,
    /// Transition “handles” (item 138): the material is clip-locally extended to
    /// [−head_ext, clip_len + tail_ext). The automation normalization is NOT
    /// moved — the denominator stays clip_len.
    head_ext: i64,
    tail_ext: i64,
    /// Source cursor for the path without a warp (rate == 1, no automation).
    plain_cursor: i64,
    /// Project settings affecting the node implementation (the pitch engine).
    opts: ChainOpts,
    /// Chain lookahead (R.48, item 337): the input runs ahead of the output by this many
    /// frames. 0 — the path is arithmetically identical to the pre-PDC version.
    la: i64,
    /// The lookahead of the content seen by node i: the sum of the lookaheads of
    /// nodes NOT EARLIER than it (each lookahead node “eats” its window with its own ring,
    /// and by the end of the chain 0 remains).
    ahead: Vec<i64>,
    /// Lookahead at the position of the LayerMix marker node (for the amount curve).
    mix_ahead: i64,
    /// Dry-side alignment ring of LayerMix: how many frames were
    /// “eaten” by lookahead nodes between the snapshot and the mix.
    dry_ring: Option<DryRing>,
}

/// Delay of the dry side by a whole number of frames (R.48): between the snapshot of the dry
/// side and the LayerMix marker node a lookahead node may stand,
/// having “eaten” part of the lookahead — without alignment the blend would mix different
/// moments of the material. With a zero delta the ring is not created at all.
struct DryRing {
    buf: Vec<f32>,
    pos: usize,
}

impl DryRing {
    fn new(frames: i64) -> DryRing {
        DryRing { buf: vec![0.0; frames.max(1) as usize * CH], pos: 0 }
    }

    /// Pass the buffer through the delay IN PLACE (input → ring, output from the
    /// ring → buffer).
    fn run(&mut self, buf: &mut [f32]) {
        let len = self.buf.len();
        for v in buf.iter_mut() {
            std::mem::swap(&mut self.buf[self.pos], v);
            self.pos = (self.pos + 1) % len;
        }
    }

    fn reset(&mut self) {
        self.buf.fill(0.0);
        self.pos = 0;
    }
}

impl ClipProcessor {
    pub fn new(
        fx: &[FxInstance],
        auto: &AutoProfiles,
        src_in_frames: i64,
        clip_len_frames: i64,
    ) -> ClipProcessor {
        Self::new_with_opts(fx, auto, src_in_frames, clip_len_frames, ChainOpts::default())
    }

    /// The same, but with the project settings (the pitch engine). The product calls
    /// exactly this one; the short `new` remains for tests and offline paths.
    pub fn new_with_opts(
        fx: &[FxInstance],
        auto: &AutoProfiles,
        src_in_frames: i64,
        clip_len_frames: i64,
        opts: ChainOpts,
    ) -> ClipProcessor {
        let mut p = ClipProcessor {
            fingerprint: chain_fingerprint(fx),
            next_local: i64::MIN,
            src_in: src_in_frames,
            warp: None,
            nodes: Vec::new(),
            mix: None,
            rate: 1.0,
            reach: (1.0, 1.0),
            auto: auto.clone(),
            warp_slot: fx
                .iter()
                .position(|f| matches!(f, FxInstance::SlowedReverb { .. }))
                .unwrap_or(0),
            clip_len: clip_len_frames.max(1),
            head_ext: 0,
            tail_ext: 0,
            plain_cursor: src_in_frames,
            opts,
            la: 0,
            ahead: Vec::new(),
            mix_ahead: 0,
            dry_ring: None,
        };
        p.apply_params(fx, true);
        p
    }

    /// Speed profile of this clip (empty — no warp/curve).
    fn rate_points(&self) -> &ParamPoints {
        self.auto.points(self.warp_slot, "rate")
    }

    /// Update the curves WITHOUT resetting the state: editing a keyframe must not
    /// silence the reverb tail (invariant 3).
    pub fn set_auto(&mut self, auto: &AutoProfiles) {
        if &self.auto != auto {
            self.auto = auto.clone();
            for (slot, node) in &mut self.nodes {
                if let AudioNode::Gain(g) = node {
                    g.bind_curves(&self.auto, *slot);
                }
                node.bind_lfo(&self.auto, *slot, self.clip_len);
            }
            // the neutral of a lookahead node may depend on the curves
            self.recompute_lookahead();
        }
    }

    /// Recompute the chain lookahead and the node lookaheads (R.48, item 337).
    /// `ahead[i] = Σ lookahead of nodes standing not earlier than i` — the node sees
    /// content running ahead of the output by exactly that much; by the end of the chain the
    /// sum goes to zero, so the output is aligned with the input.
    fn recompute_lookahead(&mut self) {
        let n = self.nodes.len();
        self.ahead.clear();
        self.ahead.resize(n, 0);
        let mut acc = 0i64;
        for i in (0..n).rev() {
            acc += self.nodes[i].1.lookahead();
            self.ahead[i] = acc;
        }
        self.la = acc;
        // lookahead at the layer-strength marker position and the dry-side delta
        let (mix_ahead, dry_delta) = match &self.mix {
            Some(m) => {
                let at = |slot: usize| {
                    self.nodes
                        .iter()
                        .position(|(s, _)| *s >= slot)
                        .map(|i| self.ahead[i])
                        .unwrap_or(0)
                };
                let mix_at = self
                    .nodes
                    .iter()
                    .position(|(s, _)| *s == m.slot)
                    .map(|i| self.ahead[i])
                    .unwrap_or(0);
                (mix_at, (at(m.from) - mix_at).max(0))
            }
            None => (0, 0),
        };
        self.mix_ahead = mix_ahead;
        self.dry_ring = (dry_delta > 0).then(|| DryRing::new(dry_delta));
    }

    /// Chain lookahead in frames — the same value as given by the
    /// static `chain_lookahead` (guarded by t322g). By it
    /// `FxStateStore` understands that the processor needs rebuilding.
    pub fn lookahead_frames(&self) -> i64 {
        self.la
    }

    /// Project settings the processor was built with: if they diverged from the
    /// model — the processor is rebuilt (`FxStateStore`).
    pub fn opts(&self) -> ChainOpts {
        self.opts
    }

    /// Update the clip length (trim) without resetting the state.
    pub fn set_clip_len(&mut self, clip_len_frames: i64) {
        self.clip_len = clip_len_frames.max(1);
        for (slot, node) in &mut self.nodes {
            node.bind_lfo(&self.auto, *slot, self.clip_len);
        }
    }

    /// Transition “handles” (item 138): allow reading [−head_ext,
    /// clip_len + tail_ext). Without a state reset (the chain structure is the same);
    /// the dry gate shifts by tail_ext, the automation normalization does not.
    pub fn set_material_ext(&mut self, head_ext: i64, tail_ext: i64) {
        self.head_ext = head_ext.max(0);
        self.tail_ext = tail_ext.max(0);
    }

    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Deliver parameters. `structural` = the first time (object creation).
    /// The chain fingerprint matched ⇒ the sequence of kinds by slots is the same
    /// ⇒ nodes are matched to slots 1:1 (invariant 3: the state
    /// lives on, parameters arrive smoothed).
    pub fn apply_params(&mut self, fx: &[FxInstance], structural: bool) {
        if structural {
            self.nodes = fx
                .iter()
                .enumerate()
                .filter_map(|(slot, f)| AudioNode::build(f).map(|n| (slot, n)))
                .collect();
        }
        debug_assert_eq!(
            self.nodes.len(),
            fx.iter().filter(|f| AudioNode::build(f).is_some()).count(),
            "node list out of sync with the chain"
        );
        self.mix = fx.iter().position(|f| f.is_layer_mix()).map(|i| match &fx[i] {
            FxInstance::LayerMix { amount, from } => {
                MixCfg { slot: i, from: (*from as usize).min(i), base_amount: *amount }
            }
            _ => MixCfg { slot: i, from: usize::MAX, base_amount: 1.0 },
        });
        for (slot, node) in &mut self.nodes {
            if structural {
                node.set_engine(self.opts.pitch_engine);
            }
            node.apply(&fx[*slot], structural);
            if let AudioNode::Gain(g) = node {
                g.bind_curves(&self.auto, *slot);
            }
            node.bind_lfo(&self.auto, *slot, self.clip_len);
        }
        self.recompute_lookahead();
        // warp half of Slowed+Reverb (source consumption)
        for f in fx {
            if let FxInstance::SlowedReverb { rate, .. } = f {
                self.rate = rate.clamp(kt_model::RATE_CLAMP.0, kt_model::RATE_CLAMP.1);
                self.reach = kt_model::rate_reach(self.rate, self.rate_points());
                let has_rate_curve = !self.rate_points().is_empty();
                if let Some(w) = &mut self.warp
                    && !has_rate_curve
                {
                    // with automation produce() will set it itself by position
                    w.set_rate(self.rate.abs());
                }
            }
        }
    }

    /// HOLD on this chunk: the speed is below the audio floor (R.57).
    ///
    /// The decision is per chunk, not per sample — by the same midpoint rule as
    /// the other automation targets: the integrand is linear, and the midpoint of the trapezoid is its
    /// exact mean. Otherwise the decision would have to be made for every frame, and the resampler cursor
    /// would not survive that.
    fn hold_at(&self, chunk_in: i64, n: usize) -> bool {
        let tau = (chunk_in as f64 + n as f64 * 0.5) / self.clip_len as f64;
        eff_rate(self.rate, self.rate_points(), tau).abs() < kt_model::RATE_AUDIO_FLOOR
    }

    /// Whether the chain stretches time BY MODULE (R.57).
    ///
    /// Precisely by module: a constant −1 does not stretch the material, so the
    /// resampler is not needed for it — it stays on the FLAT path, and “minus
    /// one” is an exact mirror, not an approximation.
    fn has_warp(&self) -> bool {
        (self.rate.abs() - 1.0).abs() > 1e-9
            || self.rate_points().iter().any(|(_, r)| (r.abs() - 1.0).abs() > 1e-9)
    }

    pub fn tail(&self) -> Tick {
        self.nodes.iter().map(|(_, n)| n.tail()).max().unwrap_or(Tick::ZERO)
    }

    pub fn tail_frames(&self) -> i64 {
        self.tail().to_audio_frames()
    }

    /// Full state reset and permission for a new seek.
    pub fn reset(&mut self) {
        self.warp = None;
        for (_, n) in &mut self.nodes {
            n.reset();
        }
        if let Some(r) = &mut self.dry_ring {
            r.reset();
        }
        self.next_local = i64::MIN;
    }

    /// Material anchor of the event (frames from file zero).
    pub fn src_in(&self) -> i64 {
        self.src_in
    }

    /// Reposition the material anchor, KEEPING the FX state (R.43, item 293).
    ///
    /// `chain_fingerprint` knows only the KINDS of slots, so trim, split,
    /// punch-out and re-anchoring of a morph piece changed `source_in` with the same
    /// `ClipId` and the same chain structure — and the processor kept reading
    /// from the OLD anchor. `reset()` did not help: `hard_seek`/`seam` compute
    /// the position as `self.src_in + source_offset(local)`. Symptom —
    /// “sound from another event came from somewhere”.
    ///
    /// This is a SEAM, not a seek: the output is continuous, and the reverb must ring on
    /// (invariant 3).
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

    /// Read the frames [local, local + out/CH) of the clip-local stream.
    /// `pull(src_frame, buf)` — the source (false = underrun).
    /// Returns the underrun counter of the warp path.
    pub fn read(
        &mut self,
        local: i64,
        out: &mut [f32],
        pull: &mut dyn FnMut(PullKind, i64, &mut [f32]) -> bool,
    ) -> u64 {
        if self.next_local != local {
            if self.next_local == i64::MIN {
                // Cold start or seek (`reset`): reset + pre-roll,
                // so that the reverb and the resampler are warmed up deterministically.
                self.hard_seek(local, pull);
            } else {
                // SEAM: the position jumped, but the OUTPUT is continuous (wrap of a
                // loop region, a clip shifted by the mouse during playback). The reverb
                // rings on through the seam — as in v1; there is no pre-roll, and nothing
                // to warm. This also removes the sinc-resampler re-creation on
                // each lap (with a region shorter than a burst there were
                // several in one callback — “the computer hums”, item 166).
                self.seam(local);
            }
        }
        let before = self.warp.as_ref().map(|w| w.underruns()).unwrap_or(0);
        self.produce(PullKind::Output, local, out, pull);
        self.next_local = local + (out.len() / CH) as i64;
        self.warp.as_ref().map(|w| w.underruns()).unwrap_or(0) - before
    }

    /// How much material the CHAIN has EATEN up to the clip-local position — the
    /// resampler cursor (R.57).
    ///
    /// Precisely the consumption A(t) = ∫|rate|, not the position: the rubato eats the stream
    /// at speed |rate| and does not know the direction. On positive
    /// curves A ≡ S bit for bit, and the previous path is intact.
    fn source_offset(&self, local: i64) -> i64 {
        let t = local as f64 / self.clip_len as f64;
        (self.clip_len as f64 * kt_model::consumed_abs(self.rate, self.rate_points(), t)).round()
            as i64
    }

    /// A POSITION jump with a continuous OUTPUT: the source is repositioned,
    /// the FX state lives on. No reset, no pre-roll (see `read`).
    fn seam(&mut self, local: i64) {
        // the source stands at the INPUT position: the chain reads ahead by the
        // lookahead (R.48, item 337). The lookahead nodes’ rings live on in this case —
        // the seam rings out, like reverb (invariant 3).
        let local = local + self.la;
        let src_start = self.src_in + self.source_offset(local);
        if self.has_warp() {
            let t = local as f64 / self.clip_len as f64;
            let r = rate_at(self.rate, self.rate_points(), t).abs();
            match &mut self.warp {
                Some(w) => {
                    w.set_rate(r);
                    w.seek_source(src_start);
                }
                None => self.warp = Some(Warp::new(r, src_start, self.reach)),
            }
        } else {
            self.warp = None;
        }
        self.plain_cursor = src_start;
    }

    fn hard_seek(&mut self, local: i64, pull: &mut dyn FnMut(PullKind, i64, &mut [f32]) -> bool) {
        for (_, n) in &mut self.nodes {
            n.reset();
        }
        // pre-roll may reach into the left “handle”, but not before the start of the
        // material (−head_ext)
        let preroll =
            self.tail_frames().max(MIN_PREROLL_FRAMES).min((local + self.head_ext).max(0));
        let start = local - preroll;
        // source position for local=start (the integral of the automation).
        // The formula does NOT change: `start` is the first INPUT position of the warm-up,
        // while the output cycle starts ahead of it by the lookahead (R.48, item 337).
        let src_start = self.src_in + self.source_offset(start);
        if self.has_warp() {
            let t = start as f64 / self.clip_len as f64;
            let r = rate_at(self.rate, self.rate_points(), t).abs();
            let mut w = Warp::new(r, src_start, self.reach);
            w.set_rate(r);
            self.warp = Some(w);
        } else {
            self.warp = None;
        }
        self.plain_cursor = src_start;
        if let Some(r) = &mut self.dry_ring {
            r.reset();
        }
        // Warm-up: render into garbage. The output cycle starts EARLIER by the lookahead
        // (its first chunk reads the input exactly from `start`), and its length grows by the same
        // amount — by the moment `local` the rings of the lookahead nodes
        // are filled with real material [local, local + la).
        // With la = 0 — the previous cycle verbatim.
        let warm = preroll + self.la;
        let start_out = start - self.la;
        let mut scratch = vec![0f32; 1024 * CH];
        let mut done = 0i64;
        while done < warm {
            let n = (warm - done).min(1024) as usize;
            self.produce(PullKind::Warmup, start_out + done, &mut scratch[..n * CH], pull);
            done += n as i64;
        }
    }

    /// Maximum step of the automation evaluation: the buffer is cut into chunks of this
    /// size REGARDLESS of the request size — an offline render that gives the clip
    /// in one piece must follow the profile just as playback does
    /// burst by burst. (Regression: evaluating once for the whole buffer gave a
    /// constant slowdown by the value in the middle of the clip and a truncated
    /// end of the source; caught by ear, closed by the tests
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
            // The OUTPUT position of the chunk (what the listener hears) and the INPUT position
            // (what the chain reads now).
            // They diverge by exactly the chain’s lookahead (R.48, item 337); with la = 0 everything below
            // is arithmetically identical to the pre-PDC version.
            let chunk_local = local + done as i64;
            let chunk_in = chunk_local + self.la;
            // automation: targets at the chunk midpoint; rubato/one-pole smooth
            if !self.auto.is_empty() {
                // EVENT DOMAIN (R.37): the chunk’s target is by OUTPUT time,
                // not by the position in the material. An early speed keyframe
                // no longer shifts the MOMENT of firing of later keys —
                // neither its own nor others’ (mix/gain/pan were indexed by the same
                // `s`). The midpoint rule of the chunk has thereby become EXACT:
                // the integrand is linear, and the midpoint of the trapezoid is
                // its exact mean.
                // the warp drives the chain’s INPUT: it is the consumer of the source
                let tau = (chunk_in as f64 + n as f64 * 0.5) / self.clip_len as f64;
                // into the resampler — the MODULE (R.57); the hold (|rate| below the floor)
                // does not change the ratio: there is nothing to read there
                let target_rate = eff_rate(self.rate, self.rate_points(), tau).abs();
                if let Some(w) = &mut self.warp
                    && target_rate >= kt_model::RATE_AUDIO_FLOOR
                {
                    w.set_rate(target_rate);
                }
                // node targets — by the same chunk-midpoint rule, but by the
                // position of the CONTENT that the node now sees: the curve
                // stays glued to the material, not to the output
                let ahead = &self.ahead;
                for (i, (slot, node)) in self.nodes.iter_mut().enumerate() {
                    let s = (chunk_local + ahead[i]) as f64 + n as f64 * 0.5;
                    node.set_targets(&self.auto, *slot, s / self.clip_len as f64);
                }
            }
            // layer-strength target at the chunk midpoint — by the same rule as the
            // other parameters (R.38 item 12: the fades are its curve)
            let mix_ahead = self.mix_ahead;
            let mix_k = self.mix.as_ref().map(|m| {
                let tau =
                    ((chunk_local + mix_ahead) as f64 + n as f64 * 0.5) / self.clip_len as f64;
                self.auto.value(m.slot, "amount", m.base_amount, tau).clamp(0.0, 1.0) as f32
            });
            let sub = &mut out[done * CH..(done + n) * CH];
            // The dry-output gate is in the OUTPUT domain: beyond the end of the MATERIAL (body +
            // the right transition “handle”, item 138) the dry signal must be zero
            // (only the reverb rings on in the tail). Thanks to this the SOURCE
            // window can be widened with a margin for the rubato priming without
            // fear of leaking dry into the tail (defect “quiet tail”: the sinc resampler
            // saw zeros beyond a hard-cut window and cut the end too early).
            // The gate — in the INPUT domain: beyond the end of the material the chain
            // gets deterministic silence FORWARD, and the rings of the lookahead nodes
            // push the body out exactly to the end of the event.
            let gate_end = self.clip_len + self.tail_ext;
            if chunk_in >= gate_end {
                // A WHOLE CHUNK IN THE TAIL (R.43, item 304): whatever the warp produces,
                // the gate below zeroes it COMPLETELY — so there is nothing to compute.
                // Skipping is byte-identical BY CONSTRUCTION: the result is the same
                // `fill(0.0)`, only without the sinc resampler
                // and without reading the source.
                // In the tail lives only one reverb, and it gets the same zeros.
                //
                // The warp state “lags behind” in this case, and that is safe:
                // returning to the material is possible only by a jump of `local`, and it is
                // caught by `read`, which repositions the source via `seam`/
                // `hard_seek`. Continuous reading of `local` only grows.
                sub.fill(0.0);
                self.plain_cursor += n as i64;
            } else if self.hold_at(chunk_in, n) {
                // HOLD (R.57, the owner’s decision “silence only at
                // zero”): speed below the floor — the material is not consumed
                // AT ALL, the output is zero, the cursor stands still. The nodes meanwhile
                // keep working: the reverb rings out the tail on zeros, just as in
                // the pause between events.
                sub.fill(0.0);
            } else {
                match &mut self.warp {
                    Some(w) => w.produce(kind, sub, pull),
                    None => {
                        if !pull(kind, self.plain_cursor, sub) {
                            sub.fill(0.0);
                        }
                        self.plain_cursor += n as i64;
                    }
                }
                if chunk_in + (n as i64) > gate_end {
                    let cut = (gate_end - chunk_in).clamp(0, n as i64) as usize;
                    sub[cut * CH..].fill(0.0);
                }
            }
            // HONEST SLOT ORDER (R.47, item 325): the nodes are executed by their
            // position in the chain. The dry side of the layer strength — the snapshot BEFORE the
            // first node with a slot >= from (reverb/volume of the event
            // stand earlier — they fall into the dry; the layer’s nodes — only into the
            // wet: the former reverb_is_layer/layer_gain became consequences of
            // position, not flags).
            let mut dry: Option<Vec<f32>> = None;
            let ahead = &self.ahead;
            let dry_ring = &mut self.dry_ring;
            for (i, (slot, node)) in self.nodes.iter_mut().enumerate() {
                if let Some(m) = &self.mix
                    && *slot >= m.from
                    && dry.is_none()
                {
                    let mut d = sub.to_vec();
                    // between the snapshot and the marker stands a lookahead node:
                    // the dry side must be delayed by the window it has eaten,
                    // otherwise the blend would mix different moments of the material
                    if let Some(r) = dry_ring.as_mut() {
                        r.run(&mut d);
                    }
                    dry = Some(d);
                }
                if matches!(node, AudioNode::LayerMix) {
                    if let (Some(k), Some(d)) = (mix_k, &dry) {
                        let ik = 1.0 - k;
                        for (v, dv) in sub.iter_mut().zip(d.iter()) {
                            *v = dv * ik + *v * k;
                        }
                    }
                } else {
                    // an event has no key: the sidechain lives only in the TRACK chain
                    // (the owner’s decision, R.65)
                    node.process(sub, chunk_local + ahead[i], None);
                }
            }
            done += n;
        }
    }
}

/// Node of the ADDRESS map of the signed speed (R.57): the accumulated consumption
/// `a`, the position in the material `p` (both — frames from `source_in`) and the direction
/// after the node (+1 forward, −1 backward, 0 hold).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AddrKnot {
    pub a: i64,
    pub p: i64,
    pub dir: i8,
}

/// The address map of the chain: at which address the material eaten by the moment
/// `a` lies. `None` — the map is monotone, the address equals the consumption, and
/// the whole previous path remains verbatim.
///
/// Positions accumulate in WHOLE numbers (`p_{k+1} = p_k ± (a_{k+1} − a_k)`), not
/// rounded separately: otherwise at the junction of a turn there would be a skipped or
/// doubled frame — a click exactly at the turning point.
pub fn chain_addr_knots(
    fx: &[FxInstance],
    auto: &AutoProfiles,
    clip_len_frames: i64,
) -> Option<Vec<AddrKnot>> {
    let len = clip_len_frames.max(1);
    let rate = fx.iter().find_map(|f| match f {
        FxInstance::SlowedReverb { rate, .. } => Some(*rate),
        _ => None,
    })?;
    let points = auto.rate_points(fx);
    if !kt_model::rate_signed(rate, points) {
        return None;
    }
    let knots = kt_model::sign_knots(rate, points);
    let mut out: Vec<AddrKnot> = Vec::with_capacity(knots.len());
    let mut p = 0i64;
    let mut prev_a = 0i64;
    for (i, k) in knots.iter().enumerate() {
        let a = (len as f64 * k.a).round() as i64;
        if i > 0 {
            let prev_dir = out.last().map(|x: &AddrKnot| x.dir).unwrap_or(1) as i64;
            p += prev_dir * (a - prev_a);
        }
        out.push(AddrKnot { a, p, dir: k.dir_after });
        prev_a = a;
    }
    Some(out)
}
