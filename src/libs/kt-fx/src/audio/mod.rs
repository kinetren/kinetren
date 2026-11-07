//! Audio processor of a clip: an FX chain over the (possibly warped)
//! source. The public surface of the crate is re-exported from lib.rs —
//! the paths of callers (`kt_fx::ClipProcessor` etc.) have not changed.

pub mod cost;
pub mod loudness;
mod nodes;
pub mod primitives;
mod sinc;
mod warp;

pub use nodes::{AudioNode, Freeverb};
pub use primitives::SmoothedParam;
pub use warp::Warp;

use crate::{CH, MIN_PREROLL_FRAMES, PullKind};
use kt_model::FxInstance;
use kt_time::Tick;

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
    // across the event's edges when the chain reads there (pitch-event-edges D1)
    let material = read_material(fx, auto, material);
    let l0 = (local.start - preroll).max(material.start).min(material.end);
    // FORWARD — for the chain’s lookahead (R.48, item 337): exactly how much of the source
    // the chain actually reads while giving the same output. The fix lives in
    // ONE function, so render and prefetch expand the same way
    // (if they diverge — a permanent cache miss, item 13/t17).
    let l1 = (local.end + chain_lookahead(fx, auto)).clamp(material.start, material.end);
    let has_warp = fx.iter().any(FxInstance::warps_time);
    if !has_warp {
        return l0..l1.max(l0);
    }
    let rate = fx.iter().find_map(FxInstance::warp_rate).unwrap_or(1.0);
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
    let material = read_material(fx, auto, material);
    let l0 = (local.start - chain_lookahead(fx, auto)).clamp(material.start, material.end);
    let l1 = (local.end + chain_lookahead(fx, auto)).clamp(material.start, material.end);
    let Some(rate) = fx.iter().find_map(FxInstance::warp_rate) else {
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
    let Some(rate) = fx.iter().find_map(FxInstance::warp_rate) else {
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
            // a plugin slot: the manifest's formula, the one home of it (D2a; t322g)
            f @ FxInstance::Plugin(p) => nodes::plugin_audio(f).map_or(0, |(m, _)| {
                kt_plugin_api::lookahead_frames_with(
                    m,
                    &kt_plugin_api::ParamValues(p.values.clone()),
                    &nodes::SlotView { auto, slot },
                )
            }),
            _ => 0i64,
        })
        .sum()
}

/// Whether the chain READS ACROSS THE EVENT'S EDGES (pitch-event-edges D1, D2) — the one home, asked
/// by the processor and by the window functions (if they diverged, render and prefetch would read
/// different frames: item 13, t17). True when a node that declares `reads_across_edges` has a
/// non-zero look-ahead and a time warp, if any, moves forward at the event's start. The nodes BEFORE
/// it process the material too, and a tail of theirs ends with the event (pitch-context-whole-chain
/// D1: the owner's morpher pitch after Slowed+Reverb tore at every join while they were excluded).
///
/// Then the chain's input around the event's edges is the event's own source — the seek pre-roll
/// reads before the in-point, the input runs `La` past the out-point — and a content gate right after
/// the last such node cuts the result back to the event: "the material pitched, then cut".
pub fn chain_context(fx: &[FxInstance], auto: &AutoProfiles) -> bool {
    warp_forward(fx, auto)
        && fx.iter().enumerate().any(|(slot, f)| {
            nodes::plugin_audio(f).is_some_and(|(m, _)| m.decl.reads_across_edges)
                && slot_lookahead(fx, auto, slot) > 0
        })
}

/// A time warp lets the context through only when the speed at the event's start moves forward
/// (pitch-context-whole-chain D2): the pre-roll consumes the material before the in-point at that
/// speed, as `consumed_abs` and the address map extrapolate it. No warp — nothing to ask.
fn warp_forward(fx: &[FxInstance], auto: &AutoProfiles) -> bool {
    fx.iter().find_map(FxInstance::warp_rate).is_none_or(|rate| {
        let rate = rate.clamp(kt_model::RATE_CLAMP.0, kt_model::RATE_CLAMP.1);
        eff_rate(rate, auto.rate_points(fx), 0.0) >= kt_model::RATE_AUDIO_FLOOR
    })
}

