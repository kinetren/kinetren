# Design: catch-up-shows-loading

## Context

Stage 10 (`stream-history-on-device`, D8) kept decode-request-storm's held frame: playing, a composite
whose stream stage lacked more than two positions ran the stage at the latest position the memo could
give without work (`StreamKernel::held`, `StreamPacing::hold`) and composed the frame from it, and the
preview drew "Catching up…" in its corner. The owner's run of stage 10 (`kkt-26.11.10b`):

- K1 / found-defects 403: "ну пусть loading показывает" — inside a loop the held picture may be from
  the history before the loop, a picture he did not watch;
- K2: "catchung up надпись ну очевидно можно в том же месте где loading писать";
- K5: "при кэтчинге - у меня vhs tape будто бы накладывается в этот момент актуальный … либо есть кадр
  либо нет, зачем накладывать эффект если предыдущий не готов?".

His answer to the diagnosis (`kkt-26.11.11` L2): "я не вижу смысла чтобы эффекты брали предыдущие картинки
и накладывались дальше - это пустая трата времени и ресурсов которая все замедляет" — no stage runs over a
previous picture anywhere, and a held composite is never stored (found-defects 483: in his stage-10 trace
248 of 920 kept-composite stores were held frames, under keys no moment asks again, while the kept tier
stood full at 658/659 MB).

K5 is the held frame's own consequence, read from the code: `layer_src` gave the held picture (position
f) to the layer, `fx` was the clip's chain resolved at the moment asked, so the event's VHS Tape (its
noise, tracking and tape state are functions of the moment) ran at t over the mosh of f; the track's
stages and the reel's stages ran over that composite. The composite was complete, presented, counted.

## Goals / Non-Goals

**Goals:** no composite in which a stage's input is not its own position; Loading (the owner's existing
look) wherever the history is behind, playing or paused, inside or outside a loop; the catch-up written
in Loading's place with how far it has to go; honest V-fps; the class walked.

**Non-Goals:** making the history walk faster (the audit's performance rows, K3/K4 — other changes);
the first picture of a preview that has none (kept, D4); the picker's cells (D6, row 460).

## Decisions

### D1. No held frame, anywhere
The owner's words leave no exception: "либо есть кадр либо нет". `user-feedback.md` holds no decision
that asks for a moving held picture; the hold was decode-request-storm's own answer to "the picture
stood" (kkt-26.11.08), and the owner has now decided the other way for the case it covers. So
`StreamPacing::hold`, `VideoRenderer::set_stream_hold`, the player's per-composite set, `stream_held`,
`GpuStages.stream_held`, `Composed.held`, `perf.vheld` and the catalog keys `perf.catching_up` /
`perf.v_held` go, and `StreamKernel::held` leaves the plugin contract: no host asks it, and a question
left in the contract invites the stand-in back. `mosh::held` stays a function of the codec: t2270 pins
the compiled schedule's answers to it (the held position is part of what the schedule must reproduce).

Rejected: keep the hold outside a loop while playing. Outside a loop the held picture is the stage's
latest, possibly seconds behind, and VHS still runs over it at the moment asked — K5 is not a loop
question.

Row 483 closes with it: a held composite was complete, so the display's executor kept it in its
present memo (`PresentMemo` keeps every complete composite) though the player did not note it as the
moment's. A short frame is incomplete now, and an incomplete composite is never kept (t2410 asserts the
frame incomplete; the memo keeps complete ones only, `kt-gpu` engine `present`).

### D2. A short frame says how short
`video_stream::run` returns `(None, 0)` when its frame lacks more than `compose_steps` positions (as it
did on a pause) and records the shortage: `VideoRenderer::stream_short` (an `AtomicU64`, the max over the
plan's stream layers, reset per plan; the sub-plans of reels and track stages share the root's counter,
as they shared the held flag). The player copies it into `GpuStages.stream_short`; the compose thread
into `Composed.catching_up`. The count is `needs(..).len()` — the positions whose source frames the
history still has to read and walk; it is computed already for the pacing decision (the trace's
`StreamRun.b`). A stopped or source-starved run is not a catch-up (count 0, plain Loading).

