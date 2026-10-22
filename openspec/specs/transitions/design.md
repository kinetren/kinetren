# Transitions — why it is this way

## What hurt and for whom

- At any cut there was only one transition — a dissolve, on both audio and video. The owner
  asked for "different 10-20 modern ones" and for "unify the control like an fx-chain".
  Later: "there are no transitions with the symbiosis of two events, there are no transition
  inversions": the impression was created by the broken push/whip (a smeared edge of frame B
  slid in instead of the frame) and by the absence of a common word "inversion" while three
  kinds each had their own flag.
- Push/whip lived broken for four rounds, because both guards were blind each in its own way:
  the render test painted frames with solid colours (any coordinate gives the same colour),
  conformance compared CPU against GPU, and the mirrors were broken in the same way. The first
  version of conformance also took kinds with defaults (`Dir::Left` — vector (−1, 0)), on which
  an error in the Y term of the formula is invisible.
- Re-dragging the corner fade and re-dragging a Move overwrite assembled a fresh crossfade and
  silently erased the chosen kind and curves — against the owner's rule "nothing is switched off
  by itself".
- Overlapping clips on one track was legal and sounded (equal-power), but the 33 kinds lived
  only at the joint: in the overlap zone the picture was cut by a hard cut. A transition inside
  a nested reel silently did not crossfade the sound — audio took the transitions of the root.
