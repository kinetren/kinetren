# Spec Delta

## ADDED Requirements

### Requirement: The segment shape lives on the key and has its own parameters
The interpolation form SHALL belong to a KEY and describe the segment AFTER it, not the curve
as a whole. The set of forms is closed, and each has its own parameters: linear (no parameters),
third order (tension), step (no parameters), ease in and ease out (tension), zigzag
(number of periods, amplitude, profile — triangle/rectangle/sine). The zigzag amplitude
SHALL be set IN UNITS OF THE PARAMETER and clamped by the target's passport, not as a fraction
of the difference between the keys: the most common zigzag is an oscillation around a CONSTANT value, where the difference
is zero. Each form SHALL be a closed, stateless function `(segment fraction, ends, parameters)`.
The default form for new keys is a PROJECT field, not a machine setting; changing the
default MUST NOT touch keys that are already set. The command "apply form to all
segments" SHALL exist for a curve and for an event and SHALL land as one step in the history.
The value at the last key is the final one; the form of the last key has no effect.
Why: a single boolean for the whole curve expresses neither "this segment as a step" nor a zigzag;
a machine setting would give "one file — two behaviors" (the Vegas lesson), and an amplitude taken from the
difference would collapse the zigzag exactly where it is needed.

#### Scenario: Shapes are computed closed and deterministic
- **WHEN** a segment is computed by each form at 0, 1 and the middle, with extreme tension values, for the zigzag with a fractional number of periods and on a constant segment
- **THEN** the ends of the segment equal the key values for ALL forms except step (for step the left value holds until the next key); the zigzag on a constant segment oscillates with the given amplitude in parameter units and is clamped by the passport; two runs give identical bytes
- **TEST** t695 (`src/libs/kt-model/tests/fx_auto.rs`)

#### Scenario: The default reproduces the former smoothing byte for byte
- **WHEN** a curve created without specifying a form is compared with the frozen oracle of the previous `curve_value(keys, ease = true, at)`
- **THEN** the values match bit for bit at all sample points; `GOLDEN_AUDIO`, `GOLDEN_VIDEO` and `GOLDEN_TITLE_VIDEO` do not move
- **TEST** t695b (`src/libs/kt-model/tests/fx_auto.rs`), t11 and t67 (`src/libs/kt-engine/tests/golden_export.rs`)

#### Scenario: Speed accepts only linear and refuses loudly
- **WHEN** any form other than linear is assigned to a key of the `rate` curve — from the interface and by command
- **THEN** the command is rejected with the stated reason (the closed form of the inverse warp map rests on a piecewise-linear profile), the status line shows an error rather than silence; the warp map computes the previous numbers
- **TEST** t696 (`src/libs/kt-model/tests/warp.rs`), t696b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The project default and the "to all" command
- **WHEN** the form default in the project is changed, then a new key is set; then the form is applied to all segments of the curve and of the whole event
- **THEN** the new key takes the default, the earlier keys are untouched; the command changes all segments in one undo; the default lives in the file, not in the program settings
- **TEST** t697 (`src/libs/kt-model/tests/fx_auto.rs`)

### Requirement: Link — a value drives another property, with a recalculation expression
`CurveSource::Link` SHALL address a source property and carry an optional recalculation
expression. The address SHALL point to any event of the PROJECT within ONE reel and MUST NOT
cross the reel boundary in either direction: outward — because the reel would stop being self-contained and the
law E (nesting is identical to the expanded content) would stop holding; inward — because public composition
parameters already exist for that purpose.
The common denominator of time for a link between DIFFERENT events is the TIMELINE tick: its own material position is converted to the timeline and back to the source material by the same
function as the rest of the program; outside the source's body its extreme value is taken.
Within ONE event no conversion is needed — the domain is shared.
Non-numeric properties SHALL be tracked derivatively: a flag by a threshold, a choice, text, media and color by snapshots at marks on the VALUE axis of the leading property (between marks the previous
snapshot holds). A dependency cycle SHALL be rejected by validation that traverses from ALL vertices and prints the
path. Source resolution SHALL run in topological order once per renderer build, before the audio stream; there is no work in the compute callback.
Why: the owner asked for a "view curve from another curve of the same event" and chose the
address space of the whole project; the rig axis is the VALUE, not time, so a snapshot switches
the font the same way a number moves the size.

