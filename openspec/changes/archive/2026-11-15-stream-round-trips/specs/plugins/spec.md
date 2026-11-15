## ADDED Requirements

### Requirement: The compute door keeps buffers and the stream memo has a live tier
A compute host MAY keep the buffers a job names on its device (`ComputeHost::run_keeping`) as held
buffers a later job reads as `Init::Held`; a dropped one SHALL go back to the device for the next job of
its size. The stream memo SHALL keep a live tier (`put_live`, `get_live`, `holds`) bounded on its own
(`LIVE_BYTES`), never evicted by the pictures and kept by `drop_unpinned`.
Why: stream-round-trips D1 — a walk's state between batches and walks without the bus.

#### Scenario: A kept buffer feeds the next job
- **WHEN** a job keeps its output buffer and a second job reads it as held, then it is dropped and the job runs again
- **THEN** the second job reads what the first wrote with nothing uploaded, and the rerun creates no buffer
- **TEST** t2708 (`src/libs/kt-gpu/tests/held.rs`)

#### Scenario: The live tier is bounded apart from the pictures
- **WHEN** pictures fill the memo's bound and live entries pass `LIVE_BYTES`
- **THEN** no live entry is evicted by the pictures, the live tier evicts its own least recently used, `drop_unpinned` keeps it and `clear` empties it
- **TEST** t2709 (`src/libs/kt-plugin-api/tests/memo_live.rs`)
