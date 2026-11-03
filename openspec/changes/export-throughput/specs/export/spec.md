## ADDED Requirements

### Requirement: An export runs its stages concurrently and writes the serial export's bytes
An export to a file SHALL run its stages concurrently inside the one output file: decode a
memory budget ahead of the composite with a thread per source, the composite on its own thread
(GPU, with frames in flight between submit and readback) or on frame-parallel workers (CPU, each
with its own scratch), the RGBA→encoder conversion on frame workers, the encoder and the muxer on
one writer, and the audio on its own thread. The stages SHALL be joined by bounded queues that
keep frame order, and the encoder SHALL receive exactly the frames and samples, in exactly the
order, of the serial export (`ExportOptions.serial`, the former one-thread export kept as the
reference); the writer SHALL interleave the audio by a fixed rule (after video frame `i`, the
audio up to that frame's end), so the call order into the muxer and the progress events do not
depend on timing. A frame being composed SHALL never be evicted before its composite (decode
waits while the unwritten chunks' frames fill the look-ahead budget). A cancel or an error in
any stage SHALL stop every stage promptly and return the cause. The video encoder SHALL be opened
with its own threads (`threads=auto`; the serial reference `1`), named in the open report. On a
software rasterizer (llvmpipe, lavapipe) the export SHALL compose on the CPU (`KT_VIDEO_GPU=1`
takes the adapter anyway). `kt-cli export --time` SHALL print the busy time of every stage and
the wall time; `--serial` runs the reference.
Why: the owner (2026-11-03, kkt-26.11.03 C1): "our final render is VERY slow, slower even than the
preview; it seems not parallel but strongly sequential". It was: one thread did decode →
composite → readback (a full wait per frame) → conversion → encode, libx264 encoded on one
thread, and the audio pass started after the last video frame. The showcase export took 324 s (llvmpipe)
and 861 s (CPU) for 104 s of 1080p25, and now takes 96 s.

#### Scenario: The pipelined export writes the serial streams
- **WHEN** the demo project (layers, transitions, FX, automation, audio FX) is rendered over a
  3 s off-grid range at 1280×720 — chunks of a few frames — serially and pipelined, and pipelined
  again
- **THEN** every video frame and every audio sample of the pipelined stream equal the serial
  passes', the frame count, missing layers and loudness are equal, and the two pipelined runs
  make the same calls into the sink in the same order, audio from the first frames on
- **TEST** t1451 (`src/libs/kt-engine/tests/export_pipeline.rs`)

#### Scenario: A cancel or an error mid-pipeline stops every stage
- **WHEN** a pipelined export is cancelled after its tenth written frame, and another one's sink
  fails writing its sixth frame
- **THEN** the first returns "cancelled" with no frame written after the cancel, the second
  returns the sink's own error (not a stage's "the writer stopped") with five frames written, and
  both calls return — every stage's thread joined
- **TEST** t1452 (`src/libs/kt-engine/tests/export_pipeline.rs`)

#### Scenario: The encoder gets its threads
- **WHEN** a two-second project is exported with libx264, pipelined and serial
- **THEN** the open report's options carry `threads=auto` and `threads=1` respectively, and both
  files hold all 60 frames and two seconds of audio
- **TEST** t1456 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: The GPU readback ring gives the executed bytes
- **WHEN** the demo project is exported on the GPU serially (`execute_into` per frame) and
  pipelined on a fresh engine (submit ahead, collect in order), and six plans go through the ring
  three in flight
- **THEN** the pipelined frames equal the serial GPU frames, and every collected frame equals the
  `execute_into` of its plan, byte for byte (one device)
- **TEST** t1457 (`src/libs/kt-gpu/tests/export_ring.rs`)
