# Proposal: cached-final-render

> DESIGN ONLY, FUTURE. The owner marked the cached final render "это на будущее" (kkt-26.11.18b U5);
> `v26.10` X1 is `later`. This change records the architecture so the pieces built now (the durable key,
> the segment manifests, the hardware cascade) stay compatible with it, and puts found-defects 773 to the
> owner. No executor takes it before his go. Tests reserved: t3322–t3335. Base: `orch-26-11-01` at `ec30cb02`.

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

- **An export may write an index of what it holds**: per frame the durable key at the export's size and
  quality, the packets' places, the GOP boundaries, and one fingerprint of everything that decides the
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
- **The preview's disk tier is NOT read by the export** (found-defects 773 closed by decision, Q1).

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

## Questions for the owner

**Q1. Found-defects 773: may an export ever take frames from the preview's disk prerender?**
Review R3 promised "a cached range exports at encoder speed". Today the prerender keeps lossy H.264 at
the preview's size and quality, and on a card a preview frame may be computed in a slightly different
order of passes than the export uses (1 code of difference at most). Invariant 11 says the export is byte
for byte the same between runs on one device.
- **a) Never.** The export reuses only earlier EXPORTS at the same settings (this change). A slow region
  is fast in the second render, not the first. *(Recommended: the preview's files are at another size
  and lossy; using them would make a final render worse than its own frames.)*
- **b) A "draft render" option** that takes the prerender's frames where they exist (preview size,
  scaled up, lossy), for a quick look-at file, never for the final.

**Q2. A cached render's bytes depend on the files it reuses. Accept that for cached renders only?**
Your X1 asks to copy frames "из одного файла в другом". A GOP copied from an earlier file is not the
GOP a fresh full render would encode (the encoder's rate control carries state across GOPs), though it
decodes to the same picture the earlier render made. Today: every render is byte-identical run to run.
- **a) Yes, only for a render with "Cached render" on**: the same project and the same set of earlier
  files give the same bytes; a render with it off stays as today. *(Recommended.)*
- **b) No**: then reuse is impossible (a copied GOP can never equal a fresh encode), and X1 is dropped.

**Q3. Where does the index of a rendered file live?**
Your X1 names both: "в что-то типо SEI/etc вкладывать хэши" and "хранить мета-инфу рядом с файлом а не
в SEI", with a tool to strip the SEI.
- **a) A file beside the output** (`<name>.kt-render`, text like the prerender's manifests) plus a list
  of the project's earlier renders in the data folder; nothing inside the video. *(Recommended: the
  delivered file stays clean; a moved file is found again by its size and content hash.)*
- **b) SEI** (user data unregistered, one per GOP, the frame keys) — survives renaming and moving, but
  goes to whoever receives the file until stripped.
- **c) Both**, SEI off by default.
