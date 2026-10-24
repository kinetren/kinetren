---
id: F-HLP-013
title: Notification center and log: where what flashed by lives
status: candidate
priority: B
cost: S
category: help/errors
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, vscode-1.136, jetbrains-2026.2, blender-5.2]
we_have: partial
crates: [kt-app, kt-engine]
invariants: [12]
ui_rules: [13, 14, 15, 17]
round: null
point: null
tests: []
depends_on: [F-HLP-001, F-HLP-010]
trackers: [losslesscut, kde-bugzilla]
checked: 2026-09-08
---

# F-HLP-013 — Notification center and log: where what flashed by lives

## Essence

During a session the program tells the editor half a hundred different things: saved, not saved, the file is gone, the GPU dropped out,
there is nowhere to paste. Each lives for seconds in one strip and disappears. The notification centre is the place where these lines
accumulate: you open it, scroll, read what you did not manage to look at, select the text of an error and paste it into an email to support.
Without it, a message that the user blinked at is a message that never existed.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | no notification centre [NOT VERIFIED] | messages are modal windows and the status line; a history window was not found in the help |
| Adobe Premiere | 26 | Events panel (Window → Events) | an alarm icon in the STATUS BAR; a double click opens the panel; "Details" — the full text of the entry, "Clear All" — clearing, and clearing the entries removes the icon from the status bar; in parallel a pop-up in the bottom right for a few seconds, the background colour = the kind of event, switched off in Preferences → Show Event Indicator |
| DaVinci Resolve | 21 | Console (Workspace → Console) [NOT VERIFIED] | a console window with tabs (Info / Errors / scripts); aimed at scripting and diagnostics, not at the everyday messages of the editor |
| Final Cut Pro | 12 | Background Tasks (Window → Background Tasks, ⌘9) | a PROGRESS window, not a log: "shows the tasks being performed and the percentage of completion", with Pause/Resume/Cancel buttons; when a task ends, its row disappears, and a separate log of messages is not described in the guide |
| After Effects | 26 | no notification centre [NOT VERIFIED] | errors are modal windows; a separate log exists only for network/background rendering, not for the session |

**Outside the core.** The mechanism does not originate from video editing: the bell-with-a-badge in the status bar and the
"notification centre" are exactly VS Code, and our implementation repeats it, down to the place and the badge. Without this
line it is unclear whom we took the form from and which part of it we did not take.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Visual Studio Code | 1.136 | Notifications (the notification centre) | the bell in the status bar opens an area where ALL notifications are available at any moment; levels info/warning/error; "Do Not Disturb" silences the pop-ups — and the errors along with them, while the history remains |
| IntelliJ IDEA | 2026.2 | Notifications (replaced Event Log) | a tool window: Suggestions + Timeline, an importance indicator (a blue dot — ordinary, a RED one — errors); unread ones are marked; notifications live "until you clear them manually or reopen the project"; **the status-bar message is clickable and opens its own entry in the window** |
| Blender | 5.2 | Info editor | a log of executed operators plus errors/warnings/info; a line is an OBJECT: a click selects, Shift adds, `B` a box, `Ctrl-I` inverts, `X` deletes, **`Ctrl-C` copies the selected entries to the clipboard** |

## The trick behind it

We have solved one and the same task in one program THREE times, with three different deduplication policies, and the correct policy is
written in the words of the owner himself — in the log he does not see.

- `kt_engine::flight::FlightRecorder` (`src/libs/kt-engine/src/flight.rs`,
  ring of 512): an entry is a structure `Rec { at_ms, kind, a, b, snap, n }`,
  a repeat increments the **counter `n`**. The rule is written in the module header as a quote of the owner's request (item 175) and is written out to disk.
- `PerfState::note` (`src/apps/kt-app/src/app.rs:4884`, ring of 800): a repeat **rewrites the last line** as `… ×N`; a comment explains why — "in v1 a spike flooded the whole log within seconds".
- `KtApp::push_notice` (`app.rs:1962`, ring of 200) — the only one of the three that the user sees: a repeat is **silently discarded**, there is no counter.

The consequence is not "the log is shorter", but an inversion of the signal for the most important message of the program. `warn("autosave failed: Permission denied")` (`app.rs:1098`)
repeats every 30 seconds: after the failure `dirty_content` is not cleared, so the next autosave deadline arrives and fails again, and the io error text is byte-for-byte the same.
`warn()` writes into TWO channels: `self.status` is assigned always, while `push_notice` silently exits from the second time on. The result is exactly the opposite of what is needed:
the transient channel is refreshed every half minute — and each time fades after 6 seconds, and while the cursor is over any widget it is not shown at all (hovering in `status_line` takes priority over the message) —
whereas the place where one comes to read the history shows one line and does not light the badge (`notices_unread` does not grow: the early exit stands BEFORE the increment).
A permanent channel asserts that "your work is not being written to disk" happened once per session.

