//! Audio render of the timeline: selection of events, sample-accurate reading of sources,
//! equal-power crossfade, gain, summing of tracks, loop-wrap.
//!
//! The semantics are pinned by tests 1–3, 14, 136 (R.24: there are no overlaps any more —
//! a crossfade is set by an explicit TransitionEvent, the invariant “clips of one track
//! butted together” is held by kt-model):
//! - a transition at the junction → equal-power crossfade over the window (outgoing √(1−t),
//!   incoming √t), the “handles” read the source beyond the edges of the used piece;
//! - different tracks are summed without weights;
//! - the clip gain (dB → linear) is applied after the weights.
//!
//! Everything is in the 48 kHz sample domain (frames): clip boundaries are rounded to a sample
//! by floor once, after that the arithmetic is exact (invariant 10 — determinism).

use crate::fetch::AudioFetcher;
use kt_media::AudioBurst;
use kt_model::{ClipEvent, ClipKind, Project};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS};
use std::sync::Arc;

/// The TRACK stage: the sum of its events → the chain → to the output (R.62).
#[derive(Debug, Clone)]
struct TrackStage {
    id: kt_model::TrackId,
    lane: u32,
    fx: Vec<kt_model::FxInstance>,
    /// Lookahead of the chain: the input leads the output by that many frames.
    ///
    /// There is NO tail as a separate field in the stage and there must not be one: the track
    /// processor keeps running while the transport plays, and after the last event
    /// it receives silence — the tail comes out by itself (t733). A field “how long to
    /// sound after the end” would be a second source of the same truth.
    la: i64,
    /// SIDECHAIN KEYS (R.65): the slot of the active chain → the track by which
    /// the detector counts. The slot is an index into the ACTIVE chain (bypass is already
    /// filtered out), because by this same index the processor hands
    /// the nodes their curves.
    ///
    /// A dead key (no track, a track that is not audio) does not get here — the device then
    /// works as an ordinary compressor, as the spec says.
    keys: Vec<(usize, kt_model::TrackId, u32)>,
}

/// A clip prepared for audio render (boundaries in 48 kHz frames).
#[derive(Debug, Clone)]
struct AudioClip {
    id: kt_model::ClipId,
    start: i64,
    end: i64,
    /// The end, taking into account the right “handle” of the transition and the FX tail (the reverb
    /// rings beyond the clip’s boundary — invariant “tail”, defect f of the old engine).
    ext_end: i64,
    source_in: i64,
    lane: u32,
    gain: f32,
    media: kt_model::MediaId,
    fx: Vec<kt_model::FxInstance>,
    /// The automation curves of the clip in the SOURCE domain (R.33): the input for
    /// the processor and for the calculation of the source window.
    auto: kt_fx::AutoProfiles,
    reversed: bool,
    /// The ADDRESS MAP of signed speed (R.57): at which address the material lies
    /// that has been consumed by the moment `a`. `None` — the map is monotonic, the address
    /// equals the consumption, and the whole former path is verbatim.
    addr: Option<std::sync::Arc<Vec<kt_fx::AddrKnot>>>,
    /// The crossfade windows from transitions (R.24, item 136), [w0, w1) in frames:
    /// the clip is incoming (rises √t) / outgoing (decays √(1−t)).
    /// Disjoint by the model’s validation.
    fade_in: Option<(i64, i64)>,
    fade_out: Option<(i64, i64)>,
    /// The crossfade curve of a transition (R.33): equal-power — the former formula,
    /// therefore GOLDEN_AUDIO does not move. Stored one per side:
    /// a clip may have different transitions on the left and on the right.
    fade_in_curve: kt_model::AudioCurve,
    fade_out_curve: kt_model::AudioCurve,
    /// “Handles”: material beyond the edges of the body, sounds only under the transition windows.
    head_ext: i64,
    tail_ext: i64,
    /// The window came from an OVERLAID transition (R.51, item 399): the curve in it is
    /// given by `fade_weight`, and the built-in equal-power `overlap_weight`
    /// is muted through `overlap_at` — otherwise a double fade.
    fade_in_excl: bool,
    fade_out_excl: bool,
    /// A nested reel and the EDIT of this instance (R.59, stage 3).
    /// `None` — an ordinary source. An empty set of overrides gives the same
    /// edit as its neighbours, and the sub-mix stays ONE per reel.
    nest: Option<crate::video::NestKey>,
}

/// The crossfade weight over the transition windows of the clip. It does not know the
/// position or the direction (the reverse transport uses the same one). The formulas are verbatim from
/// weight_at (test 2): for a converted overlap the weights are bit-for-bit identical.
/// Beyond the right edge of fade_out — 1.0: only the FX tail (reverb) sounds there,
/// it is not weighted (the render_fx_clip rule).
fn fade_weight(clip: &AudioClip, f: i64) -> f32 {
    if let Some((w0, w1)) = clip.fade_in
        && f < w1
    {
        let t = (f - w0) as f64 / (w1 - w0) as f64;
        return clip.fade_in_curve.rise(t.max(0.0)) as f32;
    }
    if let Some((w0, w1)) = clip.fade_out
        && f >= w0
        && f < w1
    {
        let t = (f - w0) as f64 / (w1 - w0) as f64;
        // the decaying side — the same formula from (1 − t)
        return clip.fade_out_curve.rise((1.0 - t).max(0.0)) as f32;
    }
    1.0
}

/// The render result of a continuous range.
///
/// **The unit of both counters is FRAMES at 48 kHz** (R.43, item 289). Before this the path
/// without FX counted frames, while the path with FX counted misses of BURSTS, so the same
/// starvation gave numbers differing by `AUDIO_BURST_FRAMES` = 1024 times.
/// The user read them as one quantity and was right that “the counter is broken”.
/// Frames were chosen because they are comparable with the length of an event and with the
/// output latency; bursts are comparable with nothing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderStats {
    /// How many OUTPUT FRAMES could not be read from the source
    /// (silence was substituted).
    pub underruns: u64,
    /// Misses of the PRE-ROLL in FRAMES (warming up FX after a seek) — these frames do not go
    /// to the output, so they are not included in `underruns` (item 166). Diagnostics:
    /// a systematic growth = prefetch does not warm the warm-up zone.
    pub warmup_misses: u64,
    /// Bursts of the SUB-MIX computed for nothing: the pre-roll of the move of the nested reel’s
    /// producer (R.55, C4). Not an underrun — they are not heard — but by them one sees
    /// the cost of reverse and stretch on an attachment, and whether the order window
    /// has diverged from what the producer reads.
    pub nest_warmups: u64,
}

/// An index bucket of time — one second of output.
const INDEX_BUCKET_FRAMES: i64 = kt_time::AUDIO_SAMPLE_RATE as i64;

pub struct AudioRenderer {
    clips: Vec<AudioClip>,
    /// The index “which events live in this second” (R.43, item 305).
    ///
    /// Without it every burst iterated over the WHOLE edit: on the user’s project
    /// (1973 events) that is 0.4 µs per event × 1973 × 107 bursts per second,
    /// that is, the cost of PLACE grew linearly with the size of the PROJECT, even if
    /// the neighbouring events stand an hour away from there. Measurement: 7 events — 3.8 ms/s,
    /// the same 7 plus 2000 distant ones — 91.7 ms/s.
    ///
    /// The buckets store indices IN ASCENDING ORDER, and the selection goes by
    /// them as well: the summation order of the clips MUST stay the same —
    /// f32 addition is not associative, and GOLDEN_AUDIO stands on it.
    index: Vec<Vec<u32>>,
    /// The start of the first bucket in frames (events sometimes lie to the left of zero because of
    /// the transition “handles”).
    index_from: i64,
    /// Who covers whom on each track — built once (item 305).
    /// Empty when the overlap is switched off: then the weight is identically 1.0.
    covers: OverlapMap,
    /// “Does this event overlap with anybody at all?” — per event.
    ///
    /// A binary search over the map is cheap compared with a scan, but it is still
    /// work PER FRAME, and on a simple edit it ate half of the
    /// render time (2.32 against 1.09 ms/s). For an event that overlaps with nobody,
    /// `overlap_weight` would return 1.0 anyway — so there is no need to search, and one bool closes the whole path. Such events
    /// make up the overwhelming majority of events in a real
    /// edit.
    overlaps: Vec<bool>,
    /// Overlaying (R.28): overlaps of clips are legal and sound as in v1 —
    /// equal-power for two, 1/N for three or more. Switched off — there are no overlaps in the
    /// model by construction, and the weight is always 1.0.
    overlap_layers: bool,
    /// Project settings that affect the device of the chains (R.48: the pitch
    /// engine). They cannot be conveyed by smoothing — a change rebuilds
    /// the processors (the check in FxStateStore).
    opts: kt_fx::ChainOpts,
    /// TRACK STAGES (R.62, change B): only those tracks whose chain is non-empty. Level, mute and solo
    /// of the stage do not require them — they are scalars and are folded into the `gain` of the event,
    /// therefore a project without track chains goes the former path byte for byte
    /// (GOLDEN_AUDIO).
    stages: Vec<TrackStage>,
    /// Renderers of nested reels (R.50, B4).
    ///
    /// An attachment is a VIRTUAL MEDIA under the fetcher, not a branch in the mixer.
    /// Therefore `render_plain_clip`, `render_fx_clip`, the weights,
    /// `chain_source_window_ext`, `fill_source` and the ORDER OF SUMMATION of f32 do not
    /// change by a single line: an attachment is simply one of the clips in
    /// `active_clips`, and GOLDEN_AUDIO does not move.
    nests: std::collections::HashMap<crate::video::NestKey, AudioRenderer>,
}

