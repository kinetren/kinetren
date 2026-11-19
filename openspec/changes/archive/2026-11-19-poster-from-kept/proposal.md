## Why

Found-defects 767: Ctrl+S queues the project's poster, and the poster composed its moment offline from
nothing — for a project with a sharp Datamosh that walked the codec's whole history again, 22.6 s of a
software device on the `posters` lane, for a moment the preview had just shown. Found-defects 695/783:
kt-app test processes of reel scenes die natively at exit ("double free or corruption", SIGSEGV) under a
loaded full run and leave nothing to read: the system's core handler keeps nothing of an unpackaged binary.

## What Changes

- `VideoGpuExec::read_kept(key)`: the composite kept under a key, read back (kt-gpu: its present memo).
- `Player::kept_picture_reader(t)`: a job to run off the UI thread that gives what the preview holds for
  `t` of the current version — the kept composite, or the disk tier's picture — or nothing.
- The poster takes that picture first (fitted to its size) and composes offline only when the preview
  holds nothing for its moment.
- A kt-app test process installs the native-crash handler with the plugin set: a native death leaves
  `<temp>/kt-test-crashes/<binary>-<pid>.raw`, the test in its header.

## Capabilities

### New Capabilities

### Modified Capabilities
- `video-plan`: an offline job reads a moment the preview holds instead of composing it.
- `telemetry`: a kt-app test process that dies natively leaves a raw report.

## Impact

kt-engine `video_plan.rs` (trait), `player.rs`; kt-gpu `engine.rs`; kt-app `welcome.rs` (posters),
`plugins.rs` (tests only).
