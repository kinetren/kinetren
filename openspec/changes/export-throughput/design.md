# Design

## Context

- `export_mp4_with_backends` (`src/libs/kt-engine/src/export.rs`) ran one thread: per chunk of
  ≤ 120 frames a blocking `read_video_span` of each source in turn, then per frame the composite
  (GPU `execute_into` with `map_async` + `poll(wait)` — the CPU waits for every frame — or the
  single-threaded CPU executor), `Muxer::write_video` (sws RGBA→YUV420P, then the encoder) and
  only after the LAST video frame the audio pass. The encoder context got no `threads`, so
  libx264 encoded on one core (the decoders do get `threads=auto`, `source_impl.rs`).
- The measuring project is `kt-cli showcase` over `/media/e/developer/kt.9/sources/showcase-media`:
  104 s of 1080p25 (2600 frames), 101 events on three lanes (a sharp layer, its blurred backdrop,
  titles and a picture-in-picture), 13 transitions, seven VP8/VP9 files up to 3840×2160.
  The machine: 32 cores, no GPU (the wgpu adapter is llvmpipe), the disk saturated by other
  sessions (the runs read from the page cache and write to `/dev/shm`).

## Decisions

### D1. The decode's RGBA frame carries a spare tail
The first measurement crashed: the showcase export died within three seconds with
`malloc(): corrupted top size`, on the GPU and the CPU path alike. A canary after the frame
found it: `sws_scale` of a 1080×1080 YUV420P picture to RGBA at its own size writes 32 bytes past
the end of the exact-size `Vec` (FFmpeg 6.1's x86 yuv2rgb converts in pixel blocks; the guard on
the stride holds for every row but the last). Every decoded frame is now converted into
`kt_ffmpeg::RGBA_TAIL_PAD` (256) spare bytes, cut off after the conversion. An earlier row's
overrun lands on the next row's start, which the next row overwrites. t1450 sweeps every even
width to 130 (worst: 56 bytes at width 2) and the owner's 1080. **Rejected:** a 16-pixel-aligned
destination stride and a copy — a second pass over every frame for what 256 bytes solve.

### D2. A stage pipeline inside ONE output file
`export_pipeline::run`:
- **decode** — one thread walks the serial loop's chunks; within a chunk each source decodes on
  its own scoped thread (the source is lent, never shared — invariant 5);
- **compose** — the GPU on one thread (`submit` ahead, `collect` in order, D5), or a dispatcher
  handing frames round robin to CPU frame workers, each with its own `ExportScratch` over the
  SHARED `VideoRenderer` (`compose_export_with`, the same `build_plan` + `execute_plan_cpu`);
- **convert** — the frame workers (`VideoConverter`, one `SwsContext` each, D4);
- **writer** — the calling thread: frame `i` from worker `i % W`, the encoder, then the audio up
  to the end of frame `i` (D3 of the interleave below);
- **audio** — `render_raw_audio_range` on its own thread into a bounded queue.

Every frame is the function of (cache, t, quality) the serial export computes; the writer takes
them in frame order; so the encoder receives the serial export's frames (t1451). Kept rejected:
chunks rendered in parallel and joined (design of `export`, F-COL-040) — this is one file, one
encoder, one frame order.

- **The cache never evicts a frame being composed.** The serial export's chunk is half its
  budget (R.15 item 1). The pipeline's cache is five budgets; decode starts a chunk only while the
  frames it decoded for the unwritten chunks stay under four (`LOOKAHEAD_BYTES`): the next chunk
  (≤ half a budget) and frames shared with older chunks fit in the rest, and the LRU drops the
  written chunks first. The first version gated on "two chunks ahead": at 4K decode sizes a chunk
  is ONE frame, and two of sixteen CPU workers were busy (337 s). The byte budget gave 101 s.
- **A stream stage keeps the serial export.** Its pre-roll (`stream_preroll`, datamosh D6)
  decodes on the compose thread from the export's own sources; `has_stream_stages` routes such a
  project to the serial path.
- **The interleave is a fixed rule**, not "whatever arrived": after video frame `i` the writer
  writes the audio up to that frame's end. The muxer's calls — and with them the file's packet
  layout — are a function of the project, and the audio no longer waits for the last video frame.
  The progress the writer reports (frames and audio frames WRITTEN) carries the same numbers on
  every run: t955 compares the event streams of two exports element by element, and the first
  version, which reported the audio thread's own count, broke it.
- **Cancel and errors.** One `halt` flag every stage checks per frame or burst; whoever sees the
  person's cancel sets it. A stage that stops drops its queue ends, and every neighbour blocked
  on them returns; the decoder's wait on the writer times out every 50 ms to look at the flag. The
  first error wins over the "the writer stopped" it causes elsewhere (t1452).
- **Pause** is the same 40 ms cooperative wait, per dispatched frame and per audio burst.

### D3. The encoder's threads
`MuxConfig.threads`: 0 → `threads=auto` (pipelined), 1 → one thread (the serial reference). It
goes into the open report's options, so the render window and `kt-cli export` show it (t1456).
x264's output depends on its thread count, so two machines write different bytes — they already
did (the encoder is a property of the machine, the golden hashes guard the raw streams); one
machine writes the same bytes run after run.

