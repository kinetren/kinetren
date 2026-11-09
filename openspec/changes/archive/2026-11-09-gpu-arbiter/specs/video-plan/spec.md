## ADDED Requirements

### Requirement: One arbiter per ring orders every device's submissions by class
Every device the program opens (the display's, the offline engine's, the export's, the compute host's)
SHALL register on the ring of its adapter: the devices of one hardware adapter share one ring whatever
their backend; a software rasterizer's device is a ring of its own. Every submission except the
display's present SHALL be admitted by its ring, the waiting submission of the highest class first
(`now`, `answer`, `near`, `visible`, `background`, `offline`), and no submission of a class is admitted
while one of a higher class waits. The class is the one its caller runs as (the compose thread: now;
the prefetch thread: near; a scheduler job: its lane's class; an export: offline); an untagged thread
is background. Admission changes timing only: the passes, their order and their bytes do not change
(invariant 11).
Why: `docs/engine-audit.md` M5, rows 134 and 149: the devices of one card share its ring and the
compute host ran whoever submitted first, so the frame on screen waited behind look-ahead and offline
work.

#### Scenario: The highest waiting class is admitted first
- **WHEN** a ring holds a piece in flight and an offline, a near and a now submission wait for it
- **THEN** the now submission is admitted when the piece completes, then the near, then the offline, whatever order they came in
- **TEST** t2250 (`src/libs/kt-gpu/src/arbiter.rs`)

#### Scenario: Devices of one card share a ring; a software device does not
- **WHEN** two hardware adapters' facts name the same card on Vulkan and on GL, and two llvmpipe devices are opened
- **THEN** the two hardware devices register on one ring, and each llvmpipe device on a ring of its own
- **TEST** t2252 (`src/libs/kt-gpu/src/arbiter.rs`)

#### Scenario: A thread's class follows its work
- **WHEN** a scheduler lane of class visible runs a job, a thread sets class near for a block, and a fresh thread asks
- **THEN** the job sees visible, the block near and the thread outside it its own, the fresh thread background
- **TEST** t2256 (`src/libs/kt-time/src/sched/tests.rs`)

### Requirement: A ring that carries a display holds at most one bounded piece in front of it
On a ring that carries the editor's display device the device time of admitted, uncompleted
submissions SHALL stay under the ring's bound, except that one piece is always admitted on an idle
ring. The bound is half a frame period of the preview's project (20 ms at 25 fps), a general rule; a
lower-class frame or compute job longer than it SHALL be cut at pass (dispatch) boundaries into pieces
whose device time, as the pass timer measured those passes on this machine, stays under the bound (a
pass longer than the bound is a piece of its own; a pass never measured ends its piece). A piece
waits for the completion of the one before. A ring with no display neither bounds nor cuts: an
export's ring of frames and a software device keep their pipelining. The cut frame's bytes SHALL equal
the uncut frame's.
Why: row 149 — on a real card the display and the offline device share the graphics ring, and one
long submission held the display. Reproduced on llvmpipe by running the preview and the offline work
on ONE device.

#### Scenario: The in-flight bound applies only with a display
- **WHEN** a ring with a display holds pieces whose predicted time reaches the bound, and the same pieces wait on a ring without one
- **THEN** the displayed ring admits the next only when the earlier completes (one piece always fits an idle ring), the other admits them all
- **TEST** t2251 (`src/libs/kt-gpu/src/arbiter.rs`)

#### Scenario: A long frame is cut on a displayed ring with the same bytes
- **WHEN** a heavy chain (VHS Tape, Camera ISP) is executed twice at 1080p by an offline engine on a device that also carries a display, once warm with its passes measured
- **THEN** the measured run is submitted in more than one piece, no piece but a single pass predicted over the bound, and its bytes equal the uncut run's
- **TEST** t2253 (`src/libs/kt-gpu/tests/arbiter.rs`)

#### Scenario: The display's latency on a shared device is bounded
- **WHEN** the preview presents a light frame every frame period on one device while an offline heavy chain and a Datamosh history run on the same device, with the arbiter and without it
- **THEN** with it the display's frame waits for at most one piece (its latency stays below the offline frame's own device time); without it the display waits for whole offline frames
- **TEST** t2254 (`src/libs/kt-gpu/tests/arbiter.rs`), bench `kt-cli gpu-share`

#### Scenario: The compute host admits the frame on screen first
- **WHEN** a near-class Datamosh job runs on the compute host and a near and a now job wait
- **THEN** the now job is admitted next, and every job's admission is in the trace with its class
- **TEST** t2255 (`src/libs/kt-gpu/tests/arbiter.rs`)

### Requirement: The preview's present waits without polling the device
`wait_presented` SHALL NOT poll the display device on a fixed short period: it sleeps on the frame's
work-done callback for the device time the pass timer measured for the frame (any submission on the
device fires the callback early), then checks once, and waits again for half the remaining guess, at
least a millisecond. A blocking wait on the display device stays forbidden (the surface's present
waits for its lock).
Why: row 272 — the present was waited for by a poll every millisecond.

#### Scenario: A present is waited for by a few checks
- **WHEN** the preview presents frames of a measured chain on a device and waits for each
- **THEN** each wait ends after the frame completed, with at most a few non-blocking checks (none per millisecond of device time)
- **TEST** t2260 (`src/libs/kt-gpu/tests/arbiter.rs`)
