## ADDED Requirements

### Requirement: A run prewarm warms the pieces a frame asks
A point-wise run handed to the executor's prewarm SHALL be warmed as the pieces a frame cuts it into
(at most `MAX_RUN_STAGES` stages each), so the first frame of a long run finds its passes warm.
Why: class-walk-revision — the prewarm passed the whole run as one key, and a key longer than the
cap is skipped; no frame ever asks it.

#### Scenario: A 40-stage run is prewarmed as 32 + 8
- **WHEN** a run of 40 point-wise stages is prewarmed and the device settles
- **THEN** two run shapes are warm: the 32-stage piece and the 8-stage tail
- **TEST** t2450 (`src/libs/kt-gpu/tests/fused_runs.rs`)

### Requirement: The stream memo's view belongs to the renderer that set it
The memo's view is a tick of the timeline of the renderer that set it (the player's preview
renderer). A stream stage SHALL apply it only in that renderer's own plan and its track stages; a
reel's sub-plan and any renderer sharing the memo that never set a view (a reel strip, a poster)
SHALL read every position of its walk back.
Why: class-walk-revision — the view was read by every renderer sharing the memo, so a walk on
another timeline read no picture back and every frame of it stayed short.

#### Scenario: A renderer that set no view reads its walk back
- **WHEN** two renderers share one memo, the player's sets its view at 8 s, and the other walks 5 s … 5 s + 8 frames
- **THEN** every frame of that walk needs nothing more
- **TEST** t2451 (`src/libs/kt-engine/tests/decode_storm.rs`)

### Requirement: A reel strip raises generators and pauses its envelope
The reel strip SHALL open the generators' per-event sources as the poster does, so a cell a
generator shows has its picture; its envelope (the offline audio render) SHALL wait at each chunk
while its job is held, as its frames do.
Why: class-walk-revision — since only complete offline composites are kept (catch-up-shows-loading
D6) every generator cell was empty; the envelope checked only the cancel and ran during playback.

#### Scenario: A generator cell has its picture
- **WHEN** a reel's strip is rendered whose only event is a solid-colour generator
- **THEN** its first cell is the generator's colour
- **TEST** t2455 (`src/apps/kt-app/src/reel_preview.rs`)

#### Scenario: The envelope waits while held
- **WHEN** the strip job is held after the envelope's first chunk
- **THEN** no further chunk runs until it is released
- **TEST** t2454 (`src/apps/kt-app/src/reel_preview.rs`)

### Requirement: Caches size against the ledger's room
The preview's stream memo and the decoded-frame cache's loop count SHALL measure their want against
the memory ledger's room — the memory available plus the growth the growing kinds already hold —
never the bare available.
Why: class-walk-revision — the cache's own growth took the free memory, so a re-sizing asked less
than it held and the asker evicted what it had just kept.

#### Scenario: A re-sizing after the memo filled keeps its cap
- **WHEN** the memo is sized for a 400-position loop, fills to its cap, the machine has that much less available, and the same loop is sized again with one checkpoint more
- **THEN** the cap does not shrink
- **TEST** t2458 (`src/libs/kt-engine/tests/memory_ledger.rs`)

### Requirement: The tools name a frame behind its stream history
The preview bench SHALL report every incomplete composite whose stream history was behind with the
positions short; `kt-cli preview` and `kt-cli pause-scan` SHALL print it.
Why: class-walk-revision — such a frame misses no decode, so the tools' one cause named nothing.

#### Scenario: A cold start over a mosh is named catching up
- **WHEN** the bench plays a range of a Datamosh event from a cold start
- **THEN** its incomplete composites are reported catching up, each with a positive count
- **TEST** t2459 (`src/libs/kt-engine/tests/decode_storm.rs`)
