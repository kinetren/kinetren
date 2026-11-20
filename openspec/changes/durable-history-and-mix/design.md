# Design: durable-history-and-mix

## Context

- render-cache-disk (video-plan design, "Slow moments are prerendered to disk at rest"): `DiskStore` under
  `<data>/render-cache/<build>/<device>/`, a 2 GiB cap (`KT_RENDER_CACHE_MB`), least recently used out,
  other builds' folders removed at open; the prerender thread of the player writes at rest (1.5 s with no
  input, never while playing or under an export's vote); D1 — a durable key without per-process serials,
  `Leaves` naming each leaf's file identity, "every file for a stream stage"; D5 — the export never reads it
  (lossy since render-cache-codec).
- The stream memo (`kt_plugin_api::stream::StreamMemo`): a bounded LRU of OPAQUE entries
  (`Arc<dyn Any + Send + Sync>`) keyed by a `u64` the kernel derives "from what determines the entry";
  a pinned tier of walk checkpoints (stream-history-on-device D5), a live tier on the compute device
  (stream-round-trips D1), loop-aware eviction (stream-memo-budget). "Speed only — never another byte."
- Datamosh's keys (`fx-video-codec` `mosh.rs`, `walk.rs`): built from the host's `StreamLayout::ident(side,
  k)` (`video_stream.rs`): the media id, the pts, the quality, `src_rev`, the front slots' key, the second
  input's chain — no file identity (within one process a media id names one file content) and no build.
