## MODIFIED Requirements

### Requirement: A covered range keeps its final mix
The audio render thread SHALL keep the final mix of what it rendered forward, per timeline burst, in the mix store (`kt_engine::mix`), and SHALL hand out a chunk the store holds whole with no source read and no effect run (`mix_replays`). A burst SHALL be held with the scene hash of its frames (`AudioRenderer::scene_hash`: the fingerprints of every event active there — body, handles, tail — widened by the track stages' tail and look-ahead, and what the reel shares); after an edit the store SHALL drop exactly the bursts whose scene changed. Linear play (and a loop's first lap) and the later laps of a loop region SHALL be kept apart; later laps replay the second lap. A frame held is not overwritten. The store is bounded (64 MB), least recently heard first, and cleared when a source comes or goes. Offline paths (export, render, probe) MUST NOT read it.
A kept chunk SHALL be what a continuous play gives (kept-mix-settle). Before the first live chunk after a replayed run the effects SHALL be brought back: when the replayed frames are within the settle length of the chains heard in the chunk (`AudioRenderer::settle_frames`: an event chain's `max(tail, 100 ms) + look-ahead`, a track stage's tail and look-ahead, asked at the timeline frames the folded head plays), the effects go on from where they stopped through those frames, rendered and discarded — the continuous play itself; further, the effects are reset and a play of the settle length before the chunk is rendered and discarded through the same path, a loop's wrap included. A chunk with an underrun or a pre-roll miss SHALL NOT be kept, nor any chunk within the settle length after it, nor after a discarded warm-up that missed; after a seek, no chunk within a track stage's reach SHALL be kept while its lane's events still ring in it (`AudioRenderer::cold_after_seek`: the stage has no pre-roll). The warm-up and the chunk not kept for settling SHALL each leave a trace record (`mix_chunk` aux `warm` with the frames and how, aux `unsettled`).
Why: his 400 ms covered range ran every effect on every lap (kkt-26.11.07b, line 3–4): "три проблемы - производительность, хрип звука, не рабочий кэш". Then (kkt-26.11.10 J1, found-defects 390) "опять появился косяк с пустотным звуком в начале": the live chunk after a replayed run reset the effects and was kept, so a loop's lap was recorded with the timeline before the region as its past instead of the lap before it, and replayed on every lap. A byte-exact hand-over of the FX state needs a copy of the resampler, which rubato cannot give; playing the replayed frames again (short) or the settle length (long) reconstructs it with the engine's own render.

#### Scenario: A covered range plays again without work
- **WHEN** a range with a reverb is played once, then played again from the same point
- **THEN** every chunk of the second play comes from the store, no source is read, and the samples equal the first play and a live render byte for byte
- **TEST** t1930 (`src/libs/kt-engine/tests/mix_store.rs`)

#### Scenario: An edit drops only what it determines
- **WHEN** two events far apart are covered and one's parameter changes
- **THEN** the store drops the bursts of the changed event's body and tail and keeps the other's; a second play renders only that part live
- **TEST** t1931 (`src/libs/kt-engine/tests/mix_store.rs`)

#### Scenario: From the store to live is a seek
- **WHEN** the store holds the first 40 chunks (longer than the chains' settle length) and the play goes on past them
- **THEN** the chunks after them equal a live render after a seek to the settle length before them, on the chunk grid, byte for byte, and the junction's step is within the material's own
- **TEST** t1932 (`src/libs/kt-engine/tests/mix_store.rs`)

#### Scenario: Loop laps
- **WHEN** a region off the burst grid loops six times
- **THEN** the first lap is kept as linear play and the second as the lap, every later lap comes from the store and equals the second
- **TEST** t1933 (`src/libs/kt-engine/tests/mix_store.rs`)

#### Scenario: The player's loop and edits during it
- **WHEN** the player loops a short range of a large project (a reel placed 400 times) for 8 s while edits arrive, each rebuild longer than the ring
- **THEN** no more than two laps render live, the rest comes from the kept mix, with no callback or render underrun
- **TEST** t1935 (`src/libs/kt-engine/tests/mix_player.rs`)

#### Scenario: A partly kept loop keeps the continuous laps
- **WHEN** the first 7 chunks of his 163 ms loop over a reverb are kept from an earlier play from the region start, and the loop then replays them and goes live 634 frames before the wrap
- **THEN** the effects go on through the 7 replayed chunks (7168 frames warmed, no reset); the first two laps heard equal the continuous play from the seek byte for byte — the second rings the first — and every later frame is the kept second lap's
- **TEST** t2370 (`src/libs/kt-engine/tests/mix_settle.rs`)

#### Scenario: Nothing is kept within the settle length after a hole
- **WHEN** a source burst is missing once during a linear play over a reverb
- **THEN** the store holds the play up to the starved chunk and again only from the first chunk at least the settle length past its end
- **TEST** t2371 (`src/libs/kt-engine/tests/mix_settle.rs`)

#### Scenario: A long replay warms the lap through its settle length
- **WHEN** an edit drops the kept mix around one event of a 3 s loop and the next play goes live in the second lap after a long replayed run
- **THEN** the chunk is warmed by a reset and the settle length before it on the chunk grid, through the loop; the live chunks equal that play byte for byte and are kept under the lap
- **TEST** t2372 (`src/libs/kt-engine/tests/mix_settle.rs`)

#### Scenario: A track stage after a seek is kept once it settled
- **WHEN** a seek lands where a track's reverb stage still rings from its lane's event
- **THEN** no chunk within the stage's reach after the seek is kept, the first kept chunk starts there; a seek where nothing rings is kept from its head
- **TEST** t2373 (`src/libs/kt-engine/tests/mix_settle.rs`)

#### Scenario: The short loop's start as the device hears it
- **WHEN** the player loops 230 ms over a reverb whose first part is kept from an earlier play, and an output device with no clock of its own takes whole quanta
- **THEN** the second lap the device plays equals the continuous play's second lap byte for byte, every lap from the fourth equals the third, no lap starts quieter than half the second's head, and nothing underruns
- **TEST** t2374 (`src/libs/kt-engine/tests/mix_settle_player.rs`)

## ADDED Requirements

### Requirement: A reset reaches every stream, and every stream it reaches warms
A seek, a change of direction or a loop flip (`FxStateStore::reset_all`) SHALL reset every stream through an effect chain the render thread keeps: the events' processors, their ringing laps, the track stages, the nested reels' sub-mix streams AND the sidechain key branches. A nested reel's move SHALL pre-roll the tail and look-ahead of its own track stages as well as its events' chains.
Why: the class walk of kept-mix-settle. The key branch was left out of the reset: its chains went on across the jump as a seam and rang the old place into the new place's detector, so two seeks to one place ducked differently. A reel whose events were dry but whose track carried a delay or a reverb had a zero pre-roll, so a move inside it started the stage cold — silence where continuous play rings.

#### Scenario: A seek resets the key branch
- **WHEN** a ducked music track's key (the speech track) carries a delay, and the same place is reached by a seek after playing through the speech and after playing a silent place
- **THEN** both seeks sound the same, byte for byte
- **TEST** t2375 (`src/libs/kt-engine/tests/sidechain.rs`)

#### Scenario: A reel's move pre-rolls its track stage
- **WHEN** a reel's only event is dry and its track has a 600 ms delay with no feedback, and a seek lands on the echo past a block boundary of the reel's stream
- **THEN** the seek plays the continuous play's bytes; without the stage in the pre-roll it played silence
- **TEST** t2376 (`src/libs/kt-engine/tests/nest_render.rs`)
