## ADDED Requirements

### Requirement: The readback ring follows the device's latency
An export composing on one device SHALL keep as many frames in flight between submission and readback as
the device takes to hand them back, bounded by the ring's bytes: the ring SHALL hold, for a frame size, a
staging budget's worth of frames (at least three), and the compose thread SHALL hand on every frame the
device has finished as soon as it has finished, in submission order, waiting for a frame only while the
ring is full. The frames written SHALL be byte for byte those of a ring of any other depth and of
`execute_into`. The ring's depth and every frame's time from submission to collection SHALL be recorded in
the export's profile and the trace.
Why: kkt-26.11.18 T3, "отрендерил даже самый простой регион - и скорость 1x cur". His trace: the device
worked 1.06 ms a 1080p frame and handed it back 52.7 ms after its submission; a fixed ring of three frames
made the export three frames per 52.7 ms, 0.94× real time of a 60 fps project, with every stage idle.

#### Scenario: The ring holds more than three frames of a 1080p export, and gives the executed bytes
- **WHEN** an export engine is asked its ring depth for 1080p, 4K and 8K frames, and as many 1080p-sized
  demo plans as the 1080p depth are submitted before the first collect, polled until done and collected
- **THEN** the 1080p depth is more than three, a larger frame gets no deeper ring and never fewer than
  three; every submission is accepted, the ring's first frame reports done before it is collected, and
  every collected frame equals the `execute_into` of its plan, byte for byte
- **TEST** t2960 (`src/libs/kt-gpu/tests/export_ring.rs`)

#### Scenario: The compose thread keeps the device's latency in flight and hands on finished frames at once
- **WHEN** a pipelined export of forty frames runs on an executor whose frames are done a fixed time after
  their submission and whose ring holds twelve; and again on one whose frames are done at once
- **THEN** with the latency the profile's ring records more than three frames in flight at once and the
  frames written equal the serial export's; with no latency at most two frames were ever in flight (each was
  handed on at its next submission); both record forty frames from submission to collection
- **TEST** t2961 (`src/libs/kt-engine/tests/export_pipeline.rs`)

## MODIFIED Requirements

### Requirement: The render log is kept in the window and in one file
The progress window SHALL hold a collapsible log section (collapsed while the run is healthy,
opened by itself when the run ends in an error) with at most the last 800 lines; each line SHALL
be prefixed by the UI with the elapsed time of the run. The log SHALL contain the engine's log
lines, the backend choice and its reason, the sources rendered as placeholders, the Pause, Resume
and Cancel requests and the result. The same lines SHALL be recorded in the telemetry journal, which
mirrors them into the run's trace; no other file SHALL be written for them (no `render.log`). After a
failure the window's Open log SHALL open the telemetry journal.
Why: v1 had an 800-line log panel and wrote a log file on error
(`projectrenderer.cpp`, `render-error-<UTC>.log`); a failed render used to leave one error line in
the window and nothing on disk. kkt-26.11.18 T4: "объедини лог рендера в телеметрию, нахуя ты его
выделил отдельно то?!?" — every line already went into the journal and the trace, and `render.log` was a
third copy the owner had to collect apart from the trace he sends.

#### Scenario: The log is bounded, rotated and named
- **WHEN** the harness runs a scripted job that emits 1000 log lines and an error, with the data
  directory in a temp folder and the telemetry journal bound to a test journal; then presses Open log
- **THEN** the window's log holds the last 800 lines, each starting with an elapsed stamp, and is
  open after the error; the test journal holds all 1001 lines as `EXPORT` records; no `render.log` or
  `render.log.1` exists in the data directory; Open log opens the telemetry journal's path; the window
  shows no log path; every text of the log section exists in the five catalogs
- **TEST** t959 (`src/apps/kt-app/src/layout_tests.rs`, `src/apps/kt-app/src/i18n_tests.rs`, `src/apps/kt-app/src/font_tests.rs`)
