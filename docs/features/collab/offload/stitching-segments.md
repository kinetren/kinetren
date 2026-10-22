---
id: F-COL-040
title: Stitching pieces after parallel rendering
status: rejected
priority: C
cost: L
category: collab/offload
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, flamenco-3]
we_have: no
crates: [kt-ffmpeg, kt-engine]
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

# F-COL-040 — Stitching pieces after parallel rendering

## Essence

A farm has computed the film on five machines and returned five files. Next, someone must turn them into one — and this is exactly the step for whose sake everything was begun: five pieces cannot be handed to the customer. The editor expects that the program which ordered the pieces will also assemble them. In practice it does not: assembly is everywhere moved outside and done by a separate tool, and an attempt to glue finished video files "end to end" produces a seam, whose cause is then sought for months in the wrong place.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 build 66 | none | there is nothing to divide the render with, so there is nothing to assemble; the batch script makes N INDEPENDENT files by regions, not N pieces of one |
| Adobe Premiere | 26.x | none (assembly by hand) | pieces computed separately are returned into the project as ordinary material and joined in a second sequence — that is, they are encoded once more |
| DaVinci Resolve | 21 Studio | none | Remote Rendering hands the job to another machine WHOLE and does not divide it: one queue row is one machine, so assembly never arises as a task |
| Final Cut Pro | 12.3 | none (Compressor) | distribution and assembly are moved into a second product; FCP itself produces no pieces |
| After Effects | 26.x | "Post-Render Action", and in practice a sequence of frames | a composition can be divided by in/out, but the recommended output is a sequence of frames: then "assembly" degenerates into a single encoding pass over ready files, not into stitching of video |

**Outside the core.** Without farms this feature cannot be described: the only ones who truly divide and assemble are job managers, and for them assembly is named as a separate stage with a separate cost.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Flamenco | 3.x | a separate assembly task after the render tasks | there is no cloud, both sides are in one network; assembly is one more job of the same kind "command + range + requirements", executed by ONE machine after the others (`docs/features/_editors.md`) |
| OpenCue | 1.x (docs.opencue.io, 2026) | a dependency between layers | the assembly layer depends on the render layer; this is still one command on one host — parallelism ends exactly where encoding begins |

## The trick behind it

**Nobody stitches video — everyone avoids the situation in which they would have to.** The farm's rule is formulated through refusal, not through assembly: "Always render to image sequences, not video files — if a frame fails, only that frame needs re-rendering". The point is that this same rule also removes the question of the seam entirely: a sequence of frames has no seam, because the unit is a frame, not a segment. Encoding after that is done in ONE pass by ONE machine, and parallelism ends exactly where the codec begins. That is, the "parallel rendering" of the industry is parallel COUNTING OF FRAMES, not parallel output of a file, and the difference is visible only to someone who has tried.

**Whoever does stitch looks for the cause of the seam in the wrong place.** The classic case: a composition was cut in two, computed as two MOV files, joined with `ffmpeg` — and a one-second freeze of picture and sound at the joint was obtained. The author suspects an inexact frame boundary; the respondent dismisses the suspicion and suggests looking for black frames and "the program inserts a gap". Neither names the real class of causes — a mismatch of encoding parameters between the pieces (different bitrate, a different position of the keyframe, a different start of timestamps). This is the price of stitching: the trouble arises in the codec, but is discussed in the editing program.

**For us the group of frames is OURS, and it does not coincide with the boundary of a piece.** The muxer sets `gop_size = fps * 2` (`src/libs/kt-ffmpeg/src/muxer_impl.rs:113`) and `max_b_frames = 0`, that is, a keyframe every two seconds, not where someone has decided to cut the job. A byte-level joining without re-encoding requires that the boundary of a piece BE a boundary of a group of frames; with an arbitrary range it almost never is. Moreover, our muxer is physically one-way: `avio_open` is opened only for writing, `write_video` accepts RGBA, and `write_audio` — f32 (`src/libs/kt-ffmpeg/src/muxer_impl.rs:278` and `:305`), and a path "take ready packets from a foreign file and lay them into one's own" does not exist in the crate at all. And the last detail that breaks even a theoretical joining: `video_pts` in the muxer is counted from zero independently of the export range — a piece carries no address of its own, it always starts from the beginning.

