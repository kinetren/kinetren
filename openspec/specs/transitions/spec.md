# Transitions

## Purpose

A transition between two events of one track: a vocabulary of 33 picture kinds with one
kernel code for preview and export, an audio crossfade with a choice of curve, inversion,
parameter curves, solo fades, transitions on a joint and on an overlap, their model, format and window.

## Requirements

### Requirement: One transition vocabulary, grows only through the registry
The transition kinds SHALL be enumerated by a single registry (`TransitionKind::ALL`, 33 kinds:
crossfade, wipes, patterns, shifts, stylised, volumetric); the parameter specs of each kind
SHALL be built by the same passport macro as for effects, and the UI of transition fields MUST NOT
have its own form. The default splice is a crossfade; new kinds are chosen explicitly.
The sound SHALL crossfade under any picture kind (there are no stylised audio transitions — the
owner's decision), and the audio curve is chosen from four (equal-power by default, linear, S-curve,
exponential). A new kind that is not in the registry, or that has not decided whether it is
symmetric, MUST NOT build.
Why: a single registry makes it impossible for "what can be applied" and "what is drawn" to drift
apart — the owner complained about exactly this mismatch.

#### Scenario: Completeness of the specs and audio curves
- **WHEN** all kinds of the registry are walked
- **THEN** there are exactly 33 of them, the tags are unique, the defaults of the specs equal the constructor's defaults, the values are clamped; at the middle of a joint equal-power keeps the sum of powers, linear gives the half-sum, exponential — a deliberate dip
- **TEST** t223 (`src/libs/kt-model/tests/transitions.rs`)

#### Scenario: The kind and the curve live in the file and raise the version by content
- **WHEN** a project with a non-crossfade, a non-default audio curve or an inversion is saved and read
- **THEN** the kind, the curve and the inversion survive the round trip; a meaningful `transition_type` goes outward, a foreign type is not substituted; a file with a single crossfade is byte-stable and stays on the prior version; when returning to a crossfade our envelope is cleared; the key `invert` is written only when true and raises the file to v7; an unreadable kind is a remark, not silence
- **TEST** t222 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Transition kernels — one translation to numbers; CPU reference and WGSL mirror are edited as a pair
A kind from the model SHALL be turned into a flat set "mode + 8 numbers" exactly once, and the CPU kernel
and the WGSL mirror of the composite SHALL read the same numbers and repeat the same formulas;
`kt-engine/transitions.rs` and `composite.wgsl` MUST be edited in one commit. The mode numbers are a
contract only between these two files (the tag goes into the file, the ready set of numbers goes into the
cache keys), a new mode SHALL be added only at the end. The crossfade MUST NOT enter the kernels: it stays
the integer dissolve path, byte-for-byte as before. The parameters of a kind SHALL be included in the GPU plan key.
Why: a divergence of the mirrors means that preview and export show different things (invariant 7), and
without the parameters in the key a readback cache would return the transition frame of another kind.

#### Scenario: Conformance of each mode on both axes and at non-default phases
- **WHEN** each of the 32 non-crossfade modes runs on the CPU and the GPU at several phases
- **THEN** the frames converge within the tolerance of the conformance helper (volumetric — `FrameTol::GEOM`); the case list MUST contain both axes and both flags of the directional kinds, shifted centres for the kinds with a centre, the phase between a quarter and the middle, and the solo fades — at the defaults (`Dir::Left`, centre 0.5/0.5, k=0.5) a broken mirror is invisible
- **TEST** t224 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: The kind reaches the picture and to the alpha of a solo fade
- **WHEN** a pair on a joint is rendered in the middle of the window with a wipe, blinds, push, dip and crossfade
- **THEN** the wipe gives two monotone areas with a sharp edge, the blinds give symmetry at both edges, the push gives an exchange of halves, the dip gives fading to the colour, the crossfade gives a uniform mix; a repeat is byte-for-byte; for a wipe fade the alpha cuts the frame, for a crossfade fade it is one value over the whole frame
- **TEST** t225 (`src/libs/kt-engine/tests/transitions_video.rs`)

#### Scenario: The window's pair is one layer before the composite
- **WHEN** both frames of the pair are in the cache, or one half is missing
- **THEN** the pair is blended linearly (k/4096, a frame without alpha — A=255) into one layer before alpha_over, preview and export by one code; the missing half gets the full weight and `complete = false`; an FX chain on the members of the pair does not break the dissolve
- **TEST** t137 (`src/libs/kt-engine/tests/transitions_video.rs`), t182 (`src/libs/kt-gpu/tests/parity.rs`)

