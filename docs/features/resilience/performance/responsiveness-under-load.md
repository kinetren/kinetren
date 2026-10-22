---
id: F-RES-010
title: Responsiveness under load: the program is alive but not responding
status: candidate
priority: B
cost: M
category: resilience/performance
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-app, kt-engine, kt-media]
invariants: [5, 6, 8, 12]
ui_rules: [12, 16, 19]
round: null
point: null
tests: []
depends_on: []
trackers: [kde-bugzilla, kde-invent, pitivi]
checked: 2026-09-08
---

# F-RES-010 — Responsiveness under load: the program is alive but not responding

## Essence

The editor tells "slow" apart from "broken" by one sign: does the window react. A render that runs
for forty minutes is normal work; a timeline that does not scroll for two seconds after a wheel click is a defect, even if
after two seconds everything has been drawn. The subject of this note is not the speed of computation but
what happens with INPUT while the computation runs: whether a click is accepted, whether the cursor moves,
whether the sound plays, and whether it is visible that the program is busy and with what.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | peak files `.sfk` next to the media [NOT VERIFIED] | the waveform is computed once when the file is added and lies as a sidecar; reopening the project does not recompute it. No general background-task window was found in the help |
| Adobe Premiere | 26 | Media Cache (`.pek` peaks, `.cfa` conform) + Progress panel | import puts peaks and decompressed audio into a SHARED cache directory (one for all projects), conforming runs in the background, and the clip is available before it finishes [NOT VERIFIED] |
| DaVinci Resolve | 21 | media-pool background tasks + render cache | Optimized Media / Proxy and the transition cache are computed in the background, and the state is shown by a coloured strip under the timeline ruler [NOT VERIFIED] |
| Final Cut Pro | 12 | Background Tasks (Window → Background Tasks, ⌘9) | a separate LIST WINDOW of tasks: a line per task, a percentage, Pause / Resume / Cancel on each. The only one of the five that shows background work as an object that can be controlled (more in F-HLP-013) |
| After Effects | 26 | Multi-Frame Rendering + disk cache | long operations run on the UI thread with modal progress and cancellation by Esc; a background queue exists only for network rendering [NOT VERIFIED] |

Open-source editors (Kdenlive, Pitivi) do not appear here as comparison rows — they are
sources of the count, not of the mechanism (`_editors.md`, section "Open-source software"). Their
design is analysed below as an explanation of where the count comes from.

## The trick behind it

**First.** What slows things down is not the "drawing code" but the ABSENCE OF PRIORITY FOR A TASK.
The Kdenlive maintainer writes plainly, in the list of causes of slowdowns, that
background tasks are created through `QtConcurrent::Map`, which cannot change
priority, and that the workaround is to launch external processes with `nice`, as
Shotcut does. From this follows a consequence that is absent from every list of features:
for them, "disable waveforms" really cures the freeze on zoom — not
because waveforms are heavy in themselves, but because they occupy the same no-priority pool into which the work needed
right now also gets queued. As long as there is no priority, any small background
job competes with playback on equal terms.

**Second.** In the same 2019 list there is an unfulfilled item: "separate cache
for audio and video, so that the sound is smooth and the video is jerky". That is, the degradation "the sound does not break up, the picture freezes" is for them not a property of the
architecture but a wish in the TODO. This is exactly what is recorded for us in
invariants 5 and 6 and held up by tests, and the difference between "we would like" and
"invariant" is the whole difference in behaviour under load.

**Third.** In Pitivi the waveform is recomputed when the playhead moves: the author of the
task notices that the drawing under the cursor CHANGES, and concludes from this
that it is recomputed on every shift. The waveform there is a function of the VIEW, not
a function of the file. Precisely for this reason they have zoom and scroll cost decodes, while for us —
under invariant 8 — they cost nothing: the peak pyramid is a property of the media and knows
nothing about the view.

## Pros

