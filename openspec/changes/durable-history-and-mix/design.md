# Design: durable-history-and-mix

> Revision 2026-11-24: the owner's D6 ("Только финальные кадры", kkt-26.11.22 line 9) withdrew the history tier
> (former D3, D4, D9 and the LZ4 codec). What stands: the restart line (D1), the mix tier (D2, D5, D6), the class
> walk (D7), the stages (D8), the tree facts (D10). The withdrawn part is recorded in "Withdrawn".

## Context

- render-cache-disk (video-plan design, "Slow moments are prerendered to disk at rest"): the final composites of
  slow moments on disk under a durable key, written at rest by the player (1.5 s with no input, never while
  playing or under an export's vote); `project-cache-folder` moves them beside the project as
  `results/v<FORMAT>/<device class>/frames/`. The export never reads them.
- The kept mix (`mix.rs`, cached-range-plays, kept-mix-settle, loop-keeps-its-lap): units of one burst
  keyed by `(MixCtx, burst index)` holding `[lo, hi)`, the `scene_hash` and the samples; owned by the
  audio render thread alone, no lock (invariant 2); the scene hash = `scene_base` + the fingerprints of
  the events heard (model text via `hash_debug`, source address, gain, the nest's content) — no file identity
  and no build. Exports never read it.
- The stream memo (Datamosh's history) stays a per-process memo (the owner's D6): a moment with a final frame
  on disk needs no history, any other walks from its cut.

## Goals / Non-Goals

Goals: after a restart, the mix already computed by an earlier run on this machine comes back from disk; nothing
restored can be stale; no byte changes; the render thread never waits on a disk; the trace shows what a restart
cost.

Non-goals: storing the stream history (D6); carrying the mix across devices or decoder majors; the export
reading the mix.

## D1. What a restart costs (the trace)

The trace gains a `Restart` summary over the first minute after a project opens:
- the history positions walked from their cut (`kt_plugin_api::stream::walked` already counts them per walk),
  with their device time — the price of D6, shown so the owner sees it on his projects;
- the covered audio bursts rendered live again, and the bursts the mix tier restored.

`kt-cli trace summary` prints "after the start: N history positions walked (M ms of device), K covered audio
bursts rendered again, R restored".

## D2. One tenant of the project's cache folder

The mix runs are `mix/v<FORMAT>/` in the project's cache folder (`project-cache-folder`, kkt-26.11.20 line 19).
They share its rules:
- the entry header and the atomic write (`project-cache-folder` D1);
- an entry of another salt is a miss, removed when met (no sweep at open);
- the folder's cycle (`project-cache-folder` D3, the owner's B2): one order by value per byte across the final
  frames, the copies and the mix runs, never what this process used in the last minute; a notice once per
  session.

## D5. The kept mix on disk

- **Key**: `(MixCtx, burst index, durable scene)`, durable scene = `mix(scene_hash, salt)`. The salt is the
  store's (`project-cache-folder` D1b: the decoder's major, since the decoders' float output may move between
  majors, and the versions of the audio kernels that ran; not the whole build) and the identities of the audio
  files heard in the burst's widened range.
