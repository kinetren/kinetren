---
id: F-RES-004
title: What survives a crash: the recovery mechanism
status: done
priority: B
cost: M
category: resilience/recovery
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08, blender-5.2]
we_have: yes
crates: [kt-app, kt-engine]
trackers: [kde-bugzilla, kde-invent, pitivi]
invariants: [2, 6, 12]
ui_rules: [14, 16, 19, 20]
round: "R.49"
point: 353
tests: [357, 358, 359, 360, 361, 365, 366, 367, 368, 373, 374, 375]
depends_on: [F-HLP-014, F-PRJ-002, F-HLP-013]
checked: 2026-09-15
---

# F-RES-004 — What survives a crash: the recovery mechanism

## Essence

The program has vanished. The editor launches it again — and from then on what decides everything is not
what the program SAYS (that is F-HLP-014), but what it DOES: which file it
opens, what proves that the file is fresher than the others, and whether what was in memory a second before the end
is still on disk at all. Three different mechanisms: what survived
(the autosave and its moment), what is offered (the choice of snapshot at startup), and
what proves it (a comparison of dates, not the order of checks).

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Auto Save + `.bak` | a `.bak` lies next to the project; on the next launch NOTHING is offered — the file is opened by hand. The mechanism is passive: it keeps, but does not choose. Analysed in F-HLP-014 |
| Adobe Premiere | 26 | Auto Save (folder of copies) | numbered copies in a separate folder, the original is not touched; choosing a snapshot is entirely up to the person — by file name and date in Explorer. [NOT VERIFIED] — Adobe's pages did not open in the second wave either (F-HLP-014, "Sources") |
| DaVinci Resolve | 21 | Live Save + Project Backups | the only one of the core editors where choosing a snapshot happens on SCREEN, not in Explorer: right-click the project → Project Backups → a list of snapshots. But it appears on the person's request, not at startup after a crash |
| Final Cut Pro | 12.3 | (none) | there is nothing to restore and nothing to offer: there is no "Save" command, and "restore" means opening a second library from the backup folder |
| After Effects | 26 | Automatically Save Copies | copies in a subfolder next to the project; nothing is offered at startup. [NOT VERIFIED] for the same reason |

**Outside the core.** Without these two rows the topic cannot be described: in both programs
the recovery mechanism is not only documented but also visible in the public
tracker — that is, not only the construction is verifiable, but also whether it works
(`docs/features/_editors.md`, section "Software outside the core").

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Kdenlive | 26.08.0 | Automatic backup + Recover | a backup copy on EVERY save into a hidden `.backup`, tiered thinning (20/hour + 20/day + 20/7 days + 20 old ones); after a crash, when the same project is opened, it ASKS whether to recover, and separately there is a list of snapshots with dates. The only one that both offers on its own and keeps a list |
| Blender | 5.2 LTS | Recover → Last Session / Auto Save | the person makes the diagnosis: "quit by itself" — `quit.blend`, "crashed" — `Auto Save`, and the second item opens a file browser in the system temp. A wrong pick opens the wrong work |

## The trick behind it

The F-HLP-014 reconnaissance analysed HOW people are told about it (whether the person learns
about the recovery, who collects the diagnostics, where the report goes). Here — three
details of the mechanism itself that do not appear in the feature lists of anyone.

**1. Working recovery compares TWO DATES, rather than walking the paths in order.** The formulation is more than fifteen years old and is recorded verbatim in pitivi
#909 (2010): if the backup file's modification time is newer than the original — run the checks and ask; otherwise stay silent. Kdenlive goes further and keeps
a LIST of snapshots with dates, because "newer" is not always "what is needed".
The difference between comparing dates and an ordered fallback is invisible for exactly as long as
both paths point to the same thing, and it is total on the day they stop: in kde-invent #2189 (open since 15.05.2026) Kdenlive offers
recovery and misses with the file — "the files that kdenlive try to
open.. is not present (where the name is taken?)" — and yet the manual choice
from the list opens the same project.

