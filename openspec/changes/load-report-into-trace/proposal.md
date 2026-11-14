## Why

The owner (kkt-26.11.14, registry row M3): "the load report moves into the trace; `load-report.txt`
goes; the button opens the trace folder". found-defects 487: the load report's header — every
adapter, the cards the monitors hang off, the power choice, the wall-clock time of the run — lived
only in `load-report.txt` (`load_watch.rs` `header()`), which nothing reads; its body was already
the telemetry journal's records. The owner diagnoses from the structured trace (`kt-cli trace`), so a
fact that is only in a text file is a fact he never sees.

## What Changes

- The machine's facts go into the trace as `MACHINE` text records (the form `gpu.<role>.*` and the
  processor/memory/OS basics already use, repeated at every segment's head): `adapter.<i>` (name /
  kind / backend / PCI ids), `monitors`, `power` — from `kt_gpu::trace_machine`, called by the load
  watch's thread at its start (kt-app) and by `kt-cli --trace`; `clock` — the run's start in local
  time with its offset, and `ffmpeg` — the FFmpeg major the run loaded, until now only in `kt-app.log`'s
  run header (kt-app). No new `Ev` number: these are facts of a run, not events.
- `kt-cli trace machine` names the run and its start (UTC from the segment head) before the facts;
  `summary` ends with the same lines; `dump` starts with one JSON line `{"machine":{…}}`.
- `load-report.txt` is no longer written: the hot-spell writer, `write_now`, the header and the
  report body go; `Journal::since` with them.
- The Diagnostics load section's button "Open the load report" becomes "Open the trace folder"
  (`diag.trace_folder`), opening `<data>/trace` (made if absent); five catalogs, the CJK subset.
- The journal pane's "open folder" hint no longer promises a load report.

## Capabilities

### Modified Capabilities
- `telemetry`: the load report requirement is removed; a new requirement puts its header into the
  trace; the uncounted-work requirement no longer names the report.
- `ui-timeline`: the UI frame's cost goes to the journal's LOAD record, not the report.

## Impact

`kt-gpu` context (`trace_machine`, `machine_facts`), `kt-app` load_watch / tele_journal / app
(Diagnostics) / i18n, `kt-cli` main and trace_query, live `idle`. Tests t2570–t2573; t1967 removed
with its requirement (owner's decision kkt-26.11.14 M3).
