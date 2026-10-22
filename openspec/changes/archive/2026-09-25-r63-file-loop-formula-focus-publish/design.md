# Design

## Context

See proposal.md — Why. The reconnaissance for each item with line numbers — `docs/user-feedback.md`,
R.63, "What the reconnaissance showed". The facts the design starts from:

- R.61 moved the shape of a segment onto the keyframe and removed `ease` from the model; the reader expands
  the former boolean into one shape per curve (`implied_shape`), and the speed validator from the same round
  accepts only the linear form (`RateShapeMustBeLinear`). Warp never read the shape at all.
- The engine does not know the length of the media (law P): the source window is computed per event, and a "miss
  inside the window" is an underrun. The model does not know the length either: `MediaRef` is a path and an id; the duration
  reaches only whoever opened the source (`MediaInfo.duration`, kt-app at the moment of setting).
- egui 0.36 gives focus away from `TextEdit` on Esc/Tab, on another focusable widget, or on its disappearance; a click on a non-focusable
  widget (`Sense::click_and_drag`) does not touch the focus. `handle_keys` returns entirely to `egui_wants_keyboard_input()`.
- The formula field: the text = `match &c.source` every frame, applied on `r.changed()` when the parse is legal, `c.keys.clear()` there
  too; the graph — `curve_value(&c.keys, at)`.
- The menu of a segment/keyframe: the target comes from `hover_pos()` every frame inside `context_menu`.
- Publication: `ParamOwner` — `FxSlot | Generator | ClipAttr | Transition | Camera |
  Nested`; the lead geometry is the event field `geometry`; `PublishParam` without a reel address,
  the confirmation calls `enter_reel_for_authoring`; the consumer is `InstParamsWin`.
- Ducking: `draw_duck_dialog` (window `duck`), `DuckParams::default()` on every opening.
- The flight log of paintitblack: after jumps in the review mp4 (H.264 1080p, 870 s) `u` ≈ 48 000/s at an offline cost of 2% of real time; `w=4800` — a miss of the whole pre-roll.
- Version 26.09.23-d5da7cf — the archive of change A; there are no object tracks (change B) in it.

## Goals / Non-Goals

**Goals:**
- Each defect is closed by a test that is red on the old code, and by a live scenario where the defect
  was green in the harness (the formula, focus, menu, file, jump).
- The owner's rules are guarded by guards (the window registry, the name scanner), not by a paragraph.
- No earlier contract is weakened: the tests of the consumer window move to the head of the strip
  with the same assertions.

**Non-Goals:**
- A live sidechain compressor (change `sidechain-ducking-compressor`), the geometry frame in
  the preview and the new form of the fields (change `geometry-on-screen-controls`), track envelopes.
- A change of the record format: we write as before; the envelope version does not move.
- Skipping decode/reading past the end of the material as an optimization — only honest silence.

## Decisions

- **Speed migration: `ease: true` on `rate` → `Linear`, on the others → the former
  smoothing.** The alternative — weaken the validator ("speed with a smooth shape is
  allowed, the shape is ignored") — was rejected: it is a silent branch "one thing in the editor, another in the sound",
  exactly what R.61 forbade. The reader knows the parameter's name, so they also know the
  correct shape. Guard — t747 on the owner's file shape (curves of `rate` in three places:
  the event, the morpher patch, the nested reel).
- **The edge of the material is reported by the SOURCE, not the plan.** The fetcher knows the length (`MediaInfo.duration`
  for a file; for a reel — the end of the content; for a generator — infinity). `fill_source`
  gets "past the end of the material" from the fetcher and fills with silence without an underrun; for video —
  `last frame hold` by the same flag at the worker. The plan still orders by event
  (invariant 4 intact: order ⊇ read), the worker answers keys past EOF with a negative cache (t27
  already exists). The alternative — teaching the plan the length of the media — was rejected: law P (the source address
  is a property of the source).