/// A fetcher behind which nested reels look like ordinary sources.
///
/// The sub-mix is NOT cached in `BurstCache` and cannot be: its production is a run of
/// the reel’s FX state, and the state is touched ONLY by the audio render thread
/// (invariant 2). So the producer lives here.
///
/// **Three branches and not one more** (R.55, C4):
///
/// 1. the burst is already in the ring — hand it out, do not touch the state;
/// 2. it is ahead of the cursor not further than a block — CATCH UP forward, the intermediate ones into
///    the ring (this is how accelerated stretch and small jumps work);
/// 3. otherwise (backward, far forward, the first access) — the CANONICAL
///    MOVE: reset, start from the block boundary minus the pre-roll, forward to
///    the required point. The canonicity of the grid is what makes the value of a burst a pure
///    function of the index: the ambiguity “we arrived continuously / moved” remains
///    exactly one, and it is the same one already legalized by the pair
///    `seam`/`hard_seek` of an ordinary clip.
///
/// There must be no fourth branch: each new one is a new kind of
/// nondeterminism, which does not occur in audio (invariant 11).
struct NestFetcher<'a> {
    inner: &'a mut dyn AudioFetcher,
    reels: &'a std::collections::HashMap<crate::video::NestKey, AudioRenderer>,
    states: &'a mut std::collections::HashMap<kt_model::ClipId, crate::fxstore::NestStream>,
    /// The pre-roll and the block are properties of the REEL, computed when the renderer is assembled.
    prerolls: &'a std::collections::HashMap<crate::video::NestKey, (u32, u32)>,
    /// Which sub-renderer reads which INSTANCE (R.59, stage 3). At one address of the reel there are
    /// now several of them — one per edit —
    /// and one can choose the right one only by knowing the READER.
    keys: &'a std::collections::HashMap<kt_model::ClipId, crate::video::NestKey>,
    /// Who reads NOW (`begin_clip`). An attachment has its own stream for each
    /// instance, and without this five places of one reel would fight over one
    /// cursor of the producer.
    cur: Option<kt_model::ClipId>,
    /// How many bursts of the sub-mix were computed for nothing (the pre-roll of the move).
    /// This is NOT an underrun: they are not heard, but by them one sees that the order window
    /// has diverged from what the producer reads.
    warmups: u64,
    /// Whether the producer is ALLOWED. In the raw reverse transport (`render_frames_raw`)
    /// the reel plays dry, it has no state at all, and blocks are not needed.
    raw: bool,
}

impl NestFetcher<'_> {
    /// Render a burst of the sub-mix FORWARD, advancing the reel’s state.
    fn produce(
        reel: &AudioRenderer,
        inner: &mut dyn AudioFetcher,
        st: &mut crate::fxstore::NestStream,
        idx: i64,
        raw: bool,
    ) -> std::sync::Arc<kt_media::AudioBurst> {
        let mut data = Box::new([0.0f32; AUDIO_BURST_FRAMES * AUDIO_CHANNELS]);
        let at = idx * AUDIO_BURST_FRAMES as i64;
        if raw {
            reel.render_frames_raw(inner, at, &mut data[..]);
        } else {
            reel.render_frames_with(inner, &mut st.store, at, &mut data[..]);
        }
        st.next = Some(idx + 1);
        std::sync::Arc::new(kt_media::AudioBurst { idx, data })
    }
}

impl AudioFetcher for NestFetcher<'_> {
    /// The edge of the material is a QUESTION FOR THE SOURCE, and the wrapper must
    /// pass it on (R.63 item 2). The trait’s default (“there is no edge”) here
    /// would silently switch off the whole mechanism: a reel has no material of its own,
    /// while its leaves and ordinary media have one, and the answer is known by `inner`.
    fn material_end_frames(&mut self, media: kt_model::MediaId) -> Option<i64> {
        // the reel itself has no edge: its silence is given by its own sub-mix
        if self.reels.keys().any(|k| k.0 == media) {
            return None;
        }
        self.inner.material_end_frames(media)
    }

    fn begin_clip(&mut self, id: kt_model::ClipId) {
        self.cur = Some(id);
        self.inner.begin_clip(id);
    }

    fn audio_burst(
        &mut self,
        media: kt_model::MediaId,
        idx: i64,
    ) -> Option<std::sync::Arc<kt_media::AudioBurst>> {
        if !self.reels.keys().any(|k| k.0 == media) {
            return self.inner.audio_burst(media, idx);
        }
        // The reader MUST be declared: without it five instances of one
        // reel would share one stream. We catch it in debug, and in release we honestly
        // hand out silence, not someone else’s sound.
        debug_assert!(self.cur.is_some(), "submix read without begin_clip: reader unknown");
        let who = self.cur?;
        // The EDIT is chosen by the READER (R.59, stage 3): the address of the reel is shared,
        // while the sub-mix of an instance with overrides is its own.
        let key = self.keys.get(&who).copied().filter(|k| k.0 == media)?;
        let reel = self.reels.get(&key)?;
        let (preroll, block) = self.prerolls.get(&key).copied().unwrap_or((0, 1));
        let raw = self.raw;
        let st = {
            let e = self.states.entry(who);
            use std::collections::hash_map::Entry;
            match e {
                Entry::Occupied(o) => o.into_mut(),
                Entry::Vacant(v) => {
                    v.insert(crate::fxstore::NestStream::new_for(key, preroll, block))
                }
            }
        };
        if st.seq != key {
            *st = crate::fxstore::NestStream::new_for(key, preroll, block);
        }
        st.preroll = preroll;
        st.block = block;
        // 1) already computed
        if let Some(b) = st.cached(idx) {
            return Some(b);
        }
        // 2) catch up forward — not further than a block: further it is cheaper to move
        let gap = block.max(1) as i64;
        let catch_up = matches!(st.next, Some(n) if n <= idx && idx - n < gap);
        if !catch_up {
            // 3) the CANONICAL MOVE from the block boundary minus the pre-roll
            st.rewind();
            st.next = Some(st.block_start(idx) - preroll as i64);
        }
        let mut from = st.next.unwrap_or(idx);
        // there is no point computing a pre-roll to the left of the reel’s zero: there is silence
        if from < 0 {
            from = 0;
            st.next = Some(0);
        }
        let mut out = None;
        for i in from..=idx {
            let b = Self::produce(reel, self.inner, st, i, raw);
            if i == idx {
                out = Some(b.clone());
            } else {
                self.warmups += 1;
            }
            st.push(i, b);
        }
        out
    }
}

/// Overlap maps BY TRACK NUMBER: the lookup goes on every frame,
/// therefore an array here, not a hash table.
type OverlapMap = Vec<LaneCover>;

/// The overlap map of one track (R.43, item 305).
///
/// `overlap_weight` asks one thing: which events of THIS track cover
/// the frame. The set changes only at event boundaries, so it can be
/// computed ONCE by a sweep, and thereafter answered by a binary search. Previously the function scanned
/// the WHOLE edit ON EVERY FRAME: on the user’s project
/// (1973 events) that is 1973 × 48000 × the number of active clip checks per
/// second, and the cost of PLACE grew linearly with the size of the PROJECT — an event an hour
/// away from there cost as much as the neighbouring ones. Measurement: 7 events — 3.6 ms
/// per second of sound, the same 7 plus 2000 distant ones — 92 ms.
///
/// The sets are stored in ASCENDING order of indices: `overlap_weight`
/// takes “the first two” precisely in it, and GOLDEN_AUDIO stands on this.
#[derive(Default)]
struct LaneCover {
    /// Interval boundaries, ascending; one fewer intervals.
    bounds: Vec<i64>,
    /// The covering events of each interval.
    cover: Vec<Vec<u32>>,
}

impl LaneCover {
    fn at(&self, frame: i64) -> &[u32] {
        if self.bounds.len() < 2 {
            return &[];
        }
        match self.bounds.binary_search(&frame) {
            Ok(i) if i + 1 < self.bounds.len() => &self.cover[i],
            Ok(_) => &[],
            Err(0) => &[],
            Err(i) if i < self.bounds.len() => &self.cover[i - 1],
            Err(_) => &[],
        }
    }
}

/// Sweep the track and record who covers whom.
fn build_lane_covers(clips: &[AudioClip]) -> OverlapMap {
    let mut by_lane: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    for (i, c) in clips.iter().enumerate() {
        by_lane.entry(c.lane).or_default().push(i as u32);
    }
    let lanes = clips.iter().map(|c| c.lane).max().map(|m| m as usize + 1).unwrap_or(0);
    let mut out: OverlapMap = (0..lanes).map(|_| LaneCover::default()).collect();
    for (lane, idx) in by_lane {
        // the sweep events: an end before a start at one point —
        // the event’s interval is half-open [start, end)
        let mut ev: Vec<(i64, i8, u32)> = Vec::with_capacity(idx.len() * 2);
        for &i in &idx {
            let c = &clips[i as usize];
            if c.end > c.start {
                ev.push((c.start, 1, i));
                ev.push((c.end, 0, i));
            }
        }
        ev.sort_unstable_by_key(|&(p, kind, i)| (p, kind, i));
        let mut cover: Vec<Vec<u32>> = Vec::new();
        let mut bounds: Vec<i64> = Vec::new();
        let mut active: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        let mut k = 0usize;
        while k < ev.len() {
            let pos = ev[k].0;
            if let Some(&last) = bounds.last() {
                debug_assert!(pos >= last);
                cover.push(active.iter().copied().collect());
            }
            bounds.push(pos);
            while k < ev.len() && ev[k].0 == pos {
                if ev[k].1 == 1 {
                    active.insert(ev[k].2);
                } else {
                    active.remove(&ev[k].2);
                }
                k += 1;
            }
        }
        if bounds.len() >= 2 {
            debug_assert_eq!(cover.len() + 1, bounds.len());
            out[lane as usize] = LaneCover { bounds, cover };
        }
    }
    out
}

