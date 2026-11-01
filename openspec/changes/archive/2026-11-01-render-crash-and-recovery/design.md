# Design

## Context

The owner's crash (kt26.10.31-4748ea8a, `fails/26.10.31/kt-app-crash.seen.txt`):

```
panic: panicked at …/core/src/time.rs:962:23:
cannot convert float seconds to Duration: value is negative
   6: <core::time::Duration>::from_secs_f64
   7: <kinetren::app::KtApp>::draw_render_dialog
```

The chain behind the rest of the complaint, read from the code at 4b886b1a:

- lesson 5 open → `save()` (every 30 s) writes `last_project = …/lessons/lesson-5.kt`;
- File → Import → `open_older` → `project_path = None` (the document is unnamed): `save()` stores
  nothing, so `last_project` keeps `lesson-5`; the recent list never holds a sandbox
  (`note_recent`), so at the next start `plan_start` falls back to `last_project`;
- the crash; the next start: recovery is pending → the "former start" opens `lesson-5.kt`, and the
  recovery dialog offers the unnamed autosave;
- "Restore" → `restore_recovery` replaces `self.model` and calls `sync_media_sources`; since
  nothing had edited the opened lesson yet, `media_synced` is false, and the first sync only
  snapshots the paths ("the sources have already been raised by build_player"). The player keeps
  the lesson's sources at media numbers 1…n, the imported edit's media numbers are also 1…n:
  the edit plays SMPTE bars and noise. `project_path` stays `lesson-5.kt`, hence the title, and a
  Ctrl+S would have written the imported edit over the lesson.

## Decisions

### D1. One count of a range's frames, the estimates clamp

`kt_engine::export::video_frames_in(fps, r0, r1)` = the number of grid ticks `t` with
`align_floor(r0) ≤ t < r1` = ⌈(r1 − align_floor(r0)) / step⌉, zero for an empty range. The total
in `ExportProgress` and the loop of `render_raw_video_range_with` both start from
`align_floor(r0)`; the total now says so by construction. The loop is not changed: its first frame
is the frame that CONTAINS `r0` — the same frame the preview shows at `r0` (invariant 7), so
moving the loop to `align_ceil` would drop a frame the person sees.

The window still clamps (`frac ∈ [0, 1]`, `Duration::try_from_secs_f64(..).unwrap_or_default()`):
a counter mismatch must cost a wrong estimate, never the program. The same class sits in the
FFmpeg download ETA (`ffmpeg_setup::eta`: a tiny positive rate overflows the `Duration`), guarded
the same way.

### D2. The supervisor: the same binary, run twice

`main()` asks `supervisor::run()` first. Without `KT_SUPERVISED` in the environment the process
is the SUPERVISOR: it starts `current_exe()` with the same arguments and `KT_SUPERVISED=1`,
inherits stdin/stdout/stderr, and waits. It initialises nothing else — no eframe, no GPU, no
FFmpeg, no telemetry — so it costs one more `exec` of a binary the page cache already holds. The
child is the editor exactly as before.

Classification of the child's exit (`supervisor::classify`, a pure function):

| exit | verdict |
|---|---|
| code 0 | clean — the supervisor exits 0 |
| SIGTERM, SIGINT, SIGHUP (Linux) | stopped on request — no window, the supervisor exits with 128 + signal |
| any other signal (SIGSEGV, SIGABRT, SIGBUS, SIGILL, SIGFPE, SIGKILL …) | crash |
| any non-zero code (101 = a Rust panic; on Windows the NTSTATUS codes 0xC0000005, 0xC00000FD, 0xC0000409 …) | crash |

On a crash the supervisor gathers what happened:
- the panic report `kt-app-crash.txt` if it is FRESH (modified after the child started) — the
  panic text is its `panic:` line and the line after it;
- otherwise it writes the report itself in the same format (`kinetren <version>`, `time:`,
  `exit: …`, the log tail), so "the last report is always in place" holds for signals too;
- the log lines of the crashed run (D5).

It then marks the report as shown (a `shown:` line appended) and opens the crash window (D3).
"Restore the project" starts the child again (a loop in the supervisor) with `KT_AFTER_CRASH=1`;
"Close" and the window's cross end the supervisor with the child's code.

**The child dies with the supervisor** (Linux): `pre_exec` sets `PR_SET_PDEATHSIG(SIGTERM)` and
exits at once if the parent already changed. Without this, `kill <pid>` on what the person (or a
live scenario, or the session manager) sees as "the program" would leave the editor running with
nobody watching. On Windows a killed supervisor leaves the editor running unwatched; nothing is
lost (the editor is the same program as before this change), so a job object is not added.

`KT_NO_SUPERVISOR=1` runs the editor directly in the first process — for a debugger, which would
otherwise attach to the supervisor.

Alternatives considered:

