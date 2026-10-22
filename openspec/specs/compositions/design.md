# Compositions — why it is this way

## What hurt and for whom

Before nesting, "an effect on the result" could not be expressed: effects live on events, and the
only workaround — render a piece and lay it back as a file — breaks non-destructiveness. A survey of five editors (note `nested-sequences`) showed a common
trouble: a nest as a copy of a file, its own settings for the nest (softness, jerks, an intermediate resample),
and "enter N levels to understand where the frame comes from". The owner made two decisions before the code:
a nesting is a reference to an object INSIDE the project, and it INHERITS the parent's settings.

Then it hurt round after round: slowing down and reverb on a composition did not work (there was a direct
prohibition, not a breakage); inside a composition the root played, and the view stayed on the root's axis;
a morpher and a camera were lost on collapse (the layer went inside and died without a row order); the owner demanded a hybrid mode — expand a composition in the same timeline
and play the whole program; a title in a slowed composition went in steps; a track under the ceiling was not displayed; insertion split an A/V pair to the ends of the edit; Enter did not lead
inside; compositions had no preview on the timeline, and later it was not cached; inside an
open reel one could not expand a nested one; an effect on a composition inside a composition
crashed the program; and finally, there was no way to parameterize the template of a news issue.

## Decisions and why

- **A reel is a reference inside the project, an instance is an ordinary event.** One reel in N places,
  an edit inside changes all of them; trim, drag, copy-paste, transitions, reversal and warp work with the
  ready event code. The address is a `MediaId` in one space with the files, but without a `MediaRef`: the question "is this a file?" is asked with a string in fourteen places, and a path for a reel would
  turn each of them into a silent wrong result.
- **Inheritance of settings ⇒ the E-law ⇒ an oracle.** A nesting with an empty chain is identical to
  its content, so expansion into effective clips is not a simplification but the reference against which
  the real sub-tree of the plan is checked byte for byte, for video and audio.
- **A nesting is a layer input, not a branch of the plan.** An instance has a level, a fade, `u`, a camera;
  it can be half of a transition; a separate branch would duplicate all of this in both
  executors (invariant 7). Only the source of the pre-FX frame changes.
- **Audio: a virtual media under the fetcher, a sub-mix only forward.** The mixer knows nothing about reels —
  hence the E-law for sound byte for byte and the untouched GOLDEN_AUDIO. Reversal and stretching act on the samples of the finished mix: "reversing the expanded" means "render forward and play backwards", there is no third result. The pre-roll and the block are functions
  of the project, not of the memory budget: otherwise the output bytes would depend on the budget (invariant 11).
- **Three keys.** The reel (a tag in the address), the instance (`fx_hash`), the revision (a recursive hash
  of the content); with publication the revision is a pair (reel, set), and with an empty set it equals
  the hash of the reel, so that projects without publications do not move a single key.
- **The reel's level ≠ the entry of the instance's chain.** The level is the order, one per reel (otherwise the inner sheet would have two cache keys); the entry is the execution, what this instance's chain is able to consume; when they diverge — fitting by one decision for both backends.
- **The editing context is an exchange of content**, not a parameter of the commands; expansion in place is the same trick with a time offset and a track number. The strip in the tail of the numeric space of tracks — inserting a root track does not touch it by construction, and "expand — collapse" byte-for-byte identity makes the addressing of edits unnecessary altogether:
  there is physically no way to cross the boundary.
- **Track number = row rank.** The engine sums by number, the screen draws in row order; the rule "z-order == screen" is held by one reordering gate and a validation guard. There is one exception — the expansion benchmark.
- **The reel's time is continuous; only the file sheet sets a floor.** A reel is not a source, it has no frames to snap to; a procedural source is computed at any tick.
  Frame blending is inherited by the leaves — one sub-composite per frame.
