# Design: disk-bytes-once

## Context

- `Player::compose` (`player.rs`): `show_kept(video, frame, t, q, on)` returns `None` at once when `on`
  (the display's present options) is `None`, which is the case for `Target::Bytes`; the bytes target then
  calls `disk_bytes`, which reads `DiskStore::get(dk)` and moves `p.rgba` into `out`, every call.
- `restore_from_disk(display, …)` already takes any `Arc<dyn VideoGpuExec>`: it computes `present_key` of
  the probe plan, `restored_key(present, dk)`, returns it if held, otherwise reads the store, `keep`s the
  picture and notes the moment kept.
- kt-gpu `Engine::execute`: with `keeps_bytes` (the executor off the window's device) a kept composite is a
  texture in `present_memo`, read back by `read_kept`; it is found by `plan_key(plan)` of the FULL plan.
  `VideoGpuExec::read_kept(key, out)` reads a kept composite by key.

## D1. One way to show a kept moment, for both targets

`show_kept` takes the target instead of the options:

- probe plan → `present_key`; if not held → `restore_from_disk(exec, …)` (both targets);
- `Target::Display(o)`: `present_kept(key, o)` (today);
- `Target::Bytes(out)`: `read_kept(key, out)`, the size checked against the plan's output size.

The executor is `display_exec()` for the display target and the bytes path's executor (`video_gpu.exec`)
for the bytes target — the same one `disk_bytes` used.

## D2. The restore happens once

The first lap restores into the executor's memo under `restored_key`; every later lap finds it held and
reads it back (~a texture copy at the preview size, no decode). The memo is bounded and loop-aware already
(`keep_loop`, memory-ledger D4); a moment it drops is restored again from disk, as on the display path.

## D3. What a kept moment on the bytes path costs now

Before: an ordinary kept moment needed the full plan (`plan_key`), hence the stream stage's history before
the memo was even asked. After: the probe plan's key, as structured-telemetry made the display path. The
readback memo for `!keeps_bytes` executors is untouched (it keys by `plan_key`, which the export does not
use and the harness paths do).

## D4. Class walk: every place that turns a stored or kept moment into a picture

| Place | Before | After |
|---|---|---|
| Display path, kept (`show_kept` → `present_kept`) | probe key | unchanged |
| Display path, disk (`restore_from_disk`) | restored once | unchanged |
| Bytes path, disk (`disk_bytes`) | read + decode every lap | restored once (D2) |
| Bytes path, kept (`execute` → `present_memo` by `plan_key`) | full plan first | probe key (D3) |
| Poster from kept (`read_kept` by an offline job) | by key | unchanged |
| Render-ahead bytes route (`ahead_by_bytes`) | keeps on the keeper | unchanged |
| kt-cli `preview --render-cache`, `pause-scan` | bytes path | gains D2/D3 |
| Export | never reads the tier | unchanged |

## D5. Tests

t3356 (the bytes path restores a disk moment once over ten laps: one store read, ten memo hits, the bytes
equal the stored picture), t3357 (a kept moment on the bytes path is shown with no stream stage run: the
probe path, the stream's walk counter does not move), t3358 (the live `render_cache` under
`WGPU_BACKEND=gl` counts restores ≤ the moments, not ×10; the kt-cli `preview --render-cache` line).
Spare: t3359, t3360.
