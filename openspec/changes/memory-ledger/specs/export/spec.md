## ADDED Requirements

### Requirement: An export's device keeps no upload past the frames in flight
A device that composes an export SHALL keep an uploaded source frame only while a frame in flight
(the readback ring's) or the frame being built still reads it, so a source frame used by consecutive
frames is uploaded once and nothing older stays. A texture a frame being built reads SHALL NOT be
reused for another upload of the same frame, on any device. The frames written SHALL NOT change.
Why: found-defects 10, 85, 96 — each export device kept a 256 MB upload cache for frames an export
uploads once; at 1080p that is 31 frames per device, on three or four devices.

#### Scenario: Uploads past the ring are dropped, the bytes stay
- **WHEN** a project whose frames each read eight distinct source frames is exported on an export device and on a preview device with a full upload cache
- **THEN** the export device holds at most the uploads of the frames in flight after every frame, a source frame read by two consecutive frames is uploaded once, and the frames written are byte for byte the preview device's
- **TEST** t2298 (`src/libs/kt-gpu/tests/export_ring.rs`)

### Requirement: An export claims the preview's growth while it runs
While an export runs in the process, the memory ledger SHALL grant no tenant anything above its base:
the preview's kept composites and stream history drop to their bases when the export starts, and are
granted their wants again when the last export ends. The export's own holds and caches are untouched.
Why: an export started from the editor sized its devices and its tight mode on the memory the preview's
grown memos had left (found-defects 10).

#### Scenario: The export takes the room and gives it back
- **WHEN** the preview's present memo and stream memo hold grants above their bases and an export runs
- **THEN** during the export both caps are their bases and the memos hold no more; after it every grant is back and the export's frames are the same as with no grants
- **TEST** t2299 (`src/libs/kt-engine/tests/memory_ledger.rs`)
