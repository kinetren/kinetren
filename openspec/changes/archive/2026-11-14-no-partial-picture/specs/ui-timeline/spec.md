## ADDED Requirements

### Requirement: A preview with no complete picture is an empty place under Loading
The preview SHALL draw only complete composites. Until the first complete one lands (a fresh project, a
preview that has shown nothing), the preview's place SHALL stay empty, and after 180 ms of incompleteness
the Loading scrim and its text ("Loading", or "Loading — catching up: N") SHALL stand in the frame's
place over the empty area exactly as they stand over a held picture. No option or path SHALL present an
incomplete composite on the display target or upload one as the preview's texture; the preview asks no
work for one beyond learning that it is incomplete (the executor stops where it knows).
Why: kkt-26.11.14 M1 — "пустое, нигде не допустима частичный показ и введение в заблуждение, частичный
показ - это опять лишняя работа по отрисовке!!" (found-defects 461: the first picture of a fresh preview
was presented incomplete, `show_incomplete: !shown_any`, because the scrim was drawn only over a picture).

#### Scenario: No picture yet is an empty place under Loading
- **WHEN** the preview has no picture, the frame asked is incomplete and 180 ms have passed
- **THEN** "Loading" is written at the centre of the preview's place and no image is painted there
- **PATH** open a heavy project → the preview's place is dark with "Loading" in its centre, no picture under it → the first complete frame lands → the picture, no scrim
- **TEST** t2533 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A person opens the heavy showcase
- **PATH** a small project is open → File › Open… → the heavy showcase → the timeline shows its events, the preview's place is empty under Loading → the picture lands, whole
- **WHEN** the live scenario polls the preview while the fresh project loads
- **THEN** no picture is ever drawn under Loading before the first full one (the old build drew 29 looks of a partial picture on one run); the open's work and the pause after it are printed
- **TEST** t2534 live `open_heavy.py` (`src/scripts/live/open_heavy.py`, via `src/scripts/live-check.sh`)

### Requirement: The meter under the cursor shows no level of a partial mix
On a pause, the level measured under a moving cursor (`Player::sample_levels_at`) SHALL be `None` when a
burst of the mix it renders is not in the cache; the meter SHALL then fall as on a still pause instead of
showing the level of the part cached (the rest read as silence) or holding another position's. Once the
bursts are cached, the level is the mix's.
Why: the class walk of no-partial-picture — the scrub meter showed a partial mix as the moment's level,
the audio twin of a partial picture.

#### Scenario: A partial mix gives no level
- **WHEN** the meter's probe asks a moment whose bursts the source has not delivered, then the same moment once they are cached
- **THEN** the first answer is `None`; the second is the tone's level
- **TEST** t2532 (`src/libs/kt-engine/tests/no_partial.rs`)
