# Spec Delta

## ADDED Requirements

### Requirement: The video compose cost is a number, in the panel and in the probe
The performance panel SHALL show the compose time of the last shown video frame and its
average over the last second, in milliseconds, next to the existing FX backend line ("FX:
GPU (<adapter>)" or the CPU line). The CPU line names the reason when the GPU was given up
for the session. `kt-cli probe <project> <from> <to> --time`
SHALL print, after the audio section, a video section: the frames composed, compose ms p50 /
p95 / max, the backend, and the passes and sub-samples per frame. The section is computed on
a warm cache, without the UI, through the same plan executor as the preview. The SPEED is
judged on a release build, as for audio.
Why: ktprompts v26.10.18 item 8. "fps drops" could not be split between the grid, the
morpher's strength slot, the GPU readback or a CPU fallback, because there was no number for
the video compose. `probe --time` measured only the audio render (R.43, item 301).

#### Scenario: The probe and the panel name the cost and the backend
- **WHEN** `kt-cli probe` is run with `--time` over a project with a morpher event grid 12×30 on a 4 s event, once with `KT_VIDEO_GPU=0`; and the same project is played in the harness with the performance panel open
- **THEN** the probe prints the video section with the frame count of the range, p50/p95/max in ms, `backend=CPU` for the forced run, and per-frame passes and sub-samples equal to the counters of t946; the panel shows "Compose" with a millisecond value that changes while playing, next to the FX backend line, and stops repainting when playback stops
- **PATH** open the project with the morpher grid → open the Performance panel → it shows "Compose — ms" under "Preview 960×540" and the line "FX: GPU (<adapter>)" (or the CPU line with the reason) → press Play → the compose value updates while the grid grows from 1×1 to 12×30 → press Stop → the last values stay and the panel no longer repaints
- **TEST** t947 (`src/apps/kt-cli/tests/cli.rs`, `src/apps/kt-app/src/layout_tests.rs`)
