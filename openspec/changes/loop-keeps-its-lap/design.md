# Design: loop-keeps-its-lap

## The mechanism (the class)

**A bounded cache over a cycle longer than itself.** Least-recently-used order over a loop of N
entries in a cache of M < N evicts, at every insertion, the entry read longest ago — which is the one
the next lap reads first. Nothing survives to be read again: a loop one entry too long keeps nothing,
and every lap pays the whole work (a decode, an effect run, a codec walk, an audio render). The fix
is an admission rule over the current lap: keep a stable part of the lap, pass the rest through. The
present memo had one (memory-ledger D5: refuse over a lap), the stream memo got one (stream-memo-budget
D1: a window past the kept part); the kept mix, the frames cache and the front memo had none.

A second, smaller class rode along: **a keeper outside the ledger** (rows 542, 550) — a fixed constant
that neither grows where the memory is nor counts in the sum.

## Decisions

- **D1. The kept mix grows by the ledger, first in the order of value.** `Tenant::Mix` joins
  `GROWING` ahead of the kept composites: a minute of a loop's mix is 22 MB (a tenth of a second of
  1080p frames in the frames cache) and a hit saves every decode and every effect of the audio render;
  an underrun is heard, a missed composite costs a frame. The want is the region's bursts plus an
  eighth, rounded to 16 MB (`mix_want`); typical loops ask tens of MB, so the memos behind it lose
  little. `grants` takes N kinds (const generic): t2290's three-kind form stands.
