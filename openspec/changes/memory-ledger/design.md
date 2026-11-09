# Design: memory-ledger

## Context

Row 9 of the order table of `docs/engine-audit.md` (M9, scheduler stage 3). Before numbers, release
`kt-cli` of the base `e8091e8e`, llvmpipe, load average 20–45:

**The owner's region** (`problem_region_26_11_09` of the relinked copy, event 2149: Camera ISP →
Datamosh → VHS Tape; 11.09 s = 665 grid frames at 60 fps), `kt-cli preview … 4419.903 4430.987 --size
640x360 --loop 240 --trace`, read with `kt-cli trace loop`:

| | before |
|---|---|
| 240 s of looping | 1710 composites made, 4369 shown from kept, 414 incomplete, V-fps 22.9 |
| laps 19–21 (shown / kept / made) | 418 / 380 / 38, 418 / 380 / 38, 266 / 241 / 25 |
| "evicted from the kept composites before this lap" | 7–48 a lap from lap 9 on |

640×360 is 0.92 MB a composite, so the constant 512 MB holds 582 of the region's 665 moments: the
memo is full before the first lap ends, and its LRU drops exactly the moment the next lap reads first.
The loop never becomes all-kept (found-defects 310). The "key changed: another stream frame" lines of
the early laps are frames composed while the stream stage showed a held frame (structured-telemetry);
they converge as the stream memo holds the history.

Export peaks (`/usr/bin/time` max RSS, `kt-cli export … libx264 --time`, our showcase, under
`MemoryMax=12G`): the table of D7.

After (same command, release `kt-cli` of this change), and the base binary run again right after it
at the same load (load average 7–11) for a fair pair:

| 240 s over his region, 640×360 | base | memory-ledger |
|---|---|---|
| composed / shown from kept | 1935 / 5407 | 1172 / 8943 |
| laps 14–21: made a lap | 47–58 | 0 (every lap from 10 on) |
| V-fps, incomplete | 28.9, 313 | 40.4, 326 |
| process CPU | 11.8 cores | 6.5 cores |
| ledger (`kt-cli trace memory`) | — | growth budget 5.4 GB of 10.8 GB free; kept 659 MB cap, stream 1576 MB cap |

The heavy showcase (`showcase --heavy`, 30–60 s at 640×360) could not be looped under the 12 GB cap
with either binary: both were killed by the OOM killer within 48–87 s. That led to D11.

## The mechanism (the class)