## Pros

- Division by frames gives a linear speed-up where a frame is expensive: this is true for 3D and compositing and is the real reason farms exist.
- After a failure, one must recompute not the whole film but one piece — and the finer the unit, the cheaper the failure (the limiting case is one frame).
- Assembly, moved out as a separate stage with a dependency, honestly shows that encoding does not parallelise: this is not a shortcoming of the manager but a property of the subject.
- Pieces as INDEPENDENT files (region = file) are a useful thing in themselves, and they require neither joining nor a farm: these are simply N jobs with different ranges.

## Cons

- The seam at the joint arises not in the picture but in the codec, and therefore cannot be reproduced by the one who fixes it: they have a different machine, a different encoder, and a different position of the keyframe.
- A byte-level joining requires agreement of EVERYTHING — codec, profile, bitrate, resolution, time base, and the boundary of the group of frames; a violation of any item turns the joining into re-encoding, that is, cancels the gain.
- Re-encoding for the sake of joining eats what the division was for: one pass over the whole material returns, only now also with a loss of quality.
- The timestamps of pieces do not add up by themselves: each file starts from zero, and the address of a piece has to be stored somewhere next to it.
- The acceleration of temporal effects (F-COL-038) requires overlap of pieces, and the overlap must be discarded at assembly — that is, the assembler must know about rendering more than "fold the files in order".

## What users say

A verbatim case of stitching, and a verbatim answer in which the real cause is not named:

> I split an after composition in 2 parts. Rendered them separately (mov) and
> tried to concatenate them using FFMPEG. It works well except for a 1-second
> stop (both audio and video) at the joint.
> — Mariela Sepulveda, Creative COW, 19.09.2016,
> https://creativecow.net/forums/thread/split-composition-into-separate-render-items-at-ex/

> There are frames in every After Effects composition, so that's not your
> problem.
> — Daniel Waldron, same thread, 19.09.2016

And the rule by which the industry bypasses the task instead of solving it:

> Always render to image sequences, not video files — if a frame fails, only
> that frame needs re-rendering.
> — "How Render Farms Work — Technical Guide (2026)", SuperRenders,
> published 04.04.2026, updated 07.09.2026,
> https://superrendersfarm.com/article/how-render-farms-work-technical-guide

`trackers:` is empty deliberately (rule 12): there are no public trackers for the five core editors, and in the twelve exported trackers of open-source NLEs there are no farms, no workers, and no joining of pieces — the topic does not exist, and a number here would be an invention, not a signal.

## How it fits Kinotrain
CAPABILITY: openspec/specs/export/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
Rejected: joining render segments is not needed — export is in-process and deterministic, and the chunks go into one muxer.
OFFLINE: DOES NOT REMAIN — `src/libs/kt-ffmpeg/src/muxer_impl.rs:113` — nothing of the trick remains, and the place where this is visible is exactly here: we set the group of frames ourselves every two seconds, independently of where someone wanted to cut the work; and next to it the muxer is open only for writing and accepts only RGBA and f32 — a path "take ready packets and lay them over" does not exist in the crate at all, therefore there is nothing to stitch with today.
NETWORK COST: N files from N machines, and whoever assembles them — invariant 9 — export is in-process, with no external binary and no temporary files, while all industrial assemblers of pieces are an external process, so "calling concat" is not forbidden to us at high cost but is impossible by contract.

## Estimate

