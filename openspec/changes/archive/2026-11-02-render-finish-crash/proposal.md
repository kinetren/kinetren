# Proposal

## Why

The owner (kkt-26.11.01b item 2, registry row B1): rendering the region `rend` of
`fails/26.11.01/render_crash/posss.kt` — or any 0.2 s piece anywhere — reaches 100 % and the whole
program folds; "and it may even save the file. This is horror." His build was `26.11.01-a04b201d`.

Found while registering: the tests write into the person's own data directory. The owner's journal
`~/.local/share/kinetren/kt-app.log` is full of `RENDER render log not written (/tmp/kt-t959-…)` lines
and rotated at 14:09, so his real records are gone; the same directory holds 7934 lock files, a
recovery file and a workspace written by tests.

## What Changes

- **The crash, diagnosed.** The cause is the panic of 2026-10-31 (`Duration::from_secs_f64` of a
  negative remaining time in the render window): an off-grid region renders more frames than the
  floored count it announced, the fraction passes 1 during the audio phase and the UI thread panics
  — after the video is written, which is why the file is sometimes complete. Every hand-drawn region
  is off the 24 fps grid, hence "0.2 s anywhere". It was fixed by `render-crash-and-recovery`
  (commit e72ea7a8), merged AFTER the owner's build a04b201d. Reproduced on the old code by a new
  end-to-end test (real export → render window → Close), which fails with the owner's panic text and
  passes on master; the owner's project renders through the real GUI path to the end on master.
- An end-to-end regression test of the owner's case (t1312) and a live scenario
  (`src/scripts/live/renderregion.py`, our fixture): a real render of a 0.2 s off-grid region at
  24 fps through the window, to the finish, Close, and the program draws on without a crash window.
- The last `Duration::from_secs_f64` in a UI draw path (the lesson joke timer) goes through the
  guarded `render_dialog::secs`.
- **Tests never touch the person's data directory.** In a test build `journal::data_dir()` — the one
  root of the journal, crash report, recovery, locks, workspace, presets, posters, render log and
  eframe storage — answers with a per-process temp folder (`kt-test-data-<pid>`, folders of dead test
  processes swept), or with `KINETREN_DATA_DIR` when the test that started the process set it. The
  FFmpeg download root follows it in a test build. A guard test checks the redirection and that one
  of the test's journal lines lands in its own journal, not the person's (t1310); a source guard
  forbids asking `kt-dirs` or eframe for the person's directories anywhere but `data_dir` (t1311).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `export`: a render of any region finishes and the program lives on after it — the owner's case
  end to end (the existing requirement "The announced frame total is the frames the export renders"
  is unchanged; a new requirement holds the end-to-end path).
- `durability`: a test process never reads or writes the person's data directory.

## Impact

- `src/apps/kt-app/src/journal.rs` (`data_dir` test redirection, t1310, t1311),
  `ffmpeg_setup.rs` (`download_root` in a test build), `render_dialog.rs` (the render log follows
  `data_dir`), `lessons.rs` (guarded duration), `layout_tests.rs` (t1312).
- `src/scripts/live/renderregion.py` (new live scenario).
- No model, format, engine or dependency change. The owner's journal itself is not repaired: the
  rotated-away records are lost; from this change on no test writes there.
