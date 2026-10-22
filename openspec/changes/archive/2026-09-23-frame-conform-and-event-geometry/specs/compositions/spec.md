# Spec Delta

## MODIFIED Requirements

### Requirement: A reel — a link inside the project in one address space
A reel SHALL be an object inside the document (not a file), and an instance — an ordinary event
whose media address resolves to the reel. A reel SHALL have optional OWN raster and fps
(default — the project; the audio rate is always shared); a reel instance SHALL be conformed into the frame of its
parent by the same rule and the same leading "Geometry" node as a file, with view = the reel's raster
(SAR 1:1); the reel's scale step SHALL be computed from its raster, and the memo keys of the sub-composite
and the reel's preview SHALL include its raster and fps. The identifiers of events, groups,
transitions, morphers and media SHALL be unique across the WHOLE document; fresh addresses MUST
be computed over the whole tree, not from the root. No model reference (transition ends, morpher
targets, link group) MUST cross a reel boundary. The search for the source from an event SHALL
live in one place. A reel that is the document's root SHALL be a legal document.
Why: a reel has no path — otherwise the worker, the watchdog and the index service would take it for a
file; matching ids from different reels would silently exchange material through the format's global maps; a square or vertical composition inside 16:9 is otherwise inexpressible (the owner's decision in R.62, a revision of R.50).

#### Scenario: One address space and the closedness of reels
- **WHEN** a document contains reels, and in different reels the same ids or a reference across the boundary are found
- **THEN** validation rejects the document; fresh addresses are computed as the maximum over the whole tree
- **TEST** t425 (`src/libs/kt-model/tests/nest.rs`), t575 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: A reel does not lag behind the root in content fields
- **WHEN** a new content field is added to the root
- **THEN** a test with exhaustive destructuring of both structures does not compile until it is decided whether the content is the reel's or a property of the document
- **TEST** t426 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: The source from an event is searched in one place
- **WHEN** code goes from an event to its media
- **THEN** it goes through the single home of source resolution; direct accesses to `MediaRef` by event address are forbidden by a scanning guard
- **TEST** t426b (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A reel as the root is a legitimate document
- **WHEN** the current reel is served as the document root (offline count, a player inside the reel)
- **THEN** such a project is valid and renders
- **TEST** t598 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: The reel's own raster is conformed by the rule, the level and keys come from the raster
- **WHEN** a 1080×1080 reel sits in a 1920×1080 project with the rules "fit" and "fill"; the reel's raster is changed by a command
- **THEN** "fit" gives a square with side margins, "fill" cuts the top and bottom; the reel's step is computed from 1080×1080; the memo key of the sub-composite and the reel's preview after the change of raster differ; the command is one undo step
- **TEST** t723 (`src/libs/kt-model/tests/nest.rs`, `src/libs/kt-engine/tests/nest_render.rs`)

### Requirement: Law E — a nested item with an empty chain is identical to its content
A nesting with an empty chain, an identity leading node, level 1, without reversal, warp and
transitions on the edges, in a reel WITHOUT its own raster and fps, SHALL render byte for byte
as the expanded content — for video and for audio, in the preview and in export. A reel with its own
raster or fps is not subject to law E by construction (its frame is a different canvas or a different
grid), and the test SHALL assert this negatively. The order of frames and bursts SHALL go
inward and match the order of the expanded content; the cost SHALL be linear in the number of events
(a reel of 500 events, placed three times). The expansion into effective clips SHALL apply
the instance's overrides.
Why: the inline form is the reference oracle of the real subtree of the plan; an order that did not descend
inside is a permanent cache miss (invariant 4); a reel's own raster is the owner's decision in R.62, and the oracle is honestly bounded rather than blurred by a tolerance.

#### Scenario: Audio and video byte for byte
- **WHEN** a project with a nesting and its expanded version are rendered
- **THEN** audio and video match byte for byte, preview == export
- **TEST** t434 (`src/libs/kt-engine/tests/nest_render.rs`), t435 (`src/libs/kt-engine/tests/nest_render.rs`), t447 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: The order is the same and the cost is linear
- **WHEN** the frame orders and the build cost of a nesting and of the expansion are compared
- **THEN** the orders match; the cost ceiling for a reel of 500 events ×3 is not exceeded
- **TEST** t436 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: The E-law with baked-in overrides
- **WHEN** an instance with overrides is compared with the expanded content into which the values are baked
- **THEN** the result is byte for byte
- **TEST** t688 (`src/libs/kt-engine/tests/nest_param.rs`)

#### Scenario: The reel's own settings take it out of the E-law
- **WHEN** the same reel receives its own raster (or its own fps) and is compared with the expansion
- **THEN** the results differ (a negative oracle), and after resetting the settings to "project" they are again equal byte for byte
- **TEST** t725 (`src/libs/kt-engine/tests/nest_render.rs`)

### Requirement: The reel's internal time is continuous; only the file sheet sets a floor on the grid
The instance's internal time SHALL be the editing time of the reel without rounding to the frame
grid; a procedural source (generator, title, placeholder) SHALL be computed on the continuous
tick; only the file sheet applies the grid floor. A reel with its OWN fps SHALL quantize
all of its internal time to its own grid (a frame holds until the next one, without blending) before
expanding the sub-plan; without its own fps the path is continuous, the bytes are as before. The audio
of the reel does not depend on its fps. Frame blending on a nesting SHALL be inherited downward and
done by the LEAVES (one sub-composite per frame); the flag must travel into the order, the chunk budget,
coverage and the memo key. Source facts SHALL come from whoever opened the source.
Why: at rate 0.5 two neighboring frames got one internal tick, and titles,
automation and cameras inside went in steps of two frames; two sub-composites would be twice as expensive and would bring back the steps; the reel's own fps (the owner's decision in R.62) is precisely the intended steps, so it is enabled only by an explicit field.

#### Scenario: A slowed reel animates in half steps
- **WHEN** an instance is slowed by half, with a title with automation and a file inside
- **THEN** the even frame equals the frame of rate 1, the odd one — the frame of the reel at the half frame; the footage is pinned to the grid, the order does not move
- **TEST** t579 (`src/libs/kt-engine/tests/nest_render.rs`), t580 (`src/libs/kt-engine/tests/nest_render.rs`), t585 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: Blending on a nested reel is done by the leaves
- **WHEN** frame blending is enabled on the instance
- **THEN** the result equals the blend of the neighboring frames of the SOURCE inside; the order carries both sides; "nearest" holds the frame
- **TEST** t581 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: The same on GPU
- **WHEN** the slowed reel and blending are executed on GPU
- **THEN** conformance is EXACT with CPU, a half frame ≠ a frame, blending ≠ nearest, determinism
- **TEST** t586 (`src/libs/kt-gpu/tests/gpu_render.rs`), t587 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: The reel's own fps holds a frame
- **WHEN** a reel at 12 fps with an animated title sits in a 24 fps project; the same reel without its own fps
- **THEN** with fps the frames go in pairs (the odd one equals the even one), the automation and cameras inside are quantized; without fps — t579/t580 with their previous bytes; on GPU conformance is EXACT
- **TEST** t724 (`src/libs/kt-engine/tests/nest_render.rs`), t724b (`src/libs/kt-gpu/tests/gpu_render.rs`)
