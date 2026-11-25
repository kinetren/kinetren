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

(Revised 2026-11-25 against orch `fc60a5d9`.)

- **Key**: `(MixCtx, burst index, durable scene)`, durable scene = `mix(scene_hash, salt)`. The salt has three parts:
  1. the decoder's major (the decoders' float output may move between majors);
  2. **what makes the sound**: `AUDIO_PATH_VERSION`, the hash of the files named in a new `kt-engine/audio-path.txt` (built by the same `build.rs` as `composite-path.txt`: `audio.rs`,
     `fxstore.rs`, `mix.rs`' render path, kt-fx `audio/`, the resampler, `kt-media`'s audio decode, `kt_plugin_api::{math, dsp}`), and `kernel_version` of every audio plugin the project's chains name.
     `Made` knows picture kernels only;
  3. the identities of the sounds heard in the burst's widened range. **The sound's identity is the ORIGINAL file's**: `persistent-cache` swaps a source's picture for its copy's, and the
     cache then names the source by the copy (`CopySource::identity`) while its sound goes on being the original's. So `MediaSource::sound_identity()` (default: `identity()`; `CopySource`: the original's),
     kept per media by `MediaParts` at the raise, and NOT `BurstCache::ident`: with that the kept mix of a heavy clip would be a miss whenever its copy came or went. A media without one is
     never written.
- **Form**: runs of consecutive bursts of one context, raw interleaved f32, a manifest per run (the context, the
  first burst, each burst's durable scene). Only WHOLE bursts (`lo = 0, hi = UNIT`). Raw: 23 MB a minute, and a read is a copy. `Tenant::Mix` of the cache folder exists and has its place in the one
  order (`by_value`); the entry's value is its render time, measured as the chunk was rendered.
- **No thread of a picture or of the sound calls the disk** (gpu-waits-named, found-defects 835: work for a picture called the disk while it held what the next frame needed). Reads and writes run
  on a lane of their own, `Lane::open("mix-disk", Class::Background)`, one job a run.
- **Writing**: the render thread publishes, through `MixCoverage`, an immutable snapshot of the whole units not yet written (an `Arc` of the samples it keeps: no copy on the render thread). A whole unit is never
  written to again (first recordings stay), so the shared `Arc` is never cloned for writing and the warm render allocates nothing (t2684). The `render-cache` thread posts the writes: a step at the top of its loop, after its
  play/export check and BEFORE it asks whether an encoder opens (a machine with no hardware encoder keeps its mix too), under the same quiet. What is in hand when the process ends is left
  (`kt_time::shutdown::ending`, as the open segment, row 972); a file without its manifest goes at the next open.
- **Restoring**: the prefetch thread (beside its `mix.budget.ask`) asks the lane for the runs whose scenes match, ahead of the playhead and of the loop region, at most one run in hand; the lane's job sends the
  units over a bounded lock-free ring (`rtrb`); the render thread drains it at the top of a pass into its store as first recordings (a burst already held is not overwritten). Invariant 2 holds: the store
  is touched by the render thread alone, and it never reads a file. Invariant 6: the callback is not involved. A disk that stands stands the lane: the sound is rendered live as today.
- **The program stage** (loudness-norms) is after the store: its gain is in neither the scene nor the salt, so a stored run plays through any stage.
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

## D10. What the tree has today (checked at orch `fc60a5d9`; names decide, not lines)

- `mix.rs` `Unit.data` is `Box<[f32]>` with `[lo, hi)`: it becomes `Arc<[f32]>`; `MixStore::write` fills a partial unit in place, so the snapshot takes whole units only (D5).
- `player.rs`: `RenderCtx` has no channel in; `audio_render_loop` drains the new ring at the top of a pass; `prefetch_loop` asks `mix.budget.ask(mix_want(region))`, where the lane is asked too.
- `render_cache.rs` `run_prerender`: the play/export check, then `display_exec`, the store, the build, the scan, `full || unavailable` → parks. The mix step goes before `display_exec`. `RenderCache` already has two lanes
  (`render-cache-restore`, `render-cache-close`) as the model of a disk call off a frame's path.
- `scene_hash` (`audio.rs`) walks the widened range's active clips: `sound_key(range)` beside it walks the same for the identities. A reel's inner sources count: the nest's clips recursively (its `content` is in `fp`, its files are not).
- `kt_time::trace::render_cache` holds the record kinds; `trace cache` and `trace summary` are in `kt-cli/src/trace_query.rs`.

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