### D3. Loading's text names the catch-up
The scrim is the owner's rule (incomplete past 180 ms, the last full picture dimmed); its centre text
becomes `tf!(perf.loading_catching_up, n)` — "Loading — catching up: {0}" (the program's "noun: N"
form, no plurals) while the frame on screen is short, else `perf.loading`. Same font, colour and place;
the corner marker goes. `preview_catching_up` is set from each composite taken; an incomplete
composite does not overwrite the picture, but it does update the text. Nothing new repaints at rest: the
scrim's own timer (`rest_wake`) already asks the one frame at 180 ms.

### D4. What an incomplete composite does — unchanged, and the one presented exception
The app already keeps the last FULL picture and never presents an incomplete composite over it
(round 14 items 14–15, "the flicker black frame ↔ Loading"). The one incomplete composite ever presented
is the first picture of a preview that has shown nothing (`show_incomplete: !shown_any`), under the
scrim. That rule is the owner's (round 14: a picture under Loading rather than an empty area) and stays;
the stream stage gives it no stand-in now (its layer is absent, not old). Recorded as row 461 so he can
decide whether even that frame should be empty.

### D5. Frame blending lacking a side is incomplete
Walking the class found a second stand-in (see the table): a blended layer (R.37 C5) whose neighbour or
base frame is not in the cache took the other at full weight (`blend_frames` → `a.or(b)`), and the
executor reported the layer COMPLETE: the preview presented it and kept it, every later stage of the
chain ran over the stand-in, and the export's bytes depended on the cache's timing (the kt-engine
CLAUDE.md's chunk-budget note exists to keep that branch from firing). Now `layer_src` gives no frame
when either side of a pair is missing: the frame is incomplete, the preview waits (Loading), the export
retries as for any missing frame. A transition pair's missing half was already incomplete (t137's
scenario: "full weight and `complete = false`") and is never presented — kept.

### D6. Offline frames keep only complete composites
The reel strip and the project poster (`compose_offline_frame`) kept any composite the offline device
returned, complete or not: a layer whose file failed to open or whose history was cut short was simply
absent, and the track and reel stages ran over the rest; the reel strip then wrote the cell into the
sidecar's disk tier. Now an incomplete offline composite leaves the cell empty (the strip already shows a
missing cell), and both paths open a missing file as the editor does — the placeholder source with the
file's name (R.33 items 206–209) — so a project with a missing file still gets the picture its preview
shows.

### D7. Trace
No new event: the `loading` record (8) takes `b` = positions short when the scrim comes; `kt-cli trace
loading` prints "stream catching up: N positions" for such a stretch. The held constants
(`wait::STREAM_HELD`, `exec::STREAM_HELD`, `show::HELD`, `stream::HELD`) stay so older traces read;
nothing writes them.

## The class walk

Mechanism: a stage of the composite receives an input that is not its own position (a stand-in) and the
stages after it run over it at the moment asked; the composite is a picture no moment has. Checked:

