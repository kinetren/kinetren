---
id: F-HLP-030
title: First launch on someone else's hardware: what the program says about itself
status: candidate
priority: B
cost: M
category: help/onboarding
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2]
we_have: partial
crates: [kt-app, kt-ffmpeg, kt-media, kt-fx, kt-engine]
invariants: [9, 10, 12]
ui_rules: [9, 13, 19]
round: null
point: null
tests: []
depends_on: [F-HLP-026, F-HLP-011, F-HLP-013, F-HLP-033]
checked: 2026-09-07
---

# F-HLP-030 — First launch on someone else's hardware: what the program says about itself

## Essence

The editor has launched the program for the first time and does not know the main thing: what he sees right now — is it the program's own ability, or is it his machine that can do this. Hence the single question of the section: which capabilities were found out ONLY just now and ONLY here (interface acceleration, GPU for effects, the version of the FFmpeg found, the CPU instruction set, a hardware encoder, the cache size), and which of them the program names out loud, and which it leaves to the user to figure out from symptoms.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Options → Preferences → Video ("GPU acceleration of video processing") and → File I/O ("Hardware decoder to use") | the found device and the acceleration mode DO exist, but one has to go and look at them in two different tabs, and the program does not speak about them by itself. Indirect confirmation of the strength of this gap — a pinned support post: it asks the user to separately report the status of both settings and attach screenshots of both tabs. There is no first-launch wizard; [NOT VERIFIED] live |
| Adobe Premiere | 26.x | System Compatibility Report (at LAUNCH) | the only mechanism in the core that speaks about the hardware by itself: at startup the video driver version is checked against known incompatible ones and a report is shown. That is, the check does not wait until the user runs into a symptom. From Adobe's documentation; the same source is cited by F-HLP-026 |
| DaVinci Resolve | 21 | Preferences → Memory and GPU | "what was found" is done as a LIST: GPU processing mode (Auto/CUDA/OpenCL/Metal) and GPU selection mode (Auto/manual) with a list of the found devices and checkboxes. The key consequence of the device: an empty list is itself a diagnosis, visible without a single performance number. The "Quick Setup" wizard at first launch — [NOT VERIFIED], it could not be confirmed from the primary source |
| Final Cut Pro | 12.3 | none | the set of capabilities is fixed by the platform: the hardware and the media engine are Apple's, and "what if nothing was found for you" does not arise as a question. The system information is handed over to the OS. [NOT VERIFIED] |
| Adobe After Effects | 26.x | Preferences → Previews → GPU Information… | the view closest to our task: the dialog lists not a device but VERDICTS ON CAPABILITIES — "Fast Draft: Available", the amount of texture memory, the vendor/device/version of OpenGL, the version of the CUDA driver, and "Current Usable Memory … (at application launch)", that is, a fact with the moment of its validity explicitly named. The content is confirmed by a verbatim transcription by a user (see the quotes); the presence in 26.x — [NOT VERIFIED], `helpx.adobe.com` did not open this time either directly or through a proxy |

**Outside the core.** One line is needed because in the whole core there is not a single program whose set of capabilities, as ours, is assembled from SYSTEM libraries rather than shipped by the installer: Adobe, BMD and Apple have the runtime inside the distribution, and the question "what got picked up on this machine" does not arise for them.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | Preferences → System → Cycles Render Devices | the user chooses a backend (CUDA, OptiX, HIP, oneAPI, Metal), and only then "Enable the available devices for the chosen backend" — the list depends on the drivers installed IN THE SYSTEM. The manual separately describes a fallback with a named cost: when video memory is short, Cycles moves to system memory, "This has a performance impact" |

## The trick behind it

The payload that a single status bar line does not hold here is — a **TABLE OF VERDICTS ON CAPABILITIES, TRUE ONLY ON THIS MACHINE AND ONLY AT THE NAMED MOMENT**: five or six lines "can / cannot / not found", which have different ages (some are known on the first frame, some after opening the first file, some only at the moment of rendering), and not a single on-screen representative to hover the mouse over. A status line carries one fact at one moment and by its nature cannot carry a table with dates. For us the payload will go into the body of the "Diagnostics" window as a block under the origin of the build (F-HLP-026), and in the line itself will remain exactly what is already there today: one phrase at first launch, and only when a capability came out WORSE than promised on the site.

