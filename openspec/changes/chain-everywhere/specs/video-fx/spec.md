## MODIFIED Requirements

### Requirement: The effect thumbnail is "fit and apply", and no more
A cell of an event or a morpher (a variant, a strength-axis point, a library preset) SHALL be the
frame the preview composes after the click: the project the player plays at the cursor's moment,
the candidate at the tail of the event's chain (its track soloed) or of the morpher's video patch,
composed by `frame_plan` on the offline side engine at the REFERENCE size (320×180), then shrunk
into the cell. A second path that re-creates the chain around the effect in the UI MUST NOT be
introduced. Until the composite is complete the cell SHALL show Loading — never the sample, a
source still or a partial composite. The built-in sample (frozen by a golden, with a transparent
variant) is the base only where no chain exists: the sample job is "fit and apply" of the chain on
the offline engine with `in_scale = 1`; its icon of a kind with an identity default is the first
variant; the grain of noise SHALL be pinned to the tick.
Why: the picture in the picker must coincide with what results after a click; the owner
(2026-11-14): "НИГДЕ НЕ ДОПУСТИМО чтобы эффекты накладывались мимо цепочки" — a still with
"chain + candidate" showed Datamosh's candidates over the un-moshed frame (found-defects 460).
Pixel parameters give a different strength at 96×54 and at 1080p — the reference size stays.

#### Scenario: The sample is frozen and deterministic
- **WHEN** the sample is generated twice in three sizes/modes
- **THEN** the hashes equal the reference, the sizes and the alpha flag are correct
- **TEST** t544 (`src/libs/kt-fx/tests/fx_thumb.rs`)

#### Scenario: The sample feeds every class of effects
- **WHEN** a mirror on any axis, greenscreen, colour, blur/sharpen/edges/pixelate/raster are applied to the sample
- **THEN** each result differs from the input, and greenscreen produces pixels with zero alpha
- **TEST** t544b (`src/libs/kt-fx/tests/fx_thumb.rs`)

#### Scenario: Identity remains identity, the variant does not
- **WHEN** the thumbnail is computed for the identity default of a kind and for its variant with zoom
- **THEN** the default equals the fitted sample, the variant differs
- **TEST** t544c (`src/libs/kt-fx/tests/fx_thumb.rs`)

#### Scenario: A shadow requires an alpha sample
- **WHEN** DropShadow is computed on an opaque sample and on an alpha sample
- **THEN** on the opaque one — passthrough, on the alpha sample — not
- **TEST** t544d (`src/libs/kt-fx/tests/fx_thumb.rs`)

#### Scenario: The candidate goes where the click puts it
- **WHEN** a scene is made for an event on the second video track with a bypassed slot, for a missing event and for a missing morpher
- **THEN** the candidate stands at the tail of the event's chain unbypassed, only that event's track is soloed, and the missing places give no scene
- **TEST** t2554 (`src/apps/kt-app/src/fx_thumbs.rs`)

#### Scenario: A cell is the frame after the click
- **WHEN** a cell renders the candidate mirror for an event with posterize on a track with its own posterize stage
- **THEN** the cell equals, byte for byte, the offline composite of the project with mirror appended, fitted into a cell, and differs from the chain over the sample
- **TEST** t2555 (`src/apps/kt-app/src/fx_thumbs.rs`)
- **PATH** open an event's effects window, wait out the misclick guard: each cell of the gallery shows the event's frame as the preview will after that click

#### Scenario: A cell after Datamosh shows the candidate over the mosh
- **WHEN** a cell renders the candidate mirror for shot B of a cut mosh (datamosh on shot B), inside the mosh
- **THEN** the cell equals the composite after the click (the history pre-rolled) and differs from mirror over the un-moshed frame
- **TEST** t2556 (`src/apps/kt-app/src/fx_thumbs.rs`)
- **PATH** open the effects window of an event with Datamosh: the cells show "Loading" while the history is computed, then each candidate over the mosh

### Requirement: The thumbnail service is a service, not a task
Thumbnails SHALL be computed only by a dedicated service: its scheduler lane, a LIFO queue with
deduplication, cancellation by generation, a RAM LRU of 256 cells. A scene cell's sources SHALL be
the service's own, opened on its lane as the composite names them (invariant 5's offline path: not
shared, no promised pace, finite, cancellable by generation and by the lane's pause), with a cache,
a stream memo and a build memo of its own; the last cell window to close drops them. The cell key is
(base, recipe); a scene's base names the project's revision, the place and the moment, its recipe
the candidate. The 800 ms mis-click guard SHALL guard opening and decoding: before it a scene cell is
Loading and asks nothing. A cell that cannot complete SHALL NOT be asked again until the windows
close. The egui texture cache lives in the service.
Why: scrolling/zooming and opening the picker do not generate decode requests of the live path (the
cache invariant); a real frame needs the event's files and history, and at rest the service costs
nothing.

#### Scenario: Queue, LRU, recipe, delivery, guard
- **WHEN** requests are placed, the generation grows, the LRU overflows, the chain/moment/camera changes, a request is delivered
- **THEN** the latest request goes first, duplicates collapse, the old generation is cancelled, the oldest is evicted, the recipe distinguishes only what changes the picture, and the delivered cell has the cell's size
- **TEST** t546 (`src/apps/kt-app/src/fx_thumbs.rs`)

#### Scenario: Render path — request first, then texture
- **WHEN** the picker context asks for a cell's texture
- **THEN** first a request is placed and `None` is returned; after the worker finishes, the texture is returned
- **TEST** t546g (`src/apps/kt-app/src/fx_thumbs.rs`)