| Place | Stand-in possible? | Found |
|---|---|---|
| Stream stage (Datamosh, any `StreamKernel`) in the live preview, playing | yes: the held picture | removed (D1); incomplete with its shortage (D2) |
| Stream stage paused | no: already incomplete | unchanged |
| Stream stage in an export (`stream_preroll`, no pacing) | no: the stage renders its own position or the frame is missing | confirmed by t2412 |
| Stream stage in the reel strip / poster (`stream_preroll`, cancel flag) | a cancelled or starved pre-roll left the layer absent and the composite was kept | D6 |
| Stream stage inside a reel or a track stage | the held picture travelled through the shared flag | gone with D1; shortage reaches the root (t2413) |
| Kept composites (`show_kept`, `PresentMemo`) | no: keyed by the stage's own key; complete only | unchanged (t2145, t2414) |
| Event chain after the stream slot (VHS Tape, Camera ISP, any later effect) | ran over the held picture | no input → no chain (t2411) |
| Track stages, reel stages, morpher layers over a short layer | ran over a composite holding the held picture | incomplete, not presented (t2411, t2413) |
| Frame blending pair (R.37 C5) with one side missing | yes: the other side at full weight, reported complete | D5 (t2418) |
| Transition pair with one half missing | full weight of the present half, `complete = false` | not presented; kept (t137) |
| Decode miss of an ordinary layer | no: the cache key is exact; layer absent, incomplete | unchanged |
| A decoder's "nearest frame" for a pts | the file's own grid frame, the position by definition | not the class |
| Slow route (offline device, bytes) | a late complete picture of ITS moment, never mixed | not the class |
| First picture of a fresh preview (`show_incomplete`) | absent layers, later stages over the rest, under the scrim | kept by the owner's rule (D4), row 461 |
| Nested reel sub-composite missing a leaf | `sub_ok` false → incomplete | unchanged |
| Freeze frame (`frame_hold_piece`) | a held picture by the person's own edit | not the class |
| Datamosh's sound (CPU codec on the audio thread) | no history substitution: renders its own position | not the class |
| VHS / ISP sound pairs | own position | not the class |
| FX picker cells (`fx_thumbs`) | a chain with a stream slot runs later candidates over the un-moshed base (the slot passes through) | a look question, row 460 |
| Timeline filmstrip, bin thumbnails | pre-FX source frames, no stages | not the class |
| `kt-cli pause-scan` / `preview` bench | composes as the preview does | follows D1 |
| V-fps | held composites were not counted already; incomplete ones are not | unchanged; `vheld` gone |

## Measured

Release, llvmpipe (no card here), footage in /dev/shm, `kt-cli preview … --size 640x360 --trace`; before =
stage 10's release (a3c98bf7, the code this change starts from), after = this branch. The counts are read
from each run's trace (`stream_run` aux, `frame_exec` flags).

| What | Before | After |
|---|---|---|
| showcase `--mosh`, cold play 69.5–72.5 s | 4 composites showed a held mosh (VHS Tape over another position), 8 complete, 114 incomplete; V-fps 3.0 | 0 held, 11 complete, 132 incomplete (Loading); V-fps 2.2 |
| his relinked copy, loop 4419.91–4422 s for 30 s | 29 held composites shown, 27 of them stored as kept composites (row 483); 936 complete; V-fps 32.0 (held counted as shown by the bench) | 0 held, 0 held stored; 862 complete, 238 incomplete; V-fps 28.5, all new pictures of their moments |
| same, VHS Tape frames run / processor | 154 VHS runs, 311.5 s of CPU over 30.1 s (10.3 cores) | 127 VHS runs, 274.8 s over 30.2 s (9.1 cores) |
| kept composites stored over the loop | 126 own + 27 held | 126 own |

The first lap now shows Loading where it showed a held picture; the later laps are the kept composites
of the moments' own pictures in both builds. Live `datamosh` (t2416): paused 1.9 s past the computed
history, then playing past a jump — "Loading — catching up: 38" in the preview's centre over the last
full picture, 0 vfps, no corner note; `idle` and `heavy_loop` green.

t2410/t2411/t2413 hold the renderer's shape (a short frame has no stand-in, says how short); the old
renderer passed them with its hold off, so the red-on-old guard is t2414 (the player set the hold while
playing: the old answer was a complete held frame).

## Risks / Trade-offs

- While the history is behind, a playing preview now stands on the last full picture under Loading
  where it moved before. The owner chose it (403, K5).
- An export with a frame-pair stand-in now retries instead of writing the stand-in: an export over a
  frame the cache can never hold fails (its own pre-roll heals what is missing; t1601's pressure test
  runs on).
- A reel-strip cell of an incomplete frame is now empty, not partial.

## Class-walk revision 2026-11

The callers, consumers and sibling paths of this change were walked again in the code by
`class-walk-revision` (2026-11-14); its `design.md` holds this change's row: what was checked, the
misses fixed (tests t2450–t2459) and the ones recorded (found-defects 510–524).
