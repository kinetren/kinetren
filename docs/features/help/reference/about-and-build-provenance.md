---
id: F-HLP-026
title: "About" — build version and its contents on the user's machine
status: candidate
priority: B
cost: S
category: help/reference
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2, vscode-1.136]
we_have: partial
crates: [kt-app, kt-version, kt-ffmpeg]
invariants: [10, 12]
ui_rules: [9, 13]
round: null
point: null
tests: []
depends_on: [F-HLP-014, F-HLP-033]
checked: 2026-09-07
---

# F-HLP-026 — "About" — build version and its contents on the user's machine

## Essence

One place that people come to on purpose and where the program speaks about ITSELF, not about the project: what build this is, what it was assembled from ON THIS machine, and one button that puts all of it on the clipboard. The editor needs this exactly twice: when he writes "it does not work for me" and when he checks that he launched the right program. It covers nothing and delays nothing — the window is opened, read, and closed.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Help → About… | the version and build number in one line ("Version 2026.0 (Build 66)"). The COMPOSITION is not shown there: the GPU model, the video driver version and "which plugin decodes the file" are in Options → Preferences → File I/O and → Video, and in the Windows Device Manager. There is no "copy" button — that is why the community keeps a pinned post-list of ten items and asks for screenshots of the preference tabs |
| Adobe Premiere | 26.x | Help → About Adobe Premiere + System Compatibility Report | About gives the version and build. A separate, stronger mechanism: at LAUNCH the program itself checks the video driver version against known incompatible ones and shows a report — the composition of the environment is not only displayed but also verified. The report is confirmed by the documentation; the look of About itself — [NOT VERIFIED]: `helpx.adobe.com` did not open from the main domain from this environment |
| DaVinci Resolve | 21 | DaVinci Resolve → About DaVinci Resolve | the full version AND the build number. The key detail: on the splash the version is ROUNDED ("DaVinci Resolve 17" in large letters), the exact one ("17.1B BUILD 15") is only in About, in small type at the bottom; BMD support in threads asks for About, not for the splash |
| Final Cut Pro | 12.3 | Final Cut Pro → About Final Cut Pro | the application version; collecting system information is left to the OS (System Information / Feedback Assistant), there is none in the editor itself. [NOT VERIFIED]: the Apple page "About Final Cut Pro" (support.apple.com/en-us/102057) turned out to be a product overview, not a description of the window |
| Adobe After Effects | 26.x | Help → About After Effects | the device is the same as Premiere's: one help platform and one compatibility-report mechanism for the whole line. [NOT VERIFIED]: the AE page through an accessible helpx mirror did not load |

None of the five shows in About the composition of ITS OWN build (which libraries were picked up and of which versions) — for all five it is fixed by the installer, and there is nothing to show. The question "what the running thing is assembled from" simply does not arise for them; for us it arises at every launch (invariant 10).

**Outside the core.** Both lines below are the ORIGIN of two details that the five editors do not have: the revision hash in the "About" window and the button that collects the report for the user. Both details are taken from "The trick behind it".

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | Help → About Blender; Help → System Info | About shows THREE fields: Version, Date (the compilation date) and **Hash — the git hash of the build**, and the manual says directly why it is needed. The composition is moved to a separate command, which generates a text file `system-info.txt` — that is, "show" and "hand out" are separated into two commands |
| Visual Studio Code | 1.136 | Help → About; Help → Report Issue | About — version, commit, date, versions of Electron/Chromium/Node/V8, and OS. But it is not About that works, it is Report Issue: it itself substitutes the version, the platform and the list of extensions into the body of the report, and the project's instructions point not to About but to it |

## The trick behind it

The cargo that one status-bar line cannot hold here is **SOMEONE ELSE'S NAME: a dozen facts not about the project and not about the program, but about the MACHINE on which the program is running right now**. There is nothing to hover over: "the version of the loaded avcodec" has no on-screen representative, but all of these facts are needed at once and exactly at the moment when they must be TAKEN OUT OF THE PROGRAM — into someone else's messenger, someone else's tracker, someone else's phone call. For us the cargo will fit into the body of an already existing window, "Diagnostics", which people already open on purpose, and it will leave with one button.

**1. The exact version is hidden under the rounded one, and people lose hours on this.** In Resolve the splash writes "17" in large letters, and "17.1B BUILD 15" in small ones and only in About; a person in a thread spent hours on this and wrote "Sorry to ask such a simple question". For us the same fork is made deliberately and in the other direction, but with the same consequence: `kt_version::display()` (`src/libs/kt-version/src/lib.rs`) returns `FULL` = `ktYY.MM.DD-<hash8>` in debug and `SHORT` = `ktYY.MM` in release. That is, the least exact version is seen exactly by the one who has many builds and installs them over each other — the release user. The owner demanded the exact opposite (R.12 item 3), and demanded it before the release branch existed at all.

