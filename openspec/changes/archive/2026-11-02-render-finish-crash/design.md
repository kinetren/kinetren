# Design

## Context

See proposal.md — Why. The owner's report for this round carries no crash text: his journal, the
only place the crash would have been recorded, was rotated away by the tests (D3). The diagnosis is
therefore a reproduction.

- `fails/26.10.31/kt-app-crash.seen.txt` (his previous render crash, build kt26.10.31-4748ea8a):
  `cannot convert float seconds to Duration: value is negative` in `KtApp::draw_render_dialog`.
- That crash was fixed by `render-crash-and-recovery` (commit e72ea7a8, 2026-11-01 04:12): the
  announced total became `video_frames_in` (the frames from the one containing the region start) and
  the window clamps its fraction and builds every display `Duration` through `render_dialog::secs`.
  `git merge-base --is-ancestor e72ea7a8 a04b201d` is false: the owner's build of 26.11.01 predates
  the fix.
- Arithmetic of his region `rend` (733162…2522453 ticks, 24 fps = 3750 ticks a frame): it starts at
  frame 195.51, so the old announced total was ⌊1789291 / 3750⌋ = 477 while 478 frames were rendered.
  Any 0.2 s region (18000 ticks) announces 4 and renders 5 or 6 unless both edges sit on the grid —
  and a region drawn by hand never does: "even 0.2 s anywhere".
- The video phase reports progress every 10 frames; after it the counter is set to the rendered count
  and the audio phase reports it — the fraction passes 1 then, while the audio is being written, and
  the UI thread panics. The process dies with the encoder half-way: "it may even save the file".
- On master: `kt-cli export` of his project (paths repointed in a scratch copy) renders 1976 frames
  and exits 0; the real GUI (Xvfb, llvmpipe, his project's copy) renders the region `rend` to the
  finish, Close, and the process lives on.

## Goals / Non-Goals

**Goals:**
- Prove the cause on the old code and hold the owner's case end to end with a test that drives the
  REAL export through the window (the existing t1070/t1071 cover the counting and the window
  separately, with scripted numbers).
- No test touches the person's data directory again, with a guard that fails when a new path does.

**Non-Goals:**
- Repairing the owner's journal: the rotated records are gone.
- Joining the background job threads (`BgJob` is not joined by design; a finished render's thread
  has already dropped its sources, GPU engine and muxer before it sends the result).

## Decisions

### D1. The regression test drives the real engine through the window

t1312 sets a 24 fps project with a 0.2 s region a third of a frame off the grid, starts the render
the way the settings window does (`start_render` → `launch_render` through the `test_pick` seam),
and steps the harness until the result: every progress message the engine sends is drawn. A panic
anywhere in the window fails it; it also asserts the progress never passes the total and the rendered
count equals it (6; the owner's build announced 4), then presses Close and checks the file. With the
two halves of e72ea7a8 reverted (the floored total, the unclamped fraction with `from_secs_f64`), it
fails with the owner's own panic text — the reproduction recorded in tasks.md.

Rejected: a test of the counting alone (t1070 has it) or of the window alone with scripted numbers
(t1071): each held, yet neither is the path the owner walked; the end-to-end test is what would have
caught a NEW mismatch between the engine's stream and the window.

The live scenario `renderregion.py` walks the same path in the real binary on our synthetic fixture
and checks the supervisor opened no crash window and wrote no crash report: with the supervisor a
UI panic no longer ends the process the scenario started, so "the process is alive" alone would pass
a crash.

### D2. The last unguarded display `Duration`

`lessons.rs` built `Duration::from_secs_f64` for the joke timer drawn during a render. Its argument
cannot go negative today, but it is drawn in the same frames as the render window, and the class is
"a number from a clock becomes a panic": it goes through `render_dialog::secs` like every other
display duration.

### D3. One resolver, redirected in a test build

Every path of the program's data in kt-app already hangs off `journal::data_dir()` (journal, crash
report, autosave recovery, project locks, workspace via the user store, presets, recent and posters,
render log, telemetry and update-check state, eframe's `persistence_path`). The earlier fixes were
per site — `recovery_on_drop = false`, a temp `workspace_file`, a `cfg!(test)` render log dir,
`KT_USER_STORE_DIR` — and every new site missed: the journal itself, the locks and t959's log
warnings went into the owner's directory. So the redirection moves to the root: with `cfg!(test)`
`data_dir()` answers `test_data_dir()`:

- the absolute `KINETREN_DATA_DIR` if set — the supervisor tests start the test binary as a child
  with their own data folder and read its crash files there;
- otherwise `<temp>/kt-test-data-<pid>`, made once per process (a `OnceLock`). Folders of test
  processes that are gone (`/proc/<pid>` missing, Linux) are swept on the first call, so runs do not
  pile folders in `/tmp`. Per process, not per test: tests ran against one shared directory before
  and nothing relies on a private one; per process keeps parallel worktrees from sharing locks.

`ffmpeg_setup::download_root()` reads `kt_dirs::ffmpeg_download_root()` (the local directory) rather
than `data_dir`; in a test build it is `data_dir()/ffmpeg`. The render log's own `cfg!(test)` branch
goes: `data_dir()` is already safe.

Rejected:
- setting `KINETREN_DATA_DIR` for the whole test process at the first call: `set_var` in a
  multithreaded test binary races with every `getenv` of the other tests (it is `unsafe` in edition
  2024 for that reason), and there is no constructor before the harness starts the threads;
- teaching `kt-dirs` to detect a test: `cfg(test)` is per crate and a dependency is never built with
  it; a runtime guess (the binary path has `/deps/`) is the kind of key that goes stale silently.

The eframe test (`eframe_settings_file_is_where_eframe_puts_it`) compares the RESOLVER with eframe's
own choice — it calls `kt_dirs::data_dir()`, which is pure and touches nothing.

### D4. Two guards

t1310 checks the outcome: the test's `data_dir()` is not the resolver's, not inside it and not
containing it, lies in the temp directory, every derived path lies inside it, and a unique journal
line lands in the test's journal and not in the person's (read-only on the person's side). t1311
checks the cause: a source scan of `src/apps/kt-app/src` for direct requests of the person's
directories (`kt_dirs::data_dir(`, `local_dir(`, the download roots, `eframe::storage_dir(`,
`home_dir(`, `HOME`, `APPDATA`, `XDG_DATA_HOME`) with an exact allow-list and counts — the resolver,
the eframe test, the download root outside a test build. A new site that resolves on its own fails
it with its file and token.

kt-cli is out of the scan: it only READS the data directory (the FFmpeg choice, a lock holder), and
its tests that touch it already give the process its own `XDG_DATA_HOME`/`KINETREN_DATA_DIR`.

## Risks / Trade-offs

- [A test that relied on the shared real directory between processes] → none found; the supervisor
  tests pass their folder explicitly and are honoured.
- [Temp folders left by a crashed run on Windows (no sweep)] → small, the CI machine is fresh.
- [The owner may still see the crash if he runs an old build] → the What's new line names the fix;
  the report to him names the first build that has it.
