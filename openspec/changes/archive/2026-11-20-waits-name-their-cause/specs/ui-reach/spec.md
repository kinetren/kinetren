## ADDED Requirements

### Requirement: A live wait for the window names why none came
A stand and a scenario's own process SHALL print, once the window is found, the display, the program's
pid and its first log line of this start; while no window is found they SHALL say every 30 s, and on
giving up, whether the program ended (with its code), its editor child ended under a standing
supervisor, it is not a client of its display, or it is connected and still starting. A program that
ended, or whose editor child ended, SHALL end the wait at once.
Why: a run waited the whole guard on four stands with an empty log and nothing said why
(found-defects 824).

#### Scenario: A dead program and a dead editor are named and end the wait
- **WHEN** the diagnosis is run on a process that ended with code 3 and on a stand-in supervisor whose child is killed, with no askable display
- **THEN** the first is dead and named "ended (code 3)" with its first log line; the live one is "still starting" while its child lives and dead, "the editor process ended", once the child ends; the stand and `kt_live` ask it in their window waits and print the start line
- **TEST** t3239 (`src/apps/kt-cli/tests/dist_guard.rs`)
