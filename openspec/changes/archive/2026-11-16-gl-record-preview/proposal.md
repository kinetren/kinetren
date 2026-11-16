## Why

The stage-16 finish.sh on orch-26-11-01 was red twice on live `gpu_record`: "the record counts the
preview's frames". Since gl-context-holder the preview on GL composes on its own headless device and
takes the bytes path, and the bytes path timed every frame as the offline device's: each GPU record of
a playback read `N frames (offline N)`, so on GL the journal no longer told the person's frames from
offline work. The device itself was described to the trace, the error log and the holds as `headless`.

## What Changes

- A read-back frame on the preview's own devices (the one off the window's on GL, the window's under
  `KT_PREVIEW_READBACK`) is timed as the preview's; the offline engine and the side engine stay offline.
- The preview's device off the window's opens under the role `preview`: its errors, its trace
  description (`gpu.preview.*`) and its calls in flight (`holds`) name it once, not `headless`.
- Its ring is registered as fronting the person's frame (`serve_preview`), as the slow route's is.
- An engine's frame state is made (`sibling`) and released (`Drop`) at its device's GL gate:
  found-defects 683, met by the new test beside t2745's hold.
- `gpu_admit.c` = 2 marks a ring that fronts the person's frame without a display; `kt-cli trace gpu`
  prints it.

## Capabilities

### Modified Capabilities

- `durability`: making and releasing a preview engine wait at the GL gate.
- `telemetry`: the GPU record counts the preview's own device as preview; `trace gpu` marks the ring
  fronting the person's frame.

## Impact

`kt-gpu` (engine roles, the preview opener, the arbiter's admission record), `kt-time` (the field's
doc), `kt-cli` (`trace gpu`). No format change.
