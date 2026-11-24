# Design: persistent-cache

> DESIGN ONLY, revision 4 (2026-11-23), base `e7b19346`: the owner's D1 — the final preview frame only, no result at a
> stage of a chain. Revision 3 (B1) added layer results at heavy cut points; revision 2 (`disk-copies`) kept only source
> copies; revision 1 (`proxies`) the same with a manual switch.

## Context

- The disk prerender (`render-cache-disk`, spec video-plan "Slow moments are prerendered at rest…"): whole composites
  under a durable key, segments of hardware-encoded H.264, written at rest by the player.
- The project's cache folder (`project-cache-folder`, spec-engine): `<project folder>/kinetren-cache/` with `media/`,
  `reels/`, `results/v<FORMAT>/<device class>/frames/`, `mix/v<FORMAT>/`, `media/<id>/copy-*.mkv`; the folder's cycle with a notice (B2).
- The owner, D1 (`kkt-26.11.22` lines 4–5): "делай кеш имменно финального превью а не на каком-то конкретном этапе
  цепочки fx".

## D1. One store, two kinds

`Store { root: <cache folder>, kinds: Frame | Copy }` in the layout `project-cache-folder` defines (layout B; that
change owns the text). One byte budget from the cache folder, one eviction order across frames, copies and the other
tenants of `results/` and `mix/`: value per byte = measured cost saved per read × reads in this project / bytes,
lowest first; least recently used as the tie-break; frames this process used in the last minute are kept.

## D2. Frames — the final picture only

- A frame is the composite the preview shows for a moment: every layer, track stage, reel instance and the
  composition, at the preview's current quality and decode step. Nothing between a source and that picture is
  written by this store.
- Its key is the whole plan's durable key (`render_cache::durable_hash`, `project-cache-folder`'s text): the plan's
  fields without content serials, the files' identities, the generators' values, the quality and decode step, plus
  the decoder's major, every kernel's version and WGSL hash and `COMPOSITE_PATH_VERSION`; not the build.
- **What an edit costs.** The key covers the whole plan of ONE moment, so an edit invalidates exactly the moments
  whose plan it changes: a title moved from 10–12 s to 11–13 s re-renders 10–13 s; a heavy Datamosh at 20–30 s
  stays held. An edit of the heavy event itself re-renders its range — the price the owner chose over a second store.
- **Adaptive.** Written at the quality the preview plays (A10, "как играем в превью так и храним"); choosing a higher
  quality makes the prerender compose the heavy moments again at that quality at rest; frames of a quality no longer
  played are not read and go by the value order. A frame is never shown at another quality than its own (invariant 4:
  quality is part of the key and of the order).

## D3. Source copies

Revision 2's copies, kept per media in the cache folder (`media/<content id>/copy-<w>x<h>.mov`): QuickTime, H.264
all-intra, the original's timestamps, the picture only, the original's natives reported; at the size the preview plays, for heavy
sources only (long-GOP above 1920×1080, or measured slower than real time); a better copy when a higher quality is
chosen; "Use a camera's copy…" kept. A copy is a decoded SOURCE, not a stage of a chain: it is what the chain reads, at
the size the preview would decode anyway.

## D4. The key — what survives an update

As `project-cache-folder` states it (decoder major, kernel versions and WGSL hashes, `COMPOSITE_PATH_VERSION`
guarded by t3808, the store format and device class in the path). An update that touches none of these keeps the
frames. A stale entry is a miss and is removed when met; there is no sweep.

## D5. Export and determinism

No export reads any kind (invariant 11); the draft render (`cached-final-render`, A16) reads frames only and its
window says so. A frame read back is the composite within the codec's tolerance.

## D6. Class walk — "a cache at a stage of a chain" (D1) and "do we store far too much" (D2)

The mechanism the owner names: a store that keeps an intermediate picture per chain or per stage multiplies the bytes
by the number of stages and serves no picture by itself. Walked across every store this change touches or neighbours
(the full audit of all caches, memory and disk, with his project's bytes, is spec-engine's D2):