| What | Answer |
|---|---|
| Scope | L — it would require a path of packets bypassing the decoder (demux → mux), rewriting of timestamps, and a check of stream compatibility |
| Where the main work is | kt-ffmpeg (reading packets and writing them without re-encoding), kt-engine (who decides, and by which rule, that a joint is legal) |
| Project format | not touched |
| Tests to write | they would have to be invented against our own invariants: "two pieces, joined without re-encoding, are byte-equal to a whole run" is unsatisfiable with different adapters (invariant 11) and meaningless with one (then the division is not needed) |
| Depends on | F-COL-038 (the form of the job), F-COL-039 (who computes a piece), F-EXP-003 (the same missing packet path) |
| Breaks compatibility | no — the feature is not opened |

## Verdict

**Rejected.**

The reason is named by number: **invariant 9**. Our export is fully in-process: muxing and encoding go through the system FFmpeg via dlopen, with no temporary file, no external binary, and no mkfifo. All the industry-standard assembly of chunks (`ffmpeg -f concat`, `mkvmerge`, the job manager's assembler) is an external process. The second reason is **invariant 11**: our determinism is local, chunks from different adapters differ legitimately, and the difference falls exactly at the seam.

**What is NOT the reason for the rejection** (so that a year from now nobody argues about the wrong thing): not the absence of a network, which is known without this note; not the complexity of `concat`; not the amount of code; not "our projects are small." A range exists in the engine and works, and handing out five chunks as five files is possible today. The rejection is about the fact that there is nothing to put them back together with, and no reason to.

**The cheap equivalent we take instead** is already almost assembled. A chunk must be not a part of a file but an independent FILE: N regions → N jobs → N finished files, each computed entirely by one process. This is the internal queue of F-EXP-002 plus the job shape of F-COL-038, and it requires no new mechanism at all. Speeding up a single file is sought not by dividing the encoder but by refusing to re-encode untouched chunks, that is, smart render (F-EXP-003), which needs exactly the same batch path, but inside one process and without any seam at all.

**The condition for reopening is exactly one.** If smart render ever gets a path "packet from a file → into the muxer without decoding," then joining chunks becomes its special case and will cost not a separate subsystem but the rule that "a job boundary must fall on a group-of-pictures boundary." Then the note is to be reopened, but the work must start from F-EXP-003, not from the farm.

Boundaries: the queue and output parameters are F-EXP-002; the job shape is F-COL-038; why the two executors cannot be considered identical is F-COL-039; why "cloud rendering" never turned out to be rendering is F-COL-041.

## Sources

- https://creativecow.net/forums/thread/split-composition-into-separate-render-items-at-ex/
  — "Split composition into separate render items at exact frame end," a one-second stop at the seam of two glued MOV files and a reply that steers suspicion away from the frame boundary, 19.09.2016 (access date 2026-09-08)
- https://superrendersfarm.com/article/how-render-farms-work-technical-guide —
  the rule "a sequence of frames, not a video file" (published 04.04.2026, updated 07.09.2026; access date 2026-09-08)
- https://community.adobe.com/t5/after-effects-discussions/workflow-advice-needed-rendering-4k-50fps-project-in-segments/m-p/14746398
  — why, when splitting into segments, overlap is advised and output as a frame sequence, July 2024 (2026-09-08)
- https://forum.level1techs.com/t/davinci-resolve-remote-render-server-complete-guide-windows-linux/138807
  — Remote Rendering hands the job whole to one machine, with no splitting (eposvox, 12.02.2019; 2026-09-08)
- https://docs.opencue.io/docs/concepts/glossary/ — dependencies between layers, from which the assembly stage is built (2026-09-08)
- `docs/features/_editors.md`, row `flamenco-3`; `flamenco.blender.org` returns 403 from this environment — **[NOT VERIFIED]** against the primary source: the exact shape of the assembler task.
- Our code checked against the tree of 08.09.2026:
  `src/libs/kt-ffmpeg/src/muxer_impl.rs` (lines 15, 113, 210, 278, 305, 401),
  `src/libs/kt-ffmpeg/src/source_impl.rs`,
  `src/libs/kt-engine/src/export.rs`; neighboring notes as a source for our own decisions: F-EXP-002 ("the queue is internal, otherwise invariant 9"),
  F-EXP-003 (smart render), F-RES-007 (determinism boundaries).
