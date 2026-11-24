# Proposal: cached-final-render

> DESIGN (spec session). The owner answered Q1–Q3 (kkt-26.11.20 A16, lines 21–22): a draft render may take
> the preview's prerender, the cached render is accepted, its index lives inside the video as SEI. An
> executor builds it in stage 24 on the orchestrator's go. Tests reserved: t3322–t3335, t3870–t3876.
> Revised on `orch-26-11-01` at `5e7a8ea9`.

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

What the code holds today: `render_cache::durable_hash` keys a composite across processes (the plan's
fields without content serials, the leaves' file identities, the build salt; the device is the store's
folder); every prerender segment's manifest (`<id>.idx`) lists that key per frame (render-cache-codec
D3, written for this purpose). The prerender's segments themselves are lossy H.264 at the PREVIEW's size
and quality, so they cannot stand in an export at the project's size — and on a card the preview's frames
may be in a run form the export never uses (video-plan "An export's frame runs the settled form").

So the reusable thing is not the preview's tier: it is an earlier EXPORT.

## What Changes

- **An export may write an index of what it holds, inside the video** (SEI, the owner's Q3 b): per frame
  the durable key at the export's size and quality, per GOP a hash of its packets, and one fingerprint of everything that decides the
  bitstream (codec, encoder and its version, profile, level, pixel format, size, rate, rate control,
  bitrate/quality, GOP rule, colour tags, the device and the build).
- **A cached render looks up earlier renders' indexes** and, for every run of the new timeline whose keys
  equal a whole closed GOP of an earlier file with the same fingerprint, copies that GOP's packets
  (re-stamped, in decode order) instead of composing and encoding it; everything else is rendered.
- **Keyframes fall where reuse can happen**: a cached-render export starts a closed GOP wherever the key
  run of the timeline changes (an edit point) and at the GOP rule's spacing, as a function of the
  timeline, so a small fix re-encodes only the GOPs it touches.
- **Parameter sets in-band** (MKV, or MP4 with `avc3`/`hev1` sample entries), so a copied GOP carries
  its own SPS/PPS/VPS ("не мешает pps/sps/vps/etc перед сегментами класть").
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
- `video-plan`: the durable key's documented layout is the shared contract of the prerender and the
  render index.

## Impact

- kt-engine (`export.rs`, `export_pipeline.rs`: a copy path in the writer; `render_cache::durable_hash`
  at the export's quality), kt-ffmpeg (`Muxer` packet input, in-band parameter sets, a packet reader of our
  own outputs; the bitstream-filter functions as optional symbols), kt-app render window (a "Cached
  render" step), kt-cli (`export --cached`, `trace export` reuse counts).
- Invariant 11 is amended for a cached render only (Q2): its bytes are a function of the project AND the
  indexed files it reuses. Invariant 9: in-process, no external binary. No project format change.
- `docs/features/export/pipeline/smart-render-passthrough.md` (F-EXP-003) gains the owner's later word.

## The owner's decisions (kkt-26.11.20, lines 21–22)

- **Q1 b — "Опция «черновой рендер»".** A draft render may take the preview's prerender. The final render
  never does, so found-defects 773 is answered by the two modes (design D7).
- **Q2 a — "Да".** A cached render's bytes depend on the earlier files it reuses. A render with the option
  off is unchanged.
- **Q3 b — "индекс в видео (SEI)".** The index lives INSIDE the video, one SEI per GOP; nothing is written
  beside the file. The render window keeps a tool that strips it (design D5).
