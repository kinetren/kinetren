---
id: F-EXT-002
title: Scripting and automation — an API for our own operations on the project
status: candidate
priority: C
cost: M
category: plugins/scripting
editors: [vegas-pro-2026, premiere-26, resolve-21, resolve-20, fcp-12, after-effects-26]
we_have: partial
crates: [kt-otio, kt-cli, kt-model]
invariants: [2, 5, 9, 11]
ui_rules: [17, 20]
round: null
point: null
tests: []
depends_on: [F-EXT-001]
checked: 2026-09-07
---

# F-EXT-002 — Scripting and automation — an API for our own operations on the project

## Essence

An editor has to do the same thing forty times over a project: rename
and relink forty sources, lay out the edit by a table (who speaks,
from which second, in what shot), render a piece for each marker with
the marker's name in the file name, change one effect parameter on all clips
of a track, export a CSV list "clip — source — timecode" for the client,
and pull material from the storage system. By hand this is hours of identical
movements, and the question "how do I do this with a program" comes up earlier than
any question about a creative feature. The answer in five editors is built in FOUR different
ways, and the difference between them is not the language, but WHAT exactly the program
touches: a live application, a project file, or a single parameter.

## Who has it and how

Four models, to which the whole field reduces:

1. **An interpreter inside the editor** (Vegas): the script is a command
   of the editor, living in its process and its menu.
2. **RPC to a live application** (Resolve, Premiere): an external process
   talks to the running program through an object model.
3. **Exchange through a format** (FCP): there is no API at all; automation is reading and
   writing an exchange file by third-party programs.
4. **Scripting inside a parameter** (AE): an expression lives in the property itself and
   is evaluated on every render frame, not "once per button press".

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Scripting (.NET) + Application Extensions | a script is a `.cs`/`.vb` source file in the `Script Menu` folder, compiled by .NET at launch; it appears in the Tools → Scripting menu, on the toolbar and on a hotkey through the same Customize dialogs as the native commands. An extension is a compiled DLL: it loads at startup, lives for the whole session, reacts to project changes, controls playback, and shows a modeless window. Command line: `/RUNSCRIPT`, `/SCRIPTARGS`, `/CMDMODULE` |
| Adobe Premiere | 26.x | UXP plugins (since 25.6), CEP panels + ExtendScript, C++ SDK | three parallel platforms. CEP — "classic extensions… automate in-app workflows with ECMAScript 3"; UXP — ES2015, asynchronous DOM, panels and commands, distribution through the Creative Cloud Marketplace (a `.ccx` package, publisher profile, DSA requirements for the EU). Edits go as a transaction: `project.lockedAccess` → `executeTransaction(compoundAction)` — a batch of actions is undone with one Ctrl+Z |
| DaVinci Resolve | 21 (checked against the 20.x README) | Scripting API | an external Python 3 / Lua 5.1 connects to `fusionscript.so/.dll`; "DaVinci Resolve needs to be running for a script to be invoked", but there is `-nogui`. Access is set in Preferences: console only / console and command line / **from the local network**. The Workspace → Scripts menu is built by scanning the folders at startup |
| Final Cut Pro | 12.3 | no API of its own; FCPXML + Workflow Extensions | exchange is an XML document (since 1.10 — a `.fcpxmld` bundle with `Info.fcpxml` inside); export offers only the choice "Current" or "Previous". A Workflow Extension is a separate macOS application, shown as a panel inside FCP; it still goes into the project through FCPXML |
| After Effects | 26.x | `.jsx` scripts + expressions | scripts are "run from a menu item or the command line" and draw their own dockable panels; an expression lives in a PROPERTY, is evaluated on every frame, and since 15.0 can read data from JSON footage (`sourceData`), since 15.1 — connect function libraries, and since 16.0 runs on a modern JS engine instead of ES3 (a switch in the project settings) |

## The trick behind it

