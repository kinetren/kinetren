# kt-engine

> The crate's contracts and mandatory tests — `openspec/specs/{transport,audio-render,video-plan,transitions,compositions,export}/spec.md`; the reasons for the decisions — `design.md` next to it.

Render graph, decode scheduler, transport, prefetch, export. The core builds without FFmpeg/audio hardware (real sources come from kt-media through a trait).

## Invariants
- Exactly one MediaWorker thread owns the decoder; clients go only through a priority queue (Playback > NearPrefetch > FarPrefetch > Thumbnail) with generation cancellation; at equal priority, requests go ahead of the decoder's cursor (anti ping-pong seek, defect h).
- Playback is not blocked on decoding: a miss by the deadline → silence / the last frame + an underrun flag (defect l).
- FxStateStore — only on the audio-render thread; a diff of chains by clip id on model edits (invariants No. 2–3, defects b, f).
- Transport: the playhead is advanced by the audio-render thread by exactly the frames consumed by the callback; wall-clock in render paths is forbidden (determinism, invariant No. 10).
- The composition code is ONE (rev. R.44): `VideoRenderer` builds a `FramePlan` (video_plan.rs) for the preview AND for export; there are two executors — CPU (`compose_from_cache*`, the byte reference, all tests) and GPU (`dyn VideoGpuExec` from kt-gpu, injected through `Player::set_video_gpu`/`ExportBackends` — kt-engine does NOT depend on kt-gpu). Test constructors = CPU; the product passes the GPU explicitly. DeviceLost "poisons" the GPU until the end of the session (CPU fallback + notice, t185); in export a GPU error = an export error (the file does not mix backends). With R.44 the CPU fallback is FULL: Pan3d is computed on both backends (the merged geometry pass kt-fx::geom, item 308), the `GpuRequired` refusal has been removed — export without a GPU is legal and merely slower (t301). Geometry series of a chain are merged into one pass (items 307–309); with R.45 the WHOLE chain plan (segmentation, tap ops of point-wise effects, K× barriers) is built by `kt_fx::plan::plan_chain` — `apply_chain_with_mixes` feeds the chain in WHOLE slices between the snapshot points of strength, the plan of a slice coincides with the piece of the plan of the whole (t306), and the strength stops zero the K accumulators (mixing is always at 1×). **The key of the CPU memoisation of post-FX includes the resolved camera** (`postfx_key_hash`, t66d): the camera is animated and selected by the active-camera rule PER FRAME — the static fx_hash does not see it, and without the camera in the key an edit/move of the camera gives stale output. Parameter automation (R.30): curves are resolved ONCE when the plan is built (layer_src, Cow chain) — the executors see ready values, and the curves enter fx_chain_hash (t191). Export streams into the pipes of the ffmpeg process, zero temp files (invariant No. 9, defect o).
- **Transition kernels (R.33, item 224): `transitions.rs` — the CPU reference AND the packing of parameters for the GPU.** The kind from the model is turned into a `TransMix` (mode + 8 numbers) EXACTLY once; the CPU kernel and the WGSL mirror `kt-gpu/shaders/composite.wgsl` read the same numbers. Editing the formula in one file without the other = the preview and the export show different things (invariant 7); both files are edited in one commit, the test — kt-gpu t224. The crossfade does NOT go into the kernels: it remains the integer `dissolve_into` (GOLDEN_VIDEO). A solo fade at the edge is computed by the same kernel with an EMPTY second side (`Img::Clear` / the clear bit of the uniform) — a wipe "from nothing" works without a second frame.
- SIMD composite (R.32): `video_simd.rs` — AVX2 twins of alpha_over/dissolve/scale_alpha, byte-for-byte with the scalar (rules — kt-fx/CLAUDE.md, the SIMD section). The `sa == 0` branch of alpha_over is reproduced by a select: without it the formula would spoil dst. Test — `tests/simd_parity.rs` (t198).
- **The end of a transition is found by `kt_model::same_clip` + a check of the boundary** (R.35): the morpher layer cuts the target, the original's id stays on piece No. 0, and the joint goes to that piece on whose boundary it stands. The comparison `c.id == id` returned the wrong piece, and the transition SILENTLY disappeared (t237). This is the only edit of the engine for the sake of cutting: `t.joint(project)` is computed over the RAW clips and remains correct. `gpu_required_clips` collapses pieces to the original (`slice_origin`).
- **Stretching of VIDEO time (R.37 item 2)**: `Slowed+Reverb` on a video clip changes not the pixels but the CHOICE of the source frame — `clip_material_pos` goes by the integral of the speed (`kt_fx::warp_material_ticks`, a TICK input to the map: a 48 kHz grid misses the frame boundary). The point of editing is one, and from it `layers_at`, `wanted_frames`, the player's prefetch, the preview warm-up and export follow by themselves — this is the video analogue of the kt-fx invariant "a single source of truth for prefetch and render": if they diverge — an eternal cache miss and a black frame (t247d). With R.46 (item 318) the request also carries the DECODE QUALITY: `layers_at(t, quality)`/`wanted_frames(range, quality)` return `(media, pts, Quality)`, where Quality is a property of the CLIP (`clip_decode_quality` = Quality × step). The step (item 320) = min(`ss_taps(head_zoom)` of the clip, the native's margin over Quality rounded UP, `MAX_INTER_DIM / max(Quality)`); if the native is unknown — step 1. Natives come from the OPENED sources: `VideoRenderer::new` does not know them (step ≡ 1), the player and export call `new_with_natives`. A change of the composition of sources (`add_media`/`replace_media`) rebuilds the player's renderer — otherwise a file that came back would not change the step until a model edit (t316). The budgets of the export chunk and of the look-ahead window are counted by `max_decode_quality`: at Quality they are underestimated by S² times (item 15).
  **With R.58 (item 459) "step" is TWO different numbers, and they must not be confused.** `clip_decode_scale` of a reel is the step of the SUB-PLAN: it is ONE for the reel (the maximum over instances, `nest_scales`), because the request and the keys of the inner leaves MUST be one (t450, t312/t317). `clip_chain_scale` is the INPUT OF THE CHAIN of this instance = `min(the reel's step, ss_taps(head_zoom of the instance))`: an empty chain, a single layer strength, a leading stop (the morpher layer) and barriers without a step resample do NOT consume it, and nobody would return the S×-frame to Quality for them. If they diverge — the sub-composite is FITTED before the chain: one decision for both backends (`NestSrc::fit_to`), the CPU calls `kt_fx::video::fit_into`, the GPU puts `Resample` over the whole frame. If equal — there is no step, the bytes are as before (t631b). The step of the reel enters `postfx_key_hash` and the GPU `plan_key` (t631c).
  **A layer of a foreign size is not skipped silently**: `complete = false`, and the reel additionally has a `debug_assert` — its size is our arithmetic, not a message of the decoder (t633, t182). The `head_zoom` of an attachment is computed by the cameras of THAT reel where the instance stands — as in `nest_scales`.
  - A clip without a warp — `warp: None`, the path is the former BYTE-FOR-BYTE; GOLDEN_VIDEO stands on this.
  - **Reverse + stretch — "WARP, then MIRROR"**, the anchor is the actually consumed `used`. A mirror BEFORE the warp would drive the speed curve backwards and desynchronise A/V (the audio mirrors the already consumed forward stream). The phase of repeats may then differ from the mirror of the output axis by half a frame; at rate = 1 there is no discrepancy (t24 and GOLDEN are intact).
  - **A procedural source under a warp follows the CONTINUOUS tick** (R.56, item 448): the generator and the title have no frame grid of their own, and the floor gave the slowed title steps of two frames — at the root just as in a reel.
  - `u` (the progress of the event) and `burst_index` (the noise seed) remain by the OUTPUT time: otherwise the Ken Burns motion would be squeezed into the first third of the slowed clip, and the grain would freeze on repeated frames.
  - `fx_chain_hash` already hashes `fx` and `auto`, therefore a change of speed by itself invalidates the post-FX memo.
