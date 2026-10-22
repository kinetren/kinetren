---
id: F-EXT-003
title: Plugin contract: what foreign code must declare
status: accepted
priority: B
cost: XL
category: plugins/hosts
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-fx, kt-engine, kt-gpu, kt-cache, kt-otio, kt-app]
invariants: [2, 4, 7, 11]
ui_rules: []
round: R.59
point: null
tests: []
depends_on: [F-EXT-001, F-EXT-002]
trackers: []
services: []
checked: 2026-09-21
---

# F-EXT-003 — Plugin contract: what foreign code must declare

## Essence

The note is not about whether to take plugins — it is about WHAT foreign code must declare about itself in order to be admitted at all. The host cannot verify a foreign binary: it can only make it DECLARE itself and refuse the one whose declaration is incompatible with the host's promises. All live plugin APIs are built exactly this way — they are lists of properties that the plugin declares and the host executes. So the first step is neither a loader nor a sandbox, but the types: which three or four questions we ask, and what we do with each answer.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| DaVinci Resolve | 21 | OpenFX (video) + VST/AU on Fairlight (audio) | the reference OFX host: its own ResolveFX are written as OFX, so a foreign plugin sits in the same inspector and the same node as a native one. Audio is a second ecosystem, independent of the first |
| VEGAS Pro | 2026.0 | OpenFX (video) + DirectX/VST (audio) | Vegas's native video effects are also OFX bundles inside the installation; the foreign VST catalogue is scanned at every launch (F-EXT-001) |
| Adobe Premiere | 26.x | NO OFX; its own Premiere Effect SDK; audio — VST3/AU | a wholly separate ecosystem; plug-ins for After Effects also work in Premiere, the API is closed to the outside |
| After Effects | 26.x | NO OFX; AE Effect SDK | not an NLE, but it holds the second half of the effects market; manufacturers sell the licence for Adobe SEPARATELY from "all OFX hosts" |
| Final Cut Pro | 12.3 | neither OFX nor VST: FxPlug (video) + Audio Units (audio) | a fourth ecosystem, closed on macOS. The only one of the five that moved foreign code out of its own process: FxPlug 4 is a separate process (XPC) and the frame goes through IOSurface |

There are four ecosystems (OFX, Adobe, Avid, Apple), they do not intersect, and this is analysed in F-EXT-001. The other column matters here: **all four are sets of PROPERTIES that a plugin declares about itself**, and they differ not by language but by which questions the host considers it necessary to ask.

**Outside the core.** Without the two specifications the topic cannot be described: the properties are declared in them, not in the editors. They deliberately receive no keys in `_editors.md` — the seventh wave did not open new keys, and a specification can carry neither a "take / don't take" verdict.

| Document | Version | What exactly is declared | How it works |
|---|---|---|---|
| OpenFX (specification) | 1.5 (December 2024) | `kOfxImageEffectPluginRenderThreadSafety`, the properties of CUDA/Metal/OpenCL/OpenGL, descriptions of parameters | the plugin declares thread safety and GPU requirements; the host must submit to the worst value. Vulkan is absent from the list of GPU APIs |
| VST 3 SDK | 3.x (`IAudioProcessor`) | `getLatencySamples()`, `getTailSamples()` | the plugin declares its own delay and the length of its tail; compensating the delay is the job of the HOST, which shifts everything else |

## The trick behind it

**1. The real content of any plugin API is not "how to draw a pixel", but "what the host must promise and what the plugin must confess".** In OFX this is visible literally: the value `kOfxImageEffectRenderUnsafe` means that at ANY moment exactly one render call may run across the whole host — among ALL instances of this plugin. One old effect on one track makes the whole pipeline single-threaded, and in the interface this looks no different from any other effect. VST is symmetric and works from the other end: the plugin declares a delay ("the compressor looks ahead 512 samples — so the latency is 512"), and the host compensates it; if the delay changes, the host must recalculate the compensation of the whole mixer, and this is heard as a break in playback. So the plugin BINDS the host with its properties, not the other way round. For us, of the whole list, three confessions turn out to be decisive: **whether it holds state between frames, whether it has a tail, and whether it can be called from several threads.** The answers "yes / yes / no" are incompatible with our invariants 2, 4 and 11 — and this can be checked AT LOAD TIME, before the first frame, rather than caught later by clicks and by divergence of references.

**2. The GPU story of OFX is CUDA, Metal, OpenCL and OpenGL; there is no Vulkan there, and this makes the plugin a CPU island by its construction, not by our decision.** Our only GPU API is wgpu (Vulkan on Linux), pinned to eframe by the same version. Therefore the GPU side of a foreign OFX plugin is impossible to execute for us in principle: not "we did not get to it", but "there is nothing to speak in". The practical conclusion is stronger than a prohibition: since the plugin computes on the CPU in any case, its place in the plan is a barrier with a download of the frame from the video card and an upload back, that is, the same cost that Resolve pays on every frame (the quote from Norman Black in F-EXT-001). Therefore the contract with a plugin can be written immediately as a contract with a CPU executor — and no decision about "what if GPU" needs to be postponed.

