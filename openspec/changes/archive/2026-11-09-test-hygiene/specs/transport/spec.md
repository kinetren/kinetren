## ADDED Requirements

### Requirement: Space read by a slow frame keeps its meaning
The play key's presses and releases SHALL be handled in the order a frame read them. A hold SHALL be
proven: a frame that began `SPACE_HOLD_MS` after the frame that read the press finds no release in its
input; a release of a press never proven held is a tap. The double-press window SHALL be measured
between frame starts and widen by the busy frame the program measures.
Why: under the full live run `space_hold` read its two quick presses as one (a pause, not the stop at
the start), and a tap whose release a slow frame read past the threshold did nothing at all, neither a
tap nor a hold (found-defects 99, 166). The program cannot tell 150 ms from 450 ms inside a 450 ms
frame; it can keep from deciding against the person on that.

#### Scenario: A tap read late and two presses in one frame
- **WHEN** the press is read by one frame and its release by a frame 400 ms later; then one frame reads
  two presses
- **THEN** the tap plays (no boost); the two presses stop at the project start
- **TEST** t2212 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The double press on busy frames
- **WHEN** playback runs on frames 450 ms apart and the second press is read 450 ms after the first; then
  on fast frames a second press comes 2 s after the pause
- **THEN** the first pair stops at the project start; the second press resumes
- **TEST** t2213 (`src/apps/kt-app/src/layout_tests.rs`); live `space_hold`
