# Spec Delta

## MODIFIED Requirements

### Requirement: The segment shape lives on the key and has its own parameters
The interpolation shape SHALL belong to the KEY and describe the segment AFTER it, not the curve as a whole. The set of shapes is closed, and each has its own parameters: linear (no parameters), third order (tension), step (no parameters), ease in and ease out (tension), zigzag (number of periods, amplitude, profile — triangle/rectangle/sine), **segment formula** (text; variables `s` — the fraction of the path 0..1, `a` and `b` — the values of the left and right keys, `t` — seconds from the left key; the same evaluator and the same closed set of operations as the source formula). The amplitude of the zigzag SHALL be set IN UNITS OF THE PARAMETER and clamped by the target's passport, not as a fraction of the difference between keys: the most common zigzag is an oscillation around a CONSTANT value, where the difference is zero. Each shape SHALL be a closed function `(fraction of the segment, ends, parameters)` without state; for the segment formula the ends are set by the formula itself (as with a step, it is allowed not to coincide with the key), and a non-computable text SHALL be rejected at the command and on loading with the position and the name, rather than giving a silent zero. The default shape for new keys is a field of the PROJECT, not a machine setting; changing the default MUST NOT touch the keys already placed. The command "apply the shape to all segments" SHALL exist for a curve and for an event and be recorded as one history step. The value at the last key is the final one; the shape of the last key has no effect.
Why: a single boolean for the whole curve expresses neither "this segment as a step" nor a zigzag; a machine setting would give "one file — two behaviours" (the lesson of Vegas), and an amplitude taken from the difference would collapse the zigzag exactly where it is needed. R.67: the owner asked to "set something on separate areas" — a formula on a segment is exactly that area between two keys.

#### Scenario: Shapes are computed closed and deterministic
- **WHEN** a segment is computed by each shape at 0, 1 and the middle, with extreme tension values; for the zigzag — with a fractional number of periods and on a constant segment
- **THEN** the ends of the segment equal the key values for ALL shapes except the step (for the step the left value holds until the next key); the zigzag on a constant segment oscillates with the given amplitude in units of the parameter and is clamped by the passport; two runs give identical bytes
- **TEST** t695 (`src/libs/kt-model/tests/fx_auto.rs`)

#### Scenario: The default reproduces the former smoothing byte for byte
- **WHEN** a curve created without specifying a shape is compared with the frozen oracle of the previous `curve_value(keys, ease = true, at)`
- **THEN** the values match bit for bit at all sample points; `GOLDEN_AUDIO`, `GOLDEN_VIDEO` and `GOLDEN_TITLE_VIDEO` do not move
- **TEST** t695b (`src/libs/kt-model/tests/fx_auto.rs`), t11 and t67 (`src/libs/kt-engine/tests/golden_export.rs`)

#### Scenario: Speed accepts only linear and refuses loudly
- **WHEN** any shape other than linear is assigned to a key of the curve `rate` — from the interface and by a command
- **THEN** the command is rejected with a named reason (the closed form of the inverse warp map stands on a piecewise-linear profile), the status line shows an error, not silence; the warp map computes the previous numbers
- **TEST** t696 (`src/libs/kt-model/tests/warp.rs`), t696b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The project default and the "to all" command
- **WHEN** the default shape in the project is changed, then a new key is placed; then the shape is applied to all segments of the curve and of the whole event
- **THEN** the new key takes the default, the previous keys are untouched; the command changes all segments in one undo; the default lives in the file, not in the program's settings
- **TEST** t697 (`src/libs/kt-model/tests/fx_auto.rs`)

#### Scenario: The segment formula is computed on the segment and refuses loudly
- **WHEN** the segment between the keys at 1.0 s (value 0) and 3.0 s (value 1) is assigned the formula `a + (b - a) * s * s`, then `sin(t * 6.283)`; the neighbouring segments are a zigzag and a line; then a formula with an unknown name and with division by zero — by the command and by reading a file
- **THEN** the value on the segment equals the independent oracle at 0, 1/4, 1/2 and 1; the neighbouring segments compute the previous numbers; two runs are byte for byte equal; an unknown name and a division by zero are rejected with the position and the text, a file with such a shape does not load silently; the shape is written to the file and read back the same
- **TEST** t787 (`src/libs/kt-model/tests/fx_auto.rs`), t787b (`src/libs/kt-otio/tests/convert.rs`)

## ADDED Requirements

