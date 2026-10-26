---
id: F-RES-006
title: Dependence on system libraries and drivers
status: candidate
priority: B
cost: S
category: resilience/environment
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08]
we_have: partial
crates: [kt-ffmpeg, kt-media, kt-engine, kt-app, kt-gpu]
invariants: [9, 10, 12]
ui_rules: [12, 16]
round: null
point: null
tests: []
depends_on: [F-HLP-030, F-HLP-026, F-HLP-012]
trackers: [kde-bugzilla, kde-invent, pitivi]
checked: 2026-09-08
---

# F-RES-006 — Dependence on system libraries and drivers

## Essence

Half of a video editor's capabilities belong not to it: codecs live in someone else's
library, hardware encoding lives in someone else's driver, interface acceleration lives in someone else's graphics stack. The editor does not know this and need not know it: for him "does not work" is indistinguishable from "cannot". The section is about
what the program DOES when the other party's part was not in place or turned out to be a different version: what it substitutes, what it leaves unsaid, and what the session turns into.

## Who has it and how

| Editor | Version | Called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Preferences → Video ("GPU acceleration of video processing") and → File I/O ("Hardware decoder to use") | the media stack is its own, shipped with the program, so there is no "someone else's library" in its life — there is someone else's DRIVER. The choice of device is manual and laid out across two tabs; the program itself does not speak about the finding. The device is confirmed by the reconnaissance F-HLP-030, live [NOT CHECKED] |
| Adobe Premiere | 26.x | System Compatibility Report | the only one in the core that checks the OTHER party before a symptom: at launch it compares the video driver version with known incompatible ones and shows a report. The check is tied to startup, not to the first failure. From Adobe's documentation (F-HLP-030) |
| DaVinci Resolve | 21 | Preferences → Memory and GPU (GPU processing mode: Auto/CUDA/OpenCL/Metal) | the requirement on the driver is strict and declared: supported versions are listed, the devices found are shown as a list with checkboxes. A consequence of the device: an EMPTY list is in itself a diagnosis, visible without a single number (F-HLP-030) |
| Final Cut Pro | 12.3 | none | the class practically does not exist: the media engine and the driver come from one supplier, versions are reconciled by the platform, the choice of encoder is not presented to the user at all. [NOT CHECKED] |
| Adobe After Effects | 26.x | Preferences → Previews → GPU Information… | verdicts by CAPABILITIES, not by hardware: "Fast Draft: Available", the CUDA driver version, memory size with an explicit moment of truth "(at application launch)" (F-HLP-030) |

**Outside the core.** Without Kdenlive this note does not exist: it is the only available
program where both halves of the mechanism — both the detection of hardware
encoding and its refusal — are visible publicly, as a task with a permanent URL.

| Program | Version | Called | How it works |
|---|---|---|---|
| Kdenlive | 26.08.0 | a setup wizard ("check hardware acceleration") + render/proxy profiles | detection and use are separated: the wizard asks a third-party `ffmpeg` once which encoders exist, the verdict settles into the settings and gates the list of profiles, and the render itself goes out to an external process (`melt`). The distribution (AppImage/flatpak) carries ITS OWN `ffmpeg`, which on the same machine can do something other than the system one |

## The trick behind it

**Their verdict about hardware is a memory, taken by another process at another moment; we have none at all, because the question is asked of whoever will encode, and at the moment they will encode.**