/// Which events can overlap with anybody at all: an event gets here
/// if at least one interval of its track covers it AND someone else.
fn overlapping_clips(clips: &[AudioClip], covers: &OverlapMap) -> Vec<bool> {
    let mut out = vec![false; clips.len()];
    for lane in covers {
        for set in &lane.cover {
            if set.len() > 1 {
                for &i in set {
                    out[i as usize] = true;
                }
            }
        }
    }
    out
}

/// The clip’s weight from OVERLAPPING with its neighbours on the track (the return of the v1 semantics,
/// removed in R.24 together with the legality of overlaps; item 2:
/// “equal-power crossfade when two clips of one audio track overlap, 1/N
/// for 3+”). On butted clips it is inert — weight 1.0, therefore projects with explicit
/// transitions (and the GOLDEN references) do not change.
fn overlap_weight(clips: &[AudioClip], covers: &OverlapMap, ci: usize, frame: i64) -> f32 {
    let lane = clips[ci].lane;
    let mut first = usize::MAX;
    let mut second = usize::MAX;
    let mut n = 0usize;
    let here = covers.get(lane as usize).map(|m| m.at(frame)).unwrap_or(&[]);
    for &i in here {
        let i = i as usize;
        let c = &clips[i];
        {
            debug_assert!(c.lane == lane && c.start <= frame && frame < c.end);
            if n == 0 {
                first = i;
            } else if n == 1 {
                second = i;
            }
            n += 1;
        }
    }
    match n {
        0 | 1 => 1.0,
        2 => {
            let (a, b) = (&clips[first], &clips[second]);
            let ov_start = a.start.max(b.start);
            let ov_end = a.end.min(b.end);
            let t = if ov_end > ov_start {
                (frame - ov_start) as f64 / (ov_end - ov_start) as f64
            } else {
                0.5
            };
            // equal-power: the early one decays √(1−t), the late one rises √t
            if ci == first {
                ((1.0 - t).max(0.0)).sqrt() as f32
            } else {
                (t.max(0.0)).sqrt() as f32
            }
        }
        n => 1.0 / n as f32,
    }
}

impl AudioRenderer {
    pub fn new(project: &Project) -> AudioRenderer {
        AudioRenderer::build_reel(project, None, &[], 0)
    }

    fn build_reel(
        project: &Project,
        seq: Option<kt_model::MediaId>,
        overrides: &[kt_model::Override],
        depth: usize,
    ) -> AudioRenderer {
        // morpher layers are already applied (source substitution, live FX batch) —
        // the engine does not know about the morpher (H1); attachments are NOT expanded: each
        // has its own sub-mix (R.50, B4)
        let resolved = project.resolve_reel(seq, overrides);
        let effective = &resolved.clips;
        // TRACK MULTIPLIER (R.62, change B): the level in dB, muting and
        // solo by a set of ITS kind. This is yet another SCALAR after the weights and
        // the gain of the event, therefore it is folded with the `gain` of the clip into one
        // number: the order of f32 summation does not change by a single tick, and
        // GOLDEN_AUDIO stands (a stage with its own buffer is needed only
        // for a track chain — it comes separately).
        //
        // Muting is a MULTIPLIER of 0, not a removal of the clip from the graph: the FX state
        // of the event stays alive, and removing the mute does not start from a cold reverb
        // (invariant 3).
        let tracks: &[kt_model::Track] = match seq.and_then(|id| project.sequence(id)) {
            Some(s) => &s.tracks,
            None => &project.tracks,
        };
        let solo_on = tracks.iter().any(|t| t.kind == kt_model::LaneKind::Audio && t.solo);
        // STAGES: tracks with a non-empty chain. Empty — not a single extra
        // branch in the render, and the order of f32 summation is as before.
        let empty_auto = kt_fx::AutoProfiles::default();
        let mut stages: Vec<TrackStage> = Vec::new();
        {
            for (lane, t) in
                tracks.iter().filter(|t| t.kind == kt_model::LaneKind::Audio).enumerate()
            {
                let lane = lane as u32;
                let fx = t.active_fx();
                if !fx.is_empty() {
                    // keys are LIVE: the track exists and it is audio. The track number
                    // (lane) is computed by its rank among the audio tracks — in the same way
                    // as the rest of the mix computes it.
                    let keys: Vec<(usize, kt_model::TrackId, u32)> = fx
                        .iter()
                        .enumerate()
                        .filter_map(|(slot, f)| match f {
                            kt_model::FxInstance::Compressor { key: Some(k), .. } => {
                                let lane = tracks
                                    .iter()
                                    .filter(|t| t.kind == kt_model::LaneKind::Audio)
                                    .position(|t| t.id == *k)?;
                                Some((slot, *k, lane as u32))
                            }
                            _ => None,
                        })
                        .collect();
                    stages.push(TrackStage {
                        id: t.id,
                        lane,
                        la: kt_fx::chain_lookahead(&fx, &empty_auto),
                        fx,
                        keys,
                    });
                }
            }
            // THE ORDER OF STAGES — by the key dependency: a key track is computed
            // before its target. The sort is STABLE: with equal dependencies
            // the order stays as before (the row rank), otherwise the order of
            // f32 summation and GOLDEN_AUDIO would shift (R.65).
            //
            // The model does not skip the cycle (t772); here it would not loop anyway —
            // unprocessed stages are appended in the former
            // order, and the computation of the key breaks the recursion at the first level.
            if stages.iter().any(|s| !s.keys.is_empty()) {
                let mut done: Vec<TrackStage> = Vec::with_capacity(stages.len());
                let mut rest: Vec<TrackStage> = std::mem::take(&mut stages);
                while !rest.is_empty() {
                    // a stage is ready, none of whose keys is waiting its
                    // turn; we take ALL the ready ones at once, IN THE FORMER
                    // ORDER — this is what stability is
                    let ready = |s: &TrackStage, rest: &[TrackStage]| {
                        s.keys.iter().all(|(_, k, _)| !rest.iter().any(|r| r.id == *k))
                    };
                    let any = rest.iter().any(|s| ready(s, &rest));
                    if !any {
                        done.append(&mut rest); // cycle: leave as it is
                        break;
                    }
                    let snapshot = rest.clone();
                    let (now, later): (Vec<TrackStage>, Vec<TrackStage>) =
                        rest.into_iter().partition(|s| ready(s, &snapshot));
                    done.extend(now);
                    rest = later;
                }
                stages = done;
            }
        }
        let track_gain = |lane: u32| -> f32 {
            let Some(t) =
                kt_model::track_index(tracks, kt_model::LaneKind::Audio, lane).map(|i| &tracks[i])
            else {
                return 1.0;
            };
            if t.mute || (solo_on && !t.solo) {
                return 0.0;
            }
            db_to_linear(t.gain_db as f32)
        };
        // The SALT of the source address (R.59, stage 3) — the same as for video:
        // a sounding generator (SMPTE with a tone) inside a reel must hear
        // the values of ITS OWN instance.
        let salt = crate::video::gen_salt(
            match seq.and_then(|id| project.sequence(id)) {
                Some(s) => &s.published,
                None => &project.published,
            },
            overrides,
        );
        let mut clips: Vec<AudioClip> = effective
            .iter()
            .filter(|c| c.kind == ClipKind::Audio)
            .map(|c: &ClipEvent| {
                let start = c.start.to_audio_frames();
                let end = (c.start + c.duration).to_audio_frames();
                let active = c.active_fx(); // bypass slots are filtered out (item 2)
                // the clip’s curves into the SOURCE domain: the audio stream further on sees
                // only numbers (R.33)
                // REVERSE enters the normalization (R.39): the UI writes the key
                // mirrored (`clip_material_at`), and without the mirror here the DSP
                // would run the curve backwards
                let auto = kt_model::AutoProfiles::from_curves_rev(
                    &c.active_fx_auto(),
                    c.source_in.to_audio_frames(),
                    (end - start).max(1),
                    c.reversed,
                );
                let tail = kt_fx::chain_tail(&active, &auto).to_audio_frames();
                AudioClip {
                    id: c.id,
                    start,
                    end,
                    ext_end: end + tail,
                    source_in: c.source_in.to_audio_frames(),
                    lane: c.lane,
                    gain: db_to_linear(c.gain_db) * track_gain(c.lane),
                    // the address of the SOURCE, not the media (R.59, stage 2): for a
                    // sounding generator (SMPTE with a tone, a count with a
                    // beep) the values are on the event, so the stream is its own
                    media: crate::video::clip_source_addr_in(c, salt),
                    addr: kt_fx::chain_addr_knots(&active, &auto, (end - start).max(1))
                        .map(std::sync::Arc::new),
                    fx: active,
                    auto,
                    reversed: c.reversed,
                    fade_in: None,
                    fade_out: None,
                    fade_in_curve: Default::default(),
                    fade_out_curve: Default::default(),
                    head_ext: 0,
                    tail_ext: 0,
                    fade_in_excl: false,
                    fade_out_excl: false,
                    nest: project.sequence(c.media).map(|_| {
                        (c.media, crate::instance_content(project, c.media, &c.overrides))
                    }),
                }
            })
            .collect();
        // Crossfade windows from transitions (R.24): the window boundaries are floored into frames
        // by THE SAME conversion as the clip boundaries — for a converted
        // overlap the window coincides bit for bit with the old weight_at.
        // Transitions belong to ITS OWN reel (item 399: previously the root stood here, and a transition
        // inside a nested sequence silently did not crossfade the sound).
        let reel: &dyn kt_model::Reel = match seq.and_then(|id| project.sequence(id)) {
            Some(s) => s,
            None => project,
        };
        for t in resolved.transitions.iter() {
            // An OVERLAID transition (R.51, item 399): the window is the overlap zone itself,
            // there are no handles (the material on both sides is real),
            // the curve works instead of the built-in equal-power overlap
            if let Some((pa, pb)) = t.overlap_pair(reel) {
                let w0 = pb.start.to_audio_frames();
                let w1 = (pa.start + pa.duration).to_audio_frames();
                if w1 <= w0 {
                    continue;
                }
                if let Some(a) =
                    clips.iter_mut().find(|c| kt_model::same_clip(c.id, pa.id) && c.end == w1)
                {
                    a.fade_out = Some((w0, w1));
                    a.fade_out_curve = t.audio;
                    a.fade_out_excl = true;
                }
                if let Some(b) =
                    clips.iter_mut().find(|c| kt_model::same_clip(c.id, pb.id) && c.start == w0)
                {
                    b.fade_in = Some((w0, w1));
                    b.fade_in_curve = t.audio;
                    b.fade_in_excl = true;
                }
                continue;
            }
            let Some(joint) = t.joint(reel) else { continue };
            let w0 = (joint - t.in_offset).to_audio_frames();
            let w1 = (joint + t.out_offset).to_audio_frames();
            let jf = joint.to_audio_frames();
            if w1 <= w0 {
                continue;
            }
            // a fade (R.25) — a transition with one live end: the weight is computed the same way,
            // the “partner” is simply absent, and the material opposite is silence;
            // a target cut by a morpher (R.35): the junction is at the piece whose
            // boundary stands on it; the id of the original is carried by piece No. 0
            if let Some(a) = t.a.and_then(|id| {
                clips.iter_mut().find(|c| kt_model::same_clip(c.id, id) && c.end == jf)
            }) {
                // reversed FX: the mirrored source anchor is tied to
                // window_end — we do not extend the handle (a v1 limitation, item 136),
                // the fade weight is applied as is
                let ext = if a.reversed && !a.fx.is_empty() { 0 } else { w1 - a.end };
                a.fade_out = Some((w0, w1));
                a.fade_out_curve = t.audio;
                a.tail_ext = ext.max(0);
                a.ext_end += a.tail_ext;
            }
            if let Some(b) = t.b.and_then(|id| {
                clips.iter_mut().find(|c| kt_model::same_clip(c.id, id) && c.start == jf)
            }) {
                let ext = if b.reversed && !b.fx.is_empty() { 0 } else { b.start - w0 };
                b.fade_in = Some((w0, w1));
                b.fade_in_curve = t.audio;
                b.head_ext = ext.max(0);
            }
        }
        // The stable order is part of the determinism and of the crossfade rule
        // (the “early” clip is determined by (start, media)).
        clips.sort_by_key(|c| (c.start, c.end, c.lane, c.media));
        let (index, index_from) = Self::build_index(&clips);
        // overlaying is a property of the REEL on which the events lie
        let overlap_layers = match seq.and_then(|id| project.sequence(id)) {
            Some(s) => s.overlap_layers,
            None => project.overlap_layers,
        };
        let covers = if overlap_layers { build_lane_covers(&clips) } else { OverlapMap::new() };
        let overlaps = overlapping_clips(&clips, &covers);
        let mut nests = std::collections::HashMap::new();
        if depth < kt_model::MAX_NEST_DEPTH {
            for c in effective.iter().filter(|c| c.kind == ClipKind::Audio) {
                let Some(id) = project.sequence(c.media).map(|s| s.id) else { continue };
                let key = (id, crate::instance_content(project, id, &c.overrides));
                nests.entry(key).or_insert_with(|| {
                    AudioRenderer::build_reel(project, Some(id), &c.overrides, depth + 1)
                });
            }
        }
        AudioRenderer {
            clips,
            index,
            index_from,
            covers,
            overlaps,
            overlap_layers,
            opts: kt_fx::ChainOpts { pitch_engine: project.settings.pitch_engine },
            stages,
            nests,
        }
    }

