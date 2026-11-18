## Why

A heavy region (VHS Tape, Camera ISP, Datamosh) is composed again on every lap and after every restart:
the render-ahead producer keeps its composites in the display's memory only, and the owner's loop fills
from lap 5–15 (kkt-26.11.16, kkt-26.11.18 T2). The architecture review (R3,
`docs/architecture-review-26.11.17.md`) proposed a prerender like VEGAS's; asked how it should run, the
owner answered "Сам в фоне" (kkt-26.11.17 S5): the program prerenders slow regions to disk by itself in the
background — his one exception to "the program at rest costs nothing".

## What Changes

- **A disk tier of composites** (`kt_engine::render_cache`): complete preview composites kept as lossless
  files (deflate over a left-pixel delta of the RGBA), keyed by a DURABLE plan key — everything the
  executor's plan key hashes, with the frames' per-process content serials replaced by what determines
  them across processes (a file's path, size and modification time; a generator's values; a stream stage's
  own key), and salted by the program build and the device. Bounded in size (2 GiB, `KT_RENDER_CACHE_MB`),
  evicted least-recently-used; the folders of other builds are removed at open.
- **The preview reads it before composing**: the producer (before its "decoded?" check) and the compose
  thread's kept-composite path restore a moment found on disk into the display's memo and show it — no
  decode, no stage, no chain. The coverage bars draw the moments the disk holds for the current state.
- **The program prerenders by itself at rest** (`render-cache` thread of the player): after 1.5 s with no
  input, not playing, no export, it composes the moments whose measured device time (the preview's cost
  map, render-honest D2) is at least half a frame period, and the unmeasured neighbours of such moments,
  nearest the playhead first, and writes the heavy ones. Any input, Play or an export stops it at once;
  its device work runs at class `background`, behind every present.
- **Trace**: `Ev::RenderCache` records restores, stores, evictions and the prerender's measured moments;
  `kt-cli trace cache` prints them. `kt-cli preview --render-cache DIR` runs the bench with the tier and
  waits for the prerender at its end.
- **Determinism**: an export never reads the tier; a restored composite is the bytes the producer's engine
  composed for the same plan on the same device and build.

## Capabilities

### New Capabilities

### Modified Capabilities
- `video-plan`: a disk tier of preview composites under a durable key, read before composing, written by a
  prerender at rest.
- `telemetry`: the tier's work in the trace and `trace cache`.

## Impact

`kt-engine` (new `render_cache.rs`; `video.rs` durable probe; `player.rs`, `render_ahead.rs`), `kt-time`
(`Ev::RenderCache`, `job::PRERENDER`), `kt-app` (the data folder, the input signal), `kt-cli`
(`preview --render-cache`, `trace cache`). No project format change; the export path is untouched.
