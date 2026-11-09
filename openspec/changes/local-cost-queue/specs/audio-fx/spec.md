## ADDED Requirements

### Requirement: An audio kernel computes a per-block constant once
An audio kernel SHALL NOT recompute per sample, per grain or per chunk a value that is constant over a
larger unit, nor allocate per unit, where computing it once is exact and measurably cheaper: the camera
microphone's µ-law denominator once and its 128 expansion
levels as a table, its noise suppressor's window as a table and its spectrum in kept buffers, its rows by
index; Pitch's WSOLA window as a table and its candidate offsets without a list per grain; VHS Tape's sound
designs its linear-track filter from a table of the design's cosines and keeps its dropout starts while
their law stands, stepping them a cell at a time. The limiter's per-sample `ceil/thr` stays: a memo of
it measured 3 % slower (the division is off the loop's critical path). Every audio kind keeps the bytes it gave before, at its defaults, at heavy settings, with curves on
its rows, on the CPU's and the baseline path.
Why: found-defects rows 210, 211, 311, 312, 313 (kernel-specialization's and stream-layout-per-build's
class walks).

#### Scenario: The curved and switched forms keep their bytes
- **WHEN** the limiter with a moving threshold and ceiling, Pitch (WSOLA) with a curved shift, VHS Tape's sound with curves on gap, spacing, bias, dropouts and tracking, the camera microphone with each codec (AAC, 16 kHz linear, µ-law) and curves on its rows, and Datamosh's sound run 4 s, on the CPU's and the baseline path
- **THEN** every digest equals the one printed on the tree before this change
- **TEST** t2315 (`src/libs/kt-engine/tests/local_cost.rs`)

#### Scenario: The µ-law table is the formula
- **WHEN** every f64 sample on a 2²⁰-step grid of [−1.1, 1.1], the 128 level boundaries and ±0 go through the tabled µ-law and the formula
- **THEN** the results are equal to the bit
- **TEST** t2316 (`src/plugins/fx-video-camera/tests/mic.rs`)

#### Scenario: The fixes pay
- **WHEN** every audio kind is timed against the reference kernel at its defaults and heavy settings
- **THEN** each costs within its bound in t1980's units (the before/after figures of VHS Tape's curved head, the camera microphone and Pitch are in the change's design)
- **TEST** t1980 (`src/libs/kt-engine/tests/audio_fx_cost.rs`)
