## ADDED Requirements

### Requirement: A retired generator kind is refused by the reader and repaired by the import
The program SHALL keep one table of retired generator kinds with the release each went in. The project reader SHALL
refuse a file that holds one anywhere (a medium, an event of the root or of a reel, a morpher's patch), naming the
file's format, the kind, the release and the import menu item; `kt-cli` commands that read projects SHALL refuse it the
same way. The import door SHALL accept such a `.kt` and such an `.otio`, SHALL turn each `timecode` event into a title
with the timecode source, keeping its frame rate, its curve, formula, links, published parameters and overrides, its
medium's name and place, and SHALL say how many it turned. A recovery text SHALL go through the same repair when it is
restored. The source file SHALL NOT change, and the suggested name of a document imported from a `.kt` SHALL NOT be the
source's own.
Why: R.67 — "fix it in the import… the reader does not learn old forms"; without the table a removed kind would be read
as a missing plugin and the person's timecode would become a placeholder without a word.

#### Scenario: An old timecode comes in as a title
- **WHEN** a project with a timecode at 25 fps, one with a curve on its rate and a link to it, one published from a reel with an override, and one in a morpher's patch is imported, then written and read
- **THEN** four titles with the timecode source stand at the same places with the same rates; the curve, the link, the published parameter and the override point at the title's rate; at 1.5 s from 25 fps the text is 00:00:01:12; the notice names the count; the written file is read by the ordinary reader
- **TEST** t3993 (`src/libs/kt-otio/tests/convert.rs`)

#### Scenario: The reader and the command line refuse, the source is untouched
- **WHEN** the same `.kt` is opened by the reader and rendered by `kt-cli`, imported by `kt-cli import`, and a `.kt` without retired kinds is given to the import
- **THEN** the first two refuse naming the format, `timecode` and the import; `kt-cli import` writes a project; the source's bytes are unchanged; the last one says "already a project"
- **TEST** t3994 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: A recovery of an older build is restored
- **WHEN** a recovery text holding a timecode event is restored
- **THEN** the editor shows the title, the notice names the repair, the recovery file stays
- **TEST** t3996 (`src/apps/kt-app/src/layout_tests.rs`)
