# showcase

## Purpose

The public face of the repository: the English README and user guide, the screenshots they
show, and the reproducible pipeline that re-takes those screenshots from real footage.

## Requirements

### Requirement: README and user guide are English and self-consistent
`README.md`, `CONTRIBUTING.md` and every file under `docs/guide/` SHALL be written in
English and MUST NOT contain Cyrillic characters (proper names of Russian-language
documents are linked by path, not quoted). Every relative link in them SHALL resolve to an
existing file or heading anchor in the repository; every image they reference SHALL exist
under `docs/images/` (the only exception is the application icon from `src/packaging/`); every file under `docs/images/` SHALL be referenced at least once
(no orphaned screenshots). Each image SHALL stay within 900 KB and all of `docs/images/`
within 8 MB. The guard `src/scripts/readme-check.py --check` SHALL enforce all of the
above, report every violation with file and line, exit non-zero on any, and SHALL be run by
`cargo test` and by `finish.sh`.
Why: the repository is published on GitHub by the owner; a broken image or a stray Russian
paragraph on the front page is visible to every visitor, and screenshots that nothing
references accumulate silently in git history.

#### Scenario: The committed README and guide pass the guard
- **WHEN** `readme-check.py --check` runs on the repository
- **THEN** exit 0
- **TEST** t826 (`src/apps/kt-cli/tests/readme_guard.rs`)

#### Scenario: The guard catches each class of defect
- **WHEN** it runs on a temporary tree with, one at a time: a Cyrillic word in the README, a link to a missing file, a missing image, an unreferenced image, an image over budget
- **THEN** each case exits non-zero and names the file, the line (where applicable) and the defect class; the clean tree exits 0
- **TEST** t827 (`src/apps/kt-cli/tests/readme_guard.rs`)

### Requirement: README shows the real editor on free footage, including nested reels and automation
The README SHALL be about the product: what it does, screenshots, how to install and run
it, where the documentation is, and the license. It MUST NOT contain architecture,
crate lists, engine invariants, porting history or footage credits (owner, 2026-10-22:
""the README is about the product, how to run it, not about technical junk""; that material
lives in `CONTRIBUTING.md` and the design documents. The README SHALL open with the
application icon, the name and badges, then a full-window screenshot of the editor on the
showcase project DURING PLAYBACK: the media bin wide, in free layout with its groups, and
the level meters showing live levels; no reel is expanded in it. It SHALL contain a
feature tour with at least these screenshots: the timeline with a reel instance from the
MIDDLE of the edit expanded in place and centred, its veiled tails visible on both sides;
the published parameters of a reel instance with its own values; Ctrl+F search; the
Slowed+Reverb editor with its `rate`/`mix` automation curves visible; the effect gallery
scrolled to its stylize, camera and tape cells; transitions; the morpher as a before/after of
the same frame (source swap and a simulated look) plus its editor; a title/generator; the
render dialog and the diagnostics panel (owner, 2026-10-22, fourth round); the welcome window
with a recent project; Datamosh and Camera ISP as before/after pairs, the VHS Tape deck face
beside its frame, and a mask on the preview (owner, 2026-11-03). The looks the showcase project
puts on its events SHALL be the striking ones a reader notices at a glance, taken from the
gallery's own cells (a plugin's template plus a named variant), and SHALL include the
simulations; plain blur, noise or gain SHALL NOT be what a montage cut shows (owner,
2026-11-03: "the effects used are dullish, the new simulation effects are not shown"). All screenshots SHALL be
taken from the English UI (`KT_LANG=en`) on free footage (CC BY or public domain) and
SHALL be produced by `src/scripts/readme-shots.py`, never edited by hand; the
attribution of that footage SHALL be in `docs/images/CREDITS.md` and in `CREDITS.txt`
next to the downloaded files.
Why: owner's requests — "fill the timeline with all kinds of editing", "show the
slowed-reverb automation curves", "nested reels belong on the timeline screenshot",
"no old samples", "credits go with the footage", "the effects used are dullish".

