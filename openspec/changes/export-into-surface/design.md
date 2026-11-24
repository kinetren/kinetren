# Design: export-into-surface

## Context: what exists (read at orch `0c2a4065`)

- **The pass.** `yuv.wgsl` mirrors `kt_engine::yuv::convert` in integers (t3313). It writes a word texture of PACKED planes;
  `submit_as(plan, Some(layout))` (`engine.rs:2239`) copies it to a staging buffer of the ring, `collect` maps and copies.
- **The pipeline** (`export_pipeline.rs`). The compose thread submits and collects in order; `Job::Convert { planes }` goes to a frame
  worker that copies the bytes into an `AVFrame` (`fill_planes`); `write_all` writes in order on the muxer's thread.
- **The muxer** (`muxer_impl.rs`). A VAAPI candidate opens a device and a frames context (`initial_pool_size` 20, NV12: `pix10` of every
  VAAPI row is `None`, `encoders.rs:232`), and `encode_video_frame` (`:829`) does `av_hwframe_get_buffer` + `av_hwframe_transfer_data` per frame.
- **The trial** (`hw_trial.rs`). The cascade's VAAPI candidates meet a throwaway process of our own binary first.
- **The devices.** Every headless device takes `pass_time::wanted_features` (`:542`); the window's too (`kt-app/src/app.rs:11715`). An export on
  a card composes on one device (`export_engines`), which is the compute host's while it holds a stream's pictures
  (stream-picture-stays D4).
- **The fake VAAPI driver** (`fixtures/fake_va`). An H.264 encoder that writes every sample as I_PCM: a file decodes to exactly what the
  surface held; also a decoder of those files. Surfaces are `calloc` (NV12, pitch = width, no padding). It has no
  `vaExportSurfaceHandle`, no `vaAcquireBufferHandle`; its "render node" is a regular file. t1556 (the upload path, sixty frames), t1551–t1555 (the
  failures), live `hwencode`, t3129 (the decoder) run on it.

## D1. Whose memory: the encoder's surface is imported, not our image exported

The surface comes from FFmpeg's pool, the driver's own allocation for this encoder: the tiling, pitch and height
alignment its encoder wants are in it by construction. It is mapped to a DRM descriptor and imported by the composing device.

