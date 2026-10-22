---
id: F-RES-012
title: Cancelling a long operation and progress
status: done
priority: B
cost: M
category: resilience/performance
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app, kt-cli, kt-engine, kt-media]
invariants: [5, 9, 11]
ui_rules: [12, 14, 16, 19]
round: "R.49"
point: 367
tests: [398, 408, 409, 410]
depends_on: [F-RES-010]
trackers: [kde-bugzilla, kde-invent, pitivi, shotcut]
checked: 2026-09-15
---

# F-RES-012 — Cancelling a long operation and progress

## Essence

A long operation is one that does not finish in the frame where it started:
rendering, indexing on import, waveforms and thumbnails, proxies, analysis. The
editor has exactly three questions about it, and they are DIFFERENT: can they
see that it is running; is it told how much is left; can it be stopped. Answering
only the first one is how you get a progress bar that sits at 99 % for half an
hour. An operation that cannot be cancelled turns a miss of a menu item into an
hour of waiting.

This topic is adjacent to F-RES-010 and does not overlap with it: that one owns
the RESPONSIVENESS of the window, this one owns CANCELLATION and PROGRESS. "The
window keeps drawing while work is running" belongs there; "the work names itself,
counts itself and can stop" belongs here.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Rendering Progress window | a modal render window with a percentage and Cancel; there is no shared queue of background tasks, and cancel is one button for the whole render. The vendor's forum has threads "hangs during render and can't be cancelled" and "freeze stuck at 22% render" [NOT VERIFIED] |
| Adobe Premiere | 26 | Progress panel + Media Encoder queue | indexing and conforming run in the background and, per the documentation, cannot be stopped from the editor; a managed queue with Pause/Stop per job is in a separate application (AME), not in the editor [NOT VERIFIED] |
| DaVinci Resolve | 21 | Render Queue (Deliver page) | during rendering, Start Render turns into Stop Render; a queue job has ✕ in its corner; background media-pool tasks are removed from their own list [NOT VERIFIED] |
| Final Cut Pro | 12.3 | Background Tasks (Window → Background Tasks, ⌘9) | a LIST of tasks with a percentage and three buttons on each: Pause, Resume, Cancel. In addition, tasks pause THEMSELVES while the user is working (see "The trick behind it"). For the analysis of this window as a surface, see F-HLP-013 and F-RES-010 |
| After Effects | 26 | Render Queue + modal progress | a render queue with Stop per job; short long operations run modally and are dismissed with Esc; a background queue exists only for network rendering [NOT VERIFIED] |

Kdenlive, Pitivi and Shotcut do not appear here as comparison rows: according to
`_editors.md` (section "Open-source software") they are sources of the COUNT, not
of the mechanism. Their design is analysed below — it explains where the count
comes from, and in one place (Shotcut) it provides the only live proof that
cancellability is determined by architecture, not by effort.

## The trick behind it

**First: cancellability is a first-day decision, not a button.** The cancel flag
is a parameter that has to pass through every layer down to the innermost loop;
adding it later means changing signatures along the whole path. In our own
repository this is visible in two neighbouring functions. `export_mp4_with_backends`
(`src/libs/kt-engine/src/export.rs`) takes `cancel: &Arc<AtomicBool>` and checks it
on EVERY frame and every audio chunk — cancellation takes effect within one frame,
not "at the end of the current stage". `PeakPyramid::build(&mut dyn MediaSource)
-> PeakPyramid` (`src/libs/kt-media/src/index.rs`) is built the same way inside,
in chunks — `for bidx in 0..total_bursts` — but there is nowhere in it for a flag
to live: no parameter, no intermediate result; the whole pyramid is returned at once.
One language, one neighbouring crate, the difference is only in what was thought
about at the moment the signature was declared.

