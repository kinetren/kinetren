---
id: F-COL-039
title: A worker is us, without the interface
status: candidate
priority: B
cost: S
category: collab/offload
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, flamenco-3]
we_have: partial
crates: [kt-gpu, kt-cli, kt-engine, kt-app]
invariants: [7, 11]
ui_rules: []
round: null
point: null
tests: [t181, t182, t183, t184, t184b]
depends_on: []
trackers: []
services: [resolve-project-server, bm-cloud]
checked: 2026-09-08
---

# F-COL-039 — A worker is us, without the interface

## Essence

There is no "render program" on a farm. There is the same program, launched with a command-line key on a second machine: for Resolve it is `./Resolve -rr`, for After Effects — `aerender`, for Blender — the same `blender` with parameters. To an editor this looks like "handed it to the neighbouring computer", but in fact it means that a second instance of the editor is obliged to compute EXACTLY THE SAME THING — and it turns out here that nobody promised "the same thing". While one machine computes, there is nothing to check agreement against, and no need to. As soon as there are two machines, determinism stops being a pleasant property and becomes a requirement, for the violation of which the price is paid not by a test, but by the viewer.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 build 66 | none | there is no headless mode; everything that resembles a worker is the same C# script inside a running window (F-EXP-002) |
| Adobe Premiere | 26.x | Adobe Media Encoder (`ame`) | the worker is a separate PRODUCT of the same vendor, with its own queue and watch folder. The agreement of the result is guaranteed by the fact that it is always AME that encodes, not Premiere |
| DaVinci Resolve | 21 Studio | Remote Rendering | the worker is Resolve itself: on a second machine it is either open with the Remote Rendering workspace, or launched from the command line with the `-rr` key. Both ends must see one project database and the same media by agreed paths |
| Final Cut Pro | 12.3 | Compressor (+ `qmasterprefs`) | distribution is moved into a second product; FCP itself does not work as a worker on another machine in any form |
| After Effects | 26.x | `aerender` | the same engine without a window; precisely for this reason AE gets a farm, and NLEs do not: a queue item is addressed by frames, and the output is a sequence where each frame is entirely made by one machine |

**Outside the core.** The row is needed because in editors the role of a worker is hidden behind a product name, while on a farm it is named explicitly and separated from the job.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Flamenco | 3.x | worker | the manager hands out jobs, the worker runs THE SAME program with the command from the job; there is no cloud, both sides are in one network (`docs/features/_editors.md`) |
| OpenCue | 1.x (docs.opencue.io, 2026) | host, proc, service, tags | the machine is described by data: the host's tags are compared with the layer's tags, and a frame does not land on a machine that lacks the needed tag |

## The trick behind it

**A worker is a launch key, not a program, and the price of this decision is silence.** The setup guide for a Resolve node describes the standard way of raising it over SSH with the command `./Resolve -rr`, and in the same breath honestly notes that there is no feedback about the progress of the render: one can observe either from the side where the window is open, or not at all. Hence a non-obvious consequence for the design of the product: a "worker" has no interface of its own not because no one wrote one, but because writing it means writing a second product (the path of Adobe and Apple: Media Encoder and Compressor). Everyone who did not do this lives with the second machine working silently.

**The industry circumvents the agreement of results rather than ensuring it — and circumvents it by the FORM of the OUTPUT.** Where a farm really divides the work (After Effects, 3D), the output is almost always a sequence of frames, and the rule is formulated in terms of failure: "Always render to image sequences, not video files — if a frame fails, only that frame needs re-rendering". The point is that this same rule also removes the question of agreement: each frame is entirely computed by ONE machine, and there is nothing inside a frame to diverge. Where the output must be a single file — that is, in video editing — there is nothing left to distribute, and Resolve honestly hands the job whole to one machine, without trying to divide it.

**We already have the worker's role written, but not its passport.** `kt-cli export` raises a headless device and prints to stderr `video: GPU (<name>)` or `video: CPU (no GPU: …)`, and `KT_VIDEO_GPU=0` forces computation on the processor (`src/apps/kt-cli/src/main.rs`). That is, "we are the same, without an interface" is not a plan but today's binary. But the adapter is chosen by PREFERENCE, not by name: `PowerPreference::from_env().unwrap_or(LowPower)` (`src/libs/kt-gpu/src/context.rs:46`), and the environment variable `WGPU_POWER_PREF` changes the answer on a machine with two video cards. The identity of the device is at the same time known — `adapter_info` is turned into a `GpuBackendInfo` with the name, the backend, and the sign of a software rasterizer (`src/libs/kt-gpu/src/engine.rs:479`) — but it enters neither the cache key, nor the frame plan, nor the output file. The bytes are there; the signature under them is not.

## Pros

- A worker requires neither a second codebase nor a second format: if a program can compute without a window, it can already be a worker. For us this is ready.
- The launch key is the cheapest possible surface: no dialog, no settings, no installation. Everything the user needs to be told fits in one line.
- The determinism requirement, stated explicitly, also benefits single-machine work: precisely because of it we have t183 (byte-for-byte between runs on one device) and t184b (a fresh engine against the export engine).
- An output as a sequence of frames solves at once both fault tolerance and agreement — and it is the only trick of the industry that works without agreements between machines.

