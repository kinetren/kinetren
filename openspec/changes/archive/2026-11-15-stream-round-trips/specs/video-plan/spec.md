## ADDED Requirements

### Requirement: The look-ahead walks only what a stage on screen reaches
The far look-ahead of a stream history SHALL walk nothing while no stream stage is on screen, and no
look-ahead walk SHALL take more than `STREAM_WALK_SECS` of frames. The walk of the frame ON SCREEN SHALL
run at the display's class; the frames after it at the look-ahead's.
Why: found-defects 660, 662, kkt-26.11.15 N2 — with a stage frame size of 0 the far walk was 33.5 million
frames, 26.5 s of the prefetch thread every pass, and a piece without Datamosh played at 8.7 V-fps
against kt26.11.10's 21.6; the frame on screen's history waited behind every present at the look-ahead's
class.

#### Scenario: No stage on screen walks nothing far
- **WHEN** a piece with no stream stage is on screen in a project with a Datamosh elsewhere, playing
- **THEN** the far walk is 0 frames, and inside the stage it is at most a minute of frames
- **TEST** t2710 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The frame on screen walks at the display's class
- **WHEN** the look-ahead at `near` walks a cold history to the frame on screen and eight frames after it
- **THEN** the jobs up to the frame on screen run at `now` and the rest at `near`
- **TEST** t2711 (`src/libs/kt-engine/tests/decode_storm.rs`)

### Requirement: The slow route's picture is shown from the display's memo
A complete composite of the slow-chain device, once kept on the display's executor, SHALL be shown from
there at once, not handed to the interface as bytes to upload again.
Why: found-defects 622 — the slow route's frame was written into the present memo and uploaded again into
the interface's texture.

#### Scenario: The slow route shows its kept picture
- **WHEN** a moment of a chain slower than `SLOW_CHAIN` is composed on the slow device
- **THEN** it comes back on the display, the next ask is a replay with no slow run, and a moment not kept runs on the slow device and is shown on the display
- **TEST** t2705 (`src/libs/kt-engine/tests/decode_storm.rs`)
