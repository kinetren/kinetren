# Design: stream-picture-stays

## Context

- **The compute host** (`kt-gpu/src/compute.rs`, `ComputeDevice`) is a wgpu device of its own, opened
  through kt-gpu's one instance, on the primary backend. It owns:
  - the stream walk;
  - the front's engine (`stream_front_engine`, a full `GpuFxEngine` on the compute device —
    stream-front-on-device);
  - the held buffers (`kt_plugin_api::compute::Held`, `ComputeHost::run_keeping`);
  - the pool and the pipeline cache;
  - a ring registered with the arbiter for its adapter.
- **The window's device** is created by eframe (`app.rs` `run`: `WgpuSetup::CreateNew` with kt-gpu's power
  preference and the pass timer's features). On GL it has no compute, and on GL nothing but the UI thread
  touches it (gl-context-holder).
- **The composite** runs on the display's executor, the offline engine or the export engines
  (`export_engines`: several software devices, or one on a card). render-ahead put the producer on the
  display's own device on a card.
- **The crossing** (`video_stream::run`): `kernel.render(.., &mut buf)` reads the output planes back,
  converts them row by row to RGBA (stream-round-trips D5), and the layer uploads them as an ordinary
  `LayerInput::Frame`. A compute generator (`kt-plugin-api` generator on the compute host) is the same.
- **The decision this ties to:** compute-backends' verdict. wgpu reaches every vendor through Vulkan, DX12
  and Metal; no CUDA or OpenCL; one text per kernel. The owner's acceleration scope (2026-11-04): all math,
  plain WGSL compute, ARM needed. One device per adapter keeps that: no new API, the same WGSL.

## D1. The crossing in the trace (evidence; Q1 b: not a gate)

A `StreamCross` trace record per frame read back for a layer: bytes, processor microseconds of the
row conversion, upload bytes, the kernel or generator id. `kt-cli trace <dir> loop` prints per lap: "stream
frames crossed: N, X MB, Y ms of a core". It proves the crossing gone (t3821); the owner chose to share without waiting for it.

## D2. One device per adapter

When the window's backend has compute (Vulkan, DX12, Metal):

1. `kt_gpu::GpuContext` opens the display adapter's device with the features the compute host and the pass
   timer need (`TIMESTAMP_QUERY`, `PIPELINE_CACHE` where the adapter has them) and the limits the compute
   kernels ask.
2. `run` hands it to eframe as `WgpuSetup::Existing { instance, adapter, device, queue }`, so the window
   draws on it.
3. `compute::open` installs a `ComputeDevice` over that same device and queue, with its own pool, pipeline
   cache and timestamps, and the same arbiter ring (the ring is per adapter already).

**As built (stage D2):** eframe keeps creating the window's device (`WgpuSetup::CreateNew`, its descriptor
already asks the pass timer's `TIMESTAMP_QUERY`/`PIPELINE_CACHE`, the same features the compute host asks),
so its less travelled `Existing` path is not needed. In the app's creation closure, before anything can open
the compute host, `kt_gpu::compute::share_display` wraps the window's device as the process's one
`GpuContext`; where `share_refusal` allows (not GL, and the window's adapter is the one `compute_pick`
would open) the host opens over that same `Arc` (`ComputeDevice::on_context`), and the preview's engine is
built on it too (`init_video_gpu`), so `held::on`'s pointer compare holds. The trace's machine fact
`gpu.compute.shares_display` says `yes` or why not. t3817, t3818.

On GL, or when the compute host's adapter differs from the display's (a hybrid laptop whose window is on
the integrated chip while the compute picks the card), nothing changes: two devices, and the crossing.
The trace's machine facts say which ("one device per adapter: yes/no, why").

**Robustness (Q1):** a device loss on the shared device poisons both the preview and the compute host for
the session (the existing DeviceLost rule, t185). The interface keeps drawing as long as eframe's surface
can be reconfigured. If it cannot, the close path's durability steps still run (close-while-busy). The
recovery file and Ctrl+S do not need the GPU.