**2. Recovery must work REPEATEDLY, and this breaks first.**
kde-invent #309 describes a cycle that nobody designs: you recover, forget to save at once, continue from where it crashed — crash again, and the second time there is no dialog any more. The first recovery consumed its own source. For us this is exactly rule 14 of the UI checklist ("works repeatedly"), and the cost of the error here is not "inconvenient" but "a second hour of work is gone entirely". The reverse side of the same design is pitivi #2028, open for nine years: a recovered
project remains unnamed, and the person is made to "Save
as…" every time.

**3. The panic handler is the only place where the program can rescue
what is already in memory, and almost nobody installs it, because it gets in the way of
debugging.** An autosave every N seconds limits the loss to N seconds only
if the process dies BETWEEN writes; a panic, however, happens at exactly the moment
when the freshest state exists only in RAM. The price of the
hook is a spoiled stack trace and an extra layer between the crash and the debugger,
which is why it is dutifully not installed; the gain is visible once in the user's life and is worth exactly as much as an unsaved edit.

## Pros

- Autosave that writes INTO THE PROJECT ITSELF (the Live Save pattern) makes recovery
  degenerate: there is nothing to choose from, the file on disk is the last truth.
- Atomic writing (temp + rename) plus a revision guard give one write point
  for three paths (background autosave, Ctrl+S, exit) — by design, an old copy cannot overwrite
  a fresh one after the fact, not by luck.
- An unnamed project goes into a SEPARATE recovery file rather than multiplying copies
  in the user's directory: in kde-invent #2189 the same task was solved differently and produced
  "a very huge amount of kdenlive files with a 0 (zero) lenght".
- Silent recovery removes Blender's class of errors: you cannot miss an item
  "Last Session / Auto Save" where there are no items.