### Requirement: Directional kernels are guarded by a closed-form test, not by a mirror
Every directional kernel (push, whip, split, squeeze, stretch, swap, patterns, volumetric)
SHALL have a test where the expectation is computed by an independent formula on ramp frames or
analytic zones, and not by a second instance of the same formula. The incoming frame of push/whip SHALL
show its real content (it is sampled further along the direction of motion, `uv + dir·k`), not a smeared edge.
Why: CPU↔GPU conformance is blind to "both mirrors are broken in the same way", and a render test on solid
colours is blind to coordinates — so a broken push lived for four rounds.

#### Scenario: Push and whip bring in the real frame B
- **WHEN** the push/whip kernel runs on ramp frames (R along X, G along Y, B — the source marker) at an integer shift
- **THEN** the middle of the incoming strip shows the middle of frame B by the ramp formula, not its first row/column; the same through the full renderer
- **TEST** t459, t459b (`src/libs/kt-engine/tests/transitions_video.rs`)

#### Scenario: Patterns, symbiotic shifts and stylised kinds — analytic zones
- **WHEN** in the middle of the window checkerboard, split, stretch, squeeze, blinds, blocks, swap, zoom blur are rendered
- **THEN** checkerboard — opposing halves of cells; split — A|B|A; stretch — a band from the centre; squeeze — zones of B and A; blinds — the fraction of the band; blocks — each cell wholly A or B; swap — empty corners and both sides with a shadow of 0.75; zoom blur — a fixed centre
- **TEST** t467 (`src/libs/kt-engine/tests/transitions_video.rs`)

#### Scenario: Volumetric kinds — zones and transparency behind the panels
- **WHEN** flip, door, cube, peel, roll are rendered at the start, on the edge and at the end of the window
- **THEN** flip shows emptiness at the edges on the edge, door — the opening and the darkening of the leaf by the formula, cube — both faces with a shadow, peel — the back of the tail against the open, roll — the lying tail against the open; the front of the turn moves in the direction of `dir`
- **TEST** t468 (`src/libs/kt-engine/tests/transitions_video.rs`)

### Requirement: Volumetric kinds — via one ext function, with no new pass and no new bytes in the keys
The homographies of the panels of volumetric kinds SHALL be computed by one function (`trans_ext`) in f64 with
a single rounding to f32, and both backends SHALL read the same bits through the free field of the composite
uniform; a separate f64 reference for the GPU MUST NOT be created. Ext MUST NOT enter the cache keys
(it is a function of the already hashed mode, numbers and progress). Occlusion is the analytic validity of a
coordinate in the kernel; a depth buffer MUST NOT be added.
Why: parity by construction is cheaper than a tolerance, and a systematic divergence from the GPU would have to be hidden.

#### Scenario: Volumetric modes on the GPU converge with the CPU within the geometric tolerance
- **WHEN** flip/cube/door/peel/roll run on both backends with both axes and both signs
- **THEN** the frames converge within `FrameTol::GEOM`; the plan key and the hash function of the set of numbers do not change
- **TEST** t224 (`src/libs/kt-gpu/tests/parity.rs`), t468 (`src/libs/kt-engine/tests/transitions_video.rs`)

### Requirement: Inversion — a property of the event, executed by the plan before the kernel
The inversion of a transition SHALL be stored on the transition event as a flag and executed by the plan as a
single substitution "sides swapped, progress 1−k" before the kernel; the kernels, WGSL and the cache keys MUST NOT
know about the inversion. Any construction of a dissolving layer (a joint pair, an overlap pair, a solo fade) MUST
pass through one inversion helper. For a crossfade the inversion is the identity, and the integer path is not touched.
For symmetric kinds the flag SHALL be hidden; the kinds' own flags (clock direction, brightness threshold, verticality
of blinds) are other axes and remain. Audio does not see the inversion.
Why: the owner chose the "swap the sides" flag, not a progress curve; execution by the plan gives inversion to all
33 kinds without a single edit of the kernels.

#### Scenario: Inversion swaps the sides, not the kernels
- **WHEN** a pair is rendered with and without the inversion
- **THEN** the crossfade is byte-for-byte the same; the wipe swaps the sides at the same edge; an inverted push to the right equals a push to the left frame by frame; a solo fade is cut from the other side
- **TEST** t461 (`src/libs/kt-engine/tests/transitions_video.rs`)