**A cache sized by a constant that ignores the machine and the other caches.** Each keeper picked a
number when it was written — enough for its own test, on one machine — and evicts by its own LRU.
Three consequences:
1. a working set that is longer than the number (a loop) is recomputed every time, whatever the
   machine has free (row 310; row 143 was the stream memo's case);
2. nobody sees the sum, so one keeper growing (the stream memo since stream-layout-per-build) is
   invisible to the next one sized against `available` (an export's device count);
3. LRU over a cycle longer than the cache keeps nothing that the cycle reads next.

## Goals / Non-Goals

**Goals:** one ledger that every keeper of pictures, mixes and device buffers reports to; growth above
the proven working sets granted from one run-time budget in a stated order; the present memo sized by
the loop (row 310); a loop that does not fit keeps a part; an export takes the room; the export's
devices drop the uploads an export never reads again; row 119 held end to end; the ledger in the trace,
`kt-cli trace memory` and the Diagnostics window; nothing at rest.

**Non-Goals:** shrinking any base on a small machine (the bases are what the prefetch window, the
stream pacing and the export chunks are derived from — row 330); reading a card's memory through
backend-specific APIs; a global LRU across caches; the undo history and the peak pyramid (they are the
model, not caches).

## Decisions

### D1. The ledger is a table of tenant kinds in `kt_time::mem`, accounts per instance

- `Tenant` is a closed enum, its declaration order IS the order of value: `Kept` (present memo),
  `History` (stream memo), `Frames` (burst cache), then base-only kinds: `Mix`, `Upload`, `Readback`,
  `Transient`, `Compute`. A keeper opens an `Account` of its kind and calls `set(bytes)` on every
  change (two relaxed atomics: its own and its kind's sum); dropping it subtracts. No lock, no
  allocation on that path, so the audio render thread's MixStore may report too.
- Each kind has a base (the sum of the bases its accounts declare) and a want (what the keeper that
  can grow asks for, total, base included). Only `Kept`, `History` and `Frames` grow; the others are
  counted.
- Where it lives: kt-time already holds the process-wide coordination (`sched`, `trace`, `work`) with
  no dependency; kt-cache, kt-gpu and kt-engine all reach it. The probe (`kt_gpu::memory::available`,
  cgroup-aware) is injected by the host (`mem::set_probe`) as the stream memo's was; the engine's
  `set_memory_probe` now forwards there.

### D2. The budget: half of what the process could still take beyond its bases

`growth budget = (available + growth held) / 2`, where `growth held = Σ max(0, used − base)` over the
growing kinds, measured when a want or a claim changes. Adding back what the ledger already holds keeps
the budget from shrinking as the memos fill (they are what made `available` smaller). The other half
stays for what the ledger cannot count: the decoders' frames inside FFmpeg, the driver and egui, an
export's devices started later, other programs. A cgroup limit counts (the probe reads it). Unknown
memory (a platform with no probe) grants nothing: the bases are the old constants, so such a machine
behaves exactly as before.

- **Rejected: a fraction of the total memory.** It ignores what other programs and a cgroup leave.
- **Rejected: re-measuring on a clock.** The program at rest costs nothing; a sizing runs when a loop,
  a preview size, a stream frame size or a claim changes — the moments a want changes.

### D3. The order of value: kept composites, stream history, decoded frames

A kept composite replaces everything a moment costs (decode, stream stage, the chain) with one display
pass; a held stream picture saves the codec but still runs the rest of the chain (VHS Tape and Camera
ISP: ~65 ms a frame at 640×360 on llvmpipe); a decoded frame saves the decode only. When the present
memo holds a whole loop, the stream history of that loop is not read again (t2145), so granting the
composites first is never worse. Taking back goes the reverse way: decoded frames' growth, then
history, then composites. A grant that shrinks calls the account's `on_cap` hook after the ledger's lock
is released, and never for the account whose own want triggered the sizing (the present memo asks
under its engine's lock; its hook takes that lock).

- **Rejected: one global LRU by value per byte.** It needs a lock across crates (an eviction in the GPU
  engine decided on a decoder thread). The value of a byte is stable by kind, so a fixed order gives
  the same choice without a shared lock.

### D4. The present memo's want comes from the executor, told the loop's length

The player calls `VideoGpuExec::keep_loop(Some(moments))` on the display executor when the loop region
or its frame count changes, and `None` when it is cleared (a new trait method, default no-op). The
executor knows the size of what it presents, so its want is `moments × w × h × 4 × 9/8` at its last
composite size, re-asked when that size changes. The memo's cap is `base + grant`, read from its
account on every insert. The 512 MB base stays: a paused frame and a short scrub are kept as before.

### D5. A loop longer than the memo keeps a part (no admission over the current lap)

With a loop of `n` moments told and the memo full, the least recent entry is evicted only if it was
last shown more than `n` presents ago; otherwise the new composite is not kept. In a cycle each kept
moment is shown once a lap, so its age stays under `n` and the kept set is fixed: a lap shows
`cap/frame` moments from it instead of none. When the loop moves away its entries age past `n` and go
first. Without a loop `n = 0` and the memo is the plain LRU it was.

### D6. An export claims the growth

`kt_time::mem::Claim` (held by the body of `export_pipeline::run` and the serial export, like the
scheduler's `Vote`): while any claim lives the growth budget is zero, so every grant drops at once
through the hooks (the stream memo's `set_cap`, the present memo's eviction under its engine's lock,
from the export's thread). The export then measures `available` for its device count with that room
back. Dropping the last claim re-sizes with the wants as they stand. Holds are untouched: the burst
cache's holds are stronger than its cap, and a claim takes back growth only.

### D7. An export's device keeps the uploads of the frames in flight

An export uploads a source frame once (twice at most for a slowed clip read by consecutive frames).
`UploadCache` gets a frame counter (`begin_frame` per plan resolved); on an export device
(`new_for_export`) an entry not read by the last `RING` frames is dropped at the next frame start, and
on every device an entry read by the frame being resolved is never evicted or reused as a spare within
that frame (with a 256 MB budget that never happened; with fewer entries it would overwrite a layer's
texture before its pass reads it). The spares (four a size) stay. Before/after peaks:

| showcase export, max RSS (`/usr/bin/time`, `MemoryMax=12G`) | before | after |
|---|---|---|
| 1080p, run back to back at one load | 8.43 GB (96.8 s) | 7.07 GB (98.1 s) |
| 1080p, first runs (load 45 against 7) | 8.42 GB | 7.02 GB |
| 4K (tight under the 12 GB cap: two decoder threads, half the cache) | 4.76 GB | 4.96 GB |

At 1080p the upload caches of the export's devices held about a GB of frames already written. At
4K a 256 MB upload cache held seven frames, so there was nothing to take back; the 4K render stays
under 5 GB because it runs tight inside the cap (found-defects 85: measured now, on llvmpipe only).

### D8. A card's memory is learned from its first refusal

wgpu does not expose a device's memory size, and backend extensions (Vulkan's memory budget) are not
portable to DX12/Metal/GL. On a device whose type is not `Cpu` (llvmpipe's textures are the process's
memory, which the ledger counts), the present memo creates the texture its growth needs inside an
out-of-memory error scope before keeping a composite past its base. A refusal keeps the composite
unkept, sets the memo's device limit to three quarters of what it holds (evicting at once, so the
frame's own transients find room), records `mem_grant` with the limit, and leaves the device usable
(an OOM is a scoped error, not a loss). A test hook forces the refusal (t2295).

### D11. The preview's decoders take a render's threads (found by the class walk)

The heavy showcase's preview held 4.4 GB of anonymous memory 6 s after it started, with nothing cached
yet: 160 FFmpeg frame threads (`av:vp9:df` ×128, `av:vp8:df` ×32) — libavcodec's automatic count, a
thread per core, for every decoder of the preview, each frame thread keeping its own pictures. The
render already sized its decoders by memory (`memory::decode_threads`: four, two when tight;
gpu-render-hardening D1 found auto no faster for a render). A/B of the preview, same 30 s, same load
(`KT_DECODE_THREADS` on `kt-cli preview`):

| heavy showcase preview, 30 s | auto | four |
|---|---|---|
| peak RSS | 9.9 GB | 6.3 GB |
| decode time a picture | 3.6 ms | 3.9 ms |
| process CPU | 15.3 cores | 11.7 cores |
| render underruns | 23 247 frames | 0 |

So the editor sets the render's rule for the project's frame size when it opens a project
(`build_player`), `kt-cli preview` does the same (`KT_DECODE_THREADS` keeps the A/B), and the render
window restores the preview's count after opening its own sources instead of resetting it to auto.
On the owner's region (release, 240 s, 640×360) four threads against auto: V-fps 39.4 against 38.1
(four ran at load 29, auto at load 3), peak RSS 3.99 against 4.38 GB, 6.6 against 7.1 cores; both
all-kept from lap 10–11. The heavy showcase's loop is never kept on llvmpipe with either binary — its
frames come out incomplete (found-defects 334) — so it measures the memory, not the kept loop.

### D9. Visible

- Trace (appended after the highest kind, 90): `MemBudget = 91` (`a` growth budget, `b` granted,
  `c` available MB, `aux` claims), `MemGrant = 92` (`id` tenant, `a` want, `b` cap, `c` bytes held).
  Only a sizing that changed something writes.
- `kt-cli trace memory`: the sizings in order and each tenant's last cap and bytes.
- Diagnostics: one line under the occupancy bar, from `mem::snapshot()` (atomics only).

### D10. Row 119

structured-telemetry made a re-decoded entry take back its serial (t2152, cache level). The plan key
of a moment is built from those serials, so the kept composite is found again. t2294 holds it through
the player: a small cache evicts the frame, the same frame is inserted again, the present replays.

## The class walk (every cache, memo, pool and disk tier)

| Keeper | Sized by | Verdict |
|---|---|---|
| live BurstCache video/audio | 512 + 64 MB constant | account `Frames`, base; growth slot in the order (no want today); pins and holds stronger |
| export pipeline cache | 3 × 192 MB (tight: half) | account `Frames`, base; the export claims growth anyway |
| serial export cache, export StreamMemo | 192 MB, 64 MB | accounts `Frames`, `History`, base |
| reel strip / poster job caches | 64 / 16 MB per job | accounts `Frames`, base, short-lived |
| kt-cli tool caches (`stream-cost`, decode bench) | 2–3 GB, tools only | accounts `Frames`; not the product |
| PresentMemo (every engine) | 512 MB constant | **fixed**: `Kept`, base + loop grant, no-admit, refusal guard (rows 310, 119) |
| ReadbackCache | 64 MB per engine | account `Readback`, base (readback path: harness, offline engine) |
| UploadCache | 256 MB per engine | account `Upload`; **export devices: frames in flight**; same-frame reuse guard on all |
| TexturePool transients | 128 MB per engine | account `Transient`, base |
| compute host pool | 1 GiB (plugins spec, t1570) | account `Compute`, base |
| preview StreamMemo | 256 MB + loop want ≤ ¼ available | **through the ledger** (`History`, after `Kept`) |
| MixStore | 64 MB = 2.9 min of mix | account `Mix`, base; a loop longer than 2.9 min keeps nothing by LRU — row 331 |
| thumbnails, fx cells, reel strips (kt-app) | 384 / 256 / 6×48 small images | outside: bounded by count of thumbnails (< 0.1 GB), never grows with a loop or the machine |
| sidecar DB (index, peaks, thumbnails, reel tier), posters, pipeline disk cache, trace files | disk | outside: disk, not memory |
| peak pyramid | per media length | outside: it is drawn every frame, not evicted; grows with the project |
| warp memo (2^18 points), mask memo (16) | counts | outside: small math memos |
| decoders of a render | `memory::decode_threads` by the memory | unchanged |
| decoders of the preview | libavcodec's auto: a thread per core | **fixed** (D11): a render's rule by the memory when the project opens |
| export device count | `engines_for(available)` | unchanged; meets the room the claim gives back |
| undo history (80 clones), flight ring, audio ring | counts | outside: model and fixed rings |

Not memory-related, left: row 290 (the offline engine's `inner` held a whole frame) is a wait, M3.

## Risks / Trade-offs

- **A grant on a card larger than its memory** → learned by the refusal (D8); a refusal outside the
  memo's scope (another allocation) would still poison: the three-quarter step leaves room after the
  first refusal, not before. Accepted; noted in found-defects if seen.
- **The present memo grows to GBs at 1080p** (665 × 8.3 MB ≈ 5.5 GB for the owner's region): only with
  a loop set and only within half the free memory; a claim or clearing the loop brings it back.
- **Hooks under locks**: a hook is never called for the asking account and always after the ledger's
  lock is released (D3); t2291 holds the order.
- **Invariant 4**: an evicted kept moment leaves the bars at the next `recheck_shown`, as before.
- **Invariant 11**: no cap changes a byte (t2292 compares pictures, t2298 export bytes, t2299 export
  with and without grants).
