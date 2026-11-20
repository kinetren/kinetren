# Proposal: device-decode-sources

> DESIGN ONLY (spec session, owner 2026-11-20: "подготавливать архитектурные вещи - спеки. но не
> выполнять их"). Code starts after the owner answers the questions at the end and the orchestrator
> gives the go. Tests reserved: t3300–t3311. Base: `orch-26-11-01` at `ec30cb02`.

## Why

found-defects 775 closed its first half in `hw-segment-decode`: the disk prerender's segments decode
on the device (VAAPI on Linux, D3D11VA on Windows). Its next step is written in the row: "the preview's
sources and the export on the device (a change of its own: per-key decoder choice, surface pools)".

The owner's reason is the processor. kkt-26.11.18b, his words: "у нас datamosh лагает как нехуй делать"
— the processor is already the preview's bottleneck, and his decision U4 put "the hardware encoder and
decoder of the machine" in the place of processor codecs. Today every camera and phone file on the
timeline (H.264, HEVC) is decompressed by libavcodec on the cores: four decoders per 1080p file,
two per 4K one, each with frame threads (`FfmpegSource`, decode-waste D3).

Why it was not simply switched on for sources:

1. **The picture would depend on the decoder.** A device picture comes back as NV12 and the
   processor's as yuv420p; swscale converts them by different paths, and t3129 measured a mean
   difference of 1.46 between the two decodes of one segment. The burst cache's key holds the source,
   the quality and the decode step (invariant 4), not the decoder; a key served once by a device
   decoder and once by a processor decoder would hold two different frames. The export would change
   bytes run to run whenever a decoder fell back (invariant 11).
2. **A device decoder is a bounded pool of surfaces.** A 4K H.264 stream holds 17+ reference surfaces
   of 12 MB; a dense edit opens dozens of decoders. Vegas users report NVDEC taking all video memory;
   Resolve uses the device only while memory is left (F-MED-010).
3. **Every `Source::open_hw` opens a device of its own.** For the tier's one reader thread that was
   fine; for one MediaWorker per source and up to four decoders each it is dozens of VA displays.

## What Changes

- **A device picture is the processor's picture, byte for byte.** H.264, HEVC, VP9 and AV1 define
  their decoded samples exactly; the only difference was our conversion. The device's NV12/P010 is laid
  out by an exact copy into the pixel format the processor's decoder gives for that stream (yuv420p,
  yuvj420p, yuv420p10) before the one converter, so the colour rule, the swscale context key and every
  byte after it are the same. The decoder becomes invisible: the cache key, the durable key, thumbs'
  pixel version and the export do not change.
- **Which files decode on the device** is a function of the file, decided once per process: an exact
  codec, a 4:2:0 flavour the device takes, no top/left crop, and a proof on the file's first keyframe
  that the device's samples equal the processor's. Anything else stays on the processor and the
  report says why.
- **One decode device per process**, shared by reference; each decoder and its surface pool stays
  owned by the thread that owns the decoder (invariant 5 unchanged).
- **Surfaces are a tenant of the memory ledger.** A decoder opens on the device only when the ledger
  grants its pool; otherwise it opens on the processor — which, by the first point, changes no byte.
- **The preview's sources and the export's sources open this way**; thumbnails, stills, filmstrips
  and reel strips stay on the processor (one picture each: the device costs more than it spares).
- **The trace says who decoded each picture** and what carrying it back cost, so the gain is judged on
  the owner's machine (owner, 2026-11-09: "telemetry explains").

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `media-index`: four requirements — the device picture equals the processor's; which files decode on
  the device; one device per process with decoders on their owner's thread; surfaces within the
  ledger.
- `export`: the export's bytes do not depend on where its sources decode.
- `telemetry`: the trace names each picture's decoder and the transfer.

## Impact

- `src/libs/kt-ffmpeg/src/source_impl.rs` (layout after `download`, shared device, eligibility,
  proof), `src/libs/kt-ffmpeg/src/lib.rs` (`VideoDecode`, the process's device),
  `src/libs/kt-media/src/ffmpeg.rs` (`FfmpegSource` opens per decoder by the plan and the ledger),
  `src/libs/kt-media/src/factory.rs` (preview/export opens), `src/libs/kt-time/src/mem.rs` (tenant
  `Surfaces`), `src/libs/kt-engine/src/worker.rs` (the `Decoded` record), kt-cli `trace summary`,
  `src/fixtures/fake_va/ktfake_drv_video.c` (thread safety, a perturb mode, a fault mode).
- No project format change, no OTIO loss, no new required FFmpeg symbol (the frames context is read
  through the version modules that already carry `AVHWFramesContext`).
- Invariants: 4 (the key needs no decoder because the frame has none), 5 (owner thread unchanged; the
  device is a shared reference, not a decoder), 10 (optional symbols only), 11 (export byte-stable on
  one device and, as a bonus, with and without the device).
- `docs/features/media/sources/hardware-decoding.md` (F-MED-010).

## Questions for the owner

**Q1. On by default, or measured on your machine first?**
Your words (kkt-26.11.18b, U4): "сделай ты сжатие видео нормальное" — the prerender on the machine's
hardware encoder AND decoder; the reason given was the processor ("у нас datamosh лагает как нехуй
делать"). Today: only the prerender's segments decode on the card; every source file is decoded by
the processor. Carrying a picture back from the card costs a copy, and on some integrated chips
reading the card's memory is slow, so the gain is not certain on every machine; this VM has no
VAAPI device, so we cannot measure it here.
- **a) On by default** wherever the card and the file allow it; `KT_DECODE_DEVICE=off` for A/B; your
  next trace shows per picture who decoded it and what the copy cost. Since the picture is
  byte-equal, a wrong guess costs speed, never a frame. *(Recommended.)*
- **b) Off by default** until your trace of one run with `KT_DECODE_DEVICE=on` shows it wins; then a
  second small change flips it.

**Q2. A visible switch, as Premiere/Resolve/Vegas have, or none?**
Today: no setting exists; the disk tier decides by itself and Diagnostics says why when it cannot.
- **a) No switch** (as Final Cut Pro): the program decides per file, Diagnostics gets one line
  "Decoding on the card: N files; on the processor: M (why)". *(Recommended: the choice cannot change a
  frame, so there is nothing for a person to decide.)*
- **b) A checkbox in Settings** "Decode video on the card" (on/off), beside the Diagnostics line — for
  a driver that decodes correctly but slowly.
