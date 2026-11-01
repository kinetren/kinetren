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
- **The breakages are its passport, drawn by curves** (the owner on the first build, 2026-11-01:
  "no curves … no way to reset the effect, at least by keys on the curves, on the beat … the GOP is
  limited to 60 for some reason"): the GATES `mosh` (I-frames dropped while on — off is the
  inactivity), `reset` (a key rising to on forces an intact I-frame — the reset on the beat), `bloom`
  (the run's first P-frame repeats), `freeze` (the reference stops updating); the per-frame codec
  quantities take curves too (quantiser, vector scale and rotation, residual kept, block loss).
  Structural rows keep no curve: GOP (up to 600, 0 = only at the cut and the resets), block, search
  range (64), encoded height (144–1080), frames from the event before (300), second input, seed.
- **A region of interest** (the owner: "programmatic handles … aim the effect at regions — an
  object detector, faces, whole people — a bridge for the future"): the stream contract takes a
  per-frame coverage region with an identity; the datamosh shows its broken stream inside it and
  the intact one outside. No host part gives a region yet; masks and detectors will.
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
  stage (the stage is skipped there, documented); a host part that GIVES a region (masks, a
  detector); a GOP strip in the panel (the panel is the shared parameter grid).

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
