## ADDED Requirements

### Requirement: The export run reports what it opened and what it wrote, in counts only
An in-process MP4 export (`export_mp4_with_backends`) SHALL report, through its event callback:
(1) once, BEFORE the first video frame is delivered, the video encoder that actually opened — its
name, whether it is a hardware encoder, the pixel format it is fed, and the options it was opened
with — together with every candidate tried before it and the error code each returned; (2) with
every progress step, besides the frame/sample counters and the phase, the number of encoded video
bytes and audio bytes handed to the muxer so far; (3) log lines: each candidate that did not open,
each source rendered as a placeholder, each phase change, the start of finalization and any error.
The opened encoder's name SHALL equal `ExportStats.encoder`. No event SHALL carry a wall-clock
value: two runs of the same export on the same machine SHALL produce the same sequence of events
(the time of each step, the elapsed time, the speed and the remaining time are computed by the
caller). `kt-cli export` SHALL print the log lines to stderr.
Why: the muxer chooses the encoder before the first frame (`Muxer::open` in
`src/libs/kt-engine/src/export.rs`), but today only the final stats name it, so the progress
window calls it "auto encoder" for the whole run, and a silent fall-back from hardware to
software is learnt only at the end (F-EXP-004 Cons). Invariant 11 forbids the wall clock in the
offline path: counts are deterministic, clocks are not.

#### Scenario: The encoder is named before the first frame and the bytes add up
- **WHEN** a 60-frame export at 30 fps is run with the encoder forced to `libx264`, and again with
  the encoder left to the cascade
- **THEN** in both runs the first event that is not a log line is the opened-encoder report and it
  precedes every video progress step; with `libx264` it names `libx264`, software, `yuv420p` and
  options containing `crf`; with the cascade its name equals `ExportStats.encoder` and the log
  holds one "not opened" line (name and error code) for every candidate of the OS list placed
  before it that exists in the loaded FFmpeg; the video and audio byte counters never decrease, the
  last ones are greater than zero, and their sum is not larger than the size of the finished file
- **TEST** t954 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: The event stream holds no clock
- **WHEN** the same project is exported twice with `libx264` into two files and both event streams
  are recorded
- **THEN** the two streams are equal element by element (encoder report, every progress step with
  its counters and bytes, every log line), and no log line contains a time of day or a duration
- **TEST** t955 (`src/libs/kt-engine/tests/export_options.rs`)

### Requirement: The render progress window shows the output settings, the run and the speed
While a render runs, the progress window SHALL show two labelled forms side by side (one under the
other when the window is narrower than both): **Output** — resolution, frame rate, video (the name
of the encoder that opened, its pixel format and its quality setting under the name that encoder
uses — CRF, CQ, QP or global quality — or the target bitrate), audio (codec, sample rate, channels,
bitrate), mux (container and `+faststart`), backend (GPU with the adapter name, or CPU with the
reason a GPU was not used) and the output file path; **Run** — phase, progress as done/total
frames, elapsed, remaining, output length rendered/total, file size so far, the estimated final
size and the current video bitrate. Until the encoder report arrives the video row SHALL say that
the encoder is being chosen, never a guessed name. Below the forms: the speed line (min, avg, cur,
max × real time) and a speed chart drawn by the program's rolling graph; then the progress bar;
the Pause/Resume and Cancel buttons SHALL stay outside the scrolling body. The window SHALL
repaint only while the job runs (invariant 12).
Why: the owner's request to bring the window at least to v1's level of information (2026-10-27);
v1's `RenderProgressDialog` had these forms and a speed chart
(`git show cpp-reference-final:renderprogressdialog.cpp`), but its video row was a fixed string
"H.264 (libx264), yuv420p, preset veryfast, CRF 23" — we take the rows, not the fixed text
(F-EXP-005 trick 1). The quality knob means a different regulator per encoder (F-EXP-004 trick 3).
Speed and the bitrate are per second of OUTPUT so they are comparable between projects.

#### Scenario: The rows come from the run
- **WHEN** the harness opens the progress window of a job fed scripted engine events: first only a
  progress step, then the encoder report `h264_nvenc` (hardware, `yuv420p`, `cq 23`), then
  progress steps with byte counts, with the backend "GPU: llvmpipe"
- **THEN** before the report the video row says the encoder is being chosen and contains no
  encoder name; after it the Output form reads the project's resolution and frame rate, "h264_nvenc"
  with "hardware", "yuv420p" and "CQ 23" (not "CRF"), "AAC, 48 kHz stereo" with the bitrate,
  "MP4 (+faststart)", "GPU: llvmpipe" and the file path; the Run form has the phase, "N / M" frames,
  output length, the size and the bitrate computed from the scripted bytes and frames; the speed
  chart rectangle is published and non-empty; Pause and Cancel lie outside the scrolling body; no
  window outside the `WINDOWS` registry is open
- **PATH** Render… → Render settings → Render… → save dialog → name → the progress window: on
  the left "Output" with resolution, fps, "Video: h264_nvenc (hardware) · yuv420p · CQ 23", audio,
  mux, "Backend: GPU (…)", the file; on the right "Run" with phase, frames, elapsed, remaining,
  output length, size, bitrate; under them the speed line and the chart moving to the right; the
  bar; Pause and Cancel at the bottom
