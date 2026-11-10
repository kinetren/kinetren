//! Video path: selection of active clips, frame keys, building the frame plan
//! (video_plan). The pictures are composed by the GPU executor in kt-gpu.
//!
//! Invariant No. 7 (rev. R.29): composition code is ONE — build_plan decides layers,
//! order, transition weights and levels for both preview and export;
//! the executor (kt-gpu) is an execution parameter, not a second path. Layers
//! are requested at ONE quality and composited alpha-over by z
//! (lane: lower = higher, as in the old engine, where a smaller track index
//! was the foreground).

use std::sync::Arc;

use kt_cache::{BurstCache, SourceId};
use kt_media::{Quality, VideoFrame};
use kt_model::{ClipKind, MediaId, Project};
use kt_time::{FpsGrid, Tick};

use crate::video_plan::{Cam3d, FramePlan, GpuExecError, LayerSrc, PlanLayer, VideoGpuExec};

#[path = "video_stream.rs"]
mod stream;
pub use stream::{
    STREAM_MEMO_BASE, StreamPacing, memory_available, set_memory_probe, size_stream_memo, size_stream_memo_walk, stop_when, stopping,
    stream_memo, stream_memo_bytes, stream_memo_bytes_walk,
};
use stream::StreamStage;

/// VIDEO-TRACK STAGE (R.62, change B): a track with a chain, a level ≠ 1
/// or a blend mode ≠ Normal enters the plan as ONE nested layer —
/// the composite of its events (transitions, overlaps, fades, event levels
/// inside), then the track chain, then alpha × level and blending down
/// with the chosen mode. A track without this is flattened into layers as before,
/// byte for byte (GOLDEN_VIDEO relies on this).
#[derive(Debug, Clone)]
struct VideoTrackStage {
    id: kt_model::TrackId,
    lane: u32,
    fx: Vec<kt_model::FxInstance>,
    opacity: f64,
    blend: kt_model::BlendMode,
    /// The track's masks (shape-masks): static, canvas space, after the track's chain.
    masks: Vec<kt_model::Mask>,
    /// Revision of the stage: the track's contents, its chain and properties. The
    /// memoization key of the sub-composite — without it an edit inside would return
    /// a stale result (the same problem as with the reel, item 409).
    content: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct VideoClip {
    /// Track MUTED (mute, or not in the solo set, R.62): the layer does not go
    /// into the composite, but the PREFETCH ORDER remains — lifting mute otherwise
    /// would give a black frame until warmed up (invariant 4 about order == read
    /// still holds: we warm exactly what we will read once the track is restored).
    silent: bool,
    id: kt_model::ClipId,
    start: Tick,
    end: Tick,
    source_in: Tick,
    lane: u32,
    media: MediaId,
    reversed: bool,
    fx: Vec<kt_model::FxInstance>,
    /// Compiled automation of the chain (R.30; compilation — R.59,
    /// item 467). `None` — there are no live curves, and then the layer's chain
    /// is BORROWED: a clip without curves pays neither an allocation nor a single
    /// string comparison. The post-FX hash is still computed from the RAW
    /// curves, so the cache keys have not moved.
    auto_plan: Option<kt_model::AutoPlan>,
    /// Post-FX cache key (item 2 of round 7): chain parameters + clip geometry +
    /// grid step. Any model edit recreates VideoRenderer → the hash
    /// is recomputed, stale frames are not read (their key will not match).
    fx_hash: u64,
    /// Dissolve windows from transitions (R.24, item 137), ticks [w0, w1): a clip is
    /// incoming (ramping up) / outgoing (fading out). The pair blends linearly.
    fade_in: Option<(Tick, Tick)>,
    fade_out: Option<(Tick, Tick)>,
    /// Transition from the INCOMING side (the pair is computed in its layer) and
    /// a solo fade — R.33 C4. The kind is resolved BY TIME (R.34 K4:
    /// the kind's parameters have curves), so what is stored here is not a finished
    /// `TransMix`, but the kind with its curves and the window start.
    fade_in_trans: Option<TransAnim>,
    fade_out_trans: Option<TransAnim>,
    /// Fade (R.25): opposite the window there is emptiness (track edge or Gap), so there is no pair and
    /// nothing to WAIT for — the layer fades to transparency (the track below shows through,
    /// and there is none — black).
    fade_in_solo: bool,
    fade_out_solo: bool,
    /// Layer opacity 0..4096 (R.28, item 12: the event “level” as in
    /// Vegas). Multiplied by alpha during compositing — under a semi-transparent
    /// clip the track below shows through.
    opacity_k: u32,
    /// “Handles”: material beyond the body edges (ticks) — the source continues
    /// linearly; beyond the material edges the frame is clamped (edge policy).
    head_ext: Tick,
    tail_ext: Tick,
    /// Time stretch (Slowed+Reverb on a VIDEO clip, R.37 item 2).
    /// `None` — the linear path, BYTE-FOR-BYTE as before, is what
    /// GOLDEN_VIDEO relies on.
    warp: Option<VideoWarp>,
    /// MAXIMUM net zoom of the chain over the whole clip duration (R.46,
    /// item 320): how many times the chain needs an input larger than Quality. Computed
    /// once when the renderer is built (automation and camera curves
    /// are sampled on the grid); the scale step is taken for the whole clip (the owner's
    /// decision: one cache key per clip).
    head_zoom: f64,
    /// Native file size, if known. `None` — scale step 1: the decode may not be
    /// inflated blindly (a missing-file placeholder, synthetic).
    native: Option<(u32, u32)>,
    /// Context of the leading “Geometry” node (R.62): the source kind (native ×
    /// SAR) and the allowed conform rule. A reel's kind is deliberately unknown:
    /// the sub-composite is assembled at canvas size, frame = canvas.
    lead: Option<kt_fx::conform::LeadCtx>,
    /// Crop of the leading node (source fractions) — it is part of the decode size: the frame
    /// is stretched back to the full frame, the decoder returns the whole frame.
    crop: [f64; 4],
    /// Nested reel (R.50, wave B): instead of a frame from the cache the layer gets
    /// a SUB-COMPOSITE — and the chain hung on this event is applied to the
    /// ASSEMBLED picture, not to each piece. That is what all this was for.
    /// The key is a PAIR (reel, revision of the INSTANCE): since R.59 two
    /// instances of one reel can have different overrides, and that is
    /// different content. An empty set gives `content == content_hash`,
    /// so instances without overrides still share ONE
    /// sub-renderer (t442/t443 still hold).
    nest: Option<NestKey>,
    /// Reel revision hash: an edit INSIDE touches no instance field, so without it the memo
    /// would return stale data. Since R.59 it is a function of the PAIR
    /// (reel, set of overrides).
    content: u64,
    /// Sub-composite scale step — the MAXIMUM over all instances of this reel in
    /// the tree (R.50, B5). A property of the REEL, not of an instance: if they diverged,
    /// the inner clip would have two cache keys and two orders.
    nest_scale: u32,
    /// The reel's OWN fps (R.62): the instance's inner time is quantized to its
    /// grid (the frame is held). `None` — continuous, as in R.56.
    nest_fps: Option<u32>,
    /// PROCEDURAL source (R.56, item 448): the media has no grid of its own,
    /// the frame is a pure function of the tick (generators, titles, placeholder). Such a
    /// clip is not floored to the grid: under stretching it must move
    /// CONTINUOUSLY, otherwise a slowed title would jerk frame by frame.
    procedural: bool,
    /// END OF MATERIAL of the source, if known (R.63 item 2). Beyond it
    /// no frames exist, and asking for them is pointless: an event longer than the file
    /// must HOLD the last frame — just as the start of the file holds the first one.
    /// The order is then equal to the read
    /// (invariant 4): we ask for the last existing frame, not a non-existent one
    /// that the decoder would not return anyway.
    material_end: Option<Tick>,
    /// A STREAM source stage (datamosh-codec-path): the slot's kernel runs on the source in
    /// `layer_src`, and the order asks the stream it names. `None` — no stage (never on a reel).
    stream: Option<Box<StreamStage>>,
    /// The SOURCE'S revision for a stream's addresses: a generator's values live on the event,
    /// not in its address, so an edit of them must reach the codec memo's keys. 0 for a file.
    src_rev: u64,
    /// The event's masks mapped onto the materialized chain (shape-masks). `None` — none.
    masks: Option<Arc<ClipMaskPlan>>,
}

/// Where the masks of an event stand in its MATERIALIZED chain (shape-masks).
#[derive(Debug)]
struct ClipMaskPlan {
    /// The picture's masks: an alpha point right after the leading Geometry node.
    picture: Option<(usize, Vec<kt_model::Mask>)>,
    /// The masks of an active slot: (its index — the dry snapshot; masks; after the node?).
    slots: Vec<(usize, Vec<kt_model::Mask>, bool)>,
}

impl ClipMaskPlan {
    /// The event's mask list at chain positions; `None` for an event without masks.
    fn of(c: &kt_model::ClipEvent) -> Option<Arc<ClipMaskPlan>> {
        if c.kind != ClipKind::Video || c.masks.is_empty() {
            return None;
        }
        // the SAME counting as `materialize_lead`: active slots, then the node at `pos`
        let active: Vec<usize> = if c.fx_off {
            Vec::new()
        } else {
            (0..c.fx.len())
                .filter(|&i| !c.fx_bypass.get(i).copied().unwrap_or(false) && !c.fx[i].is_missing())
                .collect()
        };
        let want = c.geometry_pos.min(c.fx.len());
        let pos = (0..want).filter(|&i| !c.fx_bypass.get(i).copied().unwrap_or(false)).count();
        let pos = pos.min(active.len());
        let mut slots = Vec::new();
        for (a, &raw) in active.iter().enumerate() {
            let list = c.masks.slot(raw);
            if list.is_empty() {
                continue;
            }
            let m = if a >= pos { a + 1 } else { a };
            slots.push((m, list.to_vec(), m > pos));
        }
        let picture = (!c.masks.picture.is_empty()).then(|| (pos + 1, c.masks.picture.clone()));
        if picture.is_none() && slots.is_empty() {
            return None;
        }
        Some(Arc::new(ClipMaskPlan { picture, slots }))
    }

    /// The application points at the material tick `mat`.
    fn ops(&self, mat: Tick, project_h: f64) -> Vec<kt_fx::mask::MaskOp> {
        let mut out = Vec::new();
        for (from, list, follow) in &self.slots {
            out.push(kt_fx::mask::MaskOp {
                at: from + 1,
                kind: kt_fx::mask::MaskOpKind::Mix { from: *from },
                masks: Arc::new(kt_model::masks_at(list, mat)),
                follow_lead: *follow,
                project_h,
            });
        }
        if let Some((at, list)) = &self.picture {
            out.push(kt_fx::mask::MaskOp {
                at: *at,
                kind: kt_fx::mask::MaskOpKind::Alpha,
                masks: Arc::new(kt_model::masks_at(list, mat)),
                follow_lead: true,
                project_h,
            });
        }
        out
    }

    /// A mask point at position 0 forbids a decode step (a leading stop, like the morpher's).
    fn stops_at_zero(&self) -> bool {
        self.slots.iter().any(|(from, _, _)| *from == 0)
    }
}

// The clip's warp map moved to kt-model (R.39): one constructor for
// all — render, preview and the clip-cut rule take it from there, so
// “which frame to show” and “where we cut” can no longer diverge.
use kt_model::ClipWarp as VideoWarp;

/// A transition bound to a clip: its kind, its curves and the window start. The kind
/// is resolved at EVERY moment — otherwise the parameter animation would not reach
/// the picture (R.34 K4).
#[derive(Debug, Clone)]
struct TransAnim {
    kind: kt_model::TransitionKind,
    auto: Vec<kt_model::ParamCurve>,
    w0: Tick,
    /// Transition inversion (R.51, item 398). Executed by the PLAN, not the kernel:
    /// `apply_invert` swaps the sides and reverses the progress.
    invert: bool,
}

impl TransAnim {
    fn of(t: &kt_model::TransitionEvent, w0: Tick) -> TransAnim {
        TransAnim { kind: t.kind.clone(), auto: t.auto.clone(), w0, invert: t.invert }
    }