**1. In Vegas a script is not "called through an API" — it BECOMES a command
of the editor, and that is why the ecosystem repairs the editor for the vendor.** The script lies
in `Script Menu` as source, is compiled at launch, and next to it a
16×16 PNG named `HelloWorld.cs.png` is placed — and a menu item with its own
icon appears, which is then attached to a panel button and a hotkey through the
same Customize Toolbar / Customize Keyboard dialogs as the native commands.
The user cannot tell their command apart from a built-in one. The reverse side of
the same coin: since a script is source code that is compiled on the user's side, the
editor's API becomes a PUBLIC CONTRACT. In 2026 the
`Stereo3DOutputMode` member disappeared from the object model, and the standard script shipped with Vegas, Batch Render Timer, stopped compiling right in the launch dialog, with C# compiler errors and line numbers. A ready-made fix to the `.cs`
was in the VERY FIRST message of the thread (02.04.2026), and the vendor closed the defect
only in build 143 — the mark "fixed in b143" appeared on 20.06.2026, two and a half months
later. The very same mechanism extends the life of the editor:
Happy Otter Scripts provides rendering through NVENC in Vegas 22 and later on ANY
version of the Nvidia driver, and a third-party TTS script connects speech synthesis
where the Vegas 2026 subscription gives "only" nine languages.

**2. The Resolve API is a closed whitelist, and it is about PAINTING, not editing.** You can build an edit from scratch: `CreateTimelineFromClips` and
`AppendToTimeline` accept a dictionary `{mediaPoolItem, startFrame, endFrame,
trackIndex, recordFrame}` — that is, place a clip at the needed second of the needed
track. But there is NOTHING to EDIT what has been assembled with: `TimelineItem` has `GetStart`,
`GetEnd`, `GetLeftOffset`, `GetRightOffset` — and not a single paired setter.
Neither a shift, nor a trim, nor a cut, nor a single method about transitions.
`SetProperty` accepts a fixed list of about thirty keys —
`Pan`, `Tilt`, `ZoomX`, `CropLeft`, `CompositeMode`, `Opacity`,
`RetimeProcess` — that is, the built-in transform and nothing else: a parameter of an arbitrary OFX/ResolveFX plugin is not addressable at all. But the API does have a channel
that is absent from the interface: each marker carries `customData`, and the documentation about it honestly says "not exposed via UI and is useful for
scripting developer to attach any user specific data". A script can carry
its own data in the project that is invisible to the user — but only as a string
and only on a marker.