- **Clamping the trim — in the model by an OPTIONAL media duration.** `MediaRef` gets
  `duration: Option<Tick>`, which is written by whoever opened the source (kt-app when syncing
  sources), and which is written into the file. **Clarification during 1.3: the place in the file is not our
  envelope, but the STANDARD `available_range` of the OTIO external reference.** It is exactly about this,
  a foreign reader understands it, and we already read its start as the timecode of the material.
  We write only when the field was absent: what came from a foreign file is passthrough (rewriting it with our
  own measurement would mean "understanding it halfway"). For a generator `available_range` carries
  the consumed piece, not the length of the source, so the length does NOT become that from there.
  The fact "the file lasts N" is brought outside undo (`TimelineModel::note_media_length`, the shelf
  of region and viewport): there is nothing to roll back in a measurement, and the asterisk "project changed" from
  opening a source would violate item 5 of R.11. Trimming the right edge is limited by
  `source_in + consumed(rate-profile) ≤ duration`; without a duration — as now. Existing events longer than the material are valid (rule 20), they are drawn with hatching from the
  point where `used()` crosses the length. Reconnaissance 1.1 (below) confirmed: the duration from
  FFmpeg is correct, and the right trim has no limit at all — `trim_max_end` returns `i64::MAX / 4`.
- **The audio priority in the worker — by a selection rule, not a second queue.** In `pick_next`:
  among the requests Playback/NearPrefetch, the audio burst always comes before the video frame of the same
  generation; after decode the video frame checks the queue before the next frame of the span
  (today the span top-up goes "on top of the priorities", t27 — keep its property "each
  frame once", but cut the span at the audio burst boundary). The diagnosis must be confirmed before
  the edit: t758 with a "slow" synthetic video (a frame costlier than a burst) reproduces
  the starvation; the live `seekjump.py` on `kt-untitled-review-870s.mp4` reads the flight log. If
  the starvation turns out to be elsewhere (a seek inside the mp4, the pre-roll), that is what gets fixed — the test stays the same.
- **Focus: one rule in `KtApp::ui`, before `handle_keys`.** If in this frame there was a
  `primary_pressed` outside the rectangle of the focused widget (`ctx.memory().focused()`
  + rect from `used_ids`), the focus is removed by `surrender_focus` before the keys are read. Not in each
  window separately (rule 17: paired paths — one body; otherwise the next agent will forget).
  The Esc cascade and `surrender_focus` on closing windows remain — these are different paths.
- **The formula field: a buffer in `AutoView`, applied on confirmation.** `AutoView.formula:
  Option<(usize /*curve*/, String)>` — the text being typed; the model is updated on Enter and on
  losing focus with legal text; an illegal one is an error in the status line, the buffer stays alive.
  The graph for a derived curve is drawn by `resolve_derived` by the frame step (the same code as the resolution when the
  tape is built), distinguishable (dashed/italic label, as now).
  The alternative "apply on the first legal prefix, but do not touch the keys" was rejected:
  "t" is legal and at once changes the sound to a straight line — a surprise in the middle of typing.
- **The menu of a segment/keyframe: the target is fixed on `secondary_clicked`** in `AutoView.menu_target`
  and lives while the menu is open (`resp.context_menu` returns `Option<InnerResponse>`;
  closing — reset). The highlight of the segment is a backdrop between x keyframes; the header — both
  times of the material and the share of the event (`ctx.frac`).
- **`ParamOwner::Geometry`.** The address is the event, the parameter is the name of a field of `EventGeometry` (the same
  names as in the passport `HasParams for EventGeometry`, including `rule` as a choice).
  `apply_one` writes through `set_value` into `c.geometry`, the curves — into `geometry_auto` through
  `put_curves`. An audio event rejects the address by validation. The door is the same `PublishCtx`; the flag
  `lead_sel` no longer switches it off.
- **`PublishParam { seq, param }` and relatives.** The command carries the reel; `check_on` rejects a nonexistent one; `owner_reel_of` by the working view remains the source of `seq` in the UI.
  `enter_reel_for_authoring` remains ONLY for the author window "Public parameters" (there the person edits the reel's list — standing inside it is reasonable); from the publication confirmation it is removed. The "Deferred" item of R.59 about the transition/camera from the strip is closed by the same field.
- **The head of the strip "Composition parameters".** `fx_editor` for an instance of a reel sets a `HeadSlot` with the form `pubparam::draw_inst_head` (the body of `draw_one_inst_params` moved: a grid of handles,
  `draw_value_row`, reset, the automation section on the `AutoView` of the window).
  `InstParamsWin`, `draw_inst_params`, `open_inst_params` are removed; `open_fx_editor` with
  `ChainSel::Head` is the single door; the instance menu and double click lead there.
  The undo session is the FX window token (shared). Tests t683/t684/t685/t691 are rewritten for the head
  with the same assertions — this is the owner's decision, not a weakening.
- **Ducking in the track window, parameters in `user_store`.** A section in the audio track window: fields
  from `DuckParams` (with the same `param_row`), the choice of the source track, a "Ducking" button; the parameters
  are read from `user_store` on opening and written on invocation. The window `duck` is removed,
  the menu item is renamed. The note F-AUD-004 gets a section about the live sidechain as
  the next change.
- **The window registry — data, the guard — a scanner.** `WINDOWS: &[(&str /*id*/, &str /*why*/)]`
  in `reach_tests.rs`; the scanner looks for `egui::Window::new`/`Modal::new` and next to it `.id(egui::Id::
  new(...))` — the id is mandatory (already the rule of t129), so matching by id is reliable.
- **The owner's rules — in `apps/kt-app/CLAUDE.md`** as a separate section "R.63 — the owner's four rules" with a reference to the guard of each.

## What the implementation of section 2 showed (item 13)

- **Measurement instead of guessing.** On the owner's file the audio burst decodes in 14 µs, while a 1080p frame there takes 22-61 ms. The sound was not slow: it WAITED.
- **It was waiting for the span.** The worker takes up to 64 keys and hands them to the decoder in ONE call of `read_video_span`, without looking at the queue for up to 4 seconds: the arrived Playback burst stood there the whole time, however high its priority. This is the 1-6 s of silence on a jump.
- **Interrupting is allowed only for the sake of STRICTLY more urgent sound.** The first version interrupted the span for any waiting sound — and t21 turned red at once: the video cursor went zigzag after the audio, that is, the ping-pong seek came back (defect h). The rule was caught by a test, not by reasoning.
- **The abandoned tail of the span returns to the queue**, rather than being discarded: the keys were removed from it when it was assembled, and "someone will ask again" is true for prefetch, but not for a thumbnail or a one-off request. And ONLY the keys that were reached go into the negative cache — otherwise the abandoned tail would be marked dead, and those frames would never be shown (caught by t758).
- **The live scenario `seekjump.py` did NOT reproduce the starvation** — it is green even on the code before the fix, even with three layers on top of a long event. So it is a guard against regression, and the proof of the fix is t758 and the measurements. The owner's scene is heavier: six video tracks with effects, four media, GPU composite; it was not repeated in full in the scenario.

## What section 4 showed (items 9-12)

- **Item 9 was a hole in ADDRESSING, not a code duplicate.** The lead node "Geometry" had no `ParamOwner`, and the publication door at its captions was closed by one line `(!lead_sel)`. Its passport, `set_value` and curves are the same as those of the slot — the `apply_one` branch became five lines (t752/t752a/t752b/t752c).
- **Item 11 is fixed in the MODEL, not in the window.** The publication command for a rule targets "the reel you are standing in", so the confirmation had to first ENTER the reel. With the `seq` address in the command itself, no entry is needed: t753 submits it from the root with the strip expanded, from inside the reel and from a NEIGHBOURING reel — the result is the same. The live scenario `nestparam.py` confirms the consequence by hand: after confirmation the entry depth stayed at zero, and the handle landed in the reel of the strip.
- **Item 10 did not reproduce ANYWHERE.** t755 (engine, `RadialBlur.amount` on two instances) and t755b (the preview keyframe of an instance) are green even on the old code; the live scenario, written exactly to the complaint (publishing `amount` from the strip, editing at the second instance), shows a change of the preview pixel (5,5,5) → (10,10,12). What this honestly means: the path "slot handle → instance frame" is now closed by three guards at three levels, but the complaint ITSELF remains without a found cause. Most likely it was a consequence of item 11 (publishing from the strip took the context elsewhere, and then the person edited something other than what they thought), but nothing can prove that, and we will not present a guess as a diagnosis.
- **The GPU is not checked by a separate test, and this is deliberate.** The override is resolved in `resolve_reel` BEFORE the plan is built: the executor (CPU or GPU) receives ready numbers, and the instance's edit enters `fx_chain_hash`, `nest_memo_key` and the GPU `plan_key`. Backend agreement is held by the shared conformance tests of kt-gpu (t181+); creating a duplicate of them for the sake of publication would mean checking the same arithmetic twice.
- **Found incidentally by the live scenario:** the default name of a handle was taken with a colon ("Gain:") — the field's caption contains layout markup, but the handle name does not.

## What the finish showed (task 6.2)

- **Live scenarios are sensitive to the MACHINE'S LOAD, and this must be known in advance.** The full run of `live-check.sh` passed 45/45 cleanly. A repeat run of the same script INSIDE `finish.sh`, started right after it (load average 8-16, some scenarios take up to 5 cores each), knocked out seven scenarios in a row — `analysis`, `conform`, `corner`, `deadend`, `formula`, `fxfield`, `fxpicker`. Each of the seven was re-checked ONE BY ONE on the calmed machine and is green. The conclusion for the next agent: a red live scenario on a loaded machine is first a suspicion of timing (the gestures wait for the window's reaction with fixed pauses of 0.3-1.2 s), and only then of the code; check them one by one and in quiet.
- The build version on which all this was measured — in the round's report.

## Risks / Trade-offs

- [Media duration in the model — a new file field] → optional, written only when it is known; old files without it behave as now; the version does not move.
- [Audio priority in the worker breaks the property "each frame decoded once" (t27)] → the span is cut, not cancelled: the decoder's cursor continues from the same place; t21/t27 remain in the spec and must stay green.
- [The head of the strip for an instance: the strip of the instance's chain is already long anyway] → the head collapses, as for a generator; rule 13 — the place is already taken by the FX window.
- [Focus is removed by a click anywhere — a drag in another window] → `primary_pressed` is counted only outside the rectangle of the field; a click inside one's own window on another widget still removes focus — this is the expected behaviour (as in any editor).
- [The diagnosis of item 13 may turn out not to be in the queue priority] → t758 and the live scenario assert the consequence (underruns after pre-roll = 0), not the mechanism; whatever is found gets fixed.
- [The migration `ease`→`Linear` for speed changes the shape if someone recorded a speed with an explicit smooth shape in R.61] → impossible: the R.61 validator does not write such a file.

## Migration Plan

Only the reading of old files changes (speed curves); writing stays the same. The `duration` field of media appears in the file at the next write of projects where the source was opened; no rollback is needed — an old reader skips the field (passthrough).

## Reconnaissance 1.1: where 237.7 s comes from (closed)

Measured, not assumed:

- **The duration is correct.** `kt_media::ffmpeg::FfmpegSource::open` on this webm returns 226.268000 s — exactly what `ffprobe` gives (`format.duration`). The hypothesis "FFmpeg lies about Segment/Duration" is REMOVED; on review-mp4 it is 870.101 s, also correct.
- **The right trim has no limit at all.** `kt_model::trim_max_end` for a NON-mirrored event returns `i64::MAX / 4`: the end can be dragged as far right as you like, because the model does not know the length of the media. The symmetric `trim_min_start` does have a limit — the material on the left ends at zero, and this is visible from the event itself.
- **The reconstruction converges numerically.** The Rolling Stones event (pair 3/4) lasts 316.967 s; at rate 0.75 this is 237.725 s of material for a file of 226.268 s — 11.457 s of material past the end (15.276 s of timeline). The neighbouring event 5 on the same track — 277.912 s → 208.434 s, inside the material, and there is no defect in it. That is, the edge was dragged RIGHT before the slow effect (at rate 1 the event was already past the end), and the slowdown only reduced the overrun from 90.7 s to 11.5 s. The complaint "this happens less often with MCR" is explained by the same thing: there the event is 17.6 s inside the material.

Consequence for the design: BOTH sides of the problem must be fixed — the limit of the trim (so that this no longer happens) and an honest edge of the material in the engine (so that what has already been done plays as silence without underruns, rather than being counted as starvation). Both are already described in the decisions above; no open questions remain.