    fn mix_at(&self, t: Tick) -> crate::TransMix {
        if self.auto.is_empty() {
            return crate::TransMix::of(&self.kind);
        }
        let mut k = self.kind.clone();
        for c in &self.auto {
            if !c.keys.is_empty() {
                k.set_param(&c.param, kt_model::curve_value(&c.keys, Tick(t.0 - self.w0.0)));
            }
        }
        crate::TransMix::of(&k)
    }
}

impl VideoClip {
    /// Whether the clip is active at moment t, taking the transition handles into account.
    fn active_at(&self, t: Tick) -> bool {
        self.start.0 - self.head_ext.0 <= t.0 && t.0 < self.end.0 + self.tail_ext.0
    }
}

/// Hash of the CONTENT of a nested reel, recursive (R.50, wave B).
///
/// The promise “an edit re-creates the renderer, stale frames are not
/// read” is FALSE for a nested reel: an edit INSIDE the reel touches no field of
/// the instance clip, and its `fx_hash` stays the same. This hash is woven in
/// here and closes the gap. The separation is clean:
/// `source` distinguishes REELS, `fx_hash` — INSTANCES, `content_hash` —
/// REVISIONS.
///
/// Computed with the Debug representation — the same trick as `fx_chain_hash`:
/// f64 is printed shortest-roundtrip, so it is deterministic, and a new model field
/// gets into the hash BY ITSELF.
/// Sub-renderer address: the REEL plus the REVISION of the instance (R.59, stage 3).
pub type NestKey = (MediaId, u64);

/// Reel revision as THIS instance sees it (R.59, stage 3).
///
/// An empty set of overrides must give EXACTLY `content_hash`:
/// otherwise all memo keys, sidecar previews and GPU plans would shift
/// for projects that do not use publishing at all.
pub fn instance_content(project: &Project, seq: MediaId, overrides: &[kt_model::Override]) -> u64 {
    let base = content_hash(project, seq);
    if overrides.is_empty() {
        return base;
    }
    // Using the same trick as `content_hash` itself: the Debug representation
    // is deterministic (f64 is printed shortest-roundtrip), and a new override
    // field gets into the hash BY ITSELF.
    xxhash_rust::xxh3::xxh3_64(
        &[base.to_le_bytes().as_slice(), format!("{overrides:?}").as_bytes()].concat(),
    )
}

pub fn content_hash(project: &Project, seq: MediaId) -> u64 {
    let key = (project as *const Project as usize, seq);
    if let Some(h) = REVISIONS.with(|m| m.borrow().as_ref().and_then(|m| m.get(&key).copied())) {
        return h;
    }
    let mut memo = std::collections::HashMap::new();
    let h = hash_reel(project, seq, 0, &mut memo);
    REVISIONS.with(|m| {
        if let Some(m) = m.borrow_mut().as_mut() {
            m.insert(key, h);
        }
    });
    h
}

thread_local! {
    /// The revisions computed during one build (builder-cost-guard): the builders ask the content
    /// of a reel per placement — in the clip list, the sub-renderer key, the decode step, the
    /// generator sources — and each asked the whole reel's Debug text again, placements × events.
    /// Keyed by the project's address, alive only inside [`revisions_scope`], where the project
    /// is borrowed and cannot change.
    static REVISIONS: std::cell::RefCell<Option<std::collections::HashMap<(usize, MediaId), u64>>> =
        const { std::cell::RefCell::new(None) };
}

/// Runs a build with the reel revisions memoized (see `REVISIONS`). Nested scopes share the
/// outer one's memo.
pub fn revisions_scope<R>(f: impl FnOnce() -> R) -> R {
    let opened = REVISIONS.with(|m| {
        let mut m = m.borrow_mut();
        let open = m.is_none();
        if open {
            *m = Some(std::collections::HashMap::new());
        }
        open
    });
    // the reels resolved in this build, once per instance (edit-without-rebuild, row 155)
    let resolved = crate::incremental::open_resolved();
    struct Close(bool, bool);
    impl Drop for Close {
        fn drop(&mut self) {
            if self.0 {
                REVISIONS.with(|m| *m.borrow_mut() = None);
            }
            if self.1 {
                crate::incremental::close_resolved();
            }
        }
    }
    let _close = Close(opened, resolved);
    f()
}

/// One reel's revision. A nested reel enters by ITS revision, computed once per reel
/// (builder-cost-guard): the text of every placement's reel fed again made an outer reel of 400
/// placements hash the inner one 400 times, and every level of nesting multiplied it.
fn hash_reel(
    project: &Project,
    seq: MediaId,
    depth: usize,
    memo: &mut std::collections::HashMap<MediaId, u64>,
) -> u64 {
    use xxhash_rust::xxh3::Xxh3;
    let mut h = Xxh3::new();
    let Some(s) = project.sequence(seq) else { return h.digest() };
    if depth > kt_model::MAX_NEST_DEPTH {
        return h.digest();
    }
    kt_time::cost::add(kt_time::cost::Cost::ReelHash, s.clips.len() as u64);
    h.update(&[s.overlap_layers as u8]);
    // TRACKS of the reel (R.62) — the whole revision: the row order, switches,
    // level, blend mode and track chain change the picture and the sound
    h.update(format!("{:?}", s.tracks).as_bytes());
    h.update(format!("{:?}", s.clips).as_bytes());
    h.update(format!("{:?}", s.transitions).as_bytes());
    h.update(format!("{:?}", s.morphers).as_bytes());
    h.update(format!("{:?}", s.cameras).as_bytes());
    // the declared set of handles is also the CONTENT of the reel: narrowing a limit
    // changes what is drawn for an instance with overrides
    h.update(format!("{:?}", s.published).as_bytes());
    // its own raster and fps (R.62) — a revision: another canvas or another grid —
    // another frame; the memo keys and reel previews must see this
    h.update(format!("{:?}{:?}", s.raster, s.fps).as_bytes());
    for c in &s.clips {
        if project.sequence(c.media).is_some() {
            let inner = match memo.get(&c.media) {
                Some(&v) => v,
                None => {
                    let v = hash_reel(project, c.media, depth + 1, memo);
                    memo.insert(c.media, v);
                    v
                }
            };
            h.update(&inner.to_le_bytes());
        }
    }
    h.digest()
}

/// Chain hash for the post-FX cache. Input — the Debug representation of the parameters
/// (deterministic, f64 printed shortest-roundtrip) + geometry:
/// start/span/source_in/reversed define the mapping t→(src_pts, u),
/// frame_step — the grid (a change of project fps must invalidate).
#[allow(clippy::too_many_arguments)]
fn fx_chain_hash(
    fx: &[kt_model::FxInstance],
    auto: &[Vec<kt_model::ParamCurve>],
    start: Tick,
    end: Tick,
    source_in: Tick,
    reversed: bool,
    frame_step: i64,
    content: u64,
) -> u64 {
    // automation curves are part of the key (R.30): an edit of a keyframe must
    // invalidate post-FX frames that survived a renderer re-creation.
    // `content` (R.50) — the revision of a NESTED reel: an edit inside touches
    // no field of the instance, and without it the post-FX frame of the instance would
    // stay stale. For an ordinary clip this is zero, and the key is unchanged.
    let repr = format!(
        "{fx:?}|{auto:?}|{}|{}|{}|{reversed}|{frame_step}|{content}",
        start.0,
        end.0 - start.0,
        source_in.0
    );
    xxhash_rust::xxh3::xxh3_64(repr.as_bytes())
}

/// Source pts for the timeline moment `t` inside the clip: direct or
/// mirrored (item 16, reverse). A pure function — Test 24.
///
/// **The grid floor is applied by a LEAF, and ONLY a FILE one** (R.56, item 448). A file
/// stores frames on its own grid: asking for something between them is pointless, the decoder
/// would return the nearest anyway, and two keys for one frame are a double cache cost.
/// A procedural source (generator, title, placeholder) has no frame “between” —
/// it is computed at any tick, and a floor there would only chop the
/// motion into steps. A REEL has no floor at all: its inner time is
/// editing time, not a frame index (`clip_inner_tick`).
fn clip_source_pts(c: &VideoClip, t: Tick, grid: &FpsGrid) -> Tick {
    let m = clip_material_pos(c, t, grid);
    let m = match c.material_end {
        // beyond the end of the material we hold the LAST frame (R.63 item 2): asking for
        // a non-existent one means getting an empty layer and a permanent miss
        Some(end) if end.0 > 0 && m >= end => Tick(end.0 - 1),
        _ => m,
    };
    if c.procedural { m } else { grid.align_floor(m) }
}

/// CONTINUOUS inner time of a nesting (R.56, item 448): the reel moment at
/// which the instance plays. There is no floor here by construction — a reel has no
/// decoded frames, there is nothing to round, and rounding BREAKS: at
/// rate 0.5 two adjacent output frames got the same `inner_t`,
/// the sub-composite memo returned the same entry, and everything procedural inside
/// (titles, animation, automation, cameras) moved in two-frame steps.
fn clip_inner_tick(c: &VideoClip, t: Tick, grid: &FpsGrid) -> Tick {
    let m = clip_material_pos(c, t, grid);
    match c.nest_fps {
        // The reel's OWN fps (R.62): its time is quantized to its grid — the frame
        // is held until the next one, without blending; these steps are intentional
        Some(f) if f > 0 => {
            let step = kt_time::TICK_HZ / f as i64;
            Tick(m.0.div_euclid(step) * step)
        }
        _ => m,
    }
}

/// Whether this instance blends frames (R.56, item 448). It is inherited DOWNWARD: on
/// the reel itself there is nothing to blend (the sub-composite is a function of continuous
/// time and is already exact), but its LEAVES must blend their own pairs,
/// otherwise footage inside a slowed composition stays stepped.
fn nest_blend(c: &VideoClip, inherited: bool) -> bool {
    inherited || c.warp.as_ref().is_some_and(|w| w.blend())
}

/// The material position of moment `t` WITHOUT grid alignment — automation keys
/// live in it (R.33). One rule for the engine, the model and the UI
/// (`kt_model::clip_material_at`): reverse mirror, edge clamp — also there.
fn clip_material_pos(c: &VideoClip, t: Tick, grid: &FpsGrid) -> Tick {
    let span = (c.end.0 - c.start.0).max(1);
    let Some(w) = &c.warp else {
        return kt_model::clip_material_at(
            c.source_in,
            c.start,
            Tick(span),
            c.reversed,
            grid.frame_step(),
            t,
        );
    };
    // Stretching (R.37 item 2): material follows the integral of the speed. Beyond the body
    // (transition handles) tau leaves [0,1] — the map continues with the edge
    // segment, exactly as chain_source_window_ext does for audio.
    let tau = (t.0 - c.start.0) as f64 / span as f64;
    let consumed = w.material_ticks(span, tau);
    if c.reversed {
        // WARP, THEN MIRROR — not the other way round. The audio path (`fill_source`)
        // mirrors the ALREADY consumed forward stream; a mirror before the warp would run
        // the speed curve backwards and desynchronize A/V. The anchor is
        // the SPAN of the position (R.57), not “how much was eaten”: with a signed
        // speed the curve can go back and return, eating twice
        // more than it passed. On positive curves the span equals
        // (0, used), and the expression collapses to the previous one.
        let (smin, smax) = w.span();
        // Two clamps, and both are needed: the lower edge of the SPAN (the mirror does not
        // go further) and the START OF THE FILE — with a signed speed the span drifts past
        // the entry point and may go left of zero (R.57). There is no frame there for any
        // source, so we hold the edge one.
        Tick(
            (c.source_in.0 + smin + smax - grid.frame_step() - consumed)
                .max(c.source_in.0 + smin)
                .max(0),
        )
    } else {
        Tick((c.source_in.0 + consumed).max(0))
    }
}

pub struct VideoRenderer {
    clips: Vec<VideoClip>,
    pub grid: FpsGrid,
    /// Snapshot of the project cameras (R.29): the active/pinned Pan3d camera
    /// is resolved when the plan is built, by the same rule as in the model.
    cameras: Vec<kt_model::CameraEvent>,
    /// Project size in px — the world coordinate system of 3D effects.
    project_size: (u32, u32),
    /// Renderers of nested reels (R.50). The tree is built ONCE together with
    /// the root: an edit of the model re-creates the renderer whole, and stale nodes
    /// do not exist by construction.
    nests: std::collections::HashMap<NestKey, VideoRenderer>,
    /// Video track stages (R.62). Empty — not a single extra branch in the plan.
    stages: Vec<VideoTrackStage>,
    /// The memo of the stream stages (datamosh-codec-path, D3).
    stream_memo: Arc<kt_plugin_api::stream::StreamMemo>,
    /// How a live preview paces its stream stages (decode-request-storm); `None` — everything at
    /// once, as an export and the tools ask.
    stream_pacing: Option<StreamPacing>,
    /// The last plan showed a stream layer's held (late) frame (`StreamPacing::hold`).
    stream_held: std::sync::atomic::AtomicBool,
    /// `StreamPacing::hold` as the player last set it: an atomic, because the preview's thread
    /// composes on a renderer shared with the UI thread (ui-never-waits).
    stream_hold: std::sync::atomic::AtomicBool,
}

/// Source frame(s) for moment `t`: the main one and, in “blend” mode,
/// a neighbour with a weight (R.37 C5). The ONLY place where the pair is decided —
/// `layers_at` and `layer_src` must call exactly this, otherwise the prefetch warms
/// not what the composite reads, and that is a permanent cache miss.
///
/// `k == 0` (position exactly on the grid: integer speeds, rate = 1, clamp at the
/// edge) gives NO pair — the path stays byte-for-byte as before.
///
/// `blend_footage` — blending INHERITED from a nesting above (R.56,
/// item 448): the mode sits on the reel instance, and its leaves do the blending.
/// The reel itself never has a pair: two sub-composites per frame cost twice as much
/// on both backends and add nothing — its time is already
/// continuous.
fn clip_frame_keys(
    c: &VideoClip,
    t: Tick,
    grid: &FpsGrid,
    blend_footage: bool,
) -> (Tick, Option<(Tick, u32)>) {
    if c.nest.is_some() {
        return (clip_inner_tick(c, t, grid), None);
    }
    let a = clip_source_pts(c, t, grid);
    if !(blend_footage || c.warp.as_ref().is_some_and(|w| w.blend())) {
        return (a, None);
    }
    let m = clip_material_pos(c, t, grid);
    let step = grid.frame_step().max(1);
    let k = (((m.0 - a.0) * 4096) / step).clamp(0, 4096) as u32;
    // the neighbour is FORWARD by material index; with reversed, the material runs
    // toward it, but the fractional part is still measured forward
    if k == 0 { (a, None) } else { (a, Some((Tick(a.0 + step), k))) }
}

/// Frame key in the cache: idx = source pts in ticks (aligned to the project grid).
pub fn video_cache_idx(source_pts: Tick) -> i64 {
    source_pts.0
}

/// Decode scale step of the clip (R.46, item 320): by how many WHOLE times the pre-FX frame
/// is taken larger than Quality. A pure function of the clip and Quality —
/// deterministic, does NOT depend on the device or budgets (otherwise export
/// would depend on the machine).
///
/// Three factors: how much the chain asks for (the clip's `head_zoom`, with the same
/// quantum as the supersampling taps), how much the file can give (the native headroom
/// above Quality, rounded UP — the owner's decision: the remainder is covered by sws at decode)
/// and the ceiling on intermediate buffers.
fn clip_decode_scale(clip: &VideoClip, quality: Quality) -> u32 {
    // base — the conform size (R.62): the rule's frame on the Quality canvas,
    // stretched to the full frame; for a reel — the canvas itself
    let base = clip_base_quality(clip, quality);
    let (qw, qh) = (base.width.max(2), base.height.max(2));
    let cap = (kt_fx::plan::MAX_INTER_DIM / qw.max(qh)).max(1);
    // NESTING (R.50, B5): a reel has no “native” — the sub-composite can be
    // assembled at any size; its headroom is limited by nothing except the ceiling
    // on intermediate buffers. The scale step is taken ONE per reel (the maximum over
    // all its instances): otherwise the inner clip would have two cache keys,
    // two coverage bars and two orders — a permanent miss (t312/t317).
    if clip.nest.is_some() {
        return clip.nest_scale.min(cap).max(1);
    }
    let want = kt_fx::geom::ss_taps(clip.head_zoom);
    // a masked slot at the head of the chain is a leading stop (shape-masks)
    if want <= 1 || clip.masks.as_ref().is_some_and(|m| m.stops_at_zero()) {
        return 1;
    }
    let Some((nw, nh)) = clip.native else { return 1 };
    // file headroom: how many WHOLE Quality fit in the native size, rounded up
    // (native 1920 at Quality 1280 is scale step 2, not 1)
    let spare_w = (nw as f64 / qw as f64).ceil() as u32;
    let spare_h = (nh as f64 / qh as f64).ceil() as u32;
    let spare = spare_w.min(spare_h).max(1);
    want.min(spare).min(cap)
}

/// CHAIN INPUT of the clip (R.58, item 459) — the step that THIS instance's chain
/// is able to consume.
///
/// For a leaf it equals the decode scale step: that is derived from its own
/// chain. For a REEL the step is a property of the reel (the maximum over instances,
/// so that the inner leaf has one cache key, t450), and handing it out as the
/// chain input to EACH instance is not allowed: an empty chain, a single-strength
/// layer, a leading stop (morpher layer) and a chain without resample (Blur/Glow)
/// do not consume steps — an S× frame would never be returned to Quality. Before R.58
/// this crashed the debug build and SILENTLY dropped the nesting from the frame in the
/// release build.
fn clip_chain_scale(clip: &VideoClip, quality: Quality) -> u32 {
    let stage = clip_decode_scale(clip, quality);
    if clip.nest.is_none() {
        return stage;
    }
    // the same quantum with which `nest_scales` computed — otherwise “equal” would never
    // match, and an extra resample would appear out of nowhere
    stage.min(kt_fx::geom::ss_taps(clip.head_zoom).max(1))
}

/// Sub-composite scale step of each reel: the MAXIMUM of the request over all its
/// instances in the TREE (R.50, B5).
///
/// The scale step must be a property of the REEL, not of an instance: the content is
/// shared between them, and different steps would give the inner clip two cache keys, two
/// coverage bars and two prefetch orders — a permanent miss (t312/t317).
/// A pure function of the project: it does not depend on the device or budgets.
fn nest_scales(project: &Project) -> std::collections::HashMap<MediaId, u32> {
    let mut out: std::collections::HashMap<MediaId, u32> = std::collections::HashMap::new();
    let grid = FpsGrid::new(project.settings.fps);
    let size = (project.settings.width, project.settings.height);
    // A WALK OF THE INSTANCE TREE, not a flat scan of reels (R.59, stage
    // 3): the content of a reel now depends on the override set of the instance
    // through which it was entered — and hence the chain zoom inside it too.
    // The scale step still remains ONE per reel (t450): different steps would give
    // the inner clip two cache keys and two orders.
    // The maximum has simply become honest.
    let mut seen: std::collections::HashSet<(MediaId, u64)> = std::collections::HashSet::new();
    let mut stack: Vec<(Option<MediaId>, Vec<kt_model::Override>, usize)> =
        vec![(None, Vec::new(), 0)];
    while let Some((reel, ov, depth)) = stack.pop() {
        let resolved = crate::incremental::resolve(project, reel, &ov);
        for c in &resolved.clips {
            if c.kind != ClipKind::Video || project.sequence(c.media).is_none() {
                continue;
            }
            let fx = c.active_fx();
            let auto = c.active_fx_auto();
            let (fx, auto) = materialize_lead(c, fx, auto);
            let plan = kt_model::AutoPlan::compile(&fx, &auto);
            let lead = Some(
                kt_fx::conform::LeadCtx::new(
                    project
                        .sequence(c.media)
                        .and_then(|s| s.raster)
                        .map(|(w, h)| (w as f64, h as f64)),
                    c.geometry.rule.resolve(project.settings.conform),
                )
                .in_raster(size),
            );
            let zoom = clip_head_zoom(
                &fx,
                plan.as_ref(),
                c.start,
                c.start + c.duration,
                c.source_in,
                c.reversed,
                &grid,
                &resolved.cameras,
                size,
                lead,
            );
            let want = kt_fx::geom::ss_taps(zoom).max(1);
            let e = out.entry(c.media).or_insert(1);
            *e = (*e).max(want);
            if depth < kt_model::MAX_NEST_DEPTH
                && seen.insert((c.media, instance_content(project, c.media, &c.overrides)))
            {
                stack.push((Some(c.media), c.overrides.clone(), depth + 1));
            }
        }
    }
    out
}

/// `a \ b` on sorted sets of intervals.
fn subtract(
    a: Vec<std::ops::Range<i64>>,
    mut b: Vec<std::ops::Range<i64>>,
) -> Vec<std::ops::Range<i64>> {
    if b.is_empty() {
        return a;
    }
    b.sort_by_key(|r| r.start);
    let mut out = Vec::new();
    for r in a {
        let mut cur = r.start;
        for h in &b {
            if h.end <= cur || h.start >= r.end {
                continue;
            }
            if h.start > cur {
                out.push(cur..h.start);
            }
            cur = cur.max(h.end);
            if cur >= r.end {
                break;
            }
        }
        if cur < r.end {
            out.push(cur..r.end);
        }
    }
    out
}

fn shift_runs(v: Vec<std::ops::Range<i64>>, d: i64) -> Vec<std::ops::Range<i64>> {
    v.into_iter().map(|r| (r.start + d)..(r.end + d)).collect()
}

fn scaled(q: Quality, s: u32) -> Quality {
    if s <= 1 { q } else { Quality { width: q.width * s, height: q.height * s } }
}

/// Conform size of the clip (R.62): the size to which the decoder pulls the FULL
/// frame at scale step 1 — the rule's frame on the `quality` canvas, stretched
/// back to the full frame (the crop is cut by the node). For a reel — the canvas.
fn clip_base_quality(clip: &VideoClip, quality: Quality) -> Quality {
    let lead =
        clip.lead.unwrap_or(kt_fx::conform::LeadCtx::new(None, kt_model::ConformRule::Stretch));
    // a reel without its own raster — the parent's canvas (law E); with its own (R.62) —
    // the rule's frame, as for a file: the sub-composite is assembled in it
    if clip.nest.is_some() && lead.view.is_none() {
        return quality;
    }
    let g = kt_fx::conform::conform_geometry_in(
        lead.view,
        (quality.width, quality.height),
        lead.rule,
        clip.crop,
        lead.raster,
    );
    Quality { width: g.decode.0, height: g.decode.1 }
}

/// Decode quality of the clip. The ONLY place of decision: the prefetch order,
/// the composite read and the coverage bars must ask it — otherwise
/// what is warmed is not what is read (t247d along the size axis, t312).
fn clip_decode_quality(clip: &VideoClip, quality: Quality) -> Quality {
    scaled(clip_base_quality(clip, quality), clip_decode_scale(clip, quality))
}

/// The quality the clip's frames are ORDERED at: its decode quality, or the stream's size when a
/// stream stage replaces the source (datamosh-codec-path, D5) — the coverage bars ask this.
fn clip_order_quality(clip: &VideoClip, quality: Quality) -> Quality {
    match (&clip.stream, clip.nest) {
        (Some(st), None) => stream::stream_quality(clip, st, quality),
        _ => clip_decode_quality(clip, quality),
    }
}

/// The leading “Geometry” node goes into the engine chain (R.62). The position —
/// `geometry_pos` in the count of ACTIVE slots (bypass slots filtered out);
/// curves (`auto`) get an empty slot at the same position, so that indices
/// stay parallel; `LayerMix.from`, pointing at a slot after the
/// insertion point, is shifted by one. An audio event has no such node.
fn materialize_lead(
    c: &kt_model::ClipEvent,
    mut fx: Vec<kt_model::FxInstance>,
    mut auto: Vec<Vec<kt_model::ParamCurve>>,
) -> (Vec<kt_model::FxInstance>, Vec<Vec<kt_model::ParamCurve>>) {
    if c.kind != ClipKind::Video {
        return (fx, auto);
    }
    let want = c.geometry_pos.min(c.fx.len());
    let pos = (0..want).filter(|&i| !c.fx_bypass.get(i).copied().unwrap_or(false)).count();
    let pos = pos.min(fx.len());
    for f in fx.iter_mut() {
        if let kt_model::FxInstance::LayerMix { from, .. } = f
            && (*from as usize) >= pos
        {
            *from += 1;
        }
    }
    fx.insert(pos, c.geometry.node());
    if auto.len() >= pos {
        auto.insert(pos, c.geometry_auto.clone());
    } else if !c.geometry_auto.is_empty() {
        auto.resize(pos, Vec::new());
        auto.push(c.geometry_auto.clone());
    }
    (fx, auto)
}

/// Maximum net zoom of the chain over the clip duration (R.46, item 320).
/// Curves are sampled along the LINEAR material axis — the same one by which
/// `layer_src` reads them; cameras — along the timeline. Points: a uniform grid,
/// both ends and all curve keys (the zoom extremum almost always sits on a key).
/// The logical base is the PROJECT size: zoom barely depends on Quality
/// (differences from integer rounding of the grid are smaller than the step quantum), and
/// the step must be the same in preview and export.
#[allow(clippy::too_many_arguments)]
fn clip_head_zoom(
    fx: &[kt_model::FxInstance],
    plan: Option<&kt_model::AutoPlan>,
    start: Tick,
    end: Tick,
    source_in: Tick,
    reversed: bool,
    grid: &FpsGrid,
    cameras: &[kt_model::CameraEvent],
    project_size: (u32, u32),
    lead: Option<kt_fx::conform::LeadCtx>,
) -> f64 {
    let (pw, ph) = (project_size.0.max(2), project_size.1.max(2));
    let live = plan.is_some();
    // Identity is checked on STATIC values: a “Geometry” node with
    // zoom 1 and a curve 1 → 4 is statically identical, yet by the end of the clip
    // it requests step 4 (t311). With a live plan an early exit is impossible —
    // before R.62 it was saved by Pan, which was never counted as identity.
    if !live && !kt_fx::video::has_video_fx_in(fx, lead, pw, ph) {
        return 1.0;
    }
    let has_3d = fx.iter().any(kt_model::FxInstance::uses_camera);
    let span = (end.0 - start.0).max(1);
    let mut probes: Vec<Tick> = Vec::new();
    if live || has_3d || !cameras.is_empty() {
        const GRID: i64 = 32;
        for i in 0..=GRID {
            probes.push(Tick(start.0 + span * i / GRID));
        }
        // curve kinks are also probes: between two keys the zoom maximum
        // may sit exactly on a key
        for k_t in plan.into_iter().flat_map(|p| p.key_ticks()) {
            // the key lives on the MATERIAL axis — convert to the timeline
            let t = kt_model::clip_timeline_at(
                source_in,
                start,
                Tick(span),
                reversed,
                grid.frame_step(),
                k_t,
            );
            if start.0 <= t.0 && t.0 <= end.0 {
                probes.push(t);
            }
        }
        for cam in cameras {
            for t in [cam.start, Tick(cam.start.0 + cam.duration.0)] {
                if start.0 <= t.0 && t.0 <= end.0 {
                    probes.push(t);
                }
            }
        }
    } else {
        // static: the zoom is the same over the whole duration, the middle is enough
        probes.push(Tick(start.0 + span / 2));
    }
    let mut best = 1.0f64;
    let mut owned = fx.to_vec();
    for t in probes {
        let mat = kt_model::clip_material_at(
            source_in,
            start,
            Tick(span),
            reversed,
            grid.frame_step(),
            t,
        );
        if let Some(plan) = plan {
            owned.clone_from_slice(fx);
            plan.apply(&mut owned, mat);
        }
        let cam3d = has_3d.then(|| {
            let ev = kt_model::active_camera_in(cameras, t);
            let cam = ev.map(|e| e.params_at(t)).unwrap_or_default();
            // the logic is computed at the project size — world_scale is exactly 1
            Cam3d { cam, world_scale: 1.0 }
        });
        let u = (t.0 - start.0) as f64 / span as f64;
        best = best.max(kt_fx::plan::chain_head_zoom_in(&owned, pw, ph, u, cam3d, lead));
    }
    best
}

/// What the renderer knows about OPENED sources (R.46 item 321, R.56 item 448).
///
/// The plan knows about media exactly as `MediaId` (law P): WHAT lies at an address is a
/// property of the SOURCE, and the renderer learns this only from whoever opened the
/// source. Hence the shape: not a path and not a `MediaRef`, but the two facts the plan
/// really needs — the headroom of detail (native) and the presence of its own
/// grid of frames.
#[derive(Debug, Clone, Default)]
pub struct SourceFacts {
    /// Native size of the video source: headroom for the decode scale step.
    pub natives: std::collections::HashMap<MediaId, (u32, u32)>,
    /// Sources WITHOUT a grid of their own (R.56): the frame is a pure function of
    /// the tick. They are not floored to the grid.
    pub procedural: std::collections::HashSet<MediaId>,
    /// The kind of the source frame (native × pixel aspect ratio, R.62) — the input of
    /// the conform rule. Computed through the SAME channel as the natives.
    pub views: std::collections::HashMap<MediaId, (f64, f64)>,
    /// MATERIAL LENGTH (R.63 item 2) — through the same channel and for the same reason:
    /// only whoever opened the source knows how much material there is.
    /// Its absence is not zero, but “no edge” (generator, placeholder).
    pub lengths: std::collections::HashMap<MediaId, kt_time::Tick>,
}

impl SourceFacts {
    /// Remember the facts of an opened source. The ONLY place where it is decided
    /// whether it is “procedural”: if the player, export and kt-cli diverged on this question,
    /// the order and the read would compute different keys (invariant 4).
    pub fn observe(&mut self, media: MediaId, info: &kt_media::MediaInfo) {
        if info.duration.0 > 0 {
            self.lengths.insert(media, info.duration);
        } else {
            self.lengths.remove(&media);
        }
        // TWO facts, decided here and nowhere else (still-image-frame-rule, D1): "no grid of
        // its own" (procedural, below — the timing) and "no proportions of its own" (shapeless —
        // the frame rule). A generator, a title and the placeholder draw their frame at the
        // requested quality: shapeless. A single PICTURE has no grid but has proportions — R.67
        // gave it `fps: None`, and the renderer's "procedural ⇒ no view" branch silently stretched
        // a square still over a 16:9 canvas under every rule (ktprompts 26.10.18 item 9).
        let shapeless = info.has_video && info.fps.is_none() && !info.still;
        if info.has_video {
            self.natives.insert(media, (info.width, info.height));
        } else {
            self.natives.remove(&media);
        }
        if info.has_video && !shapeless {
            self.views.insert(media, info.view_size());
        } else {
            self.views.remove(&media);
        }
        // “No grid of its own” is exactly the definition of a procedural
        // source: a file takes its fps from the container, while a generator,
        // title or placeholder has none, and the frame is computed at any
        // requested tick.
        if info.has_video && info.fps.is_none() {
            self.procedural.insert(media);
        } else {
            self.procedural.remove(&media);
        }
    }