- **A window from the panic hook, in the dying process.** Rejected: the hook runs on whatever
  thread panicked, possibly holding the egui context or the GPU device; the event loop that
  would draw a window is the one that is unwinding; a signal (SIGSEGV, the OOM killer's SIGKILL)
  or an abort never reaches a hook at all. The window would appear sometimes, which is worse
  than never.
- **systemd-coredump / Windows Error Reporting.** Rejected: they record a dump for a developer,
  show the person nothing in the program's words, exist on one OS each, do not run in the
  portable zip's setting, and do not know about our recovery.
- **A separate `kinetren-crash` binary.** Rejected: a second file to ship and to keep at the same
  version in the tarball, the Windows zip, the Flatpak manifest and the desktop file, while the
  main binary already links eframe, the fonts and the catalogs the window needs. A mode of the
  same binary has none of that, and `dist-check.py` stays as it is.
- **The crash window at the next start (today's notice, made into a window).** Rejected by the
  owner: "right after the crash, not who knows when".

### D3. The crash window

A root eframe window of its own (the supervisor is a different process — it is not a `KtApp`
window, so the `WINDOWS` registry and the shell do not apply; its id is `kt_crash`). The same app
id as the editor (`APP_ID`), so the taskbar ties it to the program. Body, scrolling: the headline,
"what happened" in words, the version, the report path, the log tail in monospace. Buttons OUTSIDE
the scroll (the form contract of R.66): Restore the project (primary), Open the log folder (the
data directory, `util::open_in_file_manager`), Close. The language is the editor's: `KT_LANG`, else
the `lang` entry the editor stored in `app.ron` (read as text, not through eframe: the window must
not write eframe's storage — it would overwrite the editor's window geometry), else the system
locale. The theme and the fonts are the editor's (`theme::apply`, `icons::install_fonts`). It
repaints only on input (invariant 12). Under `KT_GEOM_DUMP` it writes `<dump>.crash.json` with
its button rectangles and texts — a live scenario reads it the way it reads the editor's `fields`.

### D4. A shown crash is not announced again; telemetry still learns it

The start still calls `take_crash_report` (moves the report to `.seen`, hands it to telemetry's
`app_crashed`, unchanged), but the bell line "the previous session ended abnormally" is said only
when the report has no `shown:` line — that is, when no window could show it (`KT_NO_SUPERVISOR`,
a supervisor that could not open a window, a killed supervisor). With `KT_AFTER_CRASH=1` the
recovery offer is not quiet: if nothing waits, the bell says "nothing to recover" — the person
pressed "Restore" and must hear an answer (rule 16).

### D5. The log is per run

`journal::start_run(info)` is called once at the editor's start: it shifts the fan
`kt-app.log.3 → .4`, …, `kt-app.log → .1` (four previous runs; the owner: "we have no log
rotation") and writes the header

```
==== kinetren kt26.11.01-abcdef12 · run 2026-11-01 18:10:12+03:00 · pid 4242 · linux x86_64 · FFmpeg 7 ====
```

The size threshold stays (1 MiB) and shifts the same fan, so one run never grows without bound.
Every line is stamped in LOCAL time with its offset (`2026-11-01 18:10:12+03:00`). The offset
comes from `localtime_r` (`tm_gmtoff`, libc — already in the lock file, a binding to glibc the
binary links anyway: nothing new in `ldd`, invariant 10) and on Windows from
`GetTimeZoneInformation` (`windows-sys`, one feature). No chrono for one line, as before.
`journal::last_run_tail(path, n)` returns the last `n` lines after the last header — what the crash
window shows. A second copy of the editor started while the first runs shifts the first's log too;
the header separates the runs, and the tail is still the last run's. Accepted: two copies are a
degraded mode already (the project lock says so).

### D6. Recovery restores the sources and the name

`restore_recovery` builds a new editor from the loaded document — `open_loaded` for a project's
recovery (base directory and index next to the project, `project_path` = that project, its lock
claimed after the swap), `new_with_project` for unnamed work — and installs it with `replace_with`
(the start state, the poster, the recent list travel as for any Open). The player, every source
and the thumbnail and index services are built from the restored document, so nothing of the
previous document can answer at the same media number. The restored editor keeps the asterisk.
If the document being replaced would write its own recovery on drop into the very file being
restored, that write is switched off (it would overwrite the source of the restore with older
work).

A lesson sandbox never becomes `last_project`: `save()` does not store it, and `plan_start` drops
one that an older build stored. The lesson panel opens lessons itself; reopening a sandbox at
start is never what the person asked for.

### D7. The live scenario crashes the program on purpose

The bench command `crash` (only with `KT_LIVE_CTL`, like every bench command) panics on the UI
thread — the place the owner's panic came from. No environment variable or menu item can crash a
normal run.

## Risks

- **Two processes in `ps`/the task manager.** The supervisor is ~1 MB of private memory; it is
  named the same. Documented in the guide's FAQ line in What's new.
- **A crash window that cannot open** (no display): the supervisor prints the report path to
  stderr and exits with the child's code; the next start announces the crash in the bell (D4).
- **Live scenarios kill "the program" with SIGTERM/SIGKILL.** With PDEATHSIG the editor follows;
  `recover.py` (SIGKILL of the started process) keeps its meaning — the editor dies without
  `on_exit` or `Drop`.

## Not done here

- A Windows job object (D2) — the killed-supervisor case leaves the editor running, nothing lost.
- Sending the report anywhere: there is no addressee (F-HLP-014); telemetry's `app_crashed` is
  unchanged.
