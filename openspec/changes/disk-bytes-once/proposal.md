# Proposal: disk-bytes-once

> DESIGN (spec session), small: an executor may take it without owner questions (none here). Tests
> reserved: t3356–t3360. Base: `orch-26-11-01` at `f1f063c4`.

## Why

found-defects 774: "on the bytes path (a GL preview off the window's device) a disk-tier moment is read and
inflated on every lap" — render-cache-disk's live `render_cache` under `WGPU_BACKEND=gl`: 250 restores over
ten laps, ~4 ms of a core each at 640×360. Since render-cache-codec a restore is an H.264 decode of the
moment from its segment, so each lap costs a decode per moment.

The mechanism (`src/libs/kt-engine/src/player.rs`): the display path's `show_kept` probes the moment's key
with a plan that runs no stream stage, and on a miss `restore_from_disk` uploads the disk picture into the
executor's kept memo under `restored_key(present, durable)`; the next lap finds it held. The bytes path
skips `show_kept` (it needs a display target: `on?`) and calls `disk_bytes`, which reads the store and
copies the bytes out every time, never keeping them. The executor on that path keeps composites too
(`keeps_bytes`: `present_memo` + `read_kept`, cached-range-plays on the bytes path), but only under the
FULL plan's key — so even an ordinary kept moment on the bytes path needs its full plan, with the stream
stage's history, before it is found: the structured-telemetry defect ("сто раз пройтись по небольшому
куску - он все равно что-то loading делает") stands on this path.

## What Changes

- The bytes path finds a kept moment the way the display path does: by the probe plan's key, with no
  stream stage, decode or look-ahead; a held composite is read back with `read_kept`.
- A disk moment on the bytes path is restored ONCE into the executor's kept memo under the same
  `restored_key` the display path uses, then read back from the memo on every later lap.
- `disk_bytes` is removed; `show_kept` and `restore_from_disk` serve both targets.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `video-plan`: the bytes path shows a kept or stored moment as the display path does.

## Impact

- `src/libs/kt-engine/src/player.rs` (`show_kept` for `Target::Bytes`, `disk_bytes` gone); no trait change
  (`present_key`, `holds`, `keep`, `read_kept` exist on `VideoGpuExec`).
- Invariants 4 (the bars already count the moment as kept after the restore), 12 (a lap costs a readback of
  a kept texture, not a decode).

## Questions for the owner

None: a diagnosed defect of the preview, fixed without changing a look or a limit he set.
