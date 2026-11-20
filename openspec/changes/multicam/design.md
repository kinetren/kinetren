# Design: multicam

> DESIGN ONLY, revision 1 (2026-11-20), base `a30c134d`. Q1–Q3 in `proposal.md`; the defaults follow
> the recommended options.

## Context

Read on `a30c134d`:

- **Compositions** (R.50–R.59, `compositions` spec): `Sequence` with its tracks, raster and fps; an
  instance is a `ClipEvent` whose media is the reel; `Project::resolve_reel(seq, overrides)` is the one
  door; `expand_instance` (`src/libs/kt-model/src/ops.rs`) unpacks an instance into its events; the
  engine's sub-plan of a reel and its audio sub-mix keyed by instance (`NestStream`, a stream per
  INSTANCE `ClipId`, born at its first read).
- **Sync** (R.49, F-SPD-004): `kt_media::sync::offset_from_pyramids` gives the offset of two recordings by
  their peak pyramids; the offset is stored in the sidecar (`index.rs::set_sync_offset`).
- **The morpher** swaps a source over a window (`MorphPatch.media`, cutting partially covered targets);
  F-TL-005 named it the closest relative.
- **Decode**: a worker per media; `Priority::{Playback, NearPrefetch, FarPrefetch, Thumbnail}`; the
  compose thread composes the preview (`compose_queue`), classes via `kt_time::sched`.
- **Coalescing**: `apply_coalesced` keeps a live gesture as one history step by its token.

## Goals / Non-Goals

**Goals:** sync and group cameras in one step; switch live with digits; per-angle grading; sound that does
not jump; one undo step per pass; no extra decode for angles that are not shown.

**Non-Goals:** automatic switching by the active speaker (F-TL-005 trick 4, a later assist); nested
multicams; more than 16 angles in the grid (more are allowed, switched from the menu).

## D1. The multicam composition (Q1 a)

`Sequence.multicam: Option<Multicam>`; `Multicam { angles: Vec<Angle> }`, `Angle { video: TrackId,
audio: Option<TrackId>, name: String }` — an angle is one of the reel's video tracks with its paired audio
track. The reel is otherwise an ordinary composition: its tracks, events, effects, track stages. Rules
(validate): angle tracks are distinct; an angle's video track has no overlap with another angle's inside
the multicam (each camera its own row); the multicam's own raster is the project's unless set.

`CreateMulticam` (an ops batch, one undo step) from a selection of clips:
1. group by camera: clips sharing a media are one angle; linked audio joins its video's angle;
2. align: by sound (`offset_from_pyramids` against the first angle's audio; refused with the pair named if
   an angle has no sound or the correlation is below the threshold), by timecode (source tc), or by start;
3. build the reel (a track per angle, events at their aligned offsets), name angles by file name;
4. replace the selection by one instance spanning their union (on the lowest selected video track and its
   audio), the instance's angle = the angle of the earliest clip, audio per Q2.

## D2. The instance's selection

`ClipEvent.angle: Option<AngleSel>`; `AngleSel { video: u16, audio: AudioSel }`, `AudioSel = Fixed(u16) |
FollowVideo | Track(TrackId) /* a non-angle track of the reel: the recorder */`. Legal only on an instance
of a multicam reel. `resolve_reel` with a selection drops the other angles' video tracks (and, for
`Fixed`/`Track`, every other audio track of the angles) — so the sub-plan composes one angle and the
sub-mix plays the chosen sound. Because each piece's resolved reel differs, each is its own `NestKey`
(instance content includes the selection, `instance_content`): the memo and the coverage stay correct
(invariant 4).