    pub fn forget(&mut self, media: MediaId) {
        self.natives.remove(&media);
        self.procedural.remove(&media);
        self.views.remove(&media);
    }

    /// The kind of the source frame for the frame rule, or `None` — a source drawn at the
    /// requested quality, or one not seen. The ONE reader: the renderer's lead, the on-screen
    /// geometry frame (`Player::source_view`) and `kt-cli probe` all ask here.
    pub fn view_of(&self, media: MediaId) -> Option<(f64, f64)> {
        self.views.get(&media).copied()
    }
}

/// The kind of the frame the frame rule places for event `c` whose source address is `addr`: a
/// reel instance has its reel's own raster (law E: without one the frame equals the canvas);
/// any other event — the source's view (`SourceFacts::view_of`). One function for the renderer
/// and `kt-cli probe`, so the probe prints the frame the render uses (still-image-frame-rule, D2).
pub fn lead_view(
    project: &Project,
    facts: &SourceFacts,
    c: &kt_model::ClipEvent,
    addr: MediaId,
) -> Option<(f64, f64)> {
    match project.sequence(c.media) {
        Some(s) => s.raster.map(|(w, h)| (w as f64, h as f64)),
        None => facts.view_of(addr),
    }
}

impl VideoRenderer {
    pub fn new(project: &Project) -> VideoRenderer {
        VideoRenderer::new_with_facts(project, &SourceFacts::default())
    }