### D4. Conversion on the frame workers
`Muxer::write_video` = `VideoConverter::convert` + `Muxer::write_video_frame`; the pipeline makes
one converter per worker (`Muxer::video_converter`) and recycles two `VideoFrame`s per worker
through the writer. `av_frame_make_writable` before each conversion: an encoder may still hold a
reference to the buffer it was last sent. A conversion is a pure function of the RGBA bytes, so
which converter did a frame does not show. **Rejected:** libswscale's own slice threads
(`threads` + `sws_scale_frame`) — the frame workers already convert in parallel, and its output
identity across thread counts is not promised.

### D5. GPU: a readback ring, texture reuse, the export's adapter
- `VideoGpuExec::{readback_depth, submit, collect}` (defaults: depth 0, the caller uses
  `execute_into`). kt-gpu keeps three staging buffers (`RING`): `submit` encodes the passes and the
  copy, submits and requests the map; `collect` waits for THAT submission
  (`PollType::Wait{submission_index}`) and copies the rows out. The uniform arena and the pool's
  accumulators are reused across frames in flight: `queue.write_buffer` and a later pass writing
  a texture are ordered after the earlier submission on the one queue. Before waiting for the
  next chunk the compose thread collects what is in flight — the decoder of that chunk may be
  waiting for the writer to pass those frames.
- The export path neither asks nor fills the readback memo: each moment is composed once, and the
  memo's copy of every 1080p frame (8 MB) was pure cost.
- `UploadCache` keeps up to four evicted textures per size for the next upload: an export uploads
  every source frame once, and a texture per frame was a device allocation per frame.
- `GpuContext::new_headless_for_export`: `HighPerformance` unless `WGPU_POWER_PREF` says
  otherwise. LowPower stays for the UI and its preview device (invariant 12, PRIME item 39): the
  UI heats the machine every frame a person looks at, an export is a finite job the person waits
  for, on its own device.
- **A software rasterizer composes on the CPU.** `kt_gpu::export_engine` refuses a
  `DeviceType::Cpu` adapter (llvmpipe, lavapipe) unless `KT_VIDEO_GPU=1`: it rasterizes one frame
  at a time through one queue, the CPU workers compose a frame each. On the showcase: 131.9 s on
  llvmpipe, 101.4 s on the CPU workers. The render window's backend row says why.

### D6. The stage profile
`ExportOptions.profile: Option<Arc<ExportProfile>>` — busy nanoseconds per stage (decode,
compose, readback, convert, encode, audio, audio encode) summed over the stage's threads, and the
wall time; `kt-cli export --time` prints it with kt-ffmpeg's decode counters (pictures decoded,
backward reopens, RGBA conversions). The clock is read only into this record the caller asked
for and never steers the render (invariant 11). `--serial` runs the former export, so "before"
and "after" are measured by one binary.

### D7. A span at several decode qualities decodes once
`MediaSource::read_video_span_qualities` (default: a span per quality) and
`kt_ffmpeg::Source::read_video_span_sizes`: each key's picture is chosen once and converted to
every size asked at it (a small cache of `SwsContext`s per size). Both exports decode through
`chunk_requests` + `decode_requests`, so the serial reference and the pipeline ask the same.

### D8. A far forward read seeks
`prepare_for` returned early when "the answer is already held" — the held frame before the target
and a deferred frame after it — without checking that the target was before the deferred frame.
Past it, the jump was decoded linearly: a two-minute forward jump in 4K was 3,733 pictures for one
event, and the preview did the same after a click far ahead. Now the early exit requires
`target < pending_pts`, and every key of a span is decided (two events of one file can stand
minutes apart in one span). t1453 (300 → ≤ 45 pictures).

### D9. Two places of one file, two decoders
The showcase reads a file at two places at once (a picture-in-picture of the same footage 7 s
earlier; a reel 47 s apart). One decoder seeked back and decoded a GOP of 4K for every frame:
238 reopens. The export's `FfmpegSource` opens a second decoder of the file (with the packet index
it already has) when a run of keys has no decoder that reaches it by decoding on; runs are keys
more than 2 s apart; the choice is the nearest decoder behind the run within the forward bound,
else a fresh one while there are fewer than two, else the least recently used — a function of the
requests. Decoded pictures: 101,009 → 9,190 for the 5,644 the export needs. Bounded at two: a 4K
decoder holds its threads' frame pools. **Not done here:** the preview's `MediaWorker` has the
same ping-pong (one decoder per media); its queue is a separate design (follow-up).

### D10. Revisited: "explicit SIMD in export"
`openspec/specs/export/design.md` rejected it because auto-vectorised f64 sums would break
determinism. That premise is gone: the AVX2 twins (R.32, kt-fx `video_simd.rs`, the barriers'
`simd.rs`) are byte-exact by rule and run in the export already behind `simd_level()`. What the
export lacked was cores, not lanes. The rejection is replaced by the rule that stands: SIMD in any
kernel only as a byte-exact twin of its scalar text.

## Measurements

Release build, the showcase (104.0 s, 2600 frames 1080p25, libx264 veryfast crf 23), one binary,
`--serial` = the former export. Busy seconds per stage summed over its threads.

(see the table in `openspec/specs/export/design.md`, "The final render is pipelined")

## Risks / Trade-offs

- **Memory.** The pipeline's cache is 960 MB (five serial budgets), the CPU workers' scratch at
  most 1 GiB (≤ 16 workers, eight output frames each), two decoders for a file read at two places.
  The showcase's peak RSS is in the measurements.
- **The file's bytes differ from the serial export's** (x264 with threads, audio interleaved
  earlier); the frames and samples handed to the encoder are the same (t1451), and one machine
  writes the same file every run.
