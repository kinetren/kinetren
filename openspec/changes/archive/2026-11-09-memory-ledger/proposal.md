# Proposal: memory-ledger

## Why

Row 9 of the order table of `docs/engine-audit.md` (mechanism M9, scheduler stage 3), under the
owner's standing go of 2026-11-09. His complaint behind the table (kkt-26.11.09 I2): on a heavy
Datamosh region the preview stays "Loading", and even a hundred laps over a small piece keep doing
work. `stream-layout-per-build` made the stream memo hold such a loop; what remains is that every
other cache picks its own size with a constant and none of them sees the others:

- the present memo of kept composites is a constant 512 MB (found-defects 310): 582 composites at
  640×360, 60 at 1080p. The owner's region `problem_region_26_11_09` is 665 frames at 60 fps, so a lap
  is composed again even when the stream memo holds its whole history — and an LRU over a loop longer
  than the memo keeps nothing the next lap reads;
- the live BurstCache 512 + 64 MB, the GPU pools (upload 256, transient 128, readback 64 MB per
  device), the stream memo (256 MB, or a quarter of what is free), MixStore 64 MB and the compute pool
  1 GiB sum to whatever they sum to; nobody sees the total or takes back a byte where it is worth least;
- an export started from the editor meets the preview's grown memos and sizes its devices on the
  memory they left; the export's own devices each keep a 256 MB upload cache for frames an export
  uploads once (rows 10, 85, 96: 7.3 GB at 1080p, 4K unproven).

## What Changes

- **One memory ledger** (`kt_time::mem`): every cache that holds frames, composites, mixes or device
  buffers opens an account of its tenant kind and reports its bytes. Each tenant keeps its proven
  working set (its base, the old constant) untouched; what any tenant wants ABOVE its base is
  granted from one budget measured at run time — half of what the process could hold beyond its
  bases (`available` now, cgroup limit included, plus the growth it already holds) — in a stated
  order of value: kept composites first, stream history second, decoded frames third. Unknown
  memory grants nothing. Taking back goes the reverse way.
- **The present memo is sized by the loop and the machine** (row 310): the player asks for every
  composite of the loop region at the preview's size; the memo keeps them within its grant. A loop
  that fits the budget becomes all-kept after its first laps and stays so.
- **A loop longer than its memo keeps a part of it** instead of nothing: when the memo is full of
  moments the current lap still needs, a new composite is not admitted over them.
- **The stream memo's bound comes through the ledger** (its want is unchanged from
  stream-layout-per-build; the ledger bounds the sum with the present memo).
- **An export claims the room**: while an export runs, the preview's growth is taken back (the
  stream memo and the present memo drop to their bases at once) and is granted again when it ends;
  holds stay stronger than any cap.
- **An export's devices upload once**: an export engine's upload cache keeps only the textures of
  the frames in flight, never reusing a texture a frame still reads.
- **A device's own memory is learned by its first refusal**: a kept composite whose texture the device
  cannot create is not kept, and the memo then bounds its growth below what it held.
- **Row 119 closed end to end**: a frame evicted from the BurstCache and decoded again finds its kept
  composite (the serial return of structured-telemetry, now held by a player-level test).
- **The ledger is visible**: `mem_budget`/`mem_grant` trace records, `kt-cli trace memory`, and the
  Diagnostics window's memory line (each tenant's bytes against the budget) next to the cache bar.
- The class walk (every cache, memo, pool and disk tier) and its verdicts go into design.md.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `media-index`: ADDED — one memory ledger over the caches: bases, one run-time budget, the order of
  grants and of taking back, an export's claim; holds are stronger than any cap.
- `video-plan`: MODIFIED — the present memo's bound (512 MB constant → base plus the ledger's grant for
  the loop); the stream memo's bound goes through the ledger. ADDED — a loop longer than the memo keeps
  a part; a re-decoded frame finds its kept composite; a device's refusal bounds the memo.
- `export`: ADDED — an export's devices keep no upload past the frames in flight; an export claims
  the preview's growth while it runs.
- `telemetry`: ADDED — the trace names the ledger's budget and every grant.
- `ui-timeline`: ADDED — the Diagnostics window shows the ledger.

## Impact

- `kt-time`: new module `mem` (std only, no dependency), trace kinds `MemBudget`, `MemGrant`.
- `kt-cache`: the BurstCache opens accounts (video, audio).
- `kt-gpu`: PresentMemo, UploadCache, ReadbackCache, TexturePool and the compute pool report to the
  ledger; the present memo's cap and its no-admit rule; the export engine's upload rule; the device
  refusal guard; `memory::available` stays the probe.
- `kt-engine`: the player's present want from the loop region and preview size; `size_stream_memo`
  through the ledger; MixStore account; the export claim (`export.rs`, `export_pipeline.rs`).
- `kt-app`: the probe injected into the ledger at start; the Diagnostics memory line (i18n: five
  catalogs). `kt-cli`: the probe, `trace memory`.
- No change to the `.kt` format, the plugin API, or any byte of a frame or a mix (invariant 11).
