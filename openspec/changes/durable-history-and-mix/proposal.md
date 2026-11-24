# Proposal: durable-history-and-mix

> DESIGN (spec session). The owner decided Q1 b and Q2 b (kkt-26.11.20 A14, line 19): build now, the history
> with LZ4, both tiers in the project's cache folder (`project-cache-folder`). Tests reserved: t3336–t3355.
> Revised on `orch-26-11-01` at `5e7a8ea9`.

## Why

render-cache-disk gave the video composites of slow moments a disk tier, so a heavy loop plays whole from
lap 0 after a restart. Its class walk left two other "kept results" that die with the process:

- **found-defects 770** — the Datamosh stream memo (codec state, checkpoints, broken pictures) lives in
  one process. A moment the disk tier holds needs no history; any other moment of a stream stage (a
  light moment, one past the tier's cap, one whose downstream effects were edited) walks its history
  from the anchor again: for Datamosh with no refresh that is from the event's start.
- **found-defects 771** — the audio's kept final mix (`MixStore`, cached-range-plays, the owner's "вариант
  А" of invariant 2) is the render thread's memory. After a restart every covered range is rendered
  live again: decode and every effect, each lap until it is kept again.

Both rows say "no measured need yet". The owner's rule is "caches are allowed where they pay" (2026-11-01):
keyed by what determines the result, bounded, invisible to determinism. So this change designs both
tiers the same way the video tier works, and puts a measurement first.

## What Changes

- **Two tenants of the project's cache folder** (`project-cache-folder`): the stream entries as the
  `history/` kind of the persistent cache (`results/v<FORMAT>/<device class>/history/`, beside its `frames/`), and `mix/` for mix runs, written in the folder's cycle (a stale salt dropped on a miss, least recently used out, the prerender never
  evicting what this process used).
- **Stream history pages to disk.** The stream memo gets a backing store: an entry the memo lacks is
  looked up on disk before a kernel walks for it; checkpoints and broken pictures are written there at
  rest. A kernel opts in by giving an exact codec of its entry types (Datamosh: its `Planes`). Entries
  are stored LOSSLESS and compressed with LZ4 (the owner's Q2 b): a history entry feeds every later
  frame, so one changed bit would change the rest of the event (invariant 11).
- **The kept mix pages to disk.** Kept bursts are written at rest as runs of raw f32 per context
  (linear play, each loop region's laps); after a restart the player's thread loads the runs ahead of the
  playhead and hands them to the render thread, which stays the store's only owner (invariant 2) and
  never touches the disk.
- **Keys add what a process could assume**: the files' identities (path, size, mtime), the program
  build with the FFmpeg version, and the kernel's entry-codec version; the device is the folder.
- **Measured as it is built** (the owner chose to build now): the trace states what a restart cost
  (positions walked, audio renders of covered bursts), what the tiers restored, and the history's
  compression ratio.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `video-fx`: a stream stage's memo has a durable tier; the kernel's entry codec.
- `audio-render`: the kept final mix has a durable tier, loaded off the render thread.
- `telemetry`: restores and writes of both tiers in the trace.

## Impact

- `src/libs/kt-plugin-api/src/stream.rs` (`StreamMemo` backing, `EntryCodec` on `StreamKernel`),
  `src/plugins/fx-video-codec` (Datamosh's codec of `Planes`), `src/libs/kt-engine/src/render_cache.rs`
  (the store generalised: `BlobStore` under `DiskStore`), `video_stream.rs` (the durable salt of a stage),
  `mix.rs` (`MixStore` restore handoff, `MixDisk`), `player.rs` (writes at rest by the prerender's quiet
  rule, loads ahead of the playhead), kt-cli `trace cache`.
- Invariants: 2 (the render thread still owns the mix alone; the disk is the player's thread's),
  4 (the bars: a burst/moment on disk counts as covered only when restorable), 11 (lossless, exact keys:
  the same bytes with and without the tier — so the export may read the stream tier, never the mix).
- No project format change. Disk: the cache folder's cycle (`project-cache-folder` D3, the owner's B2) covers both tenants;
  `KT_RENDER_CACHE=off` turns all three off. A new dependency: `lz4_flex` (pure Rust, its safe decoder) in
  kt-engine.

## The owner's decisions (kkt-26.11.20, line 19)

- **Q1 b — build all stages now.** The restart line in the trace (D1) stays as telemetry, no longer a gate.
- **Q2 b — the history is stored with LZ4.** His words: "я не очень понимаю почему история datamosh без
  сжатия если сам по себе datamosh это и есть сжатие? нет? ну если нет давай хотя бы LZ4". Design D9
  answers the question in plain words.
- **Where:** "у нас не будет все в одной базе sqlite хранится а будет подпапка в папке проекта с разными
  кешами". The two tiers are tenants of the project's cache folder (`history/`, `mix/`; change
  `project-cache-folder`), in its cycle, not a store of their own in the user's data folder.

## His question, answered (to relay)

"Why is the Datamosh history not compressed, if Datamosh itself is compression?"

Datamosh IS a codec: it encodes the picture and decodes it with the keyframes broken. But what the history
keeps is not the compressed stream, it is the codec's MEMORY at a frame — the last picture the decoder
painted, which the next frame's motion is applied to. The compressed packets of a moshed event are small,
but each one only says "move these blocks of the PREVIOUS picture and add this correction". With no
keyframe, frame 500 exists only by applying all 499 packets before it — and replaying them is exactly the
walk a checkpoint is there to skip. So a checkpoint has to be a whole picture.

It has to be EXACT because every later frame of the event is painted on top of it. A lossy store (JPEG,
H.264 — like the preview's prerender) would change a few values, the motion would carry that error into
every later frame, and after a restart the event would look different from an uninterrupted play and from
the export. So the store may only shrink the bytes in a way that gives back the very same bytes: a lossless
compressor. LZ4 is one: it unpacks a 1080p checkpoint in a few milliseconds of one core, far less than the
walk it replaces. It shrinks camera pictures about 1.3–2×, and the smeared, flat areas Datamosh makes more;
the trace measures the real ratio on his projects (D9).
