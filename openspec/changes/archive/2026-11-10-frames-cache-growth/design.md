# Design: frames-cache-growth

## Context

The owner's run of stage 9 (`kkt-26.11.10` J4): "кэш странно работает, внизу показывает 39/535 MB
... если программа жрет 4.7гб, то почему тут такие цифры? и почему 535 а не больше? и даже смена
качества превью сильно не заставляет его повысить потребления". His trace (read by the orchestrator,
found-defects 396, 397): the frames cap stood at 576 MB (512 video + 64 audio) the whole run while the
ledger's growth budget stood ~21 GB; eviction spikes of 537/s; the ledger accounted 1.4–3.7 GB of the
4.7 GB the process held.

Before numbers (release `kt-cli` of the base `28caeb4f`, llvmpipe, load average 5–7, our copy of the
relinked project, his region `problem_region_26_11_09` = 4419.903–4430.987 s, Camera ISP → Datamosh →
VHS Tape; `kt-cli --trace DIR preview … --size 1280x720 --loop 120`, `trace loop`, `trace memory`,
`/usr/bin/time -v`): see "Measured" below.

## The mechanism (the class)

A size derived from a constant where the ledger's grant should stand: memory-ledger gave the process
one growth budget and a want per growing kind, but a keeper that never asks, or a reader that reads the
keeper's configured number instead of its cap, makes the budget invisible to it. The decoded-frame
cache had both halves: `BurstCache` opened `Frames` with its config as base and evicted against
`config.max_video_bytes`; the look-ahead's half, the decoder's span room and the stream pacing's
quarter read the same number. And the numbers a person reads were derived from the wrong pair: the
status bar's "cache: A/V" was audio/video read as used/limit, the performance row "Cache A/V" the same,
and nothing named the process's memory, of which the ledger is a part.

## Goals / Non-Goals

Goals: the preview's frames cache grows with its look-ahead and loop at the current quality when the
machine has the memory; every reader of its bound reads the cap; an export takes the growth back at
once; the numbers say what they count and name the whole process; the trace carries the memory.

Non-goals: cutting the bases on a small machine (row 330, its own change); the kept mix's growth
(row 331: `kept-mix-settle` edits `mix.rs` in parallel, see the class walk); a device's VRAM.

## Decisions

### D1. The cache asks; the cap is read at use

