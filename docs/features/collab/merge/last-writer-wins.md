---
id: F-COL-032
title: The one who saved last wins
status: candidate
priority: B
cost: S
category: collab/merge
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, syncthing-2]
we_have: partial
crates: [kt-app, kt-cli]
invariants: []
ui_rules: [16]
round: null
point: null
tests: []
depends_on: []
trackers: [syncthing, opencue]
services: [syncthing, premiere-productions, fcp-shared-library, bm-cloud]
checked: 2026-09-08
---

# F-COL-032 — The one who saved last wins

## Essence

Two people saved one document, and one remained. This is not a scenario and not a rarity — it is what happens by default everywhere that there is no lock: the file system has no opinion about whose edit is worth more, and the write simply lies on top. So the question is not "can the loss be avoided" (it cannot) but a different one: is it NAMED? The editor either sees beside the file another person's edit with a mark of who and when made it — or sees nothing and learns of the loss an hour later, after the work.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | never | a pure last-writer-wins: the second instance writes `.veg` over the first, with no lock, no warning, and no copy of the loser. The loss is not named and not noticed `[NOT VERIFIED live]` |
| Adobe Premiere | 26.x | prevention instead of a tie-break | Productions places a `.prlock` beside the project, Team Projects occupies the sequence. Where there is no lock (a project outside production on a shared disk), the same behaviour of the file system remains |
| DaVinci Resolve | 21 | prevention instead of a tie-break | the project ceases to be a file: database records are occupied, bins and timelines separately. There is no tie-break, because the race must not happen; complaints about proxy files with the `-conflict` suffix during synchronisation show that it does happen |
| Final Cut Pro | 12.3 | prevention taken to the limit | an exclusive lock on the whole library: even reading is not available to the second. There is no loser, because there is no second writer |
| After Effects | 26.x | the same as Premiere | a `.aep` is not occupied by anything outside Team Projects `[NOT VERIFIED live]` |

**Outside the core.** All five core rows speak of prevention, and none describes what to do when prevention failed. The rule for this case is formulated in exactly one place, and it is not an editor.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Syncthing | 2.x | conflict file | the loser does not disappear but is renamed to `<name>.sync-conflict-<date>-<time>-<who>.<ext>`. The rule for choosing the winner is written down and total: the file with the OLDER modification time loses; on a tie, the device with the larger first 63 bits of its device ID. Conflict files are "treated as normal files" and spread to all nodes |

## The trick behind it

**Even an honest implementation of a tie-break is not a merge but a NAMED DEFEAT, and the name here is not a metaphor: the loser receives a literal file name made of the date, the time and whoever edited it.** The mechanism does not try to be clever. It consists of two halves, and the second matters more than the first: choose the winner (by edit time) — and SAVE the loser under a name that answers the questions "when" and "who." The half that everybody considers the main one is in fact a service one: the choice of the winner is needed only so that both sides arrive at the same one.

**Second layer — why the rule has device identifier bits in it.** Comparing edit times is not total: the clocks of two machines may coincide. The tie-break must be total, otherwise two nodes would name DIFFERENT files as losers and diverge once more, now in the resolution of the conflict itself. Hence the addition from the bits of the device ID — not "just in case," but a condition of convergence: the rule must give the same answer to both without asking each other questions.

**Third layer — the price: naming produces objects, not messages.** A conflict file is an ordinary file, and it spreads to all nodes together with the rest. Hence a living class of pain: hundreds of thousands of renamed files after an unsuccessful update, and the ten-year-old request #1746 "an external tool for resolution — merge or select the winner," which proposes calling `vimdiff` or `meld`. That is, even where the defeat is formalised, merging has been delegated OUTWARD and in ten years has not appeared inside.

## Pros

- Nothing is lost: the losing edit lies beside as a whole document; it can be opened, compared and transferred.
- The rule is written down and verifiable; a person can predict the outcome BEFORE pressing "save."
- No server and no agreement are needed: the tie-break is computed by each side independently and converges.
- It works as insurance on top of any lock: the lock prevents, the tie-break catches what could not be prevented.

## Cons

- Requires comparable clocks: the edit time is taken from the file system, and the clocks of two machines diverge; on network volumes the time is set not by the one who writes.
- Naming produces files, not notifications: they must then be removed, they land in backups and spread to all nodes.
- The winner is chosen not by meaning but by time. A later edit is not a more valuable one — the mechanism honestly does not know this.
- There is still no resolution: "choose a winner" is not "merge," and the request for merging there has been open since 2015.

