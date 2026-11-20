## Purpose

Captions as their own track kind: items in timeline time made by hand, from the transcript or from
SRT/WebVTT, styled with the vendored fonts, burned in through the one frame plan and exported as sidecar
files.

## ADDED Requirements

### Requirement: A subtitle track holds captions in timeline time
A subtitle track SHALL hold caption items with a start, a duration and multi-line text that do not
overlap on one track; captions SHALL move with ripple like other time-anchored objects; "Create captions
from text" SHALL split the visible words of a range by paragraph, pause, maximum line length and maximum
duration, keeping each word's timeline time for the highlight.
Why: F-TXT-004 — a list of lines with identity, not titles and not markers.

#### Scenario: Captions from text obey the limits
- **WHEN** captions are created from a 2-minute transcript with 42 characters per line, 2 lines, 6 s maximum
- **THEN** no caption exceeds the limits, none overlap, every word appears in exactly one caption in order, a pause of 0.7 s starts a new caption
- **TEST** t3467 (`src/libs/kt-model/tests/subtitles.rs`)

### Requirement: SRT and WebVTT round-trip
Import SHALL read SRT and WebVTT into a new subtitle track (overlapping cues into additional tracks, never
merged), times rounded to the nearest tick; export SHALL write them with millisecond times and UTF-8; a
file written and read back SHALL give the same captions.
Why: the owner's ★14 "SRT/VTT туда-обратно".

#### Scenario: Round trip and overlaps
- **WHEN** an SRT with styled tags, a VTT with cue settings and an SRT with two overlapping cues are imported, then exported and imported again
- **THEN** texts and times survive the round trip exactly; tags are reduced to plain text with a notice; the overlapping cues land on two tracks
- **TEST** t3468 (`src/libs/kt-media/tests/subtitles_io.rs`)

### Requirement: Burn-in is a layer of the one frame plan
A visible subtitle track SHALL be composed as a layer of the frame plan at its row, drawn by the title
rasterizer with the track's style and the vendored fonts, with the highlighted word of a karaoke style
taken from the caption's word times; the preview and the export SHALL compose the same bytes; a tick with
no caption SHALL add no layer.
Why: invariant 7; UI rule 17 — one way to put text on the screen.

#### Scenario: Preview and export draw the same caption
- **WHEN** a caption with a highlighted word is composed for the preview and by an export at the same tick, and a stored frame is compared
- **THEN** the bytes are equal on the device and within tolerance of the stored frame; at a tick between captions the plan has no subtitle layer
- **TEST** t3469 (`src/libs/kt-engine/tests/subtitle_layer.rs`)

### Requirement: The render writes sidecar subtitles on request
The render window SHALL offer per subtitle track burn-in and a sidecar `.srt` or `.vtt` beside the output;
the sidecar SHALL hold the captions of the rendered range with times relative to its start.
Why: a sidecar is the delivery platforms' form (F-TXT-004 trick 1).

#### Scenario: A region render writes its captions
- **WHEN** a region from 10 s to 40 s is rendered with a sidecar SRT
- **THEN** the SRT holds exactly the captions intersecting the region, clipped to it, times shifted by −10 s
- **TEST** t3470 (`src/libs/kt-engine/tests/subtitle_layer.rs`)
