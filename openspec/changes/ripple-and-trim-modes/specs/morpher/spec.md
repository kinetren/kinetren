## ADDED Requirements

### Requirement: A ripple never detaches a reference-mode layer
A ripple that moves the targets of a morpher layer in `Marks` or `Areas` mode SHALL move the referenced
markers and areas with them; when the chosen scope does not move markers and areas, such a ripple SHALL
be refused naming the layer and its reference.
Why: the layer takes its window from the reference by name (F-TL-001); a silent drift lands the patch on
someone else's piece.

#### Scenario: The layer keeps its pieces after a ripple
- **WHEN** a layer in `Marks` mode refers to a range marker over two events after an edit point, and a ripple closes 1 s before them, in a scope with markers and in one without
- **THEN** with markers the marker moved and `effective_clips` patches the same two pieces; without markers the ripple is refused naming the layer and the marker
- **TEST** t3432 (`src/libs/kt-model/tests/ripple.rs`)