- **Frame blending (R.37 C5, "Frames: blending")** is done in the COMMON code of the plan (`layer_src` → `blend_frames`), not in the executors: both backends receive an already blended pre-FX frame, and the same arithmetic does not have to be kept in sync in the CPU kernel and in WGSL (invariant No. 7 — "the composition code is one, the backend is a parameter of execution"). The kernel is the ready `dissolve_into` (byte-frozen, with an AVX2 twin); nothing new is introduced.
  - A PAIR is decided by ONE function `clip_frame_keys`; `layers_at` MUST return BOTH pts, otherwise prefetch warms one frame while the plan asks for two — an eternal miss. With R.56 it also has a second entry — `blend_footage`, inherited from the attachment above (item 448): the reel itself has no pairs, its leaves blend.
  - Weight `k == 0` (a position on the grid, integer speeds, rate = 1) gives no pair: the path is the former byte-for-byte, and GOLDEN_VIDEO stands on this.
  - `LayerBlend` remains in the layer as METADATA (the neighbour + the weight) — they are needed by the `ReadbackCache` key in kt-gpu: the same two frames with a different weight give a different picture.
  - `max_concurrent_layers` counts a blending layer as TWO frames: otherwise the export chunk evicts the neighbour before the composition, the starvation branch fires, and the EXPORT BYTES depend on the timing of the cache.
  - Starvation of either side — the one available at full weight, as for a `Dissolve` pair.
