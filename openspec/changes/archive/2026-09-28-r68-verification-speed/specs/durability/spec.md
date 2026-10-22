# Spec Delta

## MODIFIED Requirements

### Requirement: Work ends with `finish.sh`, and the proof is a live scenario
Any work on the program SHALL end with `src/scripts/finish.sh` and a
version `vYY.MM.DD-hash` in the report; the script's output MUST NOT be filtered, and the
tree is not edited while it runs. The verdict of each verification stage
MUST be taken from the command that checks (the cargo return code), and not from the one
that prints. For each item of the owner's claim there SHALL be a live
scenario in a virtual screen (the real binary, real gestures), and the test
asserts an observable consequence from the point where the person acts.
A failed test that stands on a pause or on the order of a background thread, and a `SIGBUS` without
a single line of the test on the first run after a rebuild MUST be
re-run separately before being considered a regression.

**The script is the SINGLE point of verification and names its price.** The workspace test run
SHALL be a stage of this same script, not a separate command of the
agent: otherwise the budget and the verdict fall into two unconnected halves, and the
kt-gpu stage duplicates what the workspace has already run, and therefore MUST
be removed (the SIMD stages remain — they have their own environment). Each stage
SHALL print its own time, and the script — the TOTAL; a total longer than **five minutes**
(the budget set by the owner) SHALL be flagged in the report together with the
name of the stage that consumed it. The budget and the timing SHALL be held
by a guard, not by the agent's memory.

**Live scenarios are run by several workers.** The run SHALL distribute scenarios to workers, each of which has ITS OWN virtual screen
and ITS OWN user storage; the number of workers SHALL be limited so that the load does not exceed the number of
machine cores (one kt-app in software rendering takes about seven). A scenario that fails in the parallel
run SHALL be automatically re-run ALONE; only a repeated failure counts as red, and the report SHALL name the scenarios that
passed only on the second attempt. A run without parallelism SHALL remain available with one key.
Why: `cargo test` builds a separate binary and does not rebuild `target/debug/kt-app` — the owner twice ran a month-long build and counted
defects as live ("you didn't check" ×3); `if cargo test | tail` was green on any failure; eleven "green" items of the round turned out to be broken in the window,
because the harness checked the wiring, not the consequence; `SIGBUS` is a substitution of the
binary under a mapped image on a separate partition, not a defect. R.68:
a verification cycle cost 21 minutes (live scenarios 1012 s, workspace 259 s,
build ~100 s), and a check that costs this much itself creates the temptation to
bypass it — that is, it works against the rule by which it was set up. The ban on "two
threads" was lifted by the owner with a direct decision ("Yes, with a load limit and an
automatic restart of the red") together with the mechanism that removes its
cause: a reddening under a load of ~14 now differs from a defect by
a repeated single run.

#### Scenario: Stage verdict from cargo
- **WHEN** the test stage of `finish.sh` fails
- **THEN** "WARNING: … red" is printed, along with the names of the failed tests and the path to the full log; a substitution of `cargo` by a failing one makes the stage red
- **TEST** `src/scripts/finish.sh` (the workspace/SIMD stages, log `target/finish-*-fail.log`)

#### Scenario: Live data-safety scenarios
- **WHEN** `src/scripts/live-check.sh` starts Xvfb with a separate `XDG_DATA_HOME` and `KT_AUTOSAVE_SECS`
- **THEN** `recover.py` (SIGKILL → an offer to restore, twice) and `unsaved.py` (the close cross asks, "Stay" keeps the window, "Don't save" does not touch the file) are green, screenshots and `.otio` are in `target/live-out`
- **TEST** live `recover.py`, `unsaved.py` (`src/scripts/live/recover.py`, `src/scripts/live/unsaved.py`)

#### Scenario: The cost of each stage and the five-minute budget
- **WHEN** `finish.sh` finishes a run; separately — a run in which a stage artificially lasts longer than the budget
- **THEN** a table "stage — seconds" with ALL the stages that the script executed is printed, and the total; when the total exceeds 300 s, the report carries a note with the name of the most expensive stage; not a single executed stage was left without a timing line
- **TEST** t800 (`src/apps/kt-app/src/reach_tests.rs`, calls the timing guard `src/scripts/finish-budget.py --selftest` and checks the script's list of stages against the list of measurements)

#### Scenario: A red in parallel is rerun alone
- **WHEN** in a parallel run a scenario fails the first time and passes the second; separately — it fails both times
- **THEN** in the first case the run is green, and the report says that the scenario passed on the second attempt alone; in the second — red with the name of the scenario and the path to the screenshots; a single run with one key re-runs nothing
- **TEST** t801 (`src/apps/kt-app/src/reach_tests.rs`, calls `src/scripts/live-check.sh` on a substitute scenario directory; without Xvfb the test is skipped with the named reason)

#### Scenario: Workers do not see each other
- **WHEN** the run goes on several workers
- **THEN** each scenario has its own screen number and its own user-storage directory, no scenario received its neighbor's storage, and the number of simultaneously live kt-app processes did not exceed the declared worker limit
- **TEST** t802 (`src/apps/kt-app/src/reach_tests.rs`, a substitute scenario prints `DISPLAY`/`XDG_DATA_HOME`; without Xvfb it is skipped with the named reason)
