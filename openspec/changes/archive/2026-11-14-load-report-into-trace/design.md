## Context

`load-report.txt` (idle-cpu, then telemetry-journal) was the journal's last quarter of an hour under
a machine header, written by itself after three seconds of a whole core and by a Diagnostics button.
Since telemetry-journal its body is the journal's own records; since structured-telemetry every
journal line is mirrored into the trace. What stayed only in the txt was the header
(found-defects 487): every adapter, the cards that drive the monitors, the power choice, and the
wall-clock time.

## Decisions

### D1. Machine facts, not events
The header's facts are properties of a run, not moments. The trace already has the form: `MACHINE`
text records (`key=value`), repeated at the head of every segment so a segment read alone still
names its machine, deduplicated by the reader into `Run::machine`. `gpu.<role>.*` (a device a role
opened) and `cpu`/`cores`/`memory`/`os`/`kernel`/`arch` use it. The new keys: `adapter.<i>`,
`monitors`, `power`, `clock`, `ffmpeg`. So no `Ev` number is taken (the parallel changes add events; nothing to
renumber here).

### D2. Where they are recorded
`kt_gpu::trace_machine` lists the adapters (`adapter_facts`, once per process: the first listing
loads every driver) and reads sysfs. In kt-app `app::run` calls it right after the listing it already
makes on the main thread before eframe opens its instance (segv-crashes: no listing may race eframe's
instance at start, so the load watch's thread — the first place tried — is the wrong one: it ran
before the trace was installed and recorded nothing, which the live `idle` scenario caught). The
segment heads then carry the facts for the whole run. kt-cli calls
it right after `--trace` installs the trace. `machine_facts` is the pure part (t2570).

### D3. The wall clock
The trace's times count from the process start; the segment head carries the start in Unix ns.
`kt-cli trace machine` prints it as UTC (`started:`); kt-app adds `clock` — the same moment in local
time with milliseconds and the offset (`localtime_r`), which is what the owner reads (t2571).

### D4. The button
The Diagnostics load section's button opened the data folder after asking for a report; now it opens
`<data>/trace` (created if a run recorded nothing yet), the folder a complaint hands over. The journal
pane's own "open folder" button still opens the data directory (the journal, `kt-app.log`).

## Class walk: what the program writes that nothing reads, facts only in text

Mechanism: a file written for a person to hand over, whose facts are not in the trace the owner's
diagnosis reads. Checked every file kt-app, kt-cli, kt-engine and kt-time write:

| File | Read by | Facts in the trace? | Verdict |
|---|---|---|---|
| `load-report.txt` | nothing | header: no | **fixed here** (removed, header → trace facts) |
| `telemetry-journal.log` | the crash window (`run_tail`), the supervisor's gather | every line mirrored (`trace::journal_at`) | stays: the person-readable journal |
| `kt-app.log` (+fan) | the crash window, supervisor | lines reach the journal → trace | stays |
| `<project>.ktflight.log` ("Save the playback journal") | the owner hands it over (R.28 item 10, user-feedback R.43/R.63) | the black box is `Ev::Play` + journal PLAY lines | same shape, **kept**: a button set at his request; not changed without a new one |
| `render.log` (+`.1`) | the render window's "open log" | export lines reach the journal | stays |
| panic / native crash reports | supervisor, next start | — (a dying process) | stays |
| `<dump>.load.json`, `<dump>.crash.json`, geometry dump | live scenarios | bench only | stays |
| telemetry id/seq, update-check state, user store, project lock, FFmpeg download | the program | not diagnosis | stays |
| `kt-cli trace … chrome <out>` | Perfetto | derived | stays |

Facts living only in a text file: the load report's header (fixed); the journal's RUN header (version,
OS, pid) — in the trace's RUN label and basics; `kt-app.log`'s run header (FFmpeg major, local time)
— its FFmpeg major was a fact only there; it is now the trace fact `ffmpeg` too (the header stays: the
crash window reads it).

## Risks

- A test machine with no adapter lists none: `monitors`/`power` still recorded.
- A run that never starts the load watch (the supervisor) records no adapter facts: it opens no
  device either.
