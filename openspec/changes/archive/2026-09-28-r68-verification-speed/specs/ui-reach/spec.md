# Spec Delta

## MODIFIED Requirements

### Requirement: A test asserts the observable consequence from the point where the person acts; a live scenario for each item
A reachability test SHALL assert what the person sees and does (the rectangle grew, the click hit, the block moved, the value reached the model/file), and not the wiring ("the field was set", "the item is in the menu"). The point of action is part of the assertion: the scene SHALL
give a LIST of places, and a new place is added to it, and not "checked later". The closing of
a UI item SHALL be accompanied by a live scenario in a real window (Xvfb, a real mouse),
which first proves that the gesture HAPPENED, and only then checks the consequence.

**What the live run pays for — is decided by a CRITERION, not by the author's taste.** The mouse in a real window
SHALL check: a place that has NEVER once been touched live; the hit of a gesture and its repeatability (the hit test, the edges, the order of windows, modality, drag-and-drop); and a complex
multi-step path named in the owner's claim. A check that, after a gesture has taken place, asserts ONLY the state of the model or file and repeats a gesture
already checked live in another scenario SHALL move to headless (the kt-app harness or
a model test): in a live run such a check costs seconds and adds no knowledge. The move
MUST keep the spec scenario and its `TEST` line with the NEW address — the scenario does not disappear,
only the place of the check changes; deleting a scenario without replacement remains a violation of the rule
"tests from the specs are not weakened". The move SHALL be recorded MACHINE-READABLY — by a registry
"live scenario → the tests that hold its assertions, and why", — and the guard SHALL
fail if the file of a moved scenario remains on disk or if the test it names does not
exist: without the registry "move to non-gui" and "throw out" are indistinguishable by the tree. What has never been touched live SHALL be shown by a machine
live coverage report (the registry windows and the fields that no scenario clicked), and
`finish.sh` SHALL print its total. The number of untouched places SHALL have a DECLARED
LIMIT, and exceeding the limit SHALL be red: the limit is a ratchet, not a target, and a new
place of the program either gets a live step, or the limit is moved deliberately.
Why: everything the owner complained about in the round "you didn't check" was green in the harness
and broken in the window — t253 pressed RMB on a 34-px header, where the item had been placed. R.68: 96 % of
the live run's time is unconditional pauses (`nestparam`: 136 s, of which 130 s of sleep in 591 calls), and
the scenarios partly repeat each other. The owner's decision: "move some of the tests to non-gui,
since the tests partly repeat each other's actions… and click through the complex moments and
those that were not clicked even once".

#### Scenario: The row height reaches the event
- **WHEN** an individual height is set for a row
- **THEN** the event's rectangle grew, and a click BELOW the old boundary hits the event (and not "`row_h` changed")
- **TEST** t274 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Blocks do not hide under one another and are caught by the frame
- **WHEN** "+M" is pressed again on an occupied gap; the selection frame passes over service blocks
- **THEN** each block is caught by the mouse in ITS OWN row (a free row is sought in the gap, a new one — only if there is no free one); the frame catches morphers and cameras, Del deletes everything caught in one step
- **TEST** t275, t277 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Live scenarios of the mechanism
- **WHEN** a live check is run (`scripts/live-check.sh`)
- **THEN** the edge of the morph block and the row height reach the file, RMB on the contour of an area changes the screen; RMB on a transition opens its menu, a double click on a level reaches the .otio, the bin and the meters do not stay silent; the publishing path entirely by mouse — from the right click on a caption to the second release with its own title
- **TEST** t264 (`src/scripts/live/deadend.py`), t397 (`src/scripts/live/rmb.py`), t689 (`src/scripts/live/nestparam.py`), `src/scripts/live/fxpicker.py`, `src/scripts/live/keys.py`

#### Scenario: The live coverage report and moving a check
- **WHEN** the live coverage is counted: the registry windows `WINDOWS` and the fields of the geom dump against what the scenarios `src/scripts/live/*.py` click (both by calling a probe by name, and by parsing the rectangle from the dump — the aim by coordinate is counted with the same aim); separately, a spec scenario's test address changes from live to headless; separately, a moved scenario returns to disk; separately, untouched places become more than the declared limit
- **THEN** the report names the windows and fields not touched live EVEN ONCE, their number and the declared limit, and also the move registry "scenario → new addresses"; `finish.sh` prints the total of the report; after the move the spec scenario is intact and its `TEST` line points to an existing test, while the disappearance of a scenario without replacement, the return of a moved scenario, a non-existent new address and exceeding the limit are red
- **PATH** there is no gesture: the guard reads the window registry, the geom dump and the texts of the live scenarios and prints which places a person has never once reached with the mouse in the check
- **TEST** t804 (`src/apps/kt-app/src/reach_tests.rs`, calls `src/scripts/live-coverage.py --check`)

