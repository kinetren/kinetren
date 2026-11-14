## MODIFIED Requirements

### Requirement: A run of point-wise stages is one generated pass
`plan_chain` SHALL emit `PlanOp::PointRun { i, j, res_scale }` for two or more consecutive chain
stages at a barrier position that each fuse in runs (`kt_plugin_api::fuses_in_runs`: class
`PointWise`, a `tap_<id>` WGSL, no barrier kernel); audio slots, stream stages and identity Geometry
nodes between them are transparent. An active geometry step, a barrier, a point-wise plugin with a
barrier kernel of its own (noise, greenscreen), a layer-strength slot and a stop (a strength snapshot,
a mask point) SHALL end a run; a lone stage SHALL stay a `Barrier` op. A run takes the resolution its
first stage would have taken as a barrier (the K× rule unchanged).

The GPU executor SHALL run a `PointRun` as ONE pass generated for the run's shape (the sequence of
its plugin ids): one texel load, each stage's `tap_<id>` in chain order with the stage's numbers from
the pass's uniform, the stage's rounding and clamp to 0…255 after each stage — what the stage's own
pass stores — and one store. A run longer than the uniform's room (`MAX_RUN_STAGES` = 32) SHALL be
cut into pieces of that room. A shape's pipeline SHALL be compiled and warmed on the device's warm
worker (a frame never compiles); until it is warm, and wherever the executor splits it, the stages
SHALL run as their own passes. A warm shape SHALL be tried against its own passes: the frames of the
trial SHALL show the own passes, and the fused pass SHALL run beside them into a scratch target, both
timed by the pass timer in the same frame; when the fused pass is slower per pixel than the sum of its
parts (minima of the tries) the shape SHALL be split into two halves, each tried again, else the frames
SHALL show the fused pass from then on. On a device without timestamps a warm shape SHALL run fused.
The output form of a frame SHALL be a function of its shapes' states alone, never of the trial's
alternation; a process-wide forms revision SHALL move whenever a shape's output form can change (a shape
compiled, a verdict) and SHALL be part of the readback memo's and the present memo's key, and a composite
whose resolve saw the revision move SHALL NOT be kept. An export's frame (`execute_once`, the readback
ring's `submit`) SHALL take the SETTLED form: every run's generated pass, compiled in the frame when not yet
compiled, no trial and no split; a text the device refuses runs as its own passes. The player SHALL hand the run shapes a project reaches to
the executor's `prewarm_runs` with its effect ids.

The fused and the own-pass forms SHALL give frames within 1 LSB per former boundary (the owner's
agreement, H16); on the reference software adapter they are byte-equal. On a card they are bounded, not
equal, so no kept or exported frame depends on which form a timing chose (found-defects 510).
Why: engine-audit M7, row 10: each point-wise effect read and wrote the whole frame (8 bytes a pixel,
2–3 ms at 1080p on llvmpipe, gpu-pass-cost D2). Measured after: design D-measure.

#### Scenario: The plan names runs and ends them where the class says
- **WHEN** chains of point-wise, geometry, barrier, noise/greenscreen, audio, strength and mask stages are planned, with and without stops and at K×
- **THEN** every two or more consecutive fusable stages form one `PointRun`, a lone one stays a `Barrier`, every other class ends a run, and the K of a run is the K its first stage had
- **TEST** t2191 (`src/libs/kt-fx/tests/point_runs.rs`)

#### Scenario: Fused equals unfused over every pair and a long run
- **WHEN** every ordered pair of fusable point-wise plugins (defaults and a tuned value set), a six-stage run, a 40-stage run (over the room) and a run inside a strength slot and beside a geometry series compose at 1080p fused, and again with fusion off
- **THEN** each frame is within 1 LSB per former boundary of its unfused frame, and on a software adapter byte-equal; the fused frame runs one pass per run piece
- **TEST** t2190 (`src/libs/kt-gpu/tests/fused_runs.rs`)

#### Scenario: A run shape compiles beside its frame
- **WHEN** a frame meets a run shape no frame met, on an engine whose core and effects are warm
- **THEN** the frame runs the own passes, draws nothing cold and creates no pipeline; after the worker settles the shape is ready and the next frame runs one pass with the first frame's bytes; a shape handed to `prewarm_runs` is ready before its first frame
- **TEST** t2192 (`src/libs/kt-gpu/tests/fused_runs.rs`)

#### Scenario: The switch is measured on the device
- **WHEN** a trial records fused passes slower than the sum of their parts for a four-stage shape, then its halves faster
- **THEN** the shape is split into the two halves, each half is tried and kept fused; with faster fused tries the whole shape stays fused; with no timestamps it is fused at once
- **TEST** t2193 (`src/libs/kt-gpu/src/runs.rs`)

#### Scenario: Preview and export agree on a run
- **WHEN** a project with a point-wise run is exported and the same moment composed for the preview, on a fused engine and on one with fusion off
- **THEN** the four frames are byte-equal on a software adapter
- **TEST** t2197 (`src/libs/kt-gpu/tests/fused_runs.rs`)

