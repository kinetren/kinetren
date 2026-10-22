---
id: F-EXP-002
title: Render queue and background export
status: candidate
priority: B
cost: M
category: export/pipeline
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-app, kt-engine, kt-gpu, kt-model, kt-cli]
invariants: [5, 7, 9, 12]
ui_rules: [2, 12, 13, 16, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-EXP-002 — Render queue and background export

## Essence

The editor wants to set five jobs and walk away: this region in 1080p for
the client, the same piece in 720p for a messenger, the whole film in master quality,
three marked scenes as separate files. Today for us this is five trips through the
settings window with waiting at the screen between them. The second half of the requirement is
that during rendering one can keep editing, rather than sitting in front of a
progress bar.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 b66 | `Batch Render` | NOT a function, but a C# script from the kit (`Batch Render.cs` in `Script Menu`) plus a DLL extension. It renders the project / selection / **each region as a separate file**, at once across several render templates; the output file name is built from the region's name. It lives outside the product and breaks together with the API: among the known bugs of build 66 — "The Batch Render script is not functional", fixed by the community (`Batch Render 2.0`, April 2026). There is no queue of its own in the editor itself |
| Adobe Premiere | 26.x | `Export` versus sending to **Adobe Media Encoder** | two buttons in the export mode: rendering inside Premiere (the editor is busy) and placing into the queue of a SEPARATE application. AME is an independent process with its own queue, presets and **watch folders**; while it encodes, editing continues. The classic price is desynchronization: AME works with not what is now on the screen [NOT VERIFIED: the exact mechanics of the snapshot — `helpx.adobe.com` and reddit do not open from this environment] |
| DaVinci Resolve | 21 | `Deliver` → **Render Queue** | a queue INSIDE the editor: you configure the format → `Add to Render Queue` → jobs accumulate in the list (different timelines, ranges, formats) → `Start Render`. Officially: "a render queue for exporting multiple jobs". With a separate switch `Single clip` / `Individual clips`, one job cuts the timeline into files per clip. There is remote rendering to another machine (Studio) — the only standard way to "render and edit at the same time" |
| Final Cut Pro | 12.3 | no queue; `Batch sharing` + `Compressor` | export is a background task (`Background Tasks`, ⌘9) on a par with import and rendering. `Batch sharing`: "You can export multiple projects, clips, or clip ranges at once", but "You can't mix projects and clips in a batch share". The link with `destination bundles` gives several formats "all in a single step". The real queue is moved to a separate application **Compressor** (batch, chained jobs, droplets, watch folders) |
| After Effects | 26.x | `Render Queue` (panel) | a queue inside the application; while it runs, AE is busy. The key difference from all others: ONE queue item can have several `Output Module`s, "An OutputModule object of a RenderQueueItem generates a single file or sequence via a render operation" — that is, N files in one render operation. The alternative is `Add to Adobe Media Encoder Queue` (the same separate process as Premiere's) |

## The trick behind it

**"Background export" in Final Cut is work IN IDLE TIME, not in parallel.**
Apple's Sharing honestly lies in the list of background tasks together with import,
transcoding and analysis, and the price is written there too: "If you actively use Final
Cut Pro while background tasks are running, the background tasks are
paused". That is, the button "export in the background" does not let you edit and
render at the same time — it lets you render WHILE YOU DON'T TOUCH
the program. The consequence is non-obvious and the reverse of what one expects: the more actively
you use the editor, the longer the "background" export takes, and "set it and
walk away" works in FCP literally — you have to walk away. That is exactly why Apple keeps Compressor
alongside: real parallelism is moved to another
process rather than done inside.

**In After Effects the saving is not in the queue but in the queue item.**
A queue of N jobs is N full passes: decode, composite, encoding for each. One item with N `Output Module`s is ONE pass of the composite and N
files at the output, and the API states this directly: an output module "generates a
single file or sequence via a render operation". The practical difference
is visible on "master + proxy + preview for the client": in Resolve and AME these are three
jobs and triple the cost of decoding, in AE — one. The reverse side of the same
detail: all outputs must have a common render, therefore a different resolution
or a different frame region again falls apart into separate jobs — the saving works exactly where
only the codec and container differ.

**In VEGAS the queue is moved into a script, and both consequences of this are visible
in the tracker.** The first: it regularly dies on a version change — among the known
bugs of VEGAS Pro 2026 build 66 is "The Batch Render script is not
functional", and a working build users get from each other, not from the publisher. The second, subtler: the job there is a REGION, and the output file name
is taken from the region's name, therefore batch rendering stumbles over things that a product-level
function would not stumble over: "not all characters that
are legal in region names are legal in Windows file names" — colons, quotes and slashes in scene names bring down the whole run.

## Pros

- A job = an already existing markup (region, marker-range, clip), not a separate entity: "cut the series into 12 scenes" is one run by
  markers, not 12 manual placements of boundaries and 12 save dialogs.
- A queue INSIDE the editor sees the live model, so it physically cannot have a desync "rendered not what is on the screen"; it pays
  for this with the editor being busy (Resolve, AE).
- A queue in a SEPARATE process frees the editor completely and survives its crash; it pays
  by rendering a snapshot/saved file, and any edit after queueing does not reach the output.
- Several outputs per item (AE) remove the N-fold decode and composite
  where the formats differ only in packaging.
- A watch folder is a queue without an interface: "designate any folder … that
  automatically transcodes any media files you add to it". Automation goes to
  those who have neither a scripting API nor the wish to study it.

## Cons

- "Background" without an explicit priority rule turns either into a sluggish
  editor or into a standing queue: Apple chose the second and wrote it in the help, the others write nothing, and the user learns the rule by experience.
- A separate process requires a project snapshot, and a snapshot requires discipline
  in saving; hence the eternal class of complaints "the wrong thing was exported" with the
  Premiere + AME bundle.
- A queue turns export settings into a mandatory preset: without
  saved sets, "five jobs in five formats" means five passes through the whole settings form.
- A job named after the markup (region/clip) inherits its problems — illegal characters in the file name (VEGAS) or the impossibility of renaming the output at all (Resolve `Individual clips` takes the source name
  [NOT VERIFIED: a detail from the search index, the primary source did not open]).
- An implementation "on the side" (a script, an extension) does not survive a product version:
  in the current VEGAS the standard batch render simply does not work.

## What users say

> This was reported with a simple edit to fix it by @Jetdv on the old forum
> but Vegas2026 is still being distributed with it unfixed.
> — Howard Vigorita, "Batch Render script still broken (fixed in b143)",
> 02.04.2026, https://forum.borisfx.com/t/batch-render-script-still-broken-fixed-in-b143/21682

The official release notes of VEGAS Pro 2026 build 66 (27.03.2026,
https://forum.borisfx.com/t/release-vegas-pro-2026-build-66/21161) keep
this in the list of known bugs, verbatim:

> The Batch Render script is not functional

About the model of "a separate application" — the wording of a practitioner:

> It queues exports in the background while you continue editing or
> browsing, effectively decoupling creative work from render time.
> — Michele Smith, "Efficient Workflows: Exporting Video Content for Multiple
> Social Media Platforms Simultaneously", 05.11.2025,
> https://larryjordan.com/articles/efficient-workflows-exporting-video-content-for-multiple-social-media-platforms-simultaneously/

Negative feedback specifically about "AME rendered the old version" could not be found
live: `reddit.com`, `helpx.adobe.com` and the search of
`community.adobe.com` are closed from this environment (the search index for this query returns threads from r/premiere and community.adobe.com, but the pages themselves do not
open). The class of complaints exists; the exact mechanics of the snapshot —
not confirmed.

## How it fits Kinotrain

### What is already there

Background export WORKS for us ALREADY, and works exactly the way one wants to get it from a queue:

- `KtApp::launch_render` in `src/apps/kt-app/src/render_dialog.rs` does
  `let project = self.model.project().clone()` and hands the clone to a separate
  thread `"render"`. This is already a SNAPSHOT: timeline edits after the start do not affect the running render.
- The export sources are opened separately (`open_all` there, a comment in the file header: "with FRESH FfmpegSource (not the player's decoders)"), so the render does not contend with playback for the decoder — invariant No. 5 is not touched by construction.
- The progress window `draw_render_dialog` is an ordinary `egui::Window`, not modal. Editing during rendering is already physically possible.
- The GPU executor of the export is its own headless device (`export_video_gpu()` in
  `src/apps/kt-app/src/app.rs`, `GpuContext::new_headless` in
  `src/libs/kt-gpu/src/context.rs`); the UI queue does not share it with export
  (item 185 of `docs/porting-notes.md`).
- `RenderJob` (`src/apps/kt-app/src/render_dialog.rs`) already carries everything of which a queue row consists:
  the progress channel, `cancel`, `pause`, `ExportOptions`, the output path, the list of stubs, speed statistics.
- The output name is already built from the user's template
  (`render_file_name`/`sanitize_name_part`, test t201): characters illegal in a file name
  are replaced, and the region's name is substituted only if the marker-range
  MATCHES it (`default_render_name`). This is exactly the task on which VEGAS Batch Render stumbles, and for us it is already solved.
- Candidates for jobs are already described in the model: `Marker` with `duration` and
  `is_range()`, `MarkedArea` (the name = the address, a single namespace shared with
  markers) — `src/libs/kt-model/src/project.rs`.
- A project snapshot is cheap, and this is proven: undo — snapshots of the whole `Project`
  (`push_undo`, `MAX_UNDO = 80` in `src/libs/kt-model/src/model.rs`), that is,
  in ordinary work we clone the project eighty times in a row.
- A precedent for background work with the project under a guard — `write_project_guarded`
  in `src/apps/kt-app/src/app.rs` (item 199): the background autosave compares
  the revision and cancels itself if a newer one is already on disk.
- Headless rendering exists separately: `kt-cli export <in.otio> <out.mp4>
  [encoder]` in `src/apps/kt-cli/src/main.rs`.

### What is missing

- A list of jobs and their states (waiting / running / done / failed / cancelled)
  and a decision on whether it survives exiting the program.
- Settings per job. `ExportOptions` exists, but there are no presets — a queue of
  five different formats means five passes through the settings form.
- A path per job: `rfd::FileDialog` opens synchronously inside
  `launch_render`, that is, the path is asked for at exactly the moment of starting.
- A source of jobs: "regions → jobs", "marker-ranges → jobs",
  "named areas → jobs" have no code.
- Permission to put a second job in: `start_render` answers
  `render.already_running` and exits.
- Completeness of kt-cli: `export` hard-codes `ExportOptions::default()` and
  `range = None` — no range, no resolution, no CRF, no bitrate, no
  regions. All of this exists in the engine, but is not exposed to the CLI.

### What gets in the way

1. **The model "a separate application" is closed to us by invariant No. 9.** Export
   is fully in-process: libavformat/libavcodec of the system FFmpeg
   through dlopen (`src/libs/kt-ffmpeg/`), no temporary files, no external
   `ffmpeg`, no mkfifo — and `export_mp4_with_backends`
   (`src/libs/kt-engine/src/export.rs`) keeps the muxer in its own process.
   We have no second encoder binary, and under this invariant there will not be one.
   This must be said directly: **the queue for us can only be internal**.
   At the same time the main pain of the AME model — "a not-the-right snapshot is rendered" — is absent for us by construction: the snapshot lies in memory, and there is no need to save the project in order to render.
2. **Invariant No. 7: a started export completes on its own backend.** Therefore the backend is chosen at the START of the job, not at queueing, and a GPU error in the middle of a job is an error of THIS job, not a reason to quietly finish it on the CPU. Incidentally: at present the headless device is brought up for each job; for a queue run it is more sensible to have one per run — but that is
   precisely a decision of the round, not a question of the note.
3. **Invariant No. 12 and rule 12 of the checklist — here we already have a defect.**
   In `src/apps/kt-app/src/app.rs` the `animating` flag includes
   `self.render_job.is_some()` unconditionally, and `render_job` lives until the user presses OK in the summary window. That is, after the render has FINISHED the interface keeps redrawing 60 times per second — exactly
   what the complaint of R.31 was about: "sometimes when I call the render window the fps
   also drops and does not recover until I close it"
   (`docs/user-feedback.md`). For a thirty-second export this is a trifle;
   for a queue of an hour it is that same "UI heats the system". The queue must redraw on the arrival of progress (which is sent anyway every 10 frames and once a second for audio), not unconditionally.
4. **Rules 16 and 20.** A failed job must remain in the list and be shown as an ERROR, not skipped: today `ExportError` lives in
   one window and dies together with it, and `cleanup_on_err` in
   `src/libs/kt-engine/src/export.rs` also deletes the partial file —
   the correct behaviour for a single render, but the queue must say
   exactly what and why did not work, and must not "fix up" the other jobs on its own.
5. **The price of a naive queue in number of passes.** `export_mp4_with_backends`
   accepts TWO independent sets of sources (`open_all` is called twice
   in `launch_render`) and renders first all the video, then all the sound into one muxer. So one job is already two full passes over the material, and a queue of N formats is 2N. "Several outputs from one pass" (what AE does with `Output Module`) is a change of `export_mp4_*` to several muxers on one stream of frames, that is, work in kt-engine, not in the UI. A separate task with a separate price.
6. **The fork "whose snapshot".** The clone is made at the START of the job. For a queue this means that a job queued now and started an hour later will render the model as of an hour later. This is exactly the fork because of which Premiere has two buttons, and it must be decided explicitly: a snapshot at QUEUEING is more honest and matches the meaning of "set it and walk away", and it costs one
   `Project::clone()` per job.

### Which invariants it touches

No. 9 (the model of a separate application is closed — the queue is internal only),
No. 7 (the backend is fixed at the start of the job, a GPU error = an error of the job),
No. 12 (redrawing only on motion — now violated by the render window),
No. 5 (not violated: export opens its own sources, but this is precisely the reason
why editing and rendering at the same time are possible at all).
Rules of `src/apps/kt-app/CLAUDE.md`: 2 (works during
playback), 12 (background work does not get in the way), 13 (do not waste
space — the job list does not get a separate panel), 16 (a failed
job is an error, not a faded hint), 20 (the queue does not fix anything on the user's behalf).

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 stages of the round |
| Where the main work is | kt-app: `Vec<RenderJob>` instead of `Option`, job states, a source of jobs from regions/markers/areas, honest redrawing. kt-engine — only to extract "one run" and deliver an error to a specific job |
| Project format | not touched if the queue lives in the session; a persistent queue is `metadata.kt` and a new schema version in kt-otio |
| Tests that will have to be written | a harness test of the queue window (order, cancelling one job, a failed job visible and remaining in the list); unit "regions/marker-ranges/areas → jobs" + names by template t201; a test "during the queue the UI is not redrawn unconditionally" — a pair to t38/t20-repaint; kt-engine: two jobs in a row on ONE headless device |
| Depends on | export presets (the note of the category `export/` is not yet written): without them a queue of heterogeneous formats loses its meaning |
| Breaks compatibility | no |

## Verdict

We take it, but in a lightened form: the queue is internal (otherwise — invariant No. 9),
jobs come from the ALREADY existing markup (region, marker-range, named
area), and the project snapshot is taken at the time the job is queued. The expensive part — "several
outputs from one pass" on the model of `Output Module` — we do not drag into this note: it is a change in kt-engine, not in the queue, and it needs its own note.

**An explicit recommendation on "hand this to kt-cli": no, we make the queue in the GUI.**
`kt-cli export` does indeed cover "render a batch of PROJECTS" in one
loop of the shell, but a typical queue job is "five pieces of ONE project in
different formats", and the CLI today can only do the whole project at once with
`ExportOptions::default()`: no `--range`, no resolution, no CRF, no
regions (`src/apps/kt-cli/src/main.rs`). For the queue to live in a script, one would have to either bring the CLI to the full set of options and teach it
the markup, or generate a temporary `.otio` per job — that is, write
the whole logic "markup → job → parameters" anyway, only with output to the terminal. Editing goes on in the GUI, the markup lives in the GUI, the model snapshot is available only in the GUI — the queue belongs there too. The options `--range`,
`--crf`, `--resolution` in the CLI are useful to expose as a side effect: they already exist in
`ExportOptions`, cost a dozen lines and give a reproducible headless run for tests.

## Sources

- https://forum.borisfx.com/t/release-vegas-pro-2026-build-66/21161 —
  release notes of VEGAS Pro 2026 build 66, the known bug "The Batch
  Render script is not functional" (accessed 2026-09-07)
- https://forum.borisfx.com/t/batch-render-script-still-broken-fixed-in-b143/21682
  — a thread about the broken script, quote (2026-09-07)
- https://forum.borisfx.com/t/batch-render-2-0-now-compatible-with-vegas-pro-2026/22856
  — the community fix of the script, the composition of the installation: a C# script in `Script Menu` +
  a DLL in `Application Extensions` (2026-09-07)
- https://forum.borisfx.com/t/batch-render-with-unicode/25418 — region names as
  output file names and illegal characters (2026-09-07)
- https://www.blackmagicdesign.com/products/davinciresolve/deliver —
  "a render queue for exporting multiple jobs" (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/view-background-tasks-ver64e71609/mac
  — the composition of background tasks and the rule "background tasks are paused"
  (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/batch-sharing-verd4e0cf686/mac
  — batch sharing and destination bundles (2026-09-07)
- https://support.apple.com/guide/compressor/create-and-use-watch-folders-cpsra6b76395/mac
  — the definition of a watch folder (2026-09-07)
- https://ae-scripting.docsforadobe.dev/renderqueue/outputmodule/ and
  https://ae-scripting.docsforadobe.dev/renderqueue/renderqueueitem/ —
  `OutputModule` / `numOutputModules`. This is a community mirror of the After Effects scripting guide, not an Adobe site; taken because
  `helpx.adobe.com` is unavailable (2026-09-07)
- https://larryjordan.com/articles/efficient-workflows-exporting-video-content-for-multiple-social-media-platforms-simultaneously/
  — a quote about the background queue of AME (2026-09-07)
- The Premiere and After Effects rows rely on secondary sources:
  `helpx.adobe.com`, `www.adobe.com`, `reddit.com` and the search of
  `community.adobe.com` do not open from this environment (checked
  2026-09-07, "proxy refused"/403). The mechanics of the project snapshot when sent to Media Encoder and the presence of the `Individual clips` switch in Resolve
  are confirmed only by search snippets — marked [NOT VERIFIED].
- Code: `src/apps/kt-app/src/render_dialog.rs`,
  `src/apps/kt-app/src/app.rs`, `src/libs/kt-engine/src/export.rs`,
  `src/libs/kt-gpu/src/context.rs`, `src/libs/kt-model/src/model.rs`,
  `src/libs/kt-model/src/project.rs`, `src/apps/kt-cli/src/main.rs`;
  `docs/user-feedback.md` (R.31, the complaint about FPS during the render window),
  `docs/porting-notes.md` items 185, 199.
