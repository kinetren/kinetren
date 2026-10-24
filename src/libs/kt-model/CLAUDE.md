# kt-model

> The crate's contracts and mandatory tests — `openspec/specs/{automation,morpher,compositions,transitions,project-format}/spec.md`; the reasons for the decisions — `design.md` next to it.

The timeline model + commands + undo. No FFmpeg, no audio hardware. **The PROJECT file is `project_file.rs` (R.70)**: its own `.kt` format, the serialization of the model is a
line-by-line deterministic JSON with a marker and a version at the root
(`to_text`/`from_text`, a refusal of versions ahead, recognition of a foreign file with the name
of the door). There is no disk write here — the single home of the atomic write remains in
kt-otio. The legacy reader of `.ktproj` (`from_json`/`from_json_unvalidated`) is the input of the
import door; the serde-derive of the types that kt-otio carries into `metadata.kt`
(FxInstance, MorpherEvent, UiState, BinGroup…) is frozen by the golden t148 — and
now this same representation lies in the project file, so changing the serde form
of a type means changing the format.

## Transitions (R.24, item 135; kinds — R.33 items 222-223)

- **A kind — `TransitionKind`, built like `FxInstance`**: a tag + parameter fields,
  the specs through THE SAME macro `params_for!`. The registry `TransitionKind::ALL` is
  the only list of kinds (the UI menu and the completeness of the specs in t223 go by it); a new
  kind is added AT THE END and MUST get into both it and the list of cases of the
  GPU conformance t224. `Crossfade` is the default and the former rendering path
  (GOLDEN does not move).