#### Scenario: The player prewarms the project's run shapes
- **WHEN** a project's track holds Color Grade, Posterize and Vignette, then a clip a run of two inside a strength slot, then an edit adds nothing
- **THEN** `run_shapes` gives the three-stage and the two-stage shape, and the executor is asked for the runs again only when the set changed
- **TEST** t2196 (`src/libs/kt-engine/tests/prewarm_reach.rs`)

#### Scenario: A trial never changes what a frame shows
- **WHEN** a three-stage shape no frame met is composed until its trial's verdict
- **THEN** the first frame runs its own passes; every frame of the trial shows the own passes (the fused pass timed beside them) with the first frame's bytes on a software adapter; after a fused verdict the frame shows the fused pass; the forms' revision moved
- **TEST** t2514 (`src/libs/kt-gpu/tests/fused_runs.rs`)

#### Scenario: The memo key names the run forms
- **WHEN** a plan is composed twice through the readback memo, then a new run shape is compiled, then the plan is composed twice again
- **THEN** the second ask is a hit; after the shape compiled the plan is composed again, then kept again
- **TEST** t2515 (`src/libs/kt-gpu/tests/fused_runs.rs`)

#### Scenario: An export's frame runs the settled form
- **WHEN** an export frame meets a run shape no frame met, on an engine in the measured mode, four times
- **THEN** every time the run is fused (compiled in the first frame), the bytes are the same every time and within 1 LSB a boundary of the own passes (equal on a software adapter)
- **TEST** t2516 (`src/libs/kt-gpu/tests/fused_runs.rs`)

## ADDED Requirements

### Requirement: The compute host's every submission is admitted and a job binds by offset
The compute host SHALL admit every submission on its ring by the caller's class: a job's pieces, its clock
calibration (taken after the admission, so a wait never widens its bracket; a job finding another
calibrating goes on without, so a frame's job never waits behind a lower class's calibration) and a
shader's warm-up dispatch. A job SHALL write every step's uniform into one arena with one queue write and
bind each step's slot by a dynamic offset, with one bind group for every step of the same shader, slot size
and buffers within the job.
Why: found-defects 511 (the calibration submitted outside the ring) and 520 (a uniform buffer, a write and a
bind group per dispatch: ~2.4 ms of a 267-dispatch Datamosh walk).

#### Scenario: The calibration is admitted
- **WHEN** a traced job is the compute host's first, on a device with timestamps
- **THEN** the ring admitted the job's pieces and the calibration's three submissions
- **TEST** t2510 (`src/libs/kt-gpu/tests/compute_arena.rs`)

#### Scenario: A job binds its uniforms by offset
- **WHEN** a job of 240 dispatches over two shaders, each with its own uniform, adds its uniform into a buffer
- **THEN** the buffer holds the exact sum, and the job made two bind groups
- **TEST** t2511 (`src/libs/kt-gpu/tests/compute_arena.rs`)

### Requirement: The readback memo keeps complete composites and finds them without a scan
The executor's readback memo SHALL keep only complete composites, and SHALL find, bump and evict an entry
without scanning its keys (a generation per entry).
Why: found-defects 523 — incomplete composites, keyed by what they lacked and never asked again, pushed
complete ones out of the 64 MB the strip, poster, picker and slow route share; 521 — the LRU was a linear
scan per frame.

#### Scenario: The least recently asked goes first
- **WHEN** three frames fill the memo, the first is asked, and a fourth is kept
- **THEN** the second is evicted, the first stays; a key kept again replaces itself in place
- **TEST** t2512 (`src/libs/kt-gpu/src/pool.rs`)

#### Scenario: An incomplete composite is not kept
- **WHEN** two complete and ten incomplete composites are kept into a memo of two
- **THEN** the complete ones stay and no incomplete one is kept
- **TEST** t2513 (`src/libs/kt-gpu/src/pool.rs`)

### Requirement: A saturating display leaves lower classes only its slack
While the display presents, what a lower class may leave in front of the display's next present SHALL be
the part of the frame period the display's own present leaves free (its present cost averaged over its
recent presents), at most the bound; the `near` class SHALL keep at least a quarter of the bound and the
`answer` class the bound. A display that stopped presenting leaves the bound.
Why: found-defects 491 — on his iGPU the display's composite (32.5 ms at a 16.7 ms period) filled the device,
and a lower class still got one bound in front of every display frame, which only made the display later.

#### Scenario: Room by the display's cost
- **WHEN** the display presents frames costing a quarter period, seven eighths of it, twice it, and then stops
- **THEN** a background piece has the bound, an eighth of the period, nothing and the bound again; the near class keeps a quarter bound in front of the saturating display
- **TEST** t2517 (`src/libs/kt-gpu/src/arbiter.rs`)

