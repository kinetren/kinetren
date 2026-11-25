## ADDED Requirements

### Requirement: The kept final mix survives a restart, loaded off the render thread
The kept final mix SHALL have a durable tier, the `mix/` tenant of the project's cache folder (capability
`media-index`): kept bursts SHALL be written at rest as runs of raw f32 per
context (linear play, a loop region's laps), keyed by `(context, burst, mix(scene_hash, salt))` with the salt
naming the decoder's major, what makes the sound (the audio path's code and the audio plugins of the chains, not the whole build) and the identities of the SOUNDS heard in the burst's
widened range (a source whose picture plays from a copy keeps its original's). After a restart a lane of its own SHALL read the runs matching the current scenes ahead of
the playhead and the loop region and hand them to the render thread through a bounded lock-free channel; neither the prefetch thread nor the render thread SHALL call the disk for it;
the render thread SHALL take them into its store as first recordings at the start of a chunk and SHALL
never read a file (invariant 2). A restored burst SHALL behave as a kept one, its leaving settled as the
continuous play (kept-mix-settle). The export SHALL never read it.
Why: found-defects 771 — the kept mix was the render thread's memory, and every covered range was rendered
live again after a restart; durable-history-and-mix D5.

#### Scenario: A covered range plays from disk after a restart
- **WHEN** a range with Slowed and a reverb is played until kept, the mix is written at rest, the player is dropped and a new one opens the same project and plays the range
- **THEN** the render thread hands the range out with no decode and no effect run once the runs are loaded, the samples equal the first run's kept samples, and the render thread opened no file
- **TEST** t3346 (`src/libs/kt-engine/tests/mix_durable.rs`)

#### Scenario: A stale run is not restored
- **WHEN** stored runs are asked after an event's effect value changes, after a source file is replaced with the same name, and after the store's salt changes (the decoder's major)
- **THEN** the changed bursts are misses and are rendered live, the others are restored
- **TEST** t3347 (`src/libs/kt-engine/tests/mix_durable.rs`)

#### Scenario: A copy of the picture does not lose the kept sound
- **WHEN** a heavy clip's mix is written, its picture is then swapped for its copy's and back, and the player is opened again with the copy present
- **THEN** the runs are restored each time; a source whose ORIGINAL file was replaced is a miss
- **TEST** t4145 (`src/libs/kt-engine/tests/mix_durable.rs`)

#### Scenario: Laps and linear play stay apart on disk
- **WHEN** a loop is played for three laps and linear play passes the same range, both written and restored
- **THEN** the lap context replays only in laps after the first and the linear context only in linear play, each with the samples it was recorded with
- **TEST** t3348 (`src/libs/kt-engine/tests/mix_durable.rs`)

#### Scenario: Leaving a restored range settles the effects
- **WHEN** play leaves a restored range into an unkept one
- **THEN** the effects are brought back through the replayed frames or the settle, and the samples after the edge equal a continuous play's
- **TEST** t3349 (`src/libs/kt-engine/tests/mix_durable.rs`)

#### Scenario: The handoff never blocks the render thread
- **WHEN** the channel is full and the loader keeps sending while the render thread renders under a test clock, and the lane's read stands for ten seconds
- **THEN** the loader waits, the prefetch passes go on, the render thread never waits and allocates nothing in its drain
- **TEST** t3350 (`src/libs/kt-engine/src/mix.rs`)

#### Scenario: The bars show restorable bursts
- **WHEN** after a restart the scan at rest finds runs for the current scenes
- **THEN** the kept-mix band shows those bursts, and they are handed out without work once loaded
- **TEST** t3351 (`src/libs/kt-engine/tests/mix_durable.rs`)

#### Scenario: The audio export does not read the tier
- **WHEN** a project is exported over a filled mix tier and with it off
- **THEN** the audio outputs are byte-equal and the tier's read counter does not move
- **TEST** t3352 (`src/libs/kt-engine/tests/mix_durable.rs`)
