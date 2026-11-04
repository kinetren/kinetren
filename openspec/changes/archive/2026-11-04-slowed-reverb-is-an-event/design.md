# Design

## Context

See proposal.md — Why. Each refusal, where it lives, the reason it states, and whether the reason holds
today (observed in the code, read-only):

### R1. The morpher picker shows no audio effects — no stated reason; an oversight
- `morpher_editor.rs` builds ONE chain strip (`src/apps/kt-app/src/morpher_editor.rs:444-501`, R.35 S2,
  commit b505219c "Morpher: a single chain with a violet morph slot") and calls
  `widgets::chain_strip(…, audio = false, …)` (`morpher_editor.rs:485`). Before that commit the morpher had
  two strips and the audio one passed `true`.
- `chain_strip(audio: bool)` (`src/apps/kt-app/src/widgets.rs:1464-1468`) hands it to
  `fx_registry::fx_picker_with_head` (`widgets.rs:1645-1648`, `src/apps/kt-app/src/fx_registry.rs:814`),
  whose list is `filter_entries(…, audio)` = `if audio { e.audio } else { e.video }`
  (`fx_registry.rs:589-593`). So only video kinds (and slowed reverb, which is also a video kind) appear.
- The preset list of the same strip already shows ALL presets "chains of both kinds are legitimate in the
  patch (split_chains will sort them)" (`morpher_editor.rs:468-469`), and `split_chains`
  (`morpher_editor.rs:819-840`) already routes an audio kind into `patch.audio_fx` by `is_audio()`. The
  engine applies a patch's audio chain to audio targets (morpher spec, "The engine sees only effective
  events"). Verdict: **lifted** — nothing but the picker filter refuses.

### R2. Speed refuses shapes and the segment formula — the reason holds for the profile, not the curve
- The refusal: `check_fx_auto` (`src/libs/kt-model/src/project.rs:5176-5184`) rejects a `rate` key with
  any shape but `Linear` (`RateShapeMustBeLinear`); the segment menu prints
  `k::fx::SHAPE_RATE_LINEAR` instead of the shapes for a `rate` curve (`src/apps/kt-app/src/automation.rs:1008`,
  `1024-1026`); "shape of all segments" refuses the same way (`widgets.rs:841-876`). The text
  (`src/apps/kt-app/i18n/en.toml:656`): "Speed takes the linear shape only: the closed-form warp map stands
  on a piecewise-linear profile". Safety net: `AutoProfiles::from_curves_rev` drops the shapes of a
  `rate` curve (`src/libs/kt-model/src/warp.rs:576-584`). Tests: t696 (`src/libs/kt-model/tests/warp.rs:46`),
  t696b. Design history: automation design, "The warp profile is trimmed in `segments`…" ("a new kind of
  segment shape MUST be discarded there and shown as an error") and "Shapes … `rate` stays an exception".
- The stated reason is true: `segments`/`source_progress`/`consumed_abs` (`warp.rs:118-215`, `274…`)
  integrate a piecewise-linear rate in closed form (`seg_ds`). But it constrains only the PROFILE the map
  reads. Evidence that sampling is already accepted: a formula LAYER on `rate` is legal — `resolve_one`
  (`src/libs/kt-model/src/links.rs:404-471`) samples it on the frame grid into `Linear` keys, and t707
  (`links.rs:429-498`) asserts that the resolved speed passes `check_fx_auto`. The automation spec's text of
  that scenario ("at speed the formula is rejected") contradicts its own test and is corrected here.
- A suspected neighbouring defect (to be proved by t913, not yet observed): the formula layer is resolved
  only in `effective_clips` (`project.rs:8629`), while the interface's warp readers (`clip_warp`,
  `warp.rs:961`; `slice_fits_source` etc.) read the RAW clip, and `from_curves_rev` skips a curve with no
  keys (`warp.rs:574`) — so the timeline's material map of an event with a speed formula would ignore the
  formula while the render obeys it.
- Verdict: **lifted** by sampling (D2). The closed form stays exact on what it reads.