- **The axis of the mirror of a reversed event = the actually consumed `used` (R.43, item 292).** `chain_source_window_ext(..).end` is the BOUNDARY "beyond — silence", and it includes `WARP_SOURCE_MARGIN_FRAMES` = 8192 frames of resampler margin; it cannot be the axis: mirroring from it, a slowed mirrored event reads material 170.7 ms beyond its own end, while the video anchors to `ClipWarp::used` without a margin, and A/V diverge. Both quantities live separately in `SrcWindow`; the axis is computed by `kt_fx::chain_source_used`, and the SAME axis MUST stand in the mirror branch of `wanted_audio_bursts` — otherwise an eternal cache miss (rule item 13). Test 282.
- **`FxStateStore` carries the GEOMETRY, not only the parameters (R.43, item 293).** `chain_fingerprint` knows only the KINDS of slots, therefore a trim/cut/punch-out/re-anchoring of a morph piece changes `source_in` with the same id and the same chain structure — and the processor went on reading from the old anchor ("sound from another event"). `set_src_in` moves the anchor with a SEAM, not a seek: the output stays continuous, the reverb rings out (invariant No. 3). Test 283.
- **Underruns are counted in FRAMES on both paths (R.43, item 289).** Before this the path without FX counted frames, and the path with FX counted burst misses: the same starvation gave numbers differing by a factor of 1024. The new counter is in frames, they are comparable with the length of an event and with the output latency. Test 280.
- **The raw (reverse-transport) path has its OWN `wanted_audio_bursts_raw`
  (R.43, item 295).** `render_frames_raw` plays all events through `render_plain_clip` — linearly and without a warp; the prefetch, which counted the window by the warp, warmed the wrong bursts, and the miss was eternal. Test 285.
