# Proposal

## Why

The owner asked (kkt-26.11.01, line 50; registry row D1) for a REAL datamosh — "not the snot of
our VHS and of some transition, but a real motion-compression path that we can break carefully
with parameters". His answer p.4 (`docs/user-feedback.md`, 2026-11-01): a real effect is first a
physics problem; the interface is secondary. The root `CLAUDE.md` rule says an imitation of a
physical effect (block shifts, RGB split — our `glitch` transition and `vhs`) is not a
simulation of it. A datamosh is what a motion-compensated DECODER produces when it is fed a
broken stream; the only honest way to give it is to run such a codec. The reconnaissance is
F-FX-011 (`docs/features/fx/simulate/datamosh-codec-path.md`).

The obstacle is the engine's purity: a video kernel is a function of ONE frame (F-EXT-003,
plugins D4), because the post-FX memo and the GPU readback cache are keyed on that. A codec's
frame N depends on every frame since its last intact I-frame.

## What Changes

- **An own block codec** in a new family crate `src/plugins/fx-video-codec` (plugin id
  `datamosh`): Y'CbCr 4:2:0 planes at an encoded height of its own; 16×16/8×8 macroblocks;
  block-matching motion estimation with a search range; a closed prediction loop over a
  reference buffer; an 8×8 DCT residual quantised by QP; a GOP with I-frames at the stream
  start, at the cut and every N frames. Frozen math only (invariant 11).
- **The breakages are its passport:** drop I-frames from a frame on (the reference survives the
  cut), bloom (one P-frame repeated N times), freeze the reference, scale / rotate / swap the
  vectors, take the vectors from the track above or below (a second input), quantiser, residual
  loss, macroblock-loss probability with a seed, GOP length, block size, search range, encoded
  height, and how many frames of the event before the stream starts in.
- **A new planner class `VideoClass::Stream` with a `StreamKernel`** in `kt-plugin-api`: a
  SOURCE STAGE. It is not state: its output at frame N is a pure function of its rows and of the
  source frames of a bounded HISTORY the kernel declares. The registry refuses a stream plugin
  without a stream kernel, with a GPU side, or with state/tail/look-ahead declarations.
- **The engine runs the stage in the plan's common code** (`layer_src`, as frame blending does):
  both executors receive the decoded mosh as the layer's pre-FX frame; the rest of the chain runs
  on either backend. The order (`layers_at`) asks for the stream's frames at the codec's size —
  the frame at N and, on a seek, the history the memo does not cover.
- **A content-keyed, bounded memo of codec results** (`StreamMemo` in the API, owned by the host):
  the preview shares one, each export owns its own. Keys are hashes of the rows and of the source
  addresses of the whole chain; the memo is speed only, never a different byte.
- **Export pre-rolls the codec** before a frame whose history the memo lacks, decoding in bounded
  batches, so chunked export equals one-go export and a range export equals the same frames of a
  full export.
- **Presets:** classic cut mosh (I-frame dropped at the cut), bloom, vector transplant, melt.
- **Not in scope:** a GPU codec (decision D4 of the design); a mosh on a reel instance or a track
  stage (the stage is skipped there, documented); curves on the stream rows (deferred — rows are
  Structural); a GOP strip in the panel (the panel is the shared parameter grid).

## Capabilities

### New Capabilities

### Modified Capabilities
- `video-fx`: a source-stage kind — the codec datamosh, its mechanics, breakages and
  random-access determinism.
- `plugins`: the stream planner class, its kernel contract, its refusals and the host-owned memo.

## Impact

- New crate `src/plugins/fx-video-codec`; `kt-plugin-api` (`stream` module, class, refusals);
  `kt-plugins-builtin` (id, install); `kt-engine` `video.rs` (layer source, order, decode
  qualities), `export.rs` (pre-roll); `kt-app` catalogs (name, rows, presets).
- Project format: none (a plugin slot; the rows are its passport). OTIO: a plugin slot exports
  as every plugin slot does — no new loss row.
- Tests t1235–t1259.
