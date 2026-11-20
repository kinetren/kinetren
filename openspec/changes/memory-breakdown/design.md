# Design: memory-breakdown

> DESIGN ONLY, revision 1 (2026-11-20), base `df6d0858`. Q1–Q2 in `proposal.md`; defaults recommended.

## Context

- `kt_time::mem` (memory-ledger): tenants `Kept, History, Frames, Mix, Upload, Readback, Transient, Compute`, accounts
  opened by caches, a growth budget for `GROWING`; `snapshot().used`; `resident()` (the probe from kt-gpu).
- kt-app: the status bar's "memory: N MB" and the Diagnostics rows read `PerfState::resident()` (a second-old reading);
  the Diagnostics line `memory_ledger` prints the cache tenants.
- The sidecar `ktindex.db` (kt-media), the project's cache folder (change `project-cache-folder`, the owner's A14).

## D1. Report-only accounts

`Tenant` gains kinds that are counted but never granted: `Model`, `Undo`, `Peaks`, `Thumbs`, `ReelPreview`,
`UiTextures`, `Decoders`, `AudioState`, `Fonts`. `Ledger::report(kind, bytes)` stores the latest value (an atomic);
`GROWING` and the budget are untouched, so nothing about sizing changes. Owners report:
- the model: `Project::approx_bytes()` (events × the struct size + curves' keys + strings' capacities), on each
  revision by `TimelineModel`; the undo stack sums its snapshots' estimates (shared-events will make them smaller —
  the number shows it);
- kt-app: peaks and thumbnails caches (their byte counters), reel previews, egui textures (the texture manager's
  sizes), fonts (atlas bytes);
- kt-media: each open decoder's frame pool (width × height × planes × pool size), summed by the media worker;
- kt-engine: the audio render's FX buffers and the ring (known at build).

## D2. The arithmetic

`not counted = resident − Σ accounts` (never negative; shown as 0 when the accounts exceed a stale resident reading).
The bar's sections are the accounts with ≥ 1 MB, sorted by size, then "not counted". The video-memory bar is kt-gpu's
pools and live textures by device. Both readings are taken together once a second while the section is open, from
the same snapshot.

## D3. The disk bar (Q2 a)

`<project>.ktindex.db` (the file + its WAL), the cache folder's subfolders, the recovery file — sizes from a
background `media`-lane job when the section opens and every 5 s while it stays open (a directory walk; the cache
folder's own size index when `project-cache-folder` keeps one). Each segment's hover says what clearing it costs
("peaks and thumbnails are rebuilt when needed").

## D4. The status counter (Q1 a)

Unchanged number; its hover (the one hint place) lists "3 200 MB: frames 1 100 · kept 700 · model 120 … video
memory 1 100 MB".

## D5. Class walk — "a number that does not say what it counts"

| Number | Says what it counts? | Here |
|---|---|---|
| status memory | resident (since frames-cache-growth) | hover adds the split |
| Diagnostics cache line | tenants, text | the bar + list |
| cache occupancy bar | decoded frames vs bound | unchanged, labelled |
| "Callback/renderer underruns" | labelled since R.43 | — |
| render window sizes | file size and bitrate | — |

## D6. Idle

Owners update their numbers when their contents change; the section samples once a second only while open; the disk
walk runs only while open.

## Rejected

- **Parsing `/proc/self/smaps` into sections** — the owner: "не про тупое чтение procinfo"; the mapping of anonymous
  memory to our sections is known only to the owners.
- **Counting by serializing the model** — a whole-project walk per revision; an estimate from sizes is enough for a bar.
