## ADDED Requirements

### Requirement: A stream stage on a video track decodes the track's composite
A stream slot on a video track's chain SHALL run as a stream stage over the track. The track is laid out
by one function, shared with the sound half, into segments:
- a segment starts at the first frame of an event of the track that was not active the position before (its
  transition handle included), and ends at the next such start or at the end of the contiguous run of events; a piece a
  morpher layer cut off the same event and that continues it starts none;
- a gap ends the stream;
- a segment's history starts `before` positions inside the previous segment, as an event's starts in
  the event before it.

The stage SHALL read, at each history position, the track's composite at that moment. The composite is
built by the same plan builder the track stage uses, followed by the track chain's slots in front of
the stream slot, at the stream's size, and executed by the stream front's executor. An incomplete
composite is a missing frame, never a stand-in.

The kernel SHALL be the same as on an event, with the same memo, checkpoints and device walk. A track of
one event that fills the frame, with no transitions and no slot in front, SHALL mosh byte for byte as that event's own
stream stage.

The stage's picture SHALL take the alpha of the track's composite at the moment shown, before the slots after the
stage run: where the track covers nothing now, the tracks below show. A stream slot in an ARM of the track's chain SHALL be a
read of the track at that arm, as on an event (fx-graph-dag); a track chain's wiring SHALL reach the plan and its key.

Why: found-defects 820 — Datamosh on a track was a dead control, refused since
track-mosh-and-start-wait; the classic datamosh is done on a rendered edit, whose history crosses the
edit's cuts (track-stream-stage D1–D3).

#### Scenario: The layout of a track
- **WHEN** a track holds events at 0–2 s and 2–4 s, a dissolve into a third at 3.5 s, a gap and a fourth
  event at 6 s, at 30 fps, and the kernel asks 12 frames before
- **THEN** the cuts are at the first frame of each event, the dissolve's at the window's start, the
  fourth's segment has no history before its cut, and the others start 12 positions back
- **TEST** t3379 (`src/libs/kt-engine/tests/track_stream.rs`)

#### Scenario: One event on a track moshes as the event itself
- **WHEN** one event whose source has the project's aspect and fills the frame, with no transitions, carries Datamosh on its
  own chain, and then the same values on its track's chain instead, rendered over its range
- **THEN** both pictures are equal byte for byte on one device
- **TEST** t3380 (`src/libs/kt-engine/tests/track_stream.rs`)

#### Scenario: The history crosses a cut with what the track composed
- **WHEN** a track holds a blurred event and the next event under a title on the same track, with the
  keyframe at the cut deleted
- **THEN** past the cut the stage's picture carries motion over the blurred picture and the title from
  before the cut, and the frames equal the reference walk over the composites
- **TEST** t3381 (`src/libs/kt-engine/tests/track_stream.rs`)

#### Scenario: The tracks below show where the track covers nothing
- **WHEN** Video 1 carries Datamosh over a picture-in-picture and a fade-out to the end, above a full-frame event on Video 2
- **THEN** outside the picture-in-picture Video 2 is shown byte for byte, under the fade-out the mosh's colours are the stage's and
  only the level falls, and where the track's composite is opaque the picture is the stage's byte for byte
- **TEST** t3395 (`src/libs/kt-engine/tests/track_stream.rs`)

#### Scenario: A track's graph is rendered
- **WHEN** a video track's chain is Blur, then a Merge of the Source over it, and the wire is then moved
- **THEN** the track's picture equals the same graph on a reel instance holding the track's events, not the line's, and the moved
  wire changes the stage's key and picture
- **TEST** t3396 (`src/libs/kt-engine/tests/fx_graph_render.rs`)

#### Scenario: Datamosh in an arm of a track's chain
- **WHEN** a track's chain merges its Source with an arm holding Posterize and Datamosh
- **THEN** the arm's picture equals the track stage of the line Posterize, Datamosh, and the request is the read for both
- **TEST** t3397 (`src/libs/kt-engine/tests/track_stream.rs`)

### Requirement: A track stream stage asks what it reads, paced by bytes
The order of a track stream stage SHALL ask, for every history position, the frames the track's
composite asks at that moment, at their decode sizes, including an inner stream stage's needs. The paced
window SHALL cut the positions by the bytes of all their layers.

A position's identity SHALL be the moment's one key (`moment_key`) of the composite's plan at that position and the front's
key, built by the same builder the picture's plan is, looking at the sources' names instead of their frames: no frame is
read and no cache entry's recency moves. A frame of another identity than the one its position is named by SHALL be an absent
frame. An edit of one event therefore changes identities only where that event is active, and from there on along the history.

