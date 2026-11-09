## ADDED Requirements

### Requirement: The Diagnostics window shows where the memory went
Under the cache occupancy bar the Diagnostics window SHALL show one line of the memory ledger: the
kept composites, the stream history, the decoded frames and the device buffers in megabytes, and the
room the machine gives (the bases and the growth budget). It SHALL read the ledger's counters only and
SHALL ask no repaint of its own.
Why: found-defects 310 and F-RES-011 — the occupancy bar showed the decoded frames alone, so a loop
that was not kept whole gave no sign of which memory had run out.

#### Scenario: The ledger line is painted
- **WHEN** the Diagnostics window is open while the present memo and the burst cache hold bytes
- **THEN** a line names the kept composites, the history, the frames and the device buffers with their megabytes and the room, under the occupancy bar
- **PATH** open View → Diagnostics… → under the cache occupancy bar a "Memory" line reads kept, history, frames, device and the room in MB; play a loop → the kept number grows lap by lap
- **TEST** t2301 (`src/apps/kt-app/src/diag_tests.rs`)