/// The look-ahead of one slot: the manifest's formula (D2a; t322g).
fn slot_lookahead(fx: &[FxInstance], auto: &AutoProfiles, slot: usize) -> i64 {
    match &fx[slot] {
        f @ FxInstance::Plugin(p) => nodes::plugin_audio(f).map_or(0, |(m, _)| {
            kt_plugin_api::lookahead_frames_with(
                m,
                &kt_plugin_api::ParamValues(p.values.clone()),
                &nodes::SlotView { auto, slot },
            )
        }),
        _ => 0,
    }
}

/// The material a chain READS: with the context (`chain_context`) it reaches the seek pre-roll
/// before the event's material and the look-ahead past it; otherwise the material itself.
fn read_material(
    fx: &[FxInstance],
    auto: &AutoProfiles,
    material: std::ops::Range<i64>,
) -> std::ops::Range<i64> {
    if !chain_context(fx, auto) {
        return material;
    }
    let preroll = chain_tail(fx, auto).to_audio_frames().max(MIN_PREROLL_FRAMES);
    (material.start - preroll)..(material.end + chain_lookahead(fx, auto))
}

/// PROJECT settings that affect the execution of the chain (R.48): the pitch
/// engine is one for the whole edit. Not a parameter value but the device of the node —
/// a change rebuilds the processor (checked in `FxStateStore`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChainOpts {
    pub pitch_engine: kt_model::PitchEngine,
    /// The project frame rate (sim-audio-pairs): the audio half of a simulated device reads the
    /// picture's frame grid (`AudioState::place`). 0 — unknown, read as 25.
    pub fps: u32,
}

impl ChainOpts {
    /// Where a clip starting at timeline frame `start` sits, for its nodes.
    pub fn placement(&self, start: i64) -> kt_plugin_api::Placement {
        kt_plugin_api::Placement { start, fps_num: if self.fps == 0 { 25 } else { self.fps }, fps_den: 1 }
    }
}

pub fn chain_tail(fx: &[FxInstance], auto: &AutoProfiles) -> Tick {
    fx.iter()
        .enumerate()
        .map(|(slot, f)| match f {
            // a plugin slot: the manifest's formula, the one home of it (D2a; t322f)
            f @ FxInstance::Plugin(p) => nodes::plugin_audio(f).map_or(Tick::ZERO, |(m, _)| {
                Tick::from_audio_frames(kt_plugin_api::tail_frames_with(
                    m,
                    &kt_plugin_api::ParamValues(p.values.clone()),
                    &nodes::SlotView { auto, slot },
                ))
            }),
            _ => Tick::ZERO,
        })
        .max()
        .unwrap_or(Tick::ZERO)
}