- **The output of audio FX is not cached and cannot be** (R.43, item 302): the chain is a streaming automaton, the reverb tail depends on the whole history, and the same piece on the second lap of a loop gives a DIFFERENT output. The cache holds the decoded SOURCE; a repeat does not become cheaper because of this. In video the post-FX frame is memoised (`PostFxVideo`) precisely because it is a function `(frame, chain, t)` without history. The practical consequence: the cost of a place on a loop is constant, and it must be measured with `kt-cli probe … --time` (it goes through `BurstCache`, as the program does — the diagnostics must repeat the engine's wiring).
- **The cost of a place depends on the LENGTH OF THE REGION** (R.43, item 303): the wrap-around is a seam, and a seam resets the ready resampler buffer (`Warp::seek_source`), and it is primed again, ≈0.3 ms per chain. A short region wraps more often and pays more often: 300 ms over the reverb tails costs 10.2 ms/s against 4.7 for a region of 1700 ms (7 chains, release). The worst is a region with NO BODIES of events — only tails. This must be measured in REPEAT mode (`kt-cli probe … --time` prints both the linear run and the loop): a linear measurement does not see the dependence on length at all.
- **Nothing in the hot path may be O(the whole edit)** (R.43, item 305).
  `overlap_weight` went through ALL the clips FOR EACH FRAME, and the cost of a PLACE grew linearly with the size of the PROJECT: 7 events — 3.6 ms/s, the same 7 plus 2000 distant ones — 92 ms/s. The set of covering events changes only at boundaries, therefore it is computed once by a sweep (`LaneCover`), and per frame a binary search remains; the maps lie in an array BY TRACK NUMBER (a hash per frame cost 40 % on small projects). The order of indices in the set is ascending: `overlap_weight` takes "the first two" in exactly that order, and GOLDEN_AUDIO stands on this. Tests 292/293 guard it.
  The search itself gave 11.5× on the user's project (889 → 77 ms); on top stands the flag `overlaps` — for an event that overlaps nobody, the weight is identically 1.0, and the path is closed by one `bool`. It is free and helps where many events are active at once; on a real edit it is about 1 %. **Do not compare the share of total time between benches** — it has a different denominator (item 305).
- **In the tail of FX the warp is not computed** (R.43, item 304): beyond `gate_end` the dry signal is zeroed by the gate anyway, so a chunk lying entirely beyond it is skipped — byte-identical by construction, but without the resampler and without reading the source. Tests 294/295.
- Seek: reset FX + pre-roll max(tail, 100 ms) into garbage (test 7).
- Prefetch is idempotent: wanted − coverage() → submit; keys strictly on the playback grid (defect i); the head of a loop region — a pin in the cache (test 14).

## Law P: the media path is a property of the SOURCE, not of the plan (R.53, item 412)

The renderers know about media exactly `MediaId`; WHAT lies at that address —
a file, synthetics, or a generator with parameters in the path — is irrelevant to them.
Hence the law: **`Command::SetMediaPath` has no right to clone
the project, rebuild the renderers, or touch other people's sources.** Its
only addressee is the worker of THIS media.

- `Command::touches_render_plan()` — an exhaustive match in kt-model:
  a new command will not compile until it is stated whether it moves the plan
  or not. `SetMediaPath` is the only one that answers "no".
- `Player::retune_media` puts the new path into `Queue.retune` — a SEPARATE
  field, not a `RequestKind`: a seek in the middle of a parameter drag would silently
  cancel the request by generation cancellation. The worker handles it FIRST, before
  `pick_next`, and there it also clears the source's cache — the race "the old frame
  is appended after the clearing" is removed by construction.
- `content_version` is separated from `project_version`: prefetch wakes up,
  the renderers are not rebuilt. The rebuild condition in `prefetch_loop`
  looks ONLY at `project_version`.
- A source that does not know `retune` (a file — always) is honestly recreated
  through `replace_media`; the predicate `kt_media::factory::
  can_retune` decides this, and it MUST match `MediaSource::retune` exactly (guard t499).

A precedent of the same kind — `on_region_changed` (R.43 item 296, guard t286):
`set_project` is a clone of the whole project plus a stop of the audio-render thread,
and one may not pay for an edit that the plan does not touch with them.

### A generator on the EVENT (R.59, stage 2)

The values of the generator moved from the media to the event, and law P has only
strengthened because of this: the source's address is computed from the EVENT (`clip_source_addr` =
`GEN_TAG | id` for a generator, `c.media` for all others), and the VALUES live
in the source's state and arrive through the retune lane.

