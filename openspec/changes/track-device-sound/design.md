# Design: track-device-sound

## Context

- Events: `sync_pairs_to_link_group` (kt-model `model.rs`) mirrors a paired kind's slot onto every
  linked event of the other kind; `mirror_halves` is the one n-th-occurrence rule, shared with the
  morpher patch (fx-graph D2).
- Tracks: a track has a chain (`Track.fx`, `fx_bypass`, `fx_off`, no curves — `SetTrackFx` refuses
  them). An audio track with a chain is a stage (`TrackStage`, kt-engine `audio.rs`): its events are
  summed, then `kt_fx::TrackProcessor` runs the chain at TIMELINE frames. A video track's chain runs
  over its events' composite in the plan, where a device's picture half reads `burst_index` — the
  timeline frame.
- `TrackProcessor` never calls `AudioState::place`: its nodes kept `Placement::default()` (start 0,
  25 fps). Harmless until now — no node on a track read it.

## D1. The link: `Track.sound`

- A field of the VIDEO track: `sound: Option<TrackId>`, written when set. It is a track PROPERTY
  (`TrackProps.sound`), so the track window's existing session, its Cancel and `SetTrackProps` carry
  it; no new command (the `Command` matches stay as they are).
- Valid: the target is an audio track of the same track list (the root's or the reel's), and no other
  video track names it (one-to-one: two video tracks mirroring into one chain would fight over its
  n-th slot). `SetTrackProps` refuses anything else with `TrackPropOutOfRange`; `Project::validate`
  refuses such a file with the track's address.
- **Default** (kt-app rule 4: the useful default; rule 20: nothing grows back): when a `SetTrackFx`
  gives a video track its FIRST paired slot (it held none before) and the track has no sound track,
  the model links the audio track of the same number, if one exists and is free. A link the person
  cleared while the device stays is not re-made by later edits — only a device added to a track
  without any brings the default again.
- **Removal**: removing the sound track clears the link (`remove_track`); removing the video track
  removes the link with it, the halves stay on the audio track as ordinary slots (the same as
  deleting a video event leaves its partner's halves — they are sound effects).

## D2. The pair rule for tracks

One rule, the event's: `sync_pairs_to_sound_track(tracks, id)` after `SetTrackFx` and after
`SetTrackProps`.

- `SetTrackFx` on a linked video track: `mirror_halves(video → sound track)`.
- `SetTrackFx` on an audio track that is some video track's sound: `mirror_halves(audio → video)` —
  the sound half's values reach the picture (the deck is one), its removal takes the picture half.
- `SetTrackProps` that changes the link: the old sound track loses its paired slots
  (`mirror_halves` with nothing leading), the new one gets the video track's halves.
- Bypass is each side's own; tracks have no curves, so only values are mirrored.
- The same apply step, so one undo step (`apply_coalesced` keeps the window's session as one).

## D2a. Datamosh does not pair on a track

The engine skips a stream slot on a track stage (`video-fx`: "A stream slot on a reel instance or a
track stage SHALL be skipped" — the stage decodes an EVENT's stream). Datamosh on a video track
therefore does nothing to the picture, and its sound half reads the event layout it does not have.
`track_pairs` keeps stream devices out of the track rule (both directions); VHS Tape and Camera ISP
pair. The owner's answer names all three; Datamosh on a track needs a track-level stream stage first
— recorded as found, not fixed.

## D3. The engine

- The sound half on the audio track is an ordinary audio plugin slot of a track stage: built,
  diffed and reset by `FxStateStore` as any track node (invariants 2, 3).
- `TrackProcessor` seats its nodes at `opts.placement(0)` when it builds them (structural
  `apply_params`): the track's `pos` is the timeline frame, so `start = 0` and the project's frame
  grid give the timeline moment the picture half reads (sim-audio-pairs D3: each half derives the
  device from rows + time, never from the other half's state). Inside a reel both tracks are the
  reel's, in the reel's time.
- Nothing else changes: the video track's picture half is untouched by the sound half's switch.
- Measured: the VHS and ISP sound halves read only `Placement.start` (0 on a track, which the
  default already gave); Datamosh's sound reads the fps grid, and it does not pair on a track. The
  seat is kept for any later half that reads the grid. t3232 holds the track's deck against the
  event's to float rounding: the stage sums and chunks its own way, never to the bit.

## D4. The window

- A video track's window with a sound track draws ONE graph (fx-graph D10's pair editor, rule 17):
  the video track's chain on the picture lane, the sound track's chain on the sound lane, a switch
  per lane at its In node, the device's halves in one column. Split back by lane; an unchanged side
  sends nothing (D10's lesson: a stale side re-pairs a removed device). The sound track's edits join
  the window's session and its Cancel (as the ducking key track's do).
- "Device sound" row (a combo: the free audio tracks of the reel, and "None"), shown while the chain
  holds a paired non-stream device or a link exists (rule 9: no dead control).
- Cancel: the window snapshots every audio track's chain at opening — the model links the default
  track or the row picks another, and the halves land there before the window could snapshot it
  (the harness found the first form, a snapshot at the first edit, restoring a chain that already
  held the half).
- An audio track that is a video track's sound shows its chain as before; its halves carry the
  speaker mark.

## D5. File and OTIO

- `.kt`: `"sound": <track id>` on the video track; a project with one is format 6 (the next free
  number on `6f107f06`); an older build refuses it naming the version, instead of dropping the link
  and leaving the halves unmirrored.
- OTIO: the track envelope `metadata["kt"]` carries `sound`; v17 by content; a row in "What OTIO
  export loses" (a foreign editor sees two independent tracks, the sound half an unknown effect).

## The class walk

The mechanism: "a device's sound needs a partner; a carrier without one is silent". Walked:

| Carrier | Partner | State |
|---|---|---|
| event | linked audio event | done (sim-audio-pairs) |
| morpher patch | the patch's sound chain | done (fx-graph v1a) |
| composition instance | an event: the event rule | done |
| video TRACK | none → the sound track | this change |
| track inside a reel | the reel's tracks | this change (same list) |
| generator event with a device | its linked audio, if any; a generator has no audio partner | unchanged: picture-only, as before |
| bake of a morpher | real slots on pieces → the event rule | done |
| preset / paste into a track chain | `SetTrackFx` → this rule | covered |

## Risks

- The format number 6 is also named by fx-graph-dag's design (not built). Whoever merges second
  takes the next number.
