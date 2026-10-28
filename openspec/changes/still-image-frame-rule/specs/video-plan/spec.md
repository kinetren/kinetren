# Spec Delta

## ADDED Requirements

### Requirement: The source's proportions are one fact; a single picture is conformed like a video
Whether a source has proportions of its own SHALL be decided in exactly one place, from the
facts of the opened source. This decision SHALL be separate from whether the source has a frame grid of its own.
A single picture (JPEG, PNG, any file the system FFmpeg opens as an image) has no
frame grid (it is timed like a procedural source, as R.67 decided). It DOES have proportions:
its native size × pixel aspect. The plan SHALL conform it by the event's frame rule
(fit / fill / stretch / as is, the project default or the event's own) exactly as it conforms
a video of the same native size and pixel aspect: the same placement rectangle, the same
decode size, the same Geometry node. Only a source that draws its frame at the requested quality
(a generator, a title, the missing-file placeholder) SHALL have no proportions: its frame
is the canvas whatever the rule says. The composite, the frame drawn over the picture in the Geometry
window and the `kt-cli probe` conform line SHALL read this one fact. None of them may
decide it again on its own.
Why: ktprompts v26.10.18 item 9: "a square still on a 16:9 timeline gets squashed, and in
its Geometry not one frame-rule option works … why does it work for video and not for
stills?" R.62 equated "procedural" with "no proportions". R.67 then gave a still `fps: None`,
so the still became "procedural", and the renderer dropped its view. Every rule turned into
stretch. The overlay and the probe kept the view and showed a Fit frame that the render did
not use (a second arithmetic, forbidden since R.64).

#### Scenario: Four rules — a square still equals a square video
- **WHEN** a 180×180 still (`still_square.png`) and a 180×180 one-second video of the same lavfi pattern (`square.mp4`) are each placed on a 320×180 canvas, and the event rule is set in turn to fit, fill, stretch and as is (and once "project" with the project default changed)
- **THEN** for every rule the still's placement rectangle and decode size equal the video's, and equal the analytical ones: fit — 180×180 centred with transparent side margins; fill — 320 wide, cropped top and bottom; stretch — the whole canvas; as is — 180×180 centred 1:1. The composed CPU frames of the still and the video give the same white-rectangle geometry (a pixel probe of the edges, ±1 px for the codec)
- **TEST** t937 (`src/libs/kt-engine/tests/stills.rs`)

#### Scenario: One decision — a still has a view, generators and the placeholder do not
- **WHEN** the source facts observe a single picture (square, and once with SAR 2:1), a video, a generator, a title and the missing-file placeholder
- **THEN** the picture is timed as procedural (no floor to the frame grid, no end of material: t796b unchanged) AND has its view (width × SAR, height); the video has its view and is not procedural; the generator, the title and the placeholder are procedural and have no view. The renderer's lead context, `Player::source_view` and the probe's view are the same value for each of them
- **TEST** t936 (`src/libs/kt-engine/tests/conform_rule.rs`)

#### Scenario: Both backends place the still by the rule
- **WHEN** the square still is composed with the fit rule over a coloured lower layer on the CPU and the GPU (the lead node executed by `src/libs/kt-fx/src/geom.rs` and `src/libs/kt-gpu/src/shaders/geom_fused.wgsl`)
- **THEN** the side margins are transparent (the lower layer shows through) on both; the GPU frame is within the tolerance class of t713 against the CPU frame; with the stretch rule both equal the frame the still gave before this change, byte-for-byte on the CPU
- **TEST** t938 (`src/libs/kt-gpu/tests/gpu_render.rs`, `src/libs/kt-fx/src/geom.rs`, `src/libs/kt-gpu/src/shaders/geom_fused.wgsl`)

#### Scenario: The frame over the picture is the frame of the render
- **WHEN** a project with a 320×180 canvas and an event on the square still is opened in the harness, the event is selected, its Geometry form is opened and the frame rule is switched through fit, fill, stretch and as is
- **THEN** after each switch the placement rectangle that the on-screen frame draws equals the rectangle by which the composite placed the layer (checked against the preview frame's pixels), and the preview changes: pillarbox margins for fit, cropping for fill, the whole canvas for stretch, a centred 1:1 square for as is
- **PATH** drag `still_square.png` onto the timeline → the event appears, the preview shows the square picture with transparent side margins (project rule fit) → open the FX window of the event → the Geometry node is first, its rule row shows five words (project / fit / fill / stretch / as is) → click **fill** → the picture fills the width and its top and bottom are cut → **stretch** → the picture is squashed to 16:9 → **as is** → a 180×180 square in the centre → right-click the preview, turn the on-screen controls on → in every step the frame drawn over the preview matches the picture's edges
- **TEST** t939 (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/stills.py`

#### Scenario: The probe tells the same truth
- **WHEN** `kt-cli probe` is run over a project with the square still under fit and the placeholder of a missing picture
- **THEN** the conform line of the still prints `view=180x180` and the fit frame the renderer uses; the placeholder prints `unknown/stretch`; both lines come from the same function the renderer calls
- **TEST** t940 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: Procedural sources and the goldens do not move
- **WHEN** the generator, title and placeholder scenarios and the golden exports are run after the change, and a missing picture is shown in a 4:3 project
- **THEN** generators and titles are still conformed as an identity (t471 green), the placeholder still fills the whole canvas in 4:3 (no margins), t796, t796b, t796d, t715 and `GOLDEN_VIDEO` are green with no regeneration
- **TEST** t941 (`src/libs/kt-engine/tests/stills.rs`)