## Cons

- A silent worker cannot be diagnosed: Resolve launched with the key gives neither progress nor a reason for failure, and a person learns the result from the presence of a file.
- "The same program" means "the same version": an update on one machine turns the farm into a source of divergence, and the documentation usually does not say so; it is found out in practice.
- No one promises agreement of the result. For us it is promised only inside one device (invariant 11), and this is more honest than in most cases — but precisely for this reason two machines with different adapters will give two pieces whose difference is visible at the joint and invisible in tests.
- A silent fallback breaks even identical machines: `export_video_gpu()` in `src/apps/kt-app/src/app.rs`, on failure to create a headless device, prints `eprintln!` and returns `None` (F-RES-007) — that is, one "worker" could compute on GPU and another on CPU, and no one would know.
- A machine passport not written into the result makes an investigation impossible after the fact: the file is there, but what it was computed with is no longer known.

## What users say

Verbatim about the fact that the node is the same Resolve, raised with a key, and about the price of such a solution:

> You can also run this "headless" via the command line with the -rr flag,
> though this gives you no feedback on render progress.
> — eposvox, "DaVinci Resolve Remote Render Server Complete Guide
> (Windows/Linux)", Level1Techs Forums, 12.02.2019,
> https://forum.level1techs.com/t/davinci-resolve-remote-render-server-complete-guide-windows-linux/138807

Why a farm outputs a sequence of frames rather than a file — a formulation through failure, not through agreement:

> Always render to image sequences, not video files — if a frame fails, only
> that frame needs re-rendering.
> — "How Render Farms Work — Technical Guide (2026)", SuperRenders,
> published 04.04.2026, updated 07.09.2026,
> https://superrendersfarm.com/article/how-render-farms-work-technical-guide

The `trackers:` field is deliberately empty: there are no public trackers for the five core editors (`docs/features/_trackers.md`), and the topic of distributed rendering is absent as a class from the exported trackers of open-source NLEs — there are no farms or workers there. Rule 12 is satisfied by a non-empty field, and filling it for form's sake would be fabricating a signal.

## How it fits Kinotrain

### What already exists

- `src/apps/kt-cli/src/main.rs` — a ready headless executor: `export` opens the sources, raises `GpuContext::new_headless` + `GpuFxEngine::new`, passes the executor into `export_mp4_with_backends`, and prints the chosen backend. A video encoder can be fixed by a third argument — this was established for the sake of cross-render reproducibility (test 158), that is, the requirement "compute predictably" has already been needed by us once.
- `src/libs/kt-gpu/src/context.rs:46` — `new_headless`: its own adapter without a surface; the limits repeat the egui-wgpu rule (`downlevel_webgl2` on GL, otherwise default; `max_texture_dimension_2d 8192`) precisely so that shader behaviour in the GUI and headless is the same.
- `src/libs/kt-gpu/src/engine.rs:479` — `GpuBackendInfo`: the adapter name, the backend, the sign of a software rasterizer (`llvmpipe`/`softpipe`/`swiftshader`). The machine's passport is COUNTED.
- Invariant 11 in its current edition is the only honest description of the boundary: audio byte for byte always; video on CPU byte for byte against the committed reference; video on GPU byte for byte only on ONE device (t183, t184b); between devices — only agreement with CPU within tolerances (t181, t182, `FrameTol` from `src/libs/kt-gpu/src/lib.rs`).
- `src/libs/kt-engine/src/export.rs` — an export that has started is completed on its own backend: a GPU error propagates with `?` and becomes an export error, rather than a reason to draw the remainder on the processor.

### What is lacking

- **The identity of the device is recorded nowhere.** `adapter_info` lives in `GpuContext` and in `GpuBackendInfo`, but it is not part of either the post-FX cache key (`fx_chain_hash`, `src/libs/kt-engine/src/video.rs`) or the export statistics (`ExportStats` carries only the name of the ENCODER, `src/libs/kt-engine/src/export.rs`). Under the bytes there is no signature.
- **The adapter choice is not reproducible from the job.** `PowerPreference::from_env()` reads the environment variable `WGPU_POWER_PREF`; on a machine with two video cards the same call gives different devices. We have no way to say "compute on this adapter", whereas for the encoder such a way already exists (`force_encoder`).
- **The CPU fallback is not visible in the result.** In kt-cli it at least gets printed to stderr; in kt-app — only `eprintln!`, and in the render window and in the completion report there is not a single line about the backend (F-RES-007).
- **A cross-device test does not exist and cannot exist in its present form:** t183 and t184b check repetition ON THE SAME device, t181/t182 compare GPU with CPU within tolerances. The claim "two workers will give the same result" is not checked by anything and cannot be checked on one machine.

### What stands in the way

