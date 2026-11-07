## MODIFIED Requirements

### Requirement: FX state is owned only by the audio render thread
The state of the FX chains (`FxStateStore`) SHALL belong to the audio render thread; prefetch, meters, and UI MUST NOT touch it and MUST NOT place mutexes around it. Prefetch fills only the PRE-FX cache of the decoded source. The FINAL MIX the render thread produced (after the effects, the track stages and the nested reels, before the master gain) MAY be kept by that same thread and handed out again for a covered range instead of running the effects (requirement "A covered range keeps its final mix"); nothing else produces or reads it, and the bars see it only as that thread publishes it. The sub-mix of a nested reel MUST NOT enter the `BurstCache`, and its state is keyed by the INSTANCE, not by the reel.
Why: four threads without locks over a shared delay line poisoned the reverb (defect b). Keeping the effects' output was rejected while it meant "the bytes depend on the read history"; the owner chose to keep it as a prerender does (kkt-26.11.07b, "вариант А"): a covered range keeps the sound it was first heard with, and plays almost for free.

#### Scenario: A warm cache gives the same sound as the cold reference
- **WHEN** a seek into a slowed event is performed from a cache warmed by prefetch
- **THEN** the output is byte-for-byte equal to the cold reference, and there are no cache misses — prefetch warmed the pre-FX source, not the result
- **TEST** t17 (`src/libs/kt-engine/tests/warp_prefetch.rs`)

#### Scenario: Two instances of one reel do not share the reverb
- **WHEN** one nested reel with a reverb stands in two places and sounds at the same time (capability `compositions`)
- **THEN** each instance has its own sub-mix stream and its own chains; the output does not depend on which one read first
- **TEST** t564 (`src/libs/kt-engine/tests/nest_render.rs`)

## ADDED Requirements

### Requirement: A covered range keeps its final mix
The audio render thread SHALL keep the final mix of what it rendered forward, per timeline burst, in the mix store (`kt_engine::mix`), and SHALL hand out a chunk the store holds whole with no source read and no effect run (`mix_replays`). A burst SHALL be held with the scene hash of its frames (`AudioRenderer::scene_hash`: the fingerprints of every event active there — body, handles, tail — widened by the track stages' tail and look-ahead, and what the reel shares); after an edit the store SHALL drop exactly the bursts whose scene changed. Linear play (and a loop's first lap) and the later laps of a loop region SHALL be kept apart; later laps replay the second lap. A frame held is not overwritten; a chunk with an underrun or a pre-roll miss is not kept. The first chunk the store cannot hand out whole SHALL be rendered live after the FX state is reset — exactly as after a seek to it. The store is bounded (64 MB), least recently heard first, and cleared when a source comes or goes. Offline paths (export, render, probe) MUST NOT read it.
Why: his 400 ms covered range ran every effect on every lap (kkt-26.11.07b, line 3–4): "три проблемы - производительность, хрип звука, не рабочий кэш". A byte-exact hand-over of the FX state at the boundary needs a copy of the resampler, which rubato cannot give; the seek is the engine's own rule for "playing from here".

#### Scenario: A covered range plays again without work
- **WHEN** a range with a reverb is played once, then played again from the same point
- **THEN** every chunk of the second play comes from the store, no source is read, and the samples equal the first play and a live render byte for byte
- **TEST** t1930 (`src/libs/kt-engine/tests/mix_store.rs`)

#### Scenario: An edit drops only what it determines
- **WHEN** two events far apart are covered and one's parameter changes
- **THEN** the store drops the bursts of the changed event's body and tail and keeps the other's; a second play renders only that part live
- **TEST** t1931 (`src/libs/kt-engine/tests/mix_store.rs`)

#### Scenario: From the store to live is a seek
- **WHEN** the store holds the first 40 chunks and the play goes on past them
- **THEN** the chunks after them equal a live render that started at that point, byte for byte, and the junction's step is within the material's own
- **TEST** t1932 (`src/libs/kt-engine/tests/mix_store.rs`)

#### Scenario: Loop laps
- **WHEN** a region off the burst grid loops six times
- **THEN** the first lap is kept as linear play and the second as the lap, every later lap comes from the store and equals the second
- **TEST** t1933 (`src/libs/kt-engine/tests/mix_store.rs`)

#### Scenario: The player's loop and edits during it
- **WHEN** the player loops a short range of a 6000-event project for 4 s while edits arrive
- **THEN** the kept mix plays more than three times as many bursts as are rendered live, with no callback or render underrun
- **TEST** t1935 (`src/libs/kt-engine/tests/mix_player.rs`)
