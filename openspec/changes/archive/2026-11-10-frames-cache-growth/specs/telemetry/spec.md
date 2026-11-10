## ADDED Requirements

### Requirement: The trace carries the process's memory and the frames cache's sizing
The structured trace SHALL record each sizing of the preview's decoded-frame cache (`frames_cap`: its
bound, the look-ahead window's bytes, the loop's bytes, the preview quality), and, once per busy second,
the process's resident memory with the ledger's sum and the memory available (`mem_sample`) and each
tenant's bytes and bound (`mem_used`) — only what moved by a megabyte since the last sample. A second at
rest SHALL write none of them. `kt-cli trace memory` SHALL print the sizings, each tenant's bytes at the
last busy second, the process's peak with the caches' part and the rest, and the frames cache's
evictions a second.
Why: found-defects 397 — the owner's 4.7 GB had no record: the ledger accounted 1.4–3.7 GB and nothing
said what the rest was or when it grew (kkt-26.11.10 J4).

#### Scenario: A busy second writes what moved, a still one nothing
- **WHEN** a sample follows a change of the frames' bytes, then a sample with nothing moved, then one after the frames moved again
- **THEN** the first writes the process's record and every tenant's; the second writes nothing; the third writes the process's record and the frames' alone
- **TEST** t2395 (`src/libs/kt-time/src/mem.rs`)

#### Scenario: The query names the frames cap and the whole process
- **WHEN** a run holds a frames cache sizing, two process samples, a tenant's bytes and evictions over two seconds
- **THEN** `trace memory` prints the sizing's bound, window and loop, the tenant's bytes against its bound, the process's peak with the caches' part and the rest, and the evictions with the worst second
- **TEST** t2396 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: The process's resident memory is read
- **WHEN** the process touches 64 MB more
- **THEN** its resident memory, read on Linux and Windows, grows by at least 48 MB
- **TEST** t2399 (`src/libs/kt-gpu/src/memory.rs`)
