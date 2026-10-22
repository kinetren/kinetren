---
id: F-COL-034
title: A document revision number that the file does not have
status: researched
priority: B
cost: M
category: collab/merge
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, otio-lib-0.18]
we_have: partial
crates: [kt-otio, kt-app, kt-model]
invariants: []
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-RES-003, F-COL-013]
trackers: []
services: [team-projects, resolve-project-server, syncthing]
checked: 2026-09-08
---

# F-COL-034 — A document revision number that the file does not have

## Essence

The question with which any reconciliation begins: "your edit was made FROM WHICH state of the document?" Without an answer, two versions are simply two files, and one can choose between them only as a whole. The answer requires a number that grows with every edit and travels together with the document. No editor has such a number for the editor's work: what is called "the project version" is the version of the FORMAT or the version of the PROGRAM, while the version of the document itself lives in the server's database and does not go outside.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | the file has a format version and that is all; "which of two is newer" is decided by the file's modification time in Explorer |
| Adobe Premiere | 26.x | Version History (**only** Team Projects) | a file-based project has no revisions; a revision appears exactly when the document moves to the server — then versions become objects, they can be seen as a list, and from any of them a new project can be made. In Productions, where there is no server, there is no revision either: there the lock sits beside the file |
| DaVinci Resolve | 21 | a revision inside the project database (PostgreSQL) | the document is the rows of the database, so the question "which database edit is it from" is answered by the form of storage itself; export to `.drp` loses this half — the file again becomes merely a snapshot |
| Final Cut Pro | 12.3 | none | the library is occupied as a whole, so divergence by construction does not happen and a number to count from is not needed `[NOT VERIFIED: there is no contrary statement in Apple's documentation either]` |
| After Effects | 26.x | Version History (only Team Projects) | inherits someone else's model whole: a `.aep` has no revisions, a document on the server does |

**Outside the core.** Two rows: the only place where a document's revision is formally described, and a format for which the version is certainly not a revision.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Syncthing | 2.x | version vector | a revision exists, and it is formal — a counter per device — and it is NOT stored in the file: it travels in the index messages of the protocol alongside the name and the folder |
| OpenTimelineIO | 0.18.x | schema version + family/label | a document has two versions, and both concern the format: `Clip.2`, `Marker.2` — the schemas of objects; `FAMILY`/`LABEL` — studio compatibility. There is no document-state number at any level |

## The trick behind it

**1. No one stores a document's revision IN the document — including those for whom it is formal.** Syncthing, the only system with a described outcome of divergence, keeps a version vector in the index and passes it in protocol messages: there is not a single byte about it in the file itself; identity is "folder + name", and the history lies beside it. Adobe shows the version history only for a document that lives on a server. Resolve answers the question with the form of storage — the project is the database. Hence an unexpected conclusion that overturns the statement of the task: **"add a revision number to the format" is not half of the solution but zero**; a number makes sense only insofar as something exists that issues it monotonically, and a file copied to a flash drive guarantees monotonicity in no way.

**2. Hence the observable asymmetry in the products: the version history appears together with the server and disappears together with it.** Premiere's file-based project has neither a revision nor a history; in Team Projects it has both, up to "create a new project from a version." The feature is not "a road under development" — it is simply inexpressible without a store that keeps the count.

**3. Our case is stronger and worse at the same time: the format version is computed from the CONTENT and can go DOWN.** F-RES-003 verified this by experience: the cameras and automation were removed — and the file was rewritten as `kt.v = 1`. A wonderful property for compatibility and a verdict for the role of a revision: from two of our files one cannot say which is later, even approximately. We have a number in the file, but it is not monotonic and is not about the document.

**4. And the most unexpected: a consumer of the revision ALREADY WORKS for us.** `write_project_guarded` (`src/apps/kt-app/src/app.rs:5036`) before writing compares the model's revision with the revision of the last written snapshot and refuses to write a stale one. This is exactly the mechanism by which databases compare edit logs — only the number lives in a `Mutex<u64>` in the process's memory, starts from zero on every opening and does not get into the file. What is missing is not the idea and not the code, but the carrier.

## Pros

