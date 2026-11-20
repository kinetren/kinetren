# Proposal: stream-picture-stays

> DESIGN ONLY (spec session). Code starts after the owner answers the question at the end and the
> orchestrator gives the go. Tests reserved: t3812–t3823. Base: `orch-26-11-01` at `3420bf58`.

## Why

found-defects 663: the codec's picture still crosses the bus to the composite. A stream stage (Datamosh)
walks and decodes its history on the compute host's device (stream-history-on-device,
stream-front-on-device), but the composite runs on another device:
- the display's (eframe creates it);
- the offline engine's;
- an export's.

So every shown stream frame is read back as 8-bit planes (1.5 B a pixel), converted to RGBA on the
processor, and uploaded again as a layer. An animated generator drawn by a compute shader does the same.

On a card the two devices are the same chip, and on an integrated GPU even the same memory: the trip is
pure copying. stream-round-trips already cut every crossing INSIDE the walk (rows 401, 621). This one
remains only because the devices differ.

The compute-backends decision (`docs/features/plugins/kernels/compute-backends.md`; the owner's
acceleration scope, 2026-11-04) is plain WGSL compute through wgpu on Vulkan, DX12 and Metal: no CUDA,
no OpenCL. So the answer is not another API. It is ONE wgpu device per adapter, where the window's
backend can compute.

## What Changes

- **Measure first.** The trace gains the crossing: the bytes and the processor time of every stream or
  compute-generator frame read back and uploaded. `kt-cli trace … loop` sums them per lap.
- **One device per adapter where the display can compute.** On Vulkan, DX12 or Metal the program opens the
  window's device itself, through kt-gpu's one instance, with compute and the pass timer's features. It
  hands that device to eframe (`WgpuSetup::Existing`) and installs the same device as the compute host.
  On GL (no compute) the compute host stays a device of its own, and the crossing stays (named in the
  trace).
- **A held layer input.** A stream stage or a compute generator whose output lies on the composing
  executor's device hands the plan a held buffer instead of RGBA bytes. The executor converts the planes
  into the layer's texture on the device, with the processor's integer conversion, so the bytes are
  equal (invariant 11). On any other executor the buffer is read back as today.
- **An export on a card composes on the compute host's device** (one export engine), so its stream frames
  do not cross either. On a software adapter the export keeps its several devices: there the crossing is
  a processor copy anyway.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `video-plan`: one device per adapter for the window and the compute host; a held layer input.
- `export`: on a card the export's engine is the compute host's device.

## Impact

- **kt-app:** the window's device is opened by kt-gpu and handed to eframe (`app.rs`, `run`).
- **kt-gpu:**
  - `context.rs`/`compute.rs`: the compute host on an existing device;
  - `engine.rs`: the held layer input and its planes → RGBA pass;
  - `lib.rs`: `export_engines` on a card.
- **kt-engine:** `video_plan.rs` (`LayerInput` gains the held form), `video_stream.rs` (`run` asks the
  kernel for a held output).
- **kt-plugin-api:** `StreamKernel::render_held` (default `None`: the bytes path stays), and the same for a
  compute generator.
- **Invariants:**
  - 7: one executor; the conversion pass is the processor's integer arithmetic on the device;
  - 11: the bytes are equal either way;
  - 12: no work added at rest.
  - The arbiter's rings are already per adapter (gpu-arbiter), so the UI's frames and the compute jobs on
    one device are admitted by the same ring under its frame-period bound.

## Questions for the owner

**Q1. Should the window share its GPU device with the effects' compute work?**

Today the window draws on a device eframe makes, and the codec runs on a second device of the same card.
Each shown Datamosh frame crosses between them twice. How much that costs on your machine is not
measured yet; stage 1 measures it.

Sharing removes the crossing. The cost is robustness: a compute job that loses the device (a driver reset)
would take the window's device with it. Today such a loss poisons the preview's picture, but the interface
still draws and you can save.

- **a) Measure first; share only if your trace shows the crossing costs more than a tenth of a frame.**
  *(Recommended: your rule "caches where they pay" applies to copies too, and the robustness cost is
  real.)*
- **b) Share now** on Vulkan/DX12/Metal.
- **c) Keep the devices apart** and live with the crossing.