**2. The "copy" button is not a convenience but a transfer protocol.** The difference between the two models is visible in the numbers: the VEGAS community keeps pinned a list of ten items ("Version & Build", "Windows Edition & Version", "CPU", "GPU", "GPU Driver Version", MediaInfo, File Properties, screenshots of three preference tabs), because only a person can collect this. VS Code, instead of a list, gives one command that collects the same thing by itself. And here is the price of manual collection in real life: to the direct question "version and build number" a user answered "Build 143 / Vegas Pro Plus" — a half, and the version was completed for him by the one answering. Even when there is an "About" window, a person does not copy everything from it; he copies all of it only with a button.

**3. The composition of the user's build DIFFERS from what the developer assembled, and without "About" this cannot be found out.** The five core editors do not have this problem — everything is inside the installer. For us this is invariant 10: the binary is not linked with libav*, the FFmpeg major version is determined at runtime by `avcodec_version()` (`src/libs/kt-ffmpeg/src/loader.rs:94`), the struct types are taken from the version module, and the whole set of hardware accelerations is the set of the SYSTEM FFmpeg, which the developer has never seen. The site honestly promises this ("FFmpeg 6, 7 or 8 installed system-wide. Kinotrain uses the one you already have", `site/content/en.toml`), but the program does not say which of the three has been picked up. This is exactly why Blender prints the hash in About: builds of its particular version exist in the wild in many variants, and "5.2" is an insufficient answer.

## Pros

- It is the only surface that does not depend on the project's state: it answers when no project is open, when the sources are offline, and when the GPU is already poisoned. That is why it can be asked for over the phone — whereas asking for the render window, the bin or the timeline is useless, because they first require a project to be open.
- The facts are collected from the same variables that already feed the performance panel, so they cannot diverge from it. This is not abstract tidiness: in v1 the video cache limit was duplicated by hand and lied by a factor of 384 — today the occupancy is computed from the real `max_video_bytes()`, and there is no need to start a second bookkeeping.
- The button shifts the cost of collecting from the user to the program: a list that the VEGAS community keeps pinned as a post becomes one Ctrl+V for us. The user does not forget an item, because for him there are no items.
- The revision hash turns the question "what build do you have" from "probably a recent one" into a commit name on which one can run `git show` — that is, into a verifiable fact.
- As a side effect the question "did you restart the program after the build" is solved: the version with the hash is compared with the string printed by `finish.sh`, by eye and without an argument.

## Cons

- The list of the composition is a promise, and every line has to be TRUE AT THE MOMENT OF DISPLAY. Some facts are late for us: FFmpeg is loaded lazily (`OnceLock` at the first access to media), and the GPU backend may become `Poisoned` in the middle of a session. A window opened before a project is opened will honestly say "FFmpeg: not loaded" — and that is the truth, which is easy to take for a malfunction.
- "Diagnostics" is a magnet for junk: every next round will want to add one more line here, and in a year this becomes a second performance panel. Rule 13 works against the note itself, not only for it.
- The button puts into the clipboard the GPU name, the library versions and, if one is not careful, paths. This is the release of information about the machine, and it must be visible on screen BEFORE the click, not collected secretly at the moment of the click.
- The format of what is copied instantly becomes a de facto protocol: change the order of the lines and you break other people's report templates. For now we have one user, but the rule is known in advance, and it is worth choosing the order once.
- The window by itself fixes nothing. It is needed ONLY in a pair with the recipient of the report (F-HLP-014): information that has nowhere to be sent is just numbers.

## What users say

> Vegas Pro Version & Build — e.g. "Version 2026.0 (Build 66)". Find this
> information from "About…" on the "Help" menu.
> — NickHope, pinned post "IMPORTANT! Information required to help you",
> Boris FX forum, 28.03.2026,
> https://forum.borisfx.com/t/important-information-required-to-help-you/21172

This is the FIRST item of the required list, before the symptoms; the second is the Windows version, then CPU, GPU and the video driver version ("Vegas stability can be highly dependent on your GPU driver version").

> What is the version shown in the About DaVinci Resolve menu?
> — Dwaine Maggart, Blackmagic Design DaVinci Support, thread "Version 17.1 and
> not version 17", https://forum.blackmagicdesign.com/viewtopic.php?t=134202

