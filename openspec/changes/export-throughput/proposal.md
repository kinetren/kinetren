# Proposal

## Why

The owner (2026-11-03, registry `docs/ktprompts/kkt-26.11.03-index.md` row C1): "our final render
is VERY slow, slower even than the preview; it seems not parallel but strongly sequential".
Reading the code confirms it. `export_mp4_with_backends` runs every stage on ONE thread, frame
after frame: a blocking decode of each source in turn (`read_video_span`), the composite (GPU
`execute_into` with a full CPU↔GPU wait per frame, or the single-threaded CPU executor), the
RGBA→YUV conversion, the encoder (opened without a thread count, so libx264 encodes on one core),
and only after the LAST video frame the whole audio pass. On a 32-core machine an export uses
about one core.

Measuring it found something worse first: the export of the showcase project (real footage,
`kt-cli showcase`) crashes within three seconds with `malloc(): corrupted top size`, on the GPU and
the CPU path alike. The decode's `sws_scale` YUV→RGBA of a 1080×1080 source writes 32 bytes past
the end of the exact-size frame buffer (libswscale's x86 kernels convert in pixel blocks), and the
next heap chunk's header goes with them. Any source whose RGBA width is not a multiple of the
block can corrupt the heap — in the preview too.

## What Changes

- **Two decode defects the measurement found** (they cost more than all the threading): a
  forward jump past the decoder's deferred frame decoded every picture in between (a two-minute
  jump in 4K was 3,733 pictures), and one file read at two places at once (a picture-in-picture
  of the same footage) made one decoder seek back and decode a GOP for every frame. Every key of
  a span now decides between decoding on and seeking; a span at several decode qualities decodes
  each picture once; the export's own source of such a file gets a second decoder.
- **The decode's RGBA frame carries a spare tail** (`kt_ffmpeg::RGBA_TAIL_PAD`) for
  `sws_scale` to overrun, cut off after the conversion. A sweep test holds the overrun under it.
- **A stage profile.** `ExportOptions.profile` collects the busy time of every stage (decode,
  compose, readback, convert, encode, audio) and the wall time; `kt-cli export --time` prints it.
  The clock is read only into this record and never steers the render (invariant 11).
  `ExportOptions.serial` keeps the former one-thread export as the reference (and the "before"
  of every measurement); `kt-cli export --serial` runs it.
- **A stage pipeline inside ONE output file.** Decode runs a chunk ahead, a thread per source;
  the composite runs on its own thread (GPU) or on frame-parallel workers (CPU, each with its own
  scratch); the RGBA→YUV conversion on frame-parallel converters; the encoder and the muxer on
  a writer thread; the audio on its own thread, interleaved by the writer in a fixed order. The
  stages are joined by bounded, order-preserving queues, so the frames reaching the encoder and
  their order are exactly those of the serial export. Cancellation and errors stop every stage;
  pause and progress keep working. A project with a stream stage (datamosh) keeps the serial
  composite — its pre-roll decodes on the compose thread.
- **The encoder gets its threads** (`threads=auto`, reported in the open report's options).
- **GPU readback is pipelined**: a ring of staging buffers, so the copy of frame N overlaps the
  work on N+1; the export engine does not fill the readback memo; the export's headless adapter
  is the high-performance one (the UI keeps LowPower, invariant 12).
- **Design records**: the per-stage numbers before/after, the revisited rejection of "explicit
  SIMD in export" (the byte-exact AVX2 twins exist since R.32), and why the CPU fallback is split
  by frames, not by rows.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `export`: a requirement "An export runs its stages concurrently and writes the bytes the serial
  export writes" (determinism serial == pipelined, cancellation mid-pipeline, encoder threads,
  the stage profile).
- `media-index`: a requirement "The decoder reads what it is asked, once, without corrupting
  memory" (the RGBA tail pad; a far forward read seeks; a span at several qualities decodes once;
  a file read at two places gets two decoders in the export).

## Impact

- `src/libs/kt-engine/src/export.rs` (+ `export_pipeline.rs`), `video.rs` (a composite with
  external scratch), `video_plan.rs` (`VideoGpuExec::submit`/`collect` with defaults).
- `src/libs/kt-ffmpeg/src/{lib.rs,source_impl.rs,muxer_impl.rs}` (tail pad, encoder threads,
  a converter separate from the muxer).
- `src/libs/kt-gpu/src/{engine.rs,context.rs}` (staging ring, export adapter).
- `src/apps/kt-cli/src/main.rs` (`export --time`, `--serial`), `src/apps/kt-app` (the export
  engine's adapter).
- Tests t1450–t1457; `docs/guide/whats-new.md`; `openspec/specs/export/design.md`.
