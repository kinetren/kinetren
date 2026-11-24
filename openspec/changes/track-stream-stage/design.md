# Design: track-stream-stage

## Context

- **The kernel never sees an event.** `kt_plugin_api::stream`: a `StreamKernel` asks a
  `StreamLayout`/`StreamInput` for:
  - positions: `first`, `cut`, `end`;
  - addresses: `ident(side, k)`, from which every memo key is built without reading a frame;
  - rows: `rows_at`;
  - frames: `with_frame`, `on_device`.

  `SecondFrom` already names a TRACK (`TrackAbove`/`TrackBelow`). The host is the only part that knows
  what a position reads.
- **The event host**, `ClipStream` (`kt-engine/src/video_stream.rs`):
  - Its positions are the project grid. The main stream is the event, plus `before` frames of the event
    before it on the lane.
  - `ident` mixes the media, pts, stream size, `src_rev`, the front's key, and the position when the
    front is curved.
  - Frames come from the burst cache at the stream's size. The front slots run on the front executor;
    `on_device` uses `execute_held`.
- **The track stage** (`track_stage_layer`, `video.rs`): a track is an implicit reel. `build_plan`
  composes its events (transitions, fades, levels) at the parent's time and raster. The track chain runs
  over that composite (`LayerChain::Borrowed(&st.fx)`), then the track's masks, alpha × level and the
  blend mode. The stage carries its own range, index, revision and masks (frame-work-by-edit).
- **The datamosh sound** (`fx-video-codec/src/sound.rs`): the picture's `mosh::Plan` is compiled over a
  `Layout` of the audio's event (`first`, `cut`, `end` on the picture's grid), and the audio codec
  breaks on that schedule. A track's device sounds on its sound track's stage, seated at timeline frame
  0 (track-device-sound). `track_pairs` keeps Datamosh out.
- **The refusal**: track-mosh-and-start-wait D1 and t3237.

## D1. A second host: `TrackStream`

`TrackStream` implements `StreamLayout` and `StreamInput` for ONE segment of a video track stage
(D2). It holds:
- the track stage (`VideoTrackStage`);
- the track's events and their `LayerIndex`;
- the `PlanCtx` the stage composes in (cameras, nests, `blend_footage`, the stream memo);
- the stream's size;
- the segment's `first`/`cut`/`end`;
- the plan cell.

`StreamStage::of` is reused for the track chain. Its `front` is the track chain's picture slots before
the stream slot (a track chain has no leading Geometry node). `before` and `second` are filled by the
track linker (D2, D5) instead of the event linker. The kernel, `needs`, `render`, the memo, the
checkpoints, the device walk and the sharp carrier are unchanged.

`track_stage_layer` runs the stage where `layer_src` runs an event's. The stage's output becomes the
sub-composite the track chain's remaining slots run over. The slots in front become the identity
geometry (`after_chain`), exactly as on an event. The track's masks and level stay after the chain.

## D2. The track's layout: segments at its cuts

`track_stream_layout(track, events, grid) -> Vec<Segment { first, cut, end }>` is one function in
kt-engine. The video builder (D1) and the audio builder (D6) both call it.

- **A cut** is the grid position of an event's first frame on the track, where that event was not
  active at the position before it. Under a transition the incoming event's first frame is in the
  window (Q1 a). Q1 b would put it at the joint instead.
- **A segment** runs from one cut to the next cut or to the end of the contiguous run of events.
- **A gap ends the stream (Q2 a).** The next segment has no history before its cut, as an event with no
  event before it. Otherwise `first = cut − before(p)`, clamped to the previous segment's cut.
- **Positions belong to segments by halving** over the cuts (as `second_reach` does). The stage at
  frame time `t` uses the segment holding `pos(t)`.

This gives the kernel exactly the shape it already has: a cut, a history that starts in what came
before, and an end. A dense edit is many short segments, and each walks only from its own `first`.

## D3. The input frame: the track's composite at the moment of k

The frame at position `k` on the main side is the track's sub-composite at `t_k = k · step`:

- The same `build_plan(events, index, sub_ctx, cache, t_k, q_stream)` the stage uses, with
  `stages: &[]` as today.
- Wrapped by `chain_plan_with` with the track chain's front slots (their curves at `t_k`; the track's
  camera rows for a camera-driven slot), at the stream's size.
- Executed by the stream front's executor: `execute_held` on the compute device (`on_device`), or
  `execute_once` for the bytes form (`with_frame`). The result is kept in the front memo by `(ident,
  form)`.

