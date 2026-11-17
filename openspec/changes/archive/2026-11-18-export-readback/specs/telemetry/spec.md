## MODIFIED Requirements

### Requirement: The trace says where an export's time went
`kt-cli trace … export` SHALL report each export of a run: its frames, the timeline it covered against its
wall (the real-time factor), the serial or pipelined form, each stage busy at all with its share of the wall,
and how long its submissions waited for admission on the device rings, with the pieces cut and whether a
display shared the device. It SHALL state the device's own time a frame (from the export's timed frames),
the readback ring's depth, the most and the mean frames in flight and a frame's time from submission to
collection — or, for a run recorded before those records, how long a collect waited — and the run's render
log lines (`EXPORT` journal records) within the export. A run without an export SHALL say so.
Why: kkt-26.11.15 N3 — the owner's render is slower than his preview; on his iGPU the export's frames are
admitted last on the ring the display shares (loop-laps-regression D2), and only his run can say whether
that, the work at nine times the preview's pixels, or a stage costs his render the time. kkt-26.11.18 T3:
his simple region took 52.7 ms from submission to readback for 1.06 ms of device work, which only the
device time beside the latency shows; T4: the render's lines belong to the one trace he sends.

#### Scenario: An export read back from its records
- **WHEN** a run holds an export's stage records (compose 18 s, device wait 15 s, an idle readback, a 20 s wall over 10 s of timeline), two of its admissions on a ring with a display and a cut piece, and an admission of the preview before it
- **THEN** the report reads 0.50× real time with four frame workers, compose 90 % and device wait 75 % of the wall, no readback line, two submissions that waited 3 s (15 % of the wall) with one piece cut beside a display; a run with no export says there is none
- **TEST** t2791 (`src/apps/kt-cli/src/trace_query/tests.rs`)

#### Scenario: The export view states the device time, the ring and the render's lines
- **WHEN** a run holds an export with three timed export frames of 1, 1 and 4 ms, its ring record (depth 16,
  at most 12 in flight, 30 summed over 3 submissions), its latency record (150 ms over 3 frames), two
  `EXPORT` journal lines inside it and one after it; and a second run's export without ring records but
  with three `export_frame` spans of 10, 15 and 20 ms
- **THEN** the first report says 1.00 ms a frame of device work (p50 of 3), up to 12 in flight of 16 (mean
  10.0), a frame back 50.0 ms after its submission, and lists the two lines inside the export and not the
  later one; the second says a collect waited 15.0 ms (p50 of 3)
- **TEST** t2962 (`src/apps/kt-cli/src/trace_query/tests.rs`)
