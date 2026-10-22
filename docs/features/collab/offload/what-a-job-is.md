---
id: F-COL-038
title: What a job consists of
status: candidate
priority: B
cost: S
category: collab/offload
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, flamenco-3]
we_have: partial
crates: [kt-engine, kt-cli, kt-app, kt-model]
invariants: [7, 9, 11]
ui_rules: []
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [resolve-project-server, bm-cloud]
checked: 2026-09-08
---

# F-COL-038 — What a job consists of

## Essence

The editor wants to say "compute this," walk away, and get a file. "This" is not the whole project: it is a chunk, a format, and a place to put the result. While its own machine does the computing, the shape of such a request is useful to no one: it fits in a settings window and dies together with it. As soon as someone else takes over the computing, whether a neighboring computer or the same program tomorrow morning, the shape of the request becomes the MAIN thing, because it is the only thing that moves. Industries outside video editing solved this question long ago, and solved it the same way: the job is data, not the state of a window.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 build 66 | no "job" object | rendering is a modal operation over the whole project or over a selection; batch runs live in a C# script outside the product, and the job there is a REGION, with the output file name assembled from the region name (F-EXP-002). The program has no job as an entity |
| Adobe Premiere | 26.x | sending to Adobe Media Encoder | the job lives in ANOTHER process and survives closing the editor; its shape is a snapshot of the sequence plus an output preset. Precisely because the shape has separated from the live model, the pairing has a class of "rendered the wrong thing" |
| DaVinci Resolve | 21 | `Deliver` → Render Queue; Remote Rendering (Studio) | the job is a row in the queue: timeline, range, format, path. Remote Rendering sends a READY row to another machine whole and does not divide it: one row, one machine, with no distribution of load inside the job |
| Final Cut Pro | 12.3 | `Batch sharing`, then Compressor | there is no job inside FCP: there is a background task, which even pauses while the person works. The real job object is moved out to Compressor, where it is already savable (droplet, watch folder) |
| After Effects | 26.x | Render Queue item + Output Module | a queue item carries in/out **by frame** and several outputs from one pass of the composite. The item has no requirements on the machine: their role is played by the machine on which `aerender` was launched |

**Outside the core.** Without farms this feature cannot be described: for the five core editors the job is either a dialog row or a missing entity, and one can see what it actually CONSISTS of only where it is sent to someone else's machine.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Flamenco | 3.x | job → tasks | manager and worker on one network, no cloud at all; the job is not "render the project" but a triple of "command + range of FRAMES + machine requirements" (`docs/features/_editors.md`) |
| OpenCue | 1.x (docs.opencue.io, 2026) | job → layer → frame; service | a job is a set of layers; a layer is a frame range plus a command; a frame is ONE command. Machine requirements sit in a separate object `service` (threads, memory, GPU, tags), and a layer's tag is compared with the host's tag BEFORE sending |

## The trick behind it

**The atom of a farm is not a frame but a COMMAND, and this is visible in the vocabulary, not in the marketing.** The OpenCue glossary defines the three levels literally like this: "A job is a collection of _layers_, which is sent as a _script_ to the queue to be processed on remote _cores_"; "The sub-jobs in an _outline script_ job. Each layer contains a frame range and a command to execute"; "An individual command that's contained in a _layer_." So "frame" in the farm's terminology is a unit of WORK, not a unit of picture, and precisely for this reason the same machine counts both a render and a preprocess equally well: the shape of the job knows nothing about pixels.

**The machine requirement is a FIELD of the job, checked before sending, not an error at start.** In OpenCue it is moved into a separate entity: "A service is a collection of resource requirements and tags that are associated with a _layer_. The service contains minimum and maximum thread requirements, minimum memory and gpu requirements, whether the jobs are threadable or not, and a list of tags," and a tag works as a filter: "Frames contained in a layer with a tag will only render on a host that also shares that tag." The difference from an editor is fundamental and invisible from the interface side: in an editor, "needs a GPU" is discovered when the job has already been started; in a farm, it is discovered when there is nowhere to send the job.

**Chunks are NOT independent, and the farm knows this, but it records the knowledge by hand instead of computing it.** The practice when cutting a composition into segments is formulated like this: "Render with 5 frames overlap or so. Some temporal effects and expressions produce different results without some run-up" (Mylenium, 17.07.2024). That is, the frame range in a job is in fact not an interval but an interval plus a run-up, and the size of the run-up is not part of the job's shape: a person guesses it with the word "or so." Precisely this size is computed by us from the project: `chain_tail(fx, auto).to_audio_frames().max(MIN_PREROLL_FRAMES)` (`src/libs/kt-fx/src/lib.rs:607`, minimum 100 ms at `src/libs/kt-fx/src/lib.rs:24`), and therefore the export of our sub-range runs up on its own, and "5 frames or so" is not needed for it.

## Pros

