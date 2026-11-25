## Purpose

The instruments of one frame — waveform, RGB parade, vectorscope, histogram and false colour — that read the picture the
preview shows (or, on a pause, the full-size frame of the same plan) without a second composition path, with honest 8-bit
scales and no cost at rest.

## ADDED Requirements

### Requirement: A scope measures the picture the preview shows
Every instrument SHALL be counted from the composite the preview shows, whatever path showed it: the display target, a kept
composite or the bytes of an executor with no display target. The composite SHALL be copied without the frame waiting
for the copy, and the last one SHALL arrive with no further frame asked. An incomplete frame SHALL be measured nowhere.
While playing no instrument SHALL order a decode or compose a plan.
On a pause, the moment SHALL be measured once more from the full-size composite of the project the player holds, made on
the offline route; the window SHALL say the measured size, and a moment that cannot be composed offline SHALL keep the
preview's reading, named so.
Why: kkt-26.11.20 A8 ("Окно, кадр превью"): the preview's frame while playing, a full one on a pause; invariant 7.

#### Scenario: The tap is the shown composite
- **WHEN** a composite with a single 255 pixel is presented with the tap on, presented again from the kept composites, and composed by an executor with no display target
- **THEN** the three tapped frames equal `execute_into`'s bytes; a present with both buffers in flight taps nothing and is not delayed
- **TEST** t3400 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: The paused frame is the export's
- **WHEN** a 1280×720 project with a one-pixel clipped highlight is paused with the preview at 1/4, with a stream stage in it, and inside a composition
- **THEN** the measured full frame equals the exported frame of that moment of what the player holds, the window says 1280×720; a second look at the moment composes nothing; an edit mid-drag measures it once, at the release
- **TEST** t3401 (`src/apps/kt-app/src/scopes_tests.rs`)

#### Scenario: Playing never waits for a scope, and the last frame arrives
- **WHEN** a range is played with the tap on and the copy held back three frames, then paused
- **THEN** no frame's stages include a wait for a tap, the newest finished one is handed on, and the last shown frame's tap arrives with no new ask
- **TEST** t3402 (`src/libs/kt-engine/tests/compose_queue.rs`)

#### Scenario: A slow chain and a GL display are measured too
- **WHEN** the compose thread answers with bytes (the slow route, no display target)
- **THEN** the same sink gets the same picture; an incomplete answer leaves it as it was
- **TEST** t3404 (`src/libs/kt-engine/tests/compose_queue.rs`)

### Requirement: The instruments count every pixel by fixed integer rules
The waveform SHALL bin the Rec.709 integer luma of every column, the parade R, G and B, the histogram luma, R, G and B, the
vectorscope the integer Cb and Cr, of the picture over black. A bin at 0 or 255 that is not empty SHALL be marked. The
scale SHALL be 0–255 or 0–100 %; no finer scale and no smoothing SHALL be offered. The same frame SHALL give the same counts.
Why: the path is RGBA8; Resolve's smoothing understates excursions (F-CLR-005).

#### Scenario: Known bars give known counts
- **WHEN** a frame of the six 75 % bars, black, white and one half-transparent white pixel is counted twice
- **THEN** the parade's columns stand at the bars' values, the vectorscope's points fall into the 75 % targets, the histogram holds exactly the pixel counts (the half-transparent pixel at 128), one 255 pixel lights its mark, and both runs are equal
- **TEST** t3403 (`src/apps/kt-app/src/scopes_tests.rs`)

#### Scenario: False colour paints the frame it is shown with
- **WHEN** false colour is on over a gradient, on the device and on the bytes path, then off
- **THEN** both paint the same zones by the same integers, the glow and the kept composite keep their bytes, and off gives t1525's bytes
- **TEST** t3405 (`src/libs/kt-gpu/tests/preview_display.rs`)

### Requirement: Scopes cost nothing at rest
The tap SHALL be asked only while the window is open or false colour changes, and only for a moment not yet measured. A
pause with the mouse moving over the window SHALL ask no composite, no copy and no counting. Closing the window SHALL end
its thread and free its buffers.
Why: the owner, 2026-11-07: the program at rest costs nothing, and this is tested.

#### Scenario: The idle scene with scopes open
- **WHEN** live `idle` opens the window with four instruments on the heavy showcase, pauses, and moves the mouse over the window and the preview for ten seconds
- **THEN** `ScopeTap`, `ScopeCount`, `composites`, `device_submits` of ours and `sched_jobs` stand still after the moment's two measurements
- **TEST** t3410 (`src/scripts/live/idle.py`)
