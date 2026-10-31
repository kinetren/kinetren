# Spec Delta

## ADDED Requirements

### Requirement: Generator kinds are plugins; the pseudo-path names the plugin id
Each generator kind SHALL be a generator plugin of the closed built-in set (capability
`plugins`) whose id equals today's kind tag (`solid`, `text`, `gradient`, `corners`, `checker`,
`noise`, `smpte`, `timecode`, `transparent`, `rect`, `ellipse`, `line`, `leader`, `table`,
`window`), so `gen:<kind>` paths and the event's values keep their text. The generator menu SHALL
list the registry's generator rows (a plugin may declare several rows, as the test tables do
today) and MUST NOT enumerate kinds of its own. A generator plugin's frame SHALL be a pure
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
  new build, and the generator frames at three ticks and two qualities are hashed
- **THEN** the texts and the `gen:` paths are byte-identical, the frame hashes equal the pre-move
  references (the title's own golden too), the menu offers the same 19 rows in the same order,
  and a retune of a colour is applied in place
- **TEST** t973 (`src/libs/kt-model/tests/plugin_format.rs`), t560 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: A generator whose plugin is missing is a named placeholder
- **WHEN** a project holds media `gen:wobble_grid` with an event carrying values for it, and no
  plugin `wobble_grid` is registered; the project is opened, played and saved
- **THEN** the factory raises a placeholder source labelled with the id, FFmpeg and the sidecar DB
  are not touched, one notice names the id, and the saved file keeps the path and the values
- **TEST** t973b (`src/libs/kt-media/src/factory.rs`)
