## MODIFIED Requirements

### Requirement: A software adapter composes an export on several devices
An export on a software adapter SHALL open a device per frame worker: an eighth of the cores, 2 to 4 of
them, as many as the available memory holds for the frame size, and at least one. One device SHALL serve a
real card. The export SHALL USE as many of the opened devices as its measured time says: it SHALL start on
one, add one while the frames written per second of wall grow by more than a tenth, keep the fewer on a
tie, and keep re-trying one more and one fewer as the render goes, by the same rule on every machine. The
frames written SHALL NOT depend on how many devices composed them, and the export's line SHALL name the
counts it used. The video decoders of a render SHALL open with four threads, and a render short of memory
SHALL run tight: two decoder threads, half the frame cache, four encoder threads. A render SHALL NOT die
of memory; speed is secondary.
Why: each device keeps its own textures and shaders and adds 1.3 GB at 1080p, and seven decoders at auto
held 2.7 GB of frames for nothing: the showcase went from 17 GB to 8.7 GB and from 144 s to 130 s. Row
710: four devices took 31.8 s against 12.5 s on one over a light stretch and won over the whole project —
no fixed count is right for both.

#### Scenario: Several devices write the frames of one
- **WHEN** one project is exported through one engine and through three, on the same adapter
- **THEN** the frames written are byte for byte equal
- **TEST** t1573 (`src/libs/kt-gpu/tests/export_ring.rs`)

#### Scenario: The device count follows the memory and the frame
- **WHEN** the count is asked with plenty of memory, 7 GB, 4 GB, 1 GB at 4K, or memory unknown
- **THEN** it is the ceiling, less than the ceiling, one, one (never none), and two; a bigger frame gets
  no more devices than a smaller one; little memory gets two decoder threads, any other four
- **TEST** t1642 (`src/libs/kt-gpu/tests/compute_host.rs`)

#### Scenario: The memory is read
- **WHEN** the program asks what it may take on Linux or Windows
- **THEN** it gets a number; on Linux a control group's limit counts
- **TEST** t1643 (`src/libs/kt-gpu/tests/compute_host.rs`)

#### Scenario: The count follows the measured time
- **WHEN** the rule is fed windows whose rate grows with each device up to three of four, then a stretch
  where every count runs at the same rate, then one where two win again
- **THEN** it climbs to three and stops there, comes down to one on the even stretch, goes back up to two,
  never asks for none or more than it has, and a rise of less than a tenth adds no device
- **TEST** t2900 (`src/libs/kt-engine/src/export_pipeline.rs`)

#### Scenario: A changing count writes the frames of one device
- **WHEN** a project is exported on three devices with the count scripted to change every few frames
  (1, 3, 2, 3, 1) and on one device
- **THEN** the frames are byte for byte equal, and the line names the counts used
- **TEST** t2901 (`src/libs/kt-gpu/tests/export_ring.rs`)

## ADDED Requirements

### Requirement: The render window says what the render costs against the preview
The preview SHALL keep the device time of each moment it composed, with its size. The render window SHALL
show the preview's size against the render's and how many times the pixels of a frame the render draws
when they differ, and a render time predicted from the preview's measured device time per pixel over the
range at the render's size, saying how many of the range's frames were measured; with none measured it
SHALL say to play the range once. A replayed kept composite SHALL NOT count as a measurement.
Why: row 711, review R8 — the owner's preview composes at 640×360 and his render at 1920×1080, nine times
the pixels, and "the final render is slower than the preview" (kkt-26.11.15 item 3) had nothing on screen
to explain it.

#### Scenario: The preview's costs over a range
- **WHEN** moments are recorded at 640×360 with 2 ms and 4 ms of device time, one outside the range, and a
  newer record replaces an older one of the same moment
- **THEN** the range's cost is the newest records' sum over their pixels, the count of moments measured,
  the preview's size; a range with no record has no cost
- **TEST** t2902 (`src/libs/kt-engine/src/player.rs`)

#### Scenario: The window states the size and the prediction
- **WHEN** the render settings window opens on a 1920×1080 project whose preview runs at 640×360, first
  with no preview cost measured, then with the range's frames measured at 4 ms each
- **THEN** it reads "640×360" against "1920×1080" and "9.0×", first "not measured", then a predicted time
  of frames × 4 ms × 9 and how many frames were measured
- **PATH** File → Render… → the Render settings window: the File group under the size estimate reads
  "Preview 640×360 · render 1920×1080: 9.0× the pixels of a frame" and "Render time: not measured — play the
  range once"; after the range plays, "Render time ≈ … (measured … of … frames)"
- **TEST** t2903 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Every render journals its predicted time against its actual one
A finished render SHALL write one line into the telemetry journal with the time predicted when it started
(with how many frames it was measured from and at what size) and the time it took, or that it had no
prediction.
Why: a prediction from the preview's device time leaves out decode and encode; only the line after each
render tells on the owner's machine how far it is from the truth.

#### Scenario: A render's line
- **WHEN** a scripted render with a prediction of 12 s finishes, and one with no prediction
- **THEN** the journal holds "render time: predicted 12.0 s" with the actual time and their ratio, and
  "render time: no prediction" with the actual time
- **TEST** t2904 (`src/apps/kt-app/src/layout_tests.rs`)
