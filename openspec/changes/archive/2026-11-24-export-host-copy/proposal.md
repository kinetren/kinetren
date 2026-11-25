# Proposal: export-host-copy

> DESIGN ONLY (spec session). Code starts after the owner answers the question at the end and the
> orchestrator gives the go. Tests reserved: t3312–t3321. Base: `orch-26-11-01` at `ec30cb02`.

## Why

found-defects 741, from the owner's trace `fails/26.11.18/render_simple_region` (kkt-26.11.18 T3): "after
the latency, the export's next costs on a card are the host copy and the CPU colour conversion" — copy
out 2.2 ms a frame on the compose thread (12 % of the wall), convert 26 % of the wall over four workers.
In that run neither bound the export (the device's 52.7 ms latency did; export-readback fixed it, row
740), so export-readback D4 left it "to be measured on his machine, not built blind".

What every frame of a card export does today:

```text
device composite (RGBA8) ─copy 4 B/px─► host RGBA ─swscale (CPU, SWS_BILINEAR)─► YUV planes
    ─[hardware encoder] upload 1.5 B/px (av_hwframe_transfer_data)─► surface ─► encoder
```

Three moves of the frame and a full-frame CPU pass. The same chain runs at rest in the disk prerender
(`render-cache-codec`: segments are written by the hardware encoder from read-back RGBA), where the owner
counts every core ("the program at rest costs nothing", his one exception being the prerender itself).

## What Changes

- **The trace states every move of an export frame**: copy out, convert, and the upload to the
  encoder's surface (today hidden inside `encode`), so his next render shows what binds after the
  latency fix.
- **The colour conversion moves onto the composing device.** A compute pass after the composite
  writes the encoder's planes (yuv420p, NV12, yuv420p10, P010, yuv422p10 — the closed `PixFmt` set)
  into the readback slot; the copy out is 1.5 B/px instead of 4 for 8-bit 4:2:0, and the CPU convert
  becomes a plane copy. The readback ring is bounded by bytes (export-readback D1), so 2.7× more frames
  fit in flight for free.
- **The conversion is ours and exact**: integer arithmetic, one formula with its chroma siting
  written down, a CPU reference equal byte for byte to the shader. Every path that cannot run the pass
  (a GL bytes path) converts by the reference, not by swscale; so an export's YUV is the same function
  of its RGBA on every device and every machine — less variance than today, not more (invariant 11).
- **The disk prerender's writer takes the same pass.**
- **Deferred, on the owner's word and his trace**: the composite handed to a VAAPI encoder as a
  surface with no host copy at all (a dma-buf from wgpu's Vulkan device into libva).

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `export`: the RGB→YUV conversion is exact and runs on the composing device; the readback carries
  the encoder's planes.
- `video-plan`: the disk prerender's writer reads planes from the same pass.
- `telemetry`: the export trace states copy, convert and upload per frame.

## Impact

- kt-gpu: a YUV output pass (WGSL, integer), readback slots sized by the layout's bytes,
  `ring_depth` by bytes per layout; kt-engine: `VideoGpuExec` readback with an output layout,
  `export_pipeline.rs` `Job::Convert` carrying planes, `export.rs` serial loop, `render_cache.rs` writer;
  kt-ffmpeg: `VideoConverter` replaced by the reference conversion (planes → `AVFrame`), the upload timed
  apart; kt-cli `trace … export`.
- Output files' bytes change once against former builds (the look within t2806's 6 codes). GOLDEN_VIDEO
  hashes the RGBA composite and does not move; t2805/t1451 (same settings → same bytes; pipelined =
  serial) keep holding.
- Invariants 7 (one executor: the pass is the executor's, the plan does not change), 9 (in-process,
  unchanged), 11 (strengthened: the conversion adds no device variance).

## Questions for the owner

**Q1. Hand the frame to a VAAPI encoder with no host copy at all — now, or only if your trace asks?**
Your trace (kkt-26.11.18 T3) showed copy out 2.2 ms a frame and convert 26 % of the wall over four
workers; neither bound that render — the card's 52.7 ms latency did, and export-readback fixed it.
Today a card export reads every frame back to the processor, converts it there and, for a VAAPI encoder,
uploads it again. This change moves the conversion onto the card and shrinks the read 2.7×; the upload
stays. A true zero-copy needs the composite exported from wgpu's Vulkan device as a dma-buf and imported
into libva, which wgpu does not offer: raw Vulkan code per platform (Linux VAAPI; Windows would need a
second, D3D11 shared-texture path).
- **a) Only if your next render's trace shows the copy or the upload binding it** (the trace of this
  change names both per frame). *(Recommended: it is the most machine-specific code we would own, and
  the cheap half may be enough.)*
- **b) Build it now for Linux VAAPI**, Windows later, accepting a Linux-only path in the export.
