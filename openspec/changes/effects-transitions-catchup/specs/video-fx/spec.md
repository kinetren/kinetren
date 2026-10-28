# Spec Delta

## ADDED Requirements

### Requirement: The effect catalogue is measured against the market and grows in ranked waves
The video effect catalogue SHALL have one reconnaissance note (F-FX-007,
`docs/features/fx/catalogue/market-catalogue-and-waves.md`) that lists EVERY built-in kind by its
id exactly as the registry has it (video effects, audio effects, the layer marker, transitions,
generators), groups what other editors offer by what people use, and gives each group a verdict
from a closed vocabulary: `have`, `wave Wn`, `deferred (<dependency>)`, `rejected (<reason>)`. A
row whose market side could not be checked live SHALL carry `[NOT VERIFIED]`. New video kinds
SHALL arrive only through a wave of that list: each wave is its own change, built as plugin
family crates under `src/plugins/` (capability `plugins`), and it names the note ids it depends
on. A wave that brings back a kind or a mode listed under "Rejected and why" of the video-fx,
transitions or audio-fx design SHALL carry the line `Revision: <decision> — <reason>`. Every FX
item of the owner's claims files (ktprompts) SHALL have a row with a verdict.
Why: the owner — "we are slipping badly on effects and transitions, far behind CapCut and the
like" (2026-10-27). A catalogue that is not compared with the code drifts the day the next kind is
added; a wish list without a rank is how the "67 niche plugins" become 67 half-done kernels; and
two of the obvious names (a directional blur, sepia/invert kinds) were rejected earlier with
reasons that must be answered, not forgotten.

#### Scenario: The note lists exactly the built-in kinds
- **WHEN** the guard reads the five "Our catalogue" lists of F-FX-007 and the registries
  (`FxInstance::all()` split by class, the layer marker, `TransitionKind::ALL`, the generator
  tags — after `plugin-api-stage1`, the assembler's closed list plus the core-owned kinds); then a
  temporary copy of the note with one id removed and one id invented is checked
- **THEN** each list equals its registry as a set and the counts in the list headings (18, 1, 27,
  33, 15 today) equal the registry sizes; both broken copies are red and the message names the
  missing and the extra id
- **TEST** t978 (`src/apps/kt-cli/tests/fx_catalogue.rs`)

#### Scenario: The wave list names only existing notes and capabilities
- **WHEN** the guard reads the Verdict of F-FX-007 and the waves of the change design, and
  collects every `F-XXX-NNN` id and every `openspec/specs/<cap>/` path they name; then a copy with
  an invented id `F-FX-099` is checked
- **THEN** every id is the `id:` of an existing note under `docs/features/` and every capability
  directory exists with a `spec.md`; the copy is red and names `F-FX-099`
- **TEST** t979 (`src/apps/kt-cli/tests/fx_catalogue.rs`)

#### Scenario: Every catalogue row has a verdict from the closed vocabulary
- **WHEN** the guard reads the "What people use, against what we have" table of F-FX-007
- **THEN** every row's last cell starts with `have`, `wave W<n>`, `deferred (` or `rejected (`;
  every `wave W<n>` names a wave that exists in the Verdict list; a `rejected (` or `deferred (`
  cell carries a non-empty reason or dependency; a row with no live check carries `[NOT VERIFIED]`
  in its market cell
- **TEST** t980 (`src/apps/kt-cli/tests/fx_catalogue.rs`)

#### Scenario: Reviving a rejected idea is an explicit revision
- **WHEN** the guard reads the "Rejected and why" sections of `openspec/specs/{video-fx,transitions,audio-fx}/design.md`,
  takes the closed list of rejected names kept in the test (directional blur, sepia/invert as
  kinds, a node graph, a transition progress curve, stylised audio transitions, a gallery of live
  transition previews), and scans the waves of F-FX-007 and of the change design for them
- **THEN** every wave that names one of them carries a line `Revision: … — …` in the change
  design; a copy where W2's directional blur loses its revision line is red
- **TEST** t981 (`src/apps/kt-cli/tests/fx_catalogue.rs`)

#### Scenario: The owner's FX items all have a row
- **WHEN** the guard takes the closed list of FX items of `ktprompts_v26.10` recorded in the test
  (fisheye lens corners; dirt/drops/flares on the lens; letters as physical objects; inertia of
  letters under rate; the "67 niche plugins" reference; jag edit by time remap; parallax; text
  blending and shapes holding sources; homothety; bump mapping; buffer + blend levels; audio-driven
  zoom; blur on one word of a ticker; fisheye switched on by speed changes)
- **THEN** each item is found in the note's table or its "owner's items" list with a verdict; an
  item removed from the note makes the guard red with the item's name
- **TEST** t983 (`src/apps/kt-cli/tests/fx_catalogue.rs`)
