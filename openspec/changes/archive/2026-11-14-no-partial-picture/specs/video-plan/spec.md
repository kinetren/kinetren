## MODIFIED Requirements

### Requirement: The GPU preview's picture stays on the device
The preview SHALL compose through `VideoGpuExec::present`: the passes of
`execute_into`, then a display pass into a texture the UI draws, with no wait for the device and
no copy of the frame to the CPU. The display target SHALL hold the bytes the readback path shows,
premultiplied as `Color32::from_rgba_unmultiplied` does. An incomplete frame SHALL leave the target
as it is, whether or not anything has been shown, and SHALL cost no drawing work: a plan with a layer
lacking its frame (`FramePlan::starved`) SHALL stop before its first upload, any other incomplete plan
after its resolve, with no pass encoded and no display pass. `execute_into` SHALL likewise leave an
incomplete frame's bytes empty with nothing uploaded, encoded or read back; only `execute_once` (an
export's last resort, named in its report) SHALL produce an incomplete frame's bytes. An executor with no display target SHALL compose through
the readback path; with no executor there is no picture. Export SHALL keep its readback.
Why: at 1080p the UI thread spent about 33 ms of a 40 ms frame uploading, waiting for the device
and copying the frame out, before converting it and uploading it a second time (`kt-cli preview`,
llvmpipe; preview-throughput, 2026-11-04). The p95 stall per UI frame went from 36–55 ms to 7–11 ms.

#### Scenario: The picture does not change when the readback goes
- **WHEN** half-transparent events are presented and executed at the same moments, across a
  transition and over effects
- **THEN** the display target equals the readback's bytes premultiplied by egui's rule
- **TEST** t1525 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: An incomplete frame holds the last full one
- **WHEN** a starved frame is presented after a full one, with and without the glow
- **THEN** the display target is unchanged both times (no option draws an incomplete frame)
- **TEST** t1526 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: An incomplete frame costs no drawing
- **WHEN** a starved plan is presented, executed through `execute_into` and executed through `execute_once`
- **THEN** the present and `execute_into` upload no byte, submit nothing, run no pass, read nothing back and `execute_into` leaves no bytes; `execute_once` returns the frame's bytes, reported incomplete
- **TEST** t2530 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: A plan knows its starvation
- **WHEN** a frame's plan is built over a cold cache, over a cache holding every layer but one, and over a warm one
- **THEN** `FramePlan::starved` is true, true, false
- **TEST** t2531 (`src/libs/kt-engine/tests/no_partial.rs`)

#### Scenario: The glow and a new size
- **WHEN** a frame is presented with the glow, and then a complete frame at another preview size
- **THEN** the glow holds the 32×18 picks of the readback's pixels, and the new size is a new
  texture with a new generation, while the same size keeps its generation
- **TEST** t1527 (`src/libs/kt-gpu/tests/preview_display.rs`)

#### Scenario: The program draws from the device
- **PATH** open a project with a flat clip → the light frame shows → click the ruler over a clip with
  Edges → the frame goes black, and the program reports the picture was drawn on the device
- **TEST** live scenario `src/scripts/live/gpufx.py`

### Requirement: No stage runs over a stand-in input
A composite SHALL NOT give any stage of the plan an input that is not that stage's own position — a
stream stage's picture of another position, a sub-composite of a reel or of a track stage that lacks a
layer, a neighbour of a frame pair, a decoded frame of another time — and run the stages after it (the
event's later effects such as VHS Tape or Camera ISP, the track's stages, the reel's stages) over it at
the moment asked. A layer whose own input is not there SHALL leave the frame incomplete; an incomplete
composite SHALL NOT be presented over the picture on screen and SHALL NOT be kept as the moment's
composite; the preview keeps its last full picture under its Loading look, and a preview that has none
keeps its place empty under Loading: no incomplete composite is ever presented, the first one included
(kkt-26.11.14 M1, which replaced round 14's "a picture under the scrim rather than an empty one"). A kept composite of
the exact moment SHALL still show at once, without the history. An export SHALL compose every frame from
its own positions, never short.
Why: kkt-26.11.10b K5 — "при кэтчинге - у меня vhs tape будто бы накладывается в этот момент
актуальный … либо есть кадр либо нет, зачем накладывать эффект если предыдущий не готов?": the
playing preview ran the event's VHS Tape at the moment asked over a Datamosh picture of a position
the history had reached.

#### Scenario: A playing frame whose history is behind is Loading, not a held picture
- **WHEN** a paced renderer composes a frame of a cut mosh whose history the prefetch has walked to ten positions behind it, then the history catches up
- **THEN** the first composite is incomplete, the mosh layer has no picture, the renderer reports at least eight positions short, and the executor presents nothing; after the catch-up the composite is complete, reports nothing short and is byte-identical to the unpaced renderer's
- **TEST** t2410 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: VHS Tape is never laid over another position's mosh
- **WHEN** the mosh event carries VHS Tape after Datamosh and its track carries VHS Tape as a track stage, and the frame is composed while the history is behind and again after it caught up
- **THEN** the short composite runs no chain of the mosh layer and is incomplete with nothing presented; the caught-up composite equals the unpaced renderer's bytes
- **TEST** t2411 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: An export composes every frame from its own positions
- **WHEN** a range of the mosh event with VHS Tape after it is exported through the serial export and through the pipeline
- **THEN** no frame is short, every frame's stream key is the stage's key of its own position, and the frames equal the unpaced renderer's at each moment
- **TEST** t2412 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A reel that holds a mosh reports its catch-up to the root
- **WHEN** the mosh project is placed as a reel and the root frame is composed through a paced renderer with the inner history behind
- **THEN** the root composite is incomplete and reports the inner positions short; after the inner history catches up the root composite is complete and reports nothing short
- **TEST** t2413 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The player answers a short frame with its catch-up and a kept moment at once
- **WHEN** a playing player is asked a frame of the mosh whose history is behind, then the same moment after its composite was kept and the memo dropped
- **THEN** the first answer is incomplete with a catch-up count above zero; the kept moment's answer is complete with a catch-up of zero, without the history
- **TEST** t2414 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A frame pair lacking a side is incomplete
- **WHEN** a slowed event in the frame mode "blend" is composed at a moment between two source frames with both frames cached, then with the neighbour missing, then with the base frame missing
- **THEN** the first is complete and blended; the other two are incomplete and their layer has no picture (neither side stands in at full weight)
- **TEST** t2418 (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: An offline frame keeps only a complete composite
- **WHEN** the reel strip's and the poster's offline frame is composed with one layer's source absent, and a poster is made of a project whose file is missing
- **THEN** the incomplete frame leaves no picture (no partial cell); the poster of the project with the missing file is the frame with the placeholder, as the preview shows it
- **TEST** t2419 (`src/apps/kt-app/src/reel_preview.rs`)