#### Scenario: A link inside an event and between events
- **WHEN** a property is driven by another property of the same event; then by a property of a DIFFERENT event placed elsewhere on the timeline, including under trim and reverse
- **THEN** within the event the values match at the same material tick; between events they match at the same TIMELINE tick; beyond the source's body its extreme value is taken
- **TEST** t702 (`src/libs/kt-model/tests/links.rs`)

#### Scenario: A cycle is rejected with its path
- **WHEN** property A drives B and B drives A; then a cycle of length three through different events; then a property drives itself
- **THEN** validation rejects the document and prints the path; the search runs from all vertices, not from the root; a project without links is valid as before
- **TEST** t703 (`src/libs/kt-model/tests/links.rs`)

#### Scenario: Non-numeric through thresholds and snapshots
- **WHEN** a number drives a flag through a threshold; a choice, text, a media reference and a color are driven through snapshots at marks
- **THEN** the flag switches at the threshold and does not flicker on equality; between marks the previous snapshot holds; below the first mark the first snapshot applies; a snapshot of a non-numeric type into a numeric target is rejected by validation
- **TEST** t704 (`src/libs/kt-model/tests/links.rs`)

#### Scenario: The reel boundary is not crossed
- **WHEN** a link is created from an event inside a reel to an event of the root, and back
- **THEN** both are rejected with the stated reason and a hint about public parameters; law E on a reel with links inside it holds byte for byte
- **TEST** t702b (`src/libs/kt-model/tests/links.rs`), t688 (`src/libs/kt-engine/tests/nest_param.rs`)

### Requirement: Formula — the source of a value, evaluated by its own evaluator
`CurveSource::Formula` SHALL define a curve as TEXT over time and addressable properties, without
keys. The evaluator is its own, with a CLOSED set of operations (arithmetic, comparisons, branching,
`min`/`max`/`clamp`/`abs`/`floor`, trigonometry, `lerp`, `noise` from an integer seed), with no
loops, no access to the file system, clock, or randomness from the environment. The same
text with the same inputs SHALL always and everywhere produce the same bytes (the determinism
invariant). Division by zero, overflow and `NaN` SHALL produce a NAMED parse or evaluation
error, not a silent zero. The formula text SHALL be part of the post-FX key and of the chain
hash: otherwise editing the formula would leave the old frames on screen. The formula SHALL
resolve into numbers BEFORE the audio stream, in the same place as the curve profiles.
A project without links and formulas MUST NOT pay anything: no plan is built, the chain
is borrowed, the bytes are as before.
Why: the owner's decision — "your own mini-formula, computed live on every frame, no external
interpreters"; changing a third-party engine changed the result of finished projects in After Effects, and executable code in a document is the Blender lesson.

#### Scenario: The evaluator is closed and deterministic
- **WHEN** the expressions of the set are parsed and computed, then an expression with an unknown name, with division by zero, with an unclosed parenthesis, and with an attempt to access machine time
- **THEN** the set is computed identically in two runs; each of the four errors is named by its own text with a position in the line; none of them gives a silent zero
- **TEST** t701 (`src/libs/kt-model/tests/formula.rs`)

#### Scenario: A formula as a curve without keys
- **WHEN** a property's source is a formula of time; then a formula that reads another property; then a formula on the warp speed
- **THEN** the values match the independent test oracle; reading another property uses the same address as a link; on speed the formula is rejected by the same rule as a non-linear form
- **TEST** t707 (`src/libs/kt-model/tests/formula.rs`)

