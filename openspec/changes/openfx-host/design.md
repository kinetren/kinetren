# Design: openfx-host

> DESIGN ONLY, revision 1 (2026-11-22), base `937d5879`. Q1–Q2 in `proposal.md`; defaults recommended.

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
| CPU images (or a host OpenGL/CUDA context) | a GPU-only composite in wgpu | a readback and an upload per OFX node per frame (D5) |
| float RGBA images | 8-bit composite (16-bit per project, designed) | conversion to float for the plugin; precision limited by our path |
| a host that can call render on many threads, or serialize "unsafe" plugins | one render thread for audio; video composed on our compose thread | the plugin runs in its own process, its threading is its own (D4) |
| an interact suite for overlays | our own gizmos only | overlays later |
| a host OpenGL context to share | none (wgpu) | GPU suites unused; the plugin's own GPU use is its business |
| state across frames allowed | F-EXT-003 refuses state | stateful plugins refused at registration |
| determinism not required | invariant 11 | OFX nodes marked "foreign", no cross-machine promise, cache keyed by the binary's hash |

## D4. The host process

`kinetren-ofx`, started by the program when a project first uses an OFX kind, one per program run. It loads bundles with
`dlopen`, implements the host suites, and serves render requests over a pipe with frames in shared memory (`memfd` /
a named file mapping). A crash or a hang (a render past its budget) kills the child: the node renders as its input with a
notice naming the plugin, the journal records it, the child restarts on the next request. "Render unsafe" plugins are
called one at a time inside the child.

## D5. In the plan

An OFX node is a barrier: the plan reads back the node's input (RGBA, converted to float), sends it with the parameter
values at that time, waits for the result (bounded), uploads it and continues. Temporal access: the plugin's request for
its input at another time is answered by composing the input at that time through the same plan (the request path; a
miss is a held frame with Loading). Results go to the persistent cache keyed by the input's key, the values and the
plugin binary's hash.

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

## D9. Idle

The child is not started until used; it blocks on its pipe at rest.

## Rejected

- **In-process dlopen** (Q2 b) — a plugin's crash or lock ends the program.
- **Retimer context now** — our time model is the warp; a foreign retimer would be a second one.