#### Scenario: A scene cell waits for the guard, a failed one is not retried
- **WHEN** a scene cell is asked before the guard, then after it while it is marked failed, then the windows close
- **THEN** nothing is queued before the guard, the failed cell is not queued again, and closing forgets the failure
- **TEST** t2557 (`src/apps/kt-app/src/fx_thumbs.rs`)
- **PATH** open an effects window: for the first 800 ms every cell reads "Loading" and no file is opened

#### Scenario: A stopped scene cell publishes nothing
- **WHEN** a scene cell's job runs with its stop already asked (the picker moved on)
- **THEN** it comes out as stopped, with nothing composed
- **TEST** t2558 (`src/apps/kt-app/src/fx_thumbs.rs`)

### Requirement: Picker gallery — WYSIWYG, and the click reaches the model
The effect row SHALL show the category icon, the name and a strip of cells
(variants and the suitable single-effect presets); the audio picker stays
text-only. A cell SHALL be the frame after the click (the effect thumbnail requirement), at the
cursor tick inside the event (frozen only during playback); a morpher's cells at the cursor kept
inside its span. A cell without its composite reads "Loading". The popup MUST close only by a click
outside or by an explicit choice; a cell below the scroll fold MUST NOT be drawn or requested.
Categories are grouped once at draw time; the caption is truncated to one line;
the accessible name of a cell is compound ("Blur Heavy").
Why: the egui menu default is "close on any click" — the gallery would be stillborn;
an invisible cell published a rectangle to the timeline, and the
live scenario clicked past the popup.

#### Scenario: A click on a cell reaches the model
- **WHEN** a click on a variant cell inside the popup
- **THEN** an effect with the parameters of exactly this variant appears in the event's chain; input inside the popup does not close it
- **TEST** t547 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Layout is stable and truncated
- **WHEN** the gallery is laid out with long names, repeated categories, and in the audio picker
- **THEN** the caption is truncated and no wider than the cell, the category heading appears once, the width does not jump with the query, and audio cells have none
- **TEST** t548 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Moment and camera of the strip — by engine rules
- **WHEN** the strip computes the event tick for a reversed clip and resolves the camera for the Pan 3D variant
- **THEN** the map "event tick → source frame" is one and the same as for the filmstrip and waveform and knows about reverse; the camera rule is one: pinned → active → default
- **TEST** t549 (`src/apps/kt-app/src/thumbs.rs`), t545 (`src/libs/kt-engine/tests/cam_resolve.rs`, capability `video-plan`)

### Requirement: A stream stage reads the output of the slots in front of it
The slots of a chain in front of a `Stream` slot that change the picture SHALL run on every frame of
the main stream (the event's own source and its history in the event before) before the stage's
kernel reads it, with their curves at the position's material time, their masks and the camera the
layer would resolve for them, at the stream's size; the chain after the stage SHALL NOT run them
again. The leading Geometry node stays after the stage (it places the event's picture on the
canvas). The second input (the event on the track above/below) SHALL be read through that event's
slots in front of its own stream slot (its whole chain without one), with their curves, masks and
camera, never as decoded. The stream's addresses SHALL name what feeds it: an edit of a slot in
front, of its mask, of a camera it reads, or of the second input's chain SHALL invalidate the
history, an edit of a slot after the stage SHALL NOT. With no device for the inputs' slots, the
frame SHALL be missing, never the decoded picture in its place.
Why: found-defects 484 — the order the person built (camera_isp → datamosh → vhs_tape) was not the
order run (`stream-input-order`); found-defects 541 and 540 — a masked slot in front acted on the
whole frame, a Pan3d there was skipped, another track was read without its effects (the owner,
2026-11-14: "НИГДЕ НЕ ДОПУСТИМО чтобы эффекты накладывались мимо цепочки", `chain-everywhere`).

#### Scenario: A slot in front feeds the stream
- **WHEN** shot B's chain is posterize → datamosh, composed at a frame inside the mosh
- **THEN** the frame equals the mosh of sources posterized before decoding (both shots, the history in the event before included), the slot ran on the stream's frames, and the frame differs from the chain datamosh → posterize
- **TEST** t2470 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: An edit in front invalidates the history, one after does not
- **WHEN** a frame inside the mosh was composed with the chain posterize(3) → datamosh → posterize(5) on a shared memo, and the order is asked again by renderers of posterize(3) → datamosh → posterize(4) and of posterize(4) → datamosh → posterize(5)
- **THEN** the first asks no frame of shot B, the second the whole history (7 or more)
- **TEST** t2471 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: An export runs the slot in front
- **WHEN** a range with posterize → datamosh is exported in one go and in two pieces, each with a fresh renderer, memo and cache
- **THEN** the pieces equal the whole, and the whole differs from the export of datamosh → posterize
- **TEST** t2472 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: A masked slot in front keeps its mask
- **WHEN** posterize in front of datamosh carries a left-half mask; the mask is then moved, on a shared memo
- **THEN** the frame is neither the unmasked slot's nor the no-slot's, and the moved mask makes the order ask the whole history again
- **TEST** t2550 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: A Pan3d in front has its camera
- **WHEN** Pan3d stands in front of datamosh under a camera turned +35° and −35°
- **THEN** the frame differs from the chain without it, and the two cameras give two frames
- **TEST** t2551 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: The second input runs its chain
- **WHEN** datamosh takes its vectors from the track below, where the event carries a mirror
- **THEN** the mirror runs on the second input's frames, and the frame equals the mosh of a source mirrored before decoding
- **TEST** t2552 (`src/libs/kt-engine/tests/stream_stage.rs`)

#### Scenario: An edit of the second input's chain invalidates the history
- **WHEN** a frame was composed with posterize(3) on the event below, and the order is asked by a renderer with posterize(4) there, on a shared memo
- **THEN** the second asks frames of the event below the first already held
- **TEST** t2553 (`src/libs/kt-engine/tests/stream_stage.rs`)