The user's answer in the same thread — why a rounded version is not enough:

> It says DaVinci Resolve 17 in big orange letters but at the bottom, in much
> smaller letters, it says 17.1B BUILD 15.
> — Scott Oliver, same thread (access date 2026-09-07)

> What is your VegasPro version and build number?
> — ericlnz, the first reply in the bug thread, 21.08.2026,
> https://forum.borisfx.com/t/open-last-saved-project-doesnt-open-last-saved-project/25165

Further in the same thread: the user answered "Build 143 / Vegas Pro Plus", that is, a half, and the version was completed for him by the one answering — "That's BorisFX Vegas Pro 2026 Build 143".

> **Tip:** You can easily add all this information by creating the issue using
> `Report Issue` from VS Code's Help menu.
> — the VS Code project wiki, "Submitting Bugs and Suggestions",
> https://github.com/microsoft/vscode/wiki/Submitting-Bugs-and-Suggestions
> (access date 2026-09-07)

## How it fits Kinotrain

The industry argument ("that is how it is done") is the second one here. The first is local: **the question "which build did you just launch" has already cost the project two rounds.** The root `CLAUDE.md`, the section "Rules for agents", verbatim:

> "`cargo test` builds a SEPARATE test binary and does NOT rebuild
> `target/debug/kt-app` — the one the user runs: twice because of this he got
> an outdated program and considered the defects unfixed."

The same is in item 103 of `docs/porting-notes.md` ("the user twice launched `target/debug/kt-app` a month old and considered the fixed defects alive"), and there too — why the version format is exactly this. The owner's words about the format, R.12 item 3 of `docs/user-feedback.md`:

> "Isn't it obvious what is meant: ktYY.MM.DD-commit_hash?? USER CONVENIENCE AND ERGONOMICS"

and there, in the middle of a round: "I took the build from debug" → the `finish.sh` rule. The third time the same class of problem surfaced in the delivery — a comment in `src/scripts/release-build.sh`: "previously old tarballs and unpacked directories piled up forever, and the user could take the wrong build (item 7 of R.12)". Three hits at one point from different sides, and all three are closed from the side of the BUILD. From the side of the running program, nothing is closed.

### What already exists

- `src/libs/kt-version/src/lib.rs` — `FULL` (`ktYY.MM.DD-<hash8>` by the date of the COMMIT), `SHORT` (`ktYY.MM`) and `display()`, which chooses by `debug_assertions`. Today this is `kt26.09.07-7cdcc88d`. The format and the priority of sources (env → git → fallback) — `src/libs/kt-version/build.rs`.
- The only display point in the GUI — `src/apps/kt-app/src/app.rs:797`: `kt_version::display()` goes into `ViewportCommand::Title`. And this is less than it seems: the window by default has NO FRAME (`app.rs:5241`, `.with_decorations(decorations)`, the system frame only via `KT_DECORATIONS`), and its own titlebar (`src/apps/kt-app/src/toolbar.rs:185`) draws exactly three buttons — neither the project name nor the version. The string lives in the WM's window list (taskbar, Alt+Tab), not on a surface the program draws: it cannot be selected or copied, and in release it contains neither the day nor the hash.
- `src/apps/kt-cli/src/main.rs:130` — `kt-cli version` (and a bare `kt-cli`) prints `FULL` and the tick frequency. This is the ONLY place where the release user can see the full version — and it is in the program that the editor does not launch.
- The performance panel already knows about the hardware: `app.rs:5145` puts into `gpu_info` the string `"{adapter name} / {backend}"` from `wgpu_render_state()`, the string is drawn in `app.rs:3751`; next to it (`app.rs:3754`) — the video FX backend from `player.video_backend()` (`src/libs/kt-engine/src/player.rs:177`, `Cpu` / `Gpu(info)` / `Poisoned{…}`), keys `perf.video_fx_gpu` and `perf.video_fx_cpu`. Software rendering is detected there too and gives the notice `msg.software_render`.
- The "Diagnostics" window already exists and is already opened DELIBERATELY: `draw_diagnostics` (`app.rs:5064`), the item `menu.view.diagnostics`, the GAUGE icon in the toolbar (`toolbar.rs:665`), the title `win.diag_title`, vertical scrolling, the cache occupancy from the real `max_video_bytes()`, and the log of problems. There is not a single line about versions, GPU or FFmpeg in it.
- Copying to the clipboard is already worked out and needs no new mechanism: `ui.ctx().copy_text(json)` in `src/apps/kt-app/src/widgets.rs:535` and `src/apps/kt-app/src/fx_editor.rs:463`.
- The "black box" of playback: `save_flight_log()` (`app.rs:1940`) writes `<project>.ktflight.log` from `flight.dump()` (`src/libs/kt-engine/src/flight.rs:153`).

