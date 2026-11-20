## Purpose

Point and planar tracking: an event's tracker with up to four points, or a plane, that follow its picture, sampled
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

### Requirement: A planar tracker follows a flat surface by its homography
A tracker of the planar kind SHALL hold a reference quad and, per material frame, a homography from the reference
frame with a confidence, computed from feature matches by a seeded robust fit and refined over the whole region on
an integer luma pyramid, leaving out an optional mask of the event; the same media, quad and start frame SHALL give
byte-identical samples on every machine. Its corners, centre, angle, scale and homography SHALL be link channels.
Why: the owner, 2026-11-20 (A20) — the point AND the planar tracker now; F-MSK-005 — the plane averages the error
over the area, so an occlusion degrades it instead of knocking it down.

#### Scenario: A plane under a known perspective path, partly covered
- **WHEN** a fixture warps a textured plane by a known homography path over 120 frames, with a disc covering 20 % of it for 30 frames, tracked once without and once with the disc's mask named as ignored
- **THEN** every corner lies within 0.2 px of the known path without the cover and within 0.5 px under it with the ignore mask; confidence stays ≥ 70 % with the mask; two runs are byte-identical
- **TEST** t3528 (`src/libs/kt-media/tests/tracker.rs`)

#### Scenario: A plane pins a layer in perspective
- **WHEN** the plane of t3528 is applied as "Attach event…" to a picture event and as "Mask follows" to a mask, and the project is exported
- **THEN** the attached picture's corners match the tracked quad within 0.5 px at every frame; the mask's vertices follow the homography within 0.5 px; preview and export are equal
- **TEST** t3529 (`src/libs/kt-engine/tests/tracker_links.rs`)

### Requirement: The tracker's defaults are the most convenient and exact
A new point SHALL get a feature box of 1/24 of the shorter picture side (24–96 px) centred on the click and a search
box three times larger moved by the predicted motion; one "Track" SHALL run both directions from the frame under the
playhead to the event's edges; matching SHALL use the native-size frame with sub-pixel refinement whatever the
preview quality; the stop threshold SHALL be 60 % for points and 70 % for a plane.
Why: the owner, 2026-11-20 (A20) — "по умолчанию - самые удобные и точные".

#### Scenario: One click and one Track, at a low preview quality
- **WHEN** at preview quality 1/4 a point is placed on a 1080p fixture's patch by one click and "Track" is pressed once in the middle of the event
- **THEN** the boxes are 45 and 135 px; samples cover the event from its start to its end; they equal the samples of the same run at preview 1/1 byte for byte
- **TEST** t3530 (`src/libs/kt-media/tests/tracker.rs`)
