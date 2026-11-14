# Design: chain-everywhere

## Mechanism

The owner's п.2 of `kkt-26.11.14`: an effect is never laid past its chain, anywhere. His example
(found-defects 460) is one instance of a mechanism: **a picture or sound of an effect built outside
the one composition path (`frame_plan` → `VideoGpuExec`) or the audio render chain**. Such a path
has to re-create the chain around the effect by hand, and every stage it does not re-create (the
leading Geometry node, the slots in front, a stream stage's history, the track's stages, a reel
instance's chain, masks, the camera) is an effect run out of place. The picker ran
`chain_plan(still thumbnail, event chain + candidate)`: a Datamosh slot without a history passes
its input through, so VHS Tape appeared over the un-moshed frame. The same mechanism inside the
engine: a stream stage's inputs are run outside the layer (`ClipStream`), and stage 12 rebuilt only
the slots, not their masks and camera (row 541), and not the second input's chain at all (row 540).

## Decisions

### D1. A cell is the composite of the project after the click

`fx_thumbs::render_scene`: the project the player plays (`Player::project()`), the candidate
appended where the click puts it (`with_candidate`: the tail of the anchor's chain, or of the
morpher's video patch), composed at the reference size (320×180) through `frame_plan` on the side
engine — the path the reel strip and the poster take. Nothing about the chain is re-created in the
UI; the one composition path decides. Rejected: extending the old "base frame + chain" job with the
missing stages — every future stage would have to be added there too (the very mechanism).

### D2. The anchor's track is soloed (event cells); the morpher's cell is the whole frame

An opaque layer above the event would show every cell alike; the solo keeps the event's own track
stages (they apply to the track's composite) and drops only the other tracks. A muted track stays
muted (the cell is then empty — honest). A morpher acts on many layers (Below, areas, marks), so its
cells are the whole frame at the cursor clamped into its span. An event of an expanded band (inside
a reel) gets its chain extended in its reel; the frame is not soloed there.

### D3. The service opens its own sources, after the misclick guard

The old service had no decoder by contract (the base came ready from `ThumbService`). A real frame
needs the event's sources, the history's frames and the other files of a reel. The cells' lane keeps
a `SceneCtx` per project (by `Arc` identity): sources opened as `layers_at` names them (files through
`open_source_or_placeholder`, generators through `gen_sources`), their facts, a 64 MB cache, an
offline stream memo of its own (never the preview's, stream-memo-budget) and a `BuildMemo` — each
candidate's renderer re-derives only the anchor. This is invariant 5's offline path: not shared, no
promised pace, finite, cancellable (generation, lane pause/close). Before the guard (800 ms, item
430) a cell opens nothing and is Loading. The last cell window to close drops the context (on the
lane: the UI never takes its lock).

### D4. Loading, never a stand-in

A cell without its composite draws the backdrop and "Loading" (`perf.loading`). The sample is no
longer a fallback for an event; it remains only for a sample job (no chain exists). An incomplete
composite is never shown (п.1); a cell that cannot complete is remembered (`failed`) and not asked
again until the windows close, so a pause costs nothing (row 581). Keys: `(Scene{rev, place, t},
hash(candidate))`; `rev` is the model's revision and place being edited, so a generator edit needs
no media invalidation (`invalidate_media` and the `Thumb` base are gone).

### D5. A stream stage's inputs run as their layers do

`ClipStream::run_slots` runs a set of an event's slots over a frame the kernel reads: curves at the
position's material time, `u`, the camera `resolve_cam3d` gives for those slots (cameras and project
size now on `StreamStage`), and the slots' mask ops remapped to the sub-chain
(`ClipMaskPlan::ops_for`, `chain_plan_with`). The main stream uses it with the front (row 541); the
second input with its event's `input_slots` — its chain in front of its own stream slot, without the
leading node (row 540). The addresses name what feeds them: the front's key mixes the front's masks
and, when a slot reads a camera, the cameras (and the position); the second side mixes its event's
`fx_hash` (chain, curves, placement, masks), the cameras when read, and the position when animated.

## Class walk

Every place an effect's picture or sound is made outside `frame_plan`/the audio chain, or a
base/still picture stands in for the chain's running one:

| Place | Where | Finding |
|---|---|---|
| Effects-picker variant cells | `fx_thumbs`, `fx_registry::preview_cell` | **fixed** (D1–D4): the real frame |
| Strength-axis drag cells | same cell path | **fixed** |
| Library preset cells in the picker | same | **fixed** |
| Morpher / FX-batch cells | `morpher_editor` | **fixed**: were the sample with an empty chain — the effect in a vacuum |
| Track window picker | `track_window` | no picture cells (names only): nothing laid |
| Audio picker | `fx_registry` | text only, no sound audition exists |
| Transition editor | `transition_editor` | no preview of a kind exists |
| Reel strip, project poster | `reel_preview` | `frame_plan`, complete composites only: in place (row 462 stays) |
| Timeline filmstrip, bin thumbnails, waveforms | `thumbs`, peaks | the source's own picture/sound by design: no effect is shown |
| Device panels (ISP, VHS deck, camera), generator UI | `isp_panel`, `deck_face`, `camera_editor`, `gen_ui` | controls only, no picture of the effect |
| EQ / response graph | `eq_graph` | the slot's transfer function, a property of its parameters, not a render |
| `kt-cli` probe, preview, pause-scan, export | `frame_plan` | in place |
| Stream front: slots, masks, camera | `video_stream` | **fixed** (541) |
| Stream second input | `video_stream` | **fixed** (540); an event below that itself carries a stream stage feeds its slots in front of that stage, not that stage's output (row 582) |
| The event before's frames in a cut mosh | `with_frame(Main, k < cut)` | through this event's front (stage 12 D2): the codec records the lane |
| Frame blending | `layer_src` | blends decoded pairs before the chain by design; never on a stream layer |
| Barriers, fused runs, geometry series | `plan_chain` | chain order kept (R.44/R.45) |
| Kept composites, loop fill, probe plans | keyed by the same plan | in place |
| Morpher layers in the engine | `incremental::resolve` | the patch is applied to the target pieces before the plan (H1) |
| Audio chain, track stages, reels' sub-mixes | `audio.rs` | nodes in order on the running block |
| Sidechain key branch | `audio.rs` `sum_lane` | the key track's events through their chains and the key track's chain; a keyed compressor inside the branch works plain (the recursion cut, R.65) |
| Paired audio halves (VHS Tape, Datamosh, Camera ISP) | `AudioState`s | in the audio chain; they read rows and timing, no decoded input (stage 12's walk holds) |
| Reel envelope | `render_raw_audio_range` | the real audio render |

Found on the way: Datamosh compiled its schedule past its own start when the second input began
before the stream's first position (`rows_of` indexed an empty schedule — a panic; fixed);
and in the engine test's scene the mosh does not change with the second input's picture at all
(row 580, open: the vectors from another track look unused).

## Load

The cells compute once per (project revision, place, moment, candidate); at rest nothing runs: no
retry of a failed cell, the lane's queue is empty, the repaint gate is the queue (`pending`).
Measured (t2559, debug build, the harness on llvmpipe): an event carrying Datamosh then VHS Tape,
its gallery open — 32 cells composed once in 49.8 s of processor time spread over the cores (3.2 s
of wall time; the Datamosh history is walked once and shared by the cells through the service's
memo), then 60 frames of a moving pointer over the open gallery took 90 ms of processor time with
no cell rendered or asked. The old cells cost about the same per cell (the same chain at the same
size, without the history) and were equally quiet at rest; the old build was not measured side by
side (a second target tree on a short disk).
