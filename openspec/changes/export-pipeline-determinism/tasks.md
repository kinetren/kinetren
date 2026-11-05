# Tasks

Test numbers t1600–t1619 are reserved by this change. Every stage ends with a commit (English
message, the first line = the stage and what was done, no attribution lines).

## 1. The measured "before"

- [x] 1.1 Release `kt-cli` from the base commit; the showcase pipelined twice: wall, peak RSS, missing layers, md5
- [x] 1.2 Record the cause in `design.md` (done from the instrumented run, 2026-11-04)

## 2. Holds and the pipeline (D1–D3)

- [x] 2.1 `kt-cache`: holds; t1600
- [x] 2.2 t1601 written first, and red on the old code
  - Red 8 of 8 on the old pipeline, losing 4–8 layers, a different number run to run. A scaled budget does not
    scale simply: 1/16 at 270p, and 1/4–1/16 at 1080p, lose nothing on the old code; 640×360 at 1/4 does.
- [x] 2.3 `export_pipeline.rs`: the chunks hold, the writer's progress releases, the look-ahead is the held bytes. t1601, t1451–t1458 green
  - Green 8 of 8: 24 pipelined runs equal to the serial export, frame by frame, zero lost.
- [x] 2.4 `--encoder-threads`; t1603. The demo's streams equal `--serial`
- [x] 2.5 Commit the stage

## 3. A frame without a layer (D4)

- [x] 3.1 `IncompleteFrames` in both exports, the log, kt-cli; t1602
- [x] 3.2 A frame a span dropped is asked again (D7); t1605, 46 frames lost with the retry switched off
- [x] 3.3 Commit the stage

## 4. The window (D6)

- [x] 4.1 `ExportError::Io` without "io:", the Video row, the catalogs, the log opening; t1604 with its PATH. `reach_tests.rs`, `layout_tests.rs`, `font_tests.rs`, `i18n_tests.rs` green
- [x] 4.2 Commit the stage

## 5. Finish

- [ ] 5.1 The "after": the same showcase measurement on release, the pipelined against `--serial`; record both in `openspec/specs/export/design.md`
- [ ] 5.2 Tests of the specs, `cargo test` of what was touched, clippy, live `render`
- [ ] 5.3 `docs/found-defects.md` rows 66 and 67, What's new, `kt-cache/CLAUDE.md`; archive; `target-prune.py`
