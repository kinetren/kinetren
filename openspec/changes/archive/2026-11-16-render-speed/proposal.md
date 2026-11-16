# Proposal: render-speed

## Why

kkt-26.11.15 N3 (line 6), the owner's words: "п.3. финальный рендер у нас очень медленный во всех версиях,
медленее чем превью...". None of his traces (`fails/26.11.14/2`, `fails/26.11.16`) holds an export, and the
export wrote nothing of its own into the trace or the journal: where a render's time goes was known only to
`kt-cli export --time` on our machine. Measured here (release, the showcase, `/dev/shm`), the export is not
slower than the preview at the same size; his preview composes at a third of the frame per axis (640×360 in
his `rendereg_2` load report), so a 1080p render draws nine times its pixels through the same passes.
Whether his render loses anything beyond that cannot be told from what he sends.

## What Changes

- Every export, serial or pipelined, keeps its stage profile (it was kept only when `--time` asked) and
  writes it when it ends: an `ExportStage` record per stage into the trace, and one line into the
  telemetry journal through the render window (`ExportStats::stages`), so his load report and his trace
  folder say where his render's time went.
- `kt-cli trace DIR export`: each export of a run — the real-time factor, each busy stage's share of the
  wall, and the admission waits and pieces of its submissions on a ring a display shares.
- The single-device export reports its upload, encode, device wait and copy-out (they were zero: only the
  per-worker devices filled them).
- `kt-cli export --range FROM TO --size WxH` (the render window's region and resolution), `kt-cli
  gpu-share --mosh none`: the tools of the comparison.

## Capabilities

### Modified Capabilities
- `export`: an export writes where its time went.
- `telemetry`: `trace export`.

## Impact

kt-time (`trace.rs`: `Ev::ExportStage`, `export_stage`), kt-engine (`export.rs`, `export_pipeline.rs`),
kt-gpu (`engine.rs`: `submit`/`collect` stages), kt-app (`render_dialog.rs`: the journal line;
`layout_tests.rs`), kt-cli (`main.rs`, `trace_query.rs`, `gpushare.rs`). No format change; OTIO unaffected.
