# Design

## Context

See proposal.md — Why. Today: `ClipEvent.lane: u32` is the only link to a track;
the order of rows is `Project.lane_order`; the name/height live in `UiState` outside undo; the mix is
`out += buf × fade × overlap × gain` with no level between the clip and the output (`audio.rs`);
`FxStateStore: HashMap<ClipId, ClipProcessor>`; video is alpha-over by `lane` (`video.rs`),
the morpher is flattened in `effective_clips()`; reels are a sub-plan `NestSrc` and a sub-mix under
the fetcher; the render window already lists the stubs before the start (`render_dialog.rs`).
Invariants 2, 3, 6, 7, 11; UI rules 1, 9, 13, 16, 17, 19, 20.

## Goals / Non-Goals

**Goals:**
- A track-object with a stable id; all track semantics live on it.
- A track stage in audio and video, appearing only where it is needed; the bytes of old
  projects do not move.
- Mute/solo/lock/level/chain/blend mode in the model, file, interface, and export.

**Non-Goals:**
- Track envelopes over the timeline time (a second automation domain) — the next change.
- Buses, sends, stems, sidechain; track parenthood (Vegas parent/child); a clip blend mode;
  hiding a track separately from mute.

## Decisions

- **`Track` — a vector at the root and in each reel; the order of the vector = the order of rows.**
  `lane_order`, the four counters, `lane_names`, `lane_heights` are removed. The track number
  of an event remains the rank inside the view (the invariant "number = row rank" from R.56 — one
  reorder door renumbers the contents), and `TrackId` is the stable address for
  state and the file. The alternative "`lane: TrackId` on the event" was rejected: 182 uses of
  the rank in drawing and plans, and the rank is needed for z-order anyway; the id is needed where
  the state lives.
- **Name and height — in undo.** Three parallel vectors and six sync functions
  disappear — the class "name exists, flag does not" after insertion is closed by construction.
