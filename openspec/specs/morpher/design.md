# Morpher — why it is this way

## What hurt and for whom

The morpher was born as "a patch on a frozen list of clips, applying as a whole and outside of time": a block on the timeline could be moved, and that meant nothing. The owner demanded a full layer: "active by time", "applied to everything BELOW", a reference to a region/area instead of a selection, one window with one chain instead of two kinds of morpher, and copy-paste that "points" a copied morpher at new objects.
Reconnaissance of other editors (Vegas Adjustment Events, AE adjustment layer) revealed the main flaw: they have an overlay opacity slider with fades, while we had a binary `enabled`. Later the owner brought two live projects: "sound from another event came from somewhere" (a morph block at 0.6 s at second 5020 replaced the source with the one at 148), "pan3d is not applied until I turn off the source replacement", and "on events with a fade the morpher applies its corrections to the whole event instead of by time".

## Decisions and why

- **Flattening in the model; the engine knows nothing about morphers.** A single output point (`Project::effective_clips`) feeds the preview, export, rendering and `kt-cli probe`; bake == flatten by construction, and "the batch sounds without bake" is a consequence, not a separate branch. The price is per-clip input: a layer processes each clip, not the mixed picture of the tracks below. It is precisely from this that source replacement, `Areas` with a set of tracks, and an honest bake grow — things that Vegas/AE do not have (note `docs/features/morpher/gaps/composite-below-vs-per-clip.md`).
- **"Below" is by rows; the row order lives in the model.** Otherwise rendering would depend on UI state that undo does not restore. The overlay order is bottom to top, like an adjustment layer in AE; within a row the previous order by id is kept, so the old flatten test stays valid.
- **Patch = substitution + chains of both kinds.** A direct requirement: "in the morpher UI, merge them". In the window this is one lane; the substitution is a pinned purple head.
- **Partial coverage: the target is cut along block boundaries** — the owner's decision, as with an adjustment layer; the conscious cost is that the audio chain on the piece starts anew. Later (R.55) this was narrowed: only source substitution and warp in a patch are cut, and a patch made only of effects SUPPRESSES strength with gates — "I do not understand why the morpher slams the whole event if there is an innocent transition". Cutting for the sake of a chain is not needed at all.
- **The boundary sticks to the edge of a crossfade/overlap, and this is visible.** Cutting breaks not the target but its neighbours: for video the composition order is "on top is the one that started later", for audio the ramp is computed from the geometry of the clips. This is paid for by the engine not being touched.
- **Bake cuts honestly.** The invariant "bake == what was drawn" matters more than a clean timeline; new joins appear on it.
- **Reference by name; names are unique within one space shared with markers.** Markers are addressed by time; switching the five marker commands to ids would cost more than the uniqueness the owner already asked for to support copy-paste.
- **The layer's strength is a `LayerMix` chain slot, not an event field.** It gets automation for free (fades are its curve), enters the key of the post-FX cache (60 % and 100 % are different frames), travels with copy-paste, and does not touch the event format. The closest analogue is not Vegas opacity but Effect Opacity in AE / Key Output Gain in Resolve (`docs/features/morpher/gaps/opacity-and-wet-dry.md`).
- **The layer's curve domain is block time.** The morpher has no material; it is the same domain as for a camera and a transition; keys are translated into the target's material by the common mapping.
- **Reach first, then veto.** The question "did the layer reach by time" is asked before "cut or patch whole" — otherwise the veto would hand the patch to the whole project.
- **The single item "Add morpher layer" always creates a layer in "below" mode** and raises the morph row to the top — predictable, not a "smart default from context".
- **On paste — a dialog with a prefilled affix**, the default suffix "copy" (copies sort next to the original).
- **The morpher travels into the reel together with the selection and `lane_order`** (capability `compositions`): without row order the morph rows in the reel are last, and a layer "below" silently covers the void — "the morpher does not get into the composition".

## Rejected and why

- **The morpher acting on all tracks** and **"below" by rows, but the order in UiState** — rejected: the first makes the reference modes meaningless, the second makes rendering depend on state outside undo.
- **Overlapping — patch whole** (without cutting) — rejected by the owner: a layer must act by time; later, for pure chains, it was replaced by gating, not by cancelling the cut.
- **The engine learns the geometry of the original** (exact cutting inside a crossfade) — rejected: a second source of truth about geometry and a loss of bake accuracy. Recorded as a possible stage, if sticking gets in the way on live editing.
- **A layer's bake is forbidden** — rejected: bake must yield what was drawn.
- **Real ids for markers/areas** instead of names — rejected on cost (five marker commands at times) given that name uniqueness is needed anyway.
- **Cut, but only by narrowing** and **leave as is, but rename** (for "by time" on an event with fades) — not chosen: the owner chose "do not cut for the sake of a chain".
- **A composite stage "effects on the mixed picture below"** (as in Vegas/AE) — not rejected, but deferred as a SEPARATE object, not combined with the per-clip layer via a checkbox: an object "with two semantics" is worse than two objects.
- **Strength as a `ClipEvent` field** — rejected: editing dozens of literals and the event format for the sake of what the slot gives for free.
- **Layer curves in the material domain** — impossible: a layer has no material.
- **`MorphKind::Replace | EffectBatch`** — removed: two mutually exclusive kinds for the sake of one edit.
- **Placement mode "click the morph track, Esc to cancel"** — removed together with the single item: the file is now dropped straight into the source field of the window.