The second half of the same error: the deduplication compares only with `back()`. One copied clip between two autosave failures "unglues" them, and the same 47 failures give anywhere from 1 to 47 lines depending on what the user did in the pauses.
And none of the three logs keeps BOTH ends of the pair: the bell remembers the time of the first event and does not remember the count; `PerfState` and `FlightRecorder`
remember the count, but overwrite the timestamp with the time of the last repeat. A range "from 14:02 to 14:31, 47 times" is shown by nobody.

## Pros

- The bell sits in the EXISTING strip of the status bar and has taken not a single pixel of new area (rule 13): an icon plus a number where there used to be a gap. This is the cheapest entry point of all possible.
- A notification is a snapshot of the text, not a link to a live object: a message about a deleted clip survives the deletion and remains readable.
- `Notice { at, severity, text }` is already structured: an entry has a time and an importance, so the colour of the bell (grey / blue / orange) is computed from the data, not set by the caller.
- The notifications window is the only surface of the program where the long English text of `thiserror` is visible IN FULL: `horizontal_wrapped` wraps it, unlike `layout_no_wrap` in the status strip (see F-HLP-002).
- The three channels do not interfere with each other: the engineering log does not clutter the user's log, and `FlightRecorder` does not enter the UI thread at all.

## Cons

- The price of deduplication-by-discarding: the user cannot tell "happened once" from "happened two hundred times". This is not a saving of space, but a loss of information — `×N` costs exactly as much space and keeps both quantities.
- The history lives in a field of `KtApp`, so a successful `File → Open` (`*self = app`, `app.rs:2730`), `File → New` (`app.rs:2718`) and a DnD of a project (`app.rs:4064`) wipe the whole session log together with the performance log. The binding of the log to the project is normal in itself — IntelliJ does the same ("until you manually clear them or until you reopen your project") — but for us there is an addition to this: the absence of a file. After a crash nothing remains, whereas `FlightRecorder` can be written out to disk.
- The badge counts the UNREAD, not the UNSORTED: `notices_unread` is reset by a single opening of the window (`app.rs:3845`), and after that a warning "source unavailable" is indistinguishable from "3 clips copied". In Premiere the icon in the status bar persists until the entries are CLEARED — that is, the indicator is derived from the list, not from a separate counter.
- The bell has neither a key nor a shortcut in the text of the menu item ("Notifications" without an ellipsis versus "Diagnostics…" with one — rule 17): the history can be reached only with the mouse.
- The three logs do not know about each other: the performance log has a "Clear" button, the bell does not; `FlightRecorder` has export to a file, the other two do not; none of them survives a restart of the program.
- The text of a notification is translated at the moment of recording, not at the moment of display, so after a language change the window shows a mixture of two languages. Neither t125 nor t129 catches this: the strings are legitimate, just frozen.
- A price that was not in the note and was shown by the tracker: a notification that has become a noticeable mechanism immediately grows settings. LosslessCut, over four years: display duration (#1134), a switch for informational notifications (#1541), "a system notification instead of a toast" (#2378), the set of fields (#2794), "don't block me with a modal window" (#2842). We have zero settings, and this is the right starting point, but `STATUS_TTL_MS` = 6 s (`app.rs:4096`) is a hard constant, not a preference: the first question "why does it fade so quickly" runs into a rebuild.
- The request that people actually voice is for a FILE, not a window. The only found wish about the message history asks for a `logfile / logfolder` "to review a crash" (kde-bugzilla #498747), that is, to read what happened AFTER the program has died. Our bell covers the opposite scenario — "the program is alive, the window is open" — and it stays empty exactly when it is needed; this is what makes F-HLP-014 not a neighbouring note, but the second half of the same gap.

## What users say

A video editor who HAS a notification centre asks what this icon is:

> What's the little yellow box with the exclamation point in the bottom right
> corner of the timeline? Sometimes it's a red stop sign with a white x inside.
> — Dan Lacloche, Creative COW, Adobe Premiere Pro, 11.11.2014,
> https://creativecow.net/forums/thread/bottom-right-corner-of-timeline/

Another editor's answer in the same thread — the whole domain in two phrases:

> It shows the error log. You might want to click it from time to time.
> — Tero Ahlfors, same thread, 11.11.2014,
> https://creativecow.net/forums/thread/bottom-right-corner-of-timeline/

This is exactly the thesis of the wave, applied to the bell itself: the indicator blinked in the corner, the person saw it and did not know what it was,
and could find out only from another person. Our bell is the same grey icon in the same corner, and it has exactly one hint: its own name, "Notifications" (`app.rs:3842`).

The trackers of the third wave answer a different question — not "did people notice", but "how often a task is filed about this when the mechanism exists and when it does not".

SIGNAL: a notification as a separate mechanism — 7 tasks in losslesscut, `\bnotifications?\b|\btoasts?\b`; a share of 0.4% against 0.2% in kde-bugzilla

The LosslessCut figure is twice as high because the notification EXISTS there as an object, and people argue about the object: display duration (#1134), a notification that the work has finished (#1510, #2048), a broken switch "Show informational
notifications" (#1541), a system notification instead of a toast (#2378), "send both summary and body" (#2794), replacing the modal "Success!" with a non-blocking toast
(#2842) — all seven hits. In Kdenlive the word `notification` appears in 14 of 6346 tasks and almost always means an operating-system pop-up rather than a place INSIDE the program: there is nothing to argue about, because the mechanism does not exist.

But the most interesting thing is not these numbers, but the zero. Not a single task about the history of notifications is found in any of the eleven exported rating trackers (25,442 tasks): the filter
`notifications?[ -](history|log|list|center|centre|panel|area)|(history|list|log) of (notifications?|messages?)` returns an empty answer everywhere. The request nevertheless exists — people simply phrase it not
as a window with a list, but as a FILE:

> introduce a logfile / logfolder where logs are stored to review a crash,
> i.e. what happened and if there is something to report as a bug
> — kde-bugzilla #498747, 16.01.2025, `severity: wishlist`, status CONFIRMED,
> label `triaged`, https://bugs.kde.org/show_bug.cgi?id=498747

And this is the price of the absence of a route, verbatim: the error happened, and the user got nothing.

> Fails silently, no trace of a missing file
> — losslesscut #2423, "Playback and Exporting fails silently when File is
> moved away or deleted from original path during editing", 19.04.2025,
> https://github.com/mifi/lossless-cut/issues/2423

NO QUOTES: separately about the logs of Resolve, VEGAS and Final Cut there is still no live feedback, and the trackers do not close this — none of the five core editors has a public tracker (`_trackers.md`, "Whom we do NOT take"), and a vendor's forum is not a tracker. I searched forum.blackmagicdesign.com ("Console", "render log", "where are the error messages"), vegascreativesoftware.com ("message log", "error log"), the Adobe help through the text proxy `r.jina.ai`, and also the threads of r/editors and r/davinciresolve through search proxies; reddit.com and discussions.apple.com do not open from this environment (recorded in `_editors.md`).

## How it fits Kinotrain

ADDRESS: src/apps/kt-app/src/app.rs — the class "a message flashed by and was not kept anywhere" is possible for us, and nothing catches it: `push_notice` (`app.rs:1962`) silently drops a repeat and does not increase `notices_unread`; the ring lives as a field of `KtApp` and dies on `*self = app` (`app.rs:2730`), and there is no export to a file at all — unlike `src/libs/kt-engine/src/flight.rs`, which has one (`save_flight_log`, `app.rs:1090`). Exactly the file that kde-bugzilla #498747 asks for is written by us for transport and is not written for messages.

### What already exists

- `Notice { at, severity, text }`, `KtApp::note`/`warn`, `push_notice`, a ring of 200, an unread badge and a bell colour by importance — `src/apps/kt-app/src/app.rs:22`, `:1949`, `:1962`, `:3821`. The window (`kt_notices`, `app.rs:1105`): reverse order, an empty state "Nothing yet", wrapping of long lines, Esc in the common cascade (`app.rs:1734`). 42 calls to `note()` and 6 calls to `warn()` in `apps/kt-app/src`.
- Perf log: `PerfState::note` (`app.rs:4884`), a ring of 800, `×N` dedup, the `kt_diag` window with the "Clear" and "Reset graphs" buttons (`app.rs:5064`), seven threshold warnings in plain language — the `[perf.note]` section of the catalogues `src/apps/kt-app/i18n/*.toml`.
- The "black box" `src/libs/kt-engine/src/flight.rs`: a ring of 512, dedup with a counter in the field `n`, a dump to `<project>.ktflight.log` (`KtApp::save_flight_log`, `app.rs:1090`). Test 175.
- Copying to the SYSTEM clipboard is already written twice — `ui.ctx().copy_text` in `src/apps/kt-app/src/widgets.rs:535` and `src/apps/kt-app/src/fx_editor.rs:463` — but only for the service JSON of FX chains and envelopes. For error text — never.
- Selecting text in a notification works by itself: egui `selectable_labels` is on by default and is switched off nowhere except Arabic (`src/apps/kt-app/src/i18n/mod.rs:90`, item 129).

### What is missing

1. **A `×N` counter in `push_notice`.** The mechanism already exists in the program twice; in the user log it is the only one of the three implementations that throws information away.
2. **A route for an error OPENING a project.** Checked by grep and confirmed on exactly one branch: `KtApp::new_empty(Some(e))` puts the text only into `status` (`app.rs:481`), while `notices` is created empty (`app.rs:511`). This branch is program start (`app.rs:5279`: restoring `last_project` or the autosave). After 6 seconds (`STATUS_TTL_MS`, `app.rs:4096`) the text disappears without a trace, and the bell shows "Nothing yet". `File → Open` (`app.rs:2731`), DnD (`app.rs:4065`) and importing a `.ktproj` (`app.rs:5346`) put the same error into the bell through `note()` — a divergence inside one class of events, and the most frequent entry into the program is covered worst. What is lost is the most valuable text we ever produce: `KtMetaBroken` ("metadata.kt is unreadable: …" with the position in the JSON), `KtMetaTooNew`, `UnsupportedSchema` — `src/libs/kt-otio/src/lib.rs:37`. These are also the lines that must be attached to a bug report (the topic of the neighbouring wave note, `docs/features/help/errors/crash-and-bug-report-flow.md`).
3. **Experience of switching projects.** The log must be a property of the SESSION, not a field of `KtApp` that is reassigned as a whole.
4. **An explicit "copy".** Mouse selection works, but: (a) for Arabic it is switched off deliberately and correctly (`i18n/mod.rs:90`) — an Arabic-speaking user physically cannot attach the error text to an email; (b) `Ctrl+C` with selected text goes to BOTH recipients at once. egui reads `Event::Copy` without consuming it, and `Label` removes `Sense::FOCUSABLE` from itself, so `ctx.egui_wants_keyboard_input()` stays `false` and the early exit of `KtApp::handle_keys` (`app.rs:3880`) does not fire: together with copying the text, `copy_selection()` runs (`app.rs:4014`), which overwrites the clipboard or appends to the READABLE WINDOW a line "nothing to copy" (`app.rs:2340`). And the dedup by `back()` immediately hides how many times the person tried.
5. **A road from the flashed-by line to its record.** IntelliJ has one and it costs no space: a message in the status bar is clickable and opens its record in the window. We have both halves and no transition between them.

### What gets in the way

Not the volume of code — there are only tens of lines here — but the fact that the fix has to be made in three places at once, otherwise there will be a fourth dedup policy. There are three structural constraints.

1. `Notice` stores `text: String`, assembled by `tf!` at the moment of writing, so neither `×N` (it needs a field `n` and a redraw of the last line) nor deferred translation (it needs a key plus arguments) fits the current structure without changing it.
2. The IntelliJ trick "click the status-bar line" is closed to us by the design: our line is drawn last in the frame as an `Area(Foreground)` layer with `interactable(false)` (item 83, `apps/kt-app/CLAUDE.md`) — otherwise the layer steals window drag across the status bar. The transition to the record would have to be hung on the bell that sits in the same strip, not on the text itself.
3. `×N` incremented while the window is OPEN must change the picture — that is movement, so a repaint is legitimate under invariant 12; but `×N` while the window is CLOSED must not request a repaint, otherwise the counter of autosave failures would wake the UI every 30 seconds for nothing.

### Which invariants it touches

- **Invariant 12** (repaint only on movement): redrawing the last line while the window is closed is not movement; the counter accumulates silently, and the badge changes in one frame.
- **Rule 13** (do not waste space): no new strip is created; everything happens in the existing bell and the existing window.
- **Rule 14** (a feature works REPEATEDLY, not once) — literally about this very defect, only applied not to focus but to a message: "autosave failed" today works exactly once per session.
- **Rule 15** (window edges do not steal clicks): the bell is already registered in `no_resize_rects` (`app.rs:3841`, item 104, test 101) — no new zones appear, but a "copy" button must not be placed in the status bar: that place is occupied and protected.
- **Rule 17** (paired ways are identical): two logs with different capacity, different dedup, different clearing and different ways of opening them.
- The dispute about tooltips is not touched by this note: no new surface appears, both windows already exist and open deliberately. t83 (`layout_tests.rs`) stays in force without a single change.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage of the round |
| Where the main work is | kt-app (`app.rs`: `Notice`, `push_notice`, `new_empty`, the `kt_notices` window) |
| Project format | not touched |
| Tests to write | `reach_tests::every_error_reaches_the_bell` (a broken `.otio` as `last_project` → the notification is there); a unit test on `push_notice` (two identical `warn` → one line with counter 2 and the time of the FIRST); a harness test "opening another project does not wipe the log" |
| Depends on | F-HLP-001 (one status-bar line), F-HLP-010 (`Severity` without an `Error` variant) |
| Breaks compatibility | no |

## Verdict

We take it, and cheaply: the winning mechanism is already written twice in our own tree; it does not need to be invented, only put where the person who looks for it will see it. The right dedup is with a counter: `×N` takes the same space as dropping does, and unlike dropping it keeps both values, whereas dropping turns an endlessly repeating error into a one-off and makes the number of lines depend on the user's unrelated actions.

TEST: reach_tests::every_error_reaches_the_bell — fails today: `KtApp::new_empty(Some(e))` puts the text of the opening error only into `status` (`src/apps/kt-app/src/app.rs:481`), `notices` is created empty at that point (`:511`), and after `STATUS_TTL_MS` = 6 s the line disappears without a trace; at program start (`:5279`) this is the only path, whereas `File → Open` (`:2731`), DnD (`:4065`) and the import (`:5346`) put the same error into the bell.

ONE LINE: overlap — not touched | delay — not touched | capacity — not a single character is added to the status-bar strip; the whole load (the repeat counter, the full `thiserror` text, the "copy" button) goes into the EXISTING notifications window (`src/apps/kt-app/src/app.rs:1103`), to which the user comes deliberately.

## Sources

- Tracker `losslesscut` (GitHub `mifi/lossless-cut`, 1920 tasks, sha256 `a078275dc3bd98fe`), tasks #1134, #1510, #1541, #2048, #2378, #2423, #2794, #2842 — https://github.com/mifi/lossless-cut/issues/2423 and neighbouring ones (accessed 2026-09-08)
- Tracker `kde-bugzilla` (bugs.kde.org, product=kdenlive, 6346 tasks, sha256 `8b7608e9c1038d07`), task #498747 — https://bugs.kde.org/show_bug.cgi?id=498747. Both numbers of the `SIGNAL:` line are reproduced by `python3 src/scripts/tracker-rank.py --query '\bnotifications?\b|\btoasts?\b'`; a zero result for notification history — with the same key and the filter from the text of the section. The dumps lie in `~/.cache/kinotrain/trackers/` and do not enter the repository (`_trackers.md`, section "Licences")
- Adobe, Premiere help, "View warnings, errors, and information events", https://helpx.adobe.com/premiere/desktop/edit-projects/correct-mistakes/view-warnings-errors-and-information-events.html (opened through the text proxy `r.jina.ai`, 2026-09-07)
- Creative COW, thread "bottom right corner of timeline", 11.11.2014, https://creativecow.net/forums/thread/bottom-right-corner-of-timeline/ (2026-09-07)
- Microsoft, VS Code Docs, "User Interface" (the section on the bell and the notification area, Do Not Disturb), https://code.visualstudio.com/docs/getstarted/userinterface (2026-09-07)
- JetBrains, IntelliJ IDEA 2026.2 Help, "Notifications", https://www.jetbrains.com/help/idea/notifications.html (2026-09-07)
- Blender 5.2 LTS Manual, "Info Editor", https://docs.blender.org/manual/en/latest/editors/info_editor.html (2026-09-07)
- Apple, Final Cut Pro for Mac, "View background tasks", https://support.apple.com/guide/final-cut-pro/view-background-tasks-ver64e71609/mac (through `r.jina.ai`, 2026-09-07)
- The rows for VEGAS, Resolve and After Effects are marked `[NOT VERIFIED]`: the session's `WebSearch` budget ran out, and `forum.blackmagicdesign.com`, `support.apple.com` and the Blackmagic manuals in PDF did not open from this environment during this pass. Live checks of the programs outside the core were not done — decided in `docs/features/_editors.md`.
- Our code is the primary source for all statements about "us": `src/apps/kt-app/src/app.rs`, `src/libs/kt-engine/src/flight.rs`, `src/apps/kt-app/src/i18n/mod.rs`, `src/libs/kt-otio/src/lib.rs`, items 129 and 175.
- Programs outside the core were not checked live (the decision of `_editors.md`): the VS Code, IntelliJ and Blender lines are taken from official documentation.