**1. One must name the CONSEQUENCE, not the fact, and we have this already written — once.** `msg::SOFTWARE_RENDER` (`src/apps/kt-app/i18n/en.toml:505`): "No graphics acceleration (software render) — the UI may lag, preview glow is off". A cause plus two verifiable consequences — by such a phrase a person decides whether to fix it or put up with it. The strongest line of someone else's dialog is built the same way: AE does not write "GeForce found", but "Fast Draft: Available", that is, it answers about the CAPABILITY. A device name ("GPU: AMD Radeon Graphics (RADV …) / Vulkan") without a verdict means nothing: it is precisely in this combination — the card is there, the effects run on the CPU — that the owner wrote the complaint R.31 item 1.

**2. "Present in the build" ≠ "will work", and we apply an honest check to ourselves, and a dishonest one to the user.** The automatic encoder choice (invariant 9) does not trust `avcodec_find_encoder_by_name` and opens the candidates for real; in `src/libs/kt-ffmpeg/src/muxer_impl.rs:76` this is said literally: "h264_nvenc is present almost everywhere, but without libcuda/hardware `avcodec_open2` fails". And the drop-down list of encoders in the render window (`src/apps/kt-app/src/render_dialog.rs:286`) is filled by that very check which this comment declares insufficient — that is, it offers to choose something that may not be there. The structure `EncoderChoice { name, hardware }` (`src/libs/kt-ffmpeg/src/lib.rs:84`), the only one in the tree that answers "was a hardware one obtained", is not called from anywhere.

**3. The first-launch report has exactly one addressee — the one who in an hour will write "it doesn't work for me".** The difference is visible in two lists. VEGAS support keeps pinned a request to report the status of the hardware decoder and the status of GPU acceleration, and these are TWO different preference tabs plus a third place (File Properties) for the answer to "which plugin decodes the file". For us the same conversation took place in R.31 item 1, with an answer that was ALREADY on the screen: the notice brought the raw cause inside itself (the argument `{0}` in `msg::VIDEO_FX_CPU`), the owner quoted the text of the shader validation, and item 194 came out of the complaint. A report is worth not because it is pretty, but because it shortens the circle "symptom → question → screenshot → guess" to a single paste into a chat. What to do with it next — "copy" and where to send it — is the subject of F-HLP-026; here only the composition and the moment.

## Pros

- It answers a question that is otherwise unresolvable in principle: the user cannot distinguish "the program cannot do it" from "nothing was found on this machine" — both situations look the same (no speed, no button, no difference from a setting). No documentation closes this: the answer exists only on his machine.
- A phrase with a consequence turns "it lags" into "understandable why and what we pay": two concrete losses (the interface, the glow) cost less than any conversation about architecture and do not require the reader to know the word "rasterizer".
- The raw cause INSIDE the message turns a complaint into a bug report: without the substituted `{0}` R.31 item 1 would sound like "no acceleration", and item 194 (visibility VERTEX_FRAGMENT) would have been searched blindly.
- A list of what was found that can be looked at makes EMPTINESS a diagnosis: a person whose list of devices is empty already knows the answer and does not measure fps. This is a functional device of a resolve-like panel, not its decoration.
- Smart defaults are cheaper than a wizard, silently, exactly where the answer is computed: we already choose the interface language from the OS locale (R.19 item 3, `src/apps/kt-app/src/i18n/detect.rs`), and the user does not answer a question whose answer he does not know in the first minute.

## Cons

