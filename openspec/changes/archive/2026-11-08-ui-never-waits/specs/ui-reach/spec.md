## ADDED Requirements

### Requirement: Every window and Ctrl+Z answer while an effect takes seconds a frame
While the composite of every frame takes seconds (an anomalous effect parameter), the effects window
SHALL open, draw and close, the gallery SHALL add an effect, Ctrl+Z SHALL undo, and the transport SHALL
start and stop, each at the next UI frame; the telemetry journal SHALL note no UI frame blocked for
the composite. The effects window SHALL ask whether the compute device works without opening it.
Why: the owner, kkt-26.11.07 E8 ("тупых решений не предлагай, нужны правильные") and kkt-26.11.08 H9;
CLAUDE.md: "a slow effect costs frames, never the Undo".

#### Scenario: The effects window and Ctrl+Z with the kernel stuck
- **WHEN** an effect is added to an event while the executor's composite never ends, its window is opened, Ctrl+Z is pressed, Space twice, and the window's Done is clicked
- **THEN** the window draws its Done button, Ctrl+Z removes the effect, playback starts and stops, and Done closes the window
- **PATH** double click an event → its effects window opens; add an effect whose frames never finish → the picture stays as it was, the window keeps answering; press Ctrl+Z → the effect leaves the chain; press Space twice → playback starts and stops; click Done → the window closes
- **TEST** t1848 (`src/apps/kt-app/src/ui_wait_tests.rs`)

#### Scenario: Asking about the compute device never opens it on the UI thread
- **WHEN** the device's opening stands until the test lets it go, and the window asks three times whether the device works
- **THEN** each question answers at once that nothing is known; after the release the device opens and works
- **TEST** t1847 (`src/libs/kt-plugin-api/tests/compute_now.rs`)

#### Scenario: Live, on the heavy showcase with Datamosh at its extremes
- **WHEN** the program plays over events whose frames take seconds, the person double clicks one, adds a Datamosh from the gallery, presses Ctrl+Z, clicks Done and stops
- **THEN** each consequence lands on the live model within a frame or a few, and the journal notes no UI frame of 400 ms or more
- **PATH** Home, Space → playback over the moshed events, the picture lags; double click a moshed event → its effects window opens; picker → type "datamosh" → click a cell → a second Datamosh in the chain; Ctrl+Z → it is gone; Done → the window closes; Space → playback stops
- **TEST** live `ui_wait` (`src/scripts/live/ui_wait.py`)
