# Proposal: stream-layout-per-build

## Why

The owner's complaint behind the engine audit (kkt-26.11.09, I2): a heavy Datamosh on his region
`problem_region_26_11_09` stays "Loading" for seconds, even paused and on repeated laps. Row 8 of the
order table of `docs/engine-audit.md` names the Datamosh's own share of that cost — mechanisms M2 (an
O(whole) helper inside a loop over its parts) and M7 (per-element recomputation of per-build
constants):

- every call of the stream kernel (`needs`, `held`, `key`, `render`) builds a fresh `mosh::Sched` and
  `Keys` (two `HashMap`s), walks the frame's dependency chain back to its anchor (up to 600 nodes) and
  reads the slot's rows at every node through `rows_at` (a slot clone and a curve application per
  position); the prefetch asks `needs` for every frame of its 2 s window, so a pass walks
  window × chain positions (found-defects 144: a pass 160–280 ms against a 100 ms target);
- the audio half builds a `Layout` + `Sched` per 1024-sample codec frame and walks gate runs back per
  frame (`sound.rs`, `MoshSound::sched`);
- the encoder reads the residual and the reconstruction back as one 32-bit word per sample, the
  decoder its picture likewise, and the residual is uploaded the same way (found-defects 135: ~25 MB a
  1080p encode);
- the BT.601 conversion of every source frame into Y'CbCr runs scalar on the prefetch thread's CPU
  (`codec.rs` `Planes::from_rgba`), and the output's conversion back (`to_rgba`) likewise;
- the stream memo is a constant 256 MB: at 1080 lines it holds ~20 positions, so a loop longer than
  that recomputes its history every lap (found-defects 143).

## What Changes

- The stream kernel's schedule is COMPILED once per host build and stream size: the rows per position,
  the decoder's node and dependency, the chain index from the anchor (the intra-refresh grid), the clean
  encoder's and the broken decoder's content keys — all as forward recurrences over positions, filled
  lazily from the stream's first position up to the furthest position asked. `needs`/`held`/`key`/
  `render` then walk stored arrays. The host owns the cell (`StreamLayout::plan_cell`); a host that
  gives none (tests, older hosts) gets the schedule compiled per call, the same values.
- The audio half keeps one compiled schedule across its codec frames, dropped when its rows,
  curves or placement change.
- The codec's GPU jobs read back and upload 8-bit planes four samples to a word and the residual two
  samples to a word (the shaders write and read packed words).
- The source frame's RGBA → Y'CbCr 4:2:0 conversion becomes the encoder job's first compute step; the
  output conversion back to RGBA becomes a compute job. The CPU functions stay as the byte reference
  for the tests.
- The preview's stream memo is sized at run time: from the bytes a position takes at the stage's size
  and the loop region's length (what a loop needs), bounded by a share of the memory the machine has
  available, measured when the size is asked — not a constant for one machine.
- Structured-trace records for what is added: the schedule's compile (positions filled, the time), and
  the compute job's uploaded and read-back bytes.
- Class walk: every stream/stateful stage and simulation half that rebuilds a per-build structure per
  call is checked and listed in design.md; the ones found are fixed here or recorded in
  `docs/found-defects.md` (rows 310–329).

No change to any byte: the pictures and the sound are the same bytes (t1239 pinned, t1238, t1241,
t1242, the sound tests). The one exception is recorded in design: a dependency chain longer than 8192
frames (a mosh held for minutes with no intact I-frame), where the old walk cut the chain at an anchor
that moved with every frame.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `video-fx`: the datamosh requirement gains the compiled schedule (same keys and bytes as the per-call
  walk, compiled once per build), the packed readbacks and the conversion in the compute pass.
- `audio-fx`: the stream's sound requirement — the picture's schedule is compiled once per placement,
  not per codec frame, byte for byte.
- `video-plan`: the stage's memo is sized by the loop and the machine; a loop over a mosh is kept after
  its first laps; the prefetch's per-frame `needs` costs no chain walk.
- `telemetry`: the compile record and the compute job's bytes.

## Impact

- `src/libs/kt-plugin-api/src/stream.rs` (the plan cell on `StreamLayout`; memo sizing helper).
- `src/plugins/fx-video-codec/src/{mosh.rs,codec.rs,sound.rs,lib.rs,wgsl/*.wgsl}`.
- `src/libs/kt-engine/src/{video_stream.rs,video.rs,player.rs}` (the cell per stage and size, the memo
  size).
- `src/libs/kt-gpu/src/compute.rs` (bytes in the `ComputeJob` record only; `gpu-arbiter` works in the
  same file — a small, local edit).
- `src/libs/kt-time/src/trace.rs` (a record kind for the compile).
- Tests t2270–t2289; found-defects rows 135, 143, 144 move to fixed where closed.
