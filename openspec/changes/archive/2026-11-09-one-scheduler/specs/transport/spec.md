## MODIFIED Requirements

### Requirement: When paused the player is quiet; edits to the media composition on the fly do not disturb transport
Pause SHALL park the render and prefetch threads with no clock: after the pause's tail (at most
two short passes that take the block the callback was handing out when the pause came) neither
thread makes a pass until something it acts on changes — play, pause, a seek, a rate, the preview
quality, a loop region, a published build, a retuned or replaced source, the player's drop — and
each of those wakes it; a change between a thread's last look and its park is never lost.
Rebuilding the renderers without an edit of the model MUST NOT happen, pause→resume MUST NOT
deadlock. The addition and the substitution of sources SHALL survive coverage, the playhead and the
generation, and a substitution SHALL clear the cache of the old source and recompute the decode scale
step on the fly. Why: at pause the system lagged even with an empty project, and after a file returned,
the preview showed the placeholder for a long time; then the parked threads still woke four and ten
times a second on a pause (one-scheduler: "the program at rest costs nothing").

#### Scenario: Pause — parking of the threads
- **WHEN** the player stands on pause for a second; then an edit of the model; then resume
- **THEN** ≤ 8 render passes, zero prefetch rebuilds; the edit wakes exactly one rebuild; after resume the ring fills again
- **TEST** t20 (`src/libs/kt-engine/tests/player_dynamic.rs`)

#### Scenario: A pause costs nothing and every input wakes it
- **WHEN** a paused player has settled; then a seek; then fifty seeks in a burst; then play and pause
- **THEN** over a second and a half neither the render nor the prefetch thread passes; the seek wakes both; after the burst the prefetch's last pass saw the last seek; play wakes the render thread and the pause parks it again
- **TEST** t2240 (`src/libs/kt-engine/tests/player_dynamic.rs`)

#### Scenario: Adding media incrementally
- **WHEN** a new file is added on a warm cache
- **THEN** coverage, bytes, playhead and generation are preserved, the new worker is alive
- **TEST** t18 (`src/libs/kt-engine/tests/player_dynamic.rs`)

#### Scenario: Substitution of a source on the fly (container: capability `media-index`, `video-plan`)
- **WHEN** a source that has returned to the disk replaces the placeholder during playback
- **THEN** the frames of the old source leave the cache, and the new native changes the decode scale step without an edit of the timeline — the strips and the prefetch already ask for it
- **TEST** t211, t316 (`src/libs/kt-engine/tests/player_dynamic.rs`)
