---
id: F-EXP-005
title: The render progress window — what the person learns while the file is being written
status: done
priority: B
cost: M
category: export/pipeline
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, shotcut-26.09]
we_have: yes
crates: [kt-engine, kt-ffmpeg, kt-app]
invariants: [9, 11, 12]
ui_rules: [3, 10, 13, 16, 19, 22]
round: kkt-26.10.27
point: change render-window-v1-parity (the owner's v1-level request)
tests: [954, 955, 956, 957, 958, 959]
depends_on: [F-EXP-004, F-EXP-002]
trackers: []
services: []
checked: 2026-10-28
---

# F-EXP-005 — The render progress window — what the person learns while the file is being written

## Essence

A render runs for minutes or hours, and for all that time the progress window is the only
thing the editor looks at. From it he wants three answers: WHAT is being written (the settings
that actually took effect, not the ones he asked for), HOW it is going (how much is done, how
fast, how long is left, how big the file already is) and WHAT CAME OUT (where the file is, open
it, what went wrong). A window that answers only "how much" makes him wait blind and guess the
rest from the finished file.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | the render progress dialog | a percentage, elapsed and remaining time; in the render dialog's Options — "Open folder when rendering is complete" and "Close dialog when rendering is complete"; when the render finishes, the **Cancel button turns into Close in the same place** [forum posts of several versions; the forum moved to `forum.borisfx.com`, threads redirect — not checked live] |
| Adobe Premiere | 26.x | Export progress / Adobe Media Encoder queue | in-app export shows a bar and the estimated time remaining; the queue in Media Encoder shows elapsed/remaining per item and keeps an encoding log file in the user's documents [NOT VERIFIED: `helpx.adobe.com` does not open from this environment] |
| DaVinci Resolve | 21 | Deliver → Render Queue | per job: a bar, a percentage and the time remaining; a finished job keeps the line "completed in hh:mm:ss" in the queue [NOT VERIFIED live; the BMD forum returns 403] |
| Final Cut Pro | 12.3 | Background Tasks (⌘9) + the share notification | a percentage among other background tasks; the system notification at the end carries a "Show" action that reveals the file in Finder [NOT VERIFIED] |
| After Effects | 26.x | Render Queue → **Current Render Details** | the deepest of the five: besides elapsed and "Est. Remain", the twirl-down shows the frame being rendered, the time per frame, the **current file size, the estimated final file size and the free disk space** [from the documentation as remembered; not checked live] |

**Outside the core.** Shotcut gets a line because its Jobs panel is the public, source-readable
form of "the log is the support channel": every job keeps the full engine log one click away.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Shotcut | 26.9.27 | Jobs panel → View Log / Open / Show in Folder | the job's right-click menu opens the full melt/FFmpeg log (the command line and stderr), opens the file or reveals it in the file manager; the elapsed time stays next to a finished job [from use; the source not re-read in this pass] |

## The trick behind it

**1. A window that shows settings before the encoder has opened cannot name the encoder
truthfully — and our own predecessor did not.** Kinotrain v1's `RenderProgressDialog`
(`git show cpp-reference-final:renderprogressdialog.cpp`, `setExportInfo`) printed the fixed
string "H.264 (libx264), yuv420p, preset veryfast, CRF 23" whatever was chosen: the fuller window
was partly a decoration. Where the encoder is chosen by a real open check (F-EXP-004), the only
honest moment to name it is after `avcodec_open2` succeeded — i.e. the window must learn it FROM
the run, not from the dialog that started it. The same holds for the pixel format and the
quality knob (CRF/CQ/QP/global quality differ per encoder, F-EXP-004 trick 3).

**2. File size and free space are a prediction of failure, not a statistic.** After Effects puts
"Est. Final File Size" next to "Free Disk Space" in Current Render Details: the pair tells the
editor at minute five that the render will die at minute fifty. A size row without the free space
next to it is decoration; the current bitrate (bytes per second of OUTPUT, not of wall clock)
says whether the quality setting does what he thinks.

**3. A button that changes meaning in place at the end is a trap in both directions.** In Vegas
the Cancel button becomes Close where it stood: a click aimed at Cancel a moment too late closes
the dialog, and the reverse — a cancel that lands during the final mux — throws away a file that
was 100 % rendered (the forum thread title "Accidentally canceled render at 100%"). The
counters have the same disease: a window that keeps counting after the end (elapsed grows, the
speed sinks) lies about the run that just finished.

## Pros

- Naming the encoder from the run makes the class "I asked for hardware, it silently fell back"
  visible WHILE it happens, not after an hour (F-EXP-004 Cons: "the search stays silent about
  what it lost").
- A log kept by the run (which candidates were tried and why they failed, which sources became
  placeholders, which backend composes) is what a person can paste into a report — the line
  "it crashed" becomes a file.
- Speed as × real time of the OUTPUT is comparable across projects of different fps; frames per
  second are not.
- Open file / Open folder at the end removes the trip through the file manager for the one
  action that follows every render.

## Cons

- Every row in the window is a promise that must stay true: a row "Video: h264_nvenc" that is
  wrong for one frame of the run is worse than no row.
- A speed chart is the cheapest way to make a progress window heat the machine: it redraws all
  the time (invariant 12 — repaint only while something moves).
- A log in a window is a second place for messages next to the bell; without a rule it becomes a
  copy of it.
- Opening a file "in the default application" is an OS service with its own failures (no
  handler, a sandbox, a portal) — one more thing that can say "nothing happened".

## What users say

NO QUOTES: searched the Vegas forum for the Cancel→Close trap ("Accidentally canceled render at
100%", "Close this dialog when rendering completes") — the threads of
`vegascreativesoftware.info` redirect to `forum.borisfx.com` (2026-10-28) and the posts could not be
read with their dates; `helpx.adobe.com` and the BMD forum are unreachable from this environment
(same as F-EXP-004). The owner's own words are the primary signal here: "a very sparse render
window — the C++ version used to have a fuller one" (`docs/user-feedback.md`, 2026-10-27) and
ktprompts v26.10 T12 "the render window counts statistics after finish".

## How it fits Kinotrain

CAPABILITY: openspec/specs/export/ — the contract is in `spec.md`, the "why" in `design.md`.
The engine reports events (the opened encoder, progress with packet bytes, log facts) and counts only; every clock belongs to the window in `render_dialog.rs`, which shows two forms, the speed chart, the log, a frozen final block and Open file / Open folder.

## Estimate

| What | Answer |
|---|---|
| Scope | M — two stages: the engine reports, the window shows |
| Where the main work is | kt-app (`render_dialog.rs`), kt-engine (`export.rs` events), kt-ffmpeg (`muxer_impl.rs` byte counters and the open report) |
| Project format | not touched |
| Tests that will have to be written | the engine reports the encoder before the first frame and bytes that add up to the file; the window's rows from a scripted run; counters frozen after the end; the overwrite question; a live scenario |
| Depends on | F-EXP-004 (what the encoder line can say), F-EXP-002 (a queue would reuse the same rows) |
| Breaks compatibility | no |

## Verdict

Take it, B. The owner asked for at least v1's level of information; we take v1's rows, chart
and log, but not its fixed settings string — the window learns the encoder, pixel format and
backend from the run (trick 1), adds size/bitrate/free space (trick 2), and freezes at the end
with the buttons that follow a render (trick 3).

## Sources

- `git show cpp-reference-final:renderprogressdialog.cpp`, `renderprogressdialog.h`,
  `projectrenderer.h`, `speedchartwidget.cpp` — Kinotrain v1 (read 2026-10-28).
- https://www.vegascreativesoftware.info/us/forum/accidentally-canceled-render-at-100--68357
  (redirects to `forum.borisfx.com`, 2026-10-28) and the search results for the Vegas options
  "Open folder when rendering is complete" / "Close dialog when rendering is complete"
  (2026-10-28).
- Shotcut Jobs panel (`src/docks/jobsdock.cpp` of `mltframework/shotcut`) — described from
  use, the source was not re-read in this pass (2026-10-28).
- Our code read in the tree: `src/apps/kt-app/src/render_dialog.rs`,
  `src/libs/kt-engine/src/export.rs`, `src/libs/kt-ffmpeg/src/muxer_impl.rs`,
  `src/apps/kt-app/src/graphs.rs`, `src/apps/kt-app/src/bg.rs`.
