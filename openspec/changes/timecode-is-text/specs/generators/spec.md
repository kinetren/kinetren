## MODIFIED Requirements

### Requirement: Other kinds — determinism, honest alpha, sound from one formula
Noise SHALL be counter-based: the grain is a function of a cell of the base grid 1920×1080 and the
pts of the material; the shift of the clip, the step and the Quality do not change the picture. The SMPTE tone and the
countdown beep SHALL be computed by one code with the phase from the global sample number
(a seek does not click); switching the tone on makes `has_audio` true. The countdown
SHALL show the time of the MATERIAL (a title's running value too — "A title runs a timecode, a stopwatch or a countdown"). A kind SHALL be a variant of an enum when it
has its own numbers (Rect/Ellipse/Line — three kinds; tables — one kind with a field
of variety). The alpha of a kind SHALL be declared by an exhaustive method, not by
`matches!`; the rasterization of shapes is a distance field in a compute shader, float on the CPU kinds only in the
preamble of the frame. A title with alpha under the FX chain SHALL remain transparent outside the
glyphs.
Why: a new transparent kind slipping past `matches!` would land in the composite as a black plate; a second
sine would drift out of phase.

#### Scenario: Noise, SMPTE, timecode
- **WHEN** the noise is read with a shifted `source_in` and in another Quality; the SMPTE tone is enabled; a title with the timecode source is read at a pts
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

### Requirement: Generator kinds are plugins; the pseudo-path names the plugin id
Each generator kind SHALL be a generator plugin of the closed built-in set (capability
`plugins`) whose id equals today's kind tag (`solid`, `text`, `gradient`, `corners`, `checker`,
`noise`, `smpte`, `transparent`, `rect`, `ellipse`, `line`, `leader`, `table`,
`window`; and the sound kinds `tone`, `audio_noise`, `sweep`, `beep`, `silence` of
`generators-placement-and-audio`), so `gen:<kind>` paths and the event's values keep their text. The generator menu SHALL
list the registry's generator rows (a plugin may declare several rows, as the test tables do
today) and MUST NOT enumerate kinds of its own; the row "Timecode" SHALL create a title whose source is the timecode,
at the project's frame rate. A kind this program removed is RETIRED, not missing: it is refused by the reader and
repaired by the import door (capability `project-format`), never shown as a placeholder. A generator plugin's frame SHALL be a pure
function of (the event's values with curves resolved at the source's tick, the tick, the
requested quality), and its in-place retune SHALL keep the existing strict contract: applied with
`info()` unchanged, or "recreate". A path naming an id this build does not register SHALL open as
a placeholder source that names the id — not as a broken path, and not through FFmpeg — and its
event's values SHALL be kept and written back unchanged.
Why: generators were the owner's first example of what should be developed in parallel
(ktprompts P3: "move generators and effects into plugins"); the frame-as-pure-function rule is
what makes the generator's cache bar honest (invariant 4), so it moves with the kind into the
manifest instead of being re-promised by each crate.

#### Scenario: A moved generator keeps its path, values and frame
- **WHEN** the generator families have moved into plugin crates (stage 5); a project with every
  menu row, with curves on two of them, is written by the frozen pre-move serializer and by the
  new build, and the generator frames at three ticks and two qualities are hashed (the retired
  `timecode` left both lists, kkt 2026-10-30 question 5)
- **THEN** the texts and the `gen:` paths are byte-identical, the frame hashes equal the pre-move
  references (the title's own golden too), the menu offers the same 19 rows in the same order, followed by the five sound rows,
  and a retune of a colour is applied in place
- **TEST** t973 (`src/libs/kt-model/tests/plugin_format.rs`), t560 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A generator whose plugin is missing is a named placeholder
- **WHEN** a project holds media `gen:wobble_grid` with an event carrying values for it, and no
  plugin `wobble_grid` is registered; the project is opened, played and saved
- **THEN** the factory raises a placeholder source labelled with the id, FFmpeg and the sidecar DB
  are not touched, one notice names the id, and the saved file keeps the path and the values
- **TEST** t973b (`src/libs/kt-engine/tests/plugin_missing.rs`)

## ADDED Requirements

### Requirement: The timecode is a title and nothing else
The program SHALL have one way to put a running timecode into the picture: a title with the timecode as its source. The
generator kind `timecode` SHALL NOT be registered, listed or created.
Why: the owner, v26.10 line 14 — "тот же таймкод является текстом, но я не могу управлять им так же гибко как текстом";
question 5 — "Timecode = a mode of the text".

#### Scenario: The menu's Timecode is a title
- **WHEN** Generators → Titles → "Timecode" is chosen in a 25 fps project, and the registry's ids are listed
- **THEN** the created event is a Text with the timecode source at 25 fps, its window has the font, the colours, the outline and the curves; no plugin `timecode` is registered; the plain Text row stands before it; the row reads "Timecode" in the five catalogs
- **PATH** Edit → Create a generator → Titles → "Timecode" → an event with running digits lands → double-click it → the title's window: Text: Timecode, font, fill, stroke
- **TEST** t3995 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/text_counter.py` (`./scripts/live-check.sh`)