    /// Distribute the events into per-second buckets. An event goes into each
    /// bucket that its FULL span touches — the body, the left “handle” and
    /// the FX tail: exactly what the filter in `render_frames_with` checks.
    fn build_index(clips: &[AudioClip]) -> (Vec<Vec<u32>>, i64) {
        if clips.is_empty() {
            return (Vec::new(), 0);
        }
        let lo = clips.iter().map(|c| c.start - c.head_ext).min().unwrap_or(0);
        let hi = clips.iter().map(|c| c.ext_end).max().unwrap_or(0);
        let from = lo.div_euclid(INDEX_BUCKET_FRAMES) * INDEX_BUCKET_FRAMES;
        let n = ((hi - from).max(0) / INDEX_BUCKET_FRAMES + 1) as usize;
        let mut index = vec![Vec::new(); n];
        for (i, c) in clips.iter().enumerate() {
            let a = ((c.start - c.head_ext - from) / INDEX_BUCKET_FRAMES).max(0) as usize;
            let b = (((c.ext_end - 1 - from) / INDEX_BUCKET_FRAMES).max(0) as usize).min(n - 1);
            for bucket in index.iter_mut().take(b + 1).skip(a) {
                bucket.push(i as u32);
            }
        }
        (index, from)
    }

    /// The indices of the events touching `range`, IN ASCENDING ORDER —
    /// the same set and the same order that a scan of all clips would give
    /// (the guard — test 292).
    fn active_clips(&self, range: &std::ops::Range<i64>) -> Vec<usize> {
        let hit = |i: usize| {
            let c = &self.clips[i];
            c.start - c.head_ext < range.end && c.ext_end > range.start
        };
        if self.index.is_empty() {
            return Vec::new();
        }
        let a = (range.start - self.index_from).div_euclid(INDEX_BUCKET_FRAMES);
        let b = (range.end - 1 - self.index_from).div_euclid(INDEX_BUCKET_FRAMES);
        let (a, b) = (a.max(0) as usize, (b.max(0) as usize).min(self.index.len() - 1));
        if a >= self.index.len() {
            return Vec::new();
        }
        if a == b {
            return self.index[a].iter().map(|&i| i as usize).filter(|&i| hit(i)).collect();
        }
        // several buckets: a merge of sorted lists with deduplication —
        // the order must remain ascending
        let mut out: Vec<usize> = Vec::new();
        for bucket in &self.index[a..=b] {
            for &i in bucket {
                let i = i as usize;
                if out.last() != Some(&i) && hit(i) {
                    out.push(i);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// The pre-roll and the block size of the sub-mix of THIS reel in bursts (R.55, C4).
    ///
    /// Both quantities are PURE FUNCTIONS OF THE PROJECT, and this is not pedantry: if they depended
    /// on the memory budget or on the device, the output bytes would depend on them too, and audio
    /// must be byte-for-byte identical always and everywhere
    /// (invariant 11).
    ///
    /// The pre-roll is the tail of the longest-tailed chain of the reel, recursively together with
    /// the pre-roll of its own attachments. **For a reel without tailed
    /// effects it is ZERO, and reverse costs exactly as much as
    /// forward playback**; it becomes expensive only where
    /// a reverb or a delay stands inside — and exactly as much as its
    /// tail is long.
    fn reel_preroll_bursts(&self) -> u32 {
        let mut tail = 0i64;
        for c in &self.clips {
            if !c.fx.is_empty() {
                tail = tail.max(kt_fx::chain_tail(&c.fx, &c.auto).to_audio_frames());
            }
            if let Some(sub) = c.nest.and_then(|k| self.nests.get(&k)) {
                tail = tail.max(sub.reel_preroll_bursts() as i64 * AUDIO_BURST_FRAMES as i64);
            }
        }
        if tail <= 0 {
            return 0;
        }
        let p = tail.max(kt_fx::MIN_PREROLL_FRAMES);
        let b = AUDIO_BURST_FRAMES as i64;
        ((p + b - 1) / b).clamp(1, 4096) as u32
    }

    /// The pre-roll and the block of the reel that this instance looks at.
    fn nest_preroll(&self, key: crate::video::NestKey) -> (u32, u32) {
        match self.nests.get(&key) {
            Some(sub) => {
                let p = sub.reel_preroll_bursts();
                // The block is four times longer than the pre-roll: the overheads of a move when
                // reading BACKWARD come out exactly (W + P)/W = 1.25×. The ceiling and
                // the floor come from the memory of the ring (8 KiB per burst), but THE SIZE ITSELF
                // does not depend on it: it is a function of the reel’s tail.
                let w = (4 * p).clamp(32, 256);
                (p, w)
            }
            None => (0, 1),
        }
    }

    fn nest_prerolls(&self) -> std::collections::HashMap<crate::video::NestKey, (u32, u32)> {
        self.nests.keys().map(|k| (*k, self.nest_preroll(*k))).collect()
    }

    /// Which sub-renderer reads THIS instance. The key is a pair (reel,
    /// edit): at one media address there may be several of them since R.59.
    fn nest_keys(&self) -> std::collections::HashMap<kt_model::ClipId, crate::video::NestKey> {
        self.clips.iter().filter_map(|c| c.nest.map(|k| (c.id, k))).collect()
    }

    /// How many SUB-MIXES this reel holds (R.59, stage 3): two
    /// instances without overrides must share one.
    pub fn nest_count(&self) -> usize {
        self.nests.len()
    }

    /// Synchronize the FX state with this graph (called by the owner
    /// of the store — the audio render thread — after each rebuild of the graph).
    pub fn sync_fx(&self, store: &mut crate::fxstore::FxStateStore) {
        for c in &self.clips {
            store.sync_clip(c.id, &c.fx, &c.auto, c.source_in, c.end - c.start, self.opts);
        }
        let alive: std::collections::HashSet<_> = self.clips.iter().map(|c| c.id).collect();
        store.retain_clips(&|id| alive.contains(&id));
        // TRACK CHAINS — by the same rule (R.62): the same structure → the
        // same state; a deleted track takes its own with it
        for st in &self.stages {
            store.sync_track(st.id, &st.fx, self.opts);
        }
        let live_tracks: std::collections::HashSet<_> = self.stages.iter().map(|s| s.id).collect();
        store.retain_tracks(&|id| live_tracks.contains(&id));
        // KEY BRANCHES (R.65): the target has its own sub-store, and it is synchronized
        // by exactly the same rule — the same chains, the same states
        let with_keys: Vec<(kt_model::TrackId, Vec<(kt_model::TrackId, u32)>)> = self
            .stages
            .iter()
            .filter(|s| !s.keys.is_empty())
            .map(|s| (s.id, s.keys.iter().map(|(_, k, lane)| (*k, *lane)).collect()))
            .collect();
        for (target, keys) in &with_keys {
            let opts = self.opts;
            let sub = store.key_mut(*target);
            for (key_id, key_lane) in keys {
                for c in self.clips.iter().filter(|c| c.lane == *key_lane) {
                    sub.sync_clip(c.id, &c.fx, &c.auto, c.source_in, c.end - c.start, opts);
                }
                if let Some(ks) = self.stages.iter().find(|s| s.id == *key_id) {
                    sub.sync_track(ks.id, &ks.fx, opts);
                }
            }
            let alive: std::collections::HashSet<_> = keys
                .iter()
                .flat_map(|(_, lane)| {
                    self.clips.iter().filter(move |c| c.lane == *lane).map(|c| c.id)
                })
                .collect();
            sub.retain_clips(&|id| alive.contains(&id));
            let live: std::collections::HashSet<_> = keys.iter().map(|(k, _)| *k).collect();
            sub.retain_tracks(&|id| live.contains(&id));
        }
        let targets: std::collections::HashSet<_> = with_keys.iter().map(|(t, _)| *t).collect();
        store.retain_keys(&|id| targets.contains(&id));
        // chains INSIDE reels are synchronized by the same rule: the same
        // chains — the same state objects, reverb tails live on (invariant 3)

        for c in &self.clips {
            let Some(key) = c.nest else { continue };
            let Some(sub) = self.nests.get(&key) else { continue };
            let (preroll, block) = self.nest_preroll(key);
            sub.sync_fx(&mut store.nest_mut(c.id, key, preroll, block).store);
        }
        let live: std::collections::HashSet<_> =
            self.clips.iter().filter(|c| c.nest.is_some()).map(|c| c.id).collect();
        store.retain_nests(&|id| live.contains(&id));
    }

    /// Render without external FX state (for clips without FX equivalent to
    /// `render_frames_with`; FX clips get a one-off store — fine
    /// for one-off offline calls, but not for the streaming player).
    pub fn render_frames(
        &self,
        fetcher: &mut dyn AudioFetcher,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        let mut store = crate::fxstore::FxStateStore::new();
        self.sync_fx(&mut store);
        self.render_frames_with(fetcher, &mut store, start_frame, out)
    }

    /// Render a continuous range of the timeline [start_frame, start_frame+out/2)
    /// into interleaved stereo `out`. No loop logic — wrap is done by the caller
    /// (`render_looped`), splitting the range at the region boundary.
    /// `store` — the long-lived FX state of the audio render thread.
    pub fn render_frames_with(
        &self,
        fetcher: &mut dyn AudioFetcher,
        store: &mut crate::fxstore::FxStateStore,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        // A flat reel — the PREVIOUS path with not a single extra branch: it
        // is what GOLDEN_AUDIO covers.
        if self.nests.is_empty() {
            return self.render_reel(fetcher, store, start_frame, out);
        }
        // Reel states are taken OUT: the sub-render is called from the fetcher,
        // and the fetcher lives next to the store — there is no way to take
        // both mutably at the same time.
        let mut states = store.take_nests();
        let prerolls = self.nest_prerolls();
        let keys = self.nest_keys();
        let stats = {
            let mut f = NestFetcher {
                inner: fetcher,
                reels: &self.nests,
                states: &mut states,
                prerolls: &prerolls,
                keys: &keys,
                cur: None,
                warmups: 0,
                raw: false,
            };
            let mut st = self.render_reel(&mut f, store, start_frame, out);
            st.nest_warmups += f.warmups;
            st
        };
        store.put_nests(states);
        stats
    }

    fn render_reel(
        &self,
        fetcher: &mut dyn AudioFetcher,
        store: &mut crate::fxstore::FxStateStore,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        assert_eq!(out.len() % AUDIO_CHANNELS, 0);
        out.fill(0.0);
        let frames = (out.len() / AUDIO_CHANNELS) as i64;
        let range = start_frame..start_frame + frames;
        let mut stats = RenderStats::default();

        let active = self.active_clips(&range);

        // TRACK STAGES (R.62): a track with a chain — the sum of its events
        // goes through the processor and only then lands in the output. We compute
        // them FIRST and in a separate buffer; everything else — by the old path.
        if !self.stages.is_empty() {
            let st = self.render_stages(fetcher, store, &range, out);
            stats.underruns += st.underruns;
            stats.warmup_misses += st.warmup_misses;
            stats.nest_warmups += st.nest_warmups;
        }

        // Each clip's contribution is computed per burst (source), weights per sample.
        for &ci in &active {
            let clip = &self.clips[ci];
            // a TRACK-WITH-STAGE event is already computed above
            if self.stage_of(clip.lane).is_some() {
                continue;
            }
            // WHO reads — before the first read (R.55, C4): an attachment has its own
            // sub-mix stream per instance. One virtual call
            // per clip per burst, not per frame.
            fetcher.begin_clip(clip.id);
            if !clip.fx.is_empty() {
                self.render_fx_clip(ci, fetcher, store, &range, out, &mut stats);
                continue;
            }
            self.render_plain_clip(ci, fetcher, &range, out, &mut stats);
        }
        stats
    }

    /// This track's stage, if it has one.
    fn stage_of(&self, lane: u32) -> Option<&TrackStage> {
        self.stages.iter().find(|s| s.lane == lane)
    }

    /// TRACK STAGES: sum of track events → chain → into the output (R.62).
    ///
    /// The chain input leads the output by its lookahead (`la`): events are
    /// summed over the window `[from + la, to + la)`, and the processor, with its own
    /// rings, returns the output to its place — there is no shift relative to a track
    /// without a chain (R.48, item 337). The tail is honest: after the last
    /// event the input is silence, and the chain rings out.
    fn render_stages(
        &self,
        fetcher: &mut dyn AudioFetcher,
        store: &mut crate::fxstore::FxStateStore,
        range: &std::ops::Range<i64>,
        out: &mut [f32],
    ) -> RenderStats {
        let mut stats = RenderStats::default();
        let frames = (range.end - range.start) as usize;
        let mut scratch = vec![0f32; frames * AUDIO_CHANNELS];
        // the KEY buffers are reused between stages: the render path must have
        // no extra allocations
        let mut keybufs: Vec<(usize, Vec<f32>)> = Vec::new();
        let mut pool: Vec<Vec<f32>> = Vec::new();
        for st in &self.stages {
            let win = (range.start + st.la)..(range.end + st.la);
            scratch.fill(0.0);
            // track events — with the same code as all the others
            self.sum_lane(st.lane, fetcher, store, &win, &mut scratch, &mut stats);
            // KEY SIGNALS (R.65) — ON THE TARGET WINDOW: the detector and what it
            // compresses must be time-aligned, otherwise ducking
            // triggers earlier or later than the speech.
            keybufs.clear();
            for (slot, key_id, key_lane) in &st.keys {
                let mut kb = pool.pop().unwrap_or_default();
                kb.clear();
                kb.resize(frames * AUDIO_CHANNELS, 0.0);
                // KEY WINDOW — the position of THOSE SAME samples this slot processes: nodes
                // with lookahead BEFORE it return their output through
                // their rings, and the signal at the slot is older than the chain input by their
                // sum. Without this correction ducking triggered 5 ms before
                // the speech (t770, chain “limiter + keyed compressor”).
                let delay =
                    store.track_mut(st.id).map(|p| p.slot_delay(*slot)).unwrap_or(0);
                let win = (win.start - delay)..(win.end - delay);
                {
                    // its OWN branch: its own event and key-chain states —
                    // one processor cannot be run twice per frame
                    let sub = store.key_mut(st.id);
                    self.sum_lane(*key_lane, fetcher, sub, &win, &mut kb, &mut stats);
                    if let Some(p) = sub.track_mut(*key_id) {
                        // INSIDE the key branch there are no keys: a compressor with
                        // a key works here as a plain one — that is how recursion is cut
                        p.process(win.start, &mut kb);
                    }
                }
                keybufs.push((*slot, kb));
            }
            if let Some(p) = store.track_mut(st.id) {
                p.process_keyed(win.start, &mut scratch, &|slot| {
                    keybufs.iter().find(|(s, _)| *s == slot).map(|(_, b)| &b[..])
                });
            }
            for (_, b) in keybufs.drain(..) {
                pool.push(b);
            }
            for (o, v) in out.iter_mut().zip(scratch.iter()) {
                *o += *v;
            }
        }
        stats
    }

    /// The sum of events of ONE track over the window — the common code of the track's own
    /// output and of its same-track key branch. There must be no second copy of this loop:
    /// if they diverged, the key would hear something other than what the track produces.
    fn sum_lane(
        &self,
        lane: u32,
        fetcher: &mut dyn AudioFetcher,
        store: &mut crate::fxstore::FxStateStore,
        win: &std::ops::Range<i64>,
        out: &mut [f32],
        stats: &mut RenderStats,
    ) {
        for ci in self.active_clips(win) {
            let clip = &self.clips[ci];
            if clip.lane != lane {
                continue;
            }
            fetcher.begin_clip(clip.id);
            if !clip.fx.is_empty() {
                self.render_fx_clip(ci, fetcher, store, win, out, stats);
            } else {
                self.render_plain_clip(ci, fetcher, win, out, stats);
            }
        }
    }

    /// A clip without FX: direct reading of source bursts with crossfade weights.
    /// Common code of render_frames_with and render_frames_raw (reverse preview).
    /// The overlap weight for clip `ci` in frame `f` (1.0 when overlapping is
    /// off — then there are no overlaps in the model by construction).
    fn overlap_at(&self, ci: usize, f: i64) -> f32 {
        // window of an overlapping transition (R.51): the curve has already been applied by
        // `fade_weight` — the built-in equal-power overlap is muted, otherwise
        // double fade. O(1), no scan of transitions.
        let c = &self.clips[ci];
        if (c.fade_in_excl && c.fade_in.is_some_and(|(w0, w1)| f >= w0 && f < w1))
            || (c.fade_out_excl && c.fade_out.is_some_and(|(w0, w1)| f >= w0 && f < w1))
        {
            return 1.0;
        }
        if self.overlap_layers && self.overlaps[ci] {
            overlap_weight(&self.clips, &self.covers, ci, f)
        } else {
            1.0
        }
    }

    fn render_plain_clip(
        &self,
        ci: usize,
        fetcher: &mut dyn AudioFetcher,
        range: &std::ops::Range<i64>,
        out: &mut [f32],
        stats: &mut RenderStats,
    ) {
        let clip = &self.clips[ci];
        {
            let from = (clip.start - clip.head_ext).max(range.start);
            let to = (clip.end + clip.tail_ext).min(range.end);
            if from >= to {
                return;
            }
            let mut f = from;
            let mut cursor: Option<(i64, Arc<AudioBurst>)> = None; // (burst idx, data)
            let clip_len = clip.end - clip.start;
            // END OF MATERIAL (R.63 item 2) — the same edge as its start
            // above: beyond it there is no source, and silence there is not an underrun.
            let material_end = fetcher.material_end_frames(clip.media);
            while f < to {
                // reverse: local frame k reads the source from the end (item 16);
                // local beyond the body (the transition “handles”, item 136)
                // continues the formula linearly in both directions
                let local = f - clip.start;
                let src = if clip.reversed {
                    clip.source_in + (clip_len - 1 - local)
                } else {
                    clip.source_in + local
                };
                // material to the left of source zero — silence WITHOUT underrun
                // (handle longer than material — edge policy)
                if src < 0 {
                    if clip.reversed {
                        break; // src only decreases further
                    }
                    f += (-src).min(to - f);
                    continue;
                }
                // and to the RIGHT of the material's end — exactly the same (R.63 item 2)
                if material_end.is_some_and(|end| src >= end) {
                    if !clip.reversed {
                        break; // src only increases further
                    }
                    f += 1;
                    continue;
                }
                let bidx = src.div_euclid(AUDIO_BURST_FRAMES as i64);
                let within = src.rem_euclid(AUDIO_BURST_FRAMES as i64);
                // how many frames we can take from this source burst in a row:
                // forward — up to the end of the burst; in reverse — down to its start
                let take = if clip.reversed {
                    (within + 1).min(to - f)
                } else {
                    (AUDIO_BURST_FRAMES as i64 - within).min(to - f)
                };
                let burst = match &cursor {
                    Some((i, b)) if *i == bidx => Some(b.clone()),
                    _ => {
                        let b = fetcher.audio_burst(clip.media, bidx);
                        if let Some(b) = &b {
                            cursor = Some((bidx, b.clone()));
                        }
                        b
                    }
                };
                match burst {
                    Some(b) => {
                        for k in 0..take {
                            let w =
                                fade_weight(clip, f + k) * self.overlap_at(ci, f + k) * clip.gain;
                            let oi = ((f + k - range.start) as usize) * AUDIO_CHANNELS;
                            let sk = if clip.reversed { within - k } else { within + k };
                            let si = (sk as usize) * AUDIO_CHANNELS;
                            for ch in 0..AUDIO_CHANNELS {
                                out[oi + ch] += b.data[si + ch] * w;
                            }
                        }
                    }
                    // a missing burst in the HANDLE zone (beyond the media edge) —
                    // not an underrun: we count only the body frames
                    None => {
                        stats.underruns +=
                            (clip.end.min(f + take) - clip.start.max(f)).max(0) as u64;
                    }
                }
                f += take;
            }
        }
    }

    /// Render WITHOUT FX chains — the reverse preview of transport (J4 v1.4): clips
    /// with FX play dry, because streaming states (reverb/warp) are meaningless when
    /// material is fed BACKWARDS FORWARD — a reverb tail would sound “from the
    /// future” (Vegas does the same in reverse scrubbing). Normal playback
    /// and export do NOT go through this path (invariants 2/7 intact).
    pub fn render_frames_raw(
        &self,
        fetcher: &mut dyn AudioFetcher,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        assert_eq!(out.len() % AUDIO_CHANNELS, 0);
        out.fill(0.0);
        let frames = (out.len() / AUDIO_CHANNELS) as i64;
        let range = start_frame..start_frame + frames;
        // REELS HERE TOO (R.55, C4). Until this round the raw path did not
        // wrap the fetcher, and `audio_burst(reel address)` went to the regular
        // fetcher, where there is no such source: reverse transport over an attachment
        // gave SILENCE and grew underruns per frame. At the same time
        // `wanted_audio_bursts_raw` honestly recursed inside — the request
        // went out, but the read did not reach it (violation of invariant 4).
        //
        // The raw path's sub-mix is also DRY (the “as in Vegas” decision), and dry
        // has no state at all: a burst is a pure function of the index — no blocks,
        // no pre-roll needed.
        if self.nests.is_empty() {
            return self.render_raw_reel(fetcher, &range, out);
        }
        let mut states = std::collections::HashMap::new();
        let prerolls = self.nest_prerolls();
        let keys = self.nest_keys();
        let mut f = NestFetcher {
            inner: fetcher,
            reels: &self.nests,
            states: &mut states,
            prerolls: &prerolls,
            keys: &keys,
            cur: None,
            warmups: 0,
            raw: true,
        };
        self.render_raw_reel(&mut f, &range, out)
    }

    fn render_raw_reel(
        &self,
        fetcher: &mut dyn AudioFetcher,
        range: &std::ops::Range<i64>,
        out: &mut [f32],
    ) -> RenderStats {
        let mut stats = RenderStats::default();
        for ci in self.active_clips(range) {
            // the raw path looks at the BODY with handles, not at the FX tail
            if self.clips[ci].end + self.clips[ci].tail_ext > range.start {
                fetcher.begin_clip(self.clips[ci].id);
                self.render_plain_clip(ci, fetcher, range, out, &mut stats);
            }
        }
        stats
    }

    /// Loop variant of render_frames_raw: the linear position is wrapped into the region
    /// (rem_euclid is correct also for BACKWARD motion — negative offsets).
    pub fn render_looped_raw(
        &self,
        fetcher: &mut dyn AudioFetcher,
        region: std::ops::Range<i64>,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        assert!(!region.is_empty());
        let len = region.end - region.start;
        let total = (out.len() / AUDIO_CHANNELS) as i64;
        let mut stats = RenderStats::default();
        let mut done = 0i64;
        while done < total {
            let tl = region.start + (start_frame - region.start + done).rem_euclid(len);
            let chunk = (region.end - tl).min(total - done);
            let sub = &mut out
                [(done as usize) * AUDIO_CHANNELS..((done + chunk) as usize) * AUDIO_CHANNELS];
            let s = self.render_frames_raw(fetcher, tl, sub);
            stats.underruns += s.underruns;
            done += chunk;
        }
        stats
    }

    /// FX clip: a clip-local stream through ClipProcessor (warp + reverb + gain of
    /// the chain), then crossfade weights (within the real clip) and the clip gain.
    /// In the tail zone (past end) the weight is 1.0 — the reverb rings out under
    /// its neighbours without weighting.
    fn render_fx_clip(
        &self,
        ci: usize,
        fetcher: &mut dyn AudioFetcher,
        store: &mut crate::fxstore::FxStateStore,
        range: &std::ops::Range<i64>,
        out: &mut [f32],
        stats: &mut RenderStats,
    ) {
        let clip = &self.clips[ci];
        let from = (clip.start - clip.head_ext).max(range.start);
        let to = clip.ext_end.min(range.end);
        if from >= to {
            return;
        }
        // source window of the clip: beyond its bounds — legitimate silence.
        // A single calculation with wanted_audio_bursts (chain_source_window_ext); dry
        // past the END of the MATERIAL is muted by ClipProcessor itself (output gate, item 138).
        // For reversed-FX clips the material = the body (handles disabled, item 136):
        // the mirror anchor of the source is tied to window_end.
        let clip_len = clip.end - clip.start;
        let material = -clip.head_ext..clip_len + clip.tail_ext;
        // The boundary is CONSUMPTION (R.57): the resampler cursor lives on it, and the
        // addresses under the signed curve go both left and right. For a
        // monotonic chain it is the same number as before.
        let src_window_end = clip.source_in
            + kt_fx::chain_consumption_window_ext(
                &clip.fx,
                &clip.auto,
                clip_len,
                material.clone(),
                material,
            )
            .end;
        // MIRROR AXIS ≠ window end (R.43, item 292). The window includes the resampler's margin
        // (`WARP_SOURCE_MARGIN_FRAMES`, 8192 frames = 170.7 ms) —
        // this is the correct boundary “silence beyond”, but an unfit axis: mirroring
        // from it, a slowed-down reversed clip played material shifted by
        // these 170 ms. The axis is the actually consumed `used`, THE SAME value
        // as for video (`ClipWarp::used`). For a chain without warp both coincide,
        // so GOLDEN_AUDIO does not move.
        let src_mirror_end =
            clip.source_in + kt_fx::chain_source_used(&clip.fx, &clip.auto, clip_len);
        let media = clip.media;
        let mut pull_underruns = 0u64;
        let mut warmup_misses = 0u64;
        let Some(proc) = store.get_mut(clip.id) else { return };
        proc.set_material_ext(clip.head_ext, clip.tail_ext);
        let local = from - clip.start;
        let count = (to - from) as usize;
        let mut buf = vec![0f32; count * AUDIO_CHANNELS];
        {
            let reversed = clip.reversed;
            let win = SrcWindow {
                start: clip.source_in,
                end: src_window_end,
                mirror_end: src_mirror_end,
                addr: clip.addr.as_deref().map(|v| v.as_slice()),
            };
            let mut pull = |kind: kt_fx::PullKind, src_frame: i64, dst: &mut [f32]| -> bool {
                // pre-roll misses are counted separately: they are not heard
                let n = match kind {
                    kt_fx::PullKind::Output => &mut pull_underruns,
                    kt_fx::PullKind::Warmup => &mut warmup_misses,
                };
                fill_source(fetcher, media, src_frame, win, reversed, dst, n)
            };
            proc.read(local, &mut buf, &mut pull);
        }
        stats.underruns += pull_underruns;
        stats.warmup_misses += warmup_misses;
        for k in 0..count as i64 {
            let f = from + k;
            // weights act on the body and handles (transition windows); in the
            // FX tail zone (past the material) — 1.0: the reverb rings out under
            // its neighbours without weighting.
            let w = if f < clip.end + clip.tail_ext { fade_weight(clip, f) } else { 1.0 }
                * self.overlap_at(ci, f)
                * clip.gain;
            let oi = ((f - range.start) as usize) * AUDIO_CHANNELS;
            let bi = (k as usize) * AUDIO_CHANNELS;
            for ch in 0..AUDIO_CHANNELS {
                out[oi + ch] += buf[bi + ch] * w;
            }
        }
    }

    /// Render with a loop region: the output stream of `frames` frames, timeline
    /// positions wrapped over [region) per sample (the boundary may not coincide
    /// with a burst). Test 14: continuity across the wrap.
    pub fn render_looped(
        &self,
        fetcher: &mut dyn AudioFetcher,
        region: std::ops::Range<i64>,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        let mut store = crate::fxstore::FxStateStore::new();
        self.sync_fx(&mut store);
        self.render_looped_with(fetcher, &mut store, region, start_frame, out)
    }

    pub fn render_looped_with(
        &self,
        fetcher: &mut dyn AudioFetcher,
        store: &mut crate::fxstore::FxStateStore,
        region: std::ops::Range<i64>,
        start_frame: i64,
        out: &mut [f32],
    ) -> RenderStats {
        assert!(!region.is_empty());
        let len = region.end - region.start;
        let total = (out.len() / AUDIO_CHANNELS) as i64;
        let mut stats = RenderStats::default();
        let mut done = 0i64;
        while done < total {
            let tl = region.start + (start_frame - region.start + done).rem_euclid(len);
            let chunk = (region.end - tl).min(total - done);
            let sub = &mut out
                [(done as usize) * AUDIO_CHANNELS..((done + chunk) as usize) * AUDIO_CHANNELS];
            let s = self.render_frames_with(fetcher, store, tl, sub);
            stats.underruns += s.underruns;
            stats.warmup_misses += s.warmup_misses;
            done += chunk;
        }
        stats
    }

    /// Ranges of source bursts needed to play the timeline [range) (in frames).
    /// The basis of prefetch: wanted − coverage → submit.
    pub fn wanted_audio_bursts(
        &self,
        range: std::ops::Range<i64>,
    ) -> Vec<(kt_model::MediaId, std::ops::Range<i64>)> {
        self.wanted_audio_bursts_for(range, false)
    }

    /// The same for the RAW path (reverse transport, `render_frames_raw`).
    ///
    /// R.43, item 295: the raw path plays ALL events through `render_plain_clip`,
    /// i.e. it reads the source LINEARLY and knows nothing of warp at all (the decision
    /// “dry, like Vegas”, audio.rs). And the prefetch counted the window BY WARP — and on
    /// each slowed-down event it warmed the wrong bursts. A miss here is
    /// counted in FRAMES (`render_plain_clip`), i.e. up to 1024 at a time —
    /// exactly how “underruns grow” in reverse. A single source of truth for
    /// prefetch and render — the same rule as the regular path (regression
    /// item 13, test 17).
    pub fn wanted_audio_bursts_raw(
        &self,
        range: std::ops::Range<i64>,
    ) -> Vec<(kt_model::MediaId, std::ops::Range<i64>)> {
        self.wanted_audio_bursts_for(range, true)
    }

    fn wanted_audio_bursts_for(
        &self,
        range: std::ops::Range<i64>,
        raw: bool,
    ) -> Vec<(kt_model::MediaId, std::ops::Range<i64>)> {
        let mut out = Vec::new();
        for c in &self.clips {
            // “handles” of transitions are included in wanted — prefetch warms them (item 136)
            let from = (c.start - c.head_ext).max(range.start);
            let to = (c.end + c.tail_ext).min(range.end);
            if from >= to {
                continue;
            }
            let (s0, s1) = if raw || c.fx.is_empty() {
                if c.reversed {
                    // mirror: the end of the timeline range reads the START of the chunk
                    let len = c.end - c.start;
                    (c.source_in + (len - (to - c.start)), c.source_in + (len - (from - c.start)))
                } else {
                    // without FX: the source flows 1:1 with the timeline
                    (c.source_in + (from - c.start), c.source_in + (to - c.start))
                }
            } else if c.reversed {
                // FX+reverse: the consumption window is mirrored around the CONSUMED
                // end — the same axis as in the render (R.43, item 292); handles
                // disabled (item 136), from/to here coincide with the body.
                // The absolute clamp below (`s0.max(0)`) cuts an overrun past the file's zero
                // frame: let these two calculations diverge — an endless cache miss.
                let len = c.end - c.start;
                let used = kt_fx::chain_source_used(&c.fx, &c.auto, len);
                let w = kt_fx::chain_source_window(
                    &c.fx,
                    &c.auto,
                    len,
                    (from - c.start)..(to - c.start),
                );
                (c.source_in + used - w.end, c.source_in + used - w.start)
            } else {
                // with FX: the warp consumes the source by rate_integral, not
                // linearly; wanted must cover the REAL consumption
                // (regression item 13: a linear mapping warmed the wrong bursts →
                // deeper into the clip an endless cache miss → silence)
                let len = c.end - c.start;
                let material = -c.head_ext..len + c.tail_ext;
                let w = kt_fx::chain_source_window_ext(
                    &c.fx,
                    &c.auto,
                    len,
                    material,
                    (from - c.start)..(to - c.start),
                );
                (c.source_in + w.start, c.source_in + w.end.max(w.start + 1))
            };
            // a handle may extend past the source's zero frame — there is silence,
            // bursts are not requested
            let s0 = s0.max(0);
            let s1 = s1.max(s0 + 1);
            // REQUEST GOES DEEPER (R.50): a source reel has no samples, it has
            // content. We ask it about the SAME consumption window, but in its
            // own frames — so prefetch and render share one truth (item 13).
            if let Some((key, sub)) = c.nest.and_then(|k| self.nests.get(&k).map(|s| (k, s))) {
                // The request goes DEEPER, but the sub-mix producer reads the reel
                // WIDER than the consumption window: when reading BACKWARD each burst is
                // a shift, and a shift starts from the block boundary minus
                // the pre-roll. Not widening the request means missing the
                // cache at every shift — an endless miss (item 13, t17).
                // The rule is ONE for render and prefetch: if they diverged, it
                // would be the same class of defect.
                let (preroll, block) = self.nest_preroll(key);
                // MONOTONICITY GATE (R.57): backward reading is done by both the mirror
                // instance and the signed speed — the producer does not care
                // what exactly reversed the reading.
                let w = nest_read_window(preroll, block, c.reversed || c.addr.is_some(), s0..s1);
                out.extend(sub.wanted_audio_bursts_for(w, raw));
                continue;
            }
            let b0 = s0.div_euclid(AUDIO_BURST_FRAMES as i64);
            let b1 = (s1 - 1).div_euclid(AUDIO_BURST_FRAMES as i64) + 1;
            out.push((c.media, b0..b1));
        }
        merge_burst_ranges(out)
    }
}

/// What TIMELINE SEGMENT the sub-mix producer will actually read in order to
/// deliver frames `s` (R.55, C4).
///
/// A SINGLE SOURCE OF TRUTH for render and prefetch — not “the same formula
/// in two places”, but one function: if they diverged, there would be an endless
/// cache miss (item 13, t17, t247d).
///
/// `blocks` — do we read BACKWARD? The gate is by NON-MONOTONICITY, not by warp:
/// a warp with positive speed is monotonic (`fill_source` goes along `virt`
/// forward), and needs no blocks — on the straight path the request does not move
/// by even one BURST, and “cost is linear” (t436) remains true. Backward reading is done by exactly
/// two: the mirror instance and the SIGNED speed (R.57), whose address
/// goes back along the material.
fn nest_read_window(
    preroll: u32,
    block: u32,
    blocks: bool,
    s: std::ops::Range<i64>,
) -> std::ops::Range<i64> {
    if !blocks {
        return s;
    }
    let b = AUDIO_BURST_FRAMES as i64;
    let w = (block.max(1) as i64) * b;
    let start = s.start.div_euclid(w) * w - (preroll as i64) * b;
    start.max(0)..s.end
}

/// Collapse overlapping and touching bursts ranges of one
/// media (R.43, item 297).
///
/// The user builds a freeze frame from HUNDREDS of micro-events: in his project 885
/// audio events of 90 ticks (4.8 samples) stand end to end over 0.88 s and read
/// ONE AND THE SAME source frame. Each put its own pair into `wanted`, and
/// `prefetch_audio_pass` made 885 `coverage` requests there under the cache mutex
/// and submitted duplicates — even though there is exactly one burst there.
/// The collapse changes nothing in meaning (the union of sets is the same) and
/// removes the contention warned about in `apps/kt-app/CLAUDE.md`.
fn merge_burst_ranges(
    mut v: Vec<(kt_model::MediaId, std::ops::Range<i64>)>,
) -> Vec<(kt_model::MediaId, std::ops::Range<i64>)> {
    if v.len() < 2 {
        return v;
    }
    v.sort_by_key(|(m, r)| (m.0, r.start, r.end));
    let mut out: Vec<(kt_model::MediaId, std::ops::Range<i64>)> = Vec::with_capacity(v.len());
    for (m, r) in v {
        match out.last_mut() {
            // touching ones are merged too: there is no gap between b1 and b1
            Some((pm, pr)) if *pm == m && r.start <= pr.end => pr.end = pr.end.max(r.end),
            _ => out.push((m, r)),
        }
    }
    out
}

pub fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Source window in which the event lives (frames from the file's zero).
///
/// `end` — the “silence beyond” boundary (includes the resampler margin),
/// `mirror_end` — the MIRROR AXIS of a reversed event: the actually consumed end, without
/// the margin. Before R.43 this was one value, and the mirror drifted by 170.7 ms
/// (item 292).
#[derive(Debug, Clone, Copy)]
struct SrcWindow<'a> {
    start: i64,
    /// The CONSUMPTION boundary: beyond this the chain does not consume (R.57). For a monotonic
    /// map it is also the boundary of addresses — the same number as before.
    end: i64,
    mirror_end: i64,
    /// Address map of the signed speed; `None` — address == consumption.
    addr: Option<&'a [kt_fx::AddrKnot]>,
}

/// Run of addresses covering consumption `a`: (address of the first frame,
/// direction, end of the run by consumption).
fn addr_run_at(knots: &[kt_fx::AddrKnot], a: i64) -> (i64, i64, i64) {
    let i = match knots.binary_search_by_key(&a, |k| k.a) {
        Ok(i) => i,
        Err(0) => 0,
        Err(i) => i - 1,
    };
    let k = knots[i];
    let end = knots.get(i + 1).map(|n| n.a).unwrap_or(i64::MAX);
    let off = a - k.a;
    match k.dir {
        // backward: the first frame AFTER the reversal — the one that lies before the
        // reversal point, otherwise the frame would reverse twice (palindrome)
        -1 => (k.p - 1 - off, -1, end),
        0 => (k.p, 0, end),
        _ => (k.p + off, 1, end),
    }
}

/// Fill `dst` with source frames [src_frame, +len): reads bursts through the
/// fetcher; frames outside [0, window_end) — silence WITHOUT underrun (tail/pre-roll),
/// a miss inside the window — silence with underrun. Returns true (the buffer is always
/// valid).
fn fill_source(
    fetcher: &mut dyn AudioFetcher,
    media: kt_model::MediaId,
    src_frame: i64,
    window: SrcWindow,
    reversed: bool,
    dst: &mut [f32],
    underruns: &mut u64,
) -> bool {
    match window.addr {
        None => fill_source_linear(fetcher, media, src_frame, window, reversed, dst, underruns),
        Some(knots) => {
            fill_source_signed(fetcher, media, src_frame, window, knots, reversed, dst, underruns)
        }
    }
}

/// The old path VERBATIM: address equals consumption, reverse — a static
/// mirror of the window. GOLDEN_AUDIO is built on it.
fn fill_source_linear(
    fetcher: &mut dyn AudioFetcher,
    media: kt_model::MediaId,
    src_frame: i64,
    window: SrcWindow,
    reversed: bool,
    dst: &mut [f32],
    underruns: &mut u64,
) -> bool {
    dst.fill(0.0);
    let SrcWindow { start: win_start, end: window_end, mirror_end, .. } = window;
    // END OF MATERIAL (R.63 item 2): there is no source beyond it at all, and silence
    // there is not starvation but a fact. We ask ONCE per call: for the
    // fetcher this is a map, not a read.
    let material_end = fetcher.material_end_frames(media);
    let frames = (dst.len() / AUDIO_CHANNELS) as i64;
    let mut f = 0i64;
    while f < frames {
        // Reverse (item 16): the virtual forward stream [win_start, window_end)
        // reads the REAL source mirrored inside the window; the FX chain (warp,
        // reverb) sees a regular forward stream and knows nothing about reversal.
        let virt = src_frame + f;
        if virt >= window_end {
            break; // beyond — silence only
        }
        // the mirror is computed from the CONSUMED end, not from the window end
        let src = if reversed { win_start + (mirror_end - 1 - virt) } else { virt };
        if src < 0 || virt < 0 {
            f += 1;
            continue;
        }
        let bidx = src.div_euclid(AUDIO_BURST_FRAMES as i64);
        let within = src.rem_euclid(AUDIO_BURST_FRAMES as i64);
        let take = if reversed {
            (within + 1).min(frames - f).min(window_end - virt)
        } else {
            (AUDIO_BURST_FRAMES as i64 - within).min(frames - f).min(window_end - virt)
        };
        // beyond the material's edge — silence WITHOUT underrun: a miss is a miss of
        // WHAT EXISTS (R.63 item 2)
        if material_end.is_some_and(|end| src >= end) {
            f += take;
            continue;
        }
        match fetcher.audio_burst(media, bidx) {
            Some(b) => {
                let d0 = (f as usize) * AUDIO_CHANNELS;
                if reversed {
                    for k in 0..take as usize {
                        let si = (within as usize - k) * AUDIO_CHANNELS;
                        for ch in 0..AUDIO_CHANNELS {
                            dst[d0 + k * AUDIO_CHANNELS + ch] = b.data[si + ch];
                        }
                    }
                } else {
                    let s0 = (within as usize) * AUDIO_CHANNELS;
                    let n = (take as usize) * AUDIO_CHANNELS;
                    dst[d0..d0 + n].copy_from_slice(&b.data[s0..s0 + n]);
                }
            }
            // the unit is FRAMES, as in the path without FX (R.43, item 289): “1 per
            // burst miss” gave a 1024-fold discrepancy of the counters
            None => *underruns += take.max(0) as u64,
        }
        f += take;
    }
    true
}

/// Signed speed (R.57): the virtual stream is CONSUMPTION, and the material's address
/// is taken from the address map by runs of constant direction.
///
/// The chain (warp, reverb) still sees a regular forward stream and does not know the
/// direction — as with reversal. The “Reverse” flag mirrors the finished address over the
/// map: “warp, then mirror” (R.37 item 247).
#[allow(clippy::too_many_arguments)]
fn fill_source_signed(
    fetcher: &mut dyn AudioFetcher,
    media: kt_model::MediaId,
    src_frame: i64,
    window: SrcWindow,
    knots: &[kt_fx::AddrKnot],
    reversed: bool,
    dst: &mut [f32],
    underruns: &mut u64,
) -> bool {
    dst.fill(0.0);
    let SrcWindow { start: win_start, end: window_end, mirror_end, .. } = window;
    // END OF MATERIAL (R.63 item 2): there is no source beyond it at all, and silence
    // there is not starvation but a fact. We ask ONCE per call: for the
    // fetcher this is a map, not a read.
    let material_end = fetcher.material_end_frames(media);
    let frames = (dst.len() / AUDIO_CHANNELS) as i64;
    let mut f = 0i64;
    while f < frames {
        let virt = src_frame + f;
        if virt >= window_end {
            break; // beyond — silence only
        }
        if virt < 0 {
            f += 1;
            continue;
        }
        let (addr, dir, run_end) = addr_run_at(knots, virt - win_start);
        if dir == 0 {
            // hold: this should not be reached (the chunk is muted in the
            // processor), but silently reading someone else's address is worse than silence
            f += (run_end - (virt - win_start)).clamp(1, frames - f);
            continue;
        }
        // the event's mirror — ON TOP of the map
        let (src, dir) = if reversed {
            (win_start + (mirror_end - win_start) - 1 - addr, -dir)
        } else {
            (win_start + addr, dir)
        };
        if src < 0 {
            f += 1;
            continue;
        }
        let bidx = src.div_euclid(AUDIO_BURST_FRAMES as i64);
        let within = src.rem_euclid(AUDIO_BURST_FRAMES as i64);
        let room = if dir < 0 { within + 1 } else { AUDIO_BURST_FRAMES as i64 - within };
        let take =
            room.min(frames - f).min(window_end - virt).min(run_end - (virt - win_start)).max(1);
        // beyond the material's edge — silence WITHOUT underrun: a miss is a miss of
        // WHAT EXISTS (R.63 item 2)
        if material_end.is_some_and(|end| src >= end) {
            f += take;
            continue;
        }
        match fetcher.audio_burst(media, bidx) {
            Some(b) => {
                let d0 = (f as usize) * AUDIO_CHANNELS;
                if dir < 0 {
                    for k in 0..take as usize {
                        let si = (within as usize - k) * AUDIO_CHANNELS;
                        for ch in 0..AUDIO_CHANNELS {
                            dst[d0 + k * AUDIO_CHANNELS + ch] = b.data[si + ch];
                        }
                    }
                } else {
                    let s0 = (within as usize) * AUDIO_CHANNELS;
                    let n = (take as usize) * AUDIO_CHANNELS;
                    dst[d0..d0 + n].copy_from_slice(&b.data[s0..s0 + n]);
                }
            }
            // the unit is FRAMES, as in the other paths (R.43, item 289)
            None => *underruns += take.max(0) as u64,
        }
        f += take;
    }
    true
}