#### Scenario: The formula text in the post-FX key
- **WHEN** ONLY the formula text is edited, with the same effect parameters
- **THEN** the post-FX key changes, the frame is recomputed; memoization does not return the previous frame
- **TEST** t708 (`src/libs/kt-engine/tests/postfx_cache.rs`)

#### Scenario: Zero cost without links and formulas
- **WHEN** a project where there is not a single derived source is rendered, and a project where a formula sits on ONE event out of many
- **THEN** in the first, the bytes and the plan are byte-for-byte as before, and there is no dependency graph at all; in the second, the plan of neighboring events has not changed, and the build cost has not grown with the number of events
- **TEST** t705 (`src/libs/kt-engine/tests/auto_plan.rs`), t710 (`src/libs/kt-engine/tests/cost.rs`)

### Requirement: Sources of a curve value — keys, link, formula; a plugin remains an interface
A curve SHALL have a value-source field: `Keys` (default, not serialized —
the bytes of files with keys do not change), `Link` and `Formula`. All three SHALL be WORKING
sources; the behavior of links and formulas is described in separate requirements. A source whose
address does not resolve or whose text does not parse SHALL be rejected LOUDLY on load
and on command — a silent "by keys" render would show something other than what is written in the file.
The plugin SHALL remain ONLY an interface: a pure function with no clock, I/O or state between frames; on load it is refused for a declared state or tail; it never
enters the golden-cycle registry.
Why: the slot for sources was set up in R.59 in advance precisely so that the evaluator could arrive in the next round without breaking the format; the owner left plugins for later.

#### Scenario: An unresolvable source is rejected
- **WHEN** a curve's source has an address pointing to a vanished property or has unparseable text
- **THEN** a project with ordinary curves is valid; with an unresolvable source it is an explicit error with the address and the reason
- **TEST** t649 (`src/libs/kt-model/tests/props.rs`)

#### Scenario: The plugin remains an interface
- **WHEN** the plugin contract is checked
- **THEN** a declared state or tail is a refusal on load; the plugin does not enter the golden registry
- **TEST** t649 (`src/libs/kt-model/tests/props.rs`)

## MODIFIED Requirements