## What users say

The only formally recorded model of a conflict in the domain — and both halves of the rule are visible in its wording:

> The file with the older modification time will be marked as the conflicting
> file and thus be renamed. If the modification times are equal, the file
> originating from the device which has the larger value of the first 63 bits
> for its device ID will be marked as the conflicting file.
> — Syncthing, "Syncing," docs.syncthing.net,
> https://docs.syncthing.net/users/syncing.html (accessed 2026-09-08)

SIGNAL: a conflict as a separate class of pain — 7 tasks in syncthing, `(sync.?conflict|conflict.*(file|copy|resolv|merge|winner|handling))`; a share of 0.5% against 0.0% in opencue (834 tasks, not one). The count is taken over the `collab` corpus (`python3 src/scripts/tracker-rank.py --query '(sync.?conflict|conflict.*(file|copy|resolv|merge|winner|handling))' --corpus collab`, dumps from 08.09.2026: syncthing 1528 records — a PARTIAL export, opencue 834 — a full one). **The scope of the count is named deliberately: the full export of syncthing (5946 tasks) was not complete on that date** — without a GitHub token, GitHub gives 60 requests per hour — and when it appears the number will be recounted. The lesson of wave 3 about the mobility of numbers is applied here in advance: exactly the claim that names what it was counted on survives. The contrast is explained by the design, not by popularity: a render farm has no divergences by construction — a job is immutable, and no one edits it in two hands. The heaviest task of the class is #1746 "External conflict handling tool (to merge, or select winner)," opened 01.05.2015, 21 comments, 15 reactions, still open; the loudest is #2067 about hundreds of thousands of files renamed after an update.

## How it fits Kinotrain

### What already exists

- `src/apps/kt-app/src/app.rs:5036` — `write_project_guarded`: the guard of ONE writer. It compares the model's revision in its memory and does not let the background autosave overwrite a more recent Ctrl+S. This is a tie-break inside one process — and it is honest.
- `src/apps/kt-app/src/app.rs:5051` — `write_project_atomic`: a tmp beside it plus `rename`. The write is not torn in the middle; the question "whose write" it does not ask.
- `src/libs/kt-media/src/index.rs:46` — the only place in the whole tree where the edit time of a FILE is read (`md.modified()`), and it is read about the material: size and mtime make up the identity of media in the sidecar DB. That is, we already know half of Syncthing's rule — but not for the project.
- `src/apps/kt-app/src/app.rs:5080` — `chrono_like_now()`: the only wall clock in the application. It gives "HH:MM:SS" WITHOUT A DATE and is used only for the lines of the notification log (`app.rs:5004`, `app.rs:1983`) — two entries cannot be compared by it even in theory.

### What is missing

- Reading the time of the project file. Across the whole `kt-app` there is not a single `md.modified()` and not a single watcher: an external change of the file is detected by nothing, and the background autosave targets the user's file every 30 s (`src/apps/kt-app/src/app.rs:1078`).
- Saving the loser. Even knowing of the divergence, we would have nowhere to save it: no name, no rule, no place.
- The name of whoever edited. There is no concept of a user either in the model or in the format; entity identifiers are issued as "maximum + 1" (`src/apps/kt-app/src/app.rs:3110`) and coincide for two people. The `-<who>` tail of a file name is nowhere for us to take today.
- A second writer under discipline. `kt-cli` writes `.otio` with a bare `std::fs::write` (lines 146, 177, 192, 222) — without a guard, without tmp+rename, and without a single question about what was there.

### What gets in the way

Nothing expensive: reading the mtime before writing, comparing with the value remembered on opening, and renaming the loser — these are file operations beside the already existing project record. What gets in the way is the same as in F-COL-013: all of today's save discipline is built on the assumption "exactly one writer," and a rule done halfway is worse than none — to detect someone else's edit and not save the loser means naming the loss without preventing it.

ADDRESS: src/apps/kt-cli/src/main.rs:177 — the class of pain that a foreign tracker calls "conflict files" is expressed for us by a line without a single check: the same `convert` in `kt-cli` writes `.otio` over an existing one, reading nothing. With Syncthing the loser is renamed and lies beside; with us it simply ceases to exist, and there is no class of tasks about this, because there is nothing with which to notice it.

### In the `collab/` category — two mandatory lines (rule 13)