- **D2. The render thread never sizes the ledger.** `Ledger::want` runs a sizing and the shrink hooks
  of the kinds it takes from (the frames cache evicts under its lock, the present memo takes the
  executor's) — not work for the audio render thread. `MixBudget` (shared through `MixCoverage`) holds
  the account and the store's own want; the prefetch thread asks it when the region changes; the
  store reads `base + min(grant, own want)` at each new burst (two atomics, as the frames cache reads
  its bound). A grant taken back calls the hook the player set: it wakes the render thread, which
  calls `MixStore::fit` at the top of its loop (a paused thread parks; a claim must still free the
  memory). Only the asker grows (t2602), like the frames cache.
- **D3. The kept mix keeps a PREFIX of a loop it cannot hold.** The store knows its loop exactly
  (`retain_laps(region)`, now also "keep this loop", told at the render thread's start too), and its
  laps are keyed by timeline burst, so the rule is positional, not by lap stamps: bursts of any other
  context (linear play, inside or outside the region) are evicted least recently heard first; when
  only the loop's laps are left, a new lap burst earlier than the latest kept takes the latest's place,
  a later one is not admitted (`write` returns false, trace `mix_chunk` aux `passed`). A linear burst
  finds no room in a store full of the loop's prefix while the region is set. Why a prefix: the head of
  the lap is what plays right after the wrap, the moment a live render is hardest (a seam, the effects
  warming), and a positional rule has no warm-up of its own — the kept part is the same every lap from
  the second on. The kept-mix-settle rules are untouched: the chunk past the prefix warms through the
  settle window and is not kept (t2603).
- **D4. The frames cache takes the stream memo's lap rule, per pool.** `keep_loop(on)` starts a fresh
  lap count; `lap()` is the player's mark (where it marks the stream memo's: the moment shown went back
  inside the loop while playing). Every use stamps the entry's lap (a read, a refresh, its insertion).
  While a loop is kept, a new entry put into a pool that is full and holds no stale kept entry (none
  unused for two laps) enters the WINDOW. The victim: past half the pool's bound, the window's oldest
  entry already READ; else the oldest kept entry (a stale one if any — a use stamps the lap with the
  recency, so the oldest kept carries the lowest lap — else the kept part's least recent: the window
  grows into it); else the window's oldest; else the new entry. Half, not the stream memo's quarter:
  the look-ahead takes up to half of the video bound (`video_prefetch_wants`). Only READ window
  entries go past the share: the look-ahead is also up to half the bound, and dropping its unread
  frames is thrash (the first form of t2606 dropped 1047 of them). Holds and pins stay stronger. The
  audio pool takes the same rule (its loop head stays pinned). `keep_loop(false)` turns the window into
  plain entries.
- **D5. The front memo is a `StreamMemo`.** Row 542 asked "a ledger tenant, or fold it into the
  stream memo's grant". The memo the kernel reads is per renderer, but the front runs inside
  `ClipStream`, which has none; so the front memo stays one per process and becomes a `StreamMemo` of
  its own: a History account with its 64 MB base, the lap rule told with the preview's stream memo
  (`stream_keep_loop` / `stream_lap` when the renderer shares the preview's memo), a hash map instead
  of a queue scanned per lookup. Its evictions are `stream_memo_evict` records (keys of the front's
  address). It does not grow: a front output is a position's input, re-read only when the history
  walks it again, and the stream memo keeps the history itself.

## Class walk (what was checked, what was found)

Every holder a loop or a repeated range reads, by subsystem.

| Holder | What reads it over a loop | Sizing | Admission over a lap | Verdict |
|---|---|---|---|---|
| `MixStore` (kept final mix) | the render thread, every lap | was fixed 64 MB | none | **fixed**: D1–D3 (row 550) |
| `BurstCache` video pool | prefetch, composite, every lap | ledger by the loop (frames-cache-growth) | none past the grant | **fixed**: D4 (row 551) |
| `BurstCache` audio pool | the render thread's live chunks, prefetch | 64 MB base | loop head pinned only | **fixed**: D4; the kept mix serves what it holds, so the pool serves the rest |
| `BurstCache` `PostFxVideo` kind | no executor reads it (kt-gpu CLAUDE.md) | the video pool's | D4 applies (same pool) | n/a |
| front memo (`video_stream.rs`) | a re-walk of a stream stage's history | was fixed 64 MB, outside the ledger | none | **fixed**: D5 (row 542) |
| `StreamMemo` (preview) | the stream stages' history walks | ledger (stream-memo-budget) | window past the kept part (t2490) | ok |
| `StreamMemo` (export, strip, poster) | one linear pass | fixed | no laps | ok |
| `PresentMemo` (kept composites, kt-gpu) | the display, every lap | ledger by the loop's moments | refuses over a lap (t2293) | ok |
| kt-gpu `UploadCache` (preview device) | a composite that is not kept | fixed base | LRU | n/a: a kept moment uploads nothing; a moment composed again uploads once per composite, its own cost |
| kt-gpu `ReadbackCache` | strip, poster, picker, the slow route's cold run | fixed | LRU (rows 521/523) | n/a: the slow route keeps its result in the present memo (`keep`) |
| kt-gpu transient pool, compute pool | per-frame working sets | fixed | — | n/a: not held across moments |
| kt-gpu `BindCache`, pipeline cache, pass-time seeds | frame-scoped binds, compiled code, timings | — | — | n/a: no pictures or mixes |
| kt-fx `rasterize_cached` (mask coverage, 16) | a static mask on a playing event | count cap | — | n/a: an animated mask's coverage changes per frame; the kept composite spares it |
| nested reel sub-mix (`NestFetcher` ring) | the render thread | by the project (pre-roll, block: invariant 11) | a ring, not an LRU | n/a: its size must not depend on memory |
| waveform peak pyramid, sidecar index DB | the timeline (scroll, zoom) | whole / disk | — | n/a: not a loop path (invariant 8) |
| reel previews (memory LRU + disk tier) | timeline strips | count cap | never evicts what a visible event waits for | n/a: not a playback path |
| thumbs / fx-thumbs LRU | timeline, effect gallery | count caps | never evicts what the last frames drew (t2237) | n/a |
| kt-app `CoverageCache`, `WarpCache`, names | UI frames, keyed by revision | one entry | — | n/a |
| `kt-cli preview --loop`, `probe` | tools | as the player / live | — | the preview drives the player; `probe --time` keeps measuring the live cost |

## Risks

- Mix first in the order means a long loop's mix can take growth the kept composites wanted: a 10-minute
  loop asks 260 MB. Accepted: the audio is the clock (invariant 6), and its kept minute is the cheapest.
- The window's victim search scans the LRU from its oldest (to the first read window entry, or the
  first kept one). In steady state the first entries are what it wants; a pool with no read window entry
  and no kept one is walked whole once per eviction. Pools hold hundreds to a few thousand entries.
