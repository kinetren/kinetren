## ADDED Requirements

### Requirement: A frame an unwritten chunk needs is never evicted
Every frame named by a chunk of the pipelined export SHALL be held in the cache (capability
`media-index`) from the moment that chunk's decode requests are made until its last frame is written,
whether it was decoded for that chunk or found there. Decode SHALL wait while the bytes held exceed the
look-ahead budget. A frame a span did not give SHALL be asked again alone before it is composed, as the
serial export's pre-roll does. The frames, the samples and `missing_video_layers` of the pipelined export SHALL
equal the serial export's and be the same on every run, at any relative speed of the stages.
Why: the cache stays full during an export, a composite's reads refresh frames never needed again,
and these outlived frames decoded and not yet read. A different set of frames lost a layer on
each run of the demo (5, 7, 8 and 10 layers), and the file was damaged. The earlier promise of this
capability rested on the look-ahead fitting the budget, which does not decide what LRU drops; t1451
never fills the cache.

#### Scenario: Under pressure the pipelined export is the serial export, every run
- **WHEN** 180 frames of the demo at 640×360 with a quarter of the budget, over its first transitions,
  are rendered serially, and three times pipelined into a writer slow enough that decode runs at its
  look-ahead limit
- **THEN** every run equals the serial export frame by frame, with zero missing layers
- **TEST** t1601 (`src/libs/kt-engine/tests/export_pipeline.rs`)

#### Scenario: A span that fails half way loses no frame
- **WHEN** a source's span stops with an error half way through a chunk, once, while each frame can
  still be read alone, and the range is exported serially and pipelined
- **THEN** both count the one failed span and no frame is written without its layer
- **TEST** t1605 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: The file's streams equal the serial export's
- **WHEN** `kt-cli export` writes a project pipelined with `--encoder-threads 1`, and with `--serial`
- **THEN** the video packets of the two files are identical and so are the audio packets; the
  interleave of the file may differ, because the serial export writes the audio after the video
- **TEST** t1603 (`src/apps/kt-cli/tests/cli.rs`)

### Requirement: A frame written without a layer is reported where it happens
A frame composed without one of its layers SHALL be reported during the export, for the serial and
the pipelined export alike and in the same words: one log line per run of consecutive such frames,
in frame order, naming the time of the first and the last frame and the media that gave no picture.
The render window SHALL open its log by itself when the run has such a line; kt-cli SHALL print it.
The export goes on: a layer no decoder can give is not worth a lost render, and the person is told
where to look.
Why: the only trace was a count at the end ("missing layers 5"), which says neither which frames nor
which media, so a damaged file could pass for a finished one.

#### Scenario: A source that fails for a stretch is named with its frames
- **WHEN** a source cannot give its frames for two stretches of a range, and the range is rendered
  serially and pipelined
- **THEN** both give the same `IncompleteFrames` events, in frame order, one per run of frames, naming
  that media and those times; the frames and the missing count are equal
- **TEST** t1602 (`src/libs/kt-engine/tests/export_options.rs`)

### Requirement: A failed render's window says what failed in the person's words
The error of a failed render SHALL be the export's own message, without the name of an internal error
kind. When a run ended without opening an encoder, the window's Video row SHALL say that no encoder
opened, and SHALL NOT keep saying the encoder is being chosen.
Why: "Error: io: …" names a Rust type, and "choosing the encoder…" under a finished failure says
the program is still working on it.

#### Scenario: A run that opened no encoder
- **WHEN** a scripted render ends with an export error before any encoder opened
- **THEN** the window's Video row reads "no encoder opened", its error line carries the message with
  no "io:", and the log section is open
- **PATH** File → Render… → Render… → the window: the Video row "no encoder opened", the red line
  with the reason, the log open, Open log and Close
- **TEST** t1604 (`src/apps/kt-app/src/layout_tests.rs`)