### Requirement: Copy-paste of effects, chains and envelopes — one model, one paste
Buffer blocks (a slot, a chain with the donor's `src_in`, an envelope with a effect-kind tag) SHALL live in the model
and be serialized in the same form to the system clipboard. **The system clipboard SHALL be
the only source of paste**: there is no mirror of the buffer inside the program, the paste button NEVER
turns off, the content is read ON PRESS, and content that cannot be parsed gives an error with
a reason in the status line. Paste SHALL go through ONE append function for all carriers (event, morpher, preset): keys are reattached "input to input" without
scaling, negative time is clamped to zero, curves are extended to the length of the
chain, and the donor's `fx_off` is NOT applied to the receiver. An envelope going into a DIFFERENT effect kind SHALL
be pasted with field matching: those matched by name and type land silently; for the remainder a
window "curve → target parameter" opens; not a single curve is silently lost. A carrier without
curves as a type MUST NOT exist. Paste returns commands — one undo step. When copying, the name gets the SUFFIX "copy" by default, not a prefix.
Why: two truths about one action gave a silent loss of animation; a mirror of the buffer
inside the program turned the button off based on content copied in another window — a state that must not exist (the owner's complaint in R.61).

#### Scenario: Slot, chain and envelope survive the buffer
- **WHEN** a chain with slots, bypass and keys, a separate slot and an envelope are copied, and a block is run through JSON
- **THEN** the chain is pasted onto the target material in one undo, the slot is appended to the end, the envelope lands in its kind, the block equals itself after the round trip
- **TEST** t215 (`src/libs/kt-model/tests/fx_auto.rs`)

#### Scenario: One paste function for an event and a morpher
- **WHEN** an event block is pasted into a morpher (input = 0) and back into an event with a different entry point, then a second block is appended
- **THEN** keys are computed from the donor's entry point in both directions, the slot state moves over, `fx_off` is not inherited, the second block lands at the end in parallel with the curves
- **TEST** t342 (`src/libs/kt-model/tests/fx_auto.rs`)

#### Scenario: Pasting into a morpher window carries curves
- **WHEN** a chain with a curve from the buffer is pasted into the morpher window
- **THEN** the keys arrived and were recomputed into the block's domain
- **TEST** t343 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The copy is named with a suffix
- **WHEN** a named item is pasted through the prefix window with its defaults
- **THEN** the name gets the suffix "copy" (the prefix is a toggle in the same window), repeats are numbered through the shared unique name
- **TEST** t263 (closed in `t250_paste_prefix_dialog`, `src/apps/kt-app/src/layout_tests.rs`), t249 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: Paste reads the real clipboard and explains a refusal
- **WHEN** an envelope is copied, then foreign text is put on the system clipboard, then the clipboard is empty
- **THEN** the button is active in all three cases; in the first the paste happens; in the second and third there is an error with a reason, and there is no clipboard state inside the program in the code
- **TEST** t699 (`src/apps/kt-app/src/layout_tests.rs`), t699b (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Field remapping for a foreign kind of effect
- **WHEN** an envelope is pasted into a different effect kind: some names matched, some did not, some matched by name but not by type
- **THEN** those matched by name AND type landed silently; for the rest a matching window opened with a list of free target parameters; refusing the matching does not silently lose the curve but names it
- **TEST** t700 (`src/libs/kt-model/tests/fx_auto.rs`), t700b (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Key domain — the source media tick
The position of an automation key SHALL be a tick of the MATERIAL in the `source_in` coordinate system,
not a fraction of the event's length and not a timeline tick. Attachment to the material is ensured by the
REPRESENTATION: trim, split, reverse and moving an event do not touch the curves at all; a key
outside the visible piece is legal (hidden by the trim). The conversion "timeline tick → material" is one
function for the whole project (the engine, UI and tests all call the same one); a curve is sampled at the
grid-aligned material position of the frame. The value beyond the curve's edges is the extreme one;
the domain is not clamped at the top or bottom. Anything that moves the read anchor `source_in` NOT together with the
material (cutting under a warp, swapping the source with a morpher layer) MUST shift the keys by the
same delta. Audio is automated by the same mechanism: curves are converted into profiles once
when the renderer is built; slowed&reverb has no second domain.
The shape of a segment lives ON THE KEY and refers to the segment after it; when an event is cut, the shape
of the last key of the left half is kept, and the value at the joint is the same on both sides.
A link between DIFFERENT events is computed on the common denominator — the TIMELINE tick, through the
same conversion function; there is no second rule for converting time in the program.
Why: the "event fraction" domain was rubbery (trimming the end moved the middle of the animation;
Split duplicated the curve into both halves); the owner's decision — "like in Premiere".

#### Scenario: Geometry edits do not move keys
- **WHEN** an event with a curve is trimmed at both ends, split in half and reversed
- **THEN** the keys in both halves are byte-for-byte as before, the curve value at the split joint is the same on both sides, the shape of the last key of the left half is preserved
- **TEST** t212 `t212_keys_stick_to_material_through_edits` (`src/libs/kt-model/tests/fx_auto.rs`)

#### Scenario: Curve value outside the clip body
- **WHEN** a curve is read before the first key, after the last key and at a negative position
- **THEN** the extreme value is returned (transition handles and pre-roll are legal), one point is a constant, an empty curve has no effect
- **TEST** t190 `t190_curve_value_semantics` (`src/libs/kt-model/tests/fx_auto.rs`)

#### Scenario: Old files with a fractional domain are read
- **WHEN** a file older than v3 is opened, where keys are recorded as fractions of the event length
- **THEN** keys are converted into the material's ticks according to the geometry of their own clip (mirrored for a reversed one); saving raises the version
- **TEST** t212 `t212_pre_v3_curves_migrate_to_material_ticks` (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Audio by curve, same mechanism
- **WHEN** an audio event has a gain curve, and an old file has legacy slowed keyframes
- **THEN** the volume follows the curve along the material; without a curve the signal is untouched byte for byte; the legacy field is converted into curves on read
- **TEST** t213 (`src/libs/kt-engine/tests/fx.rs`)

#### Scenario: Source substitution by a layer carries the event's curves
- **WHEN** a morpher layer with a source substitution shifts the `source_in` of an event with its own animation
- **THEN** the curve value in the middle of the event is the same as without the substitution — the keys are shifted by the same delta as the anchor
- **TEST** t291 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: Cutting a stretched clip, keys travel with the anchor
- **WHEN** an event under a warp (including a reversed one, with a speed curve) is cut in the middle
- **THEN** the piece lands in that material (the integral of speed, not linear), the keys are moved by the anchor's delta, the seam has no break, and a clip without a warp is byte for byte as before
- **TEST** t270, t270b, t270c, t270d, t270e (`src/libs/kt-model/tests/morph_slice.rs`)

### Requirement: Compiled video automation and zero cost
Curves of the video path SHALL be compiled ONCE when the renderer is built into a plan (slot,
row index of the passport, keys, segment shape); on a frame — a write by index without a search by
name and without a clone of the chain. Derived sources (link, formula) SHALL be resolved in the
same place and in TOPOLOGICAL order, once per build. A clip without live curves (no
curves, a curve without keys, an orphaned curve) MUST borrow the chain, not copy it.
Curves and links on one event MUST NOT change the plan of a neighbor. The post-FX hash is computed from the
raw curves, before compilation, and includes the formula text and the link address: the cache keys do not depend on the
method of addressing, but they DO necessarily depend on the CONTENT of the source.
Why: the owner's requirement "a simple project does not lag, physics on one source does not
weigh down the whole project"; in audio this rule has been in place from the start.

#### Scenario: The plan equals the frozen named path
- **WHEN** for all effect kinds, both ends of the range, segment shapes, reversal and orphaned curves, the plan is checked against a verbatim copy of the previous named path that lives in the test
- **THEN** the numbers match bit for bit, the orphan is not compiled, the clamp matched between the two paths
- **TEST** t643 (`src/libs/kt-model/tests/props.rs`)

#### Scenario: Nothing to compile means no plan
- **WHEN** a chain has no curves, its curves are empty or have no keys
- **THEN** compilation returns `None`
- **TEST** t643b (`src/libs/kt-model/tests/props.rs`)

#### Scenario: A clip without curves borrows the chain
- **WHEN** an event without curves and an event with an orphaned curve are rendered
- **THEN** the layer's chain is borrowed (not copied); only the clip with a live curve copies it
- **TEST** t644 (`src/libs/kt-engine/tests/auto_plan.rs`)

#### Scenario: A neighbour's curves do not affect others
- **WHEN** an event has curves or links, and its neighbors have none
- **THEN** the neighbors' plan is byte-for-byte the same as in a project without curves
- **TEST** t645 (`src/libs/kt-engine/tests/auto_plan.rs`), t705 (`src/libs/kt-engine/tests/auto_plan.rs`)

## REMOVED Requirements

### Requirement: Expressions and plugins — interfaces only
**Reason**: the requirement's basis has disappeared — the formula and the link stopped being a place in the schema and
became working sources of the value (the owner's decision in R.61). Keeping next to it the requirement
"validation MUST reject the formula" and the requirement "the formula works" would mean having two truths in the spec about one field.
**Migration**: replaced by the requirement "The value sources of a curve are keys, link, formula;
the plugin remains an interface". The part about the plugin was carried over verbatim and remains in force;
scenario `t649` was rewritten from "formula is rejected" to "an unresolvable source is rejected".
