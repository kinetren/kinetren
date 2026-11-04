## Why

The owner (2026-11-04, C2 of kkt-26.11.03, asked what to do with the CPU composite after seeing that
the render on this machine costs about 40 % more without it): "удаляй". The orchestrator's stage list
for the same day comes first because the deletion needs it: the wait of the first frame, the buffers
of `encode`, the math that has no shader yet, and the one-frame limit of a software adapter.

## What Changes

- The compute host reuses a finished job's buffers, bounded. Datamosh decodes in a shader, in the
  encoder's own job, so `predict` and `reconstruct` lose their CPU text.
- No disk cache of shaders: the 6.6 s it was meant to remove does not reproduce (D1).
- A software adapter composes frames in parallel on several devices.
- **BREAKING**: the CPU composite and every CPU and SIMD copy of a video kernel are deleted. A machine
  with no Vulkan, DX12 or Metal has no picture and says so; bytes of every video reference change.

## Capabilities

### New Capabilities

### Modified Capabilities

- `plugins`: the job's buffers, the Datamosh decoder, kernels without a CPU side.
- `video-plan`, `export`, `transport`: one executor.

## Impact

`kt-gpu`, `kt-plugin-api`, `kt-fx`, `kt-engine`, every `fx-video-*` and `transitions-*` plugin, `kt-app`
and `kt-cli`, the tests that compose through `compose_from_cache`, the invariants 7 and 11.