#### Scenario: The track verbs are clicked through from the MENU and from the WINDOW
- **WHEN** `src/scripts/live/trackverbs.py` opens the row menu by a right click on an empty place of the track and presses in it "Mute", "Solo", "Lock", "Properties…" and "Effects…"; then in the track window it clicks the checkboxes "Mute"/"Solo"/"Lock", the blend mode "Add" and "Cancel"
- **THEN** each item and each checkbox is reachable by the mouse, the consequence is VISIBLE IN THE FRAME (under a muted track — the lower one; with solo — only it; adding green to red gives yellow), "Lock" from the menu refuses a drag, "Cancel" returns both the checkbox and the mode in one step and does not leave the mode in the file; the program publishes the rectangle of each of these places, and the scenario presses each one
- **PATH** right click on an empty place of the top video track → "Mute" → in the preview red instead of green → the same item once more → green again → right click on the lower track → "Solo" → in the frame only it → "Solo" once more → "Lock" → a drag of the event does not move it → "Lock" once more → "Properties…" → the track window → the checkbox "Mute" → the frame is red → the checkbox once more → the blend mode "Add" → the frame is yellow → "Cancel" → the frame is green again, the window closed → right click on the track → "Effects…" → the same window with the chain strip
- **TEST** t806 (`src/apps/kt-app/src/reach_tests.rs`), live scenario `src/scripts/live/trackverbs.py` (`./scripts/live-check.sh`)

## ADDED Requirements

### Requirement: Live rig: the process is reused, waiting follows the FRAME, not the clock
The live check SHALL keep one kt-app process per worker and open projects in it ONE AFTER ANOTHER, instead of a cold start for each scenario. Opening a project in the stand SHALL
bring the program to a clean state (closed windows, a reset workplace), and the scenario SHALL receive proof of cleanliness before the first gesture: the stand reports
readiness, the number of open windows is zero, and the open project is the one that was requested. A scenario
that checks the program's START ITSELF (recovery after a crash, the question about unsaved
work, the project lock, importing a file of a previous version) SHALL declare "its own
process" and get it; the declaration is part of the scenario, not a setting of the run.

The wait after a gesture SHALL be tied to an OBSERVABLE event of the program — the number of
read input events, the number of the drawn frame, the appearance of a widget in the geometry dump or a change of a pixel — and not to a fixed pause. The program SHALL report next to the geometry dump BOTH the number of the drawn frame AND the number of input events it has read: one frame
is not enough — it could have STARTED before the event arrived from the X server, and then "the frame is drawn" does not
mean "the gesture is seen". The wait for "the program has finished reacting" SHALL be counted by the silence between
frames, and this silence SHALL be measured in OBSERVABLE steps of a frame, not in seconds: under load a frame takes three times longer, and a fixed threshold would declare the program calm
in the middle of a redraw. The typing of a line SHALL
arrive at the program in full, and not character by character with a pause for each. The fixed pauses in
the harness and the scenarios SHALL be enumerable: each remaining pause carries a REASON in words
(a gesture that has no observable consequence, for example the double-click time threshold), and
the guard SHALL fail on a pause without a reason. The test channel of the stand SHALL be switched on
only by an environment variable and SHALL have no effect in the product.
Why: R.68 measured that 96 % of the live run's time is sleep: `nestparam` 136 s with 130 s of
sleep in 591 calls, `curvesrc` 43 s with 42 s of sleep, `level` 4.7 s with 4.2 s of sleep. The pauses
were tuned to software rendering ("a frame takes ~0.2 s"), that is, the harness waited for HOURS
for what the program could have reported itself; the same blindness also gives false reds under load.
The owner: "cut down the kt restarts (you can open projects one after another, after all)".

#### Scenario: Projects one after another in one process, start where it is needed
- **WHEN** a worker runs three scenarios in a row that did not declare "its own process"; then a scenario that declared it (the check of recovery after SIGKILL)
- **THEN** the first three passed in ONE kt-app process: each saw its own project, an empty list of open windows and a workplace without traces of the previous scenario; the one that declared "its own process" got a new process and checked the start; the verdicts matched a run in which each scenario started by itself
- **PATH** the person does not see this: the scene is a run of the check; in the step screenshots of each scenario the first frame shows its own project and not a single open window from the previous one
- **TEST** t805 (`src/apps/kt-app/src/reach_tests.rs`), live scenarios `src/scripts/live/recover.py`, `src/scripts/live/level.py` (`./scripts/live-check.sh`)

#### Scenario: Waiting for the frame, not for the clock
- **WHEN** a scenario makes a click, types a line and waits for a widget to appear; separately, a fixed pause without a reason is added to the harness
- **THEN** each wait ends at an observable event (the program has read the input, the frame number has grown, the widget appeared in the dump, a pixel changed), the line arrived in full within one wait, and a pause without a reason makes the guard red; the total time of the live run on the same scenarios is noticeably less than before
- **PATH** there is no gesture: the guard reads the harness and the scenarios, counts the pauses without a reason and checks that the waits are tied to an event of the program
- **TEST** t803 (`src/apps/kt-app/src/reach_tests.rs`, calls `src/scripts/live-sleeps.py --check`), live scenario `src/scripts/live/nestparam.py`
