# Proposal: openfx-host

> DESIGN ONLY (spec session, 2026-11-22). Registry `v26.10-index.md` O1–O4 (no). Note F-EXT-001 (refreshed). Stage 3 of
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
shape the host (D4–D8). His R.59 decision stands: "все три; сначала будут built-in и динамические на dlopen в будущем,
openfx в будущем вернём" — OFX after the built-ins and the wasm host.

## What Changes

- **Answers to items 2–4** (design D1–D3): where OFX plugins come from (open sets we can build and test with, commercial
  ones people own), what OFX has besides filters (generator, transition and retimer contexts; on-screen interacts;
  parametric curves; GPU render suites), and where we are worse (our GPU-only composite and 8-bit path, our single
  render thread, no interact API, no host-side OpenGL/CUDA context, cross-machine determinism).
- **A host in its own process** (`kinetren-ofx`): OFX plugins are C libraries that may crash or lock; they run in a child
  process started once, frames passed through shared memory. A crash ends the child, not the program; the node goes
  bypassed with a notice; the child restarts on the next frame.
- **Mapped to our families**: an OFX "filter"/"general" plugin is a video effect node; a "generator" context plugin is
  a generator kind; a "transition" context plugin is a transition kind. Its parameters become a passport (double, int,
  choice, boolean, RGBA, 2D point); OUR curves animate them (the host hands the value at each time); a parametric
  (curve) parameter maps to our curve editor.
- **What a plugin may do, by the F-EXT-003 conditions**: a frame may read its input at other times (temporal access,
  through our request path); a plugin that keeps state between frames is refused at registration; "render unsafe"
  plugins are serialized inside the child process, never on our render thread.
- **Honest about cost and bytes**: an OFX node is a barrier with a readback (our composite is on the GPU) and an upload
  back; it is marked "foreign" in the effects window; its results are cached in the persistent cache keyed by the
  plugin binary's hash; cross-machine byte equality is not promised for it.
- **Installed through the library**: OFX bundles (`*.ofx.bundle`) from the standard OFX folders and from the library of
  downloads (kind `plugin`).

## Capabilities

### Modified Capabilities

- `plugins`: the OFX host process, the context → family mapping, the parameter mapping, the conditions, the crash rule.
- `ui-reach`: OFX plugins in the picker marked "foreign", their notices, the Library rows.

## Impact

- New binary `kinetren-ofx` (the host process; C ABI, the OFX 1.4 host suites the plugins need: property, image effect,
  parameter, memory, multithread, message, progress, timeline, interact later).
- New crate `kt-plugin-ofx` (the program's side: process supervision, shared-memory frames, the registry adapter).
- kt-engine: an OFX node as a barrier with readback/upload in the one plan; temporal reads through the request path.
- kt-app: picker marks, notices, Library rows; i18n ×5.
- The binary still does not link libav (invariant 10); the child links nothing of ours but the frame protocol.
- Idle: the child process sleeps on its pipe; it is started only when a project uses an OFX node.

## Questions to the owner

**Q1. Which plugins first?** Your item 2: "где есть эффекты/плагины openfx которые мы можем скачать-собрать-воспользоваться?"
Open sets (Natron's openfx-misc, openfx-arena, G'MIC's OFX build) can be built and tested in CI; commercial ones (Neat
Video, Sapphire, FilmConvert) are what people own, but their licences and Resolve-specific extensions vary.
- (a) **The host is built and tested against the open sets (openfx-misc, openfx-arena, G'MIC); commercial plugins work
  where they keep to plain OFX 1.4** (*recommended*: a test set we can ship in CI; nothing promised for host-specific
  extensions).
- (b) Aim at a named commercial plugin first (say which), with its extensions.

**Q2. In our process or in its own?** Your item 4 asks where we are worse; a plugin crash is the worst case.
- (a) **Its own process: a crash or a hang costs the node, not the program; frames cross through shared memory (a copy
  per frame)** (*recommended*).
- (b) In our process: no copy per frame, but a plugin's crash ends the program.
