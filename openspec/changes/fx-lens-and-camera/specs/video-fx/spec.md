# Spec Delta

## ADDED Requirements

### Requirement: The lens housing is measured in its own stage
The built-in plugin `lens_housing` SHALL darken the frame outside a rounded housing whose radius,
softness and corner roundness are fractions of ITS OWN stage's short semi-axis (the `vignette`
rule), so a `geometry` zoom placed before it does not move the housing out of the corners, and one
placed after it scales the housing with the picture. At amount 0 the output SHALL equal the input
byte for byte. It is a point-wise plugin with a positioned tap (the coordinate of its stage, a
contract of `plugin-api-stage1`), and it falls under the golden cycle (t328) and the CPU↔GPU
conformance (t329) without a copy of those checks.
Why: the owner's FX item 1 — "the lens corners" of a fisheye; a housing in frame pixels would slide
off the corners the moment the shot is zoomed, which is exactly the vignette lesson.

#### Scenario: The housing stays in the corners of its stage
- **WHEN** a frame passes `geometry` (zoom 1.5) → `lens_housing` and, separately, `lens_housing` →
  `geometry` (zoom 1.5), with the housing's defaults, and once with amount 0
- **THEN** in the first chain the four corner pixels of the output are dark and the centre is the
  zoomed input; in the second the dark region is the first chain's housing scaled by 1.5 about the
  centre; amount 0 gives the input bytes
- **TEST** t992 (`src/plugins/fx-video-color/tests/lens_housing.rs`)

### Requirement: Lens dirt shows only where the frame has highlights
The built-in plugin `lens_dirt` SHALL take the frame's highlights with the threshold of `glow` (a
blurred-luma soft knee), multiply them by a procedural dirt pattern (a counter-based hash of the
stage grid cell and a seed, pinned to the stage like `noise`'s grain) and add the product back. A
frame whose blurred luma stays below the threshold SHALL come out byte-identical; the pattern SHALL
be resolution-invariant under the K× barrier rule. It is a barrier plugin with a GPU pass (a
contract of `plugin-api-stage1`); no media file ships with it.
Why: real dirt scatters existing light and adds none (Unity's Dirtiness on the bloom, F-FX-008
trick 1); an overlay plate shows dirt on a black frame and stays put when the shot pans.

#### Scenario: A dark frame shows no dirt, a highlight does
- **WHEN** `lens_dirt` runs on a frame of luma 40 with threshold 128, then on the same frame with one
  white disc, then on the disc frame rendered at 2× and box-reduced
- **THEN** the dark frame is unchanged byte for byte; with the disc, pixels change only within the
  blur radius of the disc and the change is the dirt pattern of that seed (another seed gives another
  pattern); the 2× render reduced to 1× is within `FrameTol` of the 1× render
- **TEST** t993 (`src/plugins/fx-video-stylize/tests/lens_dirt.rs`)

### Requirement: Lens drops are a Warp mode, edited as a pair
`lens_drops` SHALL be a geometry plugin whose map is a new `drops` mode of the Warp primitive:
drop cells on the normalised stage grid, each drop's centre and radius a counter-based hash of the
cell and the seed, inside a drop the source coordinate displaced toward (or, with negative
refraction, away from) its centre. The CPU map in `src/libs/kt-fx/src/geom.rs` and its WGSL mirror in
`src/libs/kt-gpu/src/shaders/geom_fused.wgsl` SHALL change in one commit; the mode SHALL fuse with a
neighbouring `lens_distort` in one pass; refraction 0 SHALL give the input bytes of a lone step.
Why: a drop is a displacement of the source coordinate (F-FX-008 trick 2), i.e. an inverse map — the
geometry family's own form, without a new pass; the fused map stays closed to plugins
(`plugin-api-stage1` D5), so the mode is the one core edit of the wave.

#### Scenario: Drops are deterministic, fused and mirrored
- **WHEN** `lens_distort` → `lens_drops` (seed 7) is rendered on the CPU twice and on the GPU, then
  with seed 8, then with refraction 0
- **THEN** the two CPU renders are byte-identical; the GPU render is within `FrameTol::WARP` of the
  CPU (the pair `geom.rs` ↔ `geom_fused.wgsl`); the plan holds ONE geo run for both steps; seed 8
  moves the drops; refraction 0 equals the `lens_distort`-only render byte for byte
- **TEST** t994 (`src/libs/kt-fx/tests/lens_drops.rs`)

### Requirement: The lens flare is placed, not detected
The built-in plugin `lens_flare` SHALL draw a core glow and ghosts along the line from its `x`/`y`
position (fractions of its stage, animatable, allowed outside the frame) through the stage centre;
the frame content SHALL NOT move the flare. At brightness 0 the output SHALL equal the input byte
for byte.
Why: Premiere's Lens Flare is placed at a centre the editor keys onto the light (F-FX-008 trick 3);
a detector would make the flare jump between frames and break "a pure function of the parameters".

#### Scenario: The flare follows its parameters only
- **WHEN** `lens_flare` at position (0.8, 0.2) runs on two different frames, then with a curve moving
  the position from (0.8, 0.2) to (1.2, 0.2), then at brightness 0
- **THEN** the difference output − input is the same on both frames (up to the clamp at 255); on the
  curve the ghosts move along the line through the centre and stay drawn when the position leaves the
  frame; brightness 0 gives the input bytes
- **TEST** t995 (`src/plugins/fx-video-stylize/tests/lens_flare.rs`)

### Requirement: Camera looks are chain presets, not kinds
"Fisheye lens" (`lens_distort` + `lens_housing`), "Zoom Shake" and "Handheld" (a `geometry` curve with
zigzag segments, R.61) SHALL be factory chain presets in
`src/apps/kt-app/assets/fx-presets-factory.json`; no kind SHALL be added for shake or zoom punch, and
the existing "Punch in" SHALL stay unchanged. Every preset SHALL parse, name only registered kinds
and existing parameters, and apply as one undo step.
Why: CapCut ships shake and zoom as templates over a transform curve (F-FX-008); the zigzag segment
shape already expresses jitter around a constant, so a kernel would duplicate the curve system.

#### Scenario: The presets parse and apply from the picker
- **WHEN** the factory file is loaded, and the person opens the FX picker on a video event, types
  "fisheye" and clicks the "Fisheye lens" preset
- **THEN** all three new presets parse and name registered kinds and existing parameters, the
  "Handheld" curve's keys carry a zigzag shape, "Punch in" is byte-identical to its former record; the
  event's chain gains `lens_distort` and `lens_housing` in one undo step
- **PATH** open the FX picker on a video event → the gallery shows; type "fisheye" → the "Fisheye
  lens" preset row is listed under the presets; click it → the chain strip shows "Lens distort" and
  "Lens housing", the preview shows the housing in the corners; Ctrl+Z → both slots are gone
- **TEST** t996 (`src/apps/kt-app/src/user_store.rs`), t997 (`src/apps/kt-app/src/reach_tests.rs`)
