---
id: F-PRJ-003
title: A project opened on another machine
status: candidate
priority: B
cost: M
category: project/structure
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08, otio-lib-0.18]
we_have: partial
crates: [kt-otio, kt-model, kt-media, kt-app, kt-cli]
invariants: [7, 9, 10, 11]
ui_rules: [16, 19, 20]
round: null
point: null
tests: []
depends_on: [F-MED-005, F-RES-003, F-UI-003]
trackers: [shotcut, mlt, kde-bugzilla, kde-invent, openshot, flowblade, olive, pitivi, otio, losslesscut]
checked: 2026-09-08
---

# F-PRJ-003 — A project opened on another machine

## Essence

The project has moved: to a second computer, to a colleague, from Windows to Linux, from a laptop to a render machine. Paths to the media are only half the trouble; the other half is everything the project does not CONTAIN but only NAMES: a title's font, a LUT, a third-party effect, the chosen encoder, a cache folder, a decimal separator. Half of the losses are loud (the file did not open, the effect is missing); the other half are silent: the project opened, the frame is different, and nobody said anything. Here the subject is the COMPLETENESS of the move as a class: what exactly in a project is not part of the project.

Boundaries. Relinking sources and collecting media into one folder is F-MED-005 (`media/organization/relink-and-consolidate.md`); a project created by ANOTHER VERSION of the program is F-RES-003 (`resilience/durability/project-format-migration.md`); the panel layout and whose file owns it is F-UI-003 (`ui/input/workspaces-and-layouts.md`). None of the three answers the question "what else leaves silently", and that question is answered here.

## Who has it and how

The "How it works" column answers ONE question: what the project takes with it and what it leaves on the author's machine. The media-copying mechanism (Collect Files, Project Manager, Media Management, Consolidate) is covered in F-MED-005 and is not repeated here; only what it does NOT take is taken from there.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | no name; `Copy media with project` | Portability is not singled out as a subject: `.veg` is binary and Windows-only, so the question "will it open on a Mac" does not arise at all. Paths to the media are absolute and are fixed by a search dialog when the project is opened. Title fonts are taken from the system fonts; LUTs are file paths. Copying the media with the project exists; copying DEPENDENCIES does not |
| Adobe Premiere | 26.x | `Project Manager`, `Missing Fonts`, Team Projects | It moves between Windows and macOS as a single file, and yet it set up a SEPARATE product for collaborative work (Team Projects, with its own "Create a new Team Project from a version" and "Archive"). That is, the vendor itself admitted that moving the file does not solve the task. A missing font is a modal dialog on opening, with an attempt to activate the font from Adobe Fonts. A missing third-party component is reported as a separate error ("the preset used by one or more sequences in this project requires third-party components that could not be located") — `[NOT VERIFIED]`, from a forum thread, not from the documentation. Media gets collected; fonts and plugins do not |
| DaVinci Resolve | 21 | **Path Mapping** + `Project Media Locations` + `.drp` | The most complete and structurally different implementation: the path is translated ON THE FLY by a table that the RECIPIENT fills in. The manual works through exactly our scenario: the editor on a Mac (`/Users/editor/cloudfolder/Episode 12`) and the colorist on a PC (`D:\Projects\Episodes\cloudfolder\Episode 12`). There is `Automatically setup when relinking`: one manual relink creates a rule and fixes the rest. Separately, the old `Mapped Mount`, which the manual itself declares the worst ("requires each user knowing the file path of all the other users"). LUTs live in the shared installation folder, not in the project |
| Final Cut Pro | 12.3 | `Copy a Final Cut Pro project to another Mac`; `Motion Content` storage location | macOS-only, so there is no cross-OS support; however, it is the only one that made portability an EXPLICIT STORAGE SETTING. Custom titles, effects and generators made in Motion are by default kept in `~/Movies/Motion Templates` and do NOT travel with the library; the library properties have a `Motion Content: In Library` switch and a `Consolidate` button, and it must be pressed BEFORE copying. A third-party FxPlug is not covered by anything; the manual says so in plain text |
| After Effects | 26.x | `File > Dependencies > Collect Files`; warning about font substitution | The only one that counts as a dependency not just the media file: the `Collect Files` report lists "files, effects, and fonts" (F-MED-005). But it lists them in the REPORT, and copies only the files. A missing font is announced on opening and is then substituted silently until the end of the session; missing effects and footage are searched for with separate filters in the project panel. `Reduce Project`, `Obey Proxy Settings`, `Generate Report Only` are options of the same operation |

