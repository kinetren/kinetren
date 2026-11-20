# Proposal: track-stream-stage

> DESIGN ONLY (spec session). Code starts after the owner answers the questions at the end and the
> orchestrator gives the go. Tests reserved: t3379–t3398. Base: `orch-26-11-01` at `b2653fcc`.

## Why

found-defects 820: Datamosh on a video TRACK does nothing. The engine skips a stream slot on a track
stage, so track-mosh-and-start-wait made the track window refuse it with a reason (t3237). Its design
D1 named what is missing.

A stream stage today decodes an EVENT's own stream:
- the event's source frames at the codec's size;
- the event before it on the lane as the history;
- another lane's event as the second input.

On a track, the stage's input is the track's COMPOSITE at past moments. Nothing builds that today.

This is also what a person means by "mosh my edit". The classic datamosh is done on a RENDERED edit
(Avidemux on the exported file; After Effects' Datamosh 2 renders the composition and moshes the file),
so the history crosses the EDIT's cuts with whatever the editor composed there: titles, transitions,
effects. An event-level mosh imitates this through "the event before". A track-level mosh is it.

## What Changes

- **A second host of the same kernel.** `TrackStream` implements `StreamLayout` and `StreamInput` for a
  video track stage. The datamosh kernel, its memo, its checkpoints, its device walk and its sound do
  not change.
- **The track is laid out as segments at its cuts.** A segment starts where an event of the track begins
  and ends at the next such start. Its history starts `before` positions inside the previous segment, as
  an event's starts in the event before. A gap ends the stream. One function gives the layout to the
  picture and to the sound.
- **The input frame at position k is the track's composite at the moment of k.** It is built by the
  same `build_plan` the track stage uses, plus the track chain's slots in front of the stream slot. It
  is executed by the stream front's executor on the compute device and kept in the front memo, so there
  is one composition path (invariant 7).
- **Order and pacing count every layer.** A history position asks every frame the track's sub-plan
  asks at that moment, at the stream's size. The paced window and the far walk count those bytes, not
  one frame per position.
- **Keys from addresses.** A position's identity is the hash of the sub-plan's layers at that moment,
  with no content serial (the durable form). An edit of one event changes the identities only where
  that event is active. The disk tier and the durable history (durable-history-and-mix) note every file
  the segment's history reads.
- **The second input** (`SecondFrom::TrackAbove/Below`) is the composite of the adjacent video track.
- **The sound half** pairs on the track: the sound track's stage encodes its mix and breaks it on the
  picture's schedule over the same track layout. The model's `track_pairs` exclusion is lifted.
- **The refusal goes.** The track window lets Datamosh land, and t3237's requirement is replaced.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `video-fx`: a stream stage on a video track decodes the track's composite; its layout, input, order
  and keys.
- `audio-fx`: Datamosh pairs on a track; the sound half reads the track's layout.
- `ui-reach`: the track window accepts Datamosh.

## Impact

- **kt-engine:**
  - `video_stream.rs`: `TrackStream`, and `StreamStage` for a track;
  - `video.rs`: `track_stage_layer` runs the stage, the probe key, `stream_want`, `stream_advance_*`,
    `layers_at`/`wanted_frames` for the stage's history;
  - `render_cache.rs`: the leaves;
  - `export.rs` / `export_pipeline.rs`: `stream_preroll`, `project_may_stream`;
  - `audio.rs`: the track stage seats the sound half with the track layout.
- **Plugins:** `fx-video-codec` `sound.rs` takes a segmented layout (the picture's `Plan` already works
  per layout).
- **kt-model:** `track_pairs` admits Datamosh.
- **kt-app:** the track window's refusal is removed.
- **Invariants:**
  - 4: the history's frames are asked as they are read, at the same sizes;
  - 5: unchanged;
  - 7: the input is `build_plan` executed by the one executor;
  - 11: the identities have no serials, and the export pre-rolls as for events;
  - 12: no work at rest beyond today's prerender.
- **No project format change:** a stream slot on a track is already a valid file.

## Questions for the owner

Decided (2026-11-20, `kkt-26.11.20` A23, his Q40–Q42): "Разрез на входящем, пауза рвёт" — Q1 (a), the mosh cut falls
on the first frame of the incoming event; Q2 (a), a gap ends the stream; Q3 (a) as recommended (an event's own
Datamosh allowed on a track with one).

The questions as asked:

**Q1. Where does a mosh "cut" fall under a transition?**

The classic mosh breaks at the cut, where the new shot's keyframe would be. A dissolve has no single
frame where the new shot begins.

- **a) At the first frame of the incoming event**, the window's start, where its picture first appears
  in the composite. *(Recommended: the codec sees new content there.)*
- **b) At the joint**, the middle of the window, as a hard cut would have been.

Today: Datamosh does nothing on a track.

**Q2. Does a gap on the track end the mosh, or does the black feed the codec?**

- **a) A gap ends the stream.** The next event starts a fresh stream with no history, as an event with
  no event before it does today. *(Recommended: one rule for both hosts.)*
- **b) The gap's black is part of the stream.** The next shot smears into black and out of it, which
  is what a rendered file with a black gap would do in Avidemux.

**Q3. Can an event's own Datamosh stand under a track's Datamosh?**

Then each history position of the track asks the event's own history walk too, and the cost
multiplies.

- **a) Allowed.** The inner stage runs in the sub-plan as it does anywhere. Its key enters the track's
  position identity. *(Recommended: no special case; the cost shows as Loading.)*
- **b) Refused** on the track while an event below carries one, with the reason.