- A priority queue with generation-based cancellation makes possible what a no-priority pool cannot do at all:
  a seek makes already-queued work obsolete, rather than waiting for it to complete.
- Separate caches and the rule "playback reads ONLY the cache" turn a miss into degradation, not into waiting: silence or a frozen frame instead of stopping the window.
- The waveform and filmstrip from the index cost O(pixels of the view), not O(length of the material): a nine-hour clip is drawn by the same code and in the same time as a nine-second one.
- The master clock in the audio callback decouples the progress of sound from the UI load: an overloaded interface does not move playback.

## Cons

- The degradation is SILENT. Silence plus a frozen frame looks to the editor exactly
  like "the program has hung"; the underrun counter is honest, but it lives in the performance panel,
  which first has to be opened.
- Priority works INSIDE one source. There is no arbitration between the scheduler's sources:
  a hundred media files are a hundred workers competing for the disk with no arbiter.
- One thread and one open decoder for every media of the project, unconditionally, from the moment of loading — the cost is paid for files that have not been touched today.
- "Backgroundness" moves the waiting off the UI thread, but by itself does not report it: work without progress and without cancellation differs from a freeze
  only in that the window keeps redrawing.

## What users say

SIGNAL: interface freezes — 159 tasks in kde-bugzilla, `freeze|unresponsive|not respond|lag|stutter|beachball`; a share of 2.5% against 1.6% in pitivi

The third tracker of the same export, kde-invent, gives 41 tasks out of 2217 — 1.8%.

The comparison for which the note was opened: "renders slowly" in the same
kde-bugzilla — 21 tasks,
`slow.*(render|export|encod)|(render|export|encod).*slow`, a share of 0.3% against
0.0% in pitivi. **A gap of eight times.** People almost never complain that encoding takes a long time;
they complain that the window does not respond. This is the boundary between "expected and tolerable" and
"defect".

A caveat about the quality of the count, without which the topic's figure is misleading: the lexicon in
`_trackers.md` gives the topic "performance" 966 tasks, but its regexp
contains `ram` without word boundaries, and it catches `frame`, `program`, `parameter`.
With `\bram\b` 271 remain. My filter does not depend on this (it has no such fragment),
but the number 966 in the topic header is an upper estimate, not the size of the pain. The lexicon
line should be fixed separately.

> The performance issues manifest as significant lag and unresponsiveness of
> the entire application when attempting to zoom in the timeline (Ctrl +
> Scroll) and scroll the timeline left and right.
> — kde-invent #1082, 23.05.2021,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1082

> Jobs are created with QtConcurrent::Map which doesn't support adjusting job
> priority.
> — kde-invent #439 (a maintainer's edit in the task body), task of
> 15.11.2019, https://invent.kde.org/multimedia/kdenlive/-/work_items/439

> But it takes some 1-2 hours to open up my project.
> — kde-invent #1266, 02.12.2021,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1266

> Extremely sluggish/non-responsive UI during (and after) project loading.
> — pitivi #1316, 19.05.2013,
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1316

Our own user complained about this same class word for word: "every ~30 sec the program freezes and on the graph FPS drops to 0, as if some timeout is happening.
even playback doesn't need to be pressed" (`docs/user-feedback.md`, R.31 item 2).
The cause was exactly the same as in the others' case — a synchronous project save in the
UI thread from `eframe::App::save` every 30 seconds (item 197).

## How it fits Kinotrain

ADDRESS: src/libs/kt-engine/src/worker.rs — the class "zoom and playback ran into the decoder" is impossible for us: the queue is priority-based and with generation-based cancellation, and playback goes through `CacheFetcher` and does not wait for decoding at all. The class "the UI thread waits for a file" is POSSIBLE and is not caught by anything except a post-factum entry in the log: `src/apps/kt-app/src/app.rs`, `KtApp::open` → `build_player` opens EVERY media of the project synchronously, in the same frame from which the menu item was invoked.

### What already exists

