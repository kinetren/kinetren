# Design: one-scheduler

## The mechanism (the class)

Background work ran on many ad-hoc threads, each with its own queue discipline and its own timer.
Four things followed, each seen before as a single defect:

- nothing knew what was running (no counter, no trace of a job as a job);
- nothing paused all of it while playing (thumbnails and analysis never paused, the index never did);
- nothing cancelled it by one rule (flags, generations, `stop` atomics, a thread that never ended);
- some of it still waited on the UI thread (Ctrl+S's fsyncs, media opens, a decoder's join), and
  the rest woke on a clock at rest.

The change gives all of it one home and one rule, and walks every thread and every timed wait in
the workspace (the table at the end).

## D1. One executor: classes, lanes, a small pool (`kt_time::sched`)

- **Classes** in a fixed priority order: now, answer, near, visible, background, offline. The order
  is general (owner, H13: no tuning to one machine).
- **Lanes**: a producer's FIFO, ONE job at a time. A service keeps the serialization its thread gave
  it (one thumbnail decode at a time, one analysis at a time). A lane takes one-shot jobs (`post`,
  `post_keyed` — a newer job of the same key supersedes a queued one: a series of Ctrl+S writes
  the last), or runs a pump over the service's own queue (`service` + `kick`): the pump drains,
  returns `More` (run again, after higher classes and after a pause) or `Idle` (run again only on a
  kick). Kicks coalesce; a kick while the pump runs runs it once more after, so no wake is lost.
- **The pool** is sized at run time from this machine's cores: a quarter of them, 3 to 6. The
  visible, background and offline classes may hold all workers but one, so a person's answer (a
  save, a media open) never waits behind background work (t2230 proves it with the pool held).
- **Workers park on a condvar** until a post, a resume or an exit: no clock anywhere (t2233 reads
  their context switches: zero over 1.5 s at rest).
- **A panicking job** is caught; the panic hook still runs (the crash path is unchanged), the
  worker lives on and the lane runs its next job (t2232).

Rejected: a priority queue of individual jobs across all producers. A service's own queue already
knows its order (thumbs LIFO, the poster queue replacing by file, effect cells by generation); a
lane keeps that knowledge where it is and gives the scheduler only "this producer has work".

## D2. Pause votes, not per-service flags

The owner agreed (kkt-26.11.08 H15, "Да"): background services pause while playing. A `Vote` is
held by every `Player` (up in `play`, down in `pause`, down on drop) and by every export's body (up
for its length). While any vote is up the pausable classes (visible, background) start nothing,
and a running job's `checkpoint` waits on the gate (a condvar) until the vote goes down or the job
is cancelled. A pump sees `paused()` between items and returns `More`. `BgCtl::should_stop` waits on
the same gate instead of sleeping 30 ms in a loop, and its own pause flag (the render window's
Pause) notifies the gate. The per-service flags (`set_posters_paused`, `ReelPreviews::set_paused`,
`FxThumbService::set_busy` with its 25 ms sleep) are gone: one rule, set where playback is.

The UI no longer sets a pause each frame: a vote follows the transport the moment `play` is called.

Cost: a test process runs many players; a playing player pauses the background of every other test
in the process for its length. Tests wait for what must happen with `test_deadline`, so this costs
time, never a red.

## D3. Invariants

- **5 (a decoder instance is owned by one thread).** Decoder owners stay dedicated threads
  (`media-worker-<id>`). A lane's job may open a short-lived source of its own; the thumbnail pump
  keeps its open sources for ONE run and drops them when the run ends, so a source never outlives
  the thread that opened it (a later run may land on another worker). The sidecar connection
  (sqlite, `Send`) is kept between runs. The offline path's three conditions are unchanged: a lane's
  source is shared with nobody, promises no tempo, and its job is finite and cancellable (the lane's
  token, closed with its owner).
- **6 (nothing on the cpal callback).** The callback is untouched. The audio render thread stays
  dedicated; it is marked as a thread that must not wait (`kt_time::block`).
- **12 (repaint only while something moves).** Jobs wake the UI through `wake::now`/`wake::soon`
  when they publish; the analysis' 120 ms repaint while running is gone (its progress and its end
  wake the UI).

## D4. The UI thread's waits become jobs (rows 98, 148)

