# Design

## Context

This revision follows the archived `engine-audit` (2026-11-08) and the owner's two answers H12 and
H13. The per-area facts come from a read of `src/` at 8770e6b2. The measured costs come from
`engine-audit` §0 and the archived performance changes.

## Decisions

### D1. "JIT" is judged as three kinds, by what each needs to stay byte-exact

- **T1, compiled data:** evaluators built once per build.
- **T2, specialized Rust chosen at run time:** by state or by instruction set.
- **T3, run-time machine code:** Cranelift, LLVM, a graph compiler.

T1 and T2 keep audio byte-exact by construction: the same operations in the same order, and the
transcendentals from `kt_plugin_api::math`. Both are proved by the two-path byte test of
audio-fx-cost (t1981). T3 would be a second implementation of byte-exact audio. It is rejected for
now, because no measured CPU cost is bound by dispatch or by interpretation that closures cannot
remove.

### D2. Weigh against what costs on the CPU in the defects, not against today's audio share

The CPU costs measured in the defects are:
- the engine build, ~450 ms an edit (row 151);
- the prefetch pass, 160–280 ms (row 144);
- per-sample transcendentals (rows 128, 129);
- the Datamosh CPU side (rows 135, 144).

Two costs found by reading are unbounded: the LFO phase walks its rate curve from the first point
every sample, and video automation reads frame-grid curves linearly per frame. Both are T1's.

### D3. No design waits for a machine

Priorities are a fixed order. A GPU submission is bounded by the device time the existing pass timer
measures on the running device. Fusion is a general rule, judged by the bytes moved per frame (device
independent) and by time on our llvmpipe bench, with the run-length cap measured at run time. Row 149
is reproduced on llvmpipe by putting the preview and the offline route on one device.

## Risks

- The expected gains of T1 and T2 per area are guesses until each change measures them. They are
  marked as such in the table.