- `TransitionEvent.audio` — the curve of the audio crossfade (`AudioCurve`); the sound
  is crossfaded ALWAYS, stylized audio transitions deliberately do not exist
  (the user's decision of R.33). The equal-power default is not serialized.
- The trait `HasParams` (implemented by `FxInstance`, `TransitionKind` and
  `CameraParams`) — the point to which the UI attaches the unified grid of fields.
  For structs the passport is declared by the macro `params_for_struct!`.
- `TransitionEvent.auto` (R.34) — the curves of the parameters of the kind; the tick of the key position is counted from the START of the transition window. `CameraEvent.auto` — the same for the camera, from
  the start of the event. Both are resolved by `kind_at`/`params_at`, both are validated
  against the specs: a curve on a non-existent parameter fails the loading.
- A crossfade = a `TransitionEvent` at the joint of neighbouring clips (the end of `a` == the start of `b`); **a fade (R.25) — the same object with ONE live end**: `a`/`b` —
  `Option<ClipId>`, `None` means emptiness (the edge of the track or a Gap), the offset toward its side is not limited by the body, `joint()` is resolved by the live end. Both ends
  None is forbidden. The position is NOT stored — the identity by the pair of ClipIds survives
  group moves. The window [joint−in_offset, joint+out_offset), the offsets into the bodies of the neighbours, the sum ≥ MIN_TRANSITION_DURATION (=MIN_CLIP_DURATION=90).
- `reconcile_transitions` at the end of each geometric mutation: a broken joint / a vanished LIVE end → the transition dies (fades are created explicitly, and not by the remnants of the deletion of a neighbour); an empty side of a fade stays empty; a trim → a clamp of the offsets by the bodies; Split
  retargets the outgoing one onto the right half. New geometric commands
  MUST get into `touches_geometry`.
- `check_transition_full` — the single check (validate() and check_on the commands).
- `ForeignEvent` — foreign OTIO constructs: they occupy a place, are deletable, are NOT movable (the raw material is written back untouched from the Retained of kt-otio).
- **Overlapping (R.28, item 162)**: `Project.overlap_layers` (the default — on) makes the overlaps of clips LEGAL; the crossfade is computed from the overlap itself (v1: equal-power for two, 1/N for 3+), therefore the gesture of an overlay
  cuts nothing and `restore_handles` is not needed in this mode. Foreign
  placeholders must never be overlaid. Below — the mode with the option OFF:
- The invariant "clips of one (kind,lane) without overlaps" (stage 6, in force when
  `overlap_layers = false`):
  validate() + a strict check in apply_batch/apply_coalesced
  (`CommandError::InvalidLayout`, an atomic refusal). Commands that previously created overlaps silently (paste into an occupied place, dnd, magnetize) return an Err — the call-site MUST show a note/hint.
- **Handles come back on a break** (R.26, item 159): the offsets of a transition are numerically
  equal to what was cut during an overlay, therefore `ops::restore_handles(before, after)`
  yields the trims that return the material; `TimelineModel::preview` gives the
  state after the gesture in order to count the free space. The clamp (place,
  link group, `source_in`, ForeignEvent) is the DUTY of the function: `apply_batch`
  validates the result as a whole, and one trim that stuck out would roll back the whole gesture.
- The clipboard — `ops::ClipBlock { clips, transitions, morphers, areas,
  markers }`: a transition survives if both ends are in the block and the joint is intact; paste
  remaps the TransitionId. A morpher travels if AT LEAST ONE of its targets is in the block
  (targets outside are discarded on capture); an area and a range marker — by intersection with the
  window of the block, with cropping and rebasing; a point marker — if it is
  inside the window. `paste_block` takes `PasteIds` and `PasteNames`, not a growing
  list of arguments, and returns `PasteSkipped`: the batch is atomic, therefore
  a morpher without surviving targets and a marker on an occupied tick are SKIPPED and
  counted, not "fixed up" (rule 20) — otherwise one bad element would
  bring down the whole insertion, and it would look like "copy-paste does not work".
  The names of the inserted items are built by the same `unique_name`; a reference to a COPIED
  name moves to the copy, to a non-copied one — it stays on the original.

- `timeline_end` (R.25) — the editing length, when it is GREATER than the material: foreign files set it
  by a tail Gap. It is stored only while it is greater than the end of
  the clips (otherwise our own file would grow a field and lose byte stability),
  it is taken into account by `kt_engine::project_range`.

- `ClipEvent.opacity` (R.28, item 12) — the level of the video event 0..1, the default
  1.0 is NOT serialized (the files are byte-stable). The command `SetOpacity`
  does not touch the geometry. For audio the level is `gain_db`.

## Generators (R.52, item 402)

- **The vocabulary of kinds — `genmedia.rs`, and only it**: `GeneratorKind`
  (the registry `ALL`, as for `TransitionKind`, the specs by the same `params_for!`),
  `parse_gen_path`/`format_gen_path` (canon: lower case, defaults
  are not written, the colour as a single token `c=rrggbb`; the parsing is case-insensitive,
  garbage = `None` — the lesson of footnote item 211), `is_pseudo_path` ("synth:" or
  "gen:") — the single predicate for all services. The parameters of a generator are
  a function of the MEDIA (the engine cache is keyed by MediaId): they live in `MediaRef.path`,
  the edit — `Command::SetMediaPath` (the path is opaque, suitable for relink;
  recreating the source is the concern of the call-site, `sync_media_sources`).
- The string values of tokens (the title text, C2) are escaped by
  `escape_gen_value`/`unescape_gen_value` — lossless for any string.
- A new kind: a variant AT THE END of the enum + an arm in `parse/format/apply_token` +
  the specs + a picker line ×5 catalogues + rendering in kt-media + a determinism test.
  The serde representation (internally tagged `kind`) is carried by
  kt-otio as a dictionary `parameters`; fields with `#[serde(default)]`.
- **The rule of a kind (R.53, item 419): a kind = an enum variant WHEN it has ITS OWN numbers.**
  `params_for!` distributes the specs by variant, and not by the value of a field,
  therefore a rectangle, an ellipse and a line are three kinds (the corner rounding exists only in the first one; on the ellipse it would be a dead control — UI rule 9), while the five engineering tables are ONE kind with a
  field `table` (they have no numbers of their own, and five enum variants would be
  five copies of the code). In the registry `all()` there are five rows, however: the menu
  shows each variety, and the name of the kind is a function of the VALUE.
- **A kind without numbers is a legitimate state, but it must be declared.** `t470`
  checks "the specs are empty" against a LIST of such kinds by name: a kind that
  lost its specs through oversight still fails the test.
- **The serde tag MUST match `tag()`.** `rename_all = "snake_case"`
  turns `TestPattern` into the string `test_pattern`, while `tag()` returns
  `table` — the discrepancy is not caught by the compiler and surfaces as a broken
  file. Put `#[serde(rename = "…")]` and check by eye (the lesson of R.52:
  this is how `smpte_bars`/`smpte` and `noise_field`/`noise` came apart).
- **The animation of a title — FOUR NUMBERS** (`reveal`, `reveal_soft`,
  `offset_x`, `offset_y`), not a mode (R.59, stage 2). The five former
  hard-coded modes became PRESETS: `text_anim_curves` is the only
  place where their formulas live, and old files are read by it too. The branches
  remain different in essence: the reveal changes the VISIBILITY of clusters,
  the offset moves the BLOCK; they must not be merged into one "speed" — the reveal has an end,
  the movement has it outside the frame.

## Generator parameters — ON THE EVENT (R.59, stage 2)

The owner's decision: "generator parameters — on the event, as for effects;
the media stays a kind and a cache strip". Until then the text of a title was part of
the ADDRESS OF THE MEDIA, that is, shared by all places of use, and "change the
heading in the second issue" had physically nowhere to be written.

- `ClipEvent.gen: Option<GeneratorKind>` — the kind WITH VALUES;
  `ClipEvent.gen_auto: Vec<ParamCurve>` — their curves, the domain `t` is the same
  as for `fx_auto` (THE MATERIAL TICK). Both are `#[serde(default,
  skip_serializing_if)]` — files without generators are byte-for-byte as before.
- `MediaRef.path` carries ONLY THE KIND (`gen_kind_path`: `gen:text`).
  There is one exception — the "sort" of a variant whose kind name is a function of its
  VALUE (`TestPattern.table`, the rule of item 419): it is part of the address.
- **The invariant is checked by `check_clip_gen`, and it is strict**: the values exist
  if and only if the media is a generator (`GenMissing`/
  `GenOnFile`), the kind matches the path (`GenKindMismatch`), an orphan curve
  is REJECTED (`GenAutoOrphan`). For `fx_auto` an orphan is legitimate — there the slot
  could have changed its kind; here the event has exactly one kind, and the orphan
  has nowhere to come from.
- **The invariant holds BY CONSTRUCTION, not by the discipline of the caller**:
  `AddMedia`/`SetMediaPath` bring a `gen:` path to the canon of the kind,
  `AddClip`/`SetClipMedia` fit the values to the media
  (`fit_gen_to_media`), `Project::normalize` migrates old files
  (`migrate_gen_to_events` — the ONLY place of migration).
- Commands: `SetClipGen`/`SetClipGenAuto` (`touches_render_plan() ==
  false` — the law P: the values travel into the live source as a retune strip) and
  `ApplyGenToAll` ("places: N" as one history step). The values are mirrored
  across the link group on ONE media (`sync_gen_to_link_group`) — for SMPTE with
  a tone, the A/V pair MUST show and sound in one state.
- `SLICE_TAG` is lowered to `1 << 61` (`MAX_REAL_CLIP_ID` = `1 << 45`):
  the address of the generator source — `GEN_TAG | id` of the EVENT, and an event can also be
  a piece of a morpher layer. The `const _` guard in `project.rs` keeps the order
  "clip < piece < generator < reel".

## Invariants
- This is the ONLY description of a clip: kt-engine consumes `ClipEvent` directly. Do not create parallel structures "for the engine" (defect q of the old code).
- All times — `kt_time::Tick`. Not a single field in ms.
- `format_version` is read and checked on loading; loading errors are explicit `Err`, never silent dropping of broken clips (defect p).
- FX parameters — a typed enum `FxInstance`, not raw JSON.
- Semantics: the lane for video = z-order (lower — higher); an overlap of audio in one lane = a crossfade (implemented by kt-engine); link_group links the A/V pairs.
- **THE TRACK NUMBER WITHIN A KIND = ITS RANK among the rows of this kind (R.56, item 449).** `lane_order` encodes ONLY the interleaving of kinds; the order of numbers within a kind is subordinate to it. It is held by the CANON and in each `Sequence`, the guard is `validate` (`LaneOrderNotCanonical`), it is repaired on reading (`normalize` → `canonicalize_lanes`). The single door is `set_lane_order`: it names the rows by their CURRENT numbers and renumbers the contents (clips, morphers, cameras, `foreign`, `areas[].lanes`, the names and heights of the rows). Before R.56 the engine stacked the layers by number, while the screen showed them by `lane_order` — a track could stand under the ceiling and be the bottom layer ("I dropped a video there, and it is not visible"). **There is exactly one exception — the BENCH of the unmerge**: the band deliberately lives at the tail of the numeric space, `simulate` validates only after `unmerge_reel`, and `SetLaneOrder`/`SetLanes` are rejected while a band is open (`CommandError::BandsOpen`).
- Commands are validated BEFORE the undo snapshot (in the old code — a no-op undo record).

## Morpher (H1 port of v1; a LAYER since R.35)
- `Project::effective_clips()` — the SINGLE point of issuing clips to the engine/export/rendering: the morphers are applied here, kt-engine knows nothing about them.
- **`MorphPatch` carries both the source substitution and the effect chains** — one event does both (before R.35 these were the mutually exclusive `MorphKind::Replace`/`EffectBatch`). In the interface this is one fx strip, where the substitution is the purple slot first.
- **`MorphTargets` — the mode of targets**: `Below` (the default, everything below the morpher's row), `Clips` (a frozen list, the former behaviour), `Marks`/`Areas` (named ranges and areas). An empty list is forbidden ONLY for `Clips`: for a layer an empty set is legitimate.
- **The order of layer overlay — BOTTOM UP** (`(Reverse(row), MorphId)`): the lower
  layer is applied first, like an adjustment layer in AE. Within one row —
  the former rule by id. It also follows from this that a permutation of rows changes
  the picture, and `Project.lane_order` MUST go through undo.
- **`fx_off` of an event does NOT switch off the layer**: the master switch removes the chain
  of the EVENT (it moves away entirely together with the curves), while the effects of the layer are appended
  to the clean chain. The switch of the event has no relation to the layer (t214).
- **`BakeMorpher` materializes the SHARPNESS as well**: the invariant "bake == what was drawn" otherwise collapses. Piece No. 0 keeps the id of the original, the others get
  `max+1…`; the ends of transitions and the frozen targets of the OTHER morphers are re-hung on them — all in one undo step (t239).
- **`timed`** (the default true) — the action is limited by the window of the block. Files up to v4 are read with `timed = false` (`migrate_legacy_morpher`) and are rendered exactly as before.
- Commands MUST keep the frozen `targets` up to date: RemoveClip cleans the id, Split adds the right half (defect D5 of v1). RemoveMedia is blocked through `MorpherEvent::media_refs()` — a pattern match by kind no longer works, the compiler does not catch such places.
- There is no upper clamp of source_in by the duration of the source: the model does not know the durations of the media (the engine gives silence/the last frame beyond the end).
- **A target that is covered PARTIALLY is cut** (`effective_clips`): the boundaries of the pieces are
  the internal edges of the windows of all morphers, a set ONE per clip (morphers do not cut
  each other's pieces). The `source_in` rule of a piece is one per project — `clip_slice`,
  and `Command::Split` also goes through it.
- **The id of a piece is a pure function `slice_id(original, number)`**, the high bit
  is reserved (`validate` rejects a real clip above
  `MAX_REAL_CLIP_ID`), the original is recovered from the id itself (`slice_origin`).
  Piece No. 0 keeps the id of the original, therefore a clip covered ENTIRELY is not cut
  and keeps its FX state (invariant 3). Everything that addresses a clip by id among the
  effective ones MUST ask `same_clip`, not `==`.
- **The veto on cutting a warped clip is FIRST in `windows_on` and does not look at the kind** (R.37): the check stood below the reference modes, therefore in
  `Marks`/`Areas` stretched clips were cut, and there was a caveat
  `kind == Audio`, which stopped being true as soon as video learned to stretch.
- **`MorphPatch::chain_for(Video)` also yields the warp from `audio_fx`**: the warp lives
  there (the canonical home, on which `merge_chains`/`split_chains` in the UI stand),
  but it is not a pixel filter, it is a rule of frame selection. Without this the layer
  would slow the sound and would not slow the picture.
- **`windows_on` does not cut where cutting is not allowed**: a patch that does nothing
  to a clip of THIS kind (a layer of video FX over audio), a clip with its own or appended
  time warp (`FxInstance::warps_time` — the linear `source_in` of the piece would point the wrong way), a window shorter than `MIN_CLIP_DURATION`.
- **Forbidden zones** (`forbidden_zones`) — the windows of transitions and the overlaps of neighbours along the track: the boundary of the layer does not stand there, it is pushed outward. Cutting there
  breaks not the target but the SURROUNDINGS (for video the composition order `(Reverse(lane),
  start)` — the piece sticks out above the neighbour; for audio `overlap_weight` computes the ramp
  from the geometry of the clips).
- The boundary sits on the grid of the MATERIAL (a frame for video, 8 samples for audio; for a
  reversed one the count runs from the END of the body — there is the anchor), but the zone is more important than the grid: first the grid, then the pushing outward.
- `link_group` is multiplied across the pieces and this is harmless: the groups are read only
  by the model's commands via `project.clips`, and there are no pieces there.
- `fx_auto` is not touched by cutting at all — **while the clip is not stretched** (specified in
  R.39). The promise of R.33 held because with a linear map `(k.t −
  source_in)` IS the time of the keyframe event: the anchor and the axis coincide. Under a warp
  the anchor moves along the integral of the speed, while the axis MUST remain linear (R.37),
  and the keys shift by THE SAME delta — otherwise the whole curve drifts away after the anchor.
  On a clip without a warp the delta is exactly zero and the block is not executed, so for
  all former projects the promise holds word for word.
- **The rule of a piece is ONE, and it is the real one**: `clip_slice`/`slice_source_in`/
  `slice_fits_source` in `project.rs`. Before R.39 the comment promised this, while the
  linear formula had been written out by hand in six more places (`ops.rs`
  punch-out and crop, both trim commands, their mirrors `raw_trim_*`, the prefilters
  and validation) — therefore the repair of `Split` did not repair the neighbours. The guard — the
  scanning test 273 in `kt-app/src/reach_tests.rs`.

## Signed speed: two axes (R.57, item 453)

`rate` lives in −32…32, zero is allowed (the owner's decision, the semantics
of the velocity envelope of Vegas). It follows that there are TWO AXES, and they must not be confused:

| | what it is | for whom |
|---|---|---|
| `source_progress` = P(τ) = ∫rate | SIGNED position in the material (the address) | video (`clip_material_pos`), the address map of audio, prefetch windows |
| `consumed_abs` = A(τ) = ∫\|rate\| | accumulated CONSUMPTION, monotonic | the resampler cursor (`source_offset`), `output_time_for_consumption`, trims |

* **Only A is inverted.** P is non-monotonic: a point of the material may have
  several moments of output. `output_time_for_source` is kept, but it is
  legitimate only on monotonic profiles (in production — only `s < 0` in trims).
* **DO NOT TOUCH the body of P.** `seg_ds` is sign-independent, the sign appeared by itself from the
  symmetric clamp. `consumed_abs` repeats the same loop, and the split
  at the zero of speed is switched on STRICTLY at `r0*r1 < 0` — on positive
  curves the branch is not executed, and the map is bit-for-bit the former one (the guard t603d with a frozen oracle INSIDE the test; GOLDEN_* stand on this).
* **The axis of the "Reverse" mirror is the SPAN** (`ClipWarp::span()`), not `used()`.
  `used()` now honestly means "consumed" (A(1)): on a signed curve it is
  more than what was traversed. Everything positional (warmth, filmstrip, windows)
  is measured by the span.
* **The extrema of the span are at the zeros of speed.** `span_over` on a monotonic
  curve MUST return EXACTLY the ends: the identity of the windows stands on this.
* **The reach of the modulus (`rate_reach`) is computed over the SPLIT segments** —
  a curve with a change of sign passes through zero inside a segment, and at the ends
  the minimum would have come out overstated.
* **The floor `RATE_AUDIO_FLOOR = 0.01` is a property of AUDIO**, not of the position map:
  below it the sound holds (silence without consumption), while the picture honestly stands
  as a freeze frame. One constant for the map and for the processor.

## Warp speed — shared by a link pair (R.37 item 2)

- `sync_warp_to_link_group` is called from the mutations `SetFx`/`SetFxAuto`/
  `SetFxBypass` inside `mutate_on`. The rule lives IN THE MODEL, not in the FX window,
  because `SetFx` also arrives from inserting a chain from the buffer, from the bake
  of a morpher, from the format reader and from kt-cli. "The A/V pair consumes the source at
  one speed" is an invariant of editing, not a convenience of the interface. The precedent
  is `reconcile_transitions` after each geometric mutation.
- Inside `mutate_on` the mirroring falls into THE SAME undo step by construction, including under `apply_coalesced`.
- **The slot is looked up BY KIND (`warps_time`), not by index**: an audio chain
  may be `[Gain, Slowed]`, a video one `[Blur, Slowed, Mirror]`.
- Synchronous: the PRESENCE of the slot, `rate`, and the curve `"rate"`. Each has its own:
  the reverb parameters, the frame mode, the bypass of the slot. The checkbox of the slot on
  the video instance — this is exactly "switch off the slowing of the picture"; a second switch of the same
  meaning is not created (item 9 of the kt-app checklist).
- `UnlinkClip` ends the synchronization by itself; `LinkClips` does NOT align retroactively — to link two clips with different speeds and silently rewrite one would be a loss of work.

## Named areas (R.35 item 2)
- `MarkedArea { name, start, duration, lanes, color }` in `Project.areas`.
  `lanes` are SEMANTIC keys of tracks, not row numbers: a permutation of
  rows does not move the area. Empty = all tracks.
- **The name is the ADDRESS**: non-empty, unique, and the namespace is ONE with the
  range markers (`taken_names`) — "area finale" and "marker finale" in one
  picker of the morpher would be a trap. Both `validate` and
  `check_on` (`AddArea`/`RenameArea`/`SetMarkerName`) check it.
- `unique_name(taken, wanted, stem)` — ONE function for the check of commands and
  for the insertion: if they diverged in their suffixes, an insertion batch would fail
  on its own names. The UI calls it BEFORE sending the command, so the user
  sees the ready name rather than receiving a refusal.
- Commands address the area by NAME (indices diverge under undo/redo).
  `RenameArea` also corrects the references of morphers — in one history step.
- **A dead reference stays in the model and resolves to nothing.** If you delete
  an area, the morpher does not forget it: you bring it back under the same name, and everything comes alive
  (rule 20 "nothing is cut off by itself"). The UI draws such a reference in an
  error style.
- **The VETO "do not cut" ≠ "reached" (R.43, item 298).** `windows_on` first answers whether the layer reaches the clip BY TIME, and only then decides
  whether to cut it or patch it entirely. The veto of the warp stood FIRST and returned
  the whole body, never asking about time: a morph block 0.6 s long at the 5020th
  second patched EVERY event of the project that has a slowed&reverb. In the user's project this was 1973 events out of 1973 with 27 real ones — a substituted source for the sound 5000 seconds away from the block, a phantom warp over
  the whole edit and a frozen playhead. The same order is mandatory in
  `Marks`/`Areas` too: an empty set of references means "did not reach", not "covered
  entirely". Test 288.
- **The substitution of the source moves the keys of the event TOGETHER with the anchor (R.43,
  item 300).** A key is glued to the material (t212) — as long as the material is THE SAME.
  The substitution of the source by a layer changes the coordinate system, and the curve left in the
  old one drifts out of the window of the event entirely: `curve_value` clamps it
  to the last key, and the effect looks like "it has stopped applying" (this is how pan3d disappeared for the user). The rule is the same as for `clip_slice`
  (item 270): the anchor and the keys move by one delta. The curves of the LAYER ITSELF have
  no relation to this — they are recomputed separately (item 266) and
  appended afterwards. Test 291.
- For `Marks`/`Areas` the TIME is set by the reference itself, not by the block of the morpher
  (`ref_windows` + `merge_ranges`): the "by time" checkbox in these modes
  goes dark. Otherwise the user would refer to a distant range and not understand
  why nothing happened.

## Cameras (R.29, 3D compositing)
- `CameraEvent` on camera tracks (`LaneKind::Camera`, `camera_lanes`) —
  a precedent of the morph track: the event is itself both a camera and its "switching-on by time". The active camera is `active_camera_at`/`active_camera_in`:
  the covering event, a smaller lane → a larger start (Vegas) → a larger id;
  the rule is deterministic, the rendering depends on it (test 188).
- `FxInstance::class()` — Audio / Video2d / Video3d (a method, NOT a serde field);
  Video3d is a CATEGORY of the picker, not a sign of the backend: `requires_gpu()`
  was removed in R.44 (Pan3d is computed on the CPU too — the inverse homography of the merged
  geo pass, kt-fx::geom, item 308). Pan3d is the only 3D effect
  of stage 1; its units (project px, ZXY degrees, the anchor in fractions)
  are frozen by the format.
- The camera keyframes — by the progress of the event (`params_at`, linear + smoothstep
  under ease, as in CamMotion). Pan3d has NO keyframes of its own, deliberately:
  the automation of any parameter is a common mechanism (see below).

## Automation of FX parameters (R.30, the domain rewritten in R.33)
- `ClipEvent.fx_auto: Vec<Vec<ParamCurve>>` — the curves by slots, parallel to
  fx; `active_fx_auto()` aligns them with `active_fx()` (a bypass slot
  drops out together with its curves).
- **The domain of `AutoKey.t` is the MATERIAL TICK** (the coordinate system of `source_in`),
  not a fraction of the event length. The attachment to the material is ensured by the
  REPRESENTATION: `TrimStart/TrimEnd/Split/SetReversed/MoveClip` do not touch the curves at all. The rule "timeline tick → material" is one for the whole project —
  `clip_material_at` (the engine still aligns the result to the frame grid).
  A key OUTSIDE the piece of a clip is legitimate: it is a key hidden by trimming.
- Addressing by field name: `param_specs()/set_param/get_param` — all three are generated by the macro `fx_params!` from ONE declaration; do not write them by hand.
  `ParamSpec` carries min/max/default/step/unit/kind; a curve can be hung
  only on `ParamKind::Smooth` (`auto_params()`); structural parameters (the length of the reverb lines) rebuild the DSP.
- Audio is automated by the same mechanism; `SlowedReverb.automation` is a
  read-only legacy input, which `migrate_legacy_slowed` converts when files are read.
  Slowed has no domain of its own any more.
- `SetFxAuto` is a full replacement; `SetFx` trims the curves to the new length of the
  chain (the reordering of slots the editor does itself and sends both commands
  under one gesture token — one undo step). Tests 190, 212-215.
- `ClipEvent.fx_off` — the master switch of the chain, `MorpherEvent.enabled`
  — the switch of the morpher: they switch off the active chain/effect, WITHOUT touching the
  state of the slots and the curves.
- The FX clipboard — `ops::{FxSlotBlock, FxChainBlock, EnvelopeBlock}`:
  the insertion re-hangs the keys onto the entry point of the target and returns the commands
  (one undo step); an envelope is inserted only into its own kind of effect.

## Tests
- Test 12: round-trip of commands, undo, save→load→save is byte-stable.
- Test 35: morpher (flatten, bake==flatten+undo, order by id, synchronization of targets, tracks, round-trip).

## Transitions of R.51 (items 397-401)

- `TransitionEvent.invert` — the inversion "the gesture backwards" (kernel(b,a,1−k));
  `TransitionKind::symmetric()` — which ones are identities (the checkbox is hidden).
  The particular `Clock.reverse`/`Luma.invert` — other axes, they live alongside.
- **An overlapping transition**: a "staircase" pair `a.start < b.start < a.end <
  b.end` of one (kind,lane); the window is the overlap zone ITSELF
  (`window()`/`overlap_pair()`), the mode is not stored, `in/out_offset` are
  dead weight (they come back to life when returned butt-joined). A third clip in the window —
  `TransitionOverlapBusy`. `reconcile_reel` canonicalizes the roles (a is always
  the earlier one; Split/moves could have flipped the pair) and decides the fate: butt-joined ↔
  overlap — the transition SURVIVES with all its settings, a break — it dies.
  `restore_handles` skips the pairs overlapped in `before` (nothing was cut).
  In the file — `overlap_transitions` of the envelope of ITS OWN reel
  (a pair from different Tracks as a child of a sequential track is not expressible),
  the id is assigned on reading. Inversion/overlap/new kinds bump v7.
- There are 33 kinds (t223; the anchor "16" was removed by the owner's decision, R.51); new ones go AT THE END of ALL and into the cases of t224 (both axes, both flags).

## Morpher layer: CUT or SUPPRESS (R.55, item 437)

A layer has two ways of acting "by time", and the choice between them is made by
`MorpherEvent::cuts_target`, not by the caller:

* **SUPPRESS (the default).** A patch of effects alone lies over the WHOLE BODY
  of the target, and the time is set by the strength curve (`LayerMix` + `mix_curve_on`): outside the window zero, inside — the strength with fades. No cuts appear, the `ClipId` of the original
  stays intact, the transitions and the neighbours are untouched, the boundary is exact to the tick.
* **CUT.** Only where the contribution cannot be switched off by a strength slot: a substitution of the source (`replaces_source`) and a time warp in the patch
  (`warps_time_for`) — it changes not the value but how much material is consumed.
  The material grid, the forbidden zones and the `no_cut` veto remain there too.

**Pushing a boundary out of a forbidden zone — ONLY INWARD.** `snap_out_of`
takes a direction: the start of the window forward, the end backward. The former "to the nearest edge" extended the window outward, and for an event with a fade on the left and
right "by time" turned into "over the whole event" — the owner's claim of
R.55. The window now only shrinks; if no window is left — the layer did not reach here.

Three places where the error is not visible to the eye:

1. **The gates are counted PER PIECE.** The cut points are common to all layers, and a layer without cutting lies over pieces produced by OTHERS. A window from the start of the body would have drifted by the length of what was cut off.
2. **A step — two keys on NEIGHBOURING ticks.** On one tick
   `dedup_by_key` would collapse them, and instead of a step a ramp over the whole body would come out.
3. **Beyond the edge of the body no key is placed.** `clip_material_at` clamps the edges —
   two keys would collapse into one tick with different values.

A gate covering the body entirely does NOT create a slot: at the defaults the path is
byte-for-byte, and GOLDEN_VIDEO and GOLDEN_AUDIO stand on this.

## What travels with a selection — ONE RULE (R.35 item 4, R.55 item 438)

`copy_clips` → `block_extras` is the single place where it is decided what travels together with the selected events: transitions, morpher layers, areas,
markers and (since R.55) **cameras**. A second rule must not be created: if
they diverged, copy-paste and collapsing would take different things, and there would be
no way to explain it. Therefore a camera added here for the sake of collapsing automatically
travelled on copy-paste as well — and that is correct.

`ClipBlock` is also the CLIPBOARD (it is serialized), therefore new fields
are added only with `#[serde(default, skip_serializing_if = …)]`.

**What moved inside is removed from the ROOT.** Otherwise a layer would live in two places:
inside — on each piece, outside — on the assembled picture, and the same thing
would be overlaid twice.

**A layer in reference mode (`Marks`/`Areas`) does NOT travel inside.**
`validate_reel` forbids such ones inside a reel (`MorphRefInsideSequence`), and
the batch is atomic — drag it inside and collapsing the selection under it would become
impossible altogether. It stays outside and appears in the report.

**The clipboard — in the coordinates of the OWNER REEL (R.56, item 450).** One copies from
the WORKING view, and in it the unmerge band is shifted by `base[kind]`, and the shifts for
the kinds differ: if the clipboard saved these numbers, the insertion would separate the pair of video
and audio to different ends of the edit. The translation — `ClipBlock::localize`/
`offset_lanes`, the alignment to the clicked row — `shifted_to_row` (all kinds by ONE
delta, a clamp by kind); the needed tracks are created EXPLICITLY (`lanes_needed`),
and not by silent growth in `AddClip`.

**`lane_order` travels together with the contents.** A reel without it has the default
order of rows, and in it the morph rows are the LAST ones; a layer in the mode "everything below"
then covers nothing — and says nothing about it.

## Unmerging a reel IN PLACE — exchange with an offset (R.55, item 441)

A second technique of the same kind as `swap_reel`: the editing context is made
by REARRANGING THE CONTENTS, and not by a parameter passed to sixty call sites. Only with an
offset in time and in track number:

```text
t_root = t_reel + dt,   dt = inst.start − inst.source_in
track  = own + base[kind]
```

**The band occupies the TAIL of the numeric space of tracks.** The visible place under the
instance is set ONLY through `lane_order`. Then the root numbers are all
`< base`, the numbers of the band are all `>= base`, and the insertion/deletion of a root track
does not touch the band by construction.

**`merge_reel` is applied to the DERIVED view, not to the canon.** Into the document,
into the file and into the export goes a project WITHOUT bands — otherwise an edit would go to disk
in which the contents of the reel lie on the tracks of the root.

**`unmerge_reel` is a PROOF, not a parse.** Its error means
that the gesture carried an event across the boundary of reels, and the batch MUST be rejected
as a whole. This is exactly why the addressing of edits is not needed: there is physically nothing to carry away.
An instance inside its own reel — `BandEscape`; a root row in the middle of the unmerge block — `BandOrderBroken`.

**The identity `merge` + `unmerge` is BYTE-FOR-BYTE (t566).** A red test
means that the technique is wrong, not that the test is fussy. Hence also
`canon_order`: the natural order of rows has a canonical form (an empty
vector), and it is that which must be returned — otherwise "unmerged and collapsed" would write to the FILE
an explicit order where there was none.

**`dt` is invariant under trimming** (`clip_slice` moves `start` and `source_in`
by one delta) — therefore the edge of the instance is stretched right in the band, and
the contents do not move across the screen.

### Working view and history during unmerge (R.55, item 442)

`TimelineModel` holds TWO projects: `project` — the CANON (without bands, goes into the
file/player/export via `document()`), `bench` — the WORKING VIEW (the canon plus the
unmerged reels, which `project()` returns). With an empty `bands` the bench is a
byte-for-byte copy of the canon.

Rules that are easy to break:

* **`simulate` works on the BENCH and assembles the result into the CANON.** An assembly
  error = zero changes and zero entries in the history.
* **`preview` returns the BENCH of the future.** Its result is compared with
  `project()` (the return of the "handles", R.26) — comparing the canon with the bench would
  silently cut material from other people's events, and with undo.
* **Any edit of the canon MUST call `rebuild_bench()`** — including fields
  outside the history (`region`, `loop_enabled`, `ui`: they are mirrored into both).
* **The bands travel together with the context in undo/redo** (t454-analogue).
* **`Band::after_batch` — an exhaustive `match` without `_ =>`**: a new
  command will not compile until its row has been filled in. The only place where
  correctness rests on a human.

## Public parameters of a composition (R.59, stage 3)

The owner's decision: "an instance overrides ONLY what is published, by a value or by a curve". Two fields and one door.

- `Sequence.published: Vec<PublishedParam>` — the handles declared by the author:
  `id` (the address, survives renaming), `name` (FREE TEXT of the author — the content of the document, it will never get into the translation catalogue), `target: ParamAddr`, `ty`, `range` (narrowing of the limits).
- `ClipEvent.overrides: Vec<Override>` — the values of these and only these
  handles for ONE instance. Both fields have `skip_serializing_if` — the clipboard
  and old files are byte-for-byte as before.
- `ParamAddr { clip, owner, param }`; `ParamOwner::{FxSlot, Generator,
  ClipAttr, Transition, Camera, Nested}`. For `Transition`/`Camera` the field
  `clip` is not used — their numbers carry the address.
- **The field `published` exists both on `Project` and on `Sequence`**, and `swap_reel`
  swaps them — for the same reason as `morphers`/`cameras`/
  `lane_order`: entering a reel is done by EXCHANGING the contents, and the command
  edits the reel in which you stand. The real root has an empty set:
  the root is nobody's instance.

**The value of the template is NOT STORED** — `Project::addr_value` reads it from the
target live. A stored copy would silently diverge from the reel at the first
edit inside. For the same reason the list of variants of `Choice` does not go
into the file (`ParamTypeRepr` carries only the family).

**`resolve_reel(seq, overrides)` is the single door**, `effective_reel`
is its wrapper. An empty set goes by the FORMER path: `Cow::Borrowed` for transitions
and cameras, not a single clone of the reel (t673/t676). The overrides are applied
**BEFORE** the morpher layers: the author's address is the structure of the reel as he sees it,
and the morpher appends slots at the end and, under `fx_off`, cleans the chain entirely.

**The curve — in the material of the INSTANCE (which is also the reel time)**, it is converted into the material of the target by the same `clip_material_at` as the curves of the morpher layer. There are exactly three carriers (`owner_takes_curves`): `FxSlot`, `Generator`, `Nested`. For
a transition the domain is the tick from the start of the window, for a camera — the progress of the event; no conversion
exists there, and an "almost right" curve is worse than a refusal.

**The fate of the target — `reconcile_published`**, next to `reconcile_transitions` in
`simulate`, on every batch and WITHOUT a list of triggers (a list is the place where a row gets forgotten). The address is glued to the `ClipId`: it survives a trim, a move and
the LEFT half of a cut. If the target has vanished or changed its TYPE — the publication goes away,
and all its overrides in all instances, in one history step.

**`expand_instance` BAKES IN the overrides** and reports how many (`ExpandReport.overrides_baked`): the unmerge band shows the TEMPLATE (it is a reference), while the expansion is irreversible — the reel under the pieces no longer exists.
`inline_pass` also resolves the instance's set: it is the reference oracle of the law E and
what `kt-cli probe` prints.

**`overrides_touch_sources`** answers one question for the engine: can
the set reach the STATE OF A SOURCE (`Generator`, `Nested`,
`ClipAttr media`). The salt of the generator address depends on the answer — see
kt-engine/CLAUDE.md.

## Typed property (R.59)

`ParamValue::{Num, Text, Color([u8;4]), Flag, Choice(String), Media}` —
the value of a property; `HasParams::get_value/set_value/value_type` — access.
**The colour is DERIVED, not declared**: `derive_color_groups` assembles it
from the rows `<stem>_{r,g,b,a}` with the unit `Rgb8` (all 69 components are already
named this way), therefore the format does not move, and hand-written lists of colours in the interface
are a second source of the same fact. `extra_specs()` declares what is not
among the numeric rows and flags: for a title these are `text`/`family` (text) and
`align`/`anchor` (choice). `Choice` is stored by the KEY of the variant: the number would
shift if a variant were inserted in the middle.

## Shape of a segment, link and formula (R.61)

- **The shape lives on the KEY** (`AutoKey::shape`) and describes the section AFTER it;
  `ParamCurve::ease` is removed. Each kind of shape has ITS OWN parameters (tension,
  periods/amplitude/profile of the zigzag), not one common number: the owner's decision
  "the zigzag is the shape of a segment" cannot be expressed without this.
- **The default shape computes EXACTLY the former `smoothstep01`.** At tension 1 and 0
  the formula is not blended, but taken exactly: `s + (w − s)·1` is arithmetically equal to
  `w`, but in floating point it is not, and the last bit moved `GOLDEN_*`. The guard —
  t695b against a FROZEN oracle.
- **The window of the zigzag is the parabola `4s(1−s)`, not a sine:** `sin(π)` in floating point
  equals 1.2e−16, and the right end did not converge exactly with the key.
- **A new key INHERITS the shape of the segment it divides** (`insert_key`), and a new
  curve takes the project's default (`ParamCurve::constant`). Two places, one per
  rule; keys are not to be placed past them. The starting curve of the warp speed
  is always straight, and `check_fx_auto` rejects any other shape LOUDLY.
- **In the FILE the shape is written by content** (its own `Serialize` of `ParamCurve`):
  a uniform expressible shape travels in the former `ease` field and does not move the bytes, while the rest — on keys and v10. Without this the bytes of every file with
  curves would have shifted (t433b, t189b).
- **A link does not cross the boundary of a reel** (`links::check_links`): outward, the reel would stop
  being self-sufficient and the law E would break; inward — there are public parameters for that. Between DIFFERENT events the denominator is the tick of the TIMELINE; inside one event no conversion is needed.
- **Derived sources are resolved in `morph_pass`** — in the single door to
  the engine, once at assembly and in topological order. A reel without them
  is returned BORROWED: no graph is built at all.
- **The cycle is sought by a traversal from ALL vertices:** a pair of properties that lead each other
  and are read by no one is still a cycle.

## Project file — its own format (R.70)

- **The `serde` form of `Project` is the file format itself.** A field with `skip_serializing_if`
  is not written at the default; a new field without `#[serde(default)]` will break the reading of old
  files. The version (`PROJECT_FILE_VERSION`) is raised only for a breaking change;
  versions below are read by a migration in `from_text`, versions above — a refusal (`TooNew`), there is no partial
  reading.
- **Line-by-line and deterministic** — a measurable rule (t822): an edit of one event
  changes the units of the lines, and one model gives the same bytes. Compact JSON, binary and compression
  are forbidden by the owner ("indexable in git").
- **Passthrough (`otio`) is written AS IS** — its order belongs to the exchange file; the determinism of the `HashMap` fields is the responsibility of whoever builds this JSON (`Retained::to_json`).
- Tests: t811 (the round trip), t810 (an instance — a reference, not a body), t812 (the version and the doors), t822 (git).