- Pulling a crossfade apart permanently lost the trimmed material ("in Vegas, what overlapped
  remains").

## Decisions and why

- **Kinds are like effects.** `TransitionKind` is built like `FxInstance`: a tag plus fields,
  specs through the same passport macro, the registry `ALL` is the only list. The UI of fields,
  automation and completeness of specs come for free; a new kind without a row in the registry
  is visible at once. 33 kinds — a "modern mix" (Vegas classics plus fashionable ones from
  Premiere/Resolve/CapCut), the owner's decision; a strict copy of the Vegas set was proposed and
  not chosen.
- **Sound always crossfades, with a choice of curve.** There are no stylised audio transitions;
  linking the picture kind to the sound was proposed and not chosen. Equal-power is the previous
  formula byte for byte, so GOLDEN_AUDIO did not move.
- **One translation into numbers, two executors.** A kind → `TransMix` (mode + 8 numbers) exactly
  once; the CPU kernel and the WGSL mirror read the same numbers (invariant 7). This is the only
  place where the formula had to be written twice, hence the rule "both files in one commit" and
  conformance t224.
- **The crossfade does not enter the kernels.** The integer `dissolve_into` is the byte reference
  GOLDEN_VIDEO; any change to the kernels does not touch it, and inversion on it is the identity.
- **Closed form against mirror blindness.** For directional kernels the expectation is computed by an
  independent formula on ramp frames: bilinear sampling on a linear ramp at an integer shift
  degenerates into an exact value. This rule was born from the push/whip defect and caught the
  reversed axis of peel/roll (t468), which conformance also did not see.
- **Inversion is a flag on the event, executed by the plan.** The owner chose "swap the roles of
  the sides"; executing `kernel(b, a, 1−k)` before the kernel gives inversion to all kinds without
  editing the kernels, WGSL and keys (the swap changes the order of sources, k is already in the
  key). Symmetric kinds hide the flag (a control that does nothing is not shown).
- **Volumetric ones — without a new pass.** The open transitions industry (GL Transitions) does
  page peel and cube as an analytic inverse mapping in the same fragment pass; no depth buffer is
  needed. Homographies are computed in f64 by one function and rounded once — both backends read
  the same bits, parity by construction.
- **A solo fade is the same transition with an empty side.** The semantics of the OTIO reference
  (neighbour Gap): an edge transition is a dissolve with emptiness. The kernel receives an "empty"
  second side and does not read it: a wipe from nothing, without a second frame and without a
  transparent texture.
- **A transition on an overlap: the window = the overlap zone, the mode is not stored.** The owner's
  decision ("symbiosis"). The owner of the window is the geometry of the pair; the window has no
  handles, it moves with the trim. This is how the lesson of Kdenlive (F-TL-010) was taken: there the
  transition window got a second subsystem of handles and three years of undo defects. A third clip
  in the window is an error: the 1/N zones remain without transitions.
- **Returning the handles without new state.** The transition offsets are numerically equal to the
  trimmed material (this is how both the overwrite and the legacy import count), so trims to the offsets
  return exactly what was lost; it is invoked from all the gap gestures by the same undo step.
- **Format by content.** A non-crossfade / non-default curve raises the file to v3, an inversion and an
  overlay transition raise it to v7: an old build would silently play a different gesture and would keep
  the key by passthrough — refusal is more honest (the owner: "backward compatibility is not needed").

## Rejected and why

- **A transition progress curve (like the Transition Progress envelope in Vegas) and ease presets
  as a way to invert** — offered to the owner explicitly; the "swap the sides" flag was chosen.
- **A gallery of kinds with live mini-previews** — not chosen by the owner.
- **Linking the video transition kind to sound / stylised audio transitions** — not chosen;
  the sound always crossfades.
- **Storing the "cut / overlap" mode as a field and giving the overlap window its own handles** — a
  second owner of the window; this is exactly where Kdenlive accumulated years of defects.
- **Transitions on 1/N zones (three or more clips in a window)** — there are no meaningful roles;
  a validation error.
- **A separate f64 reference for volumetric kinds on the GPU** — a systematic discrepancy with the
  driver's f32 would have to be hidden by a tolerance; instead both sides read the same f32 bits.
- **A depth buffer / a new pass for 3D kinds** — occlusion is analytic, and a free field of the
  uniform is already there.
- **Volume compensation when splitting 3+ legacy overlaps across tracks** — an exact compensation of a
  per-sample varying 1/N does not exist (the owner's decision).
- **A transition as an overlap (without an explicit object at the joint)** — identity by a pair of
  clips survives group moves for free; the position is not stored.

## Non-obvious details

- The numbers of the `MODE_*` modes (`src/libs/kt-engine/src/transitions.rs`) are a contract only
  between this file and `src/libs/kt-gpu/src/shaders/composite.wgsl` (`trans`): the value goes into the
  uniform, so a new mode may be inserted only at the end. The project file gets `tag()`, the cache keys get
  the ready `TransMix` (`put_mix` in `src/libs/kt-gpu/src/engine.rs`).
- The case list of t224 (`src/libs/kt-gpu/tests/parity.rs`) MUST contain both axes and both flags of each
  directional kind, shifted centres for iris/clock/zoom/spin, and the phase between a quarter and the middle:
  the threshold of the two-phase Dip is exactly 0.5, and at k ∈ {0.25, 0.5, 0.88} its shift is not visible;
  at the default centre (0.5, 0.5) an error in the centre term is invisible. The tolerance is ×2 of the
  measurement (worst 1 LSB on 0.5 % of pixels).
- In push, a screen pixel shows the source BEHIND itself (`uv − dir·k`), while the incoming frame B stands
  right BEHIND the outgoing one — a full screen further along the direction of motion (`uv + dir·k`). The sign
  for A and for B is different; with a wrong sign the coordinate goes outside [0,1), the bilinear clamp
  (`Img::texel` in `src/libs/kt-engine/src/transitions.rs`, `t_texel` in WGSL) pins it to the edge, and a strip
  of the first row/column slides in. The same applies to the tap loop of whip.
- `apply_invert` (`src/libs/kt-engine/src/video.rs`) is the single point of inversion: it is gated by
  `!is_crossfade()`, and for a solo fade it flips `incoming` (the weight expression itself gives the complement).
  Contract: any construction of `PlanLayer::Dissolve` — including an overlap pair — goes through it, otherwise
  inversion on it is silently lost.
- `TransitionKind::symmetric()` (`src/libs/kt-model/src/project.rs`) is a deliberately exhaustive match: a new
  kind must decide whether it is symmetric. Zoom is symmetric through the mix identity `mix(y, x, 1−t) == mix(x, y, t)`;
  spin is not (the sign of the rotation changes meaningfully). For directional kinds inversion coincides with changing
  `Dir` — the duplication is harmless.
- `trans_ext` (`src/libs/kt-engine/src/transitions.rs`) fills the field `m: [f32;16]` of the composite uniform, which no
  pass had read since the old 3D pass was removed. The degeneration of an edge (θ = π/2) is a deterministic ε-shift in
  f64 before rounding; `asin` of the cylinders — via `atan2(d, √(r²−d²))`, because asin in GLSL has a permitted
  approximation, while atan2 is already within the tolerances of clock. Door reads the right leaf via H(1−X) — a second
  homography is not needed. `put_mix` and `plan_key` (`src/libs/kt-gpu/src/engine.rs`) do not hash ext — it is a function
  of what is already hashed.
- Blocks and Shake take the order of cells/jerks from the same integer hash as glitch
  (`src/libs/kt-engine/src/transitions.rs`) — determinism without a seed. The helper `fracf` repeats the semantics of the
  WGSL `fract`: `f32::fract` has a different sign on negative values. In FilmBurn and LightLeaks the contribution of the
  edge/spots is gated by alpha — otherwise a solo fade would light up emptiness.
- The solo fade (`Img::Clear` on the CPU, the clear bit of the uniform on the GPU;
  `src/libs/kt-engine/src/transitions.rs` / `composite.wgsl`) does not read the second side at all; `complete` is not reset
  (`src/libs/kt-engine/src/video.rs`) — the absence of a pair by the model is not a frame starvation, otherwise playback
  would wait for a non-existent layer.
- The event level (`opacity_k`) of a dissolve pair is blended with the same weight as the picture
  (`src/libs/kt-engine/src/video.rs`, next to `scale_alpha_into`): otherwise a crossfade between clips of different
  transparency would jump.
- The end of a transition in video is looked up by `kt_model::same_clip` plus a check of the boundary
  (`src/libs/kt-engine/src/video.rs`, the attachment of windows): the morpher layer cuts the target, the id of the original
  stays with piece No. 0, and the joint goes to the piece whose boundary is on it. The comparison `c.id == id` returned the
  wrong piece, and the transition silently disappeared (t237). `t.joint(project)` is computed from the raw clips and remains
  correct.
- The audio fade windows are precomputed in `AudioRenderer::new` (`src/libs/kt-engine/src/audio.rs`); the boundaries are floored
  into frames by the same conversion as the clip boundaries — the byte-exact conversion of old overlaps rests on this. In the
  handle zone a missing burst is not counted as an underrun. For reversed clips with FX the handles are disabled: the mirror
  anchor of the source is tied to `window_end`, and extending would shift the body.
- On an overlap, the explicit window is marked `fade_in_excl`/`fade_out_excl`, and `overlap_at`
  (`src/libs/kt-engine/src/audio.rs`) returns 1.0 in it — the curve is given by `fade_weight`, there is no double fade.
  Equal-power on an overlap = the same √ formula on the same window as for a bare overlap — GOLDEN_AUDIO stays intact by
  construction, not by chance.
- Both audio (`build_reel` in `src/libs/kt-engine/src/audio.rs`) and video (`src/libs/kt-engine/src/video.rs`) attach windows
  by `reel.transitions()`, not by `project.transitions`: otherwise a transition inside a nested reel silently disappears (t463b).
- `TransitionEvent::overlap_pair`/`window` (`src/libs/kt-model/src/project.rs`): the overlap mode is derived from the geometry — full
  containment and equal starts are not counted as a pair (there is no direction and no meaningful window); `in/out_offset` are not
  read in this mode, but they are not zeroed either — a joint transition whose pair was overlapped by a move survives and returns the
  offsets when it returns to a joint. `reconcile_reel` (`src/libs/kt-model/src/model.rs`) canonicalises `a` as the early one; the
  directional kind is mirrored at the same time (death would be worse).
- `ops::restore_handles` (`src/libs/kt-model/src/ops.rs`) skips pairs that overlapped in `before` (nothing was cut in an overlap — the trims
  would be false), and clamps by the free space of each member of the link group and by `source_in` (for reversed this is TrimEnd):
  `apply_batch` validates the result; without the clamp, the overlap would roll back the move itself. In the UI all gap gestures go through
  `KtApp::apply_batch_restoring` (`src/apps/kt-app/src/app.rs`); a new delete/move path that bypasses it will again start losing material.
  Reconcile of transitions — once per batch, not from separate mutations.
- `commit_fade_handle` (`src/apps/kt-app/src/app.rs`) and the `MovePlan::Crossfade` branch of the release handler
  (`src/apps/kt-app/src/timeline.rs`), when a transition already exists, change only the window offsets and do not assemble a new `TransitionEvent`.
- A corner fade is a transition with a zero external offset; in the top strip `transition_at` (`src/apps/kt-app/src/timeline.rs`) skips such a fade,
  otherwise the edge of the window would be clamped to the minimum and there would be nothing to remove the fade with. `transition_at` also skips an overlap
  window (there are no handles); a double click is caught by the body (`transition_body_at`).
- File: an overlap pair lives in different adjacent Tracks (the layer map) and cannot be expressed as a child of a sequential track — it travels as the list
  `overlap_transitions` in the envelope of ITS reel (`src/libs/kt-otio/src/meta.rs`, read/write in `src/libs/kt-otio/src/convert.rs`), without id and offsets; a broken
  record is a notice and is skipped. When returning to a crossfade, the `kt` envelope of the transition is cleared entirely: a stale `"kind"` would otherwise travel back,
  and the next opening would return a wipe.
- The transition window (`src/apps/kt-app/src/transition_editor.rs`) draws the numbers with the common `widgets::params_grid` through the trait `kt_model::HasParams`
  (implemented by both `FxInstance` and `TransitionKind`); direction, aperture shape and colour are drawn by their own controls (direction is not a number). Three new
  parameters without a label in the catalogue were drawn by their raw field name — each parameter of a kind needs an i18n string.
- The live scenario `trans` (`src/scripts/live/`) reads the preview pixel through `pixel_settled`: after any edit of WGSL, llvmpipe compiles the first composite frame
  for a second, and a snapshot right after a click shows the frame of the previous position — the whole section turned red, including the untouched crossfade.

## Deferred

- The decode scale step for zoom-in kinds (Zoom/ZoomBlur/Spin): sharpness when zooming in inside
  the transition window — a separate round, if it is ever needed (capability `video-plan`).
- Volumetric kernels on the CPU fallback are more expensive than wipes (perspective division,
  cylindrical unwrap) — accepted as the price of the fallback.
- A three-level menu of kinds (clip → transition → category → kind); the fallback is a flat list with
  separators, if the live check shows it is inconvenient.
