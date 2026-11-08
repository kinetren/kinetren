## ADDED Requirements

### Requirement: A native crash leaves a report that names its thread and its stack
The editor and kt-cli SHALL put a handler on SIGSEGV, SIGBUS, SIGILL, SIGFPE and SIGABRT that, at
the signal and using only async-signal-safe means (system calls, fixed static buffers, the
unwinder primed at start; no allocation and no lock of the program), writes a raw report: the
build, the signal with its code and fault address, the process id, the faulting thread's name and
id, its program counter, its stack (the unwinder's frames; when the unwinder stops within three
frames, the code addresses found on the stack), the name of every thread, and the executable
mappings. The handler SHALL then restore the default action and re-raise, so the process dies by its
own signal; an alarm SHALL bound it. A healthy process SHALL symbolize the raw report from the
binaries' line tables (a frame with none keeps `module+offset`): the editor's supervisor right after
the child's death, into `kt-app-crash.txt` with a `summary:` line (signal, thread, the top frame and
the first frame of our code), the stack, the threads and the crashed run's last telemetry-journal
lines (its RUN line found by the child's pid); the crash window SHALL show the summary and those
journal lines; with no supervisor, the next start SHALL do the same before it takes the report;
kt-cli SHALL start its own `crash-report` and print the report on stderr. The raw report SHALL be
taken once (it moves to `.seen`), and the journal's "the previous run crashed" record SHALL carry
the summary. A stack overflow MAY end without the report (no alternate signal stack, see design).
Why: the owner, kkt-26.11.08 H5: "краш скудный" — his report of a SIGSEGV named the signal and a log
of one header line; H4: "debug сборка как-то обильно падает на SIGSEGV", and no report said where.

#### Scenario: A fault in a named thread is named with its function, file and line
- **WHEN** a child process with the handler installed writes through a null pointer in a thread named `kt-crash-probe`, beside three named threads
- **THEN** it dies by SIGSEGV; the symbolized report says SIGSEGV, code 1 (address not mapped), address 0x0, thread `kt-crash-probe`; its frame 0 is `kt_crash::testing::kt_crash_probe_segv (libs/kt-crash/src/lib.rs:…)`, the caller is further down, every thread is listed with the faulted one marked; the summary begins `SIGSEGV in thread "kt-crash-probe" at kt_crash::testing::kt_crash_probe_segv`
- **TEST** t2080 (`src/libs/kt-crash/tests/handler.rs`)

#### Scenario: An abort names the code that aborted
- **WHEN** a child process aborts in a named thread
- **THEN** it dies by SIGABRT; the report says SIGABRT, sent by the process itself, names the thread and the aborting function through the C library's frames
- **TEST** t2081 (`src/libs/kt-crash/tests/handler.rs`)

#### Scenario: The raw format parses and stays exact without line tables
- **WHEN** a raw report whose mapping is a missing file is symbolized
- **THEN** its fields are read, each frame keeps `module+offset` from the mapping's file offset, and a legacy hash suffix is dropped from a name
- **TEST** t2083 (`src/libs/kt-crash/src/symbols.rs`)

#### Scenario: kt-cli prints its own crash report
- **WHEN** `kt-cli crash-probe` faults on a thread named `kt-cli-probe`
- **THEN** kt-cli dies by SIGSEGV, its stderr carries the symbolized report (signal, thread, the probe function with file and line, the faulted thread marked) and a last line naming the report file in the temporary folder, whose first line is `kt-cli <full version>`
- **TEST** t2082 (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: The editor's report carries the summary, the stack and the run's journal
- **WHEN** the editor's child installs the telemetry journal and the handler, records an action and faults in a thread named `t2084-fault`, and the supervisor gathers the crash with the child's pid
- **THEN** the report holds the exit line, `summary: SIGSEGV in thread "t2084-fault" at kt_crash::testing::kt_crash_probe_segv…`, the stack with file and line, the faulted thread marked and the run's journal lines with the action; the window's lines start at this pid's RUN line; the raw report moved to `.seen`
- **PATH** Kinetren dies by a fault → the crash window opens → under "The program was ended by the signal SIGSEGV (segmentation fault)." a monospace line names the thread and the function, and "The last lines of the log:" lists what the program and the person did last (the telemetry journal) → "Open the log folder" → `kt-app-crash.txt` holds the stack
- **TEST** t2084 (`src/apps/kt-app/src/supervisor.rs`)

#### Scenario: Without a supervisor the next start takes the raw report
- **WHEN** a child faults with no supervisor gathering it and the next start absorbs the raw report
- **THEN** `kt-app-crash.txt` exists with the summary and the run's journal lines, the raw report is gone, and the report is taken unmarked (the bell announces it)
- **TEST** t2085 (`src/apps/kt-app/src/supervisor.rs`)