This is the one composition path (invariant 7): no second compositor, no CPU path. An incomplete
sub-plan (a starved layer) gives no frame. The kernel's `None` is then starvation, and the order asks
what was missing (D4).

**The stream's size** follows `input_quality` with the track's raster (the project size) as the layer
size: the codec's `lines` while the kernel is not `carried`; the layer's size when it is carried (sharp)
or when a front slot would run above it (t2974's rule).

**Event-level stream stages inside the sub-plan** (Q3 a) run in their own `layer_src` with the same
memo. Their keys enter the track's identity (D4).

## D4. Order, pacing and keys

**Order.** For a history position `k` the needs are the layers the track's sub-plan asks at `t_k`:
`layers_at` over the track's events at the stream's size, including an inner stream stage's own needs.
`TrackStream::order` emits them position by position, ascending. Each source has its own media worker,
so every file's keys reach its worker as a forward run whatever the interleaving (the anti-ping-pong
rule picks the start of the run). The request is the read: the sub-plan asks exactly these keys
(invariant 4).

**Pacing and reach** (decode-request-storm, gpu-throughput D3, stream-round-trips D8). `window` cuts the
needs by BYTES: each position costs the sum of its layers' key bytes at their decode sizes, not one frame
of the stream's size. `compose_steps`, the near/far walk and `STREAM_WALK_SECS` keep their meaning in
positions. The render-ahead producer's `Waits` and the Loading count report positions as they do for
an event.

**Identity** (`ident(Main, k)`): `mix(TRACK_STREAM, track id, k, stream size, front_key, moment(k))`.

- `moment(k)` is the serial-free hash of the sub-plan's layers at `t_k`: for each layer, its source
  address, pts, decode quality, `src_rev`, `fx_hash`, `alpha_k`, fade and transition `k` and mode, blend
  mode; for a nest, its `content_hash` and inner time; for an inner stream stage, its kernel's `key` at
  that position. It is the per-moment part of `render_cache::durable_hash`, built by the same walk
  without decoding. A kernel without `key` salts the position with the track's revision: correct, and
  coarser.
- Consequences:
  - An edit of one event changes `moment(k)` only where that event is active. The history before the
    edited range keeps its memo entries; positions after it change through the history chain, as they
    must.
  - An edit after the stream slot does not touch the identities, as on an event.

**The layer's key.** The track stage layer's `content` and `fx_hash` mix the stage's output key, as an
event layer's do (`video.rs`), so no memo or kept composite returns another mosh state.

**The probe key** (`show_kept`, structured-telemetry). The probe plan keys the track stage by
`StreamKernel::key` over the `TrackStream` layout without running it. `key_only` gets a track twin.

**The disk tier and the durable history.** `probe_durable` notes, as leaves of a moment whose layer is
a track stream stage, every file the segment's history reads from `first` to the moment: the files of
the events active over that range, found by the stage's `LayerIndex`. (The history itself is not stored on
disk: the owner's D6, kkt-26.11.22.) render-cache-disk's rule "every file for a stream stage" thus holds per
segment, not per project.

## D5. Front, second input and tempo at track level

- **Front:** the track chain's picture slots before the stream slot, run over the composite at every
  history position, keyed by `front_key` (their slots and curves hashed). The slots after it run over
  the stage's output, as the track chain does today.
- **Second input:** `SecondFrom::TrackAbove`/`TrackBelow` names the adjacent VIDEO track in row order.
  Its frame at `k` is that track's sub-composite at `t_k`: its events and their chains, without that
  track's own chain (a track chain may carry its own stream slot, and reading through it would make two
  stages read each other). Its identity is that track's `moment(k)`. No adjacent track, or a gap there,
  means no second frame, as for an event.
- **Tempo:** a history position costs one sub-composite of the track. The pacing counts positions. The
  measured cost per position comes from the existing cost map (the walk's passes are timed like any
  pass), so render-ahead's depth and the export's pipeline need no new rule. An export pre-rolls the
  track stage's history like an event's (`stream_preroll` walks every stage of the frame, both hosts).
  A frame with a track stream stage takes the pipeline's `Planned` route (stream-round-trips D6).

## D6. The paired sound

- **kt-model:** `track_pairs` admits `datamosh`. On a linked video track the sound half
  `datamosh_sound` is mirrored onto the sound track's chain by the existing rule.
