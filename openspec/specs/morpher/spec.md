# Morpher

## Purpose

The morpher is a layer of edits over timeline events: source replacement and effect chains, applied to targets by block time with strength and fades, without a second render path — the engine sees only events that are already "flattened".

## Requirements

### Requirement: The engine sees only effective events
Morphers SHALL be applied at the single output point of the model's events (effective events); the engine, export and rendering MUST NOT know about morphers. A render with a morpher SHALL be sample-exact equal to a render after bake and to a render of the equivalent project without a morpher; the FX batch of a layer SHALL act without bake. The project's source events are not touched in this process (the output is a copy). While the project has no morphers and no reels, the output SHALL be a clone of the events (the golden references rely on this).
Why: a second path "morpher in the engine" would give a divergence between preview and export and a dead batch (a defect of the old engine: the batch did not sound without bake).

#### Scenario: Source replacement and batch sound without bake
- **WHEN** a project with a source-replacement morpher and a batch morpher is rendered, then bake is done and the render is repeated
- **THEN** both renders match byte for byte the render of a project where the clip points to the new source / carries the effect in its chain from the very start
- **TEST** t36 (`src/libs/kt-engine/tests/morpher_render.rs`)

#### Scenario: Flatten in the model: copy, typing, order, synchronization of targets
- **WHEN** a morpher is applied to a frozen list of targets
- **THEN** the source is replaced only for targets, the original clips are untouched; video effects are appended only to video clips, audio — only to audio clips; two patches on one clip are applied in ascending id order (the later one wins, cumulatively); deleting a clip removes it from the targets, a cut adds the right half; inserting/deleting a track shifts/removes events; save→load is byte-stable
- **TEST** t35 (`src/libs/kt-model/tests/morpher.rs`)

### Requirement: A patch carries both a source swap and the effect chains
One morpher event SHALL carry simultaneously a source replacement (with an absolute entry point or a delta; the absolute one takes priority) and effect chains of both kinds. The switch of the morph slot SHALL mute only the replacement. Deleting media referenced by a patch MUST be rejected by the list of the patch's references, not by the kind of morpher. The chain for video SHALL include the time warp from the patch's audio chain: the warp is a rule for choosing the frame, not a pixel filter, otherwise the layer would slow down the sound and not slow down the picture.
Why: two mutually exclusive kinds of morpher forced the user to create two events for one edit.