- A successful "Save" deletes the recovery file, so there is no false offer to recover after normal work — and that is a separate defect at the neighbours
  (kde-bugzilla #424016, "Asking to recover project after saving the
  file").

## Cons

- The startup chain is an ordered fallback, not a choice: the first existing path wins, and nobody compares dates.
- Silent recovery deprives the person of both the choice and the occasion to check: if they were not asked, a wrong pick will be discovered an hour of work later.
- Autosave into the project itself means there is no rollback: a bad edit that survived 30 seconds
  is already on disk (in detail — F-HLP-014, "Cons"; version history as a separate subject — F-PRJ-002).
- The "black box" ring lives in memory and is laid out by a button in the "Diagnostics" window — that is, the flight recorder can be read only while the plane is flying.
- A single failure of the background thread switches autosave off until the end of the session, and from outside this looks like ordinary work.

## What users say

Offered to recover — and recovered the wrong thing; this is the most frequent way
the mechanism fails:

> it will ask me to recover some files and when i do that half of my stuff is
> gone but when i close Kdenlive, reopen it and open my project, it has all
> the stuff i did last time
> — kde-invent #418, "[Improvement] Improve Auto Recovery", 28.10.2019,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/418

Seven years later, in an open task:

> when kdenlive crashes or freezes on next launch opening the same project is
> asking for the automatic recovery.... but this seem not working.
> — kde-invent #2189, 15.05.2026,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/2189

On repeatability — the task statement in full:

> Get a crash again (normally the action that created the crash will be
> repeated). Open the project but now there isn't a dialog shown to recover
> the previous session
> — kde-invent #309, "Maintain the possibility to recover a crash after a
> recovery", 04.08.2019,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/309

On the comparison of dates — a design decision recorded in 2010:

> If that backup file has a modification time that is newer than the
> original, run sanity tests on the backup file and ask the user
> — pitivi #909, "Auto-suggest restoring the previous session when a backup
> file is present", 16.09.2010,
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/909

And the price of an unnamed recovered project, open for nine years:

> Pitivi crashes / freezes often. So I often have to restore the project from
> backup.
> — pitivi #2028, "Make the Save-as for restored autosaved projects more
> friendly", 29.04.2017,
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2028

SIGNAL: recovery after a crash — 30 tasks in kde-bugzilla, `recover|restore|after crash|lost.*work|autosave|auto.?save|backup`; a share of 0.5% against 0.7% in kde-invent

Three numbers, for which the note was opened.

**First: "it crashed" and "after the crash I lost my work" are quantities differing by two orders of magnitude.** In the same kde-bugzilla the filter for the crash itself
(`crash|segfault|freeze|hang|abort|not respond`) gives 1394 tasks out of 6346 =
22.0 %, while the filter for recovery gives 30 out of 6346 = 0.5 %. The mechanism is almost never written about: people recall it only when it has failed, and then one task
costs a working day.

**Second: the shares of two trackers of the same product nearly coincide (0.5 % against 0.7 %), but the COMPOSITION does not.** In kde-bugzilla this is 16 complaints
against 1 request, in kde-invent — 9 complaints against 5 requests. The same topic
sounds in the bug tracker like "it broke", and in the development tracker like "make the
mechanism different". Of the five kde-invent requests four are about WHAT exactly
gets into the snapshot and WHEN it is taken, rather than about frequency (#309 repeated
recovery, #418 the composition of the recovered content, #935 subtitles in the autosave, #1723
a copy before the render starts); the fifth, #925 about the layout of panels, is an honest
false match on the word `restore`, and it has to be named, not hidden.

**Third, from `--neglect`: the recovery mechanism gets fixed, but the crashes do not.**
Under the recovery filter, 3 tasks in kde-bugzilla and 3 in
kde-invent remain open, and among them exactly one is older than three years — and that one is about the layout of
panels too; under the crash filter there are 118 and 55 open, the oldest is 10.0 years old.
The share of genuine fixes among closed tasks (not "closed", but precisely `FIXED`) is 63 %
for recovery against 43 % for crashes, with 419 `WORKSFORME` in the crash
topic. The conclusion for us is direct: one cannot count on "the crashes will get fixed" —
but the part that survives them gets fixed, gets fixed quickly, and stays
fixed.

## How it fits Kinotrain
CAPABILITY: openspec/specs/durability/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
After a crash — an offer to recover from the recovery copy; `fn main` is minimal, the Drop boundary and the log.
ADDRESS: `src/apps/kt-app/src/app.rs:5270`–`:5288` — the class is possible, and nothing catches it: the recovery chain chooses the file by the ORDER of checks (`arg → last_project → autosave_path`), not by comparing times, `last_project` is never deleted (`app.rs:5183`), and no test touches `run()` — t194/t199 (`src/apps/kt-app/src/layout_tests.rs:3549`, `:4373`, `:4395`) check the WRITE and the revision guard, not the choice of snapshot at startup.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2 stages: (1) a panic hook that puts the ring and the unsaved model on disk, with clearing of the mutex poisoning; (2) choosing the snapshot by file time instead of the order of checks + handling `Err(Disconnected)` of `autosave_rx` |
| Where the main work is | kt-app (`main.rs` — installing the hook; `app.rs` — `run()`, `save()`, the `autosave_rx` branch), kt-engine (`flight.rs` — `lock()` without `unwrap`, the dump from the hook) |
| Project format | does not touch it |
| Tests that will have to be written | a unit test "a fresh autosave beats a stale `last_project`" on substituted mtimes; a unit test "a poisoned ring is still dumped"; an extension of t194 to "the autosave thread died — autosave keeps working"; a unit test for repeated recovery (rule 14) |
| Depends on | F-HLP-014 (the hook and its dump — the shared half, paid once), F-PRJ-002 (list of snapshots), F-HLP-013 (log) |
| Breaks compatibility | no |

This is a frame, not a spec: what exactly the hook saves (only the ring or also the model), the
file name and the behaviour when times are equal — are the subject of the round.

## Verdict

Take it, but as TWO cheap fixes and one decision, not as a new subsystem.
Gaps 2 and 3 are a few lines each, and both silently lose work today:
the startup chain that chooses by order rather than by date, and autosave that switches off
until the end of the session after a single failure of the thread. Gap 1 is more expensive and requires
a decision about form (what exactly the hook has the right to read under invariants 2 and 6), but
without it all talk of "survives a crash" is literally wrong: today nothing survives a crash except the last write to disk.

Priority B, not A, deliberately: the mechanism we have exists and works — autosave,
atomic writing, the revision guard, recovery of an unnamed project. What is missing is not a
feature, but three properties of this feature, and the trackers show that exactly such
properties get fixed (63 % genuine fixes against 43 % for the crashes themselves).

## Sources

- kde-invent #418, "[Improvement] Improve Auto Recovery", 28.10.2019 —
  https://invent.kde.org/multimedia/kdenlive/-/work_items/418
- kde-invent #2189, "[windows?] recovering is not opening the project.. from
  recover backups it works.", open since 15.05.2026 —
  https://invent.kde.org/multimedia/kdenlive/-/work_items/2189
- kde-invent #309, "Maintain the possibility to recover a crash after a
  recovery", 04.08.2019 —
  https://invent.kde.org/multimedia/kdenlive/-/work_items/309
- kde-invent #705, "When a project RECOVERY fails don't "create" an empty
  project with the real project name.", 28.05.2020 —
  https://invent.kde.org/multimedia/kdenlive/-/work_items/705
- pitivi #909, "Auto-suggest restoring the previous session when a backup
  file is present", 16.09.2010 —
  https://gitlab.gnome.org/GNOME/pitivi/-/work_items/909
- pitivi #2028, "Make the Save-as for restored autosaved projects more
  friendly", open since 29.04.2017 —
  https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2028
- kde-bugzilla #424016, "Asking to recover project after saving the file",
  09.07.2020 — https://bugs.kde.org/show_bug.cgi?id=424016
- kde-bugzilla #348674, "Crash Recovery only works properly for unsaved
  projects", 03.06.2015 — https://bugs.kde.org/show_bug.cgi?id=348674
- The numbers are reproducible on dumps from 08.09.2026:
  `python3 src/scripts/tracker-rank.py --query 'recover|restore|after crash|lost.*work|autosave|auto.?save|backup'`
  (kde-bugzilla 30/6346 = 0.5 %, kde-invent 15/2217 = 0.7 %) and
  `--query 'crash|segfault|freeze|hang|abort|not respond'`
  (kde-bugzilla 1394/6346 = 22.0 %). The closing profile and `--neglect` were read by the
  same code (`is_fixed`, `years_open`) over the same dumps.
- **Not all trackers were exported.** As of the date of writing the cache held
  `kde-bugzilla` (6346), `kde-invent` (2217) and `pitivi` (the dump was being re-downloaded
  during the work and does not take part in the note's numbers); the nine others from the
  `_trackers.md` registry, including all of GitHub, were still downloading. All counting claims rest on the two STABLE dumps — kde-bugzilla and kde-invent.
- Kdenlive 26.08 and Blender 5.2 — the mechanism is taken from `docs/features/_editors.md`
  (section "Software outside the core"), not checked live, as decided there.
- The Premiere and After Effects rows are marked `[NOT VERIFIED]`: Adobe's help did not open in either the first or the second wave (F-HLP-014, "Sources").
- Our code is the primary source for all claims about "us":
  `src/apps/kt-app/src/app.rs`, `src/apps/kt-app/src/main.rs`,
  `src/apps/kt-app/src/layout_tests.rs`, `src/libs/kt-engine/src/flight.rs`,
  `src/Cargo.toml`, items 96, 150, 175, 197, 199, 200 of
  `docs/porting-notes.md`.

**Implemented in R.49** — docs/porting-notes.md item 353
(tests t357, t358, t359, t360, t361, t365, t366, t367, t368, t373, t374, t375); the feature's later history is there.