**3. Isolation in the industry exists for exactly one of five, and it cost a rewritten host.** FxPlug 4 at Apple is a separate process with the frame through IOSurface; all the others live with foreign code inside themselves and answer a crash with the blacklist "disable them one by one". This sets an honest bar for the round: between "inside the process without protection" and "a full-fledged sandbox" there lies not a gradation but an architectural chasm, and one cannot pretend that we are somewhere in the middle. The intermediate step that is reachable and meaningful for us is exactly one — **catch the panic and pass the frame through**: this is not protection from foreign code, it is protection of the user's session from one bad frame, and it is honestly called by its own name.

## Pros

- A contract written before the loader makes refusal CHEAP and early:
  a plugin that declares state or a tail does not load at all, and no
  reference, no cache and no thread will ever find out about it.
- The three questions ("state? tail? threads?") cover exactly the three
  invariants that F-EXT-001 named as incompatible with plugins. This
  is no coincidence: the list of questions is derived from the list of our
  promises, not from someone else's SDK.
- The order "built-in → dynamic libraries → OFX later" lets us
  write ONE internal interface and implement it three times, instead of
  inventing it three times.
- The entry point for foreign code is already one and already disciplined:
  dlopen with a version check and `RTLD_LOCAL`. We will not have to
  create a second one.
- A plugin that does not get into the golden-cycle registry cannot weaken
  any existing test: the list of kinds that the references run over
  remains a closed enumeration.

## Cons

- A contract without a runtime is a promise that has not been verified yet: until
  the first plugin is loaded, the honesty of its declarations has not been tested by anyone.
- The requirement "pure frame function" rules out whole classes of foreign effects
  (everything with accumulation: trails, temporal smoothing, detectors based on the
  previous frame) — so our "plugin" is deliberately narrower than an OFX plugin,
  and this will have to be explained.
- A panic can be caught, a segfault cannot: the contract protects against errors in Rust,
  not against errors in C.
- A project with a plugin stops being self-contained (F-EXT-001, "Cons"),
  and the contract does not cure this — it only makes the loss recognizable.
- Three models at once are three places where the agreement can diverge from itself; the only
  protection against this is a shared internal interface, and that same interface becomes a bottleneck for development.

## What users say

> Gets the current Latency in samples. The returned value defines the group
> delay or the latency of the plug-in.
> — VST 3 Interfaces, `IAudioProcessor::getLatencySamples`, https://steinbergmedia.github.io/vst3_doc/vstinterfaces/classSteinberg_1_1Vst_1_1IAudioProcessor.html (accessed 2026-09-21)

> kOfxImageEffectRenderUnsafe — indicating that only a single 'render' call
> can be made at any time among all instances
> — header `ofxImageEffect.h`, repository AcademySoftwareFoundation/openfx, https://raw.githubusercontent.com/AcademySoftwareFoundation/openfx/main/include/ofxImageEffect.h (checked while working on F-EXT-001, 2026-09-07)

> All works, but Sapphire crashes immediately Resolve as soon as i launch a
> filter.
> — dbsonic, Boris FX forum, 19.11.2021, https://forum.borisfx.com/t/sapphire-2022-crashing-in-resolve-17-4-2/12695

The first two are exactly the "confessions" from "The trick behind it", read
verbatim: one side declares a delay, the other the right to serialize
the host's entire render. Neither is a request; and the host must carry out both.
The third quote is already in F-EXT-001, and here it reads from the other side: there it was
an argument for "do not let in", here it is a description of the single case from which the contract must
protect the user (a crash right after a routine update of the plugin set, without a single edit to the project).

## How it fits Kinotrain
CAPABILITY: openspec/specs/automation/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
Accepted (interfaces): the parameter passport is built at runtime so that a plugin can declare its own; order — built-in → dylib → OpenFX later.

## Estimate

| What | Answer |
|---|---|
| Scope | XL in total; in R.59 — only types and the contract, without a runtime |
| Where the main work is | kt-model (dynamic parameter passport), kt-fx/kt-engine (the place of a foreign node in the plan and the barrier), kt-app (picker, notice, manager), kt-otio (a foreign effect in the file) |
| Project format | will be needed, but not in this round: the class "an effect I do not have" already exists (`ClipExtra.effects_foreign`) |
| Tests that will have to be written | "declared state/tail — did not load"; "panic in a foreign node = frame passes through + notice"; "golden-cycle registry does not see plugins"; "parsing of the plugin description lives in one place" |
| Depends on | F-EXT-001 (engineering reason), F-EXT-002 (scripting — a neighboring form of extension) |
| Breaks compatibility | no at this step |

