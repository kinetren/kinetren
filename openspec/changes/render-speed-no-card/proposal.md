## Why

Found-defects rows 84 and 75. With no graphics card the showcase renders in 129-130 s against 99-101 s on
the deleted CPU build, the demo in 34 s against 30 s, and nothing read where the time goes. A file
whose header promises more than it holds stalls a render: a 40 s event did not finish in 280 s.

## What Changes

- `kt-cli export --time` reads where the time goes inside the composite and what each stage waits for.
- The end of a file is met once: video and audio stop seeking past what the file holds, and a
  half-written last packet is not sent to the decoder.
- What the measurement finds in the render path (see `design.md`).

## Capabilities

### New Capabilities

### Modified Capabilities

- `media-index`, `export`.

## Impact

`kt-ffmpeg`, `kt-media`, `kt-engine` export, `kt-gpu`, `fx-video-tape`, `kt-cli`, `fixtures/gen.sh`.
