## ADDED Requirements

### Requirement: A long UI frame names the stage that waited
The journal's note of a UI frame blocked for 100 ms or more SHALL list the frame's stages by their WALL
time (timeline, events, bin, centre, composite, windows; those of a millisecond or more), so a wait —
a lock, a file, the device — is named by its stage even when the thread spent no processor time in
it. The live harness's geometry dump SHALL be written by a thread of its own, so the harness does not
put a file write into the frame it measures.
Why: the owner's journal (kkt-26.11.08 H9) said a frame stood 4.5 s and not where; the per-stage
processor time cannot see a wait. Measuring this change on his project, the note named the timeline
stage, and inside it the harness's own dump: one write stood 10–20 s on a disk busy with the
sidecar's fsyncs.

#### Scenario: A frame whose timeline stage waits
- **WHEN** a frame's timeline stage waits 5 ms without work and its centre stage takes under a millisecond
- **THEN** the frame's wall breakdown names the timeline with at least 5 ms and names neither the centre nor the frame itself
- **TEST** t1846 (`src/apps/kt-app/src/ui_wait_tests.rs`)