- **The address — from the event, not from the values.** It enters the plan, and a hash of
  the values in it would mean rebuilding the plan on every keystroke. The
  form of the request `(address, tick, quality)` has not changed, so "the request ==
  the read", the coverage bars, prefetch, export chunks and the coverage of reels
  work without edits.
- **The list of sources is computed by ONE code — `gen_sources(project)`** (the root
  plus every reel through `effective_reel`). By it they are raised by
  `Player::sync_generators`, `render_raw_*` and kt-cli. There must be no second enumeration:
  if they diverged, export would render something other than what the preview showed.
- **Foreign memos are cleared by THE SAME thread that applies the reconfiguration.**
  `Retune::Gen` carries a list of the addresses of the ancestor reels
  (`reel_chain_of_clip` → `nest_source_id`), and the worker invalidates them
  right where it clears its own cache. If the caller invalidated them — between "cleared" and
  "applied" a composite on the OLD frame would manage to pass and write the stale value again. Guard — t663.
- **Only the FULL sub-composite is memoised** (`nest_frame_cpu`). A starved
  frame is a function not of the reel's revision but of what the worker managed to put in; written under the revision's key, it gave a BLACK reel forever: the key does not move from the warm-up, and outside the composite is declared complete. The live player
  fell into this on every cold start.

