# Tasks: export-into-surface

> IMPLEMENTATION-READY (spec-engine, 2026-11-24, re-read at orch `229c694b`: render-result-check is in). Tests t4125–t4136. Areas: EXPORT, GPU (`engine.rs` submit/collect,
> `yuv.wgsl`, new `surface.rs`), kt-ffmpeg `muxer_impl.rs`/`api.rs`/`hw_trial.rs`, `fixtures/fake_va`; CACHE in section 5
> (`render_cache.rs` writer). Shares `mem.rs`'s `Tenant::Surfaces` and `ktfake_drv_video.c` with `device-decode-sources`: after its section 1,
> or before it, not beside. Sections 1–3 may start now; 4–6 not beside `loudness-norms` (running: EXPORT, DIALOG) or `cached-final-render`, and 1, 3, 4 not beside `track-stream-stage` 2 (`engine.rs`). Commit per section;
> target-prune after each.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D14, class walk D13), deltas `export`, `video-plan`, `telemetry`
- [x] 0.2 `export-host-copy` closed and archived: 0.2 is his A13; its 3.2, the idle check while the prerender
      writes and after, is 5.1's here
- [ ] 0.3 the F-EXP note of hardware encoding (`docs/features/export/pipeline/hardware-encoder-selection.md`) gains what others do with
      the encoder's surface (Resolve, OBS's dma-buf path, FFmpeg's hwmap) and this design's verdict

## 1. The pass (kt-gpu)

- [ ] 1.1 `fs_y`, `fs_uv` in `yuv.wgsl`; `Pipe::YuvY`, `Pipe::YuvUv` with `R8Uint`/`Rg8Uint` targets, compiled on ask; t4125
- [ ] 1.2 t3313 green as it is; clippy; commit

## 2. The pool (kt-ffmpeg, the fake driver)

- [ ] 2.1 `hwcontext_drm.h` in build.rs; the optional table of `av_hwframe_map` (t961's rule for it); `SurfacePool`, `Surface`, `DmaBuf`; the frames
      context's size; `Muxer::write_surface`
- [ ] 2.2 the fake driver: surfaces in memfds, `vaExportSurfaceHandle` in its vtable place (libva 2.x `va_backend.h`), DRM_PRIME_2 separate layers;
      modes `export-refuse`, `export-composed` (one layer, two planes), `export-stale` (another memfd than the surface's),
      `export-abort`; `KT_FAKE_VA_LOG` "mapped N"; t1551–t1556 and t3129 green as they are
- [ ] 2.3 the descriptor's reading and refusals; t4126; `cargo test -p kt-ffmpeg`; commit

## 3. Import, proof, trial (kt-gpu, kt-ffmpeg)

- [ ] 3.1 `kt-gpu/src/surface.rs`: conditions 2, 4, 6; `import` and its drop order; the two barrier buffers; `wanted_features`; the test double that copies
      a finished texture into the exported memfd (`testkit`)
- [ ] 3.2 the proof (D6); t4129; t4135 (opt-in `KT_EXPECT_DMABUF=1`; the CI job tries `modprobe udmabuf`, and the report says what came of it)
- [ ] 3.3 `hw_trial::serve_with`, the `surface` trial, both mains; t4130
- [ ] 3.4 kt-gpu `CLAUDE.md`: hal is touched in `holds.rs` and `surface.rs` only; commit

## 4. The export

- [ ] 4.1 `VideoGpuExec::{import_surface, submit_surface, surface_depth}`, `ExportSink::{surfaces, write_surface}`, `Job::Surface`, the serial export; the waits of D4
- [ ] 4.2 `KT_EXPORT_SURFACE`; D7's fallbacks, the empty pool of D4 (the fake driver keeps every surface until the next frame: mode `keep`); t4127, t4128, t4131, t4132; t1451, t1556, t3998,
      t963 green, t963 prints the way; every one of them compares written with announced
- [ ] 4.3 `cargo test -p kt-engine -p kt-gpu -p kt-media -p kt-cli`; clippy; commit

## 5. The prerender

- [ ] 5.1 `SegmentWriter::surfaces`, `render_cache`'s submit, the process's verdict per (device, node); t4133; the idle check on the heavy showcase
      while it writes and after

## 6. Said

- [ ] 6.1 `ExportStage` kind, `trace … export`; `ExportLog::Surface`, i18n ×5; t4134; t4136 (live `hwencode`, the log's line read)
- [ ] 6.2 C11's text to the orchestrator (D12); What's new ("a render on an Intel or AMD card hands its frames to the encoder
      without copying them through memory"); found-defects 741 closed; registry A13; the report's "Найдено, не исправлено"
