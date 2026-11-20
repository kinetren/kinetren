# Tasks: export-host-copy

> DESIGN ONLY until the owner answers Q1 (`proposal.md`) and the orchestrator gives the go. Stage 1 (the
> measurement) does not depend on Q1. Tests reserved: t3312–t3321. Base: orch `ec30cb02`.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D8, class walk D7), deltas `export`, `video-plan`, `telemetry`;
      `openspec validate export-host-copy --strict`
- [ ] 0.2 his answer Q1 in `docs/user-feedback.md` and `docs/owner-checks.md` (orchestrator)

## 1. Measure every move

- [x] 1.1 `ExportProfile::upload_ns` around the writer's surface get + transfer; `encode_ns` without it;
      `ExportStage` record (kt-time export_stage name list)
- [x] 1.2 `trace … export` prints copy out, convert, upload per frame; t3316
- [x] 1.3 found-defects 741: the row points here

## 2. The exact conversion on the device

- [ ] 2.1 kt-gpu `yuv.rs`: the Q15 reference for the five layouts, left siting; t3312
- [ ] 2.2 the WGSL pass (integer only) and layout-sized ring slots; `ring_depth` by layout bytes; t3313, t3315
- [ ] 2.3 `VideoGpuExec`: readback with `OutLayout` (submit/collect and `execute_into`); kt-engine
      pipelined and serial exports carry planes; frame workers copy planes into the `AVFrame`; t3314, t3318
- [ ] 2.4 kt-ffmpeg: `VideoConverter` → the reference (planes in, `AVFrame` out); the `chroma_location`
      tag; t3319; t2806 and t2805 still green
- [ ] 2.5 `cargo test` kt-gpu, kt-engine, kt-ffmpeg; clippy; target-prune; commit

## 3. The prerender

- [ ] 3.1 `render_cache` writer asks the layout; `SegmentWriter` takes planes; t3317
- [ ] 3.2 idle-load check on the heavy showcase while the prerender runs and after (owner rule)
- [ ] 3.3 kt-gpu `CLAUDE.md`: "a new encoder input format gets its exact layout in `yuv.rs` and the pass";
      What's new line ("exports on a card spend less processor time")
