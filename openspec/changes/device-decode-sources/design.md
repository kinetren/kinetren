# Design: device-decode-sources

## Context

- `hw-segment-decode` (video-plan design, "The disk tier's segments decode on the device"):
  `kt_ffmpeg::Source::open_hw` opens the video decoder with `hw_device_ctx` and `get_format =
  pick_hw_format`; `VideoPath::download` carries each picture to host memory by
  `av_hwframe_transfer_data` as NV12 (P010 for 10-bit), keeping time and colour tags; one decoder
  thread (D1); the device verdict is asked once per render node per process (`decode_device_verdict`,
  D2). Every decoder of the file opens the same way (`FfmpegSource::hw`).
- Its D3: NV12 and yuv420p go through different swscale paths, so the two decodes differ (mean 1.46 on
  t3129's pattern). Acceptable for the lossy tier the export never reads; NOT acceptable for sources.
- `FfmpegSource` (`src/libs/kt-media/src/ffmpeg.rs`): up to `cursors_for_size` decoders per file
  (2..=4), chosen per key by the requests, never by timing; "a frame does not depend on the decoder
  that read it" (media-index, "Every read of a file chooses its decoder per key").
- The preview: one `MediaWorker` thread per source owns it (`src/libs/kt-engine/src/worker.rs`,
  invariant 5). The export: its own sources, decoded "a chunk ahead, a thread per source within the
  chunk" (`export_pipeline.rs` `decode_chunk`): a source is moved into a scoped thread per chunk, so it
  has ONE owner at a time but not always the same thread.
- The memory ledger (`src/libs/kt-time/src/mem.rs`, media-index "One memory ledger…"): tenants with
  bases and one growth budget, grants in a fixed order of value, offline holds stronger than caps,
  already counting device buffers (upload, readback, transient, compute pools).
- The colour rule (source-colour-matrix): the matrix and range from the picture's tags, the stream's
  as fallback, `yuvj*` formats implying full range; the sws context keyed by (size, source format,
  matrix, range).

## Goals / Non-Goals

Goals: sources of the preview and the export decode on the device where it pays and is correct; no
byte of any frame, cache entry, prerender key or export depends on that choice; the device's memory is
bounded and counted; the owner's trace shows the effect.

Non-goals: zero-copy from the decode surface into wgpu (D8); the CPU colour conversion of the decoded
picture (it stays; its export twin is `export-host-copy`); hardware decode of thumbnails/stills; a
proxy workflow (F-MED-002).

## D1. The device picture is laid out as the processor's picture

The decoding processes of H.264 (8.x of the standard), HEVC, VP9 and AV1 are exact: a conformant
decoder outputs exactly the reference samples. libavcodec's software decoders are conformant (FATE
compares them to the reference streams' MD5s). A conformant device decoder therefore produces the same
Y, Cb, Cr samples — interleaved differently (NV12: Y plane + CbCr interleaved; P010: 16-bit words with
the sample in the high 10 bits).

So after `download` the picture is copied, exactly, into the layout of the processor's decoder FOR
THIS STREAM: NV12 → yuv420p (or yuvj420p when the processor's decoder of this stream reports a `yuvj`
format — H.264 full-range streams on FFmpeg 6/7 do), P010 → yuv420p10le (`>> 6`). The copy is ours (a
plane split, a shift; no swscale path to depend on), on the decoder's thread, into a frame from the
same allocator. The tags are kept. From there the picture is indistinguishable: the colour rule sees
the same format and tags, the sws context key is the same, the RGBA bytes are the same.

Which format the processor would give: the decoder of the stream is opened once on the processor at
`Source` open anyway (the packet scan and the first decoder exist before the device is asked); its
`pix_fmt` after the first decoded picture of the proof (D2) is recorded as the stream's layout.

Rejected:
- *Convert NV12 with swscale to RGBA and accept the difference, put the decoder kind into the cache
  key*: two keys per moment, coverage bars that disagree with what the export decodes, a fallback
  under memory pressure that changes the preview's picture mid-loop. Invariant 4 says the key
  determines the frame; the cheapest way to keep it is a frame that has no decoder in it.
- *Convert the processor's yuv420p into NV12 instead*: it would change every former build's bytes
  (GOLDEN_VIDEO, thumbs' pixel version, the durable keys' build salt) for nothing.

Consequence for the tier: the segments' device decode becomes byte-equal to their processor decode;
t3129's "within swscale's rounding (mean ≤ 2)" becomes "equal" (task 2.4).

## D2. Which files decode on the device

Decided once per (file identity, device) per process — a function of the file, never of timing:

1. **Exact codec**: H.264, HEVC, VP9, AV1. Not MPEG-2, MPEG-4 Part 2, VC-1 (their IDCT is allowed to
   mismatch), not ProRes/DNxHR (no VAAPI/D3D11VA decode on our platforms), not stills.
2. **A flavour the device takes**: 4:2:0, 8 or 10 bit (the device formats NV12/P010). 4:2:2/4:4:4,
   12-bit, and whatever the decoder's `get_format` does not offer the device for stay on the processor
   (today's `download` already notices "the decoder did not take the device for this stream").
3. **No top/left crop**: libavcodec cannot apply a top/left crop to a hardware frame (it adjusts only
   width/height); a stream with one would decode shifted. Bottom/right crops (1088 → 1080) are the
   common case and are applied by the transfer's size.
4. **The proof**: the file's first keyframe is decoded on the device and on the processor and the laid
   out planes compared. Equal → the file is eligible; different → the processor, the reason "the
   device's samples differ from the standard's (first keyframe, N bytes)". The cost is one extra
   keyframe decode per file per process, paid inside the first device decoder's open on its owner's
   thread.

The proof covers intra decoding only. A driver whose inter prediction or deblocking deviates would pass
it. What guards then: conformance tests on our fixtures run through the fake driver (exact by
construction) and, on a machine with a real device, `kt-cli probe … --decode-check` (task 3.3) which
decodes a whole range both ways and counts differing pictures. A deviation found there is a driver
defect; the file list of the eligibility table gets a `(vendor, codec)` refusal with its reason.

Not an entry of the cache folder: the proof is cheap and a driver update would need a re-check. A per-process memo keyed
by (file, device name) is enough. **The file's key is the source's identity** of `durable-moment-key` D1 (name, size,
mtime as opened), handed down by kt-media as a number; kt-ffmpeg stats nothing of its own. Not the content id of
`project-cache-folder` (a whole-file hash the index lane learns later). `open_hw(path, prebuilt, file_key)` takes the key as an argument (kt-ffmpeg's tests pass one). **The proof is recorded after the identity
check, not in the open**: a path may hold another file by the time it is opened (row 1065: `cursor_for` drops a decoder whose file is not the source's, after the
open), and a proof written inside the open would stand under the former file's key. So `open_hw` returns its verdict, and `FfmpegSource` calls `kt_ffmpeg::remember_proof(file_key, verdict)` once
`file_ident(path)` agreed; a cursor refused by the check records nothing.

**Surfaces never meet the caches.** `download` carries a picture to host memory and frees its surface before the frame
is converted; the burst cache, the stream's input and the moment key see host frames of the source's identity, whoever
decoded them. A source replaced on disk is a new identity, so a new proof, so a new set of decoders; `adopt` drops the
old frames and the old worker's decoders drop their pools with it.

## D3. One device per process; decoders stay on their owner's thread

Today `open_decode_device` runs per `Source::open_hw`, i.e. per decoder: a VA display (driver load,
`vaInitialize`) each time. Sources multiply that by up to four decoders per file and one worker per
file.

Decision: the process opens its decode device once (first need), under the existing verdict per node,
and hands each decoder a reference (`av_buffer_ref` of the `AVHWDeviceContext`). The process's own reference is never
released: `hw_device_opened` (exit-double-free) already puts the driver behind the process's end, and a reference
dropped in a static destructor would be the crash that row fixed. Each decoder creates
its own frames context (its surface pool) in `get_format`, and that pool lives and dies with the
decoder, owned by the thread that owns the decoder. Invariant 5 is about decoder instances: it holds
unchanged. A source moved between threads by the export's chunks (one owner at a time) is as safe as
today's software decoder.

Thread safety of the shared device: FFmpeg's D3D11VA device has its own lock (`lock`/`unlock`,
defaulting to a mutex); VAAPI relies on libva's displays being thread-safe, which Mesa's and Intel's
drivers are and which FFmpeg's own threaded CLI (7.0+) relies on when decode, filters and encode share
one device across threads. Our fake driver is NOT made for it: it gains a mutex (task 1.5), and t3304
decodes on four threads at once through one device.

Rejected: a device per owner thread (one per MediaWorker): the cost we remove is per display, and the
preview has a worker per source; a device per decoder (today): the same, four times over.

The export's encoder keeps its own device context (hw-encode-context): its open order, its trial
process per open and its failure modes are the encoder's; sharing would tie a decode failure to the
render's encoder.

## D4. Surfaces are a ledger tenant (revised 2026-11-24)

A device decoder's pool is `initial_pool_size` surfaces of the coded size in the device's format
(FFmpeg sizes it from the stream's DPB plus its extra frames). After `get_format` creates the frames
context, the decoder knows its bytes: `pool × surface bytes` (NV12 = w·h·1.5, P010 = w·h·3, at the
aligned coded size). A new ledger tenant `Surfaces` holds them, as a FIXED kind beside `Upload`, `Readback`,
`Transient` and `Compute`: it is counted and bounded, it takes no part in the growth budget.

- **Why not a growing kind** (what this design said until 2026-11-24): an export lives under `mem::claim()`
  (`render_dialog.rs:850`), and a live claim makes the growth budget ZERO (`growth_budget`, `mem.rs:137`). A
  growing `Surfaces` with base 0 would be granted nothing for the whole render: the export the owner turned
  the decoder on for would never use it. It would also be cut by whatever the preview's caches want.
- **The bound.** One process-wide bound, asked when a decoder opens: `min(1 GiB, a quarter of the memory
  available now)` (`Ledger::available`; unknown: 256 MiB). A decoder whose pool does not fit opens on the
  processor and says "no room for its surfaces (N MB of M)". A software decoder of the same file holds a pool of
  its own (frame threads + DPB pictures, not counted anywhere): the device's replaces it, it is not on top of it.
  Two decoders of a 4K file are ~420 MB, two such files at once still fit; `cursors_for_size` bounds each
  file as before.
- **Account.** `kt_time::mem::open(Surfaces, 0)`, then `set(pool bytes)` once the frames context exists; dropped with
  the decoder. kt-ffmpeg already depends on kt-time (its shutdown order). A tight render (`kt_gpu::memory::tight`)
  needs no rule: the memory available then is what makes it tight.
- **No take-back.** A bound that moves down closes nothing. A decoder refused for room asks again at its next
  backward reopen; every other kind of refusal (flavour, proof, fault) stands for the process. A decoder on
  the device stays on it across `seek_and_reopen`. By D1 none of it changes a byte.
- On iGPUs (the owner's machines) surfaces are system memory, the ledger's own currency. On a discrete card
  they are video memory, which the ledger does not measure; counting them keeps the bound conservative.
- A replaced source (`add_media` swaps the worker before it stops the old one, `player.rs`) holds two pools for a
  moment: the new one may be refused for room and asks again at its first reopen.

## D5. The keys do not change

Because of D1 the burst cache key, the order (quality and decode step, invariant 4), the disk tier's
durable key, `THUMB_PIXELS_VERSION`, `STRIP_ROW_VERSION` and the build salt stay as they are. The decode
step still scales on the processor after the layout: device scaling (VAAPI VPP, D3D11 video processor)
would be a different filter and different bytes (rejected).

## D6. The export

Today (orch `229c694b`) the sources open at these places, not one:
- the preview: `util.rs` `open_source` → `kt_media::factory::open_indexed` (kt-cli `preview` and `pause-scan` the same);
- the render window: `render_dialog.rs` `open_all` → `FfmpegSource::open` (no index);
- kt-cli export: `FfmpegSource::open_indexed` inside `open_render_sources` (since row 1059 the scope of a render's decoder settings);
- kt-cli's `render` and `probe --time`: `open_sources` → `FfmpegSource::open_indexed`.

These ask facts or sound and stay software: kt-cli's `media` facts (`open_indexed` and `FfmpegSource::open` at their own commands), `gpushare.rs`, `showcase.rs`; what the
audio-reactive generators hear (`listen::ensure`); the peak pyramid (`util.rs`).

This change routes them through one factory call, `factory::open_source(path, store, Purpose::{Preview, Export, Facts})`, so the device rule (D2) and the setting (D11) are decided in one place. The render window gains the index as a side effect (`MediaStore` of the cache folder, which its job already opens for
`listen::ensure`): its sources stop rescanning a file the index holds.

**The settings travel with the source.** `DecodeSettings` (`kt-ffmpeg/src/lib.rs:52`) are read on the opening thread, and a
render opens its sources inside `with_decode_settings`. A cursor is opened later, by `cursor_for`, on whatever thread
reads the span: outside the scope, so a render's second decoder of a file takes the PREVIEW's settings (found-defects
1064). `FfmpegSource` keeps the settings it was opened with and opens its cursors under them; the new bit, "decode on the
card", is the third field of the same struct, so it obeys the same rule. Thumbnails, the filmstrip
and stills keep their own software opens (D9). Invariant 11: by D1 the bytes are those
of the processor's decode, so two runs on one device are equal AND an export on a machine without a
device equals one with it (stronger than required). The export's stage profile adds the pictures per
decoder kind and the transfer time (`ExportStage` decode record's aux), so a slow export can be read.

A device decode error mid-stream (a driver faulting after the trial, D2 of hw-segment-decode) reopens
that decoder on the processor at the same keyframe, marks the file processor-only for the process, and
the picture is the same (D1). A render does not fail on it, and its frames written equal its frames announced
(render-result-check; t3306 and t3307 compare them).

## D7. The trace

- `Ev::Decoded` gains the decoder kind (`aux`: 0 processor, 1 device) — a picture asked by the queue
  says who decoded it.
- A new record per device decoder open / refusal: the file, the device, the pool bytes, the reason of a
  refusal (ledger, flavour, crop, proof, fault).
- The transfer time per device picture is summed per source and written with the existing per-source
  decode statistics; `kt-cli trace summary` prints "decode: N pictures on the device (transfer p50 X
  ms), M on the processor (why: …)".

## D8. Rejected for now: the surface straight into wgpu

A VA surface exported as a dma-buf (`vaExportSurfaceHandle`) could be imported into Vulkan
(`VK_EXT_external_memory_dma_buf`) and sampled by the composite without any host copy. Not now:

- wgpu 30 does import a dma-buf (export-into-surface D2, whose `surface.rs` is the code this would need), but per platform (D3D11 shared
  handles on Windows are another path);
- the burst cache is the only truth about coverage (invariant 4) and holds host frames; a device-only
  frame would be a second cache with its own coverage and its own eviction;
- the YUV → RGB conversion would move to the device and change the bytes against the stored frames'
  tolerances (invariant 7's `FrameTol`) — acceptable only as its own change with its own measure.

It stays the next step if the owner's trace shows the transfer, not the decode, as the cost.

## D9. The class walk: every reader of video through our libav

| Reader | Decodes on the device after this change | Why |
|---|---|---|
| Preview sources (MediaWorker, `FfmpegSource::open_indexed`) | yes | the subject |
| Export sources (`render_dialog.rs`, kt-cli `export`) | yes | same factory, D6 |
| Disk tier segments (`segment.rs`) | yes (already) | now byte-equal to the processor (D1) |
| kt-cli `preview`, `pause-scan`, `probe` | yes | they drive the real player |
| Thumbnails, filmstrip, bin scrub (`thumbs.rs`, `util.rs`) | no | one picture per open; opening a device decoder and its pool costs more than a keyframe |
| Stills, image sequences | no | not an exact video codec |
| Reel strips (offline render of a nested reel) | inherits | they decode through the export-like offline path: yes |
| Datamosh stream input, render-ahead, ahead-kept, kept composites | unaffected | they read decoded frames from the cache, which are the same bytes |
| Loudness, sync, peak pyramid, audio-reactive series (`listen.rs`) | no | audio |
| The render-ahead composites, the disk frames of the cache folder (segments) | as the tier | they decode as `segment.rs` |
| `persistent-cache`'s source copies (`FfmpegCopies::copy`, `kt-media/src/copy.rs`, branch `persistent-cache` 14d30724) | yes, through `open_source` | today it opens by `open_hw` as a segment does: a display per decoder, no bound, and NV12 through swscale. After this change a copy is a file like any other: the same table, proof and layout, and its picture no longer depends on whether a device opened (its `SourceIdent` names the copy's file, not its decoder, so until then two machines may keep two pictures under one key: the lossy tier, never an export) |
| the copy's MAKING (`FfmpegCopies::make`: `Source::open`, the processor) | yes, `Purpose::Preview` | a long-GOP 4K file is what the device is for; its time is measured there and decides "slow" |
| Windows (D3D11VA, NV12/P010) | yes | the same layout; wine CI has no device → processor |
| FFmpeg 6/7/8/9 | yes | no new symbol; `AVHWFramesContext` is in every version module (t961) |
| The memory ledger | changed | tenant `Surfaces`, a fixed kind (D4); `memory-breakdown` appends its report-only kinds after it |
| The GPU arbiter | unchanged | the decode block is a separate engine; it shares memory bandwidth, not shader time; the trace will show if it matters |
| The export's encoder device | unchanged | its own context (D3) |

## D11. The setting (the owner's Q2 b)

- **The switch:** Preferences (`prefs_window.rs`), FFmpeg section, the checkbox "Decode video on the card",
  on by default. It follows the Economy checkbox's pattern:
  - `StartChoice.decode_on_card: bool` in `user_store.rs`, default `true`, loaded (`:847`) and saved (`:960`) with the rest;
  - the setter `set_decode_on_card` is added to t1294's list of the store's writers;
  - a `field_probe::note("prefs.decode_on_card")` and a hint line.
- **The bit:** a process-wide atomic in kt-ffmpeg, `set_decode_device(bool)` beside `set_decode_threads` (lib.rs), read into
  `DecodeSettings { device, .. }` by `opening_settings`. It is set from the store at start and on every toggle. `KT_DECODE_DEVICE=off|on` in the
  environment wins, for A/B runs and tests.
- **The effect:** read when a decoder is opened. A toggle reaches decoders opened after it; the player
  reopens its sources on the toggle (the same path as a returned file, `replace_media`). An export reads
  the bit at its start.
- **What the person sees:** the Diagnostics line, "Decoding on the card: N files; on the processor: M
  (why)", in `draw_diagnostics` beside the memory ledger lines. When the box is off it says "off in
  Settings".
- The choice cannot change a frame (D1), so it is a speed switch, not a look.

## D10. Stages

1. kt-ffmpeg: the layout (D1), the shared device (D3), eligibility and proof (D2), fault reopen (D6);
   the fake driver's mutex, perturb and fault modes. Tests t3300–t3304, t3306.
2. kt-media + kt-time + kt-engine: per-decoder kind in `FfmpegSource` under the ledger (D4), the factory
   opens for preview/export, thumbs stay software; t3129 tightened. Tests t3305, t3307, t3309, t3310.
3. Trace and report (D7), `probe --decode-check`; Diagnostics line per Q2. Tests t3308, t3311.

## Risks

- **Transfer cost on integrated chips**: `av_hwframe_transfer_data` on VAAPI maps the surface
  (`vaDeriveImage`) and reads it; on some drivers that memory is uncached and a 4K read takes tens of
  ms. Measured by D7 on the owner's machine; Q1 decides whether the default waits for it.
- **A non-conformant driver** passes the keyframe proof and deviates on inter frames: D2's
  `--decode-check` and a refusal table; a fault mid-stream is handled by D6.
- **The `yuvj` question across majors**: FFmpeg 7.1+ deprecates `yuvj*` outputs in some decoders; the
  layout follows what the processor's decoder of THIS major reports, never a fixed table (t3300 runs per
  installed major).
