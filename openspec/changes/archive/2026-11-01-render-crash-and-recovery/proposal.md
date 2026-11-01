# Proposal

## Why

The owner, 2026-11-01 (`docs/ktprompts/kkt-26.11.01.txt` item 9, registry row K9; his log and
crash report in `fails/26.10.31`): "I was working in the 100 MB project — I just imported it and
did not save — I went to render and kt crashed. Fine, I open kt again — it offers to restore the
project — I agree, and it opens that 100 MB project, but the sources there are some synthetic ones
(SMPTE bars, noise and so on) … and the project is labelled lesson-5 … and it seems the log from an
old run stayed, i.e. we have no log rotation. And no crash window. And some bug with the recovery
or the state." His decision on the window (2026-11-01, `docs/user-feedback.md`): "I want the crash
window right after the crash, not who knows when".

Four defects, each confirmed in the code:

1. **The panic.** `Duration::from_secs_f64` of a negative number in the render progress window
   (`render_dialog.rs`, the "remaining" row): remaining = elapsed / frac · (1 − frac), and frac
   went above 1. The export floors the frame TOTAL (`(r1 − r0) / step`) while its loop renders
   from `align_floor(r0)` to `r1`: on a region whose edges are not on the frame grid the loop
   renders one frame more than the total it announced.
2. **No crash window.** The panic hook writes `kt-app-crash.txt`; only the NEXT start moves it to
   `.seen` and puts a line into the bell. A non-panic death (a signal, an abort, the OOM killer)
   leaves nothing at all.
3. **Recovery restores the wrong thing.** `save()` stores the lesson sandbox as `last_project`;
   the import leaves the document unnamed, so `last_project` still points to `lesson-5`, and the
   next start opens it before offering the recovery. `restore_recovery` then swaps only the model:
   the player keeps the lesson's sources at the same media numbers (the first media sync after an
   open only snapshots paths), so the restored edit plays the lesson's generators, and the title
   still says `lesson-5`.
4. **The log.** `kt-app.log` appends across runs, rotates only at 1 MiB, has no per-run header,
   and stamps lines in UTC without saying so.

## What Changes

- **The export announces exactly the frames it renders**: one function counts the frames of a
  range on the project grid, and both the progress total and the render loop use it. The
  progress window's estimates (remaining time, final size) clamp the fraction and never build a
  `Duration` from a negative or non-finite number; the FFmpeg download's ETA gets the same guard.
- **A crash window right after any abnormal exit.** `kinetren` starts as a thin supervisor that
  runs the editor as its child (the same binary) and waits. When the child dies abnormally (a
  panic, an abort, a fatal signal, a non-zero exit) the supervisor opens a small window: what
  happened (the panic message or the signal in words), the build version, the tail of the crashed
  run's log, and the buttons **Restore the project** (starts the editor again, which offers the
  recovery), **Open the log folder** and **Close**. A clean exit or a stop on request (SIGTERM,
  SIGINT, SIGHUP) shows nothing. On Linux the child dies with the supervisor (`PR_SET_PDEATHSIG`).
  `KT_NO_SUPERVISOR=1` runs the editor directly (debuggers).
- **The next start does not repeat a crash the window already showed**, but the report is still
  taken (moved to `.seen`) and handed to telemetry as before.
- **Recovery restores what was written.** A lesson sandbox never becomes `last_project` (neither
  when stored nor when read back). "Restore" builds the editor from the recovery file the same way
  an Open does: the player and every source are rebuilt from the restored document, and the
  document takes the name the recovery file belongs to (the project, or none for unnamed work).
- **The log is per run.** Each start moves the previous run's log to `kt-app.log.1` (up to four
  previous runs kept) and opens the new one with a header: version, OS and architecture, FFmpeg
  major, pid, and the start time in local time with its UTC offset. Every line carries local time
  with the offset. The crash window shows only the lines of the run that crashed.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `export`: the frame total of a range equals the frames the loop renders; the progress window's
  estimates are total-safe.
- `durability`: the log and crash report requirement is rewritten (per-run log, the supervisor and
  its crash window, a report the window showed is not announced again); recovery restores sources
  and the name, and a sandbox never becomes `last_project`.

## Impact

- `src/libs/kt-engine/src/export.rs` — `video_frames_in`, used by the total and the loop.
- `src/apps/kt-app/src/render_dialog.rs`, `ffmpeg_setup.rs` — the guarded estimates.
- `src/apps/kt-app/src/supervisor.rs` (new), `main.rs` — the supervisor and the crash window.
- `src/apps/kt-app/src/journal.rs` — per-run log, local time with offset, the shown mark.
- `src/apps/kt-app/src/app.rs` — `save()`, `restore_recovery`, the start notice; `welcome.rs` —
  `plan_start` drops a sandbox `last_project`; `bench.rs` — a `crash` command for the live
  scenario (only with `KT_LIVE_CTL`).
- `src/apps/kt-app/Cargo.toml` — `libc` for Linux (`prctl`, `localtime_r`; already in the lock
  file, links nothing new), `windows-sys` feature `Win32_System_Time` for the offset on Windows.
- Catalogs: a `[crash]` section in all five languages.
- Live scenario `src/scripts/live/crash.py`.
- Test numbers t1070–t1084 are reserved for this change.
