## Purpose

Point tracking: an event's tracker with up to four points that follow features of its picture, sampled
per material frame with a confidence by a deterministic matcher, run as a correctable session, and
exposed as an address that any property can link to.

## ADDED Requirements

### Requirement: Track samples are tied to the material and made deterministically
A tracker SHALL belong to an event and hold up to four points, each with a feature box, a search box and
samples (position in source pixels, confidence) keyed by the material frame of the event's media; split,
trim, move, reverse and speed changes SHALL change no sample. Samples SHALL be computed by zero-mean
normalized cross-correlation on an integer luma pyramid with sub-pixel refinement, and the same media,
boxes and start frame SHALL give byte-identical samples on every machine.
Why: the keys' representation rule (R.33); invariant 11's spirit for stored analysis.

#### Scenario: A synthetic target is followed to a tenth of a pixel
- **WHEN** a fixture moves a textured patch along a known sub-pixel path with noise over 120 frames and a point is tracked forward from frame 0
- **THEN** every sample lies within 0.1 px of the known path, confidence ≥ 90; tracking the same from the last frame backward gives the same path within 0.1 px; two runs are byte-identical
- **TEST** t3520 (`src/libs/kt-media/tests/tracker.rs`)

#### Scenario: Edits never shift samples
- **WHEN** a tracked event is split, trimmed, moved and slowed to 0.5
- **THEN** the stored samples are unchanged and the tracker's value at each timeline tick is the sample of the material frame shown there
- **TEST** t3521 (`src/libs/kt-model/tests/tracker.rs`)

### Requirement: Tracking is a session that stops and continues
A tracking run SHALL start at the material frame under the playhead, go forward or backward by one frame
or continuously, and stop at a confidence below the tracker's threshold, at the event's material edge, on
Esc and on Play, showing the stop frame and its confidence as an error; a point dragged at a frame SHALL
become a manual sample from which the next run continues; a run SHALL be one undo step, run as an
offline job at background class and make no progress while the transport plays.
Why: F-MSK-005 trick 2 — a broken track is the normal case.

#### Scenario: An occlusion stops the run and a correction continues it
- **WHEN** a fixture's patch is covered for 10 frames at frame 60 and a forward run starts at 0; the point is dragged onto the patch at frame 71 and tracking continues
- **THEN** the run stops at the first covered frame with its confidence shown; the frames 61–70 have no good samples; after the correction samples resume from 71 to the end; the session is two undo steps
- **TEST** t3522 (`src/libs/kt-media/tests/tracker.rs`)