## Verdict

**Accepted — as a CONTRACT, not as a subsystem.** The note was opened
because the owner explicitly revised F-EXT-001 in R.59: "all three, first there will be
built-in in the future and dynamic on dlopen, openfx we will bring back in the future".
The order of introduction from this decision: **built-in → dynamic
libraries (dlopen) → OFX bridge later.** In this round TYPES are written —
a shared internal interface that describes all three models — and
neither a loader, nor a manager, nor execution is written.

**The engineering argument of F-EXT-001 is not withdrawn — it is turned into CONDITIONS.** Everything
listed there as incompatible becomes lines of the contract here:

- a plugin is a **pure function** of (frame, parameters, `u`, `burst_index`);
- **no clocks**: neither wall-clock nor "how much time has passed";
- **no I/O**: no files, no network, no own caches on disk;
- **a panic is caught**, the frame passes through, and the user gets a notice (precedent — t185, a notice about GPU loss);
- **a plugin that declares state between frames or a tail is refused AT LOAD**, not at render time: post-FX memoization is built on frames without history, and
  letting such a plugin in means starting to lie with the cache;
- **a plugin never gets into the `GOLDEN_ALL` registry**: references remain a property of the product, not of the set installed by the user.

We reuse what is already written and proven, rather than designing anew:
loading with a version check and `RTLD_LOCAL` — following the pattern of
`src/libs/kt-ffmpeg/src/loader.rs`; injection of the executor as a trait object with a
declared discipline of refusal — following the pattern of `VideoGpuExec`
(`src/libs/kt-engine/src/video_plan.rs`); a registry with a single home for parsing and an honest
error — following the pattern of `open_pseudo`
(`src/libs/kt-media/src/factory.rs`).

And the reverse-sign condition, which should be written down right away: **if a plugin declared itself honestly and still violates the contract, the right answer is not to weaken the contract, but to stop loading it.** The cheap path named by
F-EXT-001 ("take foreign effects INTO THE MATERIAL, not into the chain") remains
in force and goes nowhere: it is not a competitor to this contract, but its fallback exit.

## Sources

- https://steinbergmedia.github.io/vst3_doc/vstinterfaces/classSteinberg_1_1Vst_1_1IAudioProcessor.html
  — `getLatencySamples` ("Gets the current Latency in samples. The returned
  value defines the group delay or the latency of the plug-in", example with a
  compressor and 512 samples) and `getTailSamples` (`kNoTail`,
  `x * sampleRate`, `kInfiniteTail`) (accessed 2026-09-21)
- https://steinbergmedia.github.io/vst3_dev_portal/pages/FAQ/Processing.html
  — host behavior when the latency changes: a call to `getLatencySamples()` and
  recalculation of the mixer's compensation, with a warning about a possible break in playback (accessed 2026-09-21)
- https://raw.githubusercontent.com/AcademySoftwareFoundation/openfx/main/include/ofxImageEffect.h
  — the three values of `kOfxImageEffectPluginRenderThreadSafety`, including
  `kOfxImageEffectRenderUnsafe` (checked while working on F-EXT-001,
  2026-09-07)
- https://raw.githubusercontent.com/AcademySoftwareFoundation/openfx/main/include/ofxGPURender.h
  — the CUDA/Metal/OpenCL/OpenGL properties and the only mention of Vulkan as
  a future possibility (checked while working on F-EXT-001, 2026-09-07)
- https://developer.apple.com/documentation/professional-video-applications/using-out-of-process-fxplug-plug-ins
  — "Using out-of-process FxPlug plug-ins". `[NOT CHECKED against the primary source]`:
  the page returns only the title without the article body; the statements about a separate process (XPC) and the transfer of the frame via IOSurface
  are taken from secondary retellings of the official documentation (accessed
  2026-09-21)
- https://forum.borisfx.com/t/sapphire-2022-crashing-in-resolve-17-4-2/12695
  — Resolve crash right after an update of the plugin set, 19.11.2021
  (checked while working on F-EXT-001, 2026-09-07)
- `docs/features/plugins/hosts/ofx-host.md` (F-EXT-001) — table of ecosystems,
  the price of readback on every frame, the industry maximum "blacklist instead of sandbox"; `docs/features/plugins/scripting/scripting-api.md` (F-EXT-002)
  — a neighboring form of extension and the price of a public API contract;
  `docs/user-feedback.md`, section "Round R.59 (2026-09-20)" — the owner's verbatim
  decision about the three models and the order of introduction.