| Store | Keeps a stage of a chain? | Here |
|---|---|---|
| `results/…/layers/` (revision 3, designed, never built) | yes — the picture after a heavy node | removed |
| `results/…/frames/` (the disk prerender, built) | no — the final composite | kept, the store's main kind |
| `media/<id>/copy-*.mkv` (designed) | no — a decoded source at the preview's size | kept |
| `results/…/history/` (was `durable-history-and-mix`) | a stream effect's reference state per node (Datamosh) | removed by the owner's D6 (`kkt-26.11.22` line 9, "Только финальные кадры"): no history on disk
| `mix/` (`durable-history-and-mix`) | no — the final mix | not this change |
| reel strips `reels/` | the reel's own final frames, small | not this change |
| memory: burst cache, kept composites, present memo, stream memo, kept mix | burst cache = decoded sources; stream memo = per node | spec-engine's D2 audit |

## D7. Idle

Writing follows the prerender's rule (at rest, after 1.5 s without input, background class); reading is one lookup
per moment; nothing runs at rest once the heavy moments are written. A quality change starts no work while playing.

## D8. What the tree has (engine side, spec-engine, checked at orch `d613b228`)

- **The frames' key already carries the quality.** `render_cache::put_plan` hashes `out_w`/`out_h`
  (`kt-engine/src/render_cache.rs:717–718`) and every layer's decode step; the prerender's state is
  `(built.seq, versions, q, region)` (`:1141`), so a quality change clears its `done` list and composes again at rest.
  D2's "read only at its own quality" holds by the key; nothing new is needed for it.
- **The gap: heaviness is judged by a time measured at another size.** The prerender picks moments from
  `PreviewCosts::snapshot` (`preview_cost.rs:66`, read at `render_cache.rs:1163`), which drops the size each sample was
  measured at (`Sample { ns, w, h }`, kept by `note`, `:50`). A moment measured at 640×360 is judged heavy or light
  at 1920×1080 by its 360 time, so a raised quality writes too few moments (and a lowered one too many). The rule: a
  sample counts only at its own size; a moment never measured at the current size is judged by its time at the nearest
  size scaled by pixels, and measured again when composed. t4106.
- **The value measure's inputs:** `Player::note_cost` (`player.rs:1457`) records a composed frame's cost;
  `PreviewCosts::note` keeps it per moment; the pass timer's `predict` (`kt-gpu/src/pass_time.rs:175`) prices a pass
  never measured in this process (from `pass-times-v1.tsv`). The cost saved per read is the moment's measured device
  time at its size; the reads are counted by `RenderCache`'s `restores` (`:969`) per entry in `usage.tsv`.
- **Where a frame enters the preview:** a restored moment goes into the display's memo (`RESTORE`,
  render-cache-disk D3) before the producer's "decoded?" check. The prefetch window (`prefetch.rs:126`) skips only
  moments the present memo holds (`kept_moments`, `player.rs:3569`), so a moment on disk but not yet restored is still
  decoded ahead: the cache audit's cut 1 (`docs/cache-audit.md`), given to mosh-sharp; this change must not undo it.

## Found while building (section 3)

- **QuickTime, not Matroska.** Matroska keeps milliseconds: at 23.976 fps every third picture lies a third of a
  millisecond from its own time, and where a project's grid meets that time the picture before it is chosen. A MOV track
  takes the original's time base.
- **No sound in a copy.** Copying packets needs a container that takes every codec the original may hold, and gives a
  second sound whose priming and length must be proved equal. The sound decodes fast everywhere; `CopySource` reads it
  from the original's own decoder, so "byte-equal" holds by construction.
- **The swap keeps the sound.** `replace_media` drops everything of a source; a swap of the picture must not reset a
  kept mix (a slowed reverb's is expensive). `BurstCache::adopt_picture` drops the pictures only.
- **A host with no hardware encoder makes no copy** (the owner's rule, row 775); the reason is shown.
- **The copies' decoder** is opened by `FfmpegCopies::copy` alone, so `device-decode-sources` takes over one call.

## Rejected

- **Layer results at heavy cut points (revision 3)** — rejected by the owner (D1): a second store of pictures that are
  never shown, bytes per stage, a key per prefix and a second read path into the plan.
- **Three stores** (prerender, copies, an effect cache) — three budgets that do not see each other.
- **Build-salted entries** — every update would empty a "persistent" cache.
- **Showing a frame of a higher quality at a lower one** — a different picture than composing at that quality; the
  coverage bars would claim what the plan does not make.
