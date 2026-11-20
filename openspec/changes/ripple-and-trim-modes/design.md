# Design: ripple-and-trim-modes

> DESIGN ONLY, revision 1 (2026-11-20), base `ec30cb02`. The owner's questions Q1–Q3 are in
> `proposal.md`; the defaults below follow the recommended options and change with his answers.

## Context

Read on `ec30cb02`:

- **Commands** (`src/libs/kt-model/src/model.rs`, `Command`): `MoveClip` moves a clip AND its link
  partners by one offset (`mutate_on`); `TrimStart`/`TrimEnd`/`Split`; `MoveMarker { from, to }` refused
  with `MarkerExists` on an occupied tick; `SetAreaRange`, `SetMorpher`, `SetCamera`. No command moves a
  `ForeignEvent` or sets `timeline_end`.
- **Region operations** (`src/libs/kt-model/src/ops.rs`): `delete_region`, `punch_out_region` — the
  latter `RemoveClip`s each touched clip and `AddClip`s two remainders with new ids, so a remainder's
  effect state (reverb tails, the stream history) restarts (invariant 3 is kept only "by id", and a new
  id is a new chain). `C` (region-cut-key) calls it.
- **Batches**: `apply_batch` simulates the whole series and writes one undo step; `apply_batch_restoring`
  adds `restore_handles` (material cut by a dying transition comes back, clamped by the freed space).
- **Transitions store no time** (`a`, `b`, offsets): moving both neighbours by one delta moves the
  transition; `reconcile_transitions` leaves it.
- **Overlaps are legal** (`overlap_layers`, default on): an overlap crossfades by itself.
- **The material's length is known** once opened (`MediaRef.duration`, R.63) — the slip stop.
- **Keys are glued to the material** (`AutoKey.t` is a material tick, R.33): a slip moves the
  animation with the frames (Premiere's behaviour, F-TL-002 trick 4), with no key edits.
- **The UI**: `plan_move` (`src/apps/kt-app/src/timeline.rs`) classifies drags; the transition roll
  (`timeline.rs`, "Roll of a transition joint") is the pattern for a plain roll; the loop button in
  `toolbar.rs` is the three-state pattern; keys live only in `controls.rs` (t1715).

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
  move while its referenced window does not, refuse with `LayerDetached { morpher }` (Q2 b fallback; under
  Q2 a `refs` is always true and this cannot occur).
- **Order with handles**: a rippled delete computes `restore_handles` first (on the `preview` of the
  delete), and `delta` = the length actually freed after the handles came back — otherwise the tail
  lands short by the handle's length (F-TL-001 "What gets in the way").

The plan is pure (a function of the project) and is validated as a whole by `apply_batch`; one
refusal leaves the project untouched.

## D2. Post-edit ripple

`Cmd::RippleAfterEdit { scope }` (three rows in `controls.rs`, Vegas's F / Ctrl+F / Ctrl+Shift+F in the
Vegas layout where free, our layout's keys chosen against the table, conflicts guarded by t1715).
The model remembers the last edit's `EditFootprint { at, freed: Tick, tracks }` (set by delete, region
cut, trims, paste, cut; cleared by any other command or by undo). The command ripples by it and joins
the SAME undo step (`apply_coalesced` with the edit's gesture token), so Undo removes edit and ripple at
once. With no footprint the item is disabled with the reason ("nothing to ripple: the last action freed
no time").

## D3. Auto-ripple mode

`UiState.ripple: Option<RippleScope>` (outside undo, `None` not written). The toolbar button cycles
off → affected → all (refs per Q2) with a drop-down; `Cmd::AutoRipple`. While on: the selected events'
outline and the playhead are drawn red (the already occupied places, UI rule 13), the status line's mode
hint says where to turn it off (rule 19). Triggers: Delete, Cut, region cut `C`, paste/insert, edge
trims (Q3 a: not moves). Each trigger builds its normal batch and appends `ripple_plan` in the same
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

- **Roll** (plain cut): a drag at the joint of two butt-joined events with the roll modifier: `TrimEnd`
  of the left and `TrimStart` of the right by the same `d`, clamped by both materials and by
  `MIN_CLIP_DURATION`; the transition roll stays as is.
- **Slip** (body drag with the slip modifier): `source_in += d` with start and duration fixed — a new
  `Command::Slip { id, delta }` (the trim pair would need two clamps and a temporary shape); clamped by
  `0` and `MediaRef.duration`; the link group slips together (the A/V pair stays in sync); a warped clip
  slips along consumption (`clip_slice` rule). The stop is drawn red at the edge that ran out (FCP's
  only-visible-stop, F-TL-002).
- **Slide** (body drag with the slide modifier): `MoveClip(d)` + the left butt neighbour's `TrimEnd(+d)` +
  the right one's `TrimStart(+d)`; a neighbour that is not butt-joined is not touched; clamped by the
  neighbours' material. Explicit modifier only (rule 20: it changes what was not dragged).

## D6. Close gaps (F-TL-008)

`ops::close_gaps(selection)`: per track, the selected events sorted by start, each moved left to the end
of the previous event on that track (selected or not); one batch. With "ripple" on, the events after the
last selected follow by the total. An empty selection with the playhead on a track = that track.

## D7. Format, OTIO, idle

- `.kt`: `UiState.ripple` only; the edits are ticks. No version change (`version_for` untouched).
- OTIO: nothing to lose (UI state).
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
