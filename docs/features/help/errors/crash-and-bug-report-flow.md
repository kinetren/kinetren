---
id: F-HLP-014
title: Recovery after a crash and problem report
status: candidate
priority: B
cost: M
category: help/errors
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2, jetbrains-2026.2, vscode-1.136]
we_have: partial
crates: [kt-app, kt-engine]
invariants: [2, 6, 12]
ui_rules: [13, 14, 17, 19]
round: null
point: null
tests: []
depends_on: [F-HLP-013, F-HLP-026, F-HLP-033]
trackers: [kde-invent, kde-bugzilla]
checked: 2026-09-08
---

# F-HLP-014 — Recovery after a crash and problem report

## Essence

The program has disappeared from the screen. The editor launches it again and wants to learn three
things, in this order: whether the work is intact, what exactly happened, and what to send to the
person who is fixing it. The three things are solved by three different mechanisms — autosave, a log
of the last minutes, and collection of diagnostics — and the failure of any one of them looks the same
to the person: "the program pretended nothing had happened".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Auto Save + a `.bak` file | Autosaving is enabled in the preferences; a `.bak` lives next to the project; after a crash the program does NOT restore by itself — the file is opened by hand. A 2026 forum thread: a crash in the middle of a write wiped the `.veg` itself, and the person was saved by the `.bak`. The exact text of the restore dialog at the next launch could not be confirmed — [NOT VERIFIED] |
| Adobe Premiere | 26 | Auto Save (a folder of copies) + crash report | Autosave writes NUMBERED COPIES into a separate folder next to the project; the original is untouched; restoring means opening the needed copy by hand. On a restart after a crash, sending a report to Adobe is offered. Both halves — [NOT VERIFIED]: `helpx.adobe.com` serves the Auto Save page only through a JS shell, the text cannot be retrieved (only the branch `premiere/desktop/…/correct-mistakes/` was available) |
| DaVinci Resolve | 21 | Live Save + Project Backups + Timeline backups | Closest to us: **Live Save** writes the edits into the project itself continuously, so "save" stops being an event; in parallel SEPARATE backups go to a chosen folder ("Backup location"), and restoring is a right-click on the project in the Project Manager → Project Backups → choosing a snapshot. Three switches in the preferences: Live save, Project backups, Timeline backups |
| Final Cut Pro | 12.3 | automatic library backups | There is no "Save" command at all. "Final Cut Pro automatically backs up the library at regular intervals"; only the database goes into the backup, not the media; by default the folder Final Cut Pro Backups in Movies, and backups older than a few days are deleted automatically. Restoring: File → Open Library → From Backup, a choice by time and date; the backup arrives as a SECOND library next to the current one, rather than replacing it |
| After Effects | 26 | Auto-Save (a folder of copies) | The device is the same as Premiere's — a separate folder of auto-saved copies, restoring by hand. [NOT VERIFIED]: Adobe's pages about automatic save and recovery did not open either from the main domain or from the `/in/` mirror |

None of the five has "send a report" as a button in the editor: Premiere hands this to a separate
crash report, Blackmagic and Boris FX to the forum, Apple to the operating system's Feedback Assistant.

**Outside the core.** The mechanism "one action collects diagnostics and pre-fills the report" is absent
from video editing as a class; all three rows below are the origin of specific details that the five core
editors do not have: a log that the crash ITSELF writes; one command that assembles an archive of logs;
and the step "show what will go out, and press the button yourself".

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | File → Recover → Last Session / Auto Save; Help → Report a Bug | Restoring is split into TWO items: `Last Session` loads `quit.blend`, written at EXIT ("if, for example, you closed Blender by accident"), while `Auto Save` opens a file browser in the system temp. On a crash Blender itself writes a text `blender.crash.txt` (on Linux — in `/tmp`) with a log of the tools used. `Report a Bug` opens a BROWSER with the fields already filled in: the machine configuration, the OS, the specific build |
| IntelliJ IDEA | 2026.2 | Help → Collect Logs and Diagnostic Data | One menu command collects a `.zip` with ALL logs, including build logs and automatic thread dumps. It is separately documented where to get the log by hand, "when the IDE does not start and the Help menu cannot be reached" — that is, the scenario "the program is not there" is provided for explicitly |
| Visual Studio Code | 1.136 | Help → Report Issue | The report is composed INSIDE the program (version, platform, the list of extensions are filled in automatically), but it is SENT not from the program: a preview opens, and the person sends it with his own account, to GitHub. Everything that goes outward is opt-in and is shown before sending |