/// Fingerprint of the chain’s STRUCTURE (FX kinds in order). Matches → state
/// is reused, parameters arrive via set_params (smoothed). The KIND is the variant AND its tag:
/// every plugin slot is the one `Plugin` variant, and the variant alone let an EQ's node live on
/// under a filter put in its slot (t1057).
pub fn chain_fingerprint(fx: &[FxInstance]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for f in fx {
        std::mem::discriminant(f).hash(&mut h);
        f.tag().hash(&mut h);
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
                .filter_map(|(slot, f)| AudioNode::build(f, self.opts.pitch_engine).map(|n| (slot, n)))
                .collect();
            self.fingerprint = chain_fingerprint(fx);
        }
        for (slot, node) in &mut self.nodes {
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
    /// The timeline frame of the clip's frame 0 (sim-audio-pairs, `AudioState::place`).
    origin: i64,
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
    /// The armed loop region in clip-local frames (pitch-loop-silence, D2) — `set_loop_span`.
    loop_span: Option<(i64, i64)>,
    /// The chain input position the next chunk is expected at (with a loop span): a folded
    /// input that differs from it re-seats the source. `Some(i64::MIN)` forces a re-seat.
    in_next: Option<i64>,
    /// Per node: its kind declares `reads_across_edges` (pitch-event-edges D2).
    across: Vec<bool>,
    /// The chain's time warp starts in hold or backwards (no context then: `chain_context`).
    warp_back: bool,
    /// The context applies (`chain_context`): the index of the LAST node that reads across the
    /// event's edges — the content gate stands right after it. `None` — the former edges.
    ctx: Option<usize>,
    /// Per node: an edge-reading slot whose echo row is on (pitch-tail-choice D1).
    echo_on: Vec<bool>,
    /// "Rings on" is in force (pitch-tail-choice): the context applies, the slot at `ctx` rings the
    /// echo of the nodes before it, one of them rings, and the engine has not said that the next
    /// piece of a split carries this echo (`set_ring_end`). `false` — the cut, byte for byte.
    ring: bool,
    /// The engine's word (pitch-tail-choice D5): the next event continues this one's material as a
    /// split, so its context carries the echo — this one does not ring.
    ring_off: bool,
    /// The tail twin (D3): nodes `0..=ctx` split off where the input reached the out-point, fed
    /// silence from there; it gives the output from the out-point on until it replaces them.
    twin: Option<Twin>,
    /// The twin has replaced nodes `0..=ctx` (D3): the gate does not cut past the out-point and the
    /// input stops at it, until the stream is back inside the event.
    ringing: bool,
}

/// The nodes `0..=ctx` split off at the out-point and the dry ring of a layer mix with them
/// (pitch-tail-choice D3).
struct Twin {
    nodes: Vec<(usize, AudioNode)>,
    dry_ring: Option<DryRing>,
}

/// Delay of the dry side by a whole number of frames (R.48): between the snapshot of the dry
/// side and the LayerMix marker node a lookahead node may stand,
/// having “eaten” part of the lookahead — without alignment the blend would mix different
/// moments of the material. With a zero delta the ring is not created at all.
#[derive(Clone)]
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
            origin: 0,
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
                .position(FxInstance::warps_time)
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
            loop_span: None,
            in_next: None,
            across: Vec::new(),
            warp_back: false,
            ctx: None,
            echo_on: Vec::new(),
            ring: false,
            ring_off: false,
            twin: None,
            ringing: false,
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
        // the context — the same decision as the static `chain_context` (t1109): the last node that
        // reads across the edges with a live look-ahead, unless a warp starts in hold or backwards
        let last = (0..n).rev().find(|&i| self.across[i] && self.nodes[i].1.lookahead() > 0);
        self.ctx = last.filter(|_| !self.warp_back);
        self.recompute_ring();
    }

    /// "Rings on" (pitch-tail-choice): only when something before the edge-reader rings — otherwise
    /// the choice changes nothing, and the cut path runs byte for byte.
    fn recompute_ring(&mut self) {
        self.ring = !self.ring_off
            && self.ctx.is_some_and(|c| {
                self.echo_on.get(c).copied().unwrap_or(false)
                    && self.nodes[..c].iter().any(|(_, node)| node.tail() > Tick::ZERO)
            });
        if !self.ring {
            self.twin = None;
            self.ringing = false;
        }
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

    /// The next event continues this one's material as a split (pitch-tail-choice D5): its context
    /// already carries the echo of this one's last moments, as the uncut event would, so this one does
    /// not ring it on as well. Without a state reset.
    pub fn set_ring_end(&mut self, off: bool) {
        if self.ring_off != off {
            self.ring_off = off;
            self.recompute_ring();
        }
    }

    /// Seat the clip on the timeline (sim-audio-pairs): its frame 0 is timeline frame `start`.
    /// Without a state reset — a moved clip keeps its nodes (invariant 3).
    pub fn set_origin(&mut self, start: i64) {
        if self.origin == start {
            return;
        }
        self.origin = start;
        let at = self.opts.placement(start);
        for (_, node) in &mut self.nodes {
            node.place(at);
        }
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
                .filter_map(|(slot, f)| AudioNode::build(f, self.opts.pitch_engine).map(|n| (slot, n)))
                .collect();
            self.across = self
                .nodes
                .iter()
                .map(|(slot, _)| {
                    nodes::plugin_audio(&fx[*slot]).is_some_and(|(m, _)| m.decl.reads_across_edges)
                })
                .collect();
        }
        self.warp_back = !warp_forward(fx, &self.auto);
        self.echo_on = self.nodes.iter().map(|(slot, _)| nodes::echo_rings(&fx[*slot])).collect();
        debug_assert_eq!(
            self.nodes.len(),
            fx.iter().filter(|f| AudioNode::has_node(f)).count(),
            "node list out of sync with the chain"
        );
        self.mix = fx.iter().position(|f| f.is_layer_mix()).map(|i| match &fx[i] {
            FxInstance::LayerMix { amount, from } => {
                MixCfg { slot: i, from: (*from as usize).min(i), base_amount: *amount }
            }
            _ => MixCfg { slot: i, from: usize::MAX, base_amount: 1.0 },
        });
        let at = self.opts.placement(self.origin);
        for (slot, node) in &mut self.nodes {
            node.apply(&fx[*slot], structural);
            node.bind_lfo(&self.auto, *slot, self.clip_len);
            if structural {
                node.place(at);
            }
        }
        self.recompute_lookahead();
        // warp half of Slowed+Reverb (source consumption)
        for f in fx {
            if let Some(rate) = f.warp_rate() {
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
        self.in_next = None;
        self.twin = None;
        self.ringing = false;
    }

    /// The armed loop region in CLIP-LOCAL frames, or `None` (pitch-loop-silence, D2). The
    /// renderer sets it for every call of a looped render on an event that covers the region
    /// end. With a look-ahead in the chain the INPUT then wraps first: it reads the region head
    /// `la` frames before the output wraps, so the look-ahead holds what will actually sound —
    /// before this, the last `la` frames of a lap read material PAST the region end (the limiter
    /// played it after the wrap) and the pitch node restarted from an empty ring. Without a
    /// look-ahead it changes nothing.
    pub fn set_loop_span(&mut self, span: Option<(i64, i64)>) {
        self.loop_span = span.filter(|(a, b)| b > a);
    }

    /// The loop fold of a chain position: a position past the span end continues from the span
    /// start (several times for a span shorter than the look-ahead).
    fn fold(&self, p: i64) -> i64 {
        match self.loop_span {
            Some((a, b)) if self.la > 0 && p >= b => a + (p - a).rem_euclid(b - a),
            _ => p,
        }
    }

    /// Whether the input-side fold is in force.
    fn folding(&self) -> bool {
        self.la > 0 && self.loop_span.is_some()
    }

    /// Re-seat the SOURCE at a chain input position, keeping every node's state.
    fn reseat_source(&mut self, input: i64) {
        let src_start = self.src_in + self.source_offset(input);
        if self.has_warp() {
            let t = input as f64 / self.clip_len as f64;
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
        self.in_next = Some(input);
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
            } else if self.folding() && Some(self.fold(local + self.la)) == self.in_next {
                // The output wraps to the loop start, and the INPUT already did, `la` frames
                // ago (pitch-loop-silence, D2): the chain is continuous, nothing to re-seat.
            } else if self.fresh_entry(local) || self.lap_entry(local) {
                // A FRESH ENTRY (pitch-event-edges D4): the event had stopped and is read again
                // from its first frame (a loop lap over several events) — its output was silent in
                // between, so there is nothing to ring through, and a seam would reuse a look-ahead
                // ring holding the material from where it stopped. A NEW LAP of a chain with a tail
                // starts the same way (loop-pitch-tail-dip): the engine has moved what still rings
                // of the previous lap to a stream of its own (`FxStateStore::start_lap`).
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

    /// The context applies: the chain reads the event's own source across its edges
    /// (`chain_context`, pitch-event-edges D1).
    pub fn reads_across_edges(&self) -> bool {
        self.ctx.is_some()
    }

    /// The clip-local frame the processor gives next; `None` — cold (after a reset or a seek).
    pub fn next_local(&self) -> Option<i64> {
        (self.next_local != i64::MIN).then_some(self.next_local)
    }

    /// Reading `local` would be a FRESH ENTRY (pitch-event-edges D4): the context applies, the chain
    /// has no tail, and the event is entered at its first frame or after it had played to its end —
    /// not continuously, and not as the expected input-first loop wrap. A cold processor is one too.
    pub fn fresh_entry(&self, local: i64) -> bool {
        if self.next_local == i64::MIN {
            return true;
        }
        self.tail_frames() == 0 && self.reentry(local)
    }

    /// Reading `local` starts a NEW LAP of a chain with a tail (loop-pitch-tail-dip): the context
    /// applies, and the event is entered again at its first frame or after its out-point, not
    /// continuously and not as the expected input-first wrap. The look-ahead ring holds where the
    /// previous lap stopped, so a seam would start the lap with that (≈ 75 ms of silence with "rings
    /// on"); the lap starts as a seek there does, and what still rings of the previous lap is the
    /// engine's to keep (`past_out_point`).
    pub fn lap_entry(&self, local: i64) -> bool {
        self.next_local != i64::MIN && self.tail_frames() > 0 && self.reentry(local)
    }

    /// The context applies and `local` re-enters the event (the rule shared by `fresh_entry` and
    /// `lap_entry`).
    fn reentry(&self, local: i64) -> bool {
        self.ctx.is_some()
            && self.next_local != local
            && (local == -self.head_ext || self.next_local >= self.clip_len + self.tail_ext)
            && !(self.folding() && Some(self.fold(local + self.la)) == self.in_next)
    }

    /// The processor stands in its tail: its next frame is at or past the material's end, so what it
    /// gives from here on is the echo of the lap it played (loop-pitch-tail-dip). Before the end — a
    /// lap cut by the wrap — nothing of it is heard any more.
    pub fn past_out_point(&self) -> bool {
        self.next_local != i64::MIN && self.next_local >= self.clip_len + self.tail_ext
    }

    /// Hand this processor on to the NEXT event of a split (pitch-event-edges D3): it has just
    /// played its event to the end (`shift` = its length), and the next event's frame 0 is that end.
    /// The clip-local position moves back by `shift`; the caller then gives it the next event's
    /// anchor, length and parameters (`FxStateStore::sync_clip`) — the anchor moves through a seam,
    /// and the nodes continue their streams (a pitch node sees a seam, pitch-loop-silence D1).
    pub fn hand_on(&mut self, shift: i64) {
        if self.next_local != i64::MIN {
            self.next_local -= shift;
        }
        self.in_next = None;
        self.loop_span = None;
        self.twin = None;
        self.ringing = false;
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
        let input = self.fold(local + self.la);
        // a twin split off before the jump belongs to the stream that stopped (pitch-tail-choice D3)
        self.twin = None;
        self.reseat_source(input);
    }

    fn hard_seek(&mut self, local: i64, pull: &mut dyn FnMut(PullKind, i64, &mut [f32]) -> bool) {
        for (_, n) in &mut self.nodes {
            n.reset();
        }
        self.twin = None;
        self.ringing = false;
        // pre-roll may reach into the left “handle”, but not before the start of the
        // material (−head_ext)
        // with the context (pitch-event-edges D1) it reads the source before the in-point too:
        // a pitch grain at the event's first frame needs the material the event was cut from
        let preroll = if self.ctx.is_some() {
            self.tail_frames().max(MIN_PREROLL_FRAMES)
        } else {
            self.tail_frames().max(MIN_PREROLL_FRAMES).min((local + self.head_ext).max(0))
        };
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
        self.in_next = None;
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
            let mut n = (total - done).min(Self::AUTOMATION_STEP_FRAMES);
            // The OUTPUT position of the chunk (what the listener hears) and the INPUT position
            // (what the chain reads now).
            // They diverge by exactly the chain’s lookahead (R.48, item 337); with la = 0 everything below
            // is arithmetically identical to the pre-PDC version.
            let chunk_local = local + done as i64;
            // THE INPUT WRAPS FIRST (pitch-loop-silence, D2): a chunk does not straddle a point
            // where the chain input — or a node's content position — crosses the loop end; there
            // the fold takes over. Without a loop span or a look-ahead nothing is split.
            if self.folding() {
                let (a, b) = self.loop_span.unwrap_or((0, 0));
                let span = b - a;
                let mut cut = n as i64;
                for d in std::iter::once(self.la).chain(self.ahead.iter().copied()) {
                    // the next output position at which `chunk_local + d` reaches a fold point
                    let p = chunk_local + d;
                    let next = if p < b { b } else { b + span * ((p - b) / span + 1) };
                    let at = next - d - chunk_local;
                    if at > 0 && at < cut {
                        cut = at;
                    }
                }
                n = cut as usize;
            }
            let chunk_in = self.fold(chunk_local + self.la);
            if self.folding() {
                // the source follows the folded input: re-seated where it jumps, and kept
                // silent before the event's material (an event starting inside the region)
                if self.in_next.is_some_and(|p| p != chunk_in) {
                    self.reseat_source(chunk_in);
                    self.twin = None;
                }
                self.in_next = Some(chunk_in + n as i64);
            }
            // THE TAIL TWIN TAKES OVER (pitch-tail-choice D3): once the gate's content position is
            // past the out-point for the whole chunk, the twin replaces nodes 0..=ctx — the main ones
            // hold the echo of the material past the out-point, which is never heard; back inside
            // the event (a seam, a fold) the gate cuts at the out-point again
            if let Some(c) = self.ctx.filter(|_| self.ring) {
                let p0 = self.fold(chunk_local + self.ahead.get(c + 1).copied().unwrap_or(0));
                if p0 >= self.clip_len + self.tail_ext {
                    if let Some(t) = self.twin.take() {
                        for (m, tn) in self.nodes[..=c].iter_mut().zip(t.nodes) {
                            *m = tn;
                        }
                        self.dry_ring = t.dry_ring;
                        self.ringing = true;
                    }
                } else {
                    self.ringing = false;
                }
            }
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
                let fold = |p: i64| match self.loop_span {
                    Some((a, b)) if self.la > 0 && p >= b => a + (p - a).rem_euclid(b - a),
                    _ => p,
                };
                for (i, (slot, node)) in self.nodes.iter_mut().enumerate() {
                    let s = fold(chunk_local + ahead[i]) as f64 + n as f64 * 0.5;
                    node.set_targets(&self.auto, *slot, s / self.clip_len as f64);
                }
                // the twin follows the same targets: it is the same stream fed silence
                if let Some(t) = self.twin.as_mut() {
                    for (i, (slot, node)) in t.nodes.iter_mut().enumerate() {
                        let s = fold(chunk_local + ahead[i]) as f64 + n as f64 * 0.5;
                        node.set_targets(&self.auto, *slot, s / self.clip_len as f64);
                    }
                }
            }
            // layer-strength target at the chunk midpoint — by the same rule as the
            // other parameters (R.38 item 12: the fades are its curve)
            let mix_ahead = self.mix_ahead;
            let mix_pos = self.fold(chunk_local + mix_ahead);
            let mix_k = self.mix.as_ref().map(|m| {
                let tau = (mix_pos as f64 + n as f64 * 0.5) / self.clip_len as f64;
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
            // the event's own content ends at `gate_end`; with the context (pitch-event-edges D1)
            // the INPUT runs `la` past it — the content gate after the last edge-reading node cuts
            // the result back to the event
            // a ringing stream (pitch-tail-choice D3) hears nothing past the out-point: the twin it
            // came from was fed silence there
            let gate_in = if self.ctx.is_some() && !self.ringing { gate_end + self.la } else { gate_end };
            // an input FOLDED to before the event's material (the event starts inside the
            // region): the chain gets silence there, as on the first lap before its start. An
            // unfolded input before it is the pre-roll of the context.
            let folded = chunk_in != chunk_local + self.la;
            let before_material =
                self.folding() && folded && chunk_in + (n as i64) <= -self.head_ext;
            if before_material {
                sub.fill(0.0);
                self.in_next = Some(i64::MIN);
            } else if chunk_in >= gate_in {
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
                if chunk_in + (n as i64) > gate_in {
                    let cut = (gate_in - chunk_in).clamp(0, n as i64) as usize;
                    sub[cut * CH..].fill(0.0);
                }
            }
            // THE TAIL TWIN SPLITS OFF (pitch-tail-choice D3) where the input reaches the out-point:
            // a copy of nodes 0..=ctx from exactly this state, fed the chunk with its frames from
            // the out-point on zeroed and silence after — the echo of the event's own sound
            let mut twin_sub: Vec<f32> = Vec::new();
            if let Some(c) = self.ctx.filter(|_| self.ring && !self.ringing) {
                if self.twin.is_none() && !before_material && chunk_in <= gate_end && gate_end < chunk_in + n as i64 {
                    let forked: Option<Vec<(usize, AudioNode)>> =
                        self.nodes[..=c].iter().map(|(slot, node)| node.fork().map(|f| (*slot, f))).collect();
                    if let Some(nodes) = forked {
                        self.twin = Some(Twin { nodes, dry_ring: self.dry_ring.clone() });
                    }
                }
                if self.twin.is_some() {
                    twin_sub = sub.to_vec();
                    let cut = (gate_end - chunk_in).clamp(0, n as i64) as usize;
                    twin_sub[cut * CH..].fill(0.0);
                }
            }
            // HONEST SLOT ORDER (R.47, item 325): the nodes are executed by their
            // position in the chain. The dry side of the layer strength — the snapshot BEFORE the
            // first node with a slot >= from (reverb/volume of the event
            // stand earlier — they fall into the dry; the layer’s nodes — only into the
            // wet: the former reverb_is_layer/layer_gain became consequences of
            // position, not flags).
            let span = if self.la > 0 { self.loop_span } else { None };
            let fold = move |p: i64| match span {
                Some((a, b)) if p >= b => a + (p - a).rem_euclid(b - a),
                _ => p,
            };
            let ahead = &self.ahead;
            let pos = |i: usize| fold(chunk_local + ahead[i]);
            let pass = Pass { mix: self.mix.as_ref(), mix_k, pos: &pos };
            let mut dry: Option<Vec<f32>> = None;
            let nodes_len = self.nodes.len();
            match self.ctx {
                None => pass.run(&mut self.nodes, 0..nodes_len, sub, &mut dry, &mut self.dry_ring),
                Some(c) => {
                    pass.run(&mut self.nodes, 0..c + 1, sub, &mut dry, &mut self.dry_ring);
                    // THE CONTENT GATE (pitch-event-edges D1): right after the last node that reads
                    // across the edges, everything outside the event is cut — the content position
                    // there runs ahead of the output by what the later nodes still read
                    let p0 = fold(chunk_local + if c + 1 < nodes_len { self.ahead[c + 1] } else { 0 });
                    let hi = if self.ringing { i64::MAX } else { gate_end };
                    content_gate(sub, p0, -self.head_ext, hi);
                    // a dry side still waiting for its marker: it is aligned to the marker's
                    // content position
                    let pd = fold(chunk_local + mix_ahead);
                    if let Some(d) = dry.as_mut() {
                        content_gate(d, pd, -self.head_ext, hi);
                    }
                    // the twin gives the frames from the out-point on (pitch-tail-choice D3)
                    if let Some(t) = self.twin.as_mut() {
                        let mut tdry: Option<Vec<f32>> = None;
                        pass.run(&mut t.nodes, 0..c + 1, &mut twin_sub[..], &mut tdry, &mut t.dry_ring);
                        content_gate(&mut twin_sub[..], p0, gate_end, i64::MAX);
                        for (v, tv) in sub.iter_mut().zip(twin_sub.iter()) {
                            *v += tv;
                        }
                        if let (Some(d), Some(mut td)) = (dry.as_mut(), tdry) {
                            content_gate(&mut td, pd, gate_end, i64::MAX);
                            for (v, tv) in d.iter_mut().zip(td.iter()) {
                                *v += tv;
                            }
                        }
                    }
                    pass.run(&mut self.nodes, c + 1..nodes_len, sub, &mut dry, &mut self.dry_ring);
                }
            }
            done += n;
        }
    }
}

/// One pass of a run of nodes over a chunk: the main stream and the tail twin run THIS code
/// (pitch-tail-choice D3), so the twin is the same stream fed silence.
struct Pass<'a> {
    mix: Option<&'a MixCfg>,
    mix_k: Option<f32>,
    /// The content position node `i` sees at the chunk's first frame.
    pos: &'a dyn Fn(usize) -> i64,
}

impl Pass<'_> {
    fn run(
        &self,
        nodes: &mut [(usize, AudioNode)],
        range: std::ops::Range<usize>,
        sub: &mut [f32],
        dry: &mut Option<Vec<f32>>,
        dry_ring: &mut Option<DryRing>,
    ) {
        for i in range {
            let (slot, node) = &mut nodes[i];
            if let Some(m) = self.mix
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
                *dry = Some(d);
            }
            if matches!(node, AudioNode::LayerMix) {
                if let (Some(k), Some(d)) = (self.mix_k, dry.as_ref()) {
                    let ik = 1.0 - k;
                    for (v, dv) in sub.iter_mut().zip(d.iter()) {
                        *v = dv * ik + *v * k;
                    }
                }
                continue;
            }
            // an event has no key: the sidechain lives only in the TRACK chain
            // (the owner’s decision, R.65)
            node.process(sub, (self.pos)(i), None);
        }
    }
}

/// Zero the frames of `buf` (content positions from `p0`) outside `[lo, hi)`.
fn content_gate(buf: &mut [f32], p0: i64, lo: i64, hi: i64) {
    let n = (buf.len() / CH) as i64;
    let a = lo.saturating_sub(p0).clamp(0, n) as usize;
    let b = hi.saturating_sub(p0).clamp(0, n) as usize;
    buf[..a * CH].fill(0.0);
    if b >= a {
        buf[b * CH..].fill(0.0);
    } else {
        buf.fill(0.0);
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
    let rate = fx.iter().find_map(FxInstance::warp_rate)?;
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
