# Design: test-stability

## Context

`docs/found-defects.md` listed some thirty tests and scenarios that went red under load and green
alone. The load is real: this machine's disks are virtual rotational ones, `/tmp` lives on one of them,
and a parallel build plus the test run itself reach a load of 50–80 on 32 cores. In the first
baseline of this change (no edit yet, load about 50): 1531 of 1532 green in 310 s, the one red was t815
at "writing the restore record took 44 s". Dozens of trivial tests stood 19–40 s at the same moment:
the disk, or the processes starting behind it, not the tests.

## Causes, read before any edit

| Red | Cause | Fix |
|---|---|---|
| t815 | Measured the two fsyncs of the write: 0.05 s quiet, 44 s here. The write runs on exit, autosave has its own thread | The thread's processor clock: the program's own work (D2) |
| t951, t952 | 20 s limit over a download that syncs a file | Hang guard (D1) |
| t476 | The worker opens the sidecar before its queue: a new sidecar's first WAL header is synced, on a busy disk past 45 s | Hang guard. Nothing waits on that worker |
| t411, t640, t795, t1413 | 60 s limits over the same opening | Hang guard. The locks were fixed by `sidecar-fsync-stall` |
| t663 | 60 s over a composite that waits for a decoder | Hang guard; no stale frame in these runs (below) |
| t836 | Read the events 50 ms after the drop: the start is sent from the sender thread | Waits for the start, then checks that nothing follows it |
| t362 | Red in `cargo test` only: a `flock` lives on the open file, and a child another test forks at that moment holds a copy of the descriptor until its exec | Waits for the release |
| t874, t841 | 50 ms for a call that reads a file | The bound is what a real block would cost: a whole request against a black hole (D3) |
| t20 | Fixed 300 ms for the ring to fill | Waits for the samples |
| t599 | The harness frames had already ordered the reel; its job could be anywhere | Starts the queue empty, with no frame after that |
| t1282 | On a slow frame the deferred click fired before the test's own toggle, which then stopped playback | Toggles only if not playing |
| t1524 | A real-time count read as an absolute number | Counted against a control (D4) |
| t1538 | A 3 s clock against a 1.5 s path | A source that never answers |
| t939, t796c | `gen.sh`: a shared temp name (row 29, row 58) | Unique temp names and a lock, t1670 |
| `vhs_tape`, row 47 | Ctrl+S writes on the UI thread and syncs twice (6.7 s measured at load 76). The framework's silent 10 s ran out and read the old file | `saved_project` is `save_now`, which waits and fails aloud |
| `envclip`, `corner`, `audiofx`: "did not write after Ctrl+S" | **A program defect.** `i.modifiers` is the state at the frame's end. A stalled frame that read Ctrl down, S down and Ctrl up together saw a bare S: the event was split and nothing saved. 1 run in 6 at load 50, none of 12 after that; the mechanism is held by t1672, red on the old code | The chord is read from `Event::Key`; the framework holds the modifier until the key is read |
| `read_bell`, row 79 | Esc closes the window on top of the LAST frame: pressed before the new window was drawn once, it went down the cascade. No stall was involved: nothing plays at that step | Waits for the window, then for a fresh dump. The behaviour itself is row 86 |
| `conform`, `nestshare`, `camera_mic`, `loop` (the click before the region) | **A click lost with no trace.** A move and a press in one slow frame press what the pointer was over before | `Live._arrive`: the move is read first |
| `formula`, `forms`, `nestparam`, `winshell` | Two causes of one symptom. An idle program starts a frame on the first event, so with any gap the second click lands one render later, past 0.3 s two single clicks; all four events in one frame are no gesture on an event either. And a click within 0.6 s of the click before last is egui's triple, which is no double click: a click just before the gesture lost it every time (`winshell`, 2 of 2) | The press is read alone, the rest goes in one write; the gesture waits the triple window out |
| `jump_zone` | A double click read in one frame is one jump, and the scene needed two to stand clear of the project's start | The scene jumps once more itself |
| `loop` (the folds), `space_hold` | The condition was a place the head passes, and at a frame a second the dump passes over it. The way back was measured before the head had turned | The fold is the event: "the head went back" |
| `winshell` | 50 frames in a fixed 15 s | 60 frames, however long they take |
| `ffmpeg_download` | The mirror answered after 3 s, before a busy machine reached the bar | The scenario releases the answer |
| `analysis` | Compared the screen before the thumbnails had landed | `wait_still` first |
| display collisions, row 26 | Two runs on `:99` clicked into each other's windows | `live-check.sh` takes a free block of displays and holds a lock |
| t905 (found by the new guard) | A wait for the head to move: a lap brings it back to the frame it left, so the condition could never hold. It stood 633 s, then passed | Waits for the render thread to take the count |
| ui-probe `BadWindow`, row 30 | A window closed during the tree walk | The walk skips it |

