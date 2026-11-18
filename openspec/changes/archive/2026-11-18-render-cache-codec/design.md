## Context

`render-cache-disk` (archived 2026-11-18) made the prerender: durable keys, a store, a prerender at rest.
Its files were lossless (delta + deflate, ~0.56 MB a 360p frame, 1.6× of raw). The owner's decisions on the
storage (kkt-26.11.18b U4, in order): not raw; no processor codec (his processor is the bottleneck);
finally a normal video codec on the hardware encoder, lossy accepted. U5: the key will be reused by the
cached final render.

## Decisions

### D1. Segments by the export's hardware cascade

A segment is a run of consecutive moments of one state and size (at most 250), written by
`kt_ffmpeg::Muxer` — the export's own encoder cascade with its real open check (VAAPI, NVENC, QSV, AMF,
D3D11 by OS) — as H.264 in Matroska, CRF/CQ 16, every frame a keyframe (`keyint_tenths: 0`, no B-frames):
any moment decodes alone, so a loop's wrap and a seek start at once, and a segment is cut anywhere. The
encoder must report `hardware`; otherwise the segment is refused with the cascade's failures as the reason
and the store writes nothing on this machine (no processor fallback: the owner's rule).
`KT_RENDER_CACHE_ENCODER=<name>` forces an encoder (tests, the live check in Xvfb). In-process, our
runtime-loaded libav (invariants 9, 10). kt-engine stays free of FFmpeg: the traits live in
`kt_media::segment`, the host hands `Config::codec`.

Rejected: a file per moment (an encoder and a decoder opened per frame); FFV1 / delta-deflate (processor
codecs, refused); raw RGBA (1.4 GB a minute at 360p, 12 GB at 1080p).

### D2. Reading

`render-cache-read`, one thread per store, owns the open readers (four kept, most recently used): a decoder
has one owner (invariant 5). `DiskStore::get` asks it and waits for the frame. The reader is
`FfmpegSource` (our libav decoder): software decoding today — a hardware decode path does not exist in
kt-ffmpeg's source yet (found-defects 775). Measured below, the read is a few milliseconds of a core per
moment against hundreds of milliseconds of composing.

### D3. The key for F-EXP-003 (U5)

The durable key is `xxh3_64` over a documented byte layout (`render_cache::durable_hash`: the plan's fields
in the order `plan_hash` takes them, without content serials) and the leaves' identities (`Leaves::digest`:
the source address and `src_rev` of each leaf in plan order, then each file's `(media id, xxh3(path, size,
mtime ns))`, then the project's file identities for a stream stage). It does not depend on the codec, the
encoder, the container or the store's folder; it changes when `FORMAT` (the layout) changes. It is written
as 16 hex digits, one per frame, in the segment's manifest. The cached final render (F-EXP-003) would key
each rendered frame the same way at the export's size and quality (the plan's `out_w/out_h` are in the key),
write the keys beside the rendered segments as an index, and before a render ask which runs of keys an
earlier file holds; a run whose keys all match is copied, the rest rendered. The preview's segments are not
that index (they are lossy, at the preview's size); the key and the manifest form are shared.

## Measurements

Release `kt-cli`, llvmpipe, load ~6–18; the same heavy one-second loop (VHS Tape + Camera ISP, 640×360) as
`render-cache-disk`'s measurements. No hardware encoder opens in this VM (the cascade's `h264_nvenc` and
`h264_qsv` fail, no VAAPI node opens), so the default store writes nothing here and the runs that write use
`KT_RENDER_CACHE_ENCODER=libx264`, the processor path the owner's machines will not take.

| | no tier | written (libx264 forced) | restart over it | default codec (hardware only) |
|---|---|---|---|---|
| V-fps over 20 s | 21.1 | 8.9 | 25.0 | 20.9 |
| incomplete composites | 148 | 719 | 0 | 166 |
| process CPU | 8.0 cores | 8.2 cores | 0.2 cores | 8.2 cores |
| written at rest | — | 54 moments, 16 segments, 5.0 MB, 12.7 s | — | 0 (trace: "no hardware encoder opens") |

- Size: 5.0 MB for 54 frames at 640×360 = ~95 KB a frame, ~140 MB a minute at 25 fps (raw RGBA 1.38 GB/min;
  the earlier lossless form 0.84 GB/min). At 720p ~4× and 1080p ~9× of that: ~0.55 and ~1.3 GB a minute.
- Write: libx264 at 320×180 0.7 ms a frame (t2986's measure), at 640×360 a few ms, beside 215 ms of device a
  moment to compose it; on a hardware encoder the processor does the RGBA→YUV conversion only.
- Read: 27 restores took 230 ms of reading (8.5 ms a moment, opening the segment readers included; 5.3 ms a
  frame out of order at 180p in t2986) against 215 ms of device to compose a moment. The restart's process
  CPU was 0.2 cores for the whole run (8.0 without the tier).
- The run that wrote shows V-fps 8.9 at a heavier load than the first run; its play is the tier-less play
  (nothing on disk yet), so the difference is the machine's load, not the tier.