- Every line of the report is a promise, true at a MOMENT, and the moment will have to be written literally (as AE writes "at application launch"). Our spread is greater than that of all five: FFmpeg is loaded lazily at the first access to media, the GPU executor may get poisoned in the middle of a session (`VideoBackendStatus::Poisoned`), and a report taken at startup will lie by midday.
- An early report costs the launch. To print the FFmpeg version on the first frame, it has to be LOADED on the first frame: `init()` in `src/libs/kt-ffmpeg/src/lib.rs:146` does a dlopen of the libraries and parses the function table. The laziness was not made out of carelessness, and exchanging the startup time for a line is a decision of the round, not of a note.
- A push at first launch makes the program's first phrase bad news. By rule 19 such a phrase must name the place of return, and there is no return in the interface at all: the only way to return/force the GPU is an undeclared environment variable `KT_VIDEO_GPU`, which is present neither on the site nor in the translation catalogs.
- An honest list of encoders costs real discoveries: five `avcodec_open2` calls with closing, and a failed nvenc without a driver is not an instantaneous operation. Doing this synchronously when the render window opens is not allowed (the same class as item 196: an expensive function was called on every frame and dropped the FPS).
- The longer the report, the more it resembles a "specifications page" where half the lines affect nothing — rule 13 works against this very note. The limiter is simple and must be named in advance: only what has an OBSERVABLE consequence for editing gets into the report.

## What users say

> Status of GPU acceleration of video processing. Is it ON or OFF? Which GPU?
> See Options > Preferences > Video.
> — NickHope, pinned post "IMPORTANT! Information required to help you",
> Boris FX forum (VEGAS), 28.03.2026,
> https://forum.borisfx.com/t/important-information-required-to-help-you/21172

The same post asks in a separate item for the status of the hardware decoder (Options >
Preferences > File I/O) and the file properties — "because we need to know which
"plugin" is being used to decode your media". Three places in the interface to assemble the answer to one question; F-HLP-026 quotes this same post about the build version — there it is the first item of the list, here the eighth and ninth.

> Disabling GPU acceleration of video processing is the only way I’ve gotten
> them to stop, but that comes at the cost of poorer timeline playback.
> — crusherfire, thread "Patch: Vegas Pro 2026 Build 143", 20.06.2026,
> https://forum.borisfx.com/t/patch-vegas-pro-2026-build-143/24204

This is the reverse side of the silence: the setting is turned blindly and paid for with speed, because the program did not say what exactly it gives and what it risks.

> In Preferences > Previews > GPU Information, there is written: Fast Draft:
> Available.
> — Adriano Castaldini, thread "After Effects Texture Memory doubt", Creative COW,
> https://creativecow.net/forums/thread/after-effects-texture-memory-doubt/
> (accessed 2026-09-07)

He reprinted the dialog in full, and in the reprint the device is visible: first the verdicts on capabilities (Fast Draft, ray-tracing), then the volumes and driver versions, and separately "Current Usable Memory: 9,47 GB (at application launch)" — a figure with an explicit date of validity.

> Check Texture RAM settings. Is there a reason you have GPU acceleration OFF?
> — martinb (Boris FX), thread "Mocha laggy interface", 26.04.2021,
> https://forum.borisfx.com/t/mocha-laggy-interface/11105

The direction of discovery is always the same: an expert notices it, not the owner of the machine. The user at the same time honestly listed "GPU acelleration is turned off" among the symptoms — he saw the setting and did not know that it was the cause.

## How it fits Kinotrain

The industry argument here is the second one. The first — local, and unique of its kind:
**our first launch on someone else's hardware has already happened, and it was R.31.** The owner
launched the build on AMD/RADV and wrote verbatim: "Right now my computer says there is no gpu acceleration" — and right after it the text of a pipeline creation error plus the line of the perf panel "GPU: AMD Radeon Graphics (RADV RAPHAEL_MENDOCINO) / Vulkan, FX: CPU". Four items (194–197) came out of one message. This is the only known test of the domain we have, and it says one thing: what works is not "a message about the hardware", but a message that carries the CAUSE.

### What already exists