### R3. The morpher does not cut a warped event — the reason no longer exists
- The veto: `MorpherEvent::windows_on` (`project.rs:2639-2662`): `no_cut = cuts && (warps(clip's own fx)
  || warps(patch chain))` returns the whole body. Stated reason (the comment, R.37; R.35 finding 2,
  `docs/user-feedback.md:401`): "the `source_in` of a piece computed linearly for a stretched clip points
  to the wrong place". Tests: `t238_warped_clip_is_never_sliced`
  (`src/libs/kt-engine/tests/morph_slice_render.rs:243-268`); morpher spec "A warped clip is not cut";
  video-plan "a warped clip is not cut at all".
- Since R.39, `clip_slice` (`project.rs:3021-3045`) computes a piece's `source_in` as the integral of the
  speed and moves the keys with the anchor (t270, t270b, t270c, t270d in
  `src/libs/kt-model/tests/morph_slice.rs:912-1001`), and the morpher already cuts pieces with it
  (`project.rs:8670`). The reason is gone. Consequences of the veto today: a layer whose patch carries
  slowed slows the WHOLE event regardless of its window; a source swap over part of a slowed event replaces
  its whole body. Verdict: **lifted** (D3).

### R3'. Copy, paste, cut and split of a slowed event — no refusal found in the code
- `Command::Split` validation (`src/libs/kt-model/src/model.rs:1533-1544`) checks only the bounds;
  `copy_clips`/`copy_region_block` (`src/libs/kt-model/src/ops.rs:319-380`) cut with `clip_slice`;
  `copy_selection`/`cut_action` (`src/apps/kt-app/src/app.rs:4568-4602`, `5027-5071`) refuse nothing by
  kind. What remains plausible: (a) the owner was answered with R.35's "a slowed clip cannot be cut in
  principle" (the morpher veto, R3); (b) the region-copy path — with a region set, Ctrl+C copies the region,
  not the selected event (ktprompts item 4, change `selection-copy-target`); (c) a defect only a real gesture
  shows. Verdict: **prove first** — t917 performs every gesture; anything it finds is fixed here; if nothing
  refuses, Open Question 2 asks the owner for the gesture.

## Goals / Non-Goals

**Goals:**
- No refusal of these three kinds remains; every consumer of the warp map (render, prefetch, video frame
  map, UI material map) reads one profile.
- Bytes stay: an all-linear speed curve, an unlayered or non-warped clip, `GOLDEN_AUDIO`/`GOLDEN_VIDEO`.

**Non-Goals:**
- Links on the speed curve (speed driven by another property) — see Open Question 1.
- Ctrl+C copying the selected event instead of the region (change `selection-copy-target`).

## Decisions

### D1. The picker takes a kind filter with "both"
`filter_entries`/`fx_picker_with_head`/`chain_strip` take a `PickKinds { Video, Audio, Both }` instead of
`audio: bool`; the morpher passes `Both`. The gallery shows the video kinds with thumbnails and the audio
kinds as a named list under their own heading (an audio effect has no picture: the existing `preview =
None` branch). The event and track windows keep their single kind. Alternative: two "+ effect" buttons in
the morpher — R.35 S2's owner request was ONE strip; one button keeps it.

### D2. One sampling function for the speed profile
A function in kt-model (`rate_profile_points`, next to `AutoProfiles::from_curves_rev`) turns a `rate`
curve into straight-line points: linear segments pass through untouched (bit for bit); every other shape
and a segment formula is evaluated by `SegShape::at` / the formula evaluator on a fixed grid (proposed:
480 audio frames = 10 ms, fps-independent, so the raw-clip path needs no project); a self-contained formula
layer is evaluated on the same grid by the same evaluator `resolve_one` uses. `from_curves_rev` calls it
for `rate` instead of dropping shapes; `resolve_one` skips `rate` curves without links so the profile is
built in ONE place. `check_fx_auto` drops the `RateShapeMustBeLinear` branch (the error variant stays for the
import table only if a legacy fix needs it). The UI's refusal lines and `shape_rate_linear` go.
2026-10-28, implementation: the grid is 240 frames = 450 ticks (5 ms), not 480 — the 10 ms trapezoid left 4.4 ticks of error over 10 s on `1 + 0.5·sin(6t)`, over the one-audio-frame bound of t913. The sampled profile is long, so nothing per chunk may walk it (binary search in `param_at`, `AudioClip::src_ends`; task 2.4).
Alternatives: (a) a closed-form integral per shape — the step and the zigzag integrate, but the formula
cannot, and the inverse map would need a numeric root per shape; (b) sampling on the project frame grid as
`resolve_one` does — the raw-clip readers (timeline, trim limit) have no fps at hand; a fixed audio grid
removes the dependency and is finer than any frame.
**Revision:** of the automation design rule "a new kind of segment shape MUST be discarded there and shown
as an error" (R.61) — the shapes are now sampled; reason: the owner's claim, and the closed-form reason
applies to the profile, which stays piecewise-linear.

