## Context

kt-gpu runs fragment passes on the interface's device, which on GL has no compute. llvmpipe on Vulkan
does run compute (`tests/compute_mosh.rs`). A plugin depends on kt-plugin-api only, and the Datamosh
stage runs in the plan's common code, ahead of both executors.

## Goals / Non-Goals

**Goals:** a compute door any kernel can use; the Datamosh encoder on it with its CPU text deleted.

**Non-Goals:** moving other kernels; deleting the CPU composite; bundling a software Vulkan.

## Decisions

- **D1. The door is a synchronous job.** `ComputeHost::run(&Job)`: buffers, ordered steps, buffers read
  back. Rejected: exposing command encoders, which would leak the device model into every plugin.
- **D2. The host is installed once**, like the plugin set (`kt_plugin_api::compute::install`), because
  a stream kernel is called from the plan's common code and the host cannot be threaded through it.
  `host()` gives `None` when no device exists.
- **D3. The compute device is its own**: `Backends::PRIMARY` whatever `WGPU_BACKEND` says, the GL
  interface device has no compute. WARP is refused, as for the engine.
- **D4. Jobs on one device are serialised** by a mutex in the host: a frame takes one queue, and
  the row bands are the driver's business.
- **D5. Motion search in two sweeps.** The CPU search read the left and top vectors, which is a
  sequential order; the shader runs a sweep from zero, then a sweep taking the neighbours of the first.
  Quality held in the probe (46.01 dB both).
- **D6. Reconstruction stays in Rust for now.** `predict` and `reconstruct` also serve the broken
  decoder; the shader writes the residual, and the host-side Rust rebuilds the picture from it.

## Risks / Trade-offs

- DX12 and Metal were not run here: only Vulkan through llvmpipe was. A shader their compiler refuses
  becomes the visible "no compute" state, not a crash (t1503 holds the refusal).
- On a machine with no compute device Datamosh draws the frame untouched and says why.
