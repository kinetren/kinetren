## ADDED Requirements

### Requirement: A colour is taken from the screen with the eyedropper
Every colour button of the program SHALL carry an eyedropper beside it. A click on it SHALL arm it:
the cursor becomes a crosshair and the status line says to click a colour. While armed, the next
click anywhere in the program window SHALL be taken by the eyedropper alone — nothing under it acts
— and SHALL write the screen's colour under the pointer into that row as one history step. Esc, or the
eyedropper again, SHALL put it down without writing. Where the screen cannot be read the bell SHALL
say so. The texts SHALL exist in all five catalogs.
Why: kkt-26.11.01b line 3 (B2): "greenscreen не имеет пипетки, чтобы ткнуть в тот же кадр превью или
еще где-либо взять цвет с экрана".

#### Scenario: The key colour is picked from the preview's frame
- **WHEN** a Greenscreen slot's pipette is clicked, then the preview's picture is clicked
- **THEN** the status line said to click a colour; the key colour is the screen's colour there, written as one history step; the preview did not start playing; the pipette is down
- **PATH** double click a clip → its effects window → + effect… → Greenscreen → the pipette beside Key → the cursor is a crosshair, the status line says to click a colour → click the preview's picture → Key shows the frame's colour, the background goes transparent
- **TEST** t3120 (`src/apps/kt-app/src/input_work_tests.rs`), live scenario `src/scripts/live/eyedropper.py` (`./scripts/live-check.sh`)

#### Scenario: Esc or the pipette again puts it down
- **WHEN** the pipette is armed and Esc is pressed; then armed and clicked again
- **THEN** each time it is down, the effects window is still open and no colour was written
- **PATH** the pipette → crosshair → Esc → the arrow is back, the window stays → the pipette twice → the arrow is back
- **TEST** t3121 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: Every colour button has it
- **WHEN** the kt-app sources are read for colour buttons; the catalogs are read
- **THEN** every colour button has the eyedropper call beside it; the three eyedropper texts exist in all five catalogs and the armed line names Esc
- **PATH** any colour row — a title's fill, a tint, a transition's colour — shows the pipette after its colour swatch
- **TEST** t3122 (`src/apps/kt-app/src/input_work_tests.rs`)
