# Proposal: segv-crashes

## Why

The owner, 2026-11-08 (`docs/ktprompts/kkt-26.11.08.txt`, rows H4 and H5 of its registry): "debug
сборка как-то обильно падает на SIGSEGV" and "краш скудный". His crash folder
(`fails/26.11.08/new/kt-app-crash.txt`, build kt26.11.08-b614203c) holds four lines: the version,
the time, "the editor was ended by SIGSEGV (segmentation fault), 11", and a log tail that is the
run's header line alone. Nothing says which thread died, in which library, doing what. The same
stage's `finish.sh` saw t985 die by SIGSEGV now and then (found-defects 132), and `kt-cli export`
without a display dumps core every time (found-defects 131).

The mechanism is a class, not one crash: the process dies by a native signal, inside code that is
not ours to unwind (a GPU driver, the Vulkan loader, FFmpeg, ALSA, a thread left running at exit),
and the program has no record of the moment — the panic hook never runs for a signal, and the
supervisor can only name the signal it saw.

## What Changes

- A native-crash handler (new crate `kt-crash`, Linux): on SIGSEGV, SIGBUS, SIGILL, SIGFPE and
  SIGABRT it writes, async-signal-safely, a raw report — the signal, its code and address, the
  faulting thread's name and id, its program counter and stack (the unwinder's frames, or the code
  addresses on the stack when the unwinder stops at a driver without unwind tables), every thread's
  name, and the executable mappings — then re-raises, so the process still dies by its own signal.
- The report is symbolized in a healthy process from the binaries' own line tables (the dev
  profile keeps `debug = 1`): the editor's supervisor, right after the crash; the next start when no
  supervisor took it; kt-cli's own `crash-report`, started by the handler. The editor's report
  carries a one-line summary (signal, thread, function), the stack with files and lines, the other
  threads, and the run's last telemetry-journal lines; the crash window shows the summary and those
  lines; the journal's "the previous run crashed" record carries the summary.
- Causes found and closed, each with a regression test: the Vulkan loader raced by devices opened
  at once on threads (row 131) — one wgpu instance per process, devices opened one at a time; the
  load watch re-creating a whole instance every five seconds of a hot spell; the rest as design.md
  records.

## Impact

- New crate `src/libs/kt-crash` (libc, addr2line, gimli, object, memmap2 — already in the lock's
  cache); kt-app and kt-cli depend on it.
- `kt-gpu::context`: `instance`, `opening`, `instances_created`; `adapter_facts` is cached.
- kt-app: `supervisor::gather` takes the child's pid; `journal::{install_crash_handler,
  absorb_native_report, report_summary}`; `tele_journal::run_tail`.
- kt-cli: hidden commands `crash-report <raw>` and `crash-probe`.
- Capabilities: `durability` (the native-crash report), `export` (a headless export needs no display).
