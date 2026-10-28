## REMOVED Requirements

### Requirement: The segment shape lives on the key and has its own parameters
**Reason**: its scenario "Speed accepts only linear and refuses loudly" is revoked by the owner's claim (ktprompts v26.10.18 item 2, "childish excuses"); the requirement is restated below under a new name with the speed taking every shape.
**Migration**: every other rule and scenario of the requirement moves verbatim into "The segment shape lives on the key and has its own parameters, the warp speed included"; t696/t696b are inverted, not deleted.

## ADDED Requirements

### Requirement: The segment shape lives on the key and has its own parameters, the warp speed included
The interpolation shape SHALL belong to the KEY and describe the segment AFTER it, not the whole curve.
The set of shapes is closed, and each has its own parameters: linear (no parameters), third order (tension), step (no parameters), ease in and ease out (tension),
zigzag (number of periods, amplitude, profile — triangle/rectangle/sine), **segment formula**
(text; variables `s` — the fraction of the path 0..1, `a` and `b` — the values of the left and right key, `t` —
seconds from the left key; the same evaluator and the same closed set of operations as the source formula). The amplitude of a zigzag SHALL be set IN UNITS OF THE PARAMETER and clamped
by the target's passport, not as a fraction of the difference between keys: the most frequent zigzag is an oscillation around a
CONSTANT value, where the difference is zero. Each shape SHALL be a closed function of
`(fraction of the segment, ends, parameters)` without state; for the segment formula the ends are set by the
formula itself (as with a step, it may fail to coincide with the key), and an uncomputable text SHALL
be rejected at the command and on load, with its position and name, instead of giving a silent zero. The
default shape for new keys is a PROJECT field, not a machine setting; changing the default MUST
NOT touch keys that are already placed. The command "apply the shape to all segments" SHALL
exist for a curve and for an event, and lands as one history step. The value at the
last key is the extreme one; the shape of the last key does not apply.
The warp speed (`rate`) SHALL take every shape and the segment formula like any other parameter. The warp
map SHALL read the speed through ONE sampling function that turns each non-linear segment into a
straight-line profile on a fixed grid; the render, the prefetch window, the video frame map and the
interface's material map of the event SHALL all read that same profile, so they agree to the frame. A
curve whose segments are all linear SHALL give the former profile bit for bit.
Why: a single boolean for the whole curve expresses neither "this section as a step" nor a zigzag; a machine
setting would give "one file, two behaviours" (the lesson of Vegas), and an amplitude taken from the difference would
collapse the zigzag exactly where it is needed. R.67: the owner asked to "set something on separate areas" — the formula on a segment is exactly that area between two keys.
Revision (ktprompts v26.10.18 item 2, "childish excuses"): R.61 refused every non-linear shape on
speed because the closed form of the inverse warp map stands on a piecewise-linear profile. That is a
property of the PROFILE, not of the curve the person draws: the resolver already samples a formula layer
on speed into straight keys (t707), so a shape can be sampled the same way and the closed form stays exact
on what it reads.

#### Scenario: Shapes are computed closed and deterministic
- **WHEN** a segment is computed by each shape at 0, 1 and the middle, with the extreme tension values; for a zigzag — with a fractional number of periods and on a constant segment
- **THEN** the ends of the segment equal the key values for ALL shapes except the step (for the step the left value holds until the next key); a zigzag on a constant segment oscillates with the set amplitude in units of the parameter and is clamped by the passport; two runs give identical bytes
- **TEST** t695 (`src/libs/kt-model/tests/fx_auto.rs`)

#### Scenario: The default reproduces the former smoothing byte for byte
- **WHEN** a curve created without a specified shape is compared with the frozen oracle of the former `curve_value(keys, ease = true, at)`
- **THEN** the values match bit for bit at all probes; `GOLDEN_AUDIO`, `GOLDEN_VIDEO` and `GOLDEN_TITLE_VIDEO` do not move
- **TEST** t695b (`src/libs/kt-model/tests/fx_auto.rs`), t11 and t67 (`src/libs/kt-engine/tests/golden_export.rs`)

#### Scenario: Speed takes every shape and the map follows it
- **WHEN** each shape of the closed set, a segment formula `a + (b - a) * s * s` and a formula layer `1 + 0.5 * sin(t * 6)` are assigned to the `rate` curve of a 10 s event by command; then a curve with linear segments only
- **THEN** every command is accepted (the former `RateShapeMustBeLinear` refusal is gone — t696 inverted); the consumed material and the inverse map agree with an independent numeric integration of the shaped curve within one audio frame over 10 s; the profile read from the raw event (the interface's path) and from the effective events (the render's path) are bit-for-bit equal; the all-linear curve gives the former profile bit for bit; two runs are byte-for-byte equal
- **TEST** t913 (`src/libs/kt-model/tests/warp.rs`), t696 (`src/libs/kt-model/tests/warp.rs`)

