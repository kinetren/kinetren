---
id: F-EXT-001
title: Hosting third-party plugins — OFX for video and VST for audio
status: candidate
priority: B
cost: XL
category: plugins/hosts
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-fx, kt-engine, kt-gpu, kt-cache, kt-otio, kt-app]
invariants: [1, 2, 4, 5, 6, 7, 9, 10, 11, 12]
ui_rules: [13, 17, 20]
round: null
point: null
tests: []
depends_on: [F-MED-003, F-AUD-002, F-CLR-004, F-CLR-006, F-EXT-003]
checked: 2026-09-21
---

# F-EXT-001 — Hosting third-party plugins — OFX for video and VST for audio

## Essence

The editor installs a third-party installer, and in the effects picker next to ours a foreign effect appears: the same handles, the same automation, the same chain copy-paste, working in preview and in export. The seller of an effect and the seller of an editor are different people, and the editor does not need a release for the user to get a noise reducer that the editor never intended to write. The price is not visible at once and falls entirely on the host side: foreign code runs inside our process, inside our frame and inside our render. The fork is therefore not about a feature, but about whose behaviour we promise the user — ours, or ours plus someone else's.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | OpenFX (video) + DirectX/VST (audio) | Vegas's own video effects are written as OFX and lie as bundles inside the installation (`…\Vegas Pro 2026\OFX Video Plug-Ins\Vfx1.ofx.bundle\` — the path verified in F-CLR-004: user `.cube` files are put INSIDE this bundle). There are four levels of application: "Video plug-ins can be added to tracks, video events, source media files (via the Project Media window), or to the entire video project" (F-MPH-005). Audio is a second, independent ecosystem: "add/remove DirectX and VST effects from tracks, busses, and assignable FX chains", VST presets are stored in `.fxp`/`.fxb` (the VST2 bank format, not VST3). At the same time "full VST3 support" as of April 2026 is still a subject of requests on the forum, and the third-party plugin catalogue is scanned at every launch |
| DaVinci Resolve | 21 | OpenFX (video) + VST/AU on Fairlight (audio) | the reference OFX host: its own ResolveFX are also OFX, so a third-party plugin sits in the same Inspector and the same node as a native one (F-CLR-006). Wikipedia dates support to version 10 |
| Adobe Premiere | 26.x | NO OFX; its own Premiere Effect SDK; audio — VST3/AU | a wholly different ecosystem, and this is visible from the price list of the effect maker: the Sapphire licence "Covers Adobe (After Effects, Premiere Pro) and all OFX hosts" — Adobe is listed SEPARATELY from "all OFX hosts". Plug-ins for After Effects also work in Premiere: inside Adobe there is one API, outwardly it is closed |
| After Effects | 26.x | NO OFX; AE Effect SDK | not an NLE, but it is exactly what holds the second half of the effects market. The same price list shows a third ecosystem: Avid Media Composer is licensed SEPARATELY from Adobe and from OFX, and the "Multi-host license" costs twice as much as the single-family one |
| Final Cut Pro | 12.3 | neither OFX nor VST: FxPlug (video) + Audio Units (audio) | a fourth ecosystem, closed on macOS. Video plug-ins are FxPlug/Motion, audio is AU only: VST does not load in FCP at all, and this is a regular reason for "bought a plugin — it won't install" |

The total of the table matters more than its rows: there is not one ecosystem but four (OFX, Adobe, Avid, Apple), and they do not intersect — the maker of effects sells them as different products. "Supporting plugins" does not exist at all; there is "supporting OFX", and this immediately means "not supporting half the market". The price of the question for the user is also visible: a perpetual licence for one set of effects from Boris FX costs $1,865 for a family of hosts and $3,075 for all at once — that is, more than any of the five editors.

## The trick behind it

**1. Parameters in OFX are declarative — the UI is built by the HOST, and exactly for this reason a foreign plugin looks native.** The specification puts it directly: "The OFX Parameters Suite is the means by which parameters are defined and used by the plug-in but maintained by the host", and the plugin hands over the descriptions of parameters once, in the describe action, and "a host should cache away the description in some manner". So OFX has exactly our construction: for us `params_for!` in `src/libs/kt-model/src/project.rs:2308` produces `&'static [ParamSpec]`, and `widgets::params_grid` draws a grid from them — for them it is `paramDefine()` and the host inspector. Hence the non-obvious consequence: the same plugin in Resolve and in Vegas looks DIFFERENT, because it is not the plugin that draws it. And when a plugin nevertheless needs its own picture — an overlay on top of the viewer — there was exactly one way, and it rested on OpenGL: "The interact should either issue OpenGL calls (if the plugin is using OverlayInteractV1) to draw itself, or use DrawSuite calls (if using OverlayInteractV2)", moreover among the preconditions of the drawing action stands "the openGL context for this interact has been set". The alternative — DrawSuite, "API for host- and GPU API-independent drawing" — appeared only in **OpenFX 1.5, December 2024**. Twenty years of accumulated plugins with on-screen handles count on the host giving them a live GL context.

**2. The properties of a PLUGIN bind the HOST, not the other way round.** In OFX a plugin declares `kOfxImageEffectPluginRenderThreadSafety`, and the worst of its values means literally: "kOfxImageEffectRenderUnsafe — indicating that only a single 'render' call can be made at any time among all instances". Not "this instance", not "this frame" — all instances of this plugin across the whole host are rendered one at a time. One old effect on one track makes the whole pipeline single-threaded, and nothing in the interface shows it. The symmetric thing in audio: a VST plugin declares its own DELAY, and the host is obliged to compensate it — by shifting everything else. We have no concept of an effect's latency at all (verified by grep in F-AUD-002: everything that the engine calls `latency` is the output delay of cpal), and the `chain_tail` extends the end of the event and moves nothing. The pull path gives us lookahead for free — but only to OUR code: a foreign plugin has already delayed the output.

**3. GPU in OFX means CUDA, Metal, OpenCL and OpenGL. There is no Vulkan there.** Properties such as `kOfxImageEffectPropCudaRenderSupported` take three values — `"false"`, `"true"` and `"needed"`, and "needed" means that the plugin REQUIRES a GPU buffer: with CUDA enabled "kOfxImagePropData of each image… is a CUDA memory pointer". About Vulkan the header `ofxGPURender.h` says only "Additional GPU APIs, such a Vulkan, could use similar techniques" — that is, as a future possibility, not an existing suite. The host, meanwhile, must be able to roll back: if a plugin returned `kOfxStatGLRenderFailed`, "the host should retry the render with OpenGL rendering disabled". The practical meaning for a GPU pipeline: each CPU plugin in the middle of a chain is a download of the frame from the video card and an upload back on EVERY frame, and each GPU plugin is a requirement to speak the GPU API it was written for. Our executor is wgpu 30 (Vulkan on Linux), pinned to eframe by the same version (`src/Cargo.toml`, the comment "STRICTLY in sync with wgpu inside eframe"); there is neither CUDA, nor OpenCL, nor a GL context in the process.

## Pros

- The host describes the effect once and gets four mechanisms at once: automation, chain copy-paste, presets and search in the picker — because all of them work by the description of parameters, not by knowledge about the effect. For us this is already so (`ParamSpec` + one curve widget `automation::draw`), and opening the picker to foreign effects would not require new widgets.
- A plugin is an ordinary dynamic library in a known directory (`/usr/OFX/Plugins`, `/Library/OFX/Plugins`, `C:\Program Files\Common Files\OFX\Plugins`, plus `OFX_PLUGIN_PATH`), so the set of effects changes without a release of the editor and without rebuilding anything.
- The standard is open and free: "released under a "BSD" open source license", the owner is The Open Effects Association. The host pays nothing for the right to host and asks no one's permission.
- One and the same effect binary works in Resolve, Nuke, Fusion and Vegas, that is, a plugin bought by the user moves with him; for a new editor this is the only way to get a catalogue of effects on the day of release.
- When the GPU API of the host and of the plugin match, the frame does not leave the video card at all — the host hands over a pointer to the device, not a buffer in memory.

## Cons

- A plugin runs IN THE ADDRESS SPACE of the host, so its crash is a crash of the editor together with the unsaved session. Apple is the only one of the five that took this into account, by moving FxPlug 4 into a separate process; OFX has no such requirement, and Resolve/Vegas/Nuke live with plugins inside themselves.
- Compatibility rests on the ABI of a foreign binary: the host has been updated, the system has been updated, the C++ runtime has changed — and the plugin "has disappeared from the list", and only the user who has lost it can diagnose this.
- The project stops being self-contained. For a person without this plugin the effect turns into a hole — the same class as a missing source (F-MED-003), but it is fixed not by "point to the file again" but by "buy a licence".
- The licence of the plugin is its own and is tied to a family of hosts: the same set of effects for Adobe and for OFX are different purchases, and "all at once" costs twice as much. By opening the ecosystem, the editor in fact shifts part of its price onto the user and in return gets a dependence on someone else's price list.
- The parallelism ceiling for the whole host is set by the WORST plugin in the project (`kOfxImageEffectRenderUnsafe`), and this property is visible neither in the interface nor in the effect's passport.
- A mixed chain "our GPU effect → CPU plugin → our GPU effect" costs two transfers of the frame across the bus on each frame; the gain of the GPU pipeline is eaten by one plugin.
- The determinism of the render ceases to be a property of the editor: it becomes a property of a specific set of installed plugins and their versions, and there is nothing to check it with.
- The project format must store foreign parameters as an opaque blob (the host does not know their meaning), and therefore cannot migrate them or meaningfully diff them.
- EVERY launch pays the price, not only a project with plugins: the foreign catalogue has to be scanned in order to learn what is in it. Vegas has a separate long-standing request "let us skip the VST scan", with a launch flag and manual symlinks to get around it.

## What users say

> Currently using Mocha Pro Plugin (OFX, version 5.1.0 build 12413) within
> Fusion Studio 8.2 and am running into crashes, lost work and corrupted files.
> — Stephanein (s-boulet_cf987-2-2), Boris FX forum, 26.09.2016, https://forum.borisfx.com/t/mocha-pro-plugin-crash-ofx-error-and-freeze-on-loading-project/2255

> All works, but Sapphire crashes immediately Resolve as soon as i launch a
> filter.
> — dbsonic, Boris FX forum, 19.11.2021, https://forum.borisfx.com/t/sapphire-2022-crashing-in-resolve-17-4-2/12695

> When I use the BCC BeautyStudio filter, my playback and render times become
> 0.5-1.5 frames per second.
> — Bkosari (Threadripper 3960X + RTX 3090), Boris FX forum, 19.11.2020, https://forum.borisfx.com/t/bcc-beautystudio-is-very-slow-in-davinci-resolve/9628

> It does show an OpenCL error which means it would be dropping back to CPU
> mode which would explain the performance issues you're seeing.
> — Jason Clement (Boris FX), same thread, 25.11.2020, https://forum.borisfx.com/t/bcc-beautystudio-is-very-slow-in-davinci-resolve/9628

> When can we expect full VST3 support, specifically for audio plugins from
> Waves and Izotope?
> — seitel (michl), Boris FX forum, 01.04.2026, https://forum.borisfx.com/t/vst3-support-and-flickering-fx-window/21476

> There are certain VST3 plug-ins that do not work well with Vegas […] On
> first launch there is no way to skip scanning all of these.
> — papendlebury (Phil Pendlebury), Boris FX forum, 03.04.2026, https://forum.borisfx.com/t/renewed-request-for-the-ability-to-skip-vst-scanning/21744

How to read this. The first two quotes are about the fact that foreign code lives in the address space of the host: not "the plugin is bad", but "the editor crashes and loses work", and in the second case this happens RIGHT AFTER a routine update of the set of plugins. The first, strictly speaking, is about Fusion Studio, not an NLE — but Fusion is the same OFX host, and the class of failure is the same.

The third pair is the most valuable and the rarest: a user complains of 0.5–1.5 frames per second on a Threadripper with an RTX 3090, and an engineer of the PLUGIN MAKER in the same thread names the cause — the plugin fell from OpenCL to the CPU. That is, the pipeline on the GPU does not break because of the host or because of the hardware, but because one effect in the chain could not talk to the needed GPU API. This is exactly the third "trick behind the trick", confirmed from both sides.

The last two are about sound and are recent (April 2026): in an editor that has hosted VST for more than a decade, "full VST3 support" is still a question, and scanning a foreign catalogue at launch has turned into a separate request "let us switch this off". The author of the second quote also describes his own workaround — keeping a separate folder "VEGAS VST3" with symlinks only to the plugins he actually uses. This is exactly the "turned on once and off forever": the price of an open ecosystem is paid at every launch, even if not one plugin in the project is needed.

> Resolve keeps the frame internal in GPU memory. To pass to an OpenFX plug-in
> it has to read back from GPU to system ram, possibly convert component data
> bit depth, then pass this frame to the plug-in.
> — Norman Black, Blackmagic Design forum, 14.08.2018, https://forum.blackmagicdesign.com/viewtopic.php?t=77831

This is the most valuable quote of the section: it names the MECHANISM, not the symptom. The host keeps the frame in VRAM, the plugin wants it in system memory — and every frame travels there and back. For us this would hit invariant No. 7 exactly: the GPU executor would cease to be a "parameter of execution" and would become a second path with its own cost on every frame. The industry's answer to this is not isolation and not optimisation, but a cache: "Yes, it's slow. We basically have to cache anything using Sapphire or BorisFX plug-ins" (Marc Wielage, same thread, 09.02.2022).

> I have since done more tests and can say that on my system, at least, simply
> dropping the "Resolve FX Revival Noise Reduction" effect onto any clip results
> in that clip no longer playing back anywhere near real time — without even
> touching the settings (all set to 0).
> — Philip B (Philip Behrens), Blackmagic Design forum, 23.03.2022, https://forum.blackmagicdesign.com/viewtopic.php?t=156951

In the same thread he also notes that an effect DISABLED in the inspector returns the picture, but the playback drop does not; only deleting the effect together with its settings cured it. For us this is a direct warning about the semantics of bypass: our `fx_bypass` per slot and the master switch `ClipEvent.fx_off` must mean "the slot is not computed", not "the slot's result is not shown". With our own code this is our decision and our test; with someone else's — a promise that there is nothing to keep it with.

> You have a plugin which is crashing resolve. You will have to disable them one
> at a time to identify which is the bad/incompatible plugin.
> — Rohit Gupta, Blackmagic Design (DaVinci Resolve Software Development), 01.03.2015, https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=32953

The vendor's answer to a host crash caused by foreign code is "disable them one at a time and look for it". This is the industry's maximum: a blacklist based on the fact of a crash during the scan, not a sandbox. Genuine isolation appears in the industry exactly twice — FxPlug 4 at Apple (mandatory XPC + IOSurface) and Baselight (a render instance per frame in a separate process). Both are a rewritten host architecture, not "plugin support".

## How it fits Kinotrain

### What already exists

Unexpectedly much — and this should be admitted honestly, otherwise the verdict will look like fear rather than calculation.

- **A mechanism for loading foreign code already exists, and it is the only one.** `src/libs/kt-ffmpeg/src/loader.rs` loads libav* via `libloading` (`Library::open(Some(name), RTLD_LAZY | RTLD_LOCAL)`), and `src/libs/kt-ffmpeg/src/api.rs` resolves ~45 functions via `dlsym`. A plugin is exactly the same action; one would not have to write a loader from scratch.
- **Declarative parameters are literally the OFX model.** `ParamSpec` (`src/libs/kt-model/src/project.rs:2203`), the single declaration `params_for!` (`project.rs:2308`), the trait `HasParams`, and the host draws from them: `widgets::param_row` / `widgets::params_grid` (`src/apps/kt-app/src/widgets.rs:106` and `:123`). There is one difference, but it is structural: for us all of this is `&'static` and generated by a macro at compile time, for OFX it is runtime objects.
- **The picker is already designed for a foreign catalogue.** `src/apps/kt-app/src/fx_registry.rs` is labelled "scalable to hundreds of effects — a search line (name + keywords) and grouping by category", the registry is assembled once in a `OnceLock`, and effect names are deliberately NOT translated ("this is an industry term").
- **A foreign effect in a file already has a legitimate place.** In `src/libs/kt-otio/src/convert.rs` everything that is not `kt:*` goes into `ClipExtra.effects_foreign` (the declaration at line 47, the branch `other => …` in parsing the clip, the notice "unreadable effect … kept as foreign"). That is, an OFX effect that came to us from Resolve in `.otio` already today survives opening and re-saving — it is simply not counted.
- **A hole instead of a missing resource is a solved class.** `src/libs/kt-media/src/placeholder.rs` returns a deterministic dark frame with the name of the missing file, drawn with a BUILT-IN 3×5 font precisely so that "the frame is deterministic and identical in preview, export and tests"; notifications — `App.notices` (`src/apps/kt-app/src/app.rs:277`), tests t209/t210, the precedent of the GPU-loss notification — t185.
- **An injection point for the executor exists.** `VideoGpuExec` (`src/libs/kt-engine/src/video_plan.rs:134`) — the engine knows only the trait, while wgpu lives in kt-gpu. But what is injected there is OUR executor of OUR plan, not foreign code inside the plan.

### What is missing

- An effect is nowhere an object for us: `FxInstance` is a closed enumeration (`src/libs/kt-model/src/project.rs:1901`), and opening it is not "adding a variant" but changing the way the effect exists in the model, in the file, in the cache and in the UI.
- The post-FX cache key is `fx_chain_hash` (`src/libs/kt-engine/src/video.rs:153`): xxh3 of `format!("{fx:?}|{auto:?}|…")`. The Debug print of an enumeration does not express "plugin such-and-such, version such-and-such, build such-and-such", and without this invariant No. 4 (the cache is the only truth) starts lying at the first plugin update.
- There is no concept of effect latency (F-AUD-002, verified by grep), and a VST cannot be hosted without it.
- Neither a GL context, nor CUDA, nor OpenCL in the process: the only GPU API is wgpu 30, pinned to the version inside eframe (`src/Cargo.toml`).
- There are no helper processes: in all the working code there are exactly two `Command::new` — opening a directory via `bash` (`src/apps/kt-app/src/util.rs:327`) and `git` in `src/libs/kt-version/build.rs`. Out-of-process hosting (Apple's path) is a new subsystem from scratch: IPC, frame transfer, supervision, timeouts.
- There is neither a sandbox, nor a signature, nor a whitelist — no trust model for a third-party binary at all.

### What stands in the way

1. **Invariant No. 11 — determinism.** Audio byte-for-byte always, video on the CPU — against a committed reference. The discipline with which we hold this INSIDE ourselves is recorded in `src/libs/kt-fx/CLAUDE.md` and it is draconian: "the SIMD kernel MUST be BYTE-FOR-BYTE equal to the scalar one on any CPU", "Forbidden: FMA (`fmadd` rounds differently), reassociation of sums/taps, rcp/rsqrt approximations, f32 instead of f64". A foreign binary promises none of this and cannot promise it: it itself chooses the SIMD path by CPU, itself decides how many threads to start, and may assemble tiles in the order of their completion. So it is impossible in principle to commit a reference frame with a plugin, and "preview == export byte-for-byte" ceases to be a property of the product — it becomes a property of the machine.
2. **Invariant No. 7 — one composition code.** Our FX path runs twice and converges: `apply_video_chain_pooled` (`src/libs/kt-fx/src/video.rs:61`) and its mirror `chain_ops` (`src/libs/kt-gpu/src/engine.rs:124`), conformance is guarded by t181. A plugin runs ONCE: a CPU-only plugin forces the GPU path to download the frame in the middle of the chain, a CUDA plugin requires an API that we do not have. In both cases the same plan gets two DIFFERENT results depending on the backend — exactly the "second path" for whose absence the invariant was written. We already have the precedent, and it is fresh: item 250 of R.37 refused to teach both executors frame mixing and put the mix into the COMMON code of the plan. With foreign code such a move is impossible: there is no common code.
3. **Invariants No. 2 and No. 5 — ownership of state.** For us `FxStateStore` (`src/libs/kt-engine/src/fxstore.rs`) belongs to the audio-render thread, "neither prefetch nor the UI enters here", and `ClipProcessor` by the crate's rule is "Send, not Sync — one thread owns the state". OFX assumes exactly the opposite: "An instance must be able to handle simultaneous actions called on the main and render threads", and the locks are expected FROM THE HOST ("fine grained", at the level of individual calls). This is verbatim what invariant No. 2 forbids: "no mutexes around FX, no access from prefetch/meters". Invariant No. 2 was written after a concrete defect of the old engine ("4 threads without locks over a shared delay line") — and the cost of violating it is known: clicks, not a beautiful architectural complaint.
4. **Invariant No. 6 and sound.** A VST declares a delay and requires compensation; the compensation is a shift of the output relative to the master clock of cpal. Plus invariant No. 1 (inside the engine always f32 48 kHz/2ch): a plugin working at another frequency puts a resampler into the path, and this is exactly the defect of the old engine for whose sake invariant No. 1 was written.
5. **Invariant No. 10 and release delivery.** We do have dlopen, but the meaning of the rule lies not in dlopen but in the result: ONE file works everywhere. The script `src/scripts/release-build.sh` builds in an Ubuntu 22.04 container and checks two things — that `ldd … | grep libav` is empty and that the `GLIBC_` ceiling is not above 2.35. A plugin brings its own dependencies: its own libstdc++, its own CUDA runtime, its own Qt. The check "our binary is clean" then means nothing, and it is we who will fall over at the user's. A separate mine is symbols: we deliberately load FFmpeg with `RTLD_LOCAL` ("we do not pollute the global namespace"), while a plugin may drag its OWN copy of libav* of another version into the same process.
6. **The crate rule of kt-fx.** "Edit the formula — edit the scalar, the AVX2 kernel AND the WGSL shader of kt-gpu in one commit (t181 guards the third pair)". Foreign code has neither our scalar nor our shader, so the rule is not applicable to it — and it is not decoration, it is precisely the mechanism by which we hold invariants 7 and 11.
7. **UI: t100 and parameter labels.** Test 100 requires that EVERY non-ASCII character that the program can show exist in the loaded fonts, and for this it scans the sources of kt-app and the catalogues `i18n/*.toml`. The labels of a plugin's parameters come FROM THE PLUGIN, and `param_label` (`src/apps/kt-app/src/widgets.rs:84`) returns an unknown name as it is. A plugin with Japanese labels will produce tofu squares, and t100 will not catch it — there is nothing to scan. Here also rule 20 of the checklist applies ("nothing grows back or switches off by itself"): scanning the plugin catalogue at start is exactly "grows back by itself", and without scanning the plugins do not appear. What this costs in life is visible from Vegas: there the request "let us skip VST scanning" has hung for years, and people get around it with symlinks. For us at startup there is a hard requirement of the opposite sign — "the UI heats the system only when moving" (invariant No. 12).
8. **Project format.** Our effects travel as the standard `Effect` with `effect_name: "kt:<tag>"` and a body in `metadata.kt` (`src/libs/kt-otio/src/convert.rs:562`), and t144–t147 stand on this (idempotence, passthrough, versions). A plugin effect is a plugin identifier, its version and an opaque blob of parameters; no migration and no round-trip guarantees extend to the blob, and "read your file in a year" becomes a promise that depends not on us.

### Which invariants it touches

- **No. 11 (determinism)** — a direct contradiction, the most expensive one: foreign code has no references and cannot have them.
- **No. 7 (one composition code)** — the plugin runs on one backend out of two.
- **No. 2 and No. 5 (ownership of state, one thread per decoder/FX)** — the OFX model requires the host to provide locks around the plugin.
- **No. 6 (the cpal callback is the master clock)** and **No. 1 (f32 48 kHz/2ch)** — via the latency and the sample rate of a VST.
- **No. 4 (the cache is the only truth)** — the post-FX key does not express the identity and version of the plugin.
- **No. 9 and No. 10 (export is in-process, the binary is clean)** — cleanliness is checked only for our binary; a plugin makes the check meaningless.
- **No. 12 (the UI heats the system only when moving)** — scanning a foreign plugin catalogue at startup.
- **UI rules 13, 17, 20** — the plugin manager is a permanent area; "the plugin's own UI" next to our parameter grid are paired ways of doing the same thing; automatic scanning of directories is what grows back by itself.

## Estimate

| What | Answer |
|---|---|
| Size | XL — not a round, but a change of contract: invariants 7 and 11 are rewritten from "always" to "if there are no plugins" |
| Where the main work is | kt-model (the effect stops being an enumeration), kt-fx + kt-engine (calling foreign code inside the chain), kt-gpu (readback in the middle of the chain, a second GPU API or a GL context), kt-cache (the identity and version of the plugin in the key), kt-otio (a foreign effect as a blob), kt-app (plugin manager, foreign labels, foreign overlays) |
| Project format | a new version is mandatory; a class "an effect I do not have" appears, and it is not migratable |
| Tests that will have to be written | there is nothing to write them with — that is the point. Existing ones would have to be WEAKENED: t11/t67 (golden export), t181/t182/t184 (conformance and byte-for-byte GPU), t198 (SIMD parity) would get a caveat "with an empty list of plugins". Weakening regression tests is a separate decision of the owner |
| Depends on | — (neighbours: F-MED-003 "a hole instead of a resource", F-AUD-002 "latency", F-CLR-006 "why a chain and not a graph") |
| Breaks compatibility | yes, in the worst form: the project stops being self-contained, and this cannot be fixed by our code |

## Verdict

**Rejected.**

**First — about the direct requirement, honestly.** In `docs/user-feedback.md`, in the table of cross-cutting requirements, there is a line: "**no own build of FFmpeg and no plugins**" — with the note "only runtime dlopen of the system one" and a reference to invariant No. 10. The same wording is repeated in `docs/architecture.md:73`: "no plugins and no own build of FFmpeg". The line is called "System FFmpeg", and from the context "plugins" there means FFmpeg plugins (its own codecs and filters, which would have to be carried along). Therefore there is NO literal prohibition of OFX/VST in this phrase, and pretending that the question is closed by quoting it would be a fraud. What closes the question is not the letter of the requirement but its reason: the user asked for one program that works on someone else's machine with what is on that machine, and does not drag along foreign binary baggage. Hosting plugins is exactly foreign binary baggage, only it is chosen not by the author but by the user, and the author answers for the crash all the same.

**Next — the engineering reason, and it is stronger than the requirement.** Three of our invariants are built so that a plugin cancels all of them at once, not one by one. No. 11 says "render bytes are a property of the product, they are committed"; foreign code makes the bytes a property of the machine. No. 7 says "the composition code is ONE, the backend is a parameter of execution"; a plugin runs on one backend out of two and turns the backend back into a second path. No. 2 says "the FX state is touched by exactly one thread, no mutexes"; OFX requires in plain words from the host fine-grained locks around the plugin, and `kOfxImageEffectRenderUnsafe` gives the plugin the right to serialise our entire render. None of the three is "expensive to bypass" — each was written after a concrete defect of the old engine (clicks from four threads without locks, divergence of preview and export), and the price of cancelling them is measured not in person-days but in the return of those defects.

**The condition under which the note is reopened — exactly one, and it is not "plugins".** If foreign effects are ever needed, they should be taken not into the chain but INTO THE MATERIAL: run a piece through the foreign tool and get an ordinary file, which thereafter lives like any other source. Then foreign code is found neither inside our frame, nor inside our thread, nor inside our reference: determinism remains ours, because we always render our own material. The mechanism for this we ALREADY have on both sides — `BakeMorpher` shows how a destructive operation becomes ordinary data, and `src/libs/kt-media/src/placeholder.rs` shows what to do when a source is absent. This is not "plugins in the editor", it is "the editor can accept someone else's work", and its cost is two orders of magnitude smaller.

**What is NOT the reason for the rejection** (so that a year from now people do not argue about the wrong thing): not the volume of code, not the complexity of the OFX API and not "we have few effects". The picker is already designed for hundreds of effects, our declarative parameters are the very same as in OFX, and the dynamic-library loader is written and works. The rejection is about what we promise the user regarding bytes, threads and one file that works everywhere.

**Revision of R.59 (2026-09-21) — the owner's decision.**

The verdict above is NOT erased or rewritten: it remains what it was — an analysis of the price. Only its legal status changes, and it is changed by the direct will of the owner, not by new reconnaissance. In `docs/user-feedback.md`, section "Round R.59 (2026-09-20)", it is recorded verbatim: "**all three; first there will be built-in and dynamic on dlopen in the future, openfx we will bring back in the future**" — and there too, in "Owner's decisions": "Plugins — all three models at once in the interfaces… The verdict of F-EXT-001 ("rejected") is explicitly revised by this". Therefore the status of the note is changed `rejected` → `candidate`, and `checked` is updated.

What is NOT withdrawn. The engineering argument has remained entirely: invariants 11, 7 and 2 are still incompatible with foreign code that keeps state between frames, has a tail, or requires locks around itself. What has changed is the use of this argument — it has ceased to be a reason for REJECTION and has become a list of CONDITIONS. The conditions are set out in a separate note **F-EXT-003 (`plugins/hosts/plugin-abi-and-sandbox.md`)**: a plugin is a pure function of the frame, without clocks or I/O; if it declares state or a tail, it is not loaded at all; a panic is caught, the frame passes through with a notice; a plugin never gets into the golden-cycle registry. The order of introduction — built-in ones first, then dynamic libraries on dlopen, the OFX bridge later; in R.59 only the types are written, without a runtime.

The revision condition named by this note itself — "take foreign effects not into the chain but INTO THE MATERIAL" — is not withdrawn and remains a cheap path: it does not compete with the F-EXT-003 contract, but serves it as an emergency exit for everything that does not pass the contract.

## Sources

- https://openfx.readthedocs.io/en/main/Reference/ofxParameter.html — "The OFX
  Parameters Suite is the means by which parameters are defined and used by the
  plug-in but maintained by the host"; "A plugin needs to define it's
  parameters during a describe action… OfxParameterSuiteV1::paramDefine()"
  (accessed 2026-09-07)
- https://openfx.readthedocs.io/en/main/Reference/ofxImageEffectAPI.html —
  caching of descriptions by the host; "An interact is an OFX object that is used to
  draw custom user interface elements, for example overlays on top of a host's
  image viewer or custom parameter widgets" (2026-09-07)
- https://raw.githubusercontent.com/AcademySoftwareFoundation/openfx/main/include/ofxInteract.h
  — "The interact should either issue OpenGL calls (if the plugin is using
  OverlayInteractV1) to draw itself, or use DrawSuite calls (if using
  OverlayInteractV2)"; the precondition "the openGL context for this interact has
  been set" (2026-09-07)
- https://raw.githubusercontent.com/AcademySoftwareFoundation/openfx/main/include/ofxDrawSuite.h
  — "API for host- and GPU API-independent drawing", "Added in OpenFX 1.5"
  (2026-09-07)
- https://raw.githubusercontent.com/AcademySoftwareFoundation/openfx/main/include/ofxImageEffect.h
  — "kOfxImageEffectRenderUnsafe — indicating that only a single 'render' call
  can be made at any time among all instances" and two other values of
  `kOfxImageEffectPluginRenderThreadSafety` (2026-09-07)
- https://raw.githubusercontent.com/AcademySoftwareFoundation/openfx/main/include/ofxGPURender.h
  — the CUDA/Metal/OpenCL properties with the values "false"/"true"/"needed";
  "kOfxImagePropData of each image… is a CUDA memory pointer"; "the host should
  retry the render with OpenGL rendering disabled"; about Vulkan only
  "Additional GPU APIs, such a Vulkan, could use similar techniques"
  (2026-09-07)
- https://openfx.readthedocs.io/en/main/Reference/ofxThreadSafety.html — "An
  instance must be able to handle simultaneous actions called on the main and
  render threads"; the expectation from the host of "fine grained" locks at the level of individual
  calls (2026-09-07)
- https://openfx.readthedocs.io/en/main/Reference/ofxPackaging.html — the bundle
  `NAME.ofx.bundle`, "native dynamic shared objects" on Linux, the directories
  `/usr/OFX/Plugins`, `/Library/OFX/Plugins`, `C:\Program Files\Common
  Files\OFX\Plugins` and the variable `OFX_PLUGIN_PATH` (2026-09-07)
- https://en.wikipedia.org/wiki/OpenFX_(API) — "OpenFX (OFX)… is an open
  standard for 2D visual effects or compositing plug-ins", version 1.5
  (December 2024), "released under a "BSD" open source license", the owner The
  Open Effects Association; hosts — DaVinci Resolve (from version 10), Nuke (from
  4.5), Fusion (from 4.04), Natron (2026-09-07)
- https://help.magix-hub.com/video/vegas/23/en/content/topics/8-design/addvideofx.htm
  — "Video plug-ins can be added to tracks, video events, source media files
  (via the Project Media window), or to the entire video project" (2026-09-07)
- https://help.magix-hub.com/video/vegas/23/en/content/topics/9-audio/audio_plug-ins.htm
  — "add/remove DirectX and VST effects from tracks, busses, and assignable FX
  chains", VST presets in `.fxp`/`.fxb` (2026-09-07)
- https://borisfx.com/products/sapphire/ — the composition of hosts and SEPARATE
  licensing: "Covers Adobe (After Effects, Premiere Pro) and all OFX
  hosts (DaVinci Resolve, Foundry Nuke, Autodesk Flame, Blackmagic Fusion
  Studio, VEGAS Pro, and other OFX-compatible applications)", a separate
  licence for Avid, and the "Multi-host license" costs twice as much (2026-09-07)
- https://forum.borisfx.com/t/mocha-pro-plugin-crash-ofx-error-and-freeze-on-loading-project/2255
  — "crashes, lost work and corrupted files" with an OFX plugin inside Fusion
  Studio, 26.09.2016 (2026-09-07)
- https://forum.borisfx.com/t/sapphire-2022-crashing-in-resolve-17-4-2/12695 —
  a Resolve crash right after an update of the set of plugins, 19.11.2021; in the same
  thread a Boris FX employee names the cause (Metal on a machine with two GPUs) and
  promises a fix in 2022.01 (2026-09-07)
- https://forum.borisfx.com/t/bcc-beautystudio-is-very-slow-in-davinci-resolve/9628
  — 0.5–1.5 fps on an RTX 3090 and the explanation of the manufacturer's engineer: the plugin
  fell from OpenCL to the CPU, 19–25.11.2020 (2026-09-07)
- https://forum.borisfx.com/t/vst3-support-and-flickering-fx-window/21476 and
  https://forum.borisfx.com/t/renewed-request-for-the-ability-to-skip-vst-scanning/21744
  — the state of VST3 in VEGAS and the cost of scanning a foreign catalogue at launch,
  April 2026 (2026-09-07)
- https://forum.borisfx.com/t/vegas-pro-21-dont-load-sapphire-ofx-plugin-when-startup-the-poject/17527
  — the plugin lies in the standard directory `C:/Program Files/Common Files/…`, and the
  host complains when a project is opened, 30.10.2023 (2026-09-07)
- Our own notes as sources on Vegas and Resolve: F-CLR-004 (the path
  `…\Vegas Pro 2026\OFX Video Plug-Ins\Vfx1.ofx.bundle\`), F-MPH-005 (four
  levels of application), F-CLR-006 (ResolveFX in nodes), F-AUD-002 (latency and
  the absence of PDC in NLEs).

Caveats on trust. The OFX specification was read from the primary source
(openfx.readthedocs.io and the headers of the AcademySoftwareFoundation/openfx repository)
— this is the most reliable part of the note. The lines for Premiere, After Effects and Final Cut
Pro in the table "Who has it and how" rest on the well-known structure of their SDKs, not on
official help: `helpx.adobe.com` does not open at all from this environment. The separation of ecosystems is confirmed indirectly, but the primary source
is the Boris FX price list, where Adobe, OFX and Avid are licensed separately, and VEGAS
Pro is listed inside the OFX group. **`[NOT VERIFIED]` against a primary source:**
the out-of-process model of FxPlug 4 at Apple; the absence of VST support in Final Cut
Pro and the presence there of Audio Units; the restriction of third-party OFX by the Resolve Studio edition
(the blackmagicdesign.com page about Studio speaks only of "over 45
additional GPU and CPU accelerated Resolve FX" and says nothing about OFX); support of
VST3 and AU in Premiere. No host was checked live.

About the platforms. The VEGAS forum has moved to Boris FX (`vegascreativesoftware.info`
returns a 301 to `forum.borisfx.com`), and this turned out to be a stroke of luck: users of Resolve, Fusion and Flame with the same plugins also live there, so all six
quotes come from one platform, yet from four different hosts. It was read through the
Discourse JSON output (`/search.json`, `/t/<slug>/<id>.json`), since ordinary
search pages give only navigation; each quote was checked for verbatim wording by a second
request. `reddit.com`, `web.archive.org` and the search of
`forum.blackmagicdesign.com` (403 on `search.php`) are unavailable from this environment, the `WebSearch` budget in the session is exhausted, and Brave/DuckDuckGo/Mojeek
returned 429, CAPTCHA and 403 respectively.
