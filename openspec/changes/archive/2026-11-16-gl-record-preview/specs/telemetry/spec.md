## MODIFIED Requirements

### Requirement: A busy second records what each GPU pass cost
Every pass of every plan the preview, the offline engine and the export execute, and every dispatch of
the compute host, SHALL be timed on the device by a timestamp before and after it where the adapter
offers `TIMESTAMP_QUERY` (every device the program opens asks for it when the adapter has it, the
preview's eframe device included). The timestamps SHALL come back without a wait: with the frame's own
readback, or with a map a later frame collects. The load watch SHALL take the process-wide sum once a
period and, for a busy second in which anything composed, write one `GPU` record into the telemetry
journal at that second, after its `LOAD` record: the adapter, the frame size and the largest decode size
with the number of sources, the frames by who asked (preview, offline, export), the device time per
frame (the sum of its passes) and the span from the first pass's start to the last pass's end, then each
pass's mean and count named by effect and pass (`vhs_tape:s5-head-read`, `camera_isp:s07-demosaic`,
`composite`, `geometry`, `display`), the largest total first, then the compute host's jobs and
dispatches (`datamosh:search`). Diagnostics SHALL show the same record. Where the adapter has no
timestamps, a frame SHALL be timed from its submit to the device's "done" and the record SHALL say so.
A frame SHALL be counted by the device that composed it for the person: the window's device and,
where that device is the UI thread's alone (GL), the preview's own device count preview, whichever
path (present or read back) the frame took; that device's errors, trace description and calls in
flight SHALL name it `preview`.
Nothing SHALL be queried or recorded while nothing composes. `kt-cli preview` prints the record of its run.
Why: the owner, kkt-26.11.08 H9: "на моем компьютере всё что могло бы - работало пошустрее...
гораздо.."; his journal showed the UI waiting seconds on the integrated Radeon with no word of what it
ran; found-defects 108: no pass was ever measured on a real card. docs/orchestrator.md: "я не собираюсь
снимать замеры" — a normal run must carry the numbers.

#### Scenario: The passes add up to the frame
- **WHEN** three frames of a plan with VHS Tape over a second layer are read back and one goes through the export's ring
- **THEN** the sum holds four frames (offline 3, export 1) with their size, every tape pass by its name, two composites a frame, and the passes' totals equal the frames' device time to the nanosecond; the line's means times counts equal it within the rounding
- **TEST** t2020 (`src/libs/kt-gpu/tests/pass_time.rs`)

#### Scenario: The journal holds the GPU record of a busy second
- **WHEN** the load watch journals a busy second that carries the device's sum and then one that carries none
- **THEN** the journal holds LOAD, GPU at the same time with the adapter, sizes, frames and each pass's mean and count, then LOAD alone
- **TEST** t2021 (`src/apps/kt-app/src/load_watch.rs`)

#### Scenario: Nothing composes, nothing is recorded
- **WHEN** an engine is open and composes nothing, and again after a taken second
- **THEN** the sum is empty
- **TEST** t2022 (`src/libs/kt-gpu/tests/pass_time.rs`)

#### Scenario: The preview's own device counts its frames as the preview
- **WHEN** on GL the preview's engine off the window's device reads back two moments never shown before
- **THEN** the sum holds them as preview frames and none as offline or export, and its line says "preview"
- **TEST** t2747 (`src/libs/kt-gpu/tests/gl_context_holder.rs`)

### Requirement: Every ring decision is in the trace
Every admission to a ring SHALL be recorded with its class, the device time it predicted, the time it
waited and the ring; every cut piece and every stopped stream history SHALL be recorded too, with the
frame or job they belong to as the cause. `kt-cli trace gpu` SHALL report, per ring and per class,
submissions and compute jobs per second, admission waits and pieces, and the display's present
latency, and SHALL mark the ring that carries the display and a ring that fronts the person's frame
without one (the slow route; the preview's own device on GL).
Why: `docs/engine-audit.md` §2b — "every job is counted per class"; row 134 asked whether the
look-ahead's Datamosh submissions fell, which only a count by class answers.

#### Scenario: The query reports the ring by class
- **WHEN** a window holds admissions of classes now and near on one ring, a cut frame of three pieces, a stopped history and three display presents
- **THEN** `gpu` lists per class the admissions, the waits and the submissions per second, the pieces of the cut frame, the stop, and the presents' latency; the display's ring is marked as carrying the display and a ring admitted as fronting the person's frame says so
- **TEST** t2263 (`src/apps/kt-cli/src/trace_query/tests.rs`)
