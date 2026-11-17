## ADDED Requirements

### Requirement: The close is in the trace
A close that goes on SHALL write the journal's `close asked` and a `Shutdown` record (`ASKED`); every wait
of the close SHALL write a `Shutdown` span (`aux` who, `dur` how long, `b` 1 when it was left behind); the
durability point (`DURABLE`), eframe's return (`RETURNED`) and a forced exit (`FORCED`, naming what was
still waited for) SHALL be recorded too. The trace SHALL be flushed after `on_exit`, after eframe returns
and before a forced exit. `kt-cli trace … shutdown` SHALL print the close, its waits and what each thread
did after the close was asked; a run without a close SHALL say so with its last record.
Why: close-while-busy (kkt-26.11.18 T1): the owner's three traces of 2026-11-18 had no word of the close —
nothing recorded it, and a clean exit never flushed the writer's last batch — so a close that hung could
not be told from a kill.

#### Scenario: A close's waits are named
- **WHEN** a close joins a thread past its bound and another within it
- **THEN** both waits are `Shutdown` records with their names, durations and outcomes
- **TEST** t2950 (`src/libs/kt-time/src/shutdown.rs`)

#### Scenario: The live close is read back
- **WHEN** the live scenario closes the busy program
- **THEN** `kt-cli trace shutdown` of its run prints "the program closes" and no forced exit
- **TEST** t2954 live scenario `src/scripts/live/close_busy.py`