OFFLINE: DEGENERATES — `src/apps/kt-cli/src/main.rs:177` — our tie-break already works on a single machine and degenerates into a silent loss: the winner is not whoever edited later, but whoever invoked the write later, and the loser is not renamed but simply stops existing; `convert` writes a document over an existing file with a bare `fs::write` — no tmp+rename, no revision guard, no question about what was there — even though the second writer here is not hypothetical but our own.

NETWORK COST: to say whose edit came later — shared clock — the edit time is taken from the file system, and on a shared volume it is set not by the one who writes; the Syncthing rule therefore completes totality with the device-ID bits, while we have neither comparable clocks (the app's only wall clock gives the time of day without a date and serves the notification log) nor a stable machine name, so we can name the winner only by the order of calls within our own process.

### Which invariants it touches

Of the twelve — none: everything lives in `kt-app` and `kt-cli` next to the project write, and does not touch the engine, the time model, the cache, or export. Of the checklist rules in `src/apps/kt-app/CLAUDE.md`, two are touched. Rule 16: a detected divergence is an impossibility of the action, and it must be shown as an error with a cause and an address ("the file on disk was changed not by us; your version is saved next to it as …"), not as a faded status line. Rule 12: the background autosave every 30 s is exactly the writer that today wins races nobody asked the person to have.

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage: remember the file's identity on open, check it before writing, keep the loser under a label name, and report it with an error |
| Where the main work is | kt-app (`open`, `save`, the autosave stream, `on_exit`), kt-cli (the same write path instead of `fs::write`) |
| Project format | not touched: the label name is a property of the file, not of the document |
| Tests to write | a file replaced on disk is not silently overwritten; the loser is kept next to it and opens as a project; repeated saving does not produce a second label; `kt-cli` obeys the same rule as `kt-app` |
| Depends on | F-COL-013 (declaration next to the file): the lock prevents, the tie-break catches the remainder — that order is exactly right |
| Breaks compatibility | no: the loser file is an ordinary `.otio`; an older version or a foreign program simply sees one more project in the folder |

## Verdict

We take the local half, and it is almost all local. Losing already happens to us — between two `kt-app` instances, between `kt-app` and `kt-cli`, between the background autosave and an external edit of the file — and it is never named, because it is not noticed: across all of `kt-app` there is not a single read of a file's time, not a single watcher, and the app's only wall clock gives the time of day without a date and serves the notification log. Keeping the loser next to the winner under a name with date and time is a file-system operation; it requires no second participant.

We do not add the server half per the formula of F-PRJ-003 and F-PRJ-001: for the loser's name to answer "who", a user account is needed, and there is no participant. The `-<who>` suffix we cannot fill today, and we will not pretend otherwise: a name without a "who" is still valuable — it answers "when" and preserves the work — while "who" is attributed by a person.

One lesson we take directly from someone else's tracker, without deferring: naming produces FILES, not messages, and they spread. So the cleanup rule (how many losers to keep and where) is set up together with the naming itself, not later — otherwise we repeat #2067 in miniature.

Boundaries: not letting divergence happen at all — F-COL-013 (declaration next to the file) and `collab/claim/` as a whole; why nobody has merging — F-COL-029; a surface set up for a nonexistent conflict resolution — F-COL-030; how to show the difference between two versions when both files are at hand — F-COL-031; autosave over the file or next to it — F-RES-001; atomicity of the write — F-RES-002.

## Sources

- https://docs.syncthing.net/users/syncing.html — the rule for choosing the loser, the conflict file name, and "treated as normal files" (access date 2026-09-08).
- https://github.com/syncthing/syncthing/issues/1746 — "External conflict handling tool (to merge, or select winner)", created 01.05.2015, open, 21 comments, 15 reactions (metadata checked against the `syncthing` dump of 08.09.2026).
- https://github.com/syncthing/syncthing/issues/2067 — mass renaming after an update, 16.07.2015 (same source).
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration — locking of bins and timelines instead of a tie-break (access date 2026-09-08).
- https://support.apple.com/en-us/HT207128 — exclusive lock of the Final Cut library (access date 2026-09-08).
- The VEGAS and After Effects rows are marked `[NOT VERIFIED]` directly in the table.
- Our code read on 08.09.2026: `src/apps/kt-app/src/app.rs` (1078, 1983, 3110, 5004, 5036, 5051, 5080), `src/apps/kt-cli/src/main.rs` (146, 177, 192, 222), `src/libs/kt-media/src/index.rs` (46). The absence of file-mtime reads in `kt-app` was checked by grep for `modified()|mtime|SystemTime` — the only match is `chrono_like_now`.
