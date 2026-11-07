# Design: input-without-work

## Context

The owner's words are in `docs/ktprompts/kkt-26.11.07b.txt` (lines 5-7, registry rows G5 and G7), his
files in `/media/e/developer/kt.9/fails/26.11.07`. His load report, 466-515 s ago: prefetch ~90 % and
audio-render ~85 % of a core for 20 s at a time, `audio_renders` 0, `prefetch_passes` 1 a second, and
lines 482 and 512 "at rest" at 196 %. The rule he set the same day (CLAUDE.md, "A problem is a CLASS"):
his example names one place; the defect is the mechanism, walked through every subsystem.

## D1. The mechanism, measured

A chain of four links, each of which did work for input that changed nothing:

1. **A widget reported a change no one made.** egui's `DragValue` with a range clamps an existing value
   and marks the response changed (`clamp_existing_to_range` defaults to true); `Slider` does the same
   (`SliderClamping::Always`). A plugin row stored as f32 cannot hold some bounds: -0.8 is
   -0.800000012, below a floor of -0.8. The owner's VHS Tape held `dark_clip = -0.8` (his project, the
   linked pair 2132/2133). The percent field clamped it, read "changed", and `param_row_bound_ex`'s base
   path wrote it back with no gesture check (its curve path had one since R.67, rule 25).
2. **The cast undid the write.** `PluginSlot::set_idx` casts back to f32, so the next frame saw the same
   value below the floor: the loop never settled. Every frame was a change.
3. **The model took an equal value as an edit.** `apply_coalesced` swapped in the simulated project
   without comparing it; `KtApp::apply_coalesced` bumped `model_rev` and called
   `push_project_to_player`.
4. **Every edit rebuilt the whole engine.** `Player::set_project` builds a `VideoRenderer` on the UI
   thread and marks the audio render dirty: the audio render thread builds an `AudioRenderer`, the
   prefetch thread an `AudioRenderer` and a `VideoRenderer` for the new version. On the owner's
   project `kt-cli probe` measures `AudioRenderer::new` at 447 ms (release, 1987 effective events).

At rest no frame runs, so each MOUSE MOVE was one frame, one edit and one rebuild: that is why it
"calmed down" and started again on any move, why switching VHS and ISP off changed nothing (the panel
still drew the field), and why selecting another slot stopped it (the field was no longer drawn). The
counters saw `ui_frames` and one `prefetch_passes`, not the rebuilds, so the watch called it rest.

Headless reproduction (the harness, his VHS values): 400 pointer frames over the panel → 400 model
revisions; with `dark_clip` alone at -0.8 the same, without it 0.

Numbers on his relinked copy (release, the real `KtApp::open`, the effects window on event 2132 with
VHS Tape selected, 10 s of the pointer moving; processor time of the threads from `/proc`):

| | frames | prefetch | audio-render | model edits | engine builds |
|---|---|---|---|---|---|
| before (the field and the model gate reverted) | 6 in 10.9 s | 69 % | 68 % | 12 | 41 (12 swaps) |
| after | 297 in 10 s | 0 % | 0 % | 0 | 0 |

Before, the UI itself managed six frames in eleven seconds: each frame cloned the project and built a
`VideoRenderer` on the UI thread before the engine threads built theirs. The pause with the window open
and the pointer still was 0 % in both: the loop needs a frame, and at rest none runs.

## D2. The rule and where it is enforced

**Input that changes nothing changes no model, rebuilds nothing, wakes no prefetch, orders no decode; a
write of an equal value is no edit.** Two layers, so a future widget that slips past the first is still
stopped by the second:

- **The widget.** All number fields go through `widgets::param_edit` (param rows, grids, `plain_row`,
  `pair_row`, `NumRow`): `DragValue::clamp_existing_to_range(false)` and `SliderClamping::Edits`, the same
  choice `shape_num` made in R.67. The base path of `param_row_bound_ex` now writes only on
  `edit_gesture` (drag, drag stop, focus, lost focus, click) or a reset, and `write_base` reports no edit
  when the carrier holds the same value after its own clamp and cast. `event_gain_field` likewise.
- **The model.** `TimelineModel::apply_batch` and `apply_coalesced` compare the simulated project with
  the canon and return without an undo step, a revision or a started gesture when they are equal;
  `revision()` counts real edits (undo and redo included). `KtApp::apply`, `apply_batch` and
  `apply_coalesced` bump `model_rev`, push the project and set `dirty_content` only when it moved; undo
  and redo bump nothing when they fail.

Rejected: nudging the f32 cast inward at the bounds (`set_idx`). It changes stored values and so
rendered bytes of projects at a bound, and the two layers above already stop the loop.