Shotcut demonstrates the other end of the same idea. Its long operations are
SEPARATE `melt` processes, so cancelling there equals killing the process and
costs nothing; across the 1368 tasks in its tracker, complaints of "can't cancel"
are 0.0 % against 0.2 % for Kdenlive, which does the same thing inside the process.
The price of Shotcut's cheap cancellation is exactly what our invariant 9 forbids:
an external binary in the pipeline.

**Second: progress without a time estimate does not reduce alarm, it increases it — and
it is the estimate that lies.** Shotcut documents this class of problem in its own
instructions: "If a job appears to be stuck or hung (please allow ample time for
percentage to change), right-click the job in the Jobs panel and choose Stop"
(shotcut.org/FAQ). The manual explains in advance that its own percentage cannot be
trusted. The cause is structural and can be recognised by the DENOMINATOR: where the
full count is known exactly (frames of a range), the percentage cannot lie; where it
is estimated (bytes, files of unknown weight, "time to end"), it can. Ours has an
exact denominator: `total_video` is computed from the tick range and the fps BEFORE
the loop. And our time estimate is exactly the one that pitivi #1024 complains about:
`elapsed / frac * (1 - frac)` — extrapolation at the average speed from the start,
which means that a light beginning of the timeline underestimates the remainder at
the heavy end.

**Third: the best cancellation is the one nobody presses.** Our decoder queue has a
generation-based cancellation: `cancel_generations_below`
(`src/libs/kt-engine/src/worker.rs`) discards the work of stale generations, and
the seek itself, which made the work obsolete, IS the cancellation — the user makes
no decision and does not see a button. This is the invisible half of the answer to
"zoom piles up the queue": Kdenlive's maintainer writes that their tasks go through
`QtConcurrent::Map`, which cannot even change priority (quote and analysis —
F-RES-010), so they can only offer a button. Apple reached the same conclusion from a
third side: Background Tasks does have Pause/Resume/Cancel buttons, but the
documentation immediately says that "If you actively use Final Cut Pro while
background tasks are running, the background tasks are paused" — the pause is
provided by the interaction itself. Three different designs, one conclusion: a
decision that the program is able to make on its own must not be handed to the user
as a button.

## Pros

- The cancel flag is checked per frame and per chunk, not per stage: the render
  window reacts to Cancel within one frame even on a four-hour project.
- Cancellation DELETES the unfinished file (`std::fs::remove_file(out_path)` on all
  three paths: an error in the video phase, an error in the audio phase, cancellation
  before finalize) — after cancellation no artifact is left on disk that could be
  mistaken for a result.
- Pause is cooperative (a 40 ms sleep) and is itself cancellable: `while pause && !cancel` —
  by pausing, the user does not lock himself in.
- The denominator is exact, so the share does not drift: 100 % is reached exactly on
  the last frame of the range, not "roughly there".
- The progress window cannot be closed while work is running: the OK/Close buttons live
  only in the `Some(Ok)`/`Some(Err)` branches. An orphaned job without a window does not
  happen (rules 14 and 16).
- Speed is shown as four numbers (min/avg/cur/max ×realtime), not one: the divergence
  of cur and avg makes an estimation error visible without any explanation.
- Generation-based cancellation in the decoder queue costs the user zero decisions and
  works in a scenario where a button would not help at all (scrolling, scrubbing, seeking).

## Cons

- **The bar and the estimate lie in exactly the way other products are complained about.**
  `frac` in `src/apps/kt-app/src/render_dialog.rs` is computed ONLY from video, while the
  phases run sequentially: all of video, then audio. Throughout the audio phase the bar
  sits at 100 %, `eta` drops to 0 ("about ~0s left"), and `speed` =
  `video_frames / elapsed` monotonically falls to zero. Three of the four indicators show
  a finished and dying render while it is still running; only the counter "Audio {n}/{N}"
  and the phase caption move.
- The time estimate is a direct extrapolation at the average speed from the start, with no
  sliding window and no account for the fact that the phase will change.
- Everything except export has neither progress nor cancellation: `spawn_index_worker`,
  `ThumbService`, `KtApp::open`, `add_media`.
