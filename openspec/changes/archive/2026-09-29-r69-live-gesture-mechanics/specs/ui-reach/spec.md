# Spec Delta

## MODIFIED Requirements

### Requirement: Live rig: the process is reused, waiting follows the FRAME, not the clock
The live check SHALL keep one kt-app process per worker and open projects IN IT ONE AFTER ANOTHER, instead of a cold start for each scenario. Opening a project in the stand SHALL bring the program to a clean state (closed windows, reset workspace), and the scenario SHALL receive proof of cleanliness before the first gesture: the stand reports readiness, the number of open windows is zero, and the open project is the one that was requested. A scenario that checks the START of the program ITSELF (recovery after a crash, the question about unsaved work, the project lock, importing a file of an older version) SHALL declare "its own process" and receive it; the declaration is part of the scenario, not a setting of the run.

The wait after a gesture SHALL be tied to an OBSERVABLE event of the program — the number of input events read, the number of the frame drawn, the appearance of a widget in the geometry dump, or a change of a pixel — and not to a fixed pause. This SHALL apply to the GESTURE ITSELF too: click, right click, double click, drag, key, wheel and hover wait until the program READS their events, not for as long as a frame usually takes. Next to the geometry dump, the program SHALL report BOTH the number of the frame drawn AND the number of input events it has read: one frame is not enough — it could have STARTED before the event arrived from the X server, and "frame drawn" would then not mean "gesture seen". The wait for "the program has finished reacting" SHALL be computed from the quiet between frames, and this quiet SHALL be measured in OBSERVABLE steps of the frame, not in seconds: under load a frame takes three times as long, and a fixed threshold would declare the program calm in the middle of drawing. A line of text SHALL reach the program whole, not character by character with a pause for each. The window SHALL be raised when the WINDOW CHANGES and at the first gesture of a scenario, not before every gesture: raising is a round trip to the X server and its own pause, while the window does not change between two consecutive clicks.

Fixed pauses SHALL be enumerable IN THE WHOLE live check — in the harness, in the scenarios and in the mouse and keyboard driver: a sleep hidden in the driver is no different from a sleep in the harness, and a rule that does not see it holds less than it promises. Every remaining pause SHALL carry a REASON in words (a gesture that has no observable consequence, for example the egui double-click time threshold or the step of a drag between positions), and the guard SHALL fail on a pause without a reason. Reasons SHALL NOT be the ONLY measure: the total fixed sleep of a run SHALL be counted by the machine, printed, and have a DECLARED LIMIT, exceeding which is red. The limit is a ratchet, not a knife: a new pause either fits within it, or the limit is moved deliberately, in one line and with a reason.

Reading a POINT or a small area of the screen SHALL go through a direct request to the X server, not through a snapshot of the whole screen parsed by an external tool. The screenshot (`xwd`) SHALL remain the REFERENCE: the program window has a 32-bit visual, and reading the root window had already returned BLACK where there was a picture — the direct path is accepted only after a check against the reference ON THE SAME frame, and a discrepancy returns the reading to the reference. Snapshots for artifacts and for the steps of `PATH` remain snapshots.

The test channel of the stand SHALL be enabled ONLY by an environment variable and SHALL have no effect in the product.
Why: R.68 measured that 96 % of the time of a live run is sleep: `nestparam` 136 s with 130 s of sleep in 591 calls, `curvesrc` 43 s with 42 s of sleep, `level` 4.7 s with 4.2 s of sleep. The pauses were picked for the software renderer ("a frame takes about 0.2 s"), that is, the harness was waiting for HOURS for something the program could have said itself; the same blindness also gives false reds under load.
The owner: "cut down kt restarts (you can just open projects one after another)."
R.69: the wait by event reached the probes but did not reach the gestures — 387 s out of 1582 s of a sequential run remained sleep (click 0.78 s, key 0.77 s, drag 2.55 s, typing a line 0.77 s per character), and 123 s of them lived in the X driver, which the guard did not look at. All 265 s in the harness, meanwhile, were "legitimate": each pause had a reason longer than forty characters. Prose did not hold — an exception must be MEASURABLE (the same lesson as the window registry R.66 and the PATH guard R.67).

