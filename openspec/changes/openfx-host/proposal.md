# Proposal: openfx-host

> DESIGN ONLY (spec session), revision 2 (2026-11-23) after the owner's answer to Q57 (`kkt-26.11.22` D4): frames cross
> to the host process through shared memory with no copy — GPU shared memory where the plugin takes GPU images, RAM
> shared memory otherwise. Revision 1: 2026-11-22. Registry `v26.10-index.md` O1–O4 (no). Note F-EXT-001 (refreshed). Stage 3 of
> `plugin-api-stage1` D10 (built-in → wasm → dlopen/OpenFX), after `wasm-plugin-host`. Tests reserved: t3974–t3979.

## Why

The owner, `docs/ktprompts/v26.10.txt` lines 130–133, whole:

> 1. OpenFX, прикрутить поддержку эффектов.
> 2. где есть эффекты/плагины openfx которые мы можем скачать-собрать-воспользоваться?
> 3. что там еще в ofx есть? какие-то другие апи или что, что оттуда можно использовать кроме fx?
> 4. по каким моментам мы не совместимы с openfx в худшую сторону? что есть в ofx чего из-за нашей архитектуры сейчас
> нереально использовать?

**How this design reads the item**: item 1 asks for support (a host), items 2–4 are questions to be answered — where
plugins are, what else OFX offers, where our architecture is worse for OFX. The answers are in `design.md` (D1–D3) and
shape the host (D4–D8).

The owner's answer to Q57, `kkt-26.11.22.txt` line 7 (D4), whole: "п.1. а нахрена копировать кадр? почему shmem ram не
использовать? или sh mem gpu??? мы же на современных системах используем программу." **Reading, checked:** revision 1
said "frames cross through shared memory (a copy per frame)" — the copy was the GPU → RAM readback and the RAM → GPU
upload around every OFX node. He asks for no copy: a plugin that renders on the GPU gets OUR GPU image itself,
shared across the process boundary; a plugin that renders on the processor gets a RAM buffer shared with the child,
into which the GPU writes directly. The own process stays (Q2 a holds: a crash costs the node). His R.59 decision stands: "все три; сначала будут built-in и динамические на dlopen в будущем,
openfx в будущем вернём" — OFX after the built-ins and the wasm host.

## What Changes

- **Answers to items 2–4** (design D1–D3): where OFX plugins come from (open sets we can build and test with, commercial
  ones people own), what OFX has besides filters (generator, transition and retimer contexts; on-screen interacts;
  parametric curves; GPU render suites), and where we are worse (our GPU-only composite and 8-bit path, our single
  render thread, no interact API, no host-side OpenGL/CUDA context, cross-machine determinism).
- **A host in its own process** (`kinetren-ofx`): OFX plugins are C libraries that may crash or lock; they run in a child
  process started once.
- **Frames without a copy** (D10):
  - **GPU shared memory** for plugins that declare an OFX GPU render (OpenGL, CUDA, OpenCL; Vulkan where an extension
    offers it): our Vulkan image is exported (dma-buf / opaque fd on Linux, an NT handle on Windows), the child imports
    it into the plugin's API, and exported semaphores order the two processes. No byte crosses RAM.
  - **RAM shared memory** for processor plugins: a memfd (a file mapping on Windows) shared with the child; the GPU
    copies the node's input straight into it (Vulkan imports the mapping as buffer memory,
    `VK_EXT_external_memory_host`) and uploads the result straight from it. That is one device transfer each way, the
    least a processor plugin can need, and no memcpy.
  - The images and buffers come from a pool made once per size and depth, so a frame allocates nothing. A crash ends the child, not the program; the node goes
  bypassed with a notice; the child restarts on the next frame.
- **Mapped to our families**: an OFX "filter"/"general" plugin is a video effect node; a "generator" context plugin is
  a generator kind; a "transition" context plugin is a transition kind. Its parameters become a passport (double, int,
  choice, boolean, RGBA, 2D point); OUR curves animate them (the host hands the value at each time); a parametric
  (curve) parameter maps to our curve editor.
- **What a plugin may do, by the F-EXT-003 conditions**: a frame may read its input at other times (temporal access,
  through our request path); a plugin that keeps state between frames is refused at registration; "render unsafe"
  plugins are serialized inside the child process, never on our render thread.
- **Honest about cost and bytes**: an OFX node is a barrier (the plan waits for the child); the trace names its path
  (GPU shared, RAM shared, or the counted fallback copy where a device lacks the extensions) and the bytes it moved
  through RAM; it is marked "foreign" in the effects window; cross-machine byte equality is not promised for it. Its
  result is not kept apart: the persistent cache keeps final frames only (the owner's D1).
- **Installed through the library**: OFX bundles (`*.ofx.bundle`) from the standard OFX folders and from the library of
  downloads (kind `plugin`).

## Capabilities

### Modified Capabilities

- `plugins`: the OFX host process, the context → family mapping, the parameter mapping, the conditions, the crash rule.
- `ui-reach`: OFX plugins in the picker marked "foreign", their notices, the Library rows.

## Impact

- New binary `kinetren-ofx` (the host process; C ABI, the OFX 1.4 host suites the plugins need: property, image effect,
  parameter, memory, multithread, message, progress, timeline, interact later).
- New crate `kt-plugin-ofx` (the program's side: process supervision, the shared-memory protocol, the registry adapter).
- kt-gpu: exportable images and semaphores and host-memory buffers through wgpu's hal (`as_hal::<Vulkan>`, an image made
  with the export info and wrapped back as a wgpu texture), pooled; the GLES backend has the same door (`holds.rs`).
- kt-engine: an OFX node as a barrier in the one plan (hand over a shared image, wait on its semaphore); temporal reads
  through the request path.
- kt-app: picker marks, notices, Library rows; i18n ×5.
- The binary still does not link libav (invariant 10); the child links nothing of ours but the frame protocol.
- Idle: the child process sleeps on its pipe; it is started only when a project uses an OFX node.

## Questions to the owner

None open. Decided (`kkt-26.11.22` D4, registry): Q1 (a) — the host is built and tested against the open sets
(openfx-misc, openfx-arena, G'MIC); Q2 (a) — its own process, with frames through GPU or RAM shared memory and no copy.
