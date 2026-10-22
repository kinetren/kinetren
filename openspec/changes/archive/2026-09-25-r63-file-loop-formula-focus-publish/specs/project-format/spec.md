# Spec Delta

## MODIFIED Requirements

### Requirement: Old files migrate on read, one-way
Files up to the current version SHALL be read and converted to the current semantics on read; the next write SHALL already go in the new form: curves in fractions 0..1 → material ticks by the clip's geometry (v<3); camera keyframes in fractions → common curves; the morpher up to v4 → a patch with `timed: false`; `ui.lane_order` → the model's `lane_order`; the generator's values in `parameters` and the title's animation mode → values on the event and curves by the same formulas as the preset in the window; the former boolean `ease` of a curve → the segment shape on the keyframe, and for a SPEED curve (`rate`) `ease: true` SHALL be read in the LINEAR form — that is how it always sounded (the warp map sits on the piecewise-linear profile and never read the `ease` field). Migration MUST NOT produce a file that its own validation rejects: a file that opened in the previous build SHALL open in the next one too, and the sound and the frame SHALL remain as before.
Why: backward compatibility is not required, but the owner's working projects MUST open and sound/look as before; a divergence of the migration from the preset would give "the same button — a different result". R.63: `untitled_v26.09.19.otio` (14 speed curves with `ease: true`) was rejected by `RateShapeMustBeLinear` — the reader translated `ease` into "smooth", while the speed validator accepts only the straight form; the former warp did not see the shape at all, so the correct translation is "linear".

#### Scenario: Curves of the old domain are translated into material ticks
- **WHEN** a v2 file with keys 0.0 and 0.5 on a clip 0..90000 is read and re-saved
- **THEN** the keys become 0 and 45000; the write is v3
- **TEST** t212 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Legacy camera keyframes, the old morpher and the old place of order
- **WHEN** the camera carries `keyframes` as fractions of the event; the morpher is written as a bare array of targets and an externally tagged view; the row order lies in `ui.lane_order`
- **THEN** the keyframes are consumed by the migration into curves in ticks; the morpher is read with `timed: false` and the former semantics, and does not raise the version up to v4; the order moves into the model
- **TEST** t189, t240, t241 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The generator of an old file hands the values to the event
- **WHEN** a v8 file with text and an animation mode inside `parameters`, and without `kt.gen` on the clip
- **THEN** the media path was shortened to its form; the values moved to the event; the animation curves equal those the preset sets; the project is valid
- **TEST** t665 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: A speed curve with the former `ease` is read as straight and the file opens
- **WHEN** a file before R.61 is read with `rate` curves `ease: true` on the event, in the morpher's patch and inside a nested reel (the file shape as in `untitled_v26.09.19.otio`), and the same with `ease: false`
- **THEN** the project is valid; all speed keys have the shape `Linear`; the other curves with `ease: true` give the former smoothing; the warp map gives the same ticks as before R.61; re-saving writes `shape` only where it is not the default; on the old code — rejection by `RateShapeMustBeLinear`
- **TEST** t747 (`src/libs/kt-otio/tests/convert.rs`), t747b (`src/libs/kt-model/tests/fx_auto.rs`), live scenario `src/scripts/live/oldfile.py`
