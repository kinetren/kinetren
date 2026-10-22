# Generators

## Purpose

Video generators — sources without a file (solid colour, gradients, checkerboard, noise,
SMPTE bars, timecode, shapes, backdrop, countdown, engineering tables and titles):
a third kind of `MediaSource`, whose frame is a pure function (values on the event, tick,
quality), while the title is rasterized deterministically with a vendored font.

## Requirements

### Requirement: A generator — media without a file; the path carries only the kind
A generator SHALL be an ordinary media record with the pseudo-path `gen:<kind>[:<key>=…]`,
where the path canonically carries ONLY THE KIND of the generator (and the sort of the variant, if the name of the kind is
a function of the value), and not the values of the parameters. The dictionary of kinds and the parsing/assembly of the path
SHALL exist in one place (kt-model), parsing is case-insensitive, garbage gives
`None`; string values are escaped losslessly. Opening any pseudo-path
(`synth:`, `gen:`) SHALL go ONLY through the kt-media factory: a broken pseudo-path is an
honest error, it does not go to FFmpeg, and the sidecar DB is not opened for it. The lookup
of a source by a clip SHALL have one home (`Project::source()`).
Why: the parsing of `synth:` lived in two copies and diverged by case — a real
source was silently taken as a missing file.

#### Scenario: Dictionary of kinds and the canonical path
- **WHEN** a generator path is parsed and assembled back, including with text containing separators and non-ASCII
- **THEN** parsing is case-insensitive, assembly is canonical and idempotent, string values survive the round trip without loss, and each kind has a full passport of parameters
- **TEST** t469, t469b, t470 (`src/libs/kt-model/tests/genmedia.rs`)

#### Scenario: The factory is the only door for pseudo-paths
- **WHEN** a path `synth:…`, `gen:…` or an ordinary file is opened
- **THEN** the factory routes by scheme without regard to case, a broken pseudo-path returns an error, and neither a placeholder nor a call to FFmpeg
- **TEST** t471b (`src/libs/kt-media/src/factory.rs`)