#### Scenario: A patch with a replacement and chains
- **WHEN** a patch contains media as well as video and audio effects, and the morph slot is switched off
- **THEN** the source is not replaced, the effects are applied; deleting the patch's media is rejected
- **TEST** t243 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: A warp in a patch slows down the picture too
- **WHEN** a patch carries a slowdown in the audio chain and lands on a video clip
- **THEN** the video clip receives the warp in its own chain and is not cut in any target mode
- **TEST** t247 (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: One chain lane with a pinned head-replacement
- **WHEN** the morpher window is open with effects of both kinds
- **THEN** the chain lane is one; the first slot is the source replacement (it is not reordered, has no cross); effects of both kinds lie in one lane and are sorted back by effect kind; the source field accepts a file and can return the original source
- **TEST** t244, t218 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Target mode — a layer of "everything below" by rows
The default target mode SHALL be "everything below the morpher's row"; "below" is counted by the ORDER OF ROWS, and the order of rows SHALL be model state — the order of the vector of object tracks (it goes through undo, raises the content's dirtiness); morph and camera tracks are the same objects without a level and a chain. The layering order SHALL be bottom to top (the lower layer is applied first); within one row — by id. The morpher remains per-clip and is applied BEFORE the track stage (the chain, the level, the track's blend mode already see the changed clips). The frozen list of targets MUST be non-empty; an empty set of targets for a layer is legitimate.
Why: the user asked "not to struggle with the selection"; reordering rows changes the picture — so it is editing, not UI convenience; the order as the vector of tracks removes a second source of truth (`lane_order` next to the counters).

#### Scenario: The layer covers what is beneath it, bottom to top
- **WHEN** the morph row stands at the bottom of the list, then is raised to the top; two layers on different rows
- **THEN** from the bottom the layer covers nothing, from the top — everything beneath it; a video layer does not touch audio; the lower layer is applied first; reordering rows reverses the order of effects and is rolled back by undo
- **TEST** t235 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: The order of rows is model state (container, capability `ui-timeline`)
- **WHEN** rows are reordered by a command
- **THEN** the order survives undo/redo; inserting/deleting a track shifts it within the command; garbage is normalized; in the file the order is the order of tracks, there is no separate field
- **TEST** t241, t49 (`src/libs/kt-model/tests/lane_order.rs`), t241 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The morpher window shows the mode and the time
- **WHEN** the morpher window is open in layer mode and in frozen-list mode
- **THEN** in layer mode an explanation of what is covered is shown; the frozen list — by name; the "by time" checkbox edits the model in one history step
- **TEST** t242 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The morpher before the track stage
- **WHEN** a morph layer paints the clips of a track that has a track chain with a vignette
- **THEN** the vignette lands on the already-painted composite of the track; the order "morpher → track" does not depend on the position of the morph row relative to this track higher in the list
- **TEST** t736 (`src/libs/kt-engine/tests/track_video.rs`)

### Requirement: Reference modes — named areas and range markers
The morpher SHALL be able to address targets by a reference BY NAME to a named area (time × set of tracks by semantic keys, empty = all) or a range marker. Names SHALL be non-empty and unique within ONE space shared with markers; uniqueness is produced by one function for commands and for paste. Renaming SHALL update the references in one history step; deleting an area SHALL leave the reference dead (it resolves to nothing, is drawn as an error), rather than discard it. In reference modes the time is set by the reference itself, the "by time" checkbox goes out; overlapping references merge into one window.
Why: the user asked for morphers "by reference to a region/area" and the right to change his mind; "area finale" and "marker finale" in one picker is a trap.

#### Scenario: An area by name, renaming and a dead reference
- **WHEN** a morpher refers to an area; the area is renamed, then deleted
- **THEN** the window is taken from the area, not from the block; the area names the tracks; the name is unique in the common space; the reference moved to the new name and is rolled back by one undo; after deletion the reference remains and leads nowhere, the project is valid
- **TEST** t247 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: A frame becomes an area; format
- **WHEN** the user saves a selection frame as an area
- **THEN** the area gets the same tracks and time, the name is made unique silently, the dead reference remains in view; areas survive a file round trip, a project without areas is byte-stable
- **TEST** t248 (`src/apps/kt-app/src/layout_tests.rs`, `src/libs/kt-otio/tests/convert.rs`)

### Requirement: "By time" action and reachability
The morpher SHALL act within the window of its block (default `timed = true`); files of the old format are read as "outside time" and render as before. The first question to a target is WHETHER the layer REACHES it by time; only then — cut or patch whole. An empty set of references and a window shorter than the minimal length of an event SHALL mean "did not reach".
Why: the veto "do not cut a warped clip", which stood ahead of the question of reach, handed the warp and the source replacement to the whole two-hour project of the user.

#### Scenario: Format schema and old files
- **WHEN** a file predating the layer schema is read
- **THEN** the morpher receives `timed = false`, the layer raises the version, defaults are not serialized, the round trip is idempotent; a block "outside time" patches the body whole regardless of position
- **TEST** t240 (`src/libs/kt-otio/tests/convert.rs`), t236 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: The layer does not reach a warped clip outside the window
- **WHEN** a block with a warp stands 10 seconds away from a stretched clip; a second block covers it
- **THEN** the first does not touch the clip at all; the second patches it whole and does not cut it
- **TEST** t288 (`src/libs/kt-model/tests/morph_slice.rs`)

### Requirement: Cut or mute — the patch decides
A patch consisting only of effects MUST NOT cut the target: it lands on the whole body, and "by time" is expressed by a strength curve with zero outside the window (a gate); the boundary is exact to the tick; transitions and neighbours are untouched; the id and FX state of the original are intact. Only patches whose contribution cannot be switched off by a strength slot are cut: source replacement and time warp in the patch. Gates covering the body entirely create no slot (the path on defaults is byte-for-byte identical).
Why: cutting for the sake of a chain on an event with fades turned "by time" into "the whole event" — the owner's complaint, the decision "do not cut, gate instead".

#### Scenario: The "by time" window is exact on an event with fades
- **WHEN** a layer made only of effects covers the middle of an event with fades on both sides
- **THEN** no cuts appear, the transition windows have not moved, the gates stand exactly on the window's boundaries (red-proof: with the old rule the window would have become the whole body)
- **TEST** t555 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: Source replacement cuts, the boundary moves only inward
- **WHEN** a patch with a source replacement covers the same event with fades
- **THEN** the target is cut, and the boundaries are pushed out of the transition zones INWARD the window (toward the far edges of the zones), not outward
- **TEST** t556 (`src/libs/kt-model/tests/morph_slice.rs`)

### Requirement: Target cutting rules
When cutting, the boundary SHALL snap to the material grid (a frame for video, 8 samples for audio; for reversed the count runs from the end of the body), then be pushed out of forbidden zones (transition windows, neighbours' overlaps) only inward; a stub shorter than the minimum collapses to the edge of the body; a window shorter than the minimum has no effect. A clip with its own or appended time warp MUST NOT be cut — it is patched whole. A clip covered entirely is not cut and keeps its id. Cut points SHALL be shared by all layers (morphers do not cut each other's pieces). The rule for a piece's entry point is one per project, and an ordinary split of an event goes through it too.
Why: cutting breaks not the target but its neighbours (the Vegas order "on top is the one that started later", the crossfade ramp from geometry); a linear `source_in` of a stretched clip points in the wrong place.

#### Scenario: Partial coverage — three pieces with exact material
- **WHEN** a block covers the middle of a clip (direct and reversed)
- **THEN** three pieces result; the material of each matches to the tick with a manual cut
- **TEST** t233 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: Degenerate cases and forbidden zones
- **WHEN** a clip is covered entirely; the window is microscopic; a video patch over audio; the boundary falls into the crossfade window
- **THEN** the entirely covered clip is not cut and keeps its id; a micro-window does nothing; a video patch does not touch audio; the boundary sticks to the edge of the zone
- **TEST** t236 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: A warped clip is not cut
- **WHEN** an audio clip with its own slowdown is partly covered
- **THEN** it is patched whole; there are no pieces
- **TEST** t238 (`src/libs/kt-engine/tests/morph_slice_render.rs`), t247 (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: One rule for a piece's entry point (container, capability `automation`)
- **WHEN** a stretched clip (constant speed, speed curve, reverse) is cut
- **THEN** the piece's entry point is the integral of speed, the seam has no gap, keys travel with the anchor; an unwarped piece is as before
- **TEST** t270 (`src/libs/kt-model/tests/morph_slice.rs`)

### Requirement: Piece id — a pure function; piece №0 holds the original's id
The piece id SHALL be computed from the original's id and the piece number without counters and new fields; the high bit is reserved, and a real clip above the limit MUST be rejected by validation; the original is recovered from the id. Anything that addresses a clip among the effective ones SHALL compare "the same clip", not the id literally.
Why: an id that does not depend on the edits of neighbours or on moving a block preserves the FX state and the transition's address.

#### Scenario: Determinism and the reserved range
- **WHEN** pieces are computed again and a clip with an id in the reserved zone is validated
- **THEN** the piece ids are identical, the original is extracted from the id, the clip is rejected
- **TEST** t234 (`src/libs/kt-model/tests/morph_slice.rs`)

### Requirement: Cutting is invisible in render
A transition on a cut clip SHALL survive: the end of the transition is found by "the same clip" and by checking the boundary against the join; the reference is the same edit cut by hand. A patch that does nothing to a clip of this kind SHALL give the previous result byte for byte.
Why: comparing ids literally returned piece No. 0, and the transition silently disappeared.

#### Scenario: The transition is alive for video and audio
- **WHEN** the layer replaces the entry point in the tail of a clip together with the crossfade window
- **THEN** the picture and the sound equal an edit cut by hand with a transition on the right half
- **TEST** t237 (`src/libs/kt-engine/tests/morph_slice_render.rs`)

#### Scenario: The cut is not audible
- **WHEN** a video layer stands over audio; audio is cut by a patch that does not change the sound
- **THEN** the audio is not cut at all / the sound is byte-for-byte as before
- **TEST** t238 (`src/libs/kt-engine/tests/morph_slice_render.rs`)

### Requirement: Bake materializes the cut as well, in one history step
Bake SHALL make the content of the clips equal to the render view before bake, including the pieces: piece No. 0 keeps the id of the original, the others get real ids; the ends of transitions and the frozen targets of the other morphers are re-hung onto the pieces; the morpher disappears; everything — in one undo.
Why: the invariant "bake == what was drawn"; the owner rejected "bake is forbidden for a layer".

#### Scenario: Bake cuts honestly
- **WHEN** a layer's bake is done, the layer having partly covered a clip with a transition at the join
- **THEN** the clips after == the effective ones before, no synthetic ids remain, the transition is on the piece at the join; one undo restores everything
- **TEST** t239 (`src/libs/kt-model/tests/morph_slice.rs`), t35 (`src/libs/kt-model/tests/morpher.rs`)

### Requirement: Layer strength and fades — a chain slot, not an event field
The strength of the overlay SHALL be an operation of the CHAIN (a blend slot indicating from which slot the layer's contribution begins): the blend goes with the state BEFORE the layer's contribution, not with the input of the whole chain. The default (strength 1.0, fades 0) SHALL not create a slot at all. Fades at the edges of the block are a strength curve. Strength acts on sound too: the event's reverb sounds on the dry side, the layer's reverb — only on the wet side; the volume "before the layer" goes to both sides.
Why: Vegas reconnaissance — "an overlay opacity slider, which we do not have"; the slot gets automation, the post-FX cache key and copy-paste for free.

#### Scenario: Strength rolls back only the layer
- **WHEN** an event with its own effect is covered by a layer with strength 1.0 / 0.0 / 0.5
- **THEN** 1.0 — the previous frame, byte for byte; 0.0 — the frame as WITHOUT THE LAYER, but with the event's effect; 0.5 — exactly the integer crossfade of these two; at the edges with fades the layer does not act, in the middle — fully
- **TEST** t267, t267b (`src/libs/kt-engine/tests/morph_slice_render.rs`), t181 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: Strength mutes the layer's sound
- **WHEN** an audio event with its own effect is covered by a layer with strength
- **THEN** the layer's contribution is muted; the event's own effects and level are intact
- **TEST** t268 (`src/libs/kt-engine/tests/morph_slice_render.rs`)

#### Scenario: Strength and fades are edited in the window and reach the model
- **WHEN** strength and fades change in the morpher window
- **THEN** a partly covered event gets a strength slot with the required values; a fully covered one on defaults has no slot
- **TEST** t269 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Layer curves — in the block's time domain
The curves of the layer's effects SHALL live in the patch in the block's domain (0..duration) and be translated into the target's material by the same mapping as everything else (reverse is taken into account); the order of curves SHALL match the order of the chain for the kind. A patch with only curves is not considered empty. The master switch of the event's chain MUST NOT mute the layer: it removes the event's chain entirely, and the layer's effects are appended to a clean one. Inserting a chain into a morpher SHALL carry its curves.
Why: a morpher has no material — it is a layer of edits; the event's switch has no relation to someone else's layer.

#### Scenario: The layer's keys land in the target's material
- **WHEN** the layer's effect has a curve, the target is direct and reversed
- **THEN** the keys in the effective clip stand at the same moments of the timeline, recomputed into the material
- **TEST** t266 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: The event's switch and chain insertion
- **WHEN** the master switch is on for a target; a chain with an envelope is inserted into the morpher
- **THEN** the event's chain goes away entirely, while the layer's effects remain; the curves of the inserted chain reach the patch
- **TEST** t214 (`src/libs/kt-model/tests/fx_auto.rs`), t343 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: A source swap carries the event's curves in a single delta
When the source is replaced by a layer, the keys of the event's own automation SHALL shift by exactly the amount by which the material anchor has shifted; the layer's own curves are recomputed separately and appended afterwards.
Why: a key is attached to the material, but the replacement changes the coordinate system — the curve drifted entirely out of the window, and the effect "stopped applying".

#### Scenario: The curve's value does not depend on the replacement
- **WHEN** an event with a curve is covered by a layer with a source replacement and an entry-point delta
- **THEN** the curve's value in the middle of the event is the same as without the replacement
- **TEST** t291 (`src/libs/kt-model/tests/morph_slice.rs`)

### Requirement: Copy-paste carries morphers, areas and markers
The copy block SHALL include morphers (if the block has at least one target; layers and references — by window intersection), areas and range markers by intersection with the rebase. Paste SHALL remap the targets to new ids; a reference to a copied name — to the copy, to a non-copied one — leave it; a morpher without surviving targets and a marker on an occupied tick are SKIPPED with a count, and the batch passes anyway. Names — by the same uniquifier with an affix (default — suffix "copy"); the affix dialog opens only if a name is in the block.
Why: an atomic batch would have dropped the whole paste because of one bad element — and it would have looked like "copy-paste does not work".

#### Scenario: A block with morphers and names
- **WHEN** a selection with a morpher, an area and a marker is copied; it is pasted twice
- **THEN** the targets point only to new ids, the reference to a copied name is renamed, to a non-copied one — no; the morpher without targets is skipped; the second paste gives " 2"
- **TEST** t249 (`src/libs/kt-model/tests/morph_slice.rs`)

#### Scenario: The affix window
- **WHEN** the buffer has a named item / has none
- **THEN** the window opens only with a named item; Esc leaves no trace; a second Ctrl+V opens it again; confirmation is one history step
- **TEST** t250 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Morpher placement on the timeline
The single item "Add morph layer" (and the toolbar button — the same function) SHALL create a layer in "below" mode and, in one history step, raise the morph row to the top (the natural order puts it last — "below" would then be empty). A new block SHALL be placed into the first service row free on this interval (from top to bottom); a new row — only if there is none such. A morph block SHALL move across rows like a clip and be caught by the frame.
Why: blocks hidden one under another, and "a new track every time" — both are the owner's complaints.

#### Scenario: The layer does not hide a block under a block and moves across rows
- **WHEN** "+M" is pressed several times on one interval; a block is dragged to another row; the frame is dragged over service rows
- **THEN** each block is caught by the mouse in its own row; the block follows the cursor in the preview too; the frame catches morph and camera blocks
- **TEST** t275, t276, t277 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: The event preview computes the same map as render
The waveform and the filmstrip SHALL translate pixels into material through the engine's map (taking the warp and reverse into account), without creating their own formula; stretching MUST NOT generate new decoding.
Why: a linear map in the UI drew not what was playing (the container — the rule of the warp-map cache, capability `ui-timeline`).

#### Scenario: The preview follows the stretch map
- **WHEN** the event is slowed down
- **THEN** the filmstrip and the waveform take the material by the integral of speed; the step is chosen by the span consumed
- **TEST** t249 (`src/apps/kt-app/src/layout_tests.rs`)