### What is missing

- **The main thing: that very runtime knowledge is computed and thrown away.** `kt_ffmpeg::backend_info()` (`src/libs/kt-ffmpeg/src/lib.rs:171`) returns `major` and the full triple of `avcodec_version`, and the comment on the structure says literally "What was managed to be loaded (for the performance panel and diagnostics)" (`lib.rs:45`); `kt_media::ffmpeg::backend_info()` (`src/libs/kt-media/src/ffmpeg.rs:92`) re-exports it. **There is not a single call** — neither in kt-app, nor in kt-cli, nor in the tests; the same for the neighboring `backend_error()`. This is exactly the failure mechanism described in item 122 of `docs/porting-notes.md` (`lane_names_insert/remove/swap` existed, were covered by t90, and were never called from the application): a unit test stays green while the UI has not reached the function. The word "FFmpeg" appears in the catalogs of translation EXACTLY ONCE — as the title of the graph `perf.graph_decode`.
- Not a single key with the word version or about in `src/apps/kt-app/i18n/*.toml` (521 keys in `en.toml`). There is no Help menu — only `[menu.file]`, `[menu.edit]`, `[menu.view]`; `Key::F1` does not occur in `src/apps/kt-app/src/`.
- The dump of the "black box" has no header: `flight.dump()` writes only transport events. The file that the user will be asked to send carries neither the version, nor FFmpeg, nor the GPU — it cannot be tied to a build.
- There is no OS, kernel, glibc, interface language, or project format version (`kt_otio::KT_META_VERSION`) in one place, and there is no "copy" button.
- There is not a single test that looks at the version in the interface at all: `kt_version` is not mentioned in `layout_tests.rs`, nor in `reach_tests.rs`, nor in `i18n_tests.rs` (0 occurrences in each).

### What gets in the way

- Rule 9 ("nothing superfluous") and rule 13 ("DO NOT WASTE SPACE"): a separate modal window with a logo and a copyright is exactly what they are written against. The place is already occupied and already right: the body of the "Diagnostics" window, as a block above the log of problems. Introducing a new surface here is not allowed.
- Rule 14 (everything that takes focus must give it back; item 105) — one more argument against a separate modal and for the existing window, which is closed by the standard `open`.
- The cost of a line in five languages (F-HLP-033) is low here, but not zero: the VALUES are names (`kt26.09.07-7cdcc88d`, `avcodec 61.19.101`, `NVIDIA / Vulkan`), they are not translated and arrive as runtime arguments, like `{0}` in `perf.video_fx_gpu`; only 6–8 LABELS are translated. The limit of 160 characters (`src/apps/kt-app/build.rs`) is not tight for the labels, but t128 will have to be extended to the new block, and t100 to its glyphs.
- Laziness: FFmpeg is loaded at the first access to media, the GPU backend is initialized in the first frame. So the block must be able to handle three states — "loaded", "not yet", "not found" — and the third must sound like a recommendation, not like an English `thiserror` string (this is exactly the subject of F-HLP-008; here only do not spoil it).

### Which invariants it affects

- **10** (the binary is not linked with libav*, FFmpeg via dlopen) — the main one. The note changes nothing in it; it makes its CONSEQUENCE visible to the user. As long as the consequence is not visible, the invariant works only for us and does not work at all for the person who sent the report.
- **12** (repaint only on movement, in draw paths no allocations and no clones): all facts are computed once (`OnceLock` for FFmpeg, `gpu_info` — at the first frame), so the lines are assembled at the OPENING of the window and placed into ready `String`s, rather than formatted every frame. There must be no animations and no "live" counters in the block — for those there are the graphs one floor below.
- The UI rules **9** and **13** — see "What gets in the way"; **14** — focus.
- The decisions 1 and 2 of the R.40 unfreeze (overlap, delay) are not affected at all: the proposed surface is the body of a window, not a hint.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage of the round |
| Where the main work is | kt-app: a block in `draw_diagnostics`, one button through `copy_text`, 6–8 i18n keys. kt-ffmpeg/kt-media — only to CALL the already written `backend_info()`. kt-version is not touched |
| Project format | not touched |
| Tests to write | `reach_tests::build_provenance_is_reachable_and_copyable` (the version is reachable from the UI and gets into the clipboard; `backend_info()` has a caller); an extension of t128 to the new block and t100/t125 to the new keys |
| Depends on | F-HLP-014 (where to send), F-HLP-033 (the cost of a line), F-HLP-013 (the log and the bell) |
| Breaks compatibility | no |

