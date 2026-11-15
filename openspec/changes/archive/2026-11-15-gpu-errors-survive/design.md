# Design: gpu-errors-survive

## Context

The owner's kt26.11.10 died on a double click on the window title while a region looped (kkt-26.11.15
N1, `fails/26.11.14/1`). The report: a panic on the UI thread in `wgpu::backend::wgpu_core::
default_error_handler` ← `CoreSurface::configure` ← `egui_wgpu::winit::Painter::configure_surface`,
"Failed to wait for GPU to come idle before reconfiguring the Surface".

wgpu-core 30 `Device::configure_surface` runs `maintain(wait_indefinitely)` for the last submission
known at the call, and treats `WaitSucceeded` (the wait ended but newer submissions are in flight) as
`GpuWaitTimeout` — "it can only be non-empty if another thread is submitting at the same time". The
preview's thread presents on eframe's device (`GpuFxEngine::present`, the display engine built by
`new_kept`), so a resize during playback meets work submitted while it waited. `CoreSurface::configure`
hands the error to the device's error sink; with no `on_uncaptured_error` installed wgpu panics.

Reproduced live (`resize_loop`, Vulkan lavapipe on Xvfb) with the window's device kept busy from a
thread of its own (the bench's `gpu_busy`, a stand-in for the preview's thread under a heavy loop): on
the old code the first maximize panicked at the same line (`wgpu_core.rs:3981`, the supervisor's crash
window opened); with this change the journal holds the refused reconfigure and two following errors,
and the window takes its size at the next paint.

## Decisions

### D1. Every device is watched; nothing panics on a wgpu error

`kt_gpu::errors::watch(device, role)` installs `on_uncaptured_error` and `set_device_lost_callback`.
An error is counted per role and written through the program's sink (`set_sink`: the editor writes
`journal::log("GPU", …)`, i.e. `kt-app.log` and the telemetry journal; kt-cli, with no sink, stderr).
The first 20 per role in full, then every hundredth with its number: a surface refused every frame
must not flood the journal. A lost device that was not destroyed by us is written too.

Installed where every device comes from: `context::headless_with` (`headless`/`export`),
`context::new_d3d12`, `compute::open_device`, `GpuContext::from_parts` (the display device, also
kt-cli's `firstframe`), and in kt-app's eframe creation closure on `cc.wgpu_render_state` — before the
first resize, before the preview's engine exists.

Rejected: catching the panic (`catch_unwind` around eframe's event handling) — the error is raised
inside winit's callback; unwinding through it is undefined, and egui's state would be torn.

### D2. A frame's own error is its error

Every engine entry point that composes runs inside thread-local Validation and Internal scopes
(`errors::scoped`, `Inner::guarded`): `execute_measured`, `execute_into`, `execute_once`,
`execute_held`, `present`, `present_kept`, `submit`, `collect`. wgpu 30 keeps the scope stack per
thread with `std`, so another thread's error (egui's on the UI thread) never lands in a frame's scope.
An error becomes `GpuExecError::Validation(text)` — the contract the player already has: poisoned
until the end of the session, no picture and one notice in the preview, an error in an export.
`present_kept` answers `false` (nothing shown, as a miss), and the next composed frame reports.
Scopes nest: the warm draws' and `memo_keep`'s own scopes stay innermost.

### D3. The display yields to a refused reconfigure

The handler of the display role sees a `Surface::configure` error and sets a hold of two UI frames;
`ui()` spends one per frame (`errors::display_frame`). The display engine's entry points, and anything
else that submits on the window's device off the UI thread (`errors::yield_to_display`), wait while
held, at most 250 ms (the UI may stop ticking). egui 0.36 retries by itself: after a refused configure
the surface keeps the old size, the next `get_current_texture` answers `Suboptimal`/`Outdated`, and its
default `on_surface_status` reconfigures; held, the queue is idle then. Without the hold a heavy loop
could refuse every retry, and the window would not take its size until the loop stopped.

On GL the surface never reports `Suboptimal`/`Outdated`: after a refused configure egui draws the
new size into the old surface (scissor errors on submit, written, the frame lost) until the window
manager's next configure event; measured in `resize_loop` on GL, the bursts end within a second of
each toggle and the screenshots show the full new size. eframe 0.36 has no way to ask its painter for
a reconfigure (`Frame::set_wgpu_surface_config` changes the frame's copy only), so that second is
left (found-defects 672).

Rejected: a lock between the UI thread and the preview's thread around eframe's resize — eframe calls
`configure_surface` inside its winit callback, before our `ui()`, and offers no hook there.

### D4. A live scenario that meets the race

`resize_loop` (t2724): the demo project (generators only, no video, as in the owner's report), a 2 s
loop, the bench's `gpu_busy 60`, six double clicks on the title strip's free part (`title.drag`, the
owner's gesture) and four resizes; the program lives, the window takes each size, and the journal
holds at least one refused reconfigure — the scenario proves it met the race, so a green run is not a
quiet one. Its own process: a maximized window must not reach the stand's next scenario.

## The class, walked

The mechanism: a wgpu error outside every error scope, on a device with no uncaptured-error handler,
reaches wgpu's default handler and panics. Every device of the process and every path that submits:

| Device / path | Before | Now |
|---|---|---|
| eframe's display device: surface configure on resize/maximize (UI thread) | panic (N1) | written; display hold; egui reconfigures next paint |
| display device: egui's own frame at the new size into the old surface after a refused configure (scissor rect outside the target, `Queue::submit`) — found by `resize_loop` | panic | written; the next paint reconfigures |
| display device: the preview engine's `present`/`present_kept`/`execute*` (preview thread) | panic on any frame error | scoped → `Validation` → poisoned (contract); yields to a hold |
| display device: the detached preview's viewport surface | panic on its resize | same handler and hold (one device) |
| offline engine and `side_engine` (thumbnails, posters, reel strips, loop fill, slow route) | panic | watched (`headless`), frames scoped |
| export engines (`export_engines`, render dialog, kt-cli export) | panic | watched (`export`), `submit`/`collect`/`execute_once` scoped → export error |
| shadow device (`open_shadow`, warm draws) | warm draws scoped already; rest panic | watched (`headless`) |
| compute host (`compute::open_device`): codec/generator jobs | creation and some dispatches scoped; the rest panic | watched (`compute`); an unscoped dispatch error is written, the job goes on (row 670) |
| held frames on the compute device (`front_on_compute`) | panic | `execute_held` scoped; device watched as compute |
| D3D12 compile check (`new_d3d12`) | pipeline scopes; rest panic | watched (`d3d12`) |
| kt-cli devices (probe, preview, pass-cost, fuse-cost, gpu-share, firstframe) | panic | watched through the same openers; stderr |
| test engines (`test_engine`, testkit) | panic | watched (`headless`) |
| device lost (driver reset, TDR) | silent; frames saw `DeviceLost` on map | written; frames as before |
| `get_current_texture` statuses | egui's default: reconfigure / recreate / skip | unchanged (no panic path) |

Not this class (Rust-side assertions of wgpu, e.g. a buffer slice out of range): a programming error,
not a device error; none known.

## Risks

- A frame poisoned by an error it did not cause: the scopes are thread-local, so only the frame's own
  thread's errors land there. The preview engine is the one engine whose device another thread uses
  (egui), and egui's errors go to the handler.
- The hold delays the preview's frames by at most two UI frames (or 250 ms) after a refused reconfigure.
