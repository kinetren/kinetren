## MODIFIED Requirements

### Requirement: Slow moments are prerendered at rest as hardware-encoded video and played from it
The preview SHALL keep complete composites of slow moments on disk under a DURABLE key: the plan key's
inputs (every field the executor's plan key hashes) with each frame's per-process content serial replaced
by what determines the frame across processes — the identity of its source (a file's name, size and modification
time as it was opened, a generator's values; capability `media-index`, change `durable-moment-key`), a stream stage's
own key, which mixes the identities of the sources it reads; a moment with a source whose identity does not outlive the process is neither written nor read — salted by the
store's format and the device class (the folder `results/v<FORMAT>/<device class>/`), and keyed further by
the decoder's major, the version of the decode's own conversion, the versions of the kernels that ran (each plugin's version — the hash of its
crate's source, made at build time — and its WGSL text's hash) and `COMPOSITE_PATH_VERSION` (the hash of the
composite code outside the kernels, the files of `kt-engine/composite-path.txt` without comments), not by the
whole build. A procedural source SHALL bring the generators' versions, a stream stage the stream kernels'. So the same plan gives the same key in
another process and in a release that did not change those kernels, while an edit, a replaced file, a
changed kernel or another device class gives another. The key SHALL NOT depend on the
codec, the encoder or the container: it names the picture, and a later reader (the cached final render,
F-EXP-003) can match it. The pictures SHALL be stored as video segments — runs of consecutive moments, every
frame a keyframe — written in-process by the machine's HARDWARE video encoder (the export's encoder cascade
with its real open check; H.264 at a high quality) and read back by our libav decoder; each segment's
manifest SHALL name the codec, the encoder, the frame size and rate and the durable key of every frame. No
processor encoder SHALL be used unless the host forces one by name (`KT_RENDER_CACHE_ENCODER`: tests and the
live check on a machine without a hardware encoder); where no hardware encoder opens the store writes
nothing and the Diagnostics window and the trace say why. The store SHALL be the `frames/` kind of the persistent cache in the project's cache folder (`results/`; capability `media-index`, "The project's
caches live in a folder beside it"), written in that folder's cycle (capability `media-index`: the least recently used dropped first when the
folder cannot take more; `KT_RENDER_CACHE_MB` caps it alone for tests), a segment at a time, the lowest value per byte first; an entry
whose salt no longer matches SHALL be a miss and be removed when met, with no sweep when the store opens. A restored picture is the composite within the codec's tolerance.

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
whenever the display's memo lost it. kkt-26.11.18b U4 — the storage: not raw, not a processor codec ("слабо
я верю что ffv1 аппаратно ускорен"), a normal video codec on the hardware ("сделай ты сжатие видео
нормальное"); lossy accepted, the export never reads it. U5 — the key is reused later by the cached final
render. kkt-26.11.20b B1 — it is the frames kind of the persistent cache, salted by the kernels that ran
rather than the build (agreed with change `persistent-cache`), so a release keeps what it did not change.
kkt-26.11.20 A14 — the store moves beside the project ("вот с тем же превью кэшем"),
so one heavy project no longer evicts another's loop (project-cache-folder D3).

#### Scenario: The store keeps segments within its cap
- **WHEN** segments of two frames are written to a store with a small cap, two of them read, an entry of another salt is asked, a segment's video is truncated, and a codec opens no encoder
- **THEN** each frame reads back as written, the manifest names the codec, the encoder, the size, the rate and the keys and is its own text, the least recently used segment goes first, the entry of another salt is a miss and is removed while nothing else is touched, the truncated segment is a miss and removed, the prerender evicts nothing this process used, and the codec with no encoder refuses the segment
- **TEST** t2980 (`src/libs/kt-engine/src/render_cache.rs`)

#### Scenario: A segment is written by the hardware encoder or refused
- **WHEN** a segment is opened with the default codec on a machine, and again forced to the export's software H.264, then six frames are written and read back out of order
- **THEN** the default opens a hardware encoder or refuses naming why, leaving no file; the forced segment names its encoder and every frame reads back alone within the codec's tolerance
- **TEST** t2986 (`src/libs/kt-media/tests/segment.rs`)

#### Scenario: A segment is read by the hardware decoder, the processor only without one
- **WHEN** a segment is read on this machine (no render node here), and in a child process on the fake VA driver of `fixtures/fake_va` (its encoder writes I_PCM macroblocks, its VLD entry point reads them back) a segment its encoder wrote is read out of order and twice
- **THEN** on this machine the reader names the processor and why (no VAAPI device) and reads every frame as the processor's decoder does; on the fake device every decoder of the file decodes on it (`vaapi <node>`, also the ones a later read adds), each frame is within the RGB → 4:2:0 → RGB round trip of the frame written (mean ≤ 3) and within swscale's rounding of NV12 against yuv420p of the processor's decode (mean ≤ 2), and the driver counts at least the pictures read; the disk tier's reader records which decoder reads (`render_cache` aux `READER`). Windows asks D3D11VA the same way (found-defects 775)
- **TEST** t3129 (`src/libs/kt-media/tests/segment.rs`)