    /// A renderer that knows the NATIVE sizes of the sources (R.46, item 320): without
    /// them the decode scale step is always 1 — the decode may not be inflated without knowing the
    /// file's headroom.
    pub fn new_with_natives(
        project: &Project,
        natives: &std::collections::HashMap<MediaId, (u32, u32)>,
    ) -> VideoRenderer {
        VideoRenderer::new_with_facts(
            project,
            &SourceFacts { natives: natives.clone(), ..Default::default() },
        )
    }

    /// A renderer that knows the FACTS about opened sources (R.56): the native size and
    /// procedurality. The player, export and kt-cli call this constructor —
    /// otherwise a slowed title stays stepped.
    /// The renderer's derived state as text (a test's comparison of an incremental build with a
    /// fresh one, t2004): every layer, the cameras, the stages and the nested reels.
    #[doc(hidden)]
    pub fn state_text(&self) -> String {
        let mut nests: Vec<_> = self.nests.iter().collect();
        nests.sort_by_key(|(k, _)| **k);
        let nests: Vec<String> = nests.iter().map(|(k, r)| format!("{k:?}: {}", r.state_text())).collect();
        format!(
            "{:?}\n{:?} {:?} {:?}\n{:?}\n{nests:?}",
            self.clips, self.grid, self.cameras, self.project_size, self.stages
        )
    }

    pub fn new_with_facts(project: &Project, facts: &SourceFacts) -> VideoRenderer {
        crate::incremental::count_renderer_build();
        revisions_scope(|| VideoRenderer::build(project, facts))
    }

    fn build(project: &Project, facts: &SourceFacts) -> VideoRenderer {
        VideoRenderer::build_reel(project, facts, &nest_scales(project), None, &[], 0)
    }

