## ADDED Requirements

### Requirement: Mask rows in the passport; curves in the mask; the path is keyed as a whole
A mask SHALL have a passport built at run time: opacity, feather, expansion and its shape's rows —
centre, size, rotation (and roundness) of a primitive, and per vertex of a path `v<id>.x/.y/.ix/.iy/.ox/.oy`
with the vertex id as a stable address (deleting a vertex never renames another). Mode, invert and
falloff are static. The curves SHALL be `ParamCurve`s held by the mask; the domain of a picture or
slot mask is the event's material tick, as for the effect curves. A path SHALL be keyed as a whole:
the mask's stopwatch keys every shape row at the playhead, and once the path is animated a gesture on
any point keys every shape row at that tick. Inserting a vertex into an animated path SHALL key the new
vertex at every key of the path at the same curve parameter, so the shape at the keys does not change;
deleting a vertex deletes its curves. A mask row SHALL be a link target and a link source
(`self/mask<id>/<row>`, `self/fx<N>.mask<id>/<row>`), resolved with the other derived sources. A curve on
a TRACK mask is refused loudly, like every track curve.
Why: F-MSK-001 trick 3 — a shape interpolates by point correspondence, so points sharing key times
is what a person expects (AE's "Mask Path"); the tracker (T1) drives a point through a link.

#### Scenario: Passport rows, evaluation, stable vertex ids
- **WHEN** a rectangle and a path mask list their rows; a curve on `feather` and on `v2.x` is evaluated between keys; vertex 1 is deleted
- **THEN** the rows match the shape with the passport's limits; the values follow the curves at the material tick; after the deletion `v2.x` keeps its name and its curve
- **TEST** t1261 (`src/libs/kt-model/tests/masks.rs`)

#### Scenario: Path keyed as a whole, insertion keeps the shape at the keys
- **WHEN** a path is keyed by the stopwatch at t0, one vertex is moved at t1; then a vertex is inserted into a segment
- **THEN** every shape row has keys at t0 and t1; after the insertion the evaluated shape at t0 and at t1 passes through the same points as before (the new vertex lies on the old curve at both keys)
- **TEST** t1275 (`src/libs/kt-model/tests/masks.rs`)

#### Scenario: A mask point is driven by a link and a formula
- **WHEN** `v0.x` of a picture mask is linked to an effect parameter of the same event; the feather of a slot mask is a formula; a mask row is the source of another event's link; a cycle through a mask is made
- **THEN** the resolved values follow the leaders at the same material/timeline tick; the cycle is rejected with its path
- **TEST** t1263 (`src/libs/kt-model/tests/masks.rs`)

#### Scenario: Commands, undo and the chain's parallel lists
- **WHEN** masks are set on an event, a slot and a track by commands and undone; the chain is shortened by `SetFx`; a slot with masks is copied and pasted
- **THEN** each command is one undo step; `fx_masks` is trimmed with the chain; the pasted slot carries its masks with keys re-hung on the target's entry
- **TEST** t1262 (`src/libs/kt-model/tests/masks.rs`)
