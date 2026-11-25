# Design: track-stream-stage

> IMPLEMENTATION-READY (spec-engine, revision 2, 2026-11-24), read against orch `229c694b` (fx-graph v2d and
> durable-moment-key merged). Code is named by function and file; line numbers move. `shared-events` (running) changes the
> model's event vectors, not what this reads of them.

## Context (what exists)

- **The kernel never sees an event.** `kt_plugin_api::stream`: a `StreamKernel` asks a `StreamLayout` (positions, addresses, rows) and
  a `StreamInput` (frames, `on_device`). The host is the only part that knows what a position reads.
- **The event host** is `ClipStream` (`video_stream.rs`). Positions are project grid frames; `ident` mixes the media, pts, stream
  size, the source's IDENTITY (`known`, durable-moment-key D3), the front's key and the position when the front is curved. `read`
  refuses a frame of another identity than the one its position is named by.
- **The walkers.** Seven per-frame questions go over `streams_of(clip)` (a layer's own stage and its reads', fx-graph-dag D2):
  `stream_advance`, `stream_advance_run`, `stream_checkpoints`, `stream_stages_at` (`stream_want`), `stream_frame_bytes`, `leaf_keys`
  (the order), `has_stream_stages`. Each builds a `ClipStream` from a `(&VideoClip, &StreamStage)`.
- **The track stage**: `VideoTrackStage` (built in `build_reel`) and `track_stage_layer`. The stage's sub-plan is `build_plan` over its
  events with `stages: &[]`; the layer is `LayerInput::Nest` + the track's chain. A track has NO curves (the window refuses them), no
  time axis of its own (`u: 0`), no camera (`cam3d: None`).
- **A track chain's wiring never reaches the plan** (found-defects 1067, this reading): `track_stage_layer` writes `fx_in: &[]` and
  the stage's `content` hashes `active_fx()` alone. v2d lets a person branch a video track's chain (`SetTrackFxIn`); the sound
  track's stage runs the graph, the picture's runs the line.
- **The one key** is `moment_key` (`put_plan`/`put_src`): leaves named by identity, whether or not decoded.
- **The front executor** (`front_exec`, kt-gpu `stream_front_engine`) is a whole engine on the compute device; `execute_held` runs any
  `FramePlan` and leaves straight RGBA there. Upload serials are process-wide, so a multi-layer plan of any cache is safe on it.
- **The sound half** (`fx-video-codec/src/sound.rs`, `MoshSound`): its own `Layout` from `Placement{start, fps}` and the event's length;
  a track's device is seated at frame 0 (track-device-sound), and the whole track would be ONE segment.
- **The refusal**: `track_window.rs` after the graph (t3237); `track_pairs` in kt-model.

## D1. The track's line runs its wiring first (row 1067)

`VideoTrackStage` gains `fx_in = t.active_fx_in()`; `content` hashes it when it is not empty (a line's keys stay); `track_stage_layer`
hands `fx_in: &st.fx_in`. Arms then render by the lowering every event uses. Time kinds (Slowed, Stretch) in a track's arm stay
transparent, as on a reel instance: a track has no second moment to read. This is task 1.0: it stands alone and is the smallest
fix of row 1067 (an executor may take it out earlier as its own commit).

The stream slots of the chain are then found as an event's are (`build_reel`'s rule): the TRUNK's first stream slot is the stage's own
stage; a stream slot in an ARM is a read of the track (`fx_graph::reads`, keeping only reads whose time kind is a stream). Each is a
`TrackLine`.

## D2. `TrackLine`: the second host's static part

```rust
/// A stream stage of a track's chain: the trunk's (`slot: None`) or an arm's read ending at `slot`.
struct TrackLine {
    slot: Option<usize>,
    fx: Vec<FxInstance>,          // the line from the Source to the stream slot
    st: StreamStage,              // StreamStage::of(&fx, &[]); `before`/`second` stay empty
    segs: Arc<[TrackSeg]>,        // D3, shared with the sound
    second: Option<TrackSide>,    // D7
    sources: Vec<SourceId>,       // of the whole lane, reads and reels included: what names are asked of
    composed: ComposedCells,      // D5: the compiled names per stream size
}
```

`VideoTrackStage.lines: Vec<TrackLine>`, built after the events are sorted (beside `st.events`). `front_curved` is always false
(no curves). `front_key` hashes the front slots as today. The second input and segments need the lane's neighbours, so they are filled in
`link_track_lines`, beside `stream::link_stages`.

## D3. The layout: segments at the track's cuts

`kt-engine/src/track_layout.rs`, pure, called by BOTH builders (the picture's and the sound's):