## Tests
1–3 (concatenation/crossfade/gain), 7 (seek determinism), 9 (scheduler), 11 (golden export), 13 (viewport black), 14 (loop).
- Tests 166–168: `tests/loop_fx.rs` (the seam of the wrap-around, a short region, a complex edit without underruns).
- Reverse transport (J4 v1.4): rate<0 — the render thread renders blocks BACKWARDS (render_frames_raw/render_looped_raw, WITHOUT FX — dry, as in Vegas) and unrolls frame by frame before the ring; the callback does not know the direction (|rate|), the sign is applied in advance_frames_wrapped (wrap in both directions). A change of sign = a mini-seek without bumping the generation (reset FX + flush of the ring). Tests 41–45.
- **Overlaps (R.28, item 162)**: with `project.overlap_layers` the overlapping of clips is legitimate and sounds by itself — `overlap_weight` (v1: equal-power for two, 1/N for 3+) is multiplied by the transition's weight in `overlap_at`. On clips butted together it is identically 1.0, therefore GOLDEN does not move. Video (item 177): on top is the one that STARTED LATER — the Vegas rule; the composition key `(Reverse(lane), start)`. The timeline lane in kt-app draws in the same order, the hit test in the reverse.
- Transitions (R.24, items 135–138): crossfade/dissolve — an explicit TransitionEvent. Audio: the fade windows are precomputed in AudioRenderer::new (the boundaries are floored to frames by the same conversion as the clip boundaries — the conversion of old overlaps is byte-exact, GOLDEN was not regenerated); the "handles" read the source beyond the edges of the piece, beyond the edges of the material — silence / a clamped frame WITHOUT underrun; for reversed-FX clips the handles are disabled (the mirror anchor window_end). Video: the window pair is blended linearly (k/4096) into one layer before alpha_over, and the missing half — full weight + complete=false. **The fade (R.25)** — a transition with one live end: the window is marked solo, the layer is multiplied by the alpha k/4096 and goes through the ordinary alpha_over (the lower track shows through beneath it, if there is none — black), `complete` is NOT reset: there is no pair by the MODEL, there is nothing to wait for. The audio weights of the fade are the same equal-power. The render length — `project_range` = max(material, `project.timeline_end`): a trailing Gap of a foreign file is rendered black/silence (item 158). FX tail beyond the material is not weighted.
- Loop "armedness" (items 132–134): the wrap of audio/prefetch is consistent with the TRANSPORT (wrap only from inside the region; the edge `p == e` is inclusive, for reverse — `p == s`). `loop_armed` is recomputed ONLY on jumps (a change of generation / sign of rate / region values) from the wrapped playhead through `loop_arm_after_jump`; the current `render_head` is UNSUITABLE for this — with an armed loop it grows linearly and is folded on every burst. Both fill branches, the Playback prefetch and `advance_frames_wrapped` go through the single `effective_region` (None on disarm — the linear play of what is under the cursor). A flip Some↔None on the fly (Loop switched on during play, the boundary dragged past the playhead) = a mini-seek without a generation bump, like a change of sign. `video_display_frame` is a two-sided condition on the wrapped playhead, it needs no flag (inside the region ⟹ armed by construction). The reverse fill is gated symmetrically (`render_head <= re`): the guard `> rs` switched the loop off when render_head went below the start of the region. Tests 132–134.
- The video DISPLAY clock (item 56): the playhead moves in quanta of the audio callback (PipeWire up to 2048 samples = 42.7 ms) — the consumers of the position for display go through video_playhead() (display_extrapolation: the wall-clock interpolates BETWEEN steps of the master clock, clamped to the size of the last quantum; the raw output_latency jitters — latency_smooth, an EMA, is read). Audio remains the master (No. 6), offline paths do not see the wall-clock (No. 11). Test 56.
- **The wrap of the loop is a SEAM (R.28, items 166–167)**: a break of the clip-local position during CONTINUOUS output (the wrap of the region, a mouse shift of the clip during play) does not reset FX — `ClipProcessor::seam` moves the source, the reverb rings out through the circle. `hard_seek` with pre-roll stays exactly for a real seek: its sign is `reset()` (done by the player itself on generation / a sign change of rate / a flip of loop). Pre-roll misses — `RenderStats.warmup_misses`, NOT `underruns` (they are not heard). The background prefetch folds the look-ahead window into the loop region (`lookahead_windows`) — beyond the end of the region there is nothing to warm, and the evicted head costs misses on every lap.
- **The level of the event (R.28, item 174)**: `VideoClip.opacity_k` (0..4096) is multiplied by the layer's alpha right where the fades live — an ordinary layer through `scale_alpha_into`, a solo fade is multiplied, in a dissolve pair the levels are blended by the same weight as the picture. 1.0 is an identity, GOLDEN is intact.
- **"Black box" (R.28, item 175)**: `flight::FlightRecorder` — a ring of 512 records, written ONLY by the render thread (it does not enter the callback, invariant No. 6). Deduplication of consecutive ones, a snapshot of deltas once a second and only if they are non-zero, a separate event `STALL` (playing, but the playhead stands still). The dump — `Player::flight.dump()`.
- The prefetch thread skips passes when idle (a pause + the state has not changed); the audio render sleeps 8 ms during play (the ring's margin is fourfold) — 2 ms gave 500 wake-ups per second (item 53).
- **Transitions of R.51 (items 396–401)**: the inversion (`TransitionEvent.invert`) is executed by the PLAN — `apply_invert` in video.rs (swap of the sides + k→4096−k before the kernel); the kernels/WGSL/cache keys are untouched, and any assembly of a `Dissolve` pair MUST go through the helper. An overlaid transition: the window = the overlap zone (`TransitionEvent::window`), attached in audio `build_reel` and in video by `reel.transitions()` (NOT by the root — a transition inside a nested reel would otherwise silently disappear, the defect of R.50 closed in item 399); the overlap window mutes the built-in equal-power through `fade_*_excl` + `overlap_at` → 1.0. Volumetric kinds: `trans_ext` (the f64 homographies → f32) — the single source of ext for BOTH backends; it is not in the cache keys (a function of the already hashed mode/p/k). The directional kernels are guarded by the closed-form t459 on ramps: t224 checks the mirrors and is blind to "both broke in the same way".

## The sub-mix of a nested reel — a producer with pre-roll (R.50 item 387, R.55 item 440)

An embedding in audio is **a virtual medium under the fetcher**, not a branch in the
mixer: therefore `render_plain_clip`, `render_fx_clip`, the weights,
`chain_source_window_ext`, `fill_source` and the ORDER OF SUMMATION of f32 know nothing
about reels, not a line, and GOLDEN_AUDIO does not move.

**Law C4: the sub-mix is rendered ONLY FORWARD and only in ascending
reel time.** The reverse and the stretch of the instance act on the SAMPLES of the
finished mix — just as on the samples of a file. The internal reverb must
sound forward INSIDE the reel, and a mirror from the outside gives a reversed reverb; playing the reel backwards means computing a third result that belongs to no one.

**The producer (`NestFetcher`) — three branches and not one more:**

1. a burst in the ring — hand it over, do not touch the state;
2. ahead of the cursor by no more than a block — CATCH UP forward;
3. otherwise — **the canonical move**: reset, start from the BLOCK BOUNDARY minus the pre-roll, forward to the required point.

The canonicity of the grid makes the value of a burst a pure function of the index.
The two-valuedness "reached continuously / moved" remains exactly one — the same
pair `seam`/`hard_seek` as for an ordinary clip. **The fourth branch is a new
sort of non-determinism**, which does not occur in audio (invariant 11).

**Pre-roll and block are functions of the PROJECT, not of the memory budget.** This is the only
way to break invariant 11 here: the size of the ring would depend on the budget —
the bytes of the output would depend on it too. Pre-roll = the tail of the most tail-heavy
chain of the reel (recursively), block = 4× the pre-roll. A reel without tail-heavy
effects has a ZERO pre-roll, and reverse costs the same as direct playback.

**The state key is the INSTANCE (`ClipId`), not the reel.** The reel in five places is
five different streams through its chains. The reader is delivered by `AudioFetcher::
begin_clip` (an empty default, one call per clip per burst).

**The sub-mix is NOT put into the `BurstCache`**: its production is a run of the reel's
FX state, and only the audio-render thread touches the state (invariant 2).

**The request and the read — ONE function `nest_read_window`.** The producer reads the reel
wider than the consumption window (the move begins at the block boundary), and if you do not widen
the request — an eternal miss on every move (item 13, t17). The gate is by REVERSE, not by the warp: the warp is monotonic, and on the direct path the request does not move by a single burst.

**The address of the reel in the plan — `nest_source_id(seq)` (`NEST_TAG | id`).** Without the tag
it numerically coincides with the address of a FILE with the same id, and `validate` forbids
this precisely so that they do not share the cache.

## The internal time of a reel is CONTINUOUS (R.56, item 448)

**The grid floor is set by the LEAF, and only a FILE leaf.** A file has no frame "between
frames" (the decoder gives the nearest one, and two keys for one
frame are a double cost of the cache), therefore `clip_source_pts` for it did not
change. For a PROCEDURAL source (a generator, a title, a placeholder — the sign `MediaInfo.fps == None` with `has_video`) a frame is a pure function of the tick:
the floor there only splits the motion into steps. A REEL has no floor at all —
`clip_inner_tick == clip_material_pos`, its inner time is the time of the editing. Before R.56, at rate 0.5 two adjacent output frames received one
`inner_t`, the memo returned the same record, and titles/automation/cameras inside
a composition went in steps of two frames.