**3. One goal, two opposite prices: a transaction in the process versus a bundle on disk.** Premiere came to the point where a script must change the project
as a TRANSACTION: `project.lockedAccess(() => project.executeTransaction(...))`,
and the whole batch of edits becomes ONE step of history. Before this it was worse than
"many steps": ExtendScript edits did not get into the History panel at all, that is,
there was nothing to undo the script's work with, and gathering the batch into one Ctrl+Z, in Adobe's own wording, "is impossible in CEP".
The price is the platform: CEP and UXP do not mix ("there is no way to have a
CEP/UXP hybrid"), every member of the API carries a MIN VERSION, and the object model itself is still catching up with ExtendScript. FCP paid the
opposite price: there is no process at all, there is a document, and Apple itself
writes about it — FCPXML "describes certain, but not all, aspects of
projects and events useful for other applications. But, it's not a
substitute for a native Final Cut Pro project". What did not make it into the file does not
exist for automation: whether a role is enabled (Video/Audio/Caption) is not written
into FCPXML at all — the 2023 request to Apple Feedback was still open as of
2026-09-07, and no third-party tool learns the state of a role. But FCP can be automated with anything and without a single line of
API — SwiftFCPXML, OpenFCPXMLKit, FCPBridge,
CommandPost have grown up around the format — and the vendor does not have to keep compatibility with any interpreter.

## Pros

- A script that has become an editor command (Vegas) does not require the user to
  know that it is a script: it sits on a hotkey and a button, and editing it is
  a text editor job, not a rebuild. Hence a live ecosystem:
  Excalibur, Vegasaur, Happy Otter Scripts, Montage Magic — and functions
  that the vendor does not have.
- An extension living in the process (a Vegas Application Extension, a Premiere panel)
  can do what an external process cannot do in principle:
  react to changes in project data, control playback, and
  keep its modeless window next to the timeline.
- A transaction (Premiere UXP) makes a batch of edits ONE undo step:
  the user rolls back the script's work with one Ctrl+Z, not thirty.
- An external process talking to a live application (Resolve) does not require
  closing the project: the script sees what a person sees, and edits before one's eyes.
  Plus `-nogui`: the same program becomes a render farm.
- The "through a format" model (FCP) requires nothing from the editor: neither an interpreter,
  nor an API, nor compatibility with it: the script's author chooses the language, and the vendor
  is responsible only for the file. There are usually MORE tools around such a format
  than around a closed API — they are written by those who need them.
- Expressions (AE) move automation inside a parameter: the composition becomes
  a template that reads JSON and rebuilds itself, without an external
  process, without launching anything by hand.

## Cons

- An interpreter inside the process = no sandbox. The Vegas help
  states this directly: "A script has the power to delete files, read
  files, write files, execute programs, access the Internet, access files
  on your network". Responsibility shifts to the user, who
  must read someone else's C# before running it.
- The object model becomes a public contract: remove a property —
  and you break other people's scripts and your own standard ones (Batch Render Timer, 2026). The item
  "return the function to the API, even as an empty stub" appears in the
  discussion by itself.
- The ecosystem is a dependency in both directions. Around Vegasaur on the forum
  there is a dispute about whether it is supported at all (the free version is closed, participants
  disagree about the paid one), and the question "does it work with the new
  version" becomes a condition for upgrading the editor itself.
- "The API does not cover everything" is not a temporary state but the norm. In Resolve
  one cannot shift or trim a clip that is already placed, and cannot touch a plugin
  parameter; in Premiere retiming is available NEITHER through ExtendScript NOR through
  UXP; text layers were never supported. Some operations remain
  manual only, and that is the only answer the user gets.
- Two platforms at the same time (CEP + UXP) is not a transition but a split: they
  do not mix, the port is done entirely, and until UXP reaches parity,
  the developer must choose whose half of the functions to lose.
- Automation "through a format" cannot do anything that is not in the file:
  selection, gestures, playback, the state of roles, the open project. And the
  complexity of the format becomes the complexity of the API — FCPXML's rational fractions
  eat the script author's time as much as the editing itself.
- Expressions (AE) are the cheapest entry and the most expensive debugging: the error
  lives in one property of one layer, and shows up as "the render is going wrong".

## What users say

> The UXP API has been in beta since 2024, but basic features still haven't
> been released. CEP is deprecated, but UXP still can't perform the most
> basic of video editing functions.
> — briantr, 05.03.2026, https://forums.creativeclouddeveloper.com/t/uxp-full-control-over-premiere-timeline-when/11768

> We will address new API feature requests, once we get closer to
> ExtendScript <--> UXP parity.
> — bbb_999 (Adobe), 24.07.2026, https://forums.creativeclouddeveloper.com/t/time-remapping-component-is-completely-inaccessible-via-scripting-extendscript-and-uxp-premierepro-api-in-premiere-pro-26-3-0/12026

> Channel mode must be changed manually in the Inspector panel or Fairlight
> page for each clip individually. Cannot be automated.
> — swayll on the Resolve Scripting API, 18.06.2026, https://github.com/samuelgursky/davinci-resolve-mcp/issues/73

> I batch render for nearly every job and it's currently a nightmare not
> being able to use this function that has worked for every Vegas version
> prior to 2026 for me.
> — sales1 about the broken standard Batch Render Timer script, 03.04.2026,
> https://forum.borisfx.com/t/batch-render-script-still-broken-fixed-in-b143/21682

> Oh, I almost forgot: Does Vegasaur work with 2026? I rely on many of
> these tools.
> — dgreenlaw in the thread "is VP 2026 ready for real work yet", 10.06.2026,
> https://forum.borisfx.com/t/is-vp-2026-ready-for-real-work-yet-serious-question/24021

> Sadly, Vegasaur is no longer being supported.
> — arpierre, 20.04.2026, https://forum.borisfx.com/t/your-top-vegas-plugins/22623

> FCPXML is insanely complicated and confusing. Because all its time units
> are in rational numbers, it also makes it hard to easily work things out
> without a spreadsheet, drawing or calculator.
> — FCP Cafe (a developer community around Final Cut Pro), 2026,
> https://fcp.cafe/developers/fcpxml/

## How it fits Kinotrain

### What already exists

**Half of the answer exists and works: our native format is the open
OpenTimelineIO, and the Python library `opentimelineio` is already
our scripting API.** This is not a metaphor: test 158
(`src/apps/kt-cli/tests/cross_render.rs:394`) works exactly this way —
Python reads OUR project file with the reference library, converts it
with the `mlt_xml` adapter and hands it to the foreign engine melt for rendering
(`src/scripts/otio-mlt-venv.sh`). A third-party program in a foreign language
already today opens a Kinotrain project, walks through tracks, clips, markers,
transitions and effects, and gets a meaningful result.

The supports of this path in the code:

- `src/libs/kt-otio/src/lib.rs` — `load_project` (line 78) and
  `save_project` (line 90); the format is described in `docs/file-format.md`, the subtleties
  are in `src/libs/kt-otio/CLAUDE.md`.
- **Write idempotence** (test 144): `write(parse(write(x))) ==
  write(x)` byte for byte. A script that opened and saved the file does not spoil
  it with formatting — the basic condition for `git diff` and for a chain of
  several scripts.
- **Passthrough "do not half-understand"** (tests 145–146): unknown fields of known schemas stay in place, objects of unknown schemas stay as raw
  JSON, **foreign `metadata` namespaces are not touched at all**. That is,
  a third-party script can put its own data in `metadata["myscript"]`
  of any object and it will survive opening and saving in the editor. This is our
  analogue of Resolve's `customData` — only not one string on a marker, but
  structured JSON on anything.
- **Conformance with the reference** — test 149
  (`src/libs/kt-otio/tests/otio_conformance.rs`): our file survives
  a round trip through the Python library, and a timeline ASSEMBLED by Python from scratch
  is read by us and re-saved. Gate by `python3 -c "import
  opentimelineio"`; in CI — `KT_REQUIRE_OTIO_CONFORMANCE=1`.
- **Our envelope is versioned by CONTENT**:
  `KT_META_VERSION = 4` (`src/libs/kt-otio/src/meta.rs:17`), and unknown kt fields
  are carried through by passthrough ("forward mercy", R.29).
- `src/apps/kt-cli/src/main.rs` — headless: `version | new | demo |
  convert | roundtrip | render | export`. Export is fully
  in-process, by default on the GPU (`GpuContext::new_headless()`,
  line 312), with the option to pin the encoder with a third argument for reproducibility. That is, we can already "run a project without the GUI" today.
- `src/libs/kt-model/src/model.rs` — `Command` (line 98, 45 variants),
  `apply` (line 363), **`apply_batch`** (line 370) and `apply_coalesced`
  (line 408). The transaction "a batch of edits = one undo step", which
  Adobe moved to from CEP to UXP, already exists in our model, and it is a ready-made door for
  any future automation.

### What is missing

- **kt-cli has not a single EDITING command.** `new` and `demo` create,
  `convert` converts, `roundtrip` rewrites, `render`/`export`
  render — and that is all. There is no `set-fx`, no `move`, no `split`, no selection
  "all clips of track 2", no CSV report. A batch operation today
  is done only by a third-party Python, that is, it requires installing the reference
  library.
- **There is not a single line of documentation for outsiders.** `docs/file-format.md`
  describes the format for US; a page "how to automate Kinotrain" with
  a ten-line example does not exist, and a person who wants to
  rename forty clips simply will not learn about this capability.
- **Export from kt-cli ignores the region** — a deliberate divergence from
  the GUI, a direct decision of the user in R.34 ("kt-cli and region — we do not do this",
  `docs/user-feedback.md`). For the scenario "render a piece for each
  marker" this is exactly the brick that is missing, and touching it without
  a new request is not allowed.
- **Working with an OPEN project is impossible, and this is not just "inconvenient".**
  `src/apps/kt-app` does not watch its file on disk, there is no reloading,
  no IPC/socket. Worse: the background autosave writes INTO THE PROJECT FILE ITSELF
  (`let target = self.project_path.clone().unwrap_or_else(autosave_path)`,
  `src/apps/kt-app/src/app.rs:1016`), that is, it silently overwrites a script's edit with its
  version of the model. The path through the format honestly works only
  with a closed project.
- **No reaction to user actions and no custom UI panels** — that is what
  Application Extensions exist for in Vegas, and panels in Premiere and FCP.

### What gets in the way

Not the volume of code, but four constraints.

**The first — invariant No. 11 (determinism).** Audio is byte for byte always, video
on the CPU — against a committed reference. A script executed INSIDE the render
(the AE expression model) makes the picture dependent on the interpreter version,
on the order of evaluation, and on everything the author of the expression brings there;
`GOLDEN_AUDIO`/`GOLDEN_VIDEO` stop being an anchor. A script OUTSIDE
that only rewrites `.otio` does not touch this invariant at all:
the engine's input is still an ordinary project file.

**The second — invariants No. 2 and No. 5.** FX state is touched only by
the audio-render thread, and exactly one MediaWorker owns the decoder. An API to the LIVE
project (the Resolve/Premiere model) would require a thread-safe door into the
model — today it is mutated only from the UI thread through
`apply`/`apply_batch` with undo snapshots. Premiere solved this same problem
with an explicit lock (`lockedAccess`) — that is, we would have to create our own
such thing, and this is no longer "add a command to the CLI" but a new discipline
of access to the model.

**The third — UI rules 20 and 17.** Rule 20: "nothing grows or switches off by itself; an edit changes exactly what the user reached for". A script BY DEFINITION does
what the user did not press — and this is legitimate exactly as long as it is started explicitly and its
result is undone in one step. Rule 17 (paired ways of one
action behave identically) gives a ready constructive requirement:
everything the script can do must be a `Command`, otherwise we get two truths about the
model — a gesture and a script diverging in details.

**The fourth constraint — the user's requirement, and it must be read precisely, not conveniently.** The verbatim wording in `docs/user-feedback.md` (section
"Cross-cutting requirements"): **"no own build of FFmpeg and no
plugins"**; the same phrase is repeated in `docs/architecture.md:73`. The line in
the table is called "System FFmpeg" and is tied to invariant No. 10,
so "plugins" there is, by context, FFmpeg plugins. This has already been analyzed in
F-EXT-001, and rewriting the conclusion from there would be dishonest: **there is NO literal ban on scripts in this phrase**, and closing the question with a quote would be fraud.