- Exiting the program during a render leaves an unfinished file — the direct opposite of the
  Cancel button, which deletes it.
- `kt-cli render` passes a constant-false cancel flag and an empty progress closure to the
  engine: headless rendering stays silent until the end, and Ctrl+C kills it, leaving a
  truncated mp4.
- The cancellation path is covered by no test at all: across all of `src/`, `cancel` appears in
  tests once — `scheduler.rs::t9_generation_cancellation_drops_stale_work`, and that is the
  INTERNAL cancellation of the queue, not a user cancellation.

## What users say

SIGNAL: progress of a long operation — 12 tasks in pitivi, `progress ?(bar|dialog|indicat|report|info|window|display)|no progress|show.{0,12}progress|progress.{0,12}(shown|show|display|missing|wrong|stuck|not)`; a share of 0.4% against 0.1% in shotcut

The same filter yields 13 tasks in kde-bugzilla (0.2 %) and 7 in kde-invent (0.3 %).
The fourfold gap between Pitivi and Shotcut is not about code quality: Shotcut has a
Jobs panel, where a long job exists as an OBJECT with a line, a percentage and
Stop, while in Pitivi project loading and clip import run silently — half of its
hits (5 of 12) are about loading and import, not about rendering. Kdenlive is the
opposite: 11 of 13 are about the render window, that is, about progress that EXISTS
and is wrong.

Next, three classes separate, and this separation is the subject of the note.

**There is no progress.** Exactly our indexing case:

> No progress indication or feedback when refreshing timeline audio waveforms
> and thumbnails.
> — shotcut #384, 18.02.2017, https://github.com/mltframework/shotcut/issues/384

> When adding a large amount of clips to the timeline, or when loading a
> project, the GUI will become unresponsive without an indication of what is
> going on.
> — pitivi #461, 19.05.2009,
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/461

**There is progress and it lies.** The filter
`(progress|eta|estimat[a-z]*|remaining time|time remaining|percent[a-z]*|[0-9]+ ?%).{0,45}(wrong|incorrect|inaccurate|not real|too fast|instantly|inconsist|never|forever|stuck|frozen|smarter|0:00|00:00|misleading)|(stuck|hangs?|freezes?|stays?|sits?).{0,20}(at|on).{0,6}([0-9]+ ?%|estimating)`
yields 8 tasks in kde-bugzilla (0.1 %) against 2 in pitivi (0.1 %) and 1 in shotcut
(0.1 %). Of the eight, six at Kdenlive are on point (two false positives — about the
audio boost and about the file properties panel), and these are headings that read
like a description of our own window in the audio phase:

> rendering stuck with remaining time of 00:00:00
> — kde-bugzilla #487069, 15.05.2024, https://bugs.kde.org/show_bug.cgi?id=487069

> Odd timing/fps indications. Render time says remaining time 0:00:00 for hours
> while it actually finishes.
> — kde-bugzilla #489210, 26.06.2024, https://bugs.kde.org/show_bug.cgi?id=489210

> As long as the time estimate is over 1 or 2 minutes, we shouldn't show
> seconds, because they fluctuate too much and it's a bit silly.
> — pitivi #1024, 05.01.2011,
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1024

**Cannot be cancelled.** The filter
`(cancel|abort|interrupt)[a-z]*.{0,30}(render|export|encod|job|task|proxy|transcod|analy)|(render|export|encod|job|task|proxy|transcod|analy)[a-z]*.{0,30}(cancel|abort|interrupt)`
yields 10 tasks in kde-bugzilla (0.2 %), 3 in pitivi (0.1 %) and ZERO in kde-invent,
and in shotcut (0.0 %). The zero at Shotcut is not the absence of users: it has 1368
tasks and two complaints about the absence of progress. This is the absence of a CLASS,
explained by the design (an external process, see "The trick behind it").