- **The reel's preview is a service of offline rendering by the same composition code**, keyed by instance, with a disk tier in the sidecar with a media fingerprint.
- **Public parameters: the template's value is not stored, the instance overrides only what is published.** For us an instance is a reference, so the template propagates an EDIT, and the override is the only legitimate place where an instance differs from the reel.
- **Gestures.** A quick gesture is the default mode (expand in place), a named item is a named mode (the full entry); Enter cascades inward, Esc cascades outward; the strips survive a change of context (the owner's decision, a revision of the former "without a strip").

## Rejected and why

- `MediaRef { path: "seq:N" }` for a reel — the worker, the watchdog and the index service would take it
  for a file; not one of them would fail, they would silently do the wrong work.
- Own fps/raster/frequency for the attachment — the whole family of competitors' defects arrives (softness,
  jerks, resampling); the owner's decision is to inherit. **Revision (R.62, change
  `frame-conform-and-event-geometry`): a reel's own raster and its own fps — by the owner's direct
  request** ("compositions need some square ones and others rectangular inside a rectangular project");
  the audio frequency remains shared. The reason for accepting the price: without its own raster a square/vertical composition inside 16:9 is inexpressible, and its own fps
  on the continuous time of the reel (R.56) is quantization, not frame conversion.
  Consequence for the E-law: the oracle "reel == expanded" applies when the settings
  equal the project's, and is honestly bounded (a negative test), rather than blurred
  by a tolerance; a reel instance with a foreign raster is brought in by the same rule as a file.
- "Inward only events and transitions, collapsing under a morpher — a refusal" — half as long, but it leaves the class "this cannot be collapsed".
- A nesting as a `MediaSource` — its reading would wait for the frames of other workers from its own worker thread: playback stops being non-blocking (invariant 5). Recursion is in the construction of the plan.
- A sub-mix in the burst cache with warm-up by the prefetcher — producing a sub-mix is a run of the FX state, which only the audio-render thread touches (invariant 2).
- The FX-state key by reel (rather than by instance) — two sounding instances shared the reverb and fought over the producer's cursor; t564 was red on the old key.
- "The current reel" as a parameter through sixty hands of commands — missing one hand silently sends the edit to the root.
- Expansion through "the row key knows the owner + `apply_batch_in`" — parameterizing ~2700 lines of drawing and auditing 182 calls of the working view with a profile "missed — silently".
- Two sub-composites per frame for blending on a nesting — twice as expensive on both backends and brings back the steps to the procedural one (both would be assembled on the grid).
- Storing the template's value at the handle — a second source of fact, which diverges at the first edit.
- Overriding "any property inside by address" — the template loses its interface and breaks on an edit inside.
- Rejected by the owner: the expansion tails "read-only"/"do not show"; "ask on every edit of the shared reel"/"always unlink"; inserting "at the cursor" or "tracks as in the original"; "do not show the zone at zero"; a "shared data folder" for the preview cache; restoring strips "all except the input"/"only the root".
- The notes "a filmstrip does not exist for a reel" and "it cannot be in the sidecar — the key is by file identity" — withdrawn as incorrect wordings: a thumbnail BY FILE is impossible, not a render; file identity keys the media-adding API, not the schema.

## Non-obvious details