- `src/libs/kt-engine/src/worker.rs` — one thread per media, EXCLUSIVELY owning
  the decoder (invariant 5). Queue: strict order Playback >
  NearPrefetch > FarPrefetch > Thumbnail; at equal priority — the nearest
  key FORWARD from the decoder cursor (anti-ping-pong, backward only when forward
  is empty); dedup by key; `cancel_generations_below` discards the work of
  stale generations on seek; a negative cache `failed_video`/`failed_audio`,
  so that prefetch does not drive the decoder into endless seeks past EOF. The audio and video cursors are SEPARATE —
  a shared one made the video zigzag after the audio.
- `src/libs/kt-engine/src/fetch.rs` — `CacheFetcher` handles only the cache; a miss =
  `None` = silence plus underrun. Waiting for decoding in the realtime path does not exist
  physically, not by agreement.
- `src/apps/kt-app/src/audio_out.rs` — in the cpal callback there are no allocations and no locks,
  only reading of the SPSC ring; a shortfall of frames is counted as an underrun and produces
  silence (invariant 6).
- `src/apps/kt-app/src/app.rs` (preview composition) — an incomplete composite does NOT overwrite the picture: the last COMPLETE frame is held, and `incomplete_since`
  measures how long we have been incomplete.
- Invariant 8 is closed by `src/libs/kt-engine/tests/viewport.rs` (test 13): 500
  random redraws of the viewport must produce ZERO new requests to the source.
  The waveform takes the pyramid level from the zoom
  (`src/apps/kt-app/src/timeline.rs`), thumbnails come from the sidecar DB
  (`src/libs/kt-media/src/index.rs`), and the thumbnail step is QUANTISED to the ladder
  `THUMB_STEP_LADDER` (`src/apps/kt-app/src/thumbs.rs`) precisely because a continuous
  dependence on zoom gave a new set of keys on every wheel click. This is our answer to kde-invent #1082, written before we read it.
- The stall detector is already in place: a UI-thread frame longer than 100 ms writes "ui stall N
  ms" to the performance-panel log (`src/apps/kt-app/src/app.rs`, item 197), and a freeze of the
  playhead during playback is the `Ev::Stall` event in the black-box ring
  (`src/libs/kt-engine/src/flight.rs`).
- The periodic autosave is moved to a background thread (item 197, test 194) — the very complaint "freezes every 30 seconds" is closed.

### What is missing

- **Opening a project is synchronous.** `KtApp::open` → `build_player`
  (`src/apps/kt-app/src/app.rs`) in a loop calls `open_source_or_placeholder`
  on every media. Inside — `FfmpegSource::open_indexed`
  (`src/libs/kt-media/src/ffmpeg.rs`): on first acquaintance with a file this is a
  FULL scan of the packet index, and `MediaIndexDb::ensure_media`
  (`src/libs/kt-media/src/index.rs`), when the mtime does not match, also reads the file
  in full for the sake of xxhash. All of this runs in the UI frame. The second opening is cheap (the index is persisted, test 34), the first one is linear in the volume of the material. The kde-invent #1266 scenario is not softened by anything for us.
- **Import is synchronous.** Dropping files runs a loop over `dropped`, and each
  `add_media` opens the source in the same frame: thirty files = thirty
  sequential FFmpeg openings in one `update()`.
- **Background work has neither a face nor a handle.** `spawn_index_worker`
  (`src/apps/kt-app/src/util.rs`) is fire-and-forget: no handle, no progress,
  no cancel flag, and no deduplication between calls (a new thread for every opening and
  for every media addition). `ThumbService` is built the same way. The only progress in the whole
  program is the render dialog
  (`src/apps/kt-app/src/render_dialog.rs`). Cancellation is covered by F-RES-012, but
  "how much is left and what is it busy with" — that is this note.
- **The freeze is visible only after the fact and only in the performance panel.** `ui stall` goes to
  `perf.note`, not to `note`/the bell, and the panel has to be opened in advance.
  The panel does not show the maximum frame time at all: `PerfState` computes the AVERAGE fps over the measurement window, and a four-second freeze looks on average
  like merely low fps.