- A revision in the store (rather than in the file) does not lie: one side keeps the count, so "newer" is a fact, not a guess about the clock of someone else's machine.
- A version as an object ("open the version," "make a project from the version") gives the user an understandable model without a single word about merging — one chooses whole, and that is honest.
- A format version computed from the content (our case) gives compatibility for free: a file with no new capabilities opens in an old build.
- The absence of a revision in the file makes the document portable: it does not remember where it came from, and so it opens identically on any machine.

## Cons

- Without a revision, two files are comparable only as a whole: "take mine" or "take theirs" — the construction offers no third outcome.
- A revision living in the store dies on export: the `.drp` from Resolve and a project copy from Team Projects are again snapshots without history.
- A format version that goes down breaks the intuition that "bigger means newer" — and it cannot be occupied by a revision, even as a temporary solution.
- A number in the process's memory (our case) protects against a second writer INSIDE the program and knows nothing about the file on disk.

## What users say

Adobe turns on the version history there, and only there, where the document has become a server document:

> You can now access your Team Project's Version and Auto Save History by selecting
> the Team Project name in the Premiere Pro header bar.
> — Abhi Singh (Adobe), 03.10.2023,
> https://community.adobe.com/t5/premiere-pro-beta-discussions/now-in-beta-updated-version-and-auto-save-history-in-team-projects/td-p/14128940

And where revisions ought to be compared, the window has been empty for four years — confirmed by a second user in 2024:

> When trying to share a team-project, I get a window that says 'resolve conflicts'
> but the window itself is empty.
> — devGuru411, 26.08.2020,
> https://community.adobe.com/t5/team-projects-discussions/premiere-pro-team-project-resolve-conflicts-empty-window/td-p/11387956

## How it fits Kinotrain

### What already exists

- **Two revisions in memory and not one on disk.** `model_rev`
  (`src/apps/kt-app/src/app.rs:327`) grows with every edit
  (`:1992`, `:2082`, `:2102`) and serves as the key of the drawing caches; `saved_rev`
  (`:216`, `Arc<Mutex<u64>>`) — the guard of the autosave. Both are reset when the file is opened and do not get into the document.
- **The revision guard is already written and already saves work**: `write_project_guarded`
  (`src/apps/kt-app/src/app.rs:5036`) does not write a snapshot if a newer one has landed on disk; the occasion was a real loss by Ctrl+S to the background autosave.
- **The only version the file carries is the schema version**: the root of the document is written as `Timeline.1` (`src/libs/kt-otio/src/schema.rs:576`), objects as `Clip.2`, `Marker.2` (`:490`, `:522`); plus the version of our envelope
  `kt.v` (`src/libs/kt-otio/src/meta.rs`, `KT_META_VERSION = 5`), which is computed from the content and can decrease (F-RES-003).
- **The document does have a companion, but by design it is disposable**: `.ktindex.db`
  lies beside the project and on any opening error is deleted and created anew (`src/libs/kt-media/src/index.rs`). Storing there anything that must not be lost is forbidden by construction.
- **An external change of the project file is detected by nothing**: across the whole `kt-app` there is not a single read of the mtime and not a single watcher, and the background autosave targets the user's file every 30 s (`src/apps/kt-app/src/app.rs:1078`).

OFFLINE: DOES NOT REMAIN — `src/libs/kt-otio/src/schema.rs:576` — nothing of the document's revision remains, and the place where this is visible is the root of the file: everything we write into it about versions is a schema constant, and our second version, `kt.v`, is computed from the content and can decrease. Today the editor decides "which of two files is later" by the modification time in the file manager, and the program does not trust this time and does not look at it even once.

NETWORK COST: to say from which state an edit was made — project-as-database — the monotonicity of the number is ensured not by a field in the file but by the side that issues it: a copy on a flash drive continues the count independently, and two documents with revision 41 cease to be comparable.

NETWORK COST: to choose the more recent of two copies — shared clock — with equal numbers the industry decides by the edit time, and we have neither a second time nor trust in our own: the mtime of the project file is read nowhere.

### What is missing

- A monotonic counter of the document that survives saving. A place for it exists and costs nothing: `metadata["kt"]` tolerates unknown fields (passthrough, F-RES-003), so an addition does not bump the format version and does not break old builds.
- A reading of the project file's mtime before writing — the cheapest half of the answer to "the file on disk has become not mine."
- An answer to the question "what changed between two states": a revision says "different," but does not say "how" — that is already F-COL-036.