```rust
pub struct TrackSeg { pub prev_cut: Option<i64>, pub cut: i64, pub end: i64 }
impl TrackSeg { pub fn first(&self, before: u32) -> i64 }   // cut − min(before, cut − prev_cut); no prev_cut → cut
pub fn track_segments(spans: &[TrackSpan], step: i64) -> Arc<[TrackSeg]>
pub fn spans_of(resolved: &Resolved, reel: &dyn Reel, lane: u32) -> Vec<TrackSpan>  // (from, to, origin, continues)
```

- A span is an event WITH its transition handles: `[start − head_ext, end + tail_ext)`. `head_ext`/`tail_ext` are computed
  in `build_reel` today; the arithmetic moves into `spans_of`'s helper `handles(t, reel)`, and `build_reel` calls it (one rule, two callers).
- **A cut** is the grid position (`ceil_div`) of a span's `from` (Q1 a: the first frame of the incoming event, so a dissolve's is the
  window's start). Not a cut: a piece a morpher layer cut off the SAME event and that continues it (`slice_origin` equal, `from` == the
  former piece's end, the material contiguous): the person cut nothing there.
- Two spans starting at one position are one cut. A span inside another (an overlay on the same lane, `overlap_layers`) starts a cut:
  new content appears.
- **A segment** runs from its cut to the next cut or to the end of the contiguous run. A position no span covers ends the run (Q2 a).
  The next segment has `prev_cut: None`.
- Positions belong to segments by halving (`partition_point` over the cuts).
- Muted events and muted tracks keep their spans (a mute is not an edit of the timeline); a silent track has no picture stage to run.

A dense edit is many short segments, and each walks only from its own `first`. The event's own stage stays as it is: its `before` is
the event before.

## D4. One builder, two ways to look at a source: `Look`

The plan of a moment is needed in three forms: to run (frames), to key (names, no decode), to order (addresses). The first two must
come from the SAME `build_plan`, or a name drifts from its picture.

```rust
enum Look<'a> {
    Cache(&'a BurstCache),
    /// Names only: no frame is read, no recency moves. The snapshot of identities taken once per question.
    Names(&'a HashMap<SourceId, SourceIdent>),
    /// The cache's frames, but only those of the snapshot's identity (ClipStream::read's rule).
    Known(&'a BurstCache, &'a HashMap<SourceId, SourceIdent>),
}
impl Look<'_> {
    fn video(&self, s: SourceId, q: Quality, idx: i64) -> (Option<(Arc<VideoFrame>, u64)>, Option<SourceIdent>);
    fn idents(&self, of: &[SourceId]) -> Vec<Option<SourceIdent>>;
}
```

`layer_src`, `track_stage_layer` and `build_plan` take `&Look` where they take `&BurstCache` (three call sites of
`get_video_leaf`, and `key_only`/`durable` which ask `idents`). `stream::run` is reached only with a cache; with `Names` a stage is keyed by
`key_only`. In `Names` the content number is the clip's `src_rev`, so a generator's value names its frame where no identity exists (a
private cache). `frame_plan` and `probe_plan` pass `Look::Cache`: their bytes and keys do not move (t3398 and GOLDEN_VIDEO).

## D5. `TrackStream`: the composite at position `k`

`kt-engine/src/track_stream.rs`, `impl StreamLayout + StreamInput for TrackStream<'_>` for the segment holding `pos(t)`.

