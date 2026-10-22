---
id: F-HLP-029
title: Demo project and its footage
status: candidate
priority: B
cost: M
category: help/onboarding
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2, vscode-1.136]
we_have: partial
crates: [kt-cli, kt-app, kt-media]
invariants: [8, 9, 10]
ui_rules: [17, 18, 20]
round: null
point: null
tests: []
depends_on: [F-HLP-032, F-HLP-012, F-HLP-020]
trackers: [otio, mlt, openshot, pitivi]
checked: 2026-09-08
---

# F-HLP-029 — Demo project and its footage

## Essence

The program ships with one already assembled edit and the material it was assembled from. In the first minute the editor opens it and gets what neither a hint nor a lesson gives: someone else's finished work that can be played, taken apart piece by piece, broken and rendered. A demo answers not the question "what does this button do" but the question "what does a properly assembled project look like" — which the interface answers nowhere.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | I could not find a "open demo project" item in the program; the learning material lives outside it — lessons and forum threads where users themselves post links to .veg files. [NOT VERIFIED]: the online help for the 2026 line (`borisfx.com/Documentation/vegas/2026/en/`, the address from F-HLP-032) returned 404 on the check date |
| Adobe Premiere | 26.x | Learn panel | step-by-step lessons inside the window, leading through the REAL interface on attached material; there is no separate "demo project" as a file. [NOT VERIFIED]: from this environment helpx returns only navigation, the page contents are not visible |
| DaVinci Resolve | 21 | none | there is no demo project in the distribution; the vendor teaches with workbooks ("This official hands on training guide… a series of practical exercises"), and the lesson material is distributed as a separate download. [NOT VERIFIED]: the size of the lesson download |
| Final Cut Pro | 12.3 | Download Demo Project | **an item in the program's own menu**: `Final Cut Pro ‣ Download Demo Project`; the user chooses the folder where the library is saved; in it there are TWO projects ("Allie Sherlock Music Video", "Introducing Allie Sherlock"), Keyword Collections and Smart Collections built by Transcript Search and Visual Search |
| Adobe After Effects | 26.x | start screen with templates | a new project does not start from emptiness but from a choice of a template. [NOT VERIFIED]: the same helpx barrier as for Premiere |

Of the five core editors, exactly one — Final Cut Pro — considers the demo project PART OF THE PROGRAM, not part of the website. And it is also the only one that does not put it into the installer: the demo is downloaded on request, into a folder the user named.

**Outside the core.** The mechanism "the program carries a starter file and a set of demo files" was not born in video editing; in NLEs it degenerated into "download the lesson material", and it cannot be described by five lines of the core. Blender is here the ORIGIN of the mechanism and the source of "the trick behind the trick" below; VS Code is needed as the opposite pole: a deliberate refusal of a demo project in favor of a checklist for YOUR OWN project.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | Startup File · Application Templates · Demo Files | three different things under one roof. `File ‣ Defaults ‣ Save Startup File` makes the CURRENT scene what opens at launch; `File ‣ New ‣ …` offers application templates (among them Video Editing — a ready workspace for the editor: Sequencer, Preview, a trimmed Properties panel, a scene 1920×1080 at 24 fps, 10 s long); demo files (the scene of each splash, Geometry Nodes scenes) are downloaded from the site separately and are not in the installer. `File ‣ Defaults ‣ Load Factory Settings` returns both the startup file and the settings |
| Visual Studio Code | 1.136 | Walkthroughs | "multi-step checklist featuring rich content" — a checklist with pictures and an action button at each step, living on the start page. There is deliberately NO demo project: the steps lead through the user's project, not someone else's |

## The trick behind it

What one status-bar line cannot hold in principle is here — **STATE: a fully assembled work**. A line carries a verb ("what this button does"); a demo carries a noun — six named tracks, sixteen clips, a transition, a loop region, curves on effects, and a morpher over two events, all in the right relations to one another. Such cargo does not get shorter: it cannot be retold in 160 characters, because its value is in the connections, not in the list. We already have this cargo PACKED — 57 KB of `.otio` that `kt-cli demo` writes — and it runs into not text and not translation, but into eight files that do not exist.

