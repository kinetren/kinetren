## ADDED Requirements

### Requirement: The program states its rest
Every background thread and service of the product SHALL declare whether it works, is parked, or has a timed wake due, and
SHALL declare itself parked only after the last flag, counter and hand-over of its pass. One question SHALL answer
whether the program is at rest, and if not, who is not. A test or a live scenario that asserts "nothing runs" SHALL open
its window from that answer, and its failure SHALL name who woke.
Why: found-defects 828, 950, 958, 961 (and 141, 764, 782, 967 before them, each cured alone) — each measured from a flag the program sets
before its last designed step, and each was cured by a flag of its own, so the next actor's test was red again
(rest-is-said D1).

#### Scenario: Every thread is an actor or says why not
- **WHEN** the product sources are read for threads started by name, and a player with a Datamosh, a preview and the disk prerender plays and stops
- **THEN** each thread's name is registered or stands in the list with its reason, and every registered actor is parked once the rest is reached
- **TEST** t4117 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: Work handed over between the flag and the park is seen
- **WHEN** an actor raises its flag and is held before it parks while another thread hands it work; then the rest is asked; then the work is done and a later wake lands inside a window
- **THEN** the rest is not reached until the second pass parked; the window names the actor that woke, and when
- **TEST** t4118 (`src/libs/kt-time/src/rest.rs`)

#### Scenario: The stretch tells an early window
- **WHEN** the output thread's tests run with every actor held 400 ms before it parks
- **THEN** they are green; the former reading of the close (the count, then the quiet) is red on it
- **TEST** t4120 (`src/apps/kt-app/src/audio_out.rs`)

### Requirement: A wait that gives up says so
A bounded wait SHALL answer whether what it waited for came; a caller SHALL NOT read the two ends as one. A test's wait
SHALL go to the guard and name, when it gives up, what was missing, the load average and who was not at rest.
Why: found-defects 961 — the trace's flush returns after two seconds as if the records were written; with the writer
held by a busy disk the test read a file without them (rest-is-said D2).

#### Scenario: A held writer is named, not read past
- **WHEN** the trace's writer is held past the flush's limit, and then released
- **THEN** the product's flush answers no; the tests' flush returns only after the records are in the file, and asked with a deadline already past, its failure names the writer and the records left
- **TEST** t4119 (`src/libs/kt-time/src/trace/tests.rs`)

### Requirement: A bench says how it ended
A test that plays a range through the bench SHALL end by what was played or shown, never by a number of seconds chosen for
a fast machine; an end that is neither SHALL be in the report with its reason, and a test SHALL assert there was none.
Why: found-defects 966 (fixed: the loop plays on until a frame was shown) — two more tests end after 6 and 8 seconds of
wall time, and a start that never sounds ends the bench as if the range had played (rest-is-said D3).

#### Scenario: A starved run is not a played one
- **WHEN** the bench's player never hands out a first sound and the bench is stopped from outside; and the source swap of the dynamic player's loop comes after three times the former six seconds
- **THEN** the first report says it gave up and who was not at rest; the second run plays on for a second of sound past the swap and hears the new source's level
- **TEST** t4121 (`src/libs/kt-engine/tests/player_dynamic.rs`)

### Requirement: A red leaves its message
A red test run SHALL leave each failed test's output in a file that a later run does not overwrite, and the finish summary
SHALL print each one's last assertion.
Why: found-defects 825, 827, 828 and 961 — the message was lost with the run, so the cause was read from the code or not
at all (rest-is-said D5).

#### Scenario: The message survives the rerun
- **WHEN** a run with one failing test is finished, the copy is made, and a green run follows
- **THEN** the copy still holds the assertion's text and the test's name
- **TEST** t4122 (`src/apps/kt-cli/tests/dist_guard.rs`)
