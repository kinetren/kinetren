# Tasks

Test numbers t1450–t1458 are reserved by this change (t1415–t1419 and t1430–t1434 are taken by
sibling changes). Every stage ends with a commit (English message, first line = the stage and
what was done, no `Co-Authored-By` or other agent lines) and `scripts/target-prune.py`.

## 0. Measure first

- [x] 0.1 Release `kt-cli`, the showcase project (`kt-cli showcase --sources …/showcase-media`),
  `kt-cli export` timed on the GPU (llvmpipe) and the CPU path. Recorded: master crashes in 3 s
  with `malloc(): corrupted top size` on both paths (D1)
- [x] 0.2 Find the crash: a canary after every decoded RGBA frame. Recorded: 32 bytes past a
  1080×1080 frame (FFmpeg 6.1 yuv2rgb)
- [x] 0.3 The stage profile (`ExportOptions.profile`, `kt-cli export --time`, `--serial`, D6) and
  kt-ffmpeg's decode counters

## 1. The decode defects

- [x] 1.1 `RGBA_TAIL_PAD` and the conversion into it; t1450 sweeps widths (D1)
- [x] 1.2 A far forward read seeks (`prepare_for` bound, every key of a span decided); t1453 red
  on the old bound (300 pictures) (D8)
- [x] 1.3 A span at several qualities decodes once (`read_video_span_qualities`,
  `read_video_span_sizes`, the per-size `SwsContext`s); both exports decode through
  `chunk_requests` + `decode_requests`; t1454 (D7)
- [x] 1.4 Two decoders for a file read at two places (`FfmpegSource` cursors); t1455 red with one
  decoder (60 reopens) (D9)

## 2. The pipeline

- [x] 2.1 `MuxConfig.threads`, `threads=auto` in the open report; t1456 (D3)
- [x] 2.2 `VideoConverter` / `VideoFrame` / `write_video_frame` in kt-ffmpeg (D4)
- [x] 2.3 `compose_export_with` + `ExportScratch`, `has_stream_stages` (D2)
- [x] 2.4 `export_pipeline::run`: decode ahead by bytes, frame workers, writer with the fixed
  interleave, audio thread, halt; `export_mp4_with_backends` runs it unless `serial` or a stream
  stage; `render_pipelined` for tests; t1451, t1452; t955 green (the writer's counts) (D2)
- [x] 2.5 GPU: `readback_depth`/`submit`/`collect`, the staging ring, no readback memo on that
  path, upload texture reuse, `new_headless_for_export`, `export_engine` (a software rasterizer
  composes on the CPU); t1457 (D5)

- [x] 2.6 Frame-sized buffers back to the system (`frame_alloc`, the binaries' first statement);
  t1458; peak RSS 16.2 → 7.3 GB (D11)

## 3. Records and finish

- [x] 3.1 Before/after per stage on release in `openspec/specs/export/design.md`; the revisited
  SIMD rejection (D10); the media-index design for D1/D7–D9; crate CLAUDE.md lines
- [x] 3.2 `docs/guide/whats-new.md` (## kt26.11); registry `kkt-26.11.03-index.md` row C1
- [x] 3.3 `cargo test --workspace`, `cargo clippy --all-targets` (no new warnings in the changed
  lines); commit; `scripts/target-prune.py`. Recorded: 1468 passed, 1 red — t640 (load-sensitive,
  green alone, 11.7 s); clippy: one `is_multiple_of` in export.rs fixed, the two kt-cli warnings
  (main.rs:711, :1168) are older than this change
- [x] 3.4 No `finish.sh` (the orchestrator runs it per stage, owner 2026-11-03): the live scenarios
  `render` and `renderregion`. Recorded: the pipelined render finished the scenario's 40 s project
  before it reached Pause; the project is 200 s now (`kt-demo-200s.mp4`, 11 % at the Log step),
  `render` green alone on the first try, `renderregion` green; archive the change