#### Scenario: The durable key survives a process and follows what the picture is made of
- **WHEN** a moment's durable key is computed by two renderers over two frames caches (different serials), with its frames decoded and not; then after an edit of an effect parameter, a file's modification time, a generator's value, the quality, a kernel's version or WGSL text, the decoder's major, `COMPOSITE_PATH_VERSION` and the device class; and after a change of the program build that touches none of them
- **THEN** the first keys are equal; each named change gives another key, and the build change alone keeps it
- **TEST** t2981 (`src/libs/kt-engine/tests/render_cache.rs`)

#### Scenario: A prerendered moment plays from disk after a restart
- **PATH** play a slow part with VHS Tape and stop → after a second and a half with the mouse still the bars over the part turn bright → close and open the project, play the part → it plays from the first lap
- **WHEN** a player measures a heavy moment, rests until the prerender wrote it, and a new player over the same store and project plays that moment
- **THEN** the second player's display shows the moment from disk on its first ask while playing, its workers received no decode request for it, and with a lossless test codec the restored composite equals the bytes the producer's engine composes for the plan
- **TEST** t2982 (`src/libs/kt-engine/tests/render_cache.rs`)

#### Scenario: The prerender picks what was measured slow
- **WHEN** the cost map holds heavy moments (at least half a frame period), light ones and unmeasured ones next to the heavy, and a loop region
- **THEN** the candidates are the heavy moments not on disk and their unmeasured neighbours, the loop region's first, then nearest the playhead; a light moment and the moments past it are not candidates
- **TEST** t2983 (`src/libs/kt-engine/src/render_cache.rs`)

#### Scenario: A heavy run is one segment from its start
- **WHEN** a run of six moments is measured heavy, the playhead stands in its middle, and the tier prerenders it
- **THEN** the candidates come in runs of consecutive moments (the loop region's first, then by distance to the head), each from its start, and the whole run lies in one segment that starts before its first moment; taken by distance to the head with the sides alternating, the part behind the head went into a segment per moment (found-defects 776)
- **TEST** t3601 (`src/libs/kt-engine/tests/render_cache.rs`), t2983

#### Scenario: A run known by one moment is one segment
- **WHEN** one moment in the middle of a heavy run is measured (its costs arrive one at a time, as when the playhead steps) and the tier prerenders
- **THEN** the candidates take the unmeasured moments up to `RUN_REACH` (8) on each side of a heavy moment, stopping at a measured, stored or pictureless moment, and the whole run lies in one segment that starts before its first moment (found-defects 826)
- **TEST** t3600 (`src/libs/kt-engine/tests/render_cache.rs`), t2983

#### Scenario: The prerender yields to the person
- **WHEN** heavy moments are measured and the player stands paused while input keeps coming; then the input stops; then Play is pressed while it works; then an export vote is up
- **THEN** nothing is composed while input came within 1.5 s; after the quiet it writes; Play abandons the moment in work and nothing is written while playing; nothing runs under the export vote
- **TEST** t2984 (`src/libs/kt-engine/tests/render_cache.rs`)

#### Scenario: The bars draw what the disk holds for this state
- **WHEN** the store holds a prerendered loop and a new player opens the project with the loop region set; then an effect of the event is edited
- **THEN** after the scan at rest the bars draw the loop as playing without work, with no device work and no decode; after the edit they do not
- **TEST** t2985 (`src/libs/kt-engine/tests/render_cache.rs`)

#### Scenario: The editor prerenders a slow loop by itself and plays it from disk after a restart
- **PATH** open the heavy project with a one-second loop → Play: the loop plays slowly → Pause and leave the mouse: the program writes the loop's moments to its render cache, then rests → close and open the project again, Play → the loop plays from the disk from its first lap
- **WHEN** the editor plays a loop under Slowed, VHS Tape and Camera ISP on a software rasterizer, pauses with no input, and is started again on the same project (the encoder forced to the export's software H.264: the virtual screen has no hardware encoder)
- **THEN** segments appear in the render cache at rest and the program then rests; the first run's trace records heavy prerendered moments and the encoder; the restart's trace counts moments restored from disk in its first lap
- **TEST** t2988 (`src/scripts/live/render_cache.py`)

#### Scenario: The store lives in the project's folder
- **WHEN** two projects in two folders prerender a loop each, the second folder's cap smaller than its loop, and the first start of the build finds a former `<data>/render-cache`
- **THEN** each project's segments lie in its own `kinetren-cache/`, the second folder's oldest segments are replaced as its loop is written (a segment holds at most an eighth of the room; one that finds none is taken again in shorter ones) while the first folder is untouched and the bars draw no dropped moment, and the former folder is gone
- **TEST** t3833, t3834 (`src/libs/kt-engine/tests/render_cache.rs`)

#### Scenario: The composite path's version follows its code
- **WHEN** the guard hashes the composite code outside the kernels (the fusion, `plan_chain`, the core passes and shaders, the conversions, the titles and the core's generators) as `composite-path.txt` names it
- **THEN** the hash equals `COMPOSITE_PATH_VERSION`, every named file is there, every shader of the core crates is among them, and the executor, the planner and the conversions are
- **TEST** t3808 (`src/libs/kt-engine/tests/render_cache.rs`)
