# Design: telemetry-journal

## Context

Four records of what the program did existed, each built for one complaint and none for the next:
the load watch (`load_watch.rs`, idle-cpu E3) with a report stamped "N s ago"; the playback black
box (`kt_engine::flight`, R.28 item 10), a memory ring dumped into crash reports; `kt-app.log`
(`journal.rs`, R.49), warnings and refusals per run; the Diagnostics log (`PerfState::note`), memory
only. The owner's run of 61414dba showed what that costs: a report with no clock time, no record of
what he did, and a log he had to enlarge the window to see.

## D1. The mechanism and the class

The mechanism: a diagnostic record that is (a) kept apart from the others, (b) stamped late (at the
next UI frame, at the report's writing) or relatively ("N s ago", "since start", UTC without saying),
or (c) without the person's action next to it. Any one of the three breaks "say what I did and when".

The walk — every place the program writes or shows a record of what happened, with the verdict:

| Place | (a) apart | (b) stamp | (c) action | Verdict |
|---|---|---|---|---|
| `load-report.txt` (`load_watch.rs`) | yes | "N s ago" from the write; a hot spell rewritten every 5 s | none | FOLDED: each busy second is a journal record at its second; the report is the journal's last 15 min, clock time |
| Load section of Diagnostics | — | live | — | stays live (it is the last second) |
| Flight ring (`kt_engine::flight`) | yes, memory | ms since the player was built | play/pause/seek only | FOLDED by a sink called on push: each new record, and a closed run of repeats with the time of its last repeat. The ring stays: the crash report and "Save playback log" read it |
| Diagnostics log (`PerfState::note`) | yes, memory | at the next UI frame after a 0.5 s sample; UTC | none | REPLACED: underruns, cache eviction spikes, no-read drops, low video hit and offline media are raised by the load watch at the second they happen, into the journal; the Diagnostics pane shows the journal's tail |
| `kt-app.log` (`journal.rs`) | yes | local, at the moment | no | MIRRORED line for line; the file stays: the crash window reads one run's tail from it and it is written synchronously in `on_exit` and the panic hook |
| Crash report `kt-app-crash.txt` | yes | local | flight dump | STAYS apart (one file per crash, handed to the next start); the journal gets the CRASH line and is flushed by the hook |
| Notices (the bell) | UI | `chrono_like_now` = UTC hh:mm:ss without a zone | no | MIRRORED into the journal; the stamp FIXED to local time (the same defect: a clock the person does not read) |
| Render log `render.log` (`render_dialog.rs`) | yes, per export | elapsed since the job's start | export only | MIRRORED line for line ("export: …"); the file stays next to the export as the window's copy |
| Telemetry (PostHog, `telemetry.rs`) | — | — | counts actions | STAYS apart and never reads the journal ("телеметрия != трекинг"). The journal hooks the same door (`telemetry::signal`) before the consent check, so an action is journaled with telemetry off |
| `KT_GEOM_DUMP` geometry dump and `<dump>.load.json` | test only | — | — | not a record for the person; unchanged |
| stderr / `eprintln!` (10 sites in kt-app) | goes nowhere (`Terminal=false`) | — | — | unchanged: each is a developer trace or duplicates a `journal::log` line; listed in "Found, not fixed" if one is the only trace |
| `kt-cli` output | stdout of a headless run | — | — | a person's run of kt-cli is its own terminal; no journal |
| Supervisor's crash window | reads `kt-app.log` | — | — | unchanged |

What the person did, and where it enters:

| Action | Door |
|---|---|
| an edit (command name, target event) | `telemetry::signal(Commands/Command/Gesture)`: one record per batch, one per gesture (the first command of a gesture id) |
| undo / redo | `signal(Undo/Redo)` |
| project open (how, file name), save (file name) | `signal(ProjectOpened)`, `signal(Saved)` plus the path's file name from `KtApp` |
| export started/ended | the render log mirror, `signal(ExportEnded)` |
| window opened / closed | `window_shell`: the set of shell windows shown differs from the previous frame (title, once per change) |
| an effect selected in a chain | `fx_editor`: `ChainSel` changes |
| play / pause / seek / loop region / wrap / stall | the flight sink (render thread and `Player::play/pause`) |
| playback summary | the UI frame on which `playing` turns false: length, V-fps, new frames, skipped cells, rollbacks, underruns |
| the person's mark | `Cmd::MarkMoment` (Ctrl+Alt+M in every style) and the Diagnostics button with its note field |

## D2. The journal: one writer, stamped by the caller

`tele_journal.rs`. `record(kind, text)` takes the time at the call — `Clock::now()` returns the Unix
milliseconds and the process's monotonic offset — formats the line there and sends it on a channel.
The writer thread blocks in `recv` (no timer, no poll: zero wake-ups at rest, t1964), takes whatever
else is queued, and appends it with one `write_all` to a file opened with `O_APPEND` and no buffer.
The kernel then holds the line: a SIGKILL a moment later loses nothing written (t1962). The panic hook
sends a flush barrier and waits at most 300 ms. Rotation: before a write that would pass 2 MiB the file
moves to `.1` (`.1` to `.2`); three files bound the journal at 6 MiB (t1963).

Why not write from the caller: the flight sink runs on the audio render thread (invariant 6 forbids
it waiting on a disk), and the UI must not wait on one either ("no interface action waits"). A channel
send is a short allocation, which the render thread already does for its own ring.

The journal is NOT per run: a run starts with a RUN line (version, OS, pid), so a crash and the next
start read in order. The line: `2026-11-07 14:03:12.345+03:00  +123.456  KIND   text`. Kinds: RUN,
LOAD, PLAY, NOTE, ACT, MARK, LOG, EXPORT. Paths never enter it: a project is named by its file name;
media by the names the program already shows.

An in-memory tail (the last 15 minutes, at most 4000 records) serves Diagnostics and the report
without reading the file.

## D3. The load report is the journal's export

The load watch writes each non-rest second as a LOAD record at that second, and one "at rest" record
when a busy spell ends: a long rest costs nothing. The report keeps its machine header (devices, which
card drives the monitors) and lists the journal's last 15 minutes, every kind, oldest first, each with
its clock time. It is written by itself after three seconds of a whole core (at most once in five
seconds) and by the Diagnostics button.

## D4. The Diagnostics notes at the moment

`sample_perf` ran inside a UI frame: since the tick went, on a pause that is the next mouse move, and
its note carried that frame's time. The load watch already wakes once a second (its cost was set by
idle-cpu E3 and is unchanged), so a `PlayerProbe` (Arc clones of the cache, the underrun counters, the
transport, the offline set) is handed to it by each `KtApp`, and it raises the notes with its sample's
time. The graphs stay on the frame.