    fn build_reel(
        project: &Project,
        facts: &SourceFacts,
        nest_scales: &std::collections::HashMap<MediaId, u32>,
        seq: Option<MediaId>,
        overrides: &[kt_model::Override],
        depth: usize,
    ) -> VideoRenderer {
        // morpher layers are applied here, the engine does not see them (H1); nestings
        // are NOT expanded — each has its own plan node (R.50, wave B).
        // The overrides of an INSTANCE are also resolved by the model (R.59, stage
        // 3): the engine knows nothing about publishing at all, it gets ready-made
        // clips. An empty set goes the byte-for-byte prior path.
        let resolved = crate::incremental::resolve(project, seq, overrides);
        let effective = &resolved.clips;
        let grid = FpsGrid::new(project.settings.fps);
        // Cameras are OWN to each reel, and `head_zoom` must be computed
        // by them too: `nest_scales` selects cameras exactly this way, and the reel's step
        // and the instance's chain input are compared with each other
        // (`clip_chain_scale`). Different camera sets would pull them apart
        // on their own, on Pan3d inside a reel with its own cameras.
        let cams: &[kt_model::CameraEvent] = &resolved.cameras;
        // The LOGICAL raster of this composite, in which "as is" counts its pixels: the reel's
        // own raster, else the project's (still-image-frame-rule). A reel without its own raster
        // nested in a reel that has one is taken at the project's raster, as `nest_scales` does.
        let raster = seq
            .and_then(|id| project.sequence(id))
            .and_then(|s| s.raster)
            .unwrap_or((project.settings.width, project.settings.height));
        // SALT of the source address (R.59, stage 3) — one for the whole reel
        // instance. Computed by the SAME code as in `gen_sources`:
        // the order and the read must ask for one address (invariant 4).
        let salt = gen_salt(
            match seq.and_then(|id| project.sequence(id)) {
                Some(s) => &s.published,
                None => &project.published,
            },
            overrides,
        );
        // VIDEO TRACK MUTING (R.62): mute or the solo set of ITS OWN
        // kind. The track's level and blend mode are a plan stage (it
        // arrives separately), and muting a layer does not require the stage.
        let tracks: &[kt_model::Track] = match seq.and_then(|id| project.sequence(id)) {
            Some(s) => &s.tracks,
            None => &project.tracks,
        };
        let solo_on = tracks.iter().any(|t| t.kind == kt_model::LaneKind::Video && t.solo);
        // VIDEO TRACK STAGES (R.62): a chain, level ≠ 1 or mode ≠
        // Normal. The condition is STRICT: without it the track is flattened into layers
        // by the prior path, and the bytes do not move.
        let stages: Vec<VideoTrackStage> = {
            let mut out = Vec::new();
            for (lane, t) in
                tracks.iter().filter(|t| t.kind == kt_model::LaneKind::Video).enumerate()
            {
                let lane = lane as u32;
                if t.needs_video_stage() {
                    let mut h = xxhash_rust::xxh3::Xxh3::new();
                    h.update(format!("{:?}", t.active_fx()).as_bytes());
                    h.update(&t.opacity.to_le_bytes());
                    h.update(&[t.blend as u8]);
                    // masks join the revision only when present: the keys of maskless
                    // stages do not move
                    if !t.masks.is_empty() {
                        h.update(format!("{:?}", t.masks).as_bytes());
                    }
                    out.push(VideoTrackStage {
                        id: t.id,
                        lane,
                        fx: t.active_fx(),
                        opacity: t.opacity,
                        blend: t.blend,
                        masks: t.masks.clone(),
                        content: h.digest(),
                    });
                }
            }
            out
        };
        let silent_lane = |lane: u32| -> bool {
            kt_model::track_index(tracks, kt_model::LaneKind::Video, lane)
                .map(|i| &tracks[i])
                .is_some_and(|t| t.mute || (solo_on && !t.solo))
        };
        // the cameras enter every layer's decode zoom: hashed once per reel, into each layer's key
        let cams_key = crate::incremental::hash_debug(&cams);
        let mut clips: Vec<VideoClip> = effective
            .iter()
            .filter(|c| c.kind == ClipKind::Video)
            .map(|c| {
                let addr = clip_source_addr_in(c, salt);
                // conform context: the source kind and the event rule allowed by the project.
                // The kind is ONE fact (`lead_view`): a reel's own raster (law E — without one
                // the frame equals the canvas), a source drawn at the requested quality
                // (generator, title, placeholder) has none (frame = canvas, the node is identity,
                // spec generators, R.62), a single picture and a file have their view.
                let procedural = facts.procedural.contains(&addr);
                let view = lead_view(project, facts, c, addr);
                let lead = Some(
                    kt_fx::conform::LeadCtx::new(
                        view,
                        c.geometry.rule.resolve(project.settings.conform),
                    )
                    .in_raster(raster),
                );
                let nest = project
                    .sequence(c.media)
                    .map(|_| (c.media, instance_content(project, c.media, &c.overrides)));
                let nest_fps = nest.and_then(|_| project.sequence(c.media)).and_then(|s| s.fps);
                let nest_scale = nest.and_then(|(id, _)| nest_scales.get(&id).copied()).unwrap_or(1);
                let native = facts.natives.get(&addr).copied();
                // a reel has no material of its own (its frames are computed by the
                // sub-composite); a generator never has edges
                let material_end =
                    (nest.is_none() && !procedural).then(|| facts.lengths.get(&addr).copied()).flatten();
                let silent = silent_lane(c.lane);
                // everything the layer is derived from: the same key → the last build's layer,
                // copied (edit-without-rebuild)
                let key = crate::incremental::hash_fmt(format_args!(
                    "{} {} {procedural} {lead:?} {nest:?} {nest_fps:?} {nest_scale} {native:?} {material_end:?} {silent} {cams_key} {raster:?} {} {} {}",
                    crate::incremental::hash_debug(c),
                    addr.0,
                    project.settings.width,
                    project.settings.height,
                    project.settings.fps,
                ));
                crate::incremental::video_clip(key, || {
                let fx = c.active_fx(); // bypass slots are filtered out (item 2)
                let auto = c.active_fx_auto();
                // The leading “Geometry” node (R.62) is materialized INTO THE CHAIN
                // at position `geometry_pos` (in the count of active slots):
                // the model's slot indices do not move, and the engine's chain is
                // one sequence, as in Vegas.
                let (fx, auto) = materialize_lead(c, fx, auto);
                let stream = StreamStage::of(&fx).map(Box::new);
                let warp = kt_model::clip_warp_of(&fx, &auto, c.source_in, c.duration, c.reversed);
                let content = nest.map(|(_, h)| h).unwrap_or(0);
                let fx_hash = fx_chain_hash(
                    &fx,
                    &auto,
                    c.start,
                    c.start + c.duration,
                    c.source_in,
                    c.reversed,
                    grid.frame_step(),
                    content,
                );
                // decode scale step — a property of the clip (R.46): the maximum zoom over
                // the whole duration, computed before the chain is moved
                // The plan is compiled ONCE per renderer build —
                // no further parameter lookup by name (R.59).
                // The hash above is computed from RAW curves: cache keys do not
                // move because of a change in the addressing method.
                let auto_plan = kt_model::AutoPlan::compile(&fx, &auto);
                // masks (shape-masks) join the key only when present: maskless keys stay
                let masks = ClipMaskPlan::of(c);
                let fx_hash = if masks.is_some() {
                    xxhash_rust::xxh3::xxh3_64(format!("{fx_hash}|{:?}", c.masks).as_bytes())
                } else {
                    fx_hash
                };
                let crop =
                    [c.geometry.crop_l, c.geometry.crop_t, c.geometry.crop_r, c.geometry.crop_b];
                let head_zoom = clip_head_zoom(
                    &fx,
                    auto_plan.as_ref(),
                    c.start,
                    c.start + c.duration,
                    c.source_in,
                    c.reversed,
                    &grid,
                    cams,
                    (project.settings.width, project.settings.height),
                    lead,
                );
                VideoClip {
                    id: c.id,
                    start: c.start,
                    end: c.start + c.duration,
                    source_in: c.source_in,
                    lane: c.lane,
                    silent,
                    media: addr,
                    reversed: c.reversed,
                    fx,
                    auto_plan,
                    fx_hash,
                    fade_in: None,
                    fade_out: None,
                    fade_in_trans: None,
                    fade_out_trans: None,
                    fade_in_solo: false,
                    fade_out_solo: false,
                    opacity_k: (c.opacity.clamp(0.0, 1.0) * 4096.0).round() as u32,
                    head_ext: Tick::ZERO,
                    tail_ext: Tick::ZERO,
                    warp,
                    head_zoom,
                    native,
                    lead,
                    crop,
                    nest,
                    content,
                    nest_scale,
                    nest_fps,
                    procedural,
                    material_end,
                    stream: if nest.is_none() { stream } else { None },
                    src_rev: if procedural {
                        xxhash_rust::xxh3::xxh3_64(format!("{:?}|{:?}", c.r#gen, c.gen_auto).as_bytes())
                    } else {
                        0
                    },

                    masks,
                }
                })
            })
            .collect();
        // dissolve windows from transitions (R.24): pairs find each other by
        // the equality of the window on one track (compose blends them as one layer).
        // The geometry of the joints is given by the reel AS IS: overrides change
        // values, not `start`/`duration`, so `joint`/`overlap_pair`
        // are computed from it. The kind and curves of the transition are taken from the RESOLVED
        // set — and those are exactly what publishing edits.
        let reel: &dyn kt_model::Reel = match seq.and_then(|id| project.sequence(id)) {
            Some(s) => s,
            None => project,
        };
        // indexed, as in the audio build (builder-cost-guard, t2063)
        let reel = kt_model::Indexed::new(reel);
        let ends = crate::audio::PieceEdges::of(clips.iter().map(|c| (c.id, c.start, c.end)));
        for t in resolved.transitions.iter() {
            // An OVERLAID transition (R.51, item 399): the window is the overlap zone itself,
            // there are no handles (the material of both sides is real,
            // ext = 0). Further, the pair finds itself by the ordinary find_pair by
            // window equality, and build_plan collapses it into a Dissolve —
            // instead of a hard cut in the Vegas order inside the overlap zone.
            if let Some((pa, pb)) = t.overlap_pair(&reel) {
                let (w0, w1) = (pb.start, pa.start + pa.duration);
                if w1.0 <= w0.0 {
                    continue;
                }
                if let Some(a) = ends.ending(pa.id, w1).map(|i| &mut clips[i]) {
                    a.fade_out = Some((w0, w1));
                    a.fade_out_solo = false;
                    a.fade_out_trans = Some(TransAnim::of(t, w0));
                }
                if let Some(b) = ends.starting(pb.id, w0).map(|i| &mut clips[i]) {
                    b.fade_in = Some((w0, w1));
                    b.fade_in_solo = false;
                    b.fade_in_trans = Some(TransAnim::of(t, w0));
                }
                continue;
            }
            let Some(joint) = t.joint(&reel) else { continue };
            let (w0, w1) = (joint - t.in_offset, joint + t.out_offset);
            if w1.0 <= w0.0 {
                continue;
            }
            let solo = t.is_fade();
            // a morpher layer could cut the target (R.35): the id stayed on piece
            // No.0, and the joint went to the piece whose boundary sits on it —
            // without this a transition on a cut clip silently vanished
            if let Some(a) = t.a.and_then(|id| ends.ending(id, joint)).map(|i| &mut clips[i]) {
                a.fade_out = Some((w0, w1));
                a.fade_out_solo = solo;
                a.fade_out_trans = Some(TransAnim::of(t, w0));
                a.tail_ext = Tick((w1.0 - a.end.0).max(0));
            }
            if let Some(b) = t.b.and_then(|id| ends.starting(id, joint)).map(|i| &mut clips[i]) {
                b.fade_in = Some((w0, w1));
                b.fade_in_solo = solo;
                b.fade_in_trans = Some(TransAnim::of(t, w0));
                b.head_ext = Tick((b.start.0 - w0.0).max(0));
            }
        }
        // Composition order: the background first (large lane), the foreground
        // (lane 0) last. Inside ONE track, on an overlay from above, the one
        // that STARTED LATER is on top — the Vegas rule (the owner's decision after
        // R.28; before that the v1 rule “on top is the one with the smaller start”
        // applied, and the timeline strip diverged with it). Test 162.
        clips.sort_by_key(|c| (std::cmp::Reverse(c.lane), c.start.0));
        stream::link_stages(&mut clips);
        // Subtrees: one renderer for EACH reel in use.
        // The depth limit is held by the model (`validate`); here it is a safeguard
        // against a hostile file.
        let mut nests = std::collections::HashMap::new();
        if depth < kt_model::MAX_NEST_DEPTH {
            for c in effective.iter().filter(|c| c.kind == ClipKind::Video) {
                let Some(id) = project.sequence(c.media).map(|_| c.media) else { continue };
                let key = (id, instance_content(project, id, &c.overrides));
                nests.entry(key).or_insert_with(|| {
                    VideoRenderer::build_reel(
                        project,
                        facts,
                        nest_scales,
                        Some(id),
                        &c.overrides,
                        depth + 1,
                    )
                });
            }
        }
        VideoRenderer {
            clips,
            grid,
            // cameras are OWN to each reel: cameras may be placed inside a nesting,
            // and the active-camera rule is computed from its own reel
            cameras: resolved.cameras.to_vec(),
            project_size: (project.settings.width, project.settings.height),
            nests,
            stages,
            stream_memo: stream_memo().clone(),
            stream_pacing: None,
            stream_held: Default::default(),
            stream_hold: Default::default(),
        }
    }

    /// Playing or paused, for the stream stages' late frames (`StreamPacing::hold`).
    pub fn set_stream_hold(&self, hold: bool) {
        self.stream_hold.store(hold, std::sync::atomic::Ordering::Relaxed);
        for sub in self.nests.values() {
            sub.set_stream_hold(hold);
        }
    }

    /// The pacing with the hold the player last set.
    fn pacing(&self) -> Option<StreamPacing> {
        self.stream_pacing.map(|mut p| {
            p.hold = self.stream_hold.load(std::sync::atomic::Ordering::Relaxed);
            p
        })
    }

    /// Whether the last `frame_plan` showed a stream layer's late frame: such a composite is not
    /// the moment's own, and must not be kept as what plays there without work.
    pub fn stream_held(&self) -> bool {
        self.stream_held.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Pace the stream stages as a live preview must (decode-request-storm): the order names the
    /// earliest history frames that fit `order_bytes`, a composite runs a stage only when its frame
    /// lacks at most `compose_steps` positions, and the rest is `stream_advance` off the UI thread.
    pub fn set_stream_pacing(&mut self, pacing: Option<StreamPacing>) {
        for sub in self.nests.values_mut() {
            sub.set_stream_pacing(pacing);
        }
        self.stream_pacing = pacing;
        self.stream_hold.store(pacing.is_some_and(|p| p.hold), std::sync::atomic::Ordering::Relaxed);
    }

    /// The memo the stream stages of this renderer (and of its reels) keep their codec results
    /// in (datamosh-codec-path, D3): the preview's shared one by default; an export gives its own.
    pub fn set_stream_memo(&mut self, memo: Arc<kt_plugin_api::stream::StreamMemo>) {
        for sub in self.nests.values_mut() {
            sub.set_stream_memo(memo.clone());
        }
        self.stream_memo = memo;
    }

    /// Run every stream stage active at `t` as far as the cached frames allow (the export's
    /// pre-roll, D6; the live preview's prefetch thread, decode-request-storm): the results stay
    /// in the memo, so the next `layers_at` names less. `true` — a stage still lacks frames.
    pub fn stream_advance(&self, cache: &BurstCache, t: Tick, quality: Quality) -> bool {
        let mut pending = false;
        for c in self.clips.iter().filter(|c| c.active_at(t)) {
            match (&c.stream, c.nest.and_then(|k| self.nests.get(&k))) {
                (Some(_), None) => {
                    pending |= self.stream_advance_clip(cache, c, t, quality);
                    // nobody wants this advance any more: the rest of the stages neither
                    if stream::stopping() {
                        return true;
                    }
                }
                (_, Some(sub)) => {
                    pending |= sub.stream_advance(cache, clip_inner_tick(c, t, &self.grid), clip_decode_quality(c, quality))
                }
                _ => {}
            }
        }
        pending
    }

    /// Advance the stream stages over a run of moments `from ..= to` (no wrap) as ONE walk per stage
    /// (stream-history-on-device, found-defects 391): each stage active at `from` walks to its last
    /// position in the run, a stage that starts inside the run walks to `to`. The walk reaches `from`
    /// first. `true` — a stage was left short.
    pub fn stream_advance_run(&self, cache: &BurstCache, from: Tick, to: Tick, quality: Quality) -> bool {
        let step = self.grid.frame_step().max(1);
        let mut pending = false;
        let mut done: Vec<kt_model::ClipId> = Vec::new();
        for c in self.clips.iter().filter(|c| c.active_at(from) && c.stream.is_some() && c.nest.is_none()) {
            // the run's last moment inside this event
            let last = Tick(to.0.min(self.grid.align_floor(Tick(c.end.0 - 1)).0).max(from.0));
            // the frame on screen first: its batch comes back before the frames ahead are walked
            let short = self.stream_advance_clip(cache, c, from, quality);
            pending |= short;
            if !short && last.0 > from.0 && !stream::stopping() {
                pending |= self.stream_advance_clip(cache, c, last, quality);
            }
            if stream::stopping() {
                return true;
            }
            done.push(c.id);
        }
        if to.0 > from.0 {
            for c in self.clips.iter().filter(|c| c.active_at(to) && c.stream.is_some() && c.nest.is_none() && !done.contains(&c.id)) {
                pending |= self.stream_advance_clip(cache, c, to, quality);
            }
        }
        // a reel's stages: per moment, as before
        if self.clips.iter().any(|c| c.nest.is_some() && (c.active_at(from) || c.active_at(to))) {
            let mut t = from;
            while t.0 <= to.0 {
                for c in self.clips.iter().filter(|c| c.active_at(t)) {
                    if let Some(sub) = c.nest.and_then(|k| self.nests.get(&k)) {
                        pending |= sub.stream_advance(cache, clip_inner_tick(c, t, &self.grid), clip_decode_quality(c, quality));
                    }
                }
                t = Tick(t.0 + step);
            }
        }
        pending
    }

    /// One stage's advance to moment `t` (the body of `stream_advance`).
    fn stream_advance_clip(&self, cache: &BurstCache, c: &VideoClip, t: Tick, quality: Quality) -> bool {
        let Some(st) = &c.stream else { return false };
        let cs = stream::ClipStream::new(c, st, &self.grid, quality, Some(cache)).with_view(&self.stream_memo);
        let n = cs.pos(t);
        let needs = st.kernel().needs(&st.values, &cs, n, &self.stream_memo).len();
        if needs == 0 {
            return false;
        }
        let mut span = kt_time::trace::span(kt_time::trace::Ev::StreamAdvance);
        span.id(c.id.0).a(n as u64).b(needs as u64);
        let short = !st.kernel().advance(&st.values, &cs, n, &self.stream_memo);
        if cs.stopped().is_some() {
            span.aux(kt_time::trace::stream::STOPPED);
            cs.trace_stop(n);
            return true;
        }
        if kt_time::trace::on() {
            let left = st.kernel().needs(&st.values, &cs, n, &self.stream_memo).len();
            span.c(needs.saturating_sub(left) as u64).aux(if short { kt_time::trace::stream::SOURCE_MISSING } else { kt_time::trace::stream::DONE });
        }
        short
    }

    /// Tell the stream memo the moment the host shows from (stream-history-on-device D4); `None` —
    /// every position is shown (the default).
    pub fn set_stream_view(&self, from: Option<Tick>) {
        self.stream_memo.set_view(from.map(|t| t.0));
    }

    /// The checkpoints the stream stages active at `t` keep for a walk of `t`'s history (the memo's
    /// sizing); the largest over the stages.
    pub fn stream_checkpoints(&self, t: Tick, quality: Quality) -> usize {
        self.clips
            .iter()
            .filter(|c| c.active_at(t) && c.nest.is_none())
            .filter_map(|c| {
                let st = c.stream.as_ref()?;
                let cs = stream::ClipStream::new(c, st, &self.grid, quality, None);
                Some(st.kernel().checkpoints(&st.values, &cs, cs.pos(t)))
            })
            .max()
            .unwrap_or(0)
    }

    /// The largest frame (RGBA bytes) a stream stage active at `t` orders (decode-request-storm:
    /// what the player's look-ahead of the stages may keep in the memo); 0 — none.
    pub fn stream_frame_bytes(&self, t: Tick, quality: Quality) -> usize {
        self.clips
            .iter()
            .filter(|c| c.active_at(t))
            .map(|c| match (&c.stream, c.nest.and_then(|k| self.nests.get(&k))) {
                (Some(st), None) => {
                    let sq = stream::stream_quality(c, st, quality);
                    sq.width as usize * sq.height as usize * 4
                }
                (_, Some(sub)) => sub.stream_frame_bytes(clip_inner_tick(c, t, &self.grid), clip_decode_quality(c, quality)),
                _ => 0,
            })
            .max()
            .unwrap_or(0)
    }

    /// Frames needed for the composition of moment `t` (aligned by the caller):
    /// (media, source pts, DECODE QUALITY). Order — back-to-front.
    ///
    /// Quality is part of the order (R.46, item 318), not a property of the session: a clip with
    /// a zooming chain is decoded larger than Quality. The caller must
    /// warm and read EXACTLY this quality — otherwise a permanent cache miss
    /// (t247d), now along the size axis too (t312).
    pub fn layers_at(&self, t: Tick, quality: Quality) -> Vec<(MediaId, Tick, Quality)> {
        let mut out = Vec::new();
        self.layers_at_into(t, quality, false, &mut out);
        out
    }

    /// The order GOES DOWN into the nesting (R.50, wave B): a reel has no source, it has
    /// content. If it did not descend, the prefetch would warm an address that does not
    /// exist, while the composite would read real frames: a permanent cache miss,
    /// the very one that invariant 4 is against.
    fn layers_at_into(
        &self,
        t: Tick,
        quality: Quality,
        blend_footage: bool,
        out: &mut Vec<(MediaId, Tick, Quality)>,
    ) {
        for c in self.clips.iter().filter(|c| c.active_at(t)) {
            // in “blend” mode the layer reads TWO frames — the prefetch
            // must warm both, otherwise the second would never be requested
            let (a, b) = clip_frame_keys(c, t, &self.grid, blend_footage);
            match c.nest.and_then(|k| self.nests.get(&k)) {
                // the reel's inner time is CONTINUOUS (R.56, item 448): the order
                // descends at the same tick at which the plan assembles the
                // sub-composite, and the QUALITY is raised by the reel's scale step —
                // the order and the read must ask for the same thing
                // (invariant 4). The “blend” mode travels DOWN: the leaves blend,
                // and they must warm both sides of the pair.
                Some(sub) => sub.layers_at_into(
                    a,
                    clip_decode_quality(c, quality),
                    nest_blend(c, blend_footage),
                    out,
                ),
                // a STREAM stage replaces the source: the order is the stream frames its kernel
                // still needs at the codec's size, and nothing at the ordinary quality (D5)
                None if c.stream.is_some() => {
                    let st = c.stream.as_deref().expect("checked");
                    let cs = stream::ClipStream::new(c, st, &self.grid, quality, None);
                    let needs = st.kernel().needs(&st.values, &cs, cs.pos(t), &self.stream_memo);
                    match self.stream_pacing {
                        Some(p) => cs.order(cs.window(&needs, p.order_bytes), out),
                        None => cs.order(&needs, out),
                    }
                }
                None => {
                    let dq = clip_decode_quality(c, quality);
                    out.push((c.media, a, dq));
                    if let Some((p, _)) = b {
                        out.push((c.media, p, dq));
                    }
                }
            }
        }
    }

    /// All frame keys for the timeline range (prefetch/export).
    pub fn wanted_frames(
        &self,
        range: kt_time::TickRange,
        quality: Quality,
    ) -> Vec<(MediaId, Tick, Quality)> {
        let mut out = Vec::new();
        let step = self.grid.frame_step();
        let mut t = self.grid.align_floor(range.start);
        while t < range.end {
            out.extend(self.layers_at(t, quality));
            t = Tick(t.0 + step);
        }
        out.sort();
        out.dedup();
        out
    }

    /// Decode quality of each video clip (R.46, item 322): the coverage bars
    /// in the UI ask coverage for IT. Id — of the effective
    /// clip (a morpher layer could cut the source into pieces with different
    /// chains, and hence different scale steps).
    pub fn video_decode_qualities(&self, quality: Quality) -> Vec<(kt_model::ClipId, Quality)> {
        self.clips.iter().map(|c| (c.id, clip_order_quality(c, quality))).collect()
    }

    /// Maximum decode quality among the clips (R.46, item 318): the cache and lookahead
    /// budgets count memory by IT, not by Quality — otherwise
    /// an export chunk is underestimated by S² times (item 15: “black render”).
    pub fn max_decode_quality(&self, quality: Quality) -> Quality {
        self.clips
            .iter()
            .map(|c| clip_decode_quality(c, quality))
            .chain(self.clips.iter().filter(|c| c.stream.is_some()).map(|c| clip_order_quality(c, quality)))
            .chain(self.clips.iter().filter_map(|c| {
                let sub = self.nests.get(&c.nest?)?;
                Some(sub.max_decode_quality(clip_decode_quality(c, quality)))
            }))
            .max()
            .unwrap_or(quality)
    }

    /// Coverage of NESTINGS in the ticks of their content (R.50, B6).
    ///
    /// The coverage of a nesting is the **INTERSECTION** of the coverage of its children.
    /// Not the union: a union would lie in the GOOD direction — it would show as warm a
    /// place where half of the layers have not yet arrived, and that is exactly what
    /// invariant 4 is against (“the cache is the only truth”).
    ///
    /// Computed by interval arithmetic: from the reel's span the NOT-WARM part of each child
    /// is subtracted. A hole between the children stays warm — there is nothing to decode
    /// there, and that is true.
    ///
    /// **A child with time stretching participates as a whole**: the warp map
    /// converts source intervals into content intervals non-linearly, and an
    /// approximate conversion here is worse than an honest coarsening — such a child
    /// counts as warm only if all of it is warm.
    pub fn nest_coverage(
        &self,
        cache: &BurstCache,
        quality: Quality,
    ) -> Vec<(kt_model::ClipId, Vec<std::ops::Range<i64>>)> {
        let mut out = Vec::new();
        for c in &self.clips {
            let Some(sub) = c.nest.and_then(|k| self.nests.get(&k)) else { continue };
            // The window is by the SPAN of the material (R.55 C4, the axis refined in R.57):
            // for a stretched instance it is not equal to its length on the
            // timeline, and the bar would lie in the GOOD direction — which
            // invariant 4 does not allow. A mirrored interval is the same,
            // only the direction changes, not the bounds; for a signed curve the bounds
            // are given by the span, not by “how much was eaten”.
            let win = match &c.warp {
                Some(w) => {
                    let (lo, hi) = w.span();
                    (c.source_in.0 + lo)..(c.source_in.0 + hi).max(c.source_in.0 + lo + 1)
                }
                None => c.source_in.0..(c.source_in.0 + c.end.0 - c.start.0),
            };
            let dq = clip_decode_quality(c, quality);
            out.push((c.id, sub.reel_coverage(cache, dq, win, nest_blend(c, false))));
        }
        out
    }

    /// Coverage of the reel's CONTENT over the window `win` (reel ticks).
    fn reel_coverage(
        &self,
        cache: &BurstCache,
        quality: Quality,
        win: std::ops::Range<i64>,
        blend_footage: bool,
    ) -> Vec<std::ops::Range<i64>> {
        // when blending, a leaf reads ONE MORE frame on the right (R.56, item 448):
        // if the window is not widened, the bar would call warm a place whose
        // second half of the pair has not arrived, and that is a lie in the GOOD direction
        let pair_ext = |c: &VideoClip| -> i64 {
            if blend_footage || c.warp.as_ref().is_some_and(|w| w.blend()) {
                self.grid.frame_step().max(1)
            } else {
                0
            }
        };
        let mut warm = vec![win.clone()];
        for c in &self.clips {
            let body = c.start.0.max(win.start)..(c.end.0 + c.tail_ext.0).min(win.end);
            if body.start >= body.end {
                continue;
            }
            // a nested reel inside a reel — the same arithmetic one level down
            let cold = if let Some(sub) = c.nest.and_then(|k| self.nests.get(&k)) {
                // A STRETCHED or MIRRORED nesting — as a whole or not at all,
                // exactly like a stretched file child (R.55, C4):
                // `shift_runs` is a LINEAR map, and under the warp it lies, and
                // the bar has no right to lie in the GOOD direction.
                let inner = match &c.warp {
                    Some(w) => {
                        let (lo, hi) = w.span();
                        (c.source_in.0 + lo)..(c.source_in.0 + hi).max(c.source_in.0 + lo + 1)
                    }
                    None => c.source_in.0..(c.source_in.0 + c.end.0 - c.start.0),
                };
                let sub_warm = sub.reel_coverage(
                    cache,
                    clip_decode_quality(c, quality),
                    inner.clone(),
                    nest_blend(c, blend_footage),
                );
                if c.warp.is_some() || c.reversed {
                    let full =
                        sub_warm.iter().any(|r| r.start <= inner.start && r.end >= inner.end);
                    if full { Vec::new() } else { vec![body.clone()] }
                } else {
                    subtract(vec![body.clone()], shift_runs(sub_warm, c.start.0 - c.source_in.0))
                }
            } else if let Some(w) = &c.warp {
                // a stretched child — as a whole or not at all (see the doc).
                // The window is the SPAN of the material (R.57): the event length has nothing to do
                // with it — at 0.5× it is twice what was read, and the bar
                // would lie in the COLD direction.
                let (lo, hi) = w.span();
                let b0 = c.source_in.0 + lo;
                let b1 = (c.source_in.0 + hi).max(b0 + 1) + pair_ext(c);
                let runs = cache.coverage(
                    SourceId(c.media.0),
                    kt_cache::BurstKind::video(Some(clip_decode_quality(c, quality))),
                    b0..b1,
                );
                let full = runs.iter().any(|r| r.start <= b0 && r.end >= b1);
                if full { Vec::new() } else { vec![body.clone()] }
            } else {
                let ext = pair_ext(c);
                let b0 = c.source_in.0;
                let b1 = b0 + (c.end.0 - c.start.0).max(1) + ext;
                let runs = cache.coverage(
                    SourceId(c.media.0),
                    kt_cache::BurstKind::video(Some(clip_decode_quality(c, quality))),
                    b0..b1,
                );
                // source → reel time; with reversed, the material runs toward it.
                // When blending, the run is SHORTENED on the right by one frame: a place
                // is ready only if the second half of the pair has arrived too.
                let mapped: Vec<std::ops::Range<i64>> = runs
                    .into_iter()
                    .map(|r| r.start..(r.end - ext))
                    .filter(|r| r.start < r.end)
                    .map(|r| {
                        if c.reversed {
                            let len = c.end.0 - c.start.0;
                            let s = c.start.0 + (len - (r.end - c.source_in.0));
                            let e = c.start.0 + (len - (r.start - c.source_in.0));
                            s..e
                        } else {
                            (c.start.0 + (r.start - c.source_in.0))
                                ..(c.start.0 + (r.end - c.source_in.0))
                        }
                    })
                    .collect();
                subtract(vec![body.clone()], mapped)
            };
            warm = subtract(warm, cold);
        }
        warm
    }

    /// The frame at `t` from the cache through `exec`: complete, or a layer was missing and skipped
    /// (playback does not wait, invariant No. 5).
    pub fn compose_on(
        &self,
        exec: &dyn VideoGpuExec,
        cache: &BurstCache,
        t: Tick,
        quality: Quality,
        out: &mut Vec<u8>,
    ) -> Result<bool, GpuExecError> {
        exec.execute_into(&self.frame_plan(cache, t, quality), out)
    }

    /// The media whose frames the composite at `t` names and the cache does not have: what a frame
    /// that came out incomplete lacked (export-pipeline-determinism D4). Sorted, each once.
    pub fn absent_layers(&self, cache: &BurstCache, t: Tick, quality: Quality) -> Vec<MediaId> {
        let mut media: Vec<MediaId> = self
            .layers_at(t, quality)
            .into_iter()
            .filter(|(m, pts, q)| !cache.contains_video(crate::fetch::source_id(*m), Some(*q), video_cache_idx(*pts)))
            .map(|(m, _, _)| m)
            .collect();
        media.sort();
        media.dedup();
        media
    }

    /// Whether a stream stage (datamosh-codec-path) runs anywhere in this renderer's tree: its
    /// export pre-roll decodes on the compose thread, so the pipelined export keeps the serial
    /// composite for such a project (export-throughput D2).
    pub fn has_stream_stages(&self) -> bool {
        self.clips.iter().any(|c| c.stream.is_some()) || self.nests.values().any(|n| n.has_stream_stages())
    }

    /// How many SUB-RENDERERS this reel holds (R.59, stage 3).
    ///
    /// A guard for the t442/t443 gain: two instances of one reel WITHOUT
    /// overrides must share ONE node — that was the point of the tree. The key became
    /// a pair (reel, revision), and without a counter this property cannot be
    /// checked from outside.
    pub fn nest_count(&self) -> usize {
        self.nests.len()
    }

    /// Maximum number of simultaneous video layers (including transition handles) — export
    /// divides the chunk budget by this number (R.31 item 4).
    pub fn max_concurrent_layers(&self) -> usize {
        self.max_concurrent_layers_in(false)
    }

    /// The same inside a reel onto which blending is inherited (R.56,
    /// item 448): the pair is counted at the LEAF, and it must be counted where it
    /// lives — otherwise the chunk budget would diverge from what the plan does.
    fn max_concurrent_layers_in(&self, blend_footage: bool) -> usize {
        let mut evs: Vec<(i64, i32)> = Vec::with_capacity(self.clips.len() * 2);
        for c in &self.clips {
            // a blending layer holds TWO frames at once (R.37 C5). Failing to account for
            // this — the export chunk would evict the neighbour before compositing, the
            // starvation branch would fire, and EXPORT BYTES would come to depend on the timing of the
            // cache.
            // A NESTING ITSELF never has a pair (R.56, item 448): its leaves blend,
            // and the double buffer is counted INSIDE — if we counted it here too, the budget would
            // diverge from what the plan does.
            let blends =
                c.nest.is_none() && (blend_footage || c.warp.as_ref().is_some_and(|w| w.blend()));
            let mut w = if blends { 2 } else { 1 };
            // A nesting holds ITS OWN sub-composite buffer plus everything
            // that is alive inside it at the same time (R.50). Under-counting makes EXPORT
            // BYTES depend on the timing of the cache — literally the R.37 C5
            // regression.
            if let Some(sub) = c.nest.and_then(|k| self.nests.get(&k)) {
                w += sub.max_concurrent_layers_in(nest_blend(c, blend_footage)) as i32;
            }
            evs.push((c.start.0 - c.head_ext.0, w));
            evs.push((c.end.0 + c.tail_ext.0, -w));
        }
        evs.sort();
        let (mut cur, mut max) = (0i32, 0i32);
        for (_, d) in evs {
            cur += d;
            max = max.max(cur);
        }
        max.max(1) as usize
    }

    /// Frame plan for an external executor (GPU in kt-gpu). The pre-FX frames
    /// are already read from the cache (recency bumped), the layer structure is settled.
    pub fn frame_plan(&self, cache: &BurstCache, t: Tick, quality: Quality) -> FramePlan<'_> {
        self.plan_with(cache, t, quality, false)
    }

    /// The plan whose only use is its key (structured-telemetry): a stream stage gives the key its
    /// kernel names for the frame and runs nothing (`StreamKernel::key`), so asking whether the
    /// executor keeps the moment's composite costs no history. Its key equals the key of the
    /// frame's full plan whenever that plan renders the stage's own frame; never execute it.
    pub fn probe_plan(&self, cache: &BurstCache, t: Tick, quality: Quality) -> FramePlan<'_> {
        self.plan_with(cache, t, quality, true)
    }

    fn plan_with(&self, cache: &BurstCache, t: Tick, quality: Quality, probe: bool) -> FramePlan<'_> {
        let ctx = PlanCtx {
            probe,
            grid: &self.grid,
            cameras: &self.cameras,
            project_size: self.project_size,
            nests: &self.nests,
            stages: &self.stages,
            stream_memo: &self.stream_memo,
            stream_pacing: self.pacing(),
            stream_held: &self.stream_held,
            blend_footage: false,
        };
        if !probe {
            self.stream_held.store(false, std::sync::atomic::Ordering::Relaxed);
        }
        build_plan(&self.clips, &ctx, cache, t, quality)
    }
}

/// Plan-building context: the grid, cameras and the project's world size.
struct PlanCtx<'a> {
    grid: &'a FpsGrid,
    cameras: &'a [kt_model::CameraEvent],
    project_size: (u32, u32),
    /// Renderers of nested reels — sub-plans are built from them (R.50).
    nests: &'a std::collections::HashMap<NestKey, VideoRenderer>,
    /// Video track stages (R.62). In the SUB-plan of the stage itself it is empty —
    /// otherwise it would assemble itself endlessly.
    stages: &'a [VideoTrackStage],
    /// Frame blending, inherited from the nesting ABOVE (R.56, item 448).
    /// The mode sits on the reel instance, and its LEAVES execute it: this way one
    /// sub-composite remains per frame (two would cost double on the GPU), and procedural content inside does not land on the grid instead of
    /// continuous time. At the root it is always `false` — the bytes of GOLDEN_VIDEO
    /// rest on this.
    blend_footage: bool,
    /// The memo of the stream stages (datamosh-codec-path, D3): the root renderer's, inherited
    /// downward, so an export's private memo serves its reels too.
    stream_memo: &'a kt_plugin_api::stream::StreamMemo,
    /// The live preview's pacing of the stream stages (decode-request-storm), inherited downward.
    stream_pacing: Option<StreamPacing>,
    /// Set when a stream layer of the plan shows a late frame (the root renderer's flag).
    stream_held: &'a std::sync::atomic::AtomicBool,
    /// A key-only plan (`probe_plan`): stream stages are keyed, not run.
    probe: bool,
}

