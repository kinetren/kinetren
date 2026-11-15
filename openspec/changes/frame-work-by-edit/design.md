# Design: frame-work-by-edit

## The mechanism

A question asked per frame (or per edit, or per keystroke) was answered by walking the whole
structure it lives in: every event of a renderer for "what is active at t", every event of a reel
for "what does this placement's window cover", every reel for "which reel is this id", every event
of a lane for "is there a third event in this staircase window", every reel of the project for "which
addresses does this generator have". The answer depends on a few elements; the walk is over all of
them. On the showcase it is invisible; on a 2000-event edit it is the floor under every frame.

The remedy is one: lay the structure out ONCE where it is built (per build, per batch, per scope)
into an index keyed by what the question asks (a tick, a window, an id, a lane), and ask the index.
Every index here is derived from data that cannot change while it is alive (it lives in the build,
or under a borrow), so it cannot go stale silently; each answers exactly what the scan answered, in
the scan's order; none is visible to determinism (invariant 11).

## Decisions

### D1. `LayerIndex`: the renderer's events by time (row 153)

`VideoRenderer` keeps `LayerIndex` over its `clips` (sorted in composite order), built at the end of
`build_reel`: the boundaries of every event's span with its handles (`start − head_ext`,
`end + tail_ext`) sorted, and per run between two boundaries the ascending indices of the events
active there. `at(t)` is a halving plus a slice. Ascending indices are the list's order, so iterating
`at(t)` IS the former `clips.iter().filter(active_at)` — the plan's layer order, `staged_done`, the
dissolve partner's position: all unchanged. t2662 compares the two at every boundary of a scene with
overlaps and an event under everything.