- **Software-render detection and the notice** — `src/apps/kt-app/src/app.rs:5140` (`sw_render` is computed once), the adapter name in `gpu_info` (`:5145`), the warning `msg::SOFTWARE_RENDER` (`:5156`). The wording is exemplary and the only one in the catalog that names two concrete consequences.
- **Detection of the video-FX GPU executor** — `init_video_gpu` (`app.rs:340`): the engine is built on the eframe device, a failure gives `msg::VIDEO_FX_CPU` with the raw cause as an argument (`app.rs:357`). `KT_VIDEO_GPU=0` — a forced CPU (`app.rs:343`).
- **Loss of the GPU in the middle of a session** — `app.rs:772`, the notice exactly once, guarded by t185 (`src/apps/kt-app/src/layout_tests.rs:3066`); the backend status is honest (`VideoBackendStatus::Cpu/Gpu/Poisoned`, `src/libs/kt-engine/src/player.rs:180`).
- **The perf panel already answers two of the six questions**: the line "GPU: {name} / {backend}" and "FX: GPU ({0})" / "FX: CPU" (`app.rs:3757`, `:3760`).
- **The "Diagnostics" window** (`app.rs:5064`) shows the video cache limit as a number: `perf.cache_occupancy` = "Video cache usage: {0}% ({1} / {2} MB)" (`app.rs:5084`), and it takes it from the REAL `max_video_bytes()`.
- **Automatic encoder choice with a real opening check** — `src/libs/kt-ffmpeg/src/muxer_impl.rs:15` (the order nvenc → vaapi → qsv → libx264 → mpeg4) and the probe loop at `:95`; the FFmpeg log is muted during the probes, because failed candidates are the norm. The chosen name reaches the user in the render summary (`render.summary`, `src/apps/kt-app/src/render_dialog.rs:542`).
- **The FFmpeg major version is determined at runtime** by `avcodec_version()` (invariant 10), the result is kept in `BackendInfo` (`src/libs/kt-ffmpeg/src/lib.rs:171`).
- **Notifications are not lost**: `warn()` puts the text both into the status bar and into the bell (`app.rs:1956`, window `app.rs:1106`) — the mechanism of F-HLP-013.
- **The language is already chosen by a smart default**, not by a question: `src/apps/kt-app/src/i18n/detect.rs` (LC_ALL → LC_MESSAGES → LANG), the decision of R.19 item 3; the user's choice overrides it and is saved.
- **The site already makes promises about this machine**: "Your GPU encodes it … picked automatically, with a software fallback that actually works" (`site/content/en.toml:55-56`) and "FFmpeg 6, 7 or 8 installed system-wide" (`:262`). That is, the expectation of the newcomer is formed BEFORE the first launch.

### What is missing

- **The set of CPU instructions is computed and thrown away.** `kt_fx::simd::simd_name()` (`src/libs/kt-fx/src/simd.rs:42`), with the comment "String for the perf panel/diagnostics", is not called from kt-app, nor kt-cli, nor the tests. This is exactly the capability about which the owner asked "carefully, what if there are no super-modern instructions" (R.32, item 198): the fallback is made, but the answer to "so which one do I have" is shown nowhere.
- **Not a single line about FFmpeg, except counters.** `backend_info()` without callers — a finding of F-HLP-026, not reopened here; a consequence for the first launch is added: in the perf panel "FFmpeg: open … read …" is a literal in the code (`app.rs:3790`), and the version does not take part there. A user with FFmpeg 6 and a user with 8 see the same screen.
- **Hardware decoding is not used at all**, and this is not said anywhere: `open_decoder` (`src/libs/kt-ffmpeg/src/source_impl.rs:665`) sets only `threads=auto`; there is no `hwaccel` nor `av_hwdevice_ctx` in the tree. The site's promise "whatever hardware acceleration it was built with" reads wider than the truth: for us the hardware is ONLY in encoding.
- **The encoder name is known only after the fact.** The progress window writes `render.encoder_default` = "auto encoder" (`render_dialog.rs:452`) until the very end, and the real name appears in the summary. Refusal BEFORE the start is the subject of F-HLP-011; beyond it there is exactly one thing here: "which encoder this MACHINE is able to open" is a property of the machine, not of the project, and it lives in the first-launch report, from where the render window takes it.
- **The cache limit is shown, but cannot be changed, and about this there is silence.** `BurstCache::new(CacheConfig::default())` (`src/libs/kt-engine/src/player.rs:189`) — 64 MB of audio and 512 MB of video as constants; no setting, no environment variable. Rule 19 (F-HLP-009) is violated not by the text but by the absence of an addressee.
- **The first phrase of the program on a weak machine is bad news, and it overwrites the invitation.** The greeting `msg::OPEN_A_PROJECT` is put into `status` via `new_empty(Some(...))` (`app.rs:5287`, the field — `app.rs:481`) and does NOT get into the bell; on the very first frame `warn(SOFTWARE_RENDER)` overwrites the only status slot. A newcomer on llvmpipe instead of "Open a project" gets "No graphics acceleration…" — a correct message, but the first one and without the next step.