/// The chain's camera: pinned by id, otherwise the active one by `t`, otherwise the default
/// “50 mm” (identity for Pan3d with defaults). No Pan3d in the chain — no
/// camera either: `None` means “Pan3d is skipped”, as it was before R.44.
///
/// Pub, from R.54 (item 429), is called FROM `layer_src`: the resolution rule must
/// stay one for the whole program. The second consumer is the picker thumbnails
/// (kt-app): the strip shows the clip's chain with the candidate at the tail, and
/// if it assembled the camera itself, the `Pan 3D` variant in the picker would look
/// different from what it does after a click. `out_w`/`project_w` give `world_scale` —
/// the camera's world is frozen by the format in the project's world pixels (R.29),
/// so the target scale goes into it, not into the camera itself.
pub fn resolve_cam3d(
    fx: &[kt_model::FxInstance],
    cameras: &[kt_model::CameraEvent],
    t: Tick,
    out_w: u32,
    project_w: u32,
) -> Option<Cam3d> {
    // the first slot with a camera row decides (R.29), whatever its kind (D11.4)
    fx.iter()
        .find(|f| f.uses_camera())
        .map(kt_model::FxInstance::pinned_camera)
        .map(|pinned| {
            let ev = pinned
                .and_then(|id| cameras.iter().find(|c| c.id == id))
                .or_else(|| kt_model::active_camera_in(cameras, t));
            let cam = ev.map(|e| e.params_at(t)).unwrap_or_default();
            Cam3d { cam, world_scale: out_w as f64 / project_w.max(1) as f64 }
        })
}