`BurstCache::want_video(total)` (on the player's `Arc`) stores what this cache asked above its base and
asks the ledger's `Frames` want. Its bound is `max_video_bytes()` = base + min(the kind's grant, its
own want), computed from the ledger's atomics whenever it is read — at eviction, by the look-ahead, by
the span room, by the pacing. A cache that never asked (an export's, a reel strip's, a poster's) keeps
its base whatever the kind's grant: the ledger keeps one want per kind, and the player's cache is its
one asker (t2391). Read at use, not pushed: the ledger tells a keeper only when its grant SHRINKS (the
hooks), so a grant given back after an export's claim would never have reached a cached cap — the first
draft held the cap in an atomic and t2390 caught it at "the claim gone, the grant is back".

Eviction stays where it was, with one addition: the shrink hook (registered at the first want, holding
a `Weak` of the cache) evicts at once when a claim or a higher tenant takes the growth back, and the
asking call evicts for its own lower want (no hook is called for the asker). Holds and pins stay
stronger than any cap (t1600–t1605 green, t2390 checks a hold through a claim). A dropped cache gives
its want back unless another cache asked since (a reopened project builds the new player first, t2392).

### D2. The want: the window twice, or the loop and an eighth and the window

The prefetch pass now also returns the bytes of its WHOLE window (every key of every moment of the 2 s
look-ahead at its decode size, counted once, not cut by the budget, kept moments left out). The window
takes half the cap (video-prefetch-thrash), so with no loop the cap wants twice the window. With a loop
region the cache keeps every frame of the region (`loop_frame_bytes`, each key once at its decode
size) plus an eighth (what a moment reads beside its own keys: a blended pair's other side, a cut's
first frames) and the window's room besides; never less than twice the window. Rounded up to 64 MiB.

The player sizes on its prefetch thread after each pass (`size_frames_cache`): a new preview quality,
loop region or build counts the loop again and forgets the window's peak; within one such key the
window's peak only rises; the ledger is asked only when the rounded want moves. So a pass asks nothing
new in steady play, a quality change resizes in the next pass, and nothing runs at rest (the prefetch
thread parks on a pause). A loop past what the machine has is not walked to its end (the count stops
at the memory available).

The ledger's order stays memory-ledger's D3: kept composites, stream history, then decoded frames. A
loop whose composites are kept needs no frames, but its want is not lowered for that: the cap is a
bound, not an allocation — the prefetch skips kept moments, so the cache holds only what is decoded.

### D3. Every reader of the budget reads the cap

- `video_prefetch_wants`'s half: `cache.max_video_bytes()` (the cap now);
- `worker.rs` span room (half): the same call, at each span;
- the stream pacing (a quarter): a renderer is a shared build made before the cap moves, so the player
  gives it `BurstCache::video_cap_reader()` (`Weak` of the cache; the base once it is gone) and
  `VideoRenderer::pacing()` reads it when it orders — `layers_at` now goes through `pacing()` too
  (t2394);
- Diagnostics' occupancy, the performance row, the status bar: `max_video_bytes()`.

### D4. What the numbers say (found-defects 397)

- Status bar: "frames: V/cap MB" and "memory: P MB" (the process's resident memory), each with one hint
  ("Frames: decoded frames held / the bound they may grow to with the loop and quality. Memory: all the
  program holds (Diagnostics shows of what)"). The audio bytes leave the status bar: 64 MB at most and
  never what a person asks about; Diagnostics keeps them inside "frames" of the caches' line. Two items,
  not one long "video V / cap · audio A", because t128 holds the status bar's items to 190 points in
  every language.
- Performance row: its first cell was "Cache A/V MB", the same audio/video pair. It now says "Program
  P MB · rest R": the process's resident memory and what no ledger tenant counts, with a hint naming
  the rest (the decoders inside FFmpeg, the graphics driver, the interface — they have no counter of
  their own, so the rest is named, not invented). The decoded frames against their bound are the
  occupancy line right under the rows: "Decoded frames: N% of their bound (V / cap MB)", in MiB.
- The caches' line gains the kept mix ("Caches: kept · history · frames · mix · device · room") and is
  one line whatever its numbers (truncated, the whole text on hover).
- No line was added to Diagnostics: a "Program …" line under the caches' line made the instruments
  one line taller than the window's fixed 720-point opening height, and the window then opened short
  and grew by itself a frame later (t918, t1147 red). egui does not keep the size of the shell's
  sizing pass for the first visible frame; fixing that is the shell's business (row 442), so the row
  that duplicated the occupancy line took the program's figures instead.
- On a discrete card the device tenants live in its memory, not the process's, and the rest then
  reads low by them (row 441).
- The process's memory is `kt_gpu::memory::resident` (Linux `/proc/self/status` VmRSS through a stack
  buffer, Windows `K32GetProcessMemoryInfo` from kernel32, so no new DLL in the imports), injected as
  `kt_time::mem::set_resident_probe`; the UI reads it at most once a second, only in frames drawn
  anyway.

### D5. The trace carries memory, never at rest

Appended after 92: `FramesCap = 93` (the preview cache's cap, the window's peak, the loop's bytes, the
quality's height — at each sizing that asked), `MemSample = 94` (resident bytes, the ledger's sum, the
memory available), `MemUsed = 95` (a tenant's bytes and cap). `kt_time::mem::sample()` writes the last
two only for what moved by a MB since the last sample; it is called once per BUSY second — the app's load
watch (`journal_second`, which runs only when the second was busy) and `kt-cli preview`'s display loop —
so a rest writes nothing (t2395). `kt-cli trace memory` prints the sizings, each tenant's bytes at the
last busy second, the process's peak with the caches' part and the rest, and the frames cache's
evictions a second (t2396).

## Measured

Release `kt-cli` of the base `28caeb4f` against this change, llvmpipe, each run under `MemoryMax=12G`
(the cgroup is the "memory available" the ledger sees: about 5 GB of growth budget once the process
holds 7 GB), pairs back to back while other sessions loaded the machine. `kt-cli --trace DIR preview …
--loop S`, `trace memory`, `trace loop`, `/usr/bin/time -v`.

**His region** (`problem_region_26_11_09`, 1280×720, 120 s of looping; 2113 MB of frames a lap at
the codec and decode sizes it asks):

| pair, new / base (load at start) | V-fps | frame evictions (worst second) | abandoned | last lap kept / shown | peak RSS |
|---|---|---|---|---|---|
| 1: new (18) / base (6–60) | 10.7 / 6.2 (and 10.1 at load 6) | 1312 (308) / 3651 (127) | 501 / 2433 | 179 / 219 — 65 / 112 | 7.3 / 5.3 GB |
| 2: new (49) / base (10) | 4.3 / 6.0 | 1037 (439) / 3675 (114) | 272 / 2452 | 39 / 116 — 61 / 109 | 6.5 / 5.2 GB |
| 3: new (16.5) / base (26.5) | 10.4 / 8.7 | 854 (334) / 3521 (134) | 137 / 2032 | 190 / 237 — 137 / 177 | 7.4 / 5.6 GB |

The frames cap rises in the first lap (2.4–2.8 GB) and then settles at 2.0–2.2 GB as the kept
composites take their grant first (they want 2.1 GB, the history 1.3 GB): inside 12 GB the budget is
just short of the loop's 2.1 GB plus its window, so the evictions that stay fall in the first ~25
seconds and in the run's last second (the loop cleared, the want drops to the window). V-fps on this
region follows the load more than anything (the composites are the work, on llvmpipe sharing its cores
with other sessions' builds); at comparable load (pair 3) the change shows 10.4 against 8.7.
`trace memory` at the peak: the program 7.3 GB, the caches 6.4 GB, the rest 0.9 GB.

**The heavy showcase** (`kt-cli showcase --heavy`, 20–30 s at 640×360, 90 s of looping; 2.3 GB of
frames a lap): the base was OOM-killed under 12 GB both times, after 68 and 70 s, having evicted
10 030 and 9 570 frames (up to 424 a second) and kept no composite in any lap (every frame was composed
again, 600 a lap). The change ran to the end: frames cap 3.4 GB, no eviction while the loop played,
from the fourth lap 185 of 190 moments shown from kept composites, V-fps 13.5, peak RSS 6.0 GB. At 1280×720
both binaries were OOM-killed under 12 GB (memory-ledger saw the same for the base, row 334).

## The class walk: sizes derived from a constant instead of the ledger's grant

| Place | Was | Verdict |
|---|---|---|
| `BurstCache` eviction (`kt-cache`) | `config.max_video_bytes` | **fixed** (D1) |
| prefetch look-ahead half (`prefetch.rs`) | `max_video_bytes()` = config | **fixed** by D1: reads the cap |
| decoder span room (`worker.rs`) | `max_video_bytes()/2` = config | **fixed** by D1 |
| stream pacing quarter (`player.rs` `preview_video`) | config at build time | **fixed** (D3, reader) |
| Diagnostics occupancy, performance row, status bar | config; audio/video pair | **fixed** (D4) |
| the player cache's want | none ("no want today") | **fixed** (D2) |
| export pipeline cache (`pipe_cache_bytes`) | its own budget by the memory | right: an export claims the growth; never asks |
| serial export cache, stream pre-roll batch (`export.rs` cap/2) | 192 MB | right: the export's cache never asks, its cap is its base |
| reel strip / poster job caches (64 / 16 MB) | constants | right: finite jobs on one frame or a strip; never ask (t2391) |
| live audio `BurstCache` (64 MB) | constant | not the mechanism while the kept mix replays a covered loop (cached-range-plays): only live chunks read bursts; a loop longer than the kept mix falls with row 331 |
| `MixStore` (64 MB = 2.9 min) | constant | **same mechanism, not fixed here**: row 331 (a loop longer than 2.9 min keeps no mix). The fix needs `Mix` in the ledger's growing order and a shrink the render thread applies (invariant 2); `kept-mix-settle` is editing `mix.rs` now. Left open with this note |
| present memo (`Kept`) | memory-ledger: base + loop grant | right |
| stream memo (`History`) | memory-ledger: base + loop grant | right; `stream_ahead_frames` reads its cap |
| upload / readback / transient / compute pools | bases | right: per-frame working sets, not a loop's |
| bases on a small machine | constants | row 330, its own change (cutting them moves the prefetch window, the pacing and export chunks) |
| `kt-cli probe` prefetch section | `CacheConfig::default()` | a tool: it reports the window against the base, not the grown cap (row 440) |
| thumbnails, effect cells, reel strips (kt-app) | counts of small images | outside: bounded by what one frame shows; part of "the rest" |
| the ledger's "room" | bases + budget | right |

## Risks / Trade-offs

- A larger cap is more resident memory on purpose: the frames cache holds what was decoded until the
  ledger takes it back. The budget is half of what the process could still take, and an export's claim
  empties it at once; a machine without a known memory keeps the base.
- One asker per kind: a second player in one process (none in the product) would overwrite the want;
  `Drop` leaves a newer want standing.
- `max_video_bytes()` is two relaxed atomic loads more per read; it is read per pass and per span, not
  per frame of a decode.
