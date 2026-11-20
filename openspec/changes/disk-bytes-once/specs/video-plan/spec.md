## ADDED Requirements

### Requirement: The bytes path shows a kept or stored moment as the display path does
A preview composed to bytes (a GL preview off the window's device, kt-cli `preview`, the harness) SHALL find
a kept moment by the probe plan's key, with no stream stage, decode or look-ahead, and read the kept
composite back; a moment the disk tier holds SHALL be restored once into the executor's kept memo under the
same restored key the display path uses, and later laps SHALL read it from the memo, not from the store.
Why: found-defects 774 — the bytes path read and decoded a disk moment on every lap (250 restores over ten
laps, ~4 ms of a core each at 640×360), and found an ordinary kept moment only by its full plan, after the
stream stage's history (disk-bytes-once D1–D3).

#### Scenario: A disk moment is restored once on the bytes path
- **WHEN** a moment the disk tier holds is composed to bytes ten times over a loop
- **THEN** the store is read once, the nine later composes are memo reads, and every output equals the stored picture
- **TEST** t3356 (`src/libs/kt-engine/tests/render_cache_bytes.rs`)

#### Scenario: A kept moment on the bytes path needs no history
- **WHEN** a Datamosh moment kept by the bytes path's executor is composed again after its stream memo is cleared
- **THEN** it is shown from the kept memo, and the stream's walk counter and the decode requests do not move
- **TEST** t3357 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The live GL run restores each moment once
- **WHEN** the live `render_cache` scenario (the stand's preview is GL: the bytes path) restarts and plays its one-second loop eight laps
- **THEN** the restores its trace counts are at most the moments written, not eight times them
- **TEST** t3358 (live `render_cache`, `src/scripts/live/render_cache.py`)
