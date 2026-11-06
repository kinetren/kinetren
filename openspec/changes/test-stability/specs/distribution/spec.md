## ADDED Requirements

### Requirement: A full run under load tells a defect from a busy machine
A test SHALL wait for what must happen with `kt_time::test_deadline()`, a ten-minute guard against a
hang and no claim about speed. A wall-clock limit of its own is allowed only where the limit IS the
check, a budget the program promises to a person, and the guard lists each such limit with its reason.
A test that must keep up with the wall clock SHALL be judged against what the machine gave at that
moment. A live scenario SHALL wait for the program: its fixed steps stretch with the observed frame
time, and a window that closes is waited out with `wait_gone`. Nothing of this retries a test or
weakens its observable consequence. The rerun of a red live scenario alone stays, and its report still
names what passed on the second try.
Why: every full run under load gave random reds, the orchestrator rechecked each merge by hand, and a
real defect was easy to take for noise (`docs/found-defects.md`, rows 24–30, 47, 57–59, 79, 82). One
fsync took 44 s on the working disk while a build ran: "the download did not end" in 20 s, "probe not
published" in 45 s and t815's 2 s all fired on correct code.

#### Scenario: Runs of the fixture generator meet
- **WHEN** six runs of `fixtures/gen.sh` start at once on an empty `out/`, with a stand-in `ffmpeg` that
  writes its output in two steps
- **THEN** every run succeeds and no file in `out/` is half-written
- **TEST** t1670 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: A test does not bound its wait by the clock
- **WHEN** the sources of `src/apps`, `src/libs` and `src/plugins` are read
- **THEN** no line sets `Instant::now() + Duration::from_…` or compares `.elapsed() < Duration::from_…`
  outside the guard's list of limits that are the check, and every entry of that list still exists
- **TEST** t1671 (`src/apps/kt-cli/tests/dist_guard.rs`)