**Outside the core.** Two programs are mandatory under the rules of `_editors.md`, and for different reasons. Kdenlive is the only NLE for which "projects are not portable" exists as a public ticket with a status and a date, not as a forum-thread genre; the tally of all three `SIGNAL:` lines below relies on its tracker and on the trackers of its related projects. OpenTimelineIO is the ORIGIN of our `target_url` and the only place where the form of a media path is discussed as a specification, not as behavior.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Kdenlive | 26.08.0 | `Archive Project`; ticket "projects are not portable" | The project is text MLT-XML; paths are not relative everywhere (`.pgm` transitions, the `root=` root, `.TRF` tracking are absolute). The archive collection exists and does not take fonts (#501914), the built-in `Counter` generator (#420623) or built-in luma masks (#492435). The class is acknowledged by the title of ticket #518992, CONFIRMED since 15.04.2026 |
| OpenTimelineIO | 0.18.x | `ExternalReference.target_url` | A string with no declared form. The library's own tracker still keeps two open tickets about what this string must be: #904 "Clarify the form and constraints of target_url" (2021, "wild west" — verbatim) and #1985 (11.12.2025), where the discrepancy surfaced on files written by Premiere 26.1 beta. Percent-encoding, `file://` and the resolution of relative references are not pinned down in the specification |

## The trick behind it

**1. Portability is a property not of the file but of the RECEIVING setup, and the only one that recognized this moved the setting to the recipient's side.** All the others solve the problem of "how to write a path that everyone will understand", and run into the fact that such a path does not exist: `D:\` does not exist on Linux, `/Volumes/` does not exist on Windows, and a volume name is not portable in principle. Resolve turned the question around: the project stores the NAME of the shared folder, and each participant declares locally where this folder lies. "Path Mapping lets the user just present their own file path, and DaVinci Resolve takes care of the translation for all the other users instead" (Reference Manual 21, ch. 6). This is also where `Automatically setup when relinking` comes from: relinking fixes not a file but a RULE, and the other files are fixed as a consequence. The practical conclusion for us is unpleasant and precise: a relative path (our decision R.24 item 7) is the same kind of mapping, only with one immutable rule, "everything stays where it was relative to the project". It is free and covers the most frequent case, and it can do exactly nothing when the material was on a separate mount point, because then `url::relativize` returns `None` and an absolute path goes into the file (finding of F-MED-005).

**2. Every feature that put its own assets into a user storage outside the project subtracted from portability; this is the reverse side of conclusion 2 of the first wave.** The catalog README recorded: "Four features run into the absence of a 'user storage outside the project': the FX preset library (F-AUT-004), title templates (F-TXT-002), export presets (F-EXP-001), keyboard layouts (F-UI-002)". The trackers show the count from the other side. Flowblade #1168: a project on an external drive refers to `~/.local/share/flowblade/projectsdata/<hash>/savefiles`, and on another machine this produces an error not when OPENING but when SAVING, that is, it is discovered after an hour of work. FCP arrived at the only form in which "outside the project" and "portable" do not contradict each other: the storage has an `In Library` switch and a `Consolidate` button, and by default it is NOT in the portable position, while right next to it is honestly written that the switch has no effect on a third-party FxPlug. So a storage outside the project is not a free place, but the second half of each of the four features: it must have a day when its contents are put inside.

**3. A silent substitution is worse than a loss, because it is not visible in the preview, and the warning comes at the only moment when there is nothing to check it with.** A missing file is visible: a placeholder in the frame, a red card in the bin, a count in the preflight. A substituted font is visible nowhere: the frame is valid, just not the right one, and the editor learns about it from the client. AE is the only one that says it out loud ("Font substitution will occur until the originals become available"), and says it in a modal window on OPENING, when the timeline has not yet been drawn; after that the substitution lives for the whole session, and the affected layers can be found only with a separate filter in the project panel. The same class, two orders of magnitude lower, is the decimal comma: shotcut #713 describes how a project saved in a locale with a comma opens without a single error on a recipient with a dot, and all fractional numbers in it become zeros. The cause is named in mlt #580: the library serialized numbers in the interface locale, and `29,97` went into the XML. "It works on my machine" begins exactly here: both sides see a sound file and a sound program, and both are right.

## Pros

- Path mapping as a NAMED agreement (Resolve) removes the cross-OS question entirely and, incidentally, makes relinking self-accumulating: one indication creates a rule, and the rule fixes the other files. This is the same "one pointed, all found" that F-MED-005 found in three trackers, but SURVIVING the session.
- A report instead of a copy (`Generate Report Only`) is cheap and honest: a dependency can be listed even where it cannot be copied because of its license, such as a font or a plugin. F-HLP-011 treats the same surface as a preflight.
- A storage with a "put inside" switch (FCP `Motion Content: In Library`) is the only known form in which a library of presets does not reduce portability.
- The text format of the project is half the battle by itself, and users name this first: shotcut #710 begins with praise of XML versus a binary format and only then asks for relative paths. A binary `.veg` does not even allow looking at how a received project differs from its own.
- Refusing to host plugins is paid for once and closes a whole class forever: for F-EXT-001 this is formulated as a user requirement, the program "works on someone else's machine with what is on that machine".

## Cons

- Path Mapping is a setting of the RECIPIENT, so it requires that they know about it before they see "media offline". Nothing hints at it: the very fact that Resolve keeps an outdated `Mapped Mount` alongside and explains the difference in a paragraph of the manual shows that the mechanism is not self-evident.
- The report does not copy. AE lists "files, effects, and fonts" and takes only the first with it; Kdenlive got a separate task for this ("Archive project don't bundle fonts", #501914), UNCONFIRMED since 23.03.2025.
- Half of the five solved cross-OS by having nowhere to move to: `.veg` is Windows, the FCP library is macOS. "Portability" for them is measured by moving between two identical machines, and that is not the same question.
- Settings tied to the hardware travel with the project where they live in it, and the class is not even acknowledged: kde-bugzilla #476371 "Video Resolution and fps change when working on different pc" is closed as WORKSFORME.
- The locale remains a hole in the interface even after the format is fixed: in mlt #580 the serialization was fixed, but kde-invent #589 (2020) asked not to fix the format but to WARN the user that they have a comma, and to advise changing the operating system setting.
- Collecting media flattens the folder structure and loses files with the same name; this was analyzed in F-MED-005 and is not reopened here.

## What users say

The class, named by its own name, is one and the same phrase in four trackers over eight years:

> I copied the entire contents of a project file to another machine, Loading
> the project says it is missing file clips for every clip.
> — OpenShot #1334, 04.03.2018, https://github.com/OpenShot/openshot-qt/issues/1334

The same diagnosis, but the loss is not of media, it is of the program's SERVICE directory, and it is discovered on saving, not on opening:

> I have an external Harddisk with my projects. If I move the HD to another PC
> running the same version OS and Flowblade I get references to the
> ~/.local/share folder and .config/flowblade folder which result in an IO
> error on saving the project.
> — Flowblade #1168, 02.09.2024, open, https://github.com/jliljebl/flowblade/issues/1168

Why the text format is half the answer, in the words of the one who asked for relative paths:

> I would like this project file to always use relative paths when the resource
> is inside the project folder, for better portability of projects. You should
> be able to move the project folder to somewhere else without breaking any
> path references.
> — Shotcut #710, 14.03.2019, https://github.com/mltframework/shotcut/issues/710

The silent half of the class is the recipient's locale:

> best friend loads the file and realises all numbers are **zero** which did
> have any fractions (breaking everything from chroma key to ... anything,
> really)
> — Shotcut #713, 16.03.2019, https://github.com/mltframework/shotcut/issues/713

The cause, named in the library that was writing these numbers:

> At Kdenlive, we regularly have issues with different locales because by
> default numbers are converted to string using the default locale.
> — MLT #580, 05.06.2020, https://github.com/mltframework/mlt/issues/580

And negative feedback of a special kind, a request not to fix but to warn:

> Kdenlive has know issues when used on computer that uses "," (comma) as a
> decimal separator. is possible to change the decimal separator to "." (point)
> on your computer preferences
> — kde-invent #589, 22.03.2020, https://invent.kde.org/multimedia/kdenlive/-/work_items/589

About the font that is substituted and keeps being substituted:

> This project uses fonts that are not currently available on this computer.
> Font substitution will occur until the originals become available.
> — quote of the After Effects warning, cited by user jk22973525,
> 12.01.2018, https://community.adobe.com/t5/after-effects-discussions/after-effects-warning-fonts-that-are-not-currently-available/td-p/9598778

And about the form of the path in an external format, in the tracker of the reference library:

> The confusion emerged from Adobe Premiere Beta 26.1 exporting OTIO files with
> media references with said `file://` prefix
> — OTIO #1985, 11.12.2025, open, https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/1985

SIGNAL: effect as a foreign binary — 9 tasks in mlt, `frei0r|ladspa|\bofx\b|openfx|\bvst\b`; share 1.5% against 0.1% in shotcut

SIGNAL: recipient's locale — 3 tasks in shotcut, `decimal (separator|comma|point|mark)|comma.{0,20}decimal|locale.{0,30}(break|number|decimal|separator|xml|file|save|transfer)|(number|value|xml|project).{0,25}locale`; share 0.2% against 0.1% in kde-invent

SIGNAL: "project is not portable" as a title — 1 task in shotcut, `(project|projects)\b.{0,40}(not portable|portab)|portab\w*.{0,40}project`; share 0.1% against 0.0% (1 of 6346) in kde-bugzilla

What lies behind these numbers.

**The first pair is about WHERE the dependency that does not travel lives.** The filter catches not a complaint but the name of an external supplier of effects, and the spread is huge: 1.5% in mlt against 0.1% in shotcut, a fifteenfold difference between the library that LOADS plugins and the application that shows them. Between them, in descending order: olive 1.0% (18 tasks, of which #14 is a request to add VST/AU at all), kde-invent 0.5%, flowblade 0.4%, kde-bugzilla 0.2%, pitivi 0.2%. This reads unambiguously: in all nine open editors an "effect" is a foreign binary, and the price of this decision is paid not where the effect is chosen, but a floor lower, in the loader. For the move the consequence is direct: the list of a project's effects is a list of programs that MUST be installed on the recipient's side, and none of the nine carries it with itself. This is not our pain (F-EXT-001 is rejected), but it explains why for the others "give the project to a colleague" is not solved by collecting files into a folder at all.

**The second pair is about the quietest failure of the domain.** Seven tasks across three trackers is FEW, and that is the whole finding: shotcut 0.2%, kde-invent 0.1%, kde-bugzilla 0.0%. A class that breaks all fractional numbers at the recipient's side at once produced seven tickets in ten years, and they are laid out tellingly: two (shotcut #166 and #186) are not about the move at all, but about the program's own interface on the author's machine; two (kde-bugzilla #357991, #414370) are the same on the other side; one (kde-invent #589) asks not to fix but to WARN and advise changing the OS setting; and only two (shotcut #713, kde-invent #713 "Use the C locale for project files") name the real cause, that a file format has no right to depend on the locale. The reason for the small number is the same thing that makes the class dangerous: to file such a ticket one must ALREADY understand that the locale is to blame, while both sides see a working program. The numbers of the domain measure not the frequency of trouble but the frequency of guessing.

**The third pair is about the fact that the class cannot be filed as such.** A filter on the word "portable" next to the word "project" gives exactly one task in each of four trackers: kde-bugzilla #518992 "Kdenlive projects are not portable" (CONFIRMED, 15.04.2026), openshot #1334 "Projects not portable to another machine" (2018), flowblade #1168 "Projects are not portable" (2024, open), shotcut #710 (2019, about relative paths). Four independent projects, four almost verbatim matching titles, eight years, and not one duplicate, not one link to each other. This can be noticed only through the trackers: the title "not portable" has no assignee, because it has no single place in the code, and each such ticket either stands for years (flowblade, kde-bugzilla) or splits into private ones (shotcut #710 becomes relative paths; and separately #713 becomes the locale). Hence the subject of this note: the class has no ticket of its own, so it must have a note of its own.

Mandatory `--rejected` review: there are no rejections on the merits in the domain, but there is an indicative one: kde-bugzilla #476371 "Video Resolution and fps change when working on different pc", closed as WORKSFORME. By reading rule 1 of the numbers (`_trackers.md`) this is not "fixed" but "not reproduced": the class "project parameters changed when the machine changed" is not even recognized as existing. Two more rejections, losslesscut #2055 and #2343 (not_planned), are about a different meaning of the word "portable" (a program run from a flash drive), and are not counted in the third line's tally.

## How it fits Kinotrain

### What already exists

Our project refers outward EXACTLY ONCE, and this is verified by grep: across the whole of `src/libs/kt-model/src/project.rs` there is one field named `path`, namely `MediaRef::path` (`project.rs:1780`). There is no font, no LUT, no plugin, no cache folder and no encoder in the project file; not "not yet added", but because they are not in the program at all:

- **A relative path by construction.** `src/libs/kt-otio/src/url.rs`: `media_target_url()` makes the media path relative to the project directory with POSIX slashes, with a fallback to `file://` with percent-encoding; `resolve_media_path()` reads all three kinds, and `synth:<pattern>` passes through verbatim. What shotcut #710 asks for is our decision R.24 item 7 and a golden file: in `src/fixtures/demo-3d.otio` all four `target_url` values are `media/m1.mp4`, `media/m2.mp4`. The case "the whole folder was moved" is free.
- **Numbers do not depend on the locale.** A grep for `setlocale`/`LC_NUMERIC` across `src/` gives not a single hit in the serialization paths: the only mention of the locale is `apps/kt-app/src/i18n/detect.rs`, and it is about the choice of LANGUAGE, not about the number format. `serde_json` always writes the fraction with a dot. The class of shotcut #713 and mlt #580 is structurally impossible for us.
- **There are no plugins.** F-EXT-001 is rejected, and its verdict names exactly our subject: foreign code "makes bytes a property of the machine". All video and audio FX live in `src/libs/kt-fx`, in the tree, and travel together with the binary. The class "an effect turned into a hole" (the first `SIGNAL:` line) is impossible for us.
- **There are no fonts in the project, twice over.** There are no titles (F-TXT-001, `we_have: none`), so a font cannot get into the model; and the program's own interface does not depend on system fonts: the subsets are built into the binary (`src/apps/kt-app/src/icons.rs:55-57`, `include_bytes!`), and the completeness of glyphs is held by test t100. The program looks the same on someone else's machine as on its own.
- **The hardware is chosen on the spot, not taken from the file.** Invariant 10: the binary is not linked with `libav*`, FFmpeg is loaded via dlopen, so one and the same executable uses the recipient's SYSTEM FFmpeg. Invariant 9: the encoder is chosen automatically with a real opening check (`src/libs/kt-ffmpeg/src/muxer_impl.rs:16` and onward, nvenc → vaapi → qsv → libx264 → mpeg4). Render settings do not get into the file at all: `RenderSetup` (`src/apps/kt-app/src/render_dialog.rs:46`) has no `serde` and does not even survive a restart. The class "a foreign encoder traveled" is impossible.
- **The program's settings do not travel with the project.** `eframe::App::save` (`src/apps/kt-app/src/app.rs:5247`) puts exactly two lines into the application's storage: `lang` and `last_project`.
- **A refusal instead of a silent divergence where the hardware decides.** A project with 3D on a machine without a GPU is an explicit error `ExportError::GpuRequired` (`src/libs/kt-engine/src/export.rs:407`), not a silent "rendered differently".
- **The sidecar does not require a move.** `<project>.ktindex.db` (`src/libs/kt-media/src/index.rs`) is regenerated; until it is rebuilt in place, the waveforms and thumbnails show placeholders (invariant 8). A caveat: the database rows are keyed by the ABSOLUTE path, so after a move the index is built anew in full. The finding and its cost are already analyzed in F-MED-005 and are not reopened here.
- **Foreign fields survive our round trip.** Passthrough (`Retained`, `src/libs/kt-otio/src/convert.rs:72`) keeps foreign namespaces and unknown schemas verbatim, t144–t147. A received foreign project does not lose weight from the fact that we opened it.

### What is missing

1. **There is no path mapping and nothing to build it from.** Our relative path is one immutable rule; a table "logical name → local path" (Resolve Path Mapping) is absent both from the project and from the program's settings, and material from a separate mount point travels as an absolute path.
2. **There is no completeness check before handing a project over.** An analogue of `Generate Report Only` — "here is what this project needs on the other side" — does not exist; the closest thing is the list of unavailable sources at the moment the render settings are opened (`render_dialog.rs`), that is, on the RECIPIENT's side and too late.
3. **`kt-cli` cannot name dependencies.** The commands are `new`, `convert`, `demo`, `roundtrip`, `render`, `export` (`src/apps/kt-cli/src/main.rs:127`); the cheapest form of check before handing over (`kt-cli deps <project>`) is missing, although all the material for it is already in the model.
4. **`meters_w` travels with the project.** The only interface field on the far side of the boundary (`UiState::meters_w`, `src/libs/kt-model/src/project.rs:1547`; written in `src/apps/kt-app/src/app.rs:763`, overrides mine on opening — `app.rs:713`): open a foreign project, and the meters panel takes the width of the foreign one. The finding and the decision belong to F-UI-003; here it is confirmed from the other side and counts as the ONLY known case of "a setting traveled with the project".

### What gets in the way

**The path separator and the "absolute" flag are determined for us by the reader's OS, not by the file, and data is lost silently because of this.** `decode_file_url` (`src/libs/kt-otio/src/url.rs:27`) returns its result BEFORE the check for absoluteness, and `media_target_url` (`url.rs:85`), for a path that is not absolute on this OS, does `media_path.replace('\\', "/")` and writes it as a relative one. Both cases are reproduced on the built `kt-cli` with a single command (`roundtrip` over a copy of `fixtures/demo-3d.otio` with a substituted `target_url`), and both return exit code 0:

- `file://C%3A%5CFootage%5Cm1.mp4` — what OUR OWN writer produces on Windows for material on another drive (there `relativize` returns `None`) — on Linux turns into `C:/Footage/m1.mp4`, that is, the absolute path becomes RELATIVE to the project directory, and the next save writes the loss to disk;
- `media/take\01.mp4` — a legitimate file name on ext4 — turns into `media/take/01.mp4`, that is, one file becomes a file in a subdirectory.

The form `file:///C:/Footage/m2.mp4` (which, according to OTIO #1985, Premiere 26.1 writes) behaves correctly: it becomes the non-existent `/C:/…` and is honestly shown as a missing source. So of the three forms, two are silent and one is loud, and the loud one is foreign.

ADDRESS: src/libs/kt-otio/src/url.rs — the class "the project moved between OSes and silently lost its path" is POSSIBLE for us, and nothing catches it: `decode_file_url` (`url.rs:27`) returns the path before the absoluteness check, `media_target_url` (`url.rs:85`) rewrites a non-absolute path through `replace('\\', "/")`, and test t147 (`src/libs/kt-otio/tests/format.rs:289`) checks all three forms of `target_url` ONLY in POSIX form — there is not a single input with `C:`, with a backslash, or with `file:///C:/…` anywhere in the crate.

The other pressure points:

- **Byte equality of the picture is not a property of the project, but a property of the pair "project + machine".** Invariant 11: the CPU path is byte-for-byte against the reference and gated to `linux/x86_64`; the GPU path is byte-for-byte ONLY ON ONE DEVICE, and between machines only agreement with the CPU within tolerances is guaranteed (t181/t182/t183). With GPU by default (R.29) this means: the same project on another machine will legitimately produce DIFFERENT BYTES. No claim of "divergence is a defect" can be made here, and any statement about portability must make this distinction.
- **Invariant 7 (the composition code is one) holds only half.** It guarantees that preview and export coincide ON ONE backend, and says nothing about two machines. A related note about the divergence of preview and output is F-RES-008.
- **The dependency report is text, and text costs five languages and a limit of 160 characters** (`src/apps/kt-app/build.rs:42`), glyphs of t100, subsets, t125/t128/t131; details are in F-HLP-033.
- **Rule 20 of the checklist.** Path mapping is state that the program "remembers"; it must be created by an explicit command, not accumulate by itself on every relink. Resolve did the opposite (`Automatically setup when relinking` — a checkbox), and this is exactly the case where another product's default does not suit us.

### Which invariants it touches

The root `CLAUDE.md`: **9** (the export is in-process, the encoder is chosen on the recipient's machine by a real opening check) and **10** (dlopen of the system FFmpeg) are the two thanks to which our class "foreign hardware" is already solved better than in nine open editors taken together, and they must not be touched; **11** (determinism) is really affected: the promise "the recipient will get the same result" is not fulfilled by construction on the GPU path, and the note must speak about tolerances, not about bytes; **7** (the composition code is one) only as a reminder that it concerns backends, not machines.

Checklist rules of `src/apps/kt-app/CLAUDE.md`: **16** (impossibility must be an ERROR: today a foreign `C:` path gives neither an error nor a placeholder, because it turns into a valid relative one), **19** (the dialog must say where to change the decision — applies to any future path mapping), **20** (nothing grows by itself: the move rule is created by a command, not by a habit of the program).

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2–3 stages: (1) cross-OS path: `target_url` is read and written in the declared form, a foreign absolute path becomes a PLACEHOLDER, not a silent relative link — S; (2) `kt-cli deps` and the same check before handing over in the GUI — S; (3) the mapping "logical name → local path" as an explicit command — M |
| Where the main work is | kt-otio (`url.rs`: path form, the "absolute" flag without involving the reader's OS), kt-cli (`deps`), kt-app (message about an unresolvable path under rule 16), kt-model — only in stage 3 and only if the mapping is decided to be stored in the project |
| Project format | stages 1–2 do not touch it: `target_url` is already a string, the form is only pinned down and documented in `docs/file-format.md`. Stage 3 touches it — the mapping table is either a new key in `metadata.kt` (then `kt.v` grows, and the reader of old files must survive its absence), or a file of the program's settings; the fork is decided in the round, not here |
| Tests to write | extend t147 with inputs `C:\…`, `file://C%3A%5C…`, `file:///C:/…` and a relative path with a backslash in the name — today there are none; "a foreign absolute path of another OS is read as a MISSING source and is rewritten back UNTOUCHED" (idempotence under the rules of t144–t147); `kt-cli deps` lists exactly `MediaRef::path` and `synth:` and nothing more; a harness test "opening a foreign project does not move the meters panel width" (shared with F-UI-003) |
| Depends on | F-MED-005 (relinking — what the found issue is fixed with), F-RES-003 (format version — the neighboring half of "a file from another place"), F-UI-003 (`meters_w`), F-HLP-011 (preflight — the surface for the dependency report) |
| Breaks compatibility | no; stage 1 changes only the BEHAVIOR on inputs that are processed incorrectly today |

## Verdict

**We take stages 1–2 and defer stage 3 until a second scenario appears.**
The reconnaissance changed the subject of the note. It was expected that the weak point was the composition of what travels: fonts, LUTs, plugins, hardware settings. It turned out that we do not have this composition at all, and not through an oversight: there are no titles, no LUTs, plugins are rejected (F-EXT-001), the encoder is chosen on the recipient's machine by a real opening check (invariants 9 and 10), render settings do not survive even a restart, and the whole model contains exactly one field with a path. In terms of COMPLETENESS of the move we do not catch up with the five; we are in a position they do not have, and it was paid for by rejecting plugins, not by work.

The weak point turned out to be one level lower and entirely ours: **the path separator and the "absolute" flag are determined by the READER's operating system**, therefore of the three forms of a foreign `target_url`, two are lost silently, and one of those two is the one our own writer produces on Windows. This is exactly the outcome that F-RES-003 calls the third and worst ("it opened, and parts of the edit are no longer in it"), only applied not to the format version but to the path: there is no error, the exit code is 0, and the next save writes the loss. The cost of the fix is S, and all of it is in `url.rs` and in t147.

Stage 3 (path mapping modeled on Resolve) is deferred deliberately: today we have one user on one machine, and the relative path already covers the only live scenario, "everything moved together". Creating named mount points before a second participant appears means creating state that nobody can verify. But the caveat of the first wave remains in force with the opposite sign: as soon as a "user storage outside the project" appears (conclusion 2 of the README, four features), it must have a "put inside the project" switch on the same day, otherwise we would by our own hands reproduce Flowblade #1168, where a project refers to the author's service directory and dies on the recipient's side when SAVING.

## Sources

The core is based on manuals; no editor was run live. Open editors are based on tracker dumps (`_trackers.md`). Our code was verified by grep and by running the built `target/debug/kt-cli`. The access date for everything is 2026-09-08.

**Core.**

- DaVinci Resolve 21 Reference Manual, ch. 6 "Setup and Workflows", section "Path Mapping" (the Mac/PC example, `Project Media Locations`, `Automatically setup when relinking`, comparison with `Mapped Mount`) and ch. 2 "Using Path Mapping to Access Volumes From Other Operating Systems" — PDF downloaded and read locally,
  https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_21_Reference_Manual.pdf
- Final Cut Pro User Guide for Mac 12.3, "Manage Motion content" (pp. 874–875): `Motion Content: In Library`, `Consolidate`, and the phrase "Regardless, you must manually track and move any third-party (FxPlug) content, because it is not managed within the Final Cut Pro library" — PDF read locally,
  https://support.apple.com/guide/final-cut-pro/welcome/mac
- After Effects, `Collect Files` and the composition of the report ("files, effects, and fonts") — based on the analysis in F-MED-005, not reopened; the text of the font-substitution warning is from the Adobe forum (link below). `[NOT VERIFIED]` for 26.x: `helpx.adobe.com` still does not open from this environment ("proxy refused the connection"), the caveat of the first wave in `_editors.md` remains in force
- Adobe Premiere 26.x: the sections `Project Manager`, `Adobe Fonts`, `Team Projects`, `Convert online clips to offline clips` are confirmed only by the table of contents of Adobe's help; `[NOT VERIFIED]` for the same reason
- VEGAS Pro 2026.0: binary format and Windows-only — from F-RES-003, where this is analyzed with a quote from the vendor; no separate check was made

**Trackers** (the numbers are reproduced by `tracker-rank.py --query` with the filters from the `SIGNAL:` lines; the dumps are in `~/.cache/kinotrain/trackers/` and are not in the repository).

- The class by its own name: https://bugs.kde.org/show_bug.cgi?id=518992 (CONFIRMED, 15.04.2026), https://github.com/OpenShot/openshot-qt/issues/1334, https://github.com/jliljebl/flowblade/issues/1168 (open), https://github.com/mltframework/shotcut/issues/710; related — openshot 988, 2727, 3129, kde-bugzilla 416469, kde-invent 729
- Locale: https://github.com/mltframework/shotcut/issues/713, https://github.com/mltframework/mlt/issues/580, https://invent.kde.org/multimedia/kdenlive/-/work_items/589 and 713 ("Use the C locale for project files"), https://bugs.kde.org/show_bug.cgi?id=414370, 357991; shotcut 166, 186
- Kdenlive paths and dependencies: 501914 (fonts are not packed, UNCONFIRMED), 431604 (`.pgm` as absolute), 495091 (the root path breaks moving the directory, ASSIGNED since 2024), 363363, 405249 (text in SVG is drawn with the machine's fonts), 476371 (closed WORKSFORME)
- Plugins as an external dependency: mlt 274, olive 14, kde-bugzilla 513318, pitivi 1449 — and the whole filter of the first `SIGNAL:` line
- The form of `target_url` and cross-OS paths in the reference library: https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/904 (open since 2021), 1985 (11.12.2025); separately 581 — the separator of the path list in an environment variable (`os.pathsep` instead of `:`), the same class at the installation level
- Another meaning of the word "portable" (a program run from a flash drive, not counted): losslesscut 645, 2055, 2343

**Forums (quotes).**

- https://community.adobe.com/t5/after-effects-discussions/after-effects-warning-fonts-that-are-not-currently-available/td-p/9598778
  (the exact text of the AE warning about font substitution, 12.01.2018)

**Our code (verified by grep and by running).**

- `src/libs/kt-otio/src/url.rs`, `src/libs/kt-otio/tests/format.rs:289` (t147), `src/libs/kt-otio/src/convert.rs:72` and `:1362`, `src/libs/kt-model/src/project.rs:1780` and `:1547`, `src/apps/kt-app/src/render_dialog.rs:46`, `src/apps/kt-app/src/app.rs:5247`, `:763` and `:713`, `src/apps/kt-app/src/icons.rs:55`, `src/libs/kt-ffmpeg/src/muxer_impl.rs:16`, `src/libs/kt-engine/src/export.rs:407`, `src/apps/kt-cli/src/main.rs:127`, `src/fixtures/demo-3d.otio`
- Reproduction of both silent losses: a copy of `fixtures/demo-3d.otio` with a substituted `target_url` + `target/debug/kt-cli roundtrip in.otio out.otio` (exit code 0, output "byte-stable: false")

**What was searched for and NOT found** (so that the next wave does not search again). In the nine open trackers there are practically no complaints about font substitution when a project is moved: the filter `missing.*font|font.*missing|no font|wrong font` gives 2 tasks out of 25,442 records (kde-bugzilla 401903, openshot 3063), and `font.{0,30}(embed|not installed|install|substitut)` gives exactly one (openshot 4536), and that one is about installing a font on ITS OWN system, not on a foreign one. It seems the class exists but is filed under another name: in open editors a title is the engine's generator or SVG, and a font substitution looks like "text is drawn incorrectly" (kde-bugzilla 405249 — text in an imported SVG; 499355 — the subtitle font, closed WORKSFORME). There is nothing to check this guess with by counting, and the next wave should not start with the word `font`. Therefore "The trick behind it" relies in this part on the manuals and the Adobe forum, not on counting. The dumps of `blender-vse` and `ffmpeg-trac` are empty as of this date; `openshot` is not finished (4516 records out of ~4907 declared in the registry), so openshot is quoted but is not included in the percentage comparisons of the `SIGNAL:` lines — the same order as in F-MED-005.
