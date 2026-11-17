## ADDED Requirements

### Requirement: Every wait for a device names its own submission
A thread that waits for what it submitted SHALL wait for that submission's index: a read-back frame, a
pipeline warm-up draw, a compute job, the compute host's clock calibration and its shader warm-up. Waiting
for everything submitted to the device SHALL be left to what that is the job of (a parked engine,
`finish_present` for tests and benches), and those waits are counted (`kt_gpu::waits_for_all`).
Why: found-defects 693, 661 — a wait for everything also waits for whatever other threads submit to the
device meanwhile (the loop fill's eight lanes on one device, a look-ahead's pieces), and holds the device's
lock through it, against the window's present on the display's device; the calibration's bracket, the clock
error of every compute record, widened to 39 ms on the owner's iGPU.

#### Scenario: No frame or job waits for everything
- **WHEN** an engine warms its pipelines and reads back three frames, and the compute host runs a job
- **THEN** no wait for everything submitted to a device was made
- **TEST** t2864 (`src/libs/kt-gpu/tests/own_waits.rs`)

### Requirement: A ring's waiter sees a completion on any device in flight
A waiter of a ring whose flights are on more than one device SHALL look at every one of those devices at
least every 2 ms while it waits; it SHALL NOT block on one device's oldest piece until that piece ends.
Why: found-defects 662 — the devices of one card finish out of each other's order; a waiter blocked on the
oldest piece of one device missed the room a piece on another made, and near work waited while the card idled.

#### Scenario: A waiter sees a completion on another device
- **WHEN** a ring with a display holds a long piece on one device, a short piece on another, and the rest of the lower pieces it keeps uncompleted, and a near piece comes to be admitted
- **THEN** it is admitted once the short piece ends, while the long one still runs
- **TEST** t2865 (`src/libs/kt-gpu/tests/own_waits.rs`)

## MODIFIED Requirements

### Requirement: The preview's present waits without polling the device
`wait_presented` SHALL NOT poll the display device on a fixed short period: it sleeps on the frame's
work-done callback for the device time the pass timer measured for the frame (any submission on the
device fires the callback early), then checks, and waits again from a quarter of that time doubling, at
least a millisecond; the waits SHALL stop growing at the frame's own time, at least 4 ms
(`present_waits`). A blocking wait on the display device stays forbidden (the surface's present waits for
its lock).
Why: row 272 — the present was waited for by a poll every millisecond; found-defects 693 — with no cap the
doubling saw a frame the device ended late at up to twice its lateness.

#### Scenario: A present is waited for by a few checks
- **WHEN** the preview presents frames of a measured chain on a device and waits for each
- **THEN** each wait ends after the frame completed, with at most a few non-blocking checks (none per millisecond of device time)
- **TEST** t2260 (`src/libs/kt-gpu/tests/arbiter.rs`)

#### Scenario: The present checks stop growing
- **WHEN** a frame was last measured at 0 (never), 0.3, 2 or 20 ms of device time
- **THEN** the first check comes after that time (at least a millisecond, 2 ms when never measured), and no later wait exceeds the frame's time or 4 ms, whichever is longer, which they reach
- **TEST** t2866 (`src/libs/kt-gpu/tests/own_waits.rs`)
