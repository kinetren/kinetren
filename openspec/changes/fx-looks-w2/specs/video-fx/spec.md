# Spec Delta

## ADDED Requirements

### Requirement: The looks of wave W2 are kinds only where no look of ours exists
The plugins `glitch_blocks`, `film_damage`, `lens_blur` and `light_leak` SHALL be built-in kinds; halation, the
directional blur, sepia, movie grain and the cinematic grade SHALL be variants or rows of existing kinds. Every new kind
at its neutral rows SHALL give the input byte for byte, with no pass, and SHALL be a pure function of the burst index,
its rows and the seed. Each is an imitation of a look; the picker and the note SHALL NOT offer it as a simulation.
Why: the owner's K7; the video-fx rejection of sepia and invert as kinds stands.

#### Scenario: A calm frame costs nothing
- **WHEN** `glitch_blocks` runs at amount 0, at amount 1 on a frame whose hash says "no", and twice on one that says "yes", with seed 7 and 8
- **THEN** the first two are the input with no pass; the last pair is byte-identical per seed and differs between seeds; a `hold` of 5 keeps one look for five frames
- **TEST** t4268 (`src/libs/kt-gpu/tests/glitch_blocks.rs`)

#### Scenario: The film path has no state
- **WHEN** `film_damage` renders frame 100 after frame 99 and cold, at 1× and 2× and box-reduced, and at all zeros
- **THEN** both renders are equal, the 2× is within `FrameTol` of the 1×, zeros give the input
- **TEST** t4269 (`src/libs/kt-gpu/tests/film_damage.rs`)

#### Scenario: Light leaks move by the burst, not by the order
- **WHEN** `light_leak` is rendered at two bursts in either order
- **THEN** each burst's frame is the same in both orders and differs from the other's
- **TEST** t4267 (`src/libs/kt-gpu/tests/light_leak.rs`)

#### Scenario: A lens blur keeps a highlight round
- **WHEN** a white point on black passes `lens_blur` of radius 12 with highlights 3, once as disc and once as hexagon
- **THEN** the energy is within 2 % of the point's for `highlights` 0, the two shapes differ, radius 0 gives the input
- **TEST** t4270 (`src/libs/kt-gpu/tests/lens_blur.rs`)

#### Scenario: The directional mode does not move the former ones
- **WHEN** `radial_blur` runs in its two former modes and then with the directional row on at 0° and 90°
- **THEN** the former bytes stand, a file without the row reads as before, the 0° streak lies along the row and the 90° along the column
- **TEST** t4271 (`src/plugins/fx-video-blur/tests/radial_mode.rs`)

#### Scenario: Four looks are variants
- **WHEN** the gallery of glow, duotone, noise and color_grade is opened
- **THEN** Halation, Sepia, Movie grain and Cinematic stand in it, each distinct from its neighbours and within the passport
- **TEST** t4272 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A look reaches the event in one click
- **WHEN** the person opens the picker on a video event, types "light leak" and clicks the cell
- **THEN** the chain gains the kind with its sliders, one undo step removes it
- **PATH** click "+ Effect" on a video event → the gallery shows; type "light leak" → the cell stands first; click it → a node "Light leak" is in the graph and the preview shows blobs of light; Ctrl+Z → it is gone
- **TEST** t4273 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: The looks are idle on a pause
- **WHEN** a paused project shows an event with the four kinds for five seconds, the mouse still
- **THEN** no frame is asked and no pass runs
- **TEST** t4274 (`src/apps/kt-app/src/idle_tests.rs`)