### D3. The morpher cuts a warped target; a patch's warp acts in its window
The `no_cut` veto is removed from `windows_on`. A warped target is cut by `clip_slice` (the integral). A
patch carrying a time warp cuts its target (it already "cuts" by `cuts_target`, R.55) and only the window
piece gets the patch's warp; the outer pieces keep the material mapping they had without the layer (the
layer never moves material outside its window). The piece id rule (piece №0 keeps the original id) and the
forbidden zones apply unchanged. **Revision:** of the morpher rule "A clip with its own or appended time
warp MUST NOT be cut" (R.35 finding 2, R.37) — reason: R.39 removed its premise; t238b is inverted, not
deleted.

### D4. The copy/paste/split proof runs through the real gestures
t917 drives the harness with the keys a person presses (Ctrl+C/V/X, S, a region copy) on a slowed event with
a speed curve and a linked partner, and asserts the observable consequence (model and render bytes), as R.40
requires.

## Risks / Trade-offs

- [Sampling cost: `source_progress` (`warp.rs:192-210`) rebuilds `segments` and walks them linearly on
  every call; a 10-minute shaped speed curve at 10 ms is 60 000 segments] → the sampling happens only for
  non-linear segments; the profile is memoised per curve content; `source_progress`/`consumed_abs` get a
  cumulative table with a binary search if the release-build probe (task 2.4) shows the cost; the measure
  is `kt-cli probe … --time` on a release build.
- [A layer with slowed now leaves a jump in the material at its right edge (the window consumed less or
  more than the time it covers)] → that is what "the layer acts in its window" means; Open Question 3
  offers the alternative.
- [Older builds reject a file with a shaped speed curve] → accepted: "backward compatibility is not needed"
  (ktprompts v26.10.18 header); the reader names the error.

## Migration Plan

Specs: the two requirements whose refusal scenarios are revoked ("The segment shape lives on the key…",
"Target cutting rules") are REMOVED and re-ADDED under extended names with every other rule and scenario
verbatim — OpenSpec keeps scenario names as identities, and a scenario titled "Speed accepts only linear"
cannot carry the opposite contract.

None for files: a file without shaped speed curves reads and writes byte-identically. Projects whose
morphers cover warped events change their render (the veto no longer spreads the patch over the whole
event) — that is the fix itself; `kt-cli probe` shows the new effective events.

## Open Questions

1. Should a LINK (speed driven by another property) be allowed on the speed curve too? The link needs other
   events, so only `effective_clips` can resolve it, and the timeline's material map of the raw event would
   not see it. Recommended: yes, in a follow-up — the UI reads the material map from the resolved snapshot
   the morpher preview already builds; not in this change.
2. If t917 finds no refusal for copy/paste/split: which gesture did the owner use when slowed "did not let"
   him copy-paste and cut? Recommended: ask with the t917 screenshots; most likely it was the morpher veto
   (R3) or the region copy (item 4).
3. A layer carrying slowed over the middle of an event: should the piece after the window continue the
   material where the slowed window stopped (shifting the rest of the event's material), instead of keeping
   its own material (D3)? Recommended: keep D3 — a layer must not change anything outside its window, the
   same rule as for every other patch; a rippling slowdown belongs on the event's own speed curve.
4. DECIDED by the owner (2026-10-28): "yes, invert t696, t696b and t238". The rewritten regression tests —
   t696/t696b (speed refuses shapes) and `t238_warped_clip_is_never_sliced`
   — invert their assertions by this claim; was: confirm once, so the rule "named tests are not
   weakened without the owner's decision" is satisfied explicitly.
