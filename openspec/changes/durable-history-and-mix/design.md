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
audio bursts rendered again". The owner chose to build all stages now (Q1 b, kkt-26.11.20), so the line is
telemetry, not a gate: it shows what the tiers saved on his next restart.

## D2. Two tenants of the project's cache folder

The owner moved every cache of a project into a folder beside it (kkt-26.11.20 line 19, change
`project-cache-folder`). The history is the `history/` kind of its persistent cache
(`results/v<FORMAT>/<device class>/history/`, beside `frames/`, kkt-26.11.20b B1; `layers/` withdrawn by the owner's D1), and the mix
runs are `mix/v<FORMAT>/`. They share its rules:
- the entry header and the atomic write (`project-cache-folder` D1);
- an entry of another salt is a miss, removed when met (no sweep at open);
- the folder's cycle (`project-cache-folder` D3, the owner's B2): a write that does not fit drops the
  history and the mix before the picture results and the media index, least recently used first, never
  what this process used in the last minute; a notice once per session.

`render_cache::DiskStore`'s folder rules become the cache folder's, and the three stores are its users.
Stream entries are one file per stage run (D3); mix runs are one file per run (D5).
`KT_RENDER_CACHE=off` turns all three off.

## D3. The stream memo's backing store

- **The kernel's entry codec** (plugin contract): `StreamKernel::entries(&self) -> Option<&'static dyn
  EntryCodec>` with `version() -> u32`, `encode(&dyn Any) -> Option<Vec<u8>>` (exact bytes of the entry;
  `None` — this entry type is not stored) and `decode(&[u8]) -> Option<Arc<dyn Any + Send + Sync>>`. A kernel
  without one is never stored (today's behaviour). Datamosh encodes `Planes` (its picture planes and the
  clean encoder's reconstruction) as the exact raw planes; the HOST compresses every stored entry with LZ4
  (the owner's Q2 b; D9), so the kernel stays one exact encoding and any later kernel's entries are
  compressed alike.
- **The durable salt of a stage**: `xxh3` of the persistent cache's salt (`project-cache-folder` D1b: the
  store's format and the device class as the folder, the decoder's major, the versions and WGSL hashes of
  the kernels that ran and `COMPOSITE_PATH_VERSION`, not the whole build), the kernel id and its codec version, and the identities of every file the stage reads (render-cache-disk's
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

## D8. Stages (all now, the owner's Q1 b)

1. D1: the restart line and the compression ratio in the trace. t3336, t3337.
2. D2 + D3: the tenants in the cache folder, the entry codec, Datamosh's codec, LZ4 in the host, the
   memo's backing, writes at rest. t3338–t3345.
3. D5 + D6: the mix runs, the handoff, the bars. t3346–t3352.
Spare: t3353–t3355.

## D9. Why the history is stored without loss, and why LZ4 (the owner's question)

The owner asked: "почему история datamosh без сжатия если сам по себе datamosh это и есть сжатие?"

- **What a checkpoint holds.** Datamosh runs a real encoder and decoder (`fx-video-codec` `walk.rs`). Two
  pictures make the walk's state at a frame:
  - the decoder's reference picture: the last frame it painted, onto which the next frame's motion
    vectors and residual are applied;
  - the clean encoder's reconstruction: what the encoder believes the decoder holds.

  These are decoded pictures (`Planes`: Y, Cb, Cr), the codec's MEMORY, not its compressed output.
- **Why not store the compressed packets.** They are small, but a P-frame packet says only "take blocks
  of the previous picture, move them, add this correction". Datamosh removes the keyframes, so the
  picture at frame k exists only by applying all k packets from the event's start. Replaying the packets
  IS the walk. A checkpoint exists to start the walk from frame k in one step, so it has to be the picture
  itself.
- **Why it must be exact.** Every later frame of the event is painted on the checkpoint. A lossy form
  (JPEG, or the hardware H.264 the preview's prerender uses) changes some values. The motion then carries
  that change into every later frame, the event after a restart differs from an uninterrupted play, and
  the export differs from the preview. Invariant 11 promises the same bytes with and without a cache. So
  the store may only shrink the bytes reversibly.
- **Why LZ4.** It is lossless and unpacks at several GB/s on one core: a 1080p checkpoint (two pictures,
  ~6 MB) comes back in a few milliseconds, far below the walk of even a few frames. Its ratio on camera
  pictures is modest, about 1.3–2×; the flat and smeared areas a mosh makes compress better.
- **Rejected:**
  - a stronger compressor (zstd at high levels) unpacks several times slower, for maybe 30 % more;
  - raw (the former recommendation) holds fewer checkpoints in the same limit.
- **The ratio is measured:** each write records its raw and stored bytes; `trace cache` prints the
  tenant's ratio. If the owner's projects show LZ4 below 1.2×, the trace says so, and a later change can
  weigh zstd's fast levels.

## D10. What the tree has today, and the corrections it forces (checked at orch `873934f7`)

- **The memo's API** (`kt-plugin-api/src/stream.rs`). `StreamMemo` (`:330`) has no `insert`. Writes go
  through `put` (`:420`), `put_transient` (`:428`) and `put_pinned` (`:436`), all into `put_entry` (`:440`);
  reads through `get<T>` (`:403`), `contains` (`:415`) and `holds` (`:596`, both tiers). The disk tier hooks
  in at `put_entry` (marks a pinned or broken entry unwritten) and at `holds`/`get` (a memory miss asks the
  disk index). `mosh::needs` (`mosh.rs:550`) already starts from `memo.holds(...)`, so no kernel change is
  needed for a walk to resume from disk.
- **The codec deals in bytes.** kt-plugin-api may depend only on kt-time, serde and libm, so `EntryCodec`
  encodes to and decodes from `Vec<u8>`. LZ4 (`lz4_flex`, a new workspace and kt-engine dependency; none is
  present today) lives in kt-engine's writer and reader.
- **What a checkpoint is.**
  - Host checkpoints are `Planes` (`fx-video-codec/src/codec.rs:18`).
  - Transients are `Clean` (recon `Planes` + a `Packet`, `codec.rs:46`).
  - With `hold_ahead` set (`walk.rs:317`), a pinned checkpoint can be a device buffer `HeldPic`
    (`walk.rs:53`, written at `:882`). `compute::Held` is opaque, with no readback API. So writing a
    device-held checkpoint at rest needs a host copy job on the compute host (a `run` that reads the
    buffer back into `Planes`), at class `background`, only at rest. Transients (`Clean`) are not stored.
- **The salt must reach the memo.** The preview's memo is one process-wide static (`stream_memo()`,
  `video_stream.rs:58`), shared by every stage. Each export makes its own (`export.rs:287`,
  `export_pipeline.rs:718`). `get(key)` knows neither the stage's salt nor its codec, so `run` (`:884`) and
  `key_only` (`:863`) set a thread-local scope, as `with_leaves` does. The scope holds the stage's salt (D3:
  the store's salt, the plugin's id, version and WGSL hash, the codec's version, and the files of
  `FileIds.all` through `RenderCache::files`, `render_cache.rs:907`) and its `EntryCodec`. The backing reads
  them from there.
- **The mix's samples are a `Box`.** `Unit.data` (`mix.rs:56`) is `Box<[f32]>`. D5's no-copy snapshot needs it
  to become `Arc<[f32]>` (one allocation per kept burst, as today).
- **There is no channel into the render thread today.** `RenderCtx` (`player.rs:2081`) has atomics, mutexes
  and the outgoing `rtrb::Producer`. The restore handoff is a second `rtrb` ring (bounded, lock-free; `rtrb`
  is already a dependency), drained at the top of a pass in `audio_render_loop` (`player.rs:2442`). The
  loader is `prefetch_loop` (`:3400`), beside its `mix.budget.ask` (`:3522`). `MixCoverage` (`mix.rs:499`)
  carries the snapshot of unwritten units out.
- **The scene hash** is `audio.rs:1012` (`scene_base` + `clips[i].fp` over the widened range's active clips).
  That widened range is where the mix salt's audio file identities are collected.
- **The trace kinds** for history and mix records go into `kt_time::trace::render_cache` (`trace.rs:631`).
  The restart summary has no recorder yet; it is new (D1), as is `kt-cli trace summary`'s line
  (`trace_query.rs:466`). `trace cache` (`:130`) is extended.
- **The store** (folder, header, `ask_room`, the key without the build) is `project-cache-folder`'s sections
  1 and 3. This change starts after them.

## Risks

- Disk read latency on a spinning disk during a walk: the read replaces a walk of many device frames; a
  read slower than the walk it replaces is visible in the trace (restore time vs walk time per entry) and
  the tier can be turned off.
- A kernel whose entry codec is not exact: t3339 round-trips every entry type the kernel stores and
  compares a walk's frames with and without the tier.