What works is not the letter but the reason named in F-EXT-001: the user asked for
one program that works on someone else's machine with what that machine has, and does not drag along someone else's binary baggage. This reason applies fully to a built-in interpreter — it is foreign code in our process, with its own lifecycle, its own
version and its own compatibility, and we are responsible for its crashes. For a script OUTSIDE that only reads and writes
JSON it does not apply at all: nothing is loaded into our process, and the presence of
Python on the machine is the script author's business. The code check confirms this state:
`libloading` appears in the workspace exactly once, in
`src/libs/kt-ffmpeg`, and not a single interpreter (`rhai`, `mlua`, `pyo3`,
`deno_core`, `boa_engine`, `wasmtime`) is in the manifests. That is,
the requirement does not forbid automation — together with the invariants it
CHOOSES for us which of the four models is ours. The related note about
OFX/VST hosts is F-EXT-001.

### Which invariants it touches

The "through the format + kt-cli" path touches **none** of the invariants: a file on
input and a file on output, the same `load_project`/`save_project`, the same
render. No. 11 is intact (determinism is computed from the file), No. 2 and No. 5 are not involved (there is no second thread in the engine), No. 9 and No. 10 are not involved (no foreign code
in the process). The only thing that has to be observed when adding editing commands
to kt-cli is UI rule 17: a CLI command must be the same
`Command` as the gesture, not a second implementation.