#### Scenario: Projects one after another in one process, start where it is needed
- **WHEN** a worker runs three scenarios in a row that did not declare "its own process"; then a scenario that declared it (a recovery check after SIGKILL)
- **THEN** the first three passed in ONE kt-app process: each saw its own project, an empty list of open windows and a workspace without traces of the previous scenario; the one that declared "its own process" got a new process and checked the start; the verdicts matched the run where each scenario started on its own
- **PATH** the person does not see this: the scene is a run of the check; in the step screenshots of each scenario, the first frame shows its own project and not a single open window from the previous one
- **TEST** t805 (`src/apps/kt-app/src/reach_tests.rs`), live scenarios `src/scripts/live/recover.py`, `src/scripts/live/level.py` (`./scripts/live-check.sh`)

#### Scenario: Waiting for the frame, not for the clock
- **WHEN** a scenario clicks, types a line and waits for a widget to appear; separately, a fixed pause without a reason is added to the harness
- **THEN** each wait ends on an observable event (the program has read the input, the frame number has increased, the widget has appeared in the dump, the pixel has changed); the line arrived whole in one wait, and a pause without a reason makes the guard red; the total time of a live run on the same scenarios is noticeably less than before
- **PATH** there is no gesture: the guard reads the harness and the scenarios, counts pauses without a reason, and checks that the waits are tied to an event of the program
- **TEST** t803 (`src/apps/kt-app/src/reach_tests.rs`, calls `src/scripts/live-sleeps.py --check`), live scenario `src/scripts/live/nestparam.py`

#### Scenario: The gesture itself waits for the read input
- **WHEN** the harness performs a click, a right click, a double click, a drag, a key press, a wheel and a hover; separately, two gestures in a row in ONE window; separately, a gesture after a window change
- **THEN** each gesture ends by the number of input events read by the program, not by the clock; the window is raised at the first gesture and when the window changes, and between two gestures in the same window it is not raised; the verdicts of all live scenarios match the run on the previous harness
- **PATH** there is no gesture: the scene is the live check run itself; the guard reads the harness and asserts that the gesture has no fixed pause without a declared reason
- **TEST** t807 (`src/apps/kt-app/src/reach_tests.rs`), comparison run `./scripts/live-check.sh` (all scenarios, verdicts before and after)

#### Scenario: Sleep is enumerable across the WHOLE check and has a limit
- **WHEN** the sleep guard reads the harness, the scenarios AND the mouse and keyboard driver; a pause without a reason is temporarily added to the driver; separately, the total sleep of a run is raised above the declared limit
- **THEN** the guard names the pause in the driver and turns red — the scope of the rule includes it on equal terms with the harness; the total fixed sleep is computed and printed, and exceeding the limit is red with the name of the most expensive pause
- **PATH** there is no gesture: the guard reads the sources of the live check and prints the sleep budget; it is verified by running the script, not by the mouse
- **TEST** t808 (`src/apps/kt-app/src/reach_tests.rs`, calls `src/scripts/live-sleeps.py --check`)

#### Scenario: A screen point is read from X, and the snapshot remains the reference
- **WHEN** the harness reads the colour of a preview point by a direct request to X and with the same frame — an `xwd` snapshot; separately, the direct path returns a colour that does not match the reference
- **THEN** the colours match, and the scenarios read the point without a snapshot of the whole screen; on a discrepancy the reading falls back to the reference and says so, rather than silently returning black
- **PATH** there is no gesture: the scene is the harness reading the screen; it is verified by comparing the two paths on the same frame
- **TEST** t809 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/trans.py` (`./scripts/live-check.sh`)
