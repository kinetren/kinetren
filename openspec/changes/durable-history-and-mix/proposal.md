# Proposal: durable-history-and-mix

> DESIGN ONLY (spec session). Code starts after the owner answers the questions at the end and the
> orchestrator gives the go. Tests reserved: t3336–t3355. Base: `orch-26-11-01` at `f1f063c4`.

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

- **One durable blob store** for kept results (the video tier's folder rules: per program build and
  device, other builds' folders removed at open, a byte cap, least recently used out, the prerender
  never evicting what this process used), with two new tenants beside the composites: stream entries
  and mix runs.
- **Stream history pages to disk.** The stream memo gets a backing store: an entry the memo lacks is
  looked up on disk before a kernel walks for it; checkpoints and broken pictures are written there at
  rest. A kernel opts in by giving an exact codec of its entry types (Datamosh: its `Planes`). Entries
  are stored LOSSLESS: a history entry feeds every later frame, so one changed bit would change the
  rest of the event (invariant 11).
- **The kept mix pages to disk.** Kept bursts are written at rest as runs of raw f32 per context
  (linear play, each loop region's laps); after a restart the player's thread loads the runs ahead of the
  playhead and hands them to the render thread, which stays the store's only owner (invariant 2) and
  never touches the disk.
- **Keys add what a process could assume**: the files' identities (path, size, mtime), the program
  build with the FFmpeg version, and the kernel's entry-codec version; the device is the folder.
- **Measured first**: the trace already counts history positions walked; a run that restarts over a
  project states what a restart cost (positions walked, audio renders of covered bursts) before and
  after.

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
- No project format change. Disk: a new cap (default 1 GiB for both tenants together, beside the
  composites' 2 GiB; `KT_RENDER_CACHE_MB` covers all).

## Questions for the owner

**Q1. Build it now, or after your trace shows a restart costs something?**
Rows 770/771 say "no measured need yet": the slow Datamosh moments are already on disk after the
prerender, and the audio render keeps real time on release. What a restart costs today: every
Datamosh moment NOT on disk walks its history again (for an event with no refresh, from its start), and
every covered audio range is rendered live again (decode + every effect) until it is kept again.
- **a) Measure first**: stage 1 adds a restart line to the trace ("history: N positions walked after the
  start; audio: M covered bursts rendered again"); your next trace after a restart decides stages 2–3.
  *(Recommended: your rule "caches where they pay".)*
- **b) Build all three stages now.**

**Q2. The history must be stored without loss. Raw, or a fast processor compressor?**
Your words on the prerender: "слабо я верю что ffv1 аппаратно ускорен … а ты тут для превью это на
постоянку предлагаешь", decision "сделай ты сжатие видео нормальное" — a hardware codec, lossy accepted.
History cannot be lossy: a Datamosh checkpoint is the codec's reference picture; one changed value
changes every later frame of the event, in the preview and in the export. Hardware codecs are lossy (or
not lossless on most chips). Today: nothing is stored.
- **a) Raw planes** — no processor work to read; ~9 MB per checkpoint at 1080 lines, ~2 MB at 540; the cap
  holds fewer. *(Recommended for the history: a read must cost less than the walk it replaces, and raw
  costs only the disk.)*
- **b) LZ4** (a byte compressor, ~4 GB/s to unpack on one core) — 2–4× more history in the same cap, a few
  ms of a core per checkpoint read.
The mix is small (23 MB a minute raw); it is stored raw in both options.
