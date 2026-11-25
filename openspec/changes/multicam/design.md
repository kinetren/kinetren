# Design: multicam

> DESIGN ONLY, revision 2 (2026-11-25), base `fc60a5d9`. Q1–Q3 decided (A18). Implementation-ready after `shared-events`.

## D0. What revision 1 said and the code contradicts

1. **`ClipEvent.angle` beside `overrides`.** A per-instance choice that changes a reel exists: `Override` on a published handle (R.59).
   Everything an angle needs is keyed by it already: `instance_content`, `NestKey`, the reel preview per instance (`reel_keys`), copy, split,
   paste, `expand_instance` (`overrides_baked`), `.kt`. A second field would have to be added to each, and forgetting one shows another
   angle's picture. An angle is an override (D2).
2. **"The right piece starts its stream at the cut: byte-equal."** A new `ClipId` is a new `NestStream`: a canonical move from a block boundary.
   It equals the continuous play only for chains with a finite memory; an equaliser or a compressor inside rings differently. The stream
   is handed on, as a split of a pitched event is one stream (`split_predecessors`), D3.
3. **Alignment by timecode "(source tc)".** No source timecode is read anywhere (no `AVDictionary` read in kt-ffmpeg but the muxer's). It is
   read here: `timecode` of the format, the video stream, then the `tmcd` data stream (D1).
4. **The cells on the compose thread, class `background`.** Since render-ahead the compose thread only presents while playing, and the
   `Visible`/`Background` classes stand still while a player plays. The cells have a lane of their own (D4).
5. **"A toggle in the preview head", "Alt + digit".** There is no head: the place is the corner row of the preview
   (`preview-canvas-and-gizmos`). Digits are free in every style (`controls.rs`: only Shift+8 in After Effects). Alt+click on a cell is taken by
   window managers: switch without a cut is Shift.
6. **`ClipEvent` in a vector**: the model's events are `Events<ClipEvent>` with `shared-events`; a live pass edits one event.
7. `collapse_into_sequence` already turns a selection into a reel keeping the `ClipId`s; "Create multicam" is it plus rows.

## Context

`published.rs` (`Override`, `ParamOwner`, `apply_overrides`), `resolve_reel` (`project.rs:7152`), `gen_salt`, `instance_content` (`video.rs:502`),
`NestStream`/`split_predecessors` (`fxstore.rs`, `audio.rs:160`), `ops::{collapse_into_sequence :1321, expand_instance :1562}`,
`kt_media::sync::offset_from_pyramids` (two files whose pyramids are ready; `sync_selected_by_audio`, `app.rs:3770`), `Priority::Thumbnail`,
`reel_preview::offline_renderer`, `kt_gpu::side_engine`, `Player::sources`.

## D1. The composition

`Sequence.multicam: Option<Multicam { angles: Vec<Angle { video: TrackId, audio: Option<TrackId>, name }> }>` (not written when `None`). Rules of
`validate`: distinct tracks of the reel; at most one multicam level in a chain of nests (an angle may hold a composition, not another multicam);
at least one angle. A multicam reel publishes two handles by construction, not written in `published`: `ANGLE` and `SOUND` (reserved `PubId`s
at the top of the range, refused elsewhere).

**Create multicam…** (`ops::create_multicam`, one batch) from a selection on at least two video tracks or files:
1. group by media, linked audio joins its video; a file without picture is a **recorder** (a plain audio track);
2. align by the dialog's choice, the first file the reference:
   - **by sound**: `offset_from_pyramids` of each against the reference on the lane `sync` (a job with a progress, the pyramids built first if
     absent: `SYNC_PENDING` becomes a wait, not a refusal); below the confidence the file is listed "not found" with three buttons: leave at its
     place, place by hand later, cancel;
   - **by timecode**: `MediaInfo.start_tc: Option<Tick>` (kt-ffmpeg `Source::timecode()`, drop-frame understood); disabled with its reason when a
     file has none;
   - **by start**;
3. `collapse_into_sequence`, then one row per angle, its chain of effects left on its event; a 1-frame difference left by the audio granule
   (`SYNC_GRANULARITY_TICKS`, 5.3 ms) is rounded to the frame for the picture and kept for the sound;
4. one linked pair of instances (picture, sound) over the union replaces the selection. Angle = the earliest clip's; sound = the recorder if
   there is one, else the first angle's (Q2 a).

## D2. The angle is an override

`ParamOwner::Angle { what: Picture | Sound }`, values `Choice`. The override applies in `apply_overrides` BEFORE morphers: `Picture(n)` drops every
other angle's video events, transitions and tracks' stages with no events; `Sound(Fixed(n) | Track(id) | Follow)` drops the other angles'
audio. `Follow` reads the Picture choice of the event's linked picture instance at resolve time (the sound event carries its own copy of the
number, kept equal by `SetAngle` in one batch, so a resolve needs no partner). No override = angle 0 and the composition's first sound.
Consequences, all existing code: other angles ask no decode and no coverage; keys differ per angle; `gen_salt` is 0 for these owners (no source's
values change), so sources are shared between angles; the instance form ("Composition parameters") draws both as galleries; a person may put
them on a template by `Nested`.

