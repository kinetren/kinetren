---
id: F-HLP-035
title: One local diagnostics journal — load, playback and what the person did, on one clock
status: in-progress
priority: B
cost: M
category: help/errors
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-app, kt-engine]
invariants: [6, 12]
ui_rules: [10, 13, 16]
round: null
point: null
tests: []
depends_on: [F-RES-010]
trackers: []
services: []
checked: 2026-11-07
---

# F-HLP-035 — One local diagnostics journal — load, playback and what the person did, on one clock

## Essence

A person who met a stall wants to say "it happened HERE, right after I opened the effects window" and
hand over one file. For that the program keeps, on his machine, one journal in which every record
carries the wall-clock time of the moment it happened: the load of each busy second, each playback
with its frame rate and underruns, the warnings, and his own actions (an edit by name, a window opened,
play, a save), plus a mark he can drop himself. It is not usage tracking: nothing leaves the machine.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| Vegas Pro | 2026 | ErrorReport folder, `*_x64.log` files | per-subsystem logs (file IO, GPU video) under `%LOCALAPPDATA%\VEGAS Pro\<ver>`, plus a crash report the Problem Reporting Service bundles; no record of what the person did, no load over time |
| Premiere | 26 | Events panel | a panel of warnings and errors (mostly third-party plug-ins) with Details and Clear All; users report they cannot copy it or find a file behind it |
| DaVinci Resolve | 21 | Help › Create Diagnostics Log on Desktop | zips the program's logs and a system description into one dated archive on the desktop; support asks for the whole archive and does not read OS logs |
| Final Cut Pro | 12 | no own journal | crash reports go to the system's Console / DiagnosticReports; the program shows no history of its own load |
| After Effects | 26 | Help › Enable Logging | an opt-in verbose log for support, switched on before reproducing the problem |

**Outside the core.** The "mark the moment" half comes from recording tools, where the person, not
the program, knows when the interesting thing happened.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| OBS Studio | 31 | log per session + "Add chapter marker" hotkey | every log line carries a time from the session start; a hotkey drops a chapter marker into the recording (Hybrid MP4/MOV only) |
| Windows Steps Recorder | (deprecated 2023) | psr.exe | records each click of the person with a time and a screenshot, with a comment button; removed without a replacement that logs |

## The trick behind it

1. **The person's own mark is the anchor.** OBS's chapter hotkey and Steps Recorder's comment exist
   because the person, not the program, knows which moment matters. A journal without a mark makes
   the person reconstruct the time from memory — exactly the owner's "НАВЕРНОЕ 487 s" (kkt-26.11.07b).
2. **Resolve's archive works because support asks for ONE thing.** One file, made by one menu item,
   dated in its name. Several separate logs (Vegas) turn every report into a question "which one?".
3. **Clock time, not "seconds ago".** A report stamped relative to its writing cannot be matched with
   what the person remembers ("after lunch", "when I opened the window"), nor with a second report.

## Pros

- One file answers "what was the program doing, and what had I just done" for any moment, also after
  a crash, because each record is in the file before the next thing happens.
- A mark turns "somewhere around 487 s" into a line the reader finds by searching for MARK.

## Cons

- A journal of actions is a record of the person's work: it must say what it never holds (media,
  paths, project content) and must not be readable by the usage telemetry.
- Written while busy, it costs a write per second; at rest it must cost nothing, or it becomes the
  load it was made to report.

## What users say

NO QUOTES: searched (2026-11-07) Resolve's forum on "Create Diagnostics Log", Adobe's community on
the Events panel log location, Vegas forums on log files, OBS forum on timestamped markers; found
support instructions and a user who "found no way to copy text from the Events panel", not a quote of
two sentences with a stable link that names the journal's time as the point. The owner's own words
are the feedback here: "сделал отложенную запись и я не могу тебе сказать точное время" and
"телеметрия != трекинг использования в posthog" (`docs/ktprompts/kkt-26.11.07b.txt`).

## How it fits Kinotrain

CAPABILITY: openspec/specs/telemetry/

### What already exists

- `src/apps/kt-app/src/load_watch.rs` — the per-second load by thread and the work counters
  (`kt_time::work`), kept a quarter of an hour in memory; `load-report.txt` stamped "N s ago".
- `src/libs/kt-engine/src/flight.rs` — the playback black box: a deduplicated ring of PLAY/PAUSE/SEEK/
  WRAP/SNAP/STALL, in memory, dumped into a crash report or by a Diagnostics button.
- `src/apps/kt-app/src/journal.rs` — `kt-app.log`, per run, local time with its offset, warnings only.
- `src/apps/kt-app/src/telemetry.rs` — `telemetry::signal`, the one door every edit, undo, save and
  project open already passes (counted for PostHog, never narrated).
- The Diagnostics log (`PerfState::note` in `app.rs`) — memory only, stamped at the next UI frame.

### What is missing

One file with all of them on one clock, written at the moment; the person's actions; a mark.

### What hinders

Invariant 6: the flight ring is written by the render thread, so the journal must take its records
without disk I/O on that thread. Invariant 12 / "the program at rest costs nothing": the journal may
not tick.

## Estimate

| What | Answer |
|---|---|
| Scope | M — one change, three stages |
| Where the main work is | kt-app (the journal, Diagnostics, the doors), kt-engine (a sink on the flight ring) |
| Project format | does not touch |
| Tests that will have to be written | event-time stamp with a fake clock; an action by name; survives a kill; rotation; no wake-ups at rest; live scenario |
| Depends on | F-RES-010 |
| Breaks compatibility | no |

## Verdict

Take: change `telemetry-journal` (kkt-26.11.07b G6, G10). One local journal written at the moment
with clock time, the load report as its export, the person's mark by a key and a Diagnostics button.

TEST: tele_journal::t1960_records_carry_the_event_time — fails today: the only file of this kind, `src/apps/kt-app/src/load_watch.rs`, stamps each second "N s ago" relative to the moment the report is written, and the Diagnostics log (`PerfState::note`) stamps a problem at the next UI frame, which on a pause may be minutes later.

## Sources

- https://help.colourlab.ai/hc/en-us/articles/26623983380247-How-to-Generate-Davinci-Resolve-Log (access date 2026-11-07)
- https://forum.blackmagicdesign.com/viewtopic.php?p=1142506 (access date 2026-11-07)
- https://helpx.adobe.com/ca/premiere/desktop/edit-projects/correct-mistakes/view-warnings-errors-and-information-events.html (access date 2026-11-07)
- https://community.adobe.com/questions-729/where-are-events-logs-stored-1397680 (access date 2026-11-07)
- https://www.vegascreativesoftware.info/us/forum/where-can-i-find-the-vegas-log-files--143379 (access date 2026-11-07)
- https://spotlightfx.com/blog/obs-chapter-extractor-for-premiere-pro (access date 2026-11-07)
- https://www.osnews.com/story/137862/microsoft-deprecates-even-more-windows-features-steps-recorder-gets-the-axe/ (access date 2026-11-07)