#### Scenario: Every README screenshot is produced by the pipeline
- **WHEN** `readme-check.py --check` reads the list of outputs declared by `readme-shots.py` and the images referenced by README/`docs/guide`
- **THEN** the two sets are equal (an image added by hand, or a declared shot missing from the docs, fails the guard)
- **TEST** t827 (`src/apps/kt-cli/tests/readme_guard.rs`)

#### Scenario: The overview, timeline and morpher shots show what they claim
- **WHEN** the pipeline takes the overview on the showcase project
- **THEN** the hero is shot during playback after widening the bin, once the playhead is well inside the mid-edit reel instance (the project's cursor sits a moment before it, inside the looped region); for the timeline shot the mid-edit reel instance (the one with overrides) is selected, Enter expands it — more timeline rows than before, all fitting the panel — and the view is zoomed and scrolled until the instance is centred and at least 300 px wide; the morpher before/after is the same frame from the project and from a copy with the morpher disabled, taken in the middle of the morpher's longest plain target whose source is not the one it swaps in; the diagnostics are shot with the loop off, playing the edit's lightest stretch
- **TEST** live `src/scripts/live/readme_shots.py`

#### Scenario: The effect shots show the effect, not its absence
- **WHEN** the pipeline takes, each on a main-track event with no look of its own, `fx-datamosh` (one right after a cut between two sources, a few frames after the cut, gallery cell Cut mosh), `fx-camera-isp` (one alone on screen, the night clip when the footage has one, gallery cell Phone at night), `fx-vhs-tape` (VHS Tape from the gallery on a bright cut — Coffee Run when the footage has it — head A of the drawn drum clicked and set Chipped) and `masks` (Duotone from the gallery, its Masks group → Pen, a five-point path with two smooth points closed on the first point, the feather pulled)
- **THEN** each before/after image is two 900 px halves of the same moment 12 px apart with no text, the "after" half taken only once the preview differs from the "before" and with the selection cleared (no transform frame over the picture); the VHS image is the effects window with the deck face beside the frame it gives; the masks image is the preview with the closed path, its points and its feather edge
- **TEST** live `src/scripts/live/readme_shots.py`

#### Scenario: A shot that waits for the owner's footage is honest about it
- **WHEN** a shot is in `SHOTS` and in `AWAITING_FOOTAGE` of the pipeline (its picture needs the showcase footage, which the owner fetches himself — 2026-10-21) and its image is absent; the docs show it while it is absent; its image is written but the docs do not show it; or `AWAITING_FOOTAGE` names a shot `SHOTS` lacks
- **THEN** the first passes the guard; the second fails as a broken `image`; the third fails as a declared shot the docs do not show; the fourth fails naming the unknown waiting shot. No placeholder image is ever committed under a waiting name
- **TEST** t1390 (`src/apps/kt-cli/tests/readme_guard.rs`)

#### Scenario: The showcase dresses its events in striking looks
- **WHEN** `kt-cli showcase` builds the project from the fixture videos
- **THEN** the styled montage cuts carry at least six effect kinds and none of them is a blur, noise or gain; a simulation (VHS Tape, Camera ISP or Datamosh) is on a video event and in the morpher's look; the VHS look-alike is on no event
- **TEST** t1412 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: The gallery and welcome shots show what they claim
- **WHEN** the pipeline takes `video-fx` and `welcome`
- **THEN** the gallery is wheeled until the last VHS Tape cell is on screen and its cells have rendered; the welcome window is opened over the showcase, a template closes the showcase, its poster is written to the store, and the window opened again from Help → Welcome window… lists it as the first recent card
- **TEST** live `src/scripts/live/readme_shots.py`

### Requirement: Screenshots are re-taken by one command, without pixel coordinates
`src/scripts/readme-shots.py --sources <dir>` SHALL build the showcase project with
`kt-cli showcase`, launch the freshly built `kt-app` in a virtual screen with the English
UI, reach every view through named probe rectangles and real gestures (no hard-coded window
coordinates), wait until the preview shows a decoded frame and media thumbnails are loaded
before each capture, and write each declared image to `docs/images/` (or to `--out <dir>`). Each step SHALL verify
its own precondition (the expected window or panel is open) and the run SHALL fail, naming
the step, instead of writing a wrong picture. The real-footage run SHALL NOT be part of `finish.sh` (the
footage exists only on the owner's machine); the same pipeline SHALL run on the generated
fixture media as a live scenario of `live-check.sh`, writing to its artifact directory and
never to `docs/images/`. Without `--sources` the command-line entry SHALL exit with a usage
message.
Why: the landing page's `site/screenshots.py` drives the window by coordinates measured
before R.49 and silently produces wrong crops after any layout change.

#### Scenario: A full re-shoot on fixture media
- **WHEN** the script runs with `--sources` pointing at the generated fixture videos and an output directory in a temporary location
- **THEN** every declared image is written, non-empty, at the declared size, and the step log lists one entry per image; a run whose FX window fails to open stops at that step with a non-zero exit
- **TEST** live `src/scripts/live/readme_shots.py`

### Requirement: The guide stays within the site's Markdown, and the shortcuts page is the one key reference
Every file under `docs/guide/` SHALL use only the Markdown the site renders: `#` headings,
paragraphs, `-` and `1.` lists with at most one level of nesting, `|` tables (a pipe inside a
cell written `\|`), fenced code blocks, `**bold**`, `*italic*`, `` `code` ``, `[text](path)`
links, an image on a line of its own, and `<kbd>…</kbd>`. Outside code it MUST NOT contain
any other HTML tag, an unpaired `<kbd>`, a `>` quote, a setext (`===`/`---` underline)
heading, a reference link `[x][y]` or a footnote `[^n]`, `_italic_`, or a backslash other
than `\|`. `docs/guide/shortcuts.md` SHALL be the complete reference of the keys and
mouse gestures the program handles. Each row describes a key only where it really does that
action, and the behaviour stated is the program's, checked against the code. Its tables SHALL
have the header `| Keys | Action |`. Every key and modifier SHALL be written as a
`<kbd>` cap, chords joined with `+`, alternatives with ` / `, and holds, double presses,
wheel and clicks in words next to the caps. It MUST NOT write a key chord as bold text. The guard
`src/scripts/readme-check.py --check` SHALL report each violation as a `markup` defect with
file and line. It runs in `cargo test` and in `finish.sh` like the other classes.
Why: the site (kinetren.github.io) shows `docs/guide/*.md` as `/docs/`, and its renderer
fails the whole build on anything outside this subset. The landing page's "Keyboard" section
was removed (owner, 2026-10-26: "a reference does not belong there"), so `shortcuts.md` is now
the only one, and it was poorer than the landing: bold-text keys, no region or view groups,
handled gestures missing.

#### Scenario: The committed guide passes the markup check
- **WHEN** `readme-check.py --check` runs on the repository
- **THEN** exit 0; `shortcuts.md` has only `| Keys | Action |` tables and no bold key chord
- **TEST** t826 (`src/apps/kt-cli/tests/readme_guard.rs`)

#### Scenario: The guard catches each markup defect
- **WHEN** it runs on a temporary tree whose guide has, one at a time: a `<br>` tag, an unpaired `<kbd>`, a `>` quote, a setext heading, a reference link, a footnote, `_italic_`, a stray backslash, a list nested two levels deep, a `shortcuts.md` table headed `| Key | Action |`, a `**Ctrl+S**` in `shortcuts.md`; and the same constructs inside a fenced block and a code span
- **THEN** each defect exits non-zero and names the file, the line and the class `markup`; `<kbd>`, `\|` in a cell and the constructs inside code are accepted; the clean tree exits 0
- **TEST** t869 (`src/apps/kt-cli/tests/readme_guard.rs`)