Invariant 11 — and it stands in the way correctly. It does not forbid a second executor; it names the price: between machines only agreement with the CPU reference within tolerances is guaranteed, and `FrameTol::THRESHOLD` does not limit outliers at all (`outlier_max: 255`, `src/libs/kt-gpu/src/lib.rs`), because threshold effects flip 0↔255 at a boundary. Therefore two pieces computed on different adapters legitimately differ — and this difference will land not inside a frame but at the joint, where the eye will see it. The only known way the industry bypasses this — to count the unit of output as a whole frame from one machine, rather than a file from several — runs for us into the fact that the output must be a file (invariant 9).

### In the `collab/` category — two mandatory lines (rule 13)

OFFLINE: REMAINS — `src/libs/kt-gpu/src/context.rs:46` — the worker's role is already executed by us: kt-cli raises a headless device with the same composition code as the window, and prints the chosen backend; on one machine this is exactly the program without an interface that the farm launches with a key, and what it lacks is not a network but a signature under the result — the name of the adapter is counted and written nowhere.

NETWORK COST: a second machine whose adapter must match the first — invariant 11 — we promise byte for byte only on one device (t183, t184b); between devices — agreement with the CPU reference within tolerances, so two pieces from two workers differ legitimately, and the difference is visible at the joint, not in the tests.

### Which invariants it touches

Invariant 11 (determinism) is the main one: it both permits the worker and names the boundary beyond which there are two workers. Invariant 7 (the composition code is one; the backend is a parameter of execution, and a started export completes on its own backend) — the reason why the headless executor is ours at all and not a second path. Rules of the UI are not touched by this note: a worker has no interface by definition.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage: the machine's passport into the result and a way to fix the adapter |
| Where the main work is | kt-gpu (adapter choice by name, not only by preference), kt-engine (`ExportStats` carries the backend and the adapter), kt-app + kt-cli (show this to the person, not only in stderr) |
| Project format | not touched |
| Tests to write | "`ExportStats` names the backend and the adapter, and under forced CPU this is visible"; "fixing the adapter by name selects exactly it or refuses explicitly" (a pair to `force_encoder`); a negative one: cross-device equality cannot be checked by a test — this must be written in the test itself |
| Depends on | F-COL-038 (the job's form: the adapter and the backend are its "machine requirements" field) |
| Breaks compatibility | no |

## Verdict

We take the local core: **a machine's signature must stand under the result**. The adapter name, the backend, and the software-rasterizer flag are already computed (`GpuBackendInfo`) — they need to be carried into `ExportStats` and to the person, and a symmetric "compute on this adapter" must be added next to `force_encoder`. This is useful already today, without any second machine: now, when we receive a strange frame, we cannot say what it was computed with.

We do not set up the server half — there is no second participant. But the note must leave a warning behind, because it is counter-intuitive: **our determinism is not "strong", it is LOCAL**. Invariant 11 promises bytes on one device, and any future proposal of "let the second machine compute it" must begin with an answer to the question of what to do with the joint of two adapters. The industry answered this question with a refusal to divide: Resolve hands the job whole to one machine, and those who do divide, divide into FRAMES and output a sequence — that is, exactly what invariant 9 does not allow us.

Boundaries: the job's form — F-COL-038; assembling pieces from different workers is rejected in F-COL-040 (and one of the reasons for the rejection is exactly this one); why "cloud rendering" nowhere turned out to be rendering — F-COL-041.

## Sources

- https://forum.level1techs.com/t/davinci-resolve-remote-render-server-complete-guide-windows-linux/138807 — a Resolve node over SSH with the command `./Resolve -rr`, the absence of feedback, the requirement of shared access to media and to the project database (eposvox, 12.02.2019; access date 2026-09-08)
- https://superrendersfarm.com/article/how-render-farms-work-technical-guide — "Always render to image sequences, not video files…" (published 04.04.2026, updated 07.09.2026; access date 2026-09-08)
- https://docs.opencue.io/docs/concepts/glossary/ — `host`, `tags`, `service`: the machine is described by data, and a frame does not go where there is no needed tag (2026-09-08)
- `docs/features/_editors.md`, row `flamenco-3`; the primary source `flamenco.blender.org` returns 403 from this environment — **[NOT VERIFIED]** against the primary source: the exact manager↔worker protocol.
- Our code checked against the tree on 08.09.2026: `src/libs/kt-gpu/src/context.rs:46`, `src/libs/kt-gpu/src/engine.rs:479`, `src/libs/kt-gpu/src/lib.rs` (`FrameTol`), `src/libs/kt-engine/src/export.rs`, `src/apps/kt-cli/src/main.rs`, `src/apps/kt-app/src/app.rs`; tests `src/libs/kt-gpu/tests/parity.rs` (t181, t182, t183) and `src/libs/kt-gpu/tests/gpu_render.rs` (t184, t184b).
- Neighbouring notes as a source on our own code: F-RES-007 (the silent CPU fallback of export and the absence of a backend line in the render window), F-EXP-002 (the queue is internal, invariant 9).
