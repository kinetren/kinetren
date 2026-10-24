---
id: F-RES-005
title: Unsaved work on exit — asterisk and a prompt on close
status: done
priority: B
cost: S
category: resilience/recovery
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-app]
invariants: []
ui_rules: [5, 12, 19]
round: "R.49"
point: 354
tests: [369, 370, 371, 372]
depends_on: [F-RES-001]
trackers: [kde-bugzilla, kde-invent, pitivi, shotcut]
checked: 2026-09-15
---

# F-RES-005 — Unsaved work on exit — asterisk and a prompt on close

## Essence

The editor closes the window, and the model holds edits that are not on disk.
Two subjects guard this, sitting on ONE flag: the indicator "there are unsaved changes" (an asterisk in the title) and the question "save before
closing?". The subject of the note is not the question itself, but WHAT raises the flag.
If zoom and scrolling raise it, the question is asked on every close, people stop reading it and answer without looking; if inserting a clip does not raise it, the program
quits silently, and the work disappears without a word.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | asterisk `*` in the title + "Save changes?" | the flag is cleared by a manual Ctrl+S. With **Live Save** on, it writes by itself — and then there is NO asterisk AT ALL; it works only after the first manual save |
| Adobe Premiere | 26.x | "Do you want to save changes?" on close | by default autosave writes COPIES to a separate directory and does not touch the working file — which is why the question is mandatory. The option "Auto save also saves the current project(s)" moves autosave onto the user's file, and the dialog stops appearing |
| DaVinci Resolve | 21 | the question on close only with Live Save off | the project lives in a database, **Live Save is on by default** and writes continuously; in collaborative mode it is forced on and cannot be turned off |
| Final Cut Pro | 12.3 | none: neither a "Save" command, nor an asterisk, nor a question | the library is written continuously ("automatically saves all the changes you make"); a version is a manual duplicate of the project, plus a library backup every 15 minutes separately |
| After Effects | 26.x | asterisk + "Save changes to …?" on exit | autosave writes copies into the `Autosave` directory and does not touch the working `.aep`; that is why both the asterisk and the question live a full cycle |

## The trick behind it

**1. The question on close is not a property of the editor but a consequence of WHERE
autosave writes: in three of the five it is switched off by the SAME toggle that
moves autosave onto the user's file, and in the fourth it is absent for the same
reason from the start.** Vegas with Live Save removes not the dialog but the ASTERISK ITSELF
(the Boris FX documentation says this in plain text); Resolve with Live Save does not
ask; Premiere with "Auto save also saves the current project(s)" stops
showing the dialog — and users discover this as a DEFECT, not as a convenience; FCP never asks, because it
has no concept of "unsaved". From this follows a conclusion absent from every list of
features: **the question "save?" protects not against losing work but against a
mistaken SAVE.** It dies exactly at the moment when the program starts writing over the file
by itself — that is, when the possibility of rejecting what was done disappears. This is a dispute about undo, not about
safety.

