## Why

`compute-kernels-render` left the render on a software adapter at 17 GB on the showcase, five rules
without a test, the physics tests reading models no shader answers to, no shader of ours run on
Windows in CI, and a memo nothing reads. The orchestrator, 2026-11-06: the render must not die of
memory, speed second; the owner allowed changing references, not leaving rules unchecked.

## What Changes

- An export's device count follows the available memory and the frame.
- t303, t332, t441, t544e, t544f, t942, t943 and t1268 are back on the GPU path.
- The VHS and camera models stand beside the shaders in a live test: an edit of one is red.
- Every shader compiles through FXC in the Windows job; a new job runs the picture on Mesa's lavapipe.
- **BREAKING**: telemetry `backend` sends `none`, not `cpu`, when there is no device.
- The post-FX memo, `memoize_postfx`, `nest_memo_key` and their tests go.

## Capabilities

### New Capabilities

### Modified Capabilities

- `export`, `video-fx`, `plugins`, `compositions`, `telemetry`, `media-index`.

## Impact

`kt-gpu`, `kt-engine`, `kt-cache`, `kt-app`, `kt-cli`, `fx-video-tape`, `fx-video-camera`,
`fx-video-codec`, the CI workflow.