## The trick behind it

The payload that a status-bar line cannot hold here is **THE PAST**. A hint answers the question "what is
this" about an object that is now under the cursor; after a crash there is no object at all, and what is
needed is an account of the last minutes — what the program was doing when it ceased to exist. Hovering the
mouse over a minute ago is impossible at any line capacity. For us this payload goes into a FILE that is
written without asking at the moment of the crash, and into ONE line at the next launch, which names the
time and the place.

**1. The value of autosave is determined not by its frequency, but by whether the person learns about it at
the right second.** The spread across the industry is maximal here, and it is not about the interval. Final
Cut says nothing, because there is nothing to say: there is no "Save" command in the program, and "restoring"
is an ordinary opening of a second library from a folder. Blender, on the contrary, demands that the person make
a diagnosis BEFORE restoring: "I quit myself" is `Recover → Last Session` (`quit.blend`, written on exit), "it
crashed" is `Recover → Auto Save`, and the second item opens not a file but a FILE BROWSER in the system temp.
Pick the wrong item — and you get the wrong work. Our case is the third: we restore silently and automatically,
and one line `msg.autosave_restored` is the only thing that says so. A silent restore and an explicit "autosave
from 14:32 restored, the file is here" are different products, and the difference shows not when everything is
fine, but when the autosave turned out to be OLDER than the person thought.

**2. "Collect diagnostics" is valuable not for its contents, but for taking away from the person the task of
working out what exactly to attach.** The cost of collecting by hand is measurable: the VEGAS community keeps a
mandatory list of ten items pinned to a post (analysed in F-HLP-026), while IntelliJ spends one menu item on the
same thing — `Collect Logs and Diagnostic Data` — and produces a `.zip`. Blender takes a middle position, and it is
the most instructive: it does not assemble an archive, it PRE-FILLS the form — "automatically populate a number of
fields … including your computer configuration, operating system, and specific Blender build" — that is, it saves
not bytes but the decision "what is important here". There it is also honestly noted that on a crash BEFORE the
start this path does not work; that is why the crash log is written separately and without the participation of
the program-in-consciousness.

**3. A report that goes out silently is a question of trust, so nobody lets it go out silently.** VS Code assembles
the content itself, but the person sends it, with his own account, after seeing the preview. Blender opens a browser,
not a socket. Apple leads to Feedback Assistant. For us this question was settled in advance and more strictly than
for anyone: there is not a single network client in the tree (`Cargo.lock` — zero occurrences of `reqwest`/`hyper`/
`tokio`), and "send a report" can only mean "put a file and say where it lies". This is not poverty: three of the five
core editors do exactly the same, only for them it is a consequence of the organization of support, while for us it is
a property of the program.

## Pros