**2. Continuous writing in Vegas and Premiere does not switch on until the project has been
saved by hand at least once** ("A project must be saved once before the Live
Save will start to work!"). An unnamed project is a common hole: continuous
writing is not yet there, the asterisk burns and cannot go out, and the only protection
is that very question on close. That is, the mechanism is at its weakest exactly in the
hour when the work is least valuable to the program and most vulnerable for the user.

**3. There is one flag, but two states — and the boundary between them depends on features, not on common sense.**
Users demand that the panel layout and window position be SAVED (in the Kdenlive tracker this is a separate class of tasks), but
do not want to be asked about them on exit. The only honest answer is two different
states of "changed": one gives the asterisk and the question, the second rides along to disk
on the side and stays silent. None of the five names this in its documentation; it is visible only from behaviour.

## Pros

- The question on close is the only place where the decision "throw the work away" is made by a person, not by a timer. A program with autosave over the file
  has no such place in principle: by the time of closing there is nothing left to throw away and nothing to return.
- The asterisk is the cheapest indicator among the existing ones: it lives in an already
  occupied place (the window title), takes up no pixel of the screen, and is read with peripheral vision.
- The "changed" flag is the only sign by which one can NOT write: without it autosave writes the whole project by timer, regardless of whether the person is working or has gone to lunch.
- The separation of "content versus convenience" pays off twice: a quieter autosave and a more honest asterisk. One and the same boundary serves both tasks.

## Cons

- **A false "dirty" state devalues the question faster than its absence does.**
  A dialog that always appears teaches people to press "yes" without reading; after
  that it protects against nothing, but still stands in the way of the exit.
- The opposite error is symmetric and quieter: an edit that did not raise the flag leads to a
  silent exit. Worse, in Pitivi an unraised flag also BLOCKED
  saving — the "Save" button stayed inactive, and the edit could not be written at all.
- The boundary "content versus convenience" does not follow from general considerations: the order of tracks is a convenience exactly until the program has a layer
  acting "on everything below". A feature appeared — the boundary moved, and not a single test will notice this by itself.
- The question on close is a modal window on the exit path: if it pops up in the wrong layer or outside the focus, the program looks hung (kde-bugzilla
  519729 — exactly this case with a minimised window).
- Autosave that clears the asterisk is honest only while it writes OVER the user's file. As soon as it starts writing a copy alongside, the same code
  starts lying: the file is old, and there is no asterisk.

## What users say

False "dirty" state — people ask to save right after saving:

> using latest build, just place a clip on the timneline, save the project,
> click on close button and there is a new request to save the project.
> — Kdenlive, 17.08.2023, OPEN for the third year, 5 comments,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1746

The same class, but caught and fixed:

> Project marked as changed immediately after opening
> — Kdenlive, 25.02.2021, RESOLVED/FIXED,
> https://bugs.kde.org/show_bug.cgi?id=433615

The opposite error — there is an edit, no flag, and the program exits silently:

> open project, Right click, Remove Track, ^q, exits without prompting to
> save.
> — Shotcut, 24.11.2015, closed,
> https://github.com/mltframework/shotcut/issues/147

And the most expensive form of it — the flag did not rise, so you CANNOT save:

> When editing audio volume curves' keyframes, the project does not get
> marked as dirty/having unsaved changes, so you cannot save until you move a
> clip within the timeline.
> — Pitivi, 20.08.2014, closed,
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1500

Autosave over the file removes the question — and it is perceived as a loss:

> I wasn't getting the 'Do you want to save changes" dialog box.
> — decaharms, Adobe Community, 02.10.2019,
> https://community.adobe.com/t5/premiere-pro-discussions/auto-save-also-saves-current-project-s/m-p/10645656

SIGNAL: guard for unsaved state — 6 tasks in pitivi, `unsaved|without saving|(prompt|warn|ask).*(sav|clos|quit|exit)|marked.*(chang|modif|unsaved)|discard.*chang`; a share of 0.2% against 0.1% in kde-bugzilla

SIGNAL: "saved — and it was lost anyway" — 16 tasks in kde-bugzilla, `(lost|lose|gone|missing|revert).*(after|upon|on).*(sav|reopen|reload|clos)|(sav|reopen|reload).*(lost|lose|gone|missing)`; a share of 0.3% against 0.1% in pitivi

SIGNAL: a convenience that did not survive closing — 3 tasks in kde-invent, `(layout|window position|divider|splitter|panel|workspace|zoom|track height|column width).{0,25}(not|isn.t).{0,10}(saved|remembered|restored|persist)`; a share of 0.1% against 0.0% in kde-bugzilla

**The breakdown into three classes is the main result of the reconnaissance.** The whole guard
(the first filter) — 15 tasks across four trackers: 6 in kde-bugzilla
(0.1 %), 2 in kde-invent (0.1 %), 6 in pitivi (0.2 %), 3 in shotcut (0.2 %).
Within these fifteen:

- **"did not ask on close"** — 4 tasks (pitivi 409 — is in fact a REQUEST
  "make a warning" from 2009, pitivi 1333, pitivi 2225,
  shotcut 147). The class consists almost entirely of "the feature does not exist yet" and
  "a specific edit does not raise the flag", rather than "the dialog broke".
- **"asked, but the edits were lost anyway"** — a separate filter, and it is THREE TIMES
  denser: 16 tasks in kde-bugzilla against 3 in pitivi, 0.3 % against 0.1 %.
  People lose work not at the dialog but after it: `#397250` "All slowmotion
  effects in project lost upon saving", `#410627` "Zoom and pan effects on
  slideshow are lost after saving and loading a project again", `#445490`
  "clips get wrong position after save and close". The dialog did its job, the answer
  was "yes", the file was written — and there is less in it than there was.
- **false "dirty" state** — 2 tasks, and both are named: kde-bugzilla 433615
  (the project was marked as changed right after OPENING, fixed) and kde-invent
  1746 (asks after saving, open for the third year, 5 comments).
  The class is rare IN THE COUNT and very expensive in its cost: it does not crash the program and does not
  lose data, so it does not get into the tracker, but it teaches the user to ignore the only question the program asks them.

It must be read this way: **the question on close is a cheap topic in the tracker because
it either exists or people ask for it to be created; the expensive topic lives one floor lower**,
in "saved and lost", and the question does not reach there at all. And separately —
kde-bugzilla 348674 "Crash Recovery only works properly for unsaved
projects" (2015, FIXED): recovery that works only for an unnamed project is the same hole of "the first save" as Live Save has, but from the other side.

The numbers are reproduced by `python3 src/scripts/tracker-rank.py --query …` on
dumps from 08.09.2026; "closed" is nowhere read here as "fixed"
(`_trackers.md`, "How to read these numbers", item 1): for the topic "project/file" the share of
genuine fixes among closed ones is 0.67 (`tracker-rank.py --themes`), that is, a third of what is closed was closed without a fix.

## How it fits Kinotrain
CAPABILITY: openspec/specs/durability/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
The close button asks, "Stay" keeps the window open, "Don't Save" leaves the file untouched; a convenience (row heights, view) does not light up the dirty state.
ADDRESS: `src/apps/kt-app/src/app.rs:2717` — the class "the program threw away my work without asking" is POSSIBLE for us and is not caught by anything: `new_project` does `*self = Self::new_empty(None)`, `open_project_dialog` (`:2722`) does `*self = app`, there is no `impl Drop for KtApp`, `on_exit` (`:5188`) is not called on this path, and up to 30 seconds of content edits, together with the undo history, disappear silently. Tests t194 and t199 (`layout_tests.rs`) guard the background nature of autosave and the non-return of the revision backwards; test t96 guards the boundary "content ≠ convenience". None of the three looks at switching the project within a session — there is not a single test on this path at all.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage. Three pieces: a final flush before switching projects (`new_project`/`open_project_dialog`) and writing the unnamed project in `on_exit`; a save point in the history that clears the asterisk when undoing back to it; a named choice "we do not ask" — a line in the help and in the status |
| Where the main work is | kt-app (`app.rs`: `new_project`, `open_project_dialog`, `on_exit`, the gate at `:1094`), kt-model (the save-revision mark in the history), i18n ×5 catalogues |
| Project format | does not touch it |
| Tests that will have to be written | kt-app `layout_tests.rs`: an edit → "File → New" → the edit HAS ended up on disk (today this turns red); an unnamed project → exit → the last edit is in the recovery file; undo back to the save point clears the asterisk, one step further lights it; t96 and t194 must stay green without changes |
| Depends on | F-RES-001 (where autosave writes — whether the asterisk is honest depends on this). Related: F-HLP-014 (recovery message), F-PRJ-002 (history and versions) |
| Breaks compatibility | no |

## Verdict

Take it, but not the question on close — the three gaps under it. The absence of a dialog is a
legitimate and already accepted choice: it repeats Final Cut Pro and coincides with
the behaviour of Vegas, Resolve and Premiere in continuous-writing mode, and the reconnaissance
shows that the dialog protects not against loss but against a mistaken save, that is, it solves
the undo problem, which we do not have on disk anyway (F-RES-001).
But the right not to ask is granted only to a program with an HONEST autosave, and ours is not honest everywhere: switching projects within a session throws away up to 30 seconds
of work and the whole history without a word, an unnamed project is not written
on exit at all, and its asterisk burns forever and therefore means nothing.
The separation `dirty_content`/`dirty_ui`/the third tier is a strong point, which should not be touched but PROTECTED by a test: moving the order of rows from convenience to
content (R.35) proved that the boundary moves after the features and does not re-examine itself. And since we do not ask, there must be a line about it in the
help: silent autosave works only when the user knows about it; there is no switch for it and there will be none, because the program has no settings window,
and adding one for a single checkbox violates rule 13.

## Sources

- Trackers (dumps of 08.09.2026 in `~/.cache/kinotrain/trackers/`, not committed to the repository):
  `kde-bugzilla` 6346 tasks, `kde-invent` 2217,
  `pitivi` 2672, `shotcut` 1368 — all four matched the number in the registry
  `_trackers.md`, that is, they were exported in full. Of the twelve in the registry at the time of writing these four were ready (`olive` was in progress, 405 of 1853, and was not taken into the count).
- Permanent references to specific Kdenlive tasks (of both trackers), Shotcut
  and Pitivi are given in the section "What users say".
- VEGAS Pro, documentation "Advanced Save — Live Save & Automatic Backups" ("The project title will not display an asterisk (*) indicating unsaved
  changes", "A project must be saved once before the Live Save will start to
  work!"):
  https://help.magix-hub.com/video/vegas/23/en/content/topics/3-projects/advancedsave.htm
  (accessed 2026-09-08). The edition of the help for line 23 was opened; the same page in the 2026 help (Boris FX) is quoted in F-RES-001 at the address
  `cdn.borisfx.com/borisfx/Documentation/vegas/2026/…/advancedsave.htm`
- Final Cut Pro User Guide, "Save and back up projects" ("automatically saves
  all the changes you make as you work on a project"):
  https://support.apple.com/guide/final-cut-pro/save-and-back-up-projects-ver79aa3d71/mac
  (accessed 2026-09-08)
- DaVinci Resolve: Live Save in "Project Save and Load", on by default
  and mandatory in collaborative mode — per documentation and reviews
  (https://beginnersapproach.com/davinci-resolve-autosave-livesave/,
  https://forum.blackmagicdesign.com/viewtopic.php?t=210348); not checked live.
- Adobe Premiere, discussion of the option "Auto save also saves the current
  project(s)":
  https://community.adobe.com/t5/premiere-pro-discussions/auto-save-also-saves-current-project-s/m-p/10645656
  (accessed 2026-09-08). The `helpx.adobe.com` help is still unavailable from this environment
  (`_editors.md`, "What was unavailable").
- Our code: `src/apps/kt-app/src/app.rs`, `src/apps/kt-app/src/toolbar.rs`,
  `src/apps/kt-app/src/bin_panel.rs`, `src/apps/kt-app/src/timeline.rs`,
  `src/libs/kt-model/src/model.rs`, `src/libs/kt-model/src/project.rs`;
  items 96, 197, 200, 235, 262; the section "Content edits ≠ convenience edits" in `src/apps/kt-app/CLAUDE.md`.

**Implemented in R.49** — item 354
(tests t369, t370, t371, t372); the feature's later history is there.
