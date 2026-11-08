# Design

## Context

The performance rounds of 2026-11-03..08 each fixed one complaint: idle-cpu, input-without-work,
ui-frame-cost, cached-range-plays, audio-fx-cost, ui-never-waits, gpu-pass-cost and
decode-request-storm. The owner then asked whether the engine should be thrown away. The full
analysis is `docs/engine-audit.md`. This record keeps the decisions on how the audit was made and
what it rests on.

## Decisions

### D1. Group by mechanism; judge architectural vs local by recurrence

A mechanism is **architectural** when the structure makes it again wherever new code is written: a
new service, producer, panel or derived datum meets it with no one deciding to. It is **local** when
a fix at its site stays fixed.

The class walk covered:
- every row of `docs/found-defects.md`;
- 25 archived changes;
- six capability design records;
- the owner's claims (C, E, G and H registries).

The rows are mapped in the appendix of `docs/engine-audit.md`.

### D2. Numbers only from archives, tests or our own runs

Every number in the audit cites its change, row or measurement. Estimates are marked **guess**.

The audit's own runs:
- release `kt-cli probe … --time`, on the relinked haski copy and on `showcase --heavy`;
- llvmpipe, at machine load 29–56;
- under `systemd-run … MemoryMax=12G MemorySwapMax=0`.

Speed on the owner's card is unknown (found-defects row 108). The audit says so wherever it matters.

### D3. Sample the rebuild before recommending the incremental engine

`perf` is not permitted on this machine (`perf_event_paranoid` = 4), so gdb interrupted
`AudioRenderer::new` at three moments. All three stacks stood in `warp::sign_knots`. That changed the
order of the plan: a linear `sign_knots` (S) comes before `edit-without-rebuild` (M). The whole
rebuild of a ~2000-event project without dense speed curves costs 1 ms of audio (the showcase).

### D4. The scheduler is designed, not built

The design (§2b) keeps the cpal callback, the audio render thread's ring fill and decoder ownership
outside scheduling (invariants 5 and 6). It decides WHEN and WHERE, never WHAT, so invariant 11
holds. It is staged so each stage ships alone: job classes and counters with the background
executor; then GPU arbitration (after the owner's `GPU` journal); then the memory ledger.

### D5. JIT verdict within the owner's acceleration scope

Scope (UF:2547-2577): all math on plain WGSL through wgpu/Vulkan; the byte reference dropped; ARM
needed; no invented language; audio stays on the CPU. Within it, each JIT kind was judged against our
measured costs:
- pipeline prewarm and cache: yes;
- fused point-wise runs as generated WGSL: later, measured;
- discrete specialization: by hand or by `override`;
- audio and CPU JIT: rejected.

## The class, walked

| Subsystem | Mechanisms found there |
|---|---|
| model → engine handoff (`push_project_to_player`, `Player::set_project`) | M1 |
| audio engine build (`build_reel`, warp maps) | M1, M2 (row 151) |
| video engine build (`VideoRenderer::new_with_facts`) | M1 (row 150) |
| audio render thread, ring, mix store | M3 (closed), M9 (closed) |
| prefetch (audio, video, stream stages) | M2 (row 144), M4, M6 |
| preview compose queue, offline device | M3 (closed), M5 (row 149) |
| compute host (Datamosh) | M3 (row 147), M4 (row 134), M5 |
| MediaWorker, decoders, RGBA conversion | M4 (closed), M5 (row 94) |
| thumbnails, fx-thumbs, posters, reel previews | M4 (row 114), M5, M6 |
| index service, analysis, watchdog, load watch | M6 (row 122) |
| sidecar | M3 (closed), M5 (closed) |
| UI frame, timeline, windows | M2 (row 107), M3 (rows 98, 148), M6 (row 140) |
| audio FX kernels | M7 |
| video FX passes, GPU pipelines | M7, M8 (rows 64, 138, 139) |
| export pipeline | M4, M5, M8, M9 (closed); M9 budget (rows 10, 85, 96) |
| caches as a whole | M9 budget |
| counters and journal | M10 |
| tests and live scenarios | M11 |

## Risks

- The verdict rests partly on llvmpipe numbers. The plan orders the card-dependent steps (the GPU
  arbiter, fusion) after the owner's journal.
- The row 151 estimate of the post-fix build cost is a guess until the fix is measured.
