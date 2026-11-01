# Spec Delta

## ADDED Requirements

### Requirement: The datamosh is a motion-compensated codec run inside the effect and broken by its rows
The kind `datamosh` SHALL be a SIMULATION of a broken inter-frame stream, never an image
operation: it SHALL contain its own block codec — Y'CbCr 4:2:0 planes at its encoded height,
block-matching motion estimation within a search range, a closed prediction loop over a
reference buffer, a DCT residual quantised by QP and a GOP with I-frames at the stream start, at
the cut and every `gop` frames (0 — only at the stream start, the cut and the resets). Its breakages SHALL be rows of
its passport, and every quantity that may change from frame to frame SHALL take curves: the gates
`mosh` (I-frames dropped while on), `reset` (a key rising to on forces an intact I-frame), `bloom`
(the run's first P-frame repeats), `freeze` (the reference stops updating), the quantiser, the vector
scale and rotation, the residual kept and the macroblock-loss probability. Only the structural rows
take none: the GOP, the block, the search range, the encoded height, the frames from the event before,
the second input (the event on the track above or below), the vector swap and the seed. No range is
capped below what the quantity physically allows: GOP to 600, search to 64 px, 300 frames before the
cut, 144–1080 lines, vector scale ±16; the quantiser keeps the codec's own 0–51.

Why: the owner asked for "a real motion-compression path that we can break carefully with
parameters", not the glitch imitations we have (F-FX-011; root rule imitation ≠ simulation); on the
first build: "no curves … no way to reset the effect, at least by keys on the curves … on the beat …
the GOP is limited to 60 for some reason, and probably the other parameters too".

#### Scenario: An intact stream reproduces its input within the quantiser
- **WHEN** a moving picture goes through the codec with no breakage at QP 10 and at QP 30
- **THEN** every decoded plane differs from the source plane by no more than the quantiser's bound, the mean error grows with QP, and the error does not grow along the GOP (closed loop)
- **TEST** t1235 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: A dropped I-frame at the cut moves the previous shot with the new shot's motion
- **WHEN** the stream starts in the event before (shot A, a still texture) and the `mosh` gate is on at the cut into shot B (another texture moving right), so the cut's I-frame is dropped
- **THEN** after the cut the output keeps A's texture, displaced by B's motion, and does not show B; with the I-frame intact the output is B within the quantiser
- **TEST** t1236 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: Bloom repeats one vector field
- **WHEN** the `bloom` gate is on over frames b … b+N
- **THEN** each frame b+1 … b+N is the previous output predicted by packet b's vectors (plus its residual), so the moving region keeps travelling by the same displacement, and after b+N the stream continues with its own packets
- **TEST** t1237 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: A frame is the same bytes however it is reached
- **WHEN** frame N inside a mosh — constant gates, a bloom run, a freeze run, a beat of mosh on / reset key / mosh off drawn by curves — is rendered after rendering 0 … N−1 in order, after a seek with an empty memo, with a memo of 1 byte, and twice on two threads
- **THEN** the bytes are identical in every case
- **TEST** t1238 (`src/plugins/fx-video-codec/tests/codec.rs`), t1239 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: The other breakages change the stream they name
- **WHEN** the `freeze` gate is on over a run, the vectors are taken from a second input, the vectors are scaled by 0 or swapped, and macroblocks are lost with probability 0 and 1
- **THEN** a frozen run predicts every frame from its first picture and leaves its trace after it; transplanted vectors move the own picture by the second input's motion; scale 0 holds the picture; probability 0 equals the clean stream and probability 1 differs from it, deterministically per seed
- **TEST** t1243 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: The person resets the mosh on the beat with curve keys
- **WHEN** the `mosh` gate is on and a `reset` key rises to on at frame 10; and, in another stream, `mosh` turns off at frame 5 with a GOP of 12
- **THEN** frame 9 is the previous shot dragged, frame 10 is the new shot's intact keyframe and frame 14 is the new shot; in the other stream the cut's keyframe was dropped (frame 9 moshed) and the keyframe at 12 is intact; the curve values are part of the keys
- **TEST** t1245 (`src/plugins/fx-video-codec/tests/codec.rs`)

#### Scenario: A region of interest confines the mosh
- **WHEN** the host gives a region covering the left half of the frame (a stand-in for a mask or an object detector)
- **THEN** inside it the frame is the broken stream's, outside it the intact stream's, byte for byte; without a region the whole frame moshes; the region's identity is in the output key
- **TEST** t1244 (`src/plugins/fx-video-codec/tests/codec.rs`)

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

#### Scenario: A reset key on the event ends the mosh on the beat
- **WHEN** the event carries a `reset` curve rising to on at a frame, with the `mosh` gate on
- **THEN** the engine reads the slot's rows per frame with their curves: before the key the frame is far from the intact codec's, on and after it close to it; a seek to a frame after the key equals playing up to it
- **TEST** t1246 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: Seeking to a frame equals playing up to it
- **WHEN** a project with a cut-mosh is composed at frame N after composing 0 … N−1, and by a fresh renderer with a fresh memo directly at N
- **THEN** the frames are byte-identical, and differ from the same project without the slot
- **TEST** t1241 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: Export in chunks equals export in one go
- **WHEN** a range containing a mosh is exported in one go, and again as three range exports — each with a fresh renderer, memo and cache, one starting inside the mosh (harder than a chunk boundary, which keeps the memo)
- **THEN** the concatenated pieces are byte-identical to the whole
- **TEST** t1242 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: The order asks what the stage reads
- **WHEN** `layers_at` is asked at a frame inside a mosh with an empty memo, and again after that frame was composed
- **THEN** the first answer holds the history at the codec size (the event before included) and no frame at the layer's ordinary quality; the second holds only the next frame's own source
- **TEST** t1248 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: A person moshes a cut in the editor
- **PATH** add two clips back to back on one track → select the second → FX → pick Datamosh "Cut mosh" → play across the cut → the first clip's picture keeps moving with the second clip's motion after the cut
- **WHEN** the live scenario runs on our fixtures
- **THEN** the frames after the cut differ from the same frames without the effect, and the screenshots of each step are saved
- **TEST** live `datamosh.py` (`src/scripts/live/datamosh.py`, via `src/scripts/live-check.sh`)