**Sound across cuts.** With `Fixed`, two adjacent pieces read the SAME audio tracks at continuous reel
time: the audio sub-mix of the right piece starts its stream at the cut (a new `ClipId`) — a reel without
tail effects has zero pre-roll and the canonical move (the nested producer, law C4) yields the same
samples as the left piece would have: no seam (t3509 compares the bytes across a cut with the uncut
instance). With tail effects in the angles, the pre-roll covers them (the reel's own rule).

## D3. Switching

- **Cut & switch** (digit, click on a cell): `Split` of the instance at the playhead (frame-aligned,
  outside a transition window — the forbidden-zone rule pushes it to the nearest legal frame inward) +
  `SetAngle` on the right piece. If the playhead stands exactly on a piece boundary, only `SetAngle` of
  the right piece.
- **Switch** (Alt + digit / Alt + click): `SetAngle` of the piece under the playhead.
- **Audio mode**: the instance's right-click menu; applies to the selected pieces.
- **Live**: while playing, switches are applied immediately (the model edit is safe under invariant 3 —
  the sub-mix stream of the left piece continues, the right piece's starts), under one gesture token from
  Play to Stop: one undo step per pass. The switch position is the DISPLAYED frame (`video_playhead()`),
  so the cut lands where the person saw it.
- **Ripple / trim / move** of pieces are ordinary edits of events (instance pieces are events).

## D4. The angle viewer (Q3 a)

A toggle in the preview head ("Angles") when the playhead stands on a multicam instance (disabled with
the reason otherwise). The preview area splits: program picture + a grid (2×2 up to 4×4 by angle count).
Each cell is a plan of the multicam reel at the instance's reel time with that angle selected, at 1/8 of
the project raster: the one plan code (invariant 7) with the angle's own effects. Cells are composed by
the compose thread after the program frame, class `background`, their decode asks at `Priority::Thumbnail`
(declared, unused today, F-TL-005) — so a starved cell shows its previous frame dimmed, never the program
frame waiting for it. Playing: cells request at most the moments the program frame shows (no look-ahead of
their own beyond 0.5 s). Paused: each cell composes once per moment and is kept by the present memo key.

Memory: N cells × (1/8 raster) — 16 angles of 1080p ≈ 16 × 0.5 MB per moment; the frames cache's sizing
counts them (frames-cache-growth: `size_frames_cache` adds the cells' window bytes).

## D5. Expand

"Expand multicam" on selected pieces: each piece becomes its active angle's events (the existing
`expand_instance` over the resolved reel with the selection) + the chosen audio. Per-angle effects on the
angle's track stage go with the expanded events by the rule `expand_instance` already has for a reel's
track stages; where that rule refuses, the expand is refused with its reason (nothing is silently lost).

## D6. Format and OTIO

`.kt`: the next free version by content (`has_multicam()`). OTIO: the reel is a nested Stack (as today);
an instance piece exports its stack with only the active angle's tracks `enabled = true` and the
selection in `metadata["kt"]`; a foreign editor plays the right angle per piece but has no multicam
(a row in "What OTIO export loses").

## D7. Class walk — "a choice among alternatives that must not cost when not shown"

| Place | Today | Here |
|---|---|---|
| inactive angles in the edit | n/a | dropped by `resolve_reel` → no decode, no prefetch, no coverage |
| angle cells | n/a | Thumbnail priority, background class, 1/8 size, bounded look-ahead |
| morpher source swap | swaps over a window | not used for multicam (Q1 a); a morpher over an instance still works as on any event |
| reel previews in the bin | content hash | the multicam reel's poster shows angle 1 |
| coverage bars of an instance | reel coverage | of the active angle per piece |
| the audio sub-mix per piece | stream per instance | zero-seam across a cut (D2) |
| scopes / formats | presented / target | the program picture; cells are not measured |

## D8. Idle

Nothing runs at rest: on a pause the grid composes each cell once per moment; hovering the grid draws
cached textures. The live `idle` scene adds the angle viewer open on a paused 9-angle multicam.

## Rejected

- **Through the morpher's UI** — the owner forbade it ("не … в тупую").
- **Takes on events (Vegas)** — no per-angle grade; sync per event.
- **Raw cells by default** — chosen by what the camera will look like (Q3 may reverse).
