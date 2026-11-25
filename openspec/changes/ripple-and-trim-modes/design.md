# Design: ripple-and-trim-modes

> DESIGN ONLY, revision 2 (2026-11-24), base `229c694b`. Q1 a decided (A9, his check C9). Q2 and Q3 were never put to him:
> asked anew in `proposal.md`, with computed consequences; they set two defaults and hold nothing. Implementation-ready.

## D0. What revision 1 said and the code contradicts

1. **"The VEGAS layout uses VEGAS's keys".** There is no VEGAS style: the five are ours (`OWN`, the owner's own, Vegas-like),
   Premiere, Resolve, Final Cut, After Effects (`controls.rs:417–585`). Ctrl+F is Find in all of them, so of Vegas's F / Ctrl+F /
   Ctrl+Shift+F only F and Ctrl+Shift+F are free. They are bound in `OWN` (a new row of his table, t1700's list grows); the
   others get no key and reach it by menu and the key dialog.
2. **Vegas's Alt+drag for slip.** Alt+drag belongs to the window manager on many Linux desktops (and on the live stand's
   openbox), the reason the wand took Ctrl+click. Slip is Shift+drag or Alt+drag, whichever the desktop leaves (D5).
3. **Locked tracks** (R.62) were not in the plan: `check_on` refuses the whole batch with `TrackLocked` for any row a command
   touches (`model.rs:1926`). A ripple over all tracks with one locked track would always be refused. D1 says what happens.
4. **Bands and reels.** With a composition expanded in place its events stand in the same `p.clips`, on rows at the tail, in the
   root's time; a ripple "over all tracks" would shift the events inside the composition AND the instance. Markers, areas,
   foreign events and `timeline_end` belong to the root only (`Sequence` has none). D1 scopes by reel.
5. **`EditFootprint` in the model and `apply_coalesced`.** `apply_batch` clears `last_gesture` (`model.rs:1658`), so a later command
   cannot join its step, and `apply_coalesced` takes one command. The model gets `amend_batch` (D2); the footprint is the app's.
6. **Two regimes of `C`.** `delete_region` with a region removes the WHOLE crossing events (and partners), `C` cuts the interior:
   both stay, each with its ripple.
7. A morph patch's source window, `FrameHold` and `SetGenSound` exist now; none moves time, so none changes the rule.

## Context

Read on `229c694b`: `Command` (`model.rs:486`: `MoveClip` moves the link group, `TrimStart/End`, `Split`, `MoveMarker`,
`SetMarkerDuration`, `SetAreaRange`, `SetMorpher`, `SetCamera`; none moves a `ForeignEvent` or sets `timeline_end`),
`simulate` (bands), `target_rows`, `ops::{punch_out_region :572, delete_region :603, restore_handles :662, paste_block :426}`,
`KtApp::{apply_batch_restoring app.rs:4839, delete_selected :7036, delete_action, punch_out_action :6136, paste_at :5956,
reel_of_clip :4721}`, `timeline.rs` (`plan_move :401`, `event_zone :234`, `Drag :618`, Ctrl = the magnet off, Shift+double click =
the region of the event), `controls.rs` (`Cmd` up to 37, `PreviewFit = 38` by `preview-canvas-and-gizmos`),
`MediaRef.duration`, keys in material ticks (R.33), legal overlaps (R.28).

## Goals / Non-Goals

**Goals:** shift-after-edit in one undo step with every time-anchored object following; identities
kept; refusals with the obstacle named; the four classic trims; close gaps by selection.

**Non-Goals:** FCP's magnetic timeline (incompatible with legal overlaps, F-TL-001 verdict); per-track
sync locks (the link group already holds A/V); "close all gaps in the project"; ripple of a speed change
(F-SPD-001 will call `ripple_plan`, not a second rule); a two-up trim preview (its own note).

## D1. One shift rule: `ripple_plan`

```text
ripple_plan(project, at: Tick, delta: Tick, scope: RippleScope, edited: &[TrackKey]) -> Result<Vec<Command>, RippleRefusal>
RippleScope = { tracks: Affected | All, refs: bool }      // refs = markers + named areas
```

- **One reel**: the plan is of the reel the edited events stand in (`reel_of_clip`): the root, the entered reel, or the band's;
  other rows of the bench are neither moved nor obstacles. `refs`, foreign placeholders and `timeline_end` take part only at
  the root. A ripple inside a band whose composition is placed elsewhere keeps the existing "shared" warning.
- **Locked tracks stay**: a locked track is out of every scope, and the status line names it ("Music is locked and did not
  move"). A link group with a member on a locked track refuses (`LockedPartner`), since the pair would part.

- **Who moves**: every event of a track in scope whose `start >= at`; each link group is moved once
  (by its member with the smallest id — `MoveClip` moves the partners), and a group with a member on a
  track OUT of scope moves anyway and is reported in the refusal list if that member would collide
  (the pair never splits: link groups are sync by construction).
- **The boundary is vertical**: `at` is one tick for every track — Vegas's per-track "edge of the edited
  event" desyncs a pair that was a frame off (F-TL-001 trick 2).
- **Spanning events** (`start < at < end` on a scope track, not the edited one): with `delta < 0` the
  shift would overlap them → `RippleRefusal::Spanning { clip }` (Q1 a). The UI highlights it in the error
  style and names it in the hint (rule 16, the t172 class).
- **Foreign placeholders** after `at` move by the new `MoveForeign { id, start }`; one spanning the point
  refuses (`ForeignOverlap` stays a law). `timeline_end`, when set and `>= at`, moves by
  `SetTimelineEnd`.
- **Markers** (when `refs`): every marker `>= at`, ordered ascending for `delta < 0` and descending for
  `delta > 0`, so none runs into a neighbour that has not moved; a marker that would land on the tick of
  an unmoved marker refuses (`MarkerCollision`), never merges (a marker's address is its tick).
  Range markers and areas that start `>= at` move whole; those spanning `at` stretch or shrink by
  `delta` at their end (`SetMarkerDuration`, `SetAreaRange`) — a region over the cut keeps covering the
  same material.
- **Morpher blocks and camera events** on scope tracks follow the clip rule (start `>= at` moves;
  spanning stretches by `delta` via `SetMorpher`/`SetCamera`); a camera track is in scope when `All`.
- **Reference-mode layers** (`MorphTargets::Marks`/`Areas`): if `refs` is false and a layer's targets
  move while its referenced window does not, refuse with `LayerDetached { morpher }`. The default until Q2 is answered:
  `refs` = (tracks == All).
- **Order with handles**: a rippled delete computes `restore_handles` first (on the `preview` of the
  delete), and `delta` = the length actually freed after the handles came back — otherwise the tail
  lands short by the handle's length (F-TL-001 "What gets in the way").

The plan is pure (a function of the project) and is validated as a whole by `apply_batch`; one
refusal leaves the project untouched.

## D2. Post-edit ripple

The app remembers `last_edit: Option<EditFootprint { at, delta, tracks, revision }>`, set by its own edit bodies (delete,
delete by region, `C`, cut, paste, edge trims) from what they computed; it is valid while `model.revision() == revision`,
so any other command, undo or redo makes it stale without a hook. `Cmd::RippleTracks`, `Cmd::RippleAll` (the next free
numbers, 39 and 40 after `PreviewFit`) run `ripple_plan` by it and `TimelineModel::amend_batch(cmds)`: `apply_batch` that
replaces `project` without `push_undo` when the top of the undo stack was made by the batch of `revision` (else an ordinary
step). One Undo gives back the edit and the ripple. Stale or absent: the item is disabled and its hint says why ("nothing
to ripple: the last action freed no time").

## D3. Auto-ripple mode

`Workspace.ripple: Off | Tracks | All` in `user_store` (a tool's mode, the person's: UI rule 23; it starts Off in every
session, as a mode that moves what is off the screen must be armed knowingly). The toolbar button cycles off → track →
all; `Cmd::AutoRipple` (41), unbound. While on: the selected events'
outline and the playhead are drawn red (the already occupied places, UI rule 13), the status line's mode
hint says where to turn it off (rule 19). Triggers: Delete, Cut, region cut `C`, paste/insert, edge
trims; a move does not, until Q3 says otherwise. Each trigger builds its normal batch and appends `ripple_plan` in the same
batch — one undo step.

## D4. Identity-keeping ripple variants (invariant 3)

- **Rippled region cut**: for a clip spanning the region `[a, b)`: `Split` at `b` (the right half gets
  the next id by Split's rule and keeps the link group), `TrimEnd` of the left part to `a` (keeps its
  id), then `ripple_plan(at = b, delta = a − b)`. Clips wholly inside are removed. No remainder is
  re-created.
- **Rippled delete of selected events**: per track in scope the freed span is the union of the deleted
  events' spans; a ripple shift per contiguous freed span, right to left, in one batch.
- **Ripple trim**: dragging an event's END by `d` shifts what follows by `d`; trimming its START
  inward by `d` keeps the event's start where it was and pulls the event and the tail left by `d` (as
  Vegas does): `TrimStart(+d)` + `MoveClip(−d)` + `ripple_plan(at = old end, −d)`.
- **Insert paste**: `ripple_plan(at = paste point, +block length)` before `paste_block`.

## D5. Roll, slip, slide

Modifiers are read at the PRESS (a modifier pressed later changes nothing), from `Event::PointerButton.modifiers`.

| Gesture | Where | Command |
|---|---|---|
| **Roll** | Shift or Alt + drag on an edge where two events are butt-joined, no transition there | `TrimEnd(a, +d)` + `TrimStart(b, +d)` in one batch; clamped by both materials, `MIN_CLIP_DURATION`, the link groups of both |
| **Slip** | Shift or Alt + drag on the body | the new `Command::Slip { id, delta }`: `source_in += δ` for the link group, start and length fixed; δ by the clip's own map (`clip_slice`'s consumption: a warped or reversed clip slips along what it reads) |
| **Slide** | Ctrl+Shift or Ctrl+Alt + drag on the body | `MoveClip(d)` + the left butt neighbour's `TrimEnd(+d)` + the right one's `TrimStart(+d)`; a neighbour that does not touch is not touched; clamped by the neighbours' material; the magnet stays on (Ctrl here is the slide's) |

- **Stops**: `0` and `MediaRef.duration` (a composition: its content's length; a still, a generator: no stop). The edge that ran
  out is drawn red and `HintKind::Error` says "the material ends here" (rule 16). While slipping, the event's filmstrip and
  waveform draw from the live `source_in` (the one map `clip_material_pos`), no command until the release.
- Keys are glued to the material and move with the frames; the region, markers and neighbours do not move.
- Shift+click and Ctrl+click on an event keep their meanings (a click is not a drag); Shift+double click too.
- The zone order of `event_zone` is unchanged: with the modifier a fade or level zone gives the edge or the body.
- `docs/guide/shortcuts.md`, the timeline table: three rows.

## D6. Close gaps (F-TL-008)

`ops::close_gaps(selection)`: per track, the selected events sorted by start, each moved left to the end
of the previous event on that track (selected or not); one batch. With "ripple" on, the events after the
last selected follow by the total. An empty selection with the playhead on a track = that track.

## D7. Format, OTIO, idle

- `.kt`: nothing; the edits are ticks. OTIO: nothing to lose.
- Idle: model-only; the indicator is static colour — no repaint while nothing moves (invariant 12). The
  live `idle` scene runs with the mode on.

## D8. Class walk — "an edit whose consequence lies outside the view"

| Place | Mechanism | What this change does |
|---|---|---|
| ripple delete / cut / region cut | tail moves off-screen | status line names the count moved and the farthest tick; refusals scroll-hint to the obstacle (rule 16) |
| paste insert | same | same |
| transitions at the boundary | `reconcile_transitions` | both neighbours move → the transition survives (t3420) |
| morpher reference windows | name → window | D1 refs / refusal |
| camera events | `active_camera_at` | follow the clip rule |
| foreign placeholders | no command today | `MoveForeign` |
| `timeline_end` | no command today | `SetTimelineEnd` |
| markers with notes | tick address | ordered moves, refusal on collision |
| loop region / render region | `Project.region` outside undo | NOT moved (a region is a view choice, not content); stated in the hint |
| locked tracks | `TrackLocked` for a whole batch | out of scope, named (D1) |
| an expanded band | its events on the same bench | one reel (D1) |
| `frame-format-variants` framings, `multicam` angles | follow their events | nothing of their own |
| curves linked to another event | read the leader's time | both move or the link's shift stays as a move leaves it today |
| speed change (F-SPD-001) | changes length | will call `ripple_plan`, no second rule |
| copy-paste blocks (`ClipBlock`) | rebased on paste | insert uses D1, the block's own rule unchanged |

## Risks

- A link group spanning tracks in and out of scope: moved as a whole (sync first), reported if blocked.
- Large projects: `ripple_plan` walks events after `at` once per track; the batch can hold thousands of
  `MoveClip` — `apply_batch` validates once (builder-cost-guard: the plan uses `Indexed`).

## Rejected

- **Magnetic timeline** — contradicts R.28 legal overlaps and rule 20 (F-TL-001).
- **Per-track sync locks** — the link group already keeps pairs; Premiere's asymmetric locks are a
  known complaint (F-TL-001).
- **Re-creating remainders** (today's `punch_out_region`) under ripple — breaks invariant 3.
