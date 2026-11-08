# kt-crash

> The crate's contracts and mandatory tests — `openspec/specs/durability/spec.md` (the native-crash
> report); the reasons — `design.md` next to it.

The native-crash report (segv-crashes): a handler on SIGSEGV, SIGBUS, SIGILL, SIGFPE and SIGABRT
writes a RAW report at the signal; `symbolize` names its frames later, in a healthy process. Linux
only; elsewhere `install` is a no-op and `symbolize` returns its input.

## Invariants

- **The handler is async-signal-safe.** System calls, fixed static buffers, the libgcc unwinder
  (primed at `install`): no allocation, no formatting machinery, no lock of ours, no `std::fs`. A new
  line in the raw report is written with `Out`; a new source of facts is a system call, never a call
  into the program (its locks may be held by the dying thread).
- **The process still dies by its own signal.** After writing, the default action is restored and
  the signal re-raised to the thread: the supervisor's verdict, the exit status (139 for SIGSEGV)
  and a core dump are what they were without the handler (t2080, t2081, t2082).
- **Bounded.** An alarm of 20 s guards the handler: a hung unwinder or symbolizer ends the process
  by the ORIGINAL signal. The signal and thread lines are written first.
- **No `SA_ONSTACK`.** Rust's per-thread alternate stack is a few KiB, too small for the unwinder;
  a C library's thread has none. A stack overflow therefore ends without this report (the
  supervisor still names the signal) — design.md says why that trade is the right one.
- **Symbolize in a healthy process.** kt-app: the supervisor (`supervisor::gather`), or the next
  start (`journal::absorb_native_report`). kt-cli: the handler starts `kt-cli crash-report <raw>` by
  a raw `clone` + `execve` (libc's `fork` runs atfork handlers and takes locks).
- **The raw report is the exact record.** Frames without a line table stay `module+offset`, so a
  report from a build we no longer have is still exact for whoever has that build.

## Tests

- t2080, t2081 (`tests/handler.rs`): a child of the test binary faults / aborts in a named thread;
  the report names the thread, the function with file and line, the other threads.
- t2083 (`src/symbols.rs`): the raw format parses; module offsets; hash suffixes dropped.
- t2082 (kt-cli `tests/cli.rs`), t2084/t2085 (kt-app `supervisor.rs`): the two programs' paths.
- A test that crashes on purpose runs in a CHILD process, never the test process.