Asked by: `build_plan` (the root, a reel's sub-plan, a track stage's sub-plan with the stage's own
index over its lane), `layers_at` (the order, `wanted_frames`, `absent_layers`, prefetch, export
chunks), `stream_advance`, `stream_advance_run`, `stream_stages_at` (the memo's want),
`stream_frame_bytes`, `stream_checkpoints`, and `reel_coverage` through `over(from, to)`.

The index also carries the dissolve partners (`find_pair` scanned all events per transition frame)
and the track stage keeps its event range, index and sub-composite revision (`track_stage_layer`
hashed every event of the lane per frame).

Memory: runs × active events. A pathological project of N events all overlapping each other costs
N² indices; real edits have a handful active at a time. Rejected: a bucket grid like the audio side's
`index` (bucket of `INDEX_BUCKET_FRAMES`): it needs a filter per event per frame and a merge across
buckets; the run cover hands the answer itself.

### D2. Reel coverage costs the window (row 154)

`reel_coverage` asks only the inner events whose span meets the window (`LayerIndex::over`), collects
their cold parts and subtracts them from the window in ONE sorted pass (`subtract` merges the holes
and walks both lists once; t2672 holds it to the former one-hole-at-a-time subtraction on random
runs). `nest_coverage` answers the placements of one release over one window and decode quality once
per call. What the bars draw is unchanged (the existing t... nest_render coverage tests).

### D3. Reels by id inside a pass (row 157)

`kt_model::with_sequence_index(project, f)`: inside `f`, `project.sequence(id)` answers by a hash
index of the reels. The index is keyed by the project's address and lives exactly as long as the
`&Project` borrow `f` runs under, so the reels cannot change while it is alive. The builders
(`VideoRenderer::new_with_facts`, `AudioRenderer::new`, `gen_sources`, the player's shared build) run
under it. Rejected: an index field on `Project` — the reels are a public `Vec` every command edits,
and a stale index would answer wrongly in silence.

### D4. A staircase transition's third event by lane (row 156)

`LaneSpans` lays a reel's events out by (kind, lane), sorted by start with the running maximum of
the ends: the events meeting a window are those starting before its end, walked back while an
earlier one can still reach it. The answer is the scan's first event in the list's order (the error
names it). `Reel::lane_busy` defaults to the scan; `Indexed` (validation) lays the lanes out on its
first call; `reconcile_reel` on the first overlapped pair of a batch. t2665.

### D5. One resolved chain per (layer, moment) (row 168)

The plan's `LayerSrc.fx` is a `LayerChain`: `Borrowed` (no live curves, as before), `Shared` (an
`Arc<[FxInstance]>` the layer's `ChainMemo` keeps for its last four material ticks) or `Owned` (a
plan changed it, the stream stage's front). The probe plan, the composite, a retry and the loop fill
of one moment copy the chain once (t2666). A playing frame still resolves its chain once per new
moment: the values are new, and the executor reads `FxInstance`s. Rejected: an overlay of values on a
borrowed template — every executor path (`chain_ops`, the plan of `kt_fx`) reads parameters from the
chain; the overlay is a rewrite of all of them for a copy the memo already shares.

The memo is a pure function of the layer and the tick; a copied layer (the incremental build, a
stream neighbour) starts empty and prints nothing in `state_text` (t2004 compares the layers).

### D6. Carriers by compiled row (row 169)

`RowCurves` compiles a carrier's curves to passport row indices once; `apply` sets them with
`set_param_idx` — the same setter `set_param` reaches (`TransitionKind` and `GeneratorKind` now say
so for a plugin row), the same `curve_value`, the same order. Used by the transition kinds of a
renderer (`TransAnim`), the cameras (`cam_rows`, `params_at_rows`) and the generator source in
kt-media. Masks compile to `MasksPlan`: the list without curves shared as an `Arc` when nothing is
keyed (no copy per frame at all), each keyed row's passport entry found once (`Mask::set_spec`); the
by-name path built the whole passport per row (`Mask::set` → `specs()`). The by-name paths stay for
the UI and count `Cost::ParamByName`; a frame counts none (t2667, which also holds the bits).

### D7. A keystroke's other addresses on the builder (row 190)

`Player::update_gen` retunes the edited event's own address and hands the rest to the builder
thread (`Builds::want_gen_resync`, carrying the reel sources to stale): `gen_sources` (a resolve of
every reel) ran on the UI thread per keystroke. The builder resyncs between builds; a source whose
passport moved is replaced there and asks a build (`refresh`). The stored project is patched only
when it does not already hold the values: the app hands the model's snapshot before syncing, so the
whole-project copy (`Arc::make_mut` of a shared project) is gone from the keystroke too (t2669).

### D8. The export's stream question (row 191)

`project_may_stream` walks the slots (events of every reel, morpher patches) for a stream kind: no
resolve, no renderer. It may say yes for a reel placed nowhere — then the export runs serially, the
same bytes (the pipelined and serial paths are held equal by t1457/export tests). t2668.

### D9. What an edit's build still walks (row 192)

Measured, not changed (see "Numbers"): the root's resolve and the Debug-text key of every event run
per edit, off the UI thread, once per version. The audio and the video builders hash disjoint events
(audio events / video events), so there is no shared work to fold. Making them incremental needs a
persistent identity of events across versions (the model's `Arc::make_mut` copies the project on
every edit), i.e. the audit's step 4. The ×10 synthetic measurement decides the verdict.

### D10. Executor and simulation vectors (rows 364, 365)

`plan_key` streams the masks' Debug text into the hash (the same digest as hashing the string, with
no string built); a run's shape key is on the stack. Camera ISP's passes are a stack list and its
`gpu` reads the shared defaults; VHS Tape builds one vector a frame, not two. The rest of the
executor's per-frame vectors (`chain_ops`'s ops, the jobs, the retired textures, a barrier's saved
nodes) are sized by the frame's own chains: work of the frame, not of the project — outside this
class, left.

## The class walked

Every place a per-frame or per-edit question can walk the project (grep of `clips.iter()`,
`sequences.iter()`, `.sequence(`, `transitions()`, `set_param(&`, `apply_curves`, `find(|`,
`position(|` in `src/libs` and `src/apps`), what was found and what was done:

