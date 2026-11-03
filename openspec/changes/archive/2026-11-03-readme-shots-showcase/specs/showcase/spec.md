## MODIFIED Requirements

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