## Non-obvious details

- **Order of checks in `windows_on`** (`src/libs/kt-model/src/project.rs`,
  `MorpherEvent::windows_on`): first `enabled`, covers-by-rows and patch-does-something to this kind, then the time window, and only then the warp veto. The veto used to come first and returned the whole body without asking about time: in the owner's project 1973 events out of 1973 got the warp with 27 real ones, and the playhead froze. Empty references in `Marks`/`Areas` mean "did not reach", not "covered entirely" (t288).
- **Cut or gate is decided by `MorpherEvent::cuts_target`**
  (`src/libs/kt-model/src/project.rs`): `replaces_source() || warps_time_for(kind)`.
  The caller does not decide this. The gate is built by `mix_curve_on` PER PIECE: the cut points are shared by all layers (`morph_pass`), and a layer without a cut lands on pieces produced by others; the window counted from the start of the original body would have shifted by the length of what was cut off.
- **A gate step is two keys on ADJACENT ticks** (`mix_curve_on`,
  `src/libs/kt-model/src/project.rs`): on a single tick `dedup_by_key` would collapse them, and instead of a step a ramp across the whole body would come out. A key beyond the edge of the body is not set: `clip_material_at` clamps the edges, and the two keys would merge into one tick with different values. A gate covering the body entirely creates no slot — GOLDEN_VIDEO/GOLDEN_AUDIO rely on this without regeneration.
- **Pushing out of a forbidden zone — inward only** (`snap_out_of`,
  `src/libs/kt-model/src/project.rs`): the start of the window moves forward, the end backward; the side is given, not chosen by proximity. "To the nearest edge" widened the window outward, and for an event with fades on both sides "by time" became "the whole event". If no window is left, the layer did not reach, and that is the honest answer.
- **Material grid first, then the zone** (`MorpherEvent::fit_window`,
  `src/libs/kt-model/src/project.rs`): the step is a frame for video, 15 ticks for audio; for reversed the count runs from the END of the body (there is the mirror anchor), otherwise the piece would yield the clamped frame instead of the real one. The edge of a zone is sometimes off the grid — and the uncut surroundings are then more expensive.
- **Forbidden zones are computed per reel, not per root** (`forbidden_zones`,
  `src/libs/kt-model/src/project.rs`): a layer may lie inside a nested reel; the settings are taken from the project — a nested reel has none of its own.
- **Piece id — `slice_id(origin, idx)` with the high bit set**
  (`src/libs/kt-model/src/project.rs`): no counters, no hashes, no new field in `ClipEvent` (that would have been an edit of 83 literals across 33 files). Piece No. 0 = the original's id, so a clip covered entirely keeps its FX state (invariant 3). Anything that looks up a clip among the effective ones must call `same_clip`, not `==`.
- **The only engine change for the sake of cutting — the ends of transitions**
  (`src/libs/kt-engine/src/video.rs` and `audio.rs`, renderer constructors):
  `same_clip` + checking the boundary against the join; `t.joint(project)` is computed from the raw clips. `gpu_required_clips` collapses pieces back to the original via `slice_origin`, otherwise preflight would name synthetic ids.
- **One rule for a piece's entry point — `clip_slice`** (`src/libs/kt-model/src/project.rs`):
  the layer cut, `Command::Split` and trim all go through it; for a stretched clip the entry point is the integral of speed, and keys travel with the anchor. The guard "there are no assignments to `source_in` outside `project.rs`" — t273 (capability `ui-reach`).
- **`chain_for(Video)` returns the warp from `audio_fx`** (`MorphPatch::chain_for`,
  `src/libs/kt-model/src/project.rs`): the warp lives in the audio chain (its canonical home, where `merge_chains`/`split_chains` in `src/apps/kt-app/src/morpher_editor.rs` operate), but this is a rule for choosing the frame. `auto_for(kind)` follows the same order as `chain_for`, otherwise a curve would end up on someone else's effect; the morpher window carries curves together with the slots.
- **The event's `fx_off` and the layer** (`MorphPatch::apply_to`,
  `src/libs/kt-model/src/project.rs`): when the switch is off, the event's chain goes away entirely (with curves and bypasses), and the layer's effects are appended to a clean one. Previously the layer appended into `c.fx`, and the event's switch turned off someone else's layer (t214).