- **Ctrl+S** (row 98): the project text and the write with its two fsyncs run on the `save` lane
  (class answer). The UI takes the outcome within `ANSWER_GRACE` (3 ms; the test's bound under
  `cfg(test)`, as `COMPOSE_GRACE` composes in the harness's frame) or in a later frame
  (`take_saved`). The asterisk clears only if no edit came while the file was written. On exit a
  save in flight is waited for (30 s). Autosave moved from a thread per write to the `autosave` lane
  (class background: it waits out playback, checklist rule 12).
- **Media gestures** (row 148): `add_media_clip_at` (a file dropped onto the timeline),
  `place_media_at` (a card placed from the bin), `sync_media_sources` (undo/redo of a media's arrival
  or a path change), the sources reopened when a file returns, and the largest-source probe of the
  render window open their file on the `media` lane; the decoder swap (`replace_media`'s join) and
  removal (`remove_media`'s join) run there too through `Player::sources()`. The gesture lands when
  the lane answers (`take_media`): the clips, the notes, the material length. A new media id is added
  to the player on the UI thread (a new worker thread is spawned; nothing is joined).
- **The counter** (`kt_time::block`): the UI thread (`KtApp::ui`) and the audio render thread mark
  themselves; a file open, an fsync, a join, a renderer build, a sidecar open count there and are
  traced with their length (`Ev::Block`). t2236 runs the gestures above in the harness and requires
  zero. Opening a PROJECT still builds its player on the UI thread (the constructor's one build,
  edit-without-rebuild) and opens its sources: not one of the listed gestures; the counter shows it,
  and it is the next step (question in the report).

## D5. Timers at rest

| Timer | Before | Now |
|---|---|---|
| audio render on a pause | 250 ms | two 100 ms tail passes (the block the callback was handing out when the pause came), then parked with no clock |
| prefetch on a pause | 100 ms | a pass while playing, while a stream stage lacks history, or after a state change; else parked |
| index service | `recv_timeout(200 ms)` | a pump, kicked by `enqueue` |
| media watchdog | `sleep(900 ms)` | a pump poked on events: the opening, focus gained or lost, play/stop; never a pointer move (a hover runs no job) |
| load watch | 1 s | parks after 5 quiet seconds; any counted work (a UI frame included) wakes it; never parks on the bench |
| effect cells | 1 s wait, 25 ms sleep while busy | a pump; the pause is the scheduler's |
| thumbnails | 2 s wait (row 174) | a pump |
| `BgCtl` pause | 30 ms sleep loop | the gate |
| telemetry heartbeat | every 5 minutes | a quiet session sleeps on its channel; the next counted activity re-arms it |

**Lost wakes.** A parked engine thread waits for the `Wake` notify count to move from what it saw
before it last read the state (`Wake::seen`, `wait_since`): a seek between its last look and its wait
is never lost (t2240 seeks in bursts). Every input a parked thread acts on notifies: play, pause, a
seek, a rate, the preview quality, a loop region, a published build, a retune or a replaced source,
the player's drop. `set_playback_rate` and `set_preview_quality` did not notify before (the timer
covered it).

**The load watch** cannot see an uncounted busy thread while parked. The rule "the load watch never
calls uncounted work rest" (t1880) still holds for every second it samples; a mouse move (a UI
frame) wakes it, and the owner's complaints are always made while moving the mouse. The bench mode
(`KT_GEOM_DUMP`) keeps sampling four times a second for the live scenarios.

## D6. Row 114: the caps never evict what is on screen

The ready LRUs of thumbnails (384) and effect cells (256) evicted the oldest entry even when the
current frame had drawn it; a filmstrip or a gallery wider than the cap re-ordered its own pictures
every frame in a circle. Each `get` now stamps the entry with the UI frame number (the process's
`Work::UiFrame` count: a clock that stands at rest); eviction skips what the last two frames drew,
so past the cap only what is on screen stays (`thumbs::evict_unshown`, t2237).

## D7. What the trace shows

`SchedQueue` (job, class, lane, who posted it), `SchedRun` (a span: queue wait, outcome), a job's
own records carry it as their cause, `SchedCancel` (reason), `SchedPause` (pause/resume, the
reasons' mask, jobs waiting), `Block` (kind, role, length). `kt-cli trace during a b` lists
`sched_run:visible:thumbs`, `sched_pause`, `block:ui:fsync` … so a pause's window shows at a glance
whether anything ran. `Work::SchedJob` counts every started job for the load watch and the live
`idle` scenario.

## D8. A test that leaned on a timer

t2144 asked a frame whose decode the prefetch thread had not yet ordered only because prefetch's
first pass came 100 ms after the quality was set. `set_preview_quality` now wakes prefetch at once,
which ordered the key first (the request then names the prefetch pass, correctly). The test now
stands the playhead past the frame, so the prefetch window never covers it; its claim is unchanged.

## The class walk: every thread, timed wait and channel worker

Threads (`thread::spawn` / `Builder::spawn`, product code):

| Thread | Before | Home now |
|---|---|---|
| `media-worker-<id>` (kt-engine worker.rs) | decoder owner, priority queue, condvar | unchanged (invariant 5); its join is a counted wait, run on the `media` lane from gestures |
| `audio-render` (player.rs) | 8 ms playing, 250 ms paused | dedicated (invariant 6), marked must-not-wait; parks with no clock on a pause |
| `prefetch` (player.rs) | 100 ms always | dedicated; parks on a pause with nothing to do |
| `engine-build` (player.rs) | condvar on the newest want | unchanged (one build per version) |
| `audio-drop` (player.rs) | a thread per old renderer dropped | unchanged: it ends at once; a drop that may take long must not stand on the render thread |
| `preview-compose` (compose_queue.rs) | latest-wins mailbox | unchanged; its finite 1 ms device poll after a present (`wait_presented`) is the GPU arbiter's (scheduler stage 2) |
| export pipeline threads (export*.rs) | bounded FIFOs, 50 ms wait while held, 40 ms sleep while paused | unchanged (offline, its own pool and devices); the export holds an EXPORT vote; its pause poll runs only while the person paused it |
| `trace-writer` (kt-time) | parks when empty | unchanged |
| `pipeline-warm` (kt-gpu compiled.rs) | started by work, gone when empty; 50 ms wait for a frame boundary while warming | unchanged |
| `compute-open`, `compute-warm` (kt-plugin-api) | one-off at first use | unchanged |
| `thumbs` (kt-app) | LIFO, 2 s timer, never ended | lane `thumbs` (visible), pump |
| `fxthumbs` | LIFO+generation, 1 s timer, 25 ms busy sleep | lane `fx-thumbs` (visible), pump; drop waits for its running cell (t1644) |
| `kt-posters` | FIFO-replace, condvar, own pause flag | lane `posters` (background), pump |
| `kt-poster-load` | a thread per welcome-window load | unchanged: a one-off read of a few PNGs when the window opens |
| `reel-preview` (`BgJob`) | a thread per job, own pause | lane `reel-preview` (visible), `BgJob::on_lane` |
| `analysis` (`BgJob`) | a thread per job, no pause | lane `analysis` (background) |
| `index-service` | `recv_timeout(200 ms)` | lane `index` (background), pump |
| `media-watchdog` | `sleep(900 ms)` | lane `media-watch` (background), poked on events |
| `autosave` | a thread per write | lane `autosave` (background) |
| Ctrl+S | the UI thread | lane `save` (answer) |
| `reopen-media` | a thread per look, then a join on the UI thread | lane `media` (answer) |
| `max-source-size` | a thread per ask | lane `media` |
| `load-watch` | 1 s | parks after 5 quiet seconds |
| `tele-journal` | blocked in `recv` | unchanged |
| `kt-telemetry` | heartbeat every 5 min | sleeps on its channel on a quiet session |
| `kt-update-check` | one-off at start (network) | unchanged: finite, at start, never at rest |
| `ffmpeg-download`, `envclip` (`BgJob::spawn`) | a thread per job | unchanged: they may hold their thread for as long as a server or the X clipboard owner likes; on a pool worker they could hold it forever |
| `render` (`BgJob::spawn`) | a thread per export | unchanged (offline class, its own devices); EXPORT vote |
| `engine-build` (kt-app, the display engine at start), `offline-engine` | one-off | unchanged |
| `geom-dump`, `live-ctl` | bench only | unchanged |
| `supervisor`, `t2084-fault`, `live-fault` | the crash path / tests | unchanged |
| kt-cli `kt-cli-probe` | the CLI | unchanged |

Timed waits outside threads above: `builds.rs` (50 ms while a consumer waits for a build, finite),
`compose_queue.rs` (`wait_timeout_while` for a composite the UI asked, finite), kt-otio `io.rs`
(a retry on Windows), `hw_trial.rs` (a child process' poll, finite), `project_lock.rs`, `analysis.rs`
and `update_check.rs` sleeps (tests or finite retries), `synthetic.rs` (a model decoder's delay,
tests). None runs at rest.

## Not in this change

- The GPU arbiter (one submission queue per device with class priority, bounded submissions, the
  compute host's jobs tagged by class, a stop check in `StreamKernel::render`): `gpu-arbiter`.
- Back-pressure from consumer capacity across producers and the memory ledger: `memory-ledger`.
- Opening a project off the UI thread (the constructor's build and source opens).
