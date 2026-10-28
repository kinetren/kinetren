# Design

## Context

See proposal.md — Why. Observed in the tree (2026-10-28):

- **The encoder is chosen before the first frame, but nobody is told.** `export_mp4_with_backends`
  (`src/libs/kt-engine/src/export.rs:416`) opens the muxer (`:453`) before the video phase; the
  cascade runs inside `Muxer::open` (`src/libs/kt-ffmpeg/src/muxer_impl.rs:120-196`), which keeps
  only the LAST failure text (`last_err`) and returns the name; `encoder_name()` is read only at
  `:538`, after both phases, into `ExportStats.encoder`. The window shows
  `force_encoder.unwrap_or("auto encoder")` (`src/apps/kt-app/src/render_dialog.rs:514`).
- **Progress is `Copy` counts and a phase** (`ExportProgress`, `export.rs:26-33`); the comment
  already says ETA/speed belong to the UI. Video progress is sent every 10 frames (`:497`), audio
  about once per second of samples (`:519`).
- **The background job keeps only the last message.** `BgJob::poll` (`src/apps/kt-app/src/bg.rs:115`)
  overwrites `progress` in a loop: a log line between two progress steps would be lost.
- **The window** (`render_dialog.rs:492-680`): one grey line of settings (`:505-514`), phase, bar,
  speed min/avg/cur/max from 0.5 s deltas (`:539-563`), a counters line with elapsed and ETA
  (`:564-578`); `elapsed` is `job.job.started.elapsed()` every frame (`:531`), so after Done it keeps
  growing — T12. The window does not use `widgets::window_body`/`window_actions` (the R.66 form
  contract), unlike the settings window (`:301`, `:465`).
- **Backend**: `crate::app::export_video_gpu()` (`src/apps/kt-app/src/app.rs:9061-9072`) runs inside
  the job thread and reports a GPU failure with `eprintln!` only.
- **Output path**: `launch_render` (`render_dialog.rs:191`) takes `pick_save` (`app.rs:8667`; the
  `KT_FILE_PICK` seam returns the env path as is) and starts at once; `avio_open` with
  `AVIO_FLAG_WRITE` truncates an existing file, and on error/cancel `cleanup_on_err` removes the
  path (`export.rs:487-490`) — so a failed render over an old file deletes the old file too.
- **Widgets to reuse**: `graphs::RollingGraph` (`src/apps/kt-app/src/graphs.rs:29`, 180-sample
  ring, `LinearAuto` axis) — the v1 `SpeedChartWidget` was a `MultiLineGraphWidget` with the same
  axis style; the `Ask` set (`app.rs:27`) drawn in one place (`app.rs:1949`); the journal's
  one-step rotation (`journal.rs:195`).
- **v1** (`git show cpp-reference-final:renderprogressdialog.cpp`): two `QFormLayout`s (Output:
  Resolution, Frame rate, Video, Audio, Mux; Run: Phase, Progress, Elapsed, Remaining, Output len),
  the speed line, a `SpeedChartWidget`, the bar, Pause/Cancel, a log button that shows an 800-line
  panel or a floating copy; OK replaces Pause/Cancel at the end. `setExportInfo` printed a FIXED
  video string — not something to copy.

## Goals / Non-Goals

**Goals:**
- Every row the window shows is true at every frame of the run (the encoder from the open report,
  the backend from the job, sizes from counted bytes).
- The engine stays deterministic: events are counts; all clocks live in kt-app.
- No new window, no new dependency.

