# Proposal: stream-front-on-device

## Why

found-defects 543: the slots in front of a stream stage (Camera ISP feeding Datamosh) ran on the
offline device, were read back to the processor and uploaded again to the compute device for the
codec — ≈ 16 ms of the advancing thread per history position at 1080 lines (stream-input-order,
"Cost"). Two small rows of the same family close with it: 561 (a geometry shape's specialized pass
replaces the interpreter's without the memo keys naming the form) and 562 (a compute job that finds
another calibrating records its device times on the older calibration).

## What Changes

- The front's executor is an engine on the compute host's own device (`kt_gpu::front_on_compute`);
  `VideoGpuExec::execute_held` runs the front's plan as `execute_once` does and leaves the complete
  frame in a buffer there (tightly packed RGBA), submitted and never waited for.
- The compute door takes such a buffer as a job's input: `compute::Init::Held`, copied on the device;
  a buffer of another device is refused. The stream input names where a frame lies:
  `StreamInput::on_device` → `OnDevice::{Held, Missing, Host}`; Datamosh's walk converts a held frame
  on the device and reads bytes only on `Host`.
- The front memo keeps either form under its own key; without a compute host of kt-gpu the front runs
  as before (offline sibling, bytes). `KT_FRONT_HELD=off` runs the bytes form for an A/B;
  `kt-cli stream-cost` says how many front runs stayed on the device.
- A geometry shape that becomes ready moves the run forms' revision (row 561).
- A job's device-time records are not placed on a calibration older than its age when the job found
  another calibrating; reading the calibration never waits for one in progress (row 562).

## Capabilities

### Modified Capabilities
- `video-fx`: the front of a stream stage reaches its codec on the compute device; a ready geometry
  shape moves the forms' revision.
- `telemetry`: a stale calibration places no record.

## Impact

kt-plugin-api (`compute::{Init::Held, Held}`, `stream::{OnDevice, StreamInput::on_device}`), kt-gpu
(`held.rs`, `compute.rs`, `engine.rs` `execute_held`, `lib.rs`, `compiled.rs`, testkit), kt-engine
(`video_plan.rs` `HeldOut`, `video_stream.rs`), fx-video-codec (`walk.rs`, `codec.rs`), kt-cli
(`stream-cost`). No format change; OTIO unaffected.