**The facts of the sources arrive through the same channel as the natives** — `SourceFacts`
(`natives` + `procedural`), `VideoRenderer::new_with_facts`. The plan remains indifferent
to the media path (law P): "what lies at the address" is a property of the
SOURCE, and the renderer learns it from whoever opened the source. If the
player, export and kt-cli diverged on this question, the request and the read would compute different
keys (invariant 4).

**The blending of frames on an attachment is done by the LEAVES.** The mode is inherited downwards
(`PlanCtx.blend_footage`, `nest_blend(clip, inherited)`), the sub-composite per frame
remains ONE, and the GPU receives an already blended pre-FX frame by the ordinary
`LayerInput::Frame`. Two sub-composites would be twice as expensive on both
backends AND would return steps to the procedural one (both would be assembled on the grid). The flag MUST travel in FOUR places, otherwise they will diverge:
`clip_frame_keys` (request == read), `max_concurrent_layers_in` (the chunk
budget), `reel_coverage` (a run of the file child shorter by a frame) and
`nest_memo_key(content_hash, blend)` — at `blend == false` the key EQUALS
`content_hash`, therefore the former records do not move. The mirror on the GPU —
`n.blend` in `plan_key`.

**A test trap:** a fresh `BurstCache` starts the numbering of content
anew, while `UploadCache` in kt-gpu is keyed ONLY by a serial number — two
caches for one engine give it identical keys for different frames. The cache in the
test MUST be one for all moments, as in the program.