> Unable to abort rendering job.
> — kde-bugzilla #345277, 17.03.2015, https://bugs.kde.org/show_bug.cgi?id=345277

> If you start rendering a movie, then cancel the rendering and then try to
> restart it again Pitivi fails to respond.
> — pitivi #35, 24.03.2006, https://gitlab.gnome.org/GNOME/pitivi/-/work_items/35

**Cancelled (closed), but it keeps going** — a separate small class, 2 tasks in
kde-bugzilla by the filter
`(cancel|abort|stop|clos)[a-z]*.{0,40}(still|keeps?|continue|does ?n.t stop|remain)|(still|keeps?|continue)[a-z]*.{0,40}(after (cancel|abort|stop)|running)`
(0.0 % against 0.0 % in pitivi — there are none of them there at all), and these are literally the headings
"a process still running after quitting" (#376861, 23.02.2017) and "Process Still
Running after Program Close" (#490519, 19.07.2024). Close in meaning is
kde-bugzilla #513510, 17.12.2025: "'Shutdown computer after rendering' still
occurs even if I abort the rendering manually" — the cancellation did not cancel the CONSEQUENCE.

The request half of the trackers asks for exactly what we lack: kde-bugzilla
#497942 (27.12.2024) — "more verbose menu / list, of running jobs i.e. proxy
clip generation, list of queued jobs, with thumbs of clip and option to cancel
job from list", and kde-invent #210 and #656 (2019–2020) — "progress bar should show
loading progress", separately for opening a project and separately for importing clips.

The owner's own user complained about the "progress" class in the form "the audio
preview disappeared" in an unnamed project (`docs/user-feedback.md`, item 3), and the fix
was not to show progress but to build the pyramid in memory. Indexing still
did not get any progress after that.

For the two closed editors there is no tracker, and this is exactly the case for which
`_trackers.md` separately notes that the vendor's forum is not a tracker:
the thread "Vegas Pro 18 hangs during render and can't be cancelled" exists, but
after the VEGAS forum moved to the Boris FX site its URL returns a 301 to the root —
such a source has, by its nature, no state, decision dates or deduplication.

## How it fits Kinotrain
CAPABILITY: openspec/specs/durability/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
A long operation is a finite, cancellable task (`BgJob`) with its own source; cancellation is checked by a test, the export ETA is deferred.
ADDRESS: `src/apps/kt-app/src/app.rs`:538 and `src/apps/kt-app/src/util.rs`:89 — the class "a long operation without progress and without cancellation" is POSSIBLE in our code and is not caught by anything: `KtApp::open` starts `spawn_index_worker` without a handle, a flag or deduplication, and `*self = app` on the next open replaces `peaks` with a new `Arc` — the old thread stays alive, keeps reading the media of the CLOSED project, holds its `ktindex.db` and writes peaks into a map that nobody reads any more. The class "zoom and playback pile up the decode queue", on the contrary, is impossible for us — `cancel_generations_below` in `src/libs/kt-engine/src/worker.rs` cancels stale generations without any user involvement.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2–3 stages of the round |
| Where the main work is | kt-app (share by phase and ETA in the window; handle and flag for indexing and thumbnails; cancelling the render in `on_exit`), kt-media (flag in `PeakPyramid::build` and in the first index scan), kt-cli (live flag + progress line on stderr) |
| Project format | does not touch it |
| Tests that will have to be written | cancellation in the middle of the video phase: `ExportError::Cancelled` and ZERO files on disk; pause is lifted by cancellation without freezing; the progress share is monotonic and reaches 1.0 only in `Done`; closing the project during indexing stops the thread; a repeated `spawn_index_worker` for the same file does not start a second thread |
| Depends on | F-RES-010 (background task surface), F-HLP-013 (log and bell) |
| Breaks compatibility | no |

This is a frame, not a spec: exactly which surface will carry the task list is decided in the
round together with F-RES-010 and F-HLP-013, three notes at once, otherwise we get three
different places for one object.

## Verdict

Take it, priority B. We have exactly one exemplary instance — the render dialog
with phases, exact counters, speed, pause, cancel and deletion
of the unfinished file — and it shows that the mechanics already exist in the engine and cost
little. Not A because the editor works around the lack by waiting: indexing
ends on its own, and a render can be cancelled.

There are two pieces of work, and both are narrow. The first is to FIX what is already shown: the share and the
time estimate in the audio phase reproduce, letter for letter, Kdenlive's class of "stuck at
100 %, 0 s left", and this is the only place in the program where we are actively misleading the user. The second is to hand out the flag and handle to
those operations that did not get them on the day they were written: indexing, thumbnails,
rendering on exit and `kt-cli`. The second half is more expensive than the first precisely
because it is the price of the decision taken on the first day.

## Sources

- The count is reproducible: `python3 src/scripts/tracker-rank.py --query '<filter>'`
  with the filters given verbatim in "What users say". At the time of writing the cache
  `~/.cache/kinotrain/trackers/` held FOUR full exports
  — kde-bugzilla (6346), kde-invent (2217), pitivi (2672), shotcut (1368);
  `mlt` (601) finished only after the section was written and does not take part in the note's comparisons
  (by the head filter it gives 1 task = 0.2 %); the other seven trackers of the registry were not exported as of 2026-09-08.
- shotcut #384 — https://github.com/mltframework/shotcut/issues/384
  (task of 18.02.2017, accessed 2026-09-08)
- shotcut #759, #225 — same, accessed 2026-09-08
- pitivi #461 — https://gitlab.gnome.org/GNOME/pitivi/-/work_items/461
  (19.05.2009, accessed 2026-09-08)
- pitivi #35 — https://gitlab.gnome.org/GNOME/pitivi/-/work_items/35
  (24.03.2006, accessed 2026-09-08)
- pitivi #186, #1024, #1296, #1666 — same, accessed 2026-09-08
- kde-bugzilla #345277 — https://bugs.kde.org/show_bug.cgi?id=345277
  (17.03.2015, accessed 2026-09-08)
- kde-bugzilla #487069, #489210, #490250, #491433, #488135, #490519, #376861,
  #513510, #497942 — https://bugs.kde.org/show_bug.cgi?id=<N>, accessed
  2026-09-08. The Bugzilla export does not contain the bodies of the tasks — the quotes from it are
  the headings, that is, the report author's own wording.
- kde-invent #210, #656, #1157, #1226 — https://invent.kde.org/multimedia/kdenlive/-/work_items/<N>,
  accessed 2026-09-08
- Shotcut FAQ, section on hung tasks — https://www.shotcut.org/FAQ/
  (accessed 2026-09-08)
- Apple, "View background tasks in Final Cut Pro for Mac" —
  https://support.apple.com/guide/final-cut-pro/view-background-tasks-ver64e71609/mac
  (accessed 2026-09-08)
- VEGAS forum: the thread "Vegas Pro 18 hangs during render and can't be cancelled"
  (vegascreativesoftware.info/us/forum/…-123163/) as of 2026-09-08 returns a 301 to
  forum.borisfx.com — the body is unavailable, so it is not taken as a quote; the table row
  is marked [NOT VERIFIED].
- The rows for the five core editors — from vendor documentation and reference
  materials, not checked live; FCP is cross-checked with the analysis in F-HLP-013 and
  F-RES-010.
- Our code was read directly: `src/apps/kt-app/src/{render_dialog,util,thumbs,app}.rs`,
  `src/libs/kt-engine/src/{export,worker,player}.rs`,
  `src/libs/kt-media/src/index.rs`, `src/apps/kt-cli/src/main.rs`,
  `src/libs/kt-engine/tests/scheduler.rs`.

**Implemented in R.49** — docs/porting-notes.md item 367
(tests t398, t408, t409, t410); the feature's later history is there.