- A job as DATA survives the process: it can be queued, repeated after a crash, sent a second time in another format, and shown in the log. All of this is a consequence of one decision: not to keep the request in the state of a window.
- A range by FRAMES, not by time, removes a whole class of arithmetic disputes: chunks [100, 200) and [200, 300) can have neither a hole nor an overlap, because the boundary is an integer, not the result of a division.
- Machine requirements declared as a field give cheap dispatching: a job is not sent where it will certainly fail to compute, and the person learns this before waiting, not after.
- One shape serves both render and preprocess: "command + range + requirements" describes equally well "compute frames 100–200" and "build peaks for this file" (F-COL-042).
- Separating "range" from "chunk" (how many units to hand one machine at a time) allows changing granularity without touching either the job or the executor.

## Cons

- A data shape must be versionable: as soon as a job survives the process, it also survives a program update, and "a field that did not exist in the previous version" becomes a compatibility question.
- A range in frames requires that frames be defined IDENTICALLY for the requester and the executor: a different fps grid, a different rounding of the boundary, or a different material timecode turns "frames 100–200" into different chunks.
- The run-up cannot be placed in the job's shape, because it is a property of the CONTENT, not of the range. Hence either overlap "by eye" or a mismatch at the seam.
- Separating the shape from the live model raises the question "whose snapshot": a job queued an hour ago describes an edit from an hour ago, and this is exactly the fork that gives Premiere two export buttons.
- Machine requirements written explicitly go stale along with the machine park: a `gpu` tag from the year before last does not mean the same thing as it does today.

## What users say

The practice of cutting a long render into chunks, and the price paid at the seam, named directly:

> Render with 5 frames overlap or so. Some temporal effects and expressions
> produce different results without some run-up.
> — Mylenium, "Workflow advice needed: rendering 4K 50fps project in
> segments," Adobe Community, 17.07.2024,
> https://community.adobe.com/t5/after-effects-discussions/workflow-advice-needed-rendering-4k-50fps-project-in-segments/m-p/14746398

In the same thread, why in the end they still move away from a video file toward a frame sequence, that is, toward "one unit of work = one file":

> I might suggest rendering an image sequence - you can start/stop literally
> anywhere, and AE will know which frames already exist.
> — Kyle Hamrick, same thread, 18.07.2024

The note has no counting part: `trackers:` is empty deliberately. Public trackers do not exist for the five core editors (`docs/features/_trackers.md`, "Whom we do NOT take"), and in the exported trackers of open source the topic of collaborative work is represented by one real request out of 25,442 tasks. Filling in the field for the sake of the form would mean fabricating a signal (rule 12).

## How it fits Kinotrain

### What already exists

- `src/libs/kt-engine/src/export.rs:396` — `range: Option<(Tick, Tick)>` in `export_mp4_with_backends`: the engine can compute a chunk, and this is not a stub — the same parameter passes through `render_raw_video_range_with` (`src/libs/kt-engine/src/export.rs:186`) and into `render_raw_audio_range` (`:93`).
- `src/libs/kt-engine/src/export.rs:298` — `gpu_required_clips` and `ExportError::GpuRequired` (`:293`): **the machine requirement is already computed from the project**, and moreover from `effective_clips`, that is, taking morpher batches into account. This is exactly what OpenCue keeps in `service`.
- `src/libs/kt-engine/src/export.rs:203` — `video.grid.align_floor(a)`: the start of the range is aligned to the fps grid, which means we have the notion of "frame" and it is shared with the preview (invariant 7).
- `src/libs/kt-fx/src/lib.rs:607` — the run-up is computed from the chain, not assigned: `chain_tail(fx, auto).to_audio_frames().max(MIN_PREROLL_FRAMES)`. Five frames "or so" are not needed by us: the value is known exactly.
- `src/apps/kt-cli/src/main.rs` — the headless executor exists and can pin the encoder via a third argument (this is already half of "machine requirements," introduced for the sake of the reproducibility of test 158).
- The output name is already assembled from a template and sanitized (`render_file_name`, test t201) — the very task on which VEGAS batch rendering trips.

### What is missing

- **A job shape as data.** `ExportOptions` (`src/libs/kt-engine/src/export.rs:337`) is an ordinary structure with `#[derive(Debug, Clone)]`; across all of `kt-engine/src` there are zero matches for `serde`. There is nowhere to record a request: it lives in the arguments of a single call.
- **A range in FRAMES.** `range` is set in ticks, and that is time, not frame numbers. Then the inconsistency begins: video takes `align_floor` for the start and the raw `b` for the end (`:203-204`, loop `while t < end`), audio uses `a.to_audio_frames()` and `b.to_audio_frames()` (`:107`), that is, truncation on a DIFFERENT grid. One and the same boundary is rounded by two rules.
- **Consistency of the counter with the work.** `total_video` is computed from the UNALIGNED `r0` (`src/libs/kt-engine/src/export.rs:429`), while the loop starts at `align_floor(r0)`. A range whose boundary does not fall on the fps grid gives progress that diverges from the number of frames actually emitted.
- **Machine requirements as fields.** `GpuRequired` is a refusal at the moment of start, not an attribute of the job. There is nothing to ask "will this compute here" before submission, although the function that answers the question has been written.
- **Output of `range` to the outside.** kt-cli passes `None` (`src/apps/kt-cli/src/main.rs`), the GUI only the region (`src/apps/kt-app/src/render_dialog.rs`). Of the three sources of chunks — region, range marker, named area — only one is exposed outward.