### What gets in the way

- Rules 9 and 13 (nothing superfluous, do not waste space) close the road both to a first-launch wizard and to a separate "specifications page". The place already exists and it is the right one: the body of the "Diagnostics" window, where people come deliberately.
- Invariant 12 (repaint only on movement, no allocations or clones on the draw paths) closes animated tours and "live" lines: the report is assembled once, into finished `String`s, when the window is opened.
- Laziness is by construction: until the first file is opened, we honestly have no answer about FFmpeg. So every line must have a third state — "not known yet" — and it must not look like a breakage (the same conclusion as in F-HLP-026).
- The cost of a line in five languages (F-HLP-033) is low here: the values are names and numbers (`avcodec 61.19.101`, `avx2`, `h264_vaapi`), they arrive as arguments, 6–8 labels are translated. But t128 will have to be extended to the new block, and t100 to its glyphs.
- Probing encoders by opening them is work with the disk and the driver; it must not be called from a UI frame (the lesson of item 196 and invariant 12); so it is either in the background or by a button.

### Which invariants it touches

- **9** (export is in-process, automatic encoder choice with a real opening check) — the note changes nothing in it; it makes its RESULT visible and requires that the list in the UI obey the same rule as the automatic choice.
- **10** (the binary is not linked with libav*, FFmpeg via dlopen) — the source of the whole problem: the set of capabilities exists only on the user's machine. The invariant is not weakened; weakening it would mean cancelling the cause of the note.
- **12** — the report is static, computed once, without animations and counters.
- UI rules **9**, **13** — against the wizard and against the heap; **19** — the phrase about the fallback must name the way back, and there is no way back yet (F-HLP-009).
- Boundaries: "About"/the origin of the build and the "copy" button — F-HLP-026; refusal before the start of the render — F-HLP-011; the bell and the log — F-HLP-013; the wording of errors — F-HLP-008.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages of the round |
| Where the main work is | kt-app: the report block in `draw_diagnostics` and one honest first-launch phrase; kt-fx/kt-ffmpeg — only CALL the already written `simd_name()`/`backend_info()`; kt-ffmpeg — the encoder probe by opening (second stage) |
| Project format | does not touch it |
| Tests that will have to be written | `reach_tests::first_run_names_what_it_found` (each runtime capability has a reader in the UI; the list of encoders does not promise what does not open); extension of t128 to the new block and of t100/t125 to the new keys |
| Depends on | F-HLP-026 (the place and the origin of the build), F-HLP-011 (refusal before the start), F-HLP-013 (the bell), F-HLP-009 (the address of return), F-HLP-033 (the cost of a line) |
| Breaks compatibility | no |

A frame, not a specification: the composition of the report lines, the moment of the encoder probe and the behavior before FFmpeg is loaded — are the subject of the round.

## Verdict

We take it. We do not set up a first-launch wizard — there is not a single question to which a newcomer has an answer that we lack: the language is already determined by the locale, we do not need folders, and about the GPU he learns not from a question but from a consequence. We take something else: a program whose capabilities are assembled from system libraries is obliged to name what exactly it found, otherwise the user cannot distinguish "cannot do it" from "nothing was found". Half of the work is already done and lies as dead code — three functions with comments "for the perf panel and diagnostics" are not called from anywhere.