The "built-in interpreter" path touches No. 11 (render determinism), No. 2 and
No. 5 (access to the model and FX state from a foreign thread), No. 3 (an edit of the model
does not destroy FX state — a script can easily do what no gesture does),
plus UI rules 17 and 20.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2 stages: editing and selection commands in kt-cli + a documentation page with Python examples. The "built-in interpreter" option is XL and a separate note |
| Where the main work is | kt-cli (argument parsing, selection of objects, report output), docs (an "automation" page); kt-otio and kt-model are not touched — all the mechanics already exist |
| Project format | does not touch it: the editing commands go through the same `Command` and the same writer, the envelope version does not move |
| Tests that will have to be written | cli tests for each editing command (result == the same `Command` from the GUI); extending test 149 with the scenario "Python edits our file — we open it without notices"; test 144 (idempotence) remains the reference; render golden tests are not touched |
| Depends on | nothing. The format, conformance with the reference library and headless rendering are already in place |
| Breaks compatibility | no |

## Verdict

**Our path is the second one: document and strengthen automation via
`.otio` + kt-cli, and do not introduce a built-in interpreter.** The comparison
is direct. An interpreter inside the process gives what the format cannot express
(reaction to actions, own panels, working with an open project), but it pays with
invariants 11, 2, 5 and 3, a public contract of the object model for years ahead
(Vegas's own standard script broke from a single removed property) and the reason
F-EXT-001 was rejected: foreign code in our
process, whose crashes we are responsible for. The format path is already half-traveled and does not cost
A SINGLE invariant: the file is
open and standard, writing is idempotent, foreign metadata passes
through, conformance with the reference Python library is closed by a test, and
rendering without a GUI works. Compared with FCP, whose XML is merely an exchange
FILE and where Apple honestly writes that it does not replace the native project, for us OTIO
IS the native project: there is nothing to lose on a round trip, and no second format
exists.

Exactly two things need finishing, and both are cheap. The first: editing and selection
commands in kt-cli — today it can only create, convert and
render, and "change a parameter of all clips of a track" has to be done by a
third-party Python. The second: a documentation page with working examples —
without it the capability exists, but nobody knows about it. Separately, it is worth recording the honest limitation: while the project is OPEN in the editor,
an external edit of the file is unsafe (the background autosave will overwrite it), and this
has to be written in plain words, not left for the user to find out.

## Sources

- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/11-autoscript/scripting_vegas.htm — the official VEGAS Pro 2026 help, the Using Scripting section (languages, folders, script icons, `/RUNSCRIPT`, `/SCRIPTARGS`, `/CMDMODULE`, the security warning, extensions). The page is marked "Last modified on April 15, 2026" (accessed 2026-09-07)
- https://forum.borisfx.com/t/batch-render-script-still-broken-fixed-in-b143/21682 (2026-09-07)
- https://forum.borisfx.com/t/your-top-vegas-plugins/22623 (2026-09-07)
- https://forum.borisfx.com/t/is-vp-2026-ready-for-real-work-yet-serious-question/24021 (2026-09-07)
- https://forum.borisfx.com/t/years-of-scripting-tutorials/21295 (2026-09-07)
- https://forum.borisfx.com/t/please-create-a-page-with-sdk-examples-ofx-scripts-extensions-for-vegas/22464 (2026-09-07)
- https://forum.borisfx.com/t/tts-vegas-script-supporting-narakeet-900-voice-languages/22854 (2026-09-07)
- https://www.jetdv.com/ — Excalibur and Montage Magic, a catalog of training materials on Vegas scripts (2026-09-07)
- https://developer.adobe.com/premiere-pro/ — the three extension paths of Premiere; the phrases "UXP is the next generation of APIs, for Premiere v25.6 and beyond" and "Automate in-app workflows with ECMAScript 3" about CEP (2026-09-07)
- https://developer.adobe.com/premiere-pro/uxp/introduction/ — UXP as an ES2015 platform (2026-09-07)
- https://developer.adobe.com/premiere-pro/uxp/ppro-reference/ — synchronous ExtendScript versus asynchronous UXP, the MIN VERSION field (2026-09-07)
- https://developer.adobe.com/premiere-pro/uxp/changelog/ — Premiere 26.5: additions, deprecations and fixes in the API itself (2026-09-07)
- https://developer.adobe.com/premiere-pro/uxp/resources/faq/ — `.ccx` packaging, Marketplace, DSA requirements for the EU (2026-09-07)
- https://forums.creativeclouddeveloper.com/t/uxp-full-control-over-premiere-timeline-when/11768 (2026-09-07)
- https://forums.creativeclouddeveloper.com/t/time-remapping-component-is-completely-inaccessible-via-scripting-extendscript-and-uxp-premierepro-api-in-premiere-pro-26-3-0/12026 (2026-09-07)
- https://forums.creativeclouddeveloper.com/t/uxp-vs-cep/9643 — "there is no way to have a CEP/UXP hybrid" (2026-09-07)
- https://forums.creativeclouddeveloper.com/t/ppro-beta-uxp-operation-history/9022 — `lockedAccess` + `executeTransaction(compoundAction)` (2026-09-07)
- https://github.com/thesleepingsage/Davinci-Resolve-Scripting-API — a mirror of the STANDARD `README.txt` of the Resolve Scripting API, marked "Last Updated: 7 May 2025". All statements about Resolve (the `SetProperty` whitelist, the absence of setters on `TimelineItem`, `customData`, `-nogui`, permissions in Preferences, `EXPORT_OTIO`) are taken from there (2026-09-07)
- https://github.com/samuelgursky/davinci-resolve-mcp/issues/73 (2026-09-07)
- https://developer.apple.com/documentation/professional-video-applications/fcpxml-reference — "It describes certain, but not all, aspects of projects and events…" (2026-09-07)
- https://developer.apple.com/documentation/professional-video-applications/importing-fcpxml-data — the `.fcpxmld` bundle, the choice "Current"/"Previous" on export (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/use-xml-to-transfer-projects-verdbd66ae/mac (2026-09-07)
- https://fcp.cafe/developers/fcpxml/ (2026-09-07)
- https://github.com/CommandPost/FCPCafe/issues/303 — the state of roles is not written into FCPXML (request of 11.12.2023, still open as of 2026-09-07)
- https://developer.adobe.com/after-effects/ — "Automate After Effects using scripts that run from a menu item or the command line" (2026-09-07)
- https://ae-expressions.docsforadobe.dev/introduction/changelog/ — the history of the expression engine: JSON data in 15.0, function libraries in 15.1, a new JS engine in 16.0 (community documentation, not the primary source; 2026-09-07)

None of the five editors was run live. The Vegas, Premiere,
FCP and AE rows rely on the official help/developer documentation plus
the vendors' forums. The Resolve row relies on the standard `README.txt` of the
Scripting API in a third-party mirror — the original ships together with the
application; the document's version is from May 2025, so **[NOT VERIFIED]**
whether the missing `TimelineItem` setters appeared in Resolve 21. For the same
reason the `editors:` field lists both keys — `resolve-21` (the current line) and
`resolve-20` (what was actually checked against). Ordinary web search was
unavailable in this session (the budget was exhausted, search engines returned CAPTCHAs), so the sources were gathered by direct
requests to the documentation sites, to the Boris FX and Adobe Discourse forums (`search.json`), and to the GitHub API.
