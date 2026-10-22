# Proposal

## Why

Two claims of the owner (R.61, 2026-09-22). The first: effects and generators need a curve that is LED by another
curve of the same event (another effect, another value), with its own recalculation formula; the resulting graph
is drawn, but cannot be edited with the mouse. As a continuation — setting a curve by a formula, without placing
keys. The second: automation needs SEGMENT SHAPES — the cubic, the zigzag, the straight line, each with its own
parameters, with the option to turn a shape on for all at once and to set up a separate curve in its own way.

Plus a defect found by the owner from an indirect sign: the "Paste envelope" button can be disabled, so the state of
the buffer lives inside the program. The reconnaissance confirms it: copying writes both to the system buffer and to
the mirror `env_clipboard`, and the availability of the button is decided by the mirror — what was copied in another
window cannot be pasted. The second half of the same place: the button goes dark for a different kind of effect, that
is, the remapping of fields is not merely absent but forbidden.

The place for this was laid in advance: `CurveSource` with the variants `Keys`/`Formula`/`Link` has existed since R.59
and today is rejected loudly by validation. The shape, however, is today one for the whole curve — the boolean `ease`.

## What Changes

The round runs in five stages; the order is chosen so that each next one relies on the previous, not the other way round.

- **S0. Chronicle and feature base.** The claim and the decisions — into the history of requirements. The verdicts
  F-AUT-007/008/009 receive the decision of the round; the verdict of F-AUT-009 in the part "the zigzag is a source, not
  a shape" is **revised by the owner's direct decision** (see design.md).
- **S1. Segment shape.** The shape moves from the curve to the KEY and describes the stretch AFTER it. A closed set with
  its own parameters for each kind: straight line, cubic (with tension), step, ease in / ease out, zigzag (periods,
  amplitude, profile). **BREAKING:** the field `ease` disappears; backward compatibility is not required (the owner's
  decision), old files are migrated on reading. The default shape is a PROJECT field, plus a command "apply the shape to
  all segments of the curve / of the whole event" with a single undo. The speed (`rate`) accepts only a straight line and
  refuses LOUDLY.
- **S2. Envelope clipboard.** The mirror inside the program is removed. The paste button never goes dark: on press the real
  system buffer is read, and if parsing failed — an error with a reason in the status line. When pasting into another kind
  of effect, fields that match by name and type land silently; for the remainder a matching window opens, "curve → target
  parameter".
- **S3. Link and evaluator.** `CurveSource::Link` starts to work: the value is led by another property by address, with a
  recalculation expression. A mini-formula evaluator with a closed set of operations appears. ANY event of the PROJECT within
  one reel is addressable (see design.md about the reel boundary). Non-numeric properties are led through thresholds (a flag)
  and snapshots at the marks of the value axis (choice, text, media, colour). The derived graph is drawn, but cannot be edited
  with the mouse. A dependency cycle is rejected by validation with its path.
- **S4. Formula as a source.** `CurveSource::Formula` starts to work: a curve without keys, set by a text of time and addressable
  properties; input from the window. The text of the formula enters the key of post-FX. A project without links and formulas pays
  zero — this is checked by a test.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `automation`: "The domain of a key — the tick of the source material" (the shape lives on the key; for a link between events the
  common denominator is the timeline time); "Copy-paste of effects, chains and envelopes" (the source is the system buffer, the
  remapping of fields); "Expressions and plugins — interfaces only" (the formula and the link cease to be only an interface);
  "Compiled automation of video and zero cost" (the plan compiles derived sources too). Plus new requirements: the segment shape,
  the link as a source, the formula as a source, the evaluator.
- `ui-timeline`: a new requirement on the interface of the shapes, the derived graph, the formula field and the envelope buffer.
- `project-format`: "The version of the envelope `kt`" — the format v10 by content: the shape on the key, the working sources of a
  curve, the default shape of the project; reading of old files by a one-way migration.

## Impact

- Model: `src/libs/kt-model/src/project.rs` (`AutoKey`, `ParamCurve`, `CurveSource`, `curve_value`), `warp.rs`
  (refusal of speed), `autoplan.rs` (compilation of sources), a new module of the evaluator and of link addressing,
  `ops.rs` (buffer blocks, remapping).
- Engine: `src/libs/kt-engine/src/video.rs` and the audio path (resolution of sources before the stream, the key of
  post-FX), `fxstore.rs`.
- Interface: `src/apps/kt-app/src/automation.rs` (shapes, derived graph, formula field), `fx_editor.rs` and
  `morpher_editor.rs` (buffer), `pubparam.rs`, `gen_ui.rs`, the catalogues `i18n/*.toml` ×5 and the font subsets.
- Format: `src/libs/kt-otio/src/convert.rs`, `docs/file-format.md`.
- References: `GOLDEN_AUDIO`, `GOLDEN_VIDEO`, `GOLDEN_TITLE_VIDEO` must NOT move — the default shape must reproduce today's
  smoothing bit for bit.
- New regression tests: t695–t710 (a project-wide numbering; the next free one is t695).