- **The plan at `k`**: `t_k = k·step`; `sub = build_plan(events, &st.index, &sub_ctx, look, t_k, q)` with the stage's own sub-context
  (`stages: &[]`), `q` the stream's size. No front: `sub` is the plan. A front: one layer, `LayerInput::Nest(sub)`, whose `fx` are the
  front slots (`u: 0`, `cam3d: None`, no masks, as the track's chain runs them) at `q`.
- **Its size**: `track_quality(st, quality) = input_quality(quality, lines, carried, !front.is_empty())`: the layer's base size is the canvas.
- **`ident(Main, k)`** = `mix(mix(TRACK_STREAM, moment_key(plan built by Look::Names)), front_key)`. It already holds `t_k`, the size, every
  layer's leaf, chain, level, transition, mask, blend mode, reel and inner stream key. Memoized per position in
  `composed` (`Mutex<HashMap<(Quality, names hash), Vec<u64>>>`, at most two name sets per size, as `plan_cell` keeps): a schedule's compile
  asks each position once, a frame asks none again. It dies with the build, as `PlanCell` does.
- **Lock order**: `Plan::ensure` holds its own lock while it asks `ident`; `ident` may reach an INNER event's kernel (`key_only`), which locks
  its own plan. A track's line is never inside its own sub-plan (`stages: &[]`), so no lock is asked twice.
- **`on_device(Main, k)`**: the front memo by `ident`; else the plan by `Look::Known`; `starved()` → `Missing`; `execute_held`
  → kept in the front memo; `Unsupported` → `Host`. The plan is BUILT before the executor's lock is taken: building runs an inner stage,
  whose own front takes the same lock.
- **`with_frame(Main, k)`**: the same by `execute_once`.
- **No device**: `Missing`, as an event's front.
- **An inner stage short of its history.** `TrackStream` carries the pacing its inner stages run under: the compose thread passes the
  preview's (an inner history short by more than `compose_steps` is a missing frame, noted as `stream_short`: Loading), the advance
  (prefetch thread, export, jobs) passes none, so the walk of the outer stage walks the inner one too. Paced in both, the inner history at a
  past position would be nobody's to walk and the frame would never come.
- **Edits.** An edit of one event moves `moment_key` only where it is active. Positions before keep their entries; the ones after it
  change through the codec's own chain of keys.

## D6. The layer: the stage's picture with the moment's own coverage

`track_stage_layer` with a trunk line: `LayerInput::Held` / `Frame` of the kernel's output at the plan's size, `fx` = the chain with the front
replaced by the identity (`after_chain`), `leaf = Leaf { key, durable, drawn: false, stream: true }`, `content = key`,
`fx_hash = mix(st.content, key)`. An arm's line is `reads.push((slot, LayerSrc))`, as an event's read.

**Coverage.** A codec has no alpha: the stage's picture is opaque. An event's stage stands before its Geometry, so what the event does
not cover stays transparent. A track's stage stands after everything, so an opaque picture would hide the tracks below under a title, a
picture-in-picture or a fade-out. So the picture takes the alpha of the track's composite AT THE MOMENT SHOWN:
`LayerSrc.matte: Option<Box<NestSrc>>` (the sub-plan at `t` and the plan's size, what the track composes today). The executor resolves it
as any sub-composite, takes its alpha by the existing `Pipe::Matte` (`by: alpha`) and cuts the picture by the existing mask-alpha pass,
both before the chain's remaining slots. Where the composite is opaque both are identities (t3380). `put_src`, `starved`, `durable`, the pass count and the
prewarm list read it. Consequence, and wanted: a smear past the moment's coverage is cut where nothing lies now.
Rejected: opaque over black (above); the alpha of the stream's own input at `lines` high (its edges are the codec's size, the picture's are not).

The codec's input is the executor's straight RGBA: under a fade the colours stay and only alpha moves, so nothing is darkened twice.

## D7. Second input

`SecondFrom::TrackAbove`/`TrackBelow` = video lane − 1 / + 1. `TrackSide { events: Range<usize>, index: LayerIndex, segs }`, the same
sub-plan of that lane's events with their chains, WITHOUT that track's own chain (two tracks' stages would read each other).
`second_start(k)` is the cut of that lane's segment holding `k`; none or a gap → `None`. `ident(Second, k)` is that side's moment key.

## D8. The order, pacing and every walker

- `VideoRenderer::streams_at(t) -> Vec<StreamAt>`: the active events' `streams_of`, and for every stage whose lane has an active, not
  silent event at `t`, its lines. `StreamAt { id: u64, clip: Option<ClipId>, host: Host }`, `Host::{Clip(&VideoClip, &StreamStage), Track(&VideoTrackStage,
  &TrackLine)}`, `open(quality, look) -> Box<dyn HostStream>`; `HostStream: StreamInput { pos, order, window, kernel, values, frame_bytes, stopped, trace_stop }`.
  The seven walkers of Context go over it; nothing else builds a stream.
- **The order** (`layers_at_into`): an event on a lane with a trunk line is NOT ordered as a flat leaf (its frames at `t` are, for the
  matte, at the plan's size: they stay) and the line's needs are added: for each needed `k`, `order_of(events, index, ctx, t_k, q)` (the body of
  `layers_at_into` made a function over a list and an index), inner stages included. The request is the read: t3382 compares the keys asked with
  the keys `Look::Known` was asked for.
- **The window** cuts positions by the bytes of ALL their keys (each key's own size), at least one position.
- `frame_bytes` (the walk's reach, `stream_ahead_frames`, `stream_far_frames`) is a position's stream picture, as an event's: the memo keeps that. The frames
  cache's want (`video_prefetch_pass_sized`) sees the real keys.
- `stream_preroll` (export, strips, posters) works over `layers_at` and needs nothing but these; `project_may_stream` and
  `prewarm::has_stream_front` count active stream slots of VIDEO tracks of the root and every reel (a track line always needs the front executor).
- `max_concurrent_layers`: unchanged (a position's frames are released by the front run; the chunk's bytes come from `layers_at`).

## D9. The paired sound

- kt-model: `track_pairs` admits every paired kind (the function goes; `paired_halves` is called directly; t3230's third clause changes by this change's delta).
- `AudioState::segments(&mut self, _segs: &Arc<[StreamSeg]>) {}` (kt-plugin-api, default: nothing), `StreamSeg { prev_cut, cut, end }` there
  too (kt-engine's `TrackSeg` is it). `TrackProcessor` hands it to its nodes where it hands `place`.
- `audio.rs` `build_reel`: a `TrackStage` whose chain holds an active sound half of a STREAM device gets `segs` of the video track that names
  it (`sound == Some(id)`), by `spans_of` + `track_segments` over the same `resolved`. No such video track: empty.
- `MoshSound::sched(n)` with segments: the segment holding `k0`; its own `Plan` (`Vec<OnceLock<Arc<Plan>>>`, compiled on first use) over
  `Layout { first: seg.first(before), cut, end }`; `reset` at the first audio frame of a segment with no `prev_cut`; between segments an intact frame
  (no break, no repeat). Without segments the code is today's byte for byte.
- **Invariant 3**: a moved cut calls `segments` on the live node: the plans are dropped, the codec's state stays.
- **The kept mix** must know: `scene_hash` gains, per stage with segments, the segments overlapping `range` widened by `stage_reach` and one segment back,
  NOT the list in `scene_base` (Debug of `TrackStage` skips the field): an edit far away keeps the kept mix.
- Two runs of one range: the layout is a pure function of the model.

## D10. Class walk: every place that assumes a stream belongs to an event

| Place | After |
|---|---|
| `track_stage_layer` wiring (row 1067) | D1 |
| `StreamStage::of`, reads of a chain | + `TrackLine` (trunk, arms) |
| `streams_of` walkers (7) | `streams_at` (D8) |
| `layer_src`'s reading of sources | `Look` (D4) |
| `moment_key` `put_src`, `FramePlan::{starved, durable}` | the matte |
| kt-gpu: resolve, pass count, prewarm of `Matte`/mask-alpha | D6 |
| `render_cache` `plugins_of`, `put_rows` | walk the matte and the arm reads (the salt, the rows) |
| the disk tier | no leaves: the key is `moment_key`; `durable` asks every source of the lane's segment |
| a retune or a replaced file mid-walk | `Look::Known`: an absent frame, then a new name |
| render-ahead `Waits`, Loading, `stream_short` | positions, unchanged |
| the export's pre-roll, `Job::Planned`, `has_stream_stages` | D8 |
| offline jobs (strip, poster, effect cells) | the same; their caches have no identities (row 1068): the name falls to addresses, so 1068's fix goes first |
| reels: a track stage inside a reel | the same host on the reel's grid; `stream_view: false` there as for its events |
| a reel's `nest_scale` (sub-plan at S×) | `quality` is the sub-plan's: the stream's size follows it |
| `video_decode_qualities`, the coverage bars | an event on such a lane is asked at the plan's size (the matte) and at the stream's: the bars keep the plan's |
| `read_rows` (the node editor's coverage rows) | events only: a track's arm has no coverage row (there is no track bar) |
| the trace (`StreamRun`, `StreamStop`, `StreamAdvance` ids) | the track's `track_source_media` id |
| `kt-cli probe` (arms per event) | prints a track's lines: "Video 1: Datamosh over the track, segment 3 of 12, 40 positions asked" |
| sharp carrier | `carried` → the canvas size (D5) |
| kt-model `track_pairs`; kt-app's refusal, `has_device`'s `!is_stream()` | admitted |
| the pickers' cells on a track chain | they run an event's picture through the candidate; a stream candidate on a track shows the event's own mosh: unchanged, honest for the look |
| memory ledger `stream_want` | `streams_at` per loop moment |
| `track-stage-preroll` (the sound's pre-roll) | the half is a node of the stage like any; its look-ahead is already counted |

## D11. Stages and tests

1. 1.0 wiring (t3396). 2. Layout and `Look` (t3379, t3398). 3. The host, the layer, the matte (t3380, t3381, t3395). 4. Order and
pacing (t3382, t3383). 5. Keys and kept moments (t3384, t3385, t3386). 6. Arms, front, second (t3387, t3388, t3397). 7. Export (t3389, t3390).
8. Sound (t3391, t3392, t4137). 9. UI and rest (t3393, t3394).

**t3380's precondition** (the law between the two hosts): the event's source has the project's aspect and fills the frame (an identity
lead), so the composite at the stream's size IS the decoded frame. Both renders on one device.

## Risks

- **Cost.** A history position composes the whole track. After a seek into a 3-layer lane the walk makes `before + (n − cut)` composites at the
  stream's size. The trace's `StreamAdvance` has their time; Loading says so; nothing waits for it.
- **The front executor's memory.** It now holds a lane's uploads, not one frame: its upload cache is the ledger's `Upload` tenant already.
- **An inner Datamosh under a track's** (Q3 a): walks multiply; the trace shows both ids.