- **Source replacement shifts the event's `fx_auto` by the same delta as `source_in`** (`MorphPatch::apply_to`, `src/libs/kt-model/src/project.rs`): the layer's curves are not related to this — they are recomputed separately and appended AFTER. The combination "the event's own chain + substitution by a layer" was not checked by any test until t291.
- **`from` of the strength slot — the slot from which THIS layer's contribution begins**
  (`FxInstance::LayerMix { amount, from }`; CPU — `apply_chain_with_mixes` in
  `src/libs/kt-engine/src/video.rs`, GPU — `layer_mix.wgsl` in `src/libs/kt-gpu`):
  the snapshot is taken exactly at the boundary of `from`, otherwise strength would roll back the event's own effects too. Layers stack, each with its own `from`. The blending kernel is the ready integer `dissolve_into`; mismatched sizes (Pan/CamMotion resample) are fitted with the same bilinear as all resamples. Audio: the reverb is one per engine chain, and a flag decides "whose it is" (the event's — on the dry side, the layer's — only on the wet side); the volume is split into "before the layer" and "the layer", otherwise strength 0 would eat the event's level.
- **`MorpherEvent.enabled` ≠ `ClipEvent.fx_off`**: both mute without touching the state of the slots and the curves (`src/libs/kt-model/src/project.rs`), but the morpher's switch turns off the layer, while the event's switch turns off only its chain.
- **`RemoveMedia` asks `MorpherEvent::media_refs`**
  (`src/libs/kt-model/src/project.rs`): the compiler does not catch pattern matching by morpher kind, and the former defect (t243) was held up by exactly this.
- **Frozen targets hold commands**: `RemoveClip` clears the id, `Split` adds the right half (`src/libs/kt-model/src/model.rs`; defect D5 of the old engine).
- **A dead area reference remains** (`MorphTargets`, `ref_windows`,
  `src/libs/kt-model/src/project.rs`): it resolves to nothing, and the morpher window draws it in an error style; returning the area under the same name revives it. `RenameArea` updates references in one step. `unique_name` (`src/libs/kt-model/src/project.rs`) is one for the commands and for paste — if they diverged by suffixes, a paste batch would fail on its own names; the UI calls it before the command.
- **Copy-paste: what travels with the selection — `block_extras`**
  (`src/libs/kt-model/src/ops.rs`) — the single place (it is also used by collapsing into a reel). `ClipBlock` is serialized to the system clipboard: new fields only with `#[serde(default, skip_serializing_if)]`. A batch is atomic, so a morpher without surviving targets is SKIPPED with a count, rather than failing the paste.
- **`timed = false` for files predating the layer schema** (`migrate_legacy_morpher`,
  `src/libs/kt-model/src/project.rs`): a one-way upgrade; old projects render as before; the envelope version is determined by content (capability `project-format`). The change of rule "cut → gate" did NOT raise the version: the schema did not change, and an old build will get roughly the same picture rather than silently wrong.
- **Head of the lane — `ChainEdit.head`, selection — `ChainSel { Head, Fx(usize) }`**
  (`src/apps/kt-app/src/morpher_editor.rs`, `fx_editor.rs`): "head = index 0" guarantees an off-by-one; the effects registry has no head and cannot have one (t48).
- **A free service row — `KtApp::free_service_lane`**
  (`src/apps/kt-app/src/app.rs`): the first row FREE ON THIS INTERVAL, from top to bottom; the function is one for the morpher and the camera. `add_morph_layer` creates the row and raises it in one history step — separately these actions make no sense.
- **Pieces of one clip may have different decode scale steps** — the coverage bars take the union (`merge_runs`, `src/apps/kt-app/src/util.rs`; capability `video-plan`).
- **A layer in reference mode does not travel inside the reel** (`validate_reel`,
  `MorphRefInsideSequence`, `src/libs/kt-model/src/project.rs`): it stays outside and gets into the collapse report; one that did travel inside is removed from the root, otherwise the layer would land twice (capability `compositions`).
- **Diagnostics — `kt-cli probe`** (`src/apps/kt-cli/src/main.rs`): the effective events AFTER the morpher layers and "foreign media in the batch". The first thing to ask about a complaint "here the sound/picture is from another event" — item 298 was found by it in a single run.

## Deferred

- The composite stage "effects on the mixed picture of the tracks below" — a separate object, not the first (`docs/features/morpher/gaps/composite-below-vs-per-clip.md`).
- Exact cutting inside a crossfade (the engine learns the geometry of the original) — only if sticking of the boundary gets in the way on live editing.
- The morph layer bypasses the ban on warp for a reel instance — recorded in R.55, not closed (capability `compositions`).