Rejected for now: making a real edit cheaper than a whole rebuild (an incremental `AudioRenderer`, one
renderer shared by the audio and prefetch threads). It is the right direction ("a real change touches
only what it determines") but a change of its own; recorded in "Found, not fixed" of the report.

## D3. The class walk

What was checked, against the mechanism "input that changes nothing does work". The sweep of kt-app
was done once by reading every write site and once by the guards t1870-t1872, which run every passport
at its floor and its ceiling.

| Place | Finding | Action |
|---|---|---|
| Number fields of every effect, transition, generator, track (the shared grid, `param_row_bound_ex`) | wrote without a gesture: VHS Tape and its sound at the floor, Slowed+Reverb, Chromatic Aberration, Camera ISP and its sound at the ceiling (t1870/t1871 red on the old code) | fixed (D2) |
| VHS Tape deck face (`deck_face.rs`): switches, flags, condition buttons, drum heads | click-only, `i != cur` | fine |
| Camera ISP pipeline (`isp_panel.rs`): bypass checkboxes, choice buttons, flags | click-only | fine; its numbers go through the grid (fixed) |
| Codec/datamosh stream panel, pan keyframes, conform row (`fx_editor.rs` `draw_video_fx_params`) | click / `changed()` of unclamped widgets | fine |
| Effect colour rows: green screen key, edges line, duotone, glow and drop shadow tint (`fx_editor.rs`) | `as u8` TRUNCATES the colour popup's float round trip (199.99998 → 199): an open popup can drift a colour down and write it each frame | fixed: round, write only a changed colour (t1877) |
| Transition colour (`transition_editor.rs`), generator/title colours (`gen_ui.rs`, compared before sending), publication overrides (`pubparam.rs`) | round and/or compare | fine; an equal write is stopped by the model gate anyway |
| Chain strip (`widgets::chain_strip`): select, drag, bypass, cross, presets | gesture | fine |
| Automation lanes (`automation.rs`): keys, shapes, formula, link | gesture; a held key drag re-sends the same key | fine; the model gate drops the equal write |
| Frequency-response graph (`eq_graph.rs`) | drag only, compares f32 against f64 | fine; the model gate drops the equal write |
| Geometry gizmo, mask editor and list (`geomctl.rs`, `maskctl.rs`, `masks.rs`) | drag / click / value compare | fine |
| Transition, track, camera, morpher windows | value compare or the model gate | fine (t1872 guards them) |
| Timeline live level drag (`timeline.rs` `Drag::Level`) | `player.set_project` on EVERY held frame, the level moved or not: a held mouse rebuilt the engine at the frame rate | fixed: only when the level moves (t1876) |
| Timeline hover, scroll, zoom | idle-cpu's `EventPaint`; no model write | fine |
| Speed control drag (`app.rs` `draw_speed_control`) | `set_playback_rate` on every held frame resets the display clock's extrapolation | fixed: only when the rate changes (no test of its own: the reset is visible only as display smoothness) |
| Preview overlays, menus, media bin, toolbar, settings and preferences windows | click / compare | fine |
| Diagnostics | samples on its own 2 Hz clock by design | fine |
| Thumbnails (`thumbs.rs`) | a sidecar JPEG that fails to decode published neither ready nor failed: the filmstrip, the bin and the effects picker re-ordered it on every frame | fixed: a failure (no test: a corrupt row needs a sidecar fixture; the branch is two lines) |
| Effect picker cells, reel previews | deduped and keyed; churn only past the LRU caps (384 thumbnails, 256 cells on screen at once) | fine; recorded |
| `model_rev` / `dirty_content` / `set_project` sites (`app.rs`, `timeline.rs`) | apply doors pushed on every call; undo/redo bumped on failure; the level drag above | fixed (D2) |
| `Player::set_loop_region` and `sample_levels_at` (`kt-engine`) | each region change built a whole `AudioRenderer` on the UI thread; the meters built another after each edit | fixed: one UI renderer per project version (t1875) |
| Paired device halves (`sync_pairs_to_link_group` in kt-model) | mirror per command, not per frame; a no-op on one half still writes the other when they differ | fine: nothing sends no-op commands per frame any more; recorded |
| Invariant 3 (a chain is not re-instantiated by an edit) | a no-op is now no edit at all; a real edit keeps `sync_fx` | fine |
| The load watch and its counters | rebuilds had no counter, and a busy thread with no counter read "at rest" | fixed: `model_edits`, `project_swaps`, `engine_builds`; "NO COUNTER MOVED" (t1880); `idle.py` judges threads, not only counters |
| Render dialog sliders (`render_dialog.rs`) | default clamping on local settings: an out-of-range bitrate is written back once | harmless, not the model; left |

## D4. The load guard sees this work

`Sample::at_rest` is now "no counter moved AND no thread other than the watch at 10 % of a core or
more"; a busy second with every counter still is `uncounted` and the line reads "NO COUNTER MOVED".
The report's format is otherwise untouched (`telemetry-journal` reworks it). The live scenario `idle`
fails a pause that holds such a thread, and adds the VHS deck step: select the slot, drag the dark clip
number to its floor by a real drag, wait for quiet, sweep the panel; the new counters must stay still and
the prefetch and audio render threads under 5 % of a core each by their processor ticks.

## What a real card could not prove here

Measured on llvmpipe. The defect and its fix are processor-side (the model, the engine threads), so the
card does not change them; the UI thread's own frame cost on his Radeon is `ui-frame-cost`'s.