| Subsystem | Question | Before | Now |
|---|---|---|---|
| video plan (`build_plan`) | layers at t | all events | `LayerIndex::at` |
| video plan | dissolve partner | all events per transition frame | index |
| video plan | track stage's events, revision | lane scan + hash per frame | per build |
| video plan | track masks | clone per frame | shared `Arc` |
| order (`layers_at`, `wanted_frames`, `absent_layers`) | frames at t | all events | index |
| prefetch (`video_prefetch_wants`) | 2 s of frames | all events × frames | index |
| export chunks, `max_concurrent_layers` | per frame / per build | all events per frame / sweep per build | index / unchanged (per build) |
| stream stages (`stream_advance*`, want, bytes, checkpoints) | stages at t | all events | index |
| stream `ClipStream::at`, `second_index` | second input at a position | lane scan per position | halving over the running end |
| stream `link_stages` | per build | every event copied even with no stage | returns at once without a stage |
| reel coverage (`nest_coverage`) | placements × reel events, `subtract` quadratic | | window index, one pass, once per window |
| audio render (`active_clips`, `LaneCover`) | events at a burst | bucket index (R.43) | unchanged |
| audio prefetch (`wanted_audio_bursts_for`) | | engine-audit: scans and rebuilds segments | per-sample-costs' area (parallel executor) — not touched |
| `Project::sequence` | reel by id | scan of reels per event | `with_sequence_index` in builds |
| `Project::source`, media lookups | file by id | scan of media | per-edit and per-open only; listed, left |
| `check_transition_full` (validate) | third event | lane scan per staircase | `LaneSpans` via `Indexed` |
| `reconcile_reel` (every batch) | third event | lane scan per staircase | `LaneSpans` |
| commands' `check_on` of one transition | third event | one scan per command | the scan (one per edit, linear) — left |
| automation of a layer | resolved chain | copy per plan | memo per (layer, tick) |
| transitions (`TransAnim::mix_at`) | params by name per frame | | `RowCurves` |
| cameras (`resolve_cam3d`) | params by name per frame | | `RowCurves` on the plan path; the UI's picker keeps by name |
| cameras (`active_camera_in`) | active camera | scan of cameras per layer | cameras are few; listed, left |
| masks (`masks_at`) | passport per row per frame | | `MasksPlan` |
| stream front (`with_front_plan`) | camera by name | per computed input | memoized by the front memo; left |
| generators (kt-media `kind_at`) | params by name per frame | | `RowCurves` |
| title keystroke (`update_gen`) | other addresses | `gen_sources` on the UI thread | builder thread |
| title keystroke (`update_gen`) | stored project | whole-project copy | none when it holds the values |
| app `sync_gen_sources` | changed values | walk of the document's events | per edit, compares, no resolve; left |
| app respawn path (`sync_generators`) | passport moved | `document()` + `gen_sources` on UI | rare (a passport change); found-defects 640 |
| export (`render_project`) | stream stage? | a whole renderer | `project_may_stream` |
| incremental build | per-event key, root resolve | per edit | measured (D9) |
| kt-gpu `plan_key` | masks Debug | `String` per masked layer | streamed |
| kt-gpu `run_pieces` | shape key | `Vec` per run | stack |
| Camera ISP `passes`, `gpu` | | `Vec` + defaults copy | stack + shared |
| VHS Tape graph | | two vectors | one |
| kt-app timeline draw | per event | `EventIndex` (ui-frame-cost) | unchanged |
| kt-app coverage bars | reel coverage at 4 Hz | P×M | the engine's window index |

### The callers walked

- `LayerSrc.fx` changed type: every constructor (kt-gpu tests and testkit, kt-cli `fusecost`,
  `passcost`, `firstframe`, `video_plan::chain_plan_with`) and every reader (kt-gpu `chain_ops`,
  `resolve_chain`; `auto_plan.rs` t644/t645 now ask "not borrowed" for "owned").
- `build_plan`'s new argument: the root, the reel sub-plan and the track stage.
- `Project::sequence` callers outside the builders keep the scan (one-off, per edit).
- `update_gen` callers: the app's `sync_gen_sources`, tests t661, t663 and t2669; the other addresses
  arrive from the builder thread a moment later — no caller waited for them synchronously.
- `has_stream_stages` callers other than the export (the player's prefetch, kt-cli, reel strips,
  thumbnails) ask a renderer they already hold: unchanged.

## Numbers

(release, footage in /dev/shm; filled by the measurement task)