### What gets in the way

Not code, but meaning. A number issued by the document itself is monotonic only within one line of saves; two people get identical numbers on the very first day — exactly as with `ClipId` (F-COL-033). So a revision in the file honestly solves exactly one task — "this file on disk is not the one I read from" — and does NOT solve the task of comparing two lines of editing; writing it while promising the second means introducing a field no one will be able to trust. The second obstacle is mundane: our own autosave is a second writer, and the revision will grow from its writes too, unless "a person's edit" and "a save" are separated.

### Which invariants it touches

None of the twelve: the counter computes nothing in the frame and touches neither time nor sound. This is a strong argument for "cheap" — and at the same time a reason not to inflate it: everything that can be done here lives in kt-app and in the `kt` envelope.

## Estimate

| What | Answer |
|---|---|
| Size | M — a counter in the `kt` envelope, a read of the mtime before writing, a question to the user instead of a silent overwrite |
| Where the main work is | kt-app (the write guard, the dialog), kt-otio (the field in `metadata["kt"]`), kt-model (what counts as an edit) |
| Project format | no version bump is needed: an unknown envelope field survives the round trip verbatim (F-RES-003), and old builds read the file as before |
| Tests to write | "the revision grows from an edit and does not grow from a save"; "a file changed from outside is not silently overwritten"; a round trip: the field survives an old build |
| Depends on | F-RES-003 (the format version and passthrough), F-COL-013 (a guard beside the file), F-RES-001 (how many states are on disk) |
| Breaks compatibility | no |

## Verdict

We take one half and honestly name the second as inexpressible. The half that works alone: **a monotonic document counter in our envelope plus a reading of the mtime before writing** — this is the answer to the question "the file on disk is no longer the one I read from," and it is needed today already, because we have a second writer without any network (kt-cli, autosave, a second window). It costs one field, which the format will swallow without a version bump, and one question to the user instead of a silent overwrite.

The second half — "from which base is your edit" — is not taken, and the reason is constructive, not organisational: a number is monotonic only where it is issued by one side, and a file has no such side anywhere in the industry. To record it as an unfinished item is impossible — this is a property of a file-based document, and Adobe and Blackmagic pay for getting around it with the same thing: the document ceases to be a file.

How we differ from neighbours. **F-RES-003** (a project created by another version of the program) owns the FORMAT version: what we understand, what we refuse to read, how passthrough works; its conclusion "our version goes down" is used here as a ready fact — and it is precisely this that closes the temptation to occupy `kt.v` as a revision. **F-COL-013** (two processes over one file) owns the lock beside the file; here the subject is the number INSIDE the file, and it is needed even where there is no lock. **F-PRJ-002** (history of edits and project versions) concerns named states and a history panel for one person; a revision is neither a name nor a step of history, but the address of a state. **F-COL-036** concerns how an edit is described once the revisions have been compared.

## Sources

- https://community.adobe.com/t5/premiere-pro-beta-discussions/now-in-beta-updated-version-and-auto-save-history-in-team-projects/td-p/14128940
  — version history for a document on a server (03.10.2023; accessed 2026-09-08)
- https://community.adobe.com/t5/team-projects-discussions/premiere-pro-team-project-resolve-conflicts-empty-window/td-p/11387956
  — the empty `Resolve Conflicts` window (26.08.2020, confirmed 16.12.2024; accessed 2026-09-08)
- https://docs.syncthing.net/specs/bep-v1.html — "The version field is a version
  vector describing the updates performed to a file by all members in the cluster";
  the vector travels in index messages, not in the file (accessed 2026-09-08)
- `docs/features/resilience/durability/project-format-migration.md` (F-RES-003) —
  a fact verified by experience: our format version is computed from the content and can
  go down
- Our code was checked by grep over `src/` on 08.09.2026: `kt-app/src/app.rs`
  (`model_rev`, `saved_rev`, `write_project_guarded`, the background autosave, the absence of an mtime read), `kt-otio/src/schema.rs` (`encode_timeline`, schema tags), `kt-otio/src/meta.rs` (`KT_META_VERSION`), `kt-media/src/index.rs` (the disposability of the sidecar).
