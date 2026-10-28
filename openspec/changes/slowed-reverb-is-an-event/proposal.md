# Proposal

## Why

ktprompts v26.10.18 items 1–3 (`docs/user-feedback.md`, round of 2026-10-27/28): "1. The morpher does not
let me put audio effects on. 2. Slowed does not let me put generated curves on — childish excuses.
3. Slowed does not let me copy-paste the event and cut it — childish excuses." The owner rejects the
reasons he was given. Each refusal was found in the code and its stated reason checked against today's
engine:

1. The morpher's single chain strip (R.35 S2) passes "video" to the effect picker, so audio kinds are
   not offered at all — no reason was ever written; it is an oversight of the merge of two strips.
2. The speed curve refuses every segment shape but linear, the segment formula included ("the closed
   form of the inverse warp map stands on a piecewise-linear profile", R.61). The reason holds only for
   the PROFILE the map reads; a shaped curve can be sampled into a straight-line profile, exactly as the
   resolver already does for a formula layer on speed (t707 accepts it).
3. The morpher refuses to cut an event with a time warp ("a linearly computed `source_in` of a piece
   of a stretched clip points to the wrong place", R.35/R.37). Since R.39 every piece's entry point is
   the integral of the speed (`clip_slice`, t270–t270d), so the reason no longer exists. For the plain
   copy-paste, cut and split of a slowed event no refusal exists in the model — what the owner hit is
   reproduced first.

## What Changes

- The morpher's effect picker offers both kinds (video and audio), like its preset list already does;
  a picked audio effect lands in the patch's audio chain and sounds on the audio targets.
- **BREAKING (contract):** the speed curve (`rate`) accepts every segment shape, the segment formula and
  a formula layer. The warp map reads a straight-line profile sampled from the shaped curve by ONE
  function on a fixed grid, the same for the render, the prefetch, the video frame map and the UI's
  material map. The refusal text and the "Speed takes the linear shape only" line disappear.
- **BREAKING (contract):** the morpher cuts an event with its own time warp at the window like any other
  target (entry points by the integral of speed); a patch carrying a time warp slows only its window,
  not the whole event.
- The copy, paste, cut (Ctrl+X) and split (S) of an event carrying slowed reverb and a speed curve are
  proved by a reach test; whatever refuses there is fixed in this change.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `automation`: the segment-shape requirement — speed takes every shape, sampled into a straight-line
  profile; the formula requirement's speed scenario is corrected to what t707 asserts.
- `morpher`: the target cutting rules — a warped clip is cut at the integral; a patch's warp acts only in
  its window.
- `video-plan`: the morpher-layer scenario "a warped clip is not cut at all" becomes "a warped clip is cut
  and the cut is neither seen nor heard".
- `ui-timeline`: a new requirement — no kind of effect or event is refused in the editing windows
  without a technical reason: the morpher picker shows audio kinds; a slowed event is copied, pasted, cut
  and split like any event.

## Impact

- Code: `src/apps/kt-app/src/morpher_editor.rs`, `src/apps/kt-app/src/widgets.rs` (`chain_strip`),
  `src/apps/kt-app/src/fx_registry.rs` (`filter_entries`, `fx_picker_with_head`: a kind filter with
  "both"); `src/libs/kt-model/src/project.rs` (`check_fx_auto`, `MorpherEvent::windows_on`),
  `src/libs/kt-model/src/warp.rs` (`AutoProfiles::from_curves_rev` samples rate shapes),
  `src/libs/kt-model/src/links.rs` (one sampling function for speed); `src/apps/kt-app/src/automation.rs`
  and `widgets.rs` (`shape_all_control`) drop the speed refusal; the five catalogs drop
  `shape_rate_linear`.
- Tests: t912–t917. Rewritten by the owner's decision (the claim): t696, t696b (speed refuses shapes) and
  `t238_warped_clip_is_never_sliced` (the morpher veto) — their old assertions are inverted, not deleted.
- Format: a `rate` key may carry a shape; an older build rejects such a file with
  `RateShapeMustBeLinear` (the owner: "backward compatibility is not needed", ktprompts header).
- No new dependency; invariants 4, 7 and 11 hold (one map function for every consumer; determinism).
