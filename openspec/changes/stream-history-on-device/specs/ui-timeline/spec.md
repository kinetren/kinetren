## ADDED Requirements

### Requirement: V-fps counts new pictures of the moment asked
The performance panel's V-fps SHALL count the shown frames that are a new picture of the moment the
preview asked for; a frame that shows a stream stage's held picture SHALL NOT count, and SHALL be
counted apart as "held" frames a second, shown next to V-fps in the Diagnostics window while non-zero.
The trace's `frame_show` record SHALL carry the held flag.
Why: found-defects 393 — his trace: 114 shows in 6 s (19/s), 111 of them a held stream frame, 45
distinct pictures, all from the history before his loop; the counter said 20 where "at most 5 are new".

#### Scenario: Every frame held is V-fps 0
- **WHEN** the app takes a second of composites of advancing moments that all show a held stream frame, then a second of complete new moments
- **THEN** the first second's V-fps is 0 and its held count is the composites taken; the second's V-fps counts them and held is 0
- **PATH** play over a Datamosh event whose history is behind → the toolbar reads 0 vfps, Diagnostics shows the held frames a second and the preview's marker; when the history catches up → the vfps rise and the marker goes
- **TEST** t2358 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: The preview marks a held stream frame
While the frame on screen is a held stream frame, the preview SHALL draw a small "catching up" marker in
its corner, without dimming the picture; the Loading scrim keeps its own rule (an incomplete frame
past 180 ms). The marker SHALL not ask for frames at rest.
Why: kkt-26.11.10 J2 — a held frame looked like playback; the Loading scrim was decided by the owner for
a frame with no picture (the flicker "black frame ↔ Loading", items 14–15 of round 14) and stays for that.

#### Scenario: The marker follows the held flag
- **WHEN** the app takes a held composite, then a complete one, then sits at rest
- **THEN** the marker is drawn over the first and not over the second, the scrim over neither, and at rest no frame is asked for
- **PATH** play over a Datamosh event whose history is behind → a small "catching up" note in the preview's corner over the moving picture; it goes when the picture is the moment's own
- **TEST** t2359 (`src/apps/kt-app/src/layout_tests.rs`)
