## ADDED Requirements

### Requirement: The trace says what the scheduler ran, paused, cancelled, and what a must-not-wait thread waited for
Every scheduled job SHALL be recorded when it is posted (its class, its lane, who posted it), when
it runs (a span with its time in the queue and its outcome; the job's own records carry it as their
cause) and when it is cancelled (the reason); every pause and resume of the background classes
SHALL be recorded with its reasons and the jobs waiting. The UI thread and the audio render thread
SHALL count every blocking call they make — a file open, an fsync, a thread join, a renderer build,
a sidecar open — and record it with its length. `kt-cli trace during` SHALL name them: a job by its
class and lane, a pause, a resume, a cancel by its lane, a blocking call by its thread and kind.
Why: `docs/engine-audit.md` M3/M10: nothing knew what background work ran, and nothing stopped the
next panel from calling a blocking API on the UI thread — "no type, lint or counter".

#### Scenario: Every job and pause is traced
- **WHEN** a job runs and records something, a queued job is cancelled, and a vote goes up and down
- **THEN** its post, run (done) and its record with it as the cause are in the trace, the cancel names its reason, the pause and the resume are there
- **TEST** t2234 (`src/libs/kt-time/src/sched/tests.rs`)

#### Scenario: Blocking calls are counted on the marked threads only
- **WHEN** an unmarked thread and the UI thread each make blocking calls, and another thread makes one
- **THEN** only the UI thread's are counted, by kind
- **TEST** t2235 (`src/libs/kt-time/src/block.rs`)

#### Scenario: The query names them
- **WHEN** a window holds a visible thumbnail job, a pause, a cancelled analysis job, a resume and an fsync on the UI thread
- **THEN** `during` lists each by class and lane, the pause and the resume, the cancel by lane, and the UI thread's fsync
- **TEST** t2243 (`src/apps/kt-cli/src/trace_query/tests.rs`)

### Requirement: At rest nothing wakes on a clock
No thread of the program SHALL wake on a timer at rest. The scheduler's workers park until a post,
a resume or an exit; the load watch parks after five quiet seconds until a work counter moves (a UI
frame included; the bench keeps sampling); telemetry's sender sleeps on its channel once a heartbeat
finds nothing new, until the next counted activity; the media watch looks at the files on the
opening, a focus change and play or stop, never on a clock and never on a pointer move. A pointer
over a still place SHALL start no job.
Why: the owner, 2026-11-07: "ПОЧЕМУ АГЕНТАМ НЕ ПРИКАЗЫВАЕТСЯ ТЕСТИРОВАТЬ ЭТО?!?"; `docs/engine-audit.md`
§2b counted seven timers at rest (rows 122, 174).

#### Scenario: The pool sleeps at rest
- **WHEN** nothing is posted for a second and a half
- **THEN** no worker of the pool makes a context switch
- **TEST** t2233 (`src/libs/kt-time/src/sched/tests.rs`)

#### Scenario: A parked watcher wakes on work only
- **WHEN** a watcher parks until work moves and nothing is counted for 300 ms, then one count
- **THEN** it stays parked, then wakes
- **TEST** t2241 (`src/libs/kt-time/src/work.rs`)

#### Scenario: A quiet session's telemetry sleeps
- **WHEN** a heartbeat carried one activity and the next found nothing new; then another activity
- **THEN** the sender sleeps and sends nothing; the activity wakes it and the next beat carries it
- **TEST** t2242 (`src/apps/kt-app/src/telemetry.rs`)