## D3. Switching

- `ops::cut_switch(project, instance, at, n)`: `Split` of the picture instance AND its linked sound at the frame-aligned moment (both, as every
  split of a pair; the right halves get a new group, so a piece can be trimmed or deleted alone), then the override of the right picture
  piece (and of the sound when `Follow`). On a boundary: only the override. Inside a transition's window: the nearest legal frame, said in words.
- `Command::SetAngle { id, what, value }` = `SetOverride` of the reserved handle.
- **The sound does not notice a cut**: `NestStream` of a right piece is handed the left one's when the right continues it (same lane, abutting,
  same `NestKey`, `source_in` continuing, no reverse, warp, overlap or transition at the join, the event chains equal): `sync_fx` moves the stream by the
  rule of `split_predecessors` without its `ctx` clause, and the instance's own chain through `hand_on`. With it the cut is byte-equal to no cut
  whatever rings inside (t3509). A seek warms it as any stream.
- **Live**: a switch while playing lands at the SHOWN frame (`video_playhead`); the batches of one Play…Stop are one undo step
  (`amend_batch` of `ripple-and-trim-modes`, brought by whichever lands first). The player is told per switch as of any edit.

## D4. The angle viewer (Q3 a)

A corner button "Angles" of the preview, only while the playhead stands on a multicam instance: the preview area splits into the program
picture and a grid 2×2…4×4 (the split drags, remembered). The cells:
- `AngleCells` (kt-engine): one `VideoRenderer` per angle of the reel-as-root with that angle (built once per revision through the build memo), composed
  at ⅛ of the raster on `sibling_exec` of the display (a card) or `side_engine`, read back to a texture each;
- a lane `angles` of class `Near` with a budget: a cell is composed only when the program's moment is kept or composed and the near prefetch has nothing
  pending; decode asks `Priority::Thumbnail` at the cell's size for the shown moment and ½ s ahead, withdrawn when the moment leaves;
- a cell without its frame keeps the former one dimmed; the program frame never waits;
- paused: each cell once per (moment, revision); sixteen cells of 1080p are 8 MB in the ledger (`Tenant::Angles`, the next number).
The active cell has the selection stroke, its name and number are under it. Click = cut and switch; Shift+click = switch; digits 1–9 and
Shift+digit the same, only while the playhead is on an instance and no text has the focus (commands `Angle1…9`, the next free numbers; an unbound
digit elsewhere does nothing). The scopes measure the program picture.

## D5. Expand, flatten

"Expand multicam" on pieces: `expand_instance` over the resolved reel (the override is baked), the angle's track chain moves by that function's rule for a
reel's track stage, a refusal names it. Nothing else.

## D6. Format and OTIO

`.kt`: the next free version by content (`Sequence.multicam` or a reserved override). OTIO as revision 1: stacks with the active angle's tracks enabled, the
selection in `metadata["kt"]`; the row stands.

## D7. Class walk: "a choice among alternatives that must cost nothing when not shown"

| Place | Here |
|---|---|
| decode, prefetch, coverage, prerender, copies (`persistent-cache`) of hidden angles | none: not in the resolved reel. A copy is made for a file the preview SHOWS, so hidden 4K angles get none |
| generators and titles inside an angle | sources are raised for every reel's content (`gen_sources` walks reels): hidden ones are opened, never read; unchanged |
| `AngleCells` | D4 |
| reel poster and strip | the first angle |
| morphers over an instance, transitions at a cut, track duck keys | as any event |
| `frame-format-variants` | a correction belongs to the piece; a split carries it |
| export | one angle per piece; `stream_preroll` as any reel |
| `multicam` + `simulated-elements`, `motion-driven-params` | links into a reel's published handles skip the reserved ones |

## D8. Idle

Nothing runs without the viewer; with it, a pause composes each cell once. Live `idle`: the viewer open on nine paused angles, the mouse over the grid.

## Rejected

- The morpher's UI (he forbade it), takes on events, raw cells.
- A field of its own for the angle (D0.1).
- Not splitting the sound: deleting a piece of a pair deletes its group, and a group of one sound and many pictures would delete the whole
  instance.