### What blocks it

Nothing structural, and this has to be said plainly: the range is computed, the run-up is computed, the machine requirement is computed, the headless executor is written. One thing blocks it: the rounding boundary. While "frame" in a job is expressed as a tick, two adjacent chunks join correctly only because the region always comes to us from the grid; a job typed by hand or coming from foreign markup does not carry this guarantee, and two places (video and audio) round it differently.

### In the `collab/` category — two mandatory lines (rule 13)

OFFLINE: DEGENERATES — `src/libs/kt-engine/src/export.rs:396` — the job shape degenerates into a set of arguments of a single call: the range, options and backends exist exactly as long as the stack frame lives, and there is nowhere to record a request — `ExportOptions` has no serialization in any form, and the range is set by time, whereas the unit of work for us, as for everyone, is the frame.

NETWORK COST: the one who accepts the shape and returns a finished file — an `external executor` — the job shape is needed exactly where a process other than ours does the computing: one process does not need a request, it is enough for it to have arguments, and all three fields of the triple are already computed by us, but none of them exists separately from the call.

### Which invariants it touches

Invariant 7 (one composition code) is the source of strength: the "frame" of a job and the "frame" of the preview must be one and the same, and for us this holds by construction. Invariant 9 (in-process export) constrains not the shape of the job but its recipient: the queue can only be internal (conclusion of F-EXP-002, not revisited). Invariant 11 (determinism) is the reason the range must be frame-based: byte equality of runs makes sense only with matching boundaries.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage of the round: a range in frames, a single rounding rule, machine requirements as the answer of a function |
| Where the main work is | kt-engine (one boundary-rounding rule for video and audio, counter from the aligned start), kt-cli (`--range`), kt-model (the source of chunks: region, range marker, named area) |
| Project format | not touched while the job lives in the session; a persistent queue is `metadata.kt` and a new schema version (the same fork as F-EXP-002) |
| Tests that will have to be written | "two adjacent chunks give exactly the same frames as one whole" (video and audio separately); "a boundary off the fps grid gives a consistent counter and frame count"; "a sub-range with a reverb chain equals the same stretch of the whole run" — the run-up already exists, but has not been verified precisely at the job boundary |
| Depends on | F-EXP-002 (the queue; its conclusion "the queue is internal" is accepted here as a given) |
| Breaks compatibility | no |

## Verdict

We take the local core — and only it. The core here is small and entirely ours: express the range in FRAMES, round the boundary by one rule for video and audio, compute the counter from the aligned start, and turn `gpu_required_clips` from a reason for an error into a question that can be asked in advance. All of this is needed even without a second machine: a queue from F-EXP-002 without a job shape is indistinguishable from five trips to a dialog.

We do not open the server half — sending a job to someone — because there is no second participant, and this note has no right to announce its appearance. Separately we fix the boundary with the neighbors: the recipient of a job is dealt with in F-COL-039 (and there the price is that determinism turns from a property into a requirement), assembling results from chunks was rejected in F-COL-040, and the cheapest item to send is not a render at all but an analysis (F-COL-042).

What the note must leave behind: **the run-up in our system is computed, not guessed**. Any future "let's cut it into chunks" must rely on `chain_tail`, not on "five frames or so"; otherwise we would, with our own hands, create that very seam for whose absence invariant 11 was written.

## Sources

- https://docs.opencue.io/docs/concepts/glossary/ — verbatim definitions of `job`, `layer`, `frame`, `service`, `tags` (access date 2026-09-08)
- https://docs.opencue.io/docs/concepts/command-execution/ — execution of a command on a frame, the chunk and the rule "Resources are allocated per-frame, so all commands in a chunk share the same resources" (2026-09-08)
- https://community.adobe.com/t5/after-effects-discussions/workflow-advice-needed-rendering-4k-50fps-project-in-segments/m-p/14746398
  — cutting into segments, a five-frame overlap and the advice to move to a frame sequence, July 2024 (2026-09-08)
- https://forum.level1techs.com/t/davinci-resolve-remote-render-server-complete-guide-windows-linux/138807
  — Remote Rendering sends a finished queue row to a machine whole (eposvox, 12.02.2019; access date 2026-09-08)
- `docs/features/_editors.md`, row `flamenco-3` — the triple "command + frame range + machine requirements." The primary source `flamenco.blender.org` returns 403 from this environment, so the table row relies on the registry, while the mechanism is confirmed by OpenCue, whose documentation is open. **[NOT VERIFIED]** against the primary source: the exact shape of the Flamenco job.
- `docs/features/_services.md` — what lies on the other side; `docs/features/_trackers.md` — why `trackers:` is empty.
- Our code checked against the tree of 08.09.2026: `src/libs/kt-engine/src/export.rs`
  (lines 93, 107, 186, 203, 293, 298, 337, 396, 429),
  `src/libs/kt-fx/src/lib.rs` (24, 607), `src/apps/kt-cli/src/main.rs`,
  `src/apps/kt-app/src/render_dialog.rs`; `serde` in `kt-engine/src` — 0 matches.