Not a cause: sidecar `database is locked` (row 27). `ece65c8f` fixed it, and none of the runs of this
change saw it.

## Decisions

### D1: one bound for a wait, and it is a hang guard

`kt_time::test_deadline()`, ten minutes. A wait for what must happen needs a bound only so that a
real hang ends the test. The number has to outlast the worst the machine does (one fsync took 137 s,
one opening round of t1171 304 s), and it costs nothing until a defect reaches it. kt-time because
every crate that tests a wait depends on it.

Rejected: raising each number by hand, because each would be a new guess; a retry, because a test that
passes on its second try says nothing; moving the tests' temp files to tmpfs. That was rejected in
`sidecar-fsync-stall` as hiding a stall a person's disk has. t1460 now holds that stall on its own, so
the old reason weakens, and tmpfs would shorten the run; it is left as a question, because it would
also take the disk out of every test.

t1671 keeps it: a line with `Instant::now() + Duration::from_…` or `.elapsed() < Duration::from_…`
fails unless the guard's list names it, with its reason. Three limits stay (t1460's 3 s under the
busy timeout, and the 250 ms and 900 ms of `video_stall`): they ARE the check. They did not go red in
these runs; if one does, it is a statement about the program under load.

### D2: t815 counts the program's work

The scenario promises "the write fits without interface lag". The wall clock of a synced write says
what the disk did. `CLOCK_THREAD_CPUTIME_ID` around `write_recovery_now` counts what the program did
(serialisation, formatting), which is what can grow into an interface lag, and it does not count the
sync, which happens on the autosave thread anyway. The size bound of the same test stays.

### D3: a bound that says what it would catch

"Does not block" against a black hole: a call that waited for it takes the request's whole timeout (10
s, 2 s). So that timeout is the bound, not 50 ms on a quiet machine. `start` reads the day's file, and
that alone passed 50 ms here.

### D4: a real-time count against a control

t1524 plays through the real player against the wall clock, and the machine decides how much it gets.
A control of one pass of the same file now plays beside it, in the same seconds. The scene passes at
70 % of what the control showed. On a free machine the control shows the grid, so the bound is the old
one, and the defect it holds (the second pass seeks back a GOP) costs the scene alone.

### D5: live scenarios run on the program's clock

`poll_step` is the step of some fifty loops that count attempts: each is a clock limit in disguise. It
now stretches with the observed frame time (`stretched`), and so do `wait_playhead`'s limits and the
scenarios' own deadlines. On a free machine nothing changes. A consequence is read with `Live.until`,
not from the dump right after the gesture. `winshell` counted frames in a fixed 15 s; the growth it holds was per frame,
so it now moves the mouse for 60 frames, more than the 50 it asked for, and takes a third of the time.
A window that closes is waited out with `wait_gone`, not looked at once.

### D6: the cost of the live run

`still` and `screen_diff` compare directly from X with PIL at `compare -fuzz 3%`'s threshold, in place of
an `xwd`, a `convert` and a `compare` per look. `settle()` ends on a still picture too: an idle
program ticks four times a second, so at a frame step past 0.1 s frames never stop, and the wait burned
its whole deadline, 10 s and more, on a screen that stood still. `readme_shots` paid that twice per
step. A blinking caret is 100 pixels of allowance (`CARET_PIXELS`), or a text field never settles.

## Measurements

To fill in from the final runs.

## Risks

- A real hang now takes ten minutes to go red. That is rare, and the run goes on in parallel.
- t1524 can pass on a machine so busy that the control itself fails. That run proves little, by
  construction; the quiet runs (CI, `finish.sh`) still hold the 70 %.
- Ctrl+S on the UI thread stands for the length of the disk's sync. That is a defect of its own, left
  in `docs/found-defects.md`.
