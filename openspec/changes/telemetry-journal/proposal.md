# Proposal: telemetry-journal

## Why

The owner, 2026-11-07, on build kt26.11.07-61414dba (`docs/ktprompts/kkt-26.11.07b.txt`, rows G6 and
G10 of its registry, found-defects 109): "агент который делал лог - не думал и сделал отложенную запись
и я не могу тебе сказать точное время"; "Ты как поймешь что я делал в load-report.txt? ... Не
догадался сделать общую телеметрию - взять playback journal и втроить в журнал телеметрии, куда и
load-report и playback и действия будут входить"; "телеметрия != трекинг использования в posthog".
And line 17 (G12): the Diagnostics log shows only when the window is made taller.

The mechanism, not the report: diagnostic records are kept apart, stamped late or relatively, and lack
what the person did, so a complaint cannot be tied to a moment and an action. `load-report.txt` says
"N s ago" relative to its writing; the Diagnostics log stamps a problem at the next UI frame (since the
4 Hz tick went, on a pause that is whenever the mouse moves next) and in UTC without saying so; the
playback black box lives in memory; `kt-app.log` holds warnings only; nothing records an action.

## What Changes

- One local telemetry journal, `telemetry-journal.log` in the data directory, never sent anywhere and
  never read by the usage telemetry. Every record carries the local wall-clock time with milliseconds
  and its offset, plus the seconds since the process started, taken at the moment of the event by the
  code that saw it. A writer thread blocked on a channel appends each record at once (no user-space
  buffer); a panic flushes it; past 2 MiB the file rotates into `.1` and `.2`.
- Into it: each busy second of the load watch (threads and work counters, "NO COUNTER MOVED") and the
  return to rest; the playback black box (play, pause, seek, loop, wrap, stall, underrun snapshots) as
  it is recorded, repeats collapsed; a playback summary at each stop (length, V-fps, new frames,
  skipped cells, underruns); the Diagnostics notes, now raised by the load watch at the second they
  happen; `kt-app.log` lines, notices, export log lines; and the person's actions: edits by command
  name and target, undo and redo, windows opened and closed, the effect selected in a chain, project
  open and save (file name only), and his own mark — Ctrl+Alt+M or the Diagnostics button "Mark the
  moment" with an optional note.
- `load-report.txt` becomes the export of the journal's last 15 minutes with absolute times; it is
  still written by itself after three seconds of a whole core.
- Diagnostics shows the journal's tail (newest at the bottom, with times) in a pane that is always on
  screen, above the fold, at any window height; "Open folder" opens the data directory.
- Notices and the Diagnostics log show local time instead of UTC.

## Impact

- `kt-engine` (`flight.rs`: a sink called when a record is pushed), `kt-app` (`tele_journal.rs` new,
  `load_watch.rs`, `journal.rs`, `app.rs` Diagnostics and the doors, `window_shell.rs`, `fx_editor.rs`,
  `render_dialog.rs`, `controls.rs`, five catalogs), `docs/guide/shortcuts.md`, `scripts/live/journal.py`.
- No project format change. The journal costs nothing at rest: no thread wakes while nothing happens.
- Capabilities `telemetry` (the journal) and `ui-reach` (the Diagnostics pane, the mark).