- **Form**: runs of consecutive bursts of one context, raw interleaved f32, a manifest per run (the context, the
  first burst, each burst's `[lo, hi)` and durable scene). Raw: 23 MB a minute, and a read is a copy.
- **Writing**: at rest, from what the store holds. The render thread owns the store: it publishes, through the
  existing `MixCoverage` path, an immutable snapshot of the units not yet written (an `Arc` of the samples it
  already keeps; no copy on the render thread); the player's thread writes them.
- **Restoring**: the player's prefetch thread, which already sizes the store's budget (`MixBudget::ask`), reads
  the runs whose scenes match the current renderer ahead of the playhead and of the loop region, and sends them
  over a bounded lock-free channel; the render thread drains it at the start of a chunk into its store as "first
  recordings" (a burst already held is not overwritten). Invariant 2 holds: the store is touched by the render
  thread alone, and it never reads a file. Invariant 6: the callback is not involved.
- **Settle**: a restored burst behaves as a kept one: leaving it, the effects are brought back to the continuous
  play (kept-mix-settle).
- **Exports never read it**: the audio export stays a live render, byte for byte (invariant 11).

## D6. Coverage bars (invariant 4)

The bars' kept-mix band adds the bursts of restorable runs for the current scene, by the composites' scan at
rest; a burst shown bright is handed out with no decode and no effect run once loaded. Video is unchanged: a
moment is covered when its final frame is kept or on disk.

## D7. Class walk: every per-process result that a restart recomputes

| Result | Survives a restart after this change | Note |
|---|---|---|
| Final composites of slow moments | yes (render-cache-disk, `project-cache-folder`) | unchanged |
| Final composites of light moments | no | cheap to compose by definition |
| Stream memo: checkpoints, broken pictures | **no** (the owner's D6) | recomputed from the cut; a moment on disk needs none |
| Stream memo: live device tier, front memo | no | per process |
| Kept final mix | yes (D5) | preview only |
| FX state of audio effects | no | state, not a result; settle covers it |
| Decoded frames (burst cache) | no | the sources are on disk already |
| Peak pyramid, thumbnails, reel strips, packet index | yes (the media index) | unchanged |
| Pass-time measurements, compiled pipelines | yes (shader cache) | unchanged |

## D8. Stages

1. D1: the restart line and the mix tier's records in the trace. t3336, t3337.
2. D2 + D5 + D6: the mix runs, the handoff, the bars. t3346–t3352.

## D10. What the tree has today (checked at orch `873934f7`, the mix half rechecked at `af14db35`)

- **The mix's samples are a `Box`.** `Unit.data` (`mix.rs:56`) is `Box<[f32]>`. D5's no-copy snapshot needs it
  to become `Arc<[f32]>` (one allocation per kept burst, as today).
- **There is no channel into the render thread today.** `RenderCtx` (`player.rs:2081`) has atomics, mutexes
  and the outgoing `rtrb::Producer`. The restore handoff is a second `rtrb` ring (bounded, lock-free; `rtrb`
  is already a dependency), drained at the top of a pass in `audio_render_loop` (`player.rs:2442`). The
  loader is `prefetch_loop` (`:3400`), beside its `mix.budget.ask` (`:3522`). `MixCoverage` (`mix.rs:499`)
  carries the snapshot of unwritten units out.
- **The scene hash** is `audio.rs:1012` (`scene_base` + `clips[i].fp` over the widened range's active clips).
  That widened range is where the mix salt's audio file identities are collected.
- **The trace kinds** for mix runs go into `kt_time::trace::render_cache` (`trace.rs:631`). The restart summary
  has no recorder yet (D1), nor `kt-cli trace summary`'s line (`trace_query.rs:466`). `trace cache` (`:130`) is
  extended.
- **The store** (folder, header, `ask_room`, the key without the build) is `project-cache-folder`'s sections
  1 and 3. This change starts after them.

## Withdrawn (the owner's D6, kkt-26.11.22 line 9)

The previous revision stored the stream history: an `EntryCodec` on `StreamKernel`, the memo's backing behind
`put_entry`/`holds`/`get`, a thread-local scope carrying the stage's salt, Datamosh's codec of `Planes`, LZ4
(`lz4_flex`) in kt-engine, a readback of device-held checkpoints at rest (t4105), the `history/` kind of the
persistent cache. The cache audit (`docs/cache-audit.md`, cut 5) asked him whether to keep the disk history or
keep only the final frames and recompute the history from the cut on a miss; he answered "Только финальные
кадры". What it costs is the walk from the cut after a restart, for a moment not held final; D1's line shows it.

## Risks

- A restored run slower to read than the live render it replaces (a spinning disk): the trace shows restore time
  against render time per run, and `KT_RENDER_CACHE=off` turns it off.