## D3. A held layer input

**Found while building (2026-11-20):** the crossing is not only in `render`. The codec's walk reads every
SHOWN position back into the memo as host planes (`walk::keep` → `read3`, `Put::Lru`), so the prefetch
thread's walk ahead already crosses for each frame it will show, and `render` then converts the memo's host
planes to RGBA. Holding the picture means the shown positions stay in the memo's live tier as `HeldPic`
(device memory, `LIVE_BYTES`) where the host shares the window's device, and the bytes path reads one back
only when an executor on another device asks.

- `LayerInput` gains `Held(HeldLayer { held: Held, w, h, form: Planes8 | Rgba8, content })`. `content` is
  the same content key the bytes path puts on the layer (the kernel's output key), so every memo key is
  unchanged.
- `StreamKernel::render_held` (default `None`) returns the output that is already on the compute host:
  Datamosh's decoded planes after the walk. A compute generator gets the same method.
- `video_stream::run` asks it first when the plan's executor `accepts(&held)` (the held buffer's device is
  the executor's: an `Arc` pointer compare inside kt-gpu). Otherwise it uses `render`, as today.
- The executor turns `Planes8` into the layer's RGBA texture with one compute or render pass doing the
  processor's row conversion in integer arithmetic (the same chroma upsampling and matrix as
  stream-round-trips D5). t3815 holds the two byte-equal.
- **What does not change:** the plan's structure, `FramePlan::starved` (a held layer is present), the
  probe keys, the durable keys (`durable_hash` hashes `content`, not the form), the coverage bars, and
  kept moments.

## D4. The export on a card

`export_engines` on a card already builds one engine. When one device per adapter holds (D2) or the export
is headless (kt-cli), that engine is built on the compute host's device, so the pipelined export's stream
frames take the held input too. On a software adapter the several devices stay (render-honest D1). Each is
a processor-memory device, and the crossing there is a memcpy (D1 measures it there too).

## D5. Class walk: every picture that leaves the device it was made on to be composed

| Producer | Made on | Composed on | After |
|---|---|---|---|
| a stream stage's frame | compute host | display / offline / export | held where one device (D3); else as today |
| a compute generator's frame | compute host | same | the same (D3) |
| the front's output | compute host's engine | the codec on the compute host | already held (stream-front-on-device) |
| a render-ahead composite | the display's sibling | the display's memo | already on the device (render-ahead) |
| the slow route's composite | the offline device | the display's memo | crosses once per kept moment (ahead-kept); out of this change |
| a prerendered disk moment | decoded on the processor | display | an upload; out of this change (disk-bytes-once covers its count) |
| a decoded source frame | processor (or device decode, device-decode-sources) | display | upload; device-decode-sources' own question |

## D6. Stages and tests

1. D1, the crossing in the trace (t3812, t3813).
2. D3, the held input on one shared test device:
   - a stream layer and a compute generator composed held equal the bytes path (t3814);
   - the planes → RGBA pass equals the processor's conversion on every size parity (t3815);
   - a foreign executor reads back (t3816).
3. D2, one device per adapter:
   - the compute host over an existing device (t3817);
   - the app's setup picks one device on Vulkan and two on GL (t3818);
   - a device loss poisons both and the interface still saves (t3819).
4. D4: the export on a card uses the compute host's device, and its frames equal the two-device export's
   (t3820).
5. The measured gain: `kt-cli preview` over a Datamosh loop, crossing bytes 0 with one device (t3821); the
   rest check with a Datamosh loop paused (t3822).

Spare: t3823.

## Risks

- **eframe's existing-device path is less travelled** than `CreateNew`. The surface's format and present
  modes are eframe's choice either way. t3818 runs the setup headless with a surface-less device.
- **One queue for the UI and the compute walk:** a long walk submission can delay a UI frame. The arbiter's
  frame-period bound applies to compute jobs (gpu-arbiter), and stage 3 measures UI frame time during a
  walk on the heavy showcase.