The frame, not a specification: the concrete set of fields, the order of lines in the buffer, and the behavior of the block before FFmpeg is loaded are the subject of the round, not of the note.

## Verdict

We take it, and cheaply: no new surface, no new data source — everything that needs to be shown is already computed, and half of it is already shown nearby. The note is not about "making a pretty window", but about the fact that the single fact which CANNOT be learned in advance (which FFmpeg was picked up on this machine) is computed by us and silently thrown away, and the version with the hash — the one the owner demanded word for word — is shown nowhere in the release, except in the CLI.

TEST: reach_tests::build_provenance_is_reachable_and_copyable — fails today four times, and all four failures are visible without running the program: (1) `kt_ffmpeg::backend_info()` with the comment "for the performance panel and diagnostics" (`src/libs/kt-ffmpeg/src/lib.rs:45`) is called by NOBODY, except its own re-export in `src/libs/kt-media/src/ffmpeg.rs:92`; (2) `kt_version::display()` has exactly one consumer — `ViewportCommand::Title` in `src/apps/kt-app/src/app.rs:797`, and the window by default has no frame (`app.rs:5241`), while its own titlebar (`toolbar.rs:185`) does not draw the version; (3) in release `display()` returns `SHORT` = `ktYY.MM`, that is, without the day and the hash that R.12 item 3 demanded; (4) in the catalogs `src/apps/kt-app/i18n/*.toml` there is not a single key about the version or the build, and `flight.dump()` writes the journal without a header.

ONE LINE: overlap — not touched | delay — not touched | capacity — in the status bar line zero new characters; a dozen facts about the build lay out as a block in the body of the ALREADY OPENED window "Diagnostics" (`app.rs:5064`) above the log of problems, plus one button that puts the same text into the clipboard through the already used `copy_text`.

## Sources

- The Boris FX forum (VEGAS), the pinned post "IMPORTANT! Information required to help you", NickHope, 28.03.2026 —
  https://forum.borisfx.com/t/important-information-required-to-help-you/21172
  (access date 2026-09-07)
- The Boris FX forum, thread "Open last saved project doesn't open last saved project", 21–22.08.2026 —
  https://forum.borisfx.com/t/open-last-saved-project-doesnt-open-last-saved-project/25165
  (access date 2026-09-07)
- The Blackmagic Design forum, thread "Version 17.1 and not version 17" (the Resolve 17.1 line) — https://forum.blackmagicdesign.com/viewtopic.php?t=134202
  (access date 2026-09-07)
- Blender Manual, Topbar → Help menu (the fields of About Blender: Version, Date, Hash; System Info) —
  https://docs.blender.org/manual/en/latest/interface/window_system/topbar.html
  (access date 2026-09-07)
- The VS Code wiki, "Submitting Bugs and Suggestions" —
  https://github.com/microsoft/vscode/wiki/Submitting-Bugs-and-Suggestions
  (access date 2026-09-07)
- Adobe, "GPU and GPU Driver requirements" (the System Compatibility Report at launch) —
  https://helpx.adobe.com/in/premiere/desktop/get-started/technical-requirements/gpu-and-gpu-driver-requirements.html
  (access date 2026-09-07)
- Apple Support, "About Final Cut Pro" — https://support.apple.com/en-us/102057
  (access date 2026-09-07; it turned out to be a product overview, the window's device is not described)
- Not checked live in any of the five core editors and in any program outside the core: there is no access to them from this environment. The Premiere and After Effects rows rest on Adobe's documentation, the VEGAS and Resolve rows on the vendors' forums, the Blender and VS Code rows on the manual and the project wikis.
- Access: the `WebSearch` budget in the session was used up before work began, everything was obtained by the workarounds from `_editors.md` — the Discourse API of the Boris FX forum (`/search.json`, `/t/<id>.json`) and the text proxy `r.jina.ai`. A clarification for the next notes of the wave: `helpx.adobe.com` does NOT open through the proxy (an HTTP/2 error), but the regional mirror `helpx.adobe.com/in/…` does open; the search of the Blackmagic forum (`search.php`) requires a session, but the concrete `viewtopic.php?t=<id>` pages can be read through the proxy, and they are found via `html.duckduckgo.com/html/?q=…` through the same proxy.