## A reel's revision = a PAIR (reel, instance overrides) (R.59, stage 3)

The engine knows nothing about publication at all: `Project::resolve_reel` hands it
ready clips. Exactly one thing changes — "the revision" has stopped being a function of
a single reel.

- `instance_content(project, seq, overrides)` EQUALS `content_hash` when the set
  is empty. Otherwise all the keys of the memo, the sidecar previews and
  the GPU plans would move for projects that do not use publication.
- `VideoRenderer::nests` and `AudioRenderer::nests` are keyed by `NestKey =
  (MediaId, u64)`; `VideoClip.nest`/`AudioClip.nest` carry the pair. Two
  instances WITHOUT overrides share ONE sub-renderer — that was the gain of t442/t443, guard `nest_count()` (t676).
- `NestStream.seq` is also a pair: when the overrides change, the sub-mix stream
  starts anew, as when re-hung onto another reel.
- `nest_scales` was rewritten from a flat enumeration of reels into a WALK OF THE TREE
  of instances with their sets. The step is still ONE per reel (t450),
  the maximum became honest (t679).
- No new cache addresses are introduced for the REEL: the instance's `content` itself
  gets into `fx_chain_hash`, `nest_memo_key`, the GPU `plan_key` and the sidecar.

**THE ADDRESS OF A GENERATOR'S SOURCE IS SALTED, and this is not "one more key".** Since R.53
the address is `GEN_TAG | id of the EVENT`, and it unambiguously determined the VALUES:
one event — one set of values. An override removes the premise: the same
event INSIDE a reel plays in two releases with a different title. Leave the
address shared — the request and the read would meet on it (invariant 4 formally
holds), and the frame would come back a FOREIGN one. Therefore:

- `gen_salt(published, overrides)` — the salt; it is EXACTLY ZERO when and only when
  no handle of the set is able to reach the source
  (`kt_model::overrides_touch_sources`). The root, a reel without publications and
  an instance without overrides get the former addresses byte for byte.
- `gen_source_addr_in(id, salt)` — with a zero salt it is the former
  `GEN_TAG | id`, otherwise a 62-bit hash of the pair. There are no free bits in the address
  (clip < piece < generator < reel), and the hash is legitimate here for the same
  reason as `content_hash`: a pure function, one code for all.
- **The salt is computed by TWO parties, and they MUST agree**: `build_reel` (the plan) and
  `gen_sources` (who raises the sources). If they diverged — an eternal miss by
  address. `gen_sources` therefore walks the TREE of instances and in addition
  enumerates every reel with template content (a reel placed nowhere is also shown in the bin and counted by the preview service).

**A side effect that is seen once.** The `Debug` of `ClipEvent` changed (a new field `overrides`), and `content_hash` is computed
from the Debug representation: all the hashes of the revisions moved ONCE, and the
sidecar previews of reels in old projects will be recomputed once on
opening. This is not a regression — the key is a function of the model, not of the file.
