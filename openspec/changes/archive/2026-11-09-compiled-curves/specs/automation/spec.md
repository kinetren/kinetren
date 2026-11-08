## ADDED Requirements

### Requirement: A formula is compiled once per text and evaluated compiled
A formula (a segment formula `SegShape::Formula`, a layer formula, a link's recomputation) SHALL be
parsed once per text and compiled into closures over numbered variable slots
(`Formula::compile`); every per-sample and per-frame evaluation SHALL use the compiled form, with no
lock, no text clone, no map of names and no allocation. The compiled form SHALL give the
interpreter's bits and the interpreter's error for every input (invariant 11): the same IEEE
operations in the same order, operands left to right, the first error wins, `if` evaluates only the
branch it takes, an unknown name fails at evaluation. A curve read at non-decreasing positions (the
sampling of a speed curve, the resolution of a link or formula on the frame grid, the formula
preview) SHALL go through a cursor (`CurveCursor`) that gives `curve_value`'s bits for any position.
An event with nothing left to resolve SHALL NOT be cloned by the link pass, and a stream stage SHALL
apply its own slot's curves (`AutoPlan::apply_slot`), not clone the chain per position.
Why: the owner's JIT verdict (H16/H17, "про JIT я вообще-то имел ввиду не только шейдеры"): compiled
CPU math is tier T1. A segment formula took a global lock, a text clone, a string-keyed lookup and a
map of four allocated names per reading — per sampled 5 ms point of a speed curve, per frame of a
link, per pixel of a graph.

#### Scenario: Compiled equals interpreted
- **WHEN** 600 formulas (every operator, function, `if`, division by zero, an unknown name, overflow and random trees) are evaluated on 40 inputs both ways
- **THEN** the values are equal to the bit and the errors are the same kind
- **TEST** t2100 (`src/libs/kt-model/tests/compiled_curves.rs`)

#### Scenario: A segment formula reads compiled; a cursor reads as the halving
- **WHEN** segment formulas (one with an error point, one with an unknown name) are read at random positions, and a 300-key curve of every shape with duplicate ticks is read by a cursor forward and backward
- **THEN** each value equals the former interpreter's and `curve_value`'s bits, and 2000 readings of a formula segment parse nothing and allocate nothing
- **TEST** t2101 (`src/libs/kt-model/tests/compiled_curves.rs`)

#### Scenario: Derived curves resolve to the interpreted values
- **WHEN** a formula layer over keys is resolved on the frame grid and previewed, and a reel whose only derived curve is a speed formula layer is resolved
- **THEN** every resolved key equals the interpreter's value at its tick, the preview reads the same numbers, and the reel is returned borrowed
- **TEST** t2109 (`src/libs/kt-model/tests/compiled_curves.rs`)

#### Scenario: One slot of the plan equals the whole plan
- **WHEN** a two-slot chain with curves is resolved at 40 positions, whole and slot by slot
- **THEN** each slot applied alone equals that slot of the whole plan
- **TEST** t2110 (`src/libs/kt-model/tests/compiled_curves.rs`)