- **Mute/solo — project content (the owner's decision).** Together with this comes the duty to
  say: the render window lists muted tracks and the active solo between the settings and "Start"
  (the place where the mute warning already stands), kt-cli prints a notice.
  Solo is a set inside the view (video and audio independently), not "mute the others": clearing
  solo restores the previous mutes (lesson from Kdenlive #421225).
- **Mute = multiplier 0, not removal from the graph.** Audio: the clips of a muted track
  still render into their stage, the output weight is 0 — the FX state stays alive (invariant 3),
  clearing mute does not start from a cold reverb. Video: a muted track is not composited
  (there is no state — the exception is legitimate), but its prefetch order remains, so that
  clearing mute does not produce a black frame.
- **The audio track stage only on a track that needs it.** Condition: the chain is non-empty
  OR the level ≠ 0 dB OR mute/solo silences the track. Without the stage, the clip is summed into the output
  in the former order — otherwise reordering the f32 additions would shift GOLDEN_AUDIO. Precedent —
  "input equals the step → former bytes". Stage: the track scratch buffer ← Σ clips (the same
  weights) → `TrackProcessor` (a chain without the clip shape: no source, no length, no warp;
  splitting `ClipProcessor` into a "producer" and a "chain" in kt-fx, the kernels are not touched)
  × level → × (0 at mute/non-solo) → into the output.
- **The FX state key is composite `Clip(ClipId) | Track(TrackId)`.** Diff by the fingerprint of the chain
  structure, as for a clip; removing a track — `retain`.
- **PDC of a track chain is lookahead, as for a clip.** The lookahead window extends forward the
  track's summing window: clips render into scratch on `[t, t + La)`, the output is aligned with
  the input, the playhead does not move. Tail: the track processor keeps running while the transport plays,
  the input is silence after the last event (the tail is honest, like `ext_end` for a clip); the cost is only for
  tracks with a chain.
- **Video: a track with a chain, a level ≠ 1, or a mode ≠ Normal — an implicit reel in the plan.**
  `PlanLayer::TrackNest { track, sub_plan, fx, alpha, blend }` is built by the same code as
  `NestSrc` for a reel (raster = the parent's canvas, time = the parent's time, the step —
  the maximum over the track's events); transitions, overlaps and fades are inside the sub-plan. A track
  without this is flattened into layers as today — the former bytes. The memo of the sub-composite — key
  `(TrackId, track revision, t)` by the same mechanism as for a reel. The morpher is still per-clip
  (flattened before the plan) and stands BEFORE the track stage.
- **Blend modes — on the track composite, both axes after alpha.** `out = lerp(below,
  blend(below, above), a_above × level)`; Normal — the former alpha_over, byte for byte. The kernels in
  pairs: CPU in `kt-engine` (`compose`), WGSL in `composite.wgsl`; the tolerance class is the same as for
  transitions. The list: Normal, Multiply, Screen, Add, Subtract, Darken, Lighten, Overlay,
  Difference. A hole on a track is transparency of the composite, not an operand (bypassing the Vegas trap,
  F-CMP-003).
- **Lock — one gate in the model.** Commands that touch events (add, move, trim,
  cut, delete, insert, change chain/level/geometry, transition at the edge) check the lock of the target
  track and answer `Refuse::TrackLocked(id)`; the UI shows an error and highlights the track (rule 16, mechanism t172).
  The track's own properties (mute, level, name, lock) remain editable. Sync-lock is not needed: we have no ripple (F-TL-006).
- **Track properties — in the passport (R.59), static.** Level, opacity, mode,
  mute — typed properties with clamping; curves on a track are rejected loudly by validation
  (precedent `CurveSource` before R.61) — the next change brings the timeline domain.
- **Interface.** Track header: a row of badges M / S / lock / FX under the name; the gutter width grows exactly by the row's gallery
  and is fixed (rule 3), the badge hit zone is at least 14 px (rule 1); the morph/camera rows are three times lower — their badges are only the lock.
  A double click on the name — rename (as today), a right click — a menu with the same
  verbs, "Track properties" — a window with level, mode, switches and a chain button. The FX editor gets a second kind of target (`Track`), the same chain-strip widget and the same automation widget (curves are rejected with a reason). Hotkeys: M/S on the selected track.
- **Format.** `Track.metadata.kt = { id, mute, solo, lock, level, blend, height }`
  (defaults are not written), `Track.enabled = !mute` (a foreign reader sees the disabled
  track honestly, the notice "no per-track mute" is removed), the track chain is `Effect.1`
  in `Track.effects` by the same code as for a clip. The name is in `Track.name` (default labels
  are still not written). Reading old files: the counters and `lane_order` → the vector
  of tracks, `ui.lane_names/lane_heights` → fields of the tracks, one-way. The v11 predicate (round
  R.62) is extended: a file with any non-default track property is v11.
- **kt-cli probe** prints for each track its id, switches, level, mode, and stage
  ("stage: yes/no, why") — first in the complaint "here nothing sounds / the picture is wrong".

### Clarifications during implementation (stage 1)

- **The row order is the order of the vector `Project.tracks`/`Sequence.tracks`.** There is no separate field
  either in the model or in the file; `lane_order`, the four counters, `ui.lane_names/lane_heights`
  remain ONLY as a legacy read input (`#[serde(skip_serializing)]`); the migration
  is one-way and lives in `Project::canonicalize_lanes`.
- **The reorder door is still one — `SetLaneOrder`.** A separate `MoveTrack` was not introduced:
  the command already names the rows by their current numbers and renumbers the contents,
  and a second door to the same vector would be a second source of truth.
- **`InsertTrack`/`RemoveTrack` — one each for all four kinds** instead of the former six
  commands (`InsertLane`, `SetLanes`, `Insert/RemoveMorphLane`, `Insert/RemoveCameraLane`):
  the kind became a parameter, because the track became an object. Removal takes the events of its
  track away in one command — the former batch "RemoveClip × N + MoveClip × N + SetLanes" in
  the application has disappeared.
- **Validation requires that the track EXISTS** (`TrackMissing`): an event on a nonexistent row
  cannot be shown. Reading adds the missing tracks (`normalize`), commands grow them
  themselves (`ensure_tracks` in `AddClip`/`MoveClip`/`AddCamera`/morpher).
- **A new track without an explicit place goes to the end of the BLOCK of its kind** (`natural_insert_pos`,
  order V..A..M..C) — the former default of the counters; it cannot be put at the end of the list,
  otherwise the default morph row would end up under the cameras.
- **The reel expansion band is NO LONGER at the tail of the number space.** The number = the rank in the vector,
  and the rows of the band stand under their instance — so the band's base is counted from the rows
  ABOVE the instance (`Band::of`), `merge_reel`/`unmerge_reel` shift the numbers of the root
  contents by the length of the block, and pasting INTO THE ROOT while a band is open steps over it
  (`paste_here`): before this, low numbers always belonged to the root, and half of the pairs silently
  moved inside the reel (guard — t596).
- **File: the track counters REMAINED in the envelope** (derived from the vector) — all former
  files' byte stability (t433b) rests on them, and the EMPTY row is expressed by them. New —
  `kt.tracks`: the row order and the properties; it is written only when the order is not natural
  or the track has something to say. The name, `enabled = !mute`, and the video/audio chain live on
  the file's own `Track` objects (a foreign editor must see them), and are not duplicated in the envelope.
- **The version is raised only by a NOVELTY** (`Track::has_novelty`: switches, level,
  mode, chain). The name and height do not raise the version: the name already lived in `Track.name`,
  the height in `ui`, and an old reader would lose only convenience.
- **The track chain commands (`SetTrackFx`) are NOT introduced in stage 1** — the fields on `Track`
  exist (the format carries them), but the command will appear together with the engine and the editor (stage 3):
  an orphaned command would break t252 and would be a feature unavailable to a human.
- **The track name and height are now CONTENT** (they go through undo, raise `dirty_content`):
  the rule "edits of content ≠ edits of convenience" is revised for them — they live on
  the object that travels with the edit.

- **The track's property passport declares the level BY KIND**: for audio `gain_db` (dB, limits
  `TRACK_GAIN_MIN_DB/MAX_DB`), for video `opacity`; a track has no foreign row at all, otherwise a dead control
  would hang in the properties window (UI rule 9). All rows are `ParamKind::Structural`:
  `auto_params()` does not return them, and there is nothing to hang a curve on, neither for the command nor for the interface; a file with a curve on a track fails to load
  (`TrackCurvesUnsupported`). The blend mode is a `Choice` by the variant KEY (`BLEND_KEYS`); the service rows of the switches have only the lock.

- **The lock is one function `Command::target_rows(p)`** with an EXHAUSTIVE match without `_ =>`:
  a new command will not compile until it is assigned to "touches events" (name the rows) or
  "does not touch" (an empty list) — the same device as `Band::after_batch`. The gate stands
  as the first line of `check_on`. The lock also covers a MOVE onto a locked track (`MoveClip` names
  both rows) and the removal of a track (it takes the events away); the track's own properties are not covered, otherwise there would be
  no way to unlock it.

### Clarifications during implementation (stage 2)

- **Without a chain the track stage is a SCALAR, and it needs no buffer of its own.** The level, mute, and solo are folded with the event's `gain` into one number (`db_to_linear(clip) × track`): the multiplier
  stands last anyway, after the weights of the transition and the overlap, so the result is the same, and
  the ORDER OF f32 ADDITION does not change by a single tick — t2/t3 and GOLDEN_AUDIO stand without a single
  branch. The scratch buffer appears together with the track chain (stage 3.1), where it is really
  needed: a chain is nonlinear, and the sum cannot otherwise be laid over it.
- **Silencing — a multiplier 0, not removing the event from the graph** (invariant 3): the silenced
  event still renders, its FX state lives on, and clearing mute continues the reverb tail
  rather than starting it anew (t732b — a guard against an "optimization" of the form "do not compute
  the silenced one").
- **Video: silencing — the flag `VideoClip.silent`, which ONLY the composite plan looks at.**
  `layers_at_into`/`wanted_frames` do not see it, so the prefetch order remains, and clearing
  mute gives a full frame without a cache miss (t735; red if the gate is removed).
- **What does not get into the file is formulated ONCE** — `kt_model::silenced_tracks(reel)`;
  the render window and `kt-cli render/export` print its result before the start (t740, t740b).
  There is no refusal: mute and solo are project content (the owner's decision), but one must not stay silent about them.

### Clarifications during implementation (stage 3)

- **`TrackProcessor` is a SECOND consumer of the same nodes, not a copy of `ClipProcessor`.**
  Half of the clip processor is the material PRODUCER (warp, cursor, "handles", tail gate,
  the automation domain by event length), and a track has none of this: its input
  is already assembled by the summer. What they share is exactly the vector `AudioNode`, so the kernels
  are not duplicated by a single line (t322/t328 green without edits).
- **The stage has no separate tail field.** The track processor keeps running while the
  transport plays, and past the last event it gets silence — the tail comes out by itself (t733).
  A field "how long to sound after the end" would be a second source of the same truth.
- **Lookahead: the input of the summing window is shifted forward by `la`**, the output is returned by the node
  rings — there is no shift relative to a track without a chain (t733b).
- **The track state is a separate map by `TrackId`**, not a composite enum key: a track and an event
  have different processors, and a branch at every access would gain nothing (t734: inserting a track above does not silence its neighbours, removing takes its own away).
- **Video: the stage is a nested layer with a sub-plan of ITS OWN events** (`track_stage_layer`), by
  the same code as a reel: raster and time are the parent's, the step is as for a flat layer. The
  address of the sub-composite is synthetic (`NEST_TAG | TRACK_TAG | id`), the revision is the track's properties
  plus the content of its events, otherwise the memo would return a stale result.
- **The blend mode lives on `LayerSrc`, not in a separate branch of `PlanLayer`**: blending is a
  property of ANY layer. `Normal` does not enter the new kernel at all, and the bytes are as before
  (GOLDEN_VIDEO). The formula `out = lerp(below, blend(below, above), α × level)` is written
  twice — `blend_ch` in kt-engine and its WGSL mirror in `composite.wgsl` — and this is a PAIR: the
  files are edited in one commit, the guard — t737 (CPU against GPU for all nine modes,
  at two levels, the tolerance class of transitions). The mode number travels in the free slot `u1.y`:
  when blending there is no fade, by construction.
- **Reels apply THEIR OWN stages for free**: both the audio and the video stages are assembled from the `tracks`
  of THAT reel for which the renderer is built (t739).

### Clarifications during implementation (stage 4)

- **Hotkeys — ALT+M / ALT+S, not bare M and S (a deviation from tasks.md 4.1).**
  The bare `M` in the program is already taken by the marker (gesture item 142, round 24), the bare `S` — by
  the cut; both were set by the owner's direct request, and a limit or binding set
  at the request does not change without a new request. If we redefined them, a person would lose two
  daily gestures for two rare ones. Therefore mute and solo take a modifier.
- **We have no "selected track" — EVENTS are selected.** The key acts on the track of
  the selected event, and without a selection on the row under the pointer (the very one whose badges are
  highlighted). The decision lives in one function `KtApp::key_target_track`, and the action itself —
  in the shared body `toggle_track_badge` with the menu and the badge (UI rule 17).
- **The track chain lives in the TRACK WINDOW, not in `fx_editor.rs` (a deviation from
  tasks.md 4.3).** A track has six things plus the chain to edit, and spreading them over two
  windows would mean forcing a person to open a second one for a neighbouring field (rule 13).
  The requirement "the same strip widget" is fulfilled literally: the window calls the same
  `widgets::chain_strip` as the event and the morpher. The FX badge and the menu item lead to the same window.
- **A track has no automation widget on purpose**, while inserting a chain WITH ENVELOPES is legitimate and possible at any moment — therefore curves are not silently dropped: they are discarded with
  a named reason and an indication of where to put them (rules 16 and 19). Silence would give
  "the effect is the same, but it sounds different", and a person would go looking for this in the engine.
- **Esc closes the track window with a CASCADE**, like the effects window: edits are applied on the fly,
  and "Undo" can revert them all at once. The window needs no handler of its own — the focus on the name
  field is released by Esc by egui itself at the start of the frame, and the key reaches the cascade with the same press.
- **The highlight of a locked row lives as long as the refusal text** (`STATUS_TTL_MS`), and
  not for one frame. The refusal also comes from a KEY (delete, cut), and `handle_keys` runs before
  drawing the timeline: if the highlight went out at the start of each pass, a person would see it for
  exactly zero frames. Guard — t745.
- **The track verbs in the menu — one body `track_menu_items` for both row menus** (the clip and the service one)
  and for all kinds. Mute, solo and the chain are shown only where they have something to do:
  a dead menu item is forbidden by rule 9 exactly as a dead badge is (t741). The verb Open for all three
  kinds of rows is the track properties window; the former note "there is nothing to open on a track" is removed.
- **The probe name of a badge carries the row kind** (`track.v1.mute`, not `track.0.mute`): V1 and A1 are
  different tracks, and without the kind letter a live scenario would press the first one it finds.

### What the live scenario found (stage 4.7)

Both defects were green in the harness — exactly the case for which the live scenarios were created.

- **The second row of the blend mode buttons overlapped "Switches".** Nine buttons wrap, and `Grid` does not measure the height of the WRAP. The mode and the switches were moved out of the grid
  into their own rows; guard — a check of the RECTANGLES drawn in t743 (by the gutters, not by the accessibility tree: the overlap is visible only in coordinates).
- **The first 8 px of the M badge were dead** — the track headers are pressed against the left edge of the window, and there is the resize zone there (rule 15). The badges are written into `no_resize_rects`; the very edge in
  `WIN_BORDER` remains with the resize, by the decision of item 9 of R.12.

The layer order in the scenario fixture: in OTIO the top track is the LAST in the file, and the first
version of the scenario expected the reverse. This is not a defect of the program but a note for the future: check the two-track fixture by color, not by the order in the JSON.

### State of checks at the end of the change (stage 5.2)

- `cargo test` per crate — green, kt-app 294 of 294 (checked four times).
- `cargo clippy --all-targets` — no new warnings; the remaining ones are older than the round
  (the macro `params_for`, `AddClip(ClipEvent)` in the size of the variant, dead methods
  `reel_preview`).
- Live scenarios — 41 of 41 green.
- **`cargo test --workspace` fails OUTSIDE our code**: the kt-app test binary dies with
  SIGBUS under full parallel load (green alone, and also in pairs with kt-media/kt-engine/kt-gpu). Memory and disk space are plentiful. Exactly two of the longest tests are not reported — t399 (the peak pyramid over the hour-long synthetic) and t640 (the reel preview from the sidecar).
  The defect is older than the round (both tests come from R.49 and R.58) and has been moved to a separate task: SIGBUS is
  a page of a mapped FILE, and one must look for the owner of the mapping, not call it a "flake".
- The scenario `nestparam` was fixed here as well: it aimed at the MIDDLE of the second release, and that one lands under the cursor at the 10th second and lasts 8 s — the middle lies beyond the right edge of the window,
  and the right click went to the WM root menu. This is exactly the trap recorded in R.37
  ("`x_at` can only do what is ON SCREEN"), and the scenario now checks the click point itself.

## Risks / Trade-offs

- [Order of f32 additions] → the stage only where it is needed; t729 holds t2/t3/GOLDEN_AUDIO.
- [A muted track is still decoded] → the cost is like Vegas; an optimization skipping the decode —
  later, the spec does not forbid it.
- [The track PDC widens the summing window — a double render of clips at the chunk boundary] →
  the track's scratch keeps the window tail, like the reel sub-mix ring; t733 measures the cost.
- [A track-as-reel is more expensive than flat layers] → enabled only by the condition; t736 —
  the bytes and the cost without the condition are as before.
- [The gutter width grows] → exactly by the badge gallery, the limit is fixed; t742 checks
  "nothing jumps" and that the mouse hits.
- [Removing `lane_order` affects t49/t241/t588–t594] → the tests are rewritten for the vector
  of tracks with the same assertions (the rank invariant did not change), they are not weakened.
- [The lock as a gate in the model — a skipped command] → a scanning guard: every command with
  an event address passes through one function `target_track(cmd)`; t738 walks the
  command registry.

## Migration Plan

No compatibility is required (the owner). Reading old files — a one-way migration of the
counters/order/names/heights into the tracks. Rollback — git.

## Open Questions

- Is an exclusive solo click (Ctrl+click — one track, as in Vegas) needed beyond the group —
  it is decided in the UI during implementation, the spec does not change.