The tracker shows a divergence between the wizard and reality in BOTH directions, and these are not
private bugs but a consequence of the check and the work being different processes.
Outward — "found it, but it does not work": the detector reports NVIDIA, while decoding fails with `CUDA_ERROR_OUT_OF_MEMORY` (kde-invent #2135,
02.02.2026). Inward — "works, but did not find it": preview via VAAPI runs, while the encoder wizard does not see it (kde-invent #1771, 17.10.2023, open). Between them
a third kind, the most unpleasant: the encoder OPENED and produced an empty file when the GPU was loaded to 40 % (kde-invent #585, 13.03.2020).

Our answer (`src/libs/kt-ffmpeg/src/muxer_impl.rs:92-160`) is structured so that the first two kinds do not exist: the list of candidates
`h264_nvenc → h264_vaapi → h264_qsv → libx264 → mpeg4` is walked at the moment
of export, and the sign of fitness is not "present in the build" but a return of
`avcodec_open2 >= 0` in the very same process that is about to write the file.
The comment there (lines 77-80) states the cause directly: "the presence of an encoder in the
build does not mean it will work". There is no cacheable verdict, so there is nothing to go stale; failed probes do not alarm the user, because for the duration of the walk the FFmpeg log level is raised to `AV_LOG_FATAL` (line 92) and lowered back
immediately after.

The third kind — "opened and lied" — is closed for us exactly as it is for them, that is,
not at all: `avcodec_open2` returned success, and from there on the driver's responsibility begins. The honest boundary of the mechanism runs here.

A second layer that none of the six has: **the choice of the library version itself is also a runtime decision**. `src/libs/kt-ffmpeg/src/loader.rs:20-45`
holds three SONAME sets (FFmpeg 8/7/6) and walks them from new to old, and
`try_candidate` (lines 91-107) does not trust the file name and checks
`avcodec_version()` against the expected major, because the name lies and the function does not.
The structure types for each major are separate (bindgen over vendored headers,
`version_module!` in `lib.rs:131-133`). The result: one executable uses
the hardware acceleration of SOMEONE ELSE'S system FFmpeg without being linked with it, and
this is not a declaration — `src/scripts/release-build.sh:55-68` breaks the build if
`ldd` sees `libav*` or if the `GLIBC_` ceiling turns out to be above the declared one.

## Pros

- By trying the options in turn, we get hardware acceleration where it WORKS, without a single question to the user and without any setup. There is no wizard, no profiles, no saved verdict — so there is nothing that can diverge from reality between launches and nothing to repair when the user has changed the driver.
- The absence of linking with `libav*` removes a whole class of someone else's pain: the package is not bound to the major version of the distribution's FFmpeg, an upgrade of the system FFmpeg from 7 to 8 does not require a rebuild, and "the ffmpeg in an AppImage can do something other than the system one"
  (kde-invent #895, 27.12.2020; in Bugzilla the same class is closed as UPSTREAM — #419954) is impossible for us by construction: we have no ffmpeg of our own.
- The order of candidates sets the policy in one line (`VIDEO_ENCODERS`,
  `muxer_impl.rs:15-21`), not a set of profiles that the user has to choose and that in someone else's tracker form a separate topic of requests
  ("Improve GPU render profiles", kde-invent #808, open since 13.10.2020).
- The encoder that actually opened goes into `ExportStats.encoder`
  (`src/libs/kt-engine/src/export.rs:520`) and reaches the render summary and the output of `kt-cli`: the user sees what the file was MADE with, not what it was intended to be made with.
- Loss of the GPU in the middle of a session does not drop the work: `DeviceLost` "poisons" the executor
  (`player.rs:575-590`), the preview quietly continues on the CPU, and the notice is shown exactly once (`app.rs:768-775`, test 185).

## Cons

- The price is the same as for everyone: we control neither the FFmpeg version, nor the driver, nor their combination. The difference is only that we ask about it at the right moment, not that we are protected from it.
- The walk is silent about what it has lost: if `h264_nvenc` did not open, the export simply ends up on `libx264`, and the user learns this only from the encoder name in the final summary — that is, after, not before. This costs several times the render time.
- An explicit choice of encoder in the fallback dialog does not exist at all: `force_encoder`
  skips all other candidates (`muxer_impl.rs:96-99`), and a failure turns into an error of the whole export. This is deliberate (test 25
  `t25_force_unknown_encoder_is_explicit_error`), but the list from which the user chooses is compiled by a weak check — F-HLP-030 has already found this, and it is not reopened here.
- 3D effects without a GPU — a refusal BEFORE the file is created (`ExportError::GpuRequired`,
  `export.rs:405-410`). Correct in essence, but this is the only place where "you have no hardware" stops the work entirely.

## What users say

SIGNAL: hardware/driver — 33 tasks in kde-invent, `nvenc|vaapi|qsv|cuda|movit|hardware accel`; share 1.5% versus 1.1% in kde-bugzilla

Third comes pitivi with 0.1 % — not because GStreamer does not have this pain, but
because it is written down in its words: `gstvaapi`, "Cannot create a VA
display" (pitivi #2108, 03.10.2017).

The number for which the note was opened is not the size of the topic but its OUTCOME. Across the three exported trackers the topic "hardware/driver" has a share of real fixes among
closed tasks of **0.46 — the worst of the sixteen topics** (the next one from the bottom
"render/export" 0.56, from the top "interface" 0.80, "translation" 0.79, "proxies"
0.77): `tracker-rank.py --themes`, the "fixes" column. Inside kde-bugzilla alone, the only one of the three where the reason for closing is set by hand, the picture is even sharper: **22 FIXED of 93 closed = 0.24**, again the worst result out of sixteen (the next one 0.39). The rest — 24 DUPLICATE, 19
WORKSFORME, 10 INVALID and 8 UPSTREAM/DOWNSTREAM/MOVED. The last share, 8.6 % of
closed, is the highest among all topics (for "render/export" it is 4.5 %, for
"interface" 3.0 %) and is a direct measurement of the phrase "this is not our code".

> using this automatically downloaded drivers Kdnelive is showing an ffmpeg
> issue during transcoding using Nvidia card. if i download the drivers
> directly from Nvidia site the transcoding is correct
> — kde-invent #1594, 05.12.2022,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1594

The same distribution, the same hardware, the same file — the difference is where the driver came from. It cannot be fixed or reproduced from within the application.

> running the setup wizard, it tells me “mediainfo” is not installed and
> proceeds to not find any hardware encoders, both on my AMD system and INTEL
> system, both of which can preview the video with VAAPI, but are unable to
> actually render with it.
> — kde-invent #1771, 17.10.2023, open,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1771

> Render a video with profle vcodec=nvenc_h264 producing just empty files.
> Strange thing is GPU-utilization goes up to 40% while doing the job?!
> — kde-invent #585, 13.03.2020, open,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/585

The mechanism that makes this class unfixable is visible in the tasks themselves and is the same for all of them: the maintainer owns none of the moving parts. The driver version is installed by the vendor or the OS (#1594), the library version by the distribution (#895), and the combination exists only for the reporter. There is nothing to reproduce it with,
so WORKSFORME here is not triage laziness but an honest answer; a fix, if it is possible at all, goes upstream and arrives a year later as someone else's release —
hence the record 8.6 % UPSTREAM/DOWNSTREAM. The only answer that scales is "works on my machine". Pitivi's answer to the same kind of task is indicative and in its own way honest: hide what the machine cannot do
("Hide OpenGL effects when there is no GL hardware", pitivi #1946,
01.12.2016).

## How it fits Kinotrain

ADDRESS: src/apps/kt-app/src/app.rs:2154-2183 — the class is possible, and nothing catches it: the absence of a SYSTEM library reaches the user as N missing FILES, and the machinery for returning sources spins forever after that, because there is nothing to return.

### What already exists

- `src/libs/kt-ffmpeg/src/loader.rs` — iteration over the SONAMEs of three majors, a check of
  `avcodec_version()`, a single diagnosis in `FfError::NotFound` with the list of what was tried and why it did not work (`lib.rs:31-36`).
- `src/libs/kt-ffmpeg/src/muxer_impl.rs:92-160` — automatic encoder choice
  by opening (invariant 9); `src/libs/kt-engine/src/export.rs:520` — the name of the
  encoder actually opened, in `ExportStats`.
- `src/scripts/release-build.sh:55-68` — invariant 10 by machine: `ldd` without
  `libav*` and the `GLIBC_` ceiling.
- `src/libs/kt-engine/src/player.rs:575-590` + `app.rs:768-775` — loss of the GPU
  switches to the CPU and says so once (test 185);
  `export.rs:405-410` — a refusal before the file is created, if 3D needs a GPU and there is none.
- `src/apps/kt-app/src/util.rs:59-67` — an unavailable source is replaced by a placeholder, the project opens and renders (R.33 item 10, tests 209-211).

### What is missing

1. **Distinguish "no file" from "no library".** Today both paths converge in
   `open_source_or_placeholder`, and on a machine without FFmpeg 6/7/8/9 the user
   gets a line for EVERY clip — "`clip-01.mp4`: source unavailable,
   placeholder shown (FFmpeg not found on this system…)" — plus the total
   "unavailable sources: 40" (`app.rs:684-689`). The cause is one and systemic,
   the report is forty items and file-based; the truth lies inside the brackets forty times.
2. **Stop searching for what was not lost.** `poll_returned_sources`
   (`app.rs:2154-2183`) is called from `draw_all` on every drawn frame: the guard (`util.rs:183-214`) checks `Path::exists()`, the files are in place,
   so `offline` is empty, so ALL placeholders count as "returned" and go into the `reopen-media` thread. The opening fails instantly
   (`BACKEND` is a `OnceLock`, `lib.rs:143`: a failed load is cached for the whole process), the placeholder stays, "nothing to report" — and on the next frame the thread is spawned again. Installing FFmpeg in the middle of a session will change nothing: the `OnceLock` does not ask again.
3. **Say out loud what was found.** `backend_error()`
   (`src/libs/kt-media/src/ffmpeg.rs:97`, the comment "The cause why
   FFmpeg did not load (for the message to the user)") is called from nowhere.
   The adjacent half — "which version was found and which accelerations are available" — was found by F-HLP-030 (dead `backend_info`, `simd_name`, `EncoderChoice`);
   here only the second half of the same hole is added: the text about the FAILURE is dead too.

### What gets in the way

Nothing structural. The diagnosis is already assembled in the right place and in the right
form — `FfError::NotFound` carries a ready list of attempts. Only the fact that the decision "the library is not there" is taken lazily, on the first opening of a file,
that is, deep inside the loop over media, where it is known only that it is a
`String`. Asking `backend_error()` once before the loop is a condition in two
places, not a subsystem.

### Which invariants it touches

Invariant 9 (export is in-process) and 10 (dlopen, no linking with
`libav*`) — the subject of the note; both are preserved: what is fixed is not the loading but the account
of its failure. Invariant 12 ("the UI does not heat the system") is violated today by item
2 — a thread per frame, as long as the project has at least one placeholder. UI rule 12
("background work does not get in the way") — the same place. UI rule 16 ("an impossibility is
an error, not a faded hint") — on the side of item 1: a systemic impossibility must be named by its own name, not by forty file entries.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage of the round |
| Where the main work is | kt-app (one condition before the loop over media + a gate on re-request), kt-media (propagate `backend_error` upward) |
| Project format | not touched |
| Tests that will have to be written | a harness test: on a backend load failure — ONE message instead of N and not a single `reopen-media` thread (checked by substituting the load path, without removing FFmpeg); a unit test: `poll_returned_sources` does not spawn a thread while the cause of the placeholder is systemic |
| Depends on | F-HLP-030 (what the program says about findings), F-HLP-012 (wordings about a missing source) |
| Breaks compatibility | no |

## Verdict

We take the narrow part, and do not touch the mechanism. The automatic choice by opening and the runtime choice of the FFmpeg major are the strongest things we have in this domain: they remove exactly
those two kinds of divergence which, in someone else's tracker, give the worst share of repaired ones in the base (0.46 across three trackers, 0.24 inside kde-bugzilla). We pay the same price as everyone else, and pretending otherwise is not allowed.

We take three consequences that do not belong to the mechanism and cost one stage: a systemic failure must be named once and by its own name, not as forty file entries; the search for "returned" sources must switch off when what was lost was not a file; the ready text of `backend_error()` must be read by someone. The first two are
not a message but behaviour, so they live here, not in `help/`.

## Sources

- Tracker dumps dated 08.09.2026, `~/.cache/kinotrain/trackers/`: kde-bugzilla
  (6346), kde-invent (2217), pitivi (2672). **The GitHub trackers of the registry
  (shotcut, mlt, otio, olive, losslesscut, flowblade, openshot),
  blender-vse and ffmpeg-trac were still being exported at the time of writing** — all the numbers of
  this note are counted over the three available ones, and the share of the topic in the third (pitivi)
  is shown separately precisely because two is too few.
- Reproducing the numbers: `python3 src/scripts/tracker-rank.py --themes`
  (the "fixes" column), `--theme 'hardware/driver'`,
  `--query 'nvenc|vaapi|qsv|cuda|movit|hardware accel'`.
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1594 (accessed
  08.09.2026)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1771 (08.09.2026)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/585 (08.09.2026)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/2135 (08.09.2026)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/895 and
  https://bugs.kde.org/show_bug.cgi?id=419954 — "own ffmpeg in the distribution"
  (08.09.2026)
- https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1946 — Pitivi's answer
  "hide what the machine cannot do" (08.09.2026)
- The core rows in "Who has it and how" rest on the reconnaissance of F-HLP-030
  (`help/onboarding/first-run-hardware-and-defaults.md`); there was no live check of
  VEGAS/Resolve/FCP of our own in this pass.
- Our code was read in the tree: `src/libs/kt-ffmpeg/src/{loader,lib,muxer_impl}.rs`,
  `src/libs/kt-engine/src/export.rs`, `src/libs/kt-media/src/ffmpeg.rs`,
  `src/apps/kt-app/src/{app,util,render_dialog}.rs`,
  `src/scripts/release-build.sh`.
