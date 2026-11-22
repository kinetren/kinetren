# Design: persistent-cache

> DESIGN ONLY, revision 3 (2026-11-22), base `b0b6a733`: the owner's B1 — heavy effects' results first, one store.
> Revision 2 (`disk-copies`) kept only source copies; revision 1 (`proxies`) the same with a manual switch.

## Context

- The disk prerender (`render-cache-disk`, spec video-plan "Slow moments are prerendered at rest…"): whole composites
  under a durable key (the plan key with content serials replaced by file identities, salted by format, build and
  device), segments of hardware-encoded H.264, 2 GiB, LRU, written at rest by the player.
- The project's cache folder (`project-cache-folder`, spec-engine): `<project folder>/kinetren-cache/` with `media/`,
  `reels/`, the prerender, the history, `mix/`; one folder per project folder (B2 a); a folder that cannot take more writes
  cyclically, dropping the oldest, with a notice (B2).
- The plan (`frame_plan`, `plan_chain`): per layer, the source frame → the chain's nodes (fused point-wise runs,
  barriers, stream stages) → the layer's picture; track stages; reel instances as sub-plans.
- Per-pass device times in the journal (gpu-pass-cost); the render-ahead producer (render-ahead) composes ahead while
  playing.

## D1. One store, three kinds

`Store { root: <cache folder>, kinds: Layer | Frame | History | Copy }` in the layout that `project-cache-folder` defines (the orchestrator's final decision, 2026-11-22: layout B — the store
format and the device class in the path; that change owns the text, this one does not restate it); one byte budget
from the cache folder, one eviction order across the results, the mix and the copies: value per byte = measured cost saved per read ×
reads in this project / bytes, lowest first; within equal value, least recently used. The prerender's rule "never evict
what this process used" stays for frames.

## D2. Layer results — the cut points

- A cut point is a position in a layer's plan after which the plan can start from a texture: after a barrier node or a
  stream stage of an event's chain, after a track stage, the output of a reel instance's sub-plan.
- Heavy: the node (or stage, or sub-plan) whose measured device time at this size is ≥ half a frame period, over the
  last three measurements. Light nodes are never cut points (their result is cheaper to compute than to decode).
- The key `stage_key(layer, cut)` = the durable inputs of the layer UP TO the cut: the source's identity (path, size,
  mtime, or a generator's values), the material position, the decode size, the content hash of the chain prefix and
  every curve's value at this moment for the prefix's nodes, masks of those nodes, for a reel its instance content
  hash; plus the rest of D4's key. Nothing after the cut, nothing of other layers, not the timeline position enters it.
- Writing: the producer (at rest, and while playing when the device has spare time) reads back the cut's texture once
  and writes it; a stage segment is a run of consecutive moments of one cut.
- Reading: `frame_plan` asks the store for each layer's deepest held cut at this moment; found, the layer's plan begins
  with the decoded texture as the cut node's output and runs the rest. The coverage bars draw a moment bright only when
  its whole frame or every heavy layer's cut is held (invariant 4: what is bright plays without the heavy work).

## D3. Frames and copies

- Frames: the disk prerender as built, moved into the cache folder's frames (`project-cache-folder`).
- Copies: revision 2's source copies, kept per media in the cache folder (`project-cache-folder`) (MKV, H.264 all-intra, the original's timestamps, audio stream-copied, the
  original's natives reported) at the size the preview plays, for heavy sources only (long-GOP above 1920×1080, or
  measured slower than real time); a better copy when a higher quality is chosen; "Use a camera's copy…" kept.

## D4. The key — what survives an update

The key of a frame or a layer result is `render_cache::durable_hash` (the plan's fields without content serials, the
file identities' digest; for a layer, the layer's sub-plan up to the kept stage) plus the decoder's major version, each
kernel's plugin version and WGSL hash, and `COMPOSITE_PATH_VERSION` — a constant bumped when the composite code outside
the kernels changes (fusion, `plan_chain`, core passes and shaders, conversions), guarded by t3808 in
`project-cache-folder`'s video-plan delta (it records the hash of those files and fails on a change without a bump). The
build is not in the key: an update that touches none of these keeps the entries. The store format and the device class
are in the path (`project-cache-folder`'s layout). A stale entry is a miss and is removed when met; there is no sweep.

## D5. Alpha and quality

A layer result may carry alpha: it is written as two hardware segments — the colour (H.264 4:2:0) and the alpha as a
luma-only segment — and recombined on upload. Quality by Q1 (a): the encoder's highest quality preset, the same as the
prerender; the restored stage is within the codec's tolerance of the composite.

## D6. Export and determinism

No export reads any kind (invariant 11); the draft render (`cached-final-render`, A16) reads frames only and its window
says so. A preview built from cached stages differs from a fresh compose by the codec's noise at most; a test bounds it.

## D7. Class walk — "work that an unrelated edit throws away"

| Work | Kept today across an edit of another layer? | Across a restart? | Here |
|---|---|---|---|
| heavy effect on one event | no (the whole composite misses) | no | layer result |
| Datamosh history | the stream memo in memory | `durable-history-and-mix` (history/) | the same store's budget |
| track stage (a heavy track effect) | no | no | layer result |
| reel instance output | the reel preview cache (strips), not frames | no | layer result |
| decoded heavy source | the burst cache (memory) | no | copy |
| whole slow frame | present memo | prerender | frame (unchanged) |
| final mix | kept mix (memory) | `mix/` (designed) | unchanged |

## D8. Coordination with `project-cache-folder` (final, 2026-11-22)

The orchestrator decided the layout once for both changes (layout B: the store format and the device class in the path,
so a cache written on another device class is never served here and a format bump leaves plain stale folders that the
eviction and the cyclic rule see). `project-cache-folder` (spec-engine) owns the layout text, the folder, its tenants,
the limit and the cycle; this change cites it. Agreed and unchanged: the key of D4 (`durable_hash` + decoder major +
kernel versions and WGSL hashes + `COMPOSITE_PATH_VERSION` guarded by t3808, no build); one eviction order by value per
byte (cost saved per read × reads in this project ÷ bytes), lowest first, LRU as the tie-break, frames used in the last
minute kept; the owner's cyclic rule (B2) with a notice. This change owns what is heavy and how a kept layer result
enters the plan.

## D9. Idle

Writing follows the prerender's rule (at rest, after 1.5 s without input, background class; while playing only in
the producer's spare time); reading is a lookup per layer per moment; nothing runs at rest once the heavy moments are
written.

## Rejected

- **Three stores** (prerender, copies, a new effect cache) — three budgets that do not see each other; the owner wants
  the effects cached, and the frame store already keys by content.
- **Caching every node's output** — a light node costs less to compute than to decode.
- **Build-salted entries** — every update would empty a "persistent" cache.
