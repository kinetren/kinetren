# Design: openfx-host

> DESIGN ONLY, revision 2 (2026-11-23), base `e7b19346`: the owner's D4 — frames through GPU or RAM shared memory, no
> copy (D10). Revision 1 (2026-11-22). Q1 (a) and Q2 (a) decided.

## Context

- F-EXT-001 (rejected, then revised by the owner in R.59: "все три … openfx в будущем вернём"): the price — invariants 11
  (bytes), 7 (one composition path), 2 (one render thread) — became the CONDITIONS of F-EXT-003: a plugin is a pure
  function of the frame; state or a tail → not loaded; a panic caught, the frame passes with a notice.
- `plugin-api-stage1` D10: built-in → wasm (`wasm-plugin-host`) → dlopen and the OFX bridge.
- Our composite runs on the GPU (wgpu) in 8-bit (a 16-bit path per project is designed, `hdr-sources-to-sdr`, A25).

## D1. Item 2 — where OFX plugins are

- Open, buildable, testable: Natron's **openfx-misc** (filters, transforms, colour, a chroma keyer, the TestRenderOFX
  host test), **openfx-arena** (ImageMagick-based effects, text, readers), **G'MIC**'s OFX bundle; Natron hosts them as
  ordinary OFX plugins (OFX 1.4 with extensions). Sources: github.com/NatronGitHub/openfx-arena,
  github.com/MotivaCG/openfx-misc, aur.archlinux.org/packages/openfx-misc (GPL), natron.readthedocs.io "Natron as an
  OpenFX Host", natrongithub.github.io, github.com/tobybear/gmic_plugin, Wikipedia "OpenFX (API)".
- Commercial (owned by users, licensed per host, often with Resolve-specific GPU suites): Neat Video, Boris FX Sapphire
  and Continuum, FilmConvert [NOT VERIFIED here].

## D2. Item 3 — what OFX has besides filters

[From the OFX programming reference; NOT VERIFIED against the current spec in this session.]
- Contexts: filter, general, **generator** (no input), **transition** (two inputs and a progress), **retimer** (a source
  at a fractional time), paint.
- Suites: parameters (including **parametric** = curve parameters), **interact** (on-screen overlays drawn by the plugin
  over the viewer), timeline (the host's time), progress, message, multithread, memory; GPU render suites (OpenGL,
  CUDA, Metal, OpenCL) as extensions.
- Mapped here: filter/general → a video effect node; generator → a generator kind; transition → a transition kind;
  retimer → refused in this stage (our warp is the time model); parametric → our curve editor; interact → later (our
  preview gizmo layer would host it).

## D3. Item 4 — where we are worse for OFX

| OFX expects | We have | Consequence |
|---|---|---|
| CPU images (or a host OpenGL/CUDA context) | a GPU-only composite in wgpu (Vulkan) | GPU plugins: our image shared into their API; CPU plugins: one device transfer each way into RAM shared memory (D10) |
| float RGBA images | 8-bit composite (16-bit per project, designed) | conversion to float for the plugin; precision limited by our path |
| a host that can call render on many threads, or serialize "unsafe" plugins | one render thread for audio; video composed on our compose thread | the plugin runs in its own process, its threading is its own (D4) |
| an interact suite for overlays | our own gizmos only | overlays later |
| a host OpenGL context to share | none (wgpu on Vulkan) | the child makes its own context and imports our exported image and semaphores into it (D10) |
| state across frames allowed | F-EXT-003 refuses state | stateful plugins refused at registration |
| determinism not required | invariant 11 | OFX nodes marked "foreign", no cross-machine promise, cache keyed by the binary's hash |

## D4. The host process

`kinetren-ofx`, started by the program when a project first uses an OFX kind, one per program run. It loads bundles with
`dlopen`, implements the host suites, and serves render requests over a Unix socket (a pipe on Windows) carrying handles, never pixels: frames
live in shared memory as D10 says. A crash or a hang (a render past its budget) kills the child: the node renders as its input with a
notice naming the plugin, the journal records it, the child restarts on the next request. "Render unsafe" plugins are
called one at a time inside the child.

## D5. In the plan

An OFX node is a barrier. The plan:
1. Hands the child a pooled shared image (or a RAM buffer) holding the node's input in the depth the plugin accepts.
2. Signals the input's semaphore and sends the parameter values at that time.
3. Waits for the output's semaphore, bounded by the hang budget.
4. Continues with the output image as the node's result.

If the plugin accepts our composite's depth (8-bit RGBA, or the 16-bit half path), the input image IS the layer's
texture, shared. Otherwise one GPU conversion pass writes the shared image, and nothing goes through RAM.

