# Tasks

Tests t2570–t2589 are reserved by this change (used: t2570–t2573); found-defects rows 590–599.
Every stage ends with a commit (English, the first line = the stage and what was done).

## 0. Records

- [x] 0.1 The owner's answer is in `docs/user-feedback.md` (round 2026-11-14, kkt-26.11.14); this change's artifacts written; `openspec validate load-report-into-trace --strict` green

## 1. The header into the trace

- [x] 1.1 `kt_gpu::machine_facts` (pure) and `trace_machine`: `adapter.<i>`, `monitors`, `power` as trace machine facts; t2570
- [x] 1.2 kt-app: the load watch's thread calls `trace_machine` at its start; `tele_journal::install` records `clock` (local start with milliseconds and offset) and `ffmpeg`; t2571
- [x] 1.3 kt-cli: `--trace` records the devices; `trace machine` names the run and its UTC start; `summary` ends with the machine; `dump` starts with it; t2572

## 2. The txt goes, the button opens the trace folder

- [x] 2.1 `load_watch.rs`: no report file, no hot-spell writer, no `write_now`/`header`/`report`; t1967 removed with its requirement; t2021 reads the journal's tail; `Journal::since` removed
- [x] 2.2 Diagnostics: "Open the trace folder" (`diag.trace_folder`) opens `<data>/trace`; the five catalogs; the journal pane's hint; the CJK subset rebuilt; t2573
- [x] 2.3 Live `idle`: the button asks for the trace folder, `kt-cli trace machine` on it names the adapters, monitors, power and clock, no `load-report.txt`; `journal.py`'s docstring

## 3. The class and the records

- [x] 3.1 The class walk (design.md): every file the program writes, who reads it, whether its facts are in the trace; FFmpeg's major (only in `kt-app.log`'s header) added as a trace fact
- [x] 3.2 found-defects 487 closed, registry M3 moved, whats-new line, comments that named the load report (kt-gpu, kt-time work, kt-app CLAUDE.md)
- [x] 3.3 Tests of the touched crates, clippy, target-prune; archive
