## ADDED Requirements

### Requirement: The process's memory is split into the program's own sections
Besides the cache tenants, open decoders and the audio render's state SHALL be accounts of the memory ledger that are
counted and never granted. The breakdown SHALL add to the ledger's process-side kinds the project model, the undo history,
waveform peaks, reel previews and picker cells, and name "not counted" as the resident memory minus their sum, never
below zero. Device-side kinds SHALL be counted as the process's only where every device is a software rasterizer. Reading
the breakdown SHALL change no cache's size, and SHALL walk the model at most once per revision and second.
Why: the owner, v26.10 line 145 — "распределить в ней информацию по нашему управлению памятью".

#### Scenario: The kinds are counted and grant nothing
- **WHEN** a ledger with wants of the growing kinds gets accounts of the two new kinds with a gigabyte in each, a claim is taken and dropped, and the trace's samples are read
- **THEN** every grant, the budget and the room are those of the ledger without them; the samples name the two kinds
- **TEST** t3962 (`src/libs/kt-time/src/mem.rs`)

#### Scenario: Holders report what they hold
- **WHEN** a 1080p source is opened and closed, and a chain with a reverb is built, played and removed
- **THEN** the decoders' and the audio state's kinds rise while they live and are back where they were after
- **TEST** t4232 (`src/libs/kt-engine/tests/memory_ledger.rs`)

#### Scenario: The model is estimated without a stall
- **WHEN** a project of 2 000 events with curves, 80 undo steps deep, is read once a second, then one event is dragged with the section closed
- **THEN** the estimate is within a third of the allocations the test's allocator counted; no reading estimates more than eight steps; the drag estimates nothing
- **TEST** t4233 (`src/libs/kt-model/tests/memory.rs`)

#### Scenario: Video memory is the devices' own count
- **WHEN** an engine composes a 4K frame and is dropped, with another engine of the same device alive
- **THEN** the number rises by at least the frame's textures, the device is counted once, and it falls after the drop; with no counters (GL) it is the ledger's device kinds and is marked as a lower bound
- **TEST** t4234 (`src/libs/kt-gpu/tests/video_memory.rs`)
