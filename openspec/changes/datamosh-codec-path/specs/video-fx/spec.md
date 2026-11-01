# Spec Delta

## ADDED Requirements

### Requirement: The datamosh is a motion-compensated codec run inside the effect and broken by its rows
The kind `datamosh` SHALL be a SIMULATION of a broken inter-frame stream, never an image
operation: it SHALL contain its own block codec — Y'CbCr 4:2:0 planes at its encoded height,
block-matching motion estimation within a search range, a closed prediction loop over a
reference buffer, a DCT residual quantised by QP and a GOP with I-frames at the stream start, at
the cut and every `gop` frames. Its breakages SHALL be rows of its passport: I-frames dropped from
a frame on, a P-frame repeated (bloom), a frozen reference, vectors scaled, rotated or swapped,
vectors taken from the event on the track above or below, residual loss, a macroblock-loss
probability with a seed, the stream starting in the event before.

Why: the owner asked for "a real motion-compression path that we can break carefully with
parameters", not the glitch imitations we have (F-FX-011; root rule imitation ≠ simulation).

#### Scenario: An intact stream reproduces its input within the quantiser
- **WHEN** a moving picture goes through the codec with no breakage at QP 10 and at QP 30
- **THEN** every decoded plane differs from the source plane by no more than the quantiser's bound, the mean error grows with QP, and the error does not grow along the GOP (closed loop)
- **TEST** t1235 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: A dropped I-frame at the cut moves the previous shot with the new shot's motion
- **WHEN** the stream starts in the event before (shot A, a still texture) and the I-frame at the cut into shot B (another texture moving right) is dropped
- **THEN** after the cut the output keeps A's texture, displaced by B's motion, and does not show B; with the I-frame intact the output is B within the quantiser
- **TEST** t1236 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: Bloom repeats one vector field
- **WHEN** a P-frame at frame b is repeated N times
- **THEN** each frame b+1 … b+N is the previous output predicted by packet b's vectors (plus its residual), so the moving region keeps travelling by the same displacement, and after b+N the stream continues with its own packets
- **TEST** t1237 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: A frame is the same bytes however it is reached
- **WHEN** frame N inside a dropped-I mosh is rendered after rendering 0 … N−1 in order, after a seek with an empty memo, with a memo of 1 byte, and twice on two threads
- **THEN** the bytes are identical in every case
- **TEST** t1238 (`src/plugins/fx-video-codec/tests/codec.rs`), t1239 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: The other breakages change the stream they name
- **WHEN** the reference is frozen, the vectors are taken from a second input, the vectors are scaled by 0 or swapped, and macroblocks are lost with probability 0 and 1
- **THEN** a frozen reference predicts every frame from the same picture; transplanted vectors move the own picture by the second input's motion; scale 0 holds the picture; probability 0 equals the clean stream and probability 1 differs from it, deterministically per seed
- **TEST** t1243 (`src/plugins/fx-video-codec/tests/codec.rs`)

### Requirement: The engine runs a stream stage in the plan's common code with random access
A layer whose chain holds a `Stream` slot SHALL take as its pre-FX frame the stage's output,
computed in the plan's common code before either executor runs, fitted to the layer's decode size
and opaque; the slot SHALL be removed from the chain the executors see. The order SHALL ask the
stream's frames at the codec's size — the frame at N, and the history the memo does not cover —
and no frame at the ordinary decode quality. The layer's content key SHALL carry the decoded
frame's key. Export SHALL pre-roll the codec before a frame whose history the memo lacks, in
batches bounded by its cache budget. A stream slot on a reel instance or a track stage SHALL be
skipped.

Why: invariants 4, 7 and 11 — the order equals the read, one composition code, the same bytes
whatever the start (`datamosh-codec-path` design D3–D6).

#### Scenario: Seeking to a frame equals playing up to it
- **WHEN** a project with a cut-mosh is composed at frame N after composing 0 … N−1, and by a fresh renderer with a fresh memo directly at N
- **THEN** the frames are byte-identical, and differ from the same project without the slot
- **TEST** t1241 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: Export in chunks equals export in one go
- **WHEN** a range containing a mosh is exported with one chunk, with chunks of three frames, and from a start inside the mosh
- **THEN** the frame bytes are identical where the ranges overlap
- **TEST** t1242 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: The order asks what the stage reads
- **WHEN** `layers_at` is asked at a frame inside a mosh with an empty memo, and again after that frame was composed
- **THEN** the first answer holds the history at the codec size (the event before included) and no frame at the layer's ordinary quality; the second holds only the next frame's own source
- **TEST** t1248 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: A person moshes a cut in the editor
- **PATH** add two clips back to back on one track → select the second → FX → pick Datamosh "Cut mosh" → play across the cut → the first clip's picture keeps moving with the second clip's motion after the cut
- **WHEN** the live scenario runs on our fixtures
- **THEN** the frames after the cut differ from the same frames without the effect, and the screenshots of each step are saved
- **TEST** t1249 (`src/apps/kt-app/tests/live/datamosh_cut.rs`)