Temporal access: the plugin's request for its input at another time is answered by composing the input at that time
through the same plan into another pooled image (the request path; a miss is a held frame with Loading). The node's
result is not stored apart: the persistent cache keeps final frames only (`persistent-cache` D2, the owner's D1).

## D6. Parameters and curves

OFX params → passport rows (double/int with ranges, choice, boolean, RGBA, double2D as a point with an on-preview handle
by our gizmo); the host hands values at the frame time from OUR curves (the plugin's own animation is not used);
parametric params → our curve editor rows.

## D7. Installing

The standard OFX folders (`/usr/OFX/Plugins`, `%CommonProgramFiles%\OFX\Plugins`) are scanned at start in the child;
the library of downloads lists open sets (kind `plugin`, sub-kind `ofx`). A scan result is cached by the bundle's mtime.

## D8. Class walk — "foreign code inside the frame"

| Door | Process | Determinism | Crash |
|---|---|---|---|
| built-in plugins | ours | ours | n/a |
| wasm plugins | ours, sandboxed | pure, same bytes | trapped |
| OFX plugins | their own | not promised, marked | the child dies, node bypassed |
| FFmpeg (system) | ours; libva probe in a child for export | — | the probe child |

## D10. Frames without a copy (the owner's D4)

**GPU shared memory** — for a plugin that declares an OFX GPU render support (OpenGL, CUDA, OpenCL, Metal on macOS;
`kOfxImageEffectPropOpenGLRenderSupported` and its siblings [names to confirm against the OFX 1.5 headers at apply]).
- kt-gpu allocates the node's input and output images itself through wgpu's hal (`Device::as_hal::<Vulkan>`): a
  `VkImage` with `VkExternalMemoryImageCreateInfo` (Linux: `DMA_BUF` or `OPAQUE_FD`; Windows: `OPAQUE_WIN32`), wrapped
  back as a `wgpu::Texture` (`create_texture_from_hal`). The composite writes it like any texture.
- The fds go to the child over the socket (`SCM_RIGHTS`; `DuplicateHandle` on Windows) once per pooled image, not per
  frame. The child imports them:
  - OpenGL: `GL_EXT_memory_object_fd` / `_win32` and `GL_EXT_semaphore_fd`;
  - CUDA: `cudaImportExternalMemory` / `cudaImportExternalSemaphore`;
  - OpenCL: `cl_khr_external_memory` / `cl_khr_external_semaphore`.
  It then gives the plugin its own texture or buffer of that API.
- Order across the processes: two exported timeline semaphores per node (`VK_KHR_external_semaphore_fd`, a
  `sync_file` on Linux), "input ready" and "output ready". Neither process waits on the CPU for the other's GPU work,
  except at the bounded hang check.
- On the GLES backend (no Vulkan) the same door is an EGLImage from a dma-buf (`holds.rs` already reaches the GLES hal).

**RAM shared memory** — for a processor plugin.
- The parent creates a memfd (`CreateFileMapping` on Windows) per pooled buffer, maps it, and imports the mapping into
  Vulkan as buffer memory (`VK_EXT_external_memory_host`).
- `copy_texture_to_buffer` then writes the input straight into the shared pages, and `copy_buffer_to_texture` reads
  the result straight from them. One device transfer each way, no memcpy by either process.
- The child maps the input read-only and the output read-write; the plugin gets the OFX image pointer into them.
- Without `VK_EXT_external_memory_host`, a mapped staging buffer plus one memcpy into the shared pages is the
  fallback. It is counted and named in the trace, never silent.

**Common rules.**
- The pool holds three images or buffers per (size, depth, path), made at warm-up. A frame creates no resource
  (video-plan "A frame creates no GPU resource after warm-up").
- The parent owns every shared object. A child crash leaves them valid. The restarted child re-imports them, and a
  hang past the budget bypasses the node.
- The trace span of an OFX node names its path (`gpu-shared` / `ram-shared` / `ram-copy`) and the bytes it moved through
  RAM: 0 on the GPU path.

## D9. Idle

The child is not started until used; it blocks on its pipe at rest.

## Rejected

- **In-process dlopen** (Q2 b) — a plugin's crash or lock ends the program; shared memory gives the same zero copy
  across the boundary.
- **A readback and an upload through private buffers per frame (revision 1)** — rejected by the owner (D4): "а нахрена
  копировать кадр?".
- **Retimer context now** — our time model is the warp; a foreign retimer would be a second one.
