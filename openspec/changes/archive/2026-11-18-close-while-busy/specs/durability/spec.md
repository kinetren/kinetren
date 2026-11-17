## ADDED Requirements

### Requirement: Closing a busy program ends it within a bound, after its durability
Closing the window or File → Quit SHALL end the process within a bound whatever the program is doing (a
loop catching up, a stream history walking, the render-ahead producer, a decoder span, GPU or compute
work in flight). `on_exit` SHALL first do the durability work, each wait bounded (the autosave in flight
5 s, a Ctrl+S being written 30 s, the recovery file); from then on the rest of the exit (the poster, the
offline engine's park, telemetry's last event, the drops that join threads, eframe's teardown, the C exit
hooks) SHALL take at most `EXIT_BOUND` (8 s): past it a watcher records what was still waited for,
flushes the trace and ends the process with `_exit(0)`. Every owner's drop joins its threads with a bound
(`kt_time::shutdown`, 2 s) and leaves a thread past it behind; the replaced app of File → Open/New is
released the same way on its `app-drop` thread.
Why: close-while-busy (kkt-26.11.18 T1): the owner closed the window while a loop caught up and had to
kill the process; the player's drop joined a prefetch pass that walked a Datamosh history for up to 37 s,
and the decoder, the sound device's thread, the effect cells and kt-gpu's exit hook waited with no bound.

#### Scenario: A close's wait is bounded and leaves a stuck thread behind
- **WHEN** a close joins a thread that does not end by its deadline, and another that does
- **THEN** the join returns at the deadline, the first is recorded as left behind and the second as ended
- **TEST** t2950 (`src/libs/kt-time/src/shutdown.rs`)

#### Scenario: A stuck exit is forced and names what it waited for
- **WHEN** the exit's watcher passes its bound while a wait of the close left a thread behind
- **THEN** a forced-exit record names that thread, the trace is flushed, and only then the process ends
- **TEST** t2951 (`src/apps/kt-app/src/exit_watch.rs`)

#### Scenario: The window closes while the loop catches up
- **WHEN** the heavy project (Camera ISP → Datamosh → VHS Tape) is closed paused while its history walks, playing on its first laps, and by File → Quit while playing
- **THEN** each process ends within 15 s of the close (a software rasterizer in Xvfb), by itself (no forced exit), and its trace names the close
- **PATH** open the heavy project paused on its loop → the window's cross → the window goes and the process ends; again playing the loop's first laps → the cross → the process ends; again playing → File → Quit → the process ends
- **TEST** t2954 live scenario `src/scripts/live/close_busy.py`
