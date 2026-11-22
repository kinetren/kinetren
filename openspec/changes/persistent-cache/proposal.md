# Proposal: persistent-cache

> DESIGN ONLY (spec session), revision 3 (2026-11-22) after the owner's second answers (`kkt-26.11.20b` B1). Was
> `proxies` (revision 1), then `disk-copies` (revision 2, A10). Tests reserved: t3440–t3459.

## Why

The owner's items, whole:

- `kkt-26.11.01` candidate ★18 (registry P1): "4K HEVC 10 бит с телефона тормозит на скрабе. Хочу «сделать прокси» в
  фоне, переключатель прокси/оригинал одним кликом, индикатор, а рендер — всегда с оригиналов."
- `kkt-26.11.20.txt` lines 12–15 (A10): "адаптивный? как играем в превью так и храним? и если выбрали лучшее качество
  то рендерим лучшее чем есть. ну и не мешает сделать рядом переключатель где будет "как в превью" или чет такое. не
  называй это прокси никогда. неудачное название. пусть будет постоянный кэш или что-то такое более компактное. просто
  кэш тоже нельзя - не понятно что он не в озу."
- `kkt-26.11.20b.txt` line 6 (B1, the name): "«Постоянный кэш»".
- `kkt-26.11.20b.txt` line 7 (B1, which files): "тяжелые. и мне не нравится почему ты думаешь только о проигрывании и
  декоде - А КАК ЖЕ ЭФФЕКТЫ??? У НАС СУТЬ ПО БОЛЬШЕЙ ЧАСТИ В КЭШИРОВАНИИ ЧТОБЫ ЭФФЕКТЫ ТЯЖЕЛЫЕ НЕ СЧИТАТЬ!"
- `kkt-26.11.20b.txt` lines 8–9 (B2, the folder): one cache folder per project folder; "непринимающая папка - это
  цикличесская запись с подчисткой старого говна и алерт в уведомлениях".

Today three disk stores are designed or built apart: the disk prerender of whole composites (`render-cache-disk`,
built), the project's cache folder (`project-cache-folder`, spec-engine's design) and the light copies of heavy sources
(revision 2 of this change). None keeps what the owner names as the point: the result of a HEAVY EFFECT. A whole
composite is lost by any edit anywhere in the frame — moving a title over a heavy Datamosh event throws the Datamosh's
work away, though its own result did not change.

## What Changes

- **One store, "Persistent cache" («Постоянный кэш»)**, inside the project's cache folder (`project-cache-folder`), with
  three kinds of entry under one budget and one eviction order:
  - **layer results** — the picture of one layer after a heavy point of its own chain (an event's chain after its
    heavy node, a track stage, a reel instance's output), keyed by exactly what determines that picture (the source's
    identity and material position, the chain prefix's content and curve values at that moment, the decode size) —
    so an edit of another layer, of the effects AFTER the cut, or of the timeline position keeps it;
  - **frames** — whole composites of slow moments (the disk prerender of `render-cache-disk`, moved in);
  - **source copies** — intra-frame copies of heavy sources (long-GOP above 1080p, or measured slower than real time)
    at the size the preview plays (revision 2's adaptive rule kept).
- **Heavy is measured, not guessed**: a node, stage or reel output whose measured device time (the per-pass times the
  journal already records) is at least half a frame period is a cut point; the store writes its result at rest and
  while playing when the device has room (the render-ahead producer's spare time).
- **Reading**: before a layer runs its chain, the plan asks the store for the latest cut point of that chain held for
  this moment; found, the layer starts from it and runs only the nodes after it. A whole frame found is shown as today.
- **A key that survives updates**: an entry is keyed by the versions of the kernels it ran (each plugin's version and
  shader text hash) and the decoder's major version, not by the whole program build — an update that does not touch a
  Datamosh keeps its cached Datamosh.
- **The switch beside the quality** reads «Постоянный кэш: как в превью / 1/2 / 1/4 / выкл.» (the copy size; "выкл."
  stops reading and writing the store); the cache folder's limit and its cyclic eviction with a notice are
  `project-cache-folder`'s (B2).
- **Never rendered**: no export reads the store (the "draft render" of `cached-final-render` reads frames only and says
  so).
- Nothing in the interface says "proxy".

## Capabilities

### Modified Capabilities

- `video-plan`: layer results as cut points of a layer's chain; reading before a chain runs; the heavy rule; the key;
  frames and copies as kinds of the one store.
- `media-index`: source copies (the file, the adaptive size, the camera's copy).
- `export`: no export reads the store.
- `ui-reach`: the switch, card badges, the store in the Diagnostics.

## Impact

- kt-engine: `render_cache.rs` becomes the store of three kinds (`Entry::{Stage, Frame, Copy}`), its root and limit from
  the cache folder; the plan's per-layer durable prefix key (`stage_key(layer, cut)`) beside the frame's durable key;
  the producer writes layer results; `frame_plan` starts a layer from a found cut.
- kt-gpu: a texture from a decoded stage segment enters the chain as the cut node's output (the same upload path).
- kt-media: the copy transcode loop (revision 2), the copy source with the original's natives.
- kt-ffmpeg: `MuxConfig` per-frame timestamps (found-defects 823); stage segments with alpha as a colour segment plus a
  luma segment of the alpha, both on the hardware encoder.
- kt-app: the switch, badges, a Diagnostics row per kind (bytes, hits, writes); i18n ×5.
- `project-cache-folder` (spec-engine, agreed in its `4efae3ae`): `render/`, `stages/`, `copies/`, `history/` side by
  side, `mix/` outside the store; one value order across the store and `mix/`; the key of D4.
- No project format change. Idle: writing happens at rest only within the prerender's existing rule.

## Questions to the owner

**Q1. How exact must a cached effect result be?** Your items above: the cache exists "чтобы эффекты тяжелые не считать";
your earlier choice for the disk prerender (`kkt-26.11.18b` U4): "сделай ты сжатие видео нормальное" — a normal video
codec on the hardware, lossy accepted, the export never reads it. A layer result is not shown as is: the effects after
the cut process it again (a sharpen after a cached blur amplifies the codec's noise).
- (a) **Lossy on the hardware encoder at a high quality, like the prerender, for all three kinds** (*recommended*: a
  tenth of the disk of lossless; the preview may differ from the render by the codec's noise, never the export).
- (b) Layer results lossless (LZ4-compressed, about 10× the disk), frames and copies lossy.