- **The instance parameters window has no and will not have its own keyframe editor** (R.60). The curves of overrides are drawn by `automation::draw` (`src/apps/kt-app/src/automation.rs`) — the same widget as the effect slot and the generator. The first edition of the window had its own record: a nameless `+` glyph created a curve, and from then on every edit of a field put a key at the cursor, and there was nowhere to look at the curve ("I don't see any automation curves anywhere"). The curves of all handles are collected in ONE row (`src/apps/kt-app/src/pubparam.rs`, `draw_one_inst_params`): inside the window the curve is called by the handle's number (`pub_key`), and on writing the name is returned as the target — two handles may aim at identically named parameters of different events, and by the target name they would merge into one line.
- **A key at the cursor is written only for a VISIBLE curve.** The rule lives in one place — `draw_value_row` (`src/apps/kt-app/src/pubparam.rs`): if there is a curve in the row — `bound_apply`, if not — a constant. A curve can be created only by a named action, so no hidden recording remains by construction (t691).
- **The handle's limits are not copied into the publication** (R.60). The default is the passport of the TARGET by address (`Project::addr_spec`, `src/libs/kt-model/src/project.rs`); ONLY the narrowing is laid into `PubRange` (`PubRange::narrowed`, `src/libs/kt-model/src/published.rs`): a value equal to the target's limit is stored as `None`. The argument is the same as against a stored template value — the copy would diverge from the target when the effect's version changes. Before R.60 an empty narrowing was drawn as zero.
- The high bit of `MediaId` is given to the engine for the address of a reel (`NEST_TAG`, `src/libs/kt-model/src/project.rs`, `src/libs/kt-engine/src/video.rs`); a file with such an address is rejected by `validate`. In the video plan the address of a reel once went WITHOUT the tag and numerically matched a file with the same id — only the difference in `key_hash` saved it.
- `Sequence` is a partial duplicate of `Project`, and a content field introduced at the root has nothing in the reel to catch it except an exhaustive destructuring of both in t426 (`src/libs/kt-model/tests/nest.rs`).
- Fresh addresses of five kinds are computed over the WHOLE tree (`Project::max_clip_id` and relatives, `ops::NestCounters`, `src/libs/kt-model/src/ops.rs`): the maximum over the root gave a silent collision with an event inside a reel, and inserting a copy next to a nesting simply did not happen — it was found by a live scenario, not by a test.
- The rule "what travels with the selection" is one — `block_extras` (`src/libs/kt-model/src/ops.rs`): a camera added for the sake of collapsing automatically travelled both on copy-paste, and that is correct. `collapse_into_sequence` projects `lane_order` onto the surviving rows — without it the morph rows are last, and the layer "everything below" covers nothing.
- `expand_instance` (`src/libs/kt-model/src/ops.rs`) INSERTS tracks rather than occupying others' — otherwise the expansion silently changes the z-order of the neighbours; the addresses of the pieces of the inline start above the numbers handed out by the morpher pass to the same clip.
- There is no return of handles on collapse: the material was not cut, and `restore_handles` would give it to clips that are not in the root (`src/libs/kt-model/src/ops.rs`).
- Premultiplication: `execute_plan_cpu` starts with transparent black, and with `da == 0` the result is a premultiplied RGB with straight alpha. One conversion `unpremultiply` (`src/libs/kt-fx/src/video.rs`) and its mirror `shaders/unpremul.wgsl` (`src/libs/kt-gpu/src/engine.rs`) are edited in one commit; the arithmetic is integer, the boundary is checked against `FrameTol::EXACT`. Without the conversion the divergence is 48 LSB against 1.
- Only the FULL sub-composite is memoized (`nest_frame_cpu`, `src/libs/kt-engine/src/video.rs`): a starved frame under the reel's edit key is a black reel forever; a live player hit this on every cold start.
- The key of the GPU plan is recursive over the PLAN of the sub-tree, but not over the address/revision of the reel (`resolve_plan`/`encode_jobs`, `src/libs/kt-gpu/src/engine.rs`): starvation inside a reel changes the picture without changing the fields of the instance; an empty plan of a sub-tree must clear the texture — the pool hands out dirty ones.
- The entry of an instance's chain: `clip_chain_scale` = min(the reel's level, what this instance's `head_zoom` consumes by the cameras of THAT reel where it stands) (`src/libs/kt-engine/src/video.rs`); fitting — `NestSrc::fit_to` (`src/libs/kt-engine/src/video_plan.rs`) → CPU `fit_into` (`src/libs/kt-fx/src/video.rs`) / GPU `Resample`. The most expensive row of the table — barriers without a resample: the `debug_assert` did not fire, and the layer dropped out silently with `complete == true`. Lesson: t450 built a falling construction and never rendered it.
- The state of nested reels — `FxStateStore.nests`, taken out for the duration of the render (`take_nests`/`put_nests`, `src/libs/kt-engine/src/fxstore.rs`): the sub-render is called from the fetcher, which lives next to the store. Exactly one last burst of the sub-mix is cached: more is impossible (the chain is a streaming automaton), less is impossible (the cursor is on the burst).
- `sync_fx` and `reset_all` go inward: without the first, the processor of the inner chain is not created and the event is LOST (a test "differs from dry" would miss it — the difference exists in silence too, which is why t449 compares energy); without the second, after a seek inside the reel the tail of another place sounds (`src/libs/kt-engine/src/audio.rs`).
- `render_frames_raw` must wrap the fetcher with reels (`src/libs/kt-engine/src/audio.rs`): otherwise the order goes inward, but the reading does not (t565).
- The order and the reading of the sub-mix are one function `nest_read_window`, gated by the REVERSAL, not by the warp (the warp is monotonic, the "cost is linear" t436 is intact); the veto on stretching asks for the VALUES, not the kind of the slot — `chain_warps_time` (`src/libs/kt-engine/src/audio.rs`, `src/libs/kt-model/src/warp.rs`), which is why reverb on a nesting is legitimate.
- The warm-up of a nesting — a window by `warp.used()`, a stretched child "entirely or not at all" (`reel_coverage`, `src/libs/kt-engine/src/video.rs`): a linear translation under the warp would lie in the favourable direction.
- Internal time: `clip_inner_tick == clip_material_pos` for a reel, the floor only for the file sheet; the sign of the procedural one — `MediaInfo.fps == None` with `has_video`, the facts travel in `SourceFacts` (`VideoRenderer::new_with_facts`, `src/libs/kt-engine/src/video.rs`). The blending flag must travel to four places: `clip_frame_keys`, `max_concurrent_layers_in`, `reel_coverage`, `nest_memo_key` (there too).
- The render test of a reel puts the frame of a procedural clip as a worker (`RequestKind::video_on_grid`) and holds ONE `BurstCache` for all moments — `UploadCache` is keyed by its serial numbers (`src/libs/kt-gpu/src/engine.rs`).
- Three views of the project (`src/libs/kt-model/src/model.rs`): `project()` — the working view, `document()` — the document, `reel_document()` — the current reel as the root. Validation runs over the DOCUMENT: in the working view the instance of the current reel lies inside the reel itself and looks like a cycle. The end of the content of the reel is taken AFTER the exchange — `sequence(c.media)` after `set_editing` holds the content of the root (`src/apps/kt-app/src/app.rs`, `enter_nest`).
- "What plays" — `KtApp::player_project` / `push_project_to_player` (`src/apps/kt-app/src/app.rs`): `set_project` was called from eight places, and a single substitution would return the root at the first edit inside; the guard t554 checks that the decision point is exactly one — a prohibition without a self-check turns green on an empty set.
- An instance at zero degenerates the formula of the cursor translation into an identity — t456 was green without the translation; the harness on a shifted instance plus the scene guard `assert_ne!(want, outside)` (`src/apps/kt-app/src/layout_tests.rs`).
- The strip: `merge_reel`/`unmerge_reel` (`src/libs/kt-model/src/project.rs`), `bench`/`simulate`/`preview` (`src/libs/kt-model/src/model.rs`). `simulate` works on the bench and collects into the canon; `preview` must return the future bench — comparing the canon with the bench would silently and with undo cut the material of others' events; any edit of the canon calls `rebuild_bench()`, including fields outside the history. `Band::after_batch` is an exhaustive `match` without `_ =>`: the only place where correctness rests on a human.
- `canon_order`: the natural order of rows is canonized into an EMPTY vector, otherwise "expanded and collapsed" writes an explicit order to the file where there was none (`src/libs/kt-model/src/project.rs`).
- `BandOrderBroken` was introduced and removed: a break of the block by a root row is produced by the very insertion of a track, and the state does not live longer than a frame. One boundary remains — `BandEscape` (`src/libs/kt-model/src/project.rs`). The tail `base + len` belongs to the strip (`Band::lane_inserted`, `src/libs/kt-model/src/model.rs`) — otherwise an event on such a track silently drifted to the root.
- `TimelineModel::clip` searches the WORKING view (`src/libs/kt-model/src/model.rs`): in the canon the event of the strip lies inside the reel, and every gesture on the strip would silently answer "no such one".
- `Drag::NestWindow` (`src/apps/kt-app/src/timeline.rs`) — the only gesture where the pointer above the row of one reel edits an event of another; the commands are ordinary `TrimStart`/`TrimEnd`, `dt` under them is invariant (`clip_slice` moves `start` and `source_in` by one delta), and the content of the strip does not move — the window slides over like a shutter.
- The veil of the tails and the zone at zero are drawn AFTER the events, and the rows of the strip are labelled with the reel's name — root names on them would lie (`src/apps/kt-app/src/timeline.rs`).
- Strips across contexts — two memories, both keep `ClipId`, not `Band`: `NestStep::band` (removed BEFORE `set_editing`, which clears the strips unconditionally) and `KtApp::reel_bands` (`src/apps/kt-app/src/app.rs`); the strip is recalculated by `expand_band` (`src/libs/kt-model/src/model.rs`), and the undo snapshot wins.
- The decision "what Enter does" — `enter_pressed_on_nest` (`src/apps/kt-app/src/app.rs`) by `bands.any(inst == id)`; two guards "expanding is allowed only at the root" stood with a wrong comment — the model did not forbid this. `after_bands_changed(keep)` holds the selection of the instance: without it the second Enter did not find its target.
- The row rank: the single gate `Project::set_lane_order` computes `LanePerm` and renumbers everything, including `Retained.tracks` of kt-otio through `Retained::remap_tracks` — otherwise the metadata of a foreign track would drift to its neighbour (`src/libs/kt-model/src/project.rs`, `src/libs/kt-otio/src/convert.rs`). Rows in gestures are addressed by KEY: `reorder_rows` first collapses the strip, and the collapse changes the number of rows.
- Clipboard: `ClipBlock::localize`/`shifted_to_row` (`src/libs/kt-model/src/ops.rs`) — the clamp is BY KIND, not by event (otherwise two tracks stick together), the tracks are created explicitly (`lanes_needed`), not by silent growth in `AddClip`: the number `base` belongs to the strip.
- Zero of the reel: `KtApp::time_floor()` — the floor of gestures, `timeline::CURSOR_FLOOR` — the cursor (`src/apps/kt-app/src/app.rs`, `src/apps/kt-app/src/timeline.rs`); there are no bare `.max(0)` over time (t637c). In the file `kt.start` is written only when it diverges from the cursor of the OTIO track (`src/libs/kt-otio/src/convert.rs`).
- Format: `KT_META_V6` — the FIRST condition of the cascade (`src/libs/kt-otio/src/meta.rs`); a foreign raw of tracks is returned only at the root — the key `(kind, lane)` inside a reel would collide with the root's; a foreign object at the level of the children of the root `Stack` is stored with its position (`Retained.root_foreign`). `read_reel` (`src/libs/kt-otio/src/convert.rs`) wraps a `Stack` without tracks in a synthetic `Track` — the rule "clip after clip with gaps" is one.
- Preview of a reel: the request goes BEFORE the render loop by key; `PeakAcc` is extracted from `PeakPyramid::build_observed`, not written next to it; the moment of the frame — `thumbs::material_at` (`src/apps/kt-app/src/thumbs.rs`); `invalidate_seq` takes the SET of live revisions (`src/apps/kt-app/src/reel_preview.rs`). The disk tier — the tables `reel_strip`/`reel_frame`/`reel_peaks` by (seq, `content_hash`, `media_fp`); the key is computed in the thread of the task, `purge_media` does not touch them, `prune_reels` — from the task (`src/libs/kt-media/src/index.rs`).
- Publication: `resolve_reel` — the only gate (`src/libs/kt-model/src/project.rs`), `reconcile_published` next to `reconcile_transitions` in `simulate` (`src/libs/kt-model/src/model.rs`) — on every batch, without a list of triggers. The field `published` exists in both `Project` and `Sequence`, and `swap_reel` exchanges them. A curve is converted by the same `clip_material_at` as the morpher layer; for a transition the domain is the tick from the window, for a camera it is the progress of the event, and an "almost correct" curve is worse than a refusal.
- The salt of the generator: `gen_salt`/`gen_source_addr_in`, computed by `build_reel` and `gen_sources` (`src/libs/kt-engine/src/video.rs`); zero when `overrides_touch_sources == false` (`src/libs/kt-model/src/published.rs`). `content_hash` is computed from the Debug representation — the new field `overrides` shifted all revisions ONCE, the sidecar preview was recalculated once on opening; this is not a migration.
- The owner reel of a publication — `owner_reel_of` by the working view (`src/apps/kt-app/src/pubparam.rs`): an event of a strip belongs to the reel of the strip, otherwise `reconcile_published` would have removed the handle in the same batch. The handle's name is checked by the DRAWN glyphs (`painted_texts`), not by the accessibility tree — the accessible name of the reset button is compound and carries the handle's name too (`src/apps/kt-app/src/layout_tests.rs`, t684).
## R.67: reel preview survives to disk

