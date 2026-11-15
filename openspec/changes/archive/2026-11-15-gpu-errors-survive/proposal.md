# Proposal: gpu-errors-survive

## Why

The owner, 2026-11-15 (`docs/ktprompts/kkt-26.11.15.txt`, row N1 of its registry; his words in
`docs/user-feedback.md`): kt26.11.10 crashed when he double-clicked the window title while a region
looped, "проект простейший даже без видео", and "очень часто оно что-то падать так начало"; he
remembers it also fell on a double click on the preview. His report
(`fails/26.11.14/1/kt-app-crash.txt`, build kt26.11.10-4275c0f8) is a panic on the UI thread:

    wgpu error: Validation Error — In Surface::configure
      Failed to wait for GPU to come idle before reconfiguring the Surface
    … egui_wgpu::winit::Painter::configure_surface ← resize ← window_event

wgpu-core's `Device::configure_surface` waits for the device to finish all submitted work and then
requires the queue to be empty; "it can only be non-empty if another thread is submitting at the same
time" (its own comment). Ours is: the preview's `present` composes on the eframe device from the
preview thread while the UI thread maximizes the window. The error goes to the device's error sink,
no `on_uncaptured_error` is installed on any device we create or receive, and wgpu's default handler
panics. The code of the current tree is the same.

The mechanism is a class: any wgpu error outside an error scope, on any device of the process, ends
the program. Frames run without scopes, the compute host runs most dispatches without one, and the
display device is shared with eframe, whose resize and present we do not drive.

## What Changes

- Every device the process creates or receives gets an uncaptured-error handler and a device-lost
  callback (`kt_gpu::errors::watch`): the error is counted per role, written to the program's log and
  telemetry journal (the first ones in full, then a count), and never panics.
- A frame's own error is its error: every engine entry point that composes (`execute*`, `present`,
  `present_kept`, `submit`, `collect`) runs inside thread-local Validation/Internal error scopes, and
  an error there returns `GpuExecError::Validation` — the existing "poisoned until the end of the
  session" contract (no picture and a notice in the preview, an error in an export) instead of a
  panic.
- The display device yields to a reconfigure: a `Surface::configure` that failed because the queue
  was busy holds the preview's presents for the next UI frames, so the reconfigure egui retries (the
  surface reports `Outdated`/`Suboptimal` at the new size) finds the queue idle. The window takes the
  new size during a busy loop, not only after it stops.
- A live scenario resizes and maximizes the window while a region loops; the program lives and the
  window has the new size.

## Capabilities

- `durability`: a GPU error never ends the program; the display yields to a reconfigure.
- `video-plan`: a frame's own GPU error is its frame's error.

## Impact

- `kt-gpu`: new `errors.rs`; `context.rs`, `compute.rs` install the handler; `engine.rs` wraps the
  entry points in scopes and holds the display's presents.
- `kt-app`: the eframe device is watched at creation; the error sink writes the journal; each UI frame
  ticks the display hold.
- `kt-cli`: the sink writes stderr.
- Tests t2720–t2725; found-defects rows 670–679 for what is left.
