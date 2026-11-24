# Proposal: persistent-cache

> DESIGN ONLY (spec session), revision 4 (2026-11-23) after the owner's answer to Q55 (`kkt-26.11.22` D1): the
> store keeps the FINAL preview frame only, never a result at a stage of a chain. Revision 3 (B1) kept layer results
> at heavy points of a chain; revision 2 (`disk-copies`) kept source copies only; revision 1 (`proxies`) the same
> with a manual switch. Tests reserved: t3440–t3459.

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
- `kkt-26.11.22.txt` lines 4–5 (D1, D2), the answer to Q55: "я не понимаю зачем тебе такие сложности - делай кеш
  имменно финального превью а не на каком-то конкретном этапе цепочки fx. у тебя что, после каждой цепочки есть
  отдельный кэш?!?!? а не дохера ли мы данных сейчас в таком случае храним??"

**Reading of D1, checked.** "Финальное превью" is the picture the preview shows: the whole composite of a moment,
after every layer, track stage and reel. The heavy effects' work is kept INSIDE that picture — a heavy moment is
not composed again while its frame is held. What goes is revision 3's second store of pictures at a cut inside a
chain (`layers/`). His D2 question (does every chain keep a cache of its own today, do we store far too much) is an
audit of all caches, owned by spec-engine (registry D2); this change answers its own part: revision 3 was a design,
nothing of `layers/` was built, and after this revision the persistent cache holds two kinds only.

## What Changes

- **One store, "Persistent cache" («Постоянный кэш»)**, inside the project's cache folder (`project-cache-folder`),
  with TWO kinds of entry under one budget and one eviction order:
  - **frames** — whole composites of moments, as the preview shows them (the disk prerender of `render-cache-disk`,
    moved in), at the quality the preview plays;
  - **source copies** — intra-frame copies of heavy sources (long-GOP above 1080p, or measured slower than real
    time) at the size the preview plays (revision 2's adaptive rule kept).
- **No result at a stage of a chain** is written anywhere on disk by this store: no layer, node, track-stage or
  reel-output picture. A test scans the store for any other kind.
- **Adaptive, "как играем в превью так и храним"**: frames are written at the preview's current quality; a higher
  quality makes better frames at rest (the old ones are no longer read and go by the eviction order); a lower quality
  writes its own.
- **An edit costs only the moments it touches**: a frame's key covers its whole plan, so a moved title re-renders the
  moments the title covered before and after the move; the heavy moments around it stay held.
- **A key that survives updates**: the decoder's major, the kernels' versions and WGSL hashes, the composite path's
  version, not the build (`project-cache-folder`'s text).
- **The switch beside the quality** reads «Постоянный кэш: как в превью / 1/2 / 1/4 / выкл.» (the copy size; "выкл."
  stops reading and writing the store).
- **Never rendered**: no export reads the store (the "draft render" of `cached-final-render` reads frames only and
  says so).
- Nothing in the interface says "proxy".

## Capabilities

### Modified Capabilities

- `video-plan`: the persistent cache keeps final frames only, at the preview's quality; frames and copies under one
  value order.
- `media-index`: source copies (the file, the adaptive size, the camera's copy).
- `export`: no export reads the store.
- `ui-reach`: the switch, card badges, the store in the Diagnostics.

## Impact

- kt-engine: `render_cache.rs` gains the copies as a second kind (`Entry::{Frame, Copy}`), its root and limit from the
  cache folder; the frames keep their durable key (`project-cache-folder`).
- kt-media: the copy transcode loop (revision 2), the copy source with the original's natives.
- kt-ffmpeg: `MuxConfig` per-frame timestamps (found-defects 823), MKV with audio stream copy, all-intra.
- kt-app: the switch, badges, a Diagnostics row per kind (bytes, hits, writes); i18n ×5.
- `project-cache-folder` (spec-engine) owns the layout; its `results/…/layers/` goes (told to spec-engine
  2026-11-23); `frames/`, `history/` (`durable-history-and-mix`), `mix/` and `media/<id>/copy-*.mkv` stay.
- No project format change. Idle: writing happens at rest only within the prerender's existing rule.

## Questions to the owner

None open. Q55 (how exact a stage result is) fell with the stages: frames stay on the hardware codec at a high quality,
your own earlier choice for the prerender (`kkt-26.11.18b` U4, "сделай ты сжатие видео нормальное").