### Requirement: Curve — keys, segment shapes, formula layer and links combine and are removed separately
The value of a curve SHALL be computed in layers in a fixed order: the keys with segment shapes give `k`; on top — an optional LAYER-formula (text with the variables `k`, `t`, `src`), and on top of that — nothing; a link (`Link`) gives `src` and without a layer acts as before. Setting a layer MUST NOT erase keys and shapes; removing the layer SHALL return exactly `k`; setting a link MUST NOT erase the layer. The source formula (`Formula`) remains a legal file form and SHALL be read as "a layer over empty keys" — that is, the same curve, only without keys.
The variable `t` of ANY formula (layer, source, segment — there from the left key) SHALL be counted in seconds from the start of the curve's MATERIAL, identically in the render and in any preview: a preview that counts `t` from the edge of the visible window would show a different line from the one that sounds. Curves with formulas SHALL be resolved into numbers before the audio stream by the same pass as the links (the frame grid, byte-for-byte determinism); a project without formulas and links MUST NOT pay anything.
The curves of the OVERRIDES of a reel instance SHALL accept the same shapes and sources as the curves of a slot (validation does not reject a formula, a layer and a link inside the reel), and SHALL be resolved into keys IN THE DOMAIN OF THE REEL at the moment of overlaying onto the content, before the translation into the target's time: a formula typed in the instance's window on the instance's axis sounds on that same axis.
A key SHALL appear ONLY from a person's gesture (click, drag, field, button): the clamp of a value by the widget, an external change of the displayed value and the course of playback MUST NOT write a key.
Why: R.67, the owner verbatim: "I can EITHER draw the points myself, or set them in a table, or set a curve over the whole range, or a formula, or set something on separate areas, AND A COMBINATION OF ALL OF THIS"; "if after this I enter a formula, the points will disappear, the zigzag will disappear… maybe I changed my mind". His choice: a formula-layer sees `k`, a formula on a segment is the shape of the stretch. Self-writing keys: the field with a curve showed the value under the cursor, while a `DragValue` with limits clamped a zigzag that went beyond the passport and reported "changed" — in his project lie keys 66.1→0.20, 69.7→0.98, 69.9→0.07, 72.3→0.99, 74.7→0.16 that he did not set.

#### Scenario: A layer over keys, and removal separately
- **WHEN** a curve with three keys and a zigzag on the second segment has the layer `k * 0.5 + 0.1` set; then the layer is removed; then a link and the layer `k + src` are set; then the link is removed
- **THEN** with the layer the values equal `0.5·k + 0.1` at every sample point, the keys and the zigzag are intact; without the layer — exactly the previous `k`; with the link and the layer — `k + src`; after removing the link the layer remains, `src` = 0; each step is one history step; the file carries the layer and is read back as the same curve
- **TEST** t787c (`src/libs/kt-model/tests/fx_auto.rs`), t787d (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The formula's `t` counts from the start of the material, and the preview is the same
- **WHEN** a layer-curve `t` (seconds) is on an event with an entry point of 10 s; the render asks for the value at material time 12 s; the graph preview is built for the window 11..13 s and for the window 5..20 s
- **THEN** the render gives 2.0 (not 12.0 and not 0); both previews give the same line as the render, at the same ticks — independently of the window; on the previous code the preview gave 0 at the left edge of the window
- **TEST** t784 (`src/libs/kt-model/tests/formula.rs`), t784b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: An instance override accepts a formula, a shape and a layer, and sounds on the instance's axis
- **WHEN** on the reel instance (the structure of the owner's project: two instances of one reel one above the other, the curve on the upper one) the override gets a hill of keys 0→1→0, then a zigzag on a segment, then the layer `k * 0.5`, then the source formula `t / 10`
- **THEN** each command is accepted (on the previous code the formula was rejected with `CurveSourceUnsupported`); the value of the target inside the reel at the middle of the hill equals 1·0.5, at the edges 0; the formula `t/10` at the moment "3 s from the start of the instance" gives 0.3 for the target; the frame of the instance changes together with the value; the resolution goes in the domain of the reel before the translation into the material of the target
- **TEST** t792 (`src/libs/kt-model/tests/pubparam.rs`), t792b (`src/libs/kt-engine/tests/nest_param.rs`)

#### Scenario: A key only from a gesture
- **WHEN** a parameter with the limit 0..1 has a zigzag of amplitude 0.1 around 1.0 living on it; the window with the field of this parameter is open; playback runs for 120 frames, the field is not touched; then the same run with the slider; then the person edits the field by hand
- **THEN** after the runs the number of keys is unchanged (on the previous code keys with the value 1.0 appeared on every frame of play); a manual edit places exactly one key at the cursor
- **TEST** t786 (`src/apps/kt-app/src/layout_tests.rs`)