## D5. The Diagnostics window shows the journal without enlarging it

The window body was one scroll: metrics, load, three graphs, then the log, which the graphs pushed
under the fold at the default height (row 109 / G12). Now the body splits: the journal pane takes the
bottom of the body outside the scroll: at least six lines, plus a quarter of what the window has
beyond the instruments and the graphs at their floor (the other three quarters go to the graphs, the
split the window had before — the live `diagnostics` expects a graph to take a quarter of any growth)
— with "Mark the moment", its
note field and "Open folder" on its head row; the rest scrolls. The class across every window is
`window-content-reach`.

## Rejected

- A BufWriter flushed on a timer: a timer is a wake-up at rest, and a kill loses the buffer.
- fsync per record: a power cut is not the complaint; an fsync per busy second stalls on a busy disk.
- Writing the journal into `kt-app.log`: that file is per run (the crash window reads one run) and
  rotates by run; the journal must read across a crash and its restart.
- A per-thread ring merged at read time: the order of records across threads would depend on reading.

## Decisions taken while applying

- **The journal speaks English, Diagnostics shows it as written.** The file is the evidence a person
  hands over (the render log's rule: "the file is for a report — English"); two languages in one
  file would make a report half unreadable to whoever reads it. The former Diagnostics notes were
  half English already ("render underruns", "prefetch thrash"); their catalog keys (`[perf.note]`)
  are gone. Diagnostics colours the person's marks and the problem notes.
- **The record's time is the event's; the file's order is the order of recording.** A note of a long
  UI frame is stamped at the frame's START and recorded at its end, so it can stand after a load
  second stamped later. Sorting would need a buffer, i.e. a delay; the reader sorts by the stamp.
- **The key is Ctrl+Alt+M in every style.** Ctrl+Shift+M would steal Premiere's and After Effects'
  Render through the lax lookup (an unbound Shift falls off: Ctrl+M). No style binds Ctrl+Alt+M and
  no modifier falls off onto it. It is added to the owner's own table too: a new chord, no existing
  binding moved (t1700 holds the old ones).
- **t1839 lost its "3 s ago" check on purpose.** That line asserted the relative stamp the owner
  complained about; it now asserts the opposite (no "ago"), and t1967 holds the report's clock time.
- **The underrun notes come from the playback black box's SNAP**, written by the render thread in the
  second they happen; the load watch's probe raises the cache and media notes.
- **Paths become file names inside the journal** (`file_names_only`): a notice says "Saved:
  /home/…/haski.kt"; the journal keeps "haski.kt". A path with spaces keeps a word of a directory at
  worst, never the whole path.
- **A notice is journaled once**: warnings and refusals through the `kt-app.log` mirror, plain notices
  in `note()`; the mark's own notice is not journaled again.

## Measured at rest (heavy showcase)

2026-11-08, the debug build (dev profile, opt 1) on Xvfb with llvmpipe, the project of
`kt-cli showcase --heavy` on `sources/showcase-media` (9 sources, 1357 events, Slowed, VHS Tape and
Camera ISP), opened and left 90 s to calm, then three windows of 30 s, read from `/proc`:

| Window | journal thread switches | journal file | load watch switches | whole process |
|---|---|---|---|---|
| rest | 0 | +0 bytes | 37 | 0.6 % of a core |
| pointer moving over the preview | 31 | +3869 bytes (a LOAD line per busy second) | 67 | 421 % (llvmpipe; the UI thread 80 %) |
| rest again | 0 | +0 bytes | 42 | 0.7 % |

The journal costs nothing at rest: no wake-up and no write. While busy it wakes once per busy second
and took no measurable processor time. The load watch's own second (one read of `/proc/self/task`)
is idle-cpu's and unchanged. The 421 % of a moving pointer over the preview is the cost of a UI frame
on this software device — `ui-frame-cost` (kkt-26.11.07b G1), not this change.