/// Layer source: pts, the pre-FX frame (read ALWAYS — a recency bump, so that
/// the coverage bars and prefetch do not “cool down” under a live post-FX cache),
/// the chain and the clip's progress.
fn layer_src<'a>(
    clip: &'a VideoClip,
    ctx: &PlanCtx<'a>,
    cache: &BurstCache,
    quality: Quality,
    t: Tick,
) -> LayerSrc<'a> {
    // For a REEL, `src_pts` is the continuous inner tick; for a leaf it is the frame pts
    // (R.56, item 448). Both branches below read exactly what
    // `layers_at_into` ordered with the same function.
    let (src_pts, pair) = clip_frame_keys(clip, t, ctx.grid, ctx.blend_footage);
    // Curves are sampled at the LINEAR material position — the same one that
    // the UI axis draws and that audio uses for its timing (R.37: “WITHOUT accounting for
    // slowing or speeding up”). Before R.39 this held `src_pts` — the WARP-
    // dependent frame position — and on a slowed clip the picture animated
    // to its own schedule, while the sound and UI followed their own.
    //
    // These are different questions, and they must not be confused: “which frame to show” —
    // the warp map (`clip_frame_keys`); “where we are on the animation axis” — the ruler.
    let mat = kt_model::clip_material_at(
        clip.source_in,
        clip.start,
        clip.end - clip.start,
        clip.reversed,
        ctx.grid.frame_step(),
        t,
    );
    let cache_idx = video_cache_idx(src_pts);
    // The address of a REEL must carry `NEST_TAG` (R.55, C4). Without it, it numerically
    // coincides with the address of a FILE with the same id — exactly what `validate`
    // forbids, so that a file and a sequence do not share the cache. Until now only
    // the difference in `key_hash` saved it; the frame-blending branch would read, at
    // this address, a frame of a FOREIGN FILE.
    let source = match clip.nest {
        Some((seq, _)) => nest_source_id(seq),
        None => SourceId(clip.media.0),
    };
    // R.46 (item 318): the layer reads a frame of ITS OWN decode quality — the same
    // one the prefetch ordered via layers_at. For a REEL this is the step of the
    // SUB-PLAN (one per reel), while the instance's chain input is computed
    // separately (R.58, item 459) — see `clip_chain_scale`.
    let stage = clip_decode_scale(clip, quality);
    let src_scale = clip_chain_scale(clip, quality);
    let dq = clip_decode_quality(clip, quality);
    // The frame is taken TOGETHER with the content serial number (R.53, item 409):
    // the address `(source, cache_idx, size)` of a new generator frame is the same
    // as that of the old one, and without the number the GPU would return the old texture.
    // A STREAM stage (datamosh-codec-path, D1/D4) replaces the source: its kernel decodes the
    // event's stream HERE, in the plan's common code, and both executors receive those bytes. The
    // frame at the ordinary decode quality is not read — it is not ordered either (D5).
    let streamed = match (&clip.stream, clip.nest) {
        (Some(st), None) => Some(
            ctx.probe
                .then(|| stream::key_only(clip, st, ctx.grid, quality, dq, t, src_pts))
                .flatten()
                .unwrap_or_else(|| {
                    stream::run(clip, st, ctx.grid, cache, ctx.stream_memo, ctx.stream_pacing, ctx.stream_held, quality, dq, t, src_pts)
                }),
        ),
        _ => None,
    };
    let (frame, content) = if clip.nest.is_some() {
        (None, 0) // the subtree's input is assembled below
    } else if let Some((f, key)) = &streamed {
        (f.clone(), *key)
    } else {
        match cache.get_video_stamped(source, Some(dq), cache_idx) {
            Some((f, serial)) => (Some(f), serial),
            None => (None, 0),
        }
    };
    // BLENDING (R.37 C5) is done HERE, in the plan's common code: the executor
    // receives an already blended pre-FX frame, and the same
    // arithmetic does not have to be kept in WGSL as well
    // (invariant No. 7). The kernel is the ready `dissolve_into`: byte-frozen.
    // FRAME BLENDING ON THE NESTING ITSELF is not done (R.56, item 448): two
    // sub-composites per frame cost double and add nothing —
    // the reel's inner time is already continuous. The mode travels
    // DOWN, to the leaves, and `clip_frame_keys` already hands out the pair there.
    debug_assert!(
        pair.is_none() || clip.nest.is_none(),
        "a nest never gets a frame pair: the leaf does the blending"
    );
    let (frame, blend) = match pair {
        None => (frame, None),
        // a stream stage reads its own stream; frame blending does not apply to it
        Some(_) if streamed.is_some() => (frame, None),
        Some((pts, k)) => {
            let idx = video_cache_idx(pts);
            let (nb, nb_content) = match cache.get_video_stamped(source, Some(dq), idx) {
                Some((f, serial)) => (Some(f), serial),
                None => (None, 0),
            };
            (
                blend_frames(frame, nb, k),
                Some(crate::video_plan::LayerBlend { cache_idx: idx, k, content: nb_content }),
            )
        }
    };
    let span = (clip.end.0 - clip.start.0).max(1);
    let u = (t.0 - clip.start.0) as f64 / span as f64;
    // automation (R.30): curves are resolved HERE, once per frame —
    // the executor receives already computed parameters (the chain is cloned
    // only with live curves)
    let fx: std::borrow::Cow<'_, [kt_model::FxInstance]> = match &clip.auto_plan {
        Some(plan) => {
            let mut owned = clip.fx.clone();
            plan.apply(&mut owned, mat);
            std::borrow::Cow::Owned(owned)
        }
        // zero cost: a clip without curves BORROWS its chain
        None => std::borrow::Cow::Borrowed(&clip.fx[..]),
    };
    let cam3d = resolve_cam3d(&clip.fx, ctx.cameras, t, quality.width, ctx.project_size.0);
    debug_assert!(
        src_scale == 1
            || kt_fx::video::has_video_fx_in(&fx, clip.lead, quality.width, quality.height),
        "a clip without video fx never gets a chain input above 1x"
    );
    // The layer's INPUT. For a nesting this is not a frame from the cache, but a SUB-COMPOSITE: the
    // reel's plan, assembled on the instance's INNER time. The inner time is
    // CONTINUOUS (`clip_inner_tick`, R.56 item 448): reverse and transition handles
    // are the same as for a file, but there is no floor — a reel has no frames to
    // snap to, yet it has titles and automation, for which a floor shows up as
    // stepping in two-frame jumps.
    let input = match clip.nest.and_then(|k| ctx.nests.get(&k).map(|r| (k.0, r))) {
        Some((id, sub)) => {
            let blend = nest_blend(clip, ctx.blend_footage);
            let sub_ctx = PlanCtx {
                grid: &sub.grid,
                cameras: &sub.cameras,
                project_size: sub.project_size,
                nests: &sub.nests,
                stages: &sub.stages,
                stream_memo: ctx.stream_memo,
                stream_pacing: ctx.stream_pacing,
                stream_held: ctx.stream_held,
                probe: ctx.probe,
                blend_footage: blend,
            };
            // The sub-plan is assembled at S×Quality (R.50, B5): the chain on the
            // nesting zooms the ASSEMBLED picture, and without headroom it would be
            // soft. The children compute their own step already against the ENLARGED
            // Quality — the conjugation of the item 317 rule one level up.
            let plan = build_plan(
                &sub.clips,
                &sub_ctx,
                cache,
                src_pts,
                clip_decode_quality(clip, quality),
            );
            crate::video_plan::LayerInput::Nest(Box::new(crate::video_plan::NestSrc {
                seq: id,
                content_hash: clip.content,
                inner_t: src_pts,
                blend,
                nest_scale: stage,
                plan,
            }))
        }
        None => crate::video_plan::LayerInput::Frame(frame),
    };
    LayerSrc {
        source,
        cache_idx,
        content,
        src_scale,
        input,
        fx,
        // the decoded stream's key joins the post-FX memo's: the same chain over another mosh is
        // another picture (the stream reads frames of OTHER events, which `fx_hash` does not see)
        fx_hash: match &streamed {
            Some((_, key)) => kt_plugin_api::stream::mix(clip.fx_hash, *key),
            None => clip.fx_hash,
        },
        u,
        cam3d,
        blend,
        blend_mode: kt_model::BlendMode::Normal,
        lead: clip.lead,
        masks: clip
            .masks
            .as_ref()
            .map(|m| m.ops(mat, ctx.project_size.1.max(1) as f64))
            .unwrap_or_default(),
    }
}

/// TRACK NESTING LAYER (R.62, change B): the composite of its events by the same
/// code as for a reel, then the track chain, then alpha × level and
/// blending down with the chosen mode.
///
/// The raster and time are PARENTAL (a track has neither settings of its own nor
/// its own zero), the scale step is the maximum over its events: the sub-plan is assembled
/// at the same Quality in which a flat layer would be assembled.
fn track_stage_layer<'a>(
    st: &'a VideoTrackStage,
    clips: &'a [VideoClip],
    ctx: &PlanCtx<'a>,
    cache: &BurstCache,
    t: Tick,
    quality: Quality,
) -> PlanLayer<'a> {
    // the sub-plan of ITS OWN events: inside — transitions, overlaps, fades and
    // event levels, exactly as for a reel
    let sub_ctx = PlanCtx {
        grid: ctx.grid,
        cameras: ctx.cameras,
        project_size: ctx.project_size,
        nests: ctx.nests,
        blend_footage: ctx.blend_footage,
        // the stage does NOT assemble itself
        stages: &[],
        stream_memo: ctx.stream_memo,
        stream_pacing: ctx.stream_pacing,
        stream_held: ctx.stream_held,
        probe: ctx.probe,
    };
    let plan = build_plan(clips, &sub_ctx, cache, t, quality);
    // revision of the sub-composite: the track's properties plus the contents of its events
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    h.update(&st.content.to_le_bytes());
    for c in clips {
        h.update(&c.fx_hash.to_le_bytes());
        h.update(&c.content.to_le_bytes());
        h.update(&c.start.0.to_le_bytes());
        h.update(&c.end.0.to_le_bytes());
    }
    let src = LayerSrc {
        source: nest_source_id(kt_model::track_source_media(st.id)),
        cache_idx: t.0,
        content: h.digest(),
        src_scale: 1,
        input: crate::video_plan::LayerInput::Nest(Box::new(crate::video_plan::NestSrc {
            seq: kt_model::track_source_media(st.id),
            content_hash: h.digest(),
            inner_t: t,
            blend: ctx.blend_footage,
            nest_scale: 1,
            plan,
        })),
        fx: std::borrow::Cow::Borrowed(&st.fx),
        fx_hash: st.content,
        u: 0.0,
        cam3d: None,
        blend: None,
        blend_mode: st.blend,
        lead: None,
        // the track's masks cut its composite after the track's chain, in canvas space
        masks: if st.masks.is_empty() {
            Vec::new()
        } else {
            vec![kt_fx::mask::MaskOp {
                at: st.fx.len(),
                kind: kt_fx::mask::MaskOpKind::Alpha,
                masks: Arc::new(st.masks.clone()),
                follow_lead: false,
                project_h: ctx.project_size.1.max(1) as f64,
            }]
        },
    };
    // track level — the composite alpha; mode — a separate axis
    let alpha_k = ((st.opacity.clamp(0.0, 1.0) * 4096.0).round() as u32).min(4096);
    PlanLayer::One { src, alpha_k, scale_alpha: alpha_k < 4096, fade: None }
}

/// Building the frame plan: which layers, in which roles and with which weights.
/// A pair of clips in a transition window is blended by a linear dissolve into ONE layer
/// (item 137); a fade at an edge/Gap (R.25) is a solo layer fading into transparency.
fn build_plan<'a>(
    clips: &'a [VideoClip],
    ctx: &PlanCtx<'a>,
    cache: &BurstCache,
    t: Tick,
    quality: Quality,
) -> FramePlan<'a> {
    let mut layers = Vec::new();
    // TRACK STAGES (R.62): a track with a stage yields ONE layer at the position of
    // its first row — its events do not go into flat layers.
    let mut staged_done: Vec<u32> = Vec::new();
    for (i, clip) in clips.iter().enumerate() {
        // a muted track does not go into the composite; its ORDER nevertheless
        // remains (`layers_at_into`/`wanted_frames` do not check the flag)
        if !clip.active_at(t) || clip.silent {
            continue;
        }
        if let Some(st) = ctx.stages.iter().find(|s| s.lane == clip.lane) {
            if staged_done.contains(&st.lane) {
                continue;
            }
            staged_done.push(st.lane);
            let lane_clips: &[VideoClip] = {
                // track events lie consecutively: the list is sorted by
                // (Reverse(lane), start)
                let from = clips.iter().position(|c| c.lane == st.lane).unwrap_or(i);
                let to = clips[from..].iter().position(|c| c.lane != st.lane).map(|n| from + n);
                &clips[from..to.unwrap_or(clips.len())]
            };
            layers.push(track_stage_layer(st, lane_clips, ctx, cache, t, quality));
            continue;
        }
        // fade at an edge/Gap: there is no pair in the model, the layer always goes through
        // alpha scaling (scale_alpha)
        if let Some((w, incoming, mix, inv)) = solo_fade(clip, t) {
            // crossfade — the previous path (the weight goes straight into alpha; bytes the same);
            // any other kind gives the shape via the kernel, and the level stays the
            // event's level
            let (alpha_k, fade) = if mix.is_crossfade() {
                (w * clip.opacity_k / 4096, None)
            } else {
                // inversion of the solo fade: flip the role BEFORE computing k — its expression
                // itself gives the complement (item 398)
                let incoming = incoming != inv;
                let k = if incoming { w } else { 4096 - w };
                (clip.opacity_k, Some(crate::video_plan::SoloFade { k, mix, incoming }))
            };
            layers.push(PlanLayer::One {
                src: layer_src(clip, ctx, cache, quality, t),
                alpha_k,
                scale_alpha: true,
                fade,
            });
            continue;
        }
        // the outgoing half of the pair is drawn in the incoming layer
        if let Some(win) = clip.fade_out
            && t.0 >= win.0.0
            && t.0 < win.1.0
            && find_pair(clips, clip.lane, win, true).is_some()
        {
            continue;
        }
        if let Some(win) = clip.fade_in
            && t.0 >= win.0.0
            && t.0 < win.1.0
            && let Some(pi) = find_pair(clips, clip.lane, win, false)
        {
            let outgoing = &clips[pi];
            debug_assert_ne!(pi, i);
            let mut k = (((t.0 - win.0.0) * 4096) / (win.1.0 - win.0.0)).clamp(0, 4096) as u32;
            // the kind is resolved by the position INSIDE the window: its parameters
            // may have curves (R.34 K4)
            let ta = clip.fade_in_trans.as_ref();
            let mix = ta.map(|a| a.mix_at(t)).unwrap_or_default();
            // the neighbours' levels are blended with the same weight as the picture;
            // the expression is symmetric to (swap, 1−k) — inversion does not touch it
            let op = (outgoing.opacity_k * (4096 - k) + clip.opacity_k * k) / 4096;
            let mut a = layer_src(outgoing, ctx, cache, quality, t);
            let mut b = layer_src(clip, ctx, cache, quality, t);
            apply_invert(&mut a, &mut b, &mut k, &mix, ta.is_some_and(|a| a.invert));
            layers.push(PlanLayer::Dissolve { a, b, k, alpha_k: op, mix });
            continue;
        }
        layers.push(PlanLayer::One {
            src: layer_src(clip, ctx, cache, quality, t),
            alpha_k: clip.opacity_k,
            scale_alpha: clip.opacity_k < 4096,
            fade: None,
        });
    }
    FramePlan { t, out_w: quality.width, out_h: quality.height, layers }
}