- **The sound half runs in the sound track's stage**, seated at timeline frame 0 with the project grid
  (track-device-sound). Its `Layout` is built from `track_stream_layout` of the LINKED VIDEO TRACK, the
  same segments. `sound.rs` compiles the picture's `mosh::Plan` per segment, as it compiles one per
  event today, so I-frames, deleted keyframes, blooms and freezes fall at the same timeline moments in
  the picture and in the sound.
- Between segments (a gap, Q2 a) the codec's predictors reset, as at an event boundary.
- Invariant 2: the half's state is the audio render thread's, keyed by the track stage, as every track
  stage's node is.
- Audio is byte for byte: the layout is a pure function of the model, and the audio builder computes it
  from the same events the video builder does.

## D7. Class walk: every place that assumes a stream stage belongs to an event

| Place | Today | After |
|---|---|---|
| `StreamStage::of` and the linker | event chains, morpher patches | + the track chain (D1) |
| `track_stage_layer` | skips a stream slot | runs the stage (D1) |
| host `ClipStream` | events only | + `TrackStream` (D1–D4) |
| `layers_at` / `wanted_frames` / `video_decode_qualities` | an event stage's needs at codec size | + a track stage's needs at the layers' sizes (D4) |
| `stream_want` (memory ledger, stream-memo-budget) | event stages and reels | + track stages, per position the stream's picture (D4) |
| `stream_advance_ahead` / `_run`, far walk | event stages of the shown layers | + track stages (D4, D5) |
| `probe_plan` / `key_only` / `show_kept` | event stages | + the track twin (D4) |
| `render_cache::probe_durable` leaves | an event stage's files | + the segment's files (D4) |
| render-ahead `Waits`, Loading count | positions short | unchanged, positions |
| export `stream_preroll`, pipeline `Planned`, `project_may_stream` | events, morphers | + tracks (D5) |
| reels: a track stage inside a reel's sub-renderer | the same `track_stage_layer` | the same host, the reel's time |
| the trace (`StreamRun`, `StreamStop` id) | the clip id | the track's source id (`track_source_media`) |
| `kt-cli probe` (stream stages in the order) | events | + tracks |
| sharp carrier (mosh-sharp) | the layer's size | the track's raster (D3) |
| kt-model `track_pairs` | Datamosh excluded | admitted (D6) |
| kt-app track window | refuses Datamosh (t3237) | accepts it (ui-reach delta) |
| prewarm `effect_ids` | walks tracks already | unchanged |
| audio track stage seat | frame 0, project grid | unchanged; the layout from D2 |

## D8. Stages and tests

1. Layout and host:
   - D2 `track_stream_layout` (t3379);
   - `TrackStream` with the composite input (t3380: a track of one event with no transitions moshes
     byte for byte as the same event's own Datamosh — the law that keeps the two hosts honest; t3381:
     the history crosses a cut and carries the previous event's picture with its own chain and a title
     above it);
   - the order (t3382: the request equals the read across a history position of three layers);
   - the pacing by bytes (t3383).
2. Keys:
   - D4 identity (t3384: an edit of an event in segment 2 keeps segment 1's memo entries and changes
     segment 2's from its first affected position);
   - probe key and kept moment (t3385);
   - disk leaves per segment (t3386).
3. Second input and front (t3387, t3388).
4. Export:
   - pre-roll and pipeline (t3389: a range export equals its part of the whole export byte for byte;
     two runs equal);
   - `project_may_stream` (t3390).
5. Sound:
   - D6 (t3391: the sound half on the sound track breaks at the same timeline moments as the picture's
     deleted keyframes; t3392: model pairing and mirroring on a track).
6. UI:
   - the track window accepts Datamosh (t3393, replacing t3237's requirement);
   - planned live scenario `track_mosh` (a dense edit's track moshed, Loading then picture, the sound
     track's half switched off and on);
   - the rest check with the track window open on the heavy showcase plus a track mosh (t3394).

Spare: t3395–t3398.

## Risks

- **Cost:** a history position composes the whole track. On a dense edit a fresh walk after a seek costs
  `before + (n − cut)` sub-composites. Loading says so, and the far walk computes ahead. The owner's rule
  "a slow effect costs frames, never the Undo" holds: nothing waits for it.
- **Memory:** a stream picture per position, as for an event, plus the front memo's sub-composites.
  The ledger's stream want counts both (D7).
- **Recursion:** a track mosh over an event mosh (Q3) multiplies walks. The identity names the inner
  key, so nothing stale is shown; the cost is visible in the trace (`StreamRun` per host).
