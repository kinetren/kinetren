# Proposal

## Why

The owner (2026-10-27, `docs/user-feedback.md`, kkt item 4): "a very sparse render window — the
C++ version used to have a fuller one; at the least bring everything up to its level of
information". Today the progress window (`src/apps/kt-app/src/render_dialog.rs`,
`draw_render_dialog`) shows one grey settings line that calls the encoder "auto encoder" until the
end, one counters line, no chart, no log, and after the finish it keeps counting (ktprompts v26.10
T12). It also writes over an existing file without a word (T U1). Both claims are assigned to this
change by the R.71 index (K5 takes U1 and T12). Feature note: F-EXP-005
(`docs/features/export/pipeline/render-progress-window.md`).

## What Changes

- **The engine reports what it opened and what it wrote** (counts only, no wall clock —
  invariant 11): before the first frame, the encoder that actually opened (name, hardware or
  not, pixel format, the options it was opened with) and every candidate tried before it with its
  error; during the run, the video and audio bytes written; along the way, log lines (sources
  that became placeholders, phase changes, the finalize step, errors). Today the encoder is known
  only in the final stats although the muxer chose it before the first frame.
- **The progress window gets v1's information**, laid out as v1 did: an *Output* form —
  resolution, frame rate, video (encoder + pixel format + its quality knob, named from the run, not
  the fixed string v1 printed), audio, mux, and new: the composition backend (GPU with the
  adapter name, or CPU with the reason) and the output file; a *Run* form — phase, progress
  done/total, elapsed, remaining, output length; the min/avg/cur/max speed line plus a speed
  chart (the performance panel's rolling graph, not a new widget); the file size so far, the
  estimated final size and the current bitrate (computed from bytes per second of OUTPUT).
- **A log inside the window**: a collapsible section (v1: an 800-line panel), each line stamped
  by the UI with the elapsed time; the same lines go to `<data_dir>/render.log` (the previous run
  kept as `render.log.1`), whose path the window shows. No floating log window (kt-app rule 22).
- **The end of the run is frozen and actionable**: elapsed, speed and remaining stop at the
  finish (T12); a final block names total time, average speed, frames, encoder, backend, file
  size and average bitrate next to the existing loudness/missing lines; **Open file** and **Open
  folder** buttons; Close is never placed where Cancel was; Cancel is disabled while the file is
  being finalized.
- **Writing over an existing file is asked first** (U1): when the chosen output path exists, the
  program's own question (the `Ask` set) names the file, its size and date, and offers Replace /
  Choose another name; nothing is deleted until Replace.
- Translation catalogs (5 languages) for every new text; a live scenario with a screenshot per
  step.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `export`: adds requirements for what the export run reports (the opened encoder before the
  first frame, byte counts, log lines — all deterministic), what the render progress window shows
  during and after the run, the render log file, and the question before overwriting an
  existing output file.

## Impact

- Code: `src/libs/kt-ffmpeg/src/muxer_impl.rs` and `src/libs/kt-ffmpeg/src/lib.rs` (open report,
  per-stream byte counters), `src/libs/kt-media/src/muxer.rs` (re-export),
  `src/libs/kt-engine/src/export.rs` (`ExportEvent`, `export_mp4_with_backends` takes an event
  callback; `export_mp4_with` keeps its progress-only callback so existing tests stay),
  `src/apps/kt-cli/src/main.rs` (prints the log lines to stderr),
  `src/apps/kt-app/src/bg.rs` (visit every message, not only the last),
  `src/apps/kt-app/src/render_dialog.rs` (the two forms, chart, log, final block, buttons),
  `src/apps/kt-app/src/app.rs` (`Ask::OverwriteRender`, `export_video_gpu` returns its reason
  instead of `eprintln!`), a small OS "open file / reveal folder" helper in kt-app,
  `src/apps/kt-app/i18n/*.toml`.
- No new crate dependency; no project-format change; no change to the encoder cascade (that is
  `hw-encode-context`, which this change's encoder line will name).
- Invariants: 9 (the encoder named from the real open), 11 (the engine reports counts, the UI
  owns every clock), 12 (the chart repaints only while the job runs). No new window in the
  `WINDOWS` registry (t757).