/// The transition partner: a clip of the same track with the SAME window on the opposite
/// side (incoming looks for outgoing and vice versa). There are few clips, a linear scan.
fn find_pair(clips: &[VideoClip], lane: u32, win: (Tick, Tick), incoming: bool) -> Option<usize> {
    clips.iter().position(|c| {
        c.lane == lane && (if incoming { c.fade_in } else { c.fade_out }) == Some(win)
    })
}

/// The SOURCE address of an event (R.59, stage 2) — the ONLY rule.
///
/// A generator's values live on the event, so its source is also its own for each
/// event: the address is taken from ITS id. For all others — the media address,
/// as before. The order shape `(address, tick, quality)` has not changed, so
/// “order == read”, the coverage bars, prefetch, export chunks and
/// reel coverage work without edits (invariant 4).
///
/// EVERYONE must ask this: the plan, prefetch, export, kt-cli and the bars.
/// If two of them diverge — a permanent cache miss on the address, exactly the class
/// of defects that invariant 4 is against.
pub fn clip_source_addr(c: &kt_model::ClipEvent) -> MediaId {
    match c.r#gen {
        Some(_) => gen_source_addr(c.id),
        None => c.media,
    }
}

/// Source address of the generator per event.
pub fn gen_source_addr(id: kt_model::ClipId) -> MediaId {
    MediaId(kt_model::GEN_TAG | id.0)
}

/// SALT of the source address inside a reel (R.59, stage 3).
///
/// Zero — exactly when no handle of the set is able to reach
/// a source (`overrides_touch_sources`), and then the addresses remain
/// byte-for-byte as before: the root, a reel without publishing and an instance without
/// overrides pay nothing.
pub fn gen_salt(published: &[kt_model::PublishedParam], ov: &[kt_model::Override]) -> u64 {
    if !kt_model::overrides_touch_sources(published, ov) {
        return 0;
    }
    // The Debug representation is deterministic (f64 is printed
    // shortest-roundtrip), and a new override field gets into the salt
    // by itself — the same trick as for `content_hash`.
    xxhash_rust::xxh3::xxh3_64(format!("{ov:?}").as_bytes()) | 1
}

/// The generator source address INSIDE an instance with overrides.
///
/// Before R.59 the address uniquely determined the values: one event — one set of values.
/// An override removes this premise (one event inside a reel plays in two releases
/// with a different title), and the address must bring it back:
/// otherwise the order and the read would converge on one address — invariant 4 formally
/// holds — and the frame would come in as someone else's, and the “second release” would silently show the first.
///
/// There are no free bits in the address (clip < piece < generator < reel), and
/// the address is salted with a HASH. This is legitimate for exactly the same reason
/// `content_hash` is: 62 bits, a pure function, and `gen_sources` computes it
/// with the SAME code — the order and the read ask for the same thing.
pub fn gen_source_addr_in(id: kt_model::ClipId, salt: u64) -> MediaId {
    if salt == 0 {
        return gen_source_addr(id);
    }
    let h = xxhash_rust::xxh3::xxh3_64(
        &[id.0.to_le_bytes().as_slice(), salt.to_le_bytes().as_slice()].concat(),
    );
    MediaId(kt_model::GEN_TAG | (h & (kt_model::GEN_TAG - 1)))
}

/// `clip_source_addr` inside a reel instance: for a generator — the salted
/// address; for all others — the media address, as before.
pub fn clip_source_addr_in(c: &kt_model::ClipEvent, salt: u64) -> MediaId {
    match c.r#gen {
        Some(_) => gen_source_addr_in(c.id, salt),
        None => c.media,
    }
}

/// All generators of the DOCUMENT: the root and each reel, through the same door of
/// clip issue as the render (`effective_reel` — morphers already
/// applied). The player, export and kt-cli bring up sources exactly by
/// this list: there must be no second enumeration, otherwise export would
/// render something other than what the preview showed.
pub fn gen_sources(project: &Project) -> Vec<(MediaId, kt_model::GenState)> {
    revisions_scope(|| gen_sources_in(project))
}

fn gen_sources_in(project: &Project) -> Vec<(MediaId, kt_model::GenState)> {
    let mut out: Vec<(MediaId, kt_model::GenState)> = Vec::new();
    let mut have: std::collections::HashSet<MediaId> = std::collections::HashSet::new();
    let mut push = |addr: MediaId, st: kt_model::GenState| {
        if have.insert(addr) {
            out.push((addr, st));
        }
    };
    // 1) EVERY reel with its own template content — as it was before R.59.
    //    A reel that was placed nowhere is also visible: the bin shows it and
    //    the preview service counts it, and its sources must not be lost.
    for seq in std::iter::once(None).chain(project.sequences.iter().map(|s| Some(s.id))) {
        for c in crate::incremental::resolve(project, seq, &[]).clips.iter() {
            if let Some(k) = &c.r#gen {
                push(
                    gen_source_addr(c.id),
                    kt_model::GenState { kind: k.clone(), auto: c.gen_auto.clone() },
                );
            }
        }
    }
    // 2) A WALK OF THE INSTANCE TREE (R.59, stage 3): an instance with
    //    overrides has its own values, and hence its own address. The list
    //    is built by the SAME code as the addresses in the plan (`build_reel`);
    //    if they diverged, the prefetch would warm an address that does not exist.
    let mut seen: std::collections::HashSet<(MediaId, u64)> = std::collections::HashSet::new();
    let mut stack: Vec<(Option<MediaId>, Vec<kt_model::Override>, usize)> =
        vec![(None, Vec::new(), 0)];
    while let Some((seq, ov, depth)) = stack.pop() {
        let published: &[kt_model::PublishedParam] = match seq {
            None => &project.published,
            Some(id) => match project.sequence(id) {
                Some(s) => &s.published,
                None => continue,
            },
        };
        let salt = gen_salt(published, &ov);
        for c in crate::incremental::resolve(project, seq, &ov).clips.iter() {
            if let Some(k) = &c.r#gen {
                push(
                    gen_source_addr_in(c.id, salt),
                    kt_model::GenState { kind: k.clone(), auto: c.gen_auto.clone() },
                );
            }
            if project.sequence(c.media).is_some()
                && depth < kt_model::MAX_NEST_DEPTH
                && seen.insert((c.media, instance_content(project, c.media, &c.overrides)))
            {
                stack.push((Some(c.media), c.overrides.clone(), depth + 1));
            }
        }
    }
    out
}

/// The REEL that CONTAINS the event, and all its ancestors — by this list the
/// memoization of sub-composites goes stale when the source is retuned
/// (R.59, stage 2).
///
/// A hidden defect closed here: an edit of a title INSIDE a reel goes through the retune strip,
/// the project does not change at that point, and the sub-composite memo key —
/// the reel's `content_hash` — is computed from the MODEL. So it
/// does not move, and the reel keeps returning the picture with the old text
/// until the first edit of the plan. Guard — t663.
///
/// The root is not on the list: it has no memo, its composite is assembled
/// every frame.
pub fn reel_chain_of_clip(project: &Project, id: kt_model::ClipId) -> Vec<MediaId> {
    let origin = kt_model::slice_origin(id).unwrap_or(id);
    let Some(home) =
        project.sequences.iter().find(|s| s.clips.iter().any(|c| c.id == origin)).map(|s| s.id)
    else {
        return Vec::new();
    };
    let mut chain = vec![home];
    let mut i = 0;
    while i < chain.len() {
        let cur = chain[i];
        i += 1;
        for s in &project.sequences {
            if !chain.contains(&s.id) && s.clips.iter().any(|c| c.media == cur) {
                chain.push(s.id);
            }
        }
    }
    chain
}

/// The reel's address in the cache. The high bit of `MediaId` is given to nestings (R.50):
/// a file with such an address would silently share the cache with a sequence, and
/// `validate` rejects this.
pub fn nest_source_id(seq: MediaId) -> SourceId {
    SourceId(kt_model::NEST_TAG | seq.0)
}

/// Solo fade (R.25) at moment t: (weight 0..4096, the clip APPEARS, kind).
/// fade_in ramps 0→1, fade_out decays 1→0. None — the moment is outside the
/// solo window (an ordinary layer or a half of a real pair).
fn solo_fade(clip: &VideoClip, t: Tick) -> Option<(u32, bool, crate::TransMix, bool)> {
    let frac =
        |w: (Tick, Tick)| (((t.0 - w.0.0) * 4096) / (w.1.0 - w.0.0).max(1)).clamp(0, 4096) as u32;
    if let Some(w) = clip.fade_in
        && clip.fade_in_solo
        && t.0 >= w.0.0
        && t.0 < w.1.0
    {
        let ta = clip.fade_in_trans.as_ref();
        return Some((
            frac(w),
            true,
            ta.map(|a| a.mix_at(t)).unwrap_or_default(),
            ta.is_some_and(|a| a.invert),
        ));
    }
    if let Some(w) = clip.fade_out
        && clip.fade_out_solo
        && t.0 >= w.0.0
        && t.0 < w.1.0
    {
        let ta = clip.fade_out_trans.as_ref();
        return Some((
            4096 - frac(w),
            false,
            ta.map(|a| a.mix_at(t)).unwrap_or_default(),
            ta.is_some_and(|a| a.invert),
        ));
    }
    None
}

/// Transition inversion (R.51, item 398): “the gesture backwards” — swap the
/// sides and reverse the progress, `kernel(b, a, 1−k)`. The crossfade is
/// not touched: for it this is an identity, and the integer `dissolve_into`
/// keeps GOLDEN_VIDEO. CONTRACT: any construction of `PlanLayer::Dissolve`
/// (including a future overlay pair) MUST go through this helper —
/// otherwise the inversion on it would silently be lost. The cache keys travel by themselves: the swap
/// changes the order of `put_src`, and k is already hashed.
fn apply_invert<'a>(
    a: &mut crate::video_plan::LayerSrc<'a>,
    b: &mut crate::video_plan::LayerSrc<'a>,
    k: &mut u32,
    mix: &crate::TransMix,
    invert: bool,
) {
    if invert && !mix.is_crossfade() {
        std::mem::swap(a, b);
        *k = 4096 - *k;
    }
}

/// Linear dissolve of the pair (item 137): dst = a·(1−k) + b·k, in integers
/// (k out of 4096 parts, rounding to nearest). A frame without alpha is treated
/// as opaque (A=255). Returns has_alpha of the result.
/// Blend a pair of frames (R.37 C5). Starvation of either side — we take the one present
/// at full weight: the same rule as for the `PlanLayer::Dissolve` pair.
fn blend_frames(
    a: Option<Arc<VideoFrame>>,
    b: Option<Arc<VideoFrame>>,
    k: u32,
) -> Option<Arc<VideoFrame>> {
    let (Some(fa), Some(fb)) = (a.as_ref(), b.as_ref()) else { return a.or(b) };
    if (fa.width, fa.height) != (fb.width, fb.height) {
        return a;
    }
    let pixels = fa.width as usize * fa.height as usize;
    let mut buf = Vec::new();
    let has_alpha = dissolve_into(&mut buf, fa, fb, k, pixels);
    Some(Arc::new(VideoFrame {
        width: fa.width,
        height: fa.height,
        rgba: buf.into(),
        has_alpha,
        pts: fa.pts,
    }))
}

fn dissolve_into(
    dst: &mut Vec<u8>,
    a: &kt_media::VideoFrame,
    b: &kt_media::VideoFrame,
    k: u32,
    pixels: usize,
) -> bool {
    let n = pixels * 4;
    dst.clear();
    dst.resize(n, 0);
    let ik = 4096 - k;
    let pixels_avail = pixels.min(a.rgba.len() / 4).min(b.rgba.len() / 4);
    for p in 0..pixels_avail {
        let i = p * 4;
        for ch in 0..4 {
            let av = if ch == 3 && !a.has_alpha { 255 } else { a.rgba[i + ch] } as u32;
            let bv = if ch == 3 && !b.has_alpha { 255 } else { b.rgba[i + ch] } as u32;
            dst[i + ch] = ((av * ik + bv * k + 2048) >> 12) as u8;
        }
    }
    a.has_alpha || b.has_alpha
}

/// BLEND MODE FORMULA (R.62, F-CMP-003) for ONE channel, in
/// fractions 0..1. The mirror is `blend_ch` in `kt-gpu/src/shaders/composite.wgsl`;
/// the files are edited in ONE commit (invariant 7, guard — conformance).
pub fn blend_ch(mode: kt_model::BlendMode, b: f64, a: f64) -> f64 {
    use kt_model::BlendMode as M;
    match mode {
        M::Normal => a,
        M::Multiply => b * a,
        M::Screen => b + a - b * a,
        M::Add => (b + a).min(1.0),
        M::Subtract => (b - a).max(0.0),
        M::Darken => b.min(a),
        M::Lighten => b.max(a),
        // classic overlay: the dark half multiplies, the light half screens
        M::Overlay => {
            if b <= 0.5 {
                2.0 * b * a
            } else {
                1.0 - 2.0 * (1.0 - b) * (1.0 - a)
            }
        }
        M::Difference => (b - a).abs(),
    }
}