#### Scenario: A pseudo-path does not touch the sidecar DB
- **WHEN** the thumbnail service or the source sync meets `gen:` media
- **THEN** sqlite is not opened (the DB key is the file's metadata, and a generator has none), and the source is raised by the factory directly
- **TEST** t504 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: An unparsable kind — an error, not silence
- **WHEN** media with an unknown kind arrives at the generator form
- **THEN** the form shows an error, and the hints of the common widgets reach the status line
- **TEST** t518 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The clip's source is looked up in one place (see capability `ui-reach`)
- **WHEN** code outside `project.rs` matches an element of `project.media` with the `media` field of a clip
- **THEN** the scanning guard names the file — for a generator event the address of the source is derived from the event, and a direct search of the media gives an empty frame
- **TEST** t426b (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: A generator frame is a pure function of exactly the requested quality
`GeneratorSource` SHALL return a frame of EXACTLY the requested Quality (the composite silently discards someone else's size), deterministically from (state, pts, Quality) — two
runs are byte-for-byte equal. "No sound" SHALL be expressed as `has_audio:false` AND
`Ok(silence)`, never `Err`. `info()` SHALL report duration=0 ("no duration"),
fps=None (the container's grid) and a native size equal to the raster of the CONTAINER (the project or the
reel in which the event stands), SAR 1:1 — so the generator is conformed as an identity and
a title in a square reel is computed in its raster; the margin of detail for the decode step is given by the
step formula (native ≥ Quality → step by the zoom of the chain). The "Max (by sources)"
pseudo-path does not take part. The frame cache by Quality SHALL hold several qualities without
double copying, and be disabled only for kinds that depend on time, and for
animated (with curves) states. The 3×5 pixel font (placeholder, timecode, countdown)
SHALL be a common module and is not the rasterization of user text.
Why: `Err` on "no sound" accumulated false underruns in the negative cache of the worker; a
native 4096×4096 with the conform rule would mean "as is" at 4096 size and fitting a
square into 16:9 — the generator must have the proportions of the container.

#### Scenario: Determinism and the `info()` contract
- **WHEN** the same generator is rendered twice in one Quality and `info()` is polled in a 1920×1080 project
- **THEN** the bytes match, the size is exactly Quality, duration=0, fps=None, native 1920×1080
- **TEST** t471 (`src/libs/kt-media/src/generator.rs`)

#### Scenario: Silence instead of an error
- **WHEN** an audio burst is read from a generator without sound
- **THEN** `Ok` with silence is returned, `has_audio` is false
- **TEST** t472 (`src/libs/kt-media/src/generator.rs`)

#### Scenario: The native of the generator feeds the decode step
- **WHEN** a zooming chain stands over the title
- **THEN** S×Quality is ordered by the step formula from the native of the container; without FX — exactly Quality
- **TEST** t484b (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: A title in the reel of its own raster
- **WHEN** a title stands in a 1080×1080 reel, and the reel is in a 1920×1080 project
- **THEN** the title is computed at 1080×1080 (native = the raster of the reel), the instance is fitted with margins, the bytes are deterministic
- **TEST** t726 (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: Cache by quality
- **WHEN** one frame is requested repeatedly in several Qualities
- **THEN** each quality is rendered once; for an animated title the cache is bypassed, and a static one does not suffer from the appearance of the animation
- **TEST** t507, t539 (`src/libs/kt-media/src/generator.rs`)

#### Scenario: The placeholder and the common pixel font are deterministic
- **WHEN** the stub frame of a missing file is rendered with a caption, including non-ASCII
- **THEN** the bytes are repeatable and do not depend on time; non-ASCII gives "?", the sound is silence
- **TEST** t208 (`src/libs/kt-media/src/placeholder.rs`)

### Requirement: Parameter values live on the event, the kind lives on the media
The values of the generator and their curves SHALL be stored on the event (`ClipEvent.gen`,
`gen_auto`); the media stays a kind and a cache bar; a mismatch between the kind of the event and the
media SHALL be rejected by validation. The source address of a generator event SHALL be
derived from the event, and the shape of the order `(address, tick, quality)` — the same as for
files: order == read for two titles of one kind. The curves SHALL be resolved by the source itself at its own tick (the plan does not know about them); without curves — zero cost. A change
of a value SHALL reach the source by retuning in place (`retune_gen`),
without rebuilding the renderer and without restarting the worker; the predicate "can it"
(`can_retune_gen`) MUST match the implementation. The retune request MUST NOT be
cancelled by a seek and MUST invalidate the cache inside the worker thread, including
the ancestor reels. Editing parameters MUST redraw the preview with a motionless
cursor.
Why: two titles on one media could not carry different text, and every key in the
title field shut down the decoder and rebuilt both renderers ("this sounds terrible").

#### Scenario: Values on the event and strict validation
- **WHEN** a generator event is created, changes its media, or is serialized
- **THEN** the media path carries only the kind, empty fields are not serialized; values of a foreign kind or without a generator medium are rejected by validation
- **TEST** t656, t657 (`src/libs/kt-model/tests/genmedia.rs`)

#### Scenario: The order equals the read for two titles of one kind
- **WHEN** two titles of one media with different text stand at one tick
- **THEN** two layers with different addresses are ordered, the composite is full, the pictures differ
- **TEST** t658 (`src/libs/kt-engine/tests/gen_event.rs`)

#### Scenario: The source resolves the curve
- **WHEN** a curve stands on a generator parameter
- **THEN** the frame is a function of the tick, the second run equals the first to the bit, and the plan does not see the curves
- **TEST** t659 (`src/libs/kt-engine/tests/gen_event.rs`)

#### Scenario: Editing a value does not rebuild the renderer
- **WHEN** `SetClipGen`/`SetClipGenAuto`/`SetMediaPath` is applied
- **THEN** the render plan is not touched, `set_project` is not called, the worker and the renderers remain as they were
- **TEST** t661, t500 (`src/libs/kt-engine/tests/player_dynamic.rs`), t501 (`src/libs/kt-engine/tests/scheduler.rs`), t502 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Changing the kind — a command over the media path
- **WHEN** `SetMediaPath` is applied and rolled back on a generator medium
- **THEN** the path is replaced opaquely (suitable for a future relink), the events are re-dressed for the new kind, undo returns the former path
- **TEST** t475 (`src/libs/kt-model/tests/genmedia.rs`)

#### Scenario: Retuning in place and its predicate
- **WHEN** the source receives new values of the same passport / of a different passport
- **THEN** `true` = applied and `info()` did not change; `false` = the caller recreates the source; the predicate matches the implementation over all pairs of kinds
- **TEST** t498, t499, t660 (`src/libs/kt-media/src/factory.rs`, `src/libs/kt-media/src/generator.rs`)

#### Scenario: The reel does not give a stale frame after retune
- **WHEN** a generator value is edited inside a nested reel with a motionless cursor
- **THEN** the sub-composite of the reel is invalidated together with the sheet and shows the new picture
- **TEST** t663 (`src/libs/kt-engine/tests/gen_event.rs`)

#### Scenario: The preview is redrawn without cursor motion
- **WHEN** a generator parameter is edited
- **THEN** the preview is rebuilt, the preview key's tick has not moved
- **TEST** t541 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Export raises its own generators and equals the preview
- **WHEN** export starts without a single external source
- **THEN** the list of sources is computed by one code for the player, export and kt-cli; the frame equals the preview byte-for-byte; GOLDEN_AUDIO/GOLDEN_VIDEO are untouched
- **TEST** t670 (`src/libs/kt-engine/tests/gen_event.rs`), t479 (`src/libs/kt-engine/tests/golden_export.rs`)

### Requirement: Exchange format — first-class `GeneratorReference.1` (see capability `project-format`)
Our generator SHALL be written with the schema `GeneratorReference.1` with `generator_kind="kt.<kind>"`,
`parameters` and a mandatory `available_range` (exactly what the clip consumed);
the values of the event SHALL go into its envelope. A foreign `generator_kind` MUST NOT be
typed — passthrough plus a noticeable stub; byte idempotence of foreign
files costs more than typing. The import alias `SMPTEBars` SHALL be canonicalized to our
`gen:smpte`. Old files with values in the path and an animation mode SHALL
migrate to the event by the same formulas that the preset sets.
Why: the old build sees a stub instead of a frame and carries the node verbatim —
there is no silent corruption, and we do not create converters (the owner's decision).

#### Scenario: The round trip of a first-class schema and a foreign kind
- **WHEN** a project with a generator is saved and read; a foreign file carries `generator_kind` not `kt.*`
- **THEN** our generator is read back as the same kind idempotently; the foreign one is passthrough with a stub, the file is byte-stable
- **TEST** t473, t474 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The SMPTEBars alias
- **WHEN** an upstream file with `SMPTEBars` is imported
- **THEN** it becomes `gen:smpte` and is canonicalized on re-saving
- **TEST** t494 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: Event values in the file and migration of old ones
- **WHEN** two titles of one kind with different text are written and read; a file of the previous build with values in the path and the animation mode in a field is read
- **THEN** each title returns with its own values; the old file gives values and curves equal to the preset
- **TEST** t664, t665 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: A title is deterministic — vendored font, pins, its own reference
The frame of a title SHALL be a pure function (parameters, W, H) without time, locale and
environment; rasterization SHALL go in a crate without GUI. The font of the frame — ONLY the
vendored one (passport `FONT-MANIFEST.txt` with sha256), the shaper and the rasterizer
are pinned `=x.y.z`, hinting is not executed, and a sixth FFmpeg library
(libass/freetype/avfilter) MUST NOT appear. A change of the font file or of a pin SHALL
mean regeneration of the title's goldens with an entry in the manifest; `GOLDEN_VIDEO` MUST
NOT move — the scene with a title lives in the separate `GOLDEN_TITLE_VIDEO`. Positions
SHALL be computed by the shaper (GPOS kerning, ligatures), pixels — by the rasterizer from the
same bytes; the scale is derived via `units_per_em` in one function. The sizes of
the parameters are relative (the size of the type is a fraction of the frame height, the rest are fractions of em). The system
family SHALL resolve to a concrete file whose path and bytes enter the identity of the
frame; a miss on this machine — substitution by the vendored byte-for-byte plus a visible
notification (once per family). Codepoints without a glyph SHALL be visible to the
user. The styling (plate/shadow/outline/fill) SHALL be composed by the stored `order`.
Why: Noto Sans has no legacy `kern` — without the shaper the kerning would be zero;
`PxScale` is the height, not the em; any `cargo update` of the rasterizer would silently shift
the bytes of the golden.

#### Scenario: The rasterization anchor and the metric
- **WHEN** a title is rendered with the vendored font; the layout is computed in the ×2 frame
- **THEN** the frame hash equals the anchor (a guard of the pins and the font); the layout is exactly ×2 (the PxScale trap)
- **TEST** t480, t481 (`src/libs/kt-text/src/lib.rs`)

#### Scenario: GPOS kerning and uncovered codepoints
- **WHEN** the pair "AV" and a string with a character outside the font are laid out
- **THEN** the width of the pair is less than the sum of the singles; the list of uncovered codepoints is non-empty and exact
- **TEST** t482, t483 (`src/libs/kt-text/src/lib.rs`)

#### Scenario: Styling and the order of layers
- **WHEN** the title has the plate, shadow and outline enabled in the given `order`
- **THEN** the outline does not eat the stroke of the glyph (fill on top), the shadow is a shifted blurred silhouette of one channel, the plate follows the line
- **TEST** t486 (`src/libs/kt-text/src/lib.rs`)

#### Scenario: System families and substitution
- **WHEN** a system family that is absent on the machine is requested; the list of families is polled again
- **THEN** the frame is byte-for-byte equal to the vendored one and marked "substituted"; the list is built once
- **TEST** t490, t506 (`src/libs/kt-text/src/lib.rs`)

#### Scenario: UI notices — "no glyph" and "font substituted", once
- **WHEN** the user enters a character without a glyph; the project carries a missing family and is edited many times
- **THEN** the form shows "No glyph: „…“ U+…"; the notice about the substitution is issued once per family, not on every keystroke
- **TEST** t488, t505 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A separate golden for the title
- **WHEN** a scene with a title over a solid is exported
- **THEN** the result is deterministic, equals the committed `GOLDEN_TITLE_VIDEO`, preview == export; moving the values to the event did not shift the reference
- **TEST** t485, t662 (`src/libs/kt-engine/tests/title_golden.rs`), t484 (`src/libs/kt-media/src/generator.rs`)

### Requirement: Title animation — numeric kinds, clusters, presets set everything
kt-text SHALL have no time: the animation arrives as numbers of a kind (`reveal`,
`reveal_soft`, `offset_x/y`), which the source computes from the pts of the material.
The layout SHALL be computed once over the full line; the unit of reveal is the
grapheme cluster of the shaper (a ligature, a base with diacritics, ZWJ — one unit);
at full visibility the EXACTLY former branch is taken. The offset of a block SHALL be
quantized to 1/64 px before being passed on — the mutual geometry of the glyphs between frames
is constant bit for bit. Animation presets SHALL set values AND curves, including
position and anchor, in one step of history; the active preset SHALL be determined by the inverse
function over both halves, and there is no flag "which preset was pressed". The formulas of the presets
and the migration of old files SHALL be the same ones.
Why: a typewriter that cuts the line feeds the shaper a different sequence
on every frame — the glyphs would mutate; a "crawl" not shifted off the edge is
useless (the owner's decision).

#### Scenario: Clusters and reveal
- **WHEN** the layout requests the boxes; `reveal` grows from 0 to 1; the soft front is enabled
- **THEN** the boxes of the clusters are returned; the clusters open in order (for RTL — right to left); the soft reveal is not a hard cut; at `reveal>=1` the frame equals the static one
- **TEST** t534, t535, t536 (`src/libs/kt-text/src/lib.rs`)

#### Scenario: Offset of a block without reshaping
- **WHEN** a block is shifted by an arbitrary fraction of a pixel
- **THEN** the glyphs are moved without changing the shape; the offset is quantized to 1/64 px
- **TEST** t537, t538 (`src/libs/kt-text/src/lib.rs`)

#### Scenario: Animation follows the time of the material
- **WHEN** an animated title is read at different pts
- **THEN** the frames differ by the pts of the material, not by the tick of the timeline
- **TEST** t539 (`src/libs/kt-media/src/generator.rs`)

#### Scenario: Preset — one step, inverse function
- **WHEN** a preset is pressed; then the user tweaks a field
- **THEN** the values, curves, position and anchor are set in one undo step; after the tweak the active preset is "matches none"
- **TEST** t540 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Generator curves from the form
- **WHEN** in the generator form (compact view) a curve is added to a parameter
- **THEN** the curve reaches the model (`gen_auto`)
- **TEST** t666 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Other kinds — determinism, honest alpha, sound from one formula
Noise SHALL be counter-based: the grain is a function of a cell of the base grid 1920×1080 and the
pts of the material; the shift of the clip, the step and the Quality do not change the picture. The SMPTE tone and the
countdown beep SHALL be computed by one code with the phase from the global sample number
(a seek does not click); switching the tone on makes `has_audio` true. The timecode and the countdown
SHALL show the time of the MATERIAL. A kind SHALL be a variant of an enum when it
has its own numbers (Rect/Ellipse/Line — three kinds; tables — one kind with a field
of variety). The alpha of a kind SHALL be declared by an exhaustive method, not by
`matches!`; the rasterization of shapes is an integer distance field, float only in the
preamble of the frame. A title with alpha under the FX chain SHALL remain transparent outside the
glyphs.
Why: a new transparent kind slipping past `matches!` would land in the composite as a black plate; a second
sine would drift out of phase.

#### Scenario: Noise, SMPTE, timecode
- **WHEN** the noise is read with a shifted `source_in` and in another Quality; the SMPTE tone is enabled; the timecode is read at a pts
- **THEN** the grain of the noise is unchanged; the tone is at the set dBFS, `has_audio` is true; the timecode digits follow the material at the set fps
- **TEST** t491, t492, t493 (`src/libs/kt-media/src/generator.rs`)

#### Scenario: Shapes — a kind is its own numbers, the coverage is honest
- **WHEN** the passports of the rectangle, the ellipse and the segment are compared; Rect and Line are rendered
- **THEN** each kind has its own parameters without dead controls; Rect covers exactly what is declared, Line is a capsule between the ends
- **TEST** t525 (`src/libs/kt-model/tests/genmedia.rs`), t528, t529 (`src/libs/kt-media/src/generator.rs`)

#### Scenario: The alpha is declared by each kind
- **WHEN** all kinds of the registry are enumerated
- **THEN** the `has_alpha` of the frame equals the declaration of the kind; a title under the FX chain does not become an opaque plate
- **TEST** t527 (`src/libs/kt-media/src/generator.rs`), t521 (`src/libs/kt-engine/tests/transitions_video.rs`)

#### Scenario: Countdown and tables
- **WHEN** the countdown is read by seconds and on "2-pop"; the tables are rendered
- **THEN** the digit decreases, the white field has a length of 1/fps, the beep is on whole seconds with gates; the values of the tables are frozen
- **TEST** t530, t533, t531 (`src/libs/kt-media/src/generator.rs`)

### Requirement: Generator UI — one list of kinds, the event under the cursor, the form at the chain head
Creating a generator SHALL be available from the main menu and from the bin menu through one
list of kinds (the entries do not enumerate the kinds themselves); creation puts the media AND a clip under the
cursor in one step of history and opens the form. The form of the generator/title SHALL live
as the first crumb of the event window (the head has no switch); all entries
are routed into it; the change of kind is its own step of history; a session of the form is one
undo step; an edit SHALL reach the model and the source, and undo too. Fractions of the frame SHALL
be shown as percentages (storage — a fraction), a double click on a label resets the
parameter; every parameter has a label, every flag has a control; the colours
of the title are shown as real (direct fields, unmultiplied components). Safe zones
SHALL be drawn over the preview while the title is open; dragging the title in the preview SHALL
write the position with the same command and the same gesture as the form. The bin card SHALL
show the name of the kind, hide a zero duration, count the "×N" places, and build
a real thumbnail without a file; an empty default frame is labelled with the name of the kind;
the event is labelled with the name of the kind and is not repainted on every keystroke.
Why: the owner did not find the single entry (right click on the empty background of the bin);
the title window was a dead end without a kind picker; a double click placed two clips overlapping.

#### Scenario: One list of kinds for all entries
- **WHEN** the main menu and the bin menu are checked
- **THEN** both call the common generators menu and do not enumerate `GeneratorKind::all()` themselves
- **TEST** t560 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Creation from the bin and placement on the timeline
- **WHEN** a kind is selected in the menu; a double click is made on the card
- **THEN** the media, the source, the card and the clip under the cursor appear in one step of history, the form is open; a double click gives one clip, not two
- **TEST** t477, t512, t514 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: An edit reaches the model and is rolled back
- **WHEN** a value is changed in the form; then undo; then 30 edits in a row
- **THEN** the model and the frame of the source are updated, undo returns the picture, the session is one step, zero restarts of the source
- **TEST** t478, t502 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Routing of the form and changing the kind
- **WHEN** the form for `gen:text` and for a solid is opened; the kind is changed in the picker
- **THEN** each kind gets its own form, a change of kind is its own step of history
- **TEST** t487, t511 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Percentages, reset, labels, colours
- **WHEN** a fraction of the frame is drawn in the form; a label receives a double click; parameters and flags are enumerated; a title with colours is opened
- **THEN** "7.0 %" is shown for a stored 0.07; the parameter is reset to the default of the spec; each parameter has a label, the flag has a control; the colours of the form equal the model
- **TEST** t516, t517, t542 (`src/apps/kt-app/src/layout_tests.rs`), t508, t509 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Safe zones and dragging in the preview
- **WHEN** a title is open; the block is dragged with the mouse in the preview
- **THEN** the 5 %/10 % safe zones are visible only while the title is open; the position is written by the same command and gesture — one undo step
- **TEST** t489, t495 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Card and event
- **WHEN** a generator lies in the bin without a file, stands in N places; an event is on the timeline
- **THEN** the thumbnail and probe are built without a file and without a DB, the kind "video without audio", the zero duration is hidden, "×N" is shown; the event is labelled with the name of the kind
- **TEST** t476 (`src/apps/kt-app/src/thumbs.rs`), t513, t515 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The generator entity in the "no dead ends" mechanism (see capability `ui-reach`)
- **WHEN** `TlEntity::Generator` appears with the verb "open parameters"
- **THEN** the verb table, the absence of orphaned commands and the panel's response to a right click cover the generator with its own row
- **TEST** t252, t253, t395 (`src/apps/kt-app/src/reach_tests.rs`)