- **TEST** t956 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/render.py`

### Requirement: The finished render is frozen, summarised and opened from the window
When the job finishes, the window SHALL stop every time-based number at the moment the result
arrived (elapsed, speed, remaining) and SHALL show a final block: total time, average speed,
frames and audio samples written, encoder, backend, file size and average bitrate, followed by the
existing loudness, missing-source and lost-layer lines. A successful render SHALL offer **Open
file** (the file in the system's default application) and **Open folder** (the file manager at the
file's folder); a failure SHALL offer opening the render log. The button that closes the window
SHALL NOT occupy the place where Cancel stood during the run. While the phase is Finalize, Cancel
SHALL be disabled with a hover hint saying the file is being finished. A failure to open the file
or the folder SHALL be reported as an error notice naming the path.
Why: ktprompts v26.10 T12 "the render window counts statistics after finish" — `elapsed` is read
from `BgJob::started` every frame, so after the end it keeps growing and the average speed sinks.
Vegas turns Cancel into Close in place — a late click cancels or closes by surprise, and a cancel
during the final mux throws away a finished file (F-EXP-005 trick 3).

#### Scenario: Counters stop at the finish and the next actions appear
- **WHEN** the harness drives a scripted job to Done with an injected clock: the result arrives at
  10 s, then two more frames are drawn at 11 s and 15 s; separately, a job is held in the Finalize
  phase; separately, a job ends with an error
- **THEN** at 11 s and 15 s elapsed, average speed and the final block read exactly what they read
  at 10 s and "remaining" is gone; the final block names total time, average speed, frames,
  encoder, backend, size and average bitrate; Open file and Open folder are present and neither
  lies on the rectangle Cancel had; in Finalize, Cancel is disabled and its hover hint names the
  finishing step; after the error the window offers the log and no Open file
- **PATH** wait for the bar to fill → the phase says "Done", the numbers stop moving; the summary
  "Rendered in 0:42 · 2.1× real time · 1260 frames · h264_nvenc · GPU · 48.3 MB · 9.2 Mbit/s"
  and the loudness line → Open folder → the file manager shows the folder holding the file
  → Open file → the video plays in the system player → Close
- **TEST** t957 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/render.py`

### Requirement: Rendering over an existing file is asked first
When the output path chosen for a render already exists, the program SHALL ask before starting,
in its own question surface (the `Ask` set, not a new window): the question SHALL name the file,
its size and its modification date, and offer **Replace** and **Choose another name**; the
question SHALL say that the name is chosen again in Render… (rule 19). Nothing SHALL be written,
truncated or deleted before Replace. Choose another name SHALL reopen the save dialog with the same
default name; closing the question SHALL return to the settings window with every setting kept. A
path that does not exist SHALL start the render with no question.
Why: ktprompts v26.10 U1 "render to an existing file — ask"; today `launch_render` passes the
path straight to the export, and the muxer's `avio_open` truncates the old file at once — a
cancelled or failed render then also deletes it (`cleanup_on_err` removes the output path).

#### Scenario: An existing file is not touched without Replace
- **WHEN** the harness points the save seam (`KT_FILE_PICK`) at an existing file of known bytes and
  presses Render…; answers "Choose another name"; then again and answers Replace; separately points
  it at a path that does not exist
- **THEN** the first press opens the overwrite question naming the file, its size and date, and the
  file's bytes are unchanged; "Choose another name" returns to the save step and the file is still
  unchanged; Replace starts the render (the progress window opens); the non-existing path starts
  the render with no question; no window outside the `WINDOWS` registry opened
- **PATH** Render… → Render settings → Render… → the save dialog → pick `kt-demo-12s.mp4`, which
  is already there → the question "kt-demo-12s.mp4 already exists (48.3 MB, 2026-10-27 14:02).
  Replace it? The name can be chosen again in Render…" with Replace and Choose another name →
  Choose another name → the save dialog again → the same file → Replace → the progress window
- **TEST** t958 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/render.py`

### Requirement: The render log is kept in the window and in one file
The progress window SHALL hold a collapsible log section (collapsed while the run is healthy,
opened by itself when the run ends in an error) with at most the last 800 lines; each line SHALL
be prefixed by the UI with the elapsed time of the run. The log SHALL contain the engine's log
lines, the backend choice and its reason, the sources rendered as placeholders, the Pause, Resume
and Cancel requests and the result. The same lines SHALL be written to `<data_dir>/render.log`; the
previous run's file SHALL be moved to `render.log.1` when a new render starts (two files, no
numbered fan), and the window SHALL show the log path. Writing the log SHALL never fail the render:
an unwritable file gives one journal line and the window says the log is only in the window.
Why: v1 had an 800-line log panel and wrote a log file on error
(`projectrenderer.cpp`, `render-error-<UTC>.log`); today a failed render leaves one error line in
the window and nothing on disk, and the GPU-backend failure goes to `eprintln!`, which a desktop
start never shows (`.desktop` has `Terminal=false`). Two files mirror the journal's rotation
(`journal.rs`: one step, no fan of numbers).

#### Scenario: The log is bounded, rotated and named
- **WHEN** the harness runs a scripted job that emits 1000 log lines and an error, with the data
  directory in a temp folder that already holds a `render.log` of a previous run; separately, the
  data directory is read-only
- **THEN** the window's log holds the last 800 lines, each starting with an elapsed stamp, and is
  open after the error; `render.log` holds all lines of this run, `render.log.1` holds the previous
  file's bytes, and the window shows the path of `render.log`; with the read-only directory the
  render result is unchanged, one journal line is written and the window says the log is only in
  the window; every new text of this change exists in the five catalogs with its placeholders and
  its glyphs are in the program's fonts
- **TEST** t959 (`src/apps/kt-app/src/layout_tests.rs`, `src/apps/kt-app/src/i18n_tests.rs`, `src/apps/kt-app/src/font_tests.rs`)
