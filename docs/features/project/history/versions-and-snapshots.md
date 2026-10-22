---
id: F-PRJ-002
title: Edit history and project versions
status: candidate
priority: B
cost: M
category: project/history
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-app, kt-otio]
invariants: [3, 12]
ui_rules: [12, 13, 17, 18, 20]
round: null
point: null
tests: []
depends_on: []
trackers: [kde-bugzilla, kde-invent, openshot, pitivi]
checked: 2026-09-08
---

# F-PRJ-002 — Edit history and project versions

## Essence

An editor made forty edits in an hour and realized the fork was at the twentieth. Ctrl+Z forty times is not an answer: they cannot see WHERE that twentieth one is, and they overshoot. Under one word, "history," three DIFFERENT things live. (1) The undo stack — linear, in memory, dies with the process. (2) The history panel — the same stack, but VISIBLE: a click on a row rolls back everything above it in one move; undo turns from reversal into navigation. (3) Versions — named states that survive the session and a crash: "the version for the client," "before I ruined the second act." We have the first; we have neither the second nor the third.

**The boundary with F-RES-001.** This note owns edit history WITHIN A SESSION: the undo stack, its visible form, and the named states that the editor creates with ITS OWN COMMAND. Everything the program writes ON ITS OWN so that work survives closing the window and a loss of power — autosave, the ring of copies, thinning, the recovery catalog — belongs to F-RES-001
(`resilience/durability/autosave-and-project-versions.md`) and is not reopened here. The cut runs along the process boundary, not along the word "version."

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Undo/Redo History (a list at the Undo button) + Advanced Backup | There is no separate dock: "You can view the edit history by clicking the arrow next to Undo button", selecting a row — "The selected action and all actions above it are reversed". Edit → Undo All and Edit → Clear Edit History. Key point: "The edit history is also cleared when you close your project". There are no project versions; instead there are two different backups — the legacy autosave (every 5 min, `.autosave.veg` into a "temporary" folder, "files are deleted when you exit the application", after a crash it offers to open them) and **Advanced Backup**, which fires "when a change is made to the project" and stores in TIERS: minutely — "only stored within the last hour", hourly — "within the last 10 hours", daily and real-time — "up to 500" with older ones evicted |
| Adobe Premiere | 26.x | History panel (Window → History) + Auto Save | The only one of the five with a real history panel: a list of states, a click — a jump back. Depth is configurable (Preferences → Memory, "History States"; default 32 [NOT VERIFIED — helpx unavailable, the number is from a secondary source]). Auto Save writes SEPARATE files into a folder "Adobe Premiere Pro Auto-Save" next to the project; when "Maximum Project Versions" is reached, the ring overwrites the oldest. There are no named versions — the community has for years been asking for "Increment and Save", as in After Effects |
| DaVinci Resolve | 21 | versions of the GRADE on a clip (Local/Remote) + Timeline Backups + Live Save | **There is no history panel.** There is no timeline VERSIONING as a feature either (see "Sources": the task's premise was wrong here) — instead there are Duplicate Timeline and "Restore Timeline Backup" from the context menu. But there is something nobody else has: several complete grades on ONE clip with switching — BMD writes that you can compare "to versions of grades within a single clip". The project lives in a database (Project Library), Live Save writes every change, backups — as separate files in the "Project Backup location" in three tiers (minutes / hours / days) [tiers — secondary source] |
| Final Cut Pro | 12.3 | Snapshot Project (Shift-Cmd-D) / Duplicate (Cmd-D) | There is no history panel, and there is no save as an action either: "automatically saves all the changes you make as you work on a project". A version is a duplicate of the project in the same browser event, and its name gets the word "Snapshot" with date and time. The difference between the two neighboring menu items is not decorative: with a snapshot "Final Cut Pro embeds copies of compound or multicam “parent” clips in the duplicate, so any changes to other instances of those clips do not affect the duplicate". Separately — automatic library backups, "the database portion of libraries only, not the media files" |
| After Effects | 26.x | no panel; Edit → History (submenu) + Levels of Undo | There is NO history panel at all — an Adobe expert answers in plain text "Only Photoshop has a history panel". Depth is Levels of Undo, default 32, maximum 99 [secondary source]. Named versions are covered not by a panel but by the hotkey File → Increment and Save: the file is saved under the same name with number +1. This is exactly what people are asking to be moved into Premiere |

## The trick behind it

**1. A history panel turns undo into navigation — and thereby turns the names of
steps into a load-bearing structure.** Vegas puts it literally: "The selected
action and all actions above it are reversed" — you do not undo the last thing,
you CHOOSE a point and everything up to it is wiped out in one batch. So the list is read by eye,
not scrolled through: forty rows of "Change" take up space on the screen and give
nothing — you still press Ctrl+Z and look at the picture. The quality of operation
names is a feature that is in no feature list, and it alone decides whether the panel is alive or dead. The subtlety that breaks this: a name must name
the GESTURE, not the command. The roll of a cut is TrimEnd+TrimStart in one step, and a row
"Trim end" will be a lie of exactly the kind that UI rule 18 forbids. The cheap mechanism "name = name of the enum variant" gives precisely "Change".

**2. A snapshot differs from a copy in that it EMBEDS references.** In FCP the Duplicate (Cmd-D) and Snapshot Project (Shift-Cmd-D) items sit side by side in the menu, and outwardly they do the same thing. In fact a duplicate refers to shared compound/multicam
clips, and tomorrow's edit of a shared clip will silently rewrite the "version saved yesterday"; a snapshot "embeds copies of compound or multicam “parent” clips in the
duplicate". That is, a version is not a copy of a file but a copy plus the RESOLUTION OF ALL
REFERENCES. Whoever did not do this saved not a state but a promise.

**3. Autosave and versions are opposite mechanisms, and autosave can work against you.** Vegas's legacy autosave deletes its own files on a normal exit
("files are deleted when you exit the application"): it is a net for a crash, not a history — by design. Premiere's autosave is a ring: "As soon it reaches the max it
overwrites the first auto save", so with a 5-minute step and 20 versions it
covers the last ~100 minutes and nothing more. Worse: having opened a good copy, you
edit INSIDE the autosave folder, and the same autosave overwrites it with the bad
state — hence the forum instruction "first thing, SAVE AS outside". Vegas
2026 answers this with Advanced Backup, which fires not on a timer but
"when a change is made to the project", and stores as a PYRAMID with thinning:
minutely "within the last hour", hourly "within the last 10 hours", daily up to 500.
A flat ring answers the question "what was there three minutes ago," a pyramid answers
the question "what was there on Tuesday," and these are different questions for the same disk.

## Pros

- A click on a history row rolls back a batch: going back 20 steps is one action
  with one scene recalculation, not 20 separate rollbacks with 20 preview flickers.
- The list makes GRANULARITY visible: the user sees that the whole drag is
  one step, and stops fearing that "undo will return only the last pixel."
- Named versions solve what undo does not solve in principle: undo lives in
  the process and dies with it, a version is a file that survives both a crash
  and one's own unsuccessful save.
- A pyramid of backups (Vegas Advanced Backup, Resolve) on the same disk covers both
  the last hour and the last week; a flat ring covers only the last hour.
- Resolve's grade versions stand apart: the variants are not of the whole project but of ONE
  object, switched in place. The comparison runs between variants, not between
  history states, and therefore does not spend the undo stack at all.
- A snapshot with frozen references (FCP) stays true to itself when the shared material is edited further — an ordinary duplicate does not guarantee this.

## Cons

- History is linear in all five: go back, make an edit — the branch forward dies. A non-linear history exists in a neighboring class of programs (Photoshop,
  History Options → Allow Non-Linear History [NOT VERIFIED]), in NLEs — nowhere.
- A history panel requires permanent space on the screen for a list that is needed once an hour. This is a direct conflict with UI rule 13.
- History depth is memory: in Premiere and AE it is moved to settings
  (History States / Levels of Undo) precisely because it runs into RAM, and
  a visible panel immediately generates the request "make it bigger."
- Autosave into the same folder where you work, when restoring, overwrites
  the good copy that was found — the rescue mechanism becomes a mechanism of loss.
- Versions inside the project bloat the project file: in FCP a snapshot with frozen
  compound clips duplicates their contents, and the library grows with every
  "just in case."
- Grade versions act on a clip, not on the timeline: in Resolve there is no way to
  say "the whole edit in variant B", and the workaround is to duplicate the entire timeline.
- Snapshot history placed INTO the project file eats the file: in OpenShot the
  saved `"history"` took up more than half of a 1.4 MB project (#1923), and
  "why does a simple project weigh 4 MB" came as a separate task from a
  user (#1483). This is the measured price of exactly the decision that
  UI rule 20 forbids for us.
- Undo built on REVERSE operations pays with its own class of defects, "the wrong thing came back": the order of effects, priority, tracks, groups
  (pitivi #1121/#2234, kde-bugzilla #500598/#498340/#462228). The class does not fade with time — the tasks lie from 2011 to 2025.
- A history panel without merging by gesture immediately turns into a staircase: holding a handle gives one entry per mouse movement, and "Requires many undo
  commands to reverse" (kde-bugzilla #462406, #460876, #400735). A visible
  list makes this defect noticeable, rather than creating it.
- Unnamed steps are not a hypothesis but a separate request, "make the history more informative" (kde-bugzilla #400736): a panel whose rows do not name
  the gesture requires rework right after it appears.

## What users say

> I have two questions the first one is i cannot find the history panel, i went
> to window and there isn't.
> — Mrizak, 24.12.2019, https://community.adobe.com/t5/after-effects-discussions/history-panel-and-adding-template-to-workspace/m-p/10818236

The answer marked as correct:

> Only Photoshop has a history panel. Only other option is the Edit menu.
> — Rick Gerard (Community Expert), 24.12.2019, the same URL

The autosave ring as a loss of history:

> As soon it reaches the max it overwrites the first auto save.
> — Ann Bens (Community Expert), 18.03.2021, https://community.adobe.com/t5/premiere-pro-discussions/where-does-auto-saved-files-go-after-exceeding-the-maximum-project-versions/td-p/11907894

Autosave working against the user:

> The KEY THING TO DO once you are able to open a "good" previous version is to
> immediately use SAVE AS.
> — Jeff Pulera, 28.07.2015, https://creativecow.net/forums/thread/recovering-a-project-with-auto-save

And why autosave alone is not enough:

> My Adobe Premiere crashed, taking with it three months of saved work —
> autosaves included. I can't find any recent files.
> — Lena Mnk, 19.10.2025, https://creativecow.net/forums/thread/adobe-premiere-crashed-wiping-3-months-of-work-with-it/

Complaints about unnamed history steps could not be found on the vendors' forums (searched
creativecow.net and community.adobe.com; reddit and helpx do not open from here) —
but in the Kdenlive tracker, which has a history panel, this request stands as a
separate task, and its title says exactly what is called the load-bearing
structure here:

> undo history should be more informative and hyperlinked
> — Kdenlive #400736, 06.11.2018, severity `wishlist`, closed as FIXED,
> https://bugs.kde.org/show_bug.cgi?id=400736

Granularity is not theory: a gesture not merged into one step immediately
becomes a complaint, and it has been filed more than once — in 2018 and twice in 2022
(#400735, #460876, #462406).

> dragging sliders on bin effects spams undo history
> — Kdenlive #400735, 06.11.2018, https://bugs.kde.org/show_bug.cgi?id=400735

> Changing lift gamma gain by clicking and holding creates multiple undo history
> entries. Requires many undo commands to reverse
> — Kdenlive #462406, 29.11.2022, closed as FIXED,
> https://bugs.kde.org/show_bug.cgi?id=462406

The dispute about WHAT must get into undo and what must not also comes from
a user, not from a developer:

> This is why all the applications have the In and Out points settings in the
> undo list, and I have to admit that when I'm editing I use the IN-OUT undo
> several times during the job.
> — Kdenlive (kde-invent) #888, 22.12.2020,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/888

The price of history placed INSIDE the project file is measured in another tracker directly:

> In that particular case, the saved history data alone accounted for more than
> half of the entire (1.4 MB) project data.
> — OpenShot #1923, 28.07.2018, closed,
> https://github.com/OpenShot/openshot-qt/issues/1923

And the same conclusion about git, to which this note comes below, was independently formulated by a user of another editor — from the same property "the project is readable JSON":

> As an .osp file appears to be json, it could easily be pretty-printed and
> stored under version control in human readable format which would produce
> useful diff's for tracking changes.
> — OpenShot #2452, 15.12.2018, closed,
> https://github.com/OpenShot/openshot-qt/issues/2452

SIGNAL: undo and edit history — 38 tasks in pitivi, `\bundo\b|\bredo\b|ctrl.?z|history (panel|stack|list)|edit history`; a share of 1.4% versus 0.4% in kde-bugzilla

The threefold difference in shares is not about "Pitivi's programmers are worse," but about the FORM
of storage, and the composition of the tasks shows this literally. In Pitivi undo is a separate subsystem of reversible operations on top of GES, and the subject of the task turns out to be the SUBSYSTEM ITSELF: "[regression] Need to reimplement undo/redo everywhere" (#1364),
"Stabilize the undo/redo system" (#1887), "Make the undo system fail fast if
unexpected things occur" (#1994), "Add tests and reenable undo/redo" (#1622).
Every new operation must be able to undo ITSELF, and every forgotten one is a separate
defect.

But the main thing — both trackers converge on ONE dominant class, and it is not
"undo did not work" but **"undo returned the WRONG THING"**: "Undo loses the ordering of
operations" (pitivi #1121), "Undo does more than it should" (#1552),
"Undo/Redo does not preserve effect priority" (#2234), "Undo reorders video
effects" (kde-bugzilla #500598), "Deleted effects are restored in wrong order"
(#498340), "Undo does not restore removed tracks if the user marked all tracks
for deletion" (#462228), "Timeline corruption when moving more then one clip and
undo afterwards" (#371353). This is the price of reverse operations, and a snapshot model
does not have it AT ALL: a state cannot be restored "partially" nor "in the wrong
order" as a whole. It is exactly for this that we pay with memory for `Vec<Project>` —
until now the account was in our favor without proof; now there is one.

The reverse side of snapshots is measured right there, in OpenShot, which PUTS them into the file (#1923, #1483): the price of snapshot history is size, and therefore its place is in memory, not in the project. And the remaining Kdenlive tasks show what our future panel will get: three tasks about merging a gesture (#400735, #460876
"Too many history entries (for CTRL+Z) when changing box-location via mouse",
#462406) and one about the name of a step (#400736). Both complaints are about names and granularity, that is, exactly the two places where we have already done
it right (`apply_coalesced`) and where nothing has been done yet (names).

## How it fits Kinotrain

### What already exists

More than half, and exactly those parts that are usually the expensive ones.

- **A stack of snapshots.** `TimelineModel { project, undo: Vec<Project>, redo:
  Vec<Project>, last_gesture }` in `src/libs/kt-model/src/model.rs`,
  `MAX_UNDO = 80` (between the AE/Premiere default of 32 and the AE maximum of 99 — no need to change).
  `push_undo` clones the entire `Project` as a whole: a jump of N steps is
  one assignment, not a replay of N reverse commands. For a history panel
  this is the ideal storage form.
- **Granularity is already right.** `apply_batch` — "a batch of commands = ONE
  undo snapshot per gesture", `apply_coalesced` merges a series by gesture token
  (the whole drag of a transition edge, the whole FX window session — one step). That is, the rows
  of the future panel already correspond to human actions, not machine
  commands. It is separately valuable that validation runs BEFORE the snapshot (`simulate` →
  `push_undo`), so there are no no-op rows in the history — the `q` defect of the old
  editor is closed, and the panel will not show empty steps.
- **The separation "content / convenience" is already drawn and matches what
  the panel should show.** `dirty_content` versus `dirty_ui` in
  `src/apps/kt-app/src/app.rs` (edits of clips/FX/morphers/markers/tracks/media/
  order of rows versus region/zoom/scroll/cursor/bin layout). This is not
  just flags: `set_region`, `set_loop_enabled`, `set_ui_state` mutate
  `Project` BYPASSING `push_undo`, `undo()`/`redo()` explicitly carry the current
  `region`/`loop_enabled` over into the restored snapshot — and `lane_order` in R.35
  on the contrary moved INTO the model, because it affects the picture. The result: the undo stack
  already contains EXACTLY the content. The history panel will not have to filter anything — a rare case where half of the feature was done in advance.
- **The mechanics of writing a version to disk are fully ready.** `kt_otio::save_project`
  + `write_project_atomic` (writing to a temporary file next to it and rename) +
  `write_project_guarded` with a revision check (`saved_rev`, R.32) + a background autosave
  thread + a final write in `on_exit` + restoring an unnamed project from
  `autosave_path()` at startup (tests t194, t199 in
  `src/apps/kt-app/src/layout_tests.rs`). Writing a version means calling the already
  existing path with a different file name.
- **A jump of N steps is safe for FX by construction.** Invariant 3: the project is replaced
  via `player.set_project(...)`, chains are diffed by clip id,
  FX states are not recreated. The reverb tail will survive a jump of 20 steps
  in exactly the same way as it survives a single undo.
- **A window, not a panel, is already a ready pattern.** Notifications, search and
  diagnostics live as `egui::Window` in `app.rs`; history will go there too and
  will not take up permanent space (UI rule 13).

### What is missing

- **Names for steps.** The stack is `Vec<Project>` and nothing more: no string, no label,
  no reference to a command. I checked the hypothesis "`Command` is an enum, the name
  we'll derive from the variant": it is TECHNICALLY true (45 variants, `#[derive(Debug,
  Clone, PartialEq)]`) and FALSE in essence. `apply_batch` accepts a
  `Vec<Command>`, `apply_coalesced` merges dozens of calls into one step — a name
  derived from the first command of the batch will call the roll of a cut "trim end"
  (a violation of UI rule 18), and the deletion of an A/V pair "deleting a clip". The name
  must come from the level of the GESTURE, that is, from the call-site; the good news is that
  all call-sites already go through three wrappers in `app.rs`
  (`apply`, `apply_batch`/`apply_batch_restoring`, `apply_coalesced`), so there are
  exactly three places to add the parameter.
- **A jump of N steps.** There is only `undo()`/`redo()` one at a time. It cannot be implemented as a loop: N calls = N `player.set_project` = N FX diffs and N
  plan rebuilds. A single transition with a single recalculation is needed.
- **Named versions and their list.** There is neither a "save version" command, nor
  a place where the list of versions lives, nor a way to open a version without losing the
  current one.
- **History on disk for autosave.** This is the main finding of the section: our autosave does NOT create versions at all. In `app.rs` the target of the background write is
  `self.project_path.clone().unwrap_or_else(autosave_path)`, that is, for a NAMED project the autosave overwrites the project file itself in place every 30 s. Plus `save()` deletes the recovery file (`remove_file(autosave_path())`).
  The result: at any moment there exists exactly ONE state on disk. We are protected
  against a crash (the file is always intact and fresh) and completely defenseless against "I ruined it and saved": Premiere in such a situation has an Auto-Save folder with
  two dozen dated copies, Vegas has Advanced Backup as a pyramid, we have — nothing. Meanwhile exactly the same path that today overwrites the file, with
  a different target name, would write a version. **This half has since been separated into
  F-RES-001** (`resilience/durability/autosave-and-project-versions.md`), where the fork "over/next to", the trigger, thinning and the catalog are analyzed; here it
  remains only as a statement, so as not to read one decision twice.
- **An analogue of grade versions.** The morpher has `enabled: bool` — a binary
  switch (`src/libs/kt-model/src/project.rs`), the clip's `fx_off` and
  `fx_bypass` are binary too. The variant "two FX chains on one clip, switched in place" is not expressed by anything: a clip has one chain.

### What gets in the way

- **UI rule 13 versus the list.** The history panel is a strip under a list
  that people look at once an hour. The answer already exists in the code (a window, like notifications and
  diagnostics), but this means our "panel" will be a window, and one must honestly check
  whether it will be dead: a window that has to be opened to jump back competes with Ctrl+Z, which does not need to be pressed.
- **UI rule 17 "paired ways".** A second way to roll back an edit will appear.
  Ctrl+Z and a click on a row must behave identically — including the forward carry of
  `region`/`loop_enabled`, which today only `undo()` does. A jump by index into the stack must repeat this carry, otherwise the region and loop
  will start jumping in time along with the edit.
- **`project.ui` in snapshots.** `UiState` is a field of `Project`, it gets into the clone and,
  unlike `region`/`loop_enabled`, is NOT carried forward. Today this is harmless (the live viewport lies in the
  fields of `KtApp`, and it is written into the model only on save), but a jump of 20 steps back will restore
  the `ui` from twenty minutes ago, and the next autosave will write it to the file. A trap in a flat spot,
  and it is cured by one line next to the existing region carry.
- **UI rule 20 "nothing grows by itself" versus versions inside the file.** The project is one `.otio`; versions inside `metadata.kt` would mean a file that
  grows by itself with every save, and the end of round-trip byte stability
  (guarded by tests t188/t190 and test 12: "save→load→save is byte-stable").
  Plus every 30-second autosave would rewrite ALL the versions at once.
  The alternative is separate files next to it, exactly after the pattern of the sidecar index
  `<project>.ktindex.db`: a version becomes an ordinary project that can simply be
  opened, `Retained` (passthrough of foreign OTIO fields,
  `src/libs/kt-otio/src/convert.rs`) is kept for free, the format is not touched at all. The choice belongs to the round, but the imbalance is obvious.
- **i18n.** The name of each gesture is a string in five catalogs
  `src/apps/kt-app/i18n/*.toml` (600 lines each at present). This is one more argument
  against "a name from 45 enum variants": 45×5 = 225 new strings against ~20×5
  for meaningful gesture names. New glyphs — a rebuild of the subsets
  (`python3 scripts/subset-fonts.py`) and test t100.

### Which invariants it touches

- **Invariant 3** (an edit of the model does not destroy FX state) — is not violated,
  and it works FOR us, but only if the jump is done by a single project replacement.
  A loop of N undos would formally not violate it, but it would re-diff the chains N times.
- **Invariant 12** (the UI heats the system only on movement) — the history window is redrawn only when
  `model_rev` changes; the list is built by revision, not per frame.
- Invariants 1, 2, 4–11 are not touched at all: the feature affects neither sound, nor cache, nor rendering,
  nor FFmpeg, nor determinism.
- **UI rules:** 12 (background writing of versions not during a gesture/playback — already
  observed by `autosave_due()`), 13 (space), 17 (paired ways), 18 (the name
  says what happened), 20 (the file does not grow by itself).

### Separately: does git not close half of the feature

It does, and this must be admitted. The project is ONE `.otio` file, and it is unusually
good for git: `write_document` in `src/libs/kt-otio/src/io.rs` prints
JSON with `PrettyFormatter`, an indent of 4 spaces and a newline at the end
(one value per line — a meaningful diff), the round trip is byte-stable
(test 12, t188, t190 — "a project without cameras/without automation is byte-stable"),
media paths are relative, and the heavy sidecar `.ktindex.db` is regenerable and
will simply go into `.gitignore`. `git commit -m "version for the client"` gives
named versions with dates, an author, rollback and — which none of the
five has — a real diff between variants and real BRANCHES instead of a linear
history. Writing one's own version store on top of such a file means competing with
git on its own turf.

But exactly what VCS cannot do remains:

1. **The history panel.** This is intra-session navigation through unsaved
   states. git sees commits, not forty steps of the last hour.
2. **Variants within one project.** "Two grades on a clip with a switch"
   (Resolve) is a property of the MODEL, not of the file: variants must coexist
   in one open project and switch in place. A git branch gives a variant of the
   WHOLE file and requires closing the editor.
3. **A conflict between autosave and the working tree.** Our autosave rewrites the project file
   every 30 s — the user's git working tree is never clean,
   and a `git checkout` in the middle of a session will be overwritten by the next autosave (the guard
   `saved_rev` compares revisions of the MODEL and knows nothing about external edits to the file).
   This is a defect already today, regardless of whether we take the feature or not.

Hence the honest conclusion: the item "project versions as files" should be weighed against
one page of documentation "put the project in git", but items 1–3 are not closed by anything on our own.

ADDRESS: `src/libs/kt-model/src/model.rs:428` — the dominant class of both trackers ("undo returned the wrong thing": the order of effects, tracks, groups) is impossible for us, because `push_undo` puts into `undo: Vec<Project>` a CLONE OF THE WHOLE PROJECT (`model.rs:344`, `MAX_UNDO = 80` at `:11`), and `undo()` (`model.rs:450`) substitutes the snapshot back as a whole: "partially" and "in the wrong order" are not expressible here. But the class #462406 ("a gesture smeared over N history entries") is possible for us and is caught by nothing: merging is done ONLY when the call-site called `apply_coalesced(cmd, gesture)` (`model.rs:414`) with a shared token, and a forgotten token gives the same ladder of entries as in Kdenlive — and there is not a single test that checks "a drag N frames long = one undo step" for EVERY gesture. Separately, and in exactly the same way, the `project.ui` trap is not caught: `undo()` explicitly carries `region` and `loop_enabled` forward (`model.rs:453`–`:454`), but `ui` does not, and a jump of 20 steps will return the viewport from twenty minutes ago, which the background autosave then writes to the file.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2–3 stages. It splits into independent pieces: gesture names + a jump of N steps (S), the history window (S), versions on disk (M), FX variants on a clip (a separate note, not included here) |
| Where the main work is | kt-model (a step label next to the snapshot, `jump_to`), kt-app (three `apply*` wrappers pass the gesture name; the history window; the autosave target), i18n ×5 catalogs |
| Project format | versions as separate files — does NOT touch it (`KT_META_VERSION` stays 4); versions inside `metadata.kt` — break byte stability and require v5, so they are not proposed |
| Tests that will have to be written | kt-model: a step name survives undo/redo and eviction by `MAX_UNDO`; `jump_to(n)` == n×undo by result, but a single project replacement; `region`/`loop_enabled`/`ui` behave on a jump as on undo. kt-app `layout_tests.rs`: the history window, reopening (UI rule 14), a click on a row == Ctrl+Z the needed number of times (UI rule 17). `font_tests.rs` t100 + `i18n_tests.rs` t125/t126/t130/t131/t171 for the new strings. t194/t199 must stay green if the autosave target changes |
| Depends on | nothing. Related: F-RES-001 (autosave, the ring of copies and everything that survives closing the window — the boundary is described in "Essence") and the future note on FX/grade variants on a clip |
| Breaks compatibility | no |

## Verdict

We take the history panel and gesture names: half of the work is already done (snapshots of the whole project, gesture granularity, a clean separation of content and
convenience), and the price is a label next to the snapshot, `jump_to`, and the window. The trackers confirm this choice from an unexpected side: the dominant class of undo defects in both comparable trackers — "the wrong thing came back" — is generated by reverse
operations that we do not have; so the history panel attaches to our already correct mechanism, rather than legitimizing a fragile one. Separately, and MORE URGENTLY — history on disk: today the autosave rewrites the project file every 30 s
in place, and exactly one state exists on disk; this is protection against a crash without protection against one's own error, and Premiere and Vegas spend far more effort on the second than on the first (this item has since been carried by F-RES-001,
`resilience/durability/autosave-and-project-versions.md`). Named versions as an independent
subsystem — hold back: a user project is one text file with a byte-stable round trip, and git solves this half better than we would;
only intra-session navigation and variants within one model are mandatory for us to build ourselves.

## Sources

- Trackers (dumps from 08.09.2026 in `~/.cache/kinotrain/trackers/`, not included in the repository — there is no license for mass distribution of texts, see
  `_trackers.md`): `pitivi` 2672 tasks, `kde-bugzilla` 6346, `kde-invent` 2217,
  `openshot` 4516 (the export was still running, the registry has 4907). The numbers in the `SIGNAL:` line are reproduced by `python3 src/scripts/tracker-rank.py --query
  '\bundo\b|\bredo\b|ctrl.?z|history (panel|stack|list)|edit history'`; the count
  goes by title, labels and component (`match()` in the script), so matches only in the body of a task are not included in the count — quotes from bodies (#1923,
  #2452, kde-invent #888) are given by name with links.
- VEGAS Pro 2026, official Boris FX documentation, "Undoing and redoing edit
  operations": https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/undoing_and_redoing_edit_operations.htm
  (accessed 2026-09-07)
- VEGAS Pro 2026, official Boris FX documentation, autosave and Advanced
  Backup: https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/3-projects/advancedsave.htm
  (accessed 2026-09-07)
- Final Cut Pro User Guide, "Duplicate projects and clips" (Snapshot Project,
  Shift-Cmd-D, freezing of compound/multicam):
  https://support.apple.com/guide/final-cut-pro/duplicate-projects-and-clips-verfd45ffa45/mac
  (accessed 2026-09-07)
- Final Cut Pro User Guide, "Save and back up projects" and "Back up projects and
  libraries": https://support.apple.com/guide/final-cut-pro/save-and-back-up-projects-ver79aa3d71/mac
  (accessed 2026-09-07)
- Blackmagic Design, the DaVinci Resolve Color page ("versions of grades within
  a single clip"): https://www.blackmagicdesign.com/products/davinciresolve/color
  (accessed 2026-09-07)
- Adobe Support Community, the absence of a History panel in After Effects:
  https://community.adobe.com/t5/after-effects-discussions/history-panel-and-adding-template-to-workspace/m-p/10818236
- Adobe Support Community, the Premiere autosave ring:
  https://community.adobe.com/t5/premiere-pro-discussions/where-does-auto-saved-files-go-after-exceeding-the-maximum-project-versions/td-p/11907894
- Creative COW, "Recovering a project with auto save":
  https://creativecow.net/forums/thread/recovering-a-project-with-auto-save
- Creative COW, "Adobe Premiere crashed, wiping 3 months of work with it":
  https://creativecow.net/forums/thread/adobe-premiere-crashed-wiping-3-months-of-work-with-it/
- Creative COW, "History panel in AE?" (the same answer, from 2013 — a sign that the state of affairs has not changed):
  https://creativecow.net/forums/thread/history-panel-in-aeae/

**What could not be verified and why.** `helpx.adobe.com` does not open from this environment (navigation policy), so the official Premiere/AE pages —
History panel, History States, Auto Save, Increment and Save — are confirmed
only by headings/snippets of search results and by answers from Adobe Community
Experts; the specific numbers (32 states in Premiere, 32/99 in AE, 20 autosave versions)
are marked as secondary. `reddit.com`, `web.archive.org` and
`discussions.apple.com` are unavailable; `forum.blackmagicdesign.com` returned 403 on
direct links, so the Resolve backup tiers (15 min / hours / days) are taken from a
secondary analysis and are not confirmed by the primary source — unlike the tiers of
Vegas Advanced Backup, which are confirmed by the official documentation.

**A correction to the premise of the task.** The task states that Resolve has
"Timeline Versions" — named versions of the timeline. The check did NOT
confirm this: Resolve has Duplicate Timeline, "Restore Timeline Backup" from the
context menu and versions of the GRADE on a clip (Local/Remote), and timeline versions
are a long-standing REQUEST from users, to which threads answer "make a copy of the
timeline and rename it". The "Who has it and how" table was corrected accordingly;
Resolve's unique idea is precisely the versions of a grade on one clip, and it is these that must be compared with
our morpher and `enabled`.
