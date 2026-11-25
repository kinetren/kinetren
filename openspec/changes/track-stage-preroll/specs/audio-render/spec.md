## ADDED Requirements

### Requirement: A track stage pre-rolls on a real seek
On a real seek (a generation change, a sign flip of the rate, a loop flip) every track stage whose lane
sounds within its tail and look-ahead before the new head SHALL reset and run its lane's sum over
`[head − tail − look-ahead, head)` through its chain into the void, its events positioned at the pre-roll's
start by their own seek, its keyed compressors' key branches pre-rolled over the same window through their own
copy of the key track's chain (the window covering the key chain's tail, look-ahead and slot delay too),
on the render thread; a stage whose lane is silent there SHALL do nothing. A seam
(a loop wrap, an event moved during play) SHALL stay a seam. An export of a range SHALL equal the whole
export's samples of that range byte for byte. The start SHALL count as warm for the kept mix: there is no cut
pre-roll (the owner's choice), and the start's prefetch SHALL cover the pre-roll window.
Why: found-defects 420 — a seek or an export range starting inside a track's reverb or delay tail played
silence where continuous play rings; event chains already pre-roll (track-stage-preroll D1–D4). The owner,
kkt-26.11.20 A15: "Всегда полный хвост".

#### Scenario: A seek into a track reverb rings as continuous play
- **WHEN** a track with a 2 s delay without feedback carries a short event, and play seeks to 0.5 s after the event's end
- **THEN** the samples after the seek equal the continuous render's samples at the same frames
- **TEST** t3361 (`src/libs/kt-engine/tests/track_mix.rs`)

#### Scenario: A silent lane costs no pre-roll
- **WHEN** play seeks into a track whose lane is silent for longer than its tail before the head
- **THEN** no lane read is made before the head and no pre-roll is counted
- **TEST** t3362 (`src/libs/kt-engine/tests/track_mix.rs`)

#### Scenario: An export range equals its part of the whole export
- **WHEN** a project is exported whole and again from a point inside a track delay's repeats (no feedback), and a reverb track's tail is checked within its tolerance
- **THEN** the range export's audio equals the whole export's samples of that range byte for byte, and GOLDEN_AUDIO is unchanged
- **TEST** t3363 (`src/libs/kt-engine/tests/golden_export.rs`)

#### Scenario: A keyed compressor's key pre-rolls
- **WHEN** play seeks to a point where a keyed compressor on a music track is ducked by speech that started 0.3 s before
- **THEN** the ducking at the seek point equals continuous play's
- **TEST** t3364 (`src/libs/kt-engine/tests/track_mix.rs`)

#### Scenario: The kept mix keeps the start after a full pre-roll
- **WHEN** a seek into a track reverb's tail is played, then played again
- **THEN** the first play keeps its start (`keep_from` is the head; t2373's cold start is gone), and the second hands it out from the kept mix
- **TEST** t3365 (`src/libs/kt-engine/tests/mix_settle.rs`)

#### Scenario: A sign flip pre-rolls, a wrap stays a seam
- **WHEN** the rate flips sign inside a stage's tail, and a loop over the same track plays three laps
- **THEN** the flip pre-rolls the stage, and the laps count no pre-roll after the first
- **TEST** t3366 (`src/libs/kt-engine/tests/loop_fx.rs`)

#### Scenario: The start's prefetch covers the pre-roll window
- **WHEN** play starts on a cold cache at a point inside a track delay's repeats
- **THEN** the start's hold waits for the window before the head as well, the pre-roll counts no warm-up miss, and the first chunk is kept
- **TEST** t4103 (`src/libs/kt-engine/tests/track_mix.rs`)

#### Scenario: The trace says what a stage pre-roll cost
- **WHEN** a run seeks twice into a track reverb's tail and once into silence, and its trace is read
- **THEN** two stage pre-roll records carry their frames and microseconds, none for the silent seek, and `kt-cli trace summary` sums them
- **TEST** t4104 (`src/apps/kt-cli/src/trace_query/tests.rs`)

## MODIFIED Requirements

### Requirement: A covered range keeps its final mix
The audio render thread SHALL keep the final mix of what it rendered forward, per timeline burst, in the mix store (`kt_engine::mix`), and SHALL hand out a chunk the store holds whole with no source read and no effect run (`mix_replays`). A burst SHALL be held with the scene hash of its frames (`AudioRenderer::scene_hash`: the fingerprints of every event active there — body, handles, tail — widened by the track stages' tail and look-ahead, and what the reel shares); after an edit the store SHALL drop exactly the bursts whose scene changed. Linear play (and a loop's first lap) and the later laps of a loop region SHALL be kept apart; later laps replay the second lap. A frame held is not overwritten. The store is bounded by 64 MB plus what the memory ledger grants the loop region it plays (every burst of the region and an eighth, asked by the player's prefetch thread when the region changes, never by the render thread, which reads the bound at use and evicts when a grant is taken back); without a loop it is least recently heard first. A loop longer than the bound SHALL keep a stable prefix of its lap: bursts of other contexts go first, then the lap's latest; a lap burst later than every kept one SHALL NOT be admitted (a trace record `mix_chunk` aux `passed`), an earlier one takes the latest's place. The store is cleared when a source comes, goes or is replaced (a file that came back, a changed path: the same media id). Offline paths (export, render, probe) MUST NOT read it.
A kept chunk SHALL be what a continuous play gives (kept-mix-settle). Before the first live chunk after a replayed run the effects SHALL be brought back: when the replayed frames are within the settle length of the chains heard in the chunk (`AudioRenderer::settle_frames`: an event chain's `max(tail, 100 ms) + look-ahead`, a track stage's tail and look-ahead, asked at the timeline frames the folded head plays), the effects go on from where they stopped through those frames, rendered and discarded — the continuous play itself; further, the effects are reset and a play of the settle length before the chunk is rendered and discarded through the same path, a loop's wrap included. A chunk with an underrun or a pre-roll miss SHALL NOT be kept, nor any chunk within the settle length after it, nor after a discarded warm-up that missed; after a seek every chain heard at the head is warm, track stages included ("A track stage pre-rolls on a real seek"), so what plays from the head is kept unless the pre-roll missed. The warm-up and the chunk not kept for settling SHALL each leave a trace record (`mix_chunk` aux `warm` with the frames and how, aux `unsettled`).
Why: his 400 ms covered range ran every effect on every lap (kkt-26.11.07b, line 3–4): "три проблемы - производительность, хрип звука, не рабочий кэш". Then (kkt-26.11.10 J1, found-defects 390) "опять появился косяк с пустотным звуком в начале": the live chunk after a replayed run reset the effects and was kept, so a loop's lap was recorded with the timeline before the region as its past instead of the lap before it, and replayed on every lap. A byte-exact hand-over of the FX state needs a copy of the resampler, which rubato cannot give; playing the replayed frames again (short) or the settle length (long) reconstructs it with the engine's own render.

loop-keeps-its-lap (row 550): a fixed 64 MB LRU kept nothing of a loop longer than 2.9 minutes — every lap rendered every burst.
#### Scenario: A replaced source is heard through a covered loop
- **WHEN** a covered loop of a DC source plays from the kept mix and the source is replaced by another level while it plays
- **THEN** the last half second plays the new level, not the former file's kept mix
- **TEST** t2540 (`src/libs/kt-engine/tests/player_dynamic.rs`)

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
- **WHEN** a seek lands where a track's reverb stage still rings from its lane's event, on a warm cache, and then on one where a burst of the pre-roll's window is missing
- **THEN** the first chunk is kept from the head; with the miss nothing is kept within the settle length after it
- **TEST** t2373 (`src/libs/kt-engine/tests/mix_settle.rs`)

#### Scenario: The short loop's start as the device hears it
- **WHEN** the player loops 230 ms over a reverb whose first part is kept from an earlier play, and an output device with no clock of its own takes whole quanta
- **THEN** the second lap the device plays equals the continuous play's second lap byte for byte, every lap from the fourth equals the third, no lap starts quieter than half the second's head, and nothing underruns
- **TEST** t2374 (`src/libs/kt-engine/tests/mix_settle_player.rs`)

#### Scenario: A loop longer than the store keeps its prefix
- **WHEN** a store of four bursts holding two linear ones is told a loop of ten and the laps are written lap after lap, an edit drops a kept burst, and separately a store with no loop is written ten bursts
- **THEN** the linear bursts go first and the lap keeps bursts 0–3, the later ones not admitted, the same every lap; a linear burst finds no room; the dropped burst's room takes the next lap burst and the dropped one takes the latest's place again; with no loop the last four stay
- **TEST** t2601 (`src/libs/kt-engine/tests/mix_store.rs`)

#### Scenario: The kept mix grows by the ledger for its loop
- **WHEN** a store asks the ledger for a loop of 256 bursts over a 128-burst base, keeps the whole loop, then a claim is taken and the store fits, then the claim ends
- **THEN** its bound is the loop's bursts and an eighth rounded to 16 MB, another store that never asked keeps its base; the bytes are reported; under the claim the store keeps the first 128 bursts; the claim's end gives the growth back
- **TEST** t2602 (`src/libs/kt-engine/tests/mix_store.rs`)

#### Scenario: A long loop replays its prefix every lap
- **WHEN** a loop of 30.5 bursts over a reverb plays six laps through a store of 12 bursts, plain and told of the loop
- **THEN** the plain store replays nothing; told, it keeps a prefix of the lap of at least 9 bursts, every lap from the third replays at least 8 chunks, and those chunks equal the kept second lap's byte for byte
- **TEST** t2603 (`src/libs/kt-engine/tests/mix_store.rs`)

#### Scenario: The player asks for its loop's mix
- **WHEN** the player's loop region is set to 400 s with memory to spare, then cleared
- **THEN** the ledger's kept-mix want becomes the region's bursts and an eighth past the 64 MB base, the store's bound that want; with no region, no want and the base
- **TEST** t2607 (`src/libs/kt-engine/tests/memory_ledger.rs`)
