## ADDED Requirements

### Requirement: A simulated device's sound rows raise the file by content
A project in which a slot of a paired video kind holds one of its sound rows (the rows its own half
never reads, `Decl.foreign_rows`: the VHS deck's audio output, linear tracks, Dolby, audio head, bias,
record level; the stream's audio bitrate and prediction; the camera's microphone) off its default
SHALL be written as project format 4 and as `metadata["kt"]` v16 in the exchange file; a project
without one SHALL keep its former version and text. An older build SHALL refuse such a file naming the
version, instead of opening it and dropping the device's sound settings on its next save. The audio
halves themselves are new plugin ids and need no raise: an older build keeps them as missing slots.
Why: an older build reads a known slot by its passport and silently ignores rows it does not name
(the precedent of pitch-tail-choice's format 3).

#### Scenario: The deck's audio output raises the format
- **WHEN** a project's VHS Tape is at its defaults; then its audio output is Normal
- **THEN** the first writes format 1 with no sound row; the second writes format 4 with
  `"audio_out": "normal"`, reads back equal, and a version above this build's ceiling is refused
- **TEST** t1485 (`src/libs/kt-model/tests/sim_pairs.rs`)
