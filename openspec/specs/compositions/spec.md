# Compositions (nested reels)

## Purpose

A piece of the edit is collapsed into a reel — an object inside the project, referenced by
instance-events. The reel sets the boundary of the processing order: the chain, level, reversal and
speed of an instance act on the ASSEMBLED picture and the shared sub-mix, not on each
piece; an edit inside reaches all places at once; an instance differs from the template
only by its published handles.

## Requirements

### Requirement: A reel — a link inside the project in one address space
A reel SHALL be an object inside the document (not a file), and an instance — an ordinary event,
whose media address resolves to the reel. A reel SHALL have optional ITS OWN raster and fps
(the default is the project; the audio frequency is always shared); an instance of a reel SHALL be conformed to the frame
of its parent by the same rule and the same leading "Geometry" node as a file, with the view = the reel's raster
(SAR 1:1); the level of the reel SHALL be computed from its raster, and the memo keys
of the sub-composite and the reel's preview SHALL include its raster and fps. The identifiers of events, groups,
transitions, morphers and media SHALL be unique across the WHOLE document; fresh addresses MUST
be computed over the whole tree, not over the root. No model reference (the ends of a transition, the targets
of a morpher, a link group) MUST cross the boundary of a reel. The search for a source from an event SHALL
live in one place. A reel standing as the root of the document SHALL be a legitimate document.
Why: a reel has no path — otherwise the worker, the watchdog and the index service would take it for a
file; identical ids from different reels would silently exchange material through the format's global maps; a square or vertical composition inside 16:9 is otherwise inexpressible
(the owner's decision R.62, revision R.50).

#### Scenario: One address space and the closedness of reels
- **WHEN** the document contains reels, and identical ids or a reference across the boundary occur in different reels
- **THEN** validation rejects the document; fresh addresses are computed as the maximum over the whole tree
- **TEST** t425 (`src/libs/kt-model/tests/nest.rs`), t575 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: A reel does not lag behind the root in content fields
- **WHEN** a new content field is introduced at the root
- **THEN** the test with an exhaustive destructuring of both structures does not compile until it is decided whether the content belongs to the reel or is a property of the document
- **TEST** t426 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: The source from an event is searched in one place
- **WHEN** code looks from an event to the media
- **THEN** it goes through the single home of source resolution; direct accesses to `MediaRef` by an event's address are forbidden by a scanning guard
- **TEST** t426b (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A reel as the root is a legitimate document
- **WHEN** the current reel is served as the root of the document (offline count, the player inside the reel)
- **THEN** such a project is valid and renders
- **TEST** t598 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: The reel's own raster is conformed by the rule, the level and keys come from the raster
- **WHEN** a 1080×1080 reel sits in a 1920×1080 project with the rules "fit" and "fill"; the reel's raster is changed by a command
- **THEN** "fit" gives a square with bars on the sides, "fill" cuts the top and bottom; the level of the reel is computed from 1080×1080; the memo key of the sub-composite and the reel's preview after the change of raster are different; the command is one undo step
- **TEST** t723 (`src/libs/kt-model/tests/nest.rs`, `src/libs/kt-engine/tests/nest_render.rs`)

### Requirement: A reel tree has no cycles and is never deeper than four levels
The cycle of reels SHALL be searched by an iterative traversal from all vertices (not only from the root), and the refusal
SHALL name the reels by their path. The nesting depth SHALL be limited to 4 for EACH
reel, including those not placed in the edit; the refusal names the reel.
Why: a pair of reels that is placed nowhere is still a cycle — a file with it would be saved and would not open;
the limit is the calculated budget of accumulators per level (~25 MiB at 1080p out of
128 MiB), not a round number.

#### Scenario: A cycle is rejected with names
- **WHEN** reel A references B, and B references A, and neither stands at the root
- **THEN** validation rejects the document and prints the path "A -> B -> A"
- **TEST** t423 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: The depth limit is computed for each reel
- **WHEN** a reel from the bin, when placed, would give a depth > 4
- **THEN** the refusal happens where the reel was assembled, and names it
- **TEST** t424 (`src/libs/kt-model/tests/nest.rs`)

### Requirement: Collapse and expand are an identity, and with a selection one rule moves
Collapsing SHALL gather the body of the reel by the same rule as copying (events, transitions, morpher layers, cameras, the row order); events move with their former ids. Areas and markers
SHALL remain at the root; a layer in reference mode (markers/areas) SHALL remain outside
and be named in the report. What has moved inward MUST be removed from the root. The instance SHALL be a pair of video+audio in one link group on the topmost of the collapsed tracks; a transition with one
end outside SHALL be re-hung onto the instance. Expansion SHALL cut the content by the instance's
window with the single piece rule, insert tracks, remove the chain, level,
reversal and warp of the instance, and name what was removed; "collapsed — expanded" SHALL be an identity
in one undo step. An occupied reel MUST NOT be deleted; the media is held by the whole tree.
Why: two rules for "what travels with the selection" would diverge silently; a layer without a row order
inside a reel died (morph rows last — the "everything below" mode covers nothing), cameras were lost, and a layer in two places was applied twice.

#### Scenario: Identity and one history step
- **WHEN** the selection is collapsed into a reel, then the instance is expanded
- **THEN** the project is byte-for-byte as before, and this is one undo step
- **TEST** t427 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: The removed chain is named
- **WHEN** an instance with a chain, level, reversal or warp is expanded
- **THEN** they are not scattered over the pieces but are removed, and the report names each
- **TEST** t428 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: An occupied reel does not go away, a transition is re-hung, media is held by the tree
- **WHEN** a reel with instances is deleted; a piece with a transition at its edge is collapsed; a file that stands only inside the reel is deleted
- **THEN** deleting the reel is rejected; the transition is re-hung onto the instance; the file is held by the whole tree
- **TEST** t429 (`src/libs/kt-model/tests/nest.rs`), t430 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: A layer, a camera and the row order move inward
- **WHEN** an area under a morpher layer and a camera is collapsed
- **THEN** the layer inside covers the same events as outside; the camera has moved, is rebased, and its row is created; at the root they are no longer present; the reference layer remained outside and is named
- **TEST** t557 (`src/libs/kt-model/tests/nest.rs`), t558 (`src/libs/kt-model/tests/nest.rs`), t559 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: Collapsing and expanding are reachable by hand
- **WHEN** a person goes through the event's menu
- **THEN** collapsing, the name window (again) and expanding work; "unlink from other places" is shown only when N > 1
- **TEST** t430b (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Format — `Stack` as the child of `Track`, sidecar is advisory
In the EXCHANGE file an instance SHALL be written as a `Stack` child of a `Track` with the body of the reel,
inlined into each instance; on reading, the instances collapse by the address of the reel, and
diverged bodies SHALL become separate reels with a notice. The sidecar of reels (the number of
tracks, the row order, morphers, cameras, the layer map) SHALL be advisory: if it is missing or does not match — a notice and reading of the edit from the file. Foreign nested `Stack` and `Track` inside a `Track`
SHALL be read as real reels. The version cascade MUST NOT move files without
attachments.
In the PROJECT file a reel SHALL be stored ONCE with its own body, and an instance SHALL be a
reference to its address; there is no inline there. The difference is one of addressing, not of taste: the exchange needs self-containedness (the reader does not know our conventions), the project needs non-duplication.
Why: object identity does not exist in OTIO JSON; a silent choice of one of two diverged bodies is the loss of someone else's work; an old build would read the attachment as a placeholder and on rewriting would return the live edit as dead raw material. R.70: the same fact — the absence of object identity — makes inlining for the PROJECT a multiplicative cost: the owner's three reels gave 15 220 clips and 115 MB.

#### Scenario: The round trip of a file with an attachment
- **WHEN** a project with a reel is saved and read in ITS OWN format; separately — exported to OTIO and read back by import
- **THEN** in both cases the reel, the instances and their ids survive the round trip, and idempotency is kept; in the project the body of the reel is written once, in the export — inlined into each instance
- **TEST** t431 (`src/libs/kt-otio/tests/convert.rs`), t810 (`src/libs/kt-model/tests/project_file.rs`)

#### Scenario: Diverged bodies and an advisory sidecar
- **WHEN** a foreign editor changed one instance and did not change the other; the sidecar is missing
- **THEN** the second body becomes a separate reel with a notice; without the sidecar the edit is read from the tracks
- **TEST** t432 (`src/libs/kt-otio/tests/convert.rs`), t433 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The version cascade does not move old files
- **WHEN** all committed `.otio` files without attachments are re-saved by export
- **THEN** byte-for-byte and with the same version
- **TEST** t433b (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Foreign attachments are real reels
- **WHEN** `nested_example.otio` is read
- **THEN** three reels, not a single placeholder; the round trip is idempotent
- **TEST** t437 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Law E — a nested item with an empty chain is identical to its content
An attachment with an empty chain, an identity leading node, level 1, without reversal, warp, and transitions at the edges, of a reel WITHOUT its own raster and fps, SHALL render byte for byte as
the expanded content — for video and for audio, in the preview and in the export. A reel with its own
raster or fps is not subject to the E-law by construction (its frame is another canvas or another
grid), and the test SHALL assert this negatively. The order of frames and bursts SHALL go
inward and coincide with the order of the expanded content; the cost SHALL be linear in the number of events
(a reel of 500 events, placed three times). The expansion into effective clips SHALL apply
the overrides of the instance.
Why: the inline is the reference oracle of the real sub-tree of the plan; an order that did not descend inward
is a permanent cache miss (invariant 4); the reel's own raster is the owner's decision R.62, and the oracle is bounded honestly rather than blurred by a tolerance.

#### Scenario: Audio and video byte for byte
- **WHEN** a project with an attachment and its expanded version are rendered
- **THEN** audio and video match byte for byte, preview == export
- **TEST** t434 (`src/libs/kt-engine/tests/nest_render.rs`), t435 (`src/libs/kt-engine/tests/nest_render.rs`), t447 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: The order is the same and the cost is linear
- **WHEN** the frame orders and the assembly cost of an attachment and of its expansion are compared
- **THEN** the orders match; the cost ceiling for a reel of 500 events ×3 is not exceeded
- **TEST** t436 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: The E-law with baked-in overrides
- **WHEN** an instance with overrides is checked against the expanded content into which the values are baked
- **THEN** the result is byte for byte
- **TEST** t688 (`src/libs/kt-engine/tests/nest_param.rs`)

#### Scenario: The reel's own settings take it out of the E-law
- **WHEN** the same reel receives its own raster (or its own fps) and is checked against the expansion
- **THEN** the results differ (a negative oracle), and after resetting the settings to "project" they are equal again, byte for byte
- **TEST** t725 (`src/libs/kt-engine/tests/nest_render.rs`)

### Requirement: Law G — the processing boundary, premultiplication at the boundary
The level, chain, reversal and speed of an instance SHALL act on the ASSEMBLED picture and the
shared sub-mix, not on each piece. At the boundary of an attachment there SHALL be exactly one
conversion of premultiplied RGB to straight alpha, integer, mirrored in WGSL;
the tolerance of the E-law on semi-transparent content is one rounding of the 8-bit step (1 LSB).
The GPU on a sub-tree SHALL converge with the CPU; the boundary exactly (`FrameTol::EXACT`).
Why: the composite accumulator stores premultiplied colour with straight alpha, and an assembled frame fed back without conversion is twice as dark exactly in the semi-transparent places.

#### Scenario: The level and the chain on the assembled picture
- **WHEN** a 50 % level or a non-linear effect is put on an attachment
- **THEN** the result ≠ the same applied to each child; without a level and a chain — identity
- **TEST** t438 (`src/libs/kt-engine/tests/nest_render.rs`), t440 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: A semi-transparent sub-composite is not twice as dark
- **WHEN** inside an attachment a semi-transparent layer lies over the background
- **THEN** the divergence from direct compositing is ≤ 1 LSB, and the fixture is not degenerate
- **TEST** t439 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: The GPU on a sub-tree
- **WHEN** an attachment with a chain and without is executed on the GPU
- **THEN** conformance with the CPU; the attachment boundary byte for byte; determinism on one device
- **TEST** t444 (`src/libs/kt-gpu/tests/gpu_render.rs`), t445 (`src/libs/kt-gpu/tests/gpu_render.rs`), t446 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: The speed of an attachment ≠ the speed of each piece
- **WHEN** the instance is slowed down
- **THEN** the result differs from slowing down each piece
- **TEST** t562 (`src/libs/kt-engine/tests/nest_render.rs`)

### Requirement: Three keys — reel, instance, revision; memo only for a full sub-composition
The address of a reel in the plan SHALL carry the high-bit tag (a file with such an address is rejected);
`fx_hash` SHALL distinguish instances; the revision SHALL be a recursive hash of the content,
woven into the key of the instance; with overrides the revision is a function of the PAIR (reel, set),
equal to the hash of the reel when the set is empty. Starvation SHALL be propagated upward, and the memoization of post-FX
SHALL be inherited downward; ONLY the full sub-composite is memoized. The memo key SHALL carry
the frame blending mode and the level of the reel; the key of the GPU plan SHALL be recursive over the plan of the
sub-tree. A regeneration of a generator inside a reel SHALL invalidate the memo of the ancestor reels in the
same thread that applies it.
Why: an edit inside a reel does not touch the fields of the instance; a starved frame under the reel's edit key
gave a black reel forever; without the sub-tree plan in the GPU key it returned a full frame over an incomplete sub-tree.

#### Scenario: Starvation and memoization cross the boundary
- **WHEN** a child's frame is absent; export with memoization
- **THEN** the outer composite is incomplete; full-size sub-composites are not memoized in the export
- **TEST** t441 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: An edit inside reaches the instance and the grandfather
- **WHEN** an event inside a reel or inside a grandchild is edited
- **THEN** the picture of the instance changes; the keys of the reels, instances and revisions differ
- **TEST** t442 (`src/libs/kt-engine/tests/nest_render.rs`), t443 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: The memo key carries the mode and the level
- **WHEN** the frame blending mode or the reel's level has changed with the same content
- **THEN** the memo does not return the old entry; with `blend == false` the key equals the hash of the revision
- **TEST** t582 (`src/libs/kt-engine/tests/nest_render.rs`), t631c (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: A reel does not return a stale result after a generator is reconfigured
- **WHEN** a generator inside a reel is reconfigured through the retune strip
- **THEN** the memo of the ancestor reels is invalidated by the worker before the application, there is no stale result
- **TEST** t663 (`src/libs/kt-engine/tests/gen_event.rs`)

### Requirement: The REEL's tier ≠ the instance chain input; a layer of another size is not silent
The decode scale of a reel SHALL be one per reel — the maximum over all instances in the tree; this is the ORDER (the strips are warmed, read and drawn by it), and the order goes inward already increased.
The entry of an instance's chain SHALL be `min(the reel's scale, what this instance's chain consumes)`; when they diverge — the sub-composite is fitted before the chain by one decision for both backends; when equal — there is no step and the bytes are the former ones. A layer of a foreign size SHALL give `complete = false` on both backends, and is never skipped silently.
Why: different scales would give the inner sheet two cache keys and two orders; an empty chain, the strength of a layer, and barriers without a resample do not consume the scale, and the S×-frame silently dropped out of the composite, while the export wrote the frame as good.

#### Scenario: The scale is a property of the reel
- **WHEN** a reel is placed twice, and one instance zooms
- **THEN** the scale is one, the order of both instances coincides and is warmed up increased
- **TEST** t450 (`src/libs/kt-engine/tests/nest_render.rs`), t679 (`src/libs/kt-engine/tests/nest_param.rs`)

#### Scenario: Each instance is rendered
- **WHEN** a twin has an empty chain, a barrier without a resample, or a leading strength of a layer
- **THEN** the frame is full, the attachment is in the frame; the input equals the scale → the bytes are the former ones; lower → the frame equals the independent oracle of fitting
- **TEST** t631 (`src/libs/kt-engine/tests/nest_render.rs`), t631b (`src/libs/kt-engine/tests/nest_render.rs`), t632 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: A layer of a foreign size
- **WHEN** a layer of the wrong size arrives in the composite
- **THEN** `complete == false` on the CPU and the GPU, the byte equality of the backends is preserved
- **TEST** t633 (`src/libs/kt-engine/tests/decode_scale.rs`), t182 (`src/libs/kt-gpu/tests/parity.rs`)

### Requirement: Nested warmth is the intersection; the chunk budget is computed by the tree
The coverage of an attachment SHALL be computed by subtracting the unwarmed part of each child from the span
of the reel (an intersection, not a union); the window is by the material actually consumed;
a stretched or mirrored child — "entirely or not at all". The export chunk budget SHALL
count an attachment as `1 + max_concurrent_layers(reel)`, and a blending pair — where it lives (inside the reel).
Why: a union would lie in the favourable direction (invariant 4); an undercount of the budget makes the export bytes depend on the timing of the cache.

#### Scenario: An intersection, not a union
- **WHEN** half of the layers of the reel have not arrived
- **THEN** the place is not shown as warm; the gap between the children is warm
- **TEST** t452 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: The chunk budget
- **WHEN** a project with an attachment, inside which frames are blended, is exported
- **THEN** the budget = 1 + the layers of the reel, the pair is counted inside (3 against 2)
- **TEST** t451 (`src/libs/kt-engine/tests/nest_render.rs`), t583 (`src/libs/kt-engine/tests/nest_render.rs`)

### Requirement: Audio — a nested item is a virtual media under the fetcher; the submix goes forward only
The sub-mix of a reel SHALL be handed out by the fetcher as an ordinary source: the mixer, the overlap weights and the order of f32 addition know nothing about reels. The sub-mix SHALL be rendered only forward in increasing reel time; the reversal and stretching of the instance act on the samples of the finished mix. The producer SHALL have exactly three branches (a burst in the ring; catching up forward no further than a block; a canonical move from a block boundary minus the pre-roll). The pre-roll and the block SHALL be functions of the project (the tail of the longest-tailed chain, recursively; for a reel without tails — zero).
The key of the FX state SHALL be the instance, not the reel. The sub-mix MUST NOT be put into the burst cache. The order and the reading SHALL go through one window function, extended by the reversal.
Why: a branch in the mixer would not give the E-law byte for byte; a fourth producer branch is a new kind of non-determinism; the size of the ring computed from the memory budget would make the output bytes a function of the budget; two instances shared one reverb and fought over the cursor.

#### Scenario: One chain per group of replicas
- **WHEN** a reverb sits on an attachment of ten replicas
- **THEN** the tail is drawn across the seams between the replicas (compared by energy), while the piecewise one is cut off
- **TEST** t449 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: The sub-mix in one piece == in pieces, a seek goes inward
- **WHEN** the sub-mix is rendered whole and in pieces, directly and under reversal
- **THEN** the bytes do not depend on the slicing of the output
- **TEST** t448 (`src/libs/kt-engine/tests/nest_render.rs`), t563 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: Two instances do not share reverb
- **WHEN** one reel sounds in two places at the same time
- **THEN** the internal chains and the burst cache are separate for each of them
- **TEST** t564 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: Raw reverse transport over a nested reel sounds
- **WHEN** transport plays backward over a nested reel (dry path)
- **THEN** both the order and the read reach the reel, and there is no silence
- **TEST** t565 (`src/libs/kt-engine/tests/nest_render.rs`)

### Requirement: Reverse and stretch of an instance — on the assembled result
An instance SHALL allow reverse and time stretch; reversing a nested reel SHALL equal the reverse of the expanded content byte for byte on the reel without tails (a definition, not an approximation). Expansion into effective clips MUST NOT approximate a mirrored or stretched instance — it stays an event. Reverse + stretch is "warp, then mirror"; the mirror is applied to what was actually consumed.
Why: "reverse of the assembled ≠ reverse of each piece" — the reverse analogue of law G; internal reverb must sound forward inside the reel, while a mirror from outside produces reversed reverb.

#### Scenario: Reversing a nested reel
- **WHEN** an instance of a reel without tails is mirrored
- **THEN** the result equals byte for byte the reverse of the expanded content
- **TEST** t561 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: Reverse and stretch together
- **WHEN** an instance is mirrored and slowed down by half
- **THEN** it plays backward in half steps, and each neighbouring pair of frames differs
- **TEST** t584 (`src/libs/kt-engine/tests/nest_render.rs`)

### Requirement: The reel's internal time is continuous; only the file sheet sets a floor on the grid
The internal time of an instance SHALL be the editing time of the reel without rounding to the frame grid; a procedural source (generator, title, placeholder) SHALL be evaluated on the continuous tick; only the file sheet applies a grid floor. A reel with its OWN fps SHALL quantize all of its internal time to its own grid (a frame holds until the next one, without blending) before the sub-plan is expanded; without its own fps the path stays continuous and the bytes are unchanged. The audio of a reel does not depend on its fps. Frame blending on a nested reel SHALL be inherited downward and performed by the LEAVES (one sub-composite per frame); the flag must travel into the order, the chunk budget, the coverage and the memo key. Source facts SHALL come from whoever opened the source.
Why: at rate 0.5 two neighbouring frames received one internal tick, so titles, automation and cameras inside stepped in pairs of frames; two sub-composites would cost twice as much and would bring the steps back; the reel's own fps (owner's decision, R.62) is exactly the intended stepping, so it is enabled only by an explicit field.

#### Scenario: A slowed reel animates in half steps
- **WHEN** an instance is slowed by half, with a title with automation and a file inside
- **THEN** the even frame equals the frame at rate 1, the odd frame equals the reel's frame at a half step; the footage is pinned to the grid, the order does not move
- **TEST** t579 (`src/libs/kt-engine/tests/nest_render.rs`), t580 (`src/libs/kt-engine/tests/nest_render.rs`), t585 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: Blending on a nested reel is done by the leaves
- **WHEN** frame blending is enabled on an instance
- **THEN** the result equals the blend of neighbouring frames of the SOURCE inside; the order carries both sides; "nearest" keeps the frame
- **TEST** t581 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: The same on GPU
- **WHEN** the slowed reel and blending run on the GPU
- **THEN** conformance is EXACT with the CPU, half step ≠ frame, blending ≠ nearest, determinism
- **TEST** t586 (`src/libs/kt-gpu/tests/gpu_render.rs`), t587 (`src/libs/kt-gpu/tests/gpu_render.rs`)

#### Scenario: The reel's own fps holds a frame
- **WHEN** a 12 fps reel with an animated title sits in a 24 fps project; the same reel without its own fps
- **THEN** with fps the frames go in pairs (odd equals even), and automation and cameras inside are quantized; without fps, t579/t580 keep their previous bytes; on GPU conformance is EXACT
- **TEST** t724 (`src/libs/kt-engine/tests/nest_render.rs`), t724b (`src/libs/kt-gpu/tests/gpu_render.rs`)

### Requirement: Edit context — swap of content; the document goes outward; the reel plays
Entering a reel SHALL be done by exchanging the content of the reel and the root, not by a parameter on commands; a command edits the reel you are standing in. There are three kinds of project: the working view (the whole interface), the document (file, export), and the current reel as root (the player inside the reel, offline count). Only the document SHALL go outward (file, export). What the player plays SHALL be decided in one place; inside a full entry the reel itself plays from its start. Every undo snapshot SHALL remember its context, and undo returns to where the edit was made. On entry and exit the cursor SHALL be translated (`t_вн = source_in + (t_нар − start)`) and clamped to the content of the reel; the view is aligned to the cursor; exit restores the whole view.
Why: "the current reel" across sixty command call sites — one missed call site silently sends the edit to the root; handing the working view outward means writing to disk a project with swapped containers; eight `set_project` call sites that took the document returned the player to the root on the first edit inside.

#### Scenario: Commands edit the reel you are standing in
- **WHEN** inside a reel a command is applied, then undo
- **THEN** the edit lands in the reel; the context survives batches and is handed back; Ctrl+Z carries the edits into the reel
- **TEST** t453 (`src/libs/kt-model/tests/nest.rs`), t454 (`src/libs/kt-model/tests/nest.rs`), t455 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: Outward only the document; playback comes from one place
- **WHEN** code hands the project to a file, an export or the player
- **THEN** the file/export receive the document; "what plays" is decided by a single function, direct senders are forbidden by a scan, and the decision point itself is exactly one
- **TEST** t456b (`src/apps/kt-app/src/reach_tests.rs`), t554 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Inside a reel, the reel plays
- **WHEN** a person has entered a reel and edits
- **THEN** the player shows the content of the reel (by content, not by calls), and the document is intact
- **TEST** t552 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Entry and exit are an identity, the view sits at the cursor
- **WHEN** entering an instance with a shifted start, and exiting
- **THEN** the cursor is translated (the scene is not degenerate: `want != outside`), the view is aligned to the cursor and clamped to the content; exit restores the zoom/scroll/region/selection
- **TEST** t456 (`src/apps/kt-app/src/layout_tests.rs`), t553 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A nested reel is its own entity with the verb "open"
- **WHEN** the table of verbs for timeline entities is checked
- **THEN** a nested reel has `Open`, four paths inside share one body, and Esc comes last in the cascade (container: full table — capability `ui-reach`)
- **TEST** t253 (`src/apps/kt-app/src/reach_tests.rs`), t457 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A nested reel looks like a nested reel
- **WHEN** an instance of a reel standing in N places is drawn
- **THEN** a mini-map of the content as a single mesh without media accesses; the ×N badge only when N > 1; the verb "to the next place"
- **TEST** t458 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: In-place expansion — a swap with an offset, byte-for-byte identity
Expanding a reel in the current view SHALL be an exchange with an offset: "`t_root = t_reel + dt`", `dt = inst.start − inst.source_in`, "`track = own + base[view]`"; the band occupies the TAIL of the numeric track space, and its visible place is set only by the row order. The model SHALL hold the canon (without bands — to the file, the player, export) and the working view (the canon plus bands — to the interface); with empty bands the view is a byte-for-byte copy of the canon. Merging and parsing SHALL be a byte-for-byte identity; parsing is the proof: a gesture that moves an event across the boundary of reels (an instance inside its own reel) rejects the whole batch. Inserting or deleting a track of any kind MUST NOT break the band; the tail track `base + len` belongs to the band. An edit in the band SHALL land in the reel; expanding changes only the view (neither the document, nor the player, nor the history); bands travel with the context in undo. The selection SHALL live in exactly one reel. One band at a time. The first edit inside an expansion of a shared reel SHALL warn about ×N once per reel per session.
Why: a layout in which "the row key knows its owner" would require auditing 182 calls of the working view, with the risk profile "missed it — silently wrong"; `dt` is invariant under trim, and the edge of an instance is dragged directly in the band over the stationary material.

#### Scenario: Byte-for-byte identity and the boundary
- **WHEN** a reel is expanded and assembled; an event is dragged across the boundary
- **THEN** the project is byte-for-byte as before (the canonical row order is an empty vector); an instance inside its own reel → the batch is rejected; a band event on a root track stays in the root
- **TEST** t566 (`src/libs/kt-model/tests/nest.rs`), t567 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: Tracks do not break the band, the edit reaches the reel
- **WHEN** with the band expanded a track is inserted/deleted; an event of the band is edited
- **THEN** the content of the reel after the edit is the same; the edit landed in the reel, the document has no reel event at the root, and Ctrl+Z does not collapse the expansion
- **TEST** t568 (`src/libs/kt-model/tests/nest.rs`), t569 (`src/libs/kt-model/tests/nest.rs`), t592 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: Expansion — view only
- **WHEN** an instance is expanded by double-click and collapsed
- **THEN** the document, what plays, and the history are unchanged; an edit from the band reaches the reel (the model is queried from the gesture's point)
- **TEST** t570 (`src/apps/kt-app/src/layout_tests.rs`), t571 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Selection in one reel and the Esc cascade
- **WHEN** a click in another band while something is selected; Esc while there is a selection, a band, and an entry
- **THEN** the selection is replaced and this is named; Esc: clear the selection → collapse the band → exit the reel
- **TEST** t572 (`src/apps/kt-app/src/layout_tests.rs`), t573 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A band inside a full entry
- **WHEN** inside an open reel X a nested reel is expanded
- **THEN** the bases are counted from X, the expand/collapse identity holds, the snapshot carries the scope and the bands; the cascade is the same as at the root
- **TEST** t634 (`src/apps/kt-app/src/layout_tests.rs`), t635 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: Bands survive a context change
- **WHEN** a person enters a reel with a second Enter and exits with Esc; entering another reel; the instance disappears
- **THEN** the band of the context being left returns on exit, and a band inside a reel returns on the next entry; the band is recalculated; a vanished instance goes silently without a band; undo across the boundary — the snapshot wins
- **TEST** t636 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Track number within a view = row rank
The tracks of the root and of each reel SHALL be objects with a stable id, and the row order is the order of their vector. Within each kind, rows SHALL run in ascending order of event number; row order encodes only the interleaving of kinds. The single reorder door SHALL renumber everything addressed by track number (clips, morphers, cameras, foreign raw material, regions), in the root and in every reel; name, height, switches, level, chain and mode travel with the object without renumbering; validation rejects non-canonical data, and reading canonicalizes it. A reel's sub-mix and sub-plan SHALL apply the reel's track stages; law E holds for tracks without stages. There is exactly one exception — the expansion bench (the band in the tail); commands that apply the invariant SHALL be rejected while a band is open and SHALL NOT collapse the band themselves.
Why: the engine stacked layers by number while the screen drew by row order — the top track was the bottom layer; three parallel property vectors and six functions that synchronize them were a class of bug "name present, flag missing" after an insertion.

#### Scenario: Reorder renumbers, composition == screen
- **WHEN** rows are reordered
- **THEN** the content is renumbered, the composition order matches the row order, the region, foreign track, name and height travel with it, and undo restores it
- **TEST** t588 (`src/libs/kt-model/tests/lane_order.rs`), t241 (`src/libs/kt-model/tests/lane_order.rs`)

#### Scenario: Guard and canonicalization on read
- **WHEN** the document or a reel is non-canonical
- **THEN** validation rejects it; reading canonicalizes the root and the reels; a file in the owner's project form is read canonically, and a track's foreign raw material travels with the track
- **TEST** t589 (`src/libs/kt-model/tests/lane_order.rs`), t590 (`src/libs/kt-model/tests/lane_order.rs`), t593 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Bench is open
- **WHEN** while a band is open, a row reorder or a track count change is invoked
- **THEN** refusal `BandsOpen`; the band is not collapsed by the refusal
- **TEST** t591 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: Track at the top — top layer
- **WHEN** a video track is added from the morph-row menu and an event is placed on it
- **THEN** the event is drawn as the top layer
- **TEST** t594 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Track id is stable, properties travel with the object
- **WHEN** a track is inserted at the top, reordered, and a neighbour is deleted; its name and height are changed
- **THEN** the id is unchanged, name/height/switches stay in place, changes of name and height are rolled back by undo, and t426 (destructuring of the root and the reel) compiles with the new field
- **TEST** t729 (`src/libs/kt-model/tests/track.rs`)

#### Scenario: Track stages inside a reel
- **WHEN** a track inside a reel has a chain and a level; the same reel without track properties
- **THEN** the instance sounds and looks with the track stage; without the properties — law E byte for byte (t434/t435/t447)
- **TEST** t739 (`src/libs/kt-engine/tests/nest_render.rs`)

### Requirement: Paste knows where it was called from; the clipboard is in the owner reel's coordinates
The clipboard SHALL be stored in the coordinates of the owning reel (translated from the working view with a band; foreign items are dropped with a count and named). A block SHALL be aligned to the clicked row (all kinds shifted by one delta, clamped per kind, truncation named); the needed tracks are created explicitly at the start of the same batch. Paste from the right-hand menu SHALL take the time AND the row of the click.
Why: a clipboard taken from a band carried numbers with different bases for video, audio and morph, and the pair drifted apart at the ends of the edit; silent growth of tracks put the event into the wrong reel.

#### Scenario: Row arithmetic and coordinate translation
- **WHEN** a block is aligned to a row; a block from a band is translated into the reel and back
- **THEN** alignment with the clicked row, clamping per kind, and a block without that kind does not move; translation is an identity, and foreign items are dropped with a count
- **TEST** t595 (`src/libs/kt-model/tests/paste_rows.rs`), t595b (`src/libs/kt-model/tests/paste_rows.rs`)

#### Scenario: Paste in one step, the pair on its own tracks
- **WHEN** the paste requires new tracks; a copy of a link pair from the band is pasted via the right mouse button into the root row
- **THEN** one history step; the pair sits in one link group on the clicked row at the click point and did not reach into the reel
- **TEST** t595c (`src/libs/kt-model/tests/paste_rows.rs`), t596 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Enter cascade goes deeper, Esc goes outward; a quick gesture gives the default mode
Double-click and Enter on a collapsed reel SHALL expand it in place (the default mode); the menu item named "Enter the reel" is the full entry. Enter on an expanded reel SHALL lead to the full entry; a repeated double-click collapses it; inside an entry a quick gesture leads deeper. The decision SHALL be made by one function based on the presence of a band; the context plays no part.
The selection of an instance SHALL survive expanding and collapsing its own band. A shortcut is advertised only where the key actually does it.
Why: the Enter toggle plus clearing the selection after expansion meant a second Enter hit empty space; the label promised a key that did not invoke the item.

#### Scenario: One decision for the whole program
- **WHEN** Enter on a collapsed reel, on an expanded one, inside an entry; "collapse all"
- **THEN** expand → enter → deeper; its own collapse keeps the selection, "collapse all" clears it
- **TEST** t597 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: A reel's zero is not a wall for gestures, but a wall for the cursor and playback
Inside a full entry the screen axis SHALL be the axis of the reel; an event may go to the left of zero (the gesture floor is "minus infinity" in the reel and 0 at the root), while the cursor, region, markers and regions may not. Bare `.max(0)` over a time position on the timeline MUST NOT exist. The zone before zero is shown under a veil; the ruler and the timecode to the left of zero carry a sign. A negative start SHALL survive the file round trip (`kt.start`, version by content). The root does not change.
Why: in an expanded band internal time could go negative freely, but in a full entry it could not: the wall stood only in the UI, and the format dropped a negative start (an OTIO track is sequential).

#### Scenario: Event past zero, cursor not
- **WHEN** inside a reel an event is dragged to the left of zero; the cursor is set to the same place
- **THEN** the event moves; the view shows the zone before zero under a veil only inside the reel; the cursor goes to zero; at the root the old −40 px remain
- **TEST** t637 (`src/apps/kt-app/src/layout_tests.rs`), t637b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Floor by name, sign of the timecode, file round trip
- **WHEN** the timeline is scanned for `.max(0)`; a negative tick is formatted; a negative start is saved
- **THEN** there are no bare clamps (zero stays for rows, lengths and `source_in`); the timecode carries a sign; the start survives the round trip
- **TEST** t637c (`src/apps/kt-app/src/reach_tests.rs`), t98 (`src/apps/kt-app/src/layout_tests.rs`), t638 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Reel preview — offline rendering by the same composition code, with a disk tier
Frames and the envelope of a reel on the timeline SHALL be rendered offline by the same composition code, keyed by (reel, edition of the INSTANCE); a request is placed before the draw cycle, by key, once (zoom and scroll do not generate orders); rendering MUST be paused during playback and export; frames go out before peaks, and until then the envelope is empty; the moment of a frame uses the same map "event tick → material" as the file. The disk tier SHALL live in the sidecar by key (reel, edition, fingerprint of the media files of the tree) in the task thread; a DB error is a cache error, not a preview. FRAMES SHALL be written to the sidecar right after the frame stage in their own transaction, and peaks afterwards in their own; a bar that reached the screen survives on disk even if the audio stage is cancelled. A cancelled task (pause, edit, close) MUST NOT enter the negative cache as a failure: the request is placed again at the first pause; only a render error counts as a failure. A second opening of a project with an unchanged reel and unchanged media files SHALL give frames and peaks from disk without a single render — both on a project with TWO instances of the same reel stacked over each other with different override sets, and on a reel with a missing source (the placeholder is also a frame, its fingerprint is "missing", and the key stays stable). Offline rendering in kt-app SHALL live in one place.
Why: a thumbnail per file is impossible (a reel has no file), but its content is rendered; `content_hash` sees the editing, not a file substitution under it; two releases with different titles would show the same frames. R.67: "the composition previews AGAIN are not saved, EVERY TIME it recalculates the previews" — writing happened only after the audio stage, and a cancel turned the key into a "failure" until the next edit; the owner's sidecar held a strip of forty identical frames that nothing asked for.

#### Scenario: Service rules
- **WHEN** the service receives requests, the edition changes, the reel does not fit into the LRU
- **THEN** the step level, the frame cap, deduplication, invalidation by edition; frames and peaks reach the screen
- **TEST** t599 (`src/apps/kt-app/src/layout_tests.rs`), t600 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Scrolling orders nothing
- **WHEN** the timeline is scrolled and zoomed over a reel
- **THEN** there are no requests; a thumbnail of the reel is not ordered; the renderer in kt-app is started only by the render dialog and by this service
- **TEST** t601 (`src/apps/kt-app/src/layout_tests.rs`), t602 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Disk tier
- **WHEN** the project is opened a second time; the mtime of the source is shifted
- **THEN** zero renders if frames and peaks are present; a miss and a re-render by fingerprint; a round trip across three tables, a foreign key is a miss, then eviction and cleanup
- **TEST** t639 (`src/libs/kt-media/tests/index_db.rs`), t640 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Preview key is per instance
- **WHEN** two instances of one reel with different overrides
- **THEN** the keys differ; the set of live editions does not evict each other's work; without overrides the key equals the reel's edition
- **TEST** t680 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Frames survive to disk without peaks, cancellation is not a failure
- **WHEN** the preview task is cancelled right after the frame stage (playback started); then pause; then the project is closed and opened again; separately — the project in the owner's file form (two reels stacked over each other with overrides, a missing still) is opened twice
- **THEN** after the cancellation the frames are already in the sidecar (the band row exists, no peaks); at pause the request is placed again and the peaks are appended; the second opening — zero renders, frames and peaks from disk; on the owner's project the second opening — zero renders (on the previous code, a render at every opening); the key is the same for both openings
- **TEST** t795 (`src/apps/kt-app/src/layout_tests.rs`), t795b (`src/libs/kt-media/tests/index_db.rs`)

### Requirement: A reel is a bin object; dropping a project asks; a foreign project — as a composition
A reel SHALL be a bin card (mini-map, ×N, verbs: place, expand, enter, rename, remove by address, create empty), distinguished by its set of addresses, not by a path prefix. Dropping a `.otio` SHALL ask "open as project / paste as composition"; "open" goes through the guard of unsaved work and the project lock. Pasting a foreign project SHALL renumber all addresses, deduplicate media by path, and leave regions, markers and settings in the foreign project with a message; one history step.
Why: a drop performed an application replacement that bypassed everything protecting unsaved work; the foreign ids that arrived would collide with the document's addresses.

#### Scenario: A foreign project as a composition
- **WHEN** a foreign `.otio` is pasted as a composition
- **THEN** its nested reels arrived, the media was recognized and not doubled, regions and markers stayed in the foreign project and this is named, and the addresses did not collide
- **TEST** t574 (`src/libs/kt-model/tests/nest.rs`), t575 (`src/libs/kt-model/tests/nest.rs`)

#### Scenario: A drop asks and does not lose work
- **WHEN** a `.otio` is dropped on the timeline with unsaved work
- **THEN** a question; "Cancel" does nothing; "Open" passes through the guard; "paste" puts the reel and the instance in one step
- **TEST** t576 (`src/apps/kt-app/src/layout_tests.rs`), t577 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A reel in the bin
- **WHEN** a reel card is in the bin
- **THEN** it is reachable by all the verbs, and an occupied one refuses to leave
- **TEST** t578 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Public parameters — the template value is not stored, overrides come before morpher layers
A reel SHALL declare handles (id, author name, target address, type, range narrowing); an instance SHALL override ONLY what is published, by value or by curve. A template value MUST NOT be stored — it is read live from the target. Resolving a reel for an instance SHALL go through a single door; overrides are applied BEFORE the morpher layers; an empty set is the old path without clones. A curve lives in the material of the instance (reel time); the carriers of a curve are a slot, a generator, a nested reel, and also the LEADING GEOMETRY of an event (`ParamOwner::Geometry` — the fields of `EventGeometry`: rule, crop, position, zoom, rotation, anchor) — by the same passport, the same `set_value`, the same curves (`geometry_auto`). Publishing commands (`PublishParam`, removal, editing of the declaration) SHALL carry the reel address EXPLICITLY (`seq`) and MUST NOT depend on which reel the person is standing in: publishing from an expanded band addresses the band's reel without entering it. An override of an effect slot by a number SHALL act on the instance's frame the same way as an override of a generator: two instances with different values of one handle on an effect slot give different frames and different memo keys (verified on `RadialBlur.amount`, not only on `zoom`). The address is glued to the target's id: it survives trim, moving, and the left half of a cut; if the target disappeared or changed type, the publication and all overrides go in one step, per batch, without a list of triggers; validation rejects orphans and foreign types. The expansion band shows the TEMPLATE; expansion bakes it in and names the count.
The range narrowing of a numeric handle SHALL by default equal the passport of the TARGET (`min`/`max` of its `ParamSpec`, obtained by address): the author's window shows the target's limits, not zero; only the narrowing is written to the file — a value equal to the target's limit is stored as absence (`None`); resetting to the target's limits is one action. The value at the consumer SHALL be clamped to the target's limits and, on top of those, to the author's narrowing.
Why: a stored copy would diverge from the reel at the first edit inside; a morpher appends slots at the end — after it the slot address would point to the wrong place; "min/max always at 0" — an empty `Option` was drawn as zero, while the target's passport already knows its limits (R.60). R.63: the leading geometry is the only carrier of properties without a publication address (the owner: "or do we have duplicated code?" — not duplicates, a hole in addressing); "why do we automatically enter the composition fully" — the command of the rule "the reel you are standing in"; "Radial blur… I changed it in the composition — and it doesn't change" — the path to an effect slot had not been proven by a test on its own parameter.

#### Scenario: Commands and two questions
- **WHEN** a handle is published, overridden, or removed
- **THEN** six commands with undo; "what is in the template" is read at the target, "what plays here" is read with the override
- **TEST** t671 (`src/libs/kt-model/tests/pubparam.rs`)

#### Scenario: The target's fate
- **WHEN** the target is trimmed, cut, moved, deleted, or changes type
- **THEN** the address survives a trim and the left half of a cut; the target's death takes the publication and its overrides in one step; orphans and foreign types are rejected by validation
- **TEST** t672 (`src/libs/kt-model/tests/pubparam.rs`), t672b (`src/libs/kt-model/tests/pubparam.rs`)

#### Scenario: Empty set — the old path
- **WHEN** an instance without overrides is resolved
- **THEN** transitions and cameras are borrowed (`Cow::Borrowed`), and not a single clone of the reel is made
- **TEST** t673 (`src/libs/kt-model/tests/pubparam.rs`)

#### Scenario: The band shows the template, expansion bakes it in
- **WHEN** an instance with overrides is expanded by its band; then expanded
- **THEN** the band shows the template values; after expansion the values are in the pieces, and the number of baked-in values is named
- **TEST** t690 (`src/libs/kt-model/tests/pubparam.rs`)

#### Scenario: Handle limits by default — the target's passport
- **WHEN** a numeric handle has just been published; the author narrows `max`; the author returns `max` to the target's limit; the author presses "reset to target limits"
- **THEN** the author's window shows the target's `min`/`max` (not 0); `PubRange` holds only the narrowing (`min: None`); a value equal to the target's limit is `None` again; after the reset `PubRange::is_empty()`, and the file without narrowings is byte for byte unchanged
- **TEST** t692 (`src/libs/kt-model/tests/pubparam.rs`), t692b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The consumer's field is clamped to the target's limits
- **WHEN** the consumer drags a handle's value beyond the target's passport without the author's narrowing; with narrowing, beyond the narrowing
- **THEN** the value does not go beyond the target's passport; with narrowing it does not go beyond the narrowing; the model and the field show the same number
- **TEST** t692b (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Leading geometry — a carrier of a handle
- **WHEN** inside a reel the zoom of an event's leading geometry is published; the instance overrides it with a number and a curve; the event is trimmed and cut; the event is deleted
- **THEN** the handle with address `Geometry` is present in the file and in the model (a predicate on the content); the instance's frame differs from the template exactly by the zoom; the curve lives in `geometry_auto` on the instance's material; the address survives a trim and the left half of a cut; the death of the event takes the handle in one step; validation rejects a geometry address on an audio event
- **TEST** t752 (`src/libs/kt-model/tests/pubparam.rs`), t752a (`src/libs/kt-engine/tests/nest_param.rs`)

#### Scenario: Publishing addresses the reel explicitly, without entering it
- **WHEN** the publish command is issued with the `seq` of a reel from the root (the reel expanded by a band), from another reel, and from inside the reel itself; `seq` does not exist
- **THEN** the handle lands in the named reel regardless of context, and `editing()` does not change; a nonexistent reel makes the command refuse with the address; a transition and a camera addressed from the band use the same `seq` (the "Deferred" item of R.59 is resolved)
- **TEST** t753 (`src/libs/kt-model/tests/pubparam.rs`)

#### Scenario: Overriding an effect slot changes the frame
- **WHEN** in a reel an event carries `RadialBlur`, and the handle `amount` is published; two instances override it with 0.0 and 0.9; only the second one is edited
- **THEN** the instances' frames differ (the first equals the frame without blur), the memo keys and `instance_content` differ, and editing the second does not touch the first; the backend does not affect this — the handle values are resolved when the PLAN IS BUILT, and the executor receives ready numbers (invariant 7)
- **TEST** t755 (`src/libs/kt-engine/tests/nest_param.rs`)

### Requirement: Reel edition — a pair (reel, set); the generator address salt in one code
Trees of sub-renderers SHALL be keyed by the pair (reel, hash of the set); two instances without overrides SHALL share one sub-renderer and the old bytes. The override curve SHALL land on the same frame of the material under trim and reverse. The source address of a generator SHALL be salted with the set hash, which is zero exactly when no handle reaches the source; one code path computes the salt for the plan and for the source list. The reel's stage SHALL be computed by walking the tree of instances with their sets. A template within a template is resolved recursively.
Why: a shared generator address across different titles formally kept "order == read", but the frame that came out was someone else's; a shift of keys in projects without publications would have redrawn all memos.

#### Scenario: A curve and two titles
- **WHEN** an instance has a curve on a handle, the instance is trimmed and mirrored; two instances with different titles
- **THEN** the curve is on the same frame of the material; the two frames differ
- **TEST** t674 (`src/libs/kt-engine/tests/nest_param.rs`), t675 (`src/libs/kt-engine/tests/nest_param.rs`)

#### Scenario: Zero cost without overrides
- **WHEN** a project has no overrides
- **THEN** one sub-renderer per reel, the old bytes
- **TEST** t676 (`src/libs/kt-engine/tests/nest_param.rs`)

#### Scenario: Logo swap, sound, template within a template
- **WHEN** a media parameter and an audio parameter are overridden; a reel with a publication stands inside a reel with a publication
- **THEN** the logo is swapped and order == read; the sound changes; the resolution is recursive
- **TEST** t677 (`src/libs/kt-engine/tests/nest_param.rs`), t678 (`src/libs/kt-engine/tests/nest_param.rs`), t686 (`src/libs/kt-engine/tests/nest_param.rs`)

#### Scenario: GPU and format
- **WHEN** two differently overridden instances run on GPU; a file with publications
- **THEN** conformance with CPU; the format round trip keeps the version by content, and a file without publications has not changed
- **TEST** t687 (`src/libs/kt-gpu/tests/gpu_render.rs`), t681 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Publishing interface — one door, the author's name verbatim, two badges
Publication SHALL be reachable from one door — a right-click on a field's label inside a composition (full entry or band). Outside a reel there is no menu item, and a hint explains this — only when compositions exist in the document. The owner reel SHALL be determined from the working view (an event of a band belongs to the band's reel), and the publication confirmation MUST NOT enter the reel: the command carries its address, and the person stays where they were. The consumer SHALL edit handles in the instance's FX window — in the pinned head of the chain band (each type with its own widget; reset sends the removal of the override; a window session is one history step); there is no separate instance-parameters window. A handle's name SHALL be drawn verbatim, bypassing the translation catalog. Badges: a pin at the tail of an event's label, a number at the head of a reel card; zero is not written.
Curves of overrides SHALL be VISIBLE in the same head, in the same automation widget as the effect slot (graph ↔ key table, selection of the active curve, keys at the cursor, one axis lock per program); a curve is created by the labelled action "Automation: add curve…" (menu over number and colour handles), not by an anonymous glyph. There MUST be no hidden key recording: editing a field places a key at the cursor only for a handle whose curve has been created and drawn; for a handle without a curve, editing a field is a constant. Automation controls SHALL have a hint in the status line (rule 10 — no pop-ups).
Why: no editor has "show everything outward" — the interface is assembled by hand; a publication into the "wrong" reel would have died in reconciliation with the same batch; a test on the accessibility tree was green on code that drew the catalogue line; the "+" glyph turned the window into a hidden automation recording mode with not a single curve on screen ("how did such a bug even get through", R.60) — a breach of "one animation mechanism for everything" (R.34). R.63: entering the reel when publishing from a band is an imposed change of context; a separate instance-parameters window is "nuts" — the owner's ban on windows without an obvious purpose.

#### Scenario: One door
- **WHEN** right-click on a label from the full entry, from the band, outside a reel; the instance menu
- **THEN** present, present, not at all (the hint explains when reels exist); the instance menu leads to the FX window with the parameter head
- **TEST** t682 (`src/apps/kt-app/src/reach_tests.rs`), t518 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Consumer window, author name, badges
- **WHEN** the FX window of an instance is open with the parameter head; the program language was switched; the instance is overridden
- **THEN** each type has its own widget, and reset sends `ClearOverride`; the name is drawn verbatim and does not change with the language; a pin on the event, a number on the card, nothing at zero
- **TEST** t683 (`src/apps/kt-app/src/layout_tests.rs`), t684 (`src/apps/kt-app/src/layout_tests.rs`), t685 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The override curve is visible and created by a named action
- **WHEN** in the parameter head a number handle has no curve and its field is edited at two different cursor positions; then a curve is created via "Automation: add curve…"; the field is edited at two cursor positions
- **THEN** without a curve the override holds one value and zero keys; after creation the window has the automation widget with this curve (graph and table probes found), the keys landed in the instance's material at the cursor and are visible in the table; there is no "+" glyph in the window; the automation buttons have a status-line hint
- **TEST** t691 (`src/apps/kt-app/src/layout_tests.rs`), t693 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Live publication scenario
- **WHEN** in the real kt-app a handle is published and overridden on an instance, then a curve is created on the handle and a key is moved
- **THEN** the consequence is visible in the frame and in the file; the curve is visible in the window, and the key sits in the file on the instance's material
- **TEST** t689 (`src/scripts/live/nestparam.py`)

#### Scenario: Publication from the band stays in the band; the effect handle acts on the instance
- **WHEN** an instance is expanded by a band in the root; right-click on the label `amount` of a radial blur of a band event, the name is confirmed; then a second instance is placed from the bin and its handle is edited in the head of the chain band
- **THEN** after the confirmation the person is still in the root with the band open (`editing()` has not changed; on the previous code it was a full entry); the handle is in the reel; editing the second instance changes its frame in the preview (a pixel), the first one does not change; in the file the override is on the second
- **TEST** t753b (`src/apps/kt-app/src/layout_tests.rs`), t755b (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/nestparam.py` (steps added)

### Requirement: Diagnostics — probe prints the reel tree and the event path
`kt-cli probe` SHALL print the tree of reels (name, address, number of events, number of places, end) and the path of each nested-reel event; for an instance — the actual content window (consumed material, warp and reverse marks).
Why: "enter N levels to understand where the frame comes from" — a drawback of the competition that we do not repeat; for a stretched instance the length on the timeline and the content window do not coincide.

#### Scenario: Reel tree in probe
- **WHEN** probe runs on a project with a nested reel
- **THEN** it prints the tree, the event path, and the literal `is a NESTED REEL`
- **TEST** t452b (`src/apps/kt-cli/tests/cli.rs`)

### Requirement: Instance override curves — the same shapes, sources and block as the slot
An override of a public parameter SHALL accept a curve with any segment shape (including a segment formula), a formula layer and a source (a formula, a link inside the reel), and the instance window SHALL show for it THE SAME "curve as parameter" block as the effect slot and the generator: the shape of all segments with parameters, the layer, the link, delete. The axis of the override curve is the instance's time; source resolution happens in this domain when overlaid on the content of the reel, before conversion to the target's time. A refusal (an address across the reel boundary) SHALL be named in the status line, not wipe the field.
Why: R.67, the owner: "in the curve automation of composition parameters you can't set the curve shapes for the whole segment (as in fx) — we end up with some kind of code duplication there instead of a generalization! And if you type something like t*2 there — after Enter it just wipes the field and nothing happens at all" — validation rejected any source for an override (`CurveSourceUnsupported`), and the instance had no block of shapes.

#### Scenario: The instance accepts a formula and shapes, one block
- **WHEN** in the head of a reel instance, a curve override has `t*2` typed and Enter pressed; a zigzag is set on all segments; the layer `k*0.5` is set; then everything is removed
- **THEN** after Enter the field holds `t*2`, the curve is a formula, and the instance's frame changes according to it (it is not wiped, not silent); the zigzag and the layer applied to the override and are visible on the graph; the block is the same code with the same labels as the slot's; removal returns the keys
- **PATH** FX window of the instance → head "Reel" → curve "Amount" → field "Formula": `t*2`, Enter → the dashed line grows, the frame blurs more toward the end → "Shape of all segments": "Zigzag" → it wobbles
- **TEST** t791 (`src/apps/kt-app/src/layout_tests.rs`), t792 (`src/libs/kt-model/tests/pubparam.rs`), live scenario `src/scripts/live/curves.py`

