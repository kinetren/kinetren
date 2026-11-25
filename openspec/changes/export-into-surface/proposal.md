# Proposal: export-into-surface

> DESIGN ONLY (spec-engine, 2026-11-24, orch `0c2a4065`). The dma-buf half of `export-host-copy` (its D6), as a change of
> its own: that change's built half closes and archives. Tests t4125–t4136.

## Why

The owner's answer, whole, `docs/ktprompts/kkt-26.11.20.txt` line 21 (registry A13):

> 18. Q19 export through dma-buf: "Сделать сейчас"

The question it answers (`owner-checks.md` at `c5b9eb30`, Q19): "Export with no host copy at all (dma-buf into VAAPI)? This
change already moves the colour conversion onto the card and reads 2.7× less. (a) recommended: only if your next render's
trace shows the copy or upload binding it; (b) build it now for Linux VAAPI." He chose (b). The question said it was Linux
VAAPI code, and that Windows comes later; nothing is asked of him again.

What a card export does with each frame today (`h264_vaapi`, `hevc_vaapi`, `av1_vaapi`, `vp9_vaapi`):

```text
composite (RGBA8) ─yuv.wgsl─► packed planes ─copy──► staging buffer ─map, memcpy──► Vec
   ─memcpy──► AVFrame ─av_hwframe_transfer_data──► VA surface ─► encoder
```

The frame leaves the card, is copied twice by the processor and goes back to the same card. After this change:

```text
composite (RGBA8) ─yuv.wgsl, drawn INTO the encoder's surface─► encoder
```

`export-host-copy` D6 deferred it on one ground that is no longer true: "wgpu offers none of that: it needs raw Vulkan". wgpu 30, the
version this program is built with, imports a dma-buf (`wgpu-hal` `texture_from_dmabuf_fd`, the feature
`VULKAN_EXTERNAL_MEMORY_DMA_BUF`). What is left raw is two barriers.

## What Changes

- **The encoder's own surfaces are drawn into.** FFmpeg's VAAPI frames pool allocates them, the driver exports each as a
  dma-buf (`av_hwframe_map` to DRM PRIME), the composing device imports it once as two textures, and the conversion pass draws
  luma and chroma straight into them. The encoder then takes the surface. No byte of the frame reaches host memory.
- **The bytes are those of today's path**: the same integer formula; the file is equal bit for bit to the upload path's.
- **Used only where it is proved, on this machine, for this render.** Every condition has a reason in words; before the
  first frame a test picture goes through a real surface and is read back and compared; a driver first meets a
  throwaway process. Anything else: today's path, and the log says why.
- **The disk prerender's writer takes the same way.**
- **The trace and the render log say** how many frames went by surface, how many by upload and why.

## Capabilities

### Modified Capabilities
- `export`: a VAAPI encoder takes the composite on the card.
- `video-plan`: the disk prerender's writer too.
- `telemetry`: the hand-over in the export's stages.

## Impact

- kt-gpu: `surface.rs` (import, barriers, proof), `yuv.wgsl` two entry points, two pipelines, `wanted_features` + the dma-buf
  feature; a direct edge to `ash` 0.38 on Linux (already in the lock through wgpu-hal: no new crate).
- kt-engine: `VideoGpuExec::{import_surface, submit_surface}`, `ExportSink::surfaces`, `export_pipeline.rs`, `render_cache.rs`.
- kt-ffmpeg: `muxer_impl.rs` (the pool, the map), an optional symbol table of one name, `hwcontext_drm.h` in the bindgen
  headers, `hw_trial` (a third trial).
- `fixtures/fake_va`: `vaExportSurfaceHandle`, its failure modes.
- Invariants: 9 (in-process; the one exception is the same throwaway process), 10 (no link, an optional symbol), 11 (the
  bytes do not depend on the way), 7 (the plan and the one composition path are untouched).
- No project format, no OTIO, no new setting. At rest: nothing.

## Questions to the owner

None. His check on a card is C11; its text gains what to look for (design D12).
