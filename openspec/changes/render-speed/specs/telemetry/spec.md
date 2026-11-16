## ADDED Requirements

### Requirement: The trace says where an export's time went
`kt-cli trace … export` SHALL report each export of a run: its frames, the timeline it covered against its
wall (the real-time factor), the serial or pipelined form, each stage busy at all with its share of the wall,
and how long its submissions waited for admission on the device rings, with the pieces cut and whether a
display shared the device. A run without an export SHALL say so.
Why: kkt-26.11.15 N3 — the owner's render is slower than his preview; on his iGPU the export's frames are
admitted last on the ring the display shares (loop-laps-regression D2), and only his run can say whether
that, the work at nine times the preview's pixels, or a stage costs his render the time.

#### Scenario: An export read back from its records
- **WHEN** a run holds an export's stage records (compose 18 s, device wait 15 s, an idle readback, a 20 s wall over 10 s of timeline), two of its admissions on a ring with a display and a cut piece, and an admission of the preview before it
- **THEN** the report reads 0.50× real time with four frame workers, compose 90 % and device wait 75 % of the wall, no readback line, two submissions that waited 3 s (15 % of the wall) with one piece cut beside a display; a run with no export says there is none
- **TEST** t2791 (`src/apps/kt-cli/src/trace_query/tests.rs`)
