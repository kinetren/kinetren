# Design: gl-context-holder

## Context

Stage 15's finish.sh: live `render`, `hwencode` and `renderregion` died at 00:05:19 within the same
second, three processes on three Xvfb displays, all in egui's paint on the main thread waiting for the
GL adapter context (`wgpu-hal-30.0.0/src/gles/egl.rs` `AdapterContext::lock`, `try_lock_for(6 s)`,
"Could not lock adapter context"), panicking again in a destructor, then aborting. Their journals hold
no load line from 00:05:07.7 to 00:05:19 and their traces no event after +56.9 s: no thread of ours
used the processor while the UI waited. hwencode's UI drew its last frame at +1.2 s. The thread
lists at the crash name `engine-build` (twice), `preview-compose`, `kinetre:disk$0` (Mesa's disk-cache
queue) among the usual ones.

wgpu-hal gles holds the context lock through every call: `Queue::submit` (and with llvmpipe the
variant compile of a first draw inside it), `create_render_pipeline` (the GLSL link, a Mesa disk-cache
lookup — a synchronous file read), `Device::wait` (`client_wait_sync` up to `i32::MAX` ns, ≈2.1 s a
call, looped by wgpu-core for an indefinite wait), `flush_mapped_ranges`, fences. The window's device
had callers besides egui: the preview's compose (`present`, `wait_presented`), the display engine's
build on `engine-build` (`new_kept` → `Compiled::compile`, every pipeline), the warm worker
(`pipeline-warm`: warm draws with a blocking wait, shapes, run shapes). One of them stalled with the
lock — by a disk stall under a Mesa cache read most plausibly (three processes at one second, no
processor time) — and the UI's next paint died six seconds later.

Measured here (`holds`, below): under 28 busy processes beside the four scenarios (load 45–54) the
window's device was held 3.2 s by `engine-build` in its pipeline compile, in many calls; no single call
past six seconds in four loaded runs, no crash reproduced by load alone. The mechanism is
reproduced deterministically instead: `gl_stall` holds the preview device's lock for nine seconds;
with the preview on the window's device (the old route) the UI's paint panics with the owner-class
message at six seconds, with this change the UI draws 120+ frames through the stall.

## Decisions

### D1. On GL the window's device is the UI thread's alone

`kt_gpu::display_is_ui_only(info)` is true exactly on GL. There `init_video_gpu` builds no engine on
eframe's device: `kt_gpu::preview_engine_off_display()` opens one headless GL device per process (its
own instance, so its own EGL context and lock — segv-crashes' `own_instance`), builds an engine on it
once, and hands each project a `sibling` (its own frame state, nothing compiled again). The preview
takes the bytes path (`preview_display` stays `None`, as `KT_PREVIEW_READBACK=1` did). A stalled GL
call of ours now waits on the preview device's lock: preview frames stop, the UI draws on. The exit
parks that engine with the offline ones (a frame of llvmpipe at exit).

Rejected: bounding our pieces on the window's device — a stall the system makes (disk, X server,
memory reclaim) is not bounded by the size of our work; the variant compile of one draw took 10 s on
an empty Mesa cache by itself. Rejected: catching the panic — it is raised inside winit's callback.

Cost: on GL each new preview picture is read back on the preview device and uploaded by the UI thread
(the bytes path's cost that preview-throughput D2 removed for cards). GL is the fallback of a machine
without Vulkan and the live suite's default; the owner's machine runs Vulkan. Live `idle` (load at
rest) is green; `cached_range` and `loop` (playback) green under load.

### D2. Who holds a device is recorded

`kt_gpu::holds::hold(role, op)` registers ONE wgpu call (pipeline creation in `render_pipe`, every
`queue.submit` of the engine and the warm kit, the blocking waits of the readback and the warm draw);
`within(role, op)` registers an entry point's span (`present`, `execute*`, `submit`, `collect`, the
pipeline compile, a warm draw, a shape compile) for context. A call on the window's device lasting
past one second is written to the journal when it ends ("display device held N ms by thread T in
OP"); `in_flight()` lists every call in flight, and the editor's panic report appends it ("GPU calls in
flight"). The holder of a future timeout is named in the report the person sends.

### D3. Test seams

`holds::hold_gl_context(device, release)` / `GpuFxEngine::hold_gl_context` take the hal device's
context lock (`as_hal::<Gles>`) until `release` returns — what a stalled call does to every other
caller. The bench's `gl_stall N` uses it on the preview engine's device. The bench's `gpu_busy`
(resize_loop's stand-in for the preview's submissions on Vulkan) does nothing on GL: nothing of the
program submits on the window's device there, and `resize_loop` expects no refused reconfigure on GL.

## The class, walked: every path that calls into the window's GL device

| Path | Thread | Before | Now |
|---|---|---|---|
| egui's paint, texture uploads, surface configure/present | UI | the lock's only legitimate user | the only user on GL |
| preview `present`/`present_kept`/`execute*` | preview-compose | on the window's device | on the preview device (GL) |
| `wait_presented` (polls) | preview-compose | on the window's device | not used on GL (bytes path) |
| display engine build (`new_kept`, every pipeline) | engine-build | on the window's device, seconds | the preview device, once per process |
| warm worker: warm draws (blocking wait), shapes, run shapes | pipeline-warm | the window's device's `Compiled` | the preview device's `Compiled` |
| `preview_display` texture registration | UI | display path | not created on GL |
| `read_display`, `finish_present` (bench/headless) | caller | window's device | not reached on GL |
| bench `gpu_busy` | live-gpu-busy | window's device | does nothing on GL |
| offline/side engine, slow route, loop fill, thumbnails, reel strips, posters | sched/lanes | own GL device (own instance) | unchanged |
| export engines | render | own GL devices | unchanged |
| compute host, held frames | compute threads | Vulkan (PRIMARY), never GL | unchanged |
| uncaptured-error handler, device-lost callback | wgpu callers | no device call inside | unchanged |
| kt-cli | — | no window | unchanged |

Vulkan, DX12 and Metal have no adapter context lock; a stall there delays a frame, and egui's paint
waits on no lock of ours with a timeout.

## Risks

- The live suite (GL) no longer runs the display path the owner uses (Vulkan); `gpufx` checks the
  display path only under `WGPU_BACKEND=vulkan` (found-defects 681).
- The bytes path's per-picture cost on the UI thread on GL (found-defects 680).