### Requirement: Kind parameters are animated the same way as effects
The parameter curves of a kind SHALL live on the transition event in ticks from the start of the window and be
resolved at every moment; at the start of the window the value SHALL coincide with the static one; a key beyond the
edge of the window is legal (hidden); a curve on a non-existent parameter SHALL be rejected by validation. A repeated
gesture over the transition (the corner of the fade, an overwrite by Move-drag) MUST edit only the window length and
MUST NOT erase the kind, the parameter curves and the audio curve.
Why: the animation mechanism is one for everything; "nothing is switched off by itself" is the owner's rule.

#### Scenario: The corner curve changes the picture
- **WHEN** a wipe has a corner curve and the start and the middle of the window are rendered
- **THEN** at the start the picture equals the static value, in the middle it differs; a key beyond the edge of the window passes validation; an orphaned curve breaks the load
- **TEST** t232 (`src/libs/kt-engine/tests/transitions_video.rs`)

#### Scenario: A repeated gesture does not erase the configured kind
- **WHEN** the corner of a configured wipe fade is pulled further, or a pair with configured blinds is overwritten again by a Move-drag
- **THEN** the kind, the parameter curve and the audio curve survive, only the window offsets change
- **TEST** t460, t460b (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: A solo fade — a transition with one live end, the same kernel with an empty side
A fade at an edge SHALL be the same transition event in which one side is empty (both empty —
an error); the joint is resolved by the live end, the offset toward the emptiness is not limited by the body, and the death
of the live end takes the transition with it. The render SHALL be computed by the same kernel with an empty second side,
which is not read; the shape of the fade lives in the alpha of the layer, and the lower track shows through beneath it
(if there is none — black); `complete` MUST NOT be reset — there is no pair by the model, nothing to wait for. The audio
weights of the fade are the same as for a crossfade; the handle toward the emptiness is not extended. A corner fade is the
same transition with a zero external offset.
Why: a "wipe from nothing" without a second frame and without a transparent texture, and playback does not wait for a
non-existent layer.

#### Scenario: The fade in the model
- **WHEN** a transition is created with one empty side, cut, copied and imported next to a Gap
- **THEN** the joint is computed by the live end, the fade survives edits and the clipboard together with its clip, a transition as the first/last child of a track is read as a fade
- **TEST** t155 (`src/libs/kt-model/tests/transitions.rs`)

#### Scenario: Rendering the fade
- **WHEN** a fade is rendered in the middle and at the edge of the window
- **THEN** audio — a ramp from silence and silence at the edge; video — half brightness in the middle, fading out to the edge, `complete` is true
- **TEST** t156 (`src/libs/kt-engine/tests/transitions_video.rs`, `src/libs/kt-engine/tests/transitions_audio.rs`)

#### Scenario: The fade in the UI — free edge and corner
- **WHEN** the edge of a fade window is pulled with no neighbour, the corner of the event is pulled in the top strip, the corner is pushed back
- **THEN** the empty-side edge is not clamped by the body; the fade is drawn with one diagonal, a crossfade — with an X; the items ""Fade in"" / ""Fade out"" appear only at the free edge; a corner fade does not change the body or `source_in`; a corner pushed back within 3 px removes the fade entirely
- **TEST** t157, t173 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: The event level is mixed where the fades live
The opacity of a video event SHALL be multiplied by the alpha of the layer at the same place as the fade:
an ordinary layer — by scaling the alpha, a solo fade — by multiplication, and for a dissolve pair the levels SHALL be
blended with the same weight as the picture. Level 1.0 is the identity. The level line in the UI is one for audio
(gain in the rail range ±6.02 dB) and for video (opacity); the corner of the fade takes priority over the line.
Why: otherwise a crossfade between clips of different transparency would jump, and the fades on audio and video would
look different (the owner's requirement).

#### Scenario: The level of a pair and of a solo
- **WHEN** a clip with a level, a solo fade with a level and a pair with different levels are rendered
- **THEN** the layer is scaled, the solo fade is multiplied, the pair blends the levels by the weight of the progress; 1.0 is the identity
- **TEST** t174 (`src/libs/kt-engine/tests/transitions_video.rs`)

#### Scenario: One level line for both kinds
- **WHEN** the level line is pulled on audio and on video
- **THEN** the rail range is preserved, the level↔gain round trip is exact, the corner of the fade has priority on both kinds
- **TEST** t176 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Audio crossfade — windows from the transition, handles beyond the edges, equal-power byte-for-byte
The audio weights SHALL come from the transition event: the fade-in/out windows are precomputed when the renderer is
built, and the boundaries are floored into frames by the same conversion as the clip boundaries. Equal-power SHALL remain
the previous formula byte for byte; the other curves are computed by their own formulas and sound different. The handles
SHALL read the source beyond the edges of the piece; beyond the edges of the material there is silence without underrun;
prefetch SHALL warm the handles; for reversed clips with FX the handles are disabled (the mirror anchor of the source is
tied to the end of the window).
Why: the conversion of old overlaps must be byte-exact (GOLDEN_AUDIO was not regenerated).

#### Scenario: A crossfade equals the old formula and reads the handles
- **WHEN** a pair with a transition is rendered through the simple and the FX path, with a missing left handle and with a reversed incoming
- **THEN** the converted overlap is byte-for-byte equal to the previous mathematics, the missing handle is silence without underruns, the FX path matches the simple one, the reversed incoming mirrors the source, the prefetch order covers the handles
- **TEST** t136 (`src/libs/kt-engine/tests/transitions_audio.rs`)

#### Scenario: Audio curve reaches the samples
- **WHEN** the same junction is rendered with four curves
- **THEN** equal-power is byte-for-byte equal to the previous formula; linear/S/exponential give a different output
- **TEST** t226 (`src/libs/kt-engine/tests/transitions_audio.rs`)

### Requirement: A transition at a cut — a pair of neighbours, the position is derived, one reconcile per batch
A junction transition SHALL be addressed by a pair of clips on one track (junction = end of a == start of b),
the position MUST NOT be stored. At the end of geometric mutations a reconcile SHALL run exactly
once per batch: a clip is gone or a junction is broken — the transition dies; a trim shortened the body — offsets
are clamped; a cut retargets the outgoing side to the right half. Validation: the junction, fitting
within the bodies, the minimum, one transition per junction, windows of one track do not overlap. The end of a transition
SHALL be searched for by "the same clip" (taking into account the pieces of a morpher cut) plus a check of the boundary, and not by
equality of ids.
Why: identity by pair survives group moves for free; comparison by id
returned the wrong piece after a cut, and the transition silently disappeared.

#### Scenario: Commands, reconcile and validation
- **WHEN** a transition is added/changed/deleted, a neighbour is trimmed, deleted, cut
- **THEN** undo restores the window; bad transitions are rejected; the death of a neighbour takes the transition with it; a trim clamps the offsets; Split retargets the outgoing side
- **TEST** t135 (`src/libs/kt-model/tests/transitions.rs`)

#### Scenario: Transition survives a cut by a morpher layer
- **WHEN** the target of a transition is cut by a layer into pieces
- **THEN** the junction goes to the piece on which its boundary stands, and the video and audio transition sound/appear as they did before the cut (capability `morpher`)
- **TEST** t237 (`src/libs/kt-engine/tests/morph_slice_render.rs`)

#### Scenario: Import of legacy overlaps
- **WHEN** an old project with an audio overlap of two clips without FX is read
- **THEN** the pair is trimmed to the middle of the overlap and gets a transition in the same window, the sound is byte-identical; 3+/FX overlaps are spread across tracks without compensation; a video overlap demotes the late clip to an inserted track; with overlap layering enabled, the overlaps are imported as they are (capability `project-format`)
- **TEST** t143, t164 (`src/libs/kt-model/tests/legacy_upgrade.rs`)

### Requirement: Breaking a crossfade returns the handles in the same history step
On a break gesture (move, delete, punch-out) the return of the cut-off material
of the dead crossfades SHALL be appended under the same undo step: the transition offsets are numerically equal to the cut material, therefore
trims return exactly what was lost. The return is clamped by the free space for each member of the
link group and by `source_in` (otherwise the batch would fail as a whole); fades and pairs overlapped before the gesture
are skipped. Trim gestures are not instrumented.
Why: "in Vegas, if you drag apart overlapping events, what overlapped remains."

#### Scenario: Return of handles
- **WHEN** a pair with a crossfade is dragged apart, a neighbour is deleted, there is little room, or there is an obstacle in the way
- **THEN** trims return what was lost, are clamped by the space and the obstacles; a fade has nothing to return; the gesture never fails
- **TEST** t159 (`src/libs/kt-model/tests/transitions.rs`)

### Requirement: An overlap transition — the window is the overlap zone itself, the mode is not stored
A transition SHALL also work on a pair of one track in a "staircase" (`a.start < b.start < a.end < b.end`):
the mode is derived from the geometry of the pair, the window = `[b.start, a.end)`, the window has no handles of its own, it
follows the trim of the clips; the offsets lie as dead weight and come back to life when returned butt-to-butt. A third clip in the
window is a validation error. Reconcile SHALL canonicalize the roles (`a` is the early one). Audio: an explicit window
is marked exclusive, the built-in overlap weight in it equals 1.0, the curve is given by the transition; equal-power
on an overlap SHALL be byte-for-byte equal to the bare overlap. Video: in the overlap zone the transition's view plays instead of the cut. The transition windows in both audio and video SHALL attach to the transitions
of ITS OWN reel, not the root. In the file the pair travels in a list in the envelope of its reel (v7).
Why: "the symbiosis of two events" — a decision of the owner; a window without handles removes the lesson of Kdenlive
(a second subsystem of the window and years of defects).

#### Scenario: Model of an overlapping transition
- **WHEN** a transition is placed on a staircase, on a full nesting, with a third clip; the pair is overlapped by a move, flipped by a cut, dragged apart, copied
- **THEN** the staircase is legal, the nesting and the third clip are rejected; the junction transition survives as an overlapping one and the offsets come back to life on return; the roles are canonicalized; a break kills the transition; the return of the pair's handles gives no false trims; the clipboard carries the pair
- **TEST** t462 (`src/libs/kt-model/tests/transitions.rs`)

#### Scenario: Overlap without a transition — the gesture changes only the position
- **WHEN** a clip on the same track is dragged into an overlap onto a neighbour without a transition
- **THEN** only its position changes: the neighbour is not trimmed, the track is not inserted, the content and order are not touched; the layering is visible live in the window
- **TEST** t163 (`src/apps/kt-app/src/layout_tests.rs`), live `overlap.py` (`src/scripts/live/overlap.py`)

#### Scenario: The audio curve on an overlap and inside a reel
- **WHEN** a staircase is rendered without a transition, with a linear curve, with equal-power; a transition stands between lines inside a nested reel
- **THEN** without a transition — the √ law of the overlap; linear — the half-sum in the middle; equal-power is byte-for-byte equal to a bare overlap; a transition inside the reel is audible and equals the expanded content (capability `compositions`)
- **TEST** t463 (`src/libs/kt-engine/tests/transitions_audio.rs`), t463b (`src/libs/kt-engine/tests/nest_render.rs`)

#### Scenario: The picture on an overlap
- **WHEN** a staircase is rendered without a transition, with a wipe, with a crossfade
- **THEN** without a transition — the former cut (the late one on top); with a wipe — both sides; with a crossfade — a uniform mix
- **TEST** t464 (`src/libs/kt-engine/tests/transitions_video.rs`)

#### Scenario: The format of an overlapped transition
- **WHEN** a project with an overlapped transition is saved, read, re-saved, and the envelope has a broken record
- **THEN** the reel's envelope carries a list of pairs without ids and offsets, the file is v7, the round trip is idempotent, the broken record is a notice and is skipped
- **TEST** t465 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The overlapped transition is reachable by gestures
- **WHEN** RMB over the zone of a bare overlap and a double click over the zone of an overlapped transition
- **THEN** the menu offers ""Transition with…"" with the staircase neighbour on a par with the joint one; a double click opens the properties; the zone of a bare overlap is marked with hatching
- **TEST** t466 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Transition window — a grid of effect parameters, two equal inputs, the kind name in the window on the strip
The kind SHALL be chosen in two equal ways: a submenu ""Transition with next/previous""
in the context menu of an event, and the properties window (a double click on the transition window on the strip, or a menu item); both take the list from the registry. Parameters SHALL be drawn by the common parameter grid;
the non-numeric (direction, shape, colour) — by their own controls; ""Cancel"" rolls the session back in one
undo step; the window of a dead transition closes by itself. The name of the kind is written into the transition window on the strip (if it does not fit — a marker dot); the crossfade remains «X». On an audio track the kind picker is not shown.
The delete item MUST name what will happen (""Remove transition…"", not ""crossfade"" on a non-crossfade).
Why: a transition should have no form of its own — exactly the mismatch of fields the owner complained about.

#### Scenario: The transition window
- **WHEN** a double click on the transition window on the strip, a choice of kind and curve, ""Cancel"", deletion of the transition
- **THEN** the window opens, the kind and the curve reach the model, ""Cancel"" returns the original kind, the whole session is one history step, the window of a dead transition closes
- **TEST** t227 (`src/apps/kt-app/src/layout_tests.rs`)
