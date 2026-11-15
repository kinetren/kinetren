# Proposal: stream-round-trips

## Why

stream-front-on-device kept the front of a stream stage on the compute device; the rest of the stream
path still crossed the bus. found-defects 620, 621, 622, 135, 401, 402, 404, 405, 490, 145 and 173:
a Datamosh walk read its own state back at every batch end and uploaded it again for the next
(compute → host → compute), and at 1080 lines the memo filled with pictures evicted the clean
encoder's state, so every frame encoded its GOP again (795 encodes for 196 positions measured on the
showcase, 104.6 MB uploaded a frame); a refresh I-frame was encoded again on every re-walk; a region of
interest at a held position took a second job; the shown frame went through a per-pixel conversion and a
bilinear copy on the processor; an export with a stream stage ran serially.

## What Changes

- The compute door keeps buffers on the device: `ComputeHost::{keeps, run_keeping}` return the named
  buffers as `Held`; kt-gpu's host implements it and gives a dropped one back as a spare.
- The stream memo gets a LIVE tier (`StreamMemo::{put_live, get_live, holds}`): device-held entries a walk
  continues from, bounded on their own (`LIVE_BYTES`), never evicted by the pictures' LRU.
- Datamosh's walk keeps the decoder's picture and the clean encoder's state on the device between its
  batches and leaves them in the live tier at its end; it reads back only what the host shows and the
  checkpoints; a refresh I-frame is kept in the live tier (402); a region of interest at a held position
  encodes its clean picture in the walk's batch (405).
- The shown frame's conversion runs row by row, and a stage whose picture is already the decode size
  skips `fit_into` (a copy there, to the byte).
- The pipelined export takes a project with a stream stage: the history advances in frame order on the
  compose thread before each frame is handed out, the range's pre-roll on the decode thread (490).
- Rows 404 (fixed by gpu-throughput D6, t2437), 622 (verified) and 145/173 (measured) get verdicts.

## Capabilities

### Modified Capabilities
- `video-fx`: a stream stage's walk keeps its state on the compute device; the shown frame's conversion.
- `plugins`: the compute door keeps buffers; the stream memo's live tier.
- `export`: a stream stage no longer keeps the export serial.

## Impact

kt-plugin-api (`compute`, `stream`), kt-gpu (`compute.rs`, `held.rs`), fx-video-codec (`codec.rs`,
`walk.rs`, `mosh.rs`), kt-fx (`fit_into`), kt-engine (`video_stream.rs`, `export.rs`,
`export_pipeline.rs`), kt-cli (`showcase --mosh-lines`, `stream-cost`'s counting host). No format change;
OTIO unaffected.
