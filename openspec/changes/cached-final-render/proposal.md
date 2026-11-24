# Proposal: cached-final-render

> IMPLEMENTATION-READY (spec-engine, revision 3, 2026-11-24, orch `229c694b`). The owner answered Q20–Q22 (kkt-26.11.20 A16,
> lines 21–22). Tests: t3322–t3335, t3870–t3876, t4138–t4141.

## Why

Two claims, one mechanism:

- **found-defects 773** — "the export does not reuse the preview's disk tier". Review R3 promised "a
  cached range exports at encoder speed". It was left open: "a frame of the preview's executor (its run
  form) is not proven equal to the export's settled form on a card, and invariant 11 forbids the export's
  bytes depending on what the preview kept".
- **The owner's FastCachedRender** (`v26.10` X1): "при рендере каждого видео в что-то типо SEI/etc
  вкладывать хэши кэша таймлайна, чтобы можно было в последующих рендарах переиспользовать
  кадры/nalu/etc из одного файла в другом … самое главное в финале, например, для h26x - соблюсти pts
  nalu. FCR можно и отображать как отдельный шаг настройки рендера, где показывается какая зона уже
  откеширована." And kkt-26.11.18b U5: "там бы в будущем ту же логику кэша использовать, только надо
  среди уже отрендеренных файлов искать как-то понимать по индексам каким-то что можем переиспользовать
  из них а что - нет."

What the code holds today: the one key of a moment (`moment_key`) names a frame by its sources' identities, and the disk
prerender keeps it per frame. It cannot serve X1: it holds the moment's PLACE (the timeline tick, the event's start and length, a reel's whole
revision), so the fix he names (a crooked cut trimmed by two frames) would rename every frame of that event and every frame after it. And an
export's frames have no names at all: its cache adopts no identity (found-defects 1068, which is a wrong picture by itself). The
prerender's segments are lossy H.264 at the PREVIEW's size and quality: they cannot stand in an export.

So the reusable thing is not the preview's tier: it is an earlier EXPORT.

## What Changes

- **A picture key** names a frame by what its kernels are given, with no place in it: the sources' identities and times, the resolved chains,
  masks and cameras; the timeline tick only where an effect reads time (a fact of the plugin, checked by a test). A shot that moved keeps its key.
- **An export may write an index of what it holds, inside the video** (SEI, the owner's Q22 b): per frame the picture key at the export's size
  and quality and what makes its pixels, per GOP a hash of its packets, and one fingerprint of everything that decides the bitstream (codec, encoder,
  profile, level, pixel format, size, rate, rate control, bitrate/quality, GOP rule, colour tags, the device). Not the program's build: an update
  that changed no pixel keeps its renders.
- **A cached render looks up earlier renders' indexes** and, for every run of the new timeline whose keys
  equal a whole closed GOP of an earlier file with the same fingerprint, copies that GOP's packets
  (re-stamped, in decode order) instead of composing and encoding it; everything else is rendered.
- **Keyframes fall where reuse can happen**: a cached-render export starts a closed GOP wherever the key
  run of the timeline changes (an edit point) and at the GOP rule's spacing, as a function of the
  timeline, so a small fix re-encodes only the GOPs it touches.
- **Parameter sets in-band** (MKV, or MP4 with `avc3`/`hev1` sample entries), so a copied GOP carries
  its own SPS/PPS/VPS ("не мешает pps/sps/vps/etc перед сегментами класть").
- **Every offline render names its sources** (found-defects 1068): two renders in one process never read each other's pictures.
- **The render window shows what is already cached** and, as its own step, which files it reuses.
- **A draft render** (the owner's Q1 b): an option that takes the frames the preview's prerender holds
  (preview size, lossy), composes the rest at the preview's quality, and writes a file marked as a draft.
  The final render never reads the prerender (found-defects 773 is answered by the two modes).

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `export`: the render index, the cached render, the keyframe placement, the in-band parameter sets,
  the window's cached zone.
- `video-plan`: the picture key; every cache of an offline render names its sources.

## Impact

- kt-engine: `moment_key.rs` (`picture_key`, `shape`), new `cached_render.rs` (GOP plan, matching, spans), `export_pipeline.rs` and `export.rs` (spans),
  `adopt_sources` in every offline cache. kt-ffmpeg: new `nal.rs`, `gop.rs`; `muxer_impl.rs` (`open_encoder`, `copy_gop`, forced IDR); no new FFmpeg symbol. kt-plugin-api: two defaulted facts. kt-media:
  `cache_folder::Tenant::Render`. kt-app render window (a "Cached render" section), kt-cli (`export --cached`, `--draft`, `strip-index`).
- Invariant 11 is amended for a cached render only (Q21): its bytes are a function of the project AND the
  files it reuses. Invariant 9: in-process, no external binary. No project format change.
- F-EXP-003 gains the owner's later word.

## The owner's decisions (kkt-26.11.20, lines 21–22)

- **Q1 b — "Опция «черновой рендер»".** A draft render may take the preview's prerender. The final render
  never does, so found-defects 773 is answered by the two modes (design D7).
- **Q2 a — "Да".** A cached render's bytes depend on the earlier files it reuses. A render with the option
  off is unchanged.
- **Q3 b — "индекс в видео (SEI)".** The index lives INSIDE the video, one SEI per GOP; nothing is written
  beside the file. The render window keeps a tool that strips it (design D5).
