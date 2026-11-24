## ADDED Requirements

### Requirement: A cached source carries the identity of what it decodes
Each source raised for live playback SHALL have an identity in the decoded-frame cache: a file's name, size and
modification time read when the file was opened; a generator's values and the key of what it hears; a synthetic
source's parameters; for any other source, and for a file whose metadata could not be read, a number unique in the
process. The identity SHALL be set in the same step that drops the source's entries of another identity, by the thread
that will insert, and an insert that names another identity SHALL be refused; so every entry of a source was decoded
from its current identity, and a frame and its identity are read in one step. Identities SHALL be read off the
interface's, the compose and the prefetch threads (no file is asked for its metadata while a plan is built).
Dropping a source's bytes alone (`invalidate_source`) SHALL keep its identity.
Why: found-defects 965, 1062, 1063 — the key of a finished frame needs to know what an absent frame would be, and
took it from the project and the disk, which may already be ahead of the decoder.

#### Scenario: The identity and the entries move together
- **WHEN** a source is adopted, frames are inserted under its identity, it is adopted again under the same identity and then under another, a late insert of the first identity arrives, the source's bytes are invalidated, and it is forgotten
- **THEN** the frames stay over the same identity and go with another; the late insert is refused; the invalidation drops the bytes and keeps the identity; a forgotten source has none and its frames are named by serials
- **TEST** t4110 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: What a source answers for its identity
- **WHEN** one file is opened twice, copied under another name, touched and opened again, and removed after it was opened; a source's file is replaced under its path and a span then asks a second decoder; a generator is opened, retuned and retuned back, and given another heard series; two placeholders are raised
- **THEN** the two opens agree; the copy and the touched file differ; the removed file keeps its answer; the source keeps one decoder, and every frame it gives is the old file's; the generator's identity moves with its values and returns with them, and moves with the series; the placeholders differ from each other and are not durable
- **TEST** t4111 (`src/libs/kt-media/tests/identity.rs`)

## MODIFIED Requirements

### Requirement: A moment held final costs no decoded frame
The look-ahead SHALL skip every moment whose final frame the program holds for the current state — a
kept composite of the display's executor or a moment of the disk prerender — asking none of its source
frames and none of its stream history. The decoded-frame cache's want SHALL leave out the same moments,
in memory and on disk alike (a moment's key does not depend on what is decoded, capability `video-plan`):
the window and the loop without them, the window's peak forgotten when their number moves. The loop SHALL be
counted again when the held number fell, when it moved by a sixteenth of the loop, and at a lap's end, not on
every pass. A moment whose kept composite waits to be checked against an edit SHALL count as held until the check
answers, at most 200 ms after the edit; the prefetch SHALL ask for that check itself, and it SHALL go from the
playhead outward. The cap still grows with what the window needs (kkt-26.11.10 J4) and never goes
below the cache's base.
Why: the cache audit (kkt-26.11.22 D2, cut 1) — on the heavy showcase the decoded frames held 7.9 GB of
10.7 GB resident: the window skipped the kept composites but not the disk prerender, and the want was a
peak and a loop counted over moments already held. Found-defects 965: while a kept composite's key contained its
decoded frames, leaving them out of the want lost the composites, and a loop that played from them at 21 frames a
second fell to 2; with the key of `durable-moment-key` it does not.

#### Scenario: Held moments neither decode nor count
- **WHEN** a loop over three 640×360 layers is walked by the prefetch passes once with nothing held, then with half its moments held
- **THEN** the loop's bytes without the held half are half the loop; with half held no source frame of a held moment is asked, and the cap comes down below the whole loop's while still holding the other half and its eighth
- **TEST** t3605 (`src/libs/kt-engine/tests/frames_cache_growth.rs`)

#### Scenario: A loop plays from its kept composites after its sources are evicted
- **PATH** put the loop region over four seconds of a dense edit → press Space → the first laps compose, the bars turn bright → let it play on → every lap plays whole, and the memory of decoded frames in Diagnostics has come down
- **WHEN** a player plays a loop whose frames exceed the cache's base for five laps on the test kit's executor, with a lap of another range between them that evicts the rest
- **THEN** once the loop is kept whole its sources leave the want and the cap is the base; the laps after that show every moment as a replay, ask no decode and report no incomplete frame, and no moment's key changes from lap to lap
- **TEST** t4112 (`src/libs/kt-engine/tests/moment_key.rs`)

#### Scenario: An edit elsewhere asks no decode, an edit here asks its own
- **WHEN** the loop of the scenario above plays from its kept composites and a title outside it is edited, then a parameter of an event inside it
- **THEN** after the first edit no source frame of the loop is asked and the moments are confirmed from the playhead outward; after the second the changed moments' sources are asked within 300 ms, the moment on screen first, and the picture on screen is complete again
- **TEST** t4113 (`src/libs/kt-engine/tests/moment_key.rs`)

#### Scenario: The held moments are not counted on every pass
- **WHEN** a 1500-moment loop fills its kept moments one by one over 1500 passes, then loses one
- **THEN** the loop is counted at most 18 times over the fill and once more for the loss, and the last want equals a count made then
- **TEST** t4114 (`src/libs/kt-engine/tests/frames_cache_growth.rs`)

#### Scenario: The stream pacing orders by the bound it has now
- **WHEN** a renderer build is paced by the player's cache and the cache's bound grows after the build
- **THEN** the order takes a quarter of the grown bound
- **TEST** t2394 (`src/libs/kt-engine/tests/frames_cache_growth.rs`)
