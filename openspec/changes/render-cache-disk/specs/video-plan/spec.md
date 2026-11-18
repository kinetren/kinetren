## ADDED Requirements

### Requirement: Slow moments are prerendered to disk at rest and played from it
The preview SHALL keep complete composites of slow moments on disk under a DURABLE key: the plan key's
inputs (every field the executor's plan key hashes) with each frame's per-process content serial replaced
by what determines the frame across processes — a file's path, size and modification time, a generator's
values (`src_rev`), a stream stage's own key plus the identities of the project's files — salted by the
format, the program build and the device, so the same plan gives the same key in another process and an
edit, a replaced file, another build or another device gives another. The files SHALL be lossless, their
total bounded (2 GiB unless `KT_RENDER_CACHE_MB`), evicted least recently used; the folders of other builds
SHALL be removed when the store opens.

Before composing a moment the display's executor does not hold — the render-ahead producer before it asks
whether the moment is decoded, the compose thread before it composes — the preview SHALL look the moment up
on disk and, found, hand it to the display's executor and show or note it, asking no decode and running no
stage or chain. The coverage bars SHALL draw the moments the store holds for the current state.

The player SHALL prerender by itself at rest: when it is not playing, no export runs, and no input came for
1.5 s, it SHALL compose the moments whose measured device time is at least half a frame period and the
unmeasured neighbours of such moments (inside the loop region first, then nearest the playhead), its
device work at class `background`, its decodes at the lowest priority of the player's own workers, and
write those measured heavy. Any input, Play, an export or the player's close SHALL stop it before its next
step and abandon the moment. With no store set, nothing is read or written and the preview is as before.
No export SHALL read or write the store.
Why: kkt-26.11.17 S5 — the owner chose a prerender of slow regions "Сам в фоне" (review R3), his one
exception to "the program at rest costs nothing"; a heavy loop was composed again on every restart and
whenever the display's memo lost it.

#### Scenario: The store keeps pictures losslessly, within its cap
- **WHEN** composites are written to a store with a small cap, one read is touched, another build's folder exists at open, and a file is truncated
- **THEN** reads give the same bytes, the least recently used entries go first to fit the cap, the touched one stays, the other build's folder is gone, and the truncated file is a miss and removed
- **TEST** t2980 (`src/libs/kt-engine/src/render_cache.rs`)

#### Scenario: The durable key survives a process and follows what the picture is made of
- **WHEN** a moment's durable key is computed by two renderers over two frames caches (different serials), with its frames decoded and not; then after an edit of an effect parameter, a file's modification time, a generator's value, the quality and the build salt
- **THEN** the first keys are equal; each change gives another key
- **TEST** t2981 (`src/libs/kt-engine/tests/render_cache.rs`)

#### Scenario: A prerendered moment plays from disk after a restart
- **PATH** play a slow part with VHS Tape and stop → after a second and a half with the mouse still the bars over the part turn bright → close and open the project, play the part → it plays from the first lap
- **WHEN** a player measures a heavy moment, rests until the prerender wrote it, and a new player over the same store and project plays that moment
- **THEN** the second player's display shows the moment from disk on its first ask while playing, its workers received no decode request for it, and the restored composite equals the bytes the producer's engine composes for the plan
- **TEST** t2982 (`src/libs/kt-engine/tests/render_cache.rs`)

#### Scenario: The prerender picks what was measured slow
- **WHEN** the cost map holds heavy moments (at least half a frame period), light ones and unmeasured ones next to the heavy, and a loop region
- **THEN** the candidates are the heavy moments not on disk and their unmeasured neighbours, the loop region's first, then nearest the playhead; a light moment and the moments past it are not candidates
- **TEST** t2983 (`src/libs/kt-engine/src/render_cache.rs`)

#### Scenario: The prerender yields to the person
- **WHEN** heavy moments are measured and the player stands paused while input keeps coming; then the input stops; then Play is pressed while it works; then an export vote is up
- **THEN** nothing is composed while input came within 1.5 s; after the quiet it writes; Play abandons the moment in work and nothing is written while playing; nothing runs under the export vote
- **TEST** t2984 (`src/libs/kt-engine/tests/render_cache.rs`)

#### Scenario: The bars draw what the disk holds for this state
- **WHEN** the store holds a prerendered loop and a new player opens the project with the loop region set; then an effect of the event is edited
- **THEN** after the scan at rest the bars draw the loop as playing without work, with no device work and no decode; after the edit they do not
- **TEST** t2985 (`src/libs/kt-engine/tests/render_cache.rs`)
