## ADDED Requirements

### Requirement: The front of a stream stage reaches its codec on the compute device
The slots in front of a stream stage SHALL run on the compute host's device when the compute host is
kt-gpu's, and their complete output SHALL reach the stage's codec as a buffer on that device, with no
readback and no upload; the bytes the codec reads SHALL be the ones the read-back front gives. An
incomplete front frame SHALL be missing, never kept. A buffer of another device SHALL be refused by
the job (an error, never a panic). Without a compute host of kt-gpu the front SHALL run on the offline
device as bytes. `KT_FRONT_HELD=off` runs the bytes form (an A/B).
Why: found-defects 543 — each history position's front was read back from the offline device and
uploaded to the compute device, ≈ 16 ms of the advancing thread at 1080 lines (`stream-front-on-device`).

#### Scenario: The codec reads the front where it lies
- **WHEN** shot B's chain is posterize → datamosh, composed at a frame inside the mosh with fresh memos, once held and once with the front read back
- **THEN** the codec's jobs copied held frames on the device, and the two frames are equal
- **TEST** t2620 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: A held frame is the read-back frame
- **WHEN** an engine on the compute device runs a posterize plan at 64×24 and at 100×30 (rows not a multiple of the copy alignment) by `execute_once` and by `execute_held`, and a compute job copies the held buffer
- **THEN** the held frame is tightly packed and its bytes equal `execute_once`'s
- **TEST** t2621 (`src/libs/kt-gpu/tests/held.rs`)

#### Scenario: Another device's buffer is refused
- **WHEN** an engine on another device holds a frame and a compute job names it
- **THEN** the job returns an error naming another device
- **TEST** t2622 (`src/libs/kt-gpu/tests/held.rs`)

### Requirement: A ready geometry shape moves the forms' revision
When a geometry program shape's specialized pass becomes ready, the run forms' revision SHALL move,
so a composite kept from the interpreter's frame is never served for a frame that runs the
specialized text.
Why: found-defects 561 — the mechanism of row 510 on the geometry pass: `Compiled::shape` answers the
interpreter while pending, then the specialized text, and the memo keys did not name that form.

#### Scenario: A shape's readiness moves the revision
- **WHEN** a geometry chain's first frame runs while its shape compiles and the device's worker settles
- **THEN** the shape is ready and the forms' revision has moved
- **TEST** t2624 (`src/libs/kt-gpu/tests/shape_forms.rs`)
