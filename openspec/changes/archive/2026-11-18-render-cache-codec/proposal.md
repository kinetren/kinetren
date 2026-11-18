## Why

`render-cache-disk` stored each prerendered moment as a lossless file (a byte delta, deflated). Asked how
the prerender is stored (kkt-26.11.18b U4), the owner refused a processor codec ("слабо я верю что ffv1
аппаратно ускорен.. у нас datamosh лагает") and then decided: "сделай ты сжатие видео нормальное" — a normal
video codec on the machine's hardware encoder and decoder. U5 adds that the cached final render
(F-EXP-003) will later reuse the prerender's keys, so the key must be a stable, documented value apart from
the codec.

## What Changes

- The store keeps SEGMENTS: runs of consecutive moments written as one short video, every frame a keyframe,
  by the export's encoder cascade in-process (`kt_media::segment::ffmpeg::FfmpegSegments`), HARDWARE ONLY;
  no hardware encoder opens → the store writes nothing, the Diagnostics window and the trace say why.
  `KT_RENDER_CACHE_ENCODER` forces an encoder by name (tests, the live check in a virtual screen).
- A segment's text manifest names the codec, the encoder, size, rate and every frame's durable key; the key
  stays the plan's durable hash, independent of codec and container.
- One `render-cache-read` thread owns every open segment reader (invariant 5).
- Eviction by segment; the trace's `render_cache` STORE/EVICT records carry moments and bytes, STORE the
  encoder's name; a NO_ENCODER stop names the reason; `trace cache` prints them.

## Capabilities

### New Capabilities

### Modified Capabilities
- `video-plan`: the disk tier's storage is hardware-encoded video segments with manifests.
- `telemetry`: `trace cache` names segments, encoders and why the cache writes nothing.

## Impact

kt-media (new `segment.rs`: the traits, `RawSegments` for tests, the FFmpeg segments), kt-engine
(`render_cache.rs` store and prerender), kt-time (`why::NO_ENCODER`), kt-cli (`trace cache`), kt-app (the
Diagnostics line, five catalogs). `miniz_oxide` leaves kt-engine.