Rejected: *our VkImage exported and wrapped by `vaCreateSurfaces` (DRM_PRIME_2)* (export-host-copy D6's sketch): the modifier must be one
BOTH accept, chosen by us from two lists, and an encoder may refuse a layout its own allocator never makes (VCN
wants its pitch alignment; Intel's encoder its Y tiling). wgpu-hal imports a dma-buf and does not export one.
*FFmpeg's Vulkan encoders* (`h264_vulkan`, FFmpeg 7.1+) on our device: FFmpeg would need our VkDevice with its video queue and extensions,
which wgpu does not open; a different encoder than the one the owner's renders use. *A linear surface as a storage buffer*:
no driver promises a linear input surface.

## D2. From a pool's surface to two textures

`kt_ffmpeg::Muxer::surfaces() -> Option<SurfacePool>` after the open (VAAPI candidates only). `SurfacePool` is `Send`, holds its own
reference of the frames context and the mapping function:
- `acquire() -> Result<Option<Surface>, FfError>`: `av_hwframe_get_buffer`; `None` when the pool is empty (`AVERROR(ENOMEM)`). FFmpeg's
  buffer pool is thread-safe, so the compose thread acquires while the writer's thread encodes. `Surface` owns the `AVFrame`; its id is
  `data[3]`.
- `describe(&Surface) -> Result<DmaBuf, String>`: `av_hwframe_map(dst: AV_PIX_FMT_DRM_PRIME, flags WRITE | DIRECT)`, read `AVDRMFrameDescriptor`,
  `dup` each object's fd, unref the mapped frame (it would hold the surface out of the pool for ever). Two layers of one
  plane each, `R8` and `GR88`, are taken; anything else is the reason "the driver exports N layers, M planes".
- `av_hwframe_map` sits in an optional table of its own (`declare_optional_api!`, one name): the present table fails whole on one missing
  name, and a missing MAP must not take VAAPI encoding away. `hwcontext_drm.h` (no system header inside) joins build.rs
  `HEADERS`. Windows compiles the same code and never reaches it.

`kt_gpu::surface::import(&DmaBuf)`: per plane, `device.as_hal::<Vulkan>()` → `texture_from_dmabuf_fd(fd, desc, modifier, stride, offset)` →
`create_texture_from_hal`, formats `R8Uint` at `w×h` and `Rg8Uint` at `w/2×h/2`, usage `RENDER_ATTACHMENT`. Uint targets because the pass is exact:
a unorm target's rounding is the driver's. Imported ONCE per surface id for the muxer's life (the pool reuses them; an
import per frame would be an allocation per frame). Dropped before the frames context: the pool object owns both, in
that field order, and waits for the device first.

## D3. The pass and the two barriers

`yuv.wgsl` gains `fs_y` and `fs_uv`: the same `luma()` and `chroma()`, one sample (two for chroma) per fragment, target `u32`. `fs` stays (planes for
every other encoder). Two small draws with the picture's viewport; a surface taller than the picture keeps its padding
as the upload leaves it. Two `Compiled` pipes, warmed only when an export asks them (`prewarm::effect_ids` untouched).

wgpu tracks the imported texture as its own: it does not know the encoder reads it. Two command buffers of our own go
around the frame's, in one `queue.submit([acquire, frame, release])`, each an encoder that does nothing else (`as_hal_mut` and wgpu's own
recording may not be mixed in one):
- acquire: queue family EXTERNAL → ours, layout UNDEFINED → COLOR_ATTACHMENT (the whole picture is drawn, its past is nobody's);
- release: COLOR_ATTACHMENT → GENERAL, ours → EXTERNAL, making the writes available: without it a driver may leave them in
  a cache the video engine does not read.
EXTERNAL is core Vulkan 1.1; FOREIGN would need an extension wgpu does not enable and Mesa treats the same. The wait is on the
processor, as today: `done(ticket)`/`collect` see the submission finished before the surface goes to the encoder. No semaphore, no
implicit fence is relied on.

## D4. The order of a frame

`VideoGpuExec::submit_surface(plan, layout, &ImportedSurface) -> ticket` and `collect` with no bytes; `surface_depth()`.
- The compose thread: `acquire` → `submit_surface` → in flight with its `Surface`. `acquire` empty: collect the oldest in flight; none in flight: wait
  on the writer's `Gate` (it moves when the encoder took a frame), never a clock.
- At most `SURFACE_RING = 8` in flight; the frames context is opened with `20 + 8` surfaces where surfaces are used, 20 as
  today otherwise: what the encoder keeps (its lookahead, B-frames) is what the 20 were for.
- `Job::Surface { t, surface, complete }` passes through the same worker queue, so the order rule (`order_tx`) does not change; the worker forwards it.
  `Done.frame` is `Frame | Surface`; `ExportSink::write_surface(Surface)` sets its pts and encodes.
- **An incomplete frame** (the RETRIES rule): composed again into the SAME surface before it is handed on, as today into the same buffer.
- **The serial export** (`export.rs`) does the same with one surface at a time, so t1451 (pipelined = serial) holds.
- **An empty plan** draws black through the pass (there is no readback to fill by the reference).

## D5. When it is used

Asked once per export, after the encoder opened; the first "no" is the reason, in the log and the trace:

| # | Condition | The reason otherwise |
|---|---|---|
| 1 | Linux, a VAAPI encoder, NV12 | (silent: another path) |
| 2 | the composing device is Vulkan, a card, with the dma-buf feature | "the device <name> takes no dma-buf" |
| 3 | one composing device (no software workers) | "composed on the processor" |
| 4 | the same card: the adapter's `device_pci_bus_id` is the node's (`/sys/class/drm/renderD…/device`) | "composes on <pci>, encodes on <pci>" |
| 5 | the surface maps to two one-plane layers with a known modifier | "the driver exports …" |
| 6 | the device can render to `R8Uint`/`Rg8Uint` of that modifier, one memory plane (`vkGetPhysicalDeviceImageFormatProperties2`, asked through the adapter's hal handle; an image created unasked is undefined behaviour, not an error) | "no render to modifier 0x…" |
| 7 | the trial process passed (D6) | its reason |
| 8 | the proof passed (D6) | "the surface read back differs (N samples)" |

`KT_EXPORT_SURFACE=off` takes the upload path (A/B runs, t4127). There is no setting: like the conversion, it changes no byte.

## D6. The proof and the trial

**The proof**, in the export's own process before its first frame: two pictures (a ramp, then its complement: a stale surface of the
first cannot pass the second) go through `import` → `fs_y`/`fs_uv` → release → wait, and the surface is read by `av_hwframe_transfer_data` into
host NV12 and compared with `yuv::convert`, every sample. Cost: two frames. It proves the layout, the modifier, the offsets and that
the driver's own read sees our writes. It does NOT prove that the encoder's engine sees them (it may read through
another cache than the driver's image copy): that is D3's release barrier and the owner's card (D12).

**The trial** (invariant 9's one exception, widened): the process that meets a libva driver now may import and draw too. `hw_trial`'s
third kind, `surface <node> <w> <h>`, opens the device, the pool, one wgpu device, imports and runs the proof, and says it. It runs after conditions
1–6 hold, at most once per export, within the existing 20 s; a fault, a hang or a refusal there is condition 7. kt-ffmpeg does not
know kt-gpu: `hw_trial::serve_with(Host { surface: fn(..) })`, called by the two binaries with kt-gpu's function; a process that does not
host it (a test harness) proves in-process, as the encoder's trial does.

## D7. What fails midway

`acquire` or `submit_surface` failing after frames went by surface: that frame and the rest go by the upload path (the encoder takes VA
surfaces either way, so the stream does not notice), the log says from which frame and why. A device error (`Validation`,
`DeviceLost`) follows the export's present rule. A pool with nothing to give and nothing in flight, with the writer ended: the export's
own error, as a stopped worker is.

## D8. The disk prerender

`render_cache`'s writer (`segment.rs` `Writer`) is the same muxer: `SegmentWriter::surfaces()`, and `Renderer` submits by surface where the preview's device
passes D5. Its trial is skipped when a render's or an earlier segment's verdict for (device, node) stands in this process: the
prerender opens a muxer per segment, and a throwaway process per ten seconds of film would be what it saves. An export
keeps "no cached verdict". At rest the writer then neither maps nor copies; the durable key does not know.

## D9. Memory

The pool is 28 NV12 surfaces: 87 MB at 1080p, 350 MB at 4K, of which 20 are there today and counted by nobody. With this way the staging
ring (up to 256 MB, `Readback`) is not allocated. The pool is reported in `Tenant::Surfaces` (device-decode-sources D4; whichever of the
two is built first appends the tenant) by its real bytes; the decoders' bound does not apply to it.

## D10. Determinism

The encoder receives the same NV12 samples whichever way they came (D6's proof per export, t4127). Two runs on one
device stay equal (invariant 11). A padded row below the picture may differ between the ways; the encoder crops it, and the upload
path already leaves it unwritten.

## D11. What the trace and the log say

`ExportStage` gains the kind `SURFACE` (frames by surface, frames by upload); copy out, convert and upload per frame are then zero, and `trace … export` prints
"handed to the encoder on the card: N of M; by upload: K (why)". The render window's log has the same line
(`ExportLog::Surface { by_surface, by_upload, why }`), i18n in every catalog; no new control, no window change.

## D12. What cannot be proved without a card (C11)

On the build machine: libva, the real FFmpeg, our fake driver; Vulkan is lavapipe; `/dev/udmabuf` (the only way to make a
dma-buf with no GPU) belongs to group kvm, which the build user is not in; there is no render node.
- **Proved here:** the pass's bytes (t4125); the descriptor's reading and every refusal (t4126); the whole protocol through real
  libva and FFmpeg with the import replaced by a copy into the exported memory (t4127–t4129, t4131, t4132); the trial's death and the
  fallback (t4130).
- **Proved only where a dma-buf exists**, opt-in and loud like t963 (`KT_EXPECT_DMABUF=1` fails instead of skipping): the import itself on lavapipe
  with udmabuf (t4135). Whether lavapipe of Mesa 25.2 offers the feature then is not known (no `vulkaninfo` here); a CI runner may have
  it with `sudo modprobe udmabuf`: the task tries and says.
- **Proved only on a card:** real modifiers and pitches of his driver; condition 6's answer; that the ENCODER sees the writes (D3's release); the
  gain. `make hw-test` (t963 with `KT_EXPECT_HW=vaapi`) on his machine then compares the decoded file with the frames and prints which way
  they went. C11's text for `owner-checks.md`: "Render a region on the card and send the trace. The log's last lines say how many
  frames were handed to the encoder on the card, or why not. Look at the result: torn blocks or old frames inside new
  ones would be the one defect the program cannot see itself."
- **The gain is not promised**: on his trace (kkt-26.11.18 T3) the copy out was 2.2 ms a frame, and the conversion has left the processor since. What
  this removes is the map, two memcpy of 3.1 MB and an upload per 1080p frame; the trace of C11 says what it was worth.

## D13. Class walk — a frame that goes through host memory between the GPU and a codec

| Place | Today | Here |
|---|---|---|
| Export → `*_vaapi`, one card | readback, 2 copies, upload | **this change** |
| Export → `*_vaapi`, a hybrid laptop (composes on the discrete card, encodes on the chip) | the same | upload stays (condition 4); composing the export on the encoder's card would be slower at the composite: not this change |
| Export → `h264_qsv`, `hevc_qsv` (Linux: libva below; Windows: D3D11) | NV12 host frames, the library uploads | not here: a QSV frame can wrap a VA surface (`AV_HWDEVICE_TYPE_QSV` derived from VAAPI); the cascade tries VAAPI first on Linux, so nobody reaches it with a working VAAPI |
| Export → `*_nvenc` | host frames, the library uploads | not here: CUDA through `VK_KHR_external_memory_fd`; wgpu-hal has no export of an opaque fd; no NVIDIA machine asked |
| Export → `*_amf`, `*_mf` (Windows) | host frames | not here (Q19: Windows later): `VULKAN_EXTERNAL_MEMORY_WIN32` is there, a D3D11 frames pool would be the twin of D2 |
| Export on software workers, software encoders | planes to host | by nature |
| Disk prerender's writer | as the export | **this change**, D8 |
| `persistent-cache` copies (`copy.rs` `make`, branch `persistent-cache`) | decode → host RGBA at the copy's size → reference YUV → upload | not here: the picture never is on our device (decode on the processor or a decode device, scale by swscale). With `device-decode-sources` it would be surface → host → surface; a VAAPI scale (VPP) would be another filter. A row for after both |
| Source decode (`device-decode-sources`) | surface → host → upload as a texture | its D8 rejects the import now (the burst cache keeps host frames); **this change's `surface.rs` is the code it would need**: the same import, sampled instead of drawn |
| Disk tier's segments, read (`segment.rs` reader) | decode surface → host → texture | as source decode |
| Datamosh and VHS (our codec on the compute device) | stays on the device (stream-picture-stays) | no codec of FFmpeg in it |
| A held picture crossing devices (`held.rs`, `stream_cross`) | device → host → device on two devices | one device since stream-picture-stays; unchanged |
| Poster, thumbnails, filmstrip, reel strips on disk | one picture, host | not worth a surface |
| Export's sound | — | — |
| `kt-cli preview`, live screenshots | readback by design | — |

## D14. The order of work

1. `fs_y`, `fs_uv` and their test (no card, no FFmpeg). 2. The pool and the descriptor (kt-ffmpeg, the fake driver). 3. The import, the
barriers, the proof, the trial. 4. The pipeline and the serial export. 5. The prerender. 6. The trace, the log, C11.

## Risks

- **A driver that passes the proof and still encodes stale pictures.** It would be seen only by eye (C11 says so). `KT_EXPORT_SURFACE=off` is the
  way back on his machine; if it is ever seen, a (vendor, driver) refusal goes into condition 2 with its reason.
- **`as_hal` discipline.** `surface.rs` is the one place (besides `holds.rs`) that touches hal; a wgpu upgrade recompiles it, and t4135 is its only
  real-import test. The rule goes into kt-gpu's CLAUDE.md.
- **The window's device gains a feature** (`wanted_features`): a feature bit asked of the adapter that has it; wgpu-hal enables the extensions
  anyway. The GL path never has it.
