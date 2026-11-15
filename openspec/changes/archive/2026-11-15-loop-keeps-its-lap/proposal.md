# Proposal: loop-keeps-its-lap

## Why

stream-memo-budget's class walk left two holders that keep nothing of a loop longer than their bound
(`docs/found-defects.md` rows 550, 551), and row 542 left the front memo outside the memory ledger.
The mechanism is one: **an LRU over a cyclic access pattern longer than itself evicts exactly what the
next lap reads first**, so a loop one entry too long keeps nothing and pays the whole work every lap.

- 550: the kept final mix (`MixStore`) was a fixed 64 MB LRU — 2.9 minutes. A longer loop rendered
  every burst, every effect, every lap; the memory ledger did not size it.
- 551: the decoded-frame cache asks the ledger for the loop, but past the grant (a machine without the
  memory) its video LRU had no rule over the lap: every lap decoded every frame.
- 542: the front memo (the outputs of the slots in front of a stream stage) was a fixed 64 MB queue,
  outside the ledger, with the same LRU over a loop.

The owner (kkt-26.11.07b): a covered range keeps its sound like a prerender in VEGAS; (2026-11-07) a
problem is a class, examined in every subsystem; (2026-11-01) caches where they pay, bounded, keyed by
what determines them.

## What Changes

- The memory ledger grows the kept mix: `Tenant::Mix` joins the growing kinds, FIRST in the order of
  value (a minute of a loop's mix is 22 MB and saves the whole audio render). `MixBudget` holds the
  account and the store's own want; the player's prefetch thread asks it by the loop region
  (`mix_want`), the render thread's store reads its bound at use and evicts when woken after a shrink.
- `MixStore` keeps a stable PREFIX of a loop longer than its bound: the bursts of other contexts go
  first, then the lap's latest; a lap burst past the kept prefix is not admitted (trace `mix_chunk` aux
  `passed`).
- `BurstCache::keep_loop` / `lap` (both pools): the stream memo's lap rule — an entry the current or
  the last lap used is not evicted for a new one; new ones pass through a window of half the pool's
  bound (the look-ahead's share), whose entries already read go first. The player tells the loop with
  the stream memo's and marks the laps.
- The front memo is a `StreamMemo` (History tenant, 64 MB base) told the preview's loop and laps with
  the stream memo (row 542).
- `kt-cli trace loop` names the new reason.

## Impact

kt-time `mem.rs`, `trace.rs`; kt-cache `lib.rs`; kt-engine `mix.rs`, `player.rs`, `video.rs`,
`video_stream.rs`; kt-cli `trace_query.rs`. Specs: media-index, audio-render. Tests t2600–t2608.