- **The scheduler ends at the media boundary.** Priorities are honest within
  a worker; there is no arbiter between workers, and a thread per media is started unconditionally
  at loading (`Player::new`, `src/libs/kt-engine/src/player.rs`).

### What gets in the way

`KtApp::open` returns a ready `KtApp` and replaces `*self` entirely — this is convenient and makes
opening atomic, but precisely for this reason it cannot simply be moved to a thread: half of the fields
(`egui::TextureHandle`, the audio output) are not `Send` and live on the
UI thread. Asynchronous opening requires splitting the opening into "model + sources" (moved out) and
"window resources" (stays), and until then one will have to show modal progress — which is already more honest than silence.

### Which invariants it touches

Invariants 5, 6, 8, 12 — all four are CONFIRMED, none is shaken:
the whole work consists in extending their rule ("you cannot wait, you can degrade") to the paths of opening and importing, where today it does not apply. The UI checklist
rules: 12 (background work does not interfere), 16 (impossibility is an error, not silence), 19 (the dialog says where it can be changed later).

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2–3 stages of the round |
| Where the main work is | kt-app (splitting `KtApp::open`, background-task surface), kt-media (opening the source with yielding) |
| Project format | does not touch it |
| Tests that will have to be written | harness: opening a project does not produce a frame longer than N ms; "ui stall" reaches the visible surface, not only the log; maximum frame time in the performance panel |
| Depends on | F-RES-012 (cancellation), F-HLP-013 (log and bell) |
| Breaks compatibility | no |

## Verdict

Take it, priority B. The engine half of the topic is closed for us more strongly than in any of the
sources of the count: the priority queue, degradation instead of waiting, and invariant 8 answer exactly those tasks that have hung in the trackers since 2013 and 2019.
Not A because the remaining half is not the responsiveness of editing but the responsiveness of OPENING and IMPORT, and the editor works around it by waiting.

The open part is one and concrete: `KtApp::open` and `add_media` wait for files in the UI
frame, and all background work is faceless. Until it exists, "the program is busy" and
"the program has hung" differ for us only in that in the second case we wrote it to a log the user never opened.

## Sources

- kde-invent #1082 — https://invent.kde.org/multimedia/kdenlive/-/work_items/1082
  (task of 23.05.2021, accessed 2026-09-08)
- kde-invent #439 — https://invent.kde.org/multimedia/kdenlive/-/work_items/439
  (task of 15.11.2019, accessed 2026-09-08)
- kde-invent #1266 — https://invent.kde.org/multimedia/kdenlive/-/work_items/1266
  (task of 02.12.2021, accessed 2026-09-08)
- kde-invent #102, #1422 — same, accessed 2026-09-08
- pitivi #1316 — https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1316
  (task of 19.05.2013, accessed 2026-09-08)
- pitivi #748, #2036 — same, accessed 2026-09-08
- kde-bugzilla #475205 "Performance: UI freezes when handling big projects" —
  https://bugs.kde.org/show_bug.cgi?id=475205 (2023-10-04, accessed 2026-09-08)
- The count is reproducible:
  `python3 src/scripts/tracker-rank.py --query '<filter>'`; as of the date of writing
  ONLY kde-bugzilla, kde-invent and pitivi were exported to the cache — the other nine
  trackers of the registry were still downloading, and their shares are absent from the note.
- The rows for the five core editors — from vendor documentation, not checked live
  (marked `[NOT VERIFIED]` in the table); the FCP Background Tasks row is cross-checked with the analysis in F-HLP-013.
- Our code was read directly: `src/libs/kt-engine/src/{worker,fetch,player,flight}.rs`,
  `src/apps/kt-app/src/{app,util,thumbs,timeline,audio_out}.rs`,
  `src/libs/kt-media/src/{ffmpeg,index}.rs`, `src/libs/kt-engine/tests/viewport.rs`.
