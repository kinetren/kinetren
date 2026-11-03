# Proposal: camera-isp-edges

## Why

The README shot `fx-camera-isp` (branch `readme-shots-showcase`, 2026-11-03, the showcase footage
`05-charlotte-by-night.webm`, look "Phone at night") shows, in the "with" half, a band about 30 px
wide along the left and top edges filled with streaks of repeated edge pixels. Two stages of the
simulation read outside what exists and clamp to the edge texel:

- the ISP's lens correction (P3b, view "lens_corrected"): "Phone at night" has a pincushion lens
  (`distortion` −0.06) and a calibrated dewarp; the dewarp pulls every output pixel's sample
  outward, up to 4.6 % of the half-width at the long edges and 7 % at the corners — beyond the
  sensor. 7 px on a 320 px frame, ≈ 44 px on 1080p;
- the lens (P0, view "optics"): a barrel lens (`distortion` > 0: "Cheap webcam" 0.12, "Security
  cam, IR" 0.25) and lateral CA sample the scene beyond the recorded frame at the corners and
  edges (9 % of the half-width at the long edges of the webcam, uncorrected).

A real camera has neither: its lens projects the world, which continues past any frame, and its
ISP never reads pixels the sensor does not have.

## What Changes

- **The lens's field is fitted to the scene.** The simulated lens's focal length is chosen so that
  its field — every ray of the sensor's border, in every channel, after distortion and lateral CA —
  is the largest one inside the recorded frame (`lens_fit`). An ideal lens: exactly the old field.
- **The ISP crops its lens correction to the sensor.** The dewarp/CA correction's output scale is
  the widest one whose every pixel maps inside the sensor (`isp_crop`), as a phone's ISP crops the
  margin its distortion correction cannot fill. Correcting distortion therefore costs field: a
  calibrated "Phone at night" frames 4.6 % tighter than the scene.
- CPU (`cpu.rs`) and GPU (`s01_optics.wgsl`, `s08_lens_correct.wgsl`, `camera_common.wgsl`) in one
  edit; the defaults (no distortion, no CA) keep their bytes (t328's golden hash).
- t1220's geometric pairs compare each output with the ideal camera at its own paraxial
  magnification (the corrected one at crop/fit, the uncorrected one at 1/fit), so the pair measures
  the warp, not the framing.
- New regression test t1430 (both executors, every preset and strong mis-tuned forms).
- The README shot `fx-camera-isp` re-taken.

## Capabilities

### Modified Capabilities

- `video-fx`: the camera films only the scene it has, and its ISP reads only the sensor; the
  correcting-block scenario's geometric reference.

## Impact

`src/plugins/fx-video-camera` (kernel and WGSL, consequences test), `src/libs/kt-gpu/tests/camera_isp.rs`,
`openspec/specs/video-fx`, `docs/guide/whats-new.md`, `docs/images/fx-camera-isp.webp`. No project
format change (no new row).