TEST: reach_tests::first_run_names_what_it_found — today it fails four times, and all four failures are visible by one reading of the files, without running the program. (1) `kt_fx::simd::simd_name()` (`src/libs/kt-fx/src/simd.rs:42`) with the comment "String for the perf panel/diagnostics" has not a single caller — the instruction level, for the sake of which the owner asked "carefully, what if there are no super-modern ones" (R.32), is computed and thrown away. (2) `EncoderChoice { name, hardware }` (`src/libs/kt-ffmpeg/src/lib.rs:84`) — the only structure answering "was a hardware one obtained" — is also without callers. (3) The encoder combo box (`src/apps/kt-app/src/render_dialog.rs:286`) is filled by `avcodec_find_encoder_by_name`, that is, by exactly the check which the muxer's own comment (`src/libs/kt-ffmpeg/src/muxer_impl.rs:76`) declares insufficient, and the failure of a forced choice arrives as the English string "no video encoder could be opened (last error: …)" with a raw AVERROR code. (4) The invitation `msg::OPEN_A_PROJECT` (`app.rs:5287`) on a machine with software rendering is overwritten by the warning in the same first frame and does not get into the bell.

ONE LINE: overlap — does not touch | delay — does not touch | capacity — in the status bar line during normal work zero new characters (the first launch on a weak machine already spends it on `msg.software_render`), and the table of verdicts goes as a block into the body of the ALREADY EXISTING "Diagnostics" window under the block of the build's origin; the name of the actually opened encoder goes into the already existing render summary.

## Sources

- Boris FX forum (VEGAS), pinned post "IMPORTANT! Information required
  to help you", NickHope, 28.03.2026 —
  https://forum.borisfx.com/t/important-information-required-to-help-you/21172
  (accessed 2026-09-07)
- Boris FX forum, thread "Patch: Vegas Pro 2026 Build 143", 20.06.2026 —
  https://forum.borisfx.com/t/patch-vegas-pro-2026-build-143/24204
  (accessed 2026-09-07)
- Boris FX forum, thread "Mocha laggy interface", 24–26.04.2021 —
  https://forum.borisfx.com/t/mocha-laggy-interface/11105 (accessed
  2026-09-07)
- Creative COW, thread "After Effects Texture Memory doubt" (verbatim
  transcript of the GPU Information dialog) —
  https://creativecow.net/forums/thread/after-effects-texture-memory-doubt/
  and "Why is Fast Draft disabled?" —
  https://creativecow.net/forums/thread/why-is-fast-draft-disabledae/
  (accessed 2026-09-07)
- Blender Manual, "GPU Rendering" (choice of backend, enabling the found
  devices, fallback to system memory with a named cost) —
  https://docs.blender.org/manual/en/latest/render/cycles/gpu_rendering.html
  (accessed 2026-09-07)
- Adobe, "GPU and GPU Driver requirements" (System Compatibility Report at
  launch) —
  https://helpx.adobe.com/in/premiere/desktop/get-started/technical-requirements/gpu-and-gpu-driver-requirements.html
  (accessed 2026-09-07; the same source as in F-HLP-026)
- Blackmagic Design forum (the Memory and GPU panel: processing mode
  Auto/CUDA/OpenCL/Metal, manual choice from the found devices) — thread
  https://forum.blackmagicdesign.com/viewtopic.php?t=100084 and others; the texts
  were obtained only as search-engine excerpts: the forum pages themselves returned 403
  in this round, also through a proxy, so the Resolve line rests on the excerpts,
  not on a page that was read
- Not one of the five core editors nor Blender was checked live: there is no access
  to them from this environment. The VEGAS and AE lines rest on forums (the vendor's and
  Creative COW), the Premiere line on Adobe's documentation, the Blender line on the
  project manual, the Resolve line on excerpts from the vendor's forum.
- Access: the `WebSearch` budget in the session turned out to be exhausted BEFORE the work began
  (200/200), so everything was obtained by the workarounds from `_editors.md`: the Discourse API of the Boris
  FX forum (`/search.json`, `/t/<id>.json`), the text proxy `r.jina.ai` and
  `html.duckduckgo.com/html/?q=…` through the same proxy. A clarification for
  the following notes: `helpx.adobe.com` did not open this time either directly
  (timeout) or through the proxy, even on the regional mirror `/in/`;
  `forum.blackmagicdesign.com/viewtopic.php` through the proxy returns 403, whereas in the first wave
  it was readable — the search engine snippet remains the only working option.
