---
id: F-RES-001
title: Autosave and project versions — on top of the file or next to it
status: done
priority: B
cost: M
category: resilience/durability
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08]
we_have: yes
crates: [kt-app, kt-otio]
invariants: []
ui_rules: [5, 12, 13, 19, 20]
round: "R.49"
point: 351
tests: [357, 358, 359, 360, 361]
depends_on: []
trackers: [kde-bugzilla, kde-invent, pitivi]
checked: 2026-09-15
---

# F-RES-001 — Autosave and project versions — on top of the file or next to it

## Essence

The program saves the work by itself. The question is exactly one, and it is not about the interval: WHERE it writes. If it writes OVER the user's file, the disk always holds exactly one state, fresh and whole; there is protection against a crash, and no protection at all against one's own error. If it writes ALONGSIDE, the disk holds a ring of dated copies, and the "yesterday's version" exists as a file. Everything else follows from this: how many copies to keep, how to thin them, where to find them, and why after the program is closed there is no more undo.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Auto Save (legacy) + Advanced Backup | Neither of the two mechanisms touches the working `.veg`. The legacy autosave writes `.autosave.veg` into a separate directory and cleans up after itself: "files are deleted when you exit the application" — this is a safety net against a crash, not a history, by design. Advanced Backup is a second, independent subsystem: "The creation of automatic backups begins when a change is made to the project", the directory of choice (temporary / the project's directory / custom), thinning in tiers — minutely "only stored within the last hour" (with the default step of 10 min this is 6 files), hourly "within the last 10 hours", daily, and real-time up to 500 s with eviction of the old ones |
| Adobe Premiere | 26.x | Auto Save + Maximum Project Versions | Auto Save writes SEPARATE files into the directory "Adobe Premiere Pro Auto-Save" next to the project (the Custom button moves it anywhere); the working file is never rewritten, and the unsaved-changes indicator is not cleared by autosave. "Maximum Project Versions" is a flat ring, 20 by default; when the limit is reached, the oldest is evicted [secondary source: `helpx.adobe.com` does not open from this environment] |
| DaVinci Resolve | 21 | Live Save + Project Backups | The only one that writes the working state CONTINUOUSLY, but it writes it to the project database, not to a user file: the user simply has no project file. Copies are a separate subsystem with a GFS scheme (grandfather-father-son) in its own directory, and it states plainly: "Opening a project backup does not overwrite the original project; project backups are always opened as independent projects". When idle, NEW copies are NOT written, so that useless ones do not evict useful ones |
| Final Cut Pro | 12.3 | library autosave + Final Cut Backups | There is no save action: "automatically saves all the changes you make as you work on a project", that is, the working library is rewritten continuously. The answer to this is separate copies: "Backups include the database portion of libraries only, not the media files", the name carries the date and time, the default directory is Movies, old ones are deleted by age. Plus Snapshot Project — a version by the person's command, not by a timer |
| After Effects | 26.x | Auto-Save + Increment and Save | Autosave puts copies into its own subdirectory (next to the project or into a specified one), "Maximum Project Versions" limits their number, the working file is not rewritten [secondary source: the same]. A named version is File → Increment and Save: the same file with number +1, alongside. Nothing in AE writes automatically over the working file: the only write "over" is the one the person ordered themselves |

**Outside the core.** Without Kdenlive this note cannot be written: the five core editors have no public tracker, and the price of how they are built is not visible in counts. Kdenlive is the only program whose safety mechanism is both documented and observable in the tracker, and "The trick behind it" below relies directly on its design.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Kdenlive | 26.08.0 | Backup + Project → Open Backup File | "A backup file is automatically created each time you save your project" — the trigger is not a timer but the "Save" command. The thinning is tiered and generous: "Kdenlive keeps up to 20 versions of your project file in the last hour, 20 versions from the current day, 20 versions in the last 7 days and 20 older versions". All of this lies NOT next to the project, but in a hidden `.backup` inside the application's data (`~/.local/share/kdenlive/.backup/`), and it can be reached only through the Project → Open Backup File dialog |

## The trick behind it

**1. The depth of undo is set not by "how many to keep" but by the TRIGGER — and nobody has it in the feature list.** The setting is called "Maximum Project Versions", it reads as a convenience setting, and in fact it answers a different question: how many of ITS OWN mistakes the user will be able to undo tomorrow, when undo has already died together with the process. But the product "step × number of copies" is a window only for a timer trigger. Kdenlive's thinning is the most generous of all (80 copies in four tiers), and its trigger is the save, and therefore the tracker holds exactly what follows from this: a person rendered for two hours, the machine overheated, and there are zero backup copies from those two hours, because in those two hours they never pressed "Save". Vegas uses as its trigger "when a change is made to the project", Premiere and AE use a timer, and Resolve uses a timer with SUPPRESSION when idle, so that identical copies do not evict the useful one from the ring. Four different answers to a question that is absent from every feature comparison, and it is exactly this that decides whether a version exists at the minute it is needed.

**2. The directory of copies is a choice between "they won't find it" and "they will clutter it", and both wrong answers lie in the trackers in plain text.** Pitivi wrote copies next to the project, and the first consequence was a request to hide them: "creates foo~.xptv files littering my hard drive". Kdenlive hid them — into a hidden `.backup` inside the application's data — and got the reverse side: in an open task from 2026 a person asks why the proposed recovery does not open the project, although choosing the same copy by hand from the list does ("why the unrecoverable project is not listed?"). Premiere and AE chose the middle — a named subdirectory next to the project — and paid with a third thing: having opened a good copy, you edit INSIDE the autosave directory, and the next autosave overwrites it (analyzed in F-PRJ-002 together with the forum instruction "first thing, SAVE AS"). There is no free option here, and the choice of place is not a matter of design but the third most important decision after "over/alongside" and the trigger.

**3. The ring of versions and the recovery file are DIFFERENT objects with opposite lifespans, and they are constantly taken for one.** The legacy Vegas autosave deletes its files on a normal exit: it must disappear, otherwise at the next launch it would offer to restore what the person deliberately rejected. Advanced Backup must survive a normal exit, otherwise it is not a history. Pitivi asked for exactly this split back in 2010: "Run sanity checks when pitivi exits normally, and if everything passes, delete the backup file". Hence the non-obvious consequence for whoever writes OVER: one object plays both roles — the user's file — and the depth of undo on disk is zero by construction. Eighty steps of undo in memory and zero steps on disk are separated not by a setting but by the boundary of the process: while the program is alive, almost everything can be undone; after closing — nothing, and precisely because the program was diligently saving.

## Pros

- A ring alongside survives the process: it is the only depth of undo that does not die together with the undo stack in memory.
- A copy is an ordinary project file, opened by the same code as the project; no format and no second reading subsystem is needed.
- The limit of the ring makes disk usage predictable and finite: "20 versions" is an upper bound, not "the folder grows".
- Suppressing writes when idle (Resolve) keeps the ring useful: without it, it is enough to go to lunch for twenty identical copies to evict the one single copy for which everything was set up.
- Tiered thinning answers both "what was it ten minutes ago" and "what was it on Tuesday" with the same mechanism (the mechanics are analyzed in F-PRJ-002, "The trick behind it" item 3 — not reopened here).
- A working file that autosave does not touch keeps the unsaved-changes indicator honest: the asterisk goes out only by the person's command.

## Cons

- Disk traffic is multiplied by the number of copies, and serializing the project is not a free operation; for us it is already a background thread, but the version directory turns a 30-second beat into constant writing.
- An open copy lies inside the service directory, and the next autosave overwrites it: the rescue mechanism works against the user, unless editing "from the ring" is explicitly forbidden.
- A hidden directory is not found (Kdenlive), an unhidden one clutters the working folder (Pitivi). There is no third place.
- The version of the project is not the version of the material: a copy refers to the same files, and rolling back to yesterday will not bring back a renamed or overwritten source.
- Continuous writing of the working state (Resolve Live Save, FCP) removes the question "did I save?" and together with it the concept of a save point: rollback is possible only to a copy, never to "the last Ctrl+S".
- The ring knows nothing about meaning: twenty copies from the last hour do not replace one named version "for the client" (this is a separate half, it belongs to F-PRJ-002).

## What users say

Generous thinning does not help if the trigger is "Save":

> Asking this because i just lose 2 hours of work ... PC crashed for
> overheating during rendering...looking at the automatic backups, no one was
> made during these 2 hours.
> — Davy Bartoloni, 18.07.2023, open to this day,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1723

Copies next to the project are the first thing people ask to hide:

> The project auto-backup feature implemented by Luis creates foo~.xptv files
> littering my hard drive.
> — Jeff F.T., 16.09.2010, https://gitlab.gnome.org/GNOME/pitivi/-/work_items/909

Hidden, they are not found; the list of copies works, but the proposed recovery does not:

> if i select last recovery on the list is opening successfully. […] why the
> unrecoverable project is not listed ?
> — Davy Bartoloni, 15.05.2026, open,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/2189

Recovery that kills what it was recovering (closed, but the title wording is a ready-made requirement for our code):

> When a project RECOVERY fails don't "create" an empty project with the real
> project name.
> — Davy Bartoloni, 28.05.2020, https://invent.kde.org/multimedia/kdenlive/-/work_items/705

And the class "the program rewrote my project itself, and there is less in it", confirmed by the triager as `grave` and open for the third year:

> Most recent added effects to the clip in any other sequence are missing if
> kdenlive alters project file.
> — Balr Gend, 25.06.2023, CONFIRMED, kdenlive 23.04.2,
> https://bugs.kde.org/show_bug.cgi?id=471428

SIGNAL: loss of a saved file — 17 tasks in kde-bugzilla, `(save|saved|saving|autosave|backup).*(lost|lose|corrupt|overwrit|gone)|(lost|corrupt|overwrit).*(project file|save)`; share 0.3% against 0.1% in kde-invent

The second number is the one for which the topic deserves a note at all. By `tracker-rank.py --themes` the topic "data loss" has a density of INCOMING DUPLICATES of 16 per 211 tasks = 7.6% — first place in the lexicon alongside "hardware/driver", and three times higher than the neighboring topic "project/file" (8 of 346 = 2.3%), even though "project/file" is one and a half times larger. A duplicate is the most expensive signal in this data (in weight it counts ×5): it means that a person ran into it INDEPENDENTLY, and the triager confirmed that it is the same thing. So the class is not "rare and frightening" but "recurring": people come with it one by one and about one and the same thing. The difference in shares, 0.3% against 0.1%, reads just as soberly: complaints of "saved and lost" live in the user bug tracker, while in the development tracker there are three times fewer of them — this class is brought in from outside, and from inside it is not noticed.

The neglect of the topic is moderate, and that makes it more unpleasant: 21 open tasks, median age 2.1 years, seven older than three years, maximum 5.9. The share of real fixes among the closed ones is 0.67, meaning a third of the closed ones were closed without a fix ("closed" ≠ "fixed", `_trackers.md`, "How to read these numbers", item 1).

## How it fits Kinotrain
CAPABILITY: openspec/specs/durability/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
Autosave writes to the recovery directory, not into the project (the owner's decision); a second copy and restoration by time.
ADDRESS: `src/apps/kt-app/src/app.rs:1063` — the class "the program itself rewrote my project" is possible for us for exactly one reason: the target of the background write is computed as `self.project_path.clone().unwrap_or_else(autosave_path)`, so a named project is overwritten every 30 s by its own autosave, and nothing catches this. `write_project_atomic` (`app.rs:4939`) guarantees that the file is whole, and `write_project_guarded` (`app.rs:4924`) guarantees that it is not older than the model; neither of the two keeps what was BEFORE. Tests t194 and t199 (`layout_tests.rs`) check the background operation and the non-return of a revision backwards — that is, they guard the quality of the write over, not the presence of a second state on the disk.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2–3 stages. It splits into: the autosave target + a ring of copies with a limit (S); the directory, the setting and tiered thinning (S); the "Open copy" window with date and time + a ban on editing "from the ring" (M) |
| Where the main work is | kt-app (`app.rs`: the write target, the ring, eviction, the list window, separating the recovery/history roles), i18n ×5 catalogs; kt-otio is not touched at all |
| Project format | does not touch it: a copy is an ordinary `.otio`, `KT_META_VERSION` stays 5 |
| Tests to write | kt-app `layout_tests.rs`: the working file is NOT rewritten in the background while versions are enabled; the ring does not exceed the limit and evicts the oldest; the asterisk does NOT go out because a copy was written (rule 5, rewrites the gate of t194/t200); the copies window opens again (rule 14) and names the path and the time (rule 19); "open a copy" does not leave the user editing inside the copies directory; t199 (the revision guard) must stay green without changes |
| Depends on | nothing. Neighbors: F-PRJ-002 (`project/history/versions-and-snapshots.md` — in-session edit history and named versions by the person's command; the boundary is described in its "Essence"), F-HLP-014 (the restore message) |
| Breaks compatibility | no |

## Verdict

We take it, and we take it as a fork in the road, not as a setting: today the background autosave writes OVER the user's file, therefore the depth of undo on disk is zero by construction, and eighty steps of undo turn into zero at the very moment the window is closed. All six compared programs — including both of those whose working state is also written continuously (Resolve, FCP) — answer this in the same way: with a separate ring of copies that does not touch the working file.
The price for us is unusually small: the write target is computed in one line (`app.rs:1063`), the write is atomic and already background, the project is one text file suitable for the role of a copy without a single change to the format, and not one invariant is touched. The hard part here is not the mechanics but the discipline: the limit of the ring in the code (rule 20), an honest asterisk during a write alongside (rule 5, rewrites the gate t194/t200), and the directory address in the interface (rule 19) — without these three one gets exactly what the tracker complains about: copies exist, but they cannot be found.
Named versions and the history panel are not included here and remain with F-PRJ-002.

## Sources

- Trackers (dumps of 08.09.2026 in `~/.cache/kinotrain/trackers/`, not in the repository): `kde-bugzilla` 6346 tasks, `kde-invent` 2217, `pitivi` 2300. At the time of writing only these three of the twelve in the `_trackers.md` registry had been exported; the numbers of the `SIGNAL:` lines are reproduced by `python3 src/scripts/tracker-rank.py --query …` on this set.
- VEGAS Pro 2026, Boris FX documentation, "Automatically saving and backing up projects": https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/3-projects/advancedsave.htm
  (access date 2026-09-08)
- Final Cut Pro User Guide, "Back up projects and libraries" and "Set storage locations": https://support.apple.com/guide/final-cut-pro/back-up-projects-and-libraries-verac3af7b81/mac
  · https://support.apple.com/guide/final-cut-pro/set-storage-locations-ver7db6ffe77/mac
  (access date 2026-09-08)
- Final Cut Pro User Guide, "Save and back up projects" ("automatically saves
  all the changes you make as you work on a project"):
  https://support.apple.com/guide/final-cut-pro/save-and-back-up-projects-ver79aa3d71/mac
  (access date 2026-09-08)
- Kdenlive 26.08 Manual, "Backup" (trigger "each time you save", tiers
  20/20/20/20, directory `.backup`):
  https://docs.kdenlive.org/en/project_and_asset_management/file_management/backup.html
  (access date 2026-09-08)
- DaVinci Resolve, manual, section "Project Backups" — the GFS scheme,
  "Opening a project backup does not overwrite the original project", suppression of writes when idle. The primary source is the BMD PDF manual; the mirror
  `steakunderwater.com/VFXPedia/__man/Resolve18-6/…/part112.htm` returned 404 on 08.09.2026, the wording is confirmed by a search index and by the analysis at
  https://hugosoarescolourist.com/resolve-backups-detailed-explanation/
  (01.2025), which separately records the discrepancy between the manual's defaults and the observed behavior. Treat as secondary.
- Adobe Premiere / After Effects: `helpx.adobe.com` does not open from this environment (checked 08.09.2026, "proxy refused the connection" and a timeout). The directory name "Adobe Premiere Pro Auto-Save", the settings "Save every" and "Maximum Project Versions" (1–20, default 20, eviction of the oldest) and the AE autosave subdirectory are taken from secondary analyses and are marked in the table as not verified against the primary source; the quote about overwriting the ring — Adobe Community, cited in F-PRJ-002.
- Our code: `src/apps/kt-app/src/app.rs` (`:1059`–`:1096` autosave, `:2290`
  `autosave_due`, `:2794` deletion of recovery, `:4095` beat, `:4908`
  `autosave_path`, `:4924` `write_project_guarded`, `:4939`
  `write_project_atomic`, `:5188` `on_exit`, `:5276` restoration),
  `src/libs/kt-model/src/model.rs:11` (`MAX_UNDO`),
  `src/libs/kt-otio/src/meta.rs:17` (`KT_META_VERSION`), items 96, 197, 200 of
  `docs/porting-notes.md`, tests t194/t199/t200 in `layout_tests.rs`.

**Implemented in R.49** — docs/porting-notes.md item 351
(tests t357, t358, t359, t360, t361); the further life of the feature is there.
