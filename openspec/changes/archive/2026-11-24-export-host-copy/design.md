# Design: export-host-copy

## Context

- found-defects 741 and export-readback D4 (`openspec/changes/archive/2026-11-18-export-readback/design.md`):
  the owner's trace — device 1.06 ms a 1080p frame, copy out 2.2 ms a frame on the compose thread, convert
  26 % of the wall over four workers that were idle 373 % of it; with the latency hidden the next bound was
  the compose thread's own 3.1 ms a frame (plan, submit, copy out), then the encoder.
- The chain: kt-gpu `Engine::submit` copies the composite texture into a ring slot buffer
  (`copy_texture_to_buffer`, RGBA8, `bpr` padded), `collect` waits, maps and copies the rows into a `Vec`
  (`copy_ns`); `export_pipeline.rs` sends `Job::Convert { rgba }` to a frame worker, whose
  `kt_ffmpeg::VideoConverter` runs `sws_scale` RGBA → the encoder's `PixFmt` with `SWS_BILINEAR` and the
  chosen matrix and range (`muxer_impl.rs`); the writer's `encode_video_frame` uploads the frame to a VAAPI
  surface for a device encoder (`av_hwframe_transfer_data`), counted inside `encode_ns`.
- Frame workers on a software adapter compose on devices of their own (`execute` → RGBA → convert in the
  same worker); a card composes on one device in the compose thread (export spec "A software adapter
  composes an export on several devices").
- The encoders' input formats are a closed set (`kt_ffmpeg::encoders::PixFmt`): yuv420p, nv12,
  yuv420p10le, p010le, yuv422p10le.
- The disk tier's writer (`segment.rs` via `kt_ffmpeg::Muxer`) goes the same RGBA → swscale → upload way.

## Goals / Non-Goals

Goals: the trace shows each move of a frame; the conversion leaves the processor; the bytes read back
shrink to the encoder's planes; an export's YUV is an exact, device-independent function of its RGBA.

Non-goals: zero-copy into the encoder (D6, owner Q1); the decode side's conversion (device-decode-sources);
changing the composite or its RGBA (GOLDEN_VIDEO untouched).

## D1. Measure every move first

`ExportProfile` gains `upload_ns` (the writer's `av_hwframe_transfer_data` and surface get), taken out of
`encode_ns`; the `ExportStage` records and `trace … export` print copy out, convert and upload per frame
beside the device time and the time back (export-readback t2962). This stage alone answers what the
owner's next render is bound by, and is useful whether or not the rest is built.

## D2. One exact conversion

A formula of ours replaces swscale's RGBA → YUV, so it can run identically on a device and on the
processor:

- Coefficients of the chosen matrix (BT.601 / BT.709, export spec "The colour tags are the conversion's")
  and range (limited 16–235/240, full 0–255) as integers in Q15; per pixel
  `Y = (kr·R + kg·G + kb·B + off_y + 2^14) >> 15`, chroma computed per pixel at Q15 BEFORE rounding.
- Chroma siting **left** (co-sited horizontally, centred vertically — the H.264/HEVC default a player
  assumes untagged): 4:2:0 `C = round(([1 2 1] horizontal at even x) · ([1 1] vertical) / 8)`, 4:2:2 the
  horizontal filter only; the edge column repeats. The stream is tagged `chroma_location = left`.
- 10-bit: the same at Q15 with the 10-bit offsets and scales; P010 puts the sample in the high 10 bits.
- Only integer adds, multiplies and shifts in i32/u32: WGSL and Rust compute the same numbers on every
  device (no float, no `fma` contraction question). The CPU reference lives beside the shader in kt-gpu
  and is used by every path without the pass.

Bytes change once against former builds; t2806's "within 6 codes through the decode that reads the tags"
stays the check of the look. Rejected: copying swscale's exact arithmetic into a shader — its C and SIMD
paths already differ, and `SWS_BILINEAR` with RGB input is not a documented filter.

## D3. The pass on the composing device

The executor gains an output layout for readbacks: `VideoGpuExec::submit_export(…, layout: OutLayout
{ pix, matrix, range })` (and the synchronous `execute_into` for frame workers). After the composite, a
compute pass reads the composite texture and writes the planes, tightly packed in the order libav's
`AVFrame` planes take them, into the slot buffer; `collect` returns them. The readback slot is sized by
the layout (`w·h·1.5` for 8-bit 4:2:0, `·3` for 10-bit 4:2:0, `·4` for 4:2:2 10-bit) and `ring_depth`
counts those bytes against `RING_BYTES`: at 1080p 8-bit the ring holds about 2.7× more frames.

The frame worker's job becomes a plane copy into the encoder's `AVFrame` (its own line sizes); the
compose thread's copy out shrinks with the bytes. The plan and its keys do not change (invariant 7): the
pass is an output stage of the executor, like the readback copy.

A frame composed on the processor-side bytes path (GL bytes path, row 774) or by a test executor
converts by the CPU reference — the same bytes.

## D4. The disk prerender

`render_cache`'s writer asks the same layout of its executor and hands planes to the segment writer
(`SegmentWriter` takes planes; `RawSegments` for tests keeps RGBA by converting back with the
reference's inverse only in tests). At rest this removes the swscale pass per prerendered moment. The
durable key is unchanged (it keys the composite, not its encoding).

## D5. Determinism

Invariant 11: the export's video bytes were already byte-stable on one device; now the conversion is
exact everywhere, so two machines whose composites are equal write equal YUV. The pipelined export
still equals the serial one (t1451), with the pass in both. An export with frame workers on several
devices converts on each device by the same integers.

## D6. Deferred: the composite as the encoder's surface

> Superseded 2026-11-24 by change `export-into-surface` (the owner's A13). Its premise below is out of date: wgpu 30
> imports a dma-buf (`wgpu-hal` `texture_from_dmabuf_fd`).

A VAAPI encoder could take the composite with no host copy: the pass writes NV12 into a VkImage created
exportable (`VK_KHR_external_memory_fd`, `VK_EXT_external_memory_dma_buf`, a linear or modifier layout
libva accepts), exported as a dma-buf and wrapped by `vaCreateSurfaces` with
`VA_SURFACE_ATTRIB_MEM_TYPE_DRM_PRIME_2`. wgpu offers none of that: it needs `as_hal` raw Vulkan, per
driver modifiers, and on Windows another path (a D3D12 shared handle opened as a D3D11 texture for
d3d11va/MF/AMF/NVENC). Owner Q1; recommended only if his trace after D1 shows the copy or the upload
binding a render.

## D7. The class walk: every path that turns a composite into encoder input

| Path | After this change | Note |
|---|---|---|
| Pipelined export, card (compose thread + ring) | pass on the device, planes back | the subject |
| Pipelined export, software adapter (a device per worker, `execute`) | pass on the worker's device | same integers |
| Serial export (`export.rs` loop) | pass via `execute_into` | t1451 keeps it equal |
| Disk prerender writer (`render_cache.rs`, `segment.rs`) | pass | CPU at rest drops |
| kt-cli `export`, `showcase` | as export | |
| GL bytes path (row 774) | CPU reference | no compute on GL |
| Thumbnails, poster frames, reel previews, PNG/still outputs | unchanged (RGBA) | no encoder |
| The preview's display | unchanged | no readback |
| Decode side YUV → RGB | unchanged here | device-decode-sources D1, D8 |
| Audio | n/a | |
| Windows (same WGSL) | pass | the encoders there take yuv420p/NV12 |

## D8. Stages

1. D1: the upload apart, the trace line. t3316. (Can merge alone.)
2. D2 + D3: the reference, the pass, layout-sized slots, frame workers' plane copy, tags. t3312–t3315,
   t3318, t3319.
3. D4: the prerender's writer. t3317.

## Risks

- The extra pass costs device time per frame (one read of the composite, one write of 1.5 B/px): small
  against the 1.06 ms composite measured, but measured by the existing `GpuFrame` records.
- A hardware encoder that needs a line alignment different from libav's frame: the plane copy into the
  `AVFrame` absorbs it (the worker copies with the frame's line sizes).

## As built (stage 2)

- **The reference lives in kt-engine** (`kt_engine::yuv`), not kt-gpu: kt-gpu depends on kt-engine, and the
  export (kt-engine), the pass (kt-gpu) and the trait's default must all reach the one formula. The
  coefficients are computed in integers from the rational Kr, Kb (no float, no libm), and each chroma's
  three weights sum to zero, so every grey is the exact middle; t3312 compares every primary with the
  formula by hand.
- **The pass is a FRAGMENT pass** (`shaders/yuv.wgsl`, `Pipe::Yuv`): the export's devices on a machine
  without a card are GL (llvmpipe, no compute, no storage — kt-gpu's "fragment passes, not compute"), so a
  compute pass would have run nowhere here. The target is a word texture (Rgba8Uint, 4096 texels a row,
  a texel a 32-bit word of the packed planes): its rows are 16384 bytes, a multiple of the copy's 256, so
  the slot holds the planes with no gap and `collect` takes them in one copy. The uniform carries the Y
  weights as u32 bits and the chroma weights as f32 VALUES (all below 2^24 in magnitude): no integer is
  passed as a float's bits, where a small one would be a denormal a device may flush.
- **The trait** (`VideoGpuExec`): `readback_depth_yuv`, `submit_yuv`, `execute_once_yuv`; the default of
  the last is the reference over `execute_once`'s RGBA, so an executor without the pass writes the same
  bytes (t3318). The ring counts the planes' bytes (`ring_depth_bytes`, t3315). An export's sink says its
  layout (`ExportSink::yuv_layout`); the muxer's converter (`MuxConverter`) is the reference plus a plane
  copy into the `AVFrame` (`VideoFrame::fill_packed`) — swscale converts no export frame any more.
- An odd canvas: the GPU path reports the composite incomplete (its layers decode at the even size); the
  pass converts the same composite as the reference does (t3313 compares the bytes). 4:2:0 encoders take
  even sizes only.
