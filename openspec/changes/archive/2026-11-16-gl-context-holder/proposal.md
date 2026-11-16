# Proposal: gl-context-holder

## Why

The owner's class N1 (kkt-26.11.15 item 1, `docs/user-feedback.md`): a busy GPU must never end the
program. gpu-errors-survive made every wgpu error a journal line; a hard wgpu-hal panic is outside
it. In the stage-15 finish.sh (load 10–45) the first tries of live `render`, `hwencode` and
`renderregion` died at the same second in three processes on three virtual screens: egui's paint on
the main thread, `wgpu-hal` gles `AdapterContext::lock` — "Could not lock adapter context. This is
most-likely a deadlock." (the 6 s `CONTEXT_LOCK_TIMEOUT_SECS`), then a panic in a destructor and an
abort; `readme_shots` of the same run showed a preview lit 0 %. Found-defects 651 had seen it once.

On GL every wgpu-hal call takes the adapter's ONE context lock and holds it through the whole call: a
pipeline's GLSL link, a variant compile at a draw, a `client_wait_sync` of up to 2.1 s, a Mesa
disk-cache read. The window's device was shared: the preview's compose thread, the display engine's
build (`engine-build`) and the warm worker all called into it beside egui's paint. The runs' journals
and traces show no CPU from any thread of the three processes for the eleven seconds before the
panics — the holder was stalled by the system (a disk stall in a Mesa cache read under the lock is the
fitting cause: the same second in three processes), not computing. Any holder of ours stalled past
six seconds ends the program; shortening our pieces cannot bound a stall the system makes.

## What Changes

- On GL the window's device is the UI thread's alone (`kt_gpu::display_is_ui_only`): the preview
  composes on a headless GL device of its own (one per process, an engine `sibling` per project, its
  own context lock) and takes the bytes path; the display engine is no longer built on the window's
  device there. Vulkan, DX12 and Metal keep the display path (no context lock).
- Who is inside a device call is recorded (`kt_gpu::holds`): one wgpu call (a pipeline creation, a
  submission, a blocking wait) and the entry point around it, by thread, role and operation. A long
  call on the window's device is a journal line; the editor's panic report lists the calls in flight.
- The bench's `gpu_busy` (resize_loop) stands in for nothing on GL and does nothing there; the bench's
  `gl_stall` holds the preview device's GL lock as a stalled call would (live `gl_stall`).

## Capabilities

- `durability`: on GL no thread but the UI's calls into the window's device; a GL call of ours stalled
  for longer than wgpu-hal's six seconds costs preview frames, never the program.
- `video-plan`: the preview's executor on GL is an engine off the window's device (bytes path).

## Impact

- `kt-gpu`: `holds.rs` (new), `lib.rs` (`display_is_ui_only`, `preview_engine_off_display`, the exit
  park), roles on `GpuFxEngine`/`Compiled`/`Kit`, holds at every submission, pipeline creation and
  blocking wait.
- `kt-app`: `init_video_gpu`/`take_video_gpu` route GL to the engine off the display; the dump's
  `compose.ui_only`; the panic report's "GPU calls in flight"; bench `gl_stall`.
- Live: `gl_stall` (new), `gpufx` and `resize_loop` know the GL route.
- Tests t2740–t2743; found-defects 651 closed, rows 680–682 for what is left.
