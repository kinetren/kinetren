## Purpose

The instruments of one frame — waveform, RGB parade, vectorscope, histogram and false colour — that
read the picture the preview presented (or, on a pause, the full-size frame of the same plan) without
a second composition path, with honest 8-bit scales and no cost at rest.

## ADDED Requirements

### Requirement: A scope measures the composite of the one frame plan
Every instrument SHALL be computed from the composite the executor presented for the preview, reduced
on the device by an integer box mean into an analysis frame at most 512 pixels wide with its block
minimum and maximum, and read back without the frame waiting for it. On a paused moment, when the
measurement of the full frame is on, the instrument SHALL be computed from the full-resolution
composite of the same plan made once on the offline route and reduced by the same rule on the CPU,
and the window SHALL name the measured quality. No instrument SHALL order a decode or compose a plan
of its own while playing.
Why: invariant 7 — one composition path; a scope of a picture the preview did not show would lie.

#### Scenario: The device reduction equals the CPU rule
- **WHEN** a stored test composite (bars 0/64/128/192/255, a single 255 pixel in a black field, a 4K gradient) is presented with the tap on, and the same bytes are reduced by the CPU rule
- **THEN** the analysis frame, its minimum and its maximum are equal byte for byte; the lone 255 pixel is present in the maximum
- **TEST** t3400 (`src/libs/kt-gpu/tests/scope_tap.rs`)

#### Scenario: The paused frame is the export's
- **WHEN** a 1280×720 project with a one-pixel clipped highlight is paused with the preview at 1/4, the full-frame measurement on
- **THEN** the measured frame is the full-size composite of the plan the export composes at that moment (equal to `compose_export_into` reduced by the same rule); the window names "1/1"; a second pause on the same moment composes nothing
- **TEST** t3401 (`src/libs/kt-engine/tests/scope_measure.rs`)

#### Scenario: Playing never waits for a scope
- **WHEN** a range is played with the Scopes window open and the tap's readback held back artificially for three frames
- **THEN** every frame is presented on time (the frame's stages do not include a scope wait), and the scope shows the newest finished tap
- **TEST** t3402 (`src/libs/kt-engine/tests/scope_measure.rs`)

### Requirement: The instruments count by fixed integer rules and never hide clipping
The waveform SHALL bin the Rec.709 integer luma of every column, the parade the R, G and B of every
column, the histogram the luma and R, G, B of the frame, the vectorscope the integer Cb/Cr; false
colour SHALL paint the preview by luma zones. The first and last row of the waveform and parade SHALL
be lit where any pixel of the full frame reached 0 or 255. The scale SHALL be labelled 0–255 or
0–100 %; no finer scale SHALL be offered. The same frame SHALL give the same counts on every run.
Why: the path is RGBA8; Resolve's smoothing understates excursions (note F-CLR-005).

#### Scenario: Known bars give known counts
- **WHEN** the instruments count a synthetic frame of the six 75 % colour bars plus black and white
- **THEN** the parade columns sit at the bar values, the vectorscope points fall in the 75 % targets, the histogram has exactly the bars' pixel counts, two runs give equal counts
- **TEST** t3403 (`src/apps/kt-app/src/scopes_tests.rs`)

#### Scenario: A lone clipped pixel lights the edge
- **WHEN** a frame of mid grey with one pixel at 255 in a 4K project is reduced to 512 px and counted
- **THEN** the waveform's top row is lit at that pixel's column, the histogram's 255 bin is not counted from the mean but the extent mark is shown
- **TEST** t3404 (`src/apps/kt-app/src/scopes_tests.rs`)

#### Scenario: False colour paints the zones
- **WHEN** false colour is on over a gradient frame
- **THEN** the overlay colours the 0–5 % band blue, 40–50 % grey, 95–100 % red, and the picture under it is the same composite
- **TEST** t3405 (`src/apps/kt-app/src/scopes_tests.rs`)

### Requirement: Scopes cost nothing at rest
The tap SHALL be asked only while the Scopes window is open or false colour is on, and only for a
presented frame whose key differs from the last measured. A pause with the mouse moving over the
window SHALL run no pass, no readback and no scope counting. Closing the window SHALL stop every tap.
Why: the owner, 2026-11-07 — the program at rest costs nothing, and this is tested.

#### Scenario: The idle scene with scopes open
- **WHEN** the live `idle` scene opens the Scopes window with four instruments on a project with heavy effects, pauses, and moves the mouse over the window and over the preview for ten seconds
- **THEN** the `ScopeTap` and `ScopeCount` work counters stay at zero after the first measurement, and the frames asked are only the hover frames the rest rules allow
- **TEST** t3410 (`src/scripts/live/idle.py`)