#### Scenario: The speed curve's shape menu offers what every curve offers
- **WHEN** the FX window of an event with slowed reverb is open on its speed curve; a segment is right-clicked; "Smooth" is chosen; then "Formula" is chosen and a text is typed; then "shape of all segments" is set to a zigzag
- **THEN** the menu lists every shape and "Formula" (no refusal line); each choice lands as one undo step; after each, the event's material end shown on the timeline (the filmstrip and the trim limit) equals the render's consumed material for the new curve
- **PATH** double-click the slowed event → the FX window opens on Slowed reverb → click the "rate" curve → the speed graph shows two keys → right-click between them → the menu lists Linear, Smooth, Step, Ease in, Ease out, Zigzag and Formula → click Smooth → the segment bends and the filmstrip of the event shifts to the new material → right-click again → Formula → type `a + (b - a) * s * s` below the graph → the segment follows the text → choose Zigzag in "Shape of all segments" → the whole speed curve zigzags and Ctrl+Z restores it in one step
- **TEST** t914 (`src/apps/kt-app/src/layout_tests.rs`), t696b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The project default and the "to all" command
- **WHEN** the default shape in the project is changed, then a new key is placed; then the shape is applied to all segments of the curve and of the whole event
- **THEN** the new key takes the default, the former keys are untouched; the command changes all segments in one undo; the default lives in the file, not in the program's settings
- **TEST** t697 (`src/libs/kt-model/tests/fx_auto.rs`)

#### Scenario: The segment formula is computed on the segment and refuses loudly
- **WHEN** the segment between the keys at 1.0 s (value 0) and 3.0 s (value 1) is assigned the formula `a + (b - a) * s * s`, then `sin(t * 6.283)`; the neighbouring segments are a zigzag and a linear; then a formula with an unknown name and with division by zero — by command and by reading the file
- **THEN** the value on the segment equals an independent oracle at 0, 1/4, 1/2 and 1; the neighbouring segments compute the former numbers; two runs are byte-for-byte equal; an unknown name and division by zero are rejected with the position and text, a file with such a shape does not load silently; the shape is written to the file and read back the same
- **TEST** t787 (`src/libs/kt-model/tests/fx_auto.rs`), t787b (`src/libs/kt-otio/tests/convert.rs`)

## MODIFIED Requirements

### Requirement: Formula — the source of a value, evaluated by its own evaluator
`CurveSource::Formula` SHALL define a curve as TEXT of time and addressable properties, without
keys. The evaluator is its own, with a CLOSED set of operations (arithmetic, comparisons, branching,
`min`/`max`/`clamp`/`abs`/`floor`, trigonometry, `lerp`, `noise` from an integer seed),
without loops, without access to the file system, the clock, or randomness from the environment. The same
text with the same inputs SHALL always and everywhere give the same bytes (the determinism
invariant). Division by zero, overflow and `NaN` SHALL give a NAMED parse or evaluation
error, not a silent zero. The formula text SHALL enter the post-FX key and the chain's hash:
otherwise editing a formula would leave the former frames on screen. A formula SHALL
resolve into numbers BEFORE the audio stream, in the same place as the curve profiles.
A project without links and formulas MUST NOT pay anything: no plan is built, the chain
is borrowed, and the bytes are the same as before.
Why: the owner's decision — "your own mini-formula, computed live on every frame, no external
interpreters"; swapping a foreign engine in After Effects changed the result of finished
projects, and executable code in a document is the lesson of Blender.

#### Scenario: The evaluator is closed and deterministic
- **WHEN** the expressions of the set are parsed and computed, then an expression with an unknown name, with division by zero, with an unclosed bracket, and with an attempt to access the machine's time
- **THEN** the set is computed identically in two runs; each of the four errors is named by its text with the position in the line; none gives a silent zero
- **TEST** t701 (`src/libs/kt-model/tests/formula.rs`)

#### Scenario: A formula as a curve without keys
- **WHEN** a property's source is a formula of time; then a formula that reads another property; then a formula on a warp speed
- **THEN** the values coincide with the independent oracle of the test; reading another property goes through the same address as a link; at speed the formula is accepted: it resolves into straight keys, the speed rule holds on them, and the warp map of the raw event reads the same profile as the render
- **TEST** t707 (`src/libs/kt-model/tests/links.rs`), t913 (`src/libs/kt-model/tests/warp.rs`)

#### Scenario: The formula text in the post-FX key
- **WHEN** ONLY the formula text is edited, with the same effect parameters
- **THEN** the post-FX key changes, the frame is recomputed; memoization does not return the former frame
- **TEST** t708 (`src/libs/kt-engine/tests/postfx_cache.rs`)

#### Scenario: Zero cost without links and formulas
- **WHEN** a project where there is not a single derived source is rendered, and a project where a formula sits on ONE event out of many
- **THEN** in the first the bytes and the plan are byte-for-byte the former ones, and there is no dependency graph at all; in the second the plan of the neighbouring events has not changed, and the build cost does not grow with the number of events
- **TEST** t705 (`src/libs/kt-engine/tests/auto_plan.rs`), t710 (`src/libs/kt-engine/tests/cost.rs`)