**1. Blender's demo is designed to be non-destructive, and this is separate work, not a side effect.** An application template keeps ITS OWN configuration: `./config/{APP_TEMPLATE_ID}/startup.blend` instead of `./config/startup.blend`. The manual says plainly: "Bundled blend-files `startup.blend` and `userpref.blend` are considered _Factory Settings_ and are never overwritten", and for rollback there is a separate command "Load Template Factory Settings" — the exact analog of the general Load Factory Settings, but within the bounds of the template. The point of the construction: one can learn from a demo only if it is not a pity to lose it, and it is not a pity only if return is guaranteed and does not touch your own settings. Whoever opens the demo as the original has no second time.

**2. Final Cut Pro does not put the demo into the installer, and this is not about the size of the distribution — it is about ownership.** `Final Cut Pro ‣ Download Demo Project` asks for a folder: from the first second the library lies where the user told it to, and IS HIS COPY. A program update will not overwrite it, and breaking it is a normal outcome, not a fault of the delivery. Exactly the same thought as Blender's, reached by another route: a demo must be a copy.

**3. Our demo is the only form of help in the project that cannot go stale silently — and that is exactly why it is visible that it has gone half-stale.** The ordinary `cargo test` runs it (`src/apps/kt-cli/tests/cli.rs`, t151: the file must contain `Transition.1` and `Marker.2`, and `roundtrip` must be byte-stable), and so does the screenshot pipeline of the site. But what is checked is the SCHEMA of the file, not whether the edit plays: the same binary that wrote the demo refuses to render it — `kt-cli demo d.otio && kt-cli render d.otio o.wav` exits with code 1 on the very first path (`src/apps/kt-cli/src/main.rs:60`, `return 1` instead of a placeholder). A demo that has a test behind it but no material is a verified schema of emptiness.

## Pros