- **Frames are written in their own transaction, peaks in their own.** A single write after the audio stage
  meant that a cancelled task (pause, edit, close) left nothing on disk — and the reel was rendered anew at every opening: "the composition preview is AGAIN not saved".
- **Cancellation is not a failure.** The key of a cancelled task does not go into the negative cache, and an incomplete
  record (frames without the envelope) RE-ISSUES the request: otherwise the envelope did not appear until the next edit of the reel. Only a render error counts as a failure.
- **There is no "peaks present" flag in the schema** — it is the non-empty `reel_peaks`. Bumping `schema_version` for the sake of one boolean would wipe the batch index, the peaks and the thumbnails of ALL media of the project.

## Deferred

- One strip at a time (recursive expansion) — the limitation is explicit, lifted by a line and a test (`src/apps/kt-app/src/app.rs`).
- Known sticking points of the strip: `lane_names` are not exchanged by `swap_reel` (the full entry shows the root names), `trim_max_end` does not hit the end of the content, `overlap_layers` of different reels in one view, highlighting of the morpher across the strip boundary, `MarkedArea.lanes` on track insertion (`src/libs/kt-model/src/project.rs`).
- Texture memoization of the sub-composite on the GPU and a switchable boundary (Collapse
  Transformations); narrowing the `UploadCache` key to the address `(source, cache_idx)` (`src/libs/kt-gpu/src/engine.rs`).
- The poster/scrub of the reel's card in the bin from the preview data (`src/apps/kt-app/src/reel_preview.rs`); a strict cross-render `nested_example` — melt
  drops the nested `Stack` (`src/apps/kt-cli/tests/cross_render.rs`).

## R.70: inline — a requirement of export, a reference — a requirement of the project

The requirement "the body of the reel inline in every instance, the sidecar advisory" described
the EXCHANGE FILE and remains correct for it: a foreign reader does not know `metadata.kt.seq` and
is obliged to see the edit. For the PROJECT file the same property is a multiplicative cost (the owner's three
reels → 15 220 clips, 115 MB), therefore the project stores the reel once, and the instance stores a reference to its address; this is a property of the model (`Project.sequences`), and the project's own format does not spoil it (t810). The laws E and G, the processing order, and the public parameters do not depend on the format — they are read from the same model.