The track layer's content key SHALL carry the stage's output key. The probe plan SHALL key the stage
without running it. A moment SHALL be kept on disk only when every source its segment's history can read has a name that
outlives the process.

Why: invariant 4 (the request is the read); one frame per position counted a three-layer position as a
third of its bytes, and a window sized that way evicted its own front (decode-request-storm); serials in
an identity would make it per process (track-stream-stage D4).

#### Scenario: The request is the read across a three-layer position
- **WHEN** a track's history position composes three layers, one under a frame-blending speed, and the
  order is asked and then the stage runs
- **THEN** every key the composite reads was asked, at its size, and nothing else was
- **TEST** t3382 (`src/libs/kt-engine/tests/track_stream.rs`)

#### Scenario: Names and frames give one key
- **WHEN** plans of a project with files, a generator, a reel, a transition, masks and an inner Datamosh are built over a cache that holds every
  frame, and again over the sources' names alone
- **THEN** the two plans have one moment key at every moment, the names' build moved no entry's recency, and the plans of the cache are today's
  keys and bytes
- **TEST** t3398 (`src/libs/kt-engine/tests/moment_key.rs`)

#### Scenario: The paced window counts every layer
- **WHEN** a track's positions hold three layers each and the pacing's byte budget fits nine frames
- **THEN** the window holds three positions, not nine
- **TEST** t3383 (`src/libs/kt-engine/tests/track_stream.rs`)

#### Scenario: An edit keeps the history before it
- **WHEN** a track's history is walked over two segments, then an event in segment 2 changes its blur
- **THEN** segment 1's memo entries are still held, segment 2's change from the first position where the
  event is active, and the frames equal a fresh walk
- **TEST** t3384 (`src/libs/kt-engine/tests/track_stream.rs`)

#### Scenario: A kept track mosh is shown without its history
- **WHEN** a moment of a track mosh is kept, the stream memo is cleared, and the moment is asked again
- **THEN** it is shown by the probe key with no stage run and no decode asked
- **TEST** t3385 (`src/libs/kt-engine/tests/track_stream.rs`)

#### Scenario: A file under the segment changes the disk key
- **WHEN** a moment of a track mosh is prerendered, then a file of an event earlier in the same segment is
  replaced with the same path and another mtime, and in another run a source there is a placeholder
- **THEN** the moment's key changes and a file of another segment does not change it; with the placeholder the moment has no disk key
- **TEST** t3386 (`src/libs/kt-engine/tests/render_cache.rs`)

### Requirement: A track stream stage's front and second input are tracks'
The slots of a track's chain in front of its stream slot SHALL run over the composite at every history
position, keyed by their slots and curves; the slots after it run over the stage's output.

The second input (`TrackAbove`/`TrackBelow`) SHALL be the composite of the adjacent video track at the same
moment: its events with their chains, without that track's own chain. No adjacent track, or a gap there,
means no second frame.

Why: a track chain may carry its own stream slot, and reading through it would make two stages read each
other (track-stream-stage D5).

#### Scenario: A slot in front runs on the history, one after does not
- **WHEN** a track chain holds Posterize, Datamosh, Blur; Posterize is edited, then Blur is edited
- **THEN** the Posterize edit changes every identity of the history; the Blur edit changes none, and both
  pictures equal fresh renders
- **TEST** t3387 (`src/libs/kt-engine/tests/track_stream.rs`)

#### Scenario: The track above is the second input
- **WHEN** a track's Datamosh takes its vectors from the track above, which carries a moving title over a
  gap
- **THEN** the vectors follow the title where it is and the stream has no second frame over the gap
- **TEST** t3388 (`src/libs/kt-engine/tests/track_stream.rs`)

### Requirement: An export renders a track mosh as the preview does
An export SHALL pre-roll a track stream stage's history before the frames that need it and compose those
frames in frame order, as for an event's stage. The export's stream question and the prewarm of the front's executor SHALL count tracks.

Why: invariant 11 — a range export equals its part of the whole export (track-stream-stage D5).

#### Scenario: A range export of a track mosh
- **WHEN** a project with a track mosh over three segments is exported whole, then a range starting
  inside segment 2, twice
- **THEN** the range's frames equal the whole export's frames of that range, and the two runs are equal
- **TEST** t3389 (`src/libs/kt-engine/tests/track_stream.rs`)

#### Scenario: The export knows a track streams
- **WHEN** a project whose only stream slot is on a track is asked whether it streams
- **THEN** it says yes
- **TEST** t3390 (`src/libs/kt-engine/tests/track_stream.rs`)
