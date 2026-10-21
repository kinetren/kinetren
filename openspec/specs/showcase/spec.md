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
"ридми это про продукт, как его запустить, а не про технический мусор"); that material
lives in `CONTRIBUTING.md` and the design documents. The README SHALL open with the
application icon, the name and badges, then a full-window screenshot of the editor on the
showcase project DURING PLAYBACK: the media bin wide, in free layout with its groups, and
the level meters showing live levels; no reel is expanded in it. It SHALL contain a
feature tour with at least these screenshots: the timeline with a reel instance from the
MIDDLE of the edit expanded in place and centred, its veiled tails visible on both sides;
the published parameters of a reel instance with its own values; Ctrl+F search; the
Slowed+Reverb editor with its `rate`/`mix` automation curves visible; the video FX
editor; transitions; the morpher as a before/after of the same frame (source swap and
look) plus its editor; a title/generator; the render dialog and the diagnostics panel
(owner, 2026-10-22, fourth round). All screenshots SHALL be
taken from the English UI (`KT_LANG=en`) on free footage (CC BY or public domain) and
SHALL be produced by `src/scripts/readme-shots.py`, never edited by hand; the
attribution of that footage SHALL be in `docs/images/CREDITS.md` and in `CREDITS.txt`
next to the downloaded files.
Why: owner's requests — "fill the timeline with all kinds of editing", "show the
slowed-reverb automation curves", "nested reels belong on the timeline screenshot",
"no old samples", "credits go with the footage".

#### Scenario: Every README screenshot is produced by the pipeline
- **WHEN** `readme-check.py --check` reads the list of outputs declared by `readme-shots.py` and the images referenced by README/`docs/guide`
- **THEN** the two sets are equal (an image added by hand, or a declared shot missing from the docs, fails the guard)
- **TEST** t827 (`src/apps/kt-cli/tests/readme_guard.rs`)

#### Scenario: The overview, timeline and morpher shots show what they claim
- **WHEN** the pipeline takes the overview on the showcase project
- **THEN** the hero is shot during playback after widening the bin; for the timeline shot the mid-edit reel instance (the one with overrides) is selected, Enter expands it — more timeline rows than before, all fitting the panel — and the view is zoomed and scrolled until the instance is centred and at least 300 px wide; the morpher before/after is the same frame from the project and from a copy with the morpher disabled
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
before Р.49 and silently produces wrong crops after any layout change.

#### Scenario: A full re-shoot on fixture media
- **WHEN** the script runs with `--sources` pointing at the generated fixture videos and an output directory in a temporary location
- **THEN** every declared image is written, non-empty, at the declared size, and the step log lists one entry per image; a run whose FX window fails to open stops at that step with a non-zero exit
- **TEST** live `src/scripts/live/readme_shots.py`
