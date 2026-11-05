# Proposal

## Why

The pipelined export (`export-throughput`) writes random broken frames. `kt-cli demo` exported with
`libx264` reports "missing layers" 5, 7, 8 or 10, a different number and a different set of frames on
every run, and those frames lack a layer, down to 5.8 dB between two runs of the same export.
`--serial` gives 0 every time. It breaks invariant 11, and a person gets a damaged file with one
easily missed number at the end of the render (`docs/found-defects.md`, row 66).

The export spec already promises the opposite ("a frame being composed SHALL never be evicted before
its composite"), and t1451 passes, because it renders 90 frames at 720p and never fills the cache.

The window also words a failed render badly (row 67): "Error: io: …", and the Video row keeps saying
"choosing the encoder…" after the choice has failed.

## What Changes

- **The chunks not yet written HOLD their frames in the cache.** A held entry is never evicted; a
  chunk's holds end when its last frame is written. Decode's look-ahead is bounded by the bytes held,
  not by an estimate. The eviction order (LRU) stops being what keeps an export correct.
- **A frame a span dropped is asked again alone before it is composed**, as the serial export's
  pre-roll does: one span that failed half way cost the pipeline 46 frames.
- **A frame composed without a layer is reported where it happens**: one log line per run of such
  frames, naming the time and the media, in both the serial and the pipelined export, in frame order.
  The window opens its log for it.
- **The test that holds this** runs under pressure (a slow writer, full 1080p): t1451's range cannot
  fill the cache.
- `kt-cli export --encoder-threads N`, so the pipelined and the serial streams can be compared at
  the same thread count: `threads=auto` against `1` gives a different x264 stream on purpose.
- The window: the export's own error text without the `io:` prefix, and a Video row that says the
  encoder did not open.

## Capabilities

### Modified Capabilities

- `export`: a frame an unwritten chunk needs is never evicted; a frame without a layer is reported;
  a failed render's window.
- `media-index`: the cache gains holds, stronger than the budget, like a pin, and counted per key.

## Impact

- Code: `src/libs/kt-cache/src/lib.rs` (holds), `src/libs/kt-engine/src/export.rs`,
  `src/libs/kt-engine/src/export_pipeline.rs`, `src/apps/kt-cli/src/main.rs`,
  `src/apps/kt-app/src/render_dialog.rs`, the five catalogs.
- Tests: `src/libs/kt-cache/src/lib.rs`, `src/libs/kt-engine/tests/export_pipeline.rs`,
  `src/apps/kt-cli/tests/cli.rs`, `src/apps/kt-app/src/layout_tests.rs`.
- No memory growth: what is held already had to fit under the look-ahead budget. No format change.
  Invariant 11 is restored, not changed. t1451 stays and is not weakened.