**Non-Goals:**
- Free disk space (After Effects' pair, F-EXP-005 trick 2): it needs a per-OS file-system call;
  the estimated final size is shown so the person can compare. Deferred.
- T13 (video 517/516, audio frame count mismatch) — a counting defect of the export itself, R.74.
- T14/T15 (loudness norms, a video analysis) and T18 (other containers/codecs) — separate items.
- Changing the encoder cascade or making VAAPI/QSV open — `hw-encode-context`.
- A render queue (F-EXP-002).

## Decisions

### D1. An event callback in the engine: `ExportEvent`
`export_mp4_with_backends` takes `impl Fn(ExportEvent) + Send + Sync`, where
`ExportEvent = Opened(EncoderReport) | Progress(ExportProgress) | Log(ExportLog)`.
`ExportProgress` stays `Copy` and gains `video_bytes`/`audio_bytes`. `EncoderReport` = name,
hardware, pixel format name, the options as `(key, value)` pairs, and `tried: Vec<(name, code)>`.
`ExportLog` is an enum of facts (`NotOpened{name, code}`, `Placeholder{label}`, `Phase(…)`,
`Finalizing`, `Error(text)`), not preformatted text, so the UI translates it and t955 compares
values. `export_mp4_with` keeps `impl Fn(ExportProgress)` (filters the events), so the four call
sites in `src/libs/kt-engine/tests/export_options.rs` and `export_mp4` stay as they are.
Alternatives: making `ExportProgress` non-`Copy` with an `Option<Arc<EncoderReport>>` (every step
carries a pointer that is set once — a state, not an event); a second callback parameter for logs
(two channels whose order relative to each other is lost).

### D2. The open report and byte counts come from the muxer
`Muxer::open` collects `tried` (every candidate that exists in the build and failed, with its
code — today only `last_err` survives) and keeps the opened candidate's options; a new
`Muxer::open_report()` returns them. `drain` adds `pkt.size` to a per-stream counter before
`av_interleaved_write_frame`; `Muxer::bytes() -> (video, audio)`. Packet bytes, not
`avio_tell`: they are per stream (the bitrate row is the VIDEO bitrate) and do not jump at the
`+faststart` rewrite. The sum is ≤ the file size (headers and `moov` are extra) — t954.
Alternative: the UI reading `metadata(out_path).len()` — mixes container overhead into the
bitrate and is not available to kt-cli or tests.

### D3. The UI owns every clock
`RenderJob` keeps `started`, `finished_at: Option<Instant>` (set in the poll where the result
arrives), and the speed samples. Elapsed = `finished_at.unwrap_or(now) - started`; remaining is
computed only while running; the speed sample (output seconds per wall second over ≥ 0.5 s) feeds
both the min/avg/cur/max line and `RollingGraph::push`. A `Clock` seam (`fn now()`) is injected in
the harness so t957 can draw frames at 11 s and 15 s without sleeping. Size so far =
`video_bytes + audio_bytes`; estimated final = so-far × total/done frames (shown after 2 % of the
frames); current bitrate = Δvideo_bytes × 8 / Δ(frames/fps) between two speed samples — a ratio of
counts, no wall clock in it.

### D4. `BgJob::poll_with` visits every message
`poll_with(|msg| …)` drains the channel calling the closure per message and still records `done`;
`poll` stays (other jobs use it). The render job's message type is a kt-app enum
`RenderMsg = Engine(ExportEvent) | Backend(BackendLine) | Missing(Vec<String>)`: the job thread
sends the backend choice (adapter name from `GpuContext::adapter_info`, or the error text that
`export_video_gpu` returns instead of printing it) and the placeholder list `open_all` already
computes (today dropped as `_`).

### D5. The window layout: two forms, the chart, the log section, actions outside
`widgets::window_body` + `window_actions` (the R.66 contract, t779): inside the body, a two-column
`Grid` per form (key right-aligned in the weak colour, value in the monospace style the
performance panel uses), placed side by side when the window is ≥ 2 × the form width, otherwise one
under the other (rule 3: the width does not jump between states — both forms are sized by their
longest value variant). Then the speed line, the chart (`RollingGraph`, one series, `LinearAuto`,
height 120), the bar, the log section (`CollapsingHeader`, default closed, forced open on error).
Actions row: while running — Pause/Resume, Cancel (disabled in Finalize with the hint via
`KtApp::hint`, rule 10); at the end — Open file, Open folder, Open log (on error), and Close at the
far right, a different place from Cancel (trick 3). `default_size` grows to fit both forms; the
window stays resizable with a max size (rule 6).

### D6. The final block replaces the run rows, the numbers are frozen
At Done the Run form switches to the final rows (total time, average speed, frames/samples,
encoder, backend, file size from `metadata` once, average bitrate), followed by the existing
loudness / missing / lost-layer lines (unchanged texts). Repaint: `is_running()` already gates
the continuous repaint; nothing in the final state animates (invariant 12, t38 family).

### D7. Open file / Open folder: one small OS helper
`os_open::open_path(path)` and `os_open::reveal_dir(path)`: on Linux `xdg-open <file|dir>`
spawned and not waited for (inside Flatpak, `xdg-open` forwards to the OpenURI portal); on Windows
`ShellExecuteW("open", …)` (the `Win32_UI_Shell` feature of the existing `windows-sys`). A spawn
failure → an error notice with the path (rule 16). This is UI, not export: invariant 9 ("no
external process") is about the export path and is untouched. No crate is added.
Alternatives: the `opener`/`open` crates (a dependency for two calls); `explorer /select,`
(selects the file but is a second code path per OS — the folder is enough).

### D8. The overwrite question is an `Ask`
`Ask::OverwriteRender { path, size, modified }`, drawn where the other questions are
(`app.rs:1949`), answers `Yes` = Replace (→ start with that path), `No` = Choose another name (→
`pick_save` again with the same default name), `Cancel` = back to the settings window (it was
closed by `launch_render`; it is reopened with its state). The check is `Path::exists` right after
`pick_save` returns. Asked always, including when the native dialog already asked (see Open
Questions 1): the native question is not guaranteed (the `KT_FILE_PICK` seam, portals that do not
confirm), and ours names size and date.

### D9. The log: a ring of 800 lines in the job, two files on disk
`RenderJob.log: VecDeque<(Duration, LogLine)>` capped at 800 (v1's `setMaximumBlockCount(800)`).
The file `<data_dir>/render.log` is opened at start after renaming an existing one to
`render.log.1`; each line is appended as it arrives (buffered, flushed on phase change and at the
end), so a crash in libav (T17) still leaves the lines before it on disk. A write error → one
`journal::log` line and a flag that the window shows. The lines are written in English
(the journal's rule: the file is for a report), the window shows them translated.

## Risks / Trade-offs

- [The window grows and becomes the "wall of numbers" the owner did not ask for] → v1's level is
  the request; the log is collapsed, the chart is one series, the final block replaces the run
  rows instead of adding to them.
- [A double overwrite question where the native dialog already asked] → accepted for now; Open
  Question 1.
- [`xdg-open` missing on a minimal system] → the error notice names the path, so the person can
  open it by hand; the render itself is unaffected.
- [Event volume: a log line per placeholder on a project with hundreds of missing files] → the
  engine sends one `Placeholder` per source, the ring keeps 800; the file keeps all.
- [The speed chart heats the machine] → it is pushed only on speed samples (2 Hz) and the repaint
  is gated by `is_running()`; t38 stays green.

## Migration Plan

No data migration. `render.log` appears on the first render. Rollback = revert; the engine's
`export_mp4_with` signature is unchanged for tests.

## Open Questions

1. **Ask even when the system dialog already asked?** GTK/portal save dialogs usually confirm
   overwriting; on such systems the person would answer twice. Recommended: ask always (one
   behaviour on every OS and in the live rig, and ours names size and date); if the owner finds it
   double, skip ours when the path came from the native dialog on Linux.
2. **How many render logs to keep?** Recommended: two (`render.log`, `render.log.1`), like the
   journal; a per-render file next to the output was rejected (litters the person's video folder).
3. **Free disk space next to the estimated size** (After Effects)? Recommended: later, as a
   separate small item — it needs a per-OS call.
