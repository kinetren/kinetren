## ADDED Requirements

### Requirement: Background work runs on one scheduler and pauses while playing
Every background service — file thumbnails, effect cells, posters, composition strips, offline
analysis, the index, the media watch, autosave — and every job that answers a person's gesture (a
save, a media open, a decoder released) SHALL run as a job on one scheduler: job classes in a fixed
order (now, answer, near, visible, background, offline), each producer on its own lane (its own
order, one job at a time), a pool sized from this machine's cores at run time (a quarter of them, 3
to 6) that always keeps one worker for the person's answers, cancellation by tokens (a lane closes
with its owner), per-class counters. While a player plays or an export runs, the visible and
background classes SHALL start nothing and a running job SHALL wait at its next checkpoint; they
resume when playback stops. A worker with nothing to run SHALL park with no clock. A ready LRU of
thumbnails or effect cells SHALL never evict what the last two UI frames drew. Decoder owners stay
threads of their own (invariant 5); a lane's job drops any source it opened before it ends.
Why: the owner, kkt-26.11.08 H15 (the scheduler may pause thumbnails, analysis and the index while
playing): "Да"; `docs/engine-audit.md` §2b counted about ten queue disciplines and seven timers at
rest; a filmstrip wider than its cap ordered its own pictures round in a circle (row 114).

#### Scenario: Lanes are FIFO one at a time, classes run in order, an answer never waits
- **WHEN** sixty jobs go to one lane; a pump is kicked many times over its own queue; the pool's capped slots are held while a visible, a background and an answer job are queued
- **THEN** the lane runs them in order, never two at once; the kicks coalesce; the answer runs at once, then visible before background when a slot frees
- **TEST** t2230 (`src/libs/kt-time/src/sched/tests.rs`)

#### Scenario: Playing pauses the background classes
- **WHEN** a visible job runs to a checkpoint while a player's vote is up, and visible and background jobs are queued meanwhile
- **THEN** the checkpoint waits and nothing starts while an answer job runs; when the vote drops all of them run
- **TEST** t2231 (`src/libs/kt-time/src/sched/tests.rs`)

#### Scenario: Cancellation, supersede, a panic
- **WHEN** a lane closes with a job running and one queued; five keyed saves are queued behind a held one; a token is cancelled while its job waits at a paused checkpoint; a job panics
- **THEN** the running job sees the close and the queued one never runs; only the newest save runs; the checkpoint releases; the next job of the panicked lane runs
- **TEST** t2232 (`src/libs/kt-time/src/sched/tests.rs`)

#### Scenario: The cap never evicts what is on screen
- **WHEN** ten pictures stand under a cap of four and six of them were drawn this frame; then two frames pass
- **THEN** only the four undrawn are evicted; after the frames move on the cap holds, oldest first
- **TEST** t2237 (`src/apps/kt-app/src/thumbs.rs`)

#### Scenario: Thumbnails wait out playback
- **WHEN** a thumbnail is asked while a player plays
- **THEN** it is not made while playing, it is made after, and the service has no thread of its own
- **TEST** t2238 (`src/apps/kt-app/src/thumbs.rs`)

#### Scenario: A person plays while thumbnails and waveforms are pending
- **WHEN** the heavy showcase project has just opened (its files being indexed, its filmstrips ordered), Space starts playback, the timeline is zoomed with Ctrl+wheel, then Space stops it
- **THEN** while playing the background classes are paused, work waits and no visible or background job starts; after the pause they resume and everything drains
- **PATH** open the heavy project → the filmstrips and waveforms start filling in; press Space → it plays, the filmstrips stop filling in; Ctrl+wheel over the timeline → the view zooms, the new thumbnails wait; press Space → playback stops and the waiting thumbnails and waveforms fill in
- **TEST** live `sched_pause` (`src/scripts/live/sched_pause.py`)