- **It teaches structure, not functions.** Neither a hint, nor a menu item, nor an effect description can show that tracks are named, that a region is set on a piece that is listened to again, and that a morpher is hung on two neighboring scenes at once. A demo shows this at one glance, because it carries ready-made RELATIONS.
- **It does not go stale silently if it is opened by a test.** The format moves forward — the demo generator stops compiling, or its file stops matching the golden. Of all the reference artifacts, only the demo and the screenshots (F-HLP-032) have this property; prose has none.
- **Reversibility makes one bold.** Blender's design (its own configuration for the template) and Apple's design (a copy in the user's folder) give the same consequence: breaking the demo is a normal move in learning, not a loss of delivery. The user starts pressing everything.
- **The demo is the only entry point where the help requires no reading.** It works even for someone who never opens the documentation (by the experience of R.38 this is our owner: complaints come from the program, not from questions to the text).
- **For us it is almost free.** The generator is already written, already called in the pipeline, and already yields a valid project: `p.normalize()` and `p.validate()` stand as the last lines of `demo_project()`.

## Cons

- **A demo built to impress does not teach.** This very objection sounds in the quote below: a scene that shows the author's mastery is unreadable for a novice. A demo project is a teaching genre, not a showcase, and these are two different files.
- **The material is the most expensive part.** Resolve and Premiere have moved it into a separate download for exactly this reason. Whatever we put into the tarball we will carry in every release, and we will have to keep it decodable.
- **A demo ages differently from code.** The project file is caught by a test; the CORRECTNESS of the edit is not. If tomorrow the morpher learns to work differently, the demo stays syntactically correct and substantively outdated, and there is no one to tell about it.
- **Synthetic material does not show the main thing.** One can analyze the structure from it, but one cannot see what a real source looks like and sounds like; a filmstrip made of a Mandelbrot set does not teach one how to look at a filmstrip.
- **A demo project is a temptation to become a second face of the product.** Blender's demo files have long lived a separate life (the splash of each release); maintaining that is more expensive than it seems, and it no longer has to do with learning.
- **A broken example lives in the distribution for years, and people come back to fix it a second time.** The same example from the OpenTimelineIO `README.md` came to the tracker twice, a year apart (#1554 on 24.02.2023, #1694 on 10.02.2024); the example `flatten_video_tracks.py` crashed with an exception (#811). A test "the file matches the schema" does not catch this by construction — this is exactly our t151, and it is a cost, not a hypothesis.
- **Material runs into licensing and storage sooner than into weight.** When the consortium behind OpenTimelineIO — the format in which we store the project — set out to make a demo timeline, the first questions were "what is the minimum for the format of the material" and "whether to store it centrally, and if so, where" (otio #699, 14.05.2020). That means the decision "put a real video into the tarball" brings not only bytes but a legal tail in every release — and an explanation of where every second came from.

## What users say

A demo file as documentation of a mechanism — that is what people open it for:

> I knew the gear was driven by a driver, but I never play with drivers so I
> downloaded the file to check it out and learn something.
> — Bobo_The_Imp, 17.02.2020,
> https://blenderartists.org/t/distance-driver-being-messed-up-by-follow-path-constraint/1207534

The reverse side, verbatim, about the proposal to include a spectacular scene in the release's demo files:

> I don’t think so, this is way too complicated for beginners to learn anything.
> — Xeofrios, 22.06.2023,
> https://blenderartists.org/t/3d-artists-hate-this-one-weird-donut-trick/1469959

From the side of the five NLEs there are still no live quotes (vendor forums are not trackers, `_trackers.md`), but there is a tracker that speaks exactly about our defect. `OpenTimelineIO` is the reference library of the very format in which we store the project (the writer is our own, `kt-otio`), and its example from `README.md` came to the tracker broken twice, a year apart:

> "The Python example from `README.md` returns an error."
> — otio #1554 "Example in the README does not work", 24.02.2023,
> https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/1554

> "I expected the sample code to run without errors."
> — otio #1694 "Sample Python code throws an error", 10.02.2024,
> https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/1694

The second was filed on the same example, with the same line and the same `AttributeError` as the first — that is, between the two reports the example stayed broken in the distribution. The same class, on a sample program separately:

> "The flatten_video_tracks.py example program fails with an exception."
> — otio #811 "flatten_video_tracks example is broken", 07.10.2020,
> https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/811

SIGNAL: an example from the distribution that does not run — 3 tasks in otio, `\b(example|demo)s?\b[^.]{0,30}(broken|does ?n.?t work|not work|fail|error|throw|outdated)|\bsample\b[^.]{0,20}\b(code|project|file|data|media)\b[^.]{0,30}(broken|error|throw|fail|not work)`; the share is 0.4% against 0.2% in mlt (1 of 601)

The numbers are small, and that is part of the finding: the class is rare, because almost nobody has examples in the distribution — but where they exist, they break and lie broken. The single task in `mlt` is our case word for word: the demo "My Name is.." from the library's own kit failed not because of the API but because of files that do not exist ("Failed to load "+My name is Inigo Montoya.txt"", #35, 12.12.2013, closed). The only difference is that at mlt this was noticed by an outside person on macOS, while for us the nonexistent source files of the demo are seen by no test: t151 does not look into them.

About the price of the second form of answer — not a file but a window. OpenShot has built-in learning via pop-up windows, and the tracker knows 13 tasks about the WINDOW ITSELF: "Cant dismiss tutorial popup window" (#1949), "Tutorial resizes window" (#736), "Tutorial blocked by taskbar" (#794), "Every time a tutorial window pops up, it forces the application out of maximized" (#1447). The share is not computed here: the openshot dump is incomplete (see "Sources"), so 13 is a lower bound, not a share. The point is different: the "file" form of answer has no window mechanics of its own, there is nothing in it to break, and the price of the demo project as a whole lies in the material, not in behavior.

## How it fits Kinotrain

### What already exists

**The whole demo-edit generator.** `src/apps/kt-cli/src/main.rs:363`, `demo_project()` — 213 lines; the comment in the header honestly states the purpose: "Demo edit for the site screenshots (a port of `gen_project` from `site/screenshots.py` — it removes the second writer of the format, R.24 stage 11)". What it builds (checked by running `kt-cli demo`: 57 040 bytes, `v: 3`):

- **six named tracks** via `UiState.lane_names`: video MAIN / OVERLAY / PLATES, audio DIALOG / MUSIC / AMBIENCE, plus the morph track BATCH;
- **16 clips**: six in a row on MAIN (0–100 s), three on OVERLAY, one on PLATES, dialog in two pieces, music in two, ambience in two;
- **six markers** with names — COLD OPEN, INTERVIEW IN, B-ROLL, NIGHT CITY, MUSIC SWELL, OUTRO;
- **a region 34–66 s with `loop_enabled = true`** — that is, the demo opens already in the mode about which R.38 asked "I do not understand how to delete the saved area region";
- **FX chains**: Pan on three clips, noise, EventGrid, glow, VHS, Mirror + edges, and SlowedReverb on the music;
- **automation of two kinds**: `rate`/`mix` curves of SlowedReverb in ticks of the clip's MATERIAL, and the camera movement with `pan_x`/`pan_y`/`zoom` curves with `ease: true`;
- **a transition**: an honest crossfade between the music clips, `in_offset` and `out_offset` of 2 s each;
- **a morpher**: an FX batch of 48–80 s on the BATCH track, `MorphTargets::Clips` on two events, a `glow(0.5)` patch;
- **bin state**: a free layout, groups CAMERA (5 media) and SOUND (3 media, `locked: true`), the cursor position and the timeline zoom.

This is exactly the list that such a note usually has to promise. We have it written, it is validated on output (`p.validate().expect("demo project is valid")`), and it is checked by test t151.

**A second demo project that is easy to forget:** `src/fixtures/demo-3d.otio` — the committed golden of cameras and Pan3d (t189, `src/libs/kt-otio/tests/convert.rs`). It too refers to the nonexistent `media/m1.mp4` and `media/m2.mp4`, and as a demonstration nobody uses it.

### What is missing

**1. The material.** Eight paths in the file — `media/drone_coast.mp4`, `city_night_broll.mp4`, `interview_camA.mp4`, `colorbars_plate.mp4`, `gradient_wash.mp4`, `music_bed.flac`, `voiceover_take3.wav`, `room_tone.wav` — and not one of these files exists. They are made by `gen_media()` in `site/screenshots.py`: five `lavfi` videos (mandelbrot, life, testsrc2, smptehdbars, gradients) and three sounds (aevalsrc, anoisesrc) by an EXTERNAL `ffmpeg` binary. The script lives in `site/`, is not part of the distribution, and is unreachable from the program. The thesis "our demo project is self-sufficient because the material is synthetic" did not pass the check: the synthetic sources (`synth:`) are not used in this project at all.

**2. Paths from the GUI.** The File menu (`src/apps/kt-app/src/toolbar.rs:246`) has exactly seven items: New, Open…, Import .ktproj…, | Save (Ctrl+S), Save As…, | Render to MP4…, | Quit. There is no demo project in the menu, nor in the empty state, nor in the Ctrl+F search. `New` gives a clean sheet — exactly the place where FCP offers to download a demo.

**3. A command registry in kt-cli itself.** The message about an unknown command (`src/apps/kt-cli/src/main.rs:354`) lists "version, new, roundtrip, render, export" — **`demo` and `convert` are absent from it, although both are implemented in the same `match`** (lines 152 and 184). There is no full `--help`: `kt-cli --help` prints the same message and exits with code 2 (checked by running). So the only existing entry to the demo is not named even where the program lists its entries. This is exactly rule 20 turned inside out: a command did not grow by itself, it was orphaned — a relative of what t252 catches in the GUI.

### What gets in the way

**The material runs into a fork of three paths, and each has its own sticking point.**

- **`synth:` — self-sufficient, but shows the wrong thing.** The scheme is supported everywhere: parsing in `src/apps/kt-app/src/util.rs:13` (case-insensitive since R.33) and in `src/apps/kt-cli/src/main.rs:21`; `docs/file-format.md` carries it verbatim (item 147). But: thumbnails for such media are DISABLED by an explicit filter `filter(|m| !m.path.starts_with("synth:"))` (`src/apps/kt-app/src/thumbs.rs:171` and `add_media` there), the frame of the synthetic source is a solid color encoding the tick number (`src/libs/kt-media/src/synthetic.rs`, `read_video_frame`), and the source duration in the application is hard-coded at some hour (`util.rs:42`). This is already written directly in item 161 of `docs/porting-notes.md`: "`synth:` sources in the application give only sound — scenarios about the PREVIEW need a real file". A demo on `synth:` would leave the filmstrip and previews empty — that is, exactly what the demo is opened for. Waveforms, by the way, will be there: the peak pyramid for `synth:` is built in process (`util.rs:102`).
- **Files in the tarball — weight and aging.** But then the picture is real, and the demo honestly shows what the material looks like.
- **Generation on first opening — a clash with the invariants.** Today's generator calls an external `ffmpeg`, and the product does not allow itself that: invariant 9 ("no temporary files and no external `ffmpeg` binary") and invariant 10 (the binary is not linked with libav\*, everything goes through dlopen). Inside the program the same material would have to be written by `kt_ffmpeg::Muxer` — that is, the demo media becomes an ordinary export whose frame source is synthetic. Separately: `release-build.sh` checks "ldd without libav\*", and any solution that pulls files into the tarball passes this check without violating its spirit.

**Invariant 8 adds a step.** Thumbnails are taken from the sidecar `<project>.ktindex.db`, and "while there is no index — placeholders". A freshly created demo looks unfinished for the first seconds, regardless of how the material was obtained; for the FIRST impression this matters more than for a working project.

**Rule 20 ("nothing grows by itself")** forbids silently creating directories in `$HOME`: the place for the demo must be named by the user — as Apple did. **Rule 17 (paired ways are the same)** is violated twice and in opposite directions: `demo` works in the CLI and does not work in the GUI; opening a project works in the GUI (with stubs, item 207), while rendering it in the CLI refuses altogether. **Rule 18** requires a menu item to say WHAT and WHERE: "Open demo project" must say where the file will land before it lands.

### Which invariants it affects

The root `CLAUDE.md`: **8** (thumbnails/waveforms from the sidecar — the demo will start with placeholders), **9** and **10** (generating the material inside the product — only through `kt_ffmpeg::Muxer`, an external `ffmpeg` is forbidden). Invariant 12 is not affected: the demo is a file, not an animation, so repaint does not grow because of it. UI checklist: **17** (paired ways), **18** (the menu item says WHAT and WHERE), **20** (nothing grows by itself, the user names the folder).

ADDRESS: `src/apps/kt-cli/src/main.rs:60` — the class "an example from the distribution does not run" (otio #1554, #1694, #811; mlt #35) is POSSIBLE for us, has already happened, and is not caught by anything: at the first missing source, render prints `{resolved}: {e}` and does `return 1` instead of `open_source_or_placeholder`, which the GUI uses in the same situation (`src/libs/kt-media/src/placeholder.rs`, F-HLP-012). The demo is written by the same binary (`demo_project()`, `main.rs:363`), and the eight paths in `media/` exist neither in the distribution nor in the tree. The only guard is t151 (`src/apps/kt-cli/tests/cli.rs`): it checks the SCHEMA of the file (`Transition.1`, `Marker.2`, a byte-stable `roundtrip`) and opens not a single media, so "the example does not run" passes it by construction: in otio exactly the same way the format tests stayed green all year between #1554 and #1694, while the example from the README was crashing.

The defect "demo writes what render does not read" has already been moved into a separate task; for this note it is needed as proof that the class is ours, not someone else's.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages of the round: the material (what exactly to fill it with) and the entry (a menu item + the CLI command list) |
| Where the main work is | kt-cli (the material and the command registry), kt-app (the menu item, saving a copy), kt-media (what plays while there is no index) |
| Project format | not touched: the demo writes the standard `kt_otio::save_project`, the file is `v: 3` today |
| Tests to write | `cli::demo_renders_out_of_the_box` (demo → render without manual preparation); a check "the command list in the unknown-command message == the set of dispatcher branches"; a harness test of the menu item in the t76 family |
| Depends on | F-HLP-032 (the pipeline already runs the demo), F-HLP-012 (stubs — the fallback if the material is lost anyway), F-HLP-020 (the empty state — the second place from which the demo is offered) |
| Breaks compatibility | no |

## Verdict

We take it. Cheaper than it looks: the edit is already written, checked by a test, and every time passes through the screenshot pipeline — what is missing is the material and the door. The reconnaissance changed the framing in one place: the task is not "make a demo project" but "make the existing demo project openable", and the main defect is not in the GUI but in the fact that the file refers to eight nonexistent sources.

TEST: cli::demo_renders_out_of_the_box — fails today, and this is verified by running: `kt-cli demo d.otio` writes 57 KB and exits with code 0, and then `kt-cli render d.otio o.wav` prints "media/drone_coast.mp4: decode failed: opening file: No such file or directory (-2)" and exits with code 1 on the FIRST of the eight paths (`src/apps/kt-cli/src/main.rs:60` — `return 1` instead of `open_source_or_placeholder`, which the GUI uses in the same situation).

CHECK: src/apps/kt-cli/src/main.rs — the clause "the command list in the message about an unknown command is assembled from the same place as the dispatcher branches: `demo` and `convert` are either named in both or in neither"; today line 354 lists five commands out of seven, and `kt-cli --help` falls into the same branch and exits with code 2.

DEFERRED: until F-HLP-020 — where exactly to offer the demo (the empty state versus the File menu item) is decided there; here only the requirement is fixed that the path from the GUI must open a COPY in the folder the user named, and not the original from the distribution.

## Sources

- `src/apps/kt-cli/src/main.rs` (`demo_project()`, the command dispatcher), `src/apps/kt-cli/tests/cli.rs` (t151), `site/screenshots.py` (`gen_media`, `gen_project`), `src/apps/kt-app/src/toolbar.rs`, `src/apps/kt-app/src/thumbs.rs`, `src/apps/kt-app/src/util.rs`, `src/libs/kt-media/src/synthetic.rs`, `docs/porting-notes.md` items 147, 161, 206–209, `docs/file-format.md` — read and checked by running the binary `src/target/debug/kt-cli` on 07.09.2026.
- Final Cut Pro 12.3, "Download a demo project":
  https://support.apple.com/guide/final-cut-pro/download-a-demo-project-ver4dec14a61/mac
  (access date 2026-09-07; the page is marked 26.08.2026)
- Blender 5.2 LTS Manual, "Application Templates":
  https://docs.blender.org/manual/en/latest/advanced/app_templates.html
  (access date 2026-09-07)
- Blender 5.2 LTS Manual, "Defaults" (Save Startup File / Load Factory Settings):
  https://docs.blender.org/manual/en/latest/getting_started/configuration/defaults.html
  (access date 2026-09-07)
- Blender 5.2 LTS Manual, "Video Editing App Template":
  https://docs.blender.org/manual/en/latest/video_editing/setup/app_template.html
  (access date 2026-09-07)
- Blender Demo Files (the demo files are downloaded separately, not in the installer):
  https://www.blender.org/download/demo-files/ (access date 2026-09-07)
- VS Code 1.136, UX Guidelines, "Walkthroughs":
  https://code.visualstudio.com/api/ux-guidelines/walkthroughs
  (access date 2026-09-07; the page is marked 02.09.2026)
- DaVinci Resolve 21, the Training page (workbooks; the lesson material separately):
  https://www.blackmagicdesign.com/products/davinciresolve/training
  (access date 2026-09-07)
- Quotes: blenderartists.org, threads 1207534 and 1469959 (see the section above).
- Trackers (export of 08.09.2026; the dumps are in `~/.cache/kinotrain/trackers/` and do not go into the repository — `_trackers.md`, the section on licenses): `otio` #699, #811, #1554, #1694; `mlt` #35; `openshot` #736, #794, #1447, #1949 (and 9 more of the same class); `pitivi` #1536. The number in the `SIGNAL:` line is reproduced by `python3 src/scripts/tracker-rank.py --query '<filter>'` with the same filter.
- **State of the export as of 08.09.2026:** `blender-vse` and `ffmpeg-trac` are empty; `openshot` has an incomplete dump (the state file shows a restart from the first page with 4516 tasks recorded out of about 4907). Therefore only `otio` 680 and `mlt` 601 — both with `done: true` — take part in the share of the `SIGNAL:` line, and the thirteen openshot tasks about the tutorial window are given as a count without a share.
- No live verification is expected for programs outside the core (the `_editors.md` rule); the Premiere, After Effects and VEGAS Pro rows are marked `[NOT VERIFIED]` — helpx and the Boris FX help do not give out their content from this environment, and the session's `WebSearch` budget was exhausted.