- An autosave that writes INTO THE PROJECT ITSELF (like Resolve's Live Save) makes restoring trivial: there is nothing
  to restore, the file is already fresh. The price of this choice is the absence of a version history, but none was promised.
- An atomic write (temp + rename) closes exactly the scenario on which the VEGAS forum loses projects: "crashed in the
  middle of saving". An interrupted write leaves the old file intact, rather than a truncated new one.
- The revision guard (`write_project_guarded`) makes a single point of writing the project: the three paths — background
  autosave, Ctrl+S and exit — cannot overwrite each other after the fact. This is rare even for large editors.
- The "black box" is already written and already compact: a ring of fixed size, deduplication with a counter, a snapshot
  of counters only when the deltas are non-zero. It does not need to be invented — it needs to be taught to survive the
  event for which it was created.
- A report as a file instead of a report over the network removes a whole class of questions: nothing to agree on, nothing
  to anonymise in the path, nothing to fix when there is no network.

## Cons

- An autosave into the project itself means that a mistake has no rollback: an unsuccessful edit that survived 30 seconds
  is already on disk. Resolve has Project Backups next to Live Save precisely for this reason; we do not have the second half.
- The "black box" records ONLY the transport: `Play`, `Pause`, `Seek`, `Region`, `Wrap`, `Dir`, `Reset`, `Snap`, `Stall`.
  Model edits, file openings, errors and exports are not in it — so it does not answer the question "what was the person
  doing when everything ended", but only "how was it playing".
- The log file is written by a button, and the button lives in the program. A ring in RAM is a flight recorder that can be
  read only while the plane is flying.
- Three logs (notifications, the performance panel, the "black box") survive neither a crash nor a change of project, and
  only one of the three can be exported outward (F-HLP-013).
- Collecting diagnostics by hand is more expensive for us than for VEGAS: that one at least has an "About" window from which
  the version is copied (F-HLP-026), while for us the version is shown nowhere in the release except `kt-cli`.
- A price that was not in the note and was shown by the tracker: our distribution is the most backtrace-unfriendly configuration
  possible. `[profile.dist]` (`src/Cargo.toml:63`) sets `strip = "symbols"`, and this is exactly what kde-invent #1535 (Flatpak)
  and #1560 (Mac without symbols) complain about. We have a mitigation and it is free — a panic hook does not need symbols,
  `PanicHookInfo` itself carries the file and the line — but it works ONLY for Rust panics. A segfault in foreign code (the
  system FFmpeg via dlopen under invariant 10, a GPU driver) gives neither a hook nor a line, and in this single class of
  crashes we will be exactly where the Flatpak build of Kdenlive is.
- We cannot distinguish "quit by itself" from "crashed" at all: the restore chain (`src/apps/kt-app/src/app.rs:5274`) runs
  identically in both cases, and there is no flag from the previous launch (the kde-invent #899 technique). The technique is
  cheap, however, and fits what is already written: `run()` creates a file, `on_exit` deletes it (`app.rs:5188`) — and since
  `on_exit` is not called on a crash, a leftover file IS the sign of a crash. Today, a silent restore of the autosave looks the
  same after a normal exit and after a catastrophe.

## What users say

A crash in the middle of an autosave is not a theory:

> This almost gave me a heart attack, had it not been for the .bak file…the
> project file was simply non-existent after that.
> — Mindmatter, thread "V26 crashed and deleted the .veg file!", Boris FX forum
> (VEGAS), 24.06.2026, https://forum.borisfx.com/t/v26-crashed-and-deleted-the-veg-file/24325

Further down the same thread — what people do when they do not trust the mechanism: one set up a "Save" button on a
Stream Deck, another on a side mouse button, a third saves "multiple steps of save". And a fourth:

> use advanced auto save it is pulled my butt out of the fire many time
> — joe.desaavedra, same thread, 24.06.2026

And this is about the second half of the task, "where is the thing that will save me": a user of Resolve 20.0.1 with
Live save, Project backups and Timeline backups switched on and an explicitly chosen folder does not know what exactly
he will see:

> If I right-click on the project name and select 'Project Backups' in the
> window that appears, should "only" my backups be listed?
> — Marco, thread "Project Backups", Blackmagic Design forum, 21.07.2025,
> https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=224766

The third wave gave what the vendors' forums could not give: a count.

SIGNAL: a crash log attached to a report — 80 tasks in kde-bugzilla, `crash (log|dump|report)|backtrace|stack ?trace|core dump|\bgdb\b|drkonqi`; a share of 1.3% against 0.3% in kde-invent

The difference is fourfold — not about the quality of the code, but about the presence of a MECHANISM. In the old
Kdenlive tracker 54 reports carry the label `drkonqi`: they were composed not by a person by hand, but by the KDE
system crash handler — it caught the signal, attached a backtrace and handed the person a ready form; all that was left
was to press a button. In the new tracker there is no such label at all, and of the six hits four are the maintainers'
own service tasks about the fact that a backtrace cannot be obtained in principle. One of them has been open for four years:

> Currently, it's not possible to get backtraces in multiple types of our
> releases, and even where it's possible to get backtraces, we're asking the
> users to run gdb manually.
> — kde-invent #1531, "Better crash logging (tracking task)", 28.09.2022,
> still open, labels `Crash Reporting`, `Code Quality`,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1531

How the same thing looks from the editor's side — in the same tracker, in a report about a crash on the
"Missing Clips or Files" dialog:

> After a few seconds kdenlive ends without any message or crash dump.
> — kde-invent #1437, 19.05.2022, label `Crash`,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1437

The neighbouring task #1535 ("Flatpak build doesn't show backtrace on SIGSEGV",
01.10.2022, open, https://invent.kde.org/multimedia/kdenlive/-/work_items/1535)
describes the extreme case — "No crash handler is triggered and nothing is
printed in the terminal" — and formulates the needed solution in the same words we
use ourselves: `ideally copied to a file`. This is exactly our distribution configuration,
and exactly our conclusion from "The trick behind it": a file, not a socket.

The same tracker also has a ready cheap technique for "finding out at STARTUP that the previous launch crashed" — an
empty flag file, created on the splash screen and deleted after a successful load:

> what about creating a "flag file" when the splash screen appears … and
> delete this file when the opengl and everything else is loaded successfully
> (timeline displayed)?
> — kde-invent #899, 29.12.2020, labels `Crash Reporting`, `Feature Request`,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/899

NO QUOTES: across the five CORE editors — VEGAS, Premiere, Resolve, Final Cut, After Effects — there is still no live feedback specifically about "collect diagnostics with one button", and the trackers will not fix this: none of the five has a public tracker (`_trackers.md`, section "Whom we do NOT take"), and a vendor's forum is not a tracker — there is no state, no deduplication and no resolution date in it. What was searched for and where on the forums is recorded in "Sources".

## How it fits Kinotrain

ADDRESS: src/apps/kt-app/src/main.rs — the class "a crash left nothing behind" is possible for us too, and nothing catches it: `panic::set_hook` does not appear in `src/` even once, `src/Cargo.toml:61` deliberately keeps stack unwinding (we do not set `panic=abort`), and `[profile.dist]` (`src/Cargo.toml:63`) carries `strip = "symbols"` — that is, in the release distribution there is neither a file nor symbols, exactly as in kde-invent #1535.

### What is already there

The strong half of ours is really strong, and this must be said directly.

- **Autosave.** `src/apps/kt-app/src/app.rs:1052` — every
  `AUTOSAVE_SECS = 30` (`app.rs:4095`), only after edits to the CONTENT, not during a gesture and not during playback
  (`autosave_due()`, `app.rs:2290`; the safety net — after two intervals of silence we write anyway). The snapshot is
  taken in the UI thread, serialization and writing go to the `autosave` thread (`app.rs:1071`).
  A named project is written INTO ITSELF, an unnamed one — to
  `$XDG_DATA_HOME/kinotrain/autosave-project.otio` (`autosave_path()`,
  `app.rs:4908`). The write is atomic (`write_project_atomic`, `app.rs:4939`:
  temp + rename) and under the revision guard (`write_project_guarded`,
  `app.rs:4924`). The final write is `on_exit` (`app.rs:5188`), waiting for the
  background one for up to 5 seconds. A successful "Save" deletes the recovery file
  (`app.rs:2794`). Tests 194 (`layout_tests.rs:3549`) and 199
  (`layout_tests.rs:4373`, `:4395`); items 96, 150, 197, 199, 200 of
  `docs/porting-notes.md`.
- **Restoring.** `run()` (`app.rs:5274`): a command-line argument → `last_project` from the eframe storage → and only if neither
  exists — the autosave of the unnamed project. The restored one stays unnamed (`a.project_path = None`) and says `msg.autosave_restored`
  (`app.rs:5283`).
- **The playback "black box".** `src/libs/kt-engine/src/flight.rs`: a ring `CAP = 512`, a record `Rec { at_ms, kind, a, b, snap, n }`,
  deduplication of consecutive repeats with a counter `n`, a snapshot of the counters once a second and only when the deltas are non-zero. Nine kinds of
  events (`Ev`), including `Stall` — "playing, but the playhead stands still". Only the render/prefetch thread writes it; it does not enter the cpal
  callback (invariant 6). The owner is `Player.flight` (`src/libs/kt-engine/src/player.rs:132`), the write points are `:360`, `:366`,
  `:700`–`:789`. Test 175 (`src/libs/kt-engine/tests/loop_fx.rs:349`), item 175.
- **The reachability of the log from the GUI — exists, and here it is.** The "Diagnostics" window (`draw_diagnostics`, `app.rs:5064`) is opened by the
  item `menu.view.diagnostics` (`toolbar.rs:340`) and by the GAUGE icon (`toolbar.rs:666`); inside is the button `perf.flight_save`
  (`app.rs:3772`), which calls `save_flight_log()` (`app.rs:1940`) and writes `<project>.ktflight.log`; the number of `Stall` events is shown right there in red.
- **The performance log** `PerfState::note` with `×N` deduplication, seven threshold warnings in plain human language, and the entry `ui stall N ms`
  when a frame of the UI thread is longer than 100 ms (`app.rs:5170`) — the only trace of "it hung".
- **Notifications and the bell** — F-HLP-013, not repeated here.

### What is missing

Checked by grep, not by memory.

1. **There is no panic handler at all.** `panic::set_hook`, `set_hook`, `catch_unwind` — ZERO occurrences in the whole
   `src/` (`--include=*.rs`). `fn main()` in `src/apps/kt-app/src/main.rs` is two lines. In `src/Cargo.toml:61` there is a
   deliberate decision "we do not set `panic=abort` — the semantics of panics stays the same as in the tested profiles", that is,
   stack unwinding DOES happen for us, and nobody has tried to catch it. Consequences: the ring of the "black box" dies together
   with the process; `on_exit` is not called, so the final write does not happen; the panic text goes to stderr, and when launched
   from `.desktop` — to the system journal, which the editor does not visit.
2. **The restore line says neither WHEN nor WHERE.** `autosave_restored` in all five catalogs (`en.toml:503` and its neighbours) is
   a value without a single `{0}`, although the substitution mechanism is used constantly nearby (`msg.saved` carries the path,
   `msg.flight_saved` carries the path). The person does not learn the time of the snapshot, nor that the file lies in
   `~/.local/share/kinotrain/`. This is also a direct violation of rule 19: the line does not name the place where the decision can be revisited.
3. **A stale `last_project` beats a fresher autosave.** `save()` (`app.rs:5183`) stores `last_project` only when `project_path` exists, and
   NEVER deletes it; `File → New` (`new_project`, `app.rs:2717`) makes the project unnamed (`new_empty`, `project_path: None`). Scenario:
   you open project A, do "New", work for an hour, crash. At startup the chain at `app.rs:5274` takes `last_project` = A (it exists) and
   never reaches the autosave at all. The hour of work lies in `autosave-project.otio`, is not deleted (deletion happens only after a
   successful "Save") and is not mentioned by a single line. There is no comparison of file times in the code.
4. **A dead autosave thread silently switches autosave off until the end of the session.** `self.autosave_rx = Some(...)` is set BEFORE
   `spawn(...).ok()` (`app.rs:1068`–`:1081`), and the collection of the result is `if let Ok(res) = rx.try_recv()` (`app.rs:1084`). A panic in the
   thread (or an unsuccessful `spawn`) drops the sender: `try_recv` returns `Err(Disconnected)` forever, the branch is not executed,
   `autosave_rx` is not reset, and `due` requires `autosave_rx.is_none()`. No message, no hint asterisk: rule 14 ("works again") is broken
   in the most expensive place of the program.
5. **There is no one-action collection of diagnostics.** The button dumps ONLY the transport ring; `flight.dump()` (`flight.rs:153`) writes
   the lines of events without a header — no version, no FFmpeg, no GPU (the header's contents are the subject of F-HLP-026), and no notification
   log or performance-panel data (F-HLP-013). Three logs glued into one file do not exist.
6. **The log file after a restore lands in an unknown place.** `save_flight_log()` for an unnamed project writes `kt-flight.log` into the CURRENT
   DIRECTORY (`app.rs:1943`), and the restored autosave is precisely unnamed (`app.rs:5282`). So exactly in the scenario "we are studying a crash" the
   log ends up in the launch directory — for `.desktop` that is the home directory or the root.
7. **There is no addressee, and by the design of the program there never will be one.** Network clients in the workspace: zero.
   `reqwest`/`ureq`/`hyper`/`tokio`/`curl` do not occur in any `Cargo.toml`, `Cargo.lock` contains none of them at all, and in
   `src/apps/kt-app/src` there is neither `TcpStream` nor `http://`. So "send a report" for us can mean exactly one thing: put a file and show where it is.

### What gets in the way

- **A panic can happen in a thread that HOLDS the mutex of the "black box".** `flight.rs` writes `self.ring.lock().unwrap()` in all methods;
  after a panic inside `push_at` the mutex is poisoned, and `records()`/`dump()` called from the hook will panic again — and a double panic is an
  `abort`, that is, exactly the loss of the file for whose sake the hook was created. The fix is one call (`unwrap_or_else(|e| e.into_inner())`),
  but it cannot be missed.
- **The hook is called on ANY thread.** Invariant 2 (FX state is touched only by the audio-render thread) and invariant 6 (no allocations or locks in the
  cpal callback) mean that the hook has no right to go into the engine for state: it may only read what already has its own synchronisation, and write a file.
- **The `KtApp` model is not reachable from the hook.** The hook is a global `'static` closure, while the project lives in `KtApp` and is reassigned
  as a whole (`*self = app`). Everything the hook can write must be put in advance into a shared place — the same `Arc<FlightRecorder>` or an atomic with the
  project path. This is a constraint of shape, not of volume.
- **Rule 13 (do not waste space) and rule 9.** A new surface must not be introduced: the restore line goes into the status bar and the bell, which already
  exist; the report goes into the body of the "Diagnostics" window, which is already opened on purpose (the log button lives there, and F-HLP-026 puts the build
  information there too).
- **The cost of the line in five languages** — F-HLP-033. Here it is small: the time and the path arrive as the arguments `{0}`/`{1}`, and 3–4 captions are translated.

### Which invariants it touches

- **2** and **6** — the boundary for the panic hook: do not touch the FX state and do not enter the audio path; read only what has its own lock.
- **12** (repaint only on movement, no allocations in draw paths): nothing animated is added, the file is written outside the frame, and the restore line is an
  existing channel that is drawn once per frame anyway.
- **Rule 13** — not a single new panel: the status bar, the bell and "Diagnostics" are already in place.
- **Rule 14** ("works again") — literally about defect 4: an autosave that switches off for good after one failure.
- **Rule 17** ("paired ways are identical") — three logs with three different life policies and one way of exporting for all three.
- **Rule 19** ("a dialog must say where the decision can be revisited later") — about defect 2: the restore message names neither the file nor the time.
- The argument about pop-up hints is not touched by this note: there are no new surfaces, and `theme.rs` and t83 (`layout_tests.rs`) stay as they are.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages of the round: (1) the hook and the dump on a crash; (2) choosing the freshest snapshot at startup and the restore line with time and path |
| Where the main work is | kt-app (`main.rs` — installing the hook; `app.rs` — the restore chain in `run()`, `save_flight_log`, handling `Err(Disconnected)` of `autosave_rx`), kt-engine (`flight.rs` — clearing the mutex poisoning, the header of the dump) |
| Project format | does not touch it |
| Tests that will have to be written | `reach_tests::recovery_says_where_and_when`; a unit test "a fresher autosave beats a stale `last_project`" by file time; a unit test "a poisoned ring is still dumped"; an extension of t194 to the case "the autosave thread died — autosave keeps working" |
| Depends on | F-HLP-026 (the report header: version, FFmpeg, GPU), F-HLP-013 (the notification log and its export), F-HLP-033 (the price of the line) |
| Breaks compatibility | no |

A frame, not a spec: the name and place of the report file, the composition of the dump and the exact behaviour under a double panic are the subject of the round, not of the note.

## Verdict

We take it. Half of the work is already done, and done well — the atomic write, the revision guard, the background autosave, the compact flight recorder — and
not done is exactly the part for whose sake all of this was started: to survive the event and tell the person about it. A "black box" that can be read only
while the program is alive, and a restore that is silent about time and place — these are not half of the solution, but two different gaps, and both are
closed with a small amount of code.

TEST: reach_tests::recovery_says_where_and_when — today it fails four times, and all four failures are visible by reading the files, without launching the program: (1) `panic::set_hook` does not appear in `src/` once, and `src/Cargo.toml:61` deliberately leaves stack unwinding, so the ring of `flight.rs` and the final write `on_exit` (`src/apps/kt-app/src/app.rs:5188`) die together with the process; (2) the key `msg.autosave_restored` in all five catalogs contains not a single `{0}` — neither the time of the snapshot nor the path `~/.local/share/kinotrain/autosave-project.otio`; (3) the restore chain `app.rs:5274` takes `last_project` before the autosave and never deletes it, so after "File → New" and a crash the OLD project opens, while the fresh work stays on disk unmentioned; (4) `save_flight_log()` (`app.rs:1943`) for an unnamed project writes `kt-flight.log` into the current directory, while the restored autosave is unnamed by construction (`app.rs:5282`).

ONE LINE: overlap — untouched | delay — untouched | capacity — only the time and the path are added to the status-bar strip, in the existing restore line; all the rest of the payload (the dump of the "black box", the gluing of the three logs, the collection button) goes into a file next to the project and into the body of the already-opened "Diagnostics" window.

## Sources

- Tracker `kde-invent` (GitLab Kdenlive, `_trackers.md`), tasks #899, #1437, #1531, #1535, #1560 — https://invent.kde.org/multimedia/kdenlive/-/work_items/1531
  and neighbours (an export of 2217 tasks, `~/.cache/kinotrain/trackers/kde-invent.jsonl`, sha256 `94c2d87c52449f19`;
  accessed 2026-09-08)
- Tracker `kde-bugzilla` (bugs.kde.org, product=kdenlive, 6346 tasks, sha256 `8b7608e9c1038d07`) — the number in the line `SIGNAL:` is reproduced by
  `python3 src/scripts/tracker-rank.py --query 'crash (log|dump|report)|backtrace|stack ?trace|core dump|\bgdb\b|drkonqi'`;
  54 of the 80 hits come from the label `drkonqi`, that is, they were filed by the system crash handler, not by a person. Dumps do not get into the repository
  (there is no licence, `_trackers.md`, section "Licences")
- Boris FX forum (VEGAS), thread "V26 crashed and deleted the .veg file!", 24–25.06.2026 — https://forum.borisfx.com/t/v26-crashed-and-deleted-the-veg-file/24325 (read
  via the Discourse API, `/t/24325.json`, accessed 2026-09-07)
- Blackmagic Design forum, thread "Project Backups", 21.07.2025 —
  https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=224766 (accessed
  2026-09-07 via the text proxy `r.jina.ai`; the forum search `search.php`
  opens only with a live `sid`, which the section page hands out)
- Apple, Final Cut Pro User Guide for Mac, "Restore a library" —
  https://support.apple.com/guide/final-cut-pro/restore-a-library-ver85d95b8a9/mac
  (accessed 2026-09-07)
- Apple, same guide, "Back up projects and libraries" —
  https://support.apple.com/guide/final-cut-pro/back-up-projects-and-libraries-verac3af7b81/mac
  (accessed 2026-09-07)
- Blender 5.2 LTS Manual, "Opening & Saving", the Recover section (Last Session,
  Auto Save) —
  https://docs.blender.org/manual/en/5.2/files/blend/open_save.html
  (accessed 2026-09-07)
- Blender 5.2 LTS Manual, "Crash" (the crash log, `blender.crash.txt` in `/tmp` on
  Linux) — https://docs.blender.org/manual/en/5.2/troubleshooting/crash.html
  (accessed 2026-09-07)
- Blender 5.2 LTS Manual, "Reporting a Bug" (Help → Report a Bug pre-fills the machine configuration, OS and build) —
  https://docs.blender.org/manual/en/5.2/troubleshooting/report_bug.html
  (accessed 2026-09-07)
- JetBrains, "Locating IDE log files" (Help → Collect Logs and Diagnostic Data; a separate section for "when the Help menu cannot be reached") —
  https://intellij-support.jetbrains.com/hc/en-us/articles/207241085-Locating-IDE-log-files
  (accessed 2026-09-07)
- Microsoft, VS Code FAQ ("use Help: Report Issue from the Command Palette"; telemetry can be switched off) —
  https://code.visualstudio.com/docs/supporting/FAQ (accessed 2026-09-07)
- The lines for Premiere and After Effects are marked `[NOT VERIFIED]`: Adobe's pages about Auto Save and automatic save & recovery did not open either from
  `helpx.adobe.com`, from the `/in/` mirror, or through `r.jina.ai` (a JS shell is returned). Only the branch
  `helpx.adobe.com/premiere/desktop/…` was available, and there is no section about autosaving in its navigation. The session's `WebSearch` budget was exhausted before work began.
- No program outside the core and none of the five core editors was checked live — decided in `docs/features/_editors.md`.
- Our code is the primary source for all statements about "us": `src/apps/kt-app/src/app.rs`, `src/apps/kt-app/src/main.rs`,
  `src/apps/kt-app/src/toolbar.rs`, `src/apps/kt-app/i18n/*.toml`,
  `src/libs/kt-engine/src/flight.rs`, `src/libs/kt-engine/src/player.rs`,
  `src/Cargo.toml`, `src/Cargo.lock`, items 96, 150, 175, 197, 199, 200 of
  `docs/porting-notes.md`.