- The kept mix (`mix.rs`, cached-range-plays, kept-mix-settle, loop-keeps-its-lap): units of one burst
  keyed by `(MixCtx, burst index)` holding `[lo, hi)`, the `scene_hash` and the samples; owned by the
  audio render thread alone, no lock (invariant 2); the scene hash = `scene_base` + the fingerprints of
  the events heard (model text via `hash_debug`, source address, gain, the nest's content) — again no file
  identity and no build. Exports never read it.

## Goals / Non-Goals

Goals: after a restart, history and mix already computed by an earlier run on this machine come back from
disk; nothing restored can be stale; no byte changes; the render thread never waits on a disk; the first
stage only measures.

Non-goals: carrying either across program builds or devices; the live (device) tier of the memo; a lossy
form of anything.

## D1. Measure what a restart costs (stage 1)

The trace gains a `Restart` summary over the first minute after a project opens: history positions walked
(`kt_plugin_api::stream::walked` already counts them per walk) and covered audio bursts rendered live
again (bursts the mix store would have held had it been kept: approximated by bursts whose scene equals a
scene the PREVIOUS run's trace wrote as kept — the trace carries the kept scenes' count, not the bytes).
`kt-cli trace summary` prints "after the start: N history positions walked (M ms of device), K covered
audio bursts rendered again". Owner Q1 decides from his trace whether stages 2–3 follow.

## D2. One blob store, three tenants

`render_cache::DiskStore` splits into a generic `BlobStore` (folder rules, cap, LRU, the "used in this
process" mark, manifests) and its users: the composites (as today, segments by the hardware codec), stream
entries (one file per entry or packed per stage, D3) and mix runs (D5). One cap for all three (the
owner's disk is short), the composites first in value: eviction takes stream entries and mix runs before
composites of the same age. `KT_RENDER_CACHE=off` turns all off.

## D3. The stream memo's backing store

- **The kernel's entry codec** (plugin contract): `StreamKernel::entries(&self) -> Option<&'static dyn
  EntryCodec>` with `version() -> u32`, `encode(&dyn Any) -> Option<Vec<u8>>` (exact bytes of the entry;
  `None` — this entry type is not stored) and `decode(&[u8]) -> Option<Arc<dyn Any + Send + Sync>>`. A kernel
  without one is never stored (today's behaviour). Datamosh encodes `Planes` (its picture planes and the
  clean encoder's reconstruction) as raw planes or LZ4 per Q2.
- **The durable salt of a stage**: `xxh3` of the program build salt (with the FFmpeg version), the kernel id
  and its codec version, and the identities of every file the stage reads (render-cache-disk's
  `Leaves` "every file for a stream stage", path + size + mtime ns). Disk key = `mix(entry key, salt)`. The
  entry key already carries positions, quality, parameters and the front slots (it is what the memo
  trusts in-process); the salt adds what a process could assume.
- **Reading**: `StreamMemo::holds(k)` and `get(k)` consult the backing store's index (loaded at open: the
  keys on disk for this folder) after the memory map; a `get` that hits disk decodes the entry, puts it in
  the memory LRU (pinned if it was pinned) and returns it. Kernels do not change: `needs` already asks
  `holds` for checkpoints and broken pictures, so a walk starts from the latest entry on disk. The read
  runs on the thread that asked (the stream producer / compose thread), as the video tier's restore does.
- **Writing at rest**: the prerender's quiet rule (1.5 s with no input, not playing, no export vote): the
  pinned checkpoints and the broken pictures the memo holds, oldest-unwritten first, entries larger than a
  tenth of the cap skipped. A device-resident (live) checkpoint is read back for writing only at rest.
- **The export may read it**: entries are lossless and keyed by everything that determines them, so a
  restored entry equals the one a walk would compute on this device (invariant 11: "the same bytes with and
  without it"). t3343 proves an export with and without the tier is byte-equal.

## D4. Why not simply prerender every stream moment

The composite tier already removes the history for moments it holds. It does not help when an effect
AFTER the stream stage is edited (every composite of the event re-keys, the history stays valid), past the
cap, or for the light moments the prerender skips by design. The history tier keys the stage's own output,
upstream of those edits.

## D5. The kept mix on disk

- **Key**: `(MixCtx, burst index, durable scene)` where durable scene = `mix(scene_hash, salt)`, salt = the
  build salt (FFmpeg version included: the decoders' float output may move between majors) and the
  identities of the audio files heard in the burst's widened range.
- **Form**: runs of consecutive bursts of one context, raw interleaved f32, a manifest per run (the
  context, the first burst, each burst's `[lo, hi)` and durable scene). Raw: 23 MB a minute, and a read is a
  copy.
- **Writing**: at rest, from what the store holds. The render thread owns the store: it publishes, through
  the existing `MixCoverage` path, an immutable snapshot of the units not yet written (an `Arc` of the
  boxed samples it already keeps; no copy on the render thread); the player's thread writes them.
- **Restoring**: the player's prefetch thread, which already sizes the store's budget (`MixBudget::ask`),
  reads the runs whose scenes match the current renderer ahead of the playhead and of the loop region, and
  sends them over a bounded lock-free channel; the render thread drains the channel at the start of a
  chunk into its store as "first recordings" (a frame already held is not overwritten). Invariant 2 holds:
  the store is touched by the render thread alone, and it never reads a file. Invariant 6: the callback is
  not involved.
- **Settle**: a restored burst behaves as a kept one: leaving it, the effects are brought back to the
  continuous play (kept-mix-settle) — nothing new.
- **Exports never read it** (as in-process): the audio export stays a live render, byte for byte
  (invariant 11, audio always).

## D6. Coverage bars (invariant 4)

Video: the bars already add moments the composites' store holds (a scan at rest). Stream entries are not
a coverage of their own (a moment is covered only when its composite is kept or on disk). Audio: the
bars' kept-mix band adds the bursts of restorable runs for the current scene, by the same scan at rest; a
burst shown bright is handed out with no decode and no effect run once loaded.

## D7. Class walk: every per-process result that a restart recomputes

| Result | Survives a restart after this change | Note |
|---|---|---|
| Kept composites of slow moments | yes (render-cache-disk) | unchanged |
| Kept composites of light moments | no | cheap to compose by definition |
| Stream memo: checkpoints, broken pictures | yes (D3) | lossless |
| Stream memo: live device tier | no | rebuilt from the host entries |
| Front memo (slots in front of a stream stage) | no | its outputs feed the stream; the stream's entries make it unnecessary for stored positions; listed as a follow-up if the trace shows it |
| Kept final mix | yes (D5) | preview only |
| FX state of audio effects | no | state, not a result; settle covers it |
| Decoded frames (burst cache) | no | the sources are on disk already |
| Peak pyramid, thumbnails, reel strips, packet index | yes (sidecar) | unchanged |
| Pass-time measurements, compiled run shapes | yes (pass-time disk cache) / per process | unchanged |
| Loudness/sync analysis | yes (sidecar) | unchanged |

## D8. Stages

1. D1: the restart line in the trace. t3336, t3337.
2. D2 + D3: the blob store, the entry codec, Datamosh's codec, the memo's backing, writes at rest. t3338–t3345.
3. D5 + D6: the mix runs, the handoff, the bars. t3346–t3352.
Spare: t3353–t3355.

## Risks

- Disk read latency on a spinning disk during a walk: the read replaces a walk of many device frames; a
  read slower than the walk it replaces is visible in the trace (restore time vs walk time per entry) and
  the tier can be turned off.
- A kernel whose entry codec is not exact: t3339 round-trips every entry type the kernel stores and
  compares a walk's frames with and without the tier.
