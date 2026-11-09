## ADDED Requirements

### Requirement: The datamosh's schedule is compiled once per build and its jobs move packed planes
The datamosh SHALL compile its stream's schedule once per layout — the rows with their curves per
position, the decoder's node and dependency, the intra-refresh grid and the content keys of the clean
encoder and of the broken decoder — and every later question of the host (what is still needed, what
is held, the frame's key, the frame itself) SHALL read it, with exactly the values a walk of the
dependency chain per question gave. The host SHALL keep one compiled schedule per stage, build and
stream size, made fresh with every build. The codec's compute jobs SHALL upload and read back 8-bit
planes four samples to a word and the residual two 16-bit samples to a word; the conversion of the
source frame into Y'CbCr 4:2:0 SHALL be a step of the encoder's job on the compute device, to the bytes
of the CPU reference, and an advance of the history (the prefetch, the export's pre-roll) SHALL
produce no picture. A dependency chain longer than 8192 frames keeps the intra-refresh
grid counted from its anchor (the walk cut such a chain at an anchor that moved with every frame).

Why: found-defects 135, 144 and the engine audit's row 8 (mechanisms M2, M7): every question built a
schedule and two maps and walked up to 600 nodes reading the curves at each; the prefetch asked it for
every frame of its 2 s window; a 1080-line encoder job read 25 MB back, a word per 8-bit sample.

#### Scenario: The compiled schedule is the walk
- **WHEN** a cut mosh, a stream with mosh/reset/bloom/freeze gates, per-frame rotation, loss and quantiser and a second input of two events, and a held mosh over 1400 frames past the refresh grid are asked the key, the needs with an empty memo and with a memo holding every 7th picture, and the held position, at every position
- **THEN** the answers equal the values pinned from the per-call walk
- **TEST** t2270 (`src/plugins/fx-video-codec/tests/schedule.rs`)

#### Scenario: A layout with the host's cell compiles each position once
- **WHEN** the same questions are asked in a scrambled order (a seek ahead, back, the rest) of a layout with a cell and of one without
- **THEN** the answers are equal, and with the cell each position's rows are read at most once
- **TEST** t2271 (`src/plugins/fx-video-codec/tests/schedule.rs`)

#### Scenario: The jobs move packed planes and the conversion is the reference
- **WHEN** frames of 100×60, 64×48 and 37×21 are encoded from RGBA on the device and from the CPU-converted planes, and a 1280×720 encoder job and decoder job are counted
- **THEN** packets and reconstructions are byte-equal; the encoder reads back the vectors, 2 bytes a residual sample and 1 a reconstruction sample, and uploads the RGBA and the 8-bit reference; the decoder reads back 1 byte a sample
- **TEST** t2273, t2274 (`src/plugins/fx-video-codec/tests/packed.rs`), t1239 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: An advance produces no picture
- **WHEN** the prefetch advances a mosh's history to a frame, then the frame is rendered
- **THEN** the frame then needs nothing, and its render runs no job and gives the bytes of a cold render
- **TEST** t2278 (`src/plugins/fx-video-codec/tests/packed.rs`)
